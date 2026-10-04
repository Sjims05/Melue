-- Connecting the player (Phase 1 step 10).

-- Playback events already counted, by the player's event id: an event delivered twice (the app closed
-- before acknowledging it) counts once. Old ones are trimmed.
CREATE TABLE playback_events_seen (
    id TEXT PRIMARY KEY,
    seen_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX playback_events_seen_at ON playback_events_seen (seen_at);

-- track_stats counts by these rules (playback.rs): a play is a song played to the end, or left after half
-- of it or 4 minutes; a skip is a song left in its first 30 seconds; anything between is neither.
