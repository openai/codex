use super::OPENAI_CURATED_MARKETPLACE_NAME;
use super::marketplace_install_root;
use super::validate_marketplace_root;
use super::validate_plugin_segment;
use codex_config::CONFIG_TOML_FILE;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::Builder;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddRequest {
    pub source: String,
    pub ref_name: Option<String>,
    pub sparse_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddOutcome {
    pub marketplace_name: String,
    pub source_display: String,
    pub installed_root: AbsolutePathBuf,
    pub already_added: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceAddError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarketplaceSource {
    Git {
        url: String,
        ref_name: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MarketplaceInstallMetadata {
    source: InstalledMarketplaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstalledMarketplaceSource {
    Git {
        url: String,
        ref_name: Option<String>,
        sparse_paths: Vec<String>,
    },
}

pub async fn add_marketplace(
    codex_home: PathBuf,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    tokio::task::spawn_blocking(move || add_marketplace_sync(codex_home.as_path(), request))
        .await
        .map_err(|err| MarketplaceAddError::Internal(format!("failed to add marketplace: {err}")))?
}

fn add_marketplace_sync(
    codex_home: &Path,
    request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    add_marketplace_sync_with_cloner(codex_home, request, clone_git_source)
}

fn add_marketplace_sync_with_cloner<F>(
    codex_home: &Path,
    request: MarketplaceAddRequest,
    clone_source: F,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError>
where
    F: Fn(&str, Option<&str>, &[String], &Path) -> Result<(), MarketplaceAddError>,
{
    let MarketplaceAddRequest {
        source,
        ref_name,
        sparse_paths,
    } = request;
    let source = parse_marketplace_source(&source, ref_name)?;

    let install_root = marketplace_install_root(codex_home);
    fs::create_dir_all(&install_root).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to create marketplace install directory {}: {err}",
            install_root.display()
        ))
    })?;

    let install_metadata = MarketplaceInstallMetadata::from_source(&source, &sparse_paths);
    if let Some(existing_root) =
        installed_marketplace_root_for_source(codex_home, &install_root, &install_metadata)?
    {
        let marketplace_name = validate_marketplace_root(&existing_root).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to validate installed marketplace at {}: {err}",
                existing_root.display()
            ))
        })?;
        record_added_marketplace_entry(codex_home, &marketplace_name, &install_metadata)?;
        return Ok(MarketplaceAddOutcome {
            marketplace_name,
            source_display: source.display(),
            installed_root: AbsolutePathBuf::try_from(existing_root).map_err(|err| {
                MarketplaceAddError::Internal(format!(
                    "failed to resolve installed marketplace root: {err}"
                ))
            })?,
            already_added: true,
        });
    }

    let staging_root = marketplace_staging_root(&install_root);
    fs::create_dir_all(&staging_root).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to create marketplace staging directory {}: {err}",
            staging_root.display()
        ))
    })?;
    let staged_root = Builder::new()
        .prefix("marketplace-add-")
        .tempdir_in(&staging_root)
        .map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to create temporary marketplace directory in {}: {err}",
                staging_root.display()
            ))
        })?;
    let staged_root = staged_root.keep();

    let MarketplaceSource::Git { url, ref_name } = &source;
    clone_source(url, ref_name.as_deref(), &sparse_paths, &staged_root)?;

    let marketplace_name = validate_marketplace_source_root(&staged_root)?;
    if marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace '{OPENAI_CURATED_MARKETPLACE_NAME}' is reserved and cannot be added from {}",
            source.display()
        )));
    }

    let destination = install_root.join(safe_marketplace_dir_name(&marketplace_name)?);
    ensure_marketplace_destination_is_inside_install_root(&install_root, &destination)?;
    if destination.exists() {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace '{marketplace_name}' is already added from a different source; remove it before adding {}",
            source.display()
        )));
    }

    replace_marketplace_root(&staged_root, &destination).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to install marketplace at {}: {err}",
            destination.display()
        ))
    })?;
    if let Err(err) =
        record_added_marketplace_entry(codex_home, &marketplace_name, &install_metadata)
    {
        if let Err(rollback_err) = fs::rename(&destination, &staged_root) {
            return Err(MarketplaceAddError::Internal(format!(
                "{err}; additionally failed to roll back installed marketplace at {}: {rollback_err}",
                destination.display()
            )));
        }
        return Err(err);
    }

    Ok(MarketplaceAddOutcome {
        marketplace_name,
        source_display: source.display(),
        installed_root: AbsolutePathBuf::try_from(destination).map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to resolve installed marketplace root: {err}"
            ))
        })?,
        already_added: false,
    })
}

fn record_added_marketplace_entry(
    codex_home: &Path,
    marketplace_name: &str,
    install_metadata: &MarketplaceInstallMetadata,
) -> Result<(), MarketplaceAddError> {
    let source = install_metadata.config_source();
    let timestamp = utc_timestamp_now()?;
    let update = MarketplaceConfigUpdate {
        last_updated: &timestamp,
        source_type: install_metadata.config_source_type(),
        source: &source,
        ref_name: install_metadata.ref_name(),
        sparse_paths: install_metadata.sparse_paths(),
    };

    record_user_marketplace(codex_home, marketplace_name, &update).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to add marketplace '{marketplace_name}' to user config.toml: {err}"
        ))
    })
}

fn installed_marketplace_root_for_source(
    codex_home: &Path,
    install_root: &Path,
    install_metadata: &MarketplaceInstallMetadata,
) -> Result<Option<PathBuf>, MarketplaceAddError> {
    let config_path = codex_home.join(CONFIG_TOML_FILE);
    let config = match fs::read_to_string(&config_path) {
        Ok(config) => config,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(MarketplaceAddError::Internal(format!(
                "failed to read user config {}: {err}",
                config_path.display()
            )));
        }
    };
    let config: toml::Value = toml::from_str(&config).map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to parse user config {}: {err}",
            config_path.display()
        ))
    })?;
    let Some(marketplaces) = config.get("marketplaces").and_then(toml::Value::as_table) else {
        return Ok(None);
    };

    for (marketplace_name, marketplace) in marketplaces {
        if !install_metadata.matches_config(marketplace) {
            continue;
        }
        let root = install_root.join(marketplace_name);
        if validate_marketplace_root(&root).is_ok() {
            return Ok(Some(root));
        }
    }

    Ok(None)
}

impl MarketplaceInstallMetadata {
    fn from_source(source: &MarketplaceSource, sparse_paths: &[String]) -> Self {
        let source = match source {
            MarketplaceSource::Git { url, ref_name } => InstalledMarketplaceSource::Git {
                url: url.clone(),
                ref_name: ref_name.clone(),
                sparse_paths: sparse_paths.to_vec(),
            },
        };
        Self { source }
    }

    fn config_source_type(&self) -> &'static str {
        match &self.source {
            InstalledMarketplaceSource::Git { .. } => "git",
        }
    }

    fn config_source(&self) -> String {
        match &self.source {
            InstalledMarketplaceSource::Git { url, .. } => url.clone(),
        }
    }

    fn ref_name(&self) -> Option<&str> {
        match &self.source {
            InstalledMarketplaceSource::Git { ref_name, .. } => ref_name.as_deref(),
        }
    }

    fn sparse_paths(&self) -> &[String] {
        match &self.source {
            InstalledMarketplaceSource::Git { sparse_paths, .. } => sparse_paths,
        }
    }

    fn matches_config(&self, marketplace: &toml::Value) -> bool {
        marketplace.get("source_type").and_then(toml::Value::as_str)
            == Some(self.config_source_type())
            && marketplace.get("source").and_then(toml::Value::as_str)
                == Some(self.config_source().as_str())
            && marketplace.get("ref").and_then(toml::Value::as_str) == self.ref_name()
            && config_sparse_paths(marketplace) == self.sparse_paths()
    }
}

fn config_sparse_paths(marketplace: &toml::Value) -> Vec<String> {
    marketplace
        .get("sparse_paths")
        .and_then(toml::Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_marketplace_source(
    source: &str,
    explicit_ref: Option<String>,
) -> Result<MarketplaceSource, MarketplaceAddError> {
    let source = source.trim();
    if source.is_empty() {
        return Err(MarketplaceAddError::InvalidRequest(
            "marketplace source must not be empty".to_string(),
        ));
    }

    let (base_source, parsed_ref) = split_source_ref(source);
    let ref_name = explicit_ref.or(parsed_ref);

    if looks_like_local_path(&base_source) {
        return Err(MarketplaceAddError::InvalidRequest(
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo".to_string(),
        ));
    }

    if is_ssh_git_url(&base_source) || is_git_url(&base_source) {
        return Ok(MarketplaceSource::Git {
            url: normalize_git_url(&base_source),
            ref_name,
        });
    }

    if looks_like_github_shorthand(&base_source) {
        return Ok(MarketplaceSource::Git {
            url: format!("https://github.com/{base_source}.git"),
            ref_name,
        });
    }

    Err(MarketplaceAddError::InvalidRequest(format!(
        "invalid marketplace source format: {source}"
    )))
}

fn split_source_ref(source: &str) -> (String, Option<String>) {
    if let Some((base, ref_name)) = source.rsplit_once('#') {
        return (base.to_string(), non_empty_ref(ref_name));
    }
    if !source.contains("://")
        && !is_ssh_git_url(source)
        && let Some((base, ref_name)) = source.rsplit_once('@')
    {
        return (base.to_string(), non_empty_ref(ref_name));
    }
    (source.to_string(), None)
}

fn non_empty_ref(ref_name: &str) -> Option<String> {
    let ref_name = ref_name.trim();
    (!ref_name.is_empty()).then(|| ref_name.to_string())
}

fn normalize_git_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    if url.starts_with("https://github.com/") && !url.ends_with(".git") {
        format!("{url}.git")
    } else {
        url.to_string()
    }
}

fn looks_like_local_path(source: &str) -> bool {
    source.starts_with("./")
        || source.starts_with("../")
        || source.starts_with('/')
        || source.starts_with("~/")
        || source == "."
        || source == ".."
}

fn is_ssh_git_url(source: &str) -> bool {
    source.starts_with("ssh://") || source.starts_with("git@") && source.contains(':')
}

fn is_git_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

fn looks_like_github_shorthand(source: &str) -> bool {
    let mut segments = source.split('/');
    let owner = segments.next();
    let repo = segments.next();
    let extra = segments.next();
    owner.is_some_and(is_github_shorthand_segment)
        && repo.is_some_and(is_github_shorthand_segment)
        && extra.is_none()
}

fn is_github_shorthand_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

fn validate_marketplace_source_root(root: &Path) -> Result<String, MarketplaceAddError> {
    let marketplace_name = validate_marketplace_root(root)
        .map_err(|err| MarketplaceAddError::InvalidRequest(err.to_string()))?;
    validate_plugin_segment(&marketplace_name, "marketplace name")
        .map_err(MarketplaceAddError::InvalidRequest)?;
    Ok(marketplace_name)
}

fn safe_marketplace_dir_name(marketplace_name: &str) -> Result<String, MarketplaceAddError> {
    let safe = marketplace_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let safe = safe.trim_matches('.').to_string();
    if safe.is_empty() || safe == ".." {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace name '{marketplace_name}' cannot be used as an install directory"
        )));
    }
    Ok(safe)
}

fn ensure_marketplace_destination_is_inside_install_root(
    install_root: &Path,
    destination: &Path,
) -> Result<(), MarketplaceAddError> {
    let install_root = install_root.canonicalize().map_err(|err| {
        MarketplaceAddError::Internal(format!(
            "failed to resolve marketplace install root {}: {err}",
            install_root.display()
        ))
    })?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| {
            MarketplaceAddError::Internal("marketplace destination has no parent".to_string())
        })?
        .canonicalize()
        .map_err(|err| {
            MarketplaceAddError::Internal(format!(
                "failed to resolve marketplace destination parent {}: {err}",
                destination.display()
            ))
        })?;
    if !destination_parent.starts_with(&install_root) {
        return Err(MarketplaceAddError::InvalidRequest(format!(
            "marketplace destination {} is outside install root {}",
            destination.display(),
            install_root.display()
        )));
    }
    Ok(())
}

fn utc_timestamp_now() -> Result<String, MarketplaceAddError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            MarketplaceAddError::Internal(format!("system clock is before Unix epoch: {err}"))
        })?;
    Ok(format_utc_timestamp(duration.as_secs() as i64))
}

fn format_utc_timestamp(seconds_since_epoch: i64) -> String {
    const SECONDS_PER_DAY: i64 = 86_400;
    let days = seconds_since_epoch.div_euclid(SECONDS_PER_DAY);
    let seconds_of_day = seconds_since_epoch.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

impl MarketplaceSource {
    fn display(&self) -> String {
        match self {
            Self::Git { url, ref_name } => match ref_name {
                Some(ref_name) => format!("{url}#{ref_name}"),
                None => url.clone(),
            },
        }
    }
}

fn clone_git_source(
    url: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    destination: &Path,
) -> Result<(), MarketplaceAddError> {
    let destination_string = destination.to_string_lossy().to_string();
    if sparse_paths.is_empty() {
        run_git(&["clone", url, destination_string.as_str()], None)?;
        if let Some(ref_name) = ref_name {
            run_git(
                &["checkout", ref_name],
                Some(Path::new(&destination_string)),
            )?;
        }
        return Ok(());
    }

    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            url,
            destination_string.as_str(),
        ],
        None,
    )?;
    let mut sparse_args = vec!["sparse-checkout", "set"];
    sparse_args.extend(sparse_paths.iter().map(String::as_str));
    run_git(&sparse_args, Some(destination))?;
    run_git(&["checkout", ref_name.unwrap_or("HEAD")], Some(destination))?;
    Ok(())
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<(), MarketplaceAddError> {
    let mut command = Command::new("git");
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let output = command.output().map_err(|err| {
        MarketplaceAddError::Internal(format!("failed to run git {}: {err}", args.join(" ")))
    })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(MarketplaceAddError::Internal(format!(
        "git {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        stdout.trim(),
        stderr.trim()
    )))
}

fn replace_marketplace_root(staged_root: &Path, destination: &Path) -> std::io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(staged_root, destination)
}

fn marketplace_staging_root(install_root: &Path) -> PathBuf {
    install_root.join(".staging")
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn github_shorthand_parses_ref_suffix() {
        assert_eq!(
            parse_marketplace_source("owner/repo@main", None).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }

    #[test]
    fn git_url_parses_fragment_ref() {
        assert_eq!(
            parse_marketplace_source("https://example.com/team/repo.git#v1", None).unwrap(),
            MarketplaceSource::Git {
                url: "https://example.com/team/repo.git".to_string(),
                ref_name: Some("v1".to_string()),
            }
        );
    }

    #[test]
    fn explicit_ref_overrides_source_ref() {
        assert_eq!(
            parse_marketplace_source("owner/repo@main", Some("release".to_string())).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("release".to_string()),
            }
        );
    }

    #[test]
    fn github_shorthand_and_git_url_normalize_to_same_source() {
        let shorthand = parse_marketplace_source("owner/repo", None).unwrap();
        let git_url = parse_marketplace_source("https://github.com/owner/repo.git", None).unwrap();

        assert_eq!(shorthand, git_url);
        assert_eq!(
            shorthand,
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn github_url_with_trailing_slash_normalizes_without_extra_path_segment() {
        assert_eq!(
            parse_marketplace_source("https://github.com/owner/repo/", None).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn non_github_https_source_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source("https://gitlab.com/owner/repo", None).unwrap(),
            MarketplaceSource::Git {
                url: "https://gitlab.com/owner/repo".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn file_url_source_is_rejected() {
        let err = parse_marketplace_source("file:///tmp/marketplace.git", None).unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid marketplace source format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_marketplace_source_rejects_local_directory_source() {
        let err = parse_marketplace_source("./marketplace", None).unwrap_err();

        assert_eq!(
            err.to_string(),
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo"
        );
    }

    #[test]
    fn ssh_url_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source("ssh://git@github.com/owner/repo.git#main", None).unwrap(),
            MarketplaceSource::Git {
                url: "ssh://git@github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }

    #[test]
    fn utc_timestamp_formats_unix_epoch_as_rfc3339_utc() {
        assert_eq!(format_utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc_timestamp(1_775_779_200), "2026-04-10T00:00:00Z");
    }

    #[test]
    fn add_marketplace_sync_installs_marketplace_and_updates_config() -> Result<()> {
        let codex_home = TempDir::new()?;
        let source_root = TempDir::new()?;
        write_marketplace_source(source_root.path(), "remote copy")?;

        let result = add_marketplace_sync_with_cloner(
            codex_home.path(),
            MarketplaceAddRequest {
                source: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
                sparse_paths: Vec::new(),
            },
            |_url, _ref_name, _sparse_paths, destination| {
                copy_dir_all(source_root.path(), destination)
                    .map_err(|err| MarketplaceAddError::Internal(err.to_string()))
            },
        )?;

        assert_eq!(result.marketplace_name, "debug");
        assert_eq!(result.source_display, "https://github.com/owner/repo.git");
        assert!(!result.already_added);
        assert!(
            result
                .installed_root
                .as_path()
                .join(".agents/plugins/marketplace.json")
                .is_file()
        );

        let config = fs::read_to_string(codex_home.path().join(CONFIG_TOML_FILE))?;
        assert!(config.contains("[marketplaces.debug]"));
        assert!(config.contains("source_type = \"git\""));
        assert!(config.contains("source = \"https://github.com/owner/repo.git\""));
        Ok(())
    }

    #[test]
    fn installed_marketplace_root_for_source_propagates_config_read_errors() -> Result<()> {
        let codex_home = TempDir::new()?;
        let config_path = codex_home.path().join(CONFIG_TOML_FILE);
        fs::create_dir(&config_path)?;

        let install_root = codex_home.path().join("marketplaces");
        let source = MarketplaceSource::Git {
            url: "https://github.com/owner/repo.git".to_string(),
            ref_name: None,
        };
        let install_metadata = MarketplaceInstallMetadata::from_source(&source, &[]);

        let err = installed_marketplace_root_for_source(
            codex_home.path(),
            &install_root,
            &install_metadata,
        )
        .unwrap_err();

        assert!(
            err.to_string().contains(&format!(
                "failed to read user config {}:",
                config_path.display()
            )),
            "unexpected error: {err}"
        );

        Ok(())
    }

    fn write_marketplace_source(source: &Path, marker: &str) -> std::io::Result<()> {
        fs::create_dir_all(source.join(".agents/plugins"))?;
        fs::create_dir_all(source.join("plugins/sample/.codex-plugin"))?;
        fs::write(
            source.join(".agents/plugins/marketplace.json"),
            r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
        )?;
        fs::write(
            source.join("plugins/sample/.codex-plugin/plugin.json"),
            r#"{"name":"sample"}"#,
        )?;
        fs::write(source.join("plugins/sample/marker.txt"), marker)?;
        Ok(())
    }

    fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_all(&source_path, &destination_path)?;
            } else {
                fs::copy(&source_path, &destination_path)?;
            }
        }
        Ok(())
    }
}
