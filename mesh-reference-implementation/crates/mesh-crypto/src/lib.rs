//! Cryptographic boundary for Mesh Protocol.
//!
//! The first repository milestone defines the boundary but intentionally does
//! not yet implement identity/session/message cryptography. Real algorithms
//! are introduced in the next milestone so they can be reviewed independently.

use mesh_types::UserId;
use sha2::{Digest, Sha256};

pub fn derive_user_id(signing_public_key: &[u8]) -> UserId {
    let digest = Sha256::digest(signing_public_key);
    let mut id = [0_u8; 16];
    id.copy_from_slice(&digest[..16]);
    UserId::from_bytes(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_derivation_is_stable() {
        let a = derive_user_id(b"test-public-key");
        let b = derive_user_id(b"test-public-key");
        assert_eq!(a, b);
    }
}
