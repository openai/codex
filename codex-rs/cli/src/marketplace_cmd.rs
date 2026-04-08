use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::find_codex_home;
use codex_core::plugins::OPENAI_CURATED_MARKETPLACE_NAME;
use codex_core::plugins::marketplace_install_root;
use codex_core::plugins::record_installed_marketplace_root;
use codex_core::plugins::validate_marketplace_root;
use codex_utils_cli::CliConfigOverrides;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

mod metadata;

#[derive(Debug, Parser)]
pub struct MarketplaceCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    subcommand: MarketplaceSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum MarketplaceSubcommand {
    /// Add a marketplace repository or local marketplace directory.
    Add(AddMarketplaceArgs),
}

#[derive(Debug, Parser)]
struct AddMarketplaceArgs {
    /// Marketplace source. Supports owner/repo[@ref], git URLs, SSH URLs, or local directories.
    source: String,

    /// Git ref to check out. Overrides any @ref or #ref suffix in SOURCE.
    #[arg(long = "ref", value_name = "REF")]
    ref_name: Option<String>,

    /// Sparse-checkout paths to use while cloning git sources.
    #[arg(long = "sparse", value_name = "PATH", num_args = 1..)]
    sparse_paths: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MarketplaceSource {
    LocalDirectory {
        path: PathBuf,
        source_id: String,
    },
    Git {
        url: String,
        ref_name: Option<String>,
        source_id: String,
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
    if !sparse_paths.is_empty() && !matches!(source, MarketplaceSource::Git { .. }) {
        bail!("--sparse can only be used with git marketplace sources");
    }

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
    if let Some(existing_root) =
        metadata::installed_marketplace_root_for_source(&install_root, &install_metadata.source_id)?
    {
        let marketplace_name = validate_marketplace_root(&existing_root).with_context(|| {
            format!(
                "failed to validate installed marketplace at {}",
                existing_root.display()
            )
        })?;
        println!(
            "Marketplace `{marketplace_name}` is already added from {}.",
            source.display()
        );
        println!("Installed marketplace root: {}", existing_root.display());
        return Ok(());
    }

    let staging_root = marketplace_staging_root(&install_root);
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

    match &source {
        MarketplaceSource::LocalDirectory { path, .. } => {
            copy_dir_recursive(path, &staged_root).with_context(|| {
                format!(
                    "failed to copy marketplace source {} into {}",
                    path.display(),
                    staged_root.display()
                )
            })?;
        }
        MarketplaceSource::Git { url, ref_name, .. } => {
            clone_git_source(url, ref_name.as_deref(), &sparse_paths, &staged_root)?;
        }
    }

    let marketplace_name = validate_marketplace_root(&staged_root)
        .with_context(|| format!("failed to validate marketplace from {}", source.display()))?;
    if marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME {
        bail!(
            "marketplace `{OPENAI_CURATED_MARKETPLACE_NAME}` is reserved and cannot be added from {}",
            source.display()
        );
    }
    metadata::write_marketplace_source_metadata(&staged_root, &install_metadata)?;
    let destination = install_root.join(safe_marketplace_dir_name(&marketplace_name)?);
    ensure_marketplace_destination_is_inside_install_root(&install_root, &destination)?;
    if destination.exists() {
        bail!(
            "marketplace `{marketplace_name}` is already added from a different source; remove it before adding {}",
            source.display()
        );
    }
    replace_marketplace_root(&staged_root, &destination)
        .with_context(|| format!("failed to install marketplace at {}", destination.display()))?;
    record_installed_marketplace_root(&codex_home, &marketplace_name, &destination)
        .with_context(|| format!("failed to record marketplace `{marketplace_name}`"))?;

    println!(
        "Added marketplace `{marketplace_name}` from {}.",
        source.display()
    );
    println!("Installed marketplace root: {}", destination.display());

    Ok(())
}

fn parse_marketplace_source(
    source: &str,
    explicit_ref: Option<String>,
) -> Result<MarketplaceSource> {
    let source = source.trim();
    if source.is_empty() {
        bail!("marketplace source must not be empty");
    }

    let source = expand_home(source);
    let path = PathBuf::from(&source);
    let path_exists = path.try_exists().with_context(|| {
        format!(
            "failed to access local marketplace source {}",
            path.display()
        )
    })?;
    if path_exists || looks_like_local_path(&source) {
        if !path_exists {
            bail!(
                "local marketplace source does not exist: {}",
                path.display()
            );
        }
        let metadata = path.metadata().with_context(|| {
            format!("failed to read local marketplace source {}", path.display())
        })?;
        if metadata.is_file() {
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                bail!(
                    "local marketplace JSON files are not supported yet; pass the marketplace root directory containing .agents/plugins/marketplace.json: {}",
                    path.display()
                );
            }
            bail!(
                "local marketplace source file must be a JSON marketplace manifest or a directory containing .agents/plugins/marketplace.json: {}",
                path.display()
            );
        }
        if !metadata.is_dir() {
            bail!(
                "local marketplace source must be a file or directory: {}",
                path.display()
            );
        }
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()))?;
        return Ok(MarketplaceSource::LocalDirectory {
            source_id: format!("directory:{}", path.display()),
            path,
        });
    }

    let (base_source, parsed_ref) = split_source_ref(&source);
    let ref_name = explicit_ref.or(parsed_ref);

    if is_ssh_git_url(&base_source) || is_http_git_url(&base_source) {
        let url = normalize_git_url(&base_source);
        return Ok(MarketplaceSource::Git {
            source_id: git_source_id("git", &url, ref_name.as_deref()),
            url,
            ref_name,
        });
    }

    if looks_like_github_shorthand(&base_source) {
        let url = format!("https://github.com/{base_source}.git");
        return Ok(MarketplaceSource::Git {
            source_id: git_source_id("github", &base_source, ref_name.as_deref()),
            url,
            ref_name,
        });
    }

    if base_source.starts_with("http://") || base_source.starts_with("https://") {
        bail!(
            "URL marketplace manifests are not supported yet; pass a git repository URL or a local marketplace directory"
        );
    }

    bail!("invalid marketplace source format: {source}");
}

fn git_source_id(kind: &str, source: &str, ref_name: Option<&str>) -> String {
    if let Some(ref_name) = ref_name {
        format!("{kind}:{source}#{ref_name}")
    } else {
        format!("{kind}:{source}")
    }
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

fn expand_home(source: &str) -> String {
    let Some(rest) = source.strip_prefix("~/") else {
        return source.to_string();
    };
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(rest).display().to_string();
    }
    source.to_string()
}

fn is_ssh_git_url(source: &str) -> bool {
    source.starts_with("git@") && source.contains(':')
}

fn is_http_git_url(source: &str) -> bool {
    (source.starts_with("http://") || source.starts_with("https://"))
        && (source.ends_with(".git") || source.starts_with("https://github.com/"))
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

pub(super) fn clone_git_source(
    url: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
    destination: &Path,
) -> Result<()> {
    let destination = destination.to_string_lossy().to_string();
    if sparse_paths.is_empty() {
        run_git(&["clone", url, destination.as_str()], None)?;
        if let Some(ref_name) = ref_name {
            run_git(&["checkout", ref_name], Some(Path::new(&destination)))?;
        }
        return Ok(());
    }

    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-checkout",
            url,
            destination.as_str(),
        ],
        None,
    )?;
    let mut sparse_args = vec!["sparse-checkout", "set"];
    sparse_args.extend(sparse_paths.iter().map(String::as_str));
    let destination = Path::new(&destination);
    run_git(&sparse_args, Some(destination))?;
    run_git(&["checkout", ref_name.unwrap_or("HEAD")], Some(destination))?;
    Ok(())
}

pub(super) fn run_git(args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut command = Command::new("git");
    command.args(args);
    command.env("GIT_TERMINAL_PROMPT", "0");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    bail!(
        "git {} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        stdout.trim(),
        stderr.trim()
    );
}

pub(super) fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if entry.file_name().to_str() == Some(".git") {
                continue;
            }
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path)?;
        } else if file_type.is_symlink() {
            copy_symlink_target(&source_path, &target_path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink_target(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(fs::read_link(source)?, target)?;
    Ok(())
}

#[cfg(windows)]
fn copy_symlink_target(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::metadata(source)?;
    if metadata.is_dir() {
        copy_dir_recursive(source, target)
    } else {
        fs::copy(source, target).map(|_| ()).map_err(Into::into)
    }
}

pub(super) fn replace_marketplace_root(staged_root: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let backup = if destination.exists() {
        let parent = destination
            .parent()
            .context("marketplace destination has no parent")?;
        let staging_root = marketplace_staging_root(parent);
        fs::create_dir_all(&staging_root)?;
        let backup = tempfile::Builder::new()
            .prefix("marketplace-backup-")
            .tempdir_in(&staging_root)?;
        let backup_root = backup.path().join("previous");
        fs::rename(destination, &backup_root)?;
        Some((backup, backup_root))
    } else {
        None
    };

    if let Err(err) = fs::rename(staged_root, destination) {
        if let Some((_, backup_root)) = backup {
            let _ = fs::rename(backup_root, destination);
        }
        return Err(err.into());
    }

    Ok(())
}

pub(super) fn marketplace_staging_root(install_root: &Path) -> PathBuf {
    install_root.join(".staging")
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

impl MarketplaceSource {
    fn source_id(&self) -> &str {
        match self {
            Self::LocalDirectory { source_id, .. } | Self::Git { source_id, .. } => source_id,
        }
    }

    fn display(&self) -> String {
        match self {
            Self::LocalDirectory { path, .. } => path.display().to_string(),
            Self::Git { url, ref_name, .. } => {
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
            parse_marketplace_source("owner/repo@main", /* explicit_ref */ None).unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("main".to_string()),
                source_id: "github:owner/repo#main".to_string(),
            }
        );
    }

    #[test]
    fn git_url_parses_fragment_ref() {
        assert_eq!(
            parse_marketplace_source(
                "https://example.com/team/repo.git#v1",
                /* explicit_ref */ None,
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "https://example.com/team/repo.git".to_string(),
                ref_name: Some("v1".to_string()),
                source_id: "git:https://example.com/team/repo.git#v1".to_string(),
            }
        );
    }

    #[test]
    fn explicit_ref_overrides_source_ref() {
        assert_eq!(
            parse_marketplace_source(
                "owner/repo@main",
                /* explicit_ref */ Some("release".to_string()),
            )
            .unwrap(),
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: Some("release".to_string()),
                source_id: "github:owner/repo#release".to_string(),
            }
        );
    }

    #[test]
    fn github_shorthand_and_git_url_have_different_source_ids() {
        let shorthand = parse_marketplace_source("owner/repo", /* explicit_ref */ None).unwrap();
        let git_url = parse_marketplace_source(
            "https://github.com/owner/repo.git",
            /* explicit_ref */ None,
        )
        .unwrap();

        assert_ne!(shorthand.source_id(), git_url.source_id());
        assert_eq!(
            shorthand,
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
                source_id: "github:owner/repo".to_string(),
            }
        );
        assert_eq!(
            git_url,
            MarketplaceSource::Git {
                url: "https://github.com/owner/repo.git".to_string(),
                ref_name: None,
                source_id: "git:https://github.com/owner/repo.git".to_string(),
            }
        );
    }
}
