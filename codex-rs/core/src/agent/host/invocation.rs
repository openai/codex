//! Bind host child preparation and startup to a live tool invocation.

use super::spawn::SpawnDestination;
use super::spawn::prepare_spawn;
use super::spawn::start_prepared;
use super::types::HostThreadSpawnReservation;
use super::types::PreparedThreadSpawn;
use crate::codex_thread::CodexThread;
use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use crate::thread_spawn::ChildSpawnSettings;
use crate::thread_spawn::ThreadCreateRequest;
use crate::thread_spawn::ThreadSpawnError;
use crate::thread_spawn::ThreadSpawnResult;
use crate::tools::context::ToolInvocation;
use codex_protocol::AgentPath;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use futures::TryFutureExt;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::Weak;
use tokio_util::sync::CancellationToken;

struct InvocationContext {
    step: Weak<StepContext>,
    finished: Mutex<CancellationToken>,
}

impl InvocationContext {
    fn finished(&self) -> MutexGuard<'_, CancellationToken> {
        self.finished.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Install in thread extension data to enable live V2 tool lookup and require unique active
/// call IDs within each turn. Host-spawned children inherit this opt-in.
pub struct HostToolInvocationLookup;

#[derive(Default)]
struct InvocationContexts(Mutex<HashMap<(String, String), Weak<InvocationContext>>>);

/// A handle to one tool call, valid only on its issuing runtime until dispatch ends.
/// It holds the runtime and step weakly. Child preparation captures history and policy.
pub struct ToolInvocationContext {
    session: Weak<Session>,
    call_id: String,
    context: Arc<InvocationContext>,
}

impl ToolInvocationContext {
    /// Order publication with revocation of the issuing tool call.
    pub(super) fn lock_active(
        &self,
    ) -> Result<MutexGuard<'_, CancellationToken>, ThreadSpawnError> {
        let finished = self.context.finished();
        if finished.is_cancelled() {
            return Err(ThreadSpawnError::Preparation(
                "spawning tool call finished".into(),
            ));
        }
        Ok(finished)
    }
}

pub(crate) fn tool_invocation(
    thread: &CodexThread,
    turn_id: &str,
    call_id: &str,
) -> CodexResult<ToolInvocationContext> {
    let context = thread
        .session
        .services
        .thread_extension_data
        .get::<InvocationContexts>()
        .and_then(|contexts| {
            contexts
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&(turn_id.to_owned(), call_id.to_owned()))
                .and_then(Weak::upgrade)
        })
        .filter(|context| !context.finished().is_cancelled())
        .ok_or_else(|| CodexErr::InvalidRequest("tool call is not active".into()))?;
    Ok(ToolInvocationContext {
        session: Arc::downgrade(&thread.session),
        call_id: call_id.to_owned(),
        context,
    })
}

pub(crate) struct ReservedInvocation {
    contexts: Arc<InvocationContexts>,
    key: (String, String),
}

/// Keeps the dispatch's ID reserved until its terminal callback returns.
#[derive(Default)]
pub(crate) struct InvocationReservation(Mutex<Option<ReservedInvocation>>);

impl InvocationReservation {
    pub(crate) fn reserve(
        &self,
        session: &Session,
        turn: &TurnContext,
        call_id: &str,
    ) -> Result<(), FunctionCallError> {
        if turn.multi_agent_version != MultiAgentVersion::V2
            || session
                .services
                .thread_extension_data
                .get::<HostToolInvocationLookup>()
                .is_none()
        {
            return Ok(());
        }

        let contexts = session
            .services
            .thread_extension_data
            .get_or_init(InvocationContexts::default);
        let key = (turn.sub_id.clone(), call_id.to_owned());
        match contexts
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key.clone())
        {
            Entry::Vacant(entry) => {
                entry.insert(Weak::new());
            }
            Entry::Occupied(_) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "tool call ID {call_id} is already active in this turn"
                )));
            }
        }
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(ReservedInvocation { contexts, key });
        Ok(())
    }

    pub(crate) fn activate(&self, invocation: &ToolInvocation) -> Option<InvocationGuard> {
        let reservation = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let reservation = reservation.as_ref()?;
        let context = Arc::new(InvocationContext {
            step: Arc::downgrade(&invocation.step_context),
            finished: Mutex::new(invocation.cancellation_token.child_token()),
        });
        let mut contexts = reservation
            .contexts
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *contexts.get_mut(&reservation.key)? = Arc::downgrade(&context);
        Some(InvocationGuard(context))
    }

    pub(crate) fn take(&self) -> Option<ReservedInvocation> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

/// Revokes host access when execution ends, including unwinding or cancellation.
pub(crate) struct InvocationGuard(Arc<InvocationContext>);

impl Drop for InvocationGuard {
    fn drop(&mut self) {
        self.0.finished().cancel();
    }
}

impl Drop for ReservedInvocation {
    fn drop(&mut self) {
        self.contexts
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

pub(crate) async fn create(
    thread: &CodexThread,
    invocation: &ToolInvocationContext,
    request: ThreadCreateRequest,
    reservation: HostThreadSpawnReservation,
) -> Result<ThreadSpawnResult, ThreadSpawnError> {
    with_spawn_request(
        thread,
        invocation,
        request,
        |request, task_name| async move {
            let prepared = Box::pin(prepare_spawn(
                thread,
                request,
                task_name,
                SpawnDestination::SameManager,
            ))
            .await?;
            Box::pin(start_prepared(
                &thread.session.services.local_agent_runtime,
                prepared,
                reservation,
                Some(invocation),
            ))
            .await
        },
    )
    .await
}

pub(crate) async fn prepare(
    thread: &CodexThread,
    invocation: &ToolInvocationContext,
    request: ThreadCreateRequest,
) -> Result<PreparedThreadSpawn, ThreadSpawnError> {
    with_spawn_request(thread, invocation, request, |request, task_name| {
        prepare_spawn(thread, request, task_name, SpawnDestination::Detached)
            .map_err(ThreadSpawnError::from)
    })
    .await
}

async fn with_spawn_request<T, F, Fut>(
    thread: &CodexThread,
    invocation: &ToolInvocationContext,
    request: ThreadCreateRequest,
    operation: F,
) -> Result<T, ThreadSpawnError>
where
    F: FnOnce(ChildSpawnSettings, AgentPath) -> Fut,
    Fut: Future<Output = Result<T, ThreadSpawnError>>,
{
    let session = &thread.session;
    if !invocation.session.ptr_eq(&Arc::downgrade(session)) {
        return Err(CodexErr::InvalidRequest(
            "tool invocation belongs to a different runtime".into(),
        )
        .into());
    }
    let context = &invocation.context;
    let finished = context.finished().clone();
    let step = context.step.upgrade().ok_or_else(|| {
        ThreadSpawnError::Preparation("spawning step is no longer available".into())
    })?;
    tokio::select! {
        biased;
        _ = finished.cancelled() => Err(ThreadSpawnError::Preparation("spawning tool call finished".into())),
        result = async {
            let _caller_pin = Arc::clone(&thread.residency_gate).read_owned().await;
            if !thread.is_running() {
                return Err(CodexErr::ThreadNotFound(session.thread_id).into());
            }
            let (request, task_name) = crate::thread_spawn::prepare(session, &step, &invocation.call_id, request).await?;
            operation(request, task_name).await
        } => result,
    }
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
