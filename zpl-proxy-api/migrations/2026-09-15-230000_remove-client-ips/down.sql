-- IPs cannot be recovered by rollback. Use an anonymous placeholder to restore
-- the old schema without fabricating IP addresses.
CREATE TABLE clients (id INTEGER PRIMARY KEY NOT NULL, ip TEXT NOT NULL);
CREATE UNIQUE INDEX idx_clients_ip ON clients(ip);
INSERT INTO clients (id, ip) VALUES (1, '');
CREATE TABLE png_requests_with_peers (
    peer_id BIGINT NOT NULL REFERENCES clients(id),
    timestamp TEXT NOT NULL,
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB,
    png_id BIGINT REFERENCES pngs(id),
    error TEXT,
    completed_at TEXT,
    cache_hit BOOLEAN NOT NULL DEFAULT 0
);
INSERT INTO png_requests_with_peers
    (rowid, peer_id, timestamp, input_id, renderer_key, png_id, error, completed_at, cache_hit)
SELECT rowid, 1, timestamp, input_id, renderer_key, png_id, error, completed_at, cache_hit
FROM png_requests;
DROP TABLE png_requests;
ALTER TABLE png_requests_with_peers RENAME TO png_requests;
CREATE INDEX idx_png_requests_input_id ON png_requests(input_id);
