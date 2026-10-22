-- A song's lyrics moved earlier or later by hand (Now playing's lyrics card), in ms: positive shows them later.
CREATE TABLE lyrics_offsets (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    offset_ms INTEGER NOT NULL
);
