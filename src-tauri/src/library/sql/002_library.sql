-- 2: the library itself.
--
-- Times are milliseconds since 1970 (UTC). Text "keys" (name_key, album_key) are names normalized by the
-- Rust side (lowercased, trimmed, spaces collapsed), so "The Beatles" and "the beatles " are one artist.

-- Where music comes from. A library can have several (and mix local folders with a synced one).
CREATE TABLE folders (
    id INTEGER PRIMARY KEY,
    -- path: a folder on a disk (desktop, or a plain path on Android).
    -- android_media: all music on the phone, through Android's media index (location is '').
    -- android_tree: a folder chosen through Android's storage picker (location is its content:// tree URI).
    kind TEXT NOT NULL CHECK (kind IN ('path', 'android_media', 'android_tree')),
    location TEXT NOT NULL,
    -- Shown in settings; defaults to the folder's own name.
    name TEXT,
    -- Unticked folders keep their tracks in the database (playlists, history) but hide them.
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    -- The folder a share syncs into (Phase 2).
    synced INTEGER NOT NULL DEFAULT 0 CHECK (synced IN (0, 1)),
    added_at INTEGER NOT NULL,
    last_scan_at INTEGER,
    UNIQUE (kind, location)
) STRICT;

-- Cover images, stored once each as files in the app's data folder (covers/<hash>.<ext>), shared by
-- every track and album that has the same picture.
CREATE TABLE covers (
    id INTEGER PRIMARY KEY,
    -- Content hash of the original image, so the same cover is kept once.
    hash TEXT NOT NULL UNIQUE,
    -- The thumbnail's file name in the covers folder.
    file TEXT NOT NULL,
    width INTEGER,
    height INTEGER
) STRICT;

CREATE TABLE artists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL UNIQUE,
    -- For sorting: "Beatles, The" style rules are applied by the Rust side.
    sort_name TEXT NOT NULL
) STRICT;

CREATE TABLE albums (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    -- The album artist; null for a compilation ("Various Artists").
    artist_id INTEGER REFERENCES artists (id) ON DELETE SET NULL,
    compilation INTEGER NOT NULL DEFAULT 0 CHECK (compilation IN (0, 1)),
    -- Albums are told apart by title and album artist (normalized), see the grouping rules.
    album_key TEXT NOT NULL UNIQUE,
    year INTEGER,
    cover_id INTEGER REFERENCES covers (id) ON DELETE SET NULL
) STRICT;

CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    folder_id INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    -- Where the file is, relative to its folder (or a content:// URI for android_media / android_tree).
    path TEXT NOT NULL,
    -- What an incremental scan compares to tell if the file changed.
    size INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    -- Tags, with fallbacks from the file and folder names applied (title is never empty).
    title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    -- The artist as tagged ("A feat. B"); the individual artists are in track_artists.
    artist_credit TEXT,
    album_id INTEGER REFERENCES albums (id) ON DELETE SET NULL,
    track_no INTEGER,
    track_total INTEGER,
    disc_no INTEGER,
    disc_total INTEGER,
    year INTEGER,
    genre TEXT,
    duration_ms INTEGER,
    -- mp3, flac, aac, alac, vorbis, wav, ...
    format TEXT,
    bitrate_kbps INTEGER,
    sample_rate INTEGER,
    cover_id INTEGER REFERENCES covers (id) ON DELETE SET NULL,
    -- First seen (for "recently added").
    added_at INTEGER NOT NULL,
    -- Set when a scan no longer finds the file. The row stays (likes, playlists and history keep pointing
    -- at it) and comes back if the file does, e.g. an SD card that's out or a share that's offline.
    missing_since INTEGER,
    UNIQUE (folder_id, path)
) STRICT;

CREATE INDEX tracks_album ON tracks (album_id, disc_no, track_no);
CREATE INDEX tracks_added ON tracks (added_at);

-- The artists of a track: its main artists and those credited with "feat.", in credit order.
CREATE TABLE track_artists (
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('main', 'featured')),
    position INTEGER NOT NULL,
    PRIMARY KEY (track_id, artist_id, role)
) STRICT, WITHOUT ROWID;

CREATE INDEX track_artists_artist ON track_artists (artist_id);

-- Playlists made in Melue ('app'), and playlist files (.m3u) from the music folders ('linked').
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('app', 'linked')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    -- Linked playlists: the file, and what it looked like when it was last read.
    file_folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
    file_path TEXT,
    file_size INTEGER,
    file_modified_at INTEGER,
    -- Linked playlists' settings: songs new in the file are 'added' or shown as 'suggested'; songs removed
    -- from the file are 'removed' or 'kept' (marked); write_back copies Melue's changes into the file.
    new_in_file TEXT NOT NULL DEFAULT 'added' CHECK (new_in_file IN ('added', 'suggested')),
    removed_from_file TEXT NOT NULL DEFAULT 'removed' CHECK (removed_from_file IN ('removed', 'kept')),
    write_back INTEGER NOT NULL DEFAULT 0 CHECK (write_back IN (0, 1)),
    CHECK ((kind = 'linked') = (file_path IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX playlists_file ON playlists (file_folder_id, file_path) WHERE kind = 'linked';

-- A playlist's songs, in order. For linked playlists each entry also says how it relates to the file:
--   both: in Melue's playlist and in the file;
--   here_only: added in Melue (not in the file);
--   removed_here: still in the file, removed in Melue (shown greyed, doesn't play);
--   suggested: new in the file and not added yet (shown greyed, doesn't play);
--   file_removed: removed from the file but kept in Melue (removed_from_file = 'kept'; marked).
-- App playlists' entries are always 'both'.
CREATE TABLE playlist_entries (
    id INTEGER PRIMARY KEY,
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    -- Null when a linked playlist's file names a song that isn't in the library (file_entry says which).
    -- Deleting a track (not just missing: its folder removed) deletes its entries; a linked playlist's
    -- file lines are still in playlist_snapshots and get matched again.
    track_id INTEGER REFERENCES tracks (id) ON DELETE CASCADE,
    -- The line from the playlist file, for linked playlists.
    file_entry TEXT,
    state TEXT NOT NULL DEFAULT 'both'
        CHECK (state IN ('both', 'here_only', 'removed_here', 'suggested', 'file_removed')),
    added_at INTEGER NOT NULL,
    CHECK (track_id IS NOT NULL OR file_entry IS NOT NULL)
) STRICT;

CREATE INDEX playlist_entries_order ON playlist_entries (playlist_id, position);
CREATE INDEX playlist_entries_track ON playlist_entries (track_id);

-- A linked playlist's file as last read (its entries in order), to tell what the other app changed since.
CREATE TABLE playlist_snapshots (
    playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    file_entry TEXT NOT NULL,
    PRIMARY KEY (playlist_id, position)
) STRICT, WITHOUT ROWID;

CREATE TABLE likes (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    liked_at INTEGER NOT NULL
) STRICT;

-- Every time a track played (from the player's playback events): listening history and "recently played".
-- Counts come from track_stats (migration 3), not from here.
CREATE TABLE plays (
    id INTEGER PRIMARY KEY,
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    started_at INTEGER NOT NULL,
    -- Played to the end (complete) or left early (skip, with where).
    completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
    position_ms INTEGER,
    -- What it was played from: 'playlist:<id>', 'album:<id>', 'artist:<id>', ...
    context TEXT
) STRICT;

CREATE INDEX plays_track ON plays (track_id, started_at);
CREATE INDEX plays_time ON plays (started_at);
