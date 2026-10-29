//! Bounded, display-only subset of the `codex-tui/1` tree contract.
//!
//! The wire tree is independent of its producer. Producers normalize text into leaf nodes and
//! arrays into composite nodes; unsupported primitives and properties fail deserialization. This
//! first slice renders one horizontal line of plain text, links, rows, spacers, dividers, and
//! numeric gaps. Links underline their text. Keys are preserved for future reconciliation.

mod render;

#[cfg(test)]
#[path = "ui_tree_tests.rs"]
mod tests;

use serde::Deserialize;
use serde::Serialize;

/// A single display node, or no content.
pub(crate) type UiTree = Option<UiNode>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum UiNode {
    Text {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Key>,
        props: TextProps,
    },
    Link {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Key>,
        props: LinkProps,
        children: Vec<UiTree>,
    },
    Row {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Key>,
        props: StackProps,
        children: Vec<UiTree>,
    },
    Spacer {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Key>,
    },
    Divider {
        #[serde(skip_serializing_if = "Option::is_none")]
        key: Option<Key>,
    },
}

impl UiNode {
    pub(crate) fn text(key: Option<Key>, text: impl Into<String>) -> Self {
        Self::Text {
            key,
            props: TextProps { text: text.into() },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkProps {
    pub(crate) href: LinkHref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
}

/// An absolute HTTP(S) destination safe to embed in a terminal hyperlink.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct LinkHref(String);

impl TryFrom<String> for LinkHref {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

impl TryFrom<&str> for LinkHref {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        crate::terminal_hyperlinks::web_destination(value)
            .filter(|destination| destination == value)
            .map(Self)
            .ok_or("link href must be an absolute HTTP(S) URL without control characters")
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Key {
    String(String),
    Number(serde_json::Number),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextProps {
    pub(crate) text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StackProps {
    /// Numeric spacing uses the profile's four-point scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) gap: Option<f64>,
}
