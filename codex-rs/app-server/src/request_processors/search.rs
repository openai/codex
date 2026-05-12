use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::fuzzy_file_search::FuzzyFileSearchSession;
use crate::fuzzy_file_search::run_fuzzy_file_search;
use crate::fuzzy_file_search::start_fuzzy_file_search_session;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::FuzzyFileSearchParams;
use codex_app_server_protocol::FuzzyFileSearchResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionStartParams;
use codex_app_server_protocol::FuzzyFileSearchSessionStartResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionStopParams;
use codex_app_server_protocol::FuzzyFileSearchSessionStopResponse;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdateParams;
use codex_app_server_protocol::FuzzyFileSearchSessionUpdateResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct SearchRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    pending_fuzzy_searches: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    fuzzy_search_sessions: Arc<Mutex<HashMap<String, FuzzyFileSearchSession>>>,
}

impl SearchRequestProcessor {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>) -> Self {
        Self {
            outgoing,
            pending_fuzzy_searches: Arc::new(Mutex::new(HashMap::new())),
            fuzzy_search_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn fuzzy_file_search(
        &self,
        params: FuzzyFileSearchParams,
    ) -> Result<FuzzyFileSearchResponse, JSONRPCErrorError> {
        let FuzzyFileSearchParams {
            query,
            roots,
            cancellation_token,
        } = params;

        let cancel_flag = match cancellation_token.clone() {
            Some(token) => {
                let mut pending_fuzzy_searches = self.pending_fuzzy_searches.lock().await;
                // if a cancellation_token is provided and a pending_request exists for
                // that token, cancel it
                if let Some(existing) = pending_fuzzy_searches.get(&token) {
                    existing.store(true, Ordering::Relaxed);
                }
                let flag = Arc::new(AtomicBool::new(false));
                pending_fuzzy_searches.insert(token.clone(), flag.clone());
                flag
            }
            None => Arc::new(AtomicBool::new(false)),
        };

        let results = match query.as_str() {
            "" => vec![],
            _ => run_fuzzy_file_search(query, roots, cancel_flag.clone()).await,
        };

        if let Some(token) = cancellation_token {
            let mut pending_fuzzy_searches = self.pending_fuzzy_searches.lock().await;
            if let Some(current_flag) = pending_fuzzy_searches.get(&token)
                && Arc::ptr_eq(current_flag, &cancel_flag)
            {
                pending_fuzzy_searches.remove(&token);
            }
        }

        Ok(FuzzyFileSearchResponse { files: results })
    }

    pub(crate) async fn fuzzy_file_search_session_start_response(
        &self,
        params: FuzzyFileSearchSessionStartParams,
    ) -> Result<FuzzyFileSearchSessionStartResponse, JSONRPCErrorError> {
        let FuzzyFileSearchSessionStartParams { session_id, roots } = params;
        if session_id.is_empty() {
            return Err(invalid_request("sessionId must not be empty"));
        }

        let session =
            start_fuzzy_file_search_session(session_id.clone(), roots, self.outgoing.clone())
                .map_err(|err| {
                    internal_error(format!("failed to start fuzzy file search session: {err}"))
                })?;
        self.fuzzy_search_sessions
            .lock()
            .await
            .insert(session_id, session);
        Ok(FuzzyFileSearchSessionStartResponse {})
    }

    pub(crate) async fn fuzzy_file_search_session_update_response(
        &self,
        params: FuzzyFileSearchSessionUpdateParams,
    ) -> Result<FuzzyFileSearchSessionUpdateResponse, JSONRPCErrorError> {
        let FuzzyFileSearchSessionUpdateParams { session_id, query } = params;
        let found = {
            let sessions = self.fuzzy_search_sessions.lock().await;
            if let Some(session) = sessions.get(&session_id) {
                session.update_query(query);
                true
            } else {
                false
            }
        };
        if !found {
            return Err(invalid_request(format!(
                "fuzzy file search session not found: {session_id}"
            )));
        }

        Ok(FuzzyFileSearchSessionUpdateResponse {})
    }

    pub(crate) async fn fuzzy_file_search_session_stop(
        &self,
        params: FuzzyFileSearchSessionStopParams,
    ) -> Result<FuzzyFileSearchSessionStopResponse, JSONRPCErrorError> {
        let FuzzyFileSearchSessionStopParams { session_id } = params;
        self.fuzzy_search_sessions.lock().await.remove(&session_id);

        Ok(FuzzyFileSearchSessionStopResponse {})
    }
}
