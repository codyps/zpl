-- SQLite foreign keys and constraints:
-- https://www.sqlite.org/foreignkeys.html
CREATE TABLE pngs (
    id INTEGER PRIMARY KEY NOT NULL,
    public_id TEXT NOT NULL UNIQUE,
    hash BLOB NOT NULL UNIQUE,
    data BLOB NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE inputs (
    id INTEGER PRIMARY KEY NOT NULL,
    hash BLOB NOT NULL UNIQUE,
    data BLOB NOT NULL
);

-- Identity belongs to a render: distinct printers can produce identical PNGs.
CREATE TABLE render_cache (
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB NOT NULL,
    png_id BIGINT NOT NULL REFERENCES pngs(id),
    printer_identity TEXT NOT NULL,
    PRIMARY KEY (input_id, renderer_key)
);

CREATE TABLE png_requests (
    timestamp TEXT NOT NULL,
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB NOT NULL,
    png_id BIGINT REFERENCES pngs(id),
    error TEXT,
    completed_at TEXT,
    cache_hit BOOLEAN NOT NULL DEFAULT 0,
    -- Pending requests and failures before preview have no captured identity.
    printer_identity TEXT
);
CREATE INDEX idx_png_requests_input_id ON png_requests(input_id);

-- Immutable identity snapshots per accepted request and per actual preview/control.
-- png_requests uses SQLite implicit rowid (not a legal foreign-key parent).
CREATE TABLE printer_requests (
    request_id BIGINT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    serial TEXT,
    firmware TEXT
);
CREATE TABLE preview_attempts (
    id INTEGER PRIMARY KEY NOT NULL,
    request_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    serial TEXT NOT NULL,
    firmware TEXT NOT NULL,
    phase TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT,
    failure_kind TEXT CHECK (failure_kind IN ('hang', 'rejected', 'unavailable', 'recovery_failed')),
    error TEXT
);
CREATE INDEX idx_preview_attempts_request ON preview_attempts(request_id);
CREATE TABLE permanent_errors (
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB NOT NULL,
    error TEXT NOT NULL,
    PRIMARY KEY (input_id, renderer_key)
);
CREATE TABLE printer_recovery (
    renderer_key BLOB PRIMARY KEY NOT NULL,
    retry_at BIGINT NOT NULL
);
