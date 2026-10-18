-- A playlist's own cover (set in Melue: a picture cropped square), over its default one (the songs' covers; for a
-- playlist linked to Spotify, later, Spotify's picture). Stored like songs' covers (covers.rs); cleaning keeps it.
ALTER TABLE playlists ADD COLUMN cover_id INTEGER REFERENCES covers (id) ON DELETE SET NULL;
