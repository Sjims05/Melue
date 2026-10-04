-- Linked playlists (Phase 1 step 9): playlist files (.m3u, .m3u8) in the music folders.

-- Set when a scan no longer finds a linked playlist's file (deleted, or its folder removed). The playlist
-- stays, marked "file gone"; it can be kept (unlinked into an app playlist) or deleted, and links up again
-- if the file comes back.
ALTER TABLE playlists ADD COLUMN file_missing_since INTEGER;

-- Playlist files the user let go of (unlinked, or deleted their playlist): not linked again by later scans.
CREATE TABLE playlist_files_ignored (
    folder_id INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    PRIMARY KEY (folder_id, path)
) STRICT, WITHOUT ROWID;
