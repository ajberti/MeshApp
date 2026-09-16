use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("unsupported protocol major version {0}")]
    UnsupportedMajorVersion(u8),
    #[error("payload length does not match header")]
    PayloadLengthMismatch,
    #[error("bundle exceeds protocol size limit")]
    BundleTooLarge,
    #[error("hop limit must be greater than zero")]
    InvalidHopLimit,
    #[error("signature must be 64 bytes")]
    InvalidSignatureLength,
    #[error("identifier must be 16 bytes")]
    InvalidIdentifier,
    #[error("invalid bundle type {0}")]
    InvalidBundleType(u64),
    #[error("invalid priority {0}")]
    InvalidPriority(u64),
    #[error("missing required CBOR field {0}")]
    MissingField(u8),
    #[error("duplicate CBOR field {0}")]
    DuplicateField(u8),
    #[error("CBOR value is truncated")]
    TruncatedCbor,
    #[error("CBOR value has trailing bytes")]
    TrailingCbor,
    #[error("indefinite-length CBOR is not allowed")]
    IndefiniteCbor,
    #[error("unexpected CBOR type")]
    UnexpectedCborType,
    #[error("CBOR integer out of range")]
    IntegerOutOfRange,
    #[error("CBOR nesting or size limit exceeded")]
    CborLimit,
    #[error("invalid UTF-8")]
    InvalidUtf8,
    #[error("sealed payload field has the wrong length")]
    InvalidSealedPayload,
}
