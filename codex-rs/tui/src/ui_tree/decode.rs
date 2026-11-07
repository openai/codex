//! Enforce the input byte limit before parsing and the UI tree limits after deserialization.

use super::MAX_GAP;
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
            if depth > 16 {
                return Err(Self::Err::custom("UI tree exceeds maximum depth 16"));
            }
            nodes += 1;
            if nodes > 256 {
                return Err(Self::Err::custom("UI tree exceeds 256 non-null nodes"));
            }
            match node {
                UiNode::String(text) => {
                    text_bytes += text.len();
                    if text_bytes > MAX_TEXT_BYTES {
                        return Err(Self::Err::custom("UI tree exceeds 16 KiB of text"));
                    }
                }
                UiNode::Text { children, .. }
                | UiNode::Bold { children }
                | UiNode::Italic { children }
                | UiNode::Underline { children }
                | UiNode::Link { children, .. }
                | UiNode::Row { children, .. } => {
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

pub(super) fn gap<'de, D: Deserializer<'de>>(de: D) -> Result<u64, D::Error> {
    let gap = u64::deserialize(de)?;
    if gap > MAX_GAP {
        return Err(D::Error::custom("gap exceeds the maximum safe integer"));
    }
    Ok(gap)
}
