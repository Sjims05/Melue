-- Auto playlists (Phase 2 step 7): plays per song per day, for the Top songs lists of the last month, half year
-- and year. Never trimmed (the history, plays, keeps only the latest songs started); small: one row per song
-- per day it was played. Days are UTC days since 1970. Counted like track_stats' plays (playback.rs).
CREATE TABLE play_days (
    track_id INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    day INTEGER NOT NULL,
    plays INTEGER NOT NULL,
    PRIMARY KEY (track_id, day)
) STRICT, WITHOUT ROWID;

CREATE INDEX play_days_day ON play_days (day);

-- What the history has so far: played to the end, or left after half the song or 4 minutes.
INSERT INTO play_days (track_id, day, plays)
SELECT p.track_id, p.started_at / 86400000, count(*)
FROM plays p JOIN tracks t ON t.id = p.track_id
WHERE p.completed = 1
   OR p.position_ms >= min(coalesce(nullif(t.duration_ms, 0) / 2, 240000), 240000)
GROUP BY p.track_id, p.started_at / 86400000;
