use codex_execpolicy2::Decision;
use codex_execpolicy2::Evaluation;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::RuleMatch;
use codex_execpolicy2::rule::PatternToken;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(std::string::ToString::to_string).collect()
}

#[test]
fn basic_match() {
    let policy_src = r#"
prefix_rule(
    id = "git_status",
    pattern = ["git", "status"],
)
    "#;
    let policy = PolicyParser::new("test.policy", policy_src)
        .parse()
        .expect("parse policy");
    let cmd = tokens(&["git", "status"]);
    let Evaluation::Match {
        decision,
        matched_rules,
    } = policy.evaluate(&cmd)
    else {
        panic!("expected match");
    };
    assert_eq!(decision, Decision::Allow);
    assert_eq!(
        matched_rules,
        vec![RuleMatch {
            rule_id: "git_status".to_string(),
            matched_prefix: tokens(&["git", "status"]),
            decision: Decision::Allow,
        }]
    );
}

#[test]
fn only_first_token_alias_expands_to_multiple_rules() {
    let policy_src = r#"
prefix_rule(
    id = "shell",
    pattern = [["bash", "sh"], ["-c", "-l"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let bash_rules = policy.rules().get_vec("bash").expect("bash rules");
    let sh_rules = policy.rules().get_vec("sh").expect("sh rules");
    assert_eq!(bash_rules.len(), 1);
    assert_eq!(sh_rules.len(), 1);

    for (cmd, prefix) in [
        (
            tokens(&["bash", "-c", "echo", "hi"]),
            tokens(&["bash", "-c"]),
        ),
        (tokens(&["sh", "-l", "echo", "hi"]), tokens(&["sh", "-l"])),
    ] {
        let Evaluation::Match { matched_rules, .. } = policy.evaluate(&cmd) else {
            panic!("expected match");
        };
        assert_eq!(matched_rules[0].matched_prefix, prefix);
    }
}

#[test]
fn tail_aliases_are_not_cartesian_expanded() {
    let policy_src = r#"
prefix_rule(
    id = "npm_install_variants",
    pattern = ["npm", ["i", "install"], ["--legacy-peer-deps", "--no-save"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let rules = policy.rules().get_vec("npm").expect("npm rules");
    assert_eq!(rules.len(), 1);
    let rule = &rules[0];
    assert_eq!(
        rule.pattern.rest,
        vec![
            PatternToken::Alts(tokens(&["i", "install"])),
            PatternToken::Alts(tokens(&["--legacy-peer-deps", "--no-save"])),
        ],
    );

    for cmd in [
        tokens(&["npm", "i", "--legacy-peer-deps"]),
        tokens(&["npm", "install", "--no-save", "leftpad"]),
    ] {
        assert!(matches!(policy.evaluate(&cmd), Evaluation::Match { .. }));
    }
}

#[test]
fn match_and_not_match_examples_are_enforced() {
    let policy_src = r#"
prefix_rule(
    id = "git_status",
    pattern = ["git", "status"],
    match = [["git", "status"]],
    not_match = [["git", "reset", "--hard"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");
    assert!(matches!(
        policy.evaluate(&tokens(&["git", "status"])),
        Evaluation::Match { .. }
    ));
    assert!(matches!(
        policy.evaluate(&tokens(&["git", "reset", "--hard"])),
        Evaluation::NoMatch
    ));
}

#[test]
fn strictest_decision_wins_across_matches() {
    let policy_src = r#"
prefix_rule(
    id = "allow_git_status",
    pattern = ["git", "status"],
    decision = "allow",
)
prefix_rule(
    id = "prompt_git",
    pattern = ["git"],
    decision = "prompt",
)
prefix_rule(
    id = "forbid_git_commit",
    pattern = ["git", "commit"],
    decision = "forbidden",
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    let status = tokens(&["git", "status"]);
    let Evaluation::Match {
        decision: status_decision,
        matched_rules: status_matches,
    } = policy.evaluate(&status)
    else {
        panic!("expected status to match");
    };
    assert_eq!(status_decision, Decision::Prompt);
    assert_eq!(
        status_matches,
        vec![
            RuleMatch {
                rule_id: "allow_git_status".to_string(),
                matched_prefix: tokens(&["git", "status"]),
                decision: Decision::Allow,
            },
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: tokens(&["git"]),
                decision: Decision::Prompt,
            }
        ]
    );

    let commit = tokens(&["git", "commit", "-m", "hi"]);
    let Evaluation::Match {
        decision: commit_decision,
        matched_rules: commit_matches,
    } = policy.evaluate(&commit)
    else {
        panic!("expected commit to match");
    };
    assert_eq!(commit_decision, Decision::Forbidden);
    assert_eq!(
        commit_matches,
        vec![
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: tokens(&["git"]),
                decision: Decision::Prompt,
            },
            RuleMatch {
                rule_id: "forbid_git_commit".to_string(),
                matched_prefix: tokens(&["git", "commit"]),
                decision: Decision::Forbidden,
            }
        ]
    );
}
