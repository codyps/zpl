PRAGMA foreign_keys = OFF;
BEGIN TRANSACTION;

CREATE TABLE png_requests_original (
    peer_id BIGINT NOT NULL,
    timestamp TEXT NOT NULL,
    input_id BIGINT NOT NULL,
    FOREIGN KEY(peer_id) REFERENCES peers(id),
    FOREIGN KEY(input_id) REFERENCES inputs(id)
);

INSERT INTO png_requests_original (rowid, peer_id, timestamp, input_id)
SELECT rowid, peer_id, timestamp, input_id FROM png_requests;

DROP TABLE png_requests;
ALTER TABLE png_requests_original RENAME TO png_requests;

COMMIT;
PRAGMA foreign_keys = ON;
