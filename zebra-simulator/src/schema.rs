diesel::table! {
    requests (id) {
        id -> BigInt,
        created_at -> Text,
        connection_id -> Text,
        transport -> Text,
        kind -> Text,
        raw -> Binary,
        response -> Binary,
        error -> Nullable<Text>,
        warnings -> Text,
        label_count -> Integer,
    }
}
diesel::table! {
    labels (request_id, page) {
        request_id -> BigInt,
        page -> Integer,
        png -> Binary,
    }
}
diesel::table! {
    settings (name) {
        name -> Text,
        value -> Text,
    }
}
