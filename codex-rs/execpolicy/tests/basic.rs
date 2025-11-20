use std::any::Any;
use std::sync::Arc;

use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::RuleMatch;
use codex_execpolicy::RuleRef;
use codex_execpolicy::rule::PatternToken;
use codex_execpolicy::rule::PrefixPattern;
use codex_execpolicy::rule::PrefixRule;
use pretty_assertions::assert_eq;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(std::string::ToString::to_string).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuleSnapshot {
    Prefix(PrefixRule),
}

fn rule_snapshots(rules: &[RuleRef]) -> Vec<RuleSnapshot> {
    rules
        .iter()
        .map(|rule| {
            let rule_any = rule.as_ref() as &dyn Any;
            if let Some(prefix_rule) = rule_any.downcast_ref::<PrefixRule>() {
                RuleSnapshot::Prefix(prefix_rule.clone())
            } else {
                panic!("unexpected rule type in RuleRef: {rule:?}");
            }
        })
        .collect()
}

#[test]
fn basic_match() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
)
    "#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();
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
fn parses_multiple_policy_files() {
    let first_policy = r#"
prefix_rule(
    pattern = ["git"],
    decision = "prompt",
)
    "#;
    let second_policy = r#"
prefix_rule(
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let mut parser = PolicyParser::new();
    parser
        .parse("first.codexpolicy", first_policy)
        .expect("parse policy");
    parser
        .parse("second.codexpolicy", second_policy)
        .expect("parse policy");
    let policy = parser.build();

    let git_rules = rule_snapshots(policy.rules().get_vec("git").expect("git rules"));
    assert_eq!(
        vec![
            RuleSnapshot::Prefix(PrefixRule {
                pattern: PrefixPattern {
                    first: Arc::from("git"),
                    rest: Vec::<PatternToken>::new().into(),
                },
                decision: Decision::Prompt,
            }),
            RuleSnapshot::Prefix(PrefixRule {
                pattern: PrefixPattern {
                    first: Arc::from("git"),
                    rest: vec![PatternToken::Single("commit".to_string())].into(),
                },
                decision: Decision::Forbidden,
            }),
        ],
        git_rules
    );

    let status_eval = policy.check(&tokens(&["git", "status"]));
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git"]),
                decision: Decision::Prompt,
            }],
        },
        status_eval
    );

    let commit_eval = policy.check(&tokens(&["git", "commit", "-m", "hi"]));
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
        commit_eval
    );
}

#[test]
fn only_first_token_alias_expands_to_multiple_rules() {
    let policy_src = r#"
prefix_rule(
    pattern = [["bash", "sh"], ["-c", "-l"]],
)
    "#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();

    let bash_rules = rule_snapshots(policy.rules().get_vec("bash").expect("bash rules"));
    let sh_rules = rule_snapshots(policy.rules().get_vec("sh").expect("sh rules"));
    assert_eq!(
        vec![RuleSnapshot::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("bash"),
                rest: vec![PatternToken::Alts(vec!["-c".to_string(), "-l".to_string()])].into(),
            },
            decision: Decision::Allow,
        })],
        bash_rules
    );
    assert_eq!(
        vec![RuleSnapshot::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("sh"),
                rest: vec![PatternToken::Alts(vec!["-c".to_string(), "-l".to_string()])].into(),
            },
            decision: Decision::Allow,
        })],
        sh_rules
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
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();

    let rules = rule_snapshots(policy.rules().get_vec("npm").expect("npm rules"));
    assert_eq!(
        vec![RuleSnapshot::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("npm"),
                rest: vec![
                    PatternToken::Alts(vec!["i".to_string(), "install".to_string()]),
                    PatternToken::Alts(vec![
                        "--legacy-peer-deps".to_string(),
                        "--no-save".to_string(),
                    ]),
                ]
                .into(),
            },
            decision: Decision::Allow,
        })],
        rules
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
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();
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
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();

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

#[test]
fn strictest_decision_across_multiple_commands() {
    let policy_src = r#"
prefix_rule(
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.codexpolicy", policy_src)
        .expect("parse policy");
    let policy = parser.build();

    let commands = vec![
        tokens(&["git", "status"]),
        tokens(&["git", "commit", "-m", "hi"]),
    ];

    let evaluation = policy.check_multiple(&commands);
    assert_eq!(
        Evaluation::Match {
            decision: Decision::Forbidden,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                },
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
        evaluation
    );
}
