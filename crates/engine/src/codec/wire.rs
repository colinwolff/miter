//! The little-endian record primitives: a byte writer and a bounds-checked reader.
//!
//! Nothing here knows what a record *means*; the two codecs above build their layouts out of
//! these calls. The reader is total: every accessor returns a [`DecodeError`] rather than
//! panicking, so a truncated or corrupted record is a value and never an abort.

use super::DecodeError;

/// Little-endian record writer.
pub(super) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(super) fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    pub(super) fn text(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes(value.as_bytes());
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Bounds-checked little-endian reader; every accessor returns `Result`.
pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn fail(&self, message: &str) -> DecodeError {
        DecodeError {
            message: message.to_string(),
            offset: self.offset,
        }
    }

    pub(super) fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.fail("length overflow"))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| self.fail("truncated record"))?;
        self.offset = end;
        Ok(slice)
    }

    pub(super) fn bytes(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        self.take(count)
    }

    pub(super) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u32(&mut self) -> Result<u32, DecodeError> {
        let raw = self.take(4)?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    pub(super) fn u64(&mut self) -> Result<u64, DecodeError> {
        let raw = self.take(8)?;
        let mut buffer = [0u8; 8];
        buffer.copy_from_slice(raw);
        Ok(u64::from_le_bytes(buffer))
    }

    pub(super) fn text(&mut self) -> Result<String, DecodeError> {
        let length = self.u32()? as usize;
        let raw = self.take(length)?;
        core::str::from_utf8(raw)
            .map(str::to_owned)
            .map_err(|_| self.fail("invalid utf-8"))
    }
}
