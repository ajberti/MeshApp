//! Canonical CBOR subset used by Mesh Protocol 1.0.
//!
//! Only definite-length unsigned integers, negative integers, byte strings,
//! UTF-8 strings and maps are encoded. Map keys are always unsigned integers
//! written in sorted numeric order so the encoding is RFC 8949 canonical.

use crate::error::WireError;

const MAJOR_UNSIGNED: u8 = 0;
const MAJOR_NEGATIVE: u8 = 1;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_ARRAY: u8 = 4;
const MAJOR_MAP: u8 = 5;
const MAJOR_TAG: u8 = 6;
const MAJOR_SIMPLE: u8 = 7;
const ADDITIONAL_INDEFINITE: u8 = 31;
const MAX_SKIP_DEPTH: u8 = 8;
const MAX_MAP_ENTRIES: u64 = 64;
const MAX_VALUE_BYTES: usize = 64 * 1024 + 4096;

pub struct CborWriter {
    buf: Vec<u8>,
}

impl CborWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    pub fn map(&mut self, len: u64) {
        self.write_header(MAJOR_MAP, len);
    }

    pub fn u64(&mut self, value: u64) {
        self.write_header(MAJOR_UNSIGNED, value);
    }

    pub fn i64(&mut self, value: i64) {
        if value >= 0 {
            self.u64(value as u64);
        } else {
            let n = (i128::from(value).unsigned_abs()) - 1;
            self.write_header(MAJOR_NEGATIVE, n as u64);
        }
    }

    pub fn bytes(&mut self, value: &[u8]) {
        self.write_header(MAJOR_BYTES, value.len() as u64);
        self.buf.extend_from_slice(value);
    }

    pub fn text(&mut self, value: &str) {
        self.write_header(MAJOR_TEXT, value.len() as u64);
        self.buf.extend_from_slice(value.as_bytes());
    }

    fn write_header(&mut self, major: u8, value: u64) {
        let mt = major << 5;
        if value < 24 {
            self.buf.push(mt | value as u8);
        } else if value <= u64::from(u8::MAX) {
            self.buf.push(mt | 24);
            self.buf.push(value as u8);
        } else if value <= u64::from(u16::MAX) {
            self.buf.push(mt | 25);
            self.buf.extend_from_slice(&(value as u16).to_be_bytes());
        } else if value <= u64::from(u32::MAX) {
            self.buf.push(mt | 26);
            self.buf.extend_from_slice(&(value as u32).to_be_bytes());
        } else {
            self.buf.push(mt | 27);
            self.buf.extend_from_slice(&value.to_be_bytes());
        }
    }
}

pub struct CborReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> CborReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn finish(self) -> Result<(), WireError> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(WireError::TrailingCbor)
        }
    }

    pub fn map(&mut self) -> Result<u64, WireError> {
        let (major, value) = self.read_header()?;
        if major != MAJOR_MAP {
            return Err(WireError::UnexpectedCborType);
        }
        if value > MAX_MAP_ENTRIES {
            return Err(WireError::CborLimit);
        }
        Ok(value)
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        let (major, value) = self.read_header()?;
        if major != MAJOR_UNSIGNED {
            return Err(WireError::UnexpectedCborType);
        }
        Ok(value)
    }

    pub fn i64(&mut self) -> Result<i64, WireError> {
        let (major, value) = self.read_header()?;
        match major {
            MAJOR_UNSIGNED => i64::try_from(value).map_err(|_| WireError::IntegerOutOfRange),
            MAJOR_NEGATIVE => {
                let n = i128::from(value) + 1;
                i64::try_from(-n).map_err(|_| WireError::IntegerOutOfRange)
            }
            _ => Err(WireError::UnexpectedCborType),
        }
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], WireError> {
        let (major, value) = self.read_header()?;
        if major != MAJOR_BYTES {
            return Err(WireError::UnexpectedCborType);
        }
        self.read_exact(value as usize)
    }

    pub fn text(&mut self) -> Result<&'a str, WireError> {
        let (major, value) = self.read_header()?;
        if major != MAJOR_TEXT {
            return Err(WireError::UnexpectedCborType);
        }
        let bytes = self.read_exact(value as usize)?;
        std::str::from_utf8(bytes).map_err(|_| WireError::InvalidUtf8)
    }

    pub fn skip_value(&mut self) -> Result<(), WireError> {
        self.skip_value_at_depth(0)
    }

    fn skip_value_at_depth(&mut self, depth: u8) -> Result<(), WireError> {
        if depth > MAX_SKIP_DEPTH {
            return Err(WireError::CborLimit);
        }
        let (major, value) = self.read_header()?;
        match major {
            MAJOR_UNSIGNED | MAJOR_NEGATIVE => Ok(()),
            MAJOR_BYTES | MAJOR_TEXT => {
                self.read_exact(value as usize)?;
                Ok(())
            }
            MAJOR_ARRAY => {
                for _ in 0..value {
                    self.skip_value_at_depth(depth + 1)?;
                }
                Ok(())
            }
            MAJOR_MAP => {
                if value > MAX_MAP_ENTRIES {
                    return Err(WireError::CborLimit);
                }
                for _ in 0..value {
                    self.skip_value_at_depth(depth + 1)?;
                    self.skip_value_at_depth(depth + 1)?;
                }
                Ok(())
            }
            MAJOR_TAG => self.skip_value_at_depth(depth + 1),
            MAJOR_SIMPLE => match value {
                20..=23 => Ok(()),
                25 => {
                    self.read_exact(2)?;
                    Ok(())
                }
                26 => {
                    self.read_exact(4)?;
                    Ok(())
                }
                27 => {
                    self.read_exact(8)?;
                    Ok(())
                }
                _ => Err(WireError::UnexpectedCborType),
            },
            _ => Err(WireError::UnexpectedCborType),
        }
    }

    fn read_header(&mut self) -> Result<(u8, u64), WireError> {
        let first = self.read_byte()?;
        let major = first >> 5;
        let additional = first & 0x1f;
        if additional == ADDITIONAL_INDEFINITE {
            return Err(WireError::IndefiniteCbor);
        }
        let value = match additional {
            n if n < 24 => u64::from(n),
            24 => u64::from(self.read_byte()?),
            25 => {
                let bytes = self.read_exact(2)?;
                let mut arr = [0u8; 2];
                arr.copy_from_slice(bytes);
                u64::from(u16::from_be_bytes(arr))
            }
            26 => {
                let bytes = self.read_exact(4)?;
                let mut arr = [0u8; 4];
                arr.copy_from_slice(bytes);
                u64::from(u32::from_be_bytes(arr))
            }
            27 => {
                let bytes = self.read_exact(8)?;
                let mut arr = [0u8; 8];
                arr.copy_from_slice(bytes);
                u64::from_be_bytes(arr)
            }
            _ => return Err(WireError::UnexpectedCborType),
        };
        Ok((major, value))
    }

    fn read_byte(&mut self) -> Result<u8, WireError> {
        let byte = *self.data.get(self.pos).ok_or(WireError::TruncatedCbor)?;
        self.pos += 1;
        Ok(byte)
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], WireError> {
        if len > MAX_VALUE_BYTES {
            return Err(WireError::CborLimit);
        }
        let end = self.pos.checked_add(len).ok_or(WireError::CborLimit)?;
        if end > self.data.len() {
            return Err(WireError::TruncatedCbor);
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_integers_use_shortest_form() {
        let mut w = CborWriter::new();
        w.u64(1);
        w.u64(23);
        w.u64(24);
        w.u64(256);
        assert_eq!(
            w.into_inner(),
            vec![0x01, 0x17, 0x18, 0x18, 0x19, 0x01, 0x00]
        );
    }

    #[test]
    fn map_header_for_eleven_entries_is_canonical() {
        let mut w = CborWriter::new();
        w.map(11);
        assert_eq!(w.into_inner(), vec![0xab]);
    }

    #[test]
    fn rejects_indefinite_maps() {
        let mut r = CborReader::new(&[0xbf]);
        assert!(matches!(r.map(), Err(WireError::IndefiniteCbor)));
    }
}
