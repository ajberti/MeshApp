use mesh_types::BundleId;
use mesh_wire::WireBundle;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use thiserror::Error;

const MIGRATION_0001: &str = include_str!("../migrations/0001_initial.sql");

pub struct MeshStore {
    conn: Connection,
}

impl MeshStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.configure()?;
        store.migrate()?;
        Ok(store)
    }

    fn configure(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys = ON;\nPRAGMA journal_mode = WAL;\nPRAGMA synchronous = NORMAL;",
        )?;
        Ok(())
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(MIGRATION_0001)?;
        let version: Option<i64> = self
            .conn
            .query_row("SELECT version FROM schema_meta LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?;
        if version.is_none() {
            self.conn
                .execute("INSERT INTO schema_meta(version) VALUES (1)", [])?;
        }
        Ok(())
    }

    pub fn contains_bundle(&self, bundle_id: BundleId) -> Result<bool, StoreError> {
        let value: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM bundles WHERE bundle_id = ?1 LIMIT 1",
                params![bundle_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.is_some())
    }

    pub fn is_tombstoned(&self, bundle_id: BundleId) -> Result<bool, StoreError> {
        let value: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM tombstones WHERE bundle_id = ?1 LIMIT 1",
                params![bundle_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.is_some())
    }

    pub fn insert_bundle(
        &mut self,
        bundle: &WireBundle,
        first_seen_at_ms: i64,
    ) -> Result<InsertOutcome, StoreError> {
        if self.is_tombstoned(bundle.immutable.bundle_id)? {
            return Ok(InsertOutcome::RejectedTombstoned);
        }
        if self.contains_bundle(bundle.immutable.bundle_id)? {
            return Ok(InsertOutcome::Duplicate);
        }

        let encoded = bundle.encode_cbor()?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO bundles(\
                bundle_id, encoded_bundle, destination_id, bundle_type, priority, first_seen_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                bundle.immutable.bundle_id.as_bytes().as_slice(),
                encoded,
                bundle.immutable.destination_id.as_bytes().as_slice(),
                bundle.immutable.bundle_type as i64,
                bundle.immutable.priority as i64,
                first_seen_at_ms,
            ],
        )?;
        tx.commit()?;
        Ok(InsertOutcome::Inserted)
    }

    pub fn list_bundles(&self) -> Result<Vec<StoredBundle>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT encoded_bundle, first_seen_at_ms, forward_count \
             FROM bundles ORDER BY priority DESC, first_seen_at_ms ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;

        let mut result = Vec::new();
        for row in rows {
            let (bytes, first_seen_at_ms, forward_count) = row?;
            let bundle = WireBundle::decode_cbor(&bytes)?;
            result.push(StoredBundle {
                bundle,
                first_seen_at_ms,
                forward_count,
            });
        }
        Ok(result)
    }

    pub fn increment_forward_count(&self, bundle_id: BundleId) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE bundles SET forward_count = forward_count + 1 WHERE bundle_id = ?1",
            params![bundle_id.as_bytes().as_slice()],
        )?;
        Ok(())
    }

    pub fn create_tombstone(
        &mut self,
        bundle_id: BundleId,
        reason: i64,
        created_at_ms: i64,
        expires_at_ms: i64,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM bundles WHERE bundle_id = ?1",
            params![bundle_id.as_bytes().as_slice()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO tombstones(\
                 bundle_id, reason, created_at_ms, expires_at_ms\
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                bundle_id.as_bytes().as_slice(),
                reason,
                created_at_ms,
                expires_at_ms
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct StoredBundle {
    pub bundle: WireBundle,
    pub first_seen_at_ms: i64,
    pub forward_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    Duplicate,
    RejectedTombstoned,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("wire-format error: {0}")]
    Wire(#[from] mesh_wire::WireError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_types::{BundleType, Priority, UserId};
    use mesh_wire::{ImmutableBundleHeader, RelayHeader, PROTOCOL_MAJOR, PROTOCOL_MINOR};

    fn sample_bundle() -> WireBundle {
        let payload = vec![1, 2, 3];
        WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id: BundleId::random(),
                bundle_type: BundleType::DirectMessage,
                sender_id: UserId::random(),
                destination_id: UserId::random(),
                created_at_ms: 0,
                ttl_seconds: 60,
                hop_limit: 20,
                priority: Priority::Normal,
                payload_length: payload.len() as u32,
            },
            relay: RelayHeader {
                hop_count: 0,
                received_at_ms: 0,
            },
            encrypted_payload: payload,
            sender_signature: vec![0; 64],
        }
    }

    #[test]
    fn duplicate_bundle_is_rejected() {
        let mut store = MeshStore::open_in_memory().unwrap();
        let bundle = sample_bundle();
        assert_eq!(
            store.insert_bundle(&bundle, 0).unwrap(),
            InsertOutcome::Inserted
        );
        assert_eq!(
            store.insert_bundle(&bundle, 0).unwrap(),
            InsertOutcome::Duplicate
        );
    }

    #[test]
    fn tombstone_prevents_reinsertion() {
        let mut store = MeshStore::open_in_memory().unwrap();
        let bundle = sample_bundle();
        store.insert_bundle(&bundle, 0).unwrap();
        store
            .create_tombstone(bundle.immutable.bundle_id, 1, 100, 1_000)
            .unwrap();
        assert_eq!(
            store.insert_bundle(&bundle, 200).unwrap(),
            InsertOutcome::RejectedTombstoned
        );
    }
}
