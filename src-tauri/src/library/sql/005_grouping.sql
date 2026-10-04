-- Grouping rules (Phase 1 step 6). Songs without tags now get their title, artist and track number from
-- the file name ("01 Artist - Title.mp3"), when their tags are read: read those again.
UPDATE tracks SET tags_read_at = NULL WHERE tag_title IS NULL OR artist_credit IS NULL OR track_no IS NULL;

-- Albums and artists are rebuilt from the tags after every scan (grouping.rs). An artist's albums:
CREATE INDEX albums_artist ON albums (artist_id);
