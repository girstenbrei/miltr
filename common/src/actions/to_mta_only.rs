use std::borrow::Cow;

use bytes::{BufMut, BytesMut};
use itertools::Itertools;

use crate::decoding::Parsable;
use crate::encoding::Writable;
use crate::{error::STAGE_DECODING, NotEnoughData};
use crate::{InvalidData, ProtocolError};
use miltr_utils::ByteParsing;

/// (Silently) discard this mail without forwarding it
#[derive(Debug, Clone)]
pub struct Discard;

impl Discard {
    const CODE: u8 = b'd';
}

impl Parsable for Discard {
    const CODE: u8 = Self::CODE;

    fn parse(_buffer: BytesMut) -> Result<Self, ProtocolError> {
        Ok(Self)
    }
}

impl Writable for Discard {
    fn write(&self, _buffer: &mut BytesMut) {}

    fn len(&self) -> usize {
        0
    }

    fn code(&self) -> u8 {
        Self::CODE
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Reject this mail, informing the smtp client about it
#[derive(Debug, Clone)]
pub struct Reject;

impl Reject {
    const CODE: u8 = b'r';
}

impl Parsable for Reject {
    const CODE: u8 = Self::CODE;

    fn parse(_buffer: BytesMut) -> Result<Self, ProtocolError> {
        Ok(Self)
    }
}

impl Writable for Reject {
    fn write(&self, _buffer: &mut BytesMut) {}

    fn len(&self) -> usize {
        0
    }

    fn code(&self) -> u8 {
        Self::CODE
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Return a tempfail code to the smtp client
#[derive(Debug, Clone)]
pub struct Tempfail;

impl Tempfail {
    const CODE: u8 = b't';
}

impl Parsable for Tempfail {
    const CODE: u8 = Self::CODE;

    fn parse(_buffer: BytesMut) -> Result<Self, ProtocolError> {
        Ok(Self)
    }
}

impl Writable for Tempfail {
    fn write(&self, _buffer: &mut BytesMut) {}

    fn len(&self) -> usize {
        0
    }

    fn code(&self) -> u8 {
        Self::CODE
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Skip this mail processing
#[derive(Debug, Clone)]
pub struct Skip;

impl Skip {
    const CODE: u8 = b's';
}

impl Parsable for Skip {
    const CODE: u8 = Self::CODE;

    fn parse(_buffer: BytesMut) -> Result<Self, ProtocolError> {
        Ok(Self)
    }
}

impl Writable for Skip {
    fn write(&self, _buffer: &mut BytesMut) {}

    fn len(&self) -> usize {
        0
    }

    fn code(&self) -> u8 {
        Self::CODE
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

const REPLY_CODE_LENGTH: usize = 3;
/// Return this status code to the smtp client
#[derive(Debug, Clone)]
pub struct Replycode {
    rcode: RCode,
    xcode: Option<XCode>,
    message: BytesMut,
}

impl Replycode {
    const CODE: u8 = b'y';

    /// Create a Replycode
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn new<R: Into<RCode>, X: Into<XCode>>(rcode: R, xcode: X, message: &str) -> Self {
        let rcode = rcode.into();
        let xcode = Some(xcode.into());

        Self {
            rcode,
            xcode,
            message: BytesMut::from(message.as_bytes()),
        }
    }

    /// Create a Replycode without xcode
    #[allow(clippy::similar_names)]
    pub fn without_xcode<R: Into<RCode>>(rcode: R, message: &str) -> Self {
        let rcode = rcode.into();

        Self {
            rcode,
            xcode: None,
            message: BytesMut::from(message.as_bytes()),
        }
    }

    /// The message associated with this reply code
    #[must_use]
    pub fn message(&self) -> Cow<str> {
        String::from_utf8_lossy(&self.message)
    }

    /// The smtp return code
    #[must_use]
    pub fn rcode(&self) -> &RCode {
        &self.rcode
    }

    /// The smtp enhanced return code
    #[must_use]
    pub fn xcode(&self) -> &Option<XCode> {
        &self.xcode
    }
}

impl Parsable for Replycode {
    const CODE: u8 = Self::CODE;

    // rcode and xcode are just named that in the docs. Keeping it consistent.
    #[allow(clippy::similar_names)]
    fn parse(mut buffer: BytesMut) -> Result<Self, ProtocolError> {
        #[allow(clippy::similar_names)]
        let Some(rcode) = buffer.delimited(b' ') else {
            return Err(NotEnoughData::new(
                STAGE_DECODING,
                "Replycode",
                "Missing nullbyte delimiter after rcode",
                1,
                0,
                buffer,
            )
            .into());
        };
        let rcode = RCode::parse(rcode)?;

        let Some(raw_message) = buffer.delimited(0) else {
            return Err(NotEnoughData::new(
                STAGE_DECODING,
                "Replycode",
                "Missing nullbyte delimiter after message",
                1,
                0,
                buffer,
            )
            .into());
        };
        let mut xcode = None;
        let mut message = raw_message;

        if let Some(pos) = message.iter().position(|c| *c == b' ') {
            if let Ok(code) = XCode::parse(BytesMut::from(message[0..pos].as_ref())) {
                xcode = Some(code);
                message = BytesMut::from(&message[pos + 1..]);
            }
        }

        Ok(Self {
            rcode,
            xcode,
            message,
        })
    }
}

impl Writable for Replycode {
    fn write(&self, buffer: &mut BytesMut) {
        buffer.put_slice(self.rcode.as_bytes());
        buffer.put_u8(b' ');
        if let Some(ref xcode) = self.xcode {
            buffer.put_slice(xcode.as_bytes());
            buffer.put_u8(b' ');
        }
        buffer.put_slice(&self.message);
        buffer.put_u8(0);
    }

    fn len(&self) -> usize {
        self.rcode.len()
            + 1
            + self.xcode.as_ref().map_or(0, |code| code.len() + 1)
            + self.message.len()
            + 1
    }

    fn code(&self) -> u8 {
        Self::CODE
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone)]
pub struct XCode {
    code: [u16; REPLY_CODE_LENGTH],
    bytes: BytesMut,
}

impl From<[u16; REPLY_CODE_LENGTH]> for XCode {
    fn from(code: [u16; REPLY_CODE_LENGTH]) -> Self {
        Self::new(code)
    }
}

impl XCode {
    pub fn new(code: [u16; REPLY_CODE_LENGTH]) -> Self {
        Self {
            code,
            bytes: BytesMut::from_iter(code.iter().map(ToString::to_string).join(".").as_bytes()),
        }
    }

    fn parse(buffer: BytesMut) -> Result<Self, InvalidData> {
        let mut positions = buffer.iter().positions(|&c| c == b'.');
        let mut code: [u16; 3] = [0_u16; REPLY_CODE_LENGTH];

        let mut start = 0;
        for c_code in code.iter_mut().take(REPLY_CODE_LENGTH - 1) {
            let Some(end) = positions.next() else {
                return Err(InvalidData {
                    msg: "missing '.' delimiter in code",
                    offending_bytes: buffer,
                });
            };
            let raw = &buffer[start..end];
            let Ok(number) = String::from_utf8_lossy(raw).parse() else {
                return Err(InvalidData {
                    msg: "invalid u16 in code",
                    offending_bytes: buffer,
                });
            };

            *c_code = number;
            start = end + 1;
        }
        let raw = &buffer[start..buffer.len()];
        let Ok(number) = String::from_utf8_lossy(raw).parse() else {
            return Err(InvalidData {
                msg: "invalid u16 in code",
                offending_bytes: buffer,
            });
        };

        code[REPLY_CODE_LENGTH - 1] = number;

        Ok(Self {
            code,
            bytes: buffer,
        })
    }

    /// The status code
    #[must_use]
    pub fn code(&self) -> [u16; REPLY_CODE_LENGTH] {
        self.code
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }
}
#[derive(Debug, Clone)]
pub struct RCode {
    code: [u8; REPLY_CODE_LENGTH],
    bytes: BytesMut,
}

impl From<[u8; REPLY_CODE_LENGTH]> for RCode {
    fn from(code: [u8; REPLY_CODE_LENGTH]) -> Self {
        Self::new(code)
    }
}

impl RCode {
    pub fn new(code: [u8; REPLY_CODE_LENGTH]) -> Self {
        Self {
            code,
            bytes: BytesMut::from_iter(code.iter().map(ToString::to_string).join("").as_bytes()),
        }
    }

    fn parse(buffer: BytesMut) -> Result<Self, InvalidData> {
        if buffer.len() < REPLY_CODE_LENGTH {
            return Err(InvalidData {
                msg: "Invalid length of code",
                offending_bytes: buffer,
            });
        }
        let mut code: [u8; 3] = [0_u8; REPLY_CODE_LENGTH];
        for (pos, c_code) in code.iter_mut().enumerate() {
            let Ok(number) = String::from_utf8_lossy(&[buffer[pos]]).parse::<u8>() else {
                return Err(InvalidData {
                    msg: "invalid u8 in code",
                    offending_bytes: buffer,
                });
            };
            *c_code = number;
        }
        Ok(Self {
            code,
            bytes: buffer,
        })
    }

    /// The status code
    #[must_use]
    pub fn code(&self) -> [u8; REPLY_CODE_LENGTH] {
        self.code
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_xcode_valid() {
        let input = BytesMut::from_iter(b"1.20.3");
        let code = XCode::parse(input).expect("Failed parsing input");

        assert_eq!(code.code, [1, 20, 3]);

        println!("{:?}", code.bytes);
        assert_eq!(6, code.bytes.len());
    }

    #[test]
    fn test_xcode_invalid() {
        let input = BytesMut::from_iter(b"1.23");
        let _code = XCode::parse(input).expect_err("Parsing did not error on invalid");
    }

    #[test]
    fn test_rcode_valid() {
        let input = BytesMut::from_iter(b"454");
        let code = RCode::parse(input).expect("Failed parsing input");

        assert_eq!(code.code, [4, 5, 4]);

        println!("{:?}", code.bytes);
        assert_eq!(code.bytes.as_ref(), b"454");
    }

    #[test]
    fn test_rcode_invalid() {
        let input = BytesMut::from_iter(b"4.54");
        let _code = RCode::parse(input).expect_err("Parsing did not error on invalid");
    }

    #[test]
    fn test_reply_parse() {
        let input = BytesMut::from_iter(b"501 5.7.0 Client initiated Authentication Exchange\0");
        let reply: Replycode = Parsable::parse(input).expect("Parsing failed");
        assert_eq!(reply.rcode.as_bytes(), b"501");
        assert_eq!(reply.xcode.expect("Parsing failed").as_bytes(), b"5.7.0");
        assert_eq!(reply.message, "Client initiated Authentication Exchange");
    }

    #[test]
    fn test_reply_parse_empty_xcode() {
        let input =
            BytesMut::from_iter(b"421 Service not available, closing transmission channel\0");
        let reply: Replycode = Parsable::parse(input).expect("Parsing failed");
        assert_eq!(reply.rcode.as_bytes(), b"421");
        assert!(reply.xcode.is_none());
        assert_eq!(
            reply.message,
            "Service not available, closing transmission channel"
        );
    }

    #[test]
    fn test_reply_write() {
        let input = BytesMut::from_iter(b"501 5.7.0 Client initiated Authentication Exchange\0");
        let reply: Replycode = Parsable::parse(input.clone()).expect("Parsing failed");
        let mut output = BytesMut::new();
        reply.write(&mut output);
        assert_eq!(output.as_ref(), input.as_ref());
    }

    #[test]
    fn test_reply_write_with_empty_xcode() {
        let input =
            BytesMut::from_iter(b"421 Service not available, closing transmission channel\0");
        let reply: Replycode = Parsable::parse(input.clone()).expect("Parsing failed");
        let mut output = BytesMut::new();
        reply.write(&mut output);
        assert_eq!(output.as_ref(), input.as_ref());
    }
}
