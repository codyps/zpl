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
