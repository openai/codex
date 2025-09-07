//! Reviewer session: PRD-aware, high-authority validator that steers the main assistant toward safe, minimal-churn completion.

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell;

use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;

use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;

use serde::Deserialize;

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

// Stylize is imported locally where needed to keep scope tight.
use tokio::time::Duration;
use tokio::time::timeout;
#[cfg(feature = "patchgate")]
use crate::git_guard::ChangeContract;
#[cfg(feature = "patchgate")]
use crate::patch_gate_integration::run_patch_gate_for_builder_output;

/// Per-event wait to receive something from the reviewer model before declaring a timeout.
const REVIEWER_EVENT_TIMEOUT_SECS: u64 = 120;

// No hard cap on events: rely on per-event timeout as safety valve.

/// Reviewer decision envelope expected from the Reviewer agent (parsed from its first JSON output).
#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub(crate) struct ReviewerDecision {
    /// follow | adjust | abort
    decision: Option<String>,
    /// Confidence in [0.0, 1.0]
    confidence: Option<f32>,
    /// Material risks (not nits).
    #[serde(default)]
    risks: Vec<String>,
    /// Next actions (smallest viable steps).
    #[serde(default)]
    next_actions: Vec<String>,
    /// Short reason in one sentence.
    reason: Option<String>,
    // Optional extras
    chosen_approach: Option<String>,
    milestone: Option<String>,
    acceptance_criteria: Option<Vec<String>>,
}

/// Spawn a PRD‑aware Reviewer session and evaluate the builder output.
///
/// Behavior:
/// - Boots a minimal-capability conversation (no tools) targeting the reviewer system prompt.
/// - Submits the reviewer bundle (PRD, prompt, assistant output, plan, diff, meta).
/// - Waits for TaskComplete; tolerates timeouts and reports them into the TUI history.
/// - Parses the first JSON object (robust to code fences / multi-line JSON).
/// - Prints a compact decision badge + brief specialist reply in the TUI.
/// - Optional autopilot: if `autopilot_on_follow` and decision == "follow", instructs main session to proceed.
///
/// Notes:
/// - We intentionally disable patch/apply/view tools and web search for the reviewer.
/// - We keep model effort/verbosity low to reduce cost while maintaining decisiveness.
pub(crate) async fn run_reviewer_session(
    server: Arc<ConversationManager>,
    base_config: &Config,
    bundle_text: String,
    app_event_tx: AppEventSender,
    main_session_id: Option<uuid::Uuid>,
    autopilot_on_follow: bool,
) {
    // Configure a lean reviewer session.
    let mut cfg = base_config.clone();
    cfg.include_plan_tool = false;
    cfg.include_apply_patch_tool = false;
    cfg.include_view_image_tool = false;
    cfg.tools_web_search_request = false;

    // Reviewer should be decisive and inexpensive.
    if let Some(fam) = find_family_for_model(&cfg.model) {
        cfg.model_family = fam;
    }
    // Respect the caller-provided effort; default summary to Detailed to
    // align with provider requirements for GPT-5 family.
    cfg.model_reasoning_summary = ReasoningSummary::Detailed;
    cfg.model_verbosity = Some(Verbosity::Low);
    // Do not override base instructions — some providers reject long custom instructions.
    // Instead, inject the reviewer preamble as <user_instructions> in the input payload.
    cfg.base_instructions = None;

    // Start conversation.
    let (conv, conv_id) = match server.new_conversation(cfg).await {
        Ok(newc) => (newc.conversation, newc.conversation_id),
        Err(e) => {
            let d = ReviewerDecision {
                decision: Some("abort".to_string()),
                reason: Some(format!("start failed: {e}")),
                ..Default::default()
            };
            let line = compact_decision_line(&d);
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_note(vec![line]),
            )));
            return;
        }
    };

    // Compose payload (context bundle only; instructions set as base_instructions).
    let user_payload = bundle_text.clone();

    // Show a compact banner so users can confirm the Reviewer actually started.
    {
        use ratatui::style::Stylize as _;
        let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
        let emoji = "🔎\u{200A}"; // magnifying glass + hair space
        let title_spans: Vec<ratatui::text::Span<'static>> = vec![
            emoji.into(),
            "Reviewer".bold().cyan(),
            " ".into(),
            "running…".dim(),
        ];
        let meta = format!(
            "model={}, effort={}",
            base_config.model, base_config.model_reasoning_effort
        )
        .dim();
        lines.push(ratatui::text::Line::from(title_spans));
        lines.push(ratatui::text::Line::from(vec!["  ".dim(), meta]));
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_info_note(lines),
        )));
    }

    // Prepend reviewer preamble as <user_instructions> so it is treated as high-priority guidance
    // without using the `instructions` field in the request.
    let preamble = format!(
        "<user_instructions>\n\n{}\n\n</user_instructions>",
        reviewer_system_preamble()
    );

    if let Err(e) = conv
        .submit(Op::UserInput {
            items: vec![
                codex_core::protocol::InputItem::Text { text: preamble },
                codex_core::protocol::InputItem::Text { text: user_payload },
            ],
        })
        .await
    {
        let d = ReviewerDecision {
            decision: Some("abort".to_string()),
            reason: Some(format!("submission failed: {e}")),
            ..Default::default()
        };
        let line = compact_decision_line(&d);
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            history_cell::new_info_note(vec![line]),
        )));
        let _ = server.remove_conversation(conv_id).await;
        return;
    }

    // Await completion with safety valves.
    let per_event = {
        if let Ok(v) = env::var("CODEX_REVIEWER_TIMEOUT_SECS") {
            v.parse::<u64>().unwrap_or(REVIEWER_EVENT_TIMEOUT_SECS)
        } else {
            REVIEWER_EVENT_TIMEOUT_SECS
        }
    };
    let per_event = Duration::from_secs(per_event);
    let mut _events_seen = 0usize; // retained for diagnostics only
    let mut raw_text: Option<String> = None;
    // Accumulate any AgentMessage/Delta content so we can fall back if
    // TaskComplete does not include `last_agent_message`.
    let mut collected_text = String::new();
    // Accumulate decision/reply here to avoid post-loop parsing warnings.
    let mut decision = ReviewerDecision::default();
    let mut reply: String = String::new();
    let mut error_messages: Vec<String> = Vec::new();

    loop {
        _events_seen += 1;

        match timeout(per_event, conv.next_event()).await {
            Ok(Ok(event)) => {
                match event.msg {
                    EventMsg::AgentMessageDelta(delta) => {
                        collected_text.push_str(&delta.delta);
                    }
                    EventMsg::AgentMessage(msg) => {
                        raw_text = Some(msg.message.clone());
                        collected_text.clear();
                        collected_text.push_str(&msg.message);
                    }
                    EventMsg::Error(err) => {
                        error_messages.push(err.message);
                    }
                    EventMsg::TaskComplete(ev) => {
                        if let Some(text) = ev.last_agent_message {
                            raw_text = Some(text.clone());
                            let (d, r) = parse_reviewer_output(&text);
                            decision = d;
                            reply = r;
                        } else {
                            // Fallback to any collected text from streaming events.
                            if !collected_text.is_empty() {
                                raw_text = Some(collected_text.clone());
                                let (d, r) = parse_reviewer_output(&collected_text);
                                decision = d;
                                reply = r;
                            } else if !error_messages.is_empty() {
                                // No text at all; if we have errors, surface the first one as reply context.
                                let msg = error_messages.join(" | ");
                                raw_text = Some(String::new());
                                decision = ReviewerDecision::default();
                                reply = format!("Reviewer error: {msg}");
                            }
                        }
                        break;
                    }
                    _ => {}
                }
            }
            Ok(Err(e)) => {
                let d = ReviewerDecision {
                    decision: Some("abort".to_string()),
                    reason: Some(format!("event error: {e}")),
                    ..Default::default()
                };
                let line = compact_decision_line(&d);
                app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_info_note(vec![line]),
                )));
                let _ = server.remove_conversation(conv_id).await;
                return;
            }
            Err(_) => {
                let d = ReviewerDecision {
                    decision: Some("abort".to_string()),
                    reason: Some(format!("timeout waiting >{REVIEWER_EVENT_TIMEOUT_SECS}s")),
                    ..Default::default()
                };
                let line = compact_decision_line(&d);
                app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_info_note(vec![line]),
                )));
                let _ = server.remove_conversation(conv_id).await;
                return;
            }
        }
    }

    // decision/reply already parsed in-loop.

    // Insert compact decision + reply into history.
    let decision_line = compact_decision_line(&decision);
    let mut lines: Vec<ratatui::text::Line<'static>> = vec![decision_line];
    if !reply.trim().is_empty() {
        use ratatui::style::Stylize as _;
        lines.push("".into());
        // Prefix each reply line with a subtle [reviewer] tag for clarity in transcript.
        for l in reply.lines() {
            let spans: Vec<ratatui::text::Span<'static>> =
                vec!["[reviewer]".dim(), " ".into(), l.to_string().into()];
            lines.push(ratatui::text::Line::from(spans));
        }
    }
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        history_cell::new_info_note(lines),
    )));

    // Optional debug overlay on parse failure: show raw reviewer output to help diagnose UNKNOWN.
    if normalize_decision(decision.decision.as_deref()) == "unknown" {
        let dbg = env::var("CODEX_REVIEWER_DEBUG").unwrap_or_default();
        // Always write a debug file to tmp on parse failure.
        let raw_owned = raw_text.clone().unwrap_or_else(|| collected_text.clone());
        let dir = env::var("CODEX_REVIEWER_DEBUG_DIR").unwrap_or("/tmp".to_string());
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = format!(
            "{}/codex-reviewer-debug-{}-{}.txt",
            dir,
            ts,
            std::process::id()
        );

        let mut file_body = String::new();
        file_body.push_str("Reviewer parse error: could not extract JSON decision header.\n\n");
        if !error_messages.is_empty() {
            file_body.push_str("Error events (from core):\n");
            for e in &error_messages {
                file_body.push_str("- ");
                file_body.push_str(e);
                file_body.push('\n');
            }
            file_body.push('\n');
        }
        if !raw_owned.is_empty() {
            if let Some((s, e)) = extract_first_json_object_span(&raw_owned) {
                match serde_json::from_str::<ReviewerDecision>(&raw_owned[s..e]) {
                    Ok(_) => {
                        file_body.push_str(
                            "Candidate JSON parsed but decision empty or missing.\n\n",
                        );
                    }
                    Err(e) => {
                        file_body.push_str(&format!("serde_json error: {e}\n\n"));
                    }
                }
                file_body.push_str("Candidate JSON span: ");
                file_body.push_str(&format!("{s}..{e}\n\n"));
                file_body.push_str("Candidate JSON:\n");
                file_body.push_str(&raw_owned[s..e]);
                file_body.push_str("\n\n");
            } else {
                file_body.push_str("No JSON-like object found (balanced braces) in output.\n\n");
            }
            file_body.push_str("Raw reviewer output (truncated):\n\n");
            file_body.push_str(&truncate_middle_utf8(&raw_owned, 8 * 1024));
        } else {
            file_body.push_str("No reviewer output received (empty).\n");
        }
        let _ = fs::write(&path, file_body);

        use ratatui::style::Stylize as _;
        let note: Vec<ratatui::text::Line<'static>> = vec![
            "[reviewer]".dim().into(),
            format!("Saved reviewer debug to {path}").into(),
        ];
        if dbg == "1" {
            // Also show an overlay for convenience when explicitly requested.
            let mut debug_text = String::new();
            debug_text.push_str(
                "Reviewer parse error: could not extract JSON decision header.\n\n",
            );
            if !error_messages.is_empty() {
                debug_text.push_str("Error events (from core):\n");
                for e in &error_messages {
                    debug_text.push_str("- ");
                    debug_text.push_str(e);
                    debug_text.push('\n');
                }
                debug_text.push('\n');
            }
            if !raw_owned.is_empty() {
                if let Some((s, e)) = extract_first_json_object_span(&raw_owned) {
                    if let Err(e) = serde_json::from_str::<ReviewerDecision>(&raw_owned[s..e]) {
                        debug_text.push_str(&format!("serde_json error: {e}\n\n"));
                    }
                } else {
                    debug_text.push_str(
                        "No JSON-like object found (balanced braces) in output.\n\n",
                    );
                }
                debug_text.push_str("Raw reviewer output (truncated):\n\n");
                debug_text.push_str(&truncate_middle_utf8(&raw_owned, 8 * 1024));
            } else {
                debug_text.push_str("No reviewer output received (empty).\n");
            }
            app_event_tx.send(AppEvent::ShowTextOverlay {
                title: "Reviewer raw output (debug)".to_string(),
                text: debug_text,
            });
        } else {
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_note(note),
            )));
        }
    }

    // Autopilot: forward a single sanitized payload on FOLLOW or ADJUST (normalized)
    let norm = normalize_decision(decision.decision.as_deref());
    if autopilot_on_follow
        && (norm == "follow" || norm == "adjust")
        && let Some(sess) = main_session_id
        && let Ok(conv_main) = server.get_conversation(sess).await
    {
        let forward_text = build_forward_payload(norm, &decision, &reply);
        // Final safety gate: if we still detect a decision-like artifact, avoid forwarding.
        if contains_decision_artifact(&forward_text) {
            use ratatui::style::Stylize as _;
            let lines: Vec<ratatui::text::Line<'static>> = vec![
                "[reviewer]".dim().into(),
                "Forward blocked: decision header detected in payload; not forwarding to Builder.".into(),
            ];
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_note(lines),
            )));
        } else {
            let _ = conv_main
                .submit(Op::UserInput {
                    items: vec![codex_core::protocol::InputItem::Text { text: forward_text }],
                })
                .await;
            // Insert a small confirmation banner so users see that autopilot
            // injected next actions into the Builder’s input.
            use ratatui::style::Stylize as _;
            let mut lines: Vec<ratatui::text::Line<'static>> = Vec::new();
            lines.push(ratatui::text::Line::from(vec![
                "⚙\u{200A}".into(),
                "Autopilot".bold().cyan(),
                " → sent Reviewer guidance to Builder".into(),
            ]));
            let preview = decision
                .next_actions
                .iter()
                .take(3)
                .enumerate()
                .map(|(i, a)| format!("  {}. {}", i + 1, a))
                .collect::<Vec<_>>()
                .join("\n");
            if !preview.is_empty() {
                for l in preview.lines() {
                    lines.push(ratatui::text::Line::from(l.to_string()));
                }
            }
            app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_info_note(lines),
            )));
        }
    }

    // Optionally run PatchGate immediately after a follow/adjust verdict.
    #[cfg(feature = "patchgate")]
    {
        let decision_mode = normalize_decision(decision.decision.as_deref());
        if matches!(decision_mode, "follow" | "adjust")
            && crate::autopilot_prefs::patchgate_enabled()
        {
            // Accept a contract via env for minimal, non-invasive wiring.
            // If missing or invalid, skip PatchGate silently.
            if let Ok(contract_json) = std::env::var("CODEX_PATCHGATE_CONTRACT_JSON")
                && !contract_json.trim().is_empty()
                && let Ok(contract) = serde_json::from_str::<ChangeContract>(&contract_json)
            {
                        let repo_path: &Path = &base_config.cwd;
                        let commit_subject = decision
                            .reason
                            .clone()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| "Autopilot commit".to_string());
                        // Run PatchGate against the bundle text; it will parse the diff envelope if present.
                        let _ = run_patch_gate_for_builder_output(
                            app_event_tx.clone(),
                            repo_path,
                            &bundle_text,
                            &contract,
                            &commit_subject,
                            /* check_only = */ false,
                            Option::<fn(&Path) -> color_eyre::eyre::Result<()>>::None,
                        )
                        .await;
            }
        }
    }

    // Cleanup reviewer conversation from memory (rollout persists on disk).
    let _ = server.remove_conversation(conv_id).await;
}

/// High-authority, ROI-driven system preamble for the reviewer.
/// Tone: decisive and brief. The reviewer never edits files nor runs commands.
fn reviewer_system_preamble() -> String {
    // Use a raw string to keep formatting readable and maintainable.
    r#"You are the Principal Reviewer & Project Director for an ongoing, multi‑step build. The Builder will often present partial implementations, trade‑off questions, or multiple approaches. Your job is not only to assess but to CHOOSE the best path and set concrete next steps to finish the project with minimal churn.

Operating context:
• Continuous iteration: expect successive partial deliveries and evolving context.
• You choose the direction; do not defer decisions unless the block is safety‑critical.
• Prefer “state assumption + proceed” over “ask + stall.” Make assumptions explicit.
• You NEVER edit files or run commands; you assess and direct.

Decision modes:
• follow — ship current work and advance; issue directives for the next micro‑milestone.
• adjust — require the smallest viable changes, then advance; still provide directives.
• abort — stop & escalate only for severe risk (security, data loss, legal, SLO breach).

Decision JSON (first line, single line; DO NOT repeat/quote it anywhere else):
{"decision":"follow|adjust|abort","confidence":0..1,"risks":[string],"next_actions":[string],"reason":string,"chosen_approach":string?,"milestone":string?,"acceptance_criteria":[string]?}

Specialist reply (≤250 words), structure:
- **Direction chosen**: state the selected approach (A/B/C) and why (3 concise trade‑offs).
- **Next micro‑milestone**: 1–3 numbered, atomic steps phrased as Builder directives (include file paths and tests).
- **Acceptance criteria**: checklist the Builder can verify (behavior, tests, perf/SLOs).
- **Risks & watchouts**: ≤3 items with mitigations.

Checklist to think through (be brief):
• PRD update: was the completed step reflected in PRD.md? Does the diff confirm the change?
PRD compliance • Correctness/edge cases • Safety/security • Performance/SLOs • Backward compatibility • Test coverage • Operability

Language: English. Be decisive and verdict‑first. Do not ask for permission unless blocked; commit to a path and provide directives the Builder can execute next.
."#
        .to_string()
}

/// Render a compact badge line with the core decision, confidence, and reason.
fn compact_decision_line(d: &ReviewerDecision) -> ratatui::text::Line<'static> {
    use ratatui::style::Stylize as _;
    let mut parts: Vec<ratatui::text::Span<'static>> = Vec::new();
    parts.push("Reviewer:".bold());
    parts.push(" ".into());
    let status = normalize_decision(d.decision.as_deref());
    let badge = match status {
        "follow" => "FOLLOW".green().bold(),
        "adjust" => "ADJUST".cyan().bold(),
        "abort" => "ABORT".red().bold(),
        _ => "UNKNOWN".dim(),
    };
    parts.push(badge);
    if let Some(c) = d.confidence {
        parts.push("  ".into());
        parts.push(format!("{c:.2}").dim());
    }
    if let Some(r) = d.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        parts.push("  ".into());
        parts.push(r.to_string().dim());
    }
    parts.into()
}

/// Map free-form decision strings to a stable tri-state for UI/autopilot.
fn normalize_decision(d: Option<&str>) -> &'static str {
    let Some(raw) = d else { return "unknown" };
    let s = raw.trim().to_ascii_lowercase();
    match s.as_str() {
        "follow" | "approve" | "continue" | "proceed" => "follow",
        "adjust" | "revise" | "fix" => "adjust",
        "abort" | "stop" | "halt" | "reject" => "abort",
        _ => {
            // Heuristic fallback: any non-empty decision becomes "adjust" rather than "unknown".
            if !s.is_empty() { "adjust" } else { "unknown" }
        }
    }
}

/// Construct the message that will be forwarded to the builder session.
/// Includes a compact header and the reviewer reply or a decision-specific fallback.
fn build_forward_payload(norm: &str, d: &ReviewerDecision, reply: &str) -> String {
    let status = match norm {
        "follow" => "FOLLOW",
        "adjust" => "ADJUST",
        "abort" => "ABORT",
        _ => "UNKNOWN",
    };
    let mut header = if let Some(c) = d.confidence {
        format!("Reviewer → Decision: {status} ({c:.2})")
    } else {
        format!("Reviewer → Decision: {status}")
    };
    if let Some(reason) = d.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        header.push_str("\nWhy: ");
        header.push_str(reason);
    }
    if let Some(m) = d.milestone.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        header.push_str("\nMilestone: ");
        header.push_str(m);
    }
    if let Some(chosen) = d
        .chosen_approach
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        header.push_str("\nChosen approach: ");
        header.push_str(chosen);
    }

    // Build body from reply + structured fields.
    let mut body = String::new();
    if !reply.trim().is_empty() {
        body.push_str(reply.trim());
        body.push('\n');
    }
    if !d.next_actions.is_empty() {
        body.push_str("\nNext actions (from reviewer):\n");
        for (i, a) in d.next_actions.iter().enumerate() {
            body.push_str(&format!("{}. {}\n", i + 1, a.trim()));
        }
    }
    if let Some(ac) = d.acceptance_criteria.as_ref().filter(|v| !v.is_empty()) {
        body.push_str("\nAcceptance criteria:\n");
        for (i, a) in ac.iter().enumerate() {
            body.push_str(&format!("AC{}. {}\n", i + 1, a.trim()));
        }
    }
    if !d.risks.is_empty() {
        body.push_str("\nRisks:\n");
        for (i, r) in d.risks.iter().enumerate() {
            body.push_str(&format!("- R{}: {}\n", i + 1, r.trim()));
        }
    }

    // Safety nets, in order of precision to breadth:
    // 1) Remove fenced ```json ...``` blocks entirely.
    body = strip_json_fenced_blocks(&body);
    // 2) Strip decision-like JSON headers (balanced braces that parse as ReviewerDecision).
    body = strip_all_decision_json_headers(&body);
    // 3) Remove any lines that still carry decision-like artifacts.
    body = strip_decision_lines(&body);

    // Remove "Decision gate" prompts; the Reviewer gate is handled by autopilot already.
    body = body
        .lines()
        .filter(|l| {
            !l.trim_start()
                .to_ascii_lowercase()
                .starts_with("decision gate:")
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{header}\n\n{body}")
}

/// Remove a single JSON object that looks like a Reviewer decision header from the text.
/// This avoids leaking the `{ "decision": ... }` header into the Builder context, while
/// preserving other code blocks that may contain braces.
fn strip_decision_json_header(s: &str) -> String {
    let text = s.to_string();
    if let Some((start, end)) = extract_first_json_object_span(&text) {
        // Only strip if this object parses as a ReviewerDecision with a non-empty decision.
        if let Ok(parsed) = serde_json::from_str::<ReviewerDecision>(&text[start..end])
            && parsed
                .decision
                .as_deref()
                .map(|d| !d.trim().is_empty())
                .unwrap_or(false)
        {
            let before = text[..start].trim_end();
            let after = text[end..].trim_start();
            let mut out = String::new();
            if !before.is_empty() {
                out.push_str(before);
                out.push('\n');
            }
            out.push_str(after);
            return out;
        }
    }
    s.to_string()
}

/// Repeatedly remove any JSON object that looks like a Reviewer decision header.
fn strip_all_decision_json_headers(s: &str) -> String {
    let mut text = s.to_string();
    loop {
        let before = text.clone();
        text = strip_decision_json_header(&text);
        if text == before {
            break;
        }
    }
    text
}

/// Remove any fenced code block labeled as json.
fn strip_json_fenced_blocks(s: &str) -> String {
    let mut out = String::new();
    let mut lines = s.lines();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        if t.starts_with("```") {
            let tag = t
                .trim_start_matches('`')
                .trim_start_matches('`')
                .trim_start_matches('`')
                .trim();
            let tag_lc = tag.to_ascii_lowercase();
            // Skip this fence block if it's json-like (json/jsonc) or unlabeled JSON code
            if tag_lc.starts_with("json") {
                // consume until next ```
                for l2 in lines.by_ref() {
                    if l2.trim_start().starts_with("```") {
                        break;
                    }
                }
                continue;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

/// Remove lines that look like decision artifacts (e.g., a lone JSON line with "decision").
fn strip_decision_lines(s: &str) -> String {
    s.lines()
        .filter(|l| {
            let t = l.trim();
            // Drop quoted or plain lines that appear to be the decision header
            let looks_json_line =
                (t.starts_with('{') || t.starts_with('>')) && t.contains("\"decision\"");
            !looks_json_line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Heuristic detection if text still contains a reviewer decision artifact.
fn contains_decision_artifact(s: &str) -> bool {
    if let Some((start, end)) = extract_first_json_object_span(s)
        && let Ok(parsed) = serde_json::from_str::<ReviewerDecision>(&s[start..end])
        && parsed
            .decision
            .as_deref()
            .map(|d| !d.trim().is_empty())
            .unwrap_or(false)
    {
        return true;
    }
    // Also detect per-line residue
    s.lines()
        .any(|l| l.trim_start().starts_with('>') && l.contains("\"decision\""))
}

/// Parse reviewer output into (decision, reply).
/// The parser is robust to:
/// - Single-line JSON (preferred by the prompt).
/// - JSON inside fenced code blocks ```json ... ```
/// - Multi-line JSON objects.
///
/// Heuristic:
/// 1) Try fenced code blocks labeled json or unlabeled.
/// 2) Fallback to a balanced-brace scan that ignores braces inside JSON strings.
/// 3) If not found, return default decision and treat the full text as reply.
// Public parse entry used by the TUI reviewer flow. Extracts a structured ReviewerDecision
// and the specialist reply. Accepts JSON in json/jsonc fences or a bare JSON object;
// otherwise returns default decision and treats the full text as the reply.
pub(crate) fn parse_reviewer_output(text: &str) -> (ReviewerDecision, String) {
    if let Some((s, e)) = extract_first_json_object_span(text)
        && let Ok(d) = serde_json::from_str::<ReviewerDecision>(&text[s..e])
    {
        let reply = text
            .get(e..)
            .map(|r| r.trim_start())
            .unwrap_or("")
            .to_string();
        return (d, reply);
    }

    // Legacy fallback: if the reviewer actually emitted a single-line JSON without fences,
    // capture the first line that resembles a JSON object and parse it.
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}')
            && let Ok(d) = serde_json::from_str::<ReviewerDecision>(trimmed)
        {
            let after = text.split_once(line).map(|(_, rest)| rest.trim());
            let reply = after.unwrap_or("").to_string();
            return (d, reply);
        }
    }

    (ReviewerDecision::default(), text.to_string())
}

/// Try to locate the first JSON object in `text` and return its byte span (start, end-exclusive).
///
/// Strategy:
/// - Prefer fenced blocks ```json ... ```; if present and valid JSON, return that span.
/// - Otherwise, run a balanced-brace scanner that ignores braces inside quoted strings.
fn extract_first_json_object_span(text: &str) -> Option<(usize, usize)> {
    // 1) Search fenced blocks: ```json ... ``` or ``` ... ```
    let mut i = 0usize;
    while let Some(start_rel) = text[i..].find("```") {
        let fence_start = i + start_rel;
        let after_fence = &text[fence_start + 3..];

        // Optional language tag until newline.
        let newline_idx = after_fence.find('\n').unwrap_or(after_fence.len());
        let tag = after_fence[..newline_idx].trim().to_ascii_lowercase();

        // Code starts after the optional tag + newline.
        let code_start = fence_start
            + 3
            + newline_idx
            + if newline_idx < after_fence.len() {
                1
            } else {
                0
            };

        if let Some(end_rel) = text[code_start..].find("```") {
            let fence_end = code_start + end_rel;
            let code = text[code_start..fence_end].trim();

            // Accept either json-tagged (json/jsonc/JSON variants) or untagged if looks like an object.
            let looks_like_json = code.starts_with('{') && code.ends_with('}');
            if (tag.is_empty() || tag.starts_with("json"))
                && looks_like_json
                && serde_json::from_str::<serde_json::Value>(code).is_ok()
            {
                return Some((code_start, fence_end));
            }
            i = fence_end + 3;
        } else {
            break;
        }
    }

    // 2) Balanced-brace scan ignoring content inside JSON strings.
    let mut in_str = false;
    let mut escape = false;
    let mut depth = 0usize;
    let mut start_idx: Option<usize> = None;

    for (idx, ch) in text.char_indices() {
        if in_str {
            if escape {
                // Current char is escaped; consume it and clear escape.
                escape = false;
                continue;
            }
            if ch == '\\' {
                // Next character is escaped.
                escape = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }

        match ch {
            '"' => in_str = true,
            '{' => {
                if depth == 0 {
                    start_idx = Some(idx);
                }
                depth = depth.saturating_add(1);
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let s = start_idx?;
                    let e = idx + ch.len_utf8();
                    let candidate = &text[s..e];
                    if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                        return Some((s, e));
                    }
                }
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_decision_variants() {
        assert_eq!(normalize_decision(Some("follow")), "follow");
        assert_eq!(normalize_decision(Some("Approve")), "follow");
        assert_eq!(normalize_decision(Some("ADJUST")), "adjust");
        assert_eq!(normalize_decision(Some("fix")), "adjust");
        assert_eq!(normalize_decision(Some("abort")), "abort");
        assert_eq!(normalize_decision(Some("reject")), "abort");
        assert_eq!(normalize_decision(Some("")), "unknown");
        assert_eq!(normalize_decision(None), "unknown");
    }

    #[test]
    fn strip_json_fenced_blocks_removes_json_and_jsonc() {
        let input = r#"
Intro
```json
{"decision":"follow","confidence":0.8}
```

```jsonc
// comment
{"decision":"adjust"}
```

```bash
echo "keep me"
```

Outro
"#;
        let out = strip_json_fenced_blocks(input);
        assert!(out.contains("Intro"));
        assert!(out.contains("```bash"));
        assert!(out.contains("echo \"keep me\""));
        assert!(out.contains("Outro"));
        assert!(!out.contains("\"decision\""));
    }

    #[test]
    fn parse_reviewer_output_from_fenced_json_uppercase() {
        let input = r#"```JSON
{"decision":"adjust","confidence":0.5,"next_actions":["do X"],"reason":"tighten impl"}
```
Specialist reply here."#;
        let (d, r) = parse_reviewer_output(input);
        assert_eq!(normalize_decision(d.decision.as_deref()), "adjust");
        assert_eq!(d.next_actions.len(), 1);
        assert!(r.contains("Specialist reply"));
    }

    #[test]
    fn parse_reviewer_output_from_jsonc_fence() {
        let input = r#"```jsonc
// comment allowed
{"decision":"follow","confidence":0.88}
```
Tail reply."#;
        let (d, r) = parse_reviewer_output(input);
        assert_eq!(normalize_decision(d.decision.as_deref()), "follow");
        assert!(r.contains("Tail reply"));
    }

    #[test]
    fn parse_reviewer_output_balanced_brace_scan() {
        // JSON object not in a fence; extra text around it.
        let input = "Intro text before {\"decision\":\"adjust\",\"confidence\":0.42} and after.";
        let (d, r) = parse_reviewer_output(input);
        assert_eq!(normalize_decision(d.decision.as_deref()), "adjust");
        assert!(r.contains("and after"));
    }

    #[test]
    fn extract_json_object_span_from_unlabeled_fence() {
        let input = r#"```
{"decision":"follow","confidence":0.9}
```

rest"#;
        let span = extract_first_json_object_span(input);
        assert!(span.is_some());
    }

    #[test]
    fn build_forward_payload_includes_actions_and_criteria_and_sanitizes() {
        let d = ReviewerDecision {
            decision: Some("follow".to_string()),
            confidence: Some(0.77),
            risks: vec!["edge case".into()],
            next_actions: vec!["apply patch".into(), "run tests".into()],
            reason: Some("looks good".into()),
            chosen_approach: Some("A".into()),
            milestone: Some("M1".into()),
            acceptance_criteria: Some(vec!["tests pass".into(), "no perf regressions".into()]),
        };
        let reply = r#"```json
{"decision":"follow"}
```
Reply text"#;
        let payload = build_forward_payload("follow", &d, reply);
        assert!(payload.contains("Reviewer → Decision: FOLLOW (0.77)"));
        assert!(payload.contains("Why: looks good"));
        assert!(payload.contains("Next actions"));
        assert!(payload.contains("Acceptance criteria"));
        // Make sure sanitized text does not leak the JSON header
        assert!(!payload.contains("\"decision\""));
    }

    #[test]
    fn contains_decision_artifact_detects_header() {
        let s = r#"{"decision":"follow","confidence":0.7}"#;
        assert!(contains_decision_artifact(s));
    }
}

/// Build the reviewer bundle text from the supplied components.
/// Truncation is UTF‑8 safe and inserts a visible elision marker when needed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_reviewer_bundle(
    prd_path: &Path,
    prd_budget_bytes: usize,
    prd_mode: PrdMode,
    prd_meta: Option<&str>,
    user_prompt: Option<&str>,
    assistant_output: Option<&str>,
    plan_text: Option<&str>,
    diff_text: Option<&str>,
    model: &str,
    effort: &str,
    cwd: &Path,
    cli_version: &str,
) -> String {
    let mut out = String::new();
    // Include PRD based on the selected mode.
    if prd_path.exists() {
        match prd_mode {
            PrdMode::Full => {
                let prd = read_truncated_utf8(prd_path, prd_budget_bytes);
                out.push_str("<prd>\n");
                out.push_str(&prd);
                out.push_str("\n</prd>\n\n");
            }
            PrdMode::TasksOnly => {
                let prd_full = read_truncated_utf8(prd_path, prd_budget_bytes);
                let tasks = extract_tasks_snapshot(&prd_full);
                out.push_str("<prd_tasks>\n");
                out.push_str(&tasks);
                out.push_str("\n</prd_tasks>\n\n");
            }
            PrdMode::Omit => {}
        }
        if let Some(meta) = prd_meta {
            out.push_str("<prd_meta>\n");
            out.push_str(meta);
            out.push_str("\n</prd_meta>\n\n");
        }
    }

    if let Some(p) = user_prompt {
        out.push_str("<prompt_now>\n");
        out.push_str(p);
        out.push_str("\n</prompt_now>\n\n");
    }
    if let Some(a) = assistant_output {
        out.push_str("<assistant_output>\n");
        out.push_str(a);
        out.push_str("\n</assistant_output>\n\n");
    }
    if let Some(pl) = plan_text {
        out.push_str("<plan>\n");
        out.push_str(pl);
        out.push_str("\n</plan>\n\n");
    }
    if let Some(df) = diff_text {
        out.push_str("<diff_summary>\n");
        out.push_str(&truncate_middle_utf8(df, 16 * 1024));
        out.push_str("\n</diff_summary>\n\n");
    }

    out.push_str("<meta>\n");
    out.push_str(&format!(
        "model={model}, effort={effort}, cwd={}, cli={cli_version}\n",
        cwd.display()
    ));
    out.push_str("</meta>\n");
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrdMode {
    Full,
    TasksOnly,
    Omit,
}

/// Read a file as String and UTF‑8‑safely truncate it in the middle if it exceeds `max_bytes`.
fn read_truncated_utf8(path: &Path, max_bytes: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => truncate_middle_utf8(&s, max_bytes),
        Err(_) => "(PRD not found)".to_string(),
    }
}

/// Extract a lightweight snapshot of tasks from a PRD document.
/// Heuristics:
/// - Prefer section starting at a header containing "Tasks" (case-insensitive)
///   and capture until the next header.
/// - Fallback: collect bullet lines ("- ", "* ") and checkbox lines ("- [ ]", "- [x]").
fn extract_tasks_snapshot(prd: &str) -> String {
    let lines = prd.lines().collect::<Vec<_>>();
    // 1) Try to find a header containing "Tasks" and capture until next header
    let mut start: Option<usize> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if (t.starts_with("##") || t.starts_with('#')) && t.to_ascii_lowercase().contains("tasks")
        {
            start = Some(i + 1);
            break;
        }
    }
    if let Some(s) = start {
        let mut end = lines.len();
        for (i, _) in lines.iter().enumerate().skip(s) {
            let t = lines[i].trim();
            if (t.starts_with("##") || t.starts_with('#')) && !t.to_ascii_lowercase().contains("tasks")
            {
                end = i;
                break;
            }
        }
        let slice = &lines[s..end];
        let trimmed = slice.to_vec().join("\n");
        let out = trimmed.trim();
        if !out.is_empty() {
            return out.to_string();
        }
    }
    // 2) Fallback: collect bullets and checkboxes
    let mut acc = String::new();
    for l in lines {
        let t = l.trim_start();
        if t.starts_with("- [") || t.starts_with("- ") || t.starts_with("* ") {
            acc.push_str(l);
            acc.push('\n');
        }
    }
    if acc.trim().is_empty() {
        "(no tasks detected)".to_string()
    } else {
        acc.trim_end().to_string()
    }
}

/// UTF‑8‑safe middle truncation with a visible elision marker.
/// Ensures we never cut through a UTF‑8 code point.
fn truncate_middle_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // For tiny budgets, return as much prefix as possible.
    if max_bytes <= 32 {
        return safe_prefix_bytes(s, max_bytes).to_string();
    }

    let marker = "\n[... omitted ...]\n";
    let marker_bytes = marker.len();

    // Split budget roughly in half while accounting for the marker.
    let head_budget = (max_bytes.saturating_sub(marker_bytes)) / 2;
    let head = safe_prefix_bytes(s, head_budget);
    // Recompute tail budget using the actual head length (may be < head_budget due to UTF‑8).
    let tail_budget = max_bytes.saturating_sub(marker_bytes + head.len());
    let tail = safe_suffix_bytes(s, tail_budget);
    format!("{head}{marker}{tail}")
}

/// Return a UTF‑8‑aligned prefix (up to `budget` bytes).
fn safe_prefix_bytes(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }
    // Find the last char boundary <= budget.
    let mut end = 0usize;
    for (idx, _) in s.char_indices() {
        if idx <= budget {
            end = idx;
        } else {
            break;
        }
    }
    &s[..end]
}

/// Return a UTF‑8‑aligned suffix (up to `budget` bytes).
fn safe_suffix_bytes(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }
    // Find the earliest char boundary from which the tail fits in `budget` bytes.
    let mut start = s.len();
    for (idx, _) in s.char_indices().rev() {
        if s.len().saturating_sub(idx) <= budget {
            start = idx;
        } else {
            break;
        }
    }
    &s[start..]
}
