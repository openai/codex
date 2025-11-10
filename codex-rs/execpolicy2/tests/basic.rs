use codex_execpolicy2::Decision;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::RuleMatch;
use codex_execpolicy2::rule::PatternToken;

fn tokens(cmd: &[&str]) -> Vec<String> {
    cmd.iter().map(|token| token.to_string()).collect()
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
    let eval = policy.evaluate(&cmd).expect("match");
    assert_eq!(eval.decision, Decision::Allow);
    assert_eq!(
        eval.matched_rules,
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
        let eval = policy.evaluate(&cmd).expect("match");
        assert_eq!(eval.matched_rules[0].matched_prefix, prefix);
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
        assert!(policy.evaluate(&cmd).is_some());
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
    let commit_eval = policy.evaluate(&commit).expect("match");
    assert_eq!(commit_eval.decision, Decision::Forbidden);
    assert_eq!(
        commit_eval.matched_rules,
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
