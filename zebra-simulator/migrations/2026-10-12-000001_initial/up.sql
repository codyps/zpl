CREATE TABLE requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    connection_id TEXT NOT NULL,
    transport TEXT NOT NULL,
    kind TEXT NOT NULL,
    raw BLOB NOT NULL,
    response BLOB NOT NULL,
    error TEXT,
    warnings TEXT NOT NULL,
    label_count INTEGER NOT NULL
);
CREATE TABLE labels (
    request_id BIGINT NOT NULL REFERENCES requests(id),
    page INTEGER NOT NULL,
    png BLOB NOT NULL,
    PRIMARY KEY (request_id, page)
);
CREATE TABLE settings (
    name TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
