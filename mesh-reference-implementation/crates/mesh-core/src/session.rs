use crate::CoreError;
use mesh_crypto::{
    derive_session_keys, handshake_transcript, open_session, seal_session, EphemeralSecret,
    SessionKeys,
};
use mesh_routing::PeerContext;
use mesh_types::{BundleId, DiscoveryId, LinkId, UserId};
use mesh_wire::{
    BundleData, BundleIdPayload, BundleOffer, ErrorPayload, Frame, FrameType, HelloPayload,
    InventorySummary, KeyPayload, CAPABILITY_TEXT, ERROR_UNSUPPORTED_VERSION, FLAG_ENCRYPTED,
    MAX_FRAME_PAYLOAD, PROTOCOL_MAJOR, PROTOCOL_MINOR,
};
use rand_core::{CryptoRng, RngCore};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    HelloSent,
    WaitKeyInit,
    WaitKeyReply,
    WaitSessionOk,
    Secure,
    Closed,
}

pub enum SessionEvent {
    Send(Frame),
    Established,
    PeerInventory(Vec<BundleId>),
    Offer(BundleOffer),
    Request(BundleId),
    Data(BundleData),
    Complete(BundleId),
    Close,
}

pub struct MeshSession {
    #[allow(dead_code)]
    link_id: LinkId,
    #[allow(dead_code)]
    peer_token: Vec<u8>,
    phase: Phase,
    local_hello: HelloPayload,
    local_hello_bytes: Vec<u8>,
    remote_hello: Option<HelloPayload>,
    remote_hello_bytes: Option<Vec<u8>>,
    local_eph: Option<EphemeralSecret>,
    local_eph_public: Option<[u8; 32]>,
    remote_eph: Option<[u8; 32]>,
    keys: Option<SessionKeys>,
    tx_counter: u64,
    rx_counter: u64,
    max_frame: usize,
    user_id: Option<UserId>,
    known_bundle_ids: HashSet<BundleId>,
    pending_offers: HashMap<BundleId, u32>,
    incoming: HashMap<BundleId, Vec<u8>>,
}

impl MeshSession {
    pub fn open<R: CryptoRng + RngCore>(
        link_id: LinkId,
        peer_token: Vec<u8>,
        local_discovery_id: DiscoveryId,
        rng: &mut R,
    ) -> (Self, Frame) {
        let mut session_nonce = [0u8; 32];
        rng.fill_bytes(&mut session_nonce);
        let local_hello = HelloPayload {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            discovery_id: local_discovery_id,
            session_nonce,
            capabilities: CAPABILITY_TEXT,
            max_frame_size: MAX_FRAME_PAYLOAD as u32,
        };
        let local_hello_bytes = local_hello.encode();
        let frame = Frame::cleartext(FrameType::Hello, local_hello_bytes.clone());
        let session = Self {
            link_id,
            peer_token,
            phase: Phase::HelloSent,
            local_hello,
            local_hello_bytes,
            remote_hello: None,
            remote_hello_bytes: None,
            local_eph: None,
            local_eph_public: None,
            remote_eph: None,
            keys: None,
            tx_counter: 0,
            rx_counter: 0,
            max_frame: MAX_FRAME_PAYLOAD,
            user_id: None,
            known_bundle_ids: HashSet::new(),
            pending_offers: HashMap::new(),
            incoming: HashMap::new(),
        };
        (session, frame)
    }

    pub fn peer_context(&self) -> PeerContext {
        PeerContext {
            user_id: self.user_id,
            known_bundle_ids: self.known_bundle_ids.clone(),
        }
    }

    pub fn set_inventory(
        &mut self,
        user_id: Option<UserId>,
        bundle_ids: impl IntoIterator<Item = BundleId>,
    ) {
        self.user_id = user_id;
        self.known_bundle_ids = bundle_ids.into_iter().collect();
    }

    pub fn set_known_ids(&mut self, ids: Vec<BundleId>) {
        self.known_bundle_ids = ids.into_iter().collect();
    }

    pub fn note_peer_has(&mut self, bundle_id: BundleId) {
        self.known_bundle_ids.insert(bundle_id);
    }

    pub fn note_offer(&mut self, offer: &BundleOffer) {
        self.pending_offers.insert(offer.bundle_id, offer.size);
    }

    pub fn plaintext_chunk_size(&self) -> usize {
        let limit = self.max_frame.min(MAX_FRAME_PAYLOAD).saturating_sub(16);
        limit.saturating_sub(64).max(1024)
    }

    pub fn is_secure(&self) -> bool {
        self.phase == Phase::Secure
    }

    pub fn append_data(&mut self, data: &BundleData) -> Result<Option<Vec<u8>>, CoreError> {
        let buf = self.incoming.entry(data.bundle_id).or_default();
        if data.offset as usize != buf.len() {
            return Err(CoreError::Session);
        }
        buf.extend_from_slice(&data.data);
        let ready = self
            .pending_offers
            .get(&data.bundle_id)
            .copied()
            .is_some_and(|size| buf.len() as u32 >= size);
        if ready {
            let complete = self.incoming.remove(&data.bundle_id).unwrap();
            self.pending_offers.remove(&data.bundle_id);
            Ok(Some(complete))
        } else {
            Ok(None)
        }
    }

    pub fn encrypt(&mut self, ty: FrameType, plaintext: Vec<u8>) -> Result<Frame, CoreError> {
        let keys = self.keys.as_ref().ok_or(CoreError::Session)?;
        if self.tx_counter == u64::MAX {
            return Err(CoreError::Session);
        }
        let flags = FLAG_ENCRYPTED;
        let aad = [PROTOCOL_MAJOR, PROTOCOL_MINOR, ty as u8, flags];
        let payload = seal_session(&keys.tx, self.tx_counter, &aad, &plaintext)?;
        self.tx_counter += 1;
        Ok(Frame {
            major: PROTOCOL_MAJOR,
            minor: PROTOCOL_MINOR,
            ty,
            flags,
            payload,
        })
    }

    pub fn handle_frame<R: CryptoRng + RngCore>(
        &mut self,
        frame: Frame,
        rng: &mut R,
    ) -> Result<Vec<SessionEvent>, CoreError> {
        if self.phase == Phase::Closed {
            return Err(CoreError::Session);
        }
        if frame.is_encrypted() {
            let ty = frame.ty;
            let plaintext = self.decrypt(&frame)?;
            return self.handle_secure(ty, plaintext);
        }
        if matches!(self.phase, Phase::Secure | Phase::WaitSessionOk) {
            return Err(CoreError::Session);
        }
        match frame.ty {
            FrameType::Hello => self.on_hello(frame, rng),
            FrameType::KeyInit => self.on_key_init(frame, rng),
            FrameType::KeyReply => self.on_key_reply(frame),
            FrameType::ErrorFrame => {
                self.phase = Phase::Closed;
                Ok(vec![SessionEvent::Close])
            }
            _ => Err(CoreError::Session),
        }
    }

    fn handle_secure(
        &mut self,
        ty: FrameType,
        plaintext: Vec<u8>,
    ) -> Result<Vec<SessionEvent>, CoreError> {
        match ty {
            FrameType::SessionOk => self.on_session_ok(),
            FrameType::InventorySummary => {
                self.require_secure()?;
                let summary = InventorySummary::decode(&plaintext)?;
                Ok(vec![SessionEvent::PeerInventory(summary.bundle_ids)])
            }
            FrameType::InventoryRequest => {
                self.require_secure()?;
                Ok(Vec::new())
            }
            FrameType::BundleOffer => {
                self.require_secure()?;
                Ok(vec![SessionEvent::Offer(BundleOffer::decode(&plaintext)?)])
            }
            FrameType::BundleRequest => {
                self.require_secure()?;
                let payload = BundleIdPayload::decode(&plaintext)?;
                Ok(vec![SessionEvent::Request(payload.bundle_id)])
            }
            FrameType::BundleData => {
                self.require_secure()?;
                Ok(vec![SessionEvent::Data(BundleData::decode(&plaintext)?)])
            }
            FrameType::BundleComplete => {
                self.require_secure()?;
                let payload = BundleIdPayload::decode(&plaintext)?;
                Ok(vec![SessionEvent::Complete(payload.bundle_id)])
            }
            FrameType::ErrorFrame => {
                self.phase = Phase::Closed;
                Ok(vec![SessionEvent::Close])
            }
            _ => Err(CoreError::Session),
        }
    }

    fn on_hello<R: CryptoRng + RngCore>(
        &mut self,
        frame: Frame,
        rng: &mut R,
    ) -> Result<Vec<SessionEvent>, CoreError> {
        if self.phase != Phase::HelloSent || self.remote_hello.is_some() {
            return Err(CoreError::Session);
        }
        let hello = HelloPayload::decode(&frame.payload)?;
        if hello.protocol_major != PROTOCOL_MAJOR {
            self.phase = Phase::Closed;
            let error = ErrorPayload {
                code: ERROR_UNSUPPORTED_VERSION,
            };
            return Ok(vec![
                SessionEvent::Send(Frame::cleartext(FrameType::ErrorFrame, error.encode())),
                SessionEvent::Close,
            ]);
        }
        if hello.discovery_id.as_bytes() == self.local_hello.discovery_id.as_bytes() {
            return Err(CoreError::Session);
        }
        self.max_frame = (self.local_hello.max_frame_size.min(hello.max_frame_size) as usize)
            .clamp(1024, MAX_FRAME_PAYLOAD);
        self.remote_hello_bytes = Some(frame.payload);
        self.remote_hello = Some(hello);
        if self.local_is_lo() {
            self.generate_ephemeral(rng);
            let key = KeyPayload {
                ephemeral_public: self.local_eph_public.unwrap(),
            };
            self.phase = Phase::WaitKeyReply;
            Ok(vec![SessionEvent::Send(Frame::cleartext(
                FrameType::KeyInit,
                key.encode(),
            ))])
        } else {
            self.phase = Phase::WaitKeyInit;
            Ok(Vec::new())
        }
    }

    fn on_key_init<R: CryptoRng + RngCore>(
        &mut self,
        frame: Frame,
        rng: &mut R,
    ) -> Result<Vec<SessionEvent>, CoreError> {
        if self.phase != Phase::WaitKeyInit {
            return Err(CoreError::Session);
        }
        let key = KeyPayload::decode(&frame.payload)?;
        self.remote_eph = Some(key.ephemeral_public);
        self.generate_ephemeral(rng);
        self.derive_keys()?;
        let reply = KeyPayload {
            ephemeral_public: self.local_eph_public.unwrap(),
        };
        let session_ok = self.encrypt(FrameType::SessionOk, empty_map())?;
        self.phase = Phase::WaitSessionOk;
        Ok(vec![
            SessionEvent::Send(Frame::cleartext(FrameType::KeyReply, reply.encode())),
            SessionEvent::Send(session_ok),
        ])
    }

    fn on_key_reply(&mut self, frame: Frame) -> Result<Vec<SessionEvent>, CoreError> {
        if self.phase != Phase::WaitKeyReply {
            return Err(CoreError::Session);
        }
        let key = KeyPayload::decode(&frame.payload)?;
        self.remote_eph = Some(key.ephemeral_public);
        self.derive_keys()?;
        let session_ok = self.encrypt(FrameType::SessionOk, empty_map())?;
        self.phase = Phase::WaitSessionOk;
        Ok(vec![SessionEvent::Send(session_ok)])
    }

    fn on_session_ok(&mut self) -> Result<Vec<SessionEvent>, CoreError> {
        if self.phase != Phase::WaitSessionOk {
            return Err(CoreError::Session);
        }
        self.phase = Phase::Secure;
        Ok(vec![SessionEvent::Established])
    }

    fn decrypt(&mut self, frame: &Frame) -> Result<Vec<u8>, CoreError> {
        let keys = self.keys.as_ref().ok_or(CoreError::Session)?;
        let aad = [frame.major, frame.minor, frame.ty as u8, frame.flags];
        let plaintext = open_session(&keys.rx, self.rx_counter, &aad, &frame.payload)?;
        self.rx_counter = self.rx_counter.checked_add(1).ok_or(CoreError::Session)?;
        Ok(plaintext)
    }

    fn generate_ephemeral<R: CryptoRng + RngCore>(&mut self, rng: &mut R) {
        let secret = EphemeralSecret::generate(rng);
        self.local_eph_public = Some(secret.public_bytes());
        self.local_eph = Some(secret);
    }

    fn derive_keys(&mut self) -> Result<(), CoreError> {
        let remote_hello_bytes = self.remote_hello_bytes.as_ref().ok_or(CoreError::Session)?;
        let local_eph = self.local_eph.as_ref().ok_or(CoreError::Session)?;
        let local_eph_public = self.local_eph_public.ok_or(CoreError::Session)?;
        let remote_eph = self.remote_eph.ok_or(CoreError::Session)?;
        let local_is_lo = self.local_is_lo();
        let (hello_lo, hello_hi, eph_lo, eph_hi) = if local_is_lo {
            (
                self.local_hello_bytes.as_slice(),
                remote_hello_bytes.as_slice(),
                local_eph_public,
                remote_eph,
            )
        } else {
            (
                remote_hello_bytes.as_slice(),
                self.local_hello_bytes.as_slice(),
                remote_eph,
                local_eph_public,
            )
        };
        let transcript = handshake_transcript(hello_lo, hello_hi, &eph_lo, &eph_hi);
        let shared = local_eph.shared_secret(&remote_eph);
        self.keys = Some(derive_session_keys(&shared, &transcript, local_is_lo)?);
        self.local_eph = None;
        Ok(())
    }

    fn local_is_lo(&self) -> bool {
        let remote = self
            .remote_hello
            .as_ref()
            .expect("remote HELLO is present")
            .discovery_id;
        self.local_hello.discovery_id.as_bytes() < remote.as_bytes()
    }

    fn require_secure(&self) -> Result<(), CoreError> {
        if self.phase == Phase::Secure {
            Ok(())
        } else {
            Err(CoreError::Session)
        }
    }
}

fn empty_map() -> Vec<u8> {
    vec![0xa0]
}
