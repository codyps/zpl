use diesel::{connection::SimpleConnection, prelude::*};

const INITIAL: &str = include_str!("../migrations/2024-10-03-035443_cache-results/up.sql");
const FIX: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql");
const REVERT: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/down.sql");

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
