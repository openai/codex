use std::sync::Arc;

use codex_app_server_protocol::AgentCommunication;
use codex_app_server_protocol::ServerNotification;
use codex_core::AgentCommunicationSink;
use codex_protocol::protocol::AgentCommunicationRecord;
use tokio::sync::mpsc;

use crate::outgoing_message::OutgoingMessageSender;

pub(crate) fn app_server_agent_communication_sink(
    outgoing: Arc<OutgoingMessageSender>,
) -> Arc<dyn AgentCommunicationSink> {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(record) = receiver.recv().await {
            outgoing
                .send_server_notification(ServerNotification::AgentCommunicationUpdated(
                    AgentCommunication::from(record),
                ))
                .await;
        }
    });
    Arc::new(AppServerAgentCommunicationSink { sender })
}

struct AppServerAgentCommunicationSink {
    sender: mpsc::UnboundedSender<AgentCommunicationRecord>,
}

impl AgentCommunicationSink for AppServerAgentCommunicationSink {
    fn emit(&self, record: AgentCommunicationRecord) {
        let _ = self.sender.send(record);
    }
}
