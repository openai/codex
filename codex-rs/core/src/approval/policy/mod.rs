mod decision_policy;

use crate::approval::CommandDecision;
use crate::approval::ast::CommandAst;
use crate::approval::classifier;
use crate::approval::command_rules::CommandCategory;
use crate::approval::parser;
use crate::exec::SandboxType;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashSet;

/// Given a list of categories, return the highest risk.
/// The enum ordering is from least to most risk; Pick max().
pub(crate) fn aggregate_categories(categories: &[CommandCategory]) -> CommandCategory {
    categories
        .iter()
        .max_by_key(|c| c.risk_rank())
        .copied()
        .unwrap_or(CommandCategory::Unrecognized)
}

pub(crate) fn evaluate_decision_policy(
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    with_escalated_permissions: bool,
    user_explicitly_approved: bool,
) -> CommandDecision {
    decision_policy::evaluate_decision_policy(
        approval_policy,
        sandbox_policy,
        with_escalated_permissions,
        user_explicitly_approved,
    )
}

/// Strict pipeline: input → AST → classify → aggregate → policy → result.
pub fn assess_command(
    command: &[String],
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
    approved_cache: &HashSet<Vec<String>>,
    with_escalated_permissions: bool,
) -> CommandDecision {
    // input → AST
    let ast = parser::parse_to_ast(command);

    // AST → classify (per SimpleAst)
    let categories: Vec<CommandCategory> = match ast {
        CommandAst::Sequence(simples) => simples
            .iter()
            .map(classifier::classify_simple_ast)
            .collect(),
        CommandAst::Unknown(_) => vec![CommandCategory::Unrecognized],
    };

    // classify → aggregate
    let category = aggregate_categories(&categories);

    // Did the user explicitly approve this exact command?
    let user_explicitly_approved = approved_cache.contains(command);

    // aggregate → policy → result
    match category {
        CommandCategory::Unrecognized => evaluate_decision_policy(
            approval_policy,
            sandbox_policy,
            with_escalated_permissions,
            user_explicitly_approved,
        ),

        CommandCategory::DeletesData => {
            // Always ask unless explicitly approved; obey AskForApproval::Never
            if user_explicitly_approved {
                CommandDecision::permit(SandboxType::None, true)
            } else if approval_policy == AskForApproval::Never {
                CommandDecision::deny(
                    "Destructive command detected; rejected by user approval settings",
                )
            } else {
                CommandDecision::require_approval()
            }
        }

        CommandCategory::ReadsFilesystem | CommandCategory::ReadsVcs => {
            // Read-only: no sandbox needed
            CommandDecision::permit(SandboxType::None, user_explicitly_approved)
        }

        CommandCategory::ModifiesFilesystem | CommandCategory::ModifiesVcs => {
            // Write operations → untrusted path
            evaluate_decision_policy(
                approval_policy,
                sandbox_policy,
                with_escalated_permissions,
                user_explicitly_approved,
            )
        }
    }
}

impl CommandCategory {
    fn risk_rank(self) -> u8 {
        match self {
            CommandCategory::ReadsFilesystem => 0,
            CommandCategory::ReadsVcs => 1,
            CommandCategory::ModifiesFilesystem => 2,
            CommandCategory::ModifiesVcs => 3,
            CommandCategory::Unrecognized => 4,
            CommandCategory::DeletesData => 5,
        }
    }
}
