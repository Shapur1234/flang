use std::io::{Bytes, Read};

use crate::error::ReadError;

/// UTF-8 decoding reader wrapper
/// Reads bytes and yields `Result<char, ReadError>`
// Based of https://users.rust-lang.org/t/idiomatic-way-to-read-chars/97262/3
pub enum Utf8Reader<R> {
    Active(Bytes<R>),
    Fused,
}

impl<R: Read> Utf8Reader<R> {
    /// Creates a new UTF-8 reader from a byte source
    pub fn new(source: R) -> Utf8Reader<R> {
        #[allow(clippy::unbuffered_bytes)]
        Utf8Reader::Active(source.bytes())
    }
}

impl<R: Read> Iterator for Utf8Reader<R> {
    type Item = Result<char, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let Utf8Reader::Active(source) = self else {
            return None;
        };

        let mut bytes = [0u8; 4];
        for n in 0..4 {
            bytes[n] = match source.next() {
                None => {
                    // EOF
                    *self = Utf8Reader::Fused;
                    return match (n, str::from_utf8(&bytes[..n])) {
                        // EOF at char boundary
                        (0, _) => None,

                        // Incomplete char at EOF
                        (_, Err(e)) => Some(Err(e.into())),

                        // Returned in previous loop iteration
                        _ => unreachable!(),
                    };
                }
                Some(Err(e)) => {
                    // I/O Error in reader
                    *self = Utf8Reader::Fused;
                    return Some(Err(e.into()));
                }

                // Byte available
                Some(Ok(byte)) => byte,
            };

            match str::from_utf8(&bytes[..=n]) {
                // Complete char has been read
                Ok(string) => {
                    return Some(Ok(string.chars().next().unwrap()));
                }

                // Invalid UTF-8 sequence in input
                Err(e) if e.error_len().is_some() => {
                    *self = Utf8Reader::Fused;
                    return Some(Err(e.into()));
                }
                _ => (),
            }
        }

        // 4 bytes is the maximum length of a UTF-8 sequence
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_utf8_reader_valid() {
        let data = "a🦀".as_bytes();
        let mut reader = Utf8Reader::new(Cursor::new(data));

        assert_eq!(reader.next().unwrap().unwrap(), 'a');
        assert_eq!(reader.next().unwrap().unwrap(), '🦀');
        assert!(reader.next().is_none());
    }

    #[test]
    fn test_utf8_reader_invalid_and_fused() {
        let data = b"\xFF\x61";
        let mut reader = Utf8Reader::new(Cursor::new(data));

        assert!(reader.next().unwrap().is_err());
        assert!(reader.next().is_none());
    }
}
