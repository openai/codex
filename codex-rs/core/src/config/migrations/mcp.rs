use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use tempfile::NamedTempFile;
use toml::Value as TomlValue;
use toml_edit::DocumentMut;

use super::super::CONFIG_TOML_FILE;
use super::super::load_config_as_toml;

/// Number of historical config backups retained during migration.
pub const BACKUP_RETENTION: usize = 3;

/// Options controlling MCP schema migration behaviour.
#[derive(Debug, Clone, Copy)]
pub struct MigrationOptions {
    pub dry_run: bool,
    pub force: bool,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            dry_run: true,
            force: false,
        }
    }
}

/// Outcome of a migration attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationReport {
    pub backed_up: bool,
    pub changes_detected: bool,
    pub from_version: u32,
    pub to_version: u32,
    pub notes: Vec<String>,
}

impl MigrationReport {
    fn unchanged(version: u32, note: impl Into<String>) -> Self {
        Self {
            backed_up: false,
            changes_detected: false,
            from_version: version,
            to_version: version,
            notes: vec![note.into()],
        }
    }
}

/// Result of creating/rotating configuration backups.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackupOutcome {
    pub created: bool,
    pub rotated: bool,
    pub backup_path: Option<PathBuf>,
}

/// Performs a best-effort migration of MCP-related configuration to schema version 2.
///
/// Behaviour:
/// * Inspects current `mcp_schema_version` (default = 1 when absent).
/// * If already at or above the target version and `force` is false, returns early.
/// * For dry-run, reports whether a change would occur without touching disk.
/// * For apply (`dry_run = false`), rotates backups and writes an updated config
///   with `mcp_schema_version = 2` (placeholder transformation for now).
pub fn migrate_to_v2(
    codex_home: &Path,
    options: &MigrationOptions,
) -> std::io::Result<MigrationReport> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    if !config_path.exists() {
        return Ok(MigrationReport::unchanged(
            1,
            "config.toml not found; nothing to migrate",
        ));
    }

    let root_value = load_config_as_toml(codex_home)?;
    let current_version = root_value
        .get("mcp_schema_version")
        .and_then(TomlValue::as_integer)
        .map(|v| v.max(0) as u32)
        .unwrap_or(1);

    if current_version >= 2 && !options.force {
        return Ok(MigrationReport::unchanged(
            current_version,
            "mcp_schema_version already at or above 2",
        ));
    }

    if options.dry_run {
        return Ok(MigrationReport {
            backed_up: false,
            changes_detected: current_version < 2 || options.force,
            from_version: current_version,
            to_version: 2,
            notes: vec!["dry-run: no changes applied".into()],
        });
    }

    let backup = create_backup_with_rotation(codex_home)?;
    let mut doc = load_config_as_document(&config_path)?;

    doc["mcp_schema_version"] = toml_edit::value(2);

    write_document_atomic(codex_home, &config_path, doc)?;

    Ok(MigrationReport {
        backed_up: backup.created,
        changes_detected: true,
        from_version: current_version,
        to_version: 2,
        notes: vec![format!(
            "mcp_schema_version updated from {} to 2",
            current_version
        )],
    })
}

/// Creates `config.toml.bak{N}` backups, rotating existing snapshots up to [`BACKUP_RETENTION`].
pub fn create_backup_with_rotation(codex_home: &Path) -> std::io::Result<BackupOutcome> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    if !config_path.exists() {
        return Ok(BackupOutcome::default());
    }

    fs::create_dir_all(codex_home)?;

    let mut rotated = false;
    for idx in (1..=BACKUP_RETENTION).rev() {
        let src = backup_path(codex_home, idx);
        if !src.exists() {
            continue;
        }
        if idx == BACKUP_RETENTION {
            fs::remove_file(&src)?;
        } else {
            let dst = backup_path(codex_home, idx + 1);
            if dst.exists() {
                fs::remove_file(&dst)?;
            }
            fs::rename(&src, &dst)?;
        }
        rotated = true;
    }

    let bak1 = backup_path(codex_home, 1);
    fs::copy(&config_path, &bak1)?;

    Ok(BackupOutcome {
        created: true,
        rotated,
        backup_path: Some(bak1),
    })
}

fn backup_path(codex_home: &Path, index: usize) -> PathBuf {
    codex_home.join(format!("config.toml.bak{index}"))
}

fn load_config_as_document(path: &Path) -> std::io::Result<DocumentMut> {
    let contents = fs::read_to_string(path)?;
    contents
        .parse::<DocumentMut>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn write_document_atomic(
    codex_home: &Path,
    config_path: &Path,
    doc: DocumentMut,
) -> std::io::Result<()> {
    fs::create_dir_all(codex_home)?;
    let tmp = NamedTempFile::new_in(codex_home)?;
    tmp.as_file().write_all(doc.to_string().as_bytes())?;
    tmp.persist(config_path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn backup_rotation_creates_and_rotates() -> std::io::Result<()> {
        let tmp = TempDir::new()?;
        let codex_home = tmp.path();
        let config_path = codex_home.join(CONFIG_TOML_FILE);
        fs::create_dir_all(codex_home)?;
        fs::write(&config_path, "model = \"gpt-5\"\n")?;

        // First backup
        let outcome1 = create_backup_with_rotation(codex_home)?;
        assert!(outcome1.created);
        assert!(outcome1.backup_path.unwrap().exists());

        // Modify config and create additional backups to trigger rotation.
        fs::write(&config_path, "model = \"o3\"\n")?;
        let _ = create_backup_with_rotation(codex_home)?;
        fs::write(&config_path, "model = \"gpt-4\"\n")?;
        let _ = create_backup_with_rotation(codex_home)?;
        fs::write(&config_path, "model = \"gpt-4.1\"\n")?;
        let outcome4 = create_backup_with_rotation(codex_home)?;
        assert!(outcome4.created);

        let bak1 = backup_path(codex_home, 1);
        let bak2 = backup_path(codex_home, 2);
        let bak3 = backup_path(codex_home, 3);
        assert!(bak1.exists());
        assert!(bak2.exists());
        assert!(bak3.exists());

        Ok(())
    }

    #[test]
    fn dry_run_reports_without_changes() -> std::io::Result<()> {
        let tmp = TempDir::new()?;
        let codex_home = tmp.path();
        let config_path = codex_home.join(CONFIG_TOML_FILE);
        fs::create_dir_all(codex_home)?;
        fs::write(&config_path, "model = \"gpt-5\"\n")?;

        let report = migrate_to_v2(codex_home, &MigrationOptions::default())?;
        assert!(report.changes_detected);
        assert!(report.notes.iter().any(|n| n.contains("dry-run")));
        assert_eq!(report.from_version, 1);
        assert_eq!(report.to_version, 2);

        // Dry run should not create backup or modify file.
        assert!(!backup_path(codex_home, 1).exists());
        let contents = fs::read_to_string(&config_path)?;
        assert!(!contents.contains("mcp_schema_version"));

        Ok(())
    }

    #[test]
    fn migrate_applies_version_and_creates_backup() -> std::io::Result<()> {
        let tmp = TempDir::new()?;
        let codex_home = tmp.path();
        let config_path = codex_home.join(CONFIG_TOML_FILE);
        fs::create_dir_all(codex_home)?;
        fs::write(&config_path, "model = \"gpt-5\"\n")?;

        let options = MigrationOptions {
            dry_run: false,
            force: false,
        };
        let report = migrate_to_v2(codex_home, &options)?;
        assert!(report.changes_detected);
        assert!(report.backed_up);
        assert_eq!(report.to_version, 2);
        assert!(backup_path(codex_home, 1).exists());

        let parsed = load_config_as_toml(codex_home)?;
        assert_eq!(
            parsed
                .get("mcp_schema_version")
                .and_then(TomlValue::as_integer),
            Some(2)
        );

        Ok(())
    }
}
