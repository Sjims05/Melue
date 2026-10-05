-- The sync (Phase 2 step 3): every file synced from the share into the synced folder, as it was on the share
-- and as the local copy was right after syncing. Tells a file changed on the share from one changed (or
-- deleted) on this device, a file moved on the share from a new one, and songs synced before from songs put
-- into the folder by hand. Paths are relative to the music folder on the share (and the synced folder).
CREATE TABLE synced_files (
    path TEXT PRIMARY KEY,
    share_size INTEGER NOT NULL,
    share_modified_ms INTEGER NOT NULL,
    local_size INTEGER NOT NULL,
    local_modified_ms INTEGER NOT NULL,
    synced_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
