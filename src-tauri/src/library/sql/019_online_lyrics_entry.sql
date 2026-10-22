-- Which LRCLIB entry a song's lyrics came from (library/online_lyrics.rs), so "Try another version" takes the next.
ALTER TABLE online_lyrics ADD COLUMN lrclib_id INTEGER;
