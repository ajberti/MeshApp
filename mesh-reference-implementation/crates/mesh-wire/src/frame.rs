use crate::error::WireError;

pub const FRAME_MAGIC: u16 = 0x4D50;
pub const FRAME_HEADER_LEN: usize = 10;
pub const MAX_FRAME_PAYLOAD: usize = 128 * 1024;
pub const FLAG_ENCRYPTED: u8 = 0x01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Hello = 0x01,
    KeyInit = 0x02,
    KeyReply = 0x03,
    SessionOk = 0x04,
    InventorySummary = 0x10,
    InventoryRequest = 0x11,
    BundleOffer = 0x20,
    BundleRequest = 0x21,
    BundleData = 0x22,
    BundleComplete = 0x23,
    ErrorFrame = 0x7F,
}

impl TryFrom<u8> for FrameType {
    type Error = WireError;

    fn try_from(value: u8) -> Result<Self, WireError> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::KeyInit),
            0x03 => Ok(Self::KeyReply),
            0x04 => Ok(Self::SessionOk),
            0x10 => Ok(Self::InventorySummary),
            0x11 => Ok(Self::InventoryRequest),
            0x20 => Ok(Self::BundleOffer),
            0x21 => Ok(Self::BundleRequest),
            0x22 => Ok(Self::BundleData),
            0x23 => Ok(Self::BundleComplete),
            0x7F => Ok(Self::ErrorFrame),
            other => Err(WireError::UnknownFrameType(other)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub major: u8,
    pub minor: u8,
    pub ty: FrameType,
    pub flags: u8,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn cleartext(ty: FrameType, payload: Vec<u8>) -> Self {
        Self {
            major: crate::PROTOCOL_MAJOR,
            minor: crate::PROTOCOL_MINOR,
            ty,
            flags: 0,
            payload,
        }
    }

    pub fn encrypted(ty: FrameType, payload: Vec<u8>) -> Self {
        Self {
            major: crate::PROTOCOL_MAJOR,
            minor: crate::PROTOCOL_MINOR,
            ty,
            flags: FLAG_ENCRYPTED,
            payload,
        }
    }

    pub fn is_encrypted(&self) -> bool {
        self.flags & FLAG_ENCRYPTED != 0
    }

    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        if self.payload.len() > MAX_FRAME_PAYLOAD {
            return Err(WireError::FrameTooLarge);
        }
        let mut out = Vec::with_capacity(FRAME_HEADER_LEN + self.payload.len());
        out.extend_from_slice(&FRAME_MAGIC.to_be_bytes());
        out.push(self.major);
        out.push(self.minor);
        out.push(self.ty as u8);
        out.push(self.flags);
        out.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < FRAME_HEADER_LEN {
            return Err(WireError::TruncatedFrame);
        }
        let magic = u16::from_be_bytes([bytes[0], bytes[1]]);
        if magic != FRAME_MAGIC {
            return Err(WireError::InvalidMagic);
        }
        if bytes[2] != crate::PROTOCOL_MAJOR {
            return Err(WireError::UnsupportedMajorVersion(bytes[2]));
        }
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[6..10]);
        let len = u32::from_be_bytes(len_bytes) as usize;
        if len > MAX_FRAME_PAYLOAD {
            return Err(WireError::FrameTooLarge);
        }
        if bytes.len() != FRAME_HEADER_LEN + len {
            return Err(WireError::TruncatedFrame);
        }
        Ok(Self {
            major: bytes[2],
            minor: bytes[3],
            ty: FrameType::try_from(bytes[4])?,
            flags: bytes[5],
            payload: bytes[FRAME_HEADER_LEN..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let frame = Frame::cleartext(FrameType::Hello, b"hi".to_vec());
        let encoded = frame.encode().unwrap();
        assert_eq!(&encoded[..2], &[0x4D, 0x50]);
        assert_eq!(Frame::decode(&encoded).unwrap(), frame);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut encoded = Frame::cleartext(FrameType::Hello, vec![]).encode().unwrap();
        encoded[0] = 0;
        assert!(matches!(
            Frame::decode(&encoded),
            Err(WireError::InvalidMagic)
        ));
    }
}
