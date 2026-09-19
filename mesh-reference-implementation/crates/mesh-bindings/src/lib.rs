//! Coarse UniFFI boundary for native Android/iOS code.
//!
//! Native owns radios, Keystore/Keychain and UI. This crate exposes `MeshEngine`
//! only: no SQLite rows, wire frames or crypto primitives.

uniffi::setup_scaffolding!("mesh");

use mesh_core::{
    ContactInfo, ConversationSummary, CoreAction, CoreError, CoreEvent, MeshConfig, MeshCore,
    PlaintextMessage,
};
use mesh_crypto::{Identity, PublicIdentity};
use mesh_types::{ConversationId, DiscoveryId, LinkId, MessageDirection, MessageState, UserId};
use rand_core::{OsRng, RngCore};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use zeroize::Zeroize;

#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MeshError {
    #[error("{0}")]
    Core(String),
    #[error("invalid identifier")]
    InvalidIdentifier,
    #[error("invalid key length")]
    InvalidKey,
    #[error("engine is closed")]
    Closed,
    #[error("lock poisoned")]
    Poisoned,
}

impl From<CoreError> for MeshError {
    fn from(err: CoreError) -> Self {
        Self::Core(err.to_string())
    }
}

/// Secrets native must persist in Keystore/Keychain and pass back to `MeshEngine::open`.
#[derive(Clone, Debug, uniffi::Record)]
pub struct MeshIdentitySecrets {
    pub user_id: Vec<u8>,
    pub signing_public: Vec<u8>,
    pub encryption_public: Vec<u8>,
    pub fingerprint: String,
    pub signing_seed: Vec<u8>,
    pub encryption_seed: Vec<u8>,
    pub local_data_key: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeshPublicIdentity {
    pub user_id: Vec<u8>,
    pub signing_public: Vec<u8>,
    pub encryption_public: Vec<u8>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeshContact {
    pub user_id: Vec<u8>,
    pub display_name: String,
    pub signing_public: Vec<u8>,
    pub encryption_public: Vec<u8>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeshConversation {
    pub conversation_id: Vec<u8>,
    pub remote_user_id: Vec<u8>,
    pub created_at_ms: i64,
    pub last_message_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeshMessageDirection {
    Outbound,
    Inbound,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeshMessageState {
    Queued,
    Relayed,
    Delivered,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeshMessage {
    pub message_id: Vec<u8>,
    pub conversation_id: Vec<u8>,
    pub sender_id: Vec<u8>,
    pub recipient_id: Vec<u8>,
    pub direction: MeshMessageDirection,
    pub state: MeshMessageState,
    pub content_type: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MeshStatus {
    pub user_id: Vec<u8>,
    pub discovery_id: Vec<u8>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeshEvent {
    PeerDiscovered {
        peer_token: Vec<u8>,
        discovery_id: Vec<u8>,
    },
    PeerLost {
        peer_token: Vec<u8>,
    },
    LinkOpened {
        link_id: u64,
        peer_token: Vec<u8>,
    },
    LinkClosed {
        link_id: u64,
    },
    BytesReceived {
        link_id: u64,
        data: Vec<u8>,
    },
    Tick {
        now_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum MeshAction {
    Connect { peer_token: Vec<u8> },
    SendBytes { link_id: u64, data: Vec<u8> },
    CloseLink { link_id: u64 },
    BundleStored { bundle_id: Vec<u8> },
    BundleDuplicate { bundle_id: Vec<u8> },
    BundleReadyForPeer { link_id: u64, bundle_id: Vec<u8> },
    MessageUpdated { message_id: Vec<u8> },
    MessageReceived { message_id: Vec<u8> },
    Log { code: String },
}

#[derive(uniffi::Object)]
pub struct MeshEngine {
    inner: Mutex<Option<MeshCore>>,
}

impl std::fmt::Debug for MeshEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshEngine").finish_non_exhaustive()
    }
}

#[uniffi::export]
pub fn generate_identity() -> MeshIdentitySecrets {
    let mut rng = OsRng;
    let mut signing_seed = [0u8; 32];
    let mut encryption_seed = [0u8; 32];
    let mut local_data_key = [0u8; 32];
    rng.fill_bytes(&mut signing_seed);
    rng.fill_bytes(&mut encryption_seed);
    rng.fill_bytes(&mut local_data_key);
    let identity = Identity::from_seeds(signing_seed, encryption_seed);
    let secrets = MeshIdentitySecrets {
        user_id: identity.user_id().as_bytes().to_vec(),
        signing_public: identity.signing_public().to_vec(),
        encryption_public: identity.encryption_public().to_vec(),
        fingerprint: to_hex(&identity.fingerprint()),
        signing_seed: signing_seed.to_vec(),
        encryption_seed: encryption_seed.to_vec(),
        local_data_key: local_data_key.to_vec(),
    };
    signing_seed.zeroize();
    encryption_seed.zeroize();
    local_data_key.zeroize();
    secrets
}

#[uniffi::export]
impl MeshEngine {
    #[uniffi::constructor]
    pub fn open(
        db_path: String,
        signing_seed: Vec<u8>,
        encryption_seed: Vec<u8>,
        local_data_key: Vec<u8>,
    ) -> Result<Arc<Self>, MeshError> {
        let core = MeshCore::open(
            db_path,
            MeshConfig::default(),
            identity_from_seeds(signing_seed, encryption_seed)?,
            key32(local_data_key)?,
        )?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(core)),
        }))
    }

    #[uniffi::constructor]
    pub fn open_in_memory(
        signing_seed: Vec<u8>,
        encryption_seed: Vec<u8>,
        local_data_key: Vec<u8>,
    ) -> Result<Arc<Self>, MeshError> {
        let core = MeshCore::open_in_memory(
            MeshConfig::default(),
            identity_from_seeds(signing_seed, encryption_seed)?,
            key32(local_data_key)?,
            mesh_core::MeshClock::System,
            mesh_core::MeshRng::os(),
        )?;
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(core)),
        }))
    }

    pub fn process_event(&self, event: MeshEvent) -> Result<Vec<MeshAction>, MeshError> {
        let event = to_core_event(event)?;
        self.with_core(|core| core.process_event(event))
            .map(|actions| actions.into_iter().map(from_core_action).collect())
    }

    pub fn send_text(
        &self,
        recipient: Vec<u8>,
        text: String,
    ) -> Result<Vec<MeshAction>, MeshError> {
        let recipient = user_id(recipient)?;
        self.with_core(|core| core.send_text(recipient, text))
            .map(|actions| actions.into_iter().map(from_core_action).collect())
    }

    pub fn add_contact(
        &self,
        signing_public: Vec<u8>,
        encryption_public: Vec<u8>,
        display_name: String,
    ) -> Result<(), MeshError> {
        let identity = PublicIdentity::new(key32(signing_public)?, key32(encryption_public)?)
            .map_err(|_| MeshError::InvalidKey)?;
        self.with_core(|core| core.add_contact(identity, display_name))
    }

    pub fn get_conversations(&self) -> Result<Vec<MeshConversation>, MeshError> {
        self.with_core(|core| core.conversations())
            .map(|rows| rows.into_iter().map(from_conversation).collect())
    }

    pub fn get_messages(
        &self,
        conversation_id: Option<Vec<u8>>,
    ) -> Result<Vec<MeshMessage>, MeshError> {
        let filter = match conversation_id {
            Some(bytes) => Some(conversation_id_from(bytes)?),
            None => None,
        };
        self.with_core(|core| core.plaintext_messages())
            .map(|rows| {
                rows.into_iter()
                    .filter(|row| filter.is_none_or(|id| row.conversation_id == id))
                    .map(from_message)
                    .collect()
            })
    }

    pub fn get_contacts(&self) -> Result<Vec<MeshContact>, MeshError> {
        self.with_core(|core| core.contacts())
            .map(|rows| rows.into_iter().map(from_contact).collect())
    }

    pub fn get_public_identity(&self) -> Result<MeshPublicIdentity, MeshError> {
        self.with_core_ref(|core| {
            let identity = core.public_identity();
            Ok(MeshPublicIdentity {
                user_id: identity.user_id.as_bytes().to_vec(),
                signing_public: identity.signing_public.to_vec(),
                encryption_public: identity.encryption_public.to_vec(),
                fingerprint: to_hex(&identity.fingerprint()),
            })
        })
    }

    pub fn get_mesh_status(&self) -> Result<MeshStatus, MeshError> {
        self.with_core_ref(|core| {
            let identity = core.public_identity();
            Ok(MeshStatus {
                user_id: core.user_id().as_bytes().to_vec(),
                discovery_id: core.local_discovery_id().as_bytes().to_vec(),
                fingerprint: to_hex(&identity.fingerprint()),
            })
        })
    }

    pub fn close(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            *guard = None;
        }
    }
}

impl MeshEngine {
    fn with_core<T>(
        &self,
        f: impl FnOnce(&mut MeshCore) -> Result<T, CoreError>,
    ) -> Result<T, MeshError> {
        let mut guard = self.inner.lock().map_err(|_| MeshError::Poisoned)?;
        let core = guard.as_mut().ok_or(MeshError::Closed)?;
        f(core).map_err(MeshError::from)
    }

    fn with_core_ref<T>(
        &self,
        f: impl FnOnce(&MeshCore) -> Result<T, MeshError>,
    ) -> Result<T, MeshError> {
        let guard = self.inner.lock().map_err(|_| MeshError::Poisoned)?;
        let core = guard.as_ref().ok_or(MeshError::Closed)?;
        f(core)
    }
}

fn identity_from_seeds(
    signing_seed: Vec<u8>,
    encryption_seed: Vec<u8>,
) -> Result<Identity, MeshError> {
    Ok(Identity::from_seeds(
        key32(signing_seed)?,
        key32(encryption_seed)?,
    ))
}

fn key32(bytes: Vec<u8>) -> Result<[u8; 32], MeshError> {
    <[u8; 32]>::try_from(bytes).map_err(|_| MeshError::InvalidKey)
}

fn id16(bytes: Vec<u8>) -> Result<[u8; 16], MeshError> {
    <[u8; 16]>::try_from(bytes).map_err(|_| MeshError::InvalidIdentifier)
}

fn user_id(bytes: Vec<u8>) -> Result<UserId, MeshError> {
    Ok(UserId::from_bytes(id16(bytes)?))
}

fn conversation_id_from(bytes: Vec<u8>) -> Result<ConversationId, MeshError> {
    Ok(ConversationId::from_bytes(id16(bytes)?))
}

fn to_core_event(event: MeshEvent) -> Result<CoreEvent, MeshError> {
    Ok(match event {
        MeshEvent::PeerDiscovered {
            peer_token,
            discovery_id,
        } => CoreEvent::PeerDiscovered {
            peer_token,
            discovery_id: DiscoveryId::from_bytes(id16(discovery_id)?),
        },
        MeshEvent::PeerLost { peer_token } => CoreEvent::PeerLost { peer_token },
        MeshEvent::LinkOpened {
            link_id,
            peer_token,
        } => CoreEvent::LinkOpened {
            link_id: LinkId(link_id),
            peer_token,
        },
        MeshEvent::LinkClosed { link_id } => CoreEvent::LinkClosed {
            link_id: LinkId(link_id),
        },
        MeshEvent::BytesReceived { link_id, data } => CoreEvent::BytesReceived {
            link_id: LinkId(link_id),
            data,
        },
        MeshEvent::Tick { now_ms } => CoreEvent::Tick { now_ms },
    })
}

fn from_core_action(action: CoreAction) -> MeshAction {
    match action {
        CoreAction::Connect { peer_token } => MeshAction::Connect { peer_token },
        CoreAction::SendBytes { link_id, data } => MeshAction::SendBytes {
            link_id: link_id.0,
            data,
        },
        CoreAction::CloseLink { link_id } => MeshAction::CloseLink { link_id: link_id.0 },
        CoreAction::BundleStored { bundle_id } => MeshAction::BundleStored {
            bundle_id: bundle_id.as_bytes().to_vec(),
        },
        CoreAction::BundleDuplicate { bundle_id } => MeshAction::BundleDuplicate {
            bundle_id: bundle_id.as_bytes().to_vec(),
        },
        CoreAction::BundleReadyForPeer { link_id, bundle_id } => MeshAction::BundleReadyForPeer {
            link_id: link_id.0,
            bundle_id: bundle_id.as_bytes().to_vec(),
        },
        CoreAction::MessageUpdated { message_id } => MeshAction::MessageUpdated {
            message_id: message_id.as_bytes().to_vec(),
        },
        CoreAction::MessageReceived { message_id } => MeshAction::MessageReceived {
            message_id: message_id.as_bytes().to_vec(),
        },
        CoreAction::Log { code } => MeshAction::Log { code },
    }
}

fn from_conversation(row: ConversationSummary) -> MeshConversation {
    MeshConversation {
        conversation_id: row.conversation_id.as_bytes().to_vec(),
        remote_user_id: row.remote_user_id.as_bytes().to_vec(),
        created_at_ms: row.created_at_ms,
        last_message_at_ms: row.last_message_at_ms,
    }
}

fn from_contact(row: ContactInfo) -> MeshContact {
    MeshContact {
        user_id: row.user_id.as_bytes().to_vec(),
        display_name: row.display_name,
        signing_public: row.signing_public.to_vec(),
        encryption_public: row.encryption_public.to_vec(),
        fingerprint: row.fingerprint,
    }
}

fn from_message(row: PlaintextMessage) -> MeshMessage {
    MeshMessage {
        message_id: row.message_id.as_bytes().to_vec(),
        conversation_id: row.conversation_id.as_bytes().to_vec(),
        sender_id: row.sender_id.as_bytes().to_vec(),
        recipient_id: row.recipient_id.as_bytes().to_vec(),
        direction: match row.direction {
            MessageDirection::Outbound => MeshMessageDirection::Outbound,
            MessageDirection::Inbound => MeshMessageDirection::Inbound,
        },
        state: match row.state {
            MessageState::Queued => MeshMessageState::Queued,
            MessageState::Relayed => MeshMessageState::Relayed,
            MessageState::Delivered => MeshMessageState::Delivered,
            MessageState::Failed => MeshMessageState::Failed,
        },
        content_type: row.content_type,
        text: row.text,
    }
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn engine_from(secrets: &MeshIdentitySecrets) -> Arc<MeshEngine> {
        MeshEngine::open_in_memory(
            secrets.signing_seed.clone(),
            secrets.encryption_seed.clone(),
            secrets.local_data_key.clone(),
        )
        .unwrap()
    }

    fn drain_sends(
        actions: &mut Vec<MeshAction>,
        link_id: u64,
        inbox: &mut VecDeque<Vec<u8>>,
        collected: &mut Vec<MeshAction>,
    ) {
        for action in actions.drain(..) {
            match action {
                MeshAction::SendBytes { link_id: id, data } if id == link_id => {
                    inbox.push_back(data)
                }
                other => collected.push(other),
            }
        }
    }

    fn pump_until_idle(
        alice: &MeshEngine,
        bob: &MeshEngine,
        a_link: u64,
        b_link: u64,
        mut a_actions: Vec<MeshAction>,
        mut b_actions: Vec<MeshAction>,
    ) -> (Vec<MeshAction>, Vec<MeshAction>) {
        let mut a_inbox = VecDeque::new();
        let mut b_inbox = VecDeque::new();
        let mut a_seen = Vec::new();
        let mut b_seen = Vec::new();
        loop {
            drain_sends(&mut a_actions, a_link, &mut b_inbox, &mut a_seen);
            drain_sends(&mut b_actions, b_link, &mut a_inbox, &mut b_seen);
            let mut progress = false;
            if let Some(data) = a_inbox.pop_front() {
                a_actions = alice
                    .process_event(MeshEvent::BytesReceived {
                        link_id: a_link,
                        data,
                    })
                    .unwrap();
                progress = true;
            }
            if let Some(data) = b_inbox.pop_front() {
                b_actions = bob
                    .process_event(MeshEvent::BytesReceived {
                        link_id: b_link,
                        data,
                    })
                    .unwrap();
                progress = true;
            }
            if !progress {
                break;
            }
        }
        (a_seen, b_seen)
    }

    #[test]
    fn generate_identity_then_open_and_send_text() {
        let alice = generate_identity();
        let bob = generate_identity();
        let alice_engine = engine_from(&alice);
        let bob_engine = engine_from(&bob);

        alice_engine
            .add_contact(
                bob.signing_public.clone(),
                bob.encryption_public.clone(),
                "Bob".into(),
            )
            .unwrap();
        bob_engine
            .add_contact(
                alice.signing_public.clone(),
                alice.encryption_public.clone(),
                "Alice".into(),
            )
            .unwrap();

        alice_engine
            .send_text(bob.user_id.clone(), "Are you safe?".into())
            .unwrap();

        let conversations = alice_engine.get_conversations().unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].remote_user_id, bob.user_id);
        assert_eq!(
            alice_engine.get_messages(None).unwrap()[0].text,
            "Are you safe?"
        );
        assert_eq!(alice_engine.get_contacts().unwrap()[0].display_name, "Bob");
        assert_eq!(
            alice_engine.get_public_identity().unwrap().user_id,
            alice.user_id
        );
    }

    #[test]
    fn ffi_session_transfers_send_text() {
        let alice = generate_identity();
        let bob = generate_identity();
        let alice_engine = engine_from(&alice);
        let bob_engine = engine_from(&bob);
        alice_engine
            .add_contact(
                bob.signing_public.clone(),
                bob.encryption_public.clone(),
                "Bob".into(),
            )
            .unwrap();
        bob_engine
            .add_contact(
                alice.signing_public.clone(),
                alice.encryption_public.clone(),
                "Alice".into(),
            )
            .unwrap();
        alice_engine
            .send_text(bob.user_id.clone(), "Are you safe?".into())
            .unwrap();

        let a_link = 1;
        let b_link = 1;
        let a_actions = alice_engine
            .process_event(MeshEvent::LinkOpened {
                link_id: a_link,
                peer_token: b"bob".to_vec(),
            })
            .unwrap();
        let b_actions = bob_engine
            .process_event(MeshEvent::LinkOpened {
                link_id: b_link,
                peer_token: b"alice".to_vec(),
            })
            .unwrap();
        let (_a_seen, b_seen) = pump_until_idle(
            &alice_engine,
            &bob_engine,
            a_link,
            b_link,
            a_actions,
            b_actions,
        );
        assert!(b_seen
            .iter()
            .any(|action| matches!(action, MeshAction::MessageReceived { .. })));
        assert_eq!(
            bob_engine.get_messages(None).unwrap()[0].text,
            "Are you safe?"
        );
    }

    #[test]
    fn close_rejects_later_calls() {
        let alice = generate_identity();
        let engine = engine_from(&alice);
        engine.close();
        let err = engine
            .send_text(alice.user_id.clone(), "hi".into())
            .unwrap_err();
        assert!(matches!(err, MeshError::Closed));
    }

    #[test]
    fn open_rejects_short_keys() {
        let err = MeshEngine::open_in_memory(vec![1, 2, 3], vec![0; 32], vec![0; 32]).unwrap_err();
        assert!(matches!(err, MeshError::InvalidKey));
    }
}
