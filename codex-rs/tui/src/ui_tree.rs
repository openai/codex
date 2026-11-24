//! Display elements for the terminal UI and their JSON representation.

mod decode;
mod render;
mod theme;

pub(crate) use render::PreparedUiTree;
pub(crate) use render::sanitize_leaf;
use theme::Color;
use theme::TextWeight;

#[cfg(test)]
#[path = "ui_tree_tests.rs"]
mod tests;

use serde::Deserialize;
use serde::Serialize;

const MAX_GAP: u64 = (1 << 53) - 1;
const MAX_TEXT_BYTES: usize = 16 * 1024;

/// A nullable root. JSON is limited to 64 KiB, 256 non-null nodes, depth 16 (root at 0),
/// and 16 KiB of UTF-8 text across all string leaves.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
pub(crate) struct UiTree(pub(crate) Option<UiNode>);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", rename = "Element")
)]
pub(crate) enum UiNode {
    Text {
        #[serde(default, skip_serializing_if = "is_default")]
        #[cfg_attr(test, ts(as = "Option<TextProps>", optional))]
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
    Col {
        props: ColProps,
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
    /// An inline span with preserved whitespace and its own theme colors.
    Code {
        children: Vec<Option<UiNode>>,
    },
    #[serde(untagged)]
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
pub(crate) struct ColProps {
    /// Blank terminal rows between flex items, as a non-negative JavaScript-safe integer.
    /// Adjacent strings form one item. Separate elements each take a slot, even when empty.
    #[serde(deserialize_with = "decode::gap")]
    #[cfg_attr(test, ts(type = "number"))]
    gap: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts", optional_fields = nullable))]
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
    /// Preserve spaces and tabs across styled children.
    #[serde(rename = "preserveWhitespace", skip_serializing_if = "is_default")]
    #[cfg_attr(test, ts(as = "Option<bool>", optional))]
    preserve_whitespace: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts", optional_fields = nullable))]
pub(crate) struct LinkProps {
    pub(crate) href: LinkHref,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
}

/// An absolute HTTP(S) URL without control characters, at most 8 KiB in UTF-8.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
#[cfg_attr(
    test,
    derive(ts_rs::TS),
    ts(export_to = "statusline.d.ts", type = "string")
)]
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

/// A root row receives the available footer width. A row inside another row takes up to its content width.
/// An explicit `justify` groups adjacent bare strings into one item. Use separate `text` or `row`
/// elements to position fields independently. Omitting `justify` keeps gaps between authored children.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts", optional_fields = nullable))]
pub(crate) struct RowProps {
    /// Horizontal spacing in terminal columns, expressed as a non-negative JavaScript-safe integer.
    #[serde(deserialize_with = "decode::gap")]
    #[cfg_attr(test, ts(type = "number"))]
    pub(crate) gap: u64,
    #[serde(
        default,
        deserialize_with = "decode::optional_token",
        skip_serializing_if = "Option::is_none"
    )]
    background: Option<Color>,
    #[serde(
        default,
        deserialize_with = "decode::token",
        skip_serializing_if = "is_default"
    )]
    #[cfg_attr(test, ts(as = "Option<RowWrap>", optional))]
    wrap: RowWrap,
    /// Position each line's items at the `start`, `center`, or `end`.
    /// `between` distributes spare columns across the gaps between children.
    #[serde(
        default,
        deserialize_with = "decode::present_token",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(test, ts(optional))]
    justify: Option<RowJustify>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
enum RowWrap {
    #[default]
    Nowrap,
    Wrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export_to = "statusline.d.ts"))]
enum RowJustify {
    #[default]
    Start,
    Center,
    End,
    Between,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
