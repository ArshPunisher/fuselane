-- Uploads in progress and the share links they become (BONDED-UPLOADS.md §3-4).
CREATE TABLE uploads (
  id TEXT PRIMARY KEY,            -- our id (random); R2's upload id is separate
  device TEXT NOT NULL,           -- who started it (device key hash)
  object_key TEXT NOT NULL,
  r2_upload_id TEXT NOT NULL,
  name TEXT NOT NULL,
  size INTEGER NOT NULL,
  mime TEXT NOT NULL,
  part_size INTEGER NOT NULL,
  part_count INTEGER NOT NULL,
  expires_days INTEGER NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('open', 'completed', 'aborted')),
  created_at INTEGER NOT NULL
);
CREATE INDEX uploads_device ON uploads(device, created_at);

CREATE TABLE shares (
  slug TEXT PRIMARY KEY,          -- 128-bit random, base62
  upload_id TEXT NOT NULL REFERENCES uploads(id),
  object_key TEXT NOT NULL,
  name TEXT NOT NULL,
  size INTEGER NOT NULL,
  mime TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  max_downloads INTEGER,
  downloads INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
);
CREATE INDEX shares_expiry ON shares(expires_at);
