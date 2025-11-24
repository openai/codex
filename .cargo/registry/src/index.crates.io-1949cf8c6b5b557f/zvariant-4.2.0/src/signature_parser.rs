use std::ops::{Bound, RangeBounds};

use crate::{subslice, Basic, ObjectPath, Result, Signature, STRUCT_SIG_END_CHAR};

#[cfg(unix)]
use crate::Fd;

#[cfg(feature = "gvariant")]
use crate::utils::MAYBE_SIGNATURE_CHAR;
use crate::utils::{
    ARRAY_SIGNATURE_CHAR, DICT_ENTRY_SIG_END_CHAR, DICT_ENTRY_SIG_START_CHAR,
    STRUCT_SIG_START_CHAR, VARIANT_SIGNATURE_CHAR,
};

#[derive(Debug, Clone)]
pub(crate) struct SignatureParser<'s> {
    signature: Signature<'s>,
    pos: usize,
    end: usize,
}

impl<'s> SignatureParser<'s> {
    pub fn new(signature: Signature<'s>) -> Self {
        let end = signature.len();

        Self {
            signature,
            pos: 0,
            end,
        }
    }

    pub(crate) unsafe fn from_bytes_unchecked(signature: &'s [u8]) -> Result<Self> {
        if signature.len() > 255 {
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &"<= 255 characters",
            ));
        }

        let signature = Signature::from_bytes_unchecked(signature);
        Ok(Self::new(signature))
    }

    pub fn validate(signature: &'s [u8]) -> Result<()> {
        // SAFETY: the parser is only used to validate the signature
        for s in unsafe { Self::from_bytes_unchecked(signature)? } {
            s?;
        }
        Ok(())
    }

    pub fn signature(&self) -> Signature<'_> {
        self.signature.slice(self.pos..self.end)
    }

    pub fn next_char(&self) -> Result<char> {
        subslice(self.signature.as_bytes(), self.pos).map(|b| *b as char)
    }

    #[inline]
    pub fn skip_char(&mut self) -> Result<()> {
        self.skip_chars(1)
    }

    pub fn skip_chars(&mut self, num_chars: usize) -> Result<()> {
        self.pos += num_chars;

        // We'll be going one char beyond at the end of parsing but not beyond that.
        if self.pos > self.end {
            return Err(serde::de::Error::invalid_length(
                self.signature.len(),
                &format!(">= {} characters", self.pos).as_str(),
            ));
        }

        Ok(())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.pos
    }

    #[inline]
    pub fn done(&self) -> bool {
        self.pos == self.end
    }

    /// Returns a slice of `self` for the provided range.
    ///
    /// # Panics
    ///
    /// Requires that begin <= end and end <= self.len(), otherwise slicing will panic.
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        let len = self.len();

        let pos = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n + 1,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&n) => n + 1,
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            pos <= end,
            "range start must not be greater than end: {:?} > {:?}",
            pos,
            end,
        );
        assert!(end <= len, "range end out of bounds: {:?} > {:?}", end, len,);

        let mut clone = self.clone();
        clone.pos += pos;
        clone.end = self.pos + end;

        clone
    }

    /// Get the next signature and increment the position.
    pub fn parse_next_signature(&mut self) -> Result<Signature<'s>> {
        let len = &self.next_signature()?.len();
        let pos = self.pos;
        self.pos += len;

        // We'll be going one char beyond at the end of parsing but not beyond that.
        if self.pos > self.end {
            return Err(serde::de::Error::invalid_length(
                self.signature.len(),
                &format!(">= {} characters", self.pos).as_str(),
            ));
        }

        Ok(self.signature.slice(pos..self.pos))
    }

    /// Get the next signature but don't increment the position.
    pub fn next_signature(&self) -> Result<Signature<'_>> {
        match self
            .signature()
            .as_bytes()
            .first()
            .map(|b| *b as char)
            .ok_or_else(|| -> crate::Error {
                serde::de::Error::invalid_length(0, &">= 1 character")
            })? {
            u8::SIGNATURE_CHAR
            | bool::SIGNATURE_CHAR
            | i16::SIGNATURE_CHAR
            | u16::SIGNATURE_CHAR
            | i32::SIGNATURE_CHAR
            | u32::SIGNATURE_CHAR
            | i64::SIGNATURE_CHAR
            | u64::SIGNATURE_CHAR
            | f64::SIGNATURE_CHAR
            | <&str>::SIGNATURE_CHAR
            | ObjectPath::SIGNATURE_CHAR
            | Signature::SIGNATURE_CHAR
            | VARIANT_SIGNATURE_CHAR => Ok(self.signature_slice(0, 1)),
            #[cfg(unix)]
            Fd::SIGNATURE_CHAR => Ok(self.signature_slice(0, 1)),
            ARRAY_SIGNATURE_CHAR => self.next_array_signature(),
            STRUCT_SIG_START_CHAR => self.next_structure_signature(),
            DICT_ENTRY_SIG_START_CHAR => self.next_dict_entry_signature(),
            #[cfg(feature = "gvariant")]
            MAYBE_SIGNATURE_CHAR => self.next_maybe_signature(),
            c => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &"a valid signature character",
            )),
        }
    }

    fn next_single_child_type_container_signature(
        &self,
        expected_sig_prefix: char,
    ) -> Result<Signature<'_>> {
        let signature = self.signature();

        if signature.len() < 2 {
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &">= 2 characters",
            ));
        }

        // We can't get None here cause we already established there is are least 2 chars above
        let c = signature
            .as_bytes()
            .first()
            .map(|b| *b as char)
            .expect("empty signature");
        if c != expected_sig_prefix {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &expected_sig_prefix.to_string().as_str(),
            ));
        }

        // There should be a valid complete signature after 'a' but not more than 1
        let child_parser = self.slice(1..);
        let child_len = child_parser.next_signature()?.len();

        Ok(self.signature_slice(0, child_len + 1))
    }

    fn next_array_signature(&self) -> Result<Signature<'_>> {
        self.next_single_child_type_container_signature(ARRAY_SIGNATURE_CHAR)
    }

    #[cfg(feature = "gvariant")]
    fn next_maybe_signature(&self) -> Result<Signature<'_>> {
        self.next_single_child_type_container_signature(MAYBE_SIGNATURE_CHAR)
    }

    fn next_structure_signature(&self) -> Result<Signature<'_>> {
        let signature = self.signature();

        if signature.len() < 3 {
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &">= 2 characters",
            ));
        }

        let mut bytes = signature.as_bytes().iter();
        // We can't get None here cause we already established there are at least 2 chars above
        let c = bytes.next().map(|b| *b as char).expect("empty signature");
        if c != STRUCT_SIG_START_CHAR {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &crate::STRUCT_SIG_START_STR,
            ));
        }
        if bytes.next().map(|b| *b as char).expect("empty signature") == STRUCT_SIG_END_CHAR {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str("()"),
                &"at least one field signature between `(` and `)`",
            ));
        }

        let mut fields_sig_len = 0;
        let mut fields_parser = self.slice(1..);
        while !fields_parser.done() && fields_parser.next_char()? != STRUCT_SIG_END_CHAR {
            fields_sig_len += fields_parser.parse_next_signature()?.len();
        }
        let c = fields_parser.next_char()?;
        if c != STRUCT_SIG_END_CHAR {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &crate::STRUCT_SIG_END_STR,
            ));
        }

        // The `(`, fields signatures and `)`.
        Ok(self.signature_slice(0, fields_sig_len + 2))
    }

    fn next_dict_entry_signature(&self) -> Result<Signature<'_>> {
        let signature = self.signature();

        if signature.len() < 4 {
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &">= 4 characters",
            ));
        }

        // We can't get None here cause we already established there are at least 4 chars above
        let bytes = signature.as_bytes();
        let c = bytes.first().map(|b| *b as char).expect("empty signature");
        if c != DICT_ENTRY_SIG_START_CHAR {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &crate::DICT_ENTRY_SIG_START_STR,
            ));
        }

        let key_parser = self.slice(1..);
        let key_signature = key_parser.next_signature()?;
        // Key's signature will always be just 1 character.
        if key_signature.len() != 1 {
            return Err(serde::de::Error::invalid_length(
                key_signature.len(),
                &"dict-entry key's signature can only be a single character",
            ));
        }

        // There should be one valid complete signature for value.
        let value_parser = self.slice(2..);
        let value_len = value_parser.next_signature()?.len();
        // signature of value + `{` + 1 char of the key signature + `}`
        let end = value_len + 3;

        if signature.len() < end {
            return Err(serde::de::Error::invalid_length(
                signature.len(),
                &format!(">= {end} characters").as_str(),
            ));
        }
        if bytes[end - 1] as char != DICT_ENTRY_SIG_END_CHAR {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Char(c),
                &crate::DICT_ENTRY_SIG_END_STR,
            ));
        }

        Ok(self.signature_slice(0, end))
    }

    fn signature_slice(&self, idx: usize, end: usize) -> Signature<'_> {
        self.signature.slice(self.pos + idx..self.pos + end)
    }
}

impl<'a> Iterator for SignatureParser<'a> {
    type Item = Result<Signature<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done() {
            None
        } else {
            Some(self.parse_next_signature())
        }
    }
}
