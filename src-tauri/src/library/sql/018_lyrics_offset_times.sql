-- When a song's lyrics timing was last changed here (ms), so the newest change wins across devices (sync/data.rs).
-- A reset to 0 is kept as a change too.
ALTER TABLE lyrics_offsets ADD COLUMN changed_at INTEGER NOT NULL DEFAULT 0;
