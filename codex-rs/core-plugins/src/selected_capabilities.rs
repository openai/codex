use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_exec_server::EnvironmentManager;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use tokio::sync::Notify;

use crate::ExecutorPluginProvider;
use crate::ResolvedSelectedCapabilityRoot;

type ResolutionFuture = Pin<
    Box<
        dyn Future<Output = Result<ResolvedSelectedCapabilityRoot, SelectedCapabilityFailure>>
            + Send
            + 'static,
    >,
>;
type IndexedResolutionFuture = Pin<
    Box<
        dyn Future<
                Output = (
                    usize,
                    Result<ResolvedSelectedCapabilityRoot, SelectedCapabilityFailure>,
                ),
            > + Send
            + 'static,
    >,
>;

/// Terminal failure while binding one selected root to its executor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedCapabilityFailure {
    message: String,
}

impl SelectedCapabilityFailure {
    /// Returns the stable diagnostic captured for this failed binding.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Resolution state for one environment-qualified selected capability root.
#[derive(Clone)]
pub enum SelectedCapabilityBindingStatus {
    /// The owning executor has not completed capability discovery yet.
    Pending,
    /// The root is bound to its exact executor and optional plugin descriptor.
    Ready(Arc<ResolvedSelectedCapabilityRoot>),
    /// Discovery failed permanently for this thread.
    Failed(Arc<SelectedCapabilityFailure>),
}

/// One selected root and its state in an immutable binding snapshot.
#[derive(Clone)]
pub struct SelectedCapabilityBindingSnapshot {
    selected_root: SelectedCapabilityRoot,
    status: SelectedCapabilityBindingStatus,
}

impl SelectedCapabilityBindingSnapshot {
    /// Returns the original root in caller-provided selection order.
    pub fn selected_root(&self) -> &SelectedCapabilityRoot {
        &self.selected_root
    }

    /// Returns this root's state at the captured generation.
    pub fn status(&self) -> &SelectedCapabilityBindingStatus {
        &self.status
    }
}

/// Immutable selected-capability view captured at one generation.
#[derive(Clone)]
pub struct SelectedCapabilitySnapshot {
    generation: u64,
    entries: Vec<SelectedCapabilityBindingSnapshot>,
}

impl SelectedCapabilitySnapshot {
    /// Returns the monotonically increasing binding generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns entries in the original selected-root order.
    pub fn entries(&self) -> &[SelectedCapabilityBindingSnapshot] {
        &self.entries
    }

    /// Returns whether every selected root is ready or permanently failed.
    pub fn is_terminal(&self) -> bool {
        self.entries
            .iter()
            .all(|entry| !matches!(entry.status(), SelectedCapabilityBindingStatus::Pending))
    }

    /// Iterates roots that are ready in caller-provided selection order.
    pub fn ready(&self) -> impl Iterator<Item = &ResolvedSelectedCapabilityRoot> {
        self.entries.iter().filter_map(|entry| match &entry.status {
            SelectedCapabilityBindingStatus::Ready(resolved) => Some(resolved.as_ref()),
            SelectedCapabilityBindingStatus::Pending
            | SelectedCapabilityBindingStatus::Failed(_) => None,
        })
    }
}

/// Thread-owned, nonblocking selected-capability resolution state.
///
/// Each selected root resolves once against its original environment. Callers
/// may capture an immutable snapshot without waiting, or await the terminal
/// snapshot to preserve legacy startup behavior.
#[derive(Clone)]
pub struct SelectedCapabilityBindings {
    inner: Arc<SelectedCapabilityBindingsInner>,
}

struct SelectedCapabilityBindingsInner {
    roots: Vec<SelectedCapabilityRoot>,
    state: Mutex<SelectedCapabilityBindingsState>,
    changed: Notify,
    resolution_task: Mutex<Option<tokio::task::AbortHandle>>,
}

struct SelectedCapabilityBindingsState {
    generation: u64,
    statuses: Vec<SelectedCapabilityBindingStatus>,
}

impl SelectedCapabilityBindings {
    /// Starts background binding for the supplied environment-qualified roots.
    ///
    /// # Panics
    ///
    /// Panics when called outside a Tokio runtime if `selected_roots` is not
    /// empty.
    pub fn new(
        selected_roots: Vec<SelectedCapabilityRoot>,
        environment_manager: Arc<EnvironmentManager>,
    ) -> Self {
        let provider = ExecutorPluginProvider::new(Arc::clone(&environment_manager));
        let mut environments = HashMap::new();
        let resolutions = selected_roots
            .iter()
            .cloned()
            .enumerate()
            .map(|(selection_order, selected_root)| {
                let provider = provider.clone();
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &selected_root.location;
                let environment = environments
                    .entry(environment_id.clone())
                    .or_insert_with(|| environment_manager.get_environment(environment_id))
                    .clone();
                let Some(environment) = environment else {
                    let message = format!(
                        "selected capability root `{}` references unavailable environment `{environment_id}`",
                        selected_root.id
                    );
                    return Box::pin(async move { Err(SelectedCapabilityFailure { message }) })
                        as ResolutionFuture;
                };
                Box::pin(async move {
                    provider
                        .resolve_selected_root_with_environment(
                            selection_order,
                            selected_root,
                            environment,
                        )
                        .await
                        .map_err(|err| SelectedCapabilityFailure {
                            message: err.to_string(),
                        })
                }) as ResolutionFuture
            })
            .collect();
        Self::from_resolutions(selected_roots, resolutions)
    }

    fn from_resolutions(
        selected_roots: Vec<SelectedCapabilityRoot>,
        resolutions: Vec<ResolutionFuture>,
    ) -> Self {
        assert_eq!(selected_roots.len(), resolutions.len());
        let inner = Arc::new(SelectedCapabilityBindingsInner {
            state: Mutex::new(SelectedCapabilityBindingsState {
                generation: 0,
                statuses: vec![SelectedCapabilityBindingStatus::Pending; selected_roots.len()],
            }),
            roots: selected_roots,
            changed: Notify::new(),
            resolution_task: Mutex::new(None),
        });

        if !resolutions.is_empty() {
            let weak_inner = Arc::downgrade(&inner);
            let resolution_task = tokio::spawn(async move {
                let mut active = FuturesUnordered::<IndexedResolutionFuture>::new();
                for (selection_order, resolution) in resolutions.into_iter().enumerate() {
                    active.push(index_resolution(selection_order, resolution));
                }
                while let Some((selection_order, resolution)) = active.next().await {
                    let Some(inner) = weak_inner.upgrade() else {
                        return;
                    };
                    let status = match resolution {
                        Ok(resolved) => SelectedCapabilityBindingStatus::Ready(Arc::new(resolved)),
                        Err(failure) => {
                            tracing::warn!(
                                selected_root = %inner.roots[selection_order].id,
                                error = failure.message(),
                                "failed to bind selected capability root"
                            );
                            SelectedCapabilityBindingStatus::Failed(Arc::new(failure))
                        }
                    };
                    {
                        let mut state = inner.state();
                        if !matches!(
                            &state.statuses[selection_order],
                            SelectedCapabilityBindingStatus::Pending
                        ) {
                            return;
                        }
                        state.statuses[selection_order] = status;
                        state.generation = state.generation.saturating_add(1);
                    }
                    inner.changed.notify_waiters();
                }
            });
            *inner
                .resolution_task
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(resolution_task.abort_handle());
        }

        Self { inner }
    }

    /// Captures current states without waiting for pending executors.
    pub fn snapshot(&self) -> SelectedCapabilitySnapshot {
        let state = self.inner.state();
        SelectedCapabilitySnapshot {
            generation: state.generation,
            entries: self
                .inner
                .roots
                .iter()
                .cloned()
                .zip(state.statuses.iter().cloned())
                .map(
                    |(selected_root, status)| SelectedCapabilityBindingSnapshot {
                        selected_root,
                        status,
                    },
                )
                .collect(),
        }
    }

    /// Waits until the binding generation differs from `generation`.
    pub async fn wait_for_change(&self, generation: u64) -> SelectedCapabilitySnapshot {
        loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let snapshot = self.snapshot();
            if snapshot.generation != generation {
                return snapshot;
            }
            changed.await;
        }
    }

    /// Waits for every selected root to become ready or permanently fail.
    pub async fn resolve_all(&self) -> SelectedCapabilitySnapshot {
        loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let snapshot = self.snapshot();
            if snapshot.is_terminal() {
                return snapshot;
            }
            changed.await;
        }
    }
}

fn index_resolution(
    selection_order: usize,
    resolution: ResolutionFuture,
) -> IndexedResolutionFuture {
    Box::pin(async move {
        let resolution = AssertUnwindSafe(resolution).catch_unwind().await;
        let resolution = resolution.unwrap_or_else(|_| {
            Err(SelectedCapabilityFailure {
                message: "selected capability resolution panicked".to_string(),
            })
        });
        (selection_order, resolution)
    })
}

impl SelectedCapabilityBindingsInner {
    fn state(&self) -> std::sync::MutexGuard<'_, SelectedCapabilityBindingsState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for SelectedCapabilityBindingsInner {
    fn drop(&mut self) {
        if let Some(resolution_task) = self
            .resolution_task
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            resolution_task.abort();
        }
    }
}

#[cfg(test)]
#[path = "selected_capabilities_tests.rs"]
mod tests;
