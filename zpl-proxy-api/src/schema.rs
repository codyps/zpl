// @generated automatically by Diesel CLI.

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
        timestamp -> Text,
        input_id -> BigInt,
        renderer_key -> Nullable<Binary>,
        png_id -> Nullable<BigInt>,
        error -> Nullable<Text>,
        completed_at -> Nullable<Text>,
        cache_hit -> Bool,
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

diesel::table! {
    render_cache (input_id, renderer_key) {
        input_id -> BigInt,
        renderer_key -> Binary,
        png_id -> BigInt,
    }
}

diesel::joinable!(inputs -> pngs (png_id));
diesel::joinable!(png_requests -> inputs (input_id));
diesel::joinable!(png_requests -> pngs (png_id));
diesel::joinable!(render_cache -> inputs (input_id));
diesel::joinable!(render_cache -> pngs (png_id));

diesel::allow_tables_to_appear_in_same_query!(inputs, png_requests, pngs, render_cache,);
