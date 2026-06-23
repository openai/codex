use std::path::PathBuf;
use std::sync::Arc;

use codex_core_skills::SkillMetadata;
use codex_core_skills::filter_skill_load_outcome_for_product;
use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

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

/// Discovers and reads skills through the filesystem owned by an execution environment.
#[derive(Clone, Debug)]
pub struct ExecutorSkillProvider {
    environment_manager: Arc<EnvironmentManager>,
    restriction_product: Option<Product>,
}

impl ExecutorSkillProvider {
    pub fn new_with_restriction_product(
        environment_manager: Arc<EnvironmentManager>,
        restriction_product: Option<Product>,
    ) -> Self {
        Self {
            environment_manager,
            restriction_product,
        }
    }
}

impl SkillProvider for ExecutorSkillProvider {
    fn list(&self, query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            let mut catalog = SkillCatalog::default();
            for selected_root in query.executor_roots {
                let selected_root_id = selected_root.id;
                let CapabilityRootLocation::Environment {
                    environment_id,
                    path,
                } = selected_root.location;
                let authority =
                    SkillAuthority::new(SkillSourceKind::Executor, selected_root_id.clone());
                let Some(environment) = self.environment_manager.get_environment(&environment_id)
                else {
                    catalog.warnings.push(format!(
                        "Selected capability root `{selected_root_id}` references unavailable environment `{environment_id}`."
                    ));
                    continue;
                };
                let root_path = match executor_absolute_path(&path) {
                    Ok(root_path) => root_path,
                    Err(err) => {
                        catalog.warnings.push(format!(
                            "Selected capability root `{selected_root_id}` has invalid path `{path}`: {err}"
                        ));
                        continue;
                    }
                };
                let file_system = environment.get_filesystem();
                let outcome = filter_skill_load_outcome_for_product(
                    load_skills_from_roots(
                        [SkillRoot {
                            path: root_path.clone(),
                            scope: SkillScope::User,
                            file_system: Arc::clone(&file_system),
                            plugin_id: None,
                            plugin_namespace: None,
                            plugin_root: None,
                        }],
                        /*plugin_skill_snapshots*/ None,
                    )
                    .await,
                    self.restriction_product,
                );
                catalog.warnings.extend(outcome.errors.iter().map(|err| {
                    format!(
                        "Failed to load executor skill at {}: {}",
                        err.path.display(),
                        err.message
                    )
                }));
                for (skill, enabled) in outcome.skills_with_enabled() {
                    catalog.push_entry(catalog_entry_from_skill(
                        skill,
                        enabled,
                        authority.clone(),
                        &selected_root_id,
                        &environment_id,
                    ));
                }
            }

            Ok(catalog)
        })
    }

    fn read(&self, request: SkillReadRequest) -> SkillProviderFuture<'_, SkillReadResult> {
        Box::pin(async move {
            if request.authority.kind != SkillSourceKind::Executor {
                return Err(SkillProviderError::new(format!(
                    "executor skill provider cannot read {} resources",
                    request.authority.kind
                )));
            }
            if request.package.0 != request.resource.as_str() {
                return Err(SkillProviderError::new(
                    "executor skill resource does not match its package",
                ));
            }
            let Some((environment_id, resource_path)) = request.resource.environment_path() else {
                return Err(SkillProviderError::new(
                    "executor skill resource is not bound to an environment",
                ));
            };
            let Some(environment) = self.environment_manager.get_environment(environment_id) else {
                return Err(SkillProviderError::new(format!(
                    "executor skill resource references unavailable environment `{environment_id}`"
                )));
            };
            let resource_path = PathUri::from_abs_path(resource_path);
            let contents = environment
                .get_filesystem()
                .read_file_text(&resource_path, /*sandbox*/ None)
                .await
                .map_err(|err| {
                    SkillProviderError::new(format!(
                        "failed to read executor skill resource {}: {err}",
                        request.resource.as_str()
                    ))
                })?;

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

fn catalog_entry_from_skill(
    skill: &SkillMetadata,
    enabled: bool,
    authority: SkillAuthority,
    selected_root_id: &str,
    environment_id: &str,
) -> SkillCatalogEntry {
    let skill_path = skill.path_to_skills_md.to_string_lossy().into_owned();
    let normalized_path = skill_path.replace('\\', "/");
    let display_path = format!(
        "skill://{selected_root_id}/{}",
        normalized_path.trim_start_matches('/')
    );
    let mut entry = SkillCatalogEntry::new(
        SkillPackageId(display_path.clone()),
        authority,
        skill.name.clone(),
        skill.description.clone(),
        SkillResourceId::environment(
            display_path.clone(),
            environment_id,
            skill.path_to_skills_md.clone(),
        ),
    )
    .with_short_description(skill.short_description.clone())
    .with_display_path(display_path)
    .with_dependencies(skill.dependencies.clone());

    if !enabled {
        entry = entry.disabled();
    }
    if !skill.allows_implicit_invocation() {
        entry = entry.hidden_from_prompt();
    }

    entry
}

fn executor_absolute_path(path: &str) -> std::io::Result<AbsolutePathBuf> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "executor path must be absolute",
        ));
    }
    AbsolutePathBuf::from_absolute_path_checked(path)
}
