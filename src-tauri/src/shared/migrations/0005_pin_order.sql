-- Pins are capped at `MAX_PINNED` and keep the order they were pinned in.
-- Existing users keep their most recently used pins; the rest are unpinned, not deleted.
ALTER TABLE clips ADD COLUMN pinned_at INTEGER;
UPDATE clips SET pinned = 0
 WHERE pinned = 1
   AND id NOT IN (SELECT id FROM clips WHERE pinned = 1 ORDER BY last_used_at DESC, id DESC LIMIT 3);
UPDATE clips SET pinned_at = last_used_at WHERE pinned = 1;
