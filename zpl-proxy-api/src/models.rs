use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::inputs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Input {
    pub id: i64,
    pub hash: Vec<u8>,
    pub data: Vec<u8>,
    pub png_id: Option<i64>,
    pub rendered_zpl_id: Option<i64>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::clients)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Client {
    pub id: i64,
    pub ip: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::clients)]
pub struct NewClient<'a> {
    pub ip: &'a str,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::pngs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Png {
    pub id: i64,
    pub hash: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::png_requests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PngRequest {
    pub rowid: i64,
    pub peer_id: i64,
    pub timestamp: String,
    pub input_id: i64,
}
