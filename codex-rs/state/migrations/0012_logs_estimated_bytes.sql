ALTER TABLE logs ADD COLUMN estimated_bytes INTEGER NOT NULL DEFAULT 0;

UPDATE logs
SET estimated_bytes =
    LENGTH(CAST(COALESCE(message, '') AS BLOB))
    + LENGTH(CAST(level AS BLOB))
    + LENGTH(CAST(target AS BLOB))
    + LENGTH(CAST(COALESCE(module_path, '') AS BLOB))
    + LENGTH(CAST(COALESCE(file, '') AS BLOB));
