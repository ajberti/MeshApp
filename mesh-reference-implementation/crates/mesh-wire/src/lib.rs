use mesh_types::{BundleId, BundleType, Priority, UserId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_MAJOR: u8 = 1;
pub const PROTOCOL_MINOR: u8 = 0;
pub const MAX_TEXT_BUNDLE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImmutableBundleHeader {
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub bundle_id: BundleId,
    pub bundle_type: BundleType,
    pub sender_id: UserId,
    pub destination_id: UserId,
    pub created_at_ms: i64,
    pub ttl_seconds: u32,
    pub hop_limit: u16,
    pub priority: Priority,
    pub payload_length: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayHeader {
    pub hop_count: u16,
    pub received_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireBundle {
    pub immutable: ImmutableBundleHeader,
    pub relay: RelayHeader,
    pub encrypted_payload: Vec<u8>,
    pub sender_signature: Vec<u8>,
}

impl WireBundle {
    pub fn validate_shape(&self) -> Result<(), WireError> {
        if self.immutable.protocol_major != PROTOCOL_MAJOR {
            return Err(WireError::UnsupportedMajorVersion(
                self.immutable.protocol_major,
            ));
        }
        if self.encrypted_payload.len() != self.immutable.payload_length as usize {
            return Err(WireError::PayloadLengthMismatch);
        }
        if self.encrypted_payload.len() > MAX_TEXT_BUNDLE_BYTES {
            return Err(WireError::BundleTooLarge);
        }
        if self.immutable.hop_limit == 0 {
            return Err(WireError::InvalidHopLimit);
        }
        Ok(())
    }

    pub fn encode_cbor(&self) -> Result<Vec<u8>, WireError> {
        self.validate_shape()?;
        serde_cbor::to_vec(self).map_err(WireError::Encode)
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_TEXT_BUNDLE_BYTES + 4096 {
            return Err(WireError::BundleTooLarge);
        }
        let bundle: WireBundle = serde_cbor::from_slice(bytes).map_err(WireError::Decode)?;
        bundle.validate_shape()?;
        Ok(bundle)
    }

    pub fn is_expired(&self, now_ms: i64, first_seen_at_ms: i64) -> bool {
        let ttl_ms = i64::from(self.immutable.ttl_seconds).saturating_mul(1000);
        now_ms.saturating_sub(first_seen_at_ms) >= ttl_ms
    }
}

#[derive(Debug, Error)]
pub enum WireError {
    #[error("unsupported protocol major version {0}")]
    UnsupportedMajorVersion(u8),
    #[error("payload length does not match header")]
    PayloadLengthMismatch,
    #[error("bundle exceeds protocol size limit")]
    BundleTooLarge,
    #[error("hop limit must be greater than zero")]
    InvalidHopLimit,
    #[error("failed to encode CBOR: {0}")]
    Encode(serde_cbor::Error),
    #[error("failed to decode CBOR: {0}")]
    Decode(serde_cbor::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bundle() -> WireBundle {
        let payload = b"ciphertext".to_vec();
        WireBundle {
            immutable: ImmutableBundleHeader {
                protocol_major: PROTOCOL_MAJOR,
                protocol_minor: PROTOCOL_MINOR,
                bundle_id: BundleId::random(),
                bundle_type: BundleType::DirectMessage,
                sender_id: UserId::random(),
                destination_id: UserId::random(),
                created_at_ms: 1_000,
                ttl_seconds: 60,
                hop_limit: 20,
                priority: Priority::Normal,
                payload_length: payload.len() as u32,
            },
            relay: RelayHeader {
                hop_count: 0,
                received_at_ms: 1_000,
            },
            encrypted_payload: payload,
            sender_signature: vec![0; 64],
        }
    }

    #[test]
    fn cbor_round_trip() {
        let bundle = sample_bundle();
        let encoded = bundle.encode_cbor().unwrap();
        let decoded = WireBundle::decode_cbor(&encoded).unwrap();
        assert_eq!(bundle, decoded);
    }

    #[test]
    fn expiry_uses_local_first_seen_time() {
        let bundle = sample_bundle();
        assert!(!bundle.is_expired(59_000, 0));
        assert!(bundle.is_expired(60_000, 0));
    }
}
