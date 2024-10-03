CREATE TABLE pngs (
    id INTEGER PRIMARY KEY NOT NULL,
    public_id TEXT NOT NULL,
    hash BLOB NOT NULL,
    data BLOB NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_pngs_public_id ON pngs(public_id);
CREATE UNIQUE INDEX idx_pngs_hash ON pngs(hash);

CREATE TABLE inputs (
    id INTEGER PRIMARY KEY NOT NULL,
    hash BLOB NOT NULL,
    data BLOB NOT NULL,

    -- we allow these to be NULL because we might not have different zpl for
    -- rendering (ie: direct render) and we might have failed to render (ie: no
    -- png).
    png_id INTEGER,
    rendered_zpl_id INTEGER,
    -- a given input has only one output (today)
    -- TODO: cache errors?
    FOREIGN KEY(png_id) REFERENCES pngs(id),
    -- we may munge the input data before rendering it, in which case this records the munged zpl
    FOREIGN KEY(rendered_zpl_id) REFERENCES inputs(id)
);

CREATE UNIQUE INDEX idx_inputs_hash ON inputs(hash);

CREATE TABLE png_requests (
    peer_id INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    input_id INTEGER NOT NULL,
    FOREIGN KEY(peer_id) REFERENCES peers(id),
    FOREIGN KEY(input_id) REFERENCES inputs(id)
);

CREATE TABLE clients (
    id INTEGER PRIMARY KEY NOT NULL,
    ip TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_clients_ip ON clients(ip);

-- vim: set ft=sql sw=4 ts=4 sts=4 et:
