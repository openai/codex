//! Immutable cloud skill snapshots for offline sources. Only declared package resources are
//! readable; the normal skills extension owns aliases, tool schemas, rendering, and pagination.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use serde::Deserialize;

use super::MAX_SKILL_RESOURCE_CONTENT_BYTES;
use super::SkillListQuery;
use super::SkillProvider;
use super::SkillProviderFuture;
use super::SkillReadRequest;
use super::SkillSearchRequest;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillProviderResult;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;

const MAX_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    skills: Vec<SnapshotSkill>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotSkill {
    package: String,
    name: String,
    description: String,
    alias_root: String,
    main_resource: String,
    #[serde(default = "default_prompt_visible")]
    prompt_visible: bool,
    resources: HashMap<String, String>,
}

fn default_prompt_visible() -> bool {
    true
}

/// Supplies a fixed cloud catalog and resource contents without contacting a skill service.
pub struct FrozenCloudSkillProvider {
    catalog: SkillCatalog,
    resources: HashMap<SkillPackageId, HashMap<String, String>>,
}

impl FrozenCloudSkillProvider {
    /// Loads once from a host-owned file; subsequent reads never access the filesystem.
    pub fn from_file(path: &Path) -> SkillProviderResult<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .and_then(|file| {
                file.take((MAX_SNAPSHOT_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
            })
            .map_err(|err| SkillProviderError::new(format!("cannot read skill snapshot: {err}")))?;
        Self::from_json(&bytes)
    }

    /// Rejects missing main resources, duplicate packages, and resources outside their package.
    pub fn from_json(bytes: &[u8]) -> SkillProviderResult<Self> {
        if bytes.len() > MAX_SNAPSHOT_BYTES {
            return Err(SkillProviderError::new("skill snapshot exceeds 64 MiB"));
        }
        let snapshot: Snapshot = serde_json::from_slice(bytes)
            .map_err(|err| SkillProviderError::new(format!("invalid skill snapshot: {err}")))?;
        let mut catalog = SkillCatalog::default();
        let mut resources = HashMap::new();
        for skill in snapshot.skills {
            let package = SkillPackageId(skill.package);
            let root = SkillPackageId(skill.alias_root.clone());
            if !skill.alias_root.starts_with("skill://")
                || root.relative_resource_path(&package.0).is_none()
                || skill.name.trim().is_empty()
                || !skill.resources.contains_key(&skill.main_resource)
                || skill.resources.iter().any(|(resource, contents)| {
                    package.relative_resource_path(resource).is_none()
                        || contents.len() > MAX_SKILL_RESOURCE_CONTENT_BYTES
                })
            {
                return Err(SkillProviderError::new("invalid skill package or resource"));
            }
            let mut entry = SkillCatalogEntry::new(
                package.clone(),
                SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME),
                skill.name,
                skill.description,
                SkillResourceId::new(skill.main_resource),
            )
            .with_display_path(&package.0)
            .with_alias_root(skill.alias_root);
            if !skill.prompt_visible {
                entry = entry.hidden_from_prompt();
            }
            catalog.entries.push(entry);
            if resources.insert(package, skill.resources).is_some() {
                return Err(SkillProviderError::new("duplicate skill package"));
            }
        }
        Ok(Self { catalog, resources })
    }
}

impl SkillProvider for FrozenCloudSkillProvider {
    fn uses_mcp_resources(&self) -> bool {
        false
    }

    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            Ok(if query.include_cloud_skills {
                self.catalog.clone()
            } else {
                SkillCatalog::default()
            })
        })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        Box::pin(async move {
            if request.authority
                != SkillAuthority::new(SkillSourceKind::Cloud, CODEX_APPS_MCP_SERVER_NAME)
            {
                return Err(SkillProviderError::new("invalid frozen skill authority"));
            }
            let contents = self
                .resources
                .get(&request.package)
                .and_then(|resources| resources.get(request.resource.as_str()))
                .ok_or_else(|| SkillProviderError::new("unknown frozen skill resource"))?;
            Ok(SkillReadResult {
                resource: request.resource,
                contents: contents.clone(),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async {
            Err(SkillProviderError::new(
                "frozen skill snapshots do not support search",
            ))
        })
    }
}

#[cfg(test)]
#[path = "frozen_cloud_tests.rs"]
mod tests;
