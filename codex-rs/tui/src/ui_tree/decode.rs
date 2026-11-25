//! Enforce the input byte limit before parsing and the UI tree limits after deserialization.

use super::MAX_SAFE_INTEGER;
use super::MAX_TEXT_BYTES;
use super::UiNode;
use super::UiTree;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeOwned;
use serde::de::Error;
use serde::de::IntoDeserializer;

impl std::str::FromStr for UiTree {
    type Err = serde_json::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() > 64 * 1024 {
            return Err(Self::Err::custom("UI tree JSON exceeds 64 KiB"));
        }
        let root: Option<UiNode> = serde_json::from_str(text)?;
        let mut pending: Vec<_> = root.iter().map(|node| (node, 0)).collect();
        let mut nodes = 0;
        let mut text_bytes = 0;
        while let Some((node, depth)) = pending.pop() {
            // A badge stores its string child directly in the node.
            let label = usize::from(matches!(node, UiNode::Badge { .. }));
            if depth + label > 16 {
                return Err(Self::Err::custom("UI tree exceeds maximum depth 16"));
            }
            nodes += 1 + label;
            if nodes > 256 {
                return Err(Self::Err::custom("UI tree exceeds 256 non-null nodes"));
            }
            match node {
                UiNode::Spacer { .. } => {}
                UiNode::String(text)
                | UiNode::Badge {
                    children: [text], ..
                } => {
                    text_bytes += text.len();
                    if text_bytes > MAX_TEXT_BYTES {
                        return Err(Self::Err::custom("UI tree exceeds 16 KiB of text"));
                    }
                }
                UiNode::Text { children, .. }
                | UiNode::Code { children }
                | UiNode::Bold { children }
                | UiNode::Italic { children }
                | UiNode::Underline { children }
                | UiNode::Link { children, .. }
                | UiNode::Row { children, .. }
                | UiNode::Box { children, .. }
                | UiNode::Col { children, .. } => {
                    pending.extend(children.iter().flatten().map(|child| (child, depth + 1)));
                }
            }
        }
        Ok(Self(root))
    }
}

pub(super) fn optional_token<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<String>::deserialize(d)?
        .map(|s| T::deserialize(s.into_deserializer()))
        .transpose()
}

pub(super) fn token<'de, D: Deserializer<'de>, T: DeserializeOwned>(d: D) -> Result<T, D::Error> {
    T::deserialize(String::deserialize(d)?.into_deserializer())
}

// A present token must be a string. Serde defaults an omitted property to `None`.
pub(super) fn present_token<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    d: D,
) -> Result<Option<T>, D::Error> {
    token(d).map(Some)
}

pub(super) fn max_lines<'de, D: Deserializer<'de>>(de: D) -> Result<Option<u64>, D::Error> {
    let lines = u64::deserialize(de)?;
    if !(1..=MAX_SAFE_INTEGER).contains(&lines) {
        return Err(D::Error::custom("maxLines must be a positive safe integer"));
    }
    Ok(Some(lines))
}

pub(super) fn gap<'de, D: Deserializer<'de>>(de: D) -> Result<u64, D::Error> {
    let gap = u64::deserialize(de)?;
    if gap > MAX_SAFE_INTEGER {
        return Err(D::Error::custom("gap exceeds the maximum safe integer"));
    }
    Ok(gap)
}
