use codex_execpolicy2::Decision;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::RuleMatch;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(|token| token.to_string()).collect()
}

#[test]
fn matches_default_git_status() {
    let policy = codex_execpolicy2::load_default_policy().expect("parse");
    let cmd = tokens(&["git", "status"]);
    let eval = policy.evaluate(&cmd).expect("match");
    assert_eq!(eval.decision, Decision::Allow);
    assert_eq!(
        eval.matched_rules,
        vec![RuleMatch {
            rule_id: "git_status".to_string(),
            matched_prefix: vec!["git".to_string(), "status".to_string()],
            decision: Decision::Allow,
        }]
    );
}

#[test]
fn pattern_expands_alternatives() {
    let policy_src = r#"
prefix_rule(
    id = "npm_install",
    pattern = ["npm", ["i", "install"]],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");

    for (cmd, matched_prefix) in [
        (tokens(&["npm", "i"]), tokens(&["npm", "i"])),
        (tokens(&["npm", "install"]), tokens(&["npm", "install"])),
    ] {
        let eval = policy.evaluate(&cmd).expect("match");
        assert_eq!(
            eval.matched_rules,
            vec![RuleMatch {
                rule_id: "npm_install".to_string(),
                matched_prefix,
                decision: Decision::Allow,
            }]
        );
    }

    let no_match = tokens(&["npmx", "install"]);
    assert!(policy.evaluate(&no_match).is_none());
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
    assert!(policy.evaluate(&tokens(&["git", "status"])).is_some());
    assert!(
        policy
            .evaluate(&tokens(&["git", "reset", "--hard"]))
            .is_none()
    );
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
    let status_eval = policy.evaluate(&status).expect("match");
    assert_eq!(status_eval.decision, Decision::Prompt);
    assert_eq!(
        status_eval.matched_rules,
        vec![
            RuleMatch {
                rule_id: "allow_git_status".to_string(),
                matched_prefix: vec!["git".to_string(), "status".to_string()],
                decision: Decision::Allow,
            },
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: vec!["git".to_string()],
                decision: Decision::Prompt,
            }
        ]
    );

    let commit = tokens(&["git", "commit", "-m", "hi"]);
    let commit_eval = policy.evaluate(&commit).expect("match");
    assert_eq!(commit_eval.decision, Decision::Forbidden);
    assert_eq!(
        commit_eval.matched_rules,
        vec![
            RuleMatch {
                rule_id: "prompt_git".to_string(),
                matched_prefix: vec!["git".to_string()],
                decision: Decision::Prompt,
            },
            RuleMatch {
                rule_id: "forbid_git_commit".to_string(),
                matched_prefix: vec!["git".to_string(), "commit".to_string()],
                decision: Decision::Forbidden,
            }
        ]
    );
}

#[test]
fn unnamed_rule_uses_source_as_name() {
    let policy_src = r#"
prefix_rule(
    id = "unnamed_rule",
    pattern = ["echo"],
)
    "#;
    let parser = PolicyParser::new("test.policy", policy_src);
    let policy = parser.parse().expect("parse policy");
    let eval = policy.evaluate(&tokens(&["echo", "hi"])).expect("match");
    assert_eq!(
        eval.matched_rules,
        vec![RuleMatch {
            rule_id: "unnamed_rule".to_string(),
            matched_prefix: vec!["echo".to_string()],
            decision: Decision::Allow,
        }]
    );
}
