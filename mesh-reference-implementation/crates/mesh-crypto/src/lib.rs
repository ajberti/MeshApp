use sha2::{Digest, Sha256};
use thiserror::Error;

mod identity;
mod message;

#[cfg(test)]
mod vectors;

pub use identity::{
    conversation_id, derive_user_id, fingerprint, verify_signature, Identity, PublicIdentity,
};
pub use message::{
    open_message, seal_message, seal_message_with, SealedMessage, MESSAGE_KDF_INFO,
    MESSAGE_KDF_SALT,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("invalid signature")]
    InvalidSignature,
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("key derivation failed")]
    KeyDerivation,
    #[error("encryption failed")]
    EncryptionFailed,
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mesh_types::UserId;

    #[test]
    fn user_id_derivation_is_stable() {
        let a = derive_user_id(b"test-public-key");
        let b = derive_user_id(b"test-public-key");
        assert_eq!(a, b);
        assert_ne!(a, UserId::from_bytes([0; 16]));
    }
}
