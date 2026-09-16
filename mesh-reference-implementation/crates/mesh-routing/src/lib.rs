use mesh_types::{BundleId, BundleType, Priority, UserId};
use mesh_wire::WireBundle;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct PeerContext {
    pub user_id: Option<UserId>,
    pub known_bundle_ids: HashSet<BundleId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutingDecision {
    SendImmediately,
    Offer,
    Skip,
    Drop,
}

#[derive(Clone, Debug)]
pub struct ControlledEpidemicRouter {
    pub normal_replication_limit: u32,
    pub high_replication_limit: u32,
    pub emergency_replication_limit: u32,
}

impl Default for ControlledEpidemicRouter {
    fn default() -> Self {
        Self {
            normal_replication_limit: 6,
            high_replication_limit: 12,
            emergency_replication_limit: 20,
        }
    }
}

impl ControlledEpidemicRouter {
    pub fn evaluate(
        &self,
        bundle: &WireBundle,
        first_seen_at_ms: i64,
        forward_count: u32,
        peer: &PeerContext,
        now_ms: i64,
    ) -> RoutingDecision {
        if bundle.is_expired(now_ms, first_seen_at_ms) {
            return RoutingDecision::Drop;
        }
        if bundle.relay.hop_count >= bundle.immutable.hop_limit {
            return RoutingDecision::Skip;
        }
        if peer.known_bundle_ids.contains(&bundle.immutable.bundle_id) {
            return RoutingDecision::Skip;
        }
        if peer.user_id == Some(bundle.immutable.destination_id) {
            return RoutingDecision::SendImmediately;
        }
        if forward_count >= self.replication_limit(bundle) {
            return RoutingDecision::Skip;
        }
        RoutingDecision::Offer
    }

    pub fn transfer_rank(bundle: &WireBundle, peer: &PeerContext) -> u8 {
        if peer.user_id == Some(bundle.immutable.destination_id) {
            return 255;
        }
        if bundle.immutable.bundle_type == BundleType::DeliveryAck {
            return 250;
        }
        match bundle.immutable.priority {
            Priority::Emergency => 240,
            Priority::High => 200,
            Priority::Normal => 100,
            Priority::Bulk => 10,
        }
    }

    fn replication_limit(&self, bundle: &WireBundle) -> u32 {
        match bundle.immutable.priority {
            Priority::Bulk => 2,
            Priority::Normal => self.normal_replication_limit,
            Priority::High => self.high_replication_limit,
            Priority::Emergency => self.emergency_replication_limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_types::{BundleId, BundleType};
    use mesh_wire::{ImmutableBundleHeader, RelayHeader, PROTOCOL_MAJOR, PROTOCOL_MINOR};

    fn bundle(destination: UserId) -> WireBundle {
        let payload = vec![7];
        WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id: BundleId::random(),
                bundle_type: BundleType::DirectMessage,
                sender_id: UserId::random(),
                destination_id: destination,
                created_at_ms: 0,
                ttl_seconds: 60,
                hop_limit: 20,
                priority: Priority::Normal,
                payload_length: 1,
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
    fn destination_is_immediate() {
        let destination = UserId::random();
        let bundle = bundle(destination);
        let peer = PeerContext {
            user_id: Some(destination),
            known_bundle_ids: HashSet::new(),
        };
        let result = ControlledEpidemicRouter::default().evaluate(&bundle, 0, 0, &peer, 1);
        assert_eq!(result, RoutingDecision::SendImmediately);
    }

    #[test]
    fn known_bundle_is_skipped() {
        let destination = UserId::random();
        let bundle = bundle(destination);
        let mut known = HashSet::new();
        known.insert(bundle.immutable.bundle_id);
        let peer = PeerContext {
            user_id: None,
            known_bundle_ids: known,
        };
        let result = ControlledEpidemicRouter::default().evaluate(&bundle, 0, 0, &peer, 1);
        assert_eq!(result, RoutingDecision::Skip);
    }
}
