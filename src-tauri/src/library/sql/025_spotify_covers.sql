-- A Spotify playlist's picture (spotify.rs): its address on Spotify (synced with the playlist), the picture as
-- downloaded here (a cover like songs') and the address it came from, and whether its songs' covers are shown
-- instead (a choice that syncs too).
ALTER TABLE playlists ADD COLUMN spotify_cover_url TEXT;
ALTER TABLE playlists ADD COLUMN spotify_cover_id INTEGER REFERENCES covers (id) ON DELETE SET NULL;
ALTER TABLE playlists ADD COLUMN spotify_cover_of TEXT;
ALTER TABLE playlists ADD COLUMN songs_cover INTEGER NOT NULL DEFAULT 0 CHECK (songs_cover IN (0, 1));
