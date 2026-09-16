use crate::identity::Identity;
use crate::CryptoError;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, XChaCha20Poly1305};
use hkdf::Hkdf;
use rand_core::{CryptoRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};
use zeroize::Zeroize;

pub const MESSAGE_KDF_SALT: &[u8] = b"MeshProtocol-1.0";
pub const MESSAGE_KDF_INFO: &[u8] = b"direct-message-key-v1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedMessage {
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

pub fn seal_message<R: CryptoRng + RngCore>(
    recipient_encryption_public: &[u8; 32],
    plaintext: &[u8],
    rng: &mut R,
) -> Result<SealedMessage, CryptoError> {
    let mut ephemeral_seed = [0u8; 32];
    let mut nonce = [0u8; 12];
    rng.fill_bytes(&mut ephemeral_seed);
    rng.fill_bytes(&mut nonce);
    let sealed = seal_message_with(
        recipient_encryption_public,
        plaintext,
        ephemeral_seed,
        nonce,
    );
    ephemeral_seed.zeroize();
    sealed
}

pub fn seal_message_with(
    recipient_encryption_public: &[u8; 32],
    plaintext: &[u8],
    ephemeral_seed: [u8; 32],
    nonce: [u8; 12],
) -> Result<SealedMessage, CryptoError> {
    let ephemeral = StaticSecret::from(ephemeral_seed);
    let ephemeral_public = X25519Public::from(&ephemeral);
    let recipient = X25519Public::from(*recipient_encryption_public);
    let shared_secret = ephemeral.diffie_hellman(&recipient);
    let mut shared = *shared_secret.as_bytes();
    let mut key_bytes = derive_message_key(&shared)?;
    shared.zeroize();
    let cipher = ChaCha20Poly1305::new((&key_bytes).into());
    let ciphertext = cipher
        .encrypt((&nonce).into(), plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)?;
    key_bytes.zeroize();
    Ok(SealedMessage {
        ephemeral_public: ephemeral_public.to_bytes(),
        nonce,
        ciphertext,
    })
}

pub fn open_message(recipient: &Identity, sealed: &SealedMessage) -> Result<Vec<u8>, CryptoError> {
    let ephemeral_public = X25519Public::from(sealed.ephemeral_public);
    let shared_secret = recipient
        .encryption_secret()
        .diffie_hellman(&ephemeral_public);
    let mut shared = *shared_secret.as_bytes();
    let mut key_bytes = derive_message_key(&shared)?;
    shared.zeroize();
    let cipher = ChaCha20Poly1305::new((&key_bytes).into());
    let plaintext = cipher
        .decrypt((&sealed.nonce).into(), sealed.ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionFailed)?;
    key_bytes.zeroize();
    Ok(plaintext)
}

pub fn seal_local(
    key: &[u8; 32],
    nonce: &[u8; 24],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(nonce.into(), plaintext)
        .map_err(|_| CryptoError::EncryptionFailed)
}

pub fn open_local(
    key: &[u8; 32],
    nonce: &[u8; 24],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(nonce.into(), ciphertext.as_ref())
        .map_err(|_| CryptoError::DecryptionFailed)
}

fn derive_message_key(shared_secret: &[u8; 32]) -> Result<[u8; 32], CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(MESSAGE_KDF_SALT), shared_secret);
    let mut okm = [0u8; 32];
    hk.expand(MESSAGE_KDF_INFO, &mut okm)
        .map_err(|_| CryptoError::KeyDerivation)?;
    Ok(okm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    #[test]
    fn round_trip_and_wrong_recipient() {
        let alice = Identity::from_seeds([0x11; 32], [0x21; 32]);
        let bob = Identity::from_seeds([0x12; 32], [0x22; 32]);
        let charlie = Identity::from_seeds([0x13; 32], [0x23; 32]);
        let sealed = seal_message_with(
            bob.encryption_public(),
            b"Are you safe?",
            [0x80; 32],
            [0xa0; 12],
        )
        .unwrap();
        assert_eq!(open_message(&bob, &sealed).unwrap(), b"Are you safe?");
        assert_eq!(
            open_message(&alice, &sealed),
            Err(CryptoError::DecryptionFailed)
        );
        assert_eq!(
            open_message(&charlie, &sealed),
            Err(CryptoError::DecryptionFailed)
        );
    }
}
