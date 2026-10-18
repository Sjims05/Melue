-- When a playlist's own cover was last set or reverted here (ms), so the newest change wins across devices
-- (sync/cover_sync.rs). Null: never changed here.
ALTER TABLE playlists ADD COLUMN cover_changed_at INTEGER;
