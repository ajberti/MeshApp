use crate::cbor::{CborReader, CborWriter};
use crate::error::WireError;
use mesh_types::{BundleId, BundleType, Priority, UserId};

pub const PROTOCOL_MAJOR: u8 = 1;
pub const PROTOCOL_MINOR: u8 = 0;
pub const MAX_TEXT_BUNDLE_BYTES: usize = 64 * 1024;
pub const SIGNATURE_LENGTH: usize = 64;

pub const KEY_VERSION: u8 = 1;
pub const KEY_BUNDLE_ID: u8 = 2;
pub const KEY_BUNDLE_TYPE: u8 = 3;
pub const KEY_SENDER_ID: u8 = 4;
pub const KEY_DESTINATION_ID: u8 = 5;
pub const KEY_CREATED_AT: u8 = 6;
pub const KEY_TTL_SECONDS: u8 = 7;
pub const KEY_HOP_LIMIT: u8 = 8;
pub const KEY_PRIORITY: u8 = 9;
pub const KEY_PAYLOAD_LENGTH: u8 = 10;
pub const KEY_ENCRYPTED_PAYLOAD: u8 = 11;
pub const KEY_SIGNATURE: u8 = 12;
pub const KEY_HOP_COUNT: u8 = 20;
pub const KEY_RECEIVED_AT: u8 = 21;

pub fn protocol_version_code(major: u8, minor: u8) -> u16 {
    u16::from(major) << 8 | u16::from(minor)
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayHeader {
    pub hop_count: u16,
    pub received_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireBundle {
    pub immutable: ImmutableBundleHeader,
    pub relay: RelayHeader,
    pub encrypted_payload: Vec<u8>,
    pub sender_signature: Vec<u8>,
}

impl WireBundle {
    pub fn protocol_version(&self) -> u16 {
        protocol_version_code(self.immutable.protocol_major, self.immutable.protocol_minor)
    }

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
        if self.sender_signature.len() != SIGNATURE_LENGTH {
            return Err(WireError::InvalidSignatureLength);
        }
        Ok(())
    }

    pub fn signed_bytes(&self) -> Result<Vec<u8>, WireError> {
        self.validate_shape()?;
        Ok(encode_signed_fields(
            &self.immutable,
            &self.encrypted_payload,
        ))
    }

    pub fn encode_cbor(&self) -> Result<Vec<u8>, WireError> {
        self.validate_shape()?;
        let mut w = CborWriter::new();
        w.map(14);
        write_signed_fields(&mut w, &self.immutable, &self.encrypted_payload);
        w.u64(u64::from(KEY_SIGNATURE));
        w.bytes(&self.sender_signature);
        w.u64(u64::from(KEY_HOP_COUNT));
        w.u64(u64::from(self.relay.hop_count));
        w.u64(u64::from(KEY_RECEIVED_AT));
        w.i64(self.relay.received_at_ms);
        Ok(w.into_inner())
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_TEXT_BUNDLE_BYTES + 4096 {
            return Err(WireError::BundleTooLarge);
        }
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut version = None;
        let mut bundle_id = None;
        let mut bundle_type = None;
        let mut sender_id = None;
        let mut destination_id = None;
        let mut created_at_ms = None;
        let mut ttl_seconds = None;
        let mut hop_limit = None;
        let mut priority = None;
        let mut payload_length = None;
        let mut encrypted_payload = None;
        let mut sender_signature = None;
        let mut hop_count = None;
        let mut received_at_ms = None;

        for _ in 0..n {
            let key = r.u64()?;
            if key > u64::from(u8::MAX) {
                r.skip_value()?;
                continue;
            }
            match key as u8 {
                KEY_VERSION => set_once(&mut version, r.u64()?, KEY_VERSION)?,
                KEY_BUNDLE_ID => set_once(&mut bundle_id, read_id16(&mut r)?, KEY_BUNDLE_ID)?,
                KEY_BUNDLE_TYPE => set_once(&mut bundle_type, r.u64()?, KEY_BUNDLE_TYPE)?,
                KEY_SENDER_ID => set_once(&mut sender_id, read_id16(&mut r)?, KEY_SENDER_ID)?,
                KEY_DESTINATION_ID => {
                    set_once(&mut destination_id, read_id16(&mut r)?, KEY_DESTINATION_ID)?
                }
                KEY_CREATED_AT => set_once(&mut created_at_ms, r.i64()?, KEY_CREATED_AT)?,
                KEY_TTL_SECONDS => set_once(&mut ttl_seconds, r.u64()?, KEY_TTL_SECONDS)?,
                KEY_HOP_LIMIT => set_once(&mut hop_limit, r.u64()?, KEY_HOP_LIMIT)?,
                KEY_PRIORITY => set_once(&mut priority, r.u64()?, KEY_PRIORITY)?,
                KEY_PAYLOAD_LENGTH => set_once(&mut payload_length, r.u64()?, KEY_PAYLOAD_LENGTH)?,
                KEY_ENCRYPTED_PAYLOAD => set_once(
                    &mut encrypted_payload,
                    r.bytes()?.to_vec(),
                    KEY_ENCRYPTED_PAYLOAD,
                )?,
                KEY_SIGNATURE => {
                    set_once(&mut sender_signature, r.bytes()?.to_vec(), KEY_SIGNATURE)?
                }
                KEY_HOP_COUNT => set_once(&mut hop_count, r.u64()?, KEY_HOP_COUNT)?,
                KEY_RECEIVED_AT => set_once(&mut received_at_ms, r.i64()?, KEY_RECEIVED_AT)?,
                _ => r.skip_value()?,
            }
        }
        r.finish()?;

        let version = require(version, KEY_VERSION)?;
        let protocol_major =
            u8::try_from(version >> 8).map_err(|_| WireError::IntegerOutOfRange)?;
        let protocol_minor =
            u8::try_from(version & 0xff).map_err(|_| WireError::IntegerOutOfRange)?;
        let bundle = Self {
            immutable: ImmutableBundleHeader {
                protocol_major,
                protocol_minor,
                bundle_id: BundleId::from_bytes(require(bundle_id, KEY_BUNDLE_ID)?),
                bundle_type: bundle_type_from(require(bundle_type, KEY_BUNDLE_TYPE)?)?,
                sender_id: UserId::from_bytes(require(sender_id, KEY_SENDER_ID)?),
                destination_id: UserId::from_bytes(require(destination_id, KEY_DESTINATION_ID)?),
                created_at_ms: require(created_at_ms, KEY_CREATED_AT)?,
                ttl_seconds: u32_from(require(ttl_seconds, KEY_TTL_SECONDS)?)?,
                hop_limit: u16_from(require(hop_limit, KEY_HOP_LIMIT)?)?,
                priority: priority_from(require(priority, KEY_PRIORITY)?)?,
                payload_length: u32_from(require(payload_length, KEY_PAYLOAD_LENGTH)?)?,
            },
            relay: RelayHeader {
                hop_count: u16_from(require(hop_count, KEY_HOP_COUNT)?)?,
                received_at_ms: require(received_at_ms, KEY_RECEIVED_AT)?,
            },
            encrypted_payload: require(encrypted_payload, KEY_ENCRYPTED_PAYLOAD)?,
            sender_signature: require(sender_signature, KEY_SIGNATURE)?,
        };
        bundle.validate_shape()?;
        Ok(bundle)
    }

    pub fn is_expired(&self, now_ms: i64, first_seen_at_ms: i64) -> bool {
        let ttl_ms = i64::from(self.immutable.ttl_seconds).saturating_mul(1000);
        now_ms.saturating_sub(first_seen_at_ms) >= ttl_ms
    }
}

fn encode_signed_fields(header: &ImmutableBundleHeader, encrypted_payload: &[u8]) -> Vec<u8> {
    let mut w = CborWriter::new();
    w.map(11);
    write_signed_fields(&mut w, header, encrypted_payload);
    w.into_inner()
}

fn write_signed_fields(
    w: &mut CborWriter,
    header: &ImmutableBundleHeader,
    encrypted_payload: &[u8],
) {
    w.u64(u64::from(KEY_VERSION));
    w.u64(u64::from(protocol_version_code(
        header.protocol_major,
        header.protocol_minor,
    )));
    w.u64(u64::from(KEY_BUNDLE_ID));
    w.bytes(header.bundle_id.as_bytes());
    w.u64(u64::from(KEY_BUNDLE_TYPE));
    w.u64(u64::from(header.bundle_type as u8));
    w.u64(u64::from(KEY_SENDER_ID));
    w.bytes(header.sender_id.as_bytes());
    w.u64(u64::from(KEY_DESTINATION_ID));
    w.bytes(header.destination_id.as_bytes());
    w.u64(u64::from(KEY_CREATED_AT));
    w.i64(header.created_at_ms);
    w.u64(u64::from(KEY_TTL_SECONDS));
    w.u64(u64::from(header.ttl_seconds));
    w.u64(u64::from(KEY_HOP_LIMIT));
    w.u64(u64::from(header.hop_limit));
    w.u64(u64::from(KEY_PRIORITY));
    w.u64(u64::from(header.priority as u8));
    w.u64(u64::from(KEY_PAYLOAD_LENGTH));
    w.u64(u64::from(header.payload_length));
    w.u64(u64::from(KEY_ENCRYPTED_PAYLOAD));
    w.bytes(encrypted_payload);
}

fn read_id16(r: &mut CborReader<'_>) -> Result<[u8; 16], WireError> {
    let bytes = r.bytes()?;
    <[u8; 16]>::try_from(bytes).map_err(|_| WireError::InvalidIdentifier)
}

fn set_once<T>(slot: &mut Option<T>, value: T, key: u8) -> Result<(), WireError> {
    if slot.is_some() {
        return Err(WireError::DuplicateField(key));
    }
    *slot = Some(value);
    Ok(())
}

fn require<T>(slot: Option<T>, key: u8) -> Result<T, WireError> {
    slot.ok_or(WireError::MissingField(key))
}

fn u32_from(value: u64) -> Result<u32, WireError> {
    u32::try_from(value).map_err(|_| WireError::IntegerOutOfRange)
}

fn u16_from(value: u64) -> Result<u16, WireError> {
    u16::try_from(value).map_err(|_| WireError::IntegerOutOfRange)
}

fn bundle_type_from(value: u64) -> Result<BundleType, WireError> {
    match value {
        0x01 => Ok(BundleType::DirectMessage),
        0x02 => Ok(BundleType::DeliveryAck),
        0x04 => Ok(BundleType::Control),
        other => Err(WireError::InvalidBundleType(other)),
    }
}

fn priority_from(value: u64) -> Result<Priority, WireError> {
    match value {
        0 => Ok(Priority::Bulk),
        1 => Ok(Priority::Normal),
        2 => Ok(Priority::High),
        3 => Ok(Priority::Emergency),
        other => Err(WireError::InvalidPriority(other)),
    }
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
                bundle_id: BundleId::from_bytes([7; 16]),
                bundle_type: BundleType::DirectMessage,
                sender_id: UserId::from_bytes([1; 16]),
                destination_id: UserId::from_bytes([2; 16]),
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
            sender_signature: vec![0; SIGNATURE_LENGTH],
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
    fn signed_bytes_start_with_canonical_map11() {
        let bundle = sample_bundle();
        let signed = bundle.signed_bytes().unwrap();
        assert_eq!(signed[0], 0xab);
    }

    #[test]
    fn hop_count_is_outside_signed_bytes() {
        let mut bundle = sample_bundle();
        let before = bundle.signed_bytes().unwrap();
        bundle.relay.hop_count = 9;
        let after = bundle.signed_bytes().unwrap();
        assert_eq!(before, after);
        assert_ne!(bundle.encode_cbor().unwrap()[0], 0);
    }

    #[test]
    fn expiry_uses_local_first_seen_time() {
        let bundle = sample_bundle();
        assert!(!bundle.is_expired(59_000, 0));
        assert!(bundle.is_expired(60_000, 0));
    }

    #[test]
    fn numeric_keys_are_used() {
        let encoded = sample_bundle().encode_cbor().unwrap();
        assert_eq!(encoded[0], 0xae, "map(14) = 0xAE");
        assert_eq!(encoded[1], 0x01, "key 1");
        assert_eq!(encoded[2], 0x19, "u16 follows for version 0x0100");
        assert_eq!(&encoded[3..5], &[0x01, 0x00]);
    }
}
