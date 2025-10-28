/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds a CommandSpec, and runs it under the current SandboxAttempt.
*/
use crate::command_safety::is_dangerous_command::command_might_be_dangerous;
use crate::command_safety::is_safe_command::is_known_safe_command;
use crate::exec::ExecToolCallOutput;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::execute_env;
use crate::tools::runtimes::build_command_spec;
use crate::tools::sandboxing::Approvable;
use crate::tools::sandboxing::ApprovalCtx;
use crate::tools::sandboxing::ProvidesSandboxRetryData;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxRetryData;
use crate::tools::sandboxing::Sandboxable;
use crate::tools::sandboxing::SandboxablePreference;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;
use crate::tools::sandboxing::ToolRuntime;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use futures::future::BoxFuture;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: std::collections::HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

impl ProvidesSandboxRetryData for ShellRequest {
    fn sandbox_retry_data(&self) -> Option<SandboxRetryData> {
        Some(SandboxRetryData {
            command: self.command.clone(),
            cwd: self.cwd.clone(),
        })
    }
}

#[derive(Default)]
pub struct ShellRuntime;

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: PathBuf,
    escalated: bool,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self
    }

    fn stdout_stream(ctx: &ToolCtx<'_>) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

// Peel common Windows shell launchers and return the inner argv slice.
fn strip_wrapper(argv: &[String]) -> &[String] {
    let mut i = 0;
    let eq = |s: &str, n: &str| {
        let sl = s.to_ascii_lowercase();
        sl == n || sl == format!("{n}.exe")
    };
    if argv.first().map(|s| eq(s, "wsl")).unwrap_or(false) {
        i += 1;
        if argv
            .get(i)
            .map(|t| {
                let t = t.to_ascii_lowercase();
                t == "--" || t == "-e" || t == "--exec"
            })
            .unwrap_or(false)
        {
            i += 1;
        }
        if argv.get(i).map(|s| eq(s, "bash")).unwrap_or(false)
            && argv
                .get(i + 1)
                .map(|s| s.eq_ignore_ascii_case("-lc"))
                .unwrap_or(false)
        {
            i += 2;
        }
        return &argv[i.min(argv.len())..];
    }
    if argv.first().map(|s| eq(s, "bash")).unwrap_or(false)
        && argv
            .get(1)
            .map(|s| s.eq_ignore_ascii_case("-lc"))
            .unwrap_or(false)
    {
        return &argv[2.min(argv.len())..];
    }
    if argv.first().map(|s| eq(s, "cmd")).unwrap_or(false)
        && argv
            .get(1)
            .map(|s| s.eq_ignore_ascii_case("/c") || s.eq_ignore_ascii_case("-c"))
            .unwrap_or(false)
    {
        return &argv[2.min(argv.len())..];
    }
    if argv
        .first()
        .map(|s| eq(s, "powershell") || eq(s, "pwsh"))
        .unwrap_or(false)
    {
        i = 1;
        while argv
            .get(i)
            .map(|s| s.eq_ignore_ascii_case("-noprofile"))
            .unwrap_or(false)
        {
            i += 1;
        }
        if argv
            .get(i)
            .map(|s| s.eq_ignore_ascii_case("-command") || s.eq_ignore_ascii_case("-c"))
            .unwrap_or(false)
        {
            i += 1;
        }
        return &argv[i.min(argv.len())..];
    }
    argv
}

// Build a tiny canonical key: program + first non-flag arg (with sed ranges de-noised).
fn canonicalize(argv: &[String]) -> Vec<String> {
    let args = strip_wrapper(argv);
    if args.is_empty() {
        return vec![];
    }
    let prog = std::path::Path::new(&args[0])
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&args[0])
        .to_ascii_lowercase();
    // Be conservative: preserve full argv; only de-noise clearly-volatile pieces.
    match prog.as_str() {
        // For sed, normalize digits in the first non-flag token (e.g., "10,20p" -> "NN,NNp").
        "sed" => {
            let mut out = Vec::with_capacity(args.len());
            out.push(prog);
            let mut normalized = false;
            for a in args.iter().skip(1) {
                if !normalized && !a.starts_with('-') {
                    out.push(
                        a.chars()
                            .map(|c| if c.is_ascii_digit() { 'N' } else { c })
                            .collect(),
                    );
                    normalized = true;
                } else {
                    out.push(a.clone());
                }
            }
            out
        }
        // For rg, keep the entire argv unchanged (wrapper-stripping handles cross-shell).
        "rg" => {
            let mut out = Vec::with_capacity(args.len());
            out.push(prog);
            out.extend(args.iter().skip(1).cloned());
            out
        }
        // Default: pass-through after stripping wrappers.
        _ => args.to_vec(),
    }
}

impl Sandboxable for ShellRuntime {
    fn sandbox_preference(&self) -> SandboxablePreference {
        SandboxablePreference::Auto
    }
    fn escalate_on_failure(&self) -> bool {
        true
    }
}

impl Approvable<ShellRequest> for ShellRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_key(&self, req: &ShellRequest) -> Self::ApprovalKey {
        ApprovalKey {
            command: req.command.clone(),
            cwd: req.cwd.clone(),
            escalated: req.with_escalated_permissions.unwrap_or(false),
        }
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let raw = self.approval_key(req);
        let stripped = ApprovalKey {
            command: strip_wrapper(&req.command).to_vec(),
            cwd: req.cwd.clone(),
            escalated: req.with_escalated_permissions.unwrap_or(false),
        };
        let canon = ApprovalKey {
            command: canonicalize(&req.command),
            cwd: req.cwd.clone(),
            escalated: req.with_escalated_permissions.unwrap_or(false),
        };
        let keys = vec![raw, stripped, canon];
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let reason = ctx
            .retry_reason
            .clone()
            .or_else(|| req.justification.clone());
        let risk = ctx.risk.clone();
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        Box::pin(async move {
            // Lookup in order: RAW → STRIPPED → CANONICAL. Persist all on approval.
            if let Some(decision) = session.services.tool_approvals.lock().await.get_any(&keys) {
                return decision;
            }

            let decision = session
                .request_command_approval(turn, call_id, command, cwd, reason, risk)
                .await;

            if matches!(decision, ReviewDecision::ApprovedForSession) {
                session
                    .services
                    .tool_approvals
                    .lock()
                    .await
                    .put_all(&keys, ReviewDecision::ApprovedForSession);
            }

            decision
        })
    }

    fn wants_initial_approval(
        &self,
        req: &ShellRequest,
        policy: AskForApproval,
        sandbox_policy: &SandboxPolicy,
    ) -> bool {
        if is_known_safe_command(&req.command) {
            return false;
        }
        match policy {
            AskForApproval::Never | AskForApproval::OnFailure => false,
            AskForApproval::OnRequest => {
                // In DangerFullAccess, only prompt if the command looks dangerous.
                if matches!(sandbox_policy, SandboxPolicy::DangerFullAccess) {
                    return command_might_be_dangerous(&req.command);
                }

                // In restricted sandboxes (ReadOnly/WorkspaceWrite), do not prompt for
                // non‑escalated, non‑dangerous commands — let the sandbox enforce
                // restrictions (e.g., block network/write) without a user prompt.
                let wants_escalation = req.with_escalated_permissions.unwrap_or(false);
                if wants_escalation {
                    return true;
                }
                command_might_be_dangerous(&req.command)
            }
            AskForApproval::UnlessTrusted => !is_known_safe_command(&req.command),
        }
    }

    fn wants_escalated_first_attempt(&self, req: &ShellRequest) -> bool {
        req.with_escalated_permissions.unwrap_or(false)
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    async fn run(
        &mut self,
        req: &ShellRequest,
        attempt: &SandboxAttempt<'_>,
        ctx: &ToolCtx<'_>,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let spec = build_command_spec(
            &req.command,
            &req.cwd,
            &req.env,
            req.timeout_ms,
            req.with_escalated_permissions,
            req.justification.clone(),
        )?;
        let env = attempt
            .env_for(&spec)
            .map_err(|err| ToolError::Codex(err.into()))?;
        let out = execute_env(&env, attempt.policy, Self::stdout_stream(ctx))
            .await
            .map_err(ToolError::Codex)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::sandboxing::ApprovalStore;
    use codex_protocol::protocol::ReviewDecision;
    use std::path::PathBuf;

    fn keys_for(cmd: &[&str]) -> Vec<ApprovalKey> {
        let v: Vec<String> = cmd.iter().map(std::string::ToString::to_string).collect();
        let cwd = PathBuf::from("C");
        let escalated = false;
        vec![
            ApprovalKey {
                command: v.clone(),
                cwd: cwd.clone(),
                escalated,
            },
            ApprovalKey {
                command: strip_wrapper(&v).to_vec(),
                cwd: cwd.clone(),
                escalated,
            },
            ApprovalKey {
                command: canonicalize(&v),
                cwd,
                escalated,
            },
        ]
    }

    #[test]
    fn approvals_match_across_windows_wrappers_and_sed_ranges() {
        let mut store = ApprovalStore::default();
        store.put_all(
            &keys_for(&["bash", "-lc", "sed", "-n", "10,20p", "file"]),
            ReviewDecision::ApprovedForSession,
        );

        for cmd in [
            &["wsl", "-e", "bash", "-lc", "sed", "-n", "15,25p", "file"][..],
            &[
                "pwsh",
                "-NoProfile",
                "-Command",
                "sed",
                "-n",
                "15,25p",
                "file",
            ][..],
            &["cmd", "/c", "sed", "-n", "15,25p", "file"][..],
        ] {
            assert_eq!(
                store.get_any(&keys_for(cmd)),
                Some(ReviewDecision::ApprovedForSession)
            );
        }

        // Different program should not match.
        assert!(store.get_any(&keys_for(&["rm", "-rf", "file"])).is_none());
    }

    #[test]
    fn approvals_match_rg_across_pwsh_bash_and_preserve_pattern() {
        let mut store = ApprovalStore::default();
        store.put_all(
            &keys_for(&["powershell", "-NoProfile", "-Command", "rg", "-n", "foo"]),
            ReviewDecision::ApprovedForSession,
        );

        for cmd in [
            &["pwsh", "-NoProfile", "-Command", "rg", "-n", "foo"][..],
            &["bash", "-lc", "rg", "-n", "foo"][..],
        ] {
            assert_eq!(
                store.get_any(&keys_for(cmd)),
                Some(ReviewDecision::ApprovedForSession)
            );
        }

        // Different pattern should not match canonical key.
        assert!(store.get_any(&keys_for(&["rg", "-n", "bar"])).is_none());
    }

    #[test]
    fn sed_canonicalization_does_not_cross_files() {
        let mut store = ApprovalStore::default();
        // Approve for session on one filename
        store.put_all(
            &keys_for(&["bash", "-lc", "sed", "-n", "10,20p", "safe.txt"]),
            ReviewDecision::ApprovedForSession,
        );
        // Same script/range but different filename must not match
        assert!(
            store
                .get_any(&keys_for(&[
                    "wsl",
                    "-e",
                    "bash",
                    "-lc",
                    "sed",
                    "-n",
                    "10,20p",
                    "secrets.txt",
                ]))
                .is_none()
        );
    }

    #[test]
    fn shell_requires_initial_approval_for_dangerous_command() {
        let rt = ShellRuntime::new();
        let req = ShellRequest {
            command: vec!["git".into(), "reset".into(), "--hard".into()],
            cwd: PathBuf::from("C"),
            timeout_ms: None,
            env: std::collections::HashMap::new(),
            with_escalated_permissions: None,
            justification: None,
        };
        // OnRequest + ReadOnly should request approval for dangerous commands
        assert!(rt.wants_initial_approval(
            &req,
            codex_protocol::protocol::AskForApproval::OnRequest,
            &crate::protocol::SandboxPolicy::ReadOnly
        ));

        // Known safe commands should not require approval under ReadOnly
        let safe = ShellRequest {
            command: vec!["echo".into(), "ok".into()],
            ..req.clone()
        };
        assert!(!rt.wants_initial_approval(
            &safe,
            codex_protocol::protocol::AskForApproval::OnRequest,
            &crate::protocol::SandboxPolicy::ReadOnly
        ));
    }

    #[test]
    fn approvals_cache_allows_dangerous_command_across_wrappers() {
        // Approve for session under pwsh wrapper
        let mut store = ApprovalStore::default();
        store.put_all(
            &keys_for(&["pwsh", "-NoProfile", "-Command", "git", "reset", "--hard"]),
            ReviewDecision::ApprovedForSession,
        );
        // Should match under bash -lc wrapper
        assert_eq!(
            store.get_any(&keys_for(&["bash", "-lc", "git", "reset", "--hard"])),
            Some(ReviewDecision::ApprovedForSession)
        );
    }
}
