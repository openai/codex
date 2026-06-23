use codex_install_context::InstallContext;
use codex_install_context::InstallMethod;
use codex_install_context::StandalonePlatform;
use codex_login::default_client::create_client;
use serde::Deserialize;
use std::collections::HashMap;

const HOMEBREW_CASK_API_URL: &str = "https://formulae.brew.sh/api/cask/codex.json";
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/openai/codex/releases/latest";
const NPM_PACKAGE_URL: &str = "https://registry.npmjs.org/@openai%2fcodex";

/// An updater command matched to the package manager or standalone installer
/// that launched Codex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    NpmGlobalLatest,
    BunGlobalLatest,
    BrewUpgrade,
    StandaloneUnix,
    StandaloneWindows,
}

impl UpdateAction {
    pub fn from_install_context(context: &InstallContext) -> Option<Self> {
        match &context.method {
            InstallMethod::Npm => Some(Self::NpmGlobalLatest),
            InstallMethod::Bun => Some(Self::BunGlobalLatest),
            InstallMethod::Brew => Some(Self::BrewUpgrade),
            InstallMethod::Standalone { platform, .. } => Some(match platform {
                StandalonePlatform::Unix => Self::StandaloneUnix,
                StandalonePlatform::Windows => Self::StandaloneWindows,
            }),
            InstallMethod::Other => None,
        }
    }

    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex"]),
            Self::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex"]),
            Self::BrewUpgrade => ("brew", &["upgrade", "--cask", "codex"]),
            Self::StandaloneUnix => (
                "sh",
                &[
                    "-c",
                    "curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh",
                ],
            ),
            Self::StandaloneWindows => (
                "powershell",
                &[
                    "-ExecutionPolicy",
                    "Bypass",
                    "-c",
                    "$env:CODEX_NON_INTERACTIVE=1; irm https://chatgpt.com/codex/install.ps1 | iex",
                ],
            ),
        }
    }

    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

pub fn get_update_action() -> Option<UpdateAction> {
    UpdateAction::from_install_context(InstallContext::current())
}

/// Fetch the newest version available through the current installation's
/// distribution channel.
pub async fn latest_version() -> anyhow::Result<String> {
    latest_version_from_sources(get_update_action(), &UpdateSources::default()).await
}

pub fn is_newer(latest: &str, current: &str) -> Option<bool> {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => Some(latest > current),
        _ => None,
    }
}

pub fn is_source_build_version(version: &str) -> bool {
    parse_version(version) == Some((0, 0, 0))
}

struct UpdateSources {
    homebrew_cask_api_url: String,
    latest_release_url: String,
    npm_package_url: String,
}

impl Default for UpdateSources {
    fn default() -> Self {
        Self {
            homebrew_cask_api_url: HOMEBREW_CASK_API_URL.to_string(),
            latest_release_url: LATEST_RELEASE_URL.to_string(),
            npm_package_url: NPM_PACKAGE_URL.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct ReleaseInfo {
    tag_name: String,
}

#[derive(Deserialize)]
struct HomebrewCaskInfo {
    version: String,
}

#[derive(Deserialize)]
struct NpmPackageInfo {
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, NpmPackageVersionInfo>,
}

#[derive(Deserialize)]
struct NpmPackageVersionInfo {
    dist: Option<NpmPackageDist>,
}

#[derive(Deserialize)]
struct NpmPackageDist {
    tarball: Option<String>,
    integrity: Option<String>,
}

async fn latest_version_from_sources(
    action: Option<UpdateAction>,
    sources: &UpdateSources,
) -> anyhow::Result<String> {
    match action {
        Some(UpdateAction::BrewUpgrade) => {
            let HomebrewCaskInfo { version } = create_client()
                .get(&sources.homebrew_cask_api_url)
                .send()
                .await?
                .error_for_status()?
                .json::<HomebrewCaskInfo>()
                .await?;
            Ok(version)
        }
        Some(UpdateAction::NpmGlobalLatest) | Some(UpdateAction::BunGlobalLatest) => {
            let latest_version =
                fetch_latest_github_release_version(&sources.latest_release_url).await?;
            let package_info = create_client()
                .get(&sources.npm_package_url)
                .send()
                .await?
                .error_for_status()?
                .json::<NpmPackageInfo>()
                .await?;
            ensure_npm_version_ready(&package_info, &latest_version)?;
            Ok(latest_version)
        }
        Some(UpdateAction::StandaloneUnix) | Some(UpdateAction::StandaloneWindows) | None => {
            fetch_latest_github_release_version(&sources.latest_release_url).await
        }
    }
}

async fn fetch_latest_github_release_version(url: &str) -> anyhow::Result<String> {
    let ReleaseInfo {
        tag_name: latest_tag_name,
    } = create_client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<ReleaseInfo>()
        .await?;
    extract_version_from_latest_tag(&latest_tag_name)
}

fn ensure_npm_version_ready(package_info: &NpmPackageInfo, version: &str) -> anyhow::Result<()> {
    let version = version.trim();
    match package_info.dist_tags.get("latest").map(String::as_str) {
        Some(latest) if latest == version => {}
        Some(latest) => anyhow::bail!(
            "npm latest dist-tag points to {latest}, expected GitHub release {version}"
        ),
        None => anyhow::bail!("npm package is missing latest dist-tag"),
    }

    let info = package_info
        .versions
        .get(version)
        .ok_or_else(|| anyhow::anyhow!("npm package version {version} is missing"))?;
    let Some(dist) = info.dist.as_ref() else {
        anyhow::bail!("npm package version {version} is missing dist metadata");
    };
    if dist.tarball.as_deref().is_none_or(str::is_empty) {
        anyhow::bail!("npm package version {version} is missing dist.tarball");
    }
    if dist.integrity.as_deref().is_none_or(str::is_empty) {
        anyhow::bail!("npm package version {version} is missing dist.integrity");
    }
    Ok(())
}

fn extract_version_from_latest_tag(latest_tag_name: &str) -> anyhow::Result<String> {
    latest_tag_name
        .strip_prefix("rust-v")
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("failed to parse latest tag name '{latest_tag_name}'"))
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
