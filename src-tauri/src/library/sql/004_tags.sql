-- Tags (Phase 1 step 5). Scans read tags in a second pass: a track whose file is new or changed gets
-- tags_read_at = NULL and the second pass fills in its tags and cover, so an interrupted pass carries on
-- the next time.

-- When the file's tags were last read (null: still to read).
ALTER TABLE tracks ADD COLUMN tags_read_at INTEGER;
-- Why the tags couldn't be read (the track keeps its file-name title and still plays).
ALTER TABLE tracks ADD COLUMN tags_error TEXT;

-- The tags as the file has them, for the grouping rules (step 6) to build albums and artists from
-- without reading the files again. title / sort_title / artist_credit are the values the app shows.
ALTER TABLE tracks ADD COLUMN tag_title TEXT;
ALTER TABLE tracks ADD COLUMN tag_album TEXT;
ALTER TABLE tracks ADD COLUMN tag_album_artist TEXT;
ALTER TABLE tracks ADD COLUMN tag_compilation INTEGER NOT NULL DEFAULT 0 CHECK (tag_compilation IN (0, 1));
ALTER TABLE tracks ADD COLUMN tag_title_sort TEXT;
ALTER TABLE tracks ADD COLUMN tag_artist_sort TEXT;
ALTER TABLE tracks ADD COLUMN tag_album_sort TEXT;
ALTER TABLE tracks ADD COLUMN tag_album_artist_sort TEXT;

-- A cover image next to the file (cover.jpg, folder.jpg, ...), in the same form as path, and its
-- modified time: used when the file has no cover of its own; a new or replaced image is read again.
ALTER TABLE tracks ADD COLUMN folder_cover TEXT;
ALTER TABLE tracks ADD COLUMN folder_cover_modified_at INTEGER;

CREATE INDEX tracks_tags_to_read ON tracks (id) WHERE tags_read_at IS NULL;

-- covers.file is the covers' base name in the covers folder: the small (300 px) and large (1000 px)
-- versions are <file>-300.jpg and <file>-1000.jpg. width / height are the original's.
