use super::*;
use axum::{
    extract::Form,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use diesel::prelude::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex as StdMutex,
    },
};
use tokio::io::{AsyncBufReadExt, BufReader};

struct Mock {
    name: String,
    config: PrinterConfig,
    labels: Arc<StdMutex<Vec<String>>>,
    firmware: Arc<StdMutex<String>>,
    mode: Arc<AtomicUsize>,
    restarts: Arc<AtomicUsize>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Mock {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
impl Mock {
    async fn new(name: &str) -> Self {
        let labels = Arc::new(StdMutex::new(Vec::new()));
        let firmware = Arc::new(StdMutex::new("V1".to_string()));
        let mode = Arc::new(AtomicUsize::new(0));
        let restarts = Arc::new(AtomicUsize::new(0));
        let hung = Arc::new(AtomicUsize::new(0));
        let pixels = (0..64 * 32)
            .map(|i| if i % 64 < 8 && i / 64 < 8 { 0 } else { 255 })
            .collect();
        let png = raster_diff::Png::encode_gray(
            &raster_diff::Raster {
                width: 64,
                height: 32,
                pixels,
            },
            203,
        )
        .unwrap();
        let app = Router::new()
            .route(
                "/zpl",
                post({
                    let labels = labels.clone();
                    let mode = mode.clone();
                    let hung = hung.clone();
                    move |Form(form): Form<HashMap<String, String>>| {
                        let labels = labels.clone();
                        let mode = mode.clone();
                        let hung = hung.clone();
                        async move {
                            let label = form.get("data").unwrap().clone();
                            labels.lock().unwrap().push(label.clone());
                            let mode = mode.load(Ordering::SeqCst);
                            if mode == 4 {
                                return (StatusCode::SERVICE_UNAVAILABLE, "offline")
                                    .into_response();
                            }
                            if label.contains("BAD") {
                                if mode == 1 {
                                    return "No preview available".into_response();
                                }
                                if mode == 2 || mode == 3 {
                                    hung.store(1, Ordering::SeqCst);
                                }
                                if mode == 5 {
                                    return (StatusCode::SERVICE_UNAVAILABLE, "offline")
                                        .into_response();
                                }
                            }
                            if hung.load(Ordering::SeqCst) != 0 {
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                return (StatusCode::SERVICE_UNAVAILABLE, "hung").into_response();
                            }
                            "<IMG SRC=\"/image\">".into_response()
                        }
                    }
                }),
            )
            .route(
                "/image",
                get(move || {
                    let png = png.clone();
                    async move { png }
                }),
            );
        let http = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", http.local_addr().unwrap());
        let control = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let control_address = control.local_addr().unwrap().to_string();
        let http_task = tokio::spawn(async move {
            axum::serve(http, app).await.unwrap();
        });
        let control_task = tokio::spawn({
            let firmware = firmware.clone();
            let restarts = restarts.clone();
            let mode = mode.clone();
            let serial = format!("serial-{name}");
            async move {
                loop {
                    let (stream, _) = control.accept().await.unwrap();
                    let mut stream = BufReader::new(stream);
                    loop {
                        let mut line = String::new();
                        if !matches!(stream.read_line(&mut line).await, Ok(n) if n > 0) {
                            break;
                        }
                        if line.contains("device.reset") {
                            restarts.fetch_add(1, Ordering::SeqCst);
                            hung.store(0, Ordering::SeqCst);
                            if mode.load(Ordering::SeqCst) == 2 {
                                mode.store(0, Ordering::SeqCst);
                            }
                            break;
                        }
                        if mode.load(Ordering::SeqCst) == 7 {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            break;
                        }
                        let value = if line.contains("device.unique_id") {
                            serial.clone()
                        } else if line.contains("device.product_name") {
                            "ZD621".into()
                        } else if line.contains("appl.name") {
                            firmware.lock().unwrap().clone()
                        } else {
                            "?".into()
                        };
                        if stream
                            .write_all(format!("\"{value}\"\r\n").as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            name: name.into(),
            config: PrinterConfig {
                url,
                control_address,
                width: 64,
                height: 32,
                headers: vec![],
                serial: None,
                admission: Default::default(),
            },
            labels,
            firmware,
            mode,
            restarts,
            tasks: vec![http_task, control_task],
        }
    }
    fn printer(&self, cache: Cache) -> Printer {
        Printer::with_timing(
            self.name.clone(),
            self.config.clone(),
            cache,
            "test",
            Timing {
                // Allow healthy loopback requests to finish on busy CI runners.
                // Simulated hangs above still exceed this deadline.
                io: Duration::from_millis(500),
                boot: Duration::from_secs(2),
                settle: Duration::from_millis(2),
                poll: Duration::from_millis(5),
                cooldown: Duration::from_secs(300),
            },
            Ownership::default(),
        )
        .unwrap()
    }
}
fn database() -> (tempfile::TempDir, Cache, SqliteConnection) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite");
    let db = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    let cache = Cache::open(path.to_str().unwrap()).unwrap();
    (dir, cache, db)
}
const LABEL: &str = "^XA^FO10,10^FDBAD^FS^XZ";

#[tokio::test]
async fn independent_names_identity_cache_and_resets() {
    let (_dir, cache, mut db) = database();
    let a = Mock::new("ZD621").await;
    let b = Mock::new("ZD621-V2").await;
    let pa = a.printer(cache.clone());
    let pb = b.printer(cache);
    let (ra, rb) = tokio::join!(
        pa.render(LABEL.into(), false),
        pb.render(LABEL.into(), false)
    );
    assert!(!ra.unwrap().1);
    assert!(!rb.unwrap().1);
    assert!(pa.render(LABEL.into(), false).await.unwrap().1);
    assert_eq!(a.labels.lock().unwrap().len(), 2);
    *a.firmware.lock().unwrap() = "V2".into();
    assert!(!pa.render(LABEL.into(), false).await.unwrap().1);
    assert!(!pa.render(LABEL.into(), true).await.unwrap().1);
    for label in a.labels.lock().unwrap().iter() {
        assert!(label.starts_with(
            "^XA^MCY^PW64^LL32^LH0,0^LS0^LT0^PON^PMN^LRN^FWN,0^CFA,9,5^BY2,3,10^CI27"
        ));
        assert!(label.ends_with("^XZ"));
    }
    use crate::schema::{preview_attempts as p, printer_requests as r};
    let history = r::table
        .order(r::request_id)
        .select((r::name, r::serial, r::firmware))
        .load::<(String, Option<String>, Option<String>)>(&mut db)
        .unwrap();
    assert_eq!(history.len(), 5);
    assert!(history
        .iter()
        .all(|(name, serial, _)| serial.as_deref() == Some(format!("serial-{name}").as_str())));
    assert_eq!(history.last().unwrap().2.as_deref(), Some("V2"));
    assert_eq!(
        p::table
            .filter(p::completed_at.is_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn deterministic_rejection_is_durable_and_refresh_cannot_bypass_it() {
    let (dir, cache, mut db) = database();
    let mock = Mock::new("ZD621").await;
    mock.mode.store(1, Ordering::SeqCst);
    let printer = mock.printer(cache);
    assert!(
        !printer
            .render(LABEL.into(), false)
            .await
            .unwrap_err()
            .downcast_ref::<Rejected>()
            .unwrap()
            .cached
    );
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 0);
    let before = mock.labels.lock().unwrap().len();
    let reopened =
        mock.printer(Cache::open(dir.path().join("db.sqlite").to_str().unwrap()).unwrap());
    assert!(
        reopened
            .render(LABEL.into(), true)
            .await
            .unwrap_err()
            .downcast_ref::<Rejected>()
            .unwrap()
            .cached
    );
    assert_eq!(before, mock.labels.lock().unwrap().len());
    use crate::schema::{permanent_errors as e, png_requests as r};
    assert_eq!(
        r::table
            .filter(r::cache_hit.eq(true))
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        1
    );
    assert_eq!(e::table.count().get_result::<i64>(&mut db).unwrap(), 1);
    *mock.firmware.lock().unwrap() = "V2".into();
    mock.mode.store(0, Ordering::SeqCst);
    assert!(!reopened.render(LABEL.into(), false).await.unwrap().1);
}

#[tokio::test]
async fn stuck_preview_restarts_and_retries_or_quarantines_reproduced_hang() {
    for policy in [AdmissionPolicy::Restricted, AdmissionPolicy::Unrestricted] {
        for mode in [2, 3] {
            let (_dir, cache, mut db) = database();
            let mut mock = Mock::new("ZD621").await;
            mock.config.admission = policy;
            mock.mode.store(mode, Ordering::SeqCst);
            let mut printer = mock.printer(cache);
            Arc::get_mut(&mut printer.0).unwrap().timing.cooldown = Duration::from_secs(3);
            let result = printer.render(LABEL.into(), false).await;
            if mode == 2 {
                assert!(!result.unwrap().1);
            } else {
                assert!(result.unwrap_err().downcast_ref::<Unavailable>().is_some());
                assert_eq!(
                    mock.restarts.load(Ordering::SeqCst),
                    1,
                    "second restart must wait for cooldown"
                );
                timeout(Duration::from_secs(10), async {
                    loop {
                        if !printer.0.state.lock().await.recovering {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                })
                .await
                .expect("automatic recovery did not finish");
                assert!(
                    printer
                        .render(LABEL.into(), true)
                        .await
                        .unwrap_err()
                        .downcast_ref::<Rejected>()
                        .unwrap()
                        .cached
                );
            }
            assert_eq!(
                mock.restarts.load(Ordering::SeqCst),
                if mode == 2 { 1 } else { 2 }
            );
            use crate::schema::preview_attempts as p;
            let phases = p::table
                .order(p::id)
                .select(p::phase)
                .load::<String>(&mut db)
                .unwrap();
            assert!(phases.iter().any(|p| p == "retry"));
            assert_eq!(
                p::table
                    .filter(p::phase.eq("label"))
                    .select(p::failure_kind)
                    .first::<Option<String>>(&mut db)
                    .unwrap()
                    .as_deref(),
                Some("hang")
            );
            assert!(phases.iter().any(|p| p == "recovery-control"));
            assert_eq!(
                p::table
                    .filter(p::serial.ne("serial-ZD621"))
                    .count()
                    .get_result::<i64>(&mut db)
                    .unwrap(),
                0
            );
        }
    }
}

#[tokio::test]
async fn outage_is_retryable_cooldown_survives_reopen_and_other_printer_runs() {
    let (dir, cache, mut db) = database();
    let a = Mock::new("a").await;
    let b = Mock::new("b").await;
    a.mode.store(4, Ordering::SeqCst);
    let pa = a.printer(cache.clone());
    let pb = b.printer(cache);
    // Hold A's state explicitly: B must finish while A cannot make progress.
    // The timeout is only a deadlock guard, not a scheduling-speed assertion.
    let state = pa.0.state.lock().await;
    let request_printer = pa.clone();
    let ra = tokio::spawn(async move { request_printer.render(LABEL.into(), false).await });
    assert!(
        timeout(Duration::from_secs(10), pb.render(LABEL.into(), false))
            .await
            .unwrap()
            .is_ok()
    );
    assert!(!ra.is_finished());
    drop(state);
    assert!(ra
        .await
        .unwrap()
        .unwrap_err()
        .downcast_ref::<Unavailable>()
        .is_some());
    let reopened = a.printer(Cache::open(dir.path().join("db.sqlite").to_str().unwrap()).unwrap());
    assert!(reopened.render(LABEL.into(), false).await.is_err());
    assert_eq!(a.restarts.load(Ordering::SeqCst), 1);
    use crate::schema::permanent_errors as e;
    assert_eq!(e::table.count().get_result::<i64>(&mut db).unwrap(), 0);
}

#[tokio::test]
async fn repeated_http_failures_are_not_permanent() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("a").await;
    mock.mode.store(5, Ordering::SeqCst);
    assert!(mock
        .printer(cache)
        .render(LABEL.into(), false)
        .await
        .unwrap_err()
        .downcast_ref::<Unavailable>()
        .is_some());
    use crate::schema::permanent_errors as e;
    assert_eq!(e::table.count().get_result::<i64>(&mut db).unwrap(), 0);
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn admission_precedes_all_identity_preview_and_cache_access() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("a").await;
    let printer = mock.printer(cache);
    assert!(printer
        .render("^XA^XFR:PRIVATE.ZPL^FS^XZ".into(), true)
        .await
        .is_err());
    assert!(mock.labels.lock().unwrap().is_empty());
    use crate::schema::png_requests as r;
    assert_eq!(r::table.count().get_result::<i64>(&mut db).unwrap(), 0);
}

#[test]
fn reset_preserves_label_fields_verbatim() {
    let label = " \n^XA^FO1,2^FH^FD_5E_7E^FS^XZ\n";
    assert!(reset_label(label, 832, 1218).ends_with("^FO1,2^FH^FD_5E_7E^FS^XZ\n"));
}

#[tokio::test]
async fn recovery_continues_without_another_user_request() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("automatic").await;
    mock.mode.store(4, Ordering::SeqCst);
    let mut printer = mock.printer(cache);
    Arc::get_mut(&mut printer.0).unwrap().timing.cooldown = Duration::from_secs(3);
    assert!(printer.render(LABEL.into(), false).await.is_err());
    assert!(printer.0.state.lock().await.recovering);
    mock.mode.store(0, Ordering::SeqCst);
    timeout(Duration::from_secs(10), async {
        loop {
            if !printer.0.state.lock().await.recovering {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("background recovery required another request");
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 1);
    use crate::schema::{png_requests as r, preview_attempts as p};
    assert_eq!(r::table.count().get_result::<i64>(&mut db).unwrap(), 1);
    assert_eq!(
        p::table
            .filter(p::phase.eq("background-control"))
            .filter(p::error.is_null())
            .filter(p::completed_at.is_not_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn recovery_does_not_stop_after_three_failed_attempts() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("persistent").await;
    mock.mode.store(4, Ordering::SeqCst);
    let mut printer = mock.printer(cache);
    let timing = &mut Arc::get_mut(&mut printer.0).unwrap().timing;
    timing.cooldown = Duration::from_millis(40);
    timing.boot = Duration::from_millis(100);
    assert!(printer.render(LABEL.into(), false).await.is_err());
    timeout(Duration::from_secs(10), async {
        while mock.restarts.load(Ordering::SeqCst) < 5 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        mock.mode.store(0, Ordering::SeqCst);
        loop {
            if !printer.0.state.lock().await.recovering {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("recovery stopped after a finite number of failures");
    use crate::schema::{png_requests as r, preview_attempts as p};
    assert_eq!(r::table.count().get_result::<i64>(&mut db).unwrap(), 1);
    assert!(
        p::table
            .filter(p::phase.eq("restart"))
            .count()
            .get_result::<i64>(&mut db)
            .unwrap()
            >= 5
    );
}

#[tokio::test]
async fn identity_hang_after_reopen_uses_marked_last_known_identity_for_recovery() {
    let (dir, cache, mut db) = database();
    let mock = Mock::new("identity").await;
    let first = mock.printer(cache);
    first.render(LABEL.into(), false).await.unwrap();
    drop(first);
    let mut printer =
        mock.printer(Cache::open(dir.path().join("db.sqlite").to_str().unwrap()).unwrap());
    Arc::get_mut(&mut printer.0).unwrap().timing.cooldown = Duration::from_millis(200);
    mock.mode.store(7, Ordering::SeqCst);
    assert!(printer.render(LABEL.into(), false).await.is_err());
    assert!(printer.0.state.lock().await.recovering);
    mock.mode.store(0, Ordering::SeqCst);
    timeout(Duration::from_secs(10), async {
        while printer.0.state.lock().await.recovering {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    use crate::schema::preview_attempts as p;
    assert_eq!(
        p::table
            .filter(p::phase.eq("identity-last-known"))
            .select((p::serial, p::firmware, p::failure_kind))
            .first::<(String, String, Option<String>)>(&mut db)
            .unwrap(),
        ("serial-identity".into(), "V1".into(), Some("hang".into()))
    );
    assert!(printer.render(LABEL.into(), false).await.unwrap().1);
}

#[tokio::test]
async fn client_disconnect_does_not_cancel_hang_recovery_or_persistence() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("disconnect").await;
    mock.mode.store(2, Ordering::SeqCst);
    let printer = mock.printer(cache);
    let request = tokio::spawn(async move { printer.render(LABEL.into(), false).await });
    timeout(Duration::from_secs(10), async {
        while mock.labels.lock().unwrap().len() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    use crate::schema::png_requests as r;
    timeout(Duration::from_secs(10), async {
        loop {
            if r::table
                .filter(r::png_id.is_not_null())
                .count()
                .get_result::<i64>(&mut db)
                .unwrap()
                == 1
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("disconnected request did not finish recovery/persistence");
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 1);
    assert_eq!(
        r::table
            .filter(r::error.is_not_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn unrestricted_preserves_input_with_caching_refresh_and_control_labels() {
    let (_dir, cache, mut db) = database();
    let mut mock = Mock::new("raw").await;
    mock.config.admission = AdmissionPolicy::Unrestricted;
    let printer = mock.printer(cache);
    // Intentionally not a standard single label. No parser, reset-prefix insertion,
    // or rendering allowlist may rewrite/reject this trusted operator stream.
    let input = "~CC! !XA!XFR:FORMAT.ZPL!XZ\n";
    for (refresh, hit) in [(false, false), (false, true), (true, false)] {
        assert_eq!(printer.render(input.into(), refresh).await.unwrap().1, hit);
    }
    assert_eq!(
        *mock.labels.lock().unwrap(),
        vec![reset_label(CONTROL, 64, 32), input.into(), input.into()]
    );
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 0);
    use crate::schema::{png_requests as r, printer_requests as p, render_cache as c};
    assert_eq!(
        r::table
            .filter(r::png_id.is_not_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        3
    );
    assert_eq!(
        p::table
            .filter(p::serial.is_not_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        3
    );
    assert_eq!(c::table.count().get_result::<i64>(&mut db).unwrap(), 1);
}

#[tokio::test]
async fn unrestricted_retries_then_caches_confirmed_rejections() {
    let (_dir, cache, mut db) = database();
    let mut mock = Mock::new("raw-failure").await;
    mock.config.admission = AdmissionPolicy::Unrestricted;
    mock.mode.store(1, Ordering::SeqCst);
    let printer = mock.printer(cache);
    let error = printer.render(LABEL.into(), false).await.unwrap_err();
    assert!(!error.downcast_ref::<Rejected>().unwrap().cached);
    let sent = mock.labels.lock().unwrap().clone();
    assert_eq!(
        sent.iter().filter(|label| label.as_str() == LABEL).count(),
        2
    );
    assert!(sent
        .iter()
        .any(|label| label == &reset_label(CONTROL, 64, 32)));
    assert!(
        printer
            .render(LABEL.into(), true)
            .await
            .unwrap_err()
            .downcast_ref::<Rejected>()
            .unwrap()
            .cached
    );
    assert_eq!(*mock.labels.lock().unwrap(), sent);
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 0);
    use crate::schema::{permanent_errors as e, png_requests as r};
    assert_eq!(e::table.count().get_result::<i64>(&mut db).unwrap(), 1);
    assert_eq!(
        r::table
            .filter(r::error.is_not_null())
            .filter(r::completed_at.is_not_null())
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn stored_and_variable_content_use_caching_and_refresh() {
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("images").await;
    let printer = mock.printer(cache);
    for input in [
        "^XA^XGR:IMAGE.GRF,1,1^FS^XZ",
        "^XA^FC^FDtime^FS^XZ",
        "^XA^SN1^FS^XZ",
    ] {
        assert!(!printer.render(input.into(), false).await.unwrap().1);
        assert!(printer.render(input.into(), false).await.unwrap().1);
        assert!(!printer.render(input.into(), true).await.unwrap().1);
    }
    assert_eq!(mock.labels.lock().unwrap().len(), 7); // initialization + six labels
    use crate::schema::render_cache as c;
    assert_eq!(c::table.count().get_result::<i64>(&mut db).unwrap(), 3);
}

#[tokio::test]
async fn policy_changes_select_a_new_cache_scope() {
    let (_dir, cache, _db) = database();
    let mut mock = Mock::new("policy").await;
    let restricted = mock.printer(cache.clone());
    restricted.render(LABEL.into(), false).await.unwrap();
    mock.config.admission = AdmissionPolicy::Unrestricted;
    let unrestricted = mock.printer(cache);
    assert_ne!(restricted.0.key, unrestricted.0.key);
    assert!(!unrestricted.render(LABEL.into(), false).await.unwrap().1);
    assert_eq!(mock.labels.lock().unwrap().last().unwrap(), LABEL);
}

#[tokio::test]
async fn metadata_survives_cache_hits_and_preserves_original_png() {
    // PNG textual chunks: https://www.w3.org/TR/png-3/#11textinfo
    // Capture provenance belongs to the stored rendering, including cache hits.
    let (_dir, cache, mut db) = database();
    let mock = Mock::new("metadata").await;
    let printer = mock.printer(cache);
    let (response, hit, identity) = printer.render(LABEL.into(), false).await.unwrap();
    assert!(!hit);
    assert_eq!(identity.model, "ZD621");
    assert_eq!(identity.serial, "serial-metadata");
    assert_eq!(identity.firmware, "V1");
    let cached = printer.render(LABEL.into(), false).await.unwrap();
    assert!(cached.1);
    assert_eq!(cached.0, response);
    assert_eq!(cached.2, identity);
    use crate::schema::{png_requests, pngs, render_cache};
    let (original, hash) = pngs::table
        .select((pngs::data, pngs::hash))
        .first::<(Vec<u8>, Vec<u8>)>(&mut db)
        .unwrap();
    use sha2::Digest;
    assert_eq!(hash, sha2::Sha256::digest(&original).to_vec());
    assert_ne!(original, response);
    assert_eq!(
        response,
        crate::png_metadata::annotate(&original, &identity, LABEL).unwrap()
    );
    let stored = render_cache::table
        .select(render_cache::printer_identity)
        .first::<String>(&mut db)
        .unwrap();
    assert_eq!(
        serde_json::from_str::<zebra_sgd::PrinterIdentity>(&stored).unwrap(),
        identity
    );
    let history = png_requests::table
        .order(png_requests::rowid)
        .select((png_requests::printer_identity, png_requests::cache_hit))
        .load::<(Option<String>, bool)>(&mut db)
        .unwrap();
    assert_eq!(
        history,
        vec![(Some(stored.clone()), false), (Some(stored), true)]
    );
}

#[tokio::test]
async fn annotation_failure_keeps_original_and_invalidates_cache_without_recovery() {
    let (_dir, cache, mut db) = database();
    let mut mock = Mock::new("annotation").await;
    mock.config.admission = AdmissionPolicy::Unrestricted;
    let printer = mock.printer(cache);
    // Unrestricted transport preserves NUL; PNG iTXt cannot represent it.
    let input = "^XA^FDnul\0text^FS^XZ";
    assert!(printer.render(input.into(), false).await.is_err());
    assert_eq!(mock.labels.lock().unwrap().last().unwrap(), input);
    assert_eq!(mock.restarts.load(Ordering::SeqCst), 0);
    use crate::schema::{png_requests, pngs, render_cache};
    assert_eq!(pngs::table.count().get_result::<i64>(&mut db).unwrap(), 1);
    assert_eq!(
        render_cache::table
            .count()
            .get_result::<i64>(&mut db)
            .unwrap(),
        0
    );
    let (png, identity, error, completed) = png_requests::table
        .select((
            png_requests::png_id,
            png_requests::printer_identity,
            png_requests::error,
            png_requests::completed_at,
        ))
        .first::<(Option<i64>, Option<String>, Option<String>, Option<String>)>(&mut db)
        .unwrap();
    assert!(png.is_some() && identity.is_some() && completed.is_some());
    assert_eq!(error.as_deref(), Some("PNG response annotation failed"));
}
