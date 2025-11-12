use std::sync::Arc;

use codex_execpolicy2::Decision;
use codex_execpolicy2::Error;
use codex_execpolicy2::Evaluation;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::Rule;
use codex_execpolicy2::RuleMatch;
use codex_execpolicy2::rule::PatternToken;
use codex_execpolicy2::rule::PrefixPattern;
use codex_execpolicy2::rule::PrefixRule;
use pretty_assertions::assert_eq;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(std::string::ToString::to_string).collect()
}

#[test]
fn basic_match() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
)
    "#;
    let policy = PolicyParser::parse("test.codexpolicy", policy_src).expect("parse policy");
    let cmd = tokens(&["git", "status"]);
    let evaluation = policy.check(&cmd);
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Allow,
            }],
        },
        evaluation
    );
}

#[test]
fn only_first_token_alias_expands_to_multiple_rules() {
    let policy_src = r#"
prefix_rule(
    pattern = [["bash", "sh"], ["-c", "-l"]],
)
    "#;
    let policy = PolicyParser::parse("test.codexpolicy", policy_src).expect("parse policy");

    let bash_rules = policy.rules().get_vec("bash").expect("bash rules");
    let sh_rules = policy.rules().get_vec("sh").expect("sh rules");
    assert_eq!(
        vec![Rule::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("bash"),
                rest: vec![PatternToken::Alts(vec!["-c".to_string(), "-l".to_string()])].into(),
            },
            decision: Decision::Allow,
        })],
        bash_rules.clone()
    );
    assert_eq!(
        vec![Rule::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("sh"),
                rest: vec![PatternToken::Alts(vec!["-c".to_string(), "-l".to_string()])].into(),
            },
            decision: Decision::Allow,
        })],
        sh_rules.clone()
    );

    let bash_eval = policy.check(&tokens(&["bash", "-c", "echo", "hi"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["bash", "-c"]),
                decision: Decision::Allow,
            }],
        },
        bash_eval
    );

    let sh_eval = policy.check(&tokens(&["sh", "-l", "echo", "hi"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["sh", "-l"]),
                decision: Decision::Allow,
            }],
        },
        sh_eval
    );
}

#[test]
fn tail_aliases_are_not_cartesian_expanded() {
    let policy_src = r#"
prefix_rule(
    pattern = ["npm", ["i", "install"], ["--legacy-peer-deps", "--no-save"]],
)
    "#;
    let policy = PolicyParser::parse("test.codexpolicy", policy_src).expect("parse policy");

    let rules = policy.rules().get_vec("npm").expect("npm rules");
    assert_eq!(
        vec![Rule::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("npm"),
                rest: vec![
                    PatternToken::Alts(vec!["i".to_string(), "install".to_string()]),
                    PatternToken::Alts(vec![
                        "--legacy-peer-deps".to_string(),
                        "--no-save".to_string()
                    ]),
                ]
                .into(),
            },
            decision: Decision::Allow,
        })],
        rules.clone()
    );

    let npm_i = policy.check(&tokens(&["npm", "i", "--legacy-peer-deps"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["npm", "i", "--legacy-peer-deps"]),
                decision: Decision::Allow,
            }],
        },
        npm_i
    );

    let npm_install = policy.check(&tokens(&["npm", "install", "--no-save", "leftpad"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["npm", "install", "--no-save"]),
                decision: Decision::Allow,
            }],
        },
        npm_install
    );
}

#[test]
fn match_and_not_match_examples_are_enforced() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
    match = [["git", "status"], "git status"],
    not_match = [
        ["git", "--config", "color.status=always", "status"],
        "git --config color.status=always status",
    ],
)
    "#;
    let policy = PolicyParser::parse("test.codexpolicy", policy_src).expect("parse policy");
    let match_eval = policy.check(&tokens(&["git", "status"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Allow,
            }],
        },
        match_eval
    );

    let no_match_eval = policy.check(&tokens(&[
        "git",
        "--config",
        "color.status=always",
        "status",
    ]));
    assert_eq!(Evaluation::NoMatch, no_match_eval);
}

#[test]
fn strictest_decision_wins_across_matches() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
    decision = "allow",
)
prefix_rule(
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let policy = PolicyParser::parse("test.codexpolicy", policy_src).expect("parse policy");

    let status = policy.check(&tokens(&["git", "status"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Prompt,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "status"]),
                    decision: Decision::Allow,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                },
            ],
        },
        status
    );

    let commit = policy.check(&tokens(&["git", "commit", "-m", "hi"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Forbidden,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                },
            ],
        },
        commit
    );
}
