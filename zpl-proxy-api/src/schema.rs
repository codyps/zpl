// @generated automatically by Diesel CLI.

diesel::table! {
    clients (id) {
        id -> Integer,
        ip -> Text,
    }
}

diesel::table! {
    inputs (id) {
        id -> Integer,
        hash -> Binary,
        data -> Binary,
        png_id -> Nullable<Integer>,
        rendered_zpl_id -> Nullable<Integer>,
    }
}

diesel::table! {
    png_requests (rowid) {
        rowid -> Integer,
        peer_id -> Integer,
        timestamp -> Text,
        input_id -> Integer,
    }
}

diesel::table! {
    pngs (id) {
        id -> Integer,
        public_id -> Text,
        hash -> Binary,
        data -> Binary,
        created_at -> Integer,
    }
}

diesel::joinable!(inputs -> pngs (png_id));
diesel::joinable!(png_requests -> inputs (input_id));

diesel::allow_tables_to_appear_in_same_query!(
    clients,
    inputs,
    png_requests,
    pngs,
);
