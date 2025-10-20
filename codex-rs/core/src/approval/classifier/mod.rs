// ============================================================================
// CLASSIFICATION - Operates solely on SimpleAst
// ============================================================================

mod git_classifier;

use crate::approval::ast::SimpleAst;
use crate::approval::command_rules::CommandCategory;
use crate::approval::command_rules::CommandMatcher;
use crate::approval::command_rules::CommandRule;
use crate::approval::git_parser;
use crate::approval::git_rules;
use crate::approval::rules_index::RULE_INDEX;

fn rule_matches_ast(rule: &CommandRule, ast: &SimpleAst) -> bool {
    if rule.tool != ast.tool {
        return false;
    }

    match &rule.matcher {
        CommandMatcher::Always => true,

        // Match if first non-flag token equals an allowed subcommand
        CommandMatcher::WithSubcommands(allowed) => ast
            .subcommand
            .as_deref()
            .map(|s| allowed.contains(&s))
            .unwrap_or(false),

        // Match only if none of the forbidden flags/args appear before `--`
        CommandMatcher::WithoutForbiddenArgs(forbidden) => {
            !ast.flags.iter().any(|f| forbidden.contains(&f.as_str()))
        }

        // Delegate to custom predicate using original argv
        CommandMatcher::Custom(pred) => pred(&ast.raw),
    }
}

fn classify_simple(ast: &SimpleAst) -> CommandCategory {
    match ast.tool.as_str() {
        "git" => {
            // Use the richer git parser, but only after AST normalization
            let git_cmd = git_parser::parse_git_command(&ast.raw);
            git_rules::classify_git_command(&git_cmd)
        }
        tool => {
            if let Some(rules) = RULE_INDEX.get(tool) {
                for rule in rules {
                    if rule_matches_ast(rule, ast) {
                        return rule.category;
                    }
                }
            }
            CommandCategory::Unrecognized
        }
    }
}

pub(crate) fn matches_rule(rule: &CommandRule, ast: &SimpleAst) -> bool {
    rule_matches_ast(rule, ast)
}

pub(crate) fn classify_simple_ast(ast: &SimpleAst) -> CommandCategory {
    classify_simple(ast)
}
