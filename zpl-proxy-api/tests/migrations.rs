use diesel::{connection::SimpleConnection, prelude::*};

const INITIAL: &str = include_str!("../migrations/2024-10-03-035443_cache-results/up.sql");
const FIX: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql");
const REVERT: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/down.sql");

#[test]
fn identity_migration_preserves_original_pngs_and_legacy_mappings() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    for migration in [
        INITIAL,
        FIX,
        include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
        include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
    ] {
        db.batch_execute(migration).unwrap();
    }
    db.batch_execute(
        "PRAGMA foreign_keys = ON;
        INSERT INTO pngs VALUES(7,'original',X'0102',X'89504E470D0A1A0A',42);
        INSERT INTO inputs(id,hash,data,png_id) VALUES(8,X'03',X'04',7);
        INSERT INTO render_cache VALUES(8,X'05',7);
        INSERT INTO png_requests(rowid,timestamp,input_id,png_id) VALUES(9,'legacy',8,7);",
    )
    .unwrap();
    for _ in 0..2 {
        db.batch_execute(include_str!(
            "../migrations/2026-10-03-000000_printer-identity/up.sql"
        ))
        .unwrap();
        use zpl_proxy_api::schema::{png_requests, pngs, render_cache};
        let original = pngs::table
            .find(7)
            .select((pngs::data, pngs::hash))
            .first::<(Vec<u8>, Vec<u8>)>(&mut db)
            .unwrap();
        assert_eq!(original, (b"\x89PNG\r\n\x1a\n".to_vec(), vec![1, 2]));
        assert_eq!(
            render_cache::table
                .select((render_cache::png_id, render_cache::printer_identity))
                .first::<(i64, Option<String>)>(&mut db)
                .unwrap(),
            (7, None)
        );
        assert_eq!(
            png_requests::table
                .find(9)
                .select((png_requests::png_id, png_requests::printer_identity))
                .first::<(Option<i64>, Option<String>)>(&mut db)
                .unwrap(),
            (Some(7), None)
        );
        db.batch_execute(include_str!(
            "../migrations/2026-10-03-000000_printer-identity/down.sql"
        ))
        .unwrap();
    }
}

#[test]
fn migration_removes_ips_and_preserves_request_history() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    for migration in [
        INITIAL,
        FIX,
        include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
    ] {
        db.batch_execute(migration).unwrap();
    }
    db.batch_execute(
        "PRAGMA foreign_keys = ON;
        INSERT INTO clients VALUES(7,'192.0.2.1');
        INSERT INTO inputs(id,hash,data) VALUES(8,X'01',X'02');
        INSERT INTO png_requests(rowid,peer_id,timestamp,input_id,error,completed_at)
        VALUES(42,7,'start',8,'printer offline','end');",
    )
    .unwrap();
    db.batch_execute(include_str!(
        "../migrations/2026-09-15-230000_remove-client-ips/up.sql"
    ))
    .unwrap();
    assert!(db.batch_execute("SELECT ip FROM clients").is_err());
    assert!(db
        .batch_execute("SELECT peer_id FROM png_requests")
        .is_err());
    db.batch_execute(include_str!(
        "../migrations/2026-10-03-000000_printer-identity/up.sql"
    ))
    .unwrap();
    use zpl_proxy_api::{models::PngRequest, schema::png_requests};
    let row = png_requests::table
        .find(42)
        .select(PngRequest::as_select())
        .first::<PngRequest>(&mut db)
        .unwrap();
    assert_eq!(row.input_id, 8);
    assert_eq!(row.timestamp, "start");
    assert_eq!(row.error.as_deref(), Some("printer offline"));
    assert_eq!(row.completed_at.as_deref(), Some("end"));
    assert!(db
        .batch_execute("SELECT trace_id FROM png_requests")
        .is_err());
}

#[test]
fn request_client_migration_preserves_rows_and_enforces_relationships() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    db.batch_execute(INITIAL).unwrap();
    // Existing installations could have written rows with enforcement disabled.
    db.batch_execute(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO clients (id, ip) VALUES (1, '127.0.0.1');
         INSERT INTO inputs (id, hash, data) VALUES (1, x'01', x'02');
         INSERT INTO png_requests (rowid, peer_id, timestamp, input_id)
         VALUES (42, 1, '2026-09-14', 1);
         PRAGMA foreign_keys = ON;",
    )
    .unwrap();

    for _ in 0..2 {
        db.batch_execute(FIX).unwrap();
        #[derive(QueryableByName)]
        struct LegacyRequest {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            rowid: i64,
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            peer_id: i64,
            #[diesel(sql_type = diesel::sql_types::Text)]
            timestamp: String,
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            input_id: i64,
        }
        let row = diesel::sql_query("SELECT rowid, peer_id, timestamp, input_id FROM png_requests")
            .get_result::<LegacyRequest>(&mut db)
            .unwrap();
        assert_eq!(
            (row.rowid, row.peer_id, row.timestamp, row.input_id),
            (42, 1, "2026-09-14".into(), 1)
        );
        assert!(db
            .batch_execute("INSERT INTO png_requests VALUES (999, 'missing client', 1)")
            .is_err());
        assert!(db
            .batch_execute("INSERT INTO png_requests VALUES (1, 'missing input', 999)")
            .is_err());
        db.batch_execute("INSERT INTO png_requests VALUES (1, 'valid', 1)")
            .unwrap();
        db.batch_execute("DELETE FROM png_requests WHERE timestamp = 'valid'")
            .unwrap();
        db.batch_execute(REVERT).unwrap();
    }
}

#[test]
fn fresh_database_accepts_request_after_migrations() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    db.batch_execute("PRAGMA foreign_keys = ON;").unwrap();
    db.batch_execute(INITIAL).unwrap();
    db.batch_execute(FIX).unwrap();
    db.batch_execute(
        "INSERT INTO clients (id, ip) VALUES (1, '127.0.0.1');
         INSERT INTO inputs (id, hash, data) VALUES (1, x'01', x'02');
         INSERT INTO png_requests VALUES (1, '2026-09-14', 1);",
    )
    .unwrap();
}

#[test]
fn printer_management_migration_preserves_legacy_history_and_round_trips() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    for migration in [
        INITIAL,
        FIX,
        include_str!("../migrations/2026-09-15-220000_persist-render-results/up.sql"),
        include_str!("../migrations/2026-09-15-230000_remove-client-ips/up.sql"),
    ] {
        db.batch_execute(migration).unwrap();
    }
    db.batch_execute("INSERT INTO inputs(id,hash,data) VALUES(1,X'01',X'02'); INSERT INTO png_requests(rowid,timestamp,input_id,error,completed_at) VALUES(42,'before',1,'old error','done');").unwrap();
    for _ in 0..2 {
        db.batch_execute(include_str!(
            "../migrations/2026-09-29-120000_printer-management/up.sql"
        ))
        .unwrap();
        use zpl_proxy_api::schema::{png_requests as r, printer_requests as p};
        assert_eq!(
            r::table
                .find(42)
                .select(r::error)
                .first::<Option<String>>(&mut db)
                .unwrap()
                .as_deref(),
            Some("old error")
        );
        assert_eq!(p::table.count().get_result::<i64>(&mut db).unwrap(), 0);
        db.batch_execute(include_str!(
            "../migrations/2026-09-29-120000_printer-management/down.sql"
        ))
        .unwrap();
    }
}
