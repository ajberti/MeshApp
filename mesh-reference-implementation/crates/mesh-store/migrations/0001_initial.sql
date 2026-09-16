PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_meta (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS bundles (
    bundle_id BLOB PRIMARY KEY CHECK(length(bundle_id) = 16),
    encoded_bundle BLOB NOT NULL,
    destination_id BLOB NOT NULL CHECK(length(destination_id) = 16),
    bundle_type INTEGER NOT NULL,
    priority INTEGER NOT NULL,
    first_seen_at_ms INTEGER NOT NULL,
    forward_count INTEGER NOT NULL DEFAULT 0,
    state INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_bundles_destination
ON bundles(destination_id);

CREATE INDEX IF NOT EXISTS idx_bundles_priority
ON bundles(priority DESC, first_seen_at_ms ASC);

CREATE TABLE IF NOT EXISTS tombstones (
    bundle_id BLOB PRIMARY KEY CHECK(length(bundle_id) = 16),
    reason INTEGER NOT NULL,
    created_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tombstones_expiry
ON tombstones(expires_at_ms);
