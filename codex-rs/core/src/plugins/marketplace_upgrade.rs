use super::installed_marketplaces::marketplace_install_root;
use super::validate_marketplace_root;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_config::types::MarketplaceConfig;
use codex_config::types::MarketplaceSourceType;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use tempfile::TempDir;
use tracing::warn;

use crate::config::CONFIG_TOML_FILE;
use crate::config::Config;

const MARKETPLACE_UPGRADE_GIT_TIMEOUT: Duration = Duration::from_secs(30);
const MARKETPLACE_INSTALL_METADATA_FILE: &str = ".codex-marketplace-install.json";

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ConfiguredMarketplaceUpgradeOutcome {
    pub upgraded_roots: Vec<AbsolutePathBuf>,
    pub all_succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfiguredGitMarketplace {
    name: String,
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    last_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
struct ActivatedMarketplaceMetadata {
    source_type: MarketplaceSourceType,
    source: String,
    ref_name: Option<String>,
    sparse_paths: Vec<String>,
    revision: String,
}

pub(super) fn upgrade_configured_git_marketplaces(
    codex_home: &Path,
    config: &Config,
) -> ConfiguredMarketplaceUpgradeOutcome {
    let marketplaces = configured_git_marketplaces(config);
    if marketplaces.is_empty() {
        return ConfiguredMarketplaceUpgradeOutcome {
            all_succeeded: true,
            ..Default::default()
        };
    }

    let install_root = marketplace_install_root(codex_home);
    let mut upgraded_roots = Vec::new();
    let mut all_succeeded = true;
    for marketplace in marketplaces {
        match upgrade_configured_git_marketplace(codex_home, &install_root, &marketplace) {
            Ok(Some(upgraded_root)) => upgraded_roots.push(upgraded_root),
            Ok(None) => {}
            Err(err) => {
                all_succeeded = false;
                warn!(
                    marketplace = marketplace.name,
                    source = marketplace.source,
                    error = %err,
                    "failed to auto-upgrade configured marketplace"
                );
            }
        }
    }

    ConfiguredMarketplaceUpgradeOutcome {
        upgraded_roots,
        all_succeeded,
    }
}

fn configured_git_marketplaces(config: &Config) -> Vec<ConfiguredGitMarketplace> {
    let Some(user_layer) = config.config_layer_stack.get_user_layer() else {
        return Vec::new();
    };
    let Some(marketplaces_value) = user_layer.config.get("marketplaces") else {
        return Vec::new();
    };
    let marketplaces = match marketplaces_value
        .clone()
        .try_into::<HashMap<String, MarketplaceConfig>>()
    {
        Ok(marketplaces) => marketplaces,
        Err(err) => {
            warn!("invalid marketplaces config while preparing auto-upgrade: {err}");
            return Vec::new();
        }
    };

    let mut configured = marketplaces
        .into_iter()
        .filter_map(|(name, marketplace)| configured_git_marketplace_from_config(name, marketplace))
        .collect::<Vec<_>>();
    configured.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    configured
}

fn configured_git_marketplace_from_config(
    name: String,
    marketplace: MarketplaceConfig,
) -> Option<ConfiguredGitMarketplace> {
    let MarketplaceConfig {
        last_updated: _,
        last_revision,
        source_type,
        source,
        ref_name,
        sparse_paths,
    } = marketplace;
    if source_type != Some(MarketplaceSourceType::Git) {
        return None;
    }
    let Some(source) = source else {
        warn!(
            marketplace = name,
            "ignoring configured Git marketplace without source"
        );
        return None;
    };
    Some(ConfiguredGitMarketplace {
        name,
        source,
        ref_name,
        sparse_paths: sparse_paths.unwrap_or_default(),
        last_revision,
    })
}

fn upgrade_configured_git_marketplace(
    codex_home: &Path,
    install_root: &Path,
    marketplace: &ConfiguredGitMarketplace,
) -> Result<Option<AbsolutePathBuf>, String> {
    super::validate_plugin_segment(&marketplace.name, "marketplace name")?;
    let remote_revision = git_remote_revision(
        &marketplace.source,
        marketplace.ref_name.as_deref(),
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    )?;
    let destination = install_root.join(&marketplace.name);
    if destination
        .join(".agents/plugins/marketplace.json")
        .is_file()
        && marketplace.last_revision.as_deref() == Some(remote_revision.as_str())
        && activated_marketplace_metadata_matches(&destination, marketplace, &remote_revision)
    {
        return Ok(None);
    }

    let staging_parent = install_root.join(".staging");
    std::fs::create_dir_all(&staging_parent).map_err(|err| {
        format!(
            "failed to create marketplace upgrade staging directory {}: {err}",
            staging_parent.display()
        )
    })?;
    let staged_dir = tempfile::Builder::new()
        .prefix("marketplace-upgrade-")
        .tempdir_in(&staging_parent)
        .map_err(|err| {
            format!(
                "failed to create temporary marketplace upgrade directory in {}: {err}",
                staging_parent.display()
            )
        })?;

    let activated_revision = clone_git_source(
        &marketplace.source,
        marketplace.ref_name.as_deref(),
        &marketplace.sparse_paths,
        staged_dir.path(),
        MARKETPLACE_UPGRADE_GIT_TIMEOUT,
    )?;
    let marketplace_name = validate_marketplace_root(staged_dir.path())
        .map_err(|err| format!("failed to validate upgraded marketplace root: {err}"))?;
    if marketplace_name != marketplace.name {
        return Err(format!(
            "upgraded marketplace name `{marketplace_name}` does not match configured marketplace `{}`",
            marketplace.name
        ));
    }
    write_activated_marketplace_metadata(staged_dir.path(), marketplace, &activated_revision)?;

    let last_updated = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let update = MarketplaceConfigUpdate {
        last_updated: &last_updated,
        last_revision: Some(&activated_revision),
        source_type: "git",
        source: &marketplace.source,
        ref_name: marketplace.ref_name.as_deref(),
        sparse_paths: &marketplace.sparse_paths,
    };
    activate_marketplace_root(&destination, staged_dir, || {
        ensure_configured_git_marketplace_unchanged(codex_home, marketplace)?;
        record_user_marketplace(codex_home, &marketplace.name, &update).map_err(|err| {
            format!(
                "failed to record upgraded marketplace `{}` in user config.toml: {err}",
                marketplace.name
            )
        })
    })?;

    AbsolutePathBuf::try_from(destination)
        .map(Some)
        .map_err(|err| format!("upgraded marketplace path is not absolute: {err}"))
}

fn activated_marketplace_metadata_matches(
    root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> bool {
    let metadata = match std::fs::read_to_string(activated_marketplace_metadata_path(root)) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    let metadata = match serde_json::from_str::<ActivatedMarketplaceMetadata>(&metadata) {
        Ok(metadata) => metadata,
        Err(err) => {
            warn!(
                marketplace = marketplace.name,
                error = %err,
                "failed to parse activated marketplace metadata"
            );
            return false;
        }
    };
    metadata == activated_marketplace_metadata(marketplace, revision)
}

fn write_activated_marketplace_metadata(
    root: &Path,
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> Result<(), String> {
    let metadata = activated_marketplace_metadata(marketplace, revision);
    let contents = serde_json::to_string_pretty(&metadata)
        .map_err(|err| format!("failed to serialize activated marketplace metadata: {err}"))?;
    std::fs::write(activated_marketplace_metadata_path(root), contents)
        .map_err(|err| format!("failed to write activated marketplace metadata: {err}"))
}

fn activated_marketplace_metadata(
    marketplace: &ConfiguredGitMarketplace,
    revision: &str,
) -> ActivatedMarketplaceMetadata {
    ActivatedMarketplaceMetadata {
        source_type: MarketplaceSourceType::Git,
        source: marketplace.source.clone(),
        ref_name: marketplace.ref_name.clone(),
        sparse_paths: marketplace.sparse_paths.clone(),
        revision: revision.to_string(),
    }
}

fn activated_marketplace_metadata_path(root: &Path) -> PathBuf {
    root.join(MARKETPLACE_INSTALL_METADATA_FILE)
}

fn ensure_configured_git_marketplace_unchanged(
    codex_home: &Path,
    expected: &ConfiguredGitMarketplace,
) -> Result<(), String> {
    let current = read_configured_git_marketplace(codex_home, &expected.name)?;
    match current {
        Some(current) if current == *expected => Ok(()),
        Some(_) => Err(format!(
            "configured marketplace `{}` changed while auto-upgrade was in flight",
            expected.name
        )),
        None => Err(format!(
            "configured marketplace `{}` was removed or is no longer a Git marketplace",
            expected.name
        )),
    }
}

fn read_configured_git_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
) -> Result<Option<ConfiguredGitMarketplace>, String> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let raw_config = match std::fs::read_to_string(&config_path) {
        Ok(raw_config) => raw_config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "failed to read user config {} while checking marketplace auto-upgrade: {err}",
                config_path.display()
            ));
        }
    };
    let config: toml::Value = toml::from_str(&raw_config).map_err(|err| {
        format!(
            "failed to parse user config {} while checking marketplace auto-upgrade: {err}",
            config_path.display()
        )
    })?;
    let Some(marketplaces_value) = config.get("marketplaces") else {
        return Ok(None);
    };
    let mut marketplaces = marketplaces_value
        .clone()
        .try_into::<HashMap<String, MarketplaceConfig>>()
        .map_err(|err| format!("invalid marketplaces config while checking auto-upgrade: {err}"))?;
    let Some(marketplace) = marketplaces.remove(marketplace_name) else {
        return Ok(None);
    };
    Ok(configured_git_marketplace_from_config(
        marketplace_name.to_string(),
        marketplace,
    ))
}

fn git_remote_revision(
    source: &str,
    ref_name: Option<&str>,
    timeout: Duration,
) -> Result<String, String> {
    if let Some(ref_name) = ref_name
        && is_full_git_sha(ref_name)
    {
        return Ok(ref_name.to_string());
    }

    let ref_name = ref_name.unwrap_or("HEAD");
    let output = run_git_command_with_timeout(
        git_command().arg("ls-remote").arg(source).arg(ref_name),
        "git ls-remote marketplace source",
        timeout,
    )?;
    ensure_git_success(&output, "git ls-remote marketplace source")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some(first_line) = stdout.lines().next() else {
        return Err("git ls-remote returned empty output for marketplace source".to_string());
    };
    let Some((revision, _)) = first_line.split_once('\t') else {
        return Err(format!(
            "unexpected git ls-remote output for marketplace source: {first_line}"
        ));
    };
    let revision = revision.trim();
    if revision.is_empty() {
        return Err("git ls-remote returned empty revision for marketplace source".to_string());
    }
    Ok(revision.to_string())
}

fn is_full_git_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn clone_git_source(
    source: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    destination: &Path,
    timeout: Duration,
) -> Result<String, String> {
    if sparse_paths.is_empty() {
        let output = run_git_command_with_timeout(
            git_command().arg("clone").arg(source).arg(destination),
            "git clone marketplace source",
            timeout,
        )?;
        ensure_git_success(&output, "git clone marketplace source")?;
        if let Some(ref_name) = ref_name {
            let output = run_git_command_with_timeout(
                git_command()
                    .arg("-C")
                    .arg(destination)
                    .arg("checkout")
                    .arg(ref_name),
                "git checkout marketplace ref",
                timeout,
            )?;
            ensure_git_success(&output, "git checkout marketplace ref")?;
        }
        return git_worktree_revision(destination, timeout);
    }

    let output = run_git_command_with_timeout(
        git_command()
            .arg("clone")
            .arg("--filter=blob:none")
            .arg("--no-checkout")
            .arg(source)
            .arg(destination),
        "git clone marketplace source",
        timeout,
    )?;
    ensure_git_success(&output, "git clone marketplace source")?;

    let mut sparse_checkout = git_command();
    sparse_checkout
        .arg("-C")
        .arg(destination)
        .arg("sparse-checkout")
        .arg("set")
        .args(sparse_paths);
    let output = run_git_command_with_timeout(
        &mut sparse_checkout,
        "git sparse-checkout marketplace source",
        timeout,
    )?;
    ensure_git_success(&output, "git sparse-checkout marketplace source")?;

    let output = run_git_command_with_timeout(
        git_command()
            .arg("-C")
            .arg(destination)
            .arg("checkout")
            .arg(ref_name.unwrap_or("HEAD")),
        "git checkout marketplace ref",
        timeout,
    )?;
    ensure_git_success(&output, "git checkout marketplace ref")?;
    git_worktree_revision(destination, timeout)
}

fn git_worktree_revision(destination: &Path, timeout: Duration) -> Result<String, String> {
    let output = run_git_command_with_timeout(
        git_command()
            .arg("-C")
            .arg(destination)
            .arg("rev-parse")
            .arg("HEAD"),
        "git rev-parse marketplace revision",
        timeout,
    )?;
    ensure_git_success(&output, "git rev-parse marketplace revision")?;

    let revision = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if revision.is_empty() {
        Err("git rev-parse returned empty revision for marketplace source".to_string())
    } else {
        Ok(revision)
    }
}

fn git_command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn activate_marketplace_root(
    destination: &Path,
    staged_dir: TempDir,
    after_activate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let staged_root = staged_dir.path();
    let Some(parent) = destination.parent() else {
        return Err(format!(
            "failed to determine marketplace install parent for {}",
            destination.display()
        ));
    };
    std::fs::create_dir_all(parent).map_err(|err| {
        format!(
            "failed to create marketplace install parent {}: {err}",
            parent.display()
        )
    })?;

    if destination.exists() {
        let backup_dir = tempfile::Builder::new()
            .prefix("marketplace-backup-")
            .tempdir_in(parent)
            .map_err(|err| {
                format!(
                    "failed to create marketplace backup directory in {}: {err}",
                    parent.display()
                )
            })?;
        let backup_root = backup_dir.path().join("root");
        std::fs::rename(destination, &backup_root).map_err(|err| {
            format!(
                "failed to move previous marketplace root out of the way at {}: {err}",
                destination.display()
            )
        })?;

        if let Err(err) = std::fs::rename(staged_root, destination) {
            let rollback_result = std::fs::rename(&backup_root, destination);
            return match rollback_result {
                Ok(()) => Err(format!(
                    "failed to activate upgraded marketplace at {}: {err}",
                    destination.display()
                )),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("root");
                    Err(format!(
                        "failed to activate upgraded marketplace at {}: {err}; failed to restore previous marketplace root (left at {}): {rollback_err}",
                        destination.display(),
                        backup_path.display()
                    ))
                }
            };
        }

        if let Err(err) = after_activate() {
            let remove_result = std::fs::remove_dir_all(destination);
            let rollback_result =
                remove_result.and_then(|()| std::fs::rename(&backup_root, destination));
            return match rollback_result {
                Ok(()) => Err(err),
                Err(rollback_err) => {
                    let backup_path = backup_dir.keep().join("root");
                    Err(format!(
                        "{err}; failed to restore previous marketplace root at {} (left at {}): {rollback_err}",
                        destination.display(),
                        backup_path.display()
                    ))
                }
            };
        }
    } else {
        std::fs::rename(staged_root, destination).map_err(|err| {
            format!(
                "failed to activate upgraded marketplace at {}: {err}",
                destination.display()
            )
        })?;
        if let Err(err) = after_activate() {
            let remove_result = std::fs::remove_dir_all(destination);
            return match remove_result {
                Ok(()) => Err(err),
                Err(remove_err) => Err(format!(
                    "{err}; failed to remove newly activated marketplace root at {}: {remove_err}",
                    destination.display()
                )),
            };
        }
    }

    Ok(())
}

fn run_git_command_with_timeout(
    command: &mut Command,
    context: &str,
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to run {context}: {err}"))?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|err| format!("failed to wait for {context}: {err}"));
            }
            Ok(None) => {}
            Err(err) => return Err(format!("failed to poll {context}: {err}")),
        }

        if start.elapsed() >= timeout {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .map_err(|err| format!("failed to wait for {context} after timeout: {err}"))?;
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return if stderr.is_empty() {
                Err(format!("{context} timed out after {}s", timeout.as_secs()))
            } else {
                Err(format!(
                    "{context} timed out after {}s: {stderr}",
                    timeout.as_secs()
                ))
            };
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn ensure_git_success(output: &Output, context: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("{context} failed with status {}", output.status))
    } else {
        Err(format!(
            "{context} failed with status {}: {stderr}",
            output.status
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_TOML_FILE;
    use crate::plugins::test_support::load_plugins_config;
    use crate::plugins::test_support::write_file;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_installs_new_revision() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: vec![
                    AbsolutePathBuf::try_from(
                        marketplace_install_root(codex_home.path()).join("debug")
                    )
                    .unwrap()
                ],
                all_succeeded: true,
            }
        );
        assert_eq!(
            std::fs::read_to_string(
                marketplace_install_root(codex_home.path()).join("debug/plugins/sample/marker.txt")
            )
            .unwrap(),
            "new"
        );
        let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap();
        assert!(config.contains(&format!(r#"last_revision = "{revision}""#)));
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_skips_matching_revision() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_installed_metadata(&installed_root, source_repo.path(), None, &[], &revision);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), &revision),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: Vec::new(),
                all_succeeded: true,
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_reclones_when_install_metadata_differs() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let revision = git_output(source_repo.path(), &["rev-parse", "HEAD"]);
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_installed_metadata(&installed_root, source_repo.path(), None, &[], &revision);
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config_with_ref(source_repo.path(), &revision, &revision),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: vec![
                    AbsolutePathBuf::try_from(
                        marketplace_install_root(codex_home.path()).join("debug")
                    )
                    .unwrap()
                ],
                all_succeeded: true,
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "new"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_keeps_existing_root_on_name_mismatch() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "other", "new");
        init_git_repo(source_repo.path());
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: Vec::new(),
                all_succeeded: false,
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_keeps_existing_root_on_git_failure() {
        let codex_home = TempDir::new().unwrap();
        let missing_repo = codex_home.path().join("missing-repo");
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(&missing_repo, "old-revision"),
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: Vec::new(),
                all_succeeded: false,
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplace_rolls_back_when_config_changes() {
        let codex_home = TempDir::new().unwrap();
        let source_repo = TempDir::new().unwrap();
        write_marketplace_repo(source_repo.path(), "debug", "new");
        init_git_repo(source_repo.path());
        let changed_source_repo = TempDir::new().unwrap();
        write_marketplace_repo(changed_source_repo.path(), "debug", "changed");
        init_git_repo(changed_source_repo.path());
        let installed_root = marketplace_install_root(codex_home.path()).join("debug");
        write_marketplace_repo(&installed_root, "debug", "old");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(source_repo.path(), "old-revision"),
        );
        let config = load_plugins_config(codex_home.path()).await;
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            &marketplace_config(changed_source_repo.path(), "changed-revision"),
        );

        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: Vec::new(),
                all_succeeded: false,
            }
        );
        assert_eq!(
            std::fs::read_to_string(installed_root.join("plugins/sample/marker.txt")).unwrap(),
            "old"
        );
        let config = std::fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE)).unwrap();
        assert!(config.contains(&changed_source_repo.path().display().to_string()));
        assert!(config.contains(r#"last_revision = "changed-revision""#));
    }

    #[tokio::test]
    async fn upgrade_configured_git_marketplaces_ignores_local_unconfigured_marketplace() {
        let codex_home = TempDir::new().unwrap();
        write_marketplace_repo(codex_home.path(), "local", "local");
        write_file(
            &codex_home.path().join(CONFIG_TOML_FILE),
            r#"[features]
plugins = true
"#,
        );

        let config = load_plugins_config(codex_home.path()).await;
        let outcome = upgrade_configured_git_marketplaces(codex_home.path(), &config);

        assert_eq!(
            outcome,
            ConfiguredMarketplaceUpgradeOutcome {
                upgraded_roots: Vec::new(),
                all_succeeded: true,
            }
        );
        assert!(
            !marketplace_install_root(codex_home.path())
                .join("local")
                .exists()
        );
    }

    #[test]
    fn full_git_sha_ref_is_already_a_remote_revision() {
        assert!(is_full_git_sha("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_full_git_sha("main"));
        assert!(!is_full_git_sha("0123456"));
    }

    fn marketplace_config(source_repo: &Path, last_revision: &str) -> String {
        format!(
            r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T00:00:00Z"
last_revision = "{last_revision}"
source_type = "git"
source = "{}"
"#,
            source_repo.display()
        )
    }

    fn marketplace_config_with_ref(
        source_repo: &Path,
        last_revision: &str,
        ref_name: &str,
    ) -> String {
        format!(
            r#"[features]
plugins = true

[marketplaces.debug]
last_updated = "2026-04-10T00:00:00Z"
last_revision = "{last_revision}"
source_type = "git"
source = "{}"
ref = "{ref_name}"
"#,
            source_repo.display()
        )
    }

    fn write_installed_metadata(
        root: &Path,
        source_repo: &Path,
        ref_name: Option<&str>,
        sparse_paths: &[String],
        revision: &str,
    ) {
        let marketplace = ConfiguredGitMarketplace {
            name: "debug".to_string(),
            source: source_repo.display().to_string(),
            ref_name: ref_name.map(str::to_string),
            sparse_paths: sparse_paths.to_vec(),
            last_revision: Some(revision.to_string()),
        };
        write_activated_marketplace_metadata(root, &marketplace, revision)
            .expect("metadata should write");
    }

    fn write_marketplace_repo(root: &Path, marketplace_name: &str, marker: &str) {
        write_file(
            &root.join(".agents/plugins/marketplace.json"),
            &format!(
                r#"{{
  "name": "{marketplace_name}",
  "plugins": [
    {{
      "name": "sample",
      "source": {{
        "source": "local",
        "path": "./plugins/sample"
      }}
    }}
  ]
}}"#
            ),
        );
        write_file(
            &root.join("plugins/sample/.codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        );
        write_file(&root.join("plugins/sample/marker.txt"), marker);
    }

    fn init_git_repo(repo: &Path) {
        git(repo, &["init"]);
        git(repo, &["config", "user.email", "codex-test@example.com"]);
        git(repo, &["config", "user.name", "Codex Test"]);
        git(repo, &["add", "."]);
        git(repo, &["commit", "-m", "initial marketplace"]);
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_output(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git should run");
        assert!(
            output.status.success(),
            "git -C {} {} failed\nstdout:\n{}\nstderr:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}
