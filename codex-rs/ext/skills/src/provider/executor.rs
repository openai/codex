use std::sync::Arc;

use codex_core_skills::loader::load_environment_skills_from_discovery;
use codex_core_skills::loader::load_environment_skills_from_root;
use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::protocol::Product;
use codex_skills::EnvironmentSkillMetadata;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use futures::StreamExt;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillPackageId;
use crate::catalog::SkillProviderError;
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::catalog::SkillSourceKind;
use crate::provider::MAX_SKILL_RESOURCE_CONTENT_BYTES;
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
            if let Some(discovery) = query.executor_capability_discovery {
                return Ok(self.list_from_discovery(&discovery));
            }
            let mut catalog = SkillCatalog::default();
            for selected_root in query.executor_roots {
                let selected_root_id = &selected_root.id;
                let CapabilityRootLocation::Environment {
                    environment_id,
                    path,
                } = &selected_root.location;
                let authority =
                    SkillAuthority::new(SkillSourceKind::Executor, selected_root_id.clone());
                let file_system = query
                    .resolved_executor_roots
                    .iter()
                    .find(|root| root.selected_root() == &selected_root)
                    .map(|root| root.environment().get_filesystem())
                    .or_else(|| {
                        self.environment_manager
                            .get_environment(environment_id)
                            .map(|environment| environment.get_filesystem())
                    });
                let Some(file_system) = file_system else {
                    catalog.warnings.push(format!(
                        "Selected capability root `{selected_root_id}` references unavailable environment `{environment_id}`."
                    ));
                    continue;
                };
                let outcome = load_environment_skills_from_root(
                    file_system.as_ref(),
                    path,
                    self.restriction_product,
                )
                .await;
                catalog.warnings.extend(outcome.warnings);
                for skill in outcome.skills {
                    catalog.push_entry(catalog_entry_from_skill(
                        &skill,
                        authority.clone(),
                        selected_root_id,
                        environment_id,
                        /*instructions*/ None,
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
            let expected_package_prefix = format!("skill://{}/", request.authority.id);
            if !request.package.0.starts_with(&expected_package_prefix)
                || request
                    .package
                    .relative_resource_path(request.resource.as_str())
                    .is_none()
            {
                return Err(SkillProviderError::new(
                    "executor skill resource does not match its package",
                ));
            }
            if let Some(contents) = request.resource.environment_contents() {
                return Ok(SkillReadResult {
                    resource: request.resource.clone(),
                    contents: contents.to_string(),
                });
            }
            let Some((environment_id, resource_path)) = request.resource.environment_path() else {
                return Err(SkillProviderError::new(
                    "executor skill resource is not bound to an environment",
                ));
            };
            let file_system = request
                .resolved_executor_roots
                .iter()
                .find(|root| root.selected_root().id == request.authority.id)
                .map(|root| root.environment().get_filesystem())
                .or_else(|| {
                    self.environment_manager
                        .get_environment(environment_id)
                        .map(|environment| environment.get_filesystem())
                });
            let Some(file_system) = file_system else {
                return Err(SkillProviderError::new(format!(
                    "executor skill resource references unavailable environment `{environment_id}`"
                )));
            };
            let contents = read_bounded_text(
                file_system.as_ref(),
                resource_path,
                request.resource.as_str(),
            )
            .await?;

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

impl ExecutorSkillProvider {
    fn list_from_discovery(
        &self,
        snapshot: &codex_exec_server::ExecutorCapabilityDiscoverySnapshot,
    ) -> SkillCatalog {
        let mut catalog = SkillCatalog::default();
        for root in snapshot.roots() {
            let selected_root_id = &root.selected_root.id;
            let CapabilityRootLocation::Environment { environment_id, .. } =
                &root.selected_root.location;
            let discovery = match &root.result {
                Ok(discovery) => discovery.as_ref(),
                Err(error) => {
                    catalog.warnings.push(format!(
                        "Selected capability root `{selected_root_id}` discovery failed: {error}"
                    ));
                    continue;
                }
            };
            let outcome =
                load_environment_skills_from_discovery(discovery, self.restriction_product);
            catalog.warnings.extend(outcome.warnings);
            let authority =
                SkillAuthority::new(SkillSourceKind::Executor, selected_root_id.clone());
            for skill in outcome.skills {
                catalog.push_entry(catalog_entry_from_skill(
                    &skill.metadata,
                    authority.clone(),
                    selected_root_id,
                    environment_id,
                    Some(skill.instructions),
                ));
            }
        }
        catalog
    }
}

fn catalog_entry_from_skill(
    skill: &EnvironmentSkillMetadata,
    authority: SkillAuthority,
    selected_root_id: &str,
    environment_id: &str,
    instructions: Option<String>,
) -> SkillCatalogEntry {
    let handle_prefix = format!("skill://{selected_root_id}/");
    let normalized_main_path = normalized_environment_path(&skill.path_to_skills_md);
    let normalized_package_path = skill.path_to_skills_md.parent().map_or_else(
        || normalized_main_path.clone(),
        |path| normalized_environment_path(&path),
    );
    let package = format!(
        "{handle_prefix}{}",
        normalized_package_path.trim_start_matches('/')
    );
    let main_resource = format!(
        "{handle_prefix}{}",
        normalized_main_path.trim_start_matches('/')
    );
    let main_prompt = match instructions {
        Some(contents) => SkillResourceId::environment_with_contents(
            main_resource.clone(),
            environment_id,
            skill.path_to_skills_md.clone(),
            contents,
        ),
        None => SkillResourceId::environment(
            main_resource.clone(),
            environment_id,
            skill.path_to_skills_md.clone(),
        ),
    };
    let entry = SkillCatalogEntry::new(
        SkillPackageId(package),
        authority,
        skill.name.clone(),
        skill.description.clone(),
        main_prompt,
    )
    .with_short_description(skill.short_description.clone())
    .with_display_path(main_resource)
    .with_dependencies(skill.dependencies.clone());

    if skill.allows_implicit_invocation() {
        entry
    } else {
        entry.hidden_from_prompt()
    }
}

fn normalized_environment_path(path: &PathUri) -> String {
    let convention = path.infer_path_convention();
    let path = path.inferred_native_path_string();
    match convention {
        Some(PathConvention::Windows) => path.replace('\\', "/"),
        Some(PathConvention::Posix) | None => path,
    }
}

async fn read_bounded_text(
    file_system: &dyn codex_exec_server::ExecutorFileSystem,
    path: &PathUri,
    resource: &str,
) -> Result<String, SkillProviderError> {
    let read_error = |err| {
        SkillProviderError::new(format!(
            "failed to read executor skill resource {resource}: {err}"
        ))
    };
    let mut stream = file_system
        .read_file_stream(path, /*sandbox*/ None)
        .await
        .map_err(&read_error)?;
    let mut contents = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(&read_error)?;
        if contents.len().saturating_add(chunk.len()) > MAX_SKILL_RESOURCE_CONTENT_BYTES {
            return Err(SkillProviderError::new(format!(
                "executor skill resource {resource} exceeds {MAX_SKILL_RESOURCE_CONTENT_BYTES} bytes"
            )));
        }
        contents.extend_from_slice(&chunk);
    }
    String::from_utf8(contents).map_err(|_| {
        SkillProviderError::new(format!(
            "executor skill resource {resource} is not valid UTF-8"
        ))
    })
}
