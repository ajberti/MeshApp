use mesh_routing::{ControlledEpidemicRouter, PeerContext, RoutingDecision};
use mesh_store::{InsertOutcome, MeshStore, StoreError};
use mesh_types::{BundleId, DiscoveryId, LinkId, MessageId, UserId};
use mesh_wire::{WireBundle, WireError};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

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
    Log {
        code: String,
    },
}

#[derive(Clone, Debug)]
struct LinkState {
    user_id: Option<UserId>,
    known_bundle_ids: HashSet<BundleId>,
}

pub struct MeshCore {
    store: MeshStore,
    router: ControlledEpidemicRouter,
    peers: HashMap<Vec<u8>, DiscoveryId>,
    links: HashMap<LinkId, LinkState>,
    local_discovery_id: DiscoveryId,
    #[allow(dead_code)]
    config: MeshConfig,
}

impl MeshCore {
    pub fn open(path: impl AsRef<Path>, config: MeshConfig) -> Result<Self, CoreError> {
        let store = MeshStore::open(path)?;
        Ok(Self::from_store(store, config))
    }

    pub fn open_in_memory(config: MeshConfig) -> Result<Self, CoreError> {
        let store = MeshStore::open_in_memory()?;
        Ok(Self::from_store(store, config))
    }

    fn from_store(store: MeshStore, config: MeshConfig) -> Self {
        let router = ControlledEpidemicRouter {
            normal_replication_limit: config.normal_replication_limit,
            high_replication_limit: config.high_replication_limit,
            emergency_replication_limit: config.emergency_replication_limit,
        };
        Self {
            store,
            router,
            peers: HashMap::new(),
            links: HashMap::new(),
            local_discovery_id: DiscoveryId::random(),
            config,
        }
    }

    pub fn local_discovery_id(&self) -> DiscoveryId {
        self.local_discovery_id
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
                peer_token: _,
            } => {
                self.links.insert(
                    link_id,
                    LinkState {
                        user_id: None,
                        known_bundle_ids: HashSet::new(),
                    },
                );
                Ok(vec![CoreAction::Log {
                    code: "LINK_OPENED".into(),
                }])
            }
            CoreEvent::LinkClosed { link_id } => {
                self.links.remove(&link_id);
                Ok(Vec::new())
            }
            CoreEvent::BytesReceived { link_id, data } => self.receive_wire_bundle(link_id, &data),
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

    fn receive_wire_bundle(
        &mut self,
        _link_id: LinkId,
        data: &[u8],
    ) -> Result<Vec<CoreAction>, CoreError> {
        let bundle = WireBundle::decode_cbor(data)?;
        let bundle_id = bundle.immutable.bundle_id;
        let outcome = self
            .store
            .insert_bundle(&bundle, bundle.relay.received_at_ms)?;
        Ok(match outcome {
            InsertOutcome::Inserted => vec![CoreAction::BundleStored { bundle_id }],
            InsertOutcome::Duplicate | InsertOutcome::RejectedTombstoned => {
                vec![CoreAction::BundleDuplicate { bundle_id }]
            }
        })
    }

    fn plan_transfers(&self, now_ms: i64) -> Result<Vec<CoreAction>, CoreError> {
        let bundles = self.store.list_bundles()?;
        let mut actions = Vec::new();

        for (link_id, link) in &self.links {
            let peer = PeerContext {
                user_id: link.user_id,
                known_bundle_ids: link.known_bundle_ids.clone(),
            };

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
        link.user_id = user_id;
        link.known_bundle_ids = bundle_ids.into_iter().collect();
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

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("storage error: {0}")]
    Store(#[from] StoreError),
    #[error("wire error: {0}")]
    Wire(#[from] WireError),
    #[error("unknown link {0:?}")]
    UnknownLink(LinkId),
}

// Reserved now so adding user-message APIs does not force a public type rename later.
#[allow(dead_code)]
fn _message_type_marker(_: MessageId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_types::{BundleType, Priority};
    use mesh_wire::{ImmutableBundleHeader, RelayHeader, PROTOCOL_MAJOR, PROTOCOL_MINOR};

    fn bundle(destination_id: UserId) -> WireBundle {
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
        let mut core = MeshCore::open_in_memory(MeshConfig::default()).unwrap();
        let destination = UserId::random();
        let bundle = bundle(destination);
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

    fn seed(start: u8) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, slot) in bytes.iter_mut().enumerate() {
            *slot = start.wrapping_add(i as u8);
        }
        bytes
    }

    #[test]
    fn signed_encrypted_bundle_is_only_readable_by_recipient() {
        use mesh_crypto::{
            conversation_id, open_message, seal_message_with, verify_signature, Identity,
        };
        use mesh_types::{ConversationId, MessageId};
        use mesh_wire::{DirectMessagePayload, SealedPayload, DIRECT_MESSAGE_VERSION};

        let alice = Identity::from_seeds(seed(0x00), seed(0x20));
        let bob = Identity::from_seeds(seed(0x40), seed(0x60));
        let charlie = Identity::from_seeds(seed(0xc0), seed(0xe0));

        let payload = DirectMessagePayload {
            version: DIRECT_MESSAGE_VERSION,
            message_id: MessageId::from_bytes([0x10; 16]),
            conversation_id: conversation_id(alice.user_id(), bob.user_id()),
            sender_id: alice.user_id(),
            recipient_id: bob.user_id(),
            sequence: 1,
            sent_at_ms: 1_700_000_000_000,
            content_type: "text/plain".into(),
            content: b"Are you safe?".to_vec(),
        };
        let plaintext = payload.encode_cbor();
        let sealed = seal_message_with(
            bob.encryption_public(),
            &plaintext,
            seed(0x80),
            [
                0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab,
            ],
        )
        .unwrap();
        let encrypted_payload = SealedPayload {
            ephemeral_public: sealed.ephemeral_public,
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext.clone(),
        }
        .encode_cbor();

        let mut bundle = WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id: BundleId::from_bytes([0x31; 16]),
                bundle_type: BundleType::DirectMessage,
                sender_id: alice.user_id(),
                destination_id: bob.user_id(),
                created_at_ms: 1_700_000_000_000,
                ttl_seconds: 60,
                hop_limit: 20,
                priority: Priority::Normal,
                payload_length: encrypted_payload.len() as u32,
            },
            relay: RelayHeader {
                hop_count: 0,
                received_at_ms: 1_700_000_000_000,
            },
            encrypted_payload,
            sender_signature: vec![0; 64],
        };
        bundle.sender_signature = alice.sign(&bundle.signed_bytes().unwrap()).to_vec();

        verify_signature(
            alice.signing_public(),
            &bundle.signed_bytes().unwrap(),
            bundle.sender_signature.as_slice().try_into().unwrap(),
        )
        .unwrap();

        bundle.relay.hop_count = 4;
        verify_signature(
            alice.signing_public(),
            &bundle.signed_bytes().unwrap(),
            bundle.sender_signature.as_slice().try_into().unwrap(),
        )
        .unwrap();

        let opened = open_message(&bob, &sealed).unwrap();
        let decoded = DirectMessagePayload::decode_cbor(&opened).unwrap();
        assert_eq!(decoded.content, b"Are you safe?");
        assert_eq!(
            decoded.conversation_id,
            ConversationId::from_bytes(*conversation_id(bob.user_id(), alice.user_id()).as_bytes())
        );
        assert!(open_message(&charlie, &sealed).is_err());

        bundle.encrypted_payload[0] ^= 1;
        assert!(verify_signature(
            alice.signing_public(),
            &bundle.signed_bytes().unwrap(),
            bundle.sender_signature.as_slice().try_into().unwrap(),
        )
        .is_err());
    }
}
