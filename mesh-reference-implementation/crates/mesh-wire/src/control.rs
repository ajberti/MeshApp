use crate::cbor::{CborReader, CborWriter};
use crate::error::WireError;
use mesh_types::{BundleId, BundleType, DiscoveryId, Priority, UserId};

pub const CAPABILITY_TEXT: u32 = 0x01;
pub const ERROR_UNSUPPORTED_VERSION: u16 = 0x0001;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelloPayload {
    pub protocol_major: u8,
    pub protocol_minor: u8,
    pub discovery_id: DiscoveryId,
    pub session_nonce: [u8; 32],
    pub capabilities: u32,
    pub max_frame_size: u32,
}

impl HelloPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(6);
        w.u64(1);
        w.u64(u64::from(self.protocol_major));
        w.u64(2);
        w.u64(u64::from(self.protocol_minor));
        w.u64(3);
        w.bytes(self.discovery_id.as_bytes());
        w.u64(4);
        w.bytes(&self.session_nonce);
        w.u64(5);
        w.u64(u64::from(self.capabilities));
        w.u64(6);
        w.u64(u64::from(self.max_frame_size));
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut major = None;
        let mut minor = None;
        let mut discovery = None;
        let mut nonce = None;
        let mut capabilities = None;
        let mut max_frame = None;
        for _ in 0..n {
            match r.u64()? {
                1 => major = Some(u8_from(r.u64()?)?),
                2 => minor = Some(u8_from(r.u64()?)?),
                3 => discovery = Some(id16(r.bytes()?)?),
                4 => nonce = Some(bytes32(r.bytes()?)?),
                5 => capabilities = Some(u32_from(r.u64()?)?),
                6 => max_frame = Some(u32_from(r.u64()?)?),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            protocol_major: major.ok_or(WireError::MissingField(1))?,
            protocol_minor: minor.ok_or(WireError::MissingField(2))?,
            discovery_id: DiscoveryId::from_bytes(discovery.ok_or(WireError::MissingField(3))?),
            session_nonce: nonce.ok_or(WireError::MissingField(4))?,
            capabilities: capabilities.ok_or(WireError::MissingField(5))?,
            max_frame_size: max_frame.ok_or(WireError::MissingField(6))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPayload {
    pub ephemeral_public: [u8; 32],
}

impl KeyPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(1);
        w.u64(1);
        w.bytes(&self.ephemeral_public);
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut ephemeral_public = None;
        for _ in 0..n {
            match r.u64()? {
                1 => ephemeral_public = Some(bytes32(r.bytes()?)?),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            ephemeral_public: ephemeral_public.ok_or(WireError::MissingField(1))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventorySummary {
    pub bundle_ids: Vec<BundleId>,
}

impl InventorySummary {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(1);
        w.u64(1);
        w.array(self.bundle_ids.len() as u64);
        for id in &self.bundle_ids {
            w.bytes(id.as_bytes());
        }
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut bundle_ids = None;
        for _ in 0..n {
            match r.u64()? {
                1 => {
                    let count = r.array()?;
                    let mut ids = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        ids.push(BundleId::from_bytes(id16(r.bytes()?)?));
                    }
                    bundle_ids = Some(ids);
                }
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            bundle_ids: bundle_ids.ok_or(WireError::MissingField(1))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleOffer {
    pub bundle_id: BundleId,
    pub bundle_type: BundleType,
    pub destination_id: UserId,
    pub size: u32,
    pub priority: Priority,
    pub hop_count: u16,
}

impl BundleOffer {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(6);
        w.u64(1);
        w.bytes(self.bundle_id.as_bytes());
        w.u64(2);
        w.u64(u64::from(self.bundle_type as u8));
        w.u64(3);
        w.bytes(self.destination_id.as_bytes());
        w.u64(4);
        w.u64(u64::from(self.size));
        w.u64(5);
        w.u64(u64::from(self.priority as u8));
        w.u64(6);
        w.u64(u64::from(self.hop_count));
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut bundle_id = None;
        let mut bundle_type = None;
        let mut destination_id = None;
        let mut size = None;
        let mut priority = None;
        let mut hop_count = None;
        for _ in 0..n {
            match r.u64()? {
                1 => bundle_id = Some(id16(r.bytes()?)?),
                2 => bundle_type = Some(r.u64()?),
                3 => destination_id = Some(id16(r.bytes()?)?),
                4 => size = Some(u32_from(r.u64()?)?),
                5 => priority = Some(r.u64()?),
                6 => hop_count = Some(u16_from(r.u64()?)?),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            bundle_id: BundleId::from_bytes(bundle_id.ok_or(WireError::MissingField(1))?),
            bundle_type: match bundle_type.ok_or(WireError::MissingField(2))? {
                0x01 => BundleType::DirectMessage,
                0x02 => BundleType::DeliveryAck,
                0x04 => BundleType::Control,
                other => return Err(WireError::InvalidBundleType(other)),
            },
            destination_id: UserId::from_bytes(destination_id.ok_or(WireError::MissingField(3))?),
            size: size.ok_or(WireError::MissingField(4))?,
            priority: match priority.ok_or(WireError::MissingField(5))? {
                0 => Priority::Bulk,
                1 => Priority::Normal,
                2 => Priority::High,
                3 => Priority::Emergency,
                other => return Err(WireError::InvalidPriority(other)),
            },
            hop_count: hop_count.ok_or(WireError::MissingField(6))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleIdPayload {
    pub bundle_id: BundleId,
}

impl BundleIdPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(1);
        w.u64(1);
        w.bytes(self.bundle_id.as_bytes());
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut bundle_id = None;
        for _ in 0..n {
            match r.u64()? {
                1 => bundle_id = Some(id16(r.bytes()?)?),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            bundle_id: BundleId::from_bytes(bundle_id.ok_or(WireError::MissingField(1))?),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleData {
    pub bundle_id: BundleId,
    pub offset: u32,
    pub data: Vec<u8>,
}

impl BundleData {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(3);
        w.u64(1);
        w.bytes(self.bundle_id.as_bytes());
        w.u64(2);
        w.u64(u64::from(self.offset));
        w.u64(3);
        w.bytes(&self.data);
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut bundle_id = None;
        let mut offset = None;
        let mut data = None;
        for _ in 0..n {
            match r.u64()? {
                1 => bundle_id = Some(id16(r.bytes()?)?),
                2 => offset = Some(u32_from(r.u64()?)?),
                3 => data = Some(r.bytes()?.to_vec()),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            bundle_id: BundleId::from_bytes(bundle_id.ok_or(WireError::MissingField(1))?),
            offset: offset.ok_or(WireError::MissingField(2))?,
            data: data.ok_or(WireError::MissingField(3))?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorPayload {
    pub code: u16,
}

impl ErrorPayload {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(1);
        w.u64(1);
        w.u64(u64::from(self.code));
        w.into_inner()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut code = None;
        for _ in 0..n {
            match r.u64()? {
                1 => code = Some(u16_from(r.u64()?)?),
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            code: code.ok_or(WireError::MissingField(1))?,
        })
    }
}

fn u8_from(value: u64) -> Result<u8, WireError> {
    u8::try_from(value).map_err(|_| WireError::IntegerOutOfRange)
}

fn u16_from(value: u64) -> Result<u16, WireError> {
    u16::try_from(value).map_err(|_| WireError::IntegerOutOfRange)
}

fn u32_from(value: u64) -> Result<u32, WireError> {
    u32::try_from(value).map_err(|_| WireError::IntegerOutOfRange)
}

fn id16(bytes: &[u8]) -> Result<[u8; 16], WireError> {
    <[u8; 16]>::try_from(bytes).map_err(|_| WireError::InvalidIdentifier)
}

fn bytes32(bytes: &[u8]) -> Result<[u8; 32], WireError> {
    <[u8; 32]>::try_from(bytes).map_err(|_| WireError::InvalidControlPayload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_round_trip() {
        let hello = HelloPayload {
            protocol_major: 1,
            protocol_minor: 0,
            discovery_id: DiscoveryId::from_bytes([3; 16]),
            session_nonce: [9; 32],
            capabilities: CAPABILITY_TEXT,
            max_frame_size: 65536,
        };
        assert_eq!(HelloPayload::decode(&hello.encode()).unwrap(), hello);
    }

    #[test]
    fn inventory_round_trip() {
        let summary = InventorySummary {
            bundle_ids: vec![BundleId::from_bytes([1; 16]), BundleId::from_bytes([2; 16])],
        };
        assert_eq!(
            InventorySummary::decode(&summary.encode()).unwrap(),
            summary
        );
    }
}
