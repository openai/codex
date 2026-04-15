use std::io;
use std::path::Path;

use toml::Value as TomlValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalityMigrationStatus {
    Applied,
    SkippedMarker,
    SkippedExplicitPersonality,
    SkippedNoSessions,
}

pub async fn maybe_migrate_personality(
    _codex_home: &Path,
    _config_toml: &TomlValue,
) -> io::Result<PersonalityMigrationStatus> {
    Ok(PersonalityMigrationStatus::SkippedNoSessions)
}
