use crate::CryptoError;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::ChaCha20Poly1305;
use hkdf::Hkdf;
use rand_core::{CryptoRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const SESSION_KDF_SALT: &[u8] = b"MeshProtocol-1.0";
pub const SESSION_KDF_INFO_LO_HI: &[u8] = b"session-key-lo-hi-v1";
pub const SESSION_KDF_INFO_HI_LO: &[u8] = b"session-key-hi-lo-v1";

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct EphemeralSecret {
    secret: StaticSecret,
}

impl EphemeralSecret {
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);
        let secret = StaticSecret::from(seed);
        seed.zeroize();
        Self { secret }
    }

    pub fn public_bytes(&self) -> [u8; 32] {
        X25519Public::from(&self.secret).to_bytes()
    }

    pub fn shared_secret(&self, remote_public: &[u8; 32]) -> [u8; 32] {
        *self
            .secret
            .diffie_hellman(&X25519Public::from(*remote_public))
            .as_bytes()
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SessionKeys {
    pub tx: [u8; 32],
    pub rx: [u8; 32],
}

pub fn handshake_transcript(
    hello_lo: &[u8],
    hello_hi: &[u8],
    eph_lo: &[u8; 32],
    eph_hi: &[u8; 32],
) -> Vec<u8> {
    let mut transcript = Vec::with_capacity(hello_lo.len() + hello_hi.len() + 64);
    transcript.extend_from_slice(hello_lo);
    transcript.extend_from_slice(hello_hi);
    transcript.extend_from_slice(eph_lo);
    transcript.extend_from_slice(eph_hi);
    transcript
}

pub fn derive_session_keys(
    shared_secret: &[u8; 32],
    transcript: &[u8],
    local_is_lo: bool,
) -> Result<SessionKeys, CryptoError> {
    let lo_hi = expand_session_key(shared_secret, SESSION_KDF_INFO_LO_HI, transcript)?;
    let hi_lo = expand_session_key(shared_secret, SESSION_KDF_INFO_HI_LO, transcript)?;
    if local_is_lo {
        Ok(SessionKeys {
            tx: lo_hi,
            rx: hi_lo,
        })
    } else {
        Ok(SessionKeys {
            tx: hi_lo,
            rx: lo_hi,
        })
    }
}

pub fn session_nonce(counter: u64) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(&counter.to_be_bytes());
    nonce
}

pub fn seal_session(
    key: &[u8; 32],
    counter: u64,
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .encrypt(
            (&session_nonce(counter)).into(),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::EncryptionFailed)
}

pub fn open_session(
    key: &[u8; 32],
    counter: u64,
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            (&session_nonce(counter)).into(),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CryptoError::DecryptionFailed)
}

fn expand_session_key(
    shared_secret: &[u8; 32],
    info_prefix: &[u8],
    transcript: &[u8],
) -> Result<[u8; 32], CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(SESSION_KDF_SALT), shared_secret);
    let mut info = Vec::with_capacity(info_prefix.len() + transcript.len());
    info.extend_from_slice(info_prefix);
    info.extend_from_slice(transcript);
    let mut okm = [0u8; 32];
    hk.expand(&info, &mut okm)
        .map_err(|_| CryptoError::KeyDerivation)?;
    Ok(okm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_keys_match_and_round_trip() {
        let mut rng_a = [0x11u8; 32];
        let mut rng_b = [0x22u8; 32];
        let a = EphemeralSecret::generate(&mut FillRng(rng_a));
        let b = EphemeralSecret::generate(&mut FillRng(rng_b));
        rng_a.zeroize();
        rng_b.zeroize();
        let shared_a = a.shared_secret(&b.public_bytes());
        let shared_b = b.shared_secret(&a.public_bytes());
        assert_eq!(shared_a, shared_b);
        let transcript = handshake_transcript(
            b"hello-lo",
            b"hello-hi",
            &a.public_bytes(),
            &b.public_bytes(),
        );
        let keys_a = derive_session_keys(&shared_a, &transcript, true).unwrap();
        let keys_b = derive_session_keys(&shared_b, &transcript, false).unwrap();
        assert_eq!(keys_a.tx, keys_b.rx);
        assert_eq!(keys_a.rx, keys_b.tx);
        let ciphertext = seal_session(&keys_a.tx, 0, b"aad", b"frame").unwrap();
        assert_eq!(
            open_session(&keys_b.rx, 0, b"aad", &ciphertext).unwrap(),
            b"frame"
        );
        assert!(open_session(&keys_b.rx, 1, b"aad", &ciphertext).is_err());
    }

    struct FillRng([u8; 32]);

    impl RngCore for FillRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0u8; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0u8; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for (i, slot) in dest.iter_mut().enumerate() {
                *slot = self.0[i % 32];
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    impl CryptoRng for FillRng {}
}
