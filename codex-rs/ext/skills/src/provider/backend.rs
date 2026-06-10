use std::collections::HashSet;
use std::time::Duration;

use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceContent;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::provider::SkillProvider;
use crate::provider::SkillProviderFuture;
use crate::provider::SkillReadRequest;
use crate::provider::SkillSearchRequest;

const BACKEND_SKILL_MIME_TYPE: &str = "mcp/skill";
const BACKEND_SKILL_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
const BACKEND_SKILL_READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESOURCE_PAGES: usize = 10;
const MAX_BACKEND_SKILLS: usize = 100;
const MAX_SKILL_NAME_CHARS: usize = 64;
const MAX_QUALIFIED_SKILL_NAME_CHARS: usize = 128;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 1_024;
const MAX_SKILL_URI_CHARS: usize = 1_024;

/// Discovers and reads backend skills through a session-owned MCP connection.
#[derive(Clone, Debug)]
pub struct BackendSkillProvider {
    server_name: String,
}

impl BackendSkillProvider {
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
        }
    }
}

impl SkillProvider for BackendSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            let Some(client) = query.mcp_resources else {
                return Ok(SkillCatalog::default());
            };
            if !client.has_server(&self.server_name).await {
                return Ok(SkillCatalog::default());
            }

            let discovery_deadline = tokio::time::Instant::now() + BACKEND_SKILL_DISCOVERY_TIMEOUT;
            let mut catalog = SkillCatalog::default();
            let mut cursor = None;
            let mut seen_cursors = HashSet::new();
            let mut skill_resources_seen = 0usize;
            let mut skipped_resources = 0usize;
            let mut truncated = false;
            let mut completed_pages = 0usize;

            for _ in 0..MAX_RESOURCE_PAGES {
                let page = match tokio::time::timeout_at(
                    discovery_deadline,
                    client.list_resources(&self.server_name, cursor.clone()),
                )
                .await
                {
                    Ok(result) => result.map_err(|err| {
                        SkillProviderError::new(format!(
                            "failed to list backend skill resources from {}: {err:#}",
                            self.server_name
                        ))
                    }),
                    Err(_) => {
                        let server_name = &self.server_name;
                        Err(SkillProviderError::new(format!(
                            "backend skill discovery from {server_name} timed out after {BACKEND_SKILL_DISCOVERY_TIMEOUT:?}"
                        )))
                    }
                };
                let result = match page {
                    Ok(result) => result,
                    Err(err) if completed_pages == 0 => return Err(err),
                    Err(err) => {
                        let page_word = if completed_pages == 1 {
                            "page"
                        } else {
                            "pages"
                        };
                        catalog.warnings.push(format!(
                            "Backend skill discovery from {} stopped after {completed_pages} resource {page_word}: {}",
                            self.server_name, err.message
                        ));
                        cursor = None;
                        break;
                    }
                };
                completed_pages = completed_pages.saturating_add(1);

                for resource in &result.resources {
                    if resource.mime_type.as_deref() != Some(BACKEND_SKILL_MIME_TYPE) {
                        continue;
                    }
                    if skill_resources_seen >= MAX_BACKEND_SKILLS {
                        truncated = true;
                        break;
                    }
                    skill_resources_seen = skill_resources_seen.saturating_add(1);
                    match catalog_entry_from_resource(resource, &self.server_name) {
                        Some(entry) => catalog.push_entry(entry),
                        None => skipped_resources = skipped_resources.saturating_add(1),
                    }
                }

                if truncated {
                    break;
                }
                let Some(next_cursor) = result.next_cursor else {
                    cursor = None;
                    break;
                };
                if !seen_cursors.insert(next_cursor.clone()) {
                    catalog.warnings.push(format!(
                        "Backend skill resource pagination from {} returned a duplicate cursor.",
                        self.server_name
                    ));
                    cursor = None;
                    break;
                }
                cursor = Some(next_cursor);
            }

            if cursor.is_some() || truncated {
                catalog.warnings.push(format!(
                    "Backend skill discovery from {} was truncated at {MAX_BACKEND_SKILLS} skills or {MAX_RESOURCE_PAGES} resource pages.",
                    self.server_name
                ));
            }
            if skipped_resources > 0 {
                catalog.warnings.push(format!(
                    "Skipped {skipped_resources} malformed backend skill resources from {}.",
                    self.server_name
                ));
            }

            Ok(catalog)
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            if request.authority
                != SkillAuthority::new(SkillSourceKind::Remote, self.server_name.clone())
            {
                return Err(SkillProviderError::new(format!(
                    "backend skill provider cannot read authority {}",
                    request.authority.id
                )));
            }
            let expected_resource = main_prompt_uri(&request.package.0);
            if request.resource.as_str() != expected_resource {
                return Err(SkillProviderError::new(
                    "backend skill resource does not match its package",
                ));
            }

            let Some(client) = request.mcp_resources.as_ref() else {
                return Err(SkillProviderError::new(
                    "session MCP resource client is not configured",
                ));
            };
            let result = tokio::time::timeout(
                BACKEND_SKILL_READ_TIMEOUT,
                client.read_resource(&self.server_name, request.resource.as_str()),
            )
            .await
            .map_err(|_| {
                SkillProviderError::new(format!(
                    "backend skill read from {} timed out after {BACKEND_SKILL_READ_TIMEOUT:?}",
                    self.server_name
                ))
            })?
            .map_err(|err| {
                SkillProviderError::new(format!(
                    "failed to read backend skill resource {}: {err:#}",
                    request.resource.as_str()
                ))
            })?;
            let contents = result
                .contents
                .into_iter()
                .find_map(|contents| match contents {
                    ResourceContent::Text { uri, text, .. } if uri == request.resource.as_str() => {
                        Some(text)
                    }
                    ResourceContent::Text { .. } | ResourceContent::Blob { .. } => None,
                });
            let Some(contents) = contents else {
                return Err(SkillProviderError::new(format!(
                    "backend skill resource {} did not return matching text contents",
                    request.resource.as_str()
                )));
            };

            Ok(SkillReadResult {
                resource: request.resource,
                contents,
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn catalog_entry_from_resource(
    resource: &Resource,
    server_name: &str,
) -> Option<SkillCatalogEntry> {
    let uri = validated_skill_uri(resource.uri.as_str())?;
    let meta = resource.meta.as_ref()?.as_object()?;
    let skill_name = normalized_label(meta.get("skill_name")?.as_str()?, MAX_SKILL_NAME_CHARS)?;
    let name = if meta.get("source").and_then(|value| value.as_str()) == Some("user") {
        skill_name
    } else {
        let plugin_name =
            normalized_label(meta.get("plugin_name")?.as_str()?, MAX_SKILL_NAME_CHARS)?;
        let qualified_name = format!("{plugin_name}:{skill_name}");
        (qualified_name.chars().count() <= MAX_QUALIFIED_SKILL_NAME_CHARS)
            .then_some(qualified_name)?
    };
    let description = normalized_description(resource.description.as_deref().unwrap_or_default())?;
    let main_prompt = main_prompt_uri(uri);

    Some(
        SkillCatalogEntry::new(
            SkillPackageId(uri.to_string()),
            SkillAuthority::new(SkillSourceKind::Remote, server_name),
            name,
            description,
            SkillResourceId::new(main_prompt),
        )
        .with_display_path(uri),
    )
}

fn validated_skill_uri(uri: &str) -> Option<&str> {
    let path = uri.strip_prefix("skill://")?;
    let invalid = path.is_empty()
        || uri.chars().count() > MAX_SKILL_URI_CHARS
        || uri
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>'));
    (!invalid).then_some(uri)
}

fn normalized_label(value: &str, max_chars: usize) -> Option<String> {
    let value = normalized_single_line(value, max_chars)?;
    let invalid = value.is_empty() || value.chars().any(|ch| matches!(ch, '&' | '<' | '>'));
    (!invalid).then_some(value)
}

fn normalized_description(value: &str) -> Option<String> {
    normalized_single_line(value, MAX_SKILL_DESCRIPTION_CHARS).map(|value| {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    })
}

fn normalized_single_line(value: &str, max_chars: usize) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let valid = value.chars().count() <= max_chars && !value.chars().any(char::is_control);
    valid.then_some(value)
}

fn main_prompt_uri(package_uri: &str) -> String {
    format!("{}/SKILL.md", package_uri.trim_end_matches('/'))
}
