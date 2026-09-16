-- Drop IP identity, preserving request IDs, results, and timestamps.
CREATE TABLE png_requests_without_ips (
    timestamp TEXT NOT NULL,
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB,
    png_id BIGINT REFERENCES pngs(id),
    error TEXT,
    completed_at TEXT,
    cache_hit BOOLEAN NOT NULL DEFAULT 0
);
INSERT INTO png_requests_without_ips
    (rowid, timestamp, input_id, renderer_key, png_id, error, completed_at, cache_hit)
SELECT rowid, timestamp, input_id, renderer_key, png_id, error, completed_at, cache_hit
FROM png_requests;
DROP TABLE png_requests;
ALTER TABLE png_requests_without_ips RENAME TO png_requests;
DROP TABLE clients;
CREATE INDEX idx_png_requests_input_id ON png_requests(input_id);
