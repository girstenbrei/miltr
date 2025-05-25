use std::borrow::Cow;

use bytes::{BufMut, BytesMut};

use crate::decoding::Parsable;
use crate::encoding::Writable;
use crate::InvalidData;
use crate::ProtocolError;
use miltr_utils::ByteParsing;

/// An smtp header received
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Header {
    name: BytesMut,
    value: BytesMut,
}

impl Header {
    const CODE: u8 = b'L';

    /// Create a Header from some bytes
    #[must_use]
    pub fn new(name: &[u8], value: &[u8]) -> Self {
        Self {
            name: BytesMut::from_iter(name),
            value: BytesMut::from_iter(value),
        }
    }
    /// The name of the received header
    #[must_use]
    pub fn name(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.name)
    }

    /// The value of the received header
    #[must_use]
    pub fn value(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.value)
    }
}

impl Parsable for Header {
    const CODE: u8 = Self::CODE;

    fn parse(mut buffer: BytesMut) -> Result<Self, ProtocolError> {
        let Some(name) = buffer.delimited(0) else {
            return Err(InvalidData::new(
                "Received header package without name terminated by null byte in it",
                buffer,
            )
            .into());
        };

        let Some(value) = buffer.delimited(0) else {
            return Err(InvalidData::new(
                "Received header package without value terminated by null byte in it",
                buffer,
            )
            .into());
        };

        Ok(Self { name, value })
    }
}

impl Writable for Header {
    fn write(&self, buffer: &mut BytesMut) {
        buffer.extend_from_slice(&self.name);
        buffer.put_u8(0);
        buffer.extend_from_slice(&self.value);
        buffer.put_u8(0);
    }

    fn len(&self) -> usize {
        self.name.len() + 1 + self.value.len() + 1
    }

    fn code(&self) -> u8 {
        Self::CODE
    }

    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.value.is_empty()
    }
}

/// After all headers have been sent, end of header is sent
#[derive(Clone, PartialEq, Debug, Default)]
pub struct EndOfHeader;

impl EndOfHeader {
    const CODE: u8 = b'N';
}

impl Parsable for EndOfHeader {
    const CODE: u8 = Self::CODE;

    fn parse(_buffer: BytesMut) -> Result<Self, ProtocolError> {
        Ok(Self)
    }
}

impl Writable for EndOfHeader {
    fn write(&self, _buffer: &mut BytesMut) {}

    fn len(&self) -> usize {
        0
    }

    fn code(&self) -> u8 {
        Self::CODE
    }

    fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::decoding::Parsable;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(BytesMut::from("name\0value\0"), Ok(Header {name: BytesMut::from("name"), value: BytesMut::from("value")} ))]
    #[case(
        BytesMut::from("name\0value"),
        Err(InvalidData::new(
            "Received header package without value terminated by null byte in it",
            BytesMut::new()
        ))
    )]
    #[case(
        BytesMut::from("namevalue\0"),
        Err(InvalidData::new(
            "Received header package without value terminated by null byte in it",
            BytesMut::new()
        ))
    )]
    fn test_header(#[case] input: BytesMut, #[case] expected: Result<Header, InvalidData>) {
        let parsed_header = Header::parse(input);

        match (expected, parsed_header) {
            (Err(expected), Err(ProtocolError::InvalidData(parsed))) => {
                assert_eq!(expected.msg, parsed.msg);
            }
            (Ok(expected), Ok(parsed)) => assert_eq!(expected, parsed),
            (expected, parsed) => panic!("Did not get expected:\n{expected:?}\n vs \n{parsed:?}"),
        }
    }
    #[cfg(feature = "count-allocations")]
    #[test]
    fn test_parse_header() {
        let buffer = BytesMut::from("name\0value\0");

        let info = allocation_counter::measure(|| {
            let res = Header::parse(buffer);

            allocation_counter::opt_out(|| {
                println!("{res:?}");
                assert!(res.is_ok());
            });
        });

        println!("{info:#?}");
        assert_eq!(info.count_total, 1);
    }
}
