//! Tests cloud cache isolation, refresh retries, and late completion.

use super::*;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::provider::SkillProvider;
use crate::provider::SkillProviderFuture;
use crate::provider::SkillReadContext;
use crate::provider::SkillSearchRequest;
use codex_extension_api::CatalogRefreshInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_mcp::McpRuntime;
use codex_protocol::protocol::Event;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Default)]
struct Provider {
    reads: AtomicUsize,
    lists: AtomicUsize,
    fail_list: AtomicBool,
    partial_list: AtomicBool,
    next_catalog: Mutex<Option<SkillCatalog>>,
    pause_read: AtomicBool,
    pause_list: AtomicBool,
    entered: Notify,
    resume: Notify,
}

impl SkillProvider for Provider {
    fn list<'a>(&'a self, _query: SkillListQuery<'a>) -> SkillProviderFuture<'a, SkillCatalog> {
        Box::pin(async move {
            self.lists.fetch_add(1, Ordering::SeqCst);
            let fail = self.fail_list.swap(false, Ordering::SeqCst);
            let catalog = self
                .next_catalog
                .lock()
                .expect("test provider catalog mutex should not be poisoned")
                .take();
            if self.pause_list.swap(false, Ordering::SeqCst) {
                self.entered.notify_one();
                self.resume.notified().await;
            }
            if fail {
                return Err(SkillProviderError::new("discovery failed"));
            }
            if let Some(catalog) = catalog {
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
            catalog_deltas_enabled: false,
            omit_descriptions: false,
            max_context_tokens: None,
            bundled_skills_enabled: false,
            cloud_skill_enabled: true,
            shadow_selection_enabled: false,
        },
        /*cloud_skills_available*/ true,
        /*extension_metrics*/ None,
    )
}

fn client() -> Arc<McpResourceClient> {
    Arc::new(McpResourceClient::new(Arc::new(McpRuntime::empty(
        /*prefix_mcp_tool_names*/ false,
    ))))
}

fn query(client: Option<Arc<McpResourceClient>>) -> SkillListQuery<'static> {
    SkillListQuery {
        turn_id: Some("turn".into()),
        executor_roots: Vec::new(),
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
        .refresh_cloud_catalog_for_step(&providers, query(Some(client())))
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
            .refresh_cloud_catalog_for_step(&providers, query(mcp_resources))
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

#[tokio::test(start_paused = true)]
async fn step_cloud_refresh_reuses_accepted_results_and_retries_failures() {
    #[derive(Default)]
    struct WarningCounter(AtomicUsize);

    impl ExtensionEventSink for WarningCounter {
        fn emit(&self, _event: Event) {}

        fn emit_warning(&self, _warning: ExtensionWarning) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let cloud = Arc::new(Provider::default());
    let providers = SkillProviders::new().with_cloud_provider(cloud.clone());
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new("thread");
    thread_store.insert(state());
    let thread_state = thread_store
        .get::<SkillsThreadState>()
        .expect("skills thread state should be initialized");
    let turn_store = ExtensionData::new("turn");
    let config = thread_state.config();
    let warnings = Arc::new(WarningCounter::default());
    let mut builder = ExtensionRegistryBuilder::with_event_sink(warnings.clone());
    crate::install_with_providers(&mut builder, providers, move |_: &()| config.clone());
    let registry = builder.build();
    let contributor = registry
        .turn_lifecycle_contributors()
        .last()
        .expect("cloud skills contributor should be registered");
    let refresh_input = CatalogRefreshInput {
        turn_id: "turn",
        session_store: &session_store,
        thread_store: &thread_store,
        turn_store: &turn_store,
    };

    for (index, delay) in [5, 10, 20, 40, 80, 160, 300, 300].into_iter().enumerate() {
        cloud.fail_list.store(true, Ordering::SeqCst);
        assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
        assert_eq!(thread_state.available_cloud_catalog(), None);
        assert_eq!(cloud.lists.load(Ordering::SeqCst), index + 1);
        assert_eq!(warnings.0.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(delay - 1)).await;
        assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
        assert_eq!(cloud.lists.load(Ordering::SeqCst), index + 1);
        tokio::time::advance(Duration::from_secs(1)).await;
    }
    cloud.pause_list.store(true, Ordering::SeqCst);
    let mut cancelled = contributor.refresh_catalog_for_step(refresh_input);
    assert!(futures::poll!(&mut cancelled).is_pending());
    drop(cancelled);
    *cloud
        .next_catalog
        .lock()
        .expect("test provider catalog mutex should not be poisoned") =
        Some(SkillCatalog::default());
    assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
    assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
    assert_eq!(cloud.lists.load(Ordering::SeqCst), 10);

    // Later steps reuse the accepted result or failure cooldown; cancellation allows retry.
    for (fails, cancelled) in [(false, false), (true, false), (false, true)] {
        session_store.insert(SkillsSessionState {
            mcp_resources: Some(client()),
            extension_metrics: None,
        });
        let previous_calls = cloud.lists.load(Ordering::SeqCst);
        let first_catalog = SkillCatalog {
            warnings: vec!["catalog warning".into()],
            ..Default::default()
        };
        *cloud
            .next_catalog
            .lock()
            .expect("test provider catalog mutex should not be poisoned") =
            Some(first_catalog.clone());
        cloud.fail_list.store(fails, Ordering::SeqCst);
        cloud.pause_list.store(true, Ordering::SeqCst);
        let mut turn_start = contributor.refresh_catalog_for_step(refresh_input);
        assert!(futures::poll!(&mut turn_start).is_pending());
        assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 1);
        *cloud
            .next_catalog
            .lock()
            .expect("test provider catalog mutex should not be poisoned") =
            Some(SkillCatalog::default());
        if cancelled {
            drop(turn_start);
        } else {
            cloud.resume.notify_one();
            assert!(!turn_start.await);
        }
        assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
        if fails {
            assert_eq!(thread_state.available_cloud_catalog(), None);
            assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 1);
            tokio::time::advance(Duration::from_secs(5)).await;
            assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
        }
        assert_eq!(
            thread_state.available_cloud_catalog(),
            Some(if fails || cancelled {
                SkillCatalog::default()
            } else {
                first_catalog
            })
        );
        assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
        assert_eq!(
            cloud.lists.load(Ordering::SeqCst),
            previous_calls + 1 + usize::from(fails || cancelled)
        );
        assert_eq!(warnings.0.load(Ordering::SeqCst), 1);
    }

    // New turns share the thread's cooldown and failure progression.
    session_store.insert(SkillsSessionState {
        mcp_resources: Some(client()),
        extension_metrics: None,
    });
    let next_turn = ExtensionData::new("next turn");
    let previous_calls = cloud.lists.load(Ordering::SeqCst);
    for turn_store in [&turn_store, &next_turn] {
        cloud.fail_list.store(true, Ordering::SeqCst);
        assert!(
            !contributor
                .refresh_catalog_for_step(CatalogRefreshInput {
                    turn_store,
                    ..refresh_input
                })
                .await
        );
        assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 1);
    }
    assert_eq!(warnings.0.load(Ordering::SeqCst), 1);
    let next_input = CatalogRefreshInput {
        turn_id: "next turn",
        turn_store: &next_turn,
        ..refresh_input
    };
    tokio::time::advance(Duration::from_secs(5)).await;
    assert!(!contributor.refresh_catalog_for_step(next_input).await);
    assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 2);
    tokio::time::advance(Duration::from_secs(9)).await;
    assert!(!contributor.refresh_catalog_for_step(refresh_input).await);
    assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 2);
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(!contributor.refresh_catalog_for_step(next_input).await);
    assert_eq!(cloud.lists.load(Ordering::SeqCst), previous_calls + 3);
    assert_eq!(warnings.0.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn late_read_cannot_return_or_populate_a_replaced_cache() {
    for replacement in [Some(client()), None] {
        let state = state();
        let provider = Arc::new(Provider::default());
        let providers = SkillProviders::new().with_cloud_provider(provider.clone());
        let original = Some(client());
        // None here exercises a refresh within the same auth generation.
        let replacement = replacement.or_else(|| original.clone());
        provider.pause_read.store(true, Ordering::SeqCst);
        let (late, ()) = tokio::join!(state.read_skill(&providers, request(original)), async {
            provider.entered.notified().await;
            state
                .refresh_cloud_catalog_for_step(&providers, query(replacement.clone()))
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
        state.refresh_cloud_catalog_for_step(&providers, query(Some(client()))),
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
    late.expect("superseded discovery should be discarded without an error");
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

#[tokio::test(start_paused = true)]
async fn cloud_catalogs_are_reused_while_fresh_or_backing_off_until_auth_changes() {
    for is_partial in [false, true] {
        for warnings in [Vec::new(), vec!["catalog diagnostic".to_string()]] {
            let state = state();
            let provider = Arc::new(Provider::default());
            let accepted = SkillCatalog {
                is_partial,
                warnings,
                ..Default::default()
            };
            *provider.next_catalog.lock().unwrap() = Some(accepted.clone());
            let providers = SkillProviders::new().with_cloud_provider(provider.clone());
            let original_client = client();
            state
                .refresh_cloud_catalog(&providers, query(Some(original_client.clone())))
                .await
                .expect("every successful provider response should be accepted");
            for turn_id in ["turn", "next turn"] {
                let mut request = query(Some(original_client.clone()));
                request.turn_id = Some(turn_id.to_string());
                state
                    .refresh_cloud_catalog_for_step(&providers, request)
                    .await
                    .expect("accepted cloud catalog should be reused");
                assert_eq!(state.available_cloud_catalog(), Some(accepted.clone()));
                assert_eq!(provider.lists.load(Ordering::SeqCst), 1);
            }
            *provider.next_catalog.lock().unwrap() = Some(SkillCatalog::default());
            state
                .refresh_cloud_catalog_for_step(&providers, query(Some(client())))
                .await
                .expect("auth changes should refresh the accepted catalog");
            assert_eq!(
                state.available_cloud_catalog(),
                Some(SkillCatalog::default())
            );
            assert_eq!(provider.lists.load(Ordering::SeqCst), 2);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn partial_or_warning_cloud_catalogs_retry_with_backoff_and_recover() {
    for (is_partial, warnings) in [
        (true, Vec::new()),
        (false, vec!["catalog diagnostic".to_string()]),
        (true, vec!["catalog diagnostic".to_string()]),
    ] {
        for original_client in [None, Some(client())] {
            let state = state();
            let provider = Arc::new(Provider::default());
            let providers = SkillProviders::new().with_cloud_provider(provider.clone());
            let retryable = SkillCatalog {
                entries: vec![SkillCatalogEntry::new(
                    SkillPackageId("skill://demo".into()),
                    SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
                    "demo",
                    "retryable catalog",
                    SkillResourceId::new("skill://demo/SKILL.md"),
                )],
                is_partial,
                warnings: warnings.clone(),
            };
            for (index, delay) in [5, 10, 20, 40, 80, 160, 300, 300].into_iter().enumerate() {
                *provider.next_catalog.lock().unwrap() = Some(retryable.clone());
                state
                    .refresh_cloud_catalog_for_step(&providers, query(original_client.clone()))
                    .await
                    .expect("partial or warning catalogs should remain usable");
                assert_eq!(state.available_cloud_catalog(), Some(retryable.clone()));
                assert_eq!(provider.lists.load(Ordering::SeqCst), index + 1);
                tokio::time::advance(Duration::from_secs(delay - 1)).await;
                state
                    .refresh_cloud_catalog_for_step(&providers, query(original_client.clone()))
                    .await
                    .expect("retry cooldown should suppress discovery");
                assert_eq!(provider.lists.load(Ordering::SeqCst), index + 1);
                tokio::time::advance(Duration::from_secs(1)).await;
            }
            let complete = SkillCatalog {
                is_partial: false,
                warnings: Vec::new(),
                ..retryable
            };
            *provider.next_catalog.lock().unwrap() = Some(complete.clone());
            state
                .refresh_cloud_catalog_for_step(&providers, query(original_client.clone()))
                .await
                .expect("retry should recover a complete catalog");
            assert_eq!(state.available_cloud_catalog(), Some(complete.clone()));
            assert!(state.catalog_state.lock().unwrap().cloud_retry.is_none());
            tokio::time::advance(Duration::from_secs(600)).await;
            state
                .refresh_cloud_catalog_for_step(&providers, query(original_client))
                .await
                .expect("clean catalogs should remain cached");
            assert_eq!(state.available_cloud_catalog(), Some(complete));
            assert_eq!(provider.lists.load(Ordering::SeqCst), 9);
        }
    }
}

#[tokio::test]
async fn cloud_refresh_replaces_catalogs_and_read_cache_for_every_accepted_result() {
    let catalog = |name: &str, is_partial| SkillCatalog {
        entries: vec![SkillCatalogEntry::new(
            SkillPackageId("skill://demo".into()),
            SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
            name,
            "test catalog",
            SkillResourceId::new("skill://demo/SKILL.md"),
        )],
        warnings: vec![format!("{name} catalog diagnostic")],
        is_partial,
    };
    for previous_partial in [false, true] {
        for incoming_partial in [false, true] {
            let state = state();
            let provider = Arc::new(Provider::default());
            let providers = SkillProviders::new().with_cloud_provider(provider.clone());
            let original_client = client();
            let previous = catalog("previous", previous_partial);
            *provider
                .next_catalog
                .lock()
                .expect("test provider catalog mutex should not be poisoned") =
                Some(previous.clone());
            state
                .refresh_cloud_catalog_for_step(&providers, query(Some(original_client.clone())))
                .await
                .expect("cloud catalog refresh should succeed");
            let generation = state
                .catalog_state
                .lock()
                .expect("catalog state mutex should not be poisoned")
                .cloud_generation
                .clone()
                .expect("initial refresh should publish a cloud generation");
            assert_eq!(
                state
                    .read_skill(&providers, request(Some(original_client.clone())))
                    .await
                    .expect("cloud skill read should succeed")
                    .contents,
                "contents 0"
            );

            let incoming = catalog("incoming", incoming_partial);
            *provider
                .next_catalog
                .lock()
                .expect("test provider catalog mutex should not be poisoned") =
                Some(incoming.clone());
            let current_client = original_client;
            state
                .refresh_cloud_catalog(&providers, query(Some(current_client.clone())))
                .await
                .expect("cloud catalog refresh should succeed");
            assert_eq!(generation.catalog, Some(previous));
            assert_eq!(state.available_cloud_catalog(), Some(incoming));
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
                "contents 1",
            );
        }
    }
}
