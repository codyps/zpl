use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Command, Output},
    thread,
    time::Duration,
};

// SGD keys/framing follow Zebra's Programming Guide, getvar and device/appl sections:
// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
fn answer(stream: TcpStream, version: &str) -> BufReader<TcpStream> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    for (key, value) in [
        ("device.product_name", "ZD621"),
        ("device.unique_id", "TEST123"),
        ("appl.name", version),
    ] {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, format!("! U1 getvar \"{key}\"\r\n"));
        // Split responses and omit newline to cover stream framing.
        reader.get_mut().write_all(b"\"").unwrap();
        reader.get_mut().write_all(value.as_bytes()).unwrap();
        reader.get_mut().write_all(b"\"").unwrap();
    }
    reader
}

fn invoke(address: SocketAddr, name: &str, bytes: &[u8], extra: &[&str]) -> Output {
    let path =
        std::env::temp_dir().join(format!("zebra-firmware-{}-{name}.zpl", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_zebra-firmware"))
        .args(["--printer", &address.to_string(), "apply"])
        .arg(&path)
        .args([
            "--model",
            "ZD621",
            "--serial",
            "TEST123",
            "--version",
            "NEW",
        ])
        .args(extra)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

fn no_upload(name: &str, model_version: &str, extra: &[&str], success: bool, message: &str) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let version = model_version.to_owned();
    let server = thread::spawn(move || {
        let mut stream = answer(listener.accept().unwrap().0, &version);
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert!(bytes.is_empty(), "unexpected firmware upload");
    });
    let output = invoke(address, name, b"test firmware", extra);
    server.join().unwrap();
    assert_eq!(output.status.success(), success, "{output:?}");
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains(message), "{all}");
}

#[test]
fn preview_does_not_upload() {
    no_upload("preview", "OLD", &[], true, "Preview only");
}

#[test]
fn already_installed_does_not_upload() {
    no_upload("installed", "NEW", &["--execute"], true, "Already running");
}

#[test]
fn wrong_identity_does_not_upload() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut stream = BufReader::new(listener.accept().unwrap().0);
        stream
            .get_mut()
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        for value in ["OTHER", "TEST123", "OLD"] {
            let mut line = String::new();
            stream.read_line(&mut line).unwrap();
            write!(stream.get_mut(), "\"{value}\"\r\n").unwrap();
        }
        let mut bytes = Vec::new();
        // Closing with an unread trailing SGD CRLF can reset TCP on macOS.
        if let Err(error) = stream.read_to_end(&mut bytes) {
            assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
        }
        assert!(bytes.is_empty());
    });
    let output = invoke(address, "wrong", b"test firmware", &["--execute"]);
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("identity mismatch"));
}

#[test]
fn uploads_exact_binary_bytes_and_verifies() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let firmware = b"~test\0\xff\r\nfirmware";
    let server = thread::spawn(move || {
        let mut stream = answer(listener.accept().unwrap().0, "OLD");
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, firmware);
        answer(listener.accept().unwrap().0, "NEW");
    });
    let output = invoke(
        address,
        "upload",
        firmware,
        &["--execute", "--wait-seconds", "5"],
    );
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Verified:"));
}

#[test]
fn lost_printer_is_not_reported_as_success_or_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let mut stream = answer(listener.accept().unwrap().0, "OLD");
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"test firmware");
    });
    let output = invoke(
        address,
        "lost",
        b"test firmware",
        &["--execute", "--wait-seconds", "1"],
    );
    server.join().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("installation NOT verified"));
}

#[test]
fn invalid_files_fail_before_connecting() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    for (name, bytes, extra) in [
        ("empty", &b""[..], vec![]),
        ("zip", &b"PK\x03\x04"[..], vec![]),
        ("hash", &b"firmware"[..], vec!["--sha256", "00"]),
    ] {
        let output = invoke(listener.local_addr().unwrap(), name, bytes, &extra);
        assert!(!output.status.success());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
