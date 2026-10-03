use clap::{Parser, Subcommand};
use eyre::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(
    version,
    about = "Inspect and update network Zebra printers using local firmware files"
)]
struct Args {
    /// Printer IP and raw TCP port (usually 9100); IPv6: [address]:9100.
    #[arg(long)]
    printer: SocketAddr,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read the model, serial number, and installed firmware using SGD.
    Inspect,
    /// Preview an update; only --execute sends the firmware.
    Apply {
        /// Extracted firmware .zpl file from Zebra, not a ZIP archive.
        firmware: PathBuf,
        #[arg(long)]
        model: String,
        #[arg(long)]
        serial: String,
        /// Exact expected appl.name value after installation.
        #[arg(long)]
        version: String,
        /// Optional expected SHA-256 of the extracted file.
        #[arg(long)]
        sha256: Option<String>,
        /// Send firmware now. The printer may reboot; keep power connected.
        #[arg(long)]
        execute: bool,
        /// Maximum seconds to wait for the expected firmware after upload.
        #[arg(long, default_value = "600", value_parser = clap::value_parser!(u64).range(1..=3600))]
        wait_seconds: u64,
    },
}

#[derive(Debug)]
struct Identity {
    model: String,
    serial: String,
    version: String,
}

fn remaining(deadline: Instant) -> Result<Duration> {
    let duration = deadline.saturating_duration_since(Instant::now());
    ensure!(!duration.is_zero(), "operation timed out");
    Ok(duration)
}

fn connect(address: SocketAddr, deadline: Instant) -> Result<TcpStream> {
    Ok(TcpStream::connect_timeout(
        &address,
        remaining(deadline)?.min(Duration::from_secs(5)),
    )?)
}

// Shared bounded SGD reads; firmware uploads still use the guarded path below.
use zebra_sgd::get_required as getvar;

fn identity(stream: &mut TcpStream, deadline: Instant) -> Result<Identity> {
    Ok(Identity {
        model: getvar(stream, "device.product_name", deadline)?,
        serial: getvar(stream, "device.unique_id", deadline)?,
        version: getvar(stream, "appl.name", deadline)?,
    })
}

fn check_identity(info: &Identity, model: &str, serial: &str) -> Result<()> {
    ensure!(
        info.model == model && info.serial == serial,
        "printer identity mismatch: expected {model:?} / {serial:?}, received {:?} / {:?}",
        info.model,
        info.serial
    );
    Ok(())
}

fn firmware_bytes(path: &PathBuf, expected_hash: Option<&str>) -> Result<Vec<u8>> {
    ensure!(
        path.extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("zpl")),
        "use the extracted Zebra .zpl firmware file"
    );
    let file = File::open(path).wrap_err("opening firmware")?;
    ensure!(
        file.metadata()?.is_file(),
        "firmware must be a regular file"
    );
    // Keep one bounded snapshot so preview/hash and upload refer to identical bytes.
    const MAX: u64 = 128 * 1024 * 1024;
    let mut bytes = Vec::new();
    file.take(MAX + 1).read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= MAX,
        "firmware must be 1 byte to 128 MiB"
    );
    ensure!(
        !bytes.starts_with(b"PK"),
        "ZIP archive detected; extract the firmware first"
    );
    let hash: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if let Some(expected) = expected_hash {
        ensure!(
            expected.eq_ignore_ascii_case(&hash),
            "firmware SHA-256 mismatch (actual {hash})"
        );
    }
    println!(
        "Firmware: {} ({} bytes)\nSHA-256: {hash}",
        path.display(),
        bytes.len()
    );
    Ok(bytes)
}

fn run(args: Args) -> Result<()> {
    // Validate the local input before contacting a printer.
    let bytes = match &args.command {
        Command::Apply {
            firmware, sha256, ..
        } => Some(firmware_bytes(firmware, sha256.as_deref())?),
        Command::Inspect => None,
    };
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut stream = connect(args.printer, deadline).wrap_err("connecting to printer")?;
    let info = identity(&mut stream, deadline).wrap_err("reading printer identity")?;
    println!(
        "Printer: {}\nModel: {}\nSerial: {}\nInstalled firmware: {}",
        args.printer, info.model, info.serial, info.version
    );
    let Command::Apply {
        model,
        serial,
        version,
        execute,
        wait_seconds,
        ..
    } = args.command
    else {
        return Ok(());
    };
    check_identity(&info, &model, &serial)?;
    ensure!(
        !version.trim().is_empty(),
        "expected version must not be empty"
    );
    println!("Requested firmware: {version}");
    if info.version == version {
        println!("Already running the requested firmware; nothing sent.");
        return Ok(());
    }
    if !execute {
        println!("Preview only; nothing sent. Review firmware compatibility and repeat with --execute to apply.");
        return Ok(());
    }
    eprintln!("Sending firmware once. Keep printer power connected and other print jobs stopped.");
    // Zebra distributes firmware as a printer-ready .zpl download:
    // https://docs.zebra.com/us/en/printers/industrial/zt411-zt421-industrial-printer-user-guide/c-zt4x1-setup/r-upgrading-the-printer-firmware/t-uploading-the-latest-firmware.html
    // Raw TCP default port: https://techdocs.zebra.com/link-os/2-14/pc/content/com/zebra/sdk/comm/connectionbuilder
    // Reuse the inspected connection. Never retry a partial/ambiguous upload.
    let deadline = Instant::now() + Duration::from_secs(120);
    let upload = (|| -> Result<()> {
        let mut pending = bytes.as_ref().unwrap().as_slice();
        while !pending.is_empty() {
            stream.set_write_timeout(Some(remaining(deadline)?))?;
            match stream.write(&pending[..pending.len().min(64 * 1024)]) {
                Ok(0) => bail!("printer closed the upload stream"),
                Ok(written) => pending = &pending[written..],
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        stream.shutdown(Shutdown::Write)?;
        Ok(())
    })();
    drop(stream);
    upload.wrap_err("upload outcome uncertain; firmware will NOT be resent. Inspect the printer before taking further action")?;
    eprintln!("Upload sent; waiting for matching identity and firmware version.");
    let deadline = Instant::now() + Duration::from_secs(wait_seconds);
    let mut last = String::from("printer has not responded");
    while Instant::now() < deadline {
        thread::sleep(
            Duration::from_secs(2).min(deadline.saturating_duration_since(Instant::now())),
        );
        let probe_deadline = deadline.min(Instant::now() + Duration::from_secs(10));
        match connect(args.printer, probe_deadline)
            .and_then(|mut s| identity(&mut s, probe_deadline))
        {
            Ok(current) => {
                check_identity(&current, &model, &serial)
                    .wrap_err("verification failed; firmware will NOT be resent")?;
                if current.version == version {
                    println!("Verified: {model} / {serial} reports firmware {version}.");
                    return Ok(());
                }
                last = format!("printer still reports {}", current.version);
            }
            Err(error) => last = error.to_string(),
        }
    }
    bail!("upload sent but installation NOT verified: {last}. Firmware will NOT be resent; inspect the printer before taking further action")
}

fn main() -> Result<()> {
    run(Args::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn rejects_malformed_unsupported_and_unbounded_sgd_responses() {
        // SGD responses must be bounded quoted values, per getvar reference above.
        for response in [
            b"garbage".to_vec(),
            b"\"?\"".to_vec(),
            b"\"\"".to_vec(),
            vec![b'\n'; 4096],
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = vec![0; b"! U1 getvar \"appl.name\"\r\n".len()];
                stream.read_exact(&mut request).unwrap();
                let _ = stream.write_all(&response);
            });
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut stream = connect(address, deadline).unwrap();
            assert!(getvar(&mut stream, "appl.name", deadline).is_err());
            server.join().unwrap();
        }
    }
}
