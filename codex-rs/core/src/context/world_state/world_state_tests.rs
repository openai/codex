use super::*;
use anyhow::Result;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct AlphaSection {
    value: String,
}

impl AlphaSection {
    fn new(value: &str) -> Self {
        Self {
            value: value.to_string(),
        }
    }
}

impl WorldStateSection for AlphaSection {
    const NAME: &'static str = "alpha";

    fn render_diff(&self, _previous: &Self) -> Option<ResponseItem> {
        None
    }
}

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct BetaSection {
    value: String,
}

impl BetaSection {
    fn new(value: &str) -> Self {
        Self {
            value: value.to_string(),
        }
    }
}

impl WorldStateSection for BetaSection {
    const NAME: &'static str = "beta";

    fn render_diff(&self, _previous: &Self) -> Option<ResponseItem> {
        None
    }
}

#[test]
fn full_json_reloads_registered_sections() -> Result<()> {
    let mut state = WorldState::default();
    state.add_section(BetaSection::new("two"));
    state.add_section(AlphaSection::new("one"));

    let full = state.json_full()?;
    assert_eq!(
        json!({
            "alpha": { "value": "one" },
            "beta": { "value": "two" },
        }),
        full,
    );

    let loaded = WorldState::from_json(full.clone())?;
    assert_eq!(full, loaded.json_full()?);
    Ok(())
}

#[test]
fn json_diff_reconstructs_current_state() -> Result<()> {
    let mut previous = WorldState::default();
    previous.add_section(AlphaSection::new("old"));
    previous.add_section(BetaSection::new("removed"));
    let mut current = WorldState::default();
    current.add_section(AlphaSection::new("new"));

    let diff = current.json_diff(&previous)?;
    assert_eq!(
        json!({
            "alpha": { "value": "new" },
            "beta": null,
        }),
        diff,
    );

    previous.apply_json_diff(&diff)?;
    assert_eq!(current.json_full()?, previous.json_full()?);
    Ok(())
}
