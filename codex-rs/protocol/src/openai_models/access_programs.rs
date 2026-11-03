//! Caller-specific explicit access programs advertised by the model catalog.
//!
//! Discovery metadata does not grant access; inference still enforces authorization.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use ts_rs::TS;

use crate::turn_input::CyberAccessProgram;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelAccessPrograms {
    /// Accepted explicit selections. An empty list is distinct from missing metadata.
    #[serde(deserialize_with = "deserialize_known_cyber_access_programs")]
    pub cyber: Vec<CyberAccessProgram>,
}

/// Saved preference to resolve against the selected model's advertised programs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedDaybreakChoice {
    Unset,
    On,
    Off,
}

impl From<Option<bool>> for SavedDaybreakChoice {
    fn from(enabled: Option<bool>) -> Self {
        match enabled {
            Some(true) => Self::On,
            Some(false) => Self::Off,
            None => Self::Unset,
        }
    }
}

impl ModelAccessPrograms {
    /// Select only an advertised program for explicit saved intent. Unset and
    /// unsupported choices remain omitted, including On without Daybreak support.
    pub fn select_saved(&self, choice: SavedDaybreakChoice) -> Option<CyberAccessProgram> {
        match choice {
            SavedDaybreakChoice::On => self.daybreak(),
            SavedDaybreakChoice::Off => self.standard(),
            SavedDaybreakChoice::Unset => None,
        }
    }

    /// Select the preferred Daybreak treatment advertised for this model.
    pub fn daybreak(&self) -> Option<CyberAccessProgram> {
        [
            CyberAccessProgram::DaybreakBlue,
            CyberAccessProgram::DaybreakRed,
        ]
        .into_iter()
        .find(|program| self.cyber.contains(program))
    }

    /// Select the standard treatment when it is advertised.
    pub fn standard(&self) -> Option<CyberAccessProgram> {
        self.cyber
            .contains(&CyberAccessProgram::Standard)
            .then_some(CyberAccessProgram::Standard)
    }
}

fn deserialize_known_cyber_access_programs<'de, D>(
    deserializer: D,
) -> Result<Vec<CyberAccessProgram>, D::Error>
where
    D: Deserializer<'de>,
{
    // New server programs must not prevent older clients from loading the catalog.
    Ok(Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .filter_map(|program| serde_json::from_value(serde_json::Value::String(program)).ok())
        .collect())
}

#[cfg(test)]
#[path = "access_programs_tests.rs"]
mod tests;
