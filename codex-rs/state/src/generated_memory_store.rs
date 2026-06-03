use crate::MemoryStore;
use crate::Phase2JobClaimOutcome;
use crate::Stage1JobClaim;
use crate::Stage1Output;
use crate::Stage1StartupClaimParams;
use codex_protocol::ThreadId;
use std::future::Future;
use std::pin::Pin;

/// Boxed generated-memory store future usable behind a trait object.
pub type GeneratedMemoryStoreFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

macro_rules! generated_memory_store {
    ($( $(#[$method_doc:meta])* fn $method:ident<$lifetime:lifetime>(
        $($arg:ident: $arg_type:ty),* $(,)?
    ) -> $output:ty; )*) => {
        /// Persistence boundary for generated-memory startup job and output state.
        ///
        /// Implementations own the stage-1 extraction rows plus the singleton
        /// phase-2 consolidation lease. Callers rely on [`MemoryStore`]
        /// ownership-token semantics: successful writes return `false` after
        /// the caller loses the relevant job.
        pub trait GeneratedMemoryStore: Send + Sync {
            $( $(#[$method_doc])* fn $method<$lifetime>(
                &$lifetime self,
                $($arg: $arg_type),*
            ) -> GeneratedMemoryStoreFuture<$lifetime, $output>; )*
        }

        impl GeneratedMemoryStore for MemoryStore {
            $( fn $method<$lifetime>(
                &$lifetime self,
                $($arg: $arg_type),*
            ) -> GeneratedMemoryStoreFuture<$lifetime, $output> {
                Box::pin(MemoryStore::$method(self, $($arg),*))
            } )*
        }
    };
}

generated_memory_store! {
    /// Prunes stale generated stage-1 outputs that are no longer retained.
    fn prune_stage1_outputs_for_retention<'a>(
        max_unused_days: i64,
        limit: usize,
    ) -> anyhow::Result<usize>;
    /// Claims eligible startup stage-1 extraction jobs.
    fn claim_stage1_jobs_for_startup<'a>(
        current_thread_id: ThreadId,
        params: Stage1StartupClaimParams<'a>,
    ) -> anyhow::Result<Vec<Stage1JobClaim>>;
    /// Marks an owned stage-1 extraction job successful with generated output.
    fn mark_stage1_job_succeeded<'a>(
        thread_id: ThreadId,
        ownership_token: &'a str,
        source_updated_at: i64,
        raw_memory: &'a str,
        rollout_summary: &'a str,
        rollout_slug: Option<&'a str>,
    ) -> anyhow::Result<bool>;
    /// Marks an owned stage-1 extraction job successful without output.
    fn mark_stage1_job_succeeded_no_output<'a>(
        thread_id: ThreadId,
        ownership_token: &'a str,
    ) -> anyhow::Result<bool>;
    /// Marks an owned stage-1 extraction job failed with retry backoff.
    fn mark_stage1_job_failed<'a>(
        thread_id: ThreadId,
        ownership_token: &'a str,
        failure_reason: &'a str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool>;
    /// Claims the singleton global phase-2 consolidation lease.
    fn try_claim_global_phase2_job<'a>(
        worker_id: ThreadId,
        lease_seconds: i64,
    ) -> anyhow::Result<Phase2JobClaimOutcome>;
    /// Returns the current generated-memory inputs for phase-2 consolidation.
    fn get_phase2_input_selection<'a>(
        n: usize,
        max_unused_days: i64,
    ) -> anyhow::Result<Vec<Stage1Output>>;
    /// Extends the owned singleton global phase-2 consolidation lease.
    fn heartbeat_global_phase2_job<'a>(
        ownership_token: &'a str,
        lease_seconds: i64,
    ) -> anyhow::Result<bool>;
    /// Marks the owned singleton global phase-2 consolidation job successful.
    fn mark_global_phase2_job_succeeded<'a>(
        ownership_token: &'a str,
        completed_watermark: i64,
        selected_outputs: &'a [Stage1Output],
    ) -> anyhow::Result<bool>;
    /// Marks the owned singleton global phase-2 consolidation job failed.
    fn mark_global_phase2_job_failed<'a>(
        ownership_token: &'a str,
        failure_reason: &'a str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool>;
    /// Finalizes a failed singleton phase-2 job after ownership may be lost.
    fn mark_global_phase2_job_failed_if_unowned<'a>(
        ownership_token: &'a str,
        failure_reason: &'a str,
        retry_delay_seconds: i64,
    ) -> anyhow::Result<bool>;
}
