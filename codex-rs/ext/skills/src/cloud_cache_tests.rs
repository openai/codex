//! Regression tests for cloud cache generation isolation and late completion.

use super::*;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::provider::SkillProvider;
use crate::provider::SkillProviderFuture;
use crate::provider::SkillReadContext;
use crate::provider::SkillSearchRequest;
use codex_mcp::McpRuntime;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;

#[derive(Default)]
struct Provider {
    reads: AtomicUsize,
    partial_list: AtomicBool,
    next_catalog: Mutex<Option<SkillCatalog>>,
    pause_read: AtomicBool,
    pause_list: AtomicBool,
    entered: Notify,
    resume: Notify,
}

impl SkillProvider for Provider {
    fn list(&self, _query: SkillListQuery) -> SkillProviderFuture<'_, SkillCatalog> {
        Box::pin(async move {
            if self.pause_list.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.resume.notified().await;
            }
            if let Some(catalog) = self
                .next_catalog
                .lock()
                .expect("test provider catalog mutex should not be poisoned")
                .take()
            {
                return Ok(catalog);
            }
            let is_partial = self.partial_list.swap(false, Ordering::SeqCst);
            Ok(SkillCatalog {
                is_partial,
                warnings: if is_partial {
                    Vec::new()
                } else {
                    vec!["account catalog".into()]
                },
                ..Default::default()
            })
        })
    }

    fn read<'a>(
        &'a self,
        request: SkillReadRequest<'a>,
    ) -> SkillProviderFuture<'a, SkillReadResult> {
        let revision = self.reads.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if self.pause_read.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.resume.notified().await;
            }
            Ok(SkillReadResult {
                resource: request.resource,
                contents: format!("contents {revision}"),
            })
        })
    }

    fn search(&self, _request: SkillSearchRequest) -> SkillProviderFuture<'_, SkillSearchResult> {
        Box::pin(async { Ok(SkillSearchResult::default()) })
    }
}

fn state() -> SkillsThreadState {
    SkillsThreadState::new(
        SkillsExtensionConfig {
            include_instructions: true,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
        /*cloud_skills_available*/ true,
    )
}

fn client() -> Arc<McpResourceClient> {
    Arc::new(McpResourceClient::new(Arc::new(McpRuntime::empty(
        /*prefix_mcp_tool_names*/ false,
    ))))
}

fn query(client: Option<Arc<McpResourceClient>>) -> SkillListQuery {
    SkillListQuery {
        turn_id: "turn".into(),
        executor_roots: Vec::new(),
        resolved_executor_roots: Vec::new(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: false,
        include_cloud_skills: true,
        mcp_resources: client,
        executor_capability_discovery: None,
        executor_capability_discovery_v2: None,
    }
}

fn request(client: Option<Arc<McpResourceClient>>) -> SkillReadRequest<'static> {
    SkillReadRequest {
        authority: SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
        package: SkillPackageId("skill://demo".into()),
        resource: SkillResourceId::new("skill://demo/SKILL.md"),
        context: SkillReadContext::Cloud {
            mcp_resources: client,
        },
    }
}

#[tokio::test]
async fn frozen_cloud_catalog_and_reads_ignore_live_mcp_changes() {
    let state = state();
    let provider = crate::FrozenCloudSkillProvider::from_json(
        serde_json::json!({"skills": [{
            "package": "skill://demo/work", "name": "demo", "description": "Frozen skill",
            "alias_root": "skill://demo", "main_resource": "skill://demo/work/SKILL.md",
            "resources": {"skill://demo/work/SKILL.md": "Frozen contents"}
        }]})
        .to_string()
        .as_bytes(),
    )
    .expect("frozen cloud skill fixture should be valid");
    let providers = SkillProviders::new().with_cloud_provider(Arc::new(provider));
    state
        .refresh_cloud_catalog(&providers, query(Some(client())))
        .await
        .expect("cloud catalog refresh should succeed");
    let catalog = state
        .available_cloud_catalog()
        .expect("cloud catalog should be available after refresh");
    let generation = state
        .catalog_state
        .lock()
        .expect("catalog state mutex should not be poisoned")
        .cloud_generation
        .clone()
        .expect("initial refresh should publish a cloud generation");
    for mcp_resources in [Some(client()), None, Some(client())] {
        let request = SkillReadRequest {
            package: SkillPackageId("skill://demo/work".into()),
            resource: SkillResourceId::new("skill://demo/work/SKILL.md"),
            ..request(mcp_resources.clone())
        };
        assert_eq!(
            state
                .read_skill(&providers, request)
                .await
                .expect("cloud skill read should succeed"),
            SkillReadResult {
                resource: SkillResourceId::new("skill://demo/work/SKILL.md"),
                contents: "Frozen contents".into(),
            }
        );
        state
            .refresh_cloud_catalog(&providers, query(mcp_resources))
            .await
            .expect("cloud catalog refresh should succeed");
        assert_eq!(state.available_cloud_catalog(), Some(catalog.clone()));
        assert!(Arc::ptr_eq(
            &generation,
            state
                .catalog_state
                .lock()
                .expect("catalog state mutex should not be poisoned")
                .cloud_generation
                .as_ref()
                .expect("frozen cloud generation should remain available")
        ));
    }
}

#[tokio::test]
async fn late_read_cannot_return_or_populate_a_replaced_cache() {
    for replacement in [Some(client()), None] {
        let state = state();
        let provider = Arc::new(Provider::default());
        let providers = SkillProviders::new().with_cloud_provider(provider.clone());
        let original = Some(client());
        // None here exercises a successful per-turn refresh on the SAME generation.
        let replacement = replacement.or_else(|| original.clone());
        provider.pause_read.store(true, Ordering::SeqCst);
        let (late, ()) = tokio::join!(state.read_skill(&providers, request(original)), async {
            provider.entered.notified().await;
            state
                .refresh_cloud_catalog(&providers, query(replacement.clone()))
                .await
                .expect("cloud catalog refresh should succeed");
            provider.resume.notify_one();
        });
        assert!(late.is_err());
        assert_eq!(
            state
                .read_skill(&providers, request(replacement))
                .await
                .expect("cloud skill read should succeed")
                .contents,
            "contents 1"
        );
    }
}

#[tokio::test]
async fn late_discovery_cannot_publish_into_a_new_generation() {
    let state = state();
    let provider = Arc::new(Provider::default());
    let providers = SkillProviders::new().with_cloud_provider(provider.clone());
    provider.pause_list.store(true, Ordering::SeqCst);
    let (late, ()) = tokio::join!(
        state.refresh_cloud_catalog(&providers, query(Some(client()))),
        async {
            provider.entered.notified().await;
            state
                .catalog_state
                .lock()
                .expect("catalog state mutex should not be poisoned")
                .prepare_cloud_generation(Some(&client()));
            provider.resume.notify_one();
        }
    );
    assert!(late.is_err());
    assert!(
        state
            .catalog_state
            .lock()
            .expect("catalog state mutex should not be poisoned")
            .cloud_generation
            .as_ref()
            .expect("replacement cloud generation should exist")
            .catalog
            .is_none()
    );
}

#[tokio::test]
async fn partial_cloud_catalog_without_warnings_is_refreshed_next_turn() {
    let state = state();
    let provider = Arc::new(Provider::default());
    provider.partial_list.store(true, Ordering::SeqCst);
    let providers = SkillProviders::new().with_cloud_provider(provider);
    let client = Some(client());
    state
        .refresh_cloud_catalog(&providers, query(client.clone()))
        .await
        .expect("cloud catalog refresh should succeed");
    assert_eq!(
        state
            .available_cloud_catalog()
            .expect("cloud catalog should be available after refresh"),
        SkillCatalog {
            is_partial: true,
            ..Default::default()
        }
    );
    state
        .refresh_cloud_catalog(&providers, query(client))
        .await
        .expect("cloud catalog refresh should succeed");
    assert_eq!(
        state
            .available_cloud_catalog()
            .expect("cloud catalog should be available after refresh"),
        SkillCatalog {
            is_partial: false,
            warnings: vec!["account catalog".into()],
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn cloud_refresh_preserves_complete_catalogs_only_within_the_same_auth_scope() {
    let catalog = |name: &str, is_partial| SkillCatalog {
        entries: vec![SkillCatalogEntry::new(
            SkillPackageId("skill://demo".into()),
            SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
            name,
            "test catalog",
            SkillResourceId::new("skill://demo/SKILL.md"),
        )],
        // Complete listings can have diagnostics; these also keep discovery eligible for refresh.
        warnings: vec![format!("{name} catalog diagnostic")],
        is_partial,
    };
    for previous_partial in [false, true] {
        for incoming_partial in [false, true] {
            for changed_auth in [false, true] {
                let state = state();
                let provider = Arc::new(Provider::default());
                let providers = SkillProviders::new().with_cloud_provider(provider.clone());
                let original_client = client();
                let mut previous = catalog("previous", previous_partial);
                *provider
                    .next_catalog
                    .lock()
                    .expect("test provider catalog mutex should not be poisoned") =
                    Some(previous.clone());
                state
                    .refresh_cloud_catalog(&providers, query(Some(original_client.clone())))
                    .await
                    .expect("cloud catalog refresh should succeed");
                let generation = state
                    .catalog_state
                    .lock()
                    .expect("catalog state mutex should not be poisoned")
                    .cloud_generation
                    .clone()
                    .expect("initial refresh should publish a cloud generation");
                let contents = state
                    .read_skill(&providers, request(Some(original_client.clone())))
                    .await
                    .expect("cloud skill read should succeed");

                let incoming = catalog("incoming", incoming_partial);
                *provider
                    .next_catalog
                    .lock()
                    .expect("test provider catalog mutex should not be poisoned") =
                    Some(incoming.clone());
                let current_client = if changed_auth {
                    client()
                } else {
                    original_client
                };
                state
                    .refresh_cloud_catalog(&providers, query(Some(current_client.clone())))
                    .await
                    .expect("cloud catalog refresh should succeed");
                let retained = !changed_auth && !previous_partial && incoming_partial;
                assert_eq!(
                    generation
                        .catalog
                        .as_ref()
                        .expect("published generation should contain a catalog")
                        .is_partial,
                    previous_partial
                );
                if retained {
                    previous.is_partial = true;
                }
                assert_eq!(
                    state.available_cloud_catalog(),
                    Some(if retained { previous } else { incoming })
                );
                assert!(!Arc::ptr_eq(
                    &generation,
                    state
                        .catalog_state
                        .lock()
                        .expect("catalog state mutex should not be poisoned")
                        .cloud_generation
                        .as_ref()
                        .expect("refresh should publish a cloud generation")
                ));
                assert_eq!(
                    state
                        .read_skill(&providers, request(Some(current_client)))
                        .await
                        .expect("cloud skill read should succeed")
                        .contents,
                    if retained {
                        contents.contents
                    } else {
                        "contents 1".to_string()
                    },
                );
            }
        }
    }
}
