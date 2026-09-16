use crate::cbor::{CborReader, CborWriter};
use crate::error::WireError;

pub const KEY_EPHEMERAL_PUBLIC: u8 = 1;
pub const KEY_NONCE: u8 = 2;
pub const KEY_CIPHERTEXT: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedPayload {
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

impl SealedPayload {
    pub fn encode_cbor(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(3);
        w.u64(u64::from(KEY_EPHEMERAL_PUBLIC));
        w.bytes(&self.ephemeral_public);
        w.u64(u64::from(KEY_NONCE));
        w.bytes(&self.nonce);
        w.u64(u64::from(KEY_CIPHERTEXT));
        w.bytes(&self.ciphertext);
        w.into_inner()
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut ephemeral_public = None;
        let mut nonce = None;
        let mut ciphertext = None;
        for _ in 0..n {
            let key = r.u64()?;
            match key {
                1 => {
                    let value = r.bytes()?;
                    let arr =
                        <[u8; 32]>::try_from(value).map_err(|_| WireError::InvalidSealedPayload)?;
                    if ephemeral_public.replace(arr).is_some() {
                        return Err(WireError::DuplicateField(KEY_EPHEMERAL_PUBLIC));
                    }
                }
                2 => {
                    let value = r.bytes()?;
                    let arr =
                        <[u8; 12]>::try_from(value).map_err(|_| WireError::InvalidSealedPayload)?;
                    if nonce.replace(arr).is_some() {
                        return Err(WireError::DuplicateField(KEY_NONCE));
                    }
                }
                3 => {
                    if ciphertext.replace(r.bytes()?.to_vec()).is_some() {
                        return Err(WireError::DuplicateField(KEY_CIPHERTEXT));
                    }
                }
                _ => r.skip_value()?,
            }
        }
        r.finish()?;
        Ok(Self {
            ephemeral_public: ephemeral_public
                .ok_or(WireError::MissingField(KEY_EPHEMERAL_PUBLIC))?,
            nonce: nonce.ok_or(WireError::MissingField(KEY_NONCE))?,
            ciphertext: ciphertext.ok_or(WireError::MissingField(KEY_CIPHERTEXT))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_payload_round_trip() {
        let sealed = SealedPayload {
            ephemeral_public: [3; 32],
            nonce: [4; 12],
            ciphertext: vec![9, 8, 7],
        };
        let encoded = sealed.encode_cbor();
        assert_eq!(encoded[0], 0xa3);
        assert_eq!(SealedPayload::decode_cbor(&encoded).unwrap(), sealed);
    }
}
