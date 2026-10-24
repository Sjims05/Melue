-- A folder that stopped being the synced one before Melue took its name back (a fix in 0.5.22) keeps the share's
-- name ("Synced music", "Share"), and looks like the synced folder: it's called after its place again.
UPDATE folders SET name = NULL WHERE synced = 0 AND name IN ('Synced music', 'Share');
