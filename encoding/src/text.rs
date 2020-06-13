//! This module contains reusable components for encoding and decoding text in DICOM
//! data structures, including support for character repertoires.
//!
//! The Character Repertoires supported by DICOM are:
//! - ISO 8859
//! - JIS X 0201-1976 Code for Information Interchange
//! - JIS X 0208-1990 Code for the Japanese Graphic Character set for information interchange
//! - JIS X 0212-1990 Code of the supplementary Japanese Graphic Character set for information interchange
//! - KS X 1001 (registered as ISO-IR 149) for Korean Language
//! - TIS 620-2533 (1990) Thai Characters Code for Information Interchange
//! - ISO 10646-1, 10646-2, and their associated supplements and extensions for Unicode character set
//! - GB 18030
//! - GB2312
//!
//! At the moment, text encoding support is limited.
//! Please see [`SpecificCharacterSet`] for a complete enumeration
//! of all supported character encoding in the crate.
//!
//! [`SpecificCharacterSet`]: ./enum.SpecificCharacterSet.html

use crate::error::{Result, TextEncodingError};
use encoding::all::{GB18030, ISO_8859_1, ISO_8859_2, UTF_8};
use encoding::{DecoderTrap, EncoderTrap, Encoding, RawDecoder, StringWriter};
use std::fmt::Debug;

/// A holder of encoding and decoding mechanisms for text in DICOM content,
/// which according to the standard, depends on the specific character set.
pub trait TextCodec {
    /// Obtain the defined term (unique name) of the text encoding,
    /// which may be used as the value of a
    /// Specific Character Set (0008, 0005) element to refer to this codec.
    ///
    /// Should contain no leading or trailing spaces.
    /// This method may be useful for testing purposes, considering that
    /// `TextCodec` is often used as a trait object.
    fn name(&self) -> &'static str;

    /// Decode the given byte buffer as a single string. The resulting string
    /// _may_ contain backslash characters ('\') to delimit individual values,
    /// and should be split later on if required.
    fn decode(&self, text: &[u8]) -> Result<String>;

    /// Encode a text value into a byte vector. The input string can
    /// feature multiple text values by using the backslash character ('\')
    /// as the value delimiter.
    fn encode(&self, text: &str) -> Result<Vec<u8>>;
}

impl<T: ?Sized> TextCodec for Box<T>
where
    T: TextCodec,
{
    fn name(&self) -> &'static str {
        self.as_ref().name()
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        self.as_ref().decode(text)
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        self.as_ref().encode(text)
    }
}

impl<'a, T: ?Sized> TextCodec for &'a T
where
    T: TextCodec,
{
    fn name(&self) -> &'static str {
        (**self).name()
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        (**self).decode(text)
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        (**self).encode(text)
    }
}

/// Type alias for a type erased text codec.
///
/// It is important because stateful decoders may need to change the expected
/// text encoding format at run-time.
pub type DynamicTextCodec = Box<dyn TextCodec>;

/// An enum type for all currently supported character sets.
#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SpecificCharacterSet {
    /// **ISO-IR 6**: the default character set.
    Default,
    /// **ISO-IR 101** (ISO-8859-1): Right-hand part of the latin alphabet no. 1,
    /// the Western Europe character set.
    IsoIr100,
    /// **ISO-IR 101** (ISO-8859-2): Right-hand part of the latin alphabet no. 2,
    /// the Eastern Europe character set.
    IsoIr101,
    /// **ISO-IR 192**:: The Unicode character set based on the UTF-8 encoding.
    IsoIr192,
    /// **GB18030**: The Simplified Chinese character set.
    GB18030,
    // Support for more text encodings is tracked in issue #40.
}

impl Default for SpecificCharacterSet {
    fn default() -> Self {
        SpecificCharacterSet::Default
    }
}

impl SpecificCharacterSet {
    pub fn from_code(uid: &str) -> Option<Self> {
        use self::SpecificCharacterSet::*;
        match uid.trim_end() {
            "Default" | "ISO_IR_6" | "ISO_IR 6" => Some(Default),
            "ISO_IR_100" | "ISO_IR 100" => Some(IsoIr100),
            "ISO_IR_101" | "ISO_IR 101" => Some(IsoIr101),
            "ISO_IR 192" => Some(IsoIr192),
            "GB18030" => Some(GB18030),
            _ => None,
        }
    }

    /// Retrieve the respective text codec.
    pub fn codec(self) -> Option<DynamicTextCodec> {
        match self {
            SpecificCharacterSet::Default => Some(Box::new(DefaultCharacterSetCodec)),
            SpecificCharacterSet::IsoIr100 => Some(Box::new(IsoIr100CharacterSetCodec)),
            SpecificCharacterSet::IsoIr101 => Some(Box::new(IsoIr101CharacterSetCodec)),
            SpecificCharacterSet::IsoIr192 => Some(Box::new(Utf8CharacterSetCodec)),
            SpecificCharacterSet::GB18030 => Some(Box::new(Gb18030CharacterSetCodec)),
        }
    }
}

fn decode_text_trap(
    _decoder: &mut dyn RawDecoder,
    input: &[u8],
    output: &mut dyn StringWriter,
) -> bool {
    let c = input[0];
    let o0 = c & 7;
    let o1 = (c & 56) >> 3;
    let o2 = (c & 192) >> 6;
    output.write_char('\\');
    output.write_char((o2 + b'0') as char);
    output.write_char((o1 + b'0') as char);
    output.write_char((o0 + b'0') as char);
    true
}

/// Data type representing the default character set.
#[derive(Debug, Default, Copy, Clone, Eq, Hash, PartialEq)]
pub struct DefaultCharacterSetCodec;

impl TextCodec for DefaultCharacterSetCodec {
    fn name(&self) -> &'static str {
        "ISO_IR 6"
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        ISO_8859_1
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|e| TextEncodingError::new(e).into())
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        ISO_8859_1
            .encode(text, EncoderTrap::Strict)
            .map_err(|e| TextEncodingError::new(e).into())
    }
}

/// Data type representing the ISO-IR 100 characters set.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Copy)]
pub struct IsoIr100CharacterSetCodec;

impl TextCodec for IsoIr100CharacterSetCodec {
    fn name(&self) -> &'static str {
        "ISO_IR 100"
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        ISO_8859_1
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|e| TextEncodingError::new(e).into())
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        ISO_8859_1
            .encode(text, EncoderTrap::Strict)
            .map_err(|e| TextEncodingError::new(e).into())
    }
}

/// Data type representing the ISO-IR 101 characters set.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Copy)]
pub struct IsoIr101CharacterSetCodec;

impl TextCodec for IsoIr101CharacterSetCodec {
    fn name(&self) -> &'static str {
        "ISO_IR 101"
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        ISO_8859_2
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|e| TextEncodingError::new(e).into())
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        ISO_8859_2
            .encode(text, EncoderTrap::Strict)
            .map_err(|e| TextEncodingError::new(e).into())
    }
}

/// Data type representing the UTF-8 character set.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Copy)]
pub struct Utf8CharacterSetCodec;

impl TextCodec for Utf8CharacterSetCodec {
    fn name(&self) -> &'static str {
        "ISO_IR 192"
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        UTF_8
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|e| TextEncodingError::new(e).into())
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        UTF_8
            .encode(text, EncoderTrap::Strict)
            .map_err(|e| TextEncodingError::new(e).into())
    }
}

/// Data type representing the GB18030 character set.
#[derive(Debug, Default, Clone, PartialEq, Eq, Copy)]
pub struct Gb18030CharacterSetCodec;

impl TextCodec for Gb18030CharacterSetCodec {
    fn name(&self) -> &'static str {
        "GB18030"
    }

    fn decode(&self, text: &[u8]) -> Result<String> {
        GB18030
            .decode(text, DecoderTrap::Call(decode_text_trap))
            .map_err(|e| TextEncodingError::new(e).into())
    }

    fn encode(&self, text: &str) -> Result<Vec<u8>> {
        GB18030
            .encode(text, EncoderTrap::Strict)
            .map_err(|e| TextEncodingError::new(e).into())
    }
}

/// The result of a text validation procedure (please see [`validate_iso_8859`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextValidationOutcome {
    /// The text is fully valid and can be safely decoded.
    Ok,
    /// Some characters may have to be replaced, other than that the text can be safely decoded.
    BadCharacters,
    /// The text cannot be decoded.
    NotOk,
}

/// Check whether the given byte slice contains valid text from the default character repertoire.
pub fn validate_iso_8859(text: &[u8]) -> TextValidationOutcome {
    if ISO_8859_1.decode(text, DecoderTrap::Strict).is_err() {
        match ISO_8859_1.decode(text, DecoderTrap::Call(decode_text_trap)) {
            Ok(_) => TextValidationOutcome::BadCharacters,
            Err(_) => TextValidationOutcome::NotOk,
        }
    } else {
        TextValidationOutcome::Ok
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Date value representation.
pub fn validate_da(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| c >= b'0' && c <= b'9') {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Time value representation.
pub fn validate_tm(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b'\\' | b'.' | b'-' | b' ' => true,
        c => c >= b'0' && c <= b'9',
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Date Time value representation.
pub fn validate_dt(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b'.' | b'-' | b'+' | b' ' | b'\\' => true,
        c => c >= b'0' && c <= b'9',
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

/// Check whether the given byte slice contains only valid characters for a
/// Code String value representation.
pub fn validate_cs(text: &[u8]) -> TextValidationOutcome {
    if text.iter().cloned().all(|c| match c {
        b' ' | b'_' => true,
        c => (c >= b'0' && c <= b'9') || (c >= b'A' && c <= b'Z'),
    }) {
        TextValidationOutcome::Ok
    } else {
        TextValidationOutcome::NotOk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_ir_6_baseline() {
        let codec = SpecificCharacterSet::Default
            .codec()
            .expect("Must be fully supported");
        assert_eq!(codec.decode(b"Smith^John").unwrap(), "Smith^John");
    }

    #[test]
    fn iso_ir_192_baseline() {
        let codec = SpecificCharacterSet::IsoIr192
            .codec()
            .expect("Should be fully supported");
        assert_eq!(
            codec.decode("Simões^John".as_bytes()).unwrap(),
            "Simões^John",
        );
        assert_eq!(
            codec.decode("Иванков^Андрей".as_bytes()).unwrap(),
            "Иванков^Андрей",
        );
    }

    #[test]
    fn iso_ir_100_baseline() {
        let codec = SpecificCharacterSet::IsoIr100
            .codec()
            .expect("Should be fully supported");
        assert_eq!(codec.decode(b"Sim\xF5es^Jo\xE3o").unwrap(), "Simões^João");
        assert_eq!(codec.decode(b"G\xfcnther^Hans").unwrap(), "Günther^Hans");
    }

    #[test]
    fn iso_ir_101_baseline() {
        let codec = SpecificCharacterSet::IsoIr101
            .codec()
            .expect("Should be fully supported");
        assert_eq!(codec.decode(b"G\xfcnther^Hans").unwrap(), "Günther^Hans");
    }
}
