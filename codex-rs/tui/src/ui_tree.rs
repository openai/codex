//! Display elements for the terminal UI and their JSON representation.

mod decode;
mod render;
mod theme;

use theme::Color;
use theme::TextWeight;

#[cfg(test)]
#[path = "ui_tree_tests.rs"]
mod tests;

use serde::Deserialize;
use serde::Serialize;

const MAX_GAP: u64 = (1 << 53) - 1;
const MAX_TEXT_BYTES: usize = 16 * 1024;

/// A nullable root with bounded depth, node count, and total UTF-8 leaf bytes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiTree(pub(crate) Option<UiNode>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum UiNode {
    Text {
        #[serde(default, skip_serializing_if = "is_default")]
        props: TextProps,
        children: Vec<Option<UiNode>>,
    },
    Link {
        props: LinkProps,
        children: Vec<Option<UiNode>>,
    },
    Row {
        props: RowProps,
        children: Vec<Option<UiNode>>,
    },
    Bold {
        children: Vec<Option<UiNode>>,
    },
    Italic {
        children: Vec<Option<UiNode>>,
    },
    Underline {
        children: Vec<Option<UiNode>>,
    },
    #[serde(untagged)]
    String(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct TextProps {
    #[serde(
        deserialize_with = "decode::optional_token",
        skip_serializing_if = "Option::is_none"
    )]
    color: Option<Color>,
    #[serde(
        deserialize_with = "decode::optional_token",
        skip_serializing_if = "Option::is_none"
    )]
    weight: Option<TextWeight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    underline: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LinkProps {
    pub(crate) href: LinkHref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
}

/// An absolute HTTP(S) URL without control characters, at most 8 KiB in UTF-8.
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
            .ok_or("link href must be an absolute HTTP(S) URL of at most 8 KiB without control characters")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RowProps {
    /// Horizontal spacing in terminal columns, expressed as a non-negative JavaScript-safe integer.
    #[serde(deserialize_with = "decode::gap")]
    pub(crate) gap: u64,
    #[serde(
        default,
        deserialize_with = "decode::optional_token",
        skip_serializing_if = "Option::is_none"
    )]
    background: Option<Color>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
