use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::inputs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Input {
    pub id: i64,
    pub hash: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::pngs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Png {
    pub id: i64,
    pub public_id: String,
    pub hash: Vec<u8>,
    pub data: Vec<u8>,
    pub created_at: i64,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::png_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PngRequest {
    pub rowid: i64,
    pub timestamp: String,
    pub input_id: i64,
    pub renderer_key: Vec<u8>,
    pub png_id: Option<i64>,
    pub error: Option<String>,
    pub completed_at: Option<String>,
    pub cache_hit: bool,
    pub printer_identity: Option<String>,
}
