use super::*;
use crate::models::PngRequest;

fn migrated() -> SqliteConnection {
    let mut connection = SqliteConnection::establish(":memory:").unwrap();
    connection.run_pending_migrations(MIGRATIONS).unwrap();
    connection
        .batch_execute("PRAGMA foreign_keys = ON;")
        .unwrap();
    connection
}
fn identity() -> zebra_sgd::PrinterIdentity {
    zebra_sgd::PrinterIdentity {
        model: "ZD621".into(),
        serial: "TEST-SERIAL".into(),
        firmware: "V93.21.33Z".into(),
        configuration: Default::default(),
    }
}
fn cache() -> Cache {
    Cache(Arc::new(Mutex::new(migrated())))
}
async fn begin(cache: &Cache, key: u8, refresh: bool) -> Attempt {
    cache
        .begin(b"^XA^XZ".to_vec(), vec![key], refresh)
        .await
        .unwrap()
}

#[tokio::test]
async fn deduplicates_bytes_but_records_every_request_and_renderer() {
    let cache = cache();
    let first = begin(&cache, 1, false).await;
    assert!(first.cached.is_none());
    cache
        .rendered(first, b"first-png".to_vec(), identity(), None)
        .await
        .unwrap();
    assert_eq!(
        begin(&cache, 1, false).await.cached.unwrap().png,
        b"first-png"
    );
    let other = begin(&cache, 2, false).await;
    assert!(other.cached.is_none());
    cache
        .rendered(other, b"first-png".to_vec(), identity(), None)
        .await
        .unwrap();
    let mut connection = cache.0.lock().unwrap();
    assert_eq!(
        inputs::table
            .count()
            .get_result::<i64>(&mut *connection)
            .unwrap(),
        1
    );
    assert_eq!(
        pngs::table
            .count()
            .get_result::<i64>(&mut *connection)
            .unwrap(),
        1
    );
    assert_eq!(
        render_cache::table
            .count()
            .get_result::<i64>(&mut *connection)
            .unwrap(),
        2
    );
    let requests = png_requests::table
        .order(png_requests::rowid)
        .select(PngRequest::as_select())
        .load::<PngRequest>(&mut *connection)
        .unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests.iter().map(|r| r.cache_hit).collect::<Vec<_>>(),
        [false, true, false]
    );
    assert!(requests
        .iter()
        .all(|r| r.completed_at.is_some() && r.png_id.is_some() && r.error.is_none()));
}

#[tokio::test]
async fn refresh_retains_history_and_errors_are_retried() {
    let cache = cache();
    cache
        .rendered(
            begin(&cache, 1, false).await,
            b"old".to_vec(),
            identity(),
            None,
        )
        .await
        .unwrap();
    let refresh = begin(&cache, 1, true).await;
    assert!(refresh.cached.is_none());
    cache
        .failure(refresh, "printer offline".into())
        .await
        .unwrap();
    let retry = begin(&cache, 1, false).await;
    assert!(retry.cached.is_none());
    cache
        .rendered(retry, b"new".to_vec(), identity(), None)
        .await
        .unwrap();
    assert_eq!(begin(&cache, 1, false).await.cached.unwrap().png, b"new");
    let mut connection = cache.0.lock().unwrap();
    assert_eq!(
        pngs::table
            .count()
            .get_result::<i64>(&mut *connection)
            .unwrap(),
        2
    );
    let requests = png_requests::table
        .order(png_requests::rowid)
        .select(PngRequest::as_select())
        .load::<PngRequest>(&mut *connection)
        .unwrap();
    assert_eq!(requests[1].error.as_deref(), Some("printer offline"));
    assert!(requests[1].completed_at.is_some());
    assert_ne!(requests[0].png_id, requests[2].png_id);
}

#[tokio::test]
async fn failed_annotation_preserves_original_bytes_and_identity_without_caching() {
    let cache = cache();
    cache
        .rendered(
            begin(&cache, 1, false).await,
            b"old".to_vec(),
            identity(),
            None,
        )
        .await
        .unwrap();
    let identity = zebra_sgd::PrinterIdentity {
        model: "ZTC ZD621-203dpi ZPL".into(),
        firmware: "V93.21.33Z".into(),
        serial: "TEST-SERIAL".into(),
        configuration: Default::default(),
    };
    let attempt = begin(&cache, 1, true).await;
    let request_id = attempt.request_id;
    cache
        .rendered(
            attempt,
            b"broken PNG".to_vec(),
            identity.clone(),
            Some("invalid printer PNG signature".into()),
        )
        .await
        .unwrap();
    assert!(begin(&cache, 1, false).await.cached.is_none());
    let mut connection = cache.0.lock().unwrap();
    let request = png_requests::table
        .find(request_id)
        .select(PngRequest::as_select())
        .first::<PngRequest>(&mut *connection)
        .unwrap();
    assert_eq!(
        request.error.as_deref(),
        Some("invalid printer PNG signature")
    );
    assert!(request.completed_at.is_some());
    assert_eq!(
        serde_json::from_str::<zebra_sgd::PrinterIdentity>(
            request.printer_identity.as_ref().unwrap()
        )
        .unwrap(),
        identity
    );
    assert_eq!(
        pngs::table
            .find(request.png_id.unwrap())
            .select(pngs::data)
            .first::<Vec<u8>>(&mut *connection)
            .unwrap(),
        b"broken PNG"
    );
}

#[tokio::test]
async fn pending_request_and_input_survive_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cache.sqlite");
    let mut connection = migrated();
    // https://www.sqlite.org/lang_vacuum.html#vacuum_with_an_into_clause
    diesel::sql_query("VACUUM INTO ?")
        .bind::<diesel::sql_types::Text, _>(path.to_str().unwrap())
        .execute(&mut connection)
        .unwrap();
    let cache = Cache::open(path.to_str().unwrap()).unwrap();
    let pending = begin(&cache, 1, false).await;
    drop(cache);
    let reopened = Cache::open(path.to_str().unwrap()).unwrap();
    let mut connection = reopened.0.lock().unwrap();
    let request = png_requests::table
        .find(pending.request_id)
        .select(PngRequest::as_select())
        .first::<PngRequest>(&mut *connection)
        .unwrap();
    assert!(request.completed_at.is_none());
    assert_eq!(
        inputs::table
            .find(request.input_id)
            .select(inputs::data)
            .first::<Vec<u8>>(&mut *connection)
            .unwrap(),
        b"^XA^XZ"
    );
}

#[test]
fn printer_fingerprint_includes_configuration() {
    let key = renderer_key(
        "http://printer/",
        &["Authorization: sample".into()],
        "v1",
        "printer:9100",
    );
    assert_ne!(
        key,
        renderer_key(
            "http://other/",
            &["Authorization: sample".into()],
            "v1",
            "printer:9100"
        )
    );
    assert_ne!(
        key,
        renderer_key("http://printer/", &[], "v1", "printer:9100")
    );
    assert_ne!(
        key,
        renderer_key(
            "http://printer/",
            &["Authorization: sample".into()],
            "v2",
            "printer:9100"
        )
    );
    assert_ne!(
        key,
        renderer_key(
            "http://printer/",
            &["Authorization: sample".into()],
            "v1",
            "printer:9101"
        )
    );
    assert_eq!(key.len(), 32);
}

#[test]
fn startup_migrates_fresh_database_and_reopens_without_reapplying() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fresh.sqlite");
    let cache = Cache::open(path.to_str().unwrap()).unwrap();
    let versions = cache.0.lock().unwrap().applied_migrations().unwrap();
    assert_eq!(versions.len(), 1);
    drop(cache);
    let reopened = Cache::open(path.to_str().unwrap()).unwrap();
    assert_eq!(
        reopened.0.lock().unwrap().applied_migrations().unwrap(),
        versions
    );
}

#[test]
fn startup_migration_failure_is_returned_without_recording_success() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.sqlite");
    let mut db = SqliteConnection::establish(path.to_str().unwrap()).unwrap();
    // Force the initial migration to fail against an incompatible table.
    db.batch_execute(
        "CREATE TABLE inputs (sentinel TEXT); INSERT INTO inputs VALUES ('preserve');",
    )
    .unwrap();
    let error = Cache::open(path.to_str().unwrap())
        .err()
        .expect("startup must fail");
    assert!(error.to_string().contains("Database migration failed"));
    assert!(db.applied_migrations().unwrap().is_empty());
    let value = diesel::select(diesel::dsl::sql::<diesel::sql_types::Text>(
        "(SELECT sentinel FROM inputs)",
    ))
    .get_result::<String>(&mut db)
    .unwrap();
    assert_eq!(value, "preserve");
}
