//! Local account sessions and display metadata for authenticated analytics.
//! The backend client owns credentials, recovery, and request identity checks.

use super::models::AccountAnalyticsGrouping as Grouping;
use super::models::AccountKind;
use crate::legacy_core::config::Config;
use codex_backend_client::AnalyticsSession;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub(super) struct Live {
    config: Arc<Config>,
    session: OnceCell<Session>,
}

pub(super) struct Session {
    pub(super) kind: AccountKind,
    pub(super) backend: AnalyticsSession,
    credit_groups: Vec<usize>,
}

impl Live {
    pub(super) fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            session: OnceCell::new(),
        }
    }

    pub(super) fn account_label(&self) -> Option<String> {
        let session = self.session.get()?;
        let account = session.backend.account();
        Some(match &account.email {
            Some(email) => format!("{email} · {}", account.id),
            None => account.id.clone(),
        })
    }

    pub(super) fn credit_groups(&self) -> &[usize] {
        self.session
            .get()
            .map_or(&[0], |session| &session.credit_groups)
    }

    pub(super) async fn session(&self) -> Result<&Session, String> {
        self.session
            .get_or_try_init(|| async {
                let session = AnalyticsSession::from_config(
                    self.config.as_ref(),
                    self.config.http_client_factory(),
                )
                .await?;
                let credit_groups = Grouping::credit_groupings(session.account().plan_type)
                    .iter()
                    .filter_map(|group| {
                        super::data::GROUPINGS
                            .iter()
                            .position(|candidate| candidate == group)
                    })
                    .collect();
                Ok(Session {
                    kind: AccountKind::from(session.account().plan_type),
                    credit_groups,
                    backend: session,
                })
            })
            .await
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
pub(super) mod tests;
