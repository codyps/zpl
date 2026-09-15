BEGIN TRANSACTION;

CREATE TABLE png_requests_fixed (
    peer_id BIGINT NOT NULL,
    timestamp TEXT NOT NULL,
    input_id BIGINT NOT NULL,
    FOREIGN KEY(peer_id) REFERENCES clients(id),
    FOREIGN KEY(input_id) REFERENCES inputs(id)
);

INSERT INTO png_requests_fixed (rowid, peer_id, timestamp, input_id)
SELECT rowid, peer_id, timestamp, input_id FROM png_requests;

DROP TABLE png_requests;
ALTER TABLE png_requests_fixed RENAME TO png_requests;

COMMIT;
