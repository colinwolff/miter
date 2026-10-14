//! Binary encoding of [`miter_ast::FunctionSymbol`]s for embedded persistence.
//!
//! The layout is versioned, little-endian, and self-describing enough to reject
//! foreign blobs; decoding never panics on truncated or corrupted input.

/// Magic prefix of every encoded record: `MITER`.
pub const MAGIC: [u8; 5] = *b"MITER";

/// Encoding version understood by this build.
pub const VERSION: u8 = 3;

/// Failures while encoding/decoding a record or talking to a store.
#[derive(Debug)]
pub enum PersistError {
    /// The embedded store or file failed.
    Io(std::io::Error),
    /// A record could not be decoded.
    Decode(DecodeError),
    /// `redb` reported an error.
    Redb(String),
}

impl core::fmt::Display for PersistError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PersistError::Io(error) => write!(formatter, "persistence i/o error: {error}"),
            PersistError::Decode(error) => write!(formatter, "record decode error: {error}"),
            PersistError::Redb(message) => write!(formatter, "redb error: {message}"),
        }
    }
}

impl std::error::Error for PersistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PersistError::Io(error) => Some(error),
            PersistError::Decode(error) => Some(error),
            PersistError::Redb(_) => None,
        }
    }
}

impl From<std::io::Error> for PersistError {
    fn from(error: std::io::Error) -> Self {
        PersistError::Io(error)
    }
}

impl From<DecodeError> for PersistError {
    fn from(error: DecodeError) -> Self {
        PersistError::Decode(error)
    }
}

/// A malformed record, reported with the byte offset that failed.
#[derive(Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// What went wrong.
    pub message: String,
    /// Offset in the record where decoding stopped.
    pub offset: usize,
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for DecodeError {}

mod symbol;
mod universal;
mod wire;

pub use symbol::{decode_symbol, encode_symbol};
pub use universal::{
    UNIVERSAL_VERSION, decode_universal, encode_universal, universal_kind_from_code,
};

#[cfg(test)]
mod tests;
