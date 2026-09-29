//! Keep per-thread product attribution separate from the shared analytics identity.

use crate::events::TrackEventRequest;
use std::collections::HashMap;
use std::collections::VecDeque;

pub(crate) const MAX_THREAD_PRODUCTS: usize = 4096;
const MAX_PRODUCT_BATCHES: usize = 16;

/// An explicit change to a thread's product attribution, independent of authentication.
#[derive(Clone)]
pub enum ThreadProductUpdate {
    Set(String),
    Clear,
}

#[derive(Default)]
pub(crate) struct ThreadProducts {
    pub(crate) products: HashMap<String, String>,
    pub(crate) registered_threads: VecDeque<String>,
}

impl ThreadProducts {
    pub(crate) fn get(&self, thread_id: &str) -> Option<&str> {
        self.products.get(thread_id).map(String::as_str)
    }

    pub(crate) fn register(&mut self, thread_id: String, product: Option<String>) {
        self.registered_threads.retain(|id| id != &thread_id);
        self.products.remove(&thread_id);
        let Some(product) = product else {
            return;
        };
        // Attribution is best-effort. Evicted threads have no product header.
        if self.products.len() >= MAX_THREAD_PRODUCTS
            && let Some(oldest) = self.registered_threads.pop_front()
        {
            self.products.remove(&oldest);
        }
        self.registered_threads.push_back(thread_id.clone());
        self.products.insert(thread_id, product);
    }
}

/// Preserve order within each product, with a bounded number of concurrent streams.
/// Too many products drops only attribution, preserving the original event order.
pub(crate) fn product_event_batches(
    events: Vec<TrackEventRequest>,
    thread_products: &ThreadProducts,
) -> Vec<(Option<&str>, Vec<TrackEventRequest>)> {
    let products: std::collections::HashSet<_> = events
        .iter()
        .map(|event| event_thread_id(event).and_then(|id| thread_products.get(id)))
        .collect();
    if products.len() > MAX_PRODUCT_BATCHES {
        return vec![(None, events)];
    }
    let mut batches: Vec<(Option<&str>, Vec<TrackEventRequest>)> = Vec::new();
    for event in events {
        let product = event_thread_id(&event).and_then(|thread_id| thread_products.get(thread_id));
        if let Some((_, batch)) = batches.iter_mut().find(|(sku, _)| *sku == product) {
            batch.push(event);
        } else {
            batches.push((product, vec![event]));
        }
    }
    batches
}

fn event_thread_id(event: &TrackEventRequest) -> Option<&str> {
    match event {
        TrackEventRequest::SkillInvocation(event) => event.event_params.thread_id.as_deref(),
        TrackEventRequest::ThreadInitialized(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::ThreadArchive(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::GuardianReview(event) => {
            Some(&event.event_params.guardian_review.thread_id)
        }
        TrackEventRequest::GuardianV2(event) => Some(&event.event_params.guardian_v2.thread_id),
        TrackEventRequest::AppMentioned(event) => event.event_params.thread_id.as_deref(),
        TrackEventRequest::AppUsed(event) => event.event_params.app.thread_id.as_deref(),
        TrackEventRequest::HookRun(event) => event.event_params.thread_id.as_deref(),
        TrackEventRequest::Compaction(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::Goal(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::ThreadHintStatus(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::TurnEvent(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::TurnSteer(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::ArtifactOperation(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::CommandExecution(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::PluginMeasurement(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::FileChange(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::McpToolCall(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::DynamicToolCall(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::ControlToolCall(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::CollabAgentToolCall(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::WebSearch(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::ImageGeneration(event) => Some(&event.event_params.base.thread_id),
        TrackEventRequest::AcceptedLineFingerprints(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::ReviewEvent(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::PluginUsed(event) => event.event_params.thread_id.as_deref(),
        TrackEventRequest::PluginInstallRequested(event) => Some(&event.event_params.thread_id),
        TrackEventRequest::PluginInstalled(_)
        | TrackEventRequest::PluginUninstalled(_)
        | TrackEventRequest::PluginEnabled(_)
        | TrackEventRequest::PluginDisabled(_)
        | TrackEventRequest::PluginInstallFailed(_)
        | TrackEventRequest::ExternalAgentConfigImportCompleted(_)
        | TrackEventRequest::ExternalAgentConfigImportFailure(_) => None,
    }
}
