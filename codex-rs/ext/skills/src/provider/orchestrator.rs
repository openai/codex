use std::collections::HashSet;
use std::time::Duration;

use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceContent;
use url::Url;

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

const ORCHESTRATOR_SKILL_MIME_TYPE: &str = "mcp/skill";
const ORCHESTRATOR_SKILL_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
const ORCHESTRATOR_SKILL_READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESOURCE_PAGES: usize = 10;
const MAX_ORCHESTRATOR_SKILLS: usize = 100;
const MAX_SKILL_NAME_CHARS: usize = 64;
const MAX_QUALIFIED_SKILL_NAME_CHARS: usize = 128;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 1_024;
const MAX_SKILL_PACKAGE_URI_CHARS: usize = 1_024;
const MAX_SKILL_RESOURCE_URI_CHARS: usize = 2_048;
const MAX_SKILL_RESOURCE_CONTENT_BYTES: usize = 1024 * 1024;

/// Discovers and reads skills owned by the orchestrator.
///
/// The provider uses session-scoped resources without exposing the transport or
/// resource server to callers that configure the skills extension.
#[derive(Clone, Debug, Default)]
pub struct OrchestratorSkillProvider;

impl OrchestratorSkillProvider {
    pub fn new() -> Self {
        Self
    }
}

impl SkillProvider for OrchestratorSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            let Some(client) = query.mcp_resources else {
                return Ok(SkillCatalog::default());
            };
            if !client.has_server(CODEX_APPS_MCP_SERVER_NAME).await {
                return Ok(SkillCatalog::default());
            }

            let discovery_deadline =
                tokio::time::Instant::now() + ORCHESTRATOR_SKILL_DISCOVERY_TIMEOUT;
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
                    client.list_resources(CODEX_APPS_MCP_SERVER_NAME, cursor.clone()),
                )
                .await
                {
                    Ok(result) => result.map_err(|err| {
                        SkillProviderError::new(format!(
                            "failed to list orchestrator skill resources: {err:#}"
                        ))
                    }),
                    Err(_) => Err(SkillProviderError::new(format!(
                        "orchestrator skill discovery timed out after {ORCHESTRATOR_SKILL_DISCOVERY_TIMEOUT:?}"
                    ))),
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
                            "Orchestrator skill discovery stopped after {completed_pages} resource {page_word}: {}",
                            err.message
                        ));
                        cursor = None;
                        break;
                    }
                };
                completed_pages = completed_pages.saturating_add(1);

                for resource in &result.resources {
                    if resource.mime_type.as_deref() != Some(ORCHESTRATOR_SKILL_MIME_TYPE) {
                        continue;
                    }
                    if skill_resources_seen >= MAX_ORCHESTRATOR_SKILLS {
                        truncated = true;
                        break;
                    }
                    skill_resources_seen = skill_resources_seen.saturating_add(1);
                    match catalog_entry_from_resource(resource) {
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
                    catalog.warnings.push(
                        "Orchestrator skill resource pagination returned a duplicate cursor."
                            .to_string(),
                    );
                    cursor = None;
                    break;
                }
                cursor = Some(next_cursor);
            }

            if cursor.is_some() || truncated {
                catalog.warnings.push(format!(
                    "Orchestrator skill discovery was truncated at {MAX_ORCHESTRATOR_SKILLS} skills or {MAX_RESOURCE_PAGES} resource pages."
                ));
            }
            if skipped_resources > 0 {
                catalog.warnings.push(format!(
                    "Skipped {skipped_resources} malformed orchestrator skill resources."
                ));
            }

            Ok(catalog)
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            if request.authority
                != SkillAuthority::new(SkillSourceKind::Orchestrator, CODEX_APPS_MCP_SERVER_NAME)
            {
                return Err(SkillProviderError::new(format!(
                    "orchestrator skill provider cannot read authority {}",
                    request.authority.id
                )));
            }
            if !resource_belongs_to_package(&request.package.0, request.resource.as_str()) {
                return Err(SkillProviderError::new(
                    "orchestrator skill resource does not match its package",
                ));
            }

            let Some(client) = request.mcp_resources.as_ref() else {
                return Err(SkillProviderError::new(
                    "session MCP resource client is not configured",
                ));
            };
            let result = tokio::time::timeout(
                ORCHESTRATOR_SKILL_READ_TIMEOUT,
                client.read_resource(CODEX_APPS_MCP_SERVER_NAME, request.resource.as_str()),
            )
            .await
            .map_err(|_| {
                SkillProviderError::new(format!(
                    "orchestrator skill read timed out after {ORCHESTRATOR_SKILL_READ_TIMEOUT:?}"
                ))
            })?
            .map_err(|err| {
                SkillProviderError::new(format!(
                    "failed to read orchestrator skill resource {}: {err:#}",
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
                    "orchestrator skill resource {} did not return matching text contents",
                    request.resource.as_str()
                )));
            };
            if contents.len() > MAX_SKILL_RESOURCE_CONTENT_BYTES {
                return Err(SkillProviderError::new(format!(
                    "orchestrator skill resource {} exceeds the {MAX_SKILL_RESOURCE_CONTENT_BYTES}-byte read limit",
                    request.resource.as_str()
                )));
            }

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

fn catalog_entry_from_resource(resource: &Resource) -> Option<SkillCatalogEntry> {
    let uri = validated_skill_uri(resource.uri.as_str(), MAX_SKILL_PACKAGE_URI_CHARS)?;
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
            SkillAuthority::new(SkillSourceKind::Orchestrator, CODEX_APPS_MCP_SERVER_NAME),
            name,
            description,
            SkillResourceId::new(main_prompt),
        )
        .with_display_path(uri),
    )
}

fn validated_skill_uri(uri: &str, max_chars: usize) -> Option<&str> {
    validated_skill_url(uri, max_chars).map(|_| uri)
}

fn validated_skill_url(uri: &str, max_chars: usize) -> Option<Url> {
    if uri.chars().count() > max_chars
        || uri
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || matches!(ch, '<' | '>'))
    {
        return None;
    }

    let url = Url::parse(uri).ok()?;
    let path_is_valid = url.path_segments().is_some_and(|segments| {
        let segments = segments.collect::<Vec<_>>();
        !segments.is_empty() && segments.iter().all(|segment| !segment.is_empty())
    });
    (url.scheme() == "skill"
        && url.as_str() == uri
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && path_is_valid)
        .then_some(url)
}

fn resource_belongs_to_package(package: &str, resource: &str) -> bool {
    let Some(package) = validated_skill_url(package, MAX_SKILL_PACKAGE_URI_CHARS) else {
        return false;
    };
    let Some(resource) = validated_skill_url(resource, MAX_SKILL_RESOURCE_URI_CHARS) else {
        return false;
    };

    let Some(package_segments) = package.path_segments() else {
        return false;
    };
    let Some(resource_segments) = resource.path_segments() else {
        return false;
    };
    let package_segments = package_segments.collect::<Vec<_>>();
    let resource_segments = resource_segments.collect::<Vec<_>>();

    package.scheme() == resource.scheme()
        && package.host_str() == resource.host_str()
        && resource_segments.len() > package_segments.len()
        && resource_segments.starts_with(&package_segments)
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
