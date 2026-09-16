PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS contacts (
    user_id BLOB PRIMARY KEY CHECK(length(user_id) = 16),
    display_name TEXT NOT NULL,
    signing_public_key BLOB NOT NULL CHECK(length(signing_public_key) = 32),
    encryption_public_key BLOB NOT NULL CHECK(length(encryption_public_key) = 32),
    trust_state INTEGER NOT NULL,
    fingerprint TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    conversation_id BLOB PRIMARY KEY CHECK(length(conversation_id) = 16),
    remote_user_id BLOB NOT NULL CHECK(length(remote_user_id) = 16),
    created_at_ms INTEGER NOT NULL,
    last_message_at_ms INTEGER
);

CREATE TABLE IF NOT EXISTS messages (
    message_id BLOB PRIMARY KEY CHECK(length(message_id) = 16),
    conversation_id BLOB NOT NULL CHECK(length(conversation_id) = 16),
    sender_id BLOB NOT NULL CHECK(length(sender_id) = 16),
    recipient_id BLOB NOT NULL CHECK(length(recipient_id) = 16),
    direction INTEGER NOT NULL,
    sent_at_ms INTEGER,
    received_at_ms INTEGER,
    state INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    content_nonce BLOB NOT NULL,
    content_ciphertext BLOB NOT NULL,
    original_bundle_id BLOB,
    FOREIGN KEY(conversation_id) REFERENCES conversations(conversation_id)
);

CREATE INDEX IF NOT EXISTS idx_messages_conversation
ON messages(conversation_id, sent_at_ms);

CREATE TABLE IF NOT EXISTS delivered_message_ids (
    message_id BLOB PRIMARY KEY CHECK(length(message_id) = 16),
    delivered_at_ms INTEGER NOT NULL
);
