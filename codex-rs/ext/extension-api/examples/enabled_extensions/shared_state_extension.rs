use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;

/// Installs the tutorial contributors used by the example host.
pub fn install(registry: &mut ExtensionRegistryBuilder<()>) {
    registry.prompt_contributor(Arc::new(StyleContributor));
    registry.prompt_contributor(Arc::new(UsageContributor));
}

#[derive(Debug)]
struct StyleContributor;

impl ContextContributor for StyleContributor {
    fn contribute<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            contribution_counts(session_store).record_style();
            contribution_counts(thread_store).record_style();

            vec![PromptFragment::developer_policy(
                "Prefer short answers unless the user asks for detail.",
            )]
        })
    }
}

#[derive(Debug)]
struct UsageContributor;

impl ContextContributor for UsageContributor {
    fn contribute<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            contribution_counts(session_store).record_usage();
            contribution_counts(thread_store).record_usage();

            vec![PromptFragment::developer_capability(
                "This extension can contribute more than one prompt fragment.",
            )]
        })
    }
}

/// Returns how many style contributions were recorded in `store`.
pub fn recorded_style_contributions(store: &ExtensionData) -> u64 {
    store
        .get::<ContributionCounts>()
        .map(|counts| counts.style())
        .unwrap_or_default()
}

/// Returns how many usage contributions were recorded in `store`.
pub fn recorded_usage_contributions(store: &ExtensionData) -> u64 {
    store
        .get::<ContributionCounts>()
        .map(|counts| counts.usage())
        .unwrap_or_default()
}

#[derive(Debug, Default)]
struct ContributionCounts {
    style: AtomicU64,
    usage: AtomicU64,
}

impl ContributionCounts {
    fn record_style(&self) {
        self.style.fetch_add(1, Ordering::Relaxed);
    }

    fn record_usage(&self) {
        self.usage.fetch_add(1, Ordering::Relaxed);
    }

    fn style(&self) -> u64 {
        self.style.load(Ordering::Relaxed)
    }

    fn usage(&self) -> u64 {
        self.usage.load(Ordering::Relaxed)
    }
}

fn contribution_counts(store: &ExtensionData) -> Arc<ContributionCounts> {
    store.get_or_init::<ContributionCounts>(Default::default)
}
