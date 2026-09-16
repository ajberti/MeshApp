use crate::cbor::{CborReader, CborWriter};
use crate::error::WireError;
use mesh_types::{ConversationId, MessageId, UserId};

pub const DIRECT_MESSAGE_VERSION: u16 = 1;
pub const KEY_VERSION: u8 = 1;
pub const KEY_MESSAGE_ID: u8 = 2;
pub const KEY_CONVERSATION_ID: u8 = 3;
pub const KEY_SENDER_ID: u8 = 4;
pub const KEY_RECIPIENT_ID: u8 = 5;
pub const KEY_SEQUENCE: u8 = 6;
pub const KEY_SENT_AT: u8 = 7;
pub const KEY_CONTENT_TYPE: u8 = 8;
pub const KEY_CONTENT: u8 = 9;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectMessagePayload {
    pub version: u16,
    pub message_id: MessageId,
    pub conversation_id: ConversationId,
    pub sender_id: UserId,
    pub recipient_id: UserId,
    pub sequence: u64,
    pub sent_at_ms: i64,
    pub content_type: String,
    pub content: Vec<u8>,
}

impl DirectMessagePayload {
    pub fn encode_cbor(&self) -> Vec<u8> {
        let mut w = CborWriter::new();
        w.map(9);
        w.u64(u64::from(KEY_VERSION));
        w.u64(u64::from(self.version));
        w.u64(u64::from(KEY_MESSAGE_ID));
        w.bytes(self.message_id.as_bytes());
        w.u64(u64::from(KEY_CONVERSATION_ID));
        w.bytes(self.conversation_id.as_bytes());
        w.u64(u64::from(KEY_SENDER_ID));
        w.bytes(self.sender_id.as_bytes());
        w.u64(u64::from(KEY_RECIPIENT_ID));
        w.bytes(self.recipient_id.as_bytes());
        w.u64(u64::from(KEY_SEQUENCE));
        w.u64(self.sequence);
        w.u64(u64::from(KEY_SENT_AT));
        w.i64(self.sent_at_ms);
        w.u64(u64::from(KEY_CONTENT_TYPE));
        w.text(&self.content_type);
        w.u64(u64::from(KEY_CONTENT));
        w.bytes(&self.content);
        w.into_inner()
    }

    pub fn decode_cbor(bytes: &[u8]) -> Result<Self, WireError> {
        let mut r = CborReader::new(bytes);
        let n = r.map()?;
        let mut version = None;
        let mut message_id = None;
        let mut conversation_id = None;
        let mut sender_id = None;
        let mut recipient_id = None;
        let mut sequence = None;
        let mut sent_at_ms = None;
        let mut content_type = None;
        let mut content = None;

        for _ in 0..n {
            let key = r.u64()?;
            match key {
                1 => assign(&mut version, r.u64()?, KEY_VERSION)?,
                2 => assign(&mut message_id, read_id(&mut r)?, KEY_MESSAGE_ID)?,
                3 => assign(&mut conversation_id, read_id(&mut r)?, KEY_CONVERSATION_ID)?,
                4 => assign(&mut sender_id, read_id(&mut r)?, KEY_SENDER_ID)?,
                5 => assign(&mut recipient_id, read_id(&mut r)?, KEY_RECIPIENT_ID)?,
                6 => assign(&mut sequence, r.u64()?, KEY_SEQUENCE)?,
                7 => assign(&mut sent_at_ms, r.i64()?, KEY_SENT_AT)?,
                8 => assign(&mut content_type, r.text()?.to_owned(), KEY_CONTENT_TYPE)?,
                9 => assign(&mut content, r.bytes()?.to_vec(), KEY_CONTENT)?,
                _ => r.skip_value()?,
            }
        }
        r.finish()?;

        Ok(Self {
            version: u16::try_from(version.ok_or(WireError::MissingField(KEY_VERSION))?)
                .map_err(|_| WireError::IntegerOutOfRange)?,
            message_id: MessageId::from_bytes(
                message_id.ok_or(WireError::MissingField(KEY_MESSAGE_ID))?,
            ),
            conversation_id: ConversationId::from_bytes(
                conversation_id.ok_or(WireError::MissingField(KEY_CONVERSATION_ID))?,
            ),
            sender_id: UserId::from_bytes(sender_id.ok_or(WireError::MissingField(KEY_SENDER_ID))?),
            recipient_id: UserId::from_bytes(
                recipient_id.ok_or(WireError::MissingField(KEY_RECIPIENT_ID))?,
            ),
            sequence: sequence.ok_or(WireError::MissingField(KEY_SEQUENCE))?,
            sent_at_ms: sent_at_ms.ok_or(WireError::MissingField(KEY_SENT_AT))?,
            content_type: content_type.ok_or(WireError::MissingField(KEY_CONTENT_TYPE))?,
            content: content.ok_or(WireError::MissingField(KEY_CONTENT))?,
        })
    }
}

fn assign<T>(slot: &mut Option<T>, value: T, key: u8) -> Result<(), WireError> {
    if slot.is_some() {
        return Err(WireError::DuplicateField(key));
    }
    *slot = Some(value);
    Ok(())
}

fn read_id(r: &mut CborReader<'_>) -> Result<[u8; 16], WireError> {
    <[u8; 16]>::try_from(r.bytes()?).map_err(|_| WireError::InvalidIdentifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_message_round_trip() {
        let payload = DirectMessagePayload {
            version: DIRECT_MESSAGE_VERSION,
            message_id: MessageId::from_bytes([1; 16]),
            conversation_id: ConversationId::from_bytes([2; 16]),
            sender_id: UserId::from_bytes([3; 16]),
            recipient_id: UserId::from_bytes([4; 16]),
            sequence: 7,
            sent_at_ms: 99,
            content_type: "text/plain".into(),
            content: b"Are you safe?".to_vec(),
        };
        let encoded = payload.encode_cbor();
        assert_eq!(encoded[0], 0xa9);
        assert_eq!(
            DirectMessagePayload::decode_cbor(&encoded).unwrap(),
            payload
        );
    }
}
