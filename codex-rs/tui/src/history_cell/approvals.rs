//! Approval, denial, and review-status transcript cells.

use super::*;

fn truncate_exec_snippet(full_cmd: &str) -> String {
    let mut snippet = match full_cmd.split_once('\n') {
        Some((first, _)) => format!("{first} ..."),
        None => full_cmd.to_string(),
    };
    snippet = truncate_text(&snippet, /*max_graphemes*/ 80);
    snippet
}

fn exec_snippet(command: &[String]) -> String {
    let full_cmd = strip_bash_lc_and_escape(command);
    truncate_exec_snippet(&full_cmd)
}

fn non_empty_exec_snippet(command: &[String]) -> Option<String> {
    let snippet = exec_snippet(command);
    (!snippet.is_empty()).then_some(snippet)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewDecision {
    Approved,
    ApprovedExecpolicyAmendment {
        proposed_execpolicy_amendment: ExecPolicyAmendment,
    },
    ApprovedForSession,
    NetworkPolicyAmendment {
        network_policy_amendment: NetworkPolicyAmendment,
    },
    Denied,
    TimedOut,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ApprovalDecisionSubject {
    Command(Vec<String>),
    NetworkAccess { target: String },
}

pub fn new_approval_decision_cell(
    subject: ApprovalDecisionSubject,
    decision: ReviewDecision,
    actor: ApprovalDecisionActor,
) -> Box<dyn HistoryCell> {
    use ReviewDecision::*;
    use codex_protocol::approvals::NetworkPolicyRuleAction;

    let (symbol, summary): (Span<'static>, Vec<Span<'static>>) = match decision {
        Approved => match subject {
            ApprovalDecisionSubject::Command(command) => {
                let summary = if let Some(snippet) = non_empty_exec_snippet(&command) {
                    vec![
                        actor.subject().into(),
                        "approved".bold(),
                        " codex to run ".into(),
                        Span::from(snippet).dim(),
                        " this time".bold(),
                    ]
                } else {
                    vec![
                        actor.subject().into(),
                        "approved".bold(),
                        " this request".into(),
                        " this time".bold(),
                    ]
                };
                ("✔ ".green(), summary)
            }
            ApprovalDecisionSubject::NetworkAccess { target } => (
                "✔ ".green(),
                vec![
                    actor.subject().into(),
                    "approved".bold(),
                    " codex network access to ".into(),
                    Span::from(target).dim(),
                    " this time".bold(),
                ],
            ),
        },
        ApprovedExecpolicyAmendment {
            proposed_execpolicy_amendment,
        } => {
            let snippet = Span::from(exec_snippet(&proposed_execpolicy_amendment.command)).dim();
            (
                "✔ ".green(),
                vec![
                    actor.subject().into(),
                    "approved".bold(),
                    " codex to always run commands that start with ".into(),
                    snippet,
                ],
            )
        }
        ApprovedForSession => match subject {
            ApprovalDecisionSubject::Command(command) => {
                let summary = if let Some(snippet) = non_empty_exec_snippet(&command) {
                    vec![
                        actor.subject().into(),
                        "approved".bold(),
                        " codex to run ".into(),
                        Span::from(snippet).dim(),
                        " every time this session".bold(),
                    ]
                } else {
                    vec![
                        actor.subject().into(),
                        "approved".bold(),
                        " this request".into(),
                        " every time this session".bold(),
                    ]
                };
                ("✔ ".green(), summary)
            }
            ApprovalDecisionSubject::NetworkAccess { target } => (
                "✔ ".green(),
                vec![
                    actor.subject().into(),
                    "approved".bold(),
                    " codex network access to ".into(),
                    Span::from(target).dim(),
                    " every time this session".bold(),
                ],
            ),
        },
        NetworkPolicyAmendment {
            network_policy_amendment,
        } => {
            let target = match subject {
                ApprovalDecisionSubject::NetworkAccess { target } => target,
                ApprovalDecisionSubject::Command(_) => network_policy_amendment.host,
            };
            match network_policy_amendment.action {
                NetworkPolicyRuleAction::Allow => (
                    "✔ ".green(),
                    vec![
                        actor.subject().into(),
                        "persisted".bold(),
                        " Codex network access to ".into(),
                        Span::from(target).dim(),
                    ],
                ),
                NetworkPolicyRuleAction::Deny => (
                    "✗ ".red(),
                    vec![
                        actor.subject().into(),
                        "denied".bold(),
                        " codex network access to ".into(),
                        Span::from(target).dim(),
                        " and saved that rule".into(),
                    ],
                ),
            }
        }
        Denied => match subject {
            ApprovalDecisionSubject::Command(command) => {
                let summary = if let Some(snippet) = non_empty_exec_snippet(&command) {
                    let snippet = Span::from(snippet).dim();
                    match actor {
                        ApprovalDecisionActor::User => vec![
                            actor.subject().into(),
                            "did not approve".bold(),
                            " codex to run ".into(),
                            snippet,
                        ],
                        ApprovalDecisionActor::Guardian => vec![
                            "Request ".into(),
                            "denied".bold(),
                            " for codex to run ".into(),
                            snippet,
                        ],
                    }
                } else {
                    match actor {
                        ApprovalDecisionActor::User => vec![
                            actor.subject().into(),
                            "did not approve".bold(),
                            " this request".into(),
                        ],
                        ApprovalDecisionActor::Guardian => {
                            vec!["Request ".into(), "denied".bold()]
                        }
                    }
                };
                ("✗ ".red(), summary)
            }
            ApprovalDecisionSubject::NetworkAccess { target } => (
                "✗ ".red(),
                vec![
                    actor.subject().into(),
                    "did not approve".bold(),
                    " codex network access to ".into(),
                    Span::from(target).dim(),
                ],
            ),
        },
        TimedOut => match subject {
            ApprovalDecisionSubject::Command(command) => {
                let summary = if let Some(snippet) = non_empty_exec_snippet(&command) {
                    vec![
                        "Review ".into(),
                        "timed out".bold(),
                        " before codex could run ".into(),
                        Span::from(snippet).dim(),
                    ]
                } else {
                    vec![
                        "Review ".into(),
                        "timed out".bold(),
                        " before this request could be approved".into(),
                    ]
                };
                ("✗ ".red(), summary)
            }
            ApprovalDecisionSubject::NetworkAccess { target } => (
                "✗ ".red(),
                vec![
                    "Review ".into(),
                    "timed out".bold(),
                    " before codex could access ".into(),
                    Span::from(target).dim(),
                ],
            ),
        },
        Abort => match subject {
            ApprovalDecisionSubject::Command(command) => {
                let summary = if let Some(snippet) = non_empty_exec_snippet(&command) {
                    vec![
                        actor.subject().into(),
                        "canceled".bold(),
                        " the request to run ".into(),
                        Span::from(snippet).dim(),
                    ]
                } else {
                    vec![
                        actor.subject().into(),
                        "canceled".bold(),
                        " this request".into(),
                    ]
                };
                ("✗ ".red(), summary)
            }
            ApprovalDecisionSubject::NetworkAccess { target } => (
                "✗ ".red(),
                vec![
                    actor.subject().into(),
                    "canceled".bold(),
                    " the request for codex network access to ".into(),
                    Span::from(target).dim(),
                ],
            ),
        },
    };

    Box::new(PrefixedWrappedHistoryCell::new(
        Line::from(summary),
        symbol,
        "  ",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecisionActor {
    User,
    Guardian,
}

impl ApprovalDecisionActor {
    fn subject(self) -> &'static str {
        match self {
            Self::User => "You ",
            Self::Guardian => "Auto-reviewer ",
        }
    }
}

pub fn new_guardian_denied_patch_request(files: Vec<String>) -> Box<dyn HistoryCell> {
    let mut summary = vec![
        "Request ".into(),
        "denied".bold(),
        " for codex to apply ".into(),
    ];
    if files.len() == 1 {
        summary.push("a patch touching ".into());
        summary.push(Span::from(files[0].clone()).dim());
    } else {
        summary.push("a patch touching ".into());
        summary.push(Span::from(files.len().to_string()).dim());
        summary.push(" files".into());
    }

    Box::new(PrefixedWrappedHistoryCell::new(
        Line::from(summary),
        "✗ ".red(),
        "  ",
    ))
}

pub fn new_guardian_denied_action_request(summary: String) -> Box<dyn HistoryCell> {
    let line = Line::from(vec![
        "Request ".into(),
        "denied".bold(),
        " for ".into(),
        Span::from(summary).dim(),
    ]);
    Box::new(PrefixedWrappedHistoryCell::new(line, "✗ ".red(), "  "))
}

pub fn new_guardian_approved_action_request(summary: String) -> Box<dyn HistoryCell> {
    let line = Line::from(vec![
        "Request ".into(),
        "approved".bold(),
        " for ".into(),
        Span::from(summary).dim(),
    ]);
    Box::new(PrefixedWrappedHistoryCell::new(line, "✔ ".green(), "  "))
}

pub fn new_guardian_timed_out_patch_request(files: Vec<String>) -> Box<dyn HistoryCell> {
    let mut summary = vec![
        "Review ".into(),
        "timed out".bold(),
        " before codex could apply ".into(),
    ];
    if files.len() == 1 {
        summary.push("a patch touching ".into());
        summary.push(Span::from(files[0].clone()).dim());
    } else {
        summary.push("a patch touching ".into());
        summary.push(Span::from(files.len().to_string()).dim());
        summary.push(" files".into());
    }

    Box::new(PrefixedWrappedHistoryCell::new(
        Line::from(summary),
        "✗ ".red(),
        "  ",
    ))
}

pub fn new_guardian_timed_out_action_request(summary: String) -> Box<dyn HistoryCell> {
    let line = Line::from(vec![
        "Review ".into(),
        "timed out".bold(),
        " before ".into(),
        Span::from(summary).dim(),
    ]);
    Box::new(PrefixedWrappedHistoryCell::new(line, "✗ ".red(), "  "))
}

/// Cyan history cell line showing the current review status.
pub(crate) fn new_review_status_line(message: String) -> PlainHistoryCell {
    PlainHistoryCell {
        lines: vec![Line::from(message.cyan())],
    }
}
