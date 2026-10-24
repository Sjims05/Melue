-- "Keep only on this device" and remove it from the other devices (playlist_sync.rs): done at the next sync.
ALTER TABLE playlists ADD COLUMN unshare INTEGER NOT NULL DEFAULT 0 CHECK (unshare IN (0, 1));
