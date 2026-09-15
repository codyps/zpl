use diesel::{connection::SimpleConnection, prelude::*};

const INITIAL: &str = include_str!("../migrations/2024-10-03-035443_cache-results/up.sql");
const FIX: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/up.sql");
const REVERT: &str = include_str!("../migrations/2026-09-14-000000_fix-request-client/down.sql");

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
        use zpl_proxy_api::schema::png_requests::dsl::*;
        let row = png_requests
            .select((rowid, peer_id, timestamp, input_id))
            .first::<(i64, i64, String, i64)>(&mut db)
            .unwrap();
        assert_eq!(row, (42, 1, "2026-09-14".into(), 1));
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
