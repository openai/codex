//! Direct-message diagnostics and delivery ownership across queues and eviction.
//! Only history commitment consumes mail; dropping its last owner releases its attachments.

use codex_extension_api::ExtensionData;
use codex_extension_api::MessageLifecycleContributor;
use codex_extension_api::MessageLifecycleInput;
use codex_protocol::ThreadId;
use codex_protocol::protocol::InterAgentCommunication;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

pub(crate) static PENDING_MAILBOX_MESSAGES: codex_diagnostics::Gauge =
    codex_diagnostics::Gauge::new("core.mailbox.pending");

const AGENT_COMMUNICATION_TARGET: &str = "codex_otel.agent_communication";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentCommunicationKind {
    Spawn,
    Message,
    Followup,
    Result,
    CompletionWatcher,
}

impl AgentCommunicationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Message => "message",
            Self::Followup => "followup",
            Self::Result | Self::CompletionWatcher => "result",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentCommunicationContext {
    pub(crate) kind: AgentCommunicationKind,
    pub(crate) sender_thread_id: ThreadId,
}

impl AgentCommunicationContext {
    pub(crate) fn new(kind: AgentCommunicationKind, sender_thread_id: ThreadId) -> Self {
        Self {
            kind,
            sender_thread_id,
        }
    }
}

pub(crate) fn send_log_content(
    communication: &InterAgentCommunication,
) -> Option<Cow<'static, str>> {
    tracing::enabled!(target: AGENT_COMMUNICATION_TARGET, tracing::Level::INFO).then(|| {
        match &communication.encrypted_content {
            Some(content) => Cow::Owned(content.clone()),
            None => Cow::Borrowed("[plaintext]"),
        }
    })
}

pub(crate) fn emit_agent_communication_send(
    communication_id: &str,
    context: &AgentCommunicationContext,
    log_content: Option<&str>,
    receiver_thread_id: ThreadId,
) {
    let Some(content) = log_content else {
        return;
    };
    tracing::info!(
        target: AGENT_COMMUNICATION_TARGET,
        {
            event.name = "codex.agent_communication",
            communication_id,
            kind = context.kind.as_str(),
            state = "send",
            sender_thread_id = %context.sender_thread_id,
            receiver_thread_id = %receiver_thread_id,
            content,
        },
        "agent communication"
    );
}

pub(crate) fn emit_agent_communication_receive(communication_id: &str) {
    tracing::info!(
        target: AGENT_COMMUNICATION_TARGET,
        {
            event.name = "codex.agent_communication",
            communication_id,
            state = "receive",
        },
        "agent communication"
    );
}

/// Pending mail with shared ownership of its captured publication state.
/// Serialization preserves the protocol shape; restored historical mail is not republished.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingAgentMessage {
    communication: InterAgentCommunication,
    #[serde(skip)]
    pub(crate) delivery: Option<MessageDelivery>,
}

impl From<InterAgentCommunication> for PendingAgentMessage {
    fn from(communication: InterAgentCommunication) -> Self {
        Self::new(communication, /*delivery*/ None)
    }
}

impl Deref for PendingAgentMessage {
    type Target = InterAgentCommunication;
    fn deref(&self) -> &Self::Target {
        &self.communication
    }
}

impl PartialEq for PendingAgentMessage {
    fn eq(&self, other: &Self) -> bool {
        self.communication == other.communication
    }
}

impl PendingAgentMessage {
    pub(crate) fn new(
        communication: InterAgentCommunication,
        delivery: Option<MessageDelivery>,
    ) -> Self {
        Self {
            communication,
            delivery,
        }
    }

    pub(crate) fn is_terminal_result(&self) -> bool {
        self.delivery
            .as_ref()
            .is_some_and(MessageDelivery::awaits_result_write)
    }

    pub(crate) fn consumed(&self, recipient: ThreadId) -> Option<ResultWrite> {
        self.delivery.as_ref()?.consumed(recipient)
    }
}

/// Only a confirmed append releases a terminal result. Dropping this receipt
/// leaves the write outstanding, so an ambiguous handoff fails.
pub(crate) struct ResultWrite(Arc<AtomicUsize>);

impl ResultWrite {
    pub(crate) fn confirm(self) {
        self.0.fetch_sub(1, Ordering::Release);
    }
}

/// The publication state retained until this delivery is consumed or abandoned.
struct MessagePublication {
    input: MessageLifecycleInput,
    observers: Box<[Arc<dyn MessageLifecycleContributor>]>,
    result_write: Option<ResultWrite>,
}

/// Clones follow one logical delivery through queues and share its consumption state.
#[derive(Clone)]
pub(crate) struct MessageDelivery(Arc<Mutex<Option<MessagePublication>>>);

impl MessageDelivery {
    pub(crate) fn publish(
        sender: ThreadId,
        observers: &[Arc<dyn MessageLifecycleContributor>],
        unconfirmed_results: Option<&Arc<AtomicUsize>>,
    ) -> Option<Self> {
        if observers.is_empty() && unconfirmed_results.is_none() {
            return None;
        }
        let input = MessageLifecycleInput {
            sender,
            data: ExtensionData::new(uuid::Uuid::now_v7().to_string()),
        };
        for observer in observers {
            observer.on_publish(&input);
        }
        let result_write = unconfirmed_results.map(|count| {
            count.fetch_add(1, Ordering::Relaxed);
            ResultWrite(Arc::clone(count))
        });
        let publication = MessagePublication {
            input,
            observers: observers.into(),
            result_write,
        };
        Some(Self(Arc::new(Mutex::new(Some(publication)))))
    }

    pub(crate) fn awaits_result_write(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|publication| publication.result_write.is_some())
    }

    /// The first consumer owns both the publication hooks and the write receipt.
    pub(crate) fn consumed(&self, recipient: ThreadId) -> Option<ResultWrite> {
        let publication = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()?;
        for observer in &publication.observers {
            observer.on_consumed(&publication.input, recipient);
        }
        publication.result_write
    }
}

impl fmt::Debug for MessageDelivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageDelivery").finish_non_exhaustive()
    }
}
