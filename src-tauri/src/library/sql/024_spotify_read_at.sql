-- When a Spotify playlist's list (or count, or "!") last changed from reading Spotify: synced with the playlist, the
-- newest read wins (playlist_sync.rs).
ALTER TABLE playlists ADD COLUMN spotify_read_at INTEGER NOT NULL DEFAULT 0;
