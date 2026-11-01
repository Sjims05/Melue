-- A play counted while the song was still playing (it had played long enough: playback.rs count_play_now), so leaving
-- it doesn't count it again.
ALTER TABLE plays ADD COLUMN counted INTEGER NOT NULL DEFAULT 0 CHECK (counted IN (0, 1));
