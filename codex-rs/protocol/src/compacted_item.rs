use crate::models::ResponseItem;
use crate::protocol::CompactedItem;
use serde::Deserialize;

// Before `window_number` was introduced, the numeric window number was serialized as
// `window_id`. Accept that shape so existing rollouts remain resumable.
impl<'de> Deserialize<'de> for CompactedItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let serialized = SerializedCompactedItem::deserialize(deserializer)?;
        let mut window_number = serialized.window_number;
        let window_id = match serialized.window_id {
            Some(SerializedWindowId::Id(window_id)) => Some(window_id),
            Some(SerializedWindowId::LegacyWindowNumber(legacy_window_number)) => {
                window_number.get_or_insert(legacy_window_number);
                None
            }
            None => None,
        };
        Ok(Self {
            message: serialized.message,
            replacement_history: serialized.replacement_history,
            window_number,
            window_id,
        })
    }
}

#[derive(Deserialize)]
struct SerializedCompactedItem {
    message: String,
    #[serde(default)]
    replacement_history: Option<Vec<ResponseItem>>,
    #[serde(default)]
    window_number: Option<u64>,
    #[serde(default)]
    window_id: Option<SerializedWindowId>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SerializedWindowId {
    Id(String),
    LegacyWindowNumber(u64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn serializes_window_number_and_id() -> Result<()> {
        let item = CompactedItem {
            message: "summary".to_string(),
            replacement_history: None,
            window_number: Some(3),
            window_id: Some("019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001".to_string()),
        };

        assert_eq!(
            serde_json::to_value(item)?,
            json!({
                "message": "summary",
                "window_number": 3,
                "window_id": "019b3f6e-7a10-7cc3-8b6e-1d09e2f7a001",
            })
        );
        Ok(())
    }

    #[test]
    fn migrates_legacy_numeric_window_id() -> Result<()> {
        let item = serde_json::from_value::<CompactedItem>(json!({
            "message": "summary",
            "window_id": 3,
        }))?;

        assert_eq!(
            item,
            CompactedItem {
                message: "summary".to_string(),
                replacement_history: None,
                window_number: Some(3),
                window_id: None,
            }
        );
        Ok(())
    }
}
