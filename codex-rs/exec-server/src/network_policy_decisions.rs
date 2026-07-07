use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;

use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkPolicyRequest;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::ProcessId;
use crate::protocol::NETWORK_POLICY_REQUEST_METHOD;
use crate::protocol::NetworkPolicyDecisionNotification;
use crate::protocol::NetworkPolicyRequestNotification;
use crate::rpc::RpcNotificationSender;

#[derive(Default)]
pub(crate) struct NetworkPolicyDecisionRelay {
    pending: Mutex<HashMap<String, PendingNetworkPolicyDecision>>,
}

struct PendingNetworkPolicyDecision {
    process_id: ProcessId,
    response_tx: oneshot::Sender<NetworkDecision>,
}

impl NetworkPolicyDecisionRelay {
    pub(crate) fn decider(
        self: &Arc<Self>,
        process_id: ProcessId,
        notifications: Arc<RwLock<Option<RpcNotificationSender>>>,
    ) -> Arc<dyn NetworkPolicyDecider> {
        let relay = Arc::clone(self);
        Arc::new(move |request: NetworkPolicyRequest| {
            let relay = Arc::clone(&relay);
            let process_id = process_id.clone();
            let notifications = Arc::clone(&notifications);
            async move { relay.request(process_id, request, notifications).await }
        })
    }

    pub(crate) fn resolve(&self, params: NetworkPolicyDecisionNotification) -> Result<(), String> {
        let mut pending_requests = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(request) = pending_requests.get(&params.request_id) else {
            return Ok(());
        };
        if request.process_id != params.process_id {
            return Err("network policy decision process id does not match request".to_string());
        }
        let Some(pending) = pending_requests.remove(&params.request_id) else {
            return Ok(());
        };
        drop(pending_requests);
        let _ = pending.response_tx.send(params.decision);
        Ok(())
    }

    pub(crate) fn fail_pending(&self, process_id: Option<&ProcessId>) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match process_id {
            Some(process_id) => pending.retain(|_, pending| pending.process_id != *process_id),
            None => pending.clear(),
        }
    }

    async fn request(
        &self,
        process_id: ProcessId,
        request: NetworkPolicyRequest,
        notifications: Arc<RwLock<Option<RpcNotificationSender>>>,
    ) -> NetworkDecision {
        let request_id = Uuid::new_v4().to_string();
        let (response_tx, response_rx) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                request_id.clone(),
                PendingNetworkPolicyDecision {
                    process_id: process_id.clone(),
                    response_tx,
                },
            );
        let notification = NetworkPolicyRequestNotification {
            request_id: request_id.clone(),
            process_id,
            request,
        };
        let Some(notifications) = notifications
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        else {
            self.fail(&request_id);
            return NetworkDecision::deny("not_allowed");
        };
        if notifications
            .notify(NETWORK_POLICY_REQUEST_METHOD, &notification)
            .await
            .is_err()
        {
            self.fail(&request_id);
            return NetworkDecision::deny("not_allowed");
        }
        response_rx
            .await
            .unwrap_or_else(|_| NetworkDecision::deny("not_allowed"))
    }

    fn fail(&self, request_id: &str) {
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(request_id);
    }
}

#[cfg(test)]
#[path = "network_policy_decisions_tests.rs"]
mod tests;
