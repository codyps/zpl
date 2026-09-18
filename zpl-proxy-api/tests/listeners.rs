//! Exercise actual descriptor inheritance in a child, without mutating the test
//! runner's environment or requiring systemd on the development machine.
//! https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html
#![cfg(unix)]

use std::{
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, Stdio},
    time::Duration,
};

use diesel::{connection::SimpleConnection, Connection};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Process {
    async fn stop(&mut self) {
        // SAFETY: the owned child has not been reaped; kill only sends SIGTERM.
        assert_eq!(
            unsafe { libc::kill(self.0.id() as libc::pid_t, libc::SIGTERM) },
            0
        );
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(status) = self.0.try_wait().unwrap() {
                    assert!(status.success(), "proxy shutdown failed: {status}");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("proxy did not shut down gracefully");
    }
}

fn directory() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("zpl-listener-")
        .tempdir()
        .unwrap()
}

fn configure(command: &mut Command, dir: &tempfile::TempDir) {
    let database = dir.path().join("db.sqlite");
    let mut connection = diesel::SqliteConnection::establish(database.to_str().unwrap()).unwrap();
    for migration in [
        include_str!("../migrations/2024-10-03-035443_cache-results/up.sql"),
        include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql"),
        include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
        include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
    ] {
        connection.batch_execute(migration).unwrap();
    }
    command
        .args(["--zd621-url", "http://127.0.0.1:9/"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("DATABASE_URL", database)
        .env("OTEL_TRACES_EXPORTER", "none")
        .env("TOKIO_WORKER_THREADS", "2")
        .stdin(Stdio::null())
        .stdout(Stdio::null());
}

fn activation(fd: i32) -> Command {
    // The shell exports its own PID then execs, preserving PID and descriptor 3.
    // No environment mutation or allocation runs in the post-fork closure.
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "export LISTEN_PID=$$; exec \"$@\"", "activate"])
        .arg(env!("CARGO_BIN_EXE_zpl-proxy-api"))
        .arg("--socket-activation")
        .env("LISTEN_FDS", "1");
    unsafe {
        // SAFETY: fd remains owned by the caller until spawn returns. dup2 and
        // fcntl are async-signal-safe and transfer a single descriptor to the child.
        command.pre_exec(move || {
            if libc::dup2(fd, 3) == -1 || libc::fcntl(3, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command
}

async fn get(client: &reqwest::Client, url: &str, process: &mut Process) -> reqwest::Response {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(
                process.0.try_wait().unwrap().is_none(),
                "proxy exited before serving HTTP"
            );
            if let Ok(response) = client.get(url).send().await {
                return response;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("HTTP listener did not become ready")
}

fn tcp_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap()
}

#[tokio::test]
async fn inherited_tcp_listener_serves_http_and_preserves_validation() {
    let dir = directory();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let mut command = activation(listener.as_raw_fd());
    configure(&mut command, &dir);
    let mut process = Process(command.spawn().unwrap());
    let client = tcp_client();
    assert!(get(&client, &url, &mut process)
        .await
        .text()
        .await
        .unwrap()
        .contains("<html>"));
    let response = client
        .post(format!("{url}api/zpl-zd621"))
        .json(&serde_json::json!({"zpl": "^XA^WD*:*.*^XZ"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    process.stop().await;
}

#[tokio::test]
async fn inherited_unix_listener_serves_http_and_keeps_systemd_owned_path() {
    let dir = directory();
    let path = dir.path().join("http.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let mut command = activation(listener.as_raw_fd());
    configure(&mut command, &dir);
    let mut process = Process(command.spawn().unwrap());
    let client = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(path.as_path())
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();
    assert!(get(&client, "http://localhost/", &mut process)
        .await
        .text()
        .await
        .unwrap()
        .contains("<html>"));
    process.stop().await;
    assert!(path.exists(), "the socket unit owns this path");
}

#[tokio::test]
async fn standalone_unix_listener_serves_http_and_removes_its_path_on_shutdown() {
    let dir = directory();
    let path = dir.path().join("http.sock");
    let mut command = Command::new(env!("CARGO_BIN_EXE_zpl-proxy-api"));
    configure(&mut command, &dir);
    command.arg("--unix-socket").arg(&path);
    let mut process = Process(command.spawn().unwrap());
    let client = reqwest::Client::builder()
        .no_proxy()
        .unix_socket(path.as_path())
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();
    assert!(get(&client, "http://localhost/", &mut process)
        .await
        .text()
        .await
        .unwrap()
        .contains("<html>"));
    process.stop().await;
    assert!(!path.exists());
}

#[test]
fn standalone_unix_listener_does_not_remove_an_existing_file() {
    let dir = directory();
    let path = dir.path().join("keep");
    std::fs::write(&path, b"do not remove").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_zpl-proxy-api"))
        .args(["--zd621-url", "http://127.0.0.1:9/", "--unix-socket"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("bind Unix socket"));
    assert_eq!(std::fs::read(&path).unwrap(), b"do not remove");
}

#[test]
fn activation_rejects_missing_wrong_pid_multiple_and_wrong_type_descriptors() {
    let binary = env!("CARGO_BIN_EXE_zpl-proxy-api");
    for count in [None, Some("1"), Some("2"), Some("invalid")] {
        let mut command = Command::new(binary);
        command
            .args(["--zd621-url", "http://127.0.0.1:9/", "--socket-activation"])
            .env("LISTEN_PID", "0")
            .env_remove("LISTEN_FDS");
        if let Some(count) = count {
            command.env("LISTEN_FDS", count);
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("LISTEN_FDS=1"));
    }
    let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut command = activation(udp.as_raw_fd());
    command.args(["--zd621-url", "http://127.0.0.1:9/"]);
    let result = command.output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("TCP or Unix stream listener"));
    // Correct PID, but multiple passed descriptors: reject rather than choosing one.
    let result = command.env("LISTEN_FDS", "2").output().unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("LISTEN_FDS=1"));
}

#[test]
fn exactly_one_listener_mode_is_required() {
    for args in [
        vec![],
        vec![
            "--bind-addr",
            "127.0.0.1:3000",
            "--unix-socket",
            "/tmp/unused.sock",
        ],
        vec!["--bind-addr", "127.0.0.1:3000", "--socket-activation"],
        vec!["--unix-socket", "/tmp/unused.sock", "--socket-activation"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_zpl-proxy-api"))
            .args(["--zd621-url", "http://127.0.0.1:9/"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
    }
}
