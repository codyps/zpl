//! Loopback-only contract tests; these establish simulator behavior, not parity
//! with physical Zebra firmware. SGD/JSON wire examples: Link-OS Addendum
//! P1058886-001 pp. 25–26; HTTP form: existing zebra-http-api client contract.
//! https://techdocs.zebra.com/link-os/2-13/webservices/content/link-os-pg-addendum-en.pdf
use std::time::Duration;
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::watch,
};
use zebra_simulator::{serve_tcp, Config, Mode, Simulator};

const LABEL: &[u8] = b"^XA^PW100^LL80^FO5,5^GB20,20,2^FS^XZ";
struct Server {
    app: Simulator,
    tcp: std::net::SocketAddr,
    json: std::net::SocketAddr,
    host: String,
    stop: watch::Sender<bool>,
    tcp_task: tokio::task::JoinHandle<eyre::Result<()>>,
    json_task: tokio::task::JoinHandle<eyre::Result<()>>,
    http_task: tokio::task::JoinHandle<()>,
    _dir: TempDir,
}
impl Server {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = Simulator::open(
            dir.path().join("printer.sqlite").to_str().unwrap(),
            Config::default(),
        )
        .unwrap();
        let tcp = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let json = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_addr = tcp.local_addr().unwrap();
        let json_addr = json.local_addr().unwrap();
        let host = format!("http://{}/", http.local_addr().unwrap());
        let (stop, receiver) = watch::channel(false);
        let tcp_task = tokio::spawn(serve_tcp(tcp, app.clone(), false, receiver.clone()));
        let json_task = tokio::spawn(serve_tcp(json, app.clone(), true, receiver));
        let router = app.router();
        let http_task = tokio::spawn(async move {
            axum::serve(http, router).await.unwrap();
        });
        Self {
            app,
            tcp: tcp_addr,
            json: json_addr,
            host,
            stop,
            tcp_task,
            json_task,
            http_task,
            _dir: dir,
        }
    }
    async fn close(self) {
        self.stop.send_replace(true);
        tokio::time::timeout(Duration::from_secs(5), self.tcp_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.json_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        self.http_task.abort();
    }
    fn client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }
}
async fn line(socket: &mut TcpStream) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut result = Vec::new();
        loop {
            let byte = socket.read_u8().await.unwrap();
            result.push(byte);
            if byte == b'\n' {
                return result;
            }
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn tcp_fragments_mixed_requests_and_persistent_previews() {
    let server = Server::new().await;
    let mut socket = TcpStream::connect(server.tcp).await.unwrap();
    socket
        .write_all(b"! U1 setvar \"device.location\" \"test desk\"\r\n! U1 get")
        .await
        .unwrap();
    socket
        .write_all(b"var \"device.location\"\r\n")
        .await
        .unwrap();
    assert_eq!(line(&mut socket).await, b"\"test desk\"\r\n");
    for byte in LABEL {
        socket.write_all(&[*byte]).await.unwrap();
    }
    // This response is a barrier proving the preceding label was committed.
    socket
        .write_all(b"! U1 getvar \"odometer.total_label_count\"\r\n")
        .await
        .unwrap();
    assert_eq!(line(&mut socket).await, b"\"1\"\r\n");
    let history = server.app.history(None, None).await.unwrap();
    assert_eq!(history.len(), 4);
    let label = history.iter().find(|r| r.kind == "label").unwrap();
    let detail = server.app.detail(label.id).await.unwrap().unwrap();
    assert_eq!(detail.raw, LABEL);
    assert!(detail.entry.error.is_none());
    let png = server.app.image(label.id, 0).await.unwrap().unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 100);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 80);
    assert!(server.app.image(label.id, 1).await.unwrap().is_none());
    let restarted = Simulator::open(
        server._dir.path().join("printer.sqlite").to_str().unwrap(),
        Config::default(),
    )
    .unwrap();
    assert_eq!(restarted.image(label.id, 0).await.unwrap().unwrap(), png);
    let reply = restarted
        .submit(
            b"! U1 getvar \"device.location\"\r\n".to_vec(),
            "test".into(),
            "restart".into(),
            Mode::Auto,
        )
        .await
        .unwrap();
    assert_eq!(reply.response, b"\"test desk\"\r\n");
    drop(socket);
    server.close().await;
}
#[tokio::test]
async fn json_port_batches_branches_unknowns_and_read_only_values() {
    let server = Server::new().await;
    let mut socket = TcpStream::connect(server.json).await.unwrap();
    socket.write_all(b"{}").await.unwrap();
    socket.write_all(br#"{"device.location":"desk","device.unique_id":"overwritten","not.real":null}{}{"device":null}"#).await.unwrap();
    let first: serde_json::Value = serde_json::from_slice(&line(&mut socket).await).unwrap();
    assert_eq!(first["device.location"], "desk");
    assert_eq!(first["device.unique_id"], "SIM000000001");
    assert!(first["not.real"].is_null());
    let branch: serde_json::Value = serde_json::from_slice(&line(&mut socket).await).unwrap();
    assert_eq!(branch["device"]["device.location"], "desk");
    let history = server.app.history(None, None).await.unwrap();
    assert_eq!(history.len(), 2);
    assert!(history[1].error.as_ref().unwrap().contains("read-only"));
    drop(socket);
    server.close().await;
}
#[tokio::test]
async fn existing_clients_can_query_identity_and_http_preview() {
    let server = Server::new().await;
    let address = server.tcp;
    let identity = tokio::task::spawn_blocking(move || {
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        zebra_sgd::printer_identity(
            &mut stream,
            std::time::Instant::now() + Duration::from_secs(5),
        )
        .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(identity.serial, "SIM000000001");
    assert_eq!(identity.configuration["ezpl.print_width"], "812");
    let client = server.client();
    let png = zebra_http_api::zpl_to_png_with_credentials(
        client.clone(),
        server.host.clone(),
        std::str::from_utf8(LABEL).unwrap(),
        "private user",
        "private password",
    )
    .await
    .unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let history = server.app.history(None, None).await.unwrap();
    assert_eq!(history[0].kind, "preview");
    let detail = server.app.detail(history[0].id).await.unwrap().unwrap();
    assert_eq!(detail.raw, LABEL);
    let response = client
        .post(format!("{}pstprnt", server.host))
        .body([LABEL, LABEL].concat())
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let history = server.app.history(None, None).await.unwrap();
    assert_eq!(history[0].label_count, 2);
    let response = client
        .get(format!("{}api/requests/{}", server.host, history[0].id))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let response = client
        .get(format!("{}api/requests/{}/raw", server.host, history[0].id))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        [LABEL, LABEL].concat()
    );
    server.close().await;
}
#[tokio::test]
async fn realtime_events_reconnect_and_history_cursor() {
    let server = Server::new().await;
    let client = server.client();
    let mut events = client
        .get(format!("{}api/events", server.host))
        .send()
        .await
        .unwrap();
    assert_eq!(events.headers()["content-type"], "text/event-stream");
    let initial = events.chunk().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&initial)
        .unwrap()
        .contains("event: requests"));
    let outcome = server
        .app
        .submit(LABEL.to_vec(), "test".into(), "1".into(), Mode::Auto)
        .await
        .unwrap();
    let update = events.chunk().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&update)
        .unwrap()
        .contains(&format!("data: {}", outcome.id)));
    drop(events);
    server
        .app
        .submit(LABEL.to_vec(), "test".into(), "2".into(), Mode::Auto)
        .await
        .unwrap();
    let rows = server.app.history(Some(outcome.id), None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].id > outcome.id);
    let mut reconnected = client
        .get(format!("{}api/events", server.host))
        .header("Last-Event-ID", outcome.id)
        .send()
        .await
        .unwrap();
    let update = reconnected.chunk().await.unwrap().unwrap();
    assert!(std::str::from_utf8(&update)
        .unwrap()
        .contains(&format!("data: {}", rows[0].id)));
    drop(reconnected);
    server.close().await;
}
#[tokio::test]
async fn shutdown_and_invalid_traffic_are_retained_without_rendering() {
    let server = Server::new().await;
    let mut socket = TcpStream::connect(server.tcp).await.unwrap();
    socket.write_all(b"^XA^FDunfinished").await.unwrap();
    socket.shutdown().await.unwrap();
    // EOF response confirms that the partial request has been persisted.
    let mut discarded = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut discarded))
        .await
        .unwrap()
        .unwrap();
    let rows = server.app.history(None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "invalid");
    assert_eq!(
        server.app.detail(rows[0].id).await.unwrap().unwrap().raw,
        b"^XA^FDunfinished"
    );
    let bad = server
        .app
        .submit(
            b"^XA^ZZ^XZ".to_vec(),
            "test".into(),
            "bad".into(),
            Mode::Auto,
        )
        .await
        .unwrap();
    assert!(bad.error.is_some());
    assert_eq!(bad.pages, 0);
    let mut json = TcpStream::connect(server.json).await.unwrap();
    json.write_all(br#"{}{"x":false}"#).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), json.read_to_end(&mut discarded))
        .await
        .unwrap()
        .unwrap();
    let rows = server.app.history(None, None).await.unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, "invalid");
    server.close().await;
}
#[tokio::test]
async fn pause_status_and_resume_are_visible_and_affect_printing() {
    let server = Server::new().await;
    let mut socket = TcpStream::connect(server.tcp).await.unwrap();
    socket.write_all(b"~PP~HS").await.unwrap();
    assert!(String::from_utf8(line(&mut socket).await)
        .unwrap()
        .starts_with("\x02030,0,1,"));
    line(&mut socket).await;
    line(&mut socket).await;
    let response = server
        .client()
        .post(format!("{}pstprnt", server.host))
        .body(LABEL)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    socket
        .write_all(b"~PS! U1 getvar \"device.pause\"\r\n")
        .await
        .unwrap();
    assert_eq!(line(&mut socket).await, b"\"off\"\r\n");
    let response = server
        .client()
        .post(format!("{}pstprnt", server.host))
        .body(LABEL)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    drop(socket);
    server.close().await;
}

#[tokio::test]
async fn pagination_limits_and_http_rejections_are_persisted() {
    let server = Server::new().await;
    for n in 0..105 {
        server
            .app
            .submit(
                b"! U1 getvar \"appl.name\"\r\n".to_vec(),
                "test".into(),
                n.to_string(),
                Mode::Auto,
            )
            .await
            .unwrap();
    }
    let latest = server.app.history(None, None).await.unwrap();
    assert_eq!(latest.len(), 100);
    let older = server
        .app
        .history(None, Some(latest.last().unwrap().id))
        .await
        .unwrap();
    assert_eq!(older.len(), 5);
    let newer = server.app.history(Some(older[0].id), None).await.unwrap();
    assert_eq!(newer.len(), 100);
    assert!(newer.windows(2).all(|r| r[0].id < r[1].id));
    let response = server
        .client()
        .post(format!("{}pstprnt", server.host))
        .body(vec![b'x'; zebra_simulator::MAX_REQUEST_BYTES + 1])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    let response = server
        .client()
        .post(format!("{}zpl", server.host))
        .form(&[("username", "private"), ("pw", "secret")])
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    let latest = server.app.history(None, None).await.unwrap();
    for row in &latest[..2] {
        assert_eq!(row.kind, "invalid");
        assert!(row.error.is_some());
        assert!(server
            .app
            .detail(row.id)
            .await
            .unwrap()
            .unwrap()
            .raw
            .is_empty());
    }
    server.close().await;
}

#[tokio::test]
async fn tcp_limit_and_binary_invalid_bytes_preserve_bounded_history() {
    let server = Server::new().await;
    let mut socket = TcpStream::connect(server.tcp).await.unwrap();
    let mut payload = b"^XA^FD".to_vec();
    payload.resize(zebra_simulator::MAX_REQUEST_BYTES, b'x');
    socket.write_all(&payload).await.unwrap();
    let mut remaining = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut remaining))
        .await
        .unwrap()
        .unwrap();
    let rows = server.app.history(None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].error.as_ref().unwrap().contains("1 MiB"));
    assert_eq!(
        server.app.detail(rows[0].id).await.unwrap().unwrap().raw,
        payload
    );
    let bad = server
        .app
        .submit(
            vec![0xff, 0, 0x80],
            "test".into(),
            "binary".into(),
            Mode::Invalid("unsupported binary traffic".into()),
        )
        .await
        .unwrap();
    assert_eq!(
        server.app.detail(bad.id).await.unwrap().unwrap().raw,
        vec![0xff, 0, 0x80]
    );
    server.close().await;
}
