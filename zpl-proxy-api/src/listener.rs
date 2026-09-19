//! HTTP listeners, including systemd's native descriptor-passing protocol.

use std::{io, net::SocketAddr};

use axum::Router;
use eyre::{bail, Context};

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = false)]
pub struct ListenOptions {
    /// Bind a TCP listener directly (standalone mode).
    #[arg(long)]
    bind_addr: Option<SocketAddr>,
    /// Bind a Unix stream socket directly; its parent directory must exist.
    #[cfg(unix)]
    #[arg(long)]
    unix_socket: Option<std::path::PathBuf>,
    /// Use exactly one TCP or Unix listener passed by systemd.
    #[cfg(unix)]
    #[arg(long)]
    socket_activation: bool,
}

pub struct Listener {
    socket: Socket,
    #[cfg(unix)]
    cleanup: Option<SocketPath>,
}

enum Socket {
    Tcp(std::net::TcpListener),
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixListener),
}

impl Listener {
    /// Call before starting threads: listenfd consumes activation environment
    /// variables, which must not race with concurrent environment access.
    pub fn open(options: &ListenOptions) -> eyre::Result<Self> {
        #[cfg(unix)]
        if options.socket_activation {
            return Self::activated();
        }
        #[cfg(unix)]
        if let Some(path) = &options.unix_socket {
            // Never unlink an existing path: it may belong to another listener.
            let listener = std::os::unix::net::UnixListener::bind(path)
                .wrap_err("bind Unix socket (remove a stale socket explicitly)")?;
            let cleanup = SocketPath::new(path.clone())?;
            listener.set_nonblocking(true)?;
            return Ok(Self {
                socket: Socket::Unix(listener),
                cleanup: Some(cleanup),
            });
        }
        let address = options
            .bind_addr
            .ok_or_else(|| eyre::eyre!("no HTTP listener selected"))?;
        let listener = std::net::TcpListener::bind(address).wrap_err("bind TCP listener")?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            socket: Socket::Tcp(listener),
            #[cfg(unix)]
            cleanup: None,
        })
    }

    #[cfg(unix)]
    fn activated() -> eyre::Result<Self> {
        // Require the native systemd protocol, not listenfd's no-PID extension.
        // https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html
        let pid = std::env::var("LISTEN_PID")
            .ok()
            .and_then(|v| v.parse::<u32>().ok());
        let count = std::env::var("LISTEN_FDS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok());
        if pid != Some(std::process::id()) || count != Some(1) {
            bail!("socket activation requires LISTEN_PID matching this process and LISTEN_FDS=1");
        }
        // Native systemd always starts at fd 3; disable listenfd's alternate-fd extension.
        std::env::remove_var("LISTEN_FDS_FIRST_FD");
        let mut descriptors = listenfd::ListenFd::from_env();
        // Failed type checks leave the descriptor available for another type.
        // https://docs.rs/listenfd/1.0.2/listenfd/struct.ListenFd.html#method.take_tcp_listener
        let socket = match descriptors.take_tcp_listener(0) {
            Ok(Some(listener)) => {
                listener.set_nonblocking(true)?;
                Socket::Tcp(listener)
            }
            _ => {
                let listener = descriptors
                    .take_unix_listener(0)
                    .wrap_err("activation descriptor must be a TCP or Unix stream listener")?
                    .ok_or_else(|| eyre::eyre!("missing activation descriptor"))?;
                listener.set_nonblocking(true)?;
                Socket::Unix(listener)
            }
        };
        // The socket unit owns its filesystem path, including across restarts.
        Ok(Self {
            socket,
            cleanup: None,
        })
    }

    pub async fn serve(self, app: Router) -> io::Result<()> {
        let Self {
            socket,
            #[cfg(unix)]
                cleanup: _cleanup,
        } = self;
        // Axum supports both Tokio listener types directly, preserving the same
        // routing, middleware, and graceful shutdown semantics for each transport.
        // https://docs.rs/axum/0.8.9/axum/serve/trait.Listener.html
        match socket {
            Socket::Tcp(listener) => {
                axum::serve(tokio::net::TcpListener::from_std(listener)?, app)
                    .with_graceful_shutdown(zpl_proxy_api::telemetry::shutdown_signal())
                    .await
            }
            #[cfg(unix)]
            Socket::Unix(listener) => {
                axum::serve(tokio::net::UnixListener::from_std(listener)?, app)
                    .with_graceful_shutdown(zpl_proxy_api::telemetry::shutdown_signal())
                    .await
            }
        }
    }
}

#[cfg(unix)]
struct SocketPath {
    path: std::path::PathBuf,
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl SocketPath {
    fn new(path: std::path::PathBuf) -> io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::symlink_metadata(&path)?;
        Ok(Self {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
}

#[cfg(unix)]
impl Drop for SocketPath {
    fn drop(&mut self) {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        // Do not remove a file that replaced our socket while we were running.
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path) {
            if metadata.file_type().is_socket()
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
            {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_tcp_binds_and_accepts() {
        let listener = Listener::open(&ListenOptions {
            bind_addr: Some("127.0.0.1:0".parse().unwrap()),
            #[cfg(unix)]
            unix_socket: None,
            #[cfg(unix)]
            socket_activation: false,
        })
        .unwrap();
        let Socket::Tcp(socket) = listener.socket else {
            panic!("expected TCP listener")
        };
        let _client = std::net::TcpStream::connect(socket.local_addr().unwrap()).unwrap();
        // A completed client connect need not make the nonblocking listener
        // immediately readable on macOS. Wait for acceptance in this test;
        // production keeps the descriptor nonblocking for Tokio.
        socket.set_nonblocking(false).unwrap();
        socket.accept().unwrap();
    }
}
