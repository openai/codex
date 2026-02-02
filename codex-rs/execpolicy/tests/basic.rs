use std::any::Any;
use std::fs;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_execpolicy::Decision;
use codex_execpolicy::Error;
use codex_execpolicy::Evaluation;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::RuleMatch;
use codex_execpolicy::RuleRef;
use codex_execpolicy::blocking_append_allow_prefix_rule;
use codex_execpolicy::rule::PatternToken;
use codex_execpolicy::rule::PrefixPattern;
use codex_execpolicy::rule::PrefixRule;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(std::string::ToString::to_string).collect()
}

fn allow_all(_: &[String]) -> Decision {
    Decision::Allow
}

fn prompt_all(_: &[String]) -> Decision {
    Decision::Prompt
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
fn append_allow_prefix_rule_dedupes_existing_rule() -> Result<()> {
    let tmp = tempdir().context("create temp dir")?;
    let policy_path = tmp.path().join("rules").join("default.rules");
    let prefix = tokens(&["python3"]);

    blocking_append_allow_prefix_rule(&policy_path, &prefix)?;
    blocking_append_allow_prefix_rule(&policy_path, &prefix)?;

    let contents = fs::read_to_string(&policy_path).context("read policy")?;
    assert_eq!(
        contents,
        r#"prefix_rule(pattern=["python3"], decision="allow")
"#
    );
    Ok(())
}

#[test]
fn basic_match() -> Result<()> {
    let policy_src = r#"
prefix_rule(
    pattern = ["git", "status"],
)
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();
    let cmd = tokens(&["git", "status"]);
    let evaluation = policy.check(&cmd, &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        evaluation
    );
    Ok(())
}

#[test]
fn justification_is_attached_to_forbidden_matches() -> Result<()> {
    let policy_src = r#"
prefix_rule(
    pattern = ["rm"],
    decision = "forbidden",
    justification = "destructive command",
)
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let evaluation = policy.check(
        &tokens(&["rm", "-rf", "/some/important/folder"]),
        &allow_all,
    );
    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["rm"]),
                decision: Decision::Forbidden,
                justification: Some("destructive command".to_string()),
            }],
        },
        evaluation
    );
    Ok(())
}

#[test]
fn justification_can_be_used_with_allow_decision() -> Result<()> {
    let policy_src = r#"
prefix_rule(
    pattern = ["ls"],
    decision = "allow",
    justification = "safe and commonly used",
)
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let evaluation = policy.check(&tokens(&["ls", "-l"]), &prompt_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["ls"]),
                decision: Decision::Allow,
                justification: Some("safe and commonly used".to_string()),
            }],
        },
        evaluation
    );
    Ok(())
}

#[test]
fn justification_cannot_be_empty() {
    let policy_src = r#"
prefix_rule(
    pattern = ["ls"],
    decision = "prompt",
    justification = "   ",
)
    "#;
    let mut parser = PolicyParser::new();
    let err = parser
        .parse("test.rules", policy_src)
        .expect_err("expected parse error");
    assert!(
        err.to_string()
            .contains("invalid rule: justification cannot be empty")
    );
}

#[test]
fn add_prefix_rule_extends_policy() -> Result<()> {
    let mut policy = Policy::empty();
    policy.add_prefix_rule(&tokens(&["ls", "-l"]), Decision::Prompt)?;

    let rules = rule_snapshots(policy.rules().get_vec("ls").context("missing ls rules")?);
    assert_eq!(
        vec![RuleSnapshot::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("ls"),
                rest: vec![PatternToken::Single(String::from("-l"))].into(),
            },
            decision: Decision::Prompt,
            justification: None,
        })],
        rules
    );

    let evaluation = policy.check(&tokens(&["ls", "-l", "/some/important/folder"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["ls", "-l"]),
                decision: Decision::Prompt,
                justification: None,
            }],
        },
        evaluation
    );
    Ok(())
}

#[test]
fn add_prefix_rule_rejects_empty_prefix() -> Result<()> {
    let mut policy = Policy::empty();
    let result = policy.add_prefix_rule(&[], Decision::Allow);

    match result.unwrap_err() {
        Error::InvalidPattern(message) => assert_eq!(message, "prefix cannot be empty"),
        other => panic!("expected InvalidPattern(..), got {other:?}"),
    }
    Ok(())
}

#[test]
fn parses_multiple_policy_files() -> Result<()> {
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
    parser.parse("first.rules", first_policy)?;
    parser.parse("second.rules", second_policy)?;
    let policy = parser.build();

    let git_rules = rule_snapshots(policy.rules().get_vec("git").context("missing git rules")?);
    assert_eq!(
        vec![
            RuleSnapshot::Prefix(PrefixRule {
                pattern: PrefixPattern {
                    first: Arc::from("git"),
                    rest: Vec::<PatternToken>::new().into(),
                },
                decision: Decision::Prompt,
                justification: None,
            }),
            RuleSnapshot::Prefix(PrefixRule {
                pattern: PrefixPattern {
                    first: Arc::from("git"),
                    rest: vec![PatternToken::Single("commit".to_string())].into(),
                },
                decision: Decision::Forbidden,
                justification: None,
            }),
        ],
        git_rules
    );

    let status_eval = policy.check(&tokens(&["git", "status"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git"]),
                decision: Decision::Prompt,
                justification: None,
            }],
        },
        status_eval
    );

    let commit_eval = policy.check(&tokens(&["git", "commit", "-m", "hi"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    justification: None,
                },
            ],
        },
        commit_eval
    );
    Ok(())
}

#[test]
fn only_first_token_alias_expands_to_multiple_rules() -> Result<()> {
    let policy_src = r#"
prefix_rule(
    pattern = [["bash", "sh"], ["-c", "-l"]],
)
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let bash_rules = rule_snapshots(
        policy
            .rules()
            .get_vec("bash")
            .context("missing bash rules")?,
    );
    let sh_rules = rule_snapshots(policy.rules().get_vec("sh").context("missing sh rules")?);
    assert_eq!(
        vec![RuleSnapshot::Prefix(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from("bash"),
                rest: vec![PatternToken::Alts(vec!["-c".to_string(), "-l".to_string()])].into(),
            },
            decision: Decision::Allow,
            justification: None,
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
            justification: None,
        })],
        sh_rules
    );

    let bash_eval = policy.check(&tokens(&["bash", "-c", "echo", "hi"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["bash", "-c"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        bash_eval
    );

    let sh_eval = policy.check(&tokens(&["sh", "-l", "echo", "hi"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["sh", "-l"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        sh_eval
    );
    Ok(())
}

#[test]
fn tail_aliases_are_not_cartesian_expanded() -> Result<()> {
    let policy_src = r#"
prefix_rule(
    pattern = ["npm", ["i", "install"], ["--legacy-peer-deps", "--no-save"]],
)
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let rules = rule_snapshots(policy.rules().get_vec("npm").context("missing npm rules")?);
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
            justification: None,
        })],
        rules
    );

    let npm_i = policy.check(&tokens(&["npm", "i", "--legacy-peer-deps"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["npm", "i", "--legacy-peer-deps"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        npm_i
    );

    let npm_install = policy.check(
        &tokens(&["npm", "install", "--no-save", "leftpad"]),
        &allow_all,
    );
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["npm", "install", "--no-save"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        npm_install
    );
    Ok(())
}

#[test]
fn match_and_not_match_examples_are_enforced() -> Result<()> {
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
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();
    let match_eval = policy.check(&tokens(&["git", "status"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Allow,
                justification: None,
            }],
        },
        match_eval
    );

    let no_match_eval = policy.check(
        &tokens(&["git", "--config", "color.status=always", "status"]),
        &allow_all,
    );
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: tokens(&["git", "--config", "color.status=always", "status",]),
                decision: Decision::Allow,
            }],
        },
        no_match_eval
    );
    Ok(())
}

#[test]
fn strictest_decision_wins_across_matches() -> Result<()> {
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
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let commit = policy.check(&tokens(&["git", "commit", "-m", "hi"]), &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    justification: None,
                },
            ],
        },
        commit
    );
    Ok(())
}

#[test]
fn strictest_decision_across_multiple_commands() -> Result<()> {
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
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();

    let commands = vec![
        tokens(&["git", "status"]),
        tokens(&["git", "commit", "-m", "hi"]),
    ];

    let evaluation = policy.check_multiple(&commands, &allow_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    justification: None,
                },
            ],
        },
        evaluation
    );
    Ok(())
}

#[test]
fn heuristics_match_is_returned_when_no_policy_matches() {
    let policy = Policy::empty();
    let command = tokens(&["python"]);

    let evaluation = policy.check(&command, &prompt_all);
    assert_eq!(
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command,
                decision: Decision::Prompt,
            }],
        },
        evaluation
    );
}
