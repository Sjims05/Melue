-- Lyrics found online (library/online_lyrics.rs: LRCLIB) for songs without their own, kept so they work offline.
-- `lrc`: the lyrics (timed LRC, or plain lines); null: not found there. `instrumental`: LRCLIB says it has no words.
-- `looked_at`: when (ms), so a song not found is asked about again after a while, not on every play.
CREATE TABLE online_lyrics (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    lrc TEXT,
    instrumental INTEGER NOT NULL DEFAULT 0,
    looked_at INTEGER NOT NULL
);
