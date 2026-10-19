//! Loads metadata within the request's sandbox and builds a bounded response.

use std::collections::HashMap;
use std::io;
use std::sync::Arc;

use codex_file_system::ExecutorFileSystem;
use codex_skills::parse_skill_frontmatter_metadata;
use codex_skills::parse_skill_metadata;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;

use crate::CapabilityRootDiscoverRequest;
use crate::CapabilityRootDiscovery;
use crate::CapabilityTextFile;
use crate::DiscoverV2CapabilitiesResponse;
use crate::ExecutorPlugin;
use crate::ExecutorSkill;
use crate::LocalFileSystem;
use crate::SkillDiscoveryRoot;
use crate::capability_discovery::MAX_CONCURRENT_ROOTS;
use crate::capability_discovery::discover_root;
use crate::discover_v2::capability_locations::CapabilityLocation;
use crate::discover_v2::capability_locations::CapabilityLocations;

const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CAPABILITIES: usize = 2_048;
const MAX_WARNINGS: usize = 64;
// Match V1: namespace, separator, and skill name.
const MAX_QUALIFIED_NAME_LEN: usize = 64 * 2 + 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum DiscoveredSource {
    Plugin(Box<ExecutorPlugin>),
    Skills(Vec<ExecutorSkill>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CapabilityDiscovery {
    content: DiscoveredSource,
    warnings: Vec<String>,
}

// Only the metadata V2 returns or needs to materialize discovered configuration.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifestMetadata {
    #[serde(default)]
    name: String,
    interface: Option<PluginManifestInterface>,
    mcp_servers: Option<serde_json::Value>,
    apps: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginManifestInterface {
    display_name: Option<String>,
}

impl Default for CapabilityDiscovery {
    fn default() -> Self {
        Self {
            content: DiscoveredSource::Skills(Vec::new()),
            warnings: Vec::new(),
        }
    }
}

pub(super) async fn load_capability_discoveries(
    file_system: &LocalFileSystem,
    locations: &CapabilityLocations,
    sandbox: Option<&crate::FileSystemSandboxContext>,
) -> io::Result<Arc<DiscoverV2CapabilitiesResponse>> {
    let sandbox = sandbox.filter(|sandbox| sandbox.should_read_from_sandbox());
    let warnings = if !locations.warnings.is_empty() {
        vec!["some capability locations could not be resolved; discovery may be incomplete".into()]
    } else {
        Vec::new()
    };
    let response = match sandbox {
        Some(sandbox) if !locations.locations.is_empty() => {
            file_system
                .sandboxed()?
                .load_sandboxed_capability_discoveries(
                    locations.locations.clone(),
                    warnings,
                    sandbox,
                )
                .await?
        }
        _ => load_capability_discovery_batch(file_system, &locations.locations, warnings).await?,
    };
    Ok(Arc::new(response))
}

/// Loads roots concurrently and bounds the combined response.
pub(crate) async fn load_capability_discovery_batch(
    file_system: &LocalFileSystem,
    locations: &[CapabilityLocation],
    warnings: Vec<String>,
) -> io::Result<DiscoverV2CapabilitiesResponse> {
    let mut response = DiscoverV2CapabilitiesResponse {
        warnings,
        ..Default::default()
    };
    let mut budget = ResponseBudget::default();
    // Use V1 concurrency; admit results in location order.
    // Yield owned locations to satisfy the RPC router's Send future bound.
    let mut discoveries = futures::stream::iter(locations.iter().cloned())
        .map(|location| load_capability_discovery(file_system, location))
        .buffered(MAX_CONCURRENT_ROOTS)
        .enumerate();
    while let Some((index, discovery)) = discoveries.next().await {
        let mut discovery = discovery?;
        if !discovery.warnings.is_empty() {
            discovery.warnings = vec![
                "some capability metadata could not be loaded; discovery may be incomplete".into(),
            ];
        }
        append_capability_discovery(&mut response, discovery, &mut budget)?;
        // A rejected oversized entry alone does not rule out smaller entries later.
        if (budget.capabilities == MAX_CAPABILITIES || budget.bytes == MAX_RESPONSE_BYTES)
            && index + 1 < locations.len()
        {
            budget.truncated = true;
            break;
        }
    }
    finish_response(response, budget.truncated)
}

async fn load_capability_discovery(
    file_system: &LocalFileSystem,
    location: CapabilityLocation,
) -> io::Result<CapabilityDiscovery> {
    // Missing roots are normal.
    match file_system
        .get_metadata(&location.root, Default::default(), /*sandbox*/ None)
        .await
    {
        Ok(metadata) if metadata.is_directory => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(CapabilityDiscovery::default());
        }
        Ok(_) => return Ok(CapabilityDiscovery::default()),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            return Ok(CapabilityDiscovery::default());
        }
        Err(error) => {
            return Ok(CapabilityDiscovery {
                warnings: vec![format!(
                    "could not inspect capability root {}: {error}",
                    location.root
                )],
                ..Default::default()
            });
        }
    }
    let discovery = discover_root(
        file_system,
        CapabilityRootDiscoverRequest {
            id: location
                .plugin
                .as_ref()
                .map(|plugin| plugin.id.clone())
                .unwrap_or_default(),
            path: location.root.clone(),
            sandbox: None,
        },
    )
    .await;
    Ok(parse_root(location, discovery))
}

fn parse_root(
    location: CapabilityLocation,
    discovery: CapabilityRootDiscovery,
) -> CapabilityDiscovery {
    let mut warnings = discovery.warnings;
    warnings.extend(discovery.error);
    if location.plugin.is_some() && discovery.plugin.is_none() {
        warnings.push(format!(
            "installed plugin at {} has no readable manifest",
            location.root
        ));
        return CapabilityDiscovery {
            warnings,
            ..Default::default()
        };
    }

    // V1 precedence: first manifest per root, nearest root per skill.
    let mut plugin_namespaces = HashMap::new();
    for manifest in &discovery.namespace_manifests {
        #[derive(Deserialize)]
        struct ManifestName {
            #[serde(default)]
            name: String,
        }
        let Some(root) = manifest.path.parent().and_then(|path| path.parent()) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<ManifestName>(&manifest.contents) else {
            continue;
        };
        let name = if parsed.name.trim().is_empty() {
            let Some(name) = root.basename() else {
                continue;
            };
            name
        } else {
            parsed.name
        };
        plugin_namespaces.entry(root).or_insert(name);
    }

    // Reuse the contents already read by V1.
    let mut skills = Vec::new();
    'skills: for skill in discovery.skills {
        let parsed = parse_skill_frontmatter_metadata(&skill.instructions.contents, || {
            skill
                .instructions
                .path
                .parent()
                .and_then(|path| path.basename())
                .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "skill".to_string())
        });
        let parsed = match parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                warnings.push(format!(
                    "invalid skill metadata at {}: {error}",
                    skill.instructions.path
                ));
                continue;
            }
        };
        let path = skill.instructions.path;
        let mut ancestor = path.parent();
        let mut namespace = None;
        while let Some(root) = ancestor {
            if let Some(name) = plugin_namespaces.get(&root) {
                // Check V1's name limit before cloning the namespace.
                if name.chars().take(MAX_QUALIFIED_NAME_LEN + 1).count()
                    + 1
                    + parsed.name.chars().count()
                    > MAX_QUALIFIED_NAME_LEN
                {
                    warnings.push(format!(
                        "invalid skill metadata at {path}: qualified name exceeds {MAX_QUALIFIED_NAME_LEN} characters"
                    ));
                    continue 'skills;
                }
                namespace = Some(name.clone());
                break;
            }
            ancestor = root.parent();
        }
        let metadata = skill.metadata.as_ref().and_then(|metadata| {
            parse_skill_metadata(&metadata.contents).map_err(|error| {
                tracing::warn!(path = %metadata.path, %error, "ignoring invalid discovered skill metadata");
            }).ok()
        }).unwrap_or_default();
        skills.push(ExecutorSkill {
            name: namespace
                .map(|namespace| format!("{namespace}:{}", parsed.name))
                .unwrap_or(parsed.name),
            description: parsed.description,
            short_description: parsed.short_description,
            path,
            discovery_root: location
                .standalone_root_id
                .as_ref()
                .map(|id| SkillDiscoveryRoot {
                    id: id.clone(),
                    path: location.root.clone(),
                }),
            dependencies: metadata.dependencies,
            policy: metadata.policy,
        });
    }
    let content = if let (Some(plugin), Some(mut files)) = (location.plugin, discovery.plugin) {
        let manifest =
            match serde_json::from_str::<PluginManifestMetadata>(&files.manifest.contents) {
                Ok(manifest) => manifest,
                Err(_) => {
                    warnings.push(format!(
                        "invalid plugin manifest at {}",
                        files.manifest.path
                    ));
                    return CapabilityDiscovery {
                        warnings,
                        ..Default::default()
                    };
                }
            };
        let name = if manifest.name.trim().is_empty() {
            location.root.basename().unwrap_or(manifest.name)
        } else {
            manifest.name
        };
        let display_name = manifest
            .interface
            .and_then(|interface| interface.display_name)
            .map(|display_name| display_name.trim().to_string())
            .filter(|display_name| !display_name.is_empty())
            .unwrap_or(name);
        // V2 materializes inline declarations; the path identifies their source manifest.
        if files.mcp_config.is_none() {
            if let Some(contents) = manifest
                .mcp_servers
                .as_ref()
                .filter(|value| value.is_object())
            {
                files.mcp_config = Some(CapabilityTextFile {
                    path: files.manifest.path.clone(),
                    contents: contents.to_string(),
                });
            } else if manifest
                .mcp_servers
                .as_ref()
                .is_some_and(serde_json::Value::is_string)
            {
                warnings.push(format!(
                    "plugin `{}` at {} declares an MCP configuration file, but discovery returned no contents",
                    plugin.id, location.root
                ));
            }
        }

        if files.apps_config.is_none() && manifest.apps.is_some() {
            warnings.push(format!(
                "plugin `{}` at {} declares an Apps configuration file, but discovery returned no contents",
                plugin.id, location.root
            ));
        }
        DiscoveredSource::Plugin(Box::new(ExecutorPlugin {
            id: plugin.id,
            display_name,
            remote_plugin_id: plugin.remote_plugin_id,
            version: plugin.version,
            root: location.root,
            files,
            skills,
        }))
    } else {
        DiscoveredSource::Skills(skills)
    };
    CapabilityDiscovery { content, warnings }
}

#[derive(Default)]
struct ResponseBudget {
    capabilities: usize,
    bytes: usize,
    truncated: bool,
}

impl ResponseBudget {
    fn admit(&mut self, capabilities: usize, bytes: usize) -> bool {
        if self.capabilities + capabilities > MAX_CAPABILITIES
            || self.bytes + bytes > MAX_RESPONSE_BYTES
        {
            self.truncated = true;
            return false;
        }
        self.capabilities += capabilities;
        self.bytes += bytes;
        true
    }
}

/// Appends within budget; duplicate skills count until consumers deduplicate them.
fn append_capability_discovery(
    response: &mut DiscoverV2CapabilitiesResponse,
    discovery: CapabilityDiscovery,
    budget: &mut ResponseBudget,
) -> io::Result<()> {
    response.warnings.extend(
        discovery
            .warnings
            .into_iter()
            .take(MAX_WARNINGS.saturating_sub(response.warnings.len())),
    );

    match discovery.content {
        DiscoveredSource::Plugin(plugin) => {
            let remaining = MAX_CAPABILITIES - budget.capabilities;
            if remaining == 0 {
                budget.truncated = true;
                return Ok(());
            }
            let mut plugin = *plugin;
            budget.truncated |= plugin.skills.len() > remaining - 1;
            plugin.skills.truncate(remaining - 1); // The plugin itself also counts.
            let size = serde_json::to_vec(&plugin).map_err(io::Error::other)?.len();
            if budget.admit(1 + plugin.skills.len(), size) {
                response.plugins.push(plugin);
            }
        }
        DiscoveredSource::Skills(skills) => {
            for skill in skills {
                if budget.capabilities == MAX_CAPABILITIES {
                    budget.truncated = true;
                    break;
                }
                let size = serde_json::to_vec(&skill).map_err(io::Error::other)?.len();
                if budget.admit(/*capabilities*/ 1, size) {
                    response.skills.push(skill);
                }
            }
        }
    }
    Ok(())
}

fn finish_response(
    mut response: DiscoverV2CapabilitiesResponse,
    truncated: bool,
) -> io::Result<DiscoverV2CapabilitiesResponse> {
    response.plugins.sort_unstable_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.root.to_string().cmp(&right.root.to_string()))
    });
    // Keep standalone skill precedence.
    response.warnings.truncate(MAX_WARNINGS);

    enforce_response_size(&mut response, truncated)?;
    Ok(response)
}

// Include JSON overhead and warnings in the final size check.
fn enforce_response_size(
    response: &mut DiscoverV2CapabilitiesResponse,
    truncated: bool,
) -> io::Result<()> {
    if truncated
        || serde_json::to_vec(&*response)
            .map_err(io::Error::other)?
            .len()
            > MAX_RESPONSE_BYTES
    {
        response.warnings.truncate(MAX_WARNINGS - 1);
        response
            .warnings
            .insert(0, "capability discovery size limit reached".to_string());
        while serde_json::to_vec(&*response)
            .map_err(io::Error::other)?
            .len()
            > MAX_RESPONSE_BYTES
        {
            // Drop extra warnings first; keep the limit warning.
            if response.warnings.len() > 1 {
                response.warnings.truncate(1);
            } else if response.skills.pop().is_none() && response.plugins.pop().is_none() {
                break;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/capability_discoveries_tests.rs"]
mod tests;
