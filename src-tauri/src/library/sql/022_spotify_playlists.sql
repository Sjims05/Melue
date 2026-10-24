-- Spotify playlists (spotify.rs): linked playlists whose "file" is a Spotify playlist (file_path 'spotify:<id>', no
-- folder). Its id there, how many songs it has there, what its first songs and count were at the last update (a long
-- one's), and whether it changed on Spotify since (the "!": update the list).
ALTER TABLE playlists ADD COLUMN spotify_id TEXT;
ALTER TABLE playlists ADD COLUMN spotify_total INTEGER;
ALTER TABLE playlists ADD COLUMN spotify_seen TEXT;
ALTER TABLE playlists ADD COLUMN spotify_changed INTEGER NOT NULL DEFAULT 0 CHECK (spotify_changed IN (0, 1));
