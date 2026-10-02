//! Refreshes the email omitted by account-update notifications without blocking terminal input.
//! Results carry a request ID so account changes and reconnects reject obsolete identities.

use super::App;
use crate::app_event::AppEvent;
use crate::app_server_session::AppServerSession;
use codex_app_server_protocol::Account;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::GetAccountParams;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::RequestId;
use std::time::Duration;
use uuid::Uuid;

impl App {
    pub(super) fn refresh_account_email(&mut self, app_server: &AppServerSession) {
        let request_id = Uuid::new_v4();
        self.account_email_request_id = Some(request_id);
        let request_handle = app_server.request_handle();
        let app_event_tx = self.app_event_tx.clone();
        tokio::spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(/*secs*/ 30),
                request_handle.request_typed::<GetAccountResponse>(ClientRequest::GetAccount {
                    request_id: RequestId::String(format!("account-status-{request_id}")),
                    params: GetAccountParams {
                        refresh_token: false,
                    },
                }),
            )
            .await;
            match result {
                Ok(Ok(response)) => {
                    if let Some(Account::Chatgpt { email, .. }) = response.account {
                        app_event_tx.send(AppEvent::AccountEmailLoaded { request_id, email });
                    }
                }
                Ok(Err(_)) | Err(_) => {
                    tracing::warn!("failed to read account identity after account update");
                }
            }
        });
    }
}
