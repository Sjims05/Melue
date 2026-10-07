-- A synced playlist's song that isn't on this device: its length too, to recognise the same song elsewhere
-- here (by artist, title and length; see sync/same_song.rs).
ALTER TABLE playlist_entries ADD COLUMN sync_duration_ms INTEGER;
