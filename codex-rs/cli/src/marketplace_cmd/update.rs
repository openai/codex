use super::clone_git_source;
use super::copy_dir_recursive;
use super::marketplace_staging_root;
use super::metadata::InstalledMarketplaceSource;
use super::metadata::MarketplaceInstallMetadata;
use super::metadata::read_marketplace_source_metadata;
use super::metadata::write_marketplace_source_metadata;
use super::replace_marketplace_root;
use super::run_git;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use clap::Parser;
use codex_core::config::find_codex_home;
use codex_core::plugins::marketplace_install_root;
use codex_core::plugins::validate_marketplace_root;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub(super) struct UpdateMarketplaceArgs {
    /// Marketplace name to update. If omitted, updates all added marketplaces.
    pub(super) name: Option<String>,
}

#[derive(Debug)]
struct InstalledMarketplace {
    name: String,
    root: PathBuf,
    metadata: Option<MarketplaceInstallMetadata>,
}

pub(super) async fn run_update(args: UpdateMarketplaceArgs) -> Result<()> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let install_root = marketplace_install_root(&codex_home);
    let marketplaces = installed_marketplaces(&install_root)?;

    if let Some(name) = args.name {
        let available_names = marketplaces
            .iter()
            .map(|marketplace| marketplace.name.clone())
            .collect::<Vec<_>>();
        let available_names = if available_names.is_empty() {
            "<none>".to_string()
        } else {
            available_names.join(", ")
        };
        let marketplace = marketplaces
            .into_iter()
            .find(|marketplace| marketplace.name == name)
            .with_context(|| {
                format!(
                    "marketplace `{name}` is not added. Available marketplaces: {available_names}"
                )
            })?;
        println!("Updating marketplace: {name}...");
        refresh_installed_marketplace(&marketplace, |message| println!("{message}"))?;
        println!("Successfully updated marketplace: {name}");
        return Ok(());
    }

    let marketplaces = marketplaces
        .into_iter()
        .filter(|marketplace| marketplace.metadata.is_some())
        .collect::<Vec<_>>();
    if marketplaces.is_empty() {
        println!("No marketplaces configured.");
        return Ok(());
    }

    println!("Updating {} marketplace(s)...", marketplaces.len());
    for marketplace in &marketplaces {
        if let Err(err) = refresh_installed_marketplace(marketplace, |message| {
            println!("{}: {message}", marketplace.name)
        }) {
            eprintln!(
                "Failed to update marketplace `{}`: {err:#}",
                marketplace.name
            );
        }
    }
    println!(
        "Successfully updated {} marketplace(s).",
        marketplaces.len()
    );

    Ok(())
}

fn refresh_installed_marketplace(
    marketplace: &InstalledMarketplace,
    on_progress: impl Fn(&str),
) -> Result<()> {
    let metadata = marketplace.metadata.as_ref().with_context(|| {
        format!(
            "marketplace `{}` was added without source metadata; remove and add it again before updating",
            marketplace.name
        )
    })?;

    match &metadata.source {
        InstalledMarketplaceSource::LocalDirectory { path } => {
            on_progress("Validating local marketplace...");
            let source_marketplace_name = validate_marketplace_root(path).with_context(|| {
                format!(
                    "failed to validate local marketplace source {}",
                    path.display()
                )
            })?;
            ensure_refreshed_marketplace_name_is_stable(
                &marketplace.name,
                &source_marketplace_name,
            )?;
            on_progress("Refreshing marketplace cache from local directory...");
            let install_root = marketplace
                .root
                .parent()
                .context("marketplace root has no parent")?;
            let staging_root = marketplace_staging_root(install_root);
            fs::create_dir_all(&staging_root)?;
            let staged_dir = tempfile::Builder::new()
                .prefix("marketplace-update-")
                .tempdir_in(&staging_root)?;
            let staged_root = staged_dir.path().to_path_buf();
            copy_dir_recursive(path, &staged_root)?;
            let refreshed_name = validate_marketplace_root(&staged_root)?;
            ensure_refreshed_marketplace_name_is_stable(&marketplace.name, &refreshed_name)?;
            write_marketplace_source_metadata(&staged_root, metadata)?;
            replace_marketplace_root(&staged_root, &marketplace.root)?;
        }
        InstalledMarketplaceSource::Git {
            url,
            ref_name,
            sparse_paths,
        } => {
            on_progress("Refreshing marketplace cache...");
            refresh_git_marketplace(&marketplace.root, url, ref_name.as_deref(), sparse_paths)
                .with_context(|| {
                    format!("failed to refresh git marketplace `{}`", marketplace.name)
                })?;
            let refreshed_name = validate_marketplace_root(&marketplace.root).with_context(|| {
                format!(
                    "marketplace `{}` was refreshed but no longer has a valid .agents/plugins/marketplace.json; remove and add it again if the repository moved or stopped being a marketplace",
                    marketplace.name
                )
            })?;
            ensure_refreshed_marketplace_name_is_stable(&marketplace.name, &refreshed_name)?;
            write_marketplace_source_metadata(&marketplace.root, metadata)?;
        }
    }

    Ok(())
}

fn refresh_git_marketplace(
    destination: &Path,
    url: &str,
    ref_name: Option<&str>,
    sparse_paths: &[String],
) -> Result<()> {
    match pull_git_source(destination, ref_name, sparse_paths) {
        Ok(()) => return Ok(()),
        Err(pull_err) => {
            if destination.exists() {
                fs::remove_dir_all(destination).with_context(|| {
                    format!(
                        "git pull failed ({pull_err:#}) and failed to remove stale marketplace directory {}; remove it manually and retry",
                        destination.display()
                    )
                })?;
            }
            clone_git_source(url, ref_name, sparse_paths, destination).with_context(|| {
                format!("git pull failed ({pull_err:#}) and fallback clone from {url} failed")
            })?;
        }
    }
    Ok(())
}

fn pull_git_source(
    destination: &Path,
    ref_name: Option<&str>,
    sparse_paths: &[String],
) -> Result<()> {
    if !destination.join(".git").is_dir() {
        bail!(
            "marketplace cache {} is not a git repository",
            destination.display()
        );
    }
    if !sparse_paths.is_empty() {
        let mut sparse_args = vec!["sparse-checkout", "set"];
        sparse_args.extend(sparse_paths.iter().map(String::as_str));
        run_git(&sparse_args, Some(destination))?;
    }
    if let Some(ref_name) = ref_name {
        run_git(&["fetch", "origin", ref_name], Some(destination))?;
        run_git(&["checkout", ref_name], Some(destination))?;
        run_git(&["pull", "origin", ref_name], Some(destination))?;
    } else {
        run_git(&["pull", "origin", "HEAD"], Some(destination))?;
    }
    if sparse_paths.is_empty()
        && let Err(err) = run_git(
            &["submodule", "update", "--init", "--recursive"],
            Some(destination),
        )
    {
        eprintln!("Warning: failed to update marketplace submodules: {err:#}");
    }
    Ok(())
}

fn installed_marketplaces(install_root: &Path) -> Result<Vec<InstalledMarketplace>> {
    let Ok(entries) = fs::read_dir(install_root) else {
        return Ok(Vec::new());
    };
    let mut marketplaces = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let root = entry.path();
        let Ok(name) = validate_marketplace_root(&root) else {
            continue;
        };
        let metadata = read_marketplace_source_metadata(&root)?;
        marketplaces.push(InstalledMarketplace {
            name,
            root,
            metadata,
        });
    }
    marketplaces.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    Ok(marketplaces)
}

fn ensure_refreshed_marketplace_name_is_stable(
    original_name: &str,
    refreshed_name: &str,
) -> Result<()> {
    if original_name != refreshed_name {
        bail!(
            "marketplace `{original_name}` refreshed successfully but now declares name `{refreshed_name}`; remove and add it again to accept a marketplace rename"
        );
    }
    Ok(())
}
