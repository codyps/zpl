DROP INDEX idx_png_requests_input_id;
ALTER TABLE png_requests DROP COLUMN cache_hit;
ALTER TABLE png_requests DROP COLUMN completed_at;
ALTER TABLE png_requests DROP COLUMN error;
ALTER TABLE png_requests DROP COLUMN png_id;
ALTER TABLE png_requests DROP COLUMN renderer_key;
DROP TABLE render_cache;
