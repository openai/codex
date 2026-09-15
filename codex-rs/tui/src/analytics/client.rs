//! Local account report loading with backend-owned authentication and identity checks.
//! Cached responses belong to one local account and user, independently of the connected server.
//! Every report uses the view's captured end date until refresh replaces the session.

use super::models::AccountAnalyticsGrouping as Grouping;
use super::models::AccountAnalyticsReport as Report;
use super::models::AccountKind;
use super::report_data::AnalyticsData;
use crate::legacy_core::config::Config;
use codex_backend_client::AnalyticsReport;
use codex_backend_client::AnalyticsSession;
use codex_backend_client::RequestError;
use codex_protocol::account::PlanType;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

pub(super) struct Live {
    config: Arc<Config>,
    end_date: chrono::NaiveDate,
    session: OnceCell<Session>,
    token_models: std::sync::RwLock<Vec<String>>,
    attributed_usage: std::sync::atomic::AtomicBool,
}

pub(super) struct Session {
    pub(super) kind: AccountKind,
    pub(super) backend: AnalyticsSession,
    credit_groups: Vec<usize>,
    cache: Mutex<HashMap<(AnalyticsReport, String, String), AnalyticsData>>,
}

impl Live {
    pub(super) fn new(config: Arc<Config>, end_date: chrono::NaiveDate) -> Self {
        Self {
            config,
            end_date,
            session: OnceCell::new(),
            token_models: std::sync::RwLock::new(Vec::new()),
            attributed_usage: std::sync::atomic::AtomicBool::new(/*v*/ true),
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

    pub(super) fn attributed_usage(&self) -> bool {
        self.attributed_usage
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(super) fn token_models(&self) -> Vec<String> {
        self.token_models
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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
                    cache: Mutex::new(HashMap::new()),
                })
            })
            .await
    }

    pub(super) async fn plan_history(&self) -> Result<Option<super::plan::Report>, String> {
        let session = self.session().await?;
        if session.kind != AccountKind::Consumer {
            return Ok(None);
        }
        let history = session
            .backend
            .request(|client| async move { client.get_plan_limit_history().await })
            .await;
        session.backend.ensure_identity().await?;
        history
            .map_err(request_error)?
            .map(super::plan::Report::parse)
            .transpose()
            .map(Option::flatten)
    }

    pub(super) async fn history(
        &self,
        report: Report,
        days: u32,
        grouping: Grouping,
    ) -> Result<Option<super::models::AccountAnalyticsHistory>, String> {
        self.filtered_history(report, days, grouping, /*model_filter*/ None)
            .await
    }

    pub(super) async fn filtered_history(
        &self,
        report: Report,
        days: u32,
        grouping: Grouping,
        model_filter: Option<&str>,
    ) -> Result<Option<super::models::AccountAnalyticsHistory>, String> {
        let end = self.end_date;
        let start = days
            .checked_sub(/*rhs*/ 1)
            .and_then(|offset| end.checked_sub_days(chrono::Days::new(u64::from(offset))))
            .ok_or("Invalid analytics date range.")?;
        let session = self.session().await?;
        session.backend.ensure_identity().await?;
        let enterprise_tokens = report == Report::Usage
            && matches!(
                session.kind,
                AccountKind::Enterprise | AccountKind::Business
            );
        let grouping =
            if enterprise_tokens && !matches!(grouping, Grouping::Model | Grouping::TokenType) {
                Grouping::TokenType
            } else {
                grouping
            };
        let route =
            route(report, grouping, session.backend.account().plan_type).ok_or_else(|| {
                "This credit breakdown is not supported for this account type.".to_string()
            })?;
        // Credit events have no range parameters. Other grouping changes reuse the same payload.
        let key = if route == AnalyticsReport::Credits {
            (route, String::new(), String::new())
        } else {
            (route, start.to_string(), end.to_string())
        };
        let cached = session.cache.lock().await.get(&key).cloned();
        let response = if let Some(response) = cached {
            Ok(response)
        } else {
            session
                .backend
                .request(|client| {
                    let (_, start, end) = key.clone();
                    async move { client.get_account_analytics(route, &start, &end).await }
                })
                .await
                .map(AnalyticsData::from)
        };
        session.backend.ensure_identity().await?;
        let response = response.map_err(request_error)?;
        let history = (|| {
            if enterprise_tokens {
                let models = super::tokens::history(response.clone(), Grouping::Model, start, end)?;
                *self
                    .token_models
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    super::render::categories(&models)
                        .into_iter()
                        .map(|model| model.key)
                        .collect();
                super::tokens::filtered_history(
                    response.clone(),
                    grouping,
                    start,
                    end,
                    model_filter,
                )
                .map(Some)
            } else {
                let grouping = if report == Report::Usage {
                    let attributed =
                        super::normalize::has_complete_attribution(&response, start, end)?;
                    self.attributed_usage
                        .store(attributed, std::sync::atomic::Ordering::Relaxed);
                    if !attributed && matches!(grouping, Grouping::Feature | Grouping::TaskStart) {
                        Grouping::Surface
                    } else {
                        grouping
                    }
                } else {
                    grouping
                };
                super::normalize::history(response.clone(), report, grouping, start, end)
            }
        })();
        let mut cache = session.cache.lock().await;
        if history.is_ok() {
            cache.insert(key, response);
        } else {
            cache.remove(&key);
        }
        history
    }
}

// Account type selects billing semantics, never activity-report eligibility.
fn route(report: Report, grouping: Grouping, plan: Option<PlanType>) -> Option<AnalyticsReport> {
    let kind = AccountKind::from(plan);
    let enterprise = kind == AccountKind::Enterprise;
    Some(match report {
        Report::Usage if enterprise => AnalyticsReport::EnterpriseTokens,
        Report::Usage if kind == AccountKind::Business => AnalyticsReport::WorkspaceCredits,
        Report::Usage => AnalyticsReport::Usage,
        Report::Messages => AnalyticsReport::Messages,
        Report::Credits if !Grouping::credit_groupings(plan).contains(&grouping) => return None,
        Report::Credits if enterprise => AnalyticsReport::EnterpriseCredits {
            breakdown: match grouping {
                Grouping::Surface => "product",
                Grouping::Model => "model",
                Grouping::Speed => "speed",
                Grouping::Reasoning => "reasoning_effort",
                Grouping::Feature | Grouping::TaskStart | Grouping::TokenType => return None,
            },
        },
        Report::Credits if kind == AccountKind::Business => AnalyticsReport::WorkspaceCredits,
        Report::Credits => AnalyticsReport::Credits,
        Report::Plugins => AnalyticsReport::Plugins {
            limit: if enterprise { 8 } else { 10 },
        },
        Report::Skills => AnalyticsReport::Skills {
            limit: if enterprise { 6 } else { 10 },
        },
    })
}

pub(super) fn request_error(error: RequestError) -> String {
    match error.status().map(|status| status.as_u16()) {
        Some(401) => "Sign in again to load this report.",
        Some(403) => "Access denied for this report.",
        Some(404) => "This report endpoint is unavailable. Press R to retry.",
        _ => "Report request failed. Press R to retry.",
    }
    .into()
}

#[cfg(test)]
#[path = "client_tests.rs"]
pub(super) mod tests;

#[cfg(test)]
#[path = "account_plan_tests.rs"]
mod account_plan_tests;
