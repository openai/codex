//! Regression tests for cloud cache generation isolation and late completion.

use super::*;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSearchResult;
use crate::provider::SkillProvider;
use crate::provider::SkillProviderFuture;
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
            Ok(SkillCatalog {
                warnings: vec!["account catalog".into()],
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
    }
}

fn request(client: Option<Arc<McpResourceClient>>) -> SkillReadRequest<'static> {
    SkillReadRequest {
        _lifetime: Default::default(),
        authority: SkillAuthority::new(SkillSourceKind::Cloud, "codex_apps"),
        package: SkillPackageId("skill://demo".into()),
        resource: SkillResourceId::new("skill://demo/SKILL.md"),
        resolved_executor_roots: Vec::new(),
        sandbox: None,
        host_snapshot: None,
        mcp_resources: client,
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
                .unwrap();
            provider.resume.notify_one();
        });
        assert!(late.is_err());
        assert_eq!(
            state
                .read_skill(&providers, request(replacement))
                .await
                .unwrap()
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
            state.cloud_cache(Some(&client()));
            provider.resume.notify_one();
        }
    );
    assert!(late.is_err());
    assert!(
        state
            .skills_extension_state
            .lock()
            .unwrap()
            .cloud_cache
            .as_ref()
            .unwrap()
            .catalog
            .is_none()
    );
}
