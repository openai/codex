use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_config::MarketplaceConfigUpdate;
use codex_config::record_user_marketplace;
use codex_core::config::find_codex_home;
use codex_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core::plugins::marketplace_install_root;
use codex_core::plugins::validate_marketplace_root;
use codex_core::plugins::validate_plugin_segment;
use codex_utils_cli::CliConfigOverrides;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

mod metadata;
mod ops;

#[derive(Debug, Parser)]
pub struct MarketplaceCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    subcommand: MarketplaceSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum MarketplaceSubcommand {
    /// Add a remote marketplace repository.
    Add(AddMarketplaceArgs),
}

#[derive(Debug, Parser)]
struct AddMarketplaceArgs {
    /// Marketplace source. Supports owner/repo[@ref], HTTP(S) Git URLs, or SSH URLs.
    source: String,

    /// Git ref to check out. Overrides any @ref or #ref suffix in SOURCE.
    #[arg(long = "ref", value_name = "REF")]
    ref_name: Option<String>,

    /// Sparse-checkout path to use while cloning git sources. Repeat to include multiple paths.
    #[arg(
        long = "sparse",
        value_name = "PATH",
        action = clap::ArgAction::Append
    )]
    sparse_paths: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MarketplaceSource {
    Git {
        url: String,
        ref_name: Option<String>,
    },
}

impl MarketplaceCli {
    pub async fn run(self) -> Result<()> {
        let MarketplaceCli {
            config_overrides,
            subcommand,
        } = self;

        // Validate overrides now. This command writes to CODEX_HOME only; marketplace discovery
        // happens from that cache root after the next plugin/list or app-server start.
        config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;

        match subcommand {
            MarketplaceSubcommand::Add(args) => run_add(args).await?,
        }

        Ok(())
    }
}

async fn run_add(args: AddMarketplaceArgs) -> Result<()> {
    let AddMarketplaceArgs {
        source,
        ref_name,
        sparse_paths,
    } = args;

    let source = parse_marketplace_source(&source, ref_name)?;

    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let install_root = marketplace_install_root(&codex_home);
    fs::create_dir_all(&install_root).with_context(|| {
        format!(
            "failed to create marketplace install directory {}",
            install_root.display()
        )
    })?;
    let install_metadata =
        metadata::MarketplaceInstallMetadata::from_source(&source, &sparse_paths);
    if let Some(existing_root) = metadata::installed_marketplace_root_for_source(
        &codex_home,
        &install_root,
        &install_metadata,
    )? {
        let marketplace_name = validate_marketplace_root(&existing_root).with_context(|| {
            format!(
                "failed to validate installed marketplace at {}",
                existing_root.display()
            )
        })?;
        record_added_marketplace(&codex_home, &marketplace_name, &install_metadata)?;
        println!(
            "Marketplace `{marketplace_name}` is already added from {}.",
            source.display()
        );
        println!("Installed marketplace root: {}", existing_root.display());
        return Ok(());
    }

    let staging_root = ops::marketplace_staging_root(&install_root);
    fs::create_dir_all(&staging_root).with_context(|| {
        format!(
            "failed to create marketplace staging directory {}",
            staging_root.display()
        )
    })?;
    let staged_dir = tempfile::Builder::new()
        .prefix("marketplace-add-")
        .tempdir_in(&staging_root)
        .with_context(|| {
            format!(
                "failed to create temporary marketplace directory in {}",
                staging_root.display()
            )
        })?;
    let staged_root = staged_dir.path().to_path_buf();

    let MarketplaceSource::Git { url, ref_name } = &source;
    ops::clone_git_source(url, ref_name.as_deref(), &sparse_paths, &staged_root)?;

    let marketplace_name = validate_marketplace_source_root(&staged_root)
        .with_context(|| format!("failed to validate marketplace from {}", source.display()))?;
    if marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
        bail!(
            "marketplace `{OPENAI_CURATED_MARKETPLACE_NAME}` is reserved and cannot be added from {}",
            source.display()
        );
    }
    let destination = install_root.join(safe_marketplace_dir_name(&marketplace_name)?);
    ensure_marketplace_destination_is_inside_install_root(&install_root, &destination)?;
    if destination.exists() {
        bail!(
            "marketplace `{marketplace_name}` is already added from a different source; remove it before adding {}",
            source.display()
        );
    }
    ops::replace_marketplace_root(&staged_root, &destination)
        .with_context(|| format!("failed to install marketplace at {}", destination.display()))?;
    if let Err(err) = record_added_marketplace(&codex_home, &marketplace_name, &install_metadata) {
        if let Err(rollback_err) = fs::rename(&destination, &staged_root) {
            bail!(
                "{err}; additionally failed to roll back installed marketplace at {}: {rollback_err}",
                destination.display()
            );
        }
        return Err(err);
    }

    println!(
        "Added marketplace `{marketplace_name}` from {}.",
        source.display()
    );
    println!("Installed marketplace root: {}", destination.display());

    Ok(())
}

fn record_added_marketplace(
    codex_home: &Path,
    marketplace_name: &str,
    install_metadata: &metadata::MarketplaceInstallMetadata,
) -> Result<()> {
    let source = install_metadata.config_source();
    let last_updated = utc_timestamp_now()?;
    let update = MarketplaceConfigUpdate {
        last_updated: &last_updated,
        last_revision: None,
        source_type: install_metadata.config_source_type(),
        source: &source,
        ref_name: install_metadata.ref_name(),
        sparse_paths: install_metadata.sparse_paths(),
    };
    record_user_marketplace(codex_home, marketplace_name, &update).with_context(|| {
        format!("failed to add marketplace `{marketplace_name}` to user config.toml")
    })?;
    Ok(())
}

fn validate_marketplace_source_root(root: &Path) -> Result<String> {
    let marketplace_name = validate_marketplace_root(root)?;
    validate_plugin_segment(&marketplace_name, "marketplace name").map_err(anyhow::Error::msg)?;
    Ok(marketplace_name)
}

fn parse_marketplace_source(
    source: &str,
    explicit_ref: Option<String>,
) -> Result<MarketplaceSource> {
    let source = source.trim();
    if source.is_empty() {
        bail!("marketplace source must not be empty");
    }

    let (base_source, parsed_ref) = split_source_ref(source);
    let ref_name = explicit_ref.or(parsed_ref);

    if looks_like_local_path(&base_source) {
        bail!(
            "local marketplace sources are not supported yet; use an HTTP(S) Git URL, SSH Git URL, or GitHub owner/repo"
        );
    }

    if is_ssh_git_url(&base_source) || is_git_url(&base_source) {
        let url = normalize_git_url(&base_source);
        return Ok(MarketplaceSource::Git { url, ref_name });
    }

    if looks_like_github_shorthand(&base_source) {
        let url = format!("https://github.com/{base_source}.git");
        return Ok(MarketplaceSource::Git { url, ref_name });
    }

    bail!("invalid marketplace source format: {source}");
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

fn safe_marketplace_dir_name(marketplace_name: &str) -> Result<String> {
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
        bail!("marketplace name `{marketplace_name}` cannot be used as an install directory");
    }
    Ok(safe)
}

fn ensure_marketplace_destination_is_inside_install_root(
    install_root: &Path,
    destination: &Path,
) -> Result<()> {
    let install_root = install_root.canonicalize().with_context(|| {
        format!(
            "failed to resolve marketplace install root {}",
            install_root.display()
        )
    })?;
    let destination_parent = destination
        .parent()
        .context("marketplace destination has no parent")?
        .canonicalize()
        .with_context(|| {
            format!(
                "failed to resolve marketplace destination parent {}",
                destination.display()
            )
        })?;
    if !destination_parent.starts_with(&install_root) {
        bail!(
            "marketplace destination {} is outside install root {}",
            destination.display(),
            install_root.display()
        );
    }
    Ok(())
}

fn utc_timestamp_now() -> Result<String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?;
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
            Self::Git { url, ref_name } => {
                if let Some(ref_name) = ref_name {
                    format!("{url}#{ref_name}")
                } else {
                    url.clone()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn github_shorthand_parses_ref_suffix() {
        assert_eq!(
            parse_marketplace_source("owner/repo@main", /*explicit_ref*/ None).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }

    #[test]
    fn git_url_parses_fragment_ref() {
        assert_eq!(
            parse_marketplace_source(
                "https://example.com/team/repo.git#v1",
                /*explicit_ref*/ None,
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "https://example.com/team/repo.git".to_string(),
                ref_name: Some("v1".to_string()),
            }
        );
    }

    #[test]
    fn explicit_ref_overrides_source_ref() {
        assert_eq!(
            parse_marketplace_source(
                "owner/repo@main",
                /*explicit_ref*/ Some("release".to_string()),
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("release".to_string()),
            }
        );
    }

    #[test]
    fn github_shorthand_and_git_url_normalize_to_same_source() {
        let shorthand = parse_marketplace_source("owner/repo", /*explicit_ref*/ None).unwrap();
        let git_url = parse_marketplace_source(
            "https://github.com/owner/repo.git",
            /*explicit_ref*/ None,
        )
        .unwrap();

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
            parse_marketplace_source("https://github.com/owner/repo/", /*explicit_ref*/ None)
                .unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn non_github_https_source_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source("https://gitlab.com/owner/repo", /*explicit_ref*/ None)
                .unwrap(),
            MarketplaceSource::Git {
                url: "https://gitlab.com/owner/repo".to_string(),
                ref_name: None,
            }
        );
    }

    #[test]
    fn file_url_source_is_rejected() {
        let err =
            parse_marketplace_source("file:///tmp/marketplace.git", /*explicit_ref*/ None)
                .unwrap_err();

        assert!(
            err.to_string()
                .contains("invalid marketplace source format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn local_path_source_is_rejected() {
        let err = parse_marketplace_source("./marketplace", /*explicit_ref*/ None).unwrap_err();

        assert!(
            err.to_string()
                .contains("local marketplace sources are not supported yet"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn ssh_url_parses_as_git_url() {
        assert_eq!(
            parse_marketplace_source(
                "ssh://git@github.com/owner/repo.git#main",
                /*explicit_ref*/ None,
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "ssh://git@github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
            }
        );
    }

    #[test]
    fn utc_timestamp_formats_unix_epoch_as_rfc3339_utc() {
        assert_eq!(
            format_utc_timestamp(/*seconds_since_epoch*/ 0),
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(
            format_utc_timestamp(/*seconds_since_epoch*/ 1_775_779_200),
            "2026-04-10T00:00:00Z"
        );
    }

    #[test]
    fn sparse_paths_parse_before_or_after_source() {
        let sparse_before_source =
            AddMarketplaceArgs::try_parse_from(["add", "--sparse", "plugins/foo", "owner/repo"])
                .unwrap();
        assert_eq!(sparse_before_source.source, "owner/repo");
        assert_eq!(sparse_before_source.sparse_paths, vec!["plugins/foo"]);

        let sparse_after_source =
            AddMarketplaceArgs::try_parse_from(["add", "owner/repo", "--sparse", "plugins/foo"])
                .unwrap();
        assert_eq!(sparse_after_source.source, "owner/repo");
        assert_eq!(sparse_after_source.sparse_paths, vec!["plugins/foo"]);

        let repeated_sparse = AddMarketplaceArgs::try_parse_from([
            "add",
            "--sparse",
            "plugins/foo",
            "--sparse",
            "skills/bar",
            "owner/repo",
        ])
        .unwrap();
        assert_eq!(repeated_sparse.source, "owner/repo");
        assert_eq!(
            repeated_sparse.sparse_paths,
            vec!["plugins/foo", "skills/bar"]
        );
    }
}
