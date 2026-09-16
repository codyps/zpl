-- Preserve legacy input/PNG records. Legacy associations have no renderer
-- identity and therefore must not be used as cache hits.
CREATE TABLE render_cache (
    input_id BIGINT NOT NULL REFERENCES inputs(id),
    renderer_key BLOB NOT NULL,
    png_id BIGINT NOT NULL REFERENCES pngs(id),
    PRIMARY KEY (input_id, renderer_key)
);

ALTER TABLE png_requests ADD COLUMN renderer_key BLOB;
ALTER TABLE png_requests ADD COLUMN png_id BIGINT REFERENCES pngs(id);
ALTER TABLE png_requests ADD COLUMN error TEXT;
ALTER TABLE png_requests ADD COLUMN completed_at TEXT;
ALTER TABLE png_requests ADD COLUMN cache_hit BOOLEAN NOT NULL DEFAULT 0;
CREATE INDEX idx_png_requests_input_id ON png_requests(input_id);
