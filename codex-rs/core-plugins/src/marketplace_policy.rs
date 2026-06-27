use crate::OPENAI_API_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_BUNDLED_ALPHA_MARKETPLACE_NAME;
use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME;
use crate::installed_marketplaces::marketplace_install_root;
use crate::installed_marketplaces::resolve_configured_marketplace_root;
use crate::is_openai_curated_marketplace_name;
use crate::marketplace::marketplace_root_dir;
use crate::marketplace_add::MarketplaceSource;
use crate::marketplace_add::parse_marketplace_source;
use crate::startup_sync::curated_plugins_api_marketplace_path;
use crate::startup_sync::curated_plugins_repo_path;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::MarketplaceAllowedSourceKind;
use codex_config::MarketplaceAllowedSourceToml;
use codex_config::RequirementSource;
use codex_config::types::MarketplaceConfig;
use codex_config::types::MarketplaceSourceType;
use codex_config::types::PluginConfig;
use codex_plugin::PluginId;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path::paths_match_after_normalization;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

enum AllowedMarketplaceSource {
    GitUrl {
        url: String,
        ref_name: Option<String>,
    },
    GitHostPattern(Regex),
    Local(AbsolutePathBuf),
}

pub(crate) struct MarketplacePolicy {
    restricted: Option<RestrictedMarketplacePolicy>,
}

struct RestrictedMarketplacePolicy {
    allowed_sources: Result<Vec<AllowedMarketplaceSource>, String>,
    source: RequirementSource,
}

impl MarketplacePolicy {
    pub(crate) fn from_requirements(requirements: &ConfigRequirements) -> Self {
        let Some(requirements) = requirements.marketplaces.as_ref().filter(|requirements| {
            requirements
                .value
                .restrict_to_allowed_sources
                .unwrap_or(false)
        }) else {
            return Self { restricted: None };
        };

        let allowed_sources = requirements
            .value
            .allowed_sources
            .iter()
            .map(|(key, allowed_source)| {
                compile_allowed_source(key, allowed_source, &requirements.source)
            })
            .collect();
        Self {
            restricted: Some(RestrictedMarketplacePolicy {
                allowed_sources,
                source: requirements.source.clone(),
            }),
        }
    }

    pub(crate) fn is_restricted(&self) -> bool {
        self.restricted.is_some()
    }

    fn validate_source(&self, source: &MarketplaceSource) -> Result<(), String> {
        let Some(RestrictedMarketplacePolicy {
            allowed_sources,
            source: requirement_source,
        }) = &self.restricted
        else {
            return Ok(());
        };
        let allowed_sources = allowed_sources.as_ref().map_err(Clone::clone)?;
        if allowed_sources
            .iter()
            .any(|allowed_source| allowed_source.matches(source))
        {
            return Ok(());
        }

        Err(format!(
            "marketplace source `{}` is not allowed by requirements from {requirement_source}",
            source.display()
        ))
    }

    pub(crate) fn validate_install(
        &self,
        config_layer_stack: &ConfigLayerStack,
        codex_home: &Path,
        marketplace_path: &AbsolutePathBuf,
        marketplace_name: &str,
    ) -> Result<(), String> {
        if !self.is_restricted() {
            return Ok(());
        }

        let root = marketplace_root_dir(marketplace_path).map_err(|err| err.to_string())?;
        if let Some(expected_name) = managed_marketplace_name(codex_home, marketplace_path, &root) {
            return validate_expected_marketplace_name(expected_name, marketplace_name);
        }

        let user_config = config_layer_stack.effective_user_config().ok_or_else(|| {
            format!(
                "marketplace `{marketplace_name}` must be added to config before plugins can be installed while marketplace source restrictions are enabled"
            )
        })?;
        let marketplace = user_config
            .get("marketplaces")
            .and_then(toml::Value::as_table)
            .and_then(|marketplaces| marketplaces.get(marketplace_name))
            .ok_or_else(|| {
                format!(
                    "marketplace `{marketplace_name}` must be added to config before plugins can be installed while marketplace source restrictions are enabled"
                )
            })?;
        self.validate_configured_marketplace(marketplace_name, marketplace)?;

        let configured_root = resolve_configured_marketplace_root(
            marketplace_name,
            marketplace,
            &marketplace_install_root(codex_home),
        )
        .ok_or_else(|| {
            format!("configured marketplace `{marketplace_name}` does not have a usable root")
        })?;
        if !paths_match_after_normalization(&configured_root, root.as_path()) {
            return Err(format!(
                "marketplace path `{}` does not match configured marketplace `{marketplace_name}`",
                root.as_path().display()
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_git_source(
        &self,
        source: &str,
        ref_name: Option<String>,
    ) -> Result<Option<MarketplaceSource>, String> {
        if !self.is_restricted() {
            return Ok(None);
        }
        let source = parse_marketplace_source(source, ref_name).map_err(|err| err.to_string())?;
        if !matches!(source, MarketplaceSource::Git { .. }) {
            return Err("configured Git marketplace source is not a Git URL".to_string());
        }
        self.validate_source(&source)?;
        Ok(Some(source))
    }

    fn validate_configured_marketplace(
        &self,
        marketplace_name: &str,
        marketplace: &toml::Value,
    ) -> Result<(), String> {
        let source = configured_marketplace_source(marketplace_name, marketplace)?;
        self.validate_source(&source)
    }
}

impl AllowedMarketplaceSource {
    fn matches(&self, source: &MarketplaceSource) -> bool {
        match (self, source) {
            (
                Self::GitUrl {
                    url: allowed_url,
                    ref_name: allowed_ref,
                },
                MarketplaceSource::Git { url, ref_name },
            ) => {
                allowed_url == url
                    && allowed_ref
                        .as_ref()
                        .is_none_or(|allowed_ref| Some(allowed_ref) == ref_name.as_ref())
            }
            (Self::GitHostPattern(pattern), MarketplaceSource::Git { url, .. }) => {
                git_hostname(url).is_some_and(|hostname| pattern.is_match(&hostname))
            }
            (Self::Local(allowed), MarketplaceSource::Local { path }) => {
                paths_match_after_normalization(allowed.as_path(), path)
            }
            (Self::GitUrl { .. } | Self::GitHostPattern(_), MarketplaceSource::Local { .. })
            | (Self::Local(_), MarketplaceSource::Git { .. }) => false,
        }
    }
}

pub(crate) fn project_effective_user_config(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> Option<toml::Value> {
    let mut user_config = config_layer_stack.effective_user_config()?;
    let policy = MarketplacePolicy::from_requirements(config_layer_stack.requirements());
    if !policy.is_restricted() {
        return Some(user_config);
    }
    let allowed_marketplace_names =
        allowed_configured_marketplace_names_with_policy(&user_config, &policy, codex_home);
    let configured_marketplace_names = user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
        .map(|marketplaces| marketplaces.keys().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();

    if let Some(marketplaces) = user_config
        .get_mut("marketplaces")
        .and_then(toml::Value::as_table_mut)
    {
        marketplaces
            .retain(|marketplace_name, _| allowed_marketplace_names.contains(marketplace_name));
    }
    if let Some(plugins) = user_config
        .get_mut("plugins")
        .and_then(toml::Value::as_table_mut)
    {
        plugins.retain(|plugin_key, _| {
            let Ok(plugin_id) = PluginId::parse(plugin_key) else {
                return false;
            };
            (is_openai_curated_marketplace_name(&plugin_id.marketplace_name)
                && !configured_marketplace_names.contains(&plugin_id.marketplace_name))
                || allowed_marketplace_names.contains(&plugin_id.marketplace_name)
        });
    }
    Some(user_config)
}

pub fn allowed_configured_marketplace_names(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> HashSet<String> {
    let Some(user_config) = config_layer_stack.effective_user_config() else {
        return HashSet::new();
    };
    let policy = MarketplacePolicy::from_requirements(config_layer_stack.requirements());
    allowed_configured_marketplace_names_with_policy(&user_config, &policy, codex_home)
}

fn allowed_configured_marketplace_names_with_policy(
    user_config: &toml::Value,
    policy: &MarketplacePolicy,
    codex_home: &Path,
) -> HashSet<String> {
    let Some(marketplaces) = user_config
        .get("marketplaces")
        .and_then(toml::Value::as_table)
    else {
        return HashSet::new();
    };
    if !policy.is_restricted() {
        return marketplaces.keys().cloned().collect();
    }
    marketplaces
        .iter()
        .filter_map(|(marketplace_name, marketplace)| {
            let allowed = match managed_marketplace_config_name(codex_home, marketplace) {
                Some(expected_name) => expected_name == marketplace_name,
                None => policy
                    .validate_configured_marketplace(marketplace_name, marketplace)
                    .is_ok(),
            };
            allowed.then(|| marketplace_name.clone())
        })
        .collect()
}

pub(crate) fn configured_plugins_from_stack(
    config_layer_stack: &ConfigLayerStack,
    codex_home: &Path,
) -> HashMap<String, PluginConfig> {
    let Some(user_config) = project_effective_user_config(config_layer_stack, codex_home) else {
        return HashMap::new();
    };
    let Some(plugins_value) = user_config.get("plugins") else {
        return HashMap::new();
    };
    match plugins_value.clone().try_into() {
        Ok(plugins) => plugins,
        Err(err) => {
            tracing::warn!("invalid plugins config: {err}");
            HashMap::new()
        }
    }
}

pub(crate) fn validate_marketplace_source_for_add(
    codex_home: &Path,
    requirements: &ConfigRequirements,
    source: &MarketplaceSource,
) -> Result<Option<&'static str>, String> {
    let policy = MarketplacePolicy::from_requirements(requirements);
    if !policy.is_restricted() {
        return Ok(None);
    }
    if let MarketplaceSource::Local { path } = source
        && let Some(expected_name) = managed_local_marketplace_name(codex_home, path)
    {
        return Ok(Some(expected_name));
    }
    policy.validate_source(source)?;
    Ok(None)
}

pub(crate) fn validate_marketplace_name_for_add(
    expected_name: Option<&'static str>,
    marketplace_name: &str,
) -> Result<(), String> {
    if let Some(expected_name) = expected_name {
        return validate_expected_marketplace_name(expected_name, marketplace_name);
    }
    if is_openai_curated_marketplace_name(marketplace_name) {
        return Err(format!(
            "marketplace `{marketplace_name}` is reserved and cannot be added from this source"
        ));
    }
    Ok(())
}

fn compile_allowed_source(
    key: &str,
    allowed_source: &MarketplaceAllowedSourceToml,
    requirement_source: &RequirementSource,
) -> Result<AllowedMarketplaceSource, String> {
    let invalid = |reason: &str| {
        format!("invalid marketplace allowed source `{key}` in {requirement_source}: {reason}")
    };
    let source = allowed_source
        .source
        .ok_or_else(|| invalid("missing source"))?;
    match source {
        MarketplaceAllowedSourceKind::Git => {
            let url = allowed_source
                .url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .ok_or_else(|| invalid("missing url"))?;
            let ref_name = match allowed_source.ref_name.as_deref() {
                Some(ref_name) if ref_name.trim().is_empty() => {
                    return Err(invalid("ref must not be empty"));
                }
                Some(ref_name) => Some(ref_name.trim().to_string()),
                None => None,
            };
            let source =
                parse_marketplace_source(url, ref_name).map_err(|err| invalid(&err.to_string()))?;
            let MarketplaceSource::Git { url, ref_name } = source else {
                return Err(invalid("expected a Git URL"));
            };
            Ok(AllowedMarketplaceSource::GitUrl { url, ref_name })
        }
        MarketplaceAllowedSourceKind::HostPattern => {
            let host_pattern = allowed_source
                .host_pattern
                .as_deref()
                .map(str::trim)
                .filter(|host_pattern| !host_pattern.is_empty())
                .ok_or_else(|| invalid("missing host_pattern"))?;
            Regex::new(host_pattern)
                .map(AllowedMarketplaceSource::GitHostPattern)
                .map_err(|err| invalid(&err.to_string()))
        }
        MarketplaceAllowedSourceKind::Local => {
            let path = allowed_source
                .path
                .as_ref()
                .filter(|path| !path.as_os_str().is_empty())
                .ok_or_else(|| invalid("missing path"))?;
            if !path.is_absolute() {
                return Err(invalid("local path must be absolute"));
            }
            let path = AbsolutePathBuf::from_absolute_path_checked(path)
                .map_err(|_| invalid("local path must be absolute"))?;
            Ok(AllowedMarketplaceSource::Local(path))
        }
    }
}

fn configured_marketplace_source(
    marketplace_name: &str,
    marketplace: &toml::Value,
) -> Result<MarketplaceSource, String> {
    let MarketplaceConfig {
        source_type,
        source,
        ref_name,
        ..
    } = marketplace
        .clone()
        .try_into()
        .map_err(|err| format!("invalid config for marketplace `{marketplace_name}`: {err}"))?;
    let source_type = source_type.ok_or_else(|| {
        format!("configured marketplace `{marketplace_name}` is missing source_type")
    })?;
    let source = source
        .ok_or_else(|| format!("configured marketplace `{marketplace_name}` is missing source"))?;
    match source_type {
        MarketplaceSourceType::Local => Ok(MarketplaceSource::Local {
            path: PathBuf::from(source),
        }),
        MarketplaceSourceType::Git => {
            let parsed = parse_marketplace_source(&source, ref_name).map_err(|err| {
                format!("invalid source for marketplace `{marketplace_name}`: {err}")
            })?;
            if matches!(parsed, MarketplaceSource::Git { .. }) {
                Ok(parsed)
            } else {
                Err(format!(
                    "configured marketplace `{marketplace_name}` source does not match source_type `git`"
                ))
            }
        }
    }
}

fn validate_expected_marketplace_name(
    expected_name: &str,
    marketplace_name: &str,
) -> Result<(), String> {
    (marketplace_name == expected_name)
        .then_some(())
        .ok_or_else(|| {
            format!(
                "marketplace manifest name `{marketplace_name}` does not match managed marketplace `{expected_name}`"
            )
        })
}

fn managed_marketplace_name(
    codex_home: &Path,
    marketplace_path: &AbsolutePathBuf,
    root: &AbsolutePathBuf,
) -> Option<&'static str> {
    if paths_match_after_normalization(
        marketplace_path.as_path(),
        curated_plugins_api_marketplace_path(codex_home),
    ) {
        return Some(OPENAI_API_CURATED_MARKETPLACE_NAME);
    }
    if paths_match_after_normalization(root.as_path(), curated_plugins_repo_path(codex_home)) {
        return Some(OPENAI_CURATED_MARKETPLACE_NAME);
    }
    managed_local_marketplace_name(codex_home, root.as_path())
}

fn managed_marketplace_config_name(
    codex_home: &Path,
    marketplace: &toml::Value,
) -> Option<&'static str> {
    if marketplace.get("source_type").and_then(toml::Value::as_str) != Some("local") {
        return None;
    }
    let path = marketplace
        .get("source")
        .and_then(toml::Value::as_str)
        .map(Path::new)
        .filter(|path| path.is_absolute())?;
    managed_local_marketplace_name(codex_home, path)
}

fn managed_local_marketplace_name(codex_home: &Path, root: &Path) -> Option<&'static str> {
    for marketplace_name in [
        OPENAI_BUNDLED_MARKETPLACE_NAME,
        OPENAI_BUNDLED_ALPHA_MARKETPLACE_NAME,
    ] {
        let expected_root = codex_home
            .join(".tmp/bundled-marketplaces")
            .join(marketplace_name);
        if paths_match_after_normalization(root, &expected_root) {
            return Some(marketplace_name);
        }
    }

    let runtime_root = dirs::cache_dir()?
        .join("codex-runtimes/codex-primary-runtime/plugins")
        .join(OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME);
    paths_match_after_normalization(root, &runtime_root)
        .then_some(OPENAI_PRIMARY_RUNTIME_MARKETPLACE_NAME)
}

fn git_hostname(url: &str) -> Option<String> {
    if let Ok(url) = Url::parse(url) {
        return url.host_str().map(str::to_ascii_lowercase);
    }
    let (_, host_and_path) = url.split_once('@')?;
    let (hostname, _) = host_and_path.split_once(':')?;
    (!hostname.is_empty()).then(|| hostname.to_ascii_lowercase())
}

#[cfg(test)]
#[path = "marketplace_policy_tests.rs"]
mod tests;
