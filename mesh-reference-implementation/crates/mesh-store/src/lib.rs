use mesh_types::{
    BundleId, ConversationId, MessageDirection, MessageId, MessageState, TrustState, UserId,
};
use mesh_wire::WireBundle;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use thiserror::Error;

const MIGRATION_0001: &str = include_str!("../migrations/0001_initial.sql");
const MIGRATION_0002: &str = include_str!("../migrations/0002_contacts_messages.sql");

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
        let version: i64 = self
            .conn
            .query_row("SELECT version FROM schema_meta LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?
            .unwrap_or(0);
        if version < 1 {
            self.conn
                .execute("INSERT INTO schema_meta(version) VALUES (1)", [])?;
        }
        if version < 2 {
            self.conn.execute_batch(MIGRATION_0002)?;
            if version < 1 {
                // version row already inserted as 1 above; bump to 2
            }
            self.conn
                .execute("UPDATE schema_meta SET version = 2", [])?;
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

    pub fn upsert_contact(&mut self, contact: &StoredContact) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO contacts(\
                user_id, display_name, signing_public_key, encryption_public_key, \
                trust_state, fingerprint, created_at_ms\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(user_id) DO UPDATE SET
                display_name = excluded.display_name,
                signing_public_key = excluded.signing_public_key,
                encryption_public_key = excluded.encryption_public_key,
                trust_state = excluded.trust_state,
                fingerprint = excluded.fingerprint",
            params![
                contact.user_id.as_bytes().as_slice(),
                contact.display_name,
                contact.signing_public.as_slice(),
                contact.encryption_public.as_slice(),
                contact.trust_state as i64,
                contact.fingerprint,
                contact.created_at_ms,
            ],
        )?;
        Ok(())
    }

    pub fn get_contact(&self, user_id: UserId) -> Result<Option<StoredContact>, StoreError> {
        let contact = self
            .conn
            .query_row(
                "SELECT user_id, display_name, signing_public_key, encryption_public_key, \
                        trust_state, fingerprint, created_at_ms \
                 FROM contacts WHERE user_id = ?1",
                params![user_id.as_bytes().as_slice()],
                |row| {
                    Ok(StoredContact {
                        user_id: UserId::from_bytes(id16(row.get(0)?)?),
                        display_name: row.get(1)?,
                        signing_public: blob32(row.get(2)?)?,
                        encryption_public: blob32(row.get(3)?)?,
                        trust_state: trust_state(row.get(4)?)?,
                        fingerprint: row.get(5)?,
                        created_at_ms: row.get(6)?,
                    })
                },
            )
            .optional()?;
        Ok(contact)
    }

    pub fn ensure_conversation(
        &mut self,
        conversation_id: ConversationId,
        remote_user_id: UserId,
        created_at_ms: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO conversations(\
                conversation_id, remote_user_id, created_at_ms\
             ) VALUES (?1, ?2, ?3)",
            params![
                conversation_id.as_bytes().as_slice(),
                remote_user_id.as_bytes().as_slice(),
                created_at_ms,
            ],
        )?;
        Ok(())
    }

    pub fn outbound_count(&self, conversation_id: ConversationId) -> Result<u64, StoreError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1 AND direction = ?2",
            params![
                conversation_id.as_bytes().as_slice(),
                MessageDirection::Outbound as i64
            ],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn has_message(&self, message_id: MessageId) -> Result<bool, StoreError> {
        let value: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM messages WHERE message_id = ?1 LIMIT 1",
                params![message_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.is_some())
    }

    pub fn insert_local_message(&mut self, message: &StoredMessageRow) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO messages(\
                message_id, conversation_id, sender_id, recipient_id, direction, \
                sent_at_ms, received_at_ms, state, content_type, content_nonce, \
                content_ciphertext, original_bundle_id\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                message.message_id.as_bytes().as_slice(),
                message.conversation_id.as_bytes().as_slice(),
                message.sender_id.as_bytes().as_slice(),
                message.recipient_id.as_bytes().as_slice(),
                message.direction as i64,
                message.sent_at_ms,
                message.received_at_ms,
                message.state as i64,
                message.content_type,
                message.content_nonce,
                message.content_ciphertext,
                message.original_bundle_id.map(|id| id.as_bytes().to_vec()),
            ],
        )?;
        self.conn.execute(
            "UPDATE conversations SET last_message_at_ms = ?1 WHERE conversation_id = ?2",
            params![
                message.sent_at_ms.or(message.received_at_ms),
                message.conversation_id.as_bytes().as_slice(),
            ],
        )?;
        Ok(())
    }

    pub fn mark_message_delivered(
        &mut self,
        message_id: MessageId,
        delivered_at_ms: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO delivered_message_ids(message_id, delivered_at_ms) \
             VALUES (?1, ?2)",
            params![message_id.as_bytes().as_slice(), delivered_at_ms],
        )?;
        Ok(())
    }

    pub fn is_message_delivered(&self, message_id: MessageId) -> Result<bool, StoreError> {
        let value: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM delivered_message_ids WHERE message_id = ?1 LIMIT 1",
                params![message_id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.is_some())
    }

    pub fn list_messages(&self) -> Result<Vec<StoredMessageRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT message_id, conversation_id, sender_id, recipient_id, direction, \
                    sent_at_ms, received_at_ms, state, content_type, content_nonce, \
                    content_ciphertext, original_bundle_id \
             FROM messages ORDER BY COALESCE(sent_at_ms, received_at_ms, 0) ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<i64>>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Vec<u8>>(9)?,
                row.get::<_, Vec<u8>>(10)?,
                row.get::<_, Option<Vec<u8>>>(11)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (
                message_id,
                conversation_id,
                sender_id,
                recipient_id,
                direction,
                sent_at_ms,
                received_at_ms,
                state,
                content_type,
                content_nonce,
                content_ciphertext,
                original_bundle_id,
            ) = row?;
            result.push(StoredMessageRow {
                message_id: MessageId::from_bytes(id16(message_id)?),
                conversation_id: ConversationId::from_bytes(id16(conversation_id)?),
                sender_id: UserId::from_bytes(id16(sender_id)?),
                recipient_id: UserId::from_bytes(id16(recipient_id)?),
                direction: message_direction(direction)?,
                sent_at_ms,
                received_at_ms,
                state: message_state(state)?,
                content_type,
                content_nonce,
                content_ciphertext,
                original_bundle_id: original_bundle_id
                    .map(id16)
                    .transpose()?
                    .map(BundleId::from_bytes),
            });
        }
        Ok(result)
    }
}

fn id16(bytes: Vec<u8>) -> Result<[u8; 16], rusqlite::Error> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn blob32(bytes: Vec<u8>) -> Result<[u8; 32], rusqlite::Error> {
    bytes.try_into().map_err(|_| rusqlite::Error::InvalidQuery)
}

fn trust_state(value: i64) -> Result<TrustState, rusqlite::Error> {
    match value {
        0 => Ok(TrustState::Unverified),
        1 => Ok(TrustState::Verified),
        2 => Ok(TrustState::Blocked),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn message_direction(value: i64) -> Result<MessageDirection, rusqlite::Error> {
    match value {
        0 => Ok(MessageDirection::Outbound),
        1 => Ok(MessageDirection::Inbound),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

fn message_state(value: i64) -> Result<MessageState, rusqlite::Error> {
    match value {
        0 => Ok(MessageState::Queued),
        1 => Ok(MessageState::Relayed),
        2 => Ok(MessageState::Delivered),
        3 => Ok(MessageState::Failed),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

#[derive(Clone, Debug)]
pub struct StoredBundle {
    pub bundle: WireBundle,
    pub first_seen_at_ms: i64,
    pub forward_count: u32,
}

#[derive(Clone, Debug)]
pub struct StoredContact {
    pub user_id: UserId,
    pub display_name: String,
    pub signing_public: [u8; 32],
    pub encryption_public: [u8; 32],
    pub trust_state: TrustState,
    pub fingerprint: String,
    pub created_at_ms: i64,
}

#[derive(Clone, Debug)]
pub struct StoredMessageRow {
    pub message_id: MessageId,
    pub conversation_id: ConversationId,
    pub sender_id: UserId,
    pub recipient_id: UserId,
    pub direction: MessageDirection,
    pub sent_at_ms: Option<i64>,
    pub received_at_ms: Option<i64>,
    pub state: MessageState,
    pub content_type: String,
    pub content_nonce: Vec<u8>,
    pub content_ciphertext: Vec<u8>,
    pub original_bundle_id: Option<BundleId>,
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
    use mesh_types::{BundleType, Priority};
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

    #[test]
    fn contact_round_trip() {
        let mut store = MeshStore::open_in_memory().unwrap();
        let contact = StoredContact {
            user_id: UserId::from_bytes([9; 16]),
            display_name: "Bob".into(),
            signing_public: [1; 32],
            encryption_public: [2; 32],
            trust_state: TrustState::Verified,
            fingerprint: "abcd".into(),
            created_at_ms: 10,
        };
        store.upsert_contact(&contact).unwrap();
        let loaded = store.get_contact(contact.user_id).unwrap().unwrap();
        assert_eq!(loaded.display_name, "Bob");
        assert_eq!(loaded.signing_public, [1; 32]);
    }
}
