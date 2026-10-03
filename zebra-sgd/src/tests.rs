use super::*;
use std::{net::TcpListener, thread, time::Duration};

fn reply(bytes: Vec<u8>) -> (TcpStream, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0; b"! U1 getvar \"appl.name\"\r\n".len()];
        stream.read_exact(&mut request).unwrap();
        assert_eq!(&request, b"! U1 getvar \"appl.name\"\r\n");
        // One byte at a time also exercises partial TCP reads.
        for byte in bytes {
            if stream.write_all(&[byte]).is_err() {
                break;
            }
        }
    });
    (stream, task)
}

#[test]
fn bounded_framing_and_explicit_unsupported_values() {
    for (bytes, expected) in [
        (b"\r\n\"V93.21.33Z\"".to_vec(), Some("V93.21.33Z")),
        (b"\"?\"\r\n".to_vec(), None),
        (b"\"\"".to_vec(), None),
    ] {
        let (mut stream, task) = reply(bytes);
        assert_eq!(
            getvar(
                &mut stream,
                "appl.name",
                Instant::now() + Duration::from_secs(2)
            )
            .unwrap()
            .as_deref(),
            expected
        );
        task.join().unwrap();
    }
    for bytes in [
        b"garbage".to_vec(),
        b"\"unterminated".to_vec(),
        b"\"bad\x00value\"".to_vec(),
        b"\"\xff\"".to_vec(),
        vec![b'\n'; 4096],
        [vec![b'"'], vec![b'x'; 4096]].concat(),
    ] {
        let (mut stream, task) = reply(bytes);
        assert!(getvar(
            &mut stream,
            "appl.name",
            Instant::now() + Duration::from_secs(2)
        )
        .is_err());
        drop(stream);
        task.join().unwrap();
    }
}

#[test]
fn snapshot_reads_only_fixed_queries_and_keeps_replies_in_order() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let task = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        for (index, name) in ["device.product_name", "device.unique_id", "appl.name"]
            .into_iter()
            .chain(RENDER_SETTINGS.iter().copied())
            .enumerate()
        {
            let expected = format!("! U1 getvar \"{name}\"\r\n");
            let mut request = vec![0; expected.len()];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(request, expected.as_bytes());
            let value = match name {
                "device.product_name" => "ZD621",
                "device.unique_id" => "TEST-SERIAL",
                "appl.name" => "V93.21.33Z",
                "head.resolution.in_dpi" => "203",
                "zpl.label_length" => "1218",
                _ => "?",
            };
            write!(
                stream,
                "\"{value}\"{}",
                if index % 2 == 0 { "\r\n" } else { "" }
            )
            .unwrap();
        }
    });
    let identity = printer_identity(&mut stream, Instant::now() + Duration::from_secs(5)).unwrap();
    assert_eq!(identity.model, "ZD621");
    assert_eq!(identity.serial, "TEST-SERIAL");
    assert_eq!(identity.firmware, "V93.21.33Z");
    assert_eq!(identity.configuration.len(), 2);
    assert_eq!(identity.configuration["head.resolution.in_dpi"], "203");
    assert_eq!(identity.configuration["zpl.label_length"], "1218");
    task.join().unwrap();
}

#[test]
fn deadline_and_command_name_are_checked_before_sending() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    peer.set_read_timeout(Some(Duration::from_millis(30)))
        .unwrap();
    assert!(getvar(&mut stream, "appl.name", Instant::now()).is_err());
    assert!(getvar(
        &mut stream,
        "appl.name\"\r\n! U1 setvar",
        Instant::now() + Duration::from_secs(1)
    )
    .is_err());
    assert!(peer.read(&mut [0]).is_err());
}

#[test]
fn silent_printer_is_bounded_by_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_peer, _) = listener.accept().unwrap();
    let started = Instant::now();
    assert!(getvar(
        &mut stream,
        "appl.name",
        started + Duration::from_millis(30)
    )
    .is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
}
