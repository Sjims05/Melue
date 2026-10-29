-- A Spotify playlist's songs' ids on Spotify (spotify.rs), by their lines: for links to them (Copy Spotify links to the
-- songs you don't have). Kept apart from the lines, which say what a song is (its artist, title and length) and stay
-- as they were. Synced in the playlist's reference.
CREATE TABLE spotify_tracks (
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    line TEXT NOT NULL,
    track TEXT NOT NULL,
    PRIMARY KEY (playlist_id, line)
);
