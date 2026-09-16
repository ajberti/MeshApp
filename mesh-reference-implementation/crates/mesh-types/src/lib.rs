use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name([u8; 16]);

        impl $name {
            pub fn random() -> Self {
                Self(*Uuid::new_v4().as_bytes())
            }

            pub const fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), Uuid::from_bytes(self.0))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", Uuid::from_bytes(self.0))
            }
        }
    };
}

id_type!(UserId);
id_type!(NodeId);
id_type!(BundleId);
id_type!(MessageId);
id_type!(ConversationId);
id_type!(DiscoveryId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LinkId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BundleType {
    DirectMessage = 0x01,
    DeliveryAck = 0x02,
    Control = 0x04,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    Bulk = 0,
    #[default]
    Normal = 1,
    High = 2,
    Emergency = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageState {
    Queued,
    Relayed,
    Delivered,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct_and_16_bytes() {
        let bundle = BundleId::random();
        let message = MessageId::random();
        assert_eq!(bundle.as_bytes().len(), 16);
        assert_eq!(message.as_bytes().len(), 16);
        assert_ne!(bundle.as_bytes(), message.as_bytes());
    }
}
