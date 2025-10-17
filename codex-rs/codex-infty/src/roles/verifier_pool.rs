use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use anyhow::Result;
use codex_core::ConversationManager;
use codex_core::cross_session::CrossSessionHub;
use codex_core::protocol::Op;

use crate::progress::ProgressReporter;
use crate::prompts;
use crate::roles::Role;
use crate::roles::verifier::VerificationRequestPayload;
use crate::roles::verifier::VerifierRole;
use crate::roles::verifier::aggregate_verdicts;
use crate::session;
use crate::signals::AggregatedVerifierVerdict;
use crate::signals::VerifierVerdict;
use crate::types::RoleConfig;
use crate::types::RunSessions;

pub struct VerificationRound {
    pub summary: AggregatedVerifierVerdict,
    pub passing_roles: Vec<String>,
}

pub struct VerifierPool {
    hub: Arc<CrossSessionHub>,
    run_id: String,
    timeout: Duration,
    progress: Option<Arc<dyn ProgressReporter>>,
    roles: Vec<VerifierRole>,
}

impl VerifierPool {
    pub fn from_sessions(
        hub: Arc<CrossSessionHub>,
        sessions: &RunSessions,
        timeout: Duration,
        progress: Option<Arc<dyn ProgressReporter>>,
    ) -> Self {
        let roles = sessions
            .verifiers
            .iter()
            .map(|v| {
                VerifierRole::new(
                    Arc::clone(&hub),
                    sessions.run_id.clone(),
                    v.role.clone(),
                    timeout,
                    progress.clone(),
                )
            })
            .collect();
        Self {
            hub,
            run_id: sessions.run_id.clone(),
            timeout,
            progress,
            roles,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }

    pub async fn collect_round(
        &self,
        request: &VerificationRequestPayload<'_>,
    ) -> Result<VerificationRound> {
        let futures = self
            .roles
            .iter()
            .map(|role| async {
                let name = role.role().to_string();
                let verdict = role.call(request).await;
                (name, verdict)
            })
            .collect::<Vec<_>>();
        let joined = futures::future::join_all(futures).await;

        let mut results: Vec<(String, VerifierVerdict)> = Vec::with_capacity(joined.len());
        let mut passing_roles: Vec<String> = Vec::new();
        for (name, verdict_res) in joined.into_iter() {
            let verdict = verdict_res
                .with_context(|| format!("verifier {} returned invalid verdict JSON", name))?;
            if let Some(progress) = self.progress.as_ref() {
                progress.verifier_verdict(&name, &verdict);
            }
            if verdict.verdict.is_pass() {
                passing_roles.push(name.clone());
            }
            results.push((name, verdict));
        }
        let summary = aggregate_verdicts(results);
        Ok(VerificationRound {
            summary,
            passing_roles,
        })
    }

    pub fn replace_role(&mut self, role_name: &str) {
        if let Some(idx) = self.roles.iter().position(|v| v.role() == role_name) {
            self.roles[idx] = VerifierRole::new(
                Arc::clone(&self.hub),
                self.run_id.clone(),
                role_name.to_string(),
                self.timeout,
                self.progress.clone(),
            );
        }
    }

    pub async fn rotate_passing(
        &mut self,
        sessions: &mut RunSessions,
        manager: &ConversationManager,
        passing_roles: &[String],
    ) -> Result<()> {
        for role in passing_roles {
            // find existing index
            let Some(idx) = sessions.verifiers.iter().position(|s| &s.role == role) else {
                continue;
            };
            let old = &sessions.verifiers[idx];
            // best-effort shutdown and unregister
            let _ = old.conversation.submit(Op::Shutdown).await;
            let _ = manager.remove_conversation(&old.conversation_id).await;

            // Reuse the existing verifier's config so overrides (e.g., base_url in tests)
            // are preserved when respawning a passing verifier.
            let config = old.config.clone();
            let role_config = RoleConfig::new(role.to_string(), config);
            let run_path = sessions.store.path();
            let session = session::spawn_role(
                Arc::clone(&self.hub),
                manager,
                &self.run_id,
                run_path,
                role_config,
                prompts::ensure_instructions,
            )
            .await?;
            sessions
                .store
                .update_rollout_path(&session.role, session.rollout_path.clone())?;
            sessions.verifiers[idx] = session;
            self.replace_role(role);
        }
        Ok(())
    }
}
