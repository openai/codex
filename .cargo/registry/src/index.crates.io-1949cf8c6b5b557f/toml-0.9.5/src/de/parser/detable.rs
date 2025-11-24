use alloc::borrow::Cow;

use serde_spanned::Spanned;

use crate::alloc_prelude::*;
use crate::de::DeString;
use crate::de::DeValue;
use crate::map::Map;

/// Type representing a TOML table, payload of the `Value::Table` variant.
///
/// By default it entries are stored in
/// [lexicographic order](https://doc.rust-lang.org/std/primitive.str.html#impl-Ord-for-str)
/// of the keys. Enable the `preserve_order` feature to store entries in the order they appear in
/// the source file.
pub type DeTable<'i> = Map<Spanned<DeString<'i>>, Spanned<DeValue<'i>>>;

impl<'i> DeTable<'i> {
    /// Parse a TOML document
    pub fn parse(input: &'i str) -> Result<Spanned<Self>, crate::de::Error> {
        let source = toml_parser::Source::new(input);
        let mut errors = crate::de::error::TomlSink::<Option<_>>::new(source);
        let value = crate::de::parser::parse_document(source, &mut errors);
        if let Some(err) = errors.into_inner() {
            Err(err)
        } else {
            Ok(value)
        }
    }

    /// Parse a TOML document, with best effort recovery on error
    pub fn parse_recoverable(input: &'i str) -> (Spanned<Self>, Vec<crate::de::Error>) {
        let source = toml_parser::Source::new(input);
        let mut errors = crate::de::error::TomlSink::<Vec<_>>::new(source);
        let value = crate::de::parser::parse_document(source, &mut errors);
        (value, errors.into_inner())
    }

    /// Ensure no data is borrowed
    pub fn make_owned(&mut self) {
        self.mut_entries(|k, v| {
            let owned = core::mem::take(k.get_mut());
            *k.get_mut() = Cow::Owned(owned.into_owned());
            v.get_mut().make_owned();
        });
    }
}
