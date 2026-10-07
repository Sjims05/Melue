-- Library data across devices (Phase 2 step 8). See sync/data.rs and sync/playlist_sync.rs.

-- Unliked songs, with when: the newest like or unlike of a song wins across devices.
CREATE TABLE unlikes (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    unliked_at INTEGER NOT NULL
) STRICT;

-- Other devices' plays of songs on the share, read from their files (replaced on every read). Times are on
-- this device's clock.
CREATE TABLE remote_track_stats (
    device_id TEXT NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    play_count INTEGER NOT NULL,
    last_played_at INTEGER,
    PRIMARY KEY (device_id, track_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE remote_play_days (
    device_id TEXT NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    day INTEGER NOT NULL,
    plays INTEGER NOT NULL,
    PRIMARY KEY (device_id, track_id, day)
) STRICT, WITHOUT ROWID;

CREATE INDEX remote_play_days_day ON remote_play_days (day);

-- Their latest songs played (the shared history).
CREATE TABLE remote_plays (
    device_id TEXT NOT NULL,
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL
) STRICT;

CREATE INDEX remote_plays_time ON remote_plays (started_at);

-- This device's plays and the other devices' together.
CREATE VIEW all_track_stats AS
SELECT track_id, sum(play_count) AS play_count, max(last_played_at) AS last_played_at
FROM (SELECT track_id, play_count, last_played_at FROM track_stats
      UNION ALL SELECT track_id, play_count, last_played_at FROM remote_track_stats)
GROUP BY track_id;

CREATE VIEW all_play_days AS
SELECT track_id, day, plays FROM play_days UNION ALL SELECT track_id, day, plays FROM remote_play_days;

CREATE VIEW all_plays AS
SELECT track_id, started_at FROM plays UNION ALL SELECT track_id, started_at FROM remote_plays;

-- Playlists on the share: its id there (app playlists, once synced), and "Only on this device".
ALTER TABLE playlists ADD COLUMN sync_id TEXT;
ALTER TABLE playlists ADD COLUMN local_only INTEGER NOT NULL DEFAULT 0 CHECK (local_only IN (0, 1));
CREATE UNIQUE INDEX playlists_sync_id ON playlists (sync_id) WHERE sync_id IS NOT NULL;

-- A synced playlist's song that isn't on this device (yet): no track, file_entry its key on the share, and
-- its title and artist to show it greyed.
ALTER TABLE playlist_entries ADD COLUMN sync_title TEXT;
ALTER TABLE playlist_entries ADD COLUMN sync_artist TEXT;

-- Each synced playlist as it was at this device's last sync: what both sides changed since (the merge).
CREATE TABLE playlist_sync_base (
    sync_id TEXT PRIMARY KEY,
    data TEXT NOT NULL
) STRICT, WITHOUT ROWID;

-- Changes that couldn't be merged, waiting for the user's choice (the playlist isn't synced meanwhile).
CREATE TABLE playlist_conflicts (
    sync_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('changed', 'deleted_there', 'deleted_here')),
    -- The share's version (none when deleted there).
    theirs TEXT,
    device_name TEXT NOT NULL,
    found_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Synced playlists deleted (here or on another device): restorable for 30 days.
CREATE TABLE deleted_playlists (
    sync_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    data TEXT NOT NULL,
    deleted_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
