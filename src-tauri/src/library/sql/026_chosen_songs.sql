-- A playlist entry whose song was chosen by hand (Find song: a Spotify playlist's song matched to one of yours that the
-- automatic match left out, like a longer version). Kept when the playlist updates, and synced to the other devices.
ALTER TABLE playlist_entries ADD COLUMN chosen INTEGER NOT NULL DEFAULT 0;
