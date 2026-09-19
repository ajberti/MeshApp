use mesh_crypto::{
    conversation_id, open_local, open_message, seal_local, seal_message, verify_signature,
    CryptoError, Identity, PublicIdentity, SealedMessage,
};
use mesh_routing::{ControlledEpidemicRouter, RoutingDecision};
use mesh_store::{InsertOutcome, MeshStore, StoreError, StoredContact, StoredMessageRow};
use mesh_types::{
    BundleId, BundleType, ConversationId, DiscoveryId, LinkId, MessageDirection, MessageId,
    MessageState, Priority, TrustState, UserId,
};
use mesh_wire::{
    BundleData, BundleIdPayload, BundleOffer, DirectMessagePayload, Frame, FrameType,
    ImmutableBundleHeader, InventorySummary, RelayHeader, SealedPayload, WireBundle, WireError,
    DIRECT_MESSAGE_VERSION, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use rand_core::{CryptoRng, OsRng, RngCore};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

mod session;
use session::{MeshSession, SessionEvent};

#[derive(Clone, Debug)]
pub struct MeshConfig {
    pub default_ttl_secs: u32,
    pub default_hop_limit: u16,
    pub relay_quota_bytes: u64,
    pub normal_replication_limit: u32,
    pub high_replication_limit: u32,
    pub emergency_replication_limit: u32,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 72 * 60 * 60,
            default_hop_limit: 20,
            relay_quota_bytes: 250 * 1024 * 1024,
            normal_replication_limit: 6,
            high_replication_limit: 12,
            emergency_replication_limit: 20,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MeshClock {
    System,
    Fixed(i64),
}

impl MeshClock {
    pub fn now_ms(&self) -> i64 {
        match self {
            Self::System => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
            Self::Fixed(now) => *now,
        }
    }
}

pub struct DeterministicRng {
    seed: [u8; 32],
    counter: u64,
}

impl DeterministicRng {
    pub fn new(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}

impl RngCore for DeterministicRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut offset = 0;
        while offset < dest.len() {
            let mut block = [0u8; 40];
            block[..32].copy_from_slice(&self.seed);
            block[32..].copy_from_slice(&self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let digest = mesh_crypto::sha256(&block);
            let n = (dest.len() - offset).min(32);
            dest[offset..offset + n].copy_from_slice(&digest[..n]);
            offset += n;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for DeterministicRng {}

pub enum MeshRng {
    System(OsRng),
    Deterministic(DeterministicRng),
}

impl MeshRng {
    pub fn os() -> Self {
        Self::System(OsRng)
    }

    pub fn deterministic(seed: [u8; 32]) -> Self {
        Self::Deterministic(DeterministicRng::new(seed))
    }
}

impl RngCore for MeshRng {
    fn next_u32(&mut self) -> u32 {
        match self {
            Self::System(rng) => rng.next_u32(),
            Self::Deterministic(rng) => rng.next_u32(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        match self {
            Self::System(rng) => rng.next_u64(),
            Self::Deterministic(rng) => rng.next_u64(),
        }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        match self {
            Self::System(rng) => rng.fill_bytes(dest),
            Self::Deterministic(rng) => rng.fill_bytes(dest),
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for MeshRng {}

#[derive(Clone, Debug)]
pub enum CoreEvent {
    PeerDiscovered {
        peer_token: Vec<u8>,
        discovery_id: DiscoveryId,
    },
    PeerLost {
        peer_token: Vec<u8>,
    },
    LinkOpened {
        link_id: LinkId,
        peer_token: Vec<u8>,
    },
    LinkClosed {
        link_id: LinkId,
    },
    BytesReceived {
        link_id: LinkId,
        data: Vec<u8>,
    },
    SendText {
        recipient: UserId,
        text: String,
    },
    Tick {
        now_ms: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreAction {
    Connect {
        peer_token: Vec<u8>,
    },
    SendBytes {
        link_id: LinkId,
        data: Vec<u8>,
    },
    CloseLink {
        link_id: LinkId,
    },
    BundleStored {
        bundle_id: BundleId,
    },
    BundleDuplicate {
        bundle_id: BundleId,
    },
    BundleReadyForPeer {
        link_id: LinkId,
        bundle_id: BundleId,
    },
    MessageUpdated {
        message_id: MessageId,
    },
    MessageReceived {
        message_id: MessageId,
    },
    Log {
        code: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaintextMessage {
    pub message_id: MessageId,
    pub conversation_id: ConversationId,
    pub sender_id: UserId,
    pub recipient_id: UserId,
    pub direction: MessageDirection,
    pub state: MessageState,
    pub content_type: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationSummary {
    pub conversation_id: ConversationId,
    pub remote_user_id: UserId,
    pub created_at_ms: i64,
    pub last_message_at_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactInfo {
    pub user_id: UserId,
    pub display_name: String,
    pub signing_public: [u8; 32],
    pub encryption_public: [u8; 32],
    pub fingerprint: String,
}

pub struct MeshCore {
    identity: Identity,
    local_data_key: [u8; 32],
    store: MeshStore,
    router: ControlledEpidemicRouter,
    peers: HashMap<Vec<u8>, DiscoveryId>,
    links: HashMap<LinkId, MeshSession>,
    local_discovery_id: DiscoveryId,
    clock: MeshClock,
    rng: MeshRng,
    config: MeshConfig,
}

impl MeshCore {
    pub fn open(
        path: impl AsRef<Path>,
        config: MeshConfig,
        identity: Identity,
        local_data_key: [u8; 32],
    ) -> Result<Self, CoreError> {
        let store = MeshStore::open(path)?;
        Ok(Self::from_store(
            store,
            config,
            identity,
            local_data_key,
            MeshClock::System,
            MeshRng::os(),
        ))
    }

    pub fn open_in_memory(
        config: MeshConfig,
        identity: Identity,
        local_data_key: [u8; 32],
        clock: MeshClock,
        rng: MeshRng,
    ) -> Result<Self, CoreError> {
        let store = MeshStore::open_in_memory()?;
        Ok(Self::from_store(
            store,
            config,
            identity,
            local_data_key,
            clock,
            rng,
        ))
    }

    fn from_store(
        store: MeshStore,
        config: MeshConfig,
        identity: Identity,
        local_data_key: [u8; 32],
        clock: MeshClock,
        mut rng: MeshRng,
    ) -> Self {
        let router = ControlledEpidemicRouter {
            normal_replication_limit: config.normal_replication_limit,
            high_replication_limit: config.high_replication_limit,
            emergency_replication_limit: config.emergency_replication_limit,
        };
        let mut discovery = [0u8; 16];
        rng.fill_bytes(&mut discovery);
        Self {
            identity,
            local_data_key,
            store,
            router,
            peers: HashMap::new(),
            links: HashMap::new(),
            local_discovery_id: DiscoveryId::from_bytes(discovery),
            clock,
            rng,
            config,
        }
    }

    pub fn user_id(&self) -> UserId {
        self.identity.user_id()
    }

    pub fn public_identity(&self) -> PublicIdentity {
        self.identity.public_identity()
    }

    pub fn local_discovery_id(&self) -> DiscoveryId {
        self.local_discovery_id
    }

    pub fn add_contact(
        &mut self,
        identity: PublicIdentity,
        display_name: impl Into<String>,
    ) -> Result<(), CoreError> {
        self.store.upsert_contact(&StoredContact {
            user_id: identity.user_id,
            display_name: display_name.into(),
            signing_public: identity.signing_public,
            encryption_public: identity.encryption_public,
            trust_state: TrustState::Verified,
            fingerprint: to_hex(&identity.fingerprint()),
            created_at_ms: self.clock.now_ms(),
        })?;
        Ok(())
    }

    pub fn send_text(
        &mut self,
        recipient: UserId,
        text: impl Into<String>,
    ) -> Result<Vec<CoreAction>, CoreError> {
        self.compose_text(recipient, text.into())
    }

    pub fn accept_bundle(&mut self, bundle: &WireBundle) -> Result<Vec<CoreAction>, CoreError> {
        self.ingest_bundle(bundle.clone())
    }

    pub fn stored_bundles(&self) -> Result<Vec<mesh_store::StoredBundle>, CoreError> {
        Ok(self.store.list_bundles()?)
    }

    pub fn plaintext_messages(&self) -> Result<Vec<PlaintextMessage>, CoreError> {
        let mut messages = Vec::new();
        for row in self.store.list_messages()? {
            let nonce: [u8; 24] = row
                .content_nonce
                .as_slice()
                .try_into()
                .map_err(|_| CoreError::InvalidMessage)?;
            let plaintext = open_local(&self.local_data_key, &nonce, &row.content_ciphertext)?;
            let text = String::from_utf8(plaintext).map_err(|_| CoreError::InvalidMessage)?;
            messages.push(PlaintextMessage {
                message_id: row.message_id,
                conversation_id: row.conversation_id,
                sender_id: row.sender_id,
                recipient_id: row.recipient_id,
                direction: row.direction,
                state: row.state,
                content_type: row.content_type,
                text,
            });
        }
        Ok(messages)
    }

    pub fn conversations(&self) -> Result<Vec<ConversationSummary>, CoreError> {
        Ok(self
            .store
            .list_conversations()?
            .into_iter()
            .map(|row| ConversationSummary {
                conversation_id: row.conversation_id,
                remote_user_id: row.remote_user_id,
                created_at_ms: row.created_at_ms,
                last_message_at_ms: row.last_message_at_ms,
            })
            .collect())
    }

    pub fn contacts(&self) -> Result<Vec<ContactInfo>, CoreError> {
        Ok(self
            .store
            .list_contacts()?
            .into_iter()
            .map(|row| ContactInfo {
                user_id: row.user_id,
                display_name: row.display_name,
                signing_public: row.signing_public,
                encryption_public: row.encryption_public,
                fingerprint: row.fingerprint,
            })
            .collect())
    }

    pub fn process_event(&mut self, event: CoreEvent) -> Result<Vec<CoreAction>, CoreError> {
        match event {
            CoreEvent::PeerDiscovered {
                peer_token,
                discovery_id,
            } => {
                self.peers.insert(peer_token.clone(), discovery_id);
                if self.local_discovery_id.as_bytes() < discovery_id.as_bytes() {
                    Ok(vec![CoreAction::Connect { peer_token }])
                } else {
                    Ok(Vec::new())
                }
            }
            CoreEvent::PeerLost { peer_token } => {
                self.peers.remove(&peer_token);
                Ok(Vec::new())
            }
            CoreEvent::LinkOpened {
                link_id,
                peer_token,
            } => {
                let (session, hello) =
                    MeshSession::open(link_id, peer_token, self.local_discovery_id, &mut self.rng);
                self.links.insert(link_id, session);
                Ok(vec![
                    CoreAction::SendBytes {
                        link_id,
                        data: hello.encode()?,
                    },
                    CoreAction::Log {
                        code: "LINK_OPENED".into(),
                    },
                ])
            }
            CoreEvent::LinkClosed { link_id } => {
                self.links.remove(&link_id);
                Ok(Vec::new())
            }
            CoreEvent::BytesReceived { link_id, data } => self.on_bytes(link_id, data),
            CoreEvent::SendText { recipient, text } => self.compose_text(recipient, text),
            CoreEvent::Tick { now_ms } => self.plan_transfers(now_ms),
        }
    }

    pub fn import_bundle(
        &mut self,
        bundle: &WireBundle,
        first_seen_at_ms: i64,
    ) -> Result<InsertOutcome, CoreError> {
        Ok(self.store.insert_bundle(bundle, first_seen_at_ms)?)
    }

    fn compose_text(
        &mut self,
        recipient: UserId,
        text: String,
    ) -> Result<Vec<CoreAction>, CoreError> {
        let contact = self
            .store
            .get_contact(recipient)?
            .ok_or(CoreError::UnknownContact(recipient))?;
        let now = self.clock.now_ms();
        let conversation_id = conversation_id(self.identity.user_id(), recipient);
        self.store
            .ensure_conversation(conversation_id, recipient, now)?;
        let sequence = self.store.outbound_count(conversation_id)? + 1;
        let message_id = MessageId::from_bytes(self.random_bytes());
        let bundle_id = BundleId::from_bytes(self.random_bytes());
        let payload = DirectMessagePayload {
            version: DIRECT_MESSAGE_VERSION,
            message_id,
            conversation_id,
            sender_id: self.identity.user_id(),
            recipient_id: recipient,
            sequence,
            sent_at_ms: now,
            content_type: "text/plain".into(),
            content: text.into_bytes(),
        };
        let sealed = seal_message(
            &contact.encryption_public,
            &payload.encode_cbor(),
            &mut self.rng,
        )?;
        let encrypted_payload = SealedPayload {
            ephemeral_public: sealed.ephemeral_public,
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        }
        .encode_cbor();
        let mut bundle = WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id,
                bundle_type: BundleType::DirectMessage,
                sender_id: self.identity.user_id(),
                destination_id: recipient,
                created_at_ms: now,
                ttl_seconds: self.config.default_ttl_secs,
                hop_limit: self.config.default_hop_limit,
                priority: Priority::Normal,
                payload_length: encrypted_payload.len() as u32,
            },
            relay: RelayHeader {
                hop_count: 0,
                received_at_ms: now,
            },
            encrypted_payload,
            sender_signature: vec![0; 64],
        };
        bundle.sender_signature = self.identity.sign(&bundle.signed_bytes()?).to_vec();
        self.store.insert_bundle(&bundle, now)?;

        let nonce = self.random_bytes::<24>();
        let ciphertext = seal_local(&self.local_data_key, &nonce, &payload.content)?;
        self.store.insert_local_message(&StoredMessageRow {
            message_id,
            conversation_id,
            sender_id: self.identity.user_id(),
            recipient_id: recipient,
            direction: MessageDirection::Outbound,
            sent_at_ms: Some(now),
            received_at_ms: None,
            state: MessageState::Queued,
            content_type: payload.content_type,
            content_nonce: nonce.to_vec(),
            content_ciphertext: ciphertext,
            original_bundle_id: Some(bundle_id),
        })?;

        Ok(vec![
            CoreAction::MessageUpdated { message_id },
            CoreAction::BundleStored { bundle_id },
        ])
    }

    fn ingest_bundle(&mut self, mut bundle: WireBundle) -> Result<Vec<CoreAction>, CoreError> {
        let local_destination = bundle.immutable.destination_id == self.identity.user_id();
        if !local_destination {
            if bundle.relay.hop_count >= bundle.immutable.hop_limit {
                return Ok(vec![CoreAction::Log {
                    code: "HOP_LIMIT".into(),
                }]);
            }
            bundle.relay.hop_count = bundle.relay.hop_count.saturating_add(1);
        }

        let now = self.clock.now_ms();
        bundle.relay.received_at_ms = now;
        let bundle_id = bundle.immutable.bundle_id;
        let outcome = self.store.insert_bundle(&bundle, now)?;
        let mut actions = match outcome {
            InsertOutcome::Inserted => vec![CoreAction::BundleStored { bundle_id }],
            InsertOutcome::Duplicate | InsertOutcome::RejectedTombstoned => {
                vec![CoreAction::BundleDuplicate { bundle_id }]
            }
        };
        if local_destination {
            if let Some(message_id) = self.deliver_local(&bundle)? {
                actions.push(CoreAction::MessageReceived { message_id });
            }
        }
        Ok(actions)
    }

    fn deliver_local(&mut self, bundle: &WireBundle) -> Result<Option<MessageId>, CoreError> {
        let sealed_payload = SealedPayload::decode_cbor(&bundle.encrypted_payload)?;
        let sealed = SealedMessage {
            ephemeral_public: sealed_payload.ephemeral_public,
            nonce: sealed_payload.nonce,
            ciphertext: sealed_payload.ciphertext,
        };
        let plaintext = match open_message(&self.identity, &sealed) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(None),
        };
        let payload = DirectMessagePayload::decode_cbor(&plaintext)?;
        if payload.recipient_id != self.identity.user_id()
            || payload.sender_id != bundle.immutable.sender_id
            || payload.recipient_id != bundle.immutable.destination_id
        {
            return Err(CoreError::InvalidMessage);
        }
        if self.store.is_message_delivered(payload.message_id)?
            || self.store.has_message(payload.message_id)?
        {
            return Ok(None);
        }
        let Some(contact) = self.store.get_contact(payload.sender_id)? else {
            return Ok(None);
        };
        let signature: [u8; 64] = bundle
            .sender_signature
            .as_slice()
            .try_into()
            .map_err(|_| CoreError::InvalidMessage)?;
        verify_signature(&contact.signing_public, &bundle.signed_bytes()?, &signature)?;

        let now = self.clock.now_ms();
        self.store
            .ensure_conversation(payload.conversation_id, payload.sender_id, now)?;
        let nonce = self.random_bytes::<24>();
        let ciphertext = seal_local(&self.local_data_key, &nonce, &payload.content)?;
        self.store.insert_local_message(&StoredMessageRow {
            message_id: payload.message_id,
            conversation_id: payload.conversation_id,
            sender_id: payload.sender_id,
            recipient_id: payload.recipient_id,
            direction: MessageDirection::Inbound,
            sent_at_ms: Some(payload.sent_at_ms),
            received_at_ms: Some(now),
            state: MessageState::Delivered,
            content_type: payload.content_type,
            content_nonce: nonce.to_vec(),
            content_ciphertext: ciphertext,
            original_bundle_id: Some(bundle.immutable.bundle_id),
        })?;
        self.store.mark_message_delivered(payload.message_id, now)?;
        Ok(Some(payload.message_id))
    }

    fn random_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut bytes = [0u8; N];
        self.rng.fill_bytes(&mut bytes);
        bytes
    }

    fn on_bytes(&mut self, link_id: LinkId, data: Vec<u8>) -> Result<Vec<CoreAction>, CoreError> {
        let frame = Frame::decode(&data)?;
        let result = {
            let session = self
                .links
                .get_mut(&link_id)
                .ok_or(CoreError::UnknownLink(link_id))?;
            session.handle_frame(frame, &mut self.rng)
        };
        match result {
            Ok(events) => self.dispatch_session_events(link_id, events),
            Err(err) => {
                self.links.remove(&link_id);
                Err(err)
            }
        }
    }

    fn dispatch_session_events(
        &mut self,
        link_id: LinkId,
        events: Vec<SessionEvent>,
    ) -> Result<Vec<CoreAction>, CoreError> {
        let mut actions = Vec::new();
        for event in events {
            match event {
                SessionEvent::Send(frame) => {
                    actions.push(CoreAction::SendBytes {
                        link_id,
                        data: frame.encode()?,
                    });
                }
                SessionEvent::Established => {
                    actions.push(CoreAction::Log {
                        code: "SESSION_ESTABLISHED".into(),
                    });
                    actions.extend(self.send_inventory(link_id)?);
                }
                SessionEvent::PeerInventory(ids) => {
                    if let Some(session) = self.links.get_mut(&link_id) {
                        session.set_known_ids(ids);
                    }
                    actions.extend(self.send_offers(link_id)?);
                }
                SessionEvent::Offer(offer) => {
                    actions.extend(self.on_bundle_offer(link_id, offer)?);
                }
                SessionEvent::Request(bundle_id) => {
                    actions.extend(self.on_bundle_request(link_id, bundle_id)?);
                }
                SessionEvent::Data(data) => {
                    actions.extend(self.on_bundle_data(link_id, data)?);
                }
                SessionEvent::Complete(bundle_id) => {
                    self.store.increment_forward_count(bundle_id)?;
                    if let Some(session) = self.links.get_mut(&link_id) {
                        session.note_peer_has(bundle_id);
                    }
                }
                SessionEvent::Close => {
                    self.links.remove(&link_id);
                    actions.push(CoreAction::CloseLink { link_id });
                }
            }
        }
        Ok(actions)
    }

    fn send_secure(
        &mut self,
        link_id: LinkId,
        ty: FrameType,
        payload: Vec<u8>,
    ) -> Result<CoreAction, CoreError> {
        let session = self
            .links
            .get_mut(&link_id)
            .ok_or(CoreError::UnknownLink(link_id))?;
        let frame = session.encrypt(ty, payload)?;
        Ok(CoreAction::SendBytes {
            link_id,
            data: frame.encode()?,
        })
    }

    fn send_inventory(&mut self, link_id: LinkId) -> Result<Vec<CoreAction>, CoreError> {
        let bundle_ids = self
            .store
            .list_bundles()?
            .into_iter()
            .map(|stored| stored.bundle.immutable.bundle_id)
            .collect();
        let payload = InventorySummary { bundle_ids }.encode();
        Ok(vec![self.send_secure(
            link_id,
            FrameType::InventorySummary,
            payload,
        )?])
    }

    fn send_offers(&mut self, link_id: LinkId) -> Result<Vec<CoreAction>, CoreError> {
        let peer = self
            .links
            .get(&link_id)
            .ok_or(CoreError::UnknownLink(link_id))?
            .peer_context();
        let now = self.clock.now_ms();
        let mut offers = Vec::new();
        for stored in self.store.list_bundles()? {
            let decision = self.router.evaluate(
                &stored.bundle,
                stored.first_seen_at_ms,
                stored.forward_count,
                &peer,
                now,
            );
            if matches!(
                decision,
                RoutingDecision::SendImmediately | RoutingDecision::Offer
            ) {
                let encoded = stored.bundle.encode_cbor()?;
                offers.push(BundleOffer {
                    bundle_id: stored.bundle.immutable.bundle_id,
                    bundle_type: stored.bundle.immutable.bundle_type,
                    destination_id: stored.bundle.immutable.destination_id,
                    size: encoded.len() as u32,
                    priority: stored.bundle.immutable.priority,
                    hop_count: stored.bundle.relay.hop_count,
                });
            }
        }
        let mut actions = Vec::new();
        for offer in &offers {
            actions.push(CoreAction::BundleReadyForPeer {
                link_id,
                bundle_id: offer.bundle_id,
            });
            actions.push(self.send_secure(link_id, FrameType::BundleOffer, offer.encode())?);
        }
        Ok(actions)
    }

    fn on_bundle_offer(
        &mut self,
        link_id: LinkId,
        offer: BundleOffer,
    ) -> Result<Vec<CoreAction>, CoreError> {
        if self.store.contains_bundle(offer.bundle_id)?
            || self.store.is_tombstoned(offer.bundle_id)?
        {
            return Ok(Vec::new());
        }
        if let Some(session) = self.links.get_mut(&link_id) {
            session.note_offer(&offer);
        }
        let payload = BundleIdPayload {
            bundle_id: offer.bundle_id,
        }
        .encode();
        Ok(vec![self.send_secure(
            link_id,
            FrameType::BundleRequest,
            payload,
        )?])
    }

    fn on_bundle_request(
        &mut self,
        link_id: LinkId,
        bundle_id: BundleId,
    ) -> Result<Vec<CoreAction>, CoreError> {
        let Some(stored) = self.store.get_bundle(bundle_id)? else {
            return Ok(Vec::new());
        };
        let encoded = stored.bundle.encode_cbor()?;
        let chunk = self
            .links
            .get(&link_id)
            .ok_or(CoreError::UnknownLink(link_id))?
            .plaintext_chunk_size();
        let mut actions = Vec::new();
        let mut offset = 0u32;
        while (offset as usize) < encoded.len() {
            let end = (offset as usize + chunk).min(encoded.len());
            let payload = BundleData {
                bundle_id,
                offset,
                data: encoded[offset as usize..end].to_vec(),
            }
            .encode();
            actions.push(self.send_secure(link_id, FrameType::BundleData, payload)?);
            offset = end as u32;
        }
        Ok(actions)
    }

    fn on_bundle_data(
        &mut self,
        link_id: LinkId,
        data: BundleData,
    ) -> Result<Vec<CoreAction>, CoreError> {
        let bundle_id = data.bundle_id;
        let complete = {
            let session = self
                .links
                .get_mut(&link_id)
                .ok_or(CoreError::UnknownLink(link_id))?;
            session.append_data(&data)?
        };
        let mut actions = Vec::new();
        if let Some(bytes) = complete {
            let bundle = WireBundle::decode_cbor(&bytes)?;
            actions.extend(self.ingest_bundle(bundle)?);
            let payload = BundleIdPayload { bundle_id }.encode();
            actions.push(self.send_secure(link_id, FrameType::BundleComplete, payload)?);
        }
        Ok(actions)
    }

    fn plan_transfers(&self, now_ms: i64) -> Result<Vec<CoreAction>, CoreError> {
        let bundles = self.store.list_bundles()?;
        let mut actions = Vec::new();

        for (link_id, link) in &self.links {
            let peer = link.peer_context();

            let mut candidates = Vec::new();
            for stored in &bundles {
                let decision = self.router.evaluate(
                    &stored.bundle,
                    stored.first_seen_at_ms,
                    stored.forward_count,
                    &peer,
                    now_ms,
                );
                if matches!(
                    decision,
                    RoutingDecision::SendImmediately | RoutingDecision::Offer
                ) {
                    candidates.push((
                        ControlledEpidemicRouter::transfer_rank(&stored.bundle, &peer),
                        stored.first_seen_at_ms,
                        stored.bundle.immutable.bundle_id,
                    ));
                }
            }

            candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            for (_, _, bundle_id) in candidates {
                actions.push(CoreAction::BundleReadyForPeer {
                    link_id: *link_id,
                    bundle_id,
                });
            }
        }
        Ok(actions)
    }

    pub fn set_peer_inventory(
        &mut self,
        link_id: LinkId,
        user_id: Option<UserId>,
        bundle_ids: impl IntoIterator<Item = BundleId>,
    ) -> Result<(), CoreError> {
        let link = self
            .links
            .get_mut(&link_id)
            .ok_or(CoreError::UnknownLink(link_id))?;
        link.set_inventory(user_id, bundle_ids);
        Ok(())
    }

    pub fn mark_forwarded(&self, bundle_id: BundleId) -> Result<(), CoreError> {
        self.store.increment_forward_count(bundle_id)?;
        Ok(())
    }

    pub fn create_tombstone(
        &mut self,
        bundle_id: BundleId,
        reason: i64,
        created_at_ms: i64,
        expires_at_ms: i64,
    ) -> Result<(), CoreError> {
        self.store
            .create_tombstone(bundle_id, reason, created_at_ms, expires_at_ms)?;
        Ok(())
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

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("storage error: {0}")]
    Store(#[from] StoreError),
    #[error("wire error: {0}")]
    Wire(#[from] WireError),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("unknown contact {0}")]
    UnknownContact(UserId),
    #[error("invalid user message")]
    InvalidMessage,
    #[error("unknown link {0:?}")]
    UnknownLink(LinkId),
    #[error("mesh session error")]
    Session,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_wire::{ImmutableBundleHeader, RelayHeader};

    fn seed(start: u8) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = start.wrapping_add(i as u8);
        }
        bytes
    }

    fn test_core(identity: Identity, rng_seed: u8) -> MeshCore {
        MeshCore::open_in_memory(
            MeshConfig::default(),
            identity,
            seed(rng_seed.wrapping_add(50)),
            MeshClock::Fixed(1_700_000_000_000),
            MeshRng::deterministic(seed(rng_seed)),
        )
        .unwrap()
    }

    fn dummy_bundle(destination_id: UserId) -> WireBundle {
        let encrypted_payload = vec![1, 2, 3, 4];
        WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id: BundleId::random(),
                bundle_type: BundleType::DirectMessage,
                sender_id: UserId::random(),
                destination_id,
                created_at_ms: 0,
                ttl_seconds: 60,
                hop_limit: 20,
                priority: Priority::Normal,
                payload_length: encrypted_payload.len() as u32,
            },
            relay: RelayHeader {
                hop_count: 0,
                received_at_ms: 0,
            },
            encrypted_payload,
            sender_signature: vec![0; 64],
        }
    }

    #[test]
    fn tick_prioritises_bundle_for_current_destination() {
        let identity = Identity::from_seeds(seed(0x01), seed(0x02));
        let mut core = test_core(identity, 3);
        let destination = UserId::random();
        let bundle = dummy_bundle(destination);
        core.import_bundle(&bundle, 0).unwrap();

        let link = LinkId(1);
        core.process_event(CoreEvent::LinkOpened {
            link_id: link,
            peer_token: b"peer".to_vec(),
        })
        .unwrap();
        core.set_peer_inventory(link, Some(destination), [])
            .unwrap();

        let actions = core.process_event(CoreEvent::Tick { now_ms: 1 }).unwrap();
        assert_eq!(
            actions,
            vec![CoreAction::BundleReadyForPeer {
                link_id: link,
                bundle_id: bundle.immutable.bundle_id,
            }]
        );
    }

    #[test]
    fn send_text_delivers_only_to_recipient() {
        let alice_id = Identity::from_seeds(seed(0x00), seed(0x20));
        let bob_id = Identity::from_seeds(seed(0x40), seed(0x60));
        let charlie_id = Identity::from_seeds(seed(0xc0), seed(0xe0));

        let mut alice = test_core(alice_id, 10);
        let mut bob = test_core(bob_id, 11);
        let mut charlie = test_core(charlie_id, 12);

        alice.add_contact(bob.public_identity(), "Bob").unwrap();
        bob.add_contact(alice.public_identity(), "Alice").unwrap();

        let actions = alice.send_text(bob.user_id(), "Are you safe?").unwrap();
        assert!(actions
            .iter()
            .any(|action| matches!(action, CoreAction::MessageUpdated { .. })));
        assert_eq!(alice.plaintext_messages().unwrap()[0].text, "Are you safe?");

        let bundle = alice.stored_bundles().unwrap()[0].bundle.clone();
        let bob_actions = bob.accept_bundle(&bundle).unwrap();
        assert!(bob_actions
            .iter()
            .any(|action| matches!(action, CoreAction::MessageReceived { .. })));
        assert_eq!(bob.plaintext_messages().unwrap()[0].text, "Are you safe?");
        assert_eq!(
            bob.plaintext_messages().unwrap()[0].sender_id,
            alice.user_id()
        );

        charlie.accept_bundle(&bundle).unwrap();
        assert!(charlie.plaintext_messages().unwrap().is_empty());
        assert_eq!(charlie.stored_bundles().unwrap().len(), 1);
        assert_eq!(
            charlie.stored_bundles().unwrap()[0].bundle.relay.hop_count,
            1
        );
    }

    #[test]
    fn send_text_requires_a_contact() {
        let alice_id = Identity::from_seeds(seed(0x00), seed(0x20));
        let bob_id = Identity::from_seeds(seed(0x40), seed(0x60));
        let mut alice = test_core(alice_id, 10);
        let err = alice
            .send_text(bob_id.user_id(), "Are you safe?")
            .unwrap_err();
        assert!(matches!(err, CoreError::UnknownContact(_)));
    }

    fn drain_sends(
        actions: &mut Vec<CoreAction>,
        link_id: LinkId,
        inbox: &mut std::collections::VecDeque<Vec<u8>>,
        collected: &mut Vec<CoreAction>,
    ) {
        for action in actions.drain(..) {
            match action {
                CoreAction::SendBytes { link_id: id, data } if id == link_id => {
                    inbox.push_back(data)
                }
                other => collected.push(other),
            }
        }
    }

    fn pump_until_idle(
        alice: &mut MeshCore,
        bob: &mut MeshCore,
        a_link: LinkId,
        b_link: LinkId,
        mut a_actions: Vec<CoreAction>,
        mut b_actions: Vec<CoreAction>,
    ) -> (Vec<CoreAction>, Vec<CoreAction>) {
        let mut a_inbox = std::collections::VecDeque::new();
        let mut b_inbox = std::collections::VecDeque::new();
        let mut a_seen = Vec::new();
        let mut b_seen = Vec::new();
        loop {
            drain_sends(&mut a_actions, a_link, &mut b_inbox, &mut a_seen);
            drain_sends(&mut b_actions, b_link, &mut a_inbox, &mut b_seen);
            let mut progress = false;
            if let Some(data) = a_inbox.pop_front() {
                a_actions = alice
                    .process_event(CoreEvent::BytesReceived {
                        link_id: a_link,
                        data,
                    })
                    .unwrap();
                progress = true;
            }
            if let Some(data) = b_inbox.pop_front() {
                b_actions = bob
                    .process_event(CoreEvent::BytesReceived {
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
    fn session_transfers_send_text_to_peer() {
        let alice_id = Identity::from_seeds(seed(0x00), seed(0x20));
        let bob_id = Identity::from_seeds(seed(0x40), seed(0x60));
        let mut alice = test_core(alice_id, 10);
        let mut bob = test_core(bob_id, 11);
        alice.add_contact(bob.public_identity(), "Bob").unwrap();
        bob.add_contact(alice.public_identity(), "Alice").unwrap();
        alice.send_text(bob.user_id(), "Are you safe?").unwrap();

        let a_link = LinkId(1);
        let b_link = LinkId(1);
        let a_actions = alice
            .process_event(CoreEvent::LinkOpened {
                link_id: a_link,
                peer_token: b"bob".to_vec(),
            })
            .unwrap();
        let b_actions = bob
            .process_event(CoreEvent::LinkOpened {
                link_id: b_link,
                peer_token: b"alice".to_vec(),
            })
            .unwrap();
        let (_a_seen, b_seen) =
            pump_until_idle(&mut alice, &mut bob, a_link, b_link, a_actions, b_actions);

        assert!(b_seen
            .iter()
            .any(|action| matches!(action, CoreAction::MessageReceived { .. })));
        assert_eq!(bob.plaintext_messages().unwrap()[0].text, "Are you safe?");
        assert_eq!(
            bob.plaintext_messages().unwrap()[0].sender_id,
            alice.user_id()
        );
    }

    #[test]
    fn session_relays_ciphertext_without_reading_it() {
        let alice_id = Identity::from_seeds(seed(0x00), seed(0x20));
        let bob_id = Identity::from_seeds(seed(0x40), seed(0x60));
        let charlie_id = Identity::from_seeds(seed(0xc0), seed(0xe0));
        let mut alice = test_core(alice_id, 10);
        let mut bob = test_core(bob_id, 11);
        let mut charlie = test_core(charlie_id, 12);
        alice.add_contact(bob.public_identity(), "Bob").unwrap();
        bob.add_contact(alice.public_identity(), "Alice").unwrap();
        alice.send_text(bob.user_id(), "Are you safe?").unwrap();

        let ac = LinkId(1);
        let ca = LinkId(1);
        let a_actions = alice
            .process_event(CoreEvent::LinkOpened {
                link_id: ac,
                peer_token: b"charlie".to_vec(),
            })
            .unwrap();
        let c_actions = charlie
            .process_event(CoreEvent::LinkOpened {
                link_id: ca,
                peer_token: b"alice".to_vec(),
            })
            .unwrap();
        pump_until_idle(&mut alice, &mut charlie, ac, ca, a_actions, c_actions);
        assert!(charlie.plaintext_messages().unwrap().is_empty());
        assert_eq!(charlie.stored_bundles().unwrap().len(), 1);
        assert_eq!(
            charlie.stored_bundles().unwrap()[0].bundle.relay.hop_count,
            1
        );

        let cb = LinkId(2);
        let bc = LinkId(2);
        let c_actions = charlie
            .process_event(CoreEvent::LinkOpened {
                link_id: cb,
                peer_token: b"bob".to_vec(),
            })
            .unwrap();
        let b_actions = bob
            .process_event(CoreEvent::LinkOpened {
                link_id: bc,
                peer_token: b"charlie".to_vec(),
            })
            .unwrap();
        let (_c_seen, b_seen) =
            pump_until_idle(&mut charlie, &mut bob, cb, bc, c_actions, b_actions);
        assert!(b_seen
            .iter()
            .any(|action| matches!(action, CoreAction::MessageReceived { .. })));
        assert_eq!(bob.plaintext_messages().unwrap()[0].text, "Are you safe?");
    }
}
