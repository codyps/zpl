// @generated automatically by Diesel CLI.

diesel::table! {
    clients (id) {
        id -> BigInt,
        ip -> Text,
    }
}

diesel::table! {
    inputs (id) {
        id -> BigInt,
        hash -> Binary,
        data -> Binary,
        png_id -> Nullable<BigInt>,
        rendered_zpl_id -> Nullable<BigInt>,
    }
}

diesel::table! {
    png_requests (rowid) {
        rowid -> BigInt,
        peer_id -> BigInt,
        timestamp -> Text,
        input_id -> BigInt,
    }
}

diesel::table! {
    pngs (id) {
        id -> BigInt,
        public_id -> Text,
        hash -> Binary,
        data -> Binary,
        created_at -> BigInt,
    }
}

diesel::joinable!(inputs -> pngs (png_id));
diesel::joinable!(png_requests -> clients (peer_id));
diesel::joinable!(png_requests -> inputs (input_id));

diesel::allow_tables_to_appear_in_same_query!(clients, inputs, png_requests, pngs,);
