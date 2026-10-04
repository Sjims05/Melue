-- 3: per-track play counts.
--
-- Per-track counts, kept up to date on every play, so they stay right even if the play history (plays) is
-- trimmed (a possible "keep history for ..." setting) and cost nothing to read.
CREATE TABLE track_stats (
    track_id INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    -- Played to the end.
    play_count INTEGER NOT NULL DEFAULT 0,
    -- Left before the end.
    skip_count INTEGER NOT NULL DEFAULT 0,
    last_played_at INTEGER
) STRICT;
