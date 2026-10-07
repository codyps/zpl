use diesel::{connection::SimpleConnection, prelude::*};
use zpl_proxy_api::{models::PngRequest, schema::png_requests};

const INITIAL: &str = include_str!("../migrations/2026-10-03-000000_create-render-cache/up.sql");
const REVERT: &str = include_str!("../migrations/2026-10-03-000000_create-render-cache/down.sql");

#[test]
fn fresh_schema_requires_cache_identity_and_enforces_relationships() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    db.batch_execute("PRAGMA foreign_keys = ON;").unwrap();
    db.batch_execute(INITIAL).unwrap();
    db.batch_execute(
        "INSERT INTO inputs(id,hash,data) VALUES(1,X'01',X'02');
         INSERT INTO pngs VALUES(1,'original',X'03',X'89504E470D0A1A0A',42);
         INSERT INTO png_requests(timestamp,input_id,renderer_key)
         VALUES('pending',1,X'04');",
    )
    .unwrap();
    let pending = png_requests::table
        .select(PngRequest::as_select())
        .first::<PngRequest>(&mut db)
        .unwrap();
    assert_eq!(pending.renderer_key, vec![4]);
    assert!(pending.png_id.is_none());
    assert!(pending.printer_identity.is_none());
    assert!(pending.completed_at.is_none());
    // SQLite NOT NULL / foreign key constraints, not just application checks.
    // https://www.sqlite.org/lang_createtable.html#notnullconst
    // https://www.sqlite.org/foreignkeys.html
    for invalid in [
        "INSERT INTO render_cache VALUES(1,X'04',1,NULL)",
        "INSERT INTO render_cache VALUES(99,X'04',1,'{}')",
        "INSERT INTO render_cache VALUES(1,X'04',99,'{}')",
        "INSERT INTO png_requests(timestamp,input_id) VALUES('missing key',1)",
        "INSERT INTO png_requests(timestamp,input_id,renderer_key) VALUES('missing input',99,X'04')",
        "UPDATE png_requests SET png_id=99",
    ] {
        assert!(db.batch_execute(invalid).is_err(), "accepted: {invalid}");
    }
    db.batch_execute(
        "INSERT INTO render_cache VALUES(1,X'04',1,'{}');
         UPDATE png_requests SET png_id=1,printer_identity='{}',completed_at='done';",
    )
    .unwrap();
    assert_eq!(
        png_requests::table
            .select(png_requests::printer_identity)
            .first::<Option<String>>(&mut db)
            .unwrap()
            .as_deref(),
        Some("{}")
    );
}

#[test]
fn initial_schema_can_be_reverted_and_recreated() {
    let mut db = SqliteConnection::establish(":memory:").unwrap();
    db.batch_execute("PRAGMA foreign_keys = ON;").unwrap();
    for _ in 0..2 {
        db.batch_execute(INITIAL).unwrap();
        db.batch_execute(
            "INSERT INTO inputs(id,hash,data) VALUES(1,X'01',X'02');
             INSERT INTO pngs VALUES(1,'original',X'03',X'04',42);
             INSERT INTO render_cache VALUES(1,X'05',1,'{}');
             INSERT INTO png_requests(timestamp,input_id,renderer_key,png_id,printer_identity)
             VALUES('rendered',1,X'05',1,'{}');",
        )
        .unwrap();
        db.batch_execute(REVERT).unwrap();
        for table in [
            "inputs",
            "pngs",
            "render_cache",
            "png_requests",
            "printer_requests",
            "preview_attempts",
            "permanent_errors",
            "printer_recovery",
        ] {
            assert!(db.batch_execute(&format!("SELECT * FROM {table}")).is_err());
        }
    }
}
