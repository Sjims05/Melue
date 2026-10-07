-- Moving songs to sync (Phase 2 step 9, sync/moving.rs).

-- A local song that's a copy of a song in the synced folder (the same artist, title and length; or moved to sync):
-- hidden in the library (its likes, plays and playlist entries moved to the synced song). The user decides whether
-- to clear the space or keep the hidden copy (kept: not asked about again).
ALTER TABLE tracks ADD COLUMN copy_of INTEGER REFERENCES tracks (id) ON DELETE SET NULL;
ALTER TABLE tracks ADD COLUMN copy_kept INTEGER NOT NULL DEFAULT 0 CHECK (copy_kept IN (0, 1));
CREATE INDEX tracks_copy_of ON tracks (copy_of) WHERE copy_of IS NOT NULL;

-- Songs moved to sync, waiting for their synced copy to be in the library: then the original is removed.
CREATE TABLE moves (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    -- Where it went, on the share (and in the synced folder).
    share_path TEXT NOT NULL,
    moved_at INTEGER NOT NULL
) STRICT;
