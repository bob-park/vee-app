-- Pinned clips are never removed automatically; excluded apps are never recorded.
ALTER TABLE clips ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE apps ADD COLUMN excluded INTEGER NOT NULL DEFAULT 0;
