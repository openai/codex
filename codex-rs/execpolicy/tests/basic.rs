use std::any::Any;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_execpolicy::Decision;
use codex_execpolicy::Error;
use codex_execpolicy::Evaluation;
use codex_execpolicy::MatchOptions;
use codex_execpolicy::NetworkRuleProtocol;
use codex_execpolicy::Policy;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::RuleMatch;
use codex_execpolicy::RuleRef;
use codex_execpolicy::blocking_append_allow_prefix_rule;
use codex_execpolicy::rule::PatternToken;
use codex_execpolicy::rule::PrefixPattern;
use codex_execpolicy::rule::PrefixRule;
use codex_utils_absolute_path::AbsolutePathBuf;
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

fn absolute_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path.to_string())
        .unwrap_or_else(|error| panic!("expected absolute path `{path}`: {error}"))
}

fn host_absolute_path(segments: &[&str]) -> String {
    let mut path = if cfg!(windows) {
        PathBuf::from(r"C:\")
    } else {
        PathBuf::from("/")
    };
    for segment in segments {
        path.push(segment);
    }
    path.to_string_lossy().into_owned()
}

fn host_executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn starlark_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
fn network_rules_compile_into_domain_lists() -> Result<()> {
    let policy_src = r#"
network_rule(host = "google.com", protocol = "http", decision = "allow")
network_rule(host = "api.github.com", protocol = "https", decision = "allow")
network_rule(host = "blocked.example.com", protocol = "https", decision = "deny")
network_rule(host = "prompt-only.example.com", protocol = "https", decision = "prompt")
    "#;
    let mut parser = PolicyParser::new();
    parser.parse("network.rules", policy_src)?;
    let policy = parser.build();

    assert_eq!(policy.network_rules().len(), 4);
    assert_eq!(
        policy.network_rules()[1].protocol,
        NetworkRuleProtocol::Https
    );

    let (allowed, denied) = policy.compiled_network_domains();
    assert_eq!(
        allowed,
        vec!["google.com".to_string(), "api.github.com".to_string()]
    );
    assert_eq!(denied, vec!["blocked.example.com".to_string()]);
    Ok(())
}

#[test]
fn network_rule_rejects_wildcard_hosts() {
    let mut parser = PolicyParser::new();
    let err = parser
        .parse(
            "network.rules",
            r#"network_rule(host="*", protocol="http", decision="allow")"#,
        )
        .expect_err("wildcard network_rule host should fail");
    assert!(err.to_string().contains("wildcards are not allowed"));
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                    resolved_program: None,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                resolved_program: None,
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
                    resolved_program: None,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    resolved_program: None,
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
                    resolved_program: None,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git"]),
                    decision: Decision::Prompt,
                    resolved_program: None,
                    justification: None,
                },
                RuleMatch::PrefixRuleMatch {
                    matched_prefix: tokens(&["git", "commit"]),
                    decision: Decision::Forbidden,
                    resolved_program: None,
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

#[test]
fn parses_host_executable_paths() -> Result<()> {
    let homebrew_git = host_absolute_path(&["opt", "homebrew", "bin", "git"]);
    let usr_git = host_absolute_path(&["usr", "bin", "git"]);
    let homebrew_git_literal = starlark_string(&homebrew_git);
    let usr_git_literal = starlark_string(&usr_git);
    let policy_src = format!(
        r#"
host_executable(
    name = "git",
    paths = [
        "{homebrew_git_literal}",
        "{usr_git_literal}",
        "{usr_git_literal}",
    ],
)
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src)?;
    let policy = parser.build();

    assert_eq!(
        policy
            .host_executables()
            .get("git")
            .expect("missing git host executable")
            .as_ref(),
        [absolute_path(&homebrew_git), absolute_path(&usr_git)]
    );
    Ok(())
}

#[test]
fn host_executable_rejects_non_absolute_path() {
    let policy_src = r#"
host_executable(name = "git", paths = ["git"])
"#;
    let mut parser = PolicyParser::new();
    let err = parser
        .parse("test.rules", policy_src)
        .expect_err("expected parse error");
    assert!(
        err.to_string()
            .contains("host_executable paths must be absolute")
    );
}

#[test]
fn host_executable_rejects_name_with_path_separator() {
    let git_path = host_absolute_path(&["usr", "bin", "git"]);
    let git_path_literal = starlark_string(&git_path);
    let policy_src =
        format!(r#"host_executable(name = "{git_path_literal}", paths = ["{git_path_literal}"])"#);
    let mut parser = PolicyParser::new();
    let err = parser
        .parse("test.rules", &policy_src)
        .expect_err("expected parse error");
    assert!(
        err.to_string()
            .contains("host_executable name must be a bare executable name")
    );
}

#[test]
fn host_executable_rejects_path_with_wrong_basename() {
    let rg_path = host_absolute_path(&["usr", "bin", "rg"]);
    let rg_path_literal = starlark_string(&rg_path);
    let policy_src = format!(r#"host_executable(name = "git", paths = ["{rg_path_literal}"])"#);
    let mut parser = PolicyParser::new();
    let err = parser
        .parse("test.rules", &policy_src)
        .expect_err("expected parse error");
    assert!(err.to_string().contains("must have basename `git`"));
}

#[test]
fn host_executable_last_definition_wins() -> Result<()> {
    let usr_git = host_absolute_path(&["usr", "bin", "git"]);
    let homebrew_git = host_absolute_path(&["opt", "homebrew", "bin", "git"]);
    let usr_git_literal = starlark_string(&usr_git);
    let homebrew_git_literal = starlark_string(&homebrew_git);
    let mut parser = PolicyParser::new();
    parser.parse(
        "shared.rules",
        &format!(r#"host_executable(name = "git", paths = ["{usr_git_literal}"])"#),
    )?;
    parser.parse(
        "user.rules",
        &format!(r#"host_executable(name = "git", paths = ["{homebrew_git_literal}"])"#),
    )?;
    let policy = parser.build();

    assert_eq!(
        policy
            .host_executables()
            .get("git")
            .expect("missing git host executable")
            .as_ref(),
        [absolute_path(&homebrew_git)]
    );
    Ok(())
}

#[test]
fn host_executable_resolution_uses_basename_rule_when_allowed() -> Result<()> {
    let git_name = host_executable_name("git");
    let git_path = host_absolute_path(&["usr", "bin", &git_name]);
    let git_path_literal = starlark_string(&git_path);
    let policy_src = format!(
        r#"
prefix_rule(pattern = ["git", "status"], decision = "prompt")
host_executable(name = "git", paths = ["{git_path_literal}"])
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src)?;
    let policy = parser.build();

    let evaluation = policy.check_with_options(
        &[git_path.clone(), "status".to_string()],
        &allow_all,
        &MatchOptions {
            resolve_host_executables: true,
        },
    );
    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Prompt,
                resolved_program: Some(absolute_path(&git_path)),
                justification: None,
            }],
        }
    );
    Ok(())
}

#[test]
fn prefix_rule_examples_honor_host_executable_resolution() -> Result<()> {
    let allowed_git_name = host_executable_name("git");
    let allowed_git = host_absolute_path(&["usr", "bin", &allowed_git_name]);
    let other_git = host_absolute_path(&["opt", "homebrew", "bin", &allowed_git_name]);
    let allowed_git_literal = starlark_string(&allowed_git);
    let other_git_literal = starlark_string(&other_git);
    let policy_src = format!(
        r#"
prefix_rule(
    pattern = ["git", "status"],
    match = [["{allowed_git_literal}", "status"]],
    not_match = [["{other_git_literal}", "status"]],
)
host_executable(name = "git", paths = ["{allowed_git_literal}"])
"#
    );

    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src)?;

    Ok(())
}

#[test]
fn host_executable_resolution_respects_explicit_empty_allowlist() -> Result<()> {
    let policy_src = r#"
prefix_rule(pattern = ["git"], decision = "prompt")
host_executable(name = "git", paths = [])
"#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();
    let git_path = host_absolute_path(&["usr", "bin", "git"]);

    let evaluation = policy.check_with_options(
        &[git_path.clone(), "status".to_string()],
        &allow_all,
        &MatchOptions {
            resolve_host_executables: true,
        },
    );
    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec![git_path, "status".to_string()],
                decision: Decision::Allow,
            }],
        }
    );
    Ok(())
}

#[test]
fn host_executable_resolution_ignores_path_not_in_allowlist() -> Result<()> {
    let allowed_git = host_absolute_path(&["usr", "bin", "git"]);
    let other_git = host_absolute_path(&["opt", "homebrew", "bin", "git"]);
    let allowed_git_literal = starlark_string(&allowed_git);
    let policy_src = format!(
        r#"
prefix_rule(pattern = ["git"], decision = "prompt")
host_executable(name = "git", paths = ["{allowed_git_literal}"])
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src)?;
    let policy = parser.build();

    let evaluation = policy.check_with_options(
        &[other_git.clone(), "status".to_string()],
        &allow_all,
        &MatchOptions {
            resolve_host_executables: true,
        },
    );
    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec![other_git, "status".to_string()],
                decision: Decision::Allow,
            }],
        }
    );
    Ok(())
}

#[test]
fn host_executable_resolution_falls_back_without_mapping() -> Result<()> {
    let policy_src = r#"
prefix_rule(pattern = ["git"], decision = "prompt")
"#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src)?;
    let policy = parser.build();
    let git_path = host_absolute_path(&["usr", "bin", "git"]);

    let evaluation = policy.check_with_options(
        &[git_path.clone(), "status".to_string()],
        &allow_all,
        &MatchOptions {
            resolve_host_executables: true,
        },
    );
    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: tokens(&["git"]),
                decision: Decision::Prompt,
                resolved_program: Some(absolute_path(&git_path)),
                justification: None,
            }],
        }
    );
    Ok(())
}

#[test]
fn host_executable_resolution_does_not_override_exact_match() -> Result<()> {
    let git_path = host_absolute_path(&["usr", "bin", "git"]);
    let git_path_literal = starlark_string(&git_path);
    let policy_src = format!(
        r#"
prefix_rule(pattern = ["{git_path_literal}"], decision = "allow")
prefix_rule(pattern = ["git"], decision = "prompt")
host_executable(name = "git", paths = ["{git_path_literal}"])
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src)?;
    let policy = parser.build();

    let evaluation = policy.check_with_options(
        &[git_path.clone(), "status".to_string()],
        &allow_all,
        &MatchOptions {
            resolve_host_executables: true,
        },
    );
    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec![git_path],
                decision: Decision::Allow,
                resolved_program: None,
                justification: None,
            }],
        }
    );
    Ok(())
}
