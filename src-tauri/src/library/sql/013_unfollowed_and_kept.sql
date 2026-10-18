-- A linked playlist can stop following its file ("Unfollowed"): it stays linked to it, but scans don't bring the
-- file's changes in and write-back is off; its greyed songs (removed here, suggested) are kept but not shown, and
-- following again shows how it differs from the file again. Like "only on this device", it isn't synced.
ALTER TABLE playlists ADD COLUMN following INTEGER NOT NULL DEFAULT 1 CHECK (following IN (0, 1));

-- Linked playlists deleted with changes of the user's: kept on this device for 30 days (Recently deleted), to
-- restore as they were (followed or not), replacing the file's playlist if it was followed again meanwhile.
-- `data`: the playlist's settings, its entries and the file as last read (JSON, see linked.rs).
CREATE TABLE deleted_linked_playlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    songs INTEGER NOT NULL,
    deleted_at INTEGER NOT NULL,
    folder_id INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    data TEXT NOT NULL
) STRICT;
