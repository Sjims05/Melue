-- Lines tapped as they started being sung (Now playing's lyrics, "Sync by tapping"): JSON [{ "line": 3, "atMs": 41250 }],
-- in line order. Lines after a tapped one follow it (sync with lyrics.rs and LyricsPanel.vue).
ALTER TABLE lyrics_offsets ADD COLUMN anchors TEXT;
