-- Keep pngs.data and its hash untouched. Identity belongs to a render, since
-- different printers/firmware may produce identical original PNG bytes.
ALTER TABLE render_cache ADD COLUMN printer_identity TEXT;
ALTER TABLE png_requests ADD COLUMN printer_identity TEXT;
