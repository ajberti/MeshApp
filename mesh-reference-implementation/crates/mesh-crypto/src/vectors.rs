use crate::identity::{verify_signature, Identity};
use crate::message::{open_message, seal_message_with};
use serde::Serialize;

const ALICE_SIGNING: [u8; 32] = seed(0x00);
const ALICE_ENCRYPTION: [u8; 32] = seed(0x20);
const BOB_SIGNING: [u8; 32] = seed(0x40);
const BOB_ENCRYPTION: [u8; 32] = seed(0x60);
const CHARLIE_SIGNING: [u8; 32] = seed(0xc0);
const CHARLIE_ENCRYPTION: [u8; 32] = seed(0xe0);
const EPHEMERAL: [u8; 32] = seed(0x80);
const NONCE: [u8; 12] = [
    0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab,
];
const PLAINTEXT: &[u8] = b"Are you safe?";

const fn seed(start: u8) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        bytes[i] = start.wrapping_add(i as u8);
        i += 1;
    }
    bytes
}

#[derive(Serialize)]
struct IdentityVector {
    signing_seed: String,
    encryption_seed: String,
    signing_public: String,
    encryption_public: String,
    user_id: String,
    fingerprint: String,
}

#[derive(Serialize)]
struct MessageVector {
    plaintext_hex: String,
    ephemeral_seed: String,
    nonce: String,
    ephemeral_public: String,
    ciphertext: String,
}

fn identity_vector(signing: [u8; 32], encryption: [u8; 32]) -> (Identity, IdentityVector) {
    let identity = Identity::from_seeds(signing, encryption);
    let vector = IdentityVector {
        signing_seed: hex::encode(signing),
        encryption_seed: hex::encode(encryption),
        signing_public: hex::encode(identity.signing_public()),
        encryption_public: hex::encode(identity.encryption_public()),
        user_id: hex::encode(identity.user_id().as_bytes()),
        fingerprint: hex::encode(identity.fingerprint()),
    };
    (identity, vector)
}

#[test]
#[ignore]
fn dump_vectors() {
    let (alice, alice_vec) = identity_vector(ALICE_SIGNING, ALICE_ENCRYPTION);
    let (bob, bob_vec) = identity_vector(BOB_SIGNING, BOB_ENCRYPTION);
    let sealed = seal_message_with(bob.encryption_public(), PLAINTEXT, EPHEMERAL, NONCE).unwrap();
    let message_vec = MessageVector {
        plaintext_hex: hex::encode(PLAINTEXT),
        ephemeral_seed: hex::encode(EPHEMERAL),
        nonce: hex::encode(NONCE),
        ephemeral_public: hex::encode(sealed.ephemeral_public),
        ciphertext: hex::encode(&sealed.ciphertext),
    };
    let signature = alice.sign(b"mesh-signed-bytes-v1");
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "identities": [alice_vec, bob_vec],
            "message": message_vec,
            "signature": {
                "message": hex::encode(b"mesh-signed-bytes-v1"),
                "signature": hex::encode(signature),
            }
        }))
        .unwrap()
    );
}

#[test]
fn committed_crypto_vectors_match() {
    let (alice, alice_vec) = identity_vector(ALICE_SIGNING, ALICE_ENCRYPTION);
    let (bob, bob_vec) = identity_vector(BOB_SIGNING, BOB_ENCRYPTION);
    let (charlie, _) = identity_vector(CHARLIE_SIGNING, CHARLIE_ENCRYPTION);

    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../test-vectors/identity.json")).unwrap();
    assert_eq!(
        serde_json::to_value([&alice_vec, &bob_vec]).unwrap(),
        expected["identities"]
    );

    let sealed = seal_message_with(bob.encryption_public(), PLAINTEXT, EPHEMERAL, NONCE).unwrap();
    let message_vec = MessageVector {
        plaintext_hex: hex::encode(PLAINTEXT),
        ephemeral_seed: hex::encode(EPHEMERAL),
        nonce: hex::encode(NONCE),
        ephemeral_public: hex::encode(sealed.ephemeral_public),
        ciphertext: hex::encode(&sealed.ciphertext),
    };
    let expected_message: serde_json::Value = serde_json::from_str(include_str!(
        "../../../test-vectors/message-encryption.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(message_vec).unwrap(), expected_message);
    assert_eq!(open_message(&bob, &sealed).unwrap(), PLAINTEXT);
    assert!(open_message(&alice, &sealed).is_err());
    assert!(open_message(&charlie, &sealed).is_err());

    let signed = b"mesh-signed-bytes-v1";
    let signature = alice.sign(signed);
    let expected_sig: serde_json::Value =
        serde_json::from_str(include_str!("../../../test-vectors/signature.json")).unwrap();
    assert_eq!(hex::encode(signature), expected_sig["signature"]);
    assert_eq!(hex::encode(signed), expected_sig["message"]);
    verify_signature(alice.signing_public(), signed, &signature).unwrap();
    let mut tampered = signature;
    tampered[0] ^= 1;
    assert!(verify_signature(alice.signing_public(), signed, &tampered).is_err());
}
