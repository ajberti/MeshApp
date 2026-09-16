use crate::{sha256, CryptoError};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use mesh_types::{ConversationId, UserId};
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use std::fmt;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicIdentity {
    pub user_id: UserId,
    pub signing_public: [u8; 32],
    pub encryption_public: [u8; 32],
}

impl PublicIdentity {
    pub fn new(signing_public: [u8; 32], encryption_public: [u8; 32]) -> Result<Self, CryptoError> {
        VerifyingKey::from_bytes(&signing_public).map_err(|_| CryptoError::InvalidPublicKey)?;
        Ok(Self {
            user_id: derive_user_id(&signing_public),
            signing_public,
            encryption_public,
        })
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.signing_public, &self.encryption_public)
    }
}

pub struct Identity {
    user_id: UserId,
    signing: SigningKey,
    encryption: StaticSecret,
    signing_public: [u8; 32],
    encryption_public: [u8; 32],
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("user_id", &self.user_id)
            .finish_non_exhaustive()
    }
}

impl Identity {
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let signing = SigningKey::generate(rng);
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        let encryption = StaticSecret::from(seed);
        seed.zeroize();
        Self::from_keys(signing, encryption)
    }

    pub fn from_seeds(signing_seed: [u8; 32], encryption_seed: [u8; 32]) -> Self {
        Self::from_keys(
            SigningKey::from_bytes(&signing_seed),
            StaticSecret::from(encryption_seed),
        )
    }

    fn from_keys(signing: SigningKey, encryption: StaticSecret) -> Self {
        let signing_public = signing.verifying_key().to_bytes();
        let encryption_public = X25519Public::from(&encryption).to_bytes();
        Self {
            user_id: derive_user_id(&signing_public),
            signing,
            encryption,
            signing_public,
            encryption_public,
        }
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn signing_public(&self) -> &[u8; 32] {
        &self.signing_public
    }

    pub fn encryption_public(&self) -> &[u8; 32] {
        &self.encryption_public
    }

    pub fn public_identity(&self) -> PublicIdentity {
        PublicIdentity {
            user_id: self.user_id,
            signing_public: self.signing_public,
            encryption_public: self.encryption_public,
        }
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        fingerprint(&self.signing_public, &self.encryption_public)
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing.sign(message).to_bytes()
    }

    pub(crate) fn encryption_secret(&self) -> &StaticSecret {
        &self.encryption
    }
}

pub fn derive_user_id(signing_public_key: &[u8]) -> UserId {
    let digest = sha256(signing_public_key);
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    UserId::from_bytes(id)
}

pub fn fingerprint(signing_public: &[u8; 32], encryption_public: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(signing_public);
    hasher.update(encryption_public);
    hasher.finalize().into()
}

pub fn conversation_id(a: UserId, b: UserId) -> ConversationId {
    let (min, max) = if a.as_bytes() <= b.as_bytes() {
        (a, b)
    } else {
        (b, a)
    };
    let mut data = [0u8; 32];
    data[..16].copy_from_slice(min.as_bytes());
    data[16..].copy_from_slice(max.as_bytes());
    let digest = sha256(&data);
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    ConversationId::from_bytes(id)
}

pub fn verify_signature(
    signing_public: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), CryptoError> {
    let verifying =
        VerifyingKey::from_bytes(signing_public).map_err(|_| CryptoError::InvalidPublicKey)?;
    let signature = Signature::from_bytes(signature);
    verifying
        .verify_strict(message, &signature)
        .map_err(|_| CryptoError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_omits_secret_field_names() {
        let id = Identity::from_seeds([1; 32], [2; 32]);
        let rendered = format!("{id:?}");
        assert!(rendered.contains("Identity"));
        assert!(!rendered.contains("signing"));
        assert!(!rendered.contains("encryption"));
    }

    #[test]
    fn conversation_id_is_order_independent() {
        let a = UserId::from_bytes([1; 16]);
        let b = UserId::from_bytes([2; 16]);
        assert_eq!(conversation_id(a, b), conversation_id(b, a));
        assert_ne!(conversation_id(a, b), conversation_id(a, a));
    }
}
