//! Immutable strings that share repeated text across independently loaded records.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use string_cache::DefaultAtom;
use ts_rs::TS;

/// Stores short strings inline and shares reference-counted storage for longer strings.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CodexCachedStr(#[schemars(with = "String")] DefaultAtom);

impl CodexCachedStr {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns a borrowed view without cloning the cached string.
    pub fn as_cow(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.as_str())
    }
}

impl Deref for CodexCachedStr {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for CodexCachedStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for CodexCachedStr {
    fn from(text: &str) -> Self {
        Self(text.into())
    }
}

impl From<String> for CodexCachedStr {
    fn from(text: String) -> Self {
        Self(text.into())
    }
}

impl fmt::Debug for CodexCachedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for CodexCachedStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

// A derived TS implementation would introduce a named type in client bindings.
impl TS for CodexCachedStr {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name() -> String {
        String::name()
    }

    fn inline() -> String {
        String::inline()
    }

    fn inline_flattened() -> String {
        String::inline_flattened()
    }

    fn decl() -> String {
        String::decl()
    }

    fn decl_concrete() -> String {
        String::decl_concrete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independently decoded text and clones share storage while remaining plain strings on the wire.
    #[test]
    fn long_strings_share_storage_across_roundtrips_and_clones() {
        let text = CodexCachedStr::from("local message override");
        let wire = serde_json::to_string(&text).unwrap();
        assert_eq!(wire, r#""local message override""#);
        let decoded: CodexCachedStr = serde_json::from_str(&wire).unwrap();
        let cloned = text.clone();
        assert!(std::ptr::eq(text.as_str(), decoded.as_str()));
        assert!(std::ptr::eq(text.as_str(), cloned.as_str()));
    }

    /// Short UTF-8 strings survive inline storage and borrow directly from the handle.
    #[test]
    fn short_text_borrows_without_allocation() {
        let short: CodexCachedStr = serde_json::from_str(r#""é""#).unwrap();
        assert!(short.0.is_inline());
        assert!(matches!(short.as_cow(), Cow::Borrowed("é")));
    }
}
