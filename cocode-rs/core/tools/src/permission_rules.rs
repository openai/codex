//! Permission rule evaluation for tool calls.
//!
//! Provides [`PermissionRuleEvaluator`] which evaluates a set of rules
//! against tool calls to produce permission decisions. Rules are matched
//! by tool name pattern and optional file path glob, with priority based
//! on [`RuleSource`] and action severity (deny > ask > allow).

use std::path::Path;

use cocode_config::PermissionsConfig;
use cocode_protocol::PermissionDecision;
use cocode_protocol::RuleSource;

/// Action to take when a permission rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    /// Deny the operation.
    Deny,
    /// Ask the user for permission.
    Ask,
    /// Allow the operation.
    Allow,
}

/// A single permission rule.
#[derive(Debug, Clone)]
pub struct PermissionRule {
    /// Source of the rule (determines priority).
    pub source: RuleSource,
    /// Tool name pattern to match (e.g. `"Edit"`, `"Bash:git *"`, `"*"`).
    pub tool_pattern: String,
    /// Optional file path glob (e.g. `"*.rs"`, `"src/**/*.ts"`).
    pub file_pattern: Option<String>,
    /// Action to take when matched.
    pub action: RuleAction,
}

/// Evaluates permission rules against tool calls.
///
/// Rules are evaluated in priority order: source priority first (Session > Command > ... > User),
/// then action severity (Deny > Ask > Allow). The first matching rule wins.
#[derive(Debug, Clone, Default)]
pub struct PermissionRuleEvaluator {
    rules: Vec<PermissionRule>,
}

impl PermissionRuleEvaluator {
    /// Create an empty evaluator.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create an evaluator with pre-loaded rules.
    pub fn with_rules(rules: Vec<PermissionRule>) -> Self {
        Self { rules }
    }

    /// Add a single rule.
    pub fn add_rule(&mut self, rule: PermissionRule) {
        self.rules.push(rule);
    }

    /// Build permission rules from a `PermissionsConfig` with a given source.
    pub fn rules_from_config(
        config: &PermissionsConfig,
        source: RuleSource,
    ) -> Vec<PermissionRule> {
        let mut rules = Vec::new();
        for pattern in &config.allow {
            rules.push(PermissionRule {
                source,
                tool_pattern: pattern.clone(),
                file_pattern: None,
                action: RuleAction::Allow,
            });
        }
        for pattern in &config.deny {
            rules.push(PermissionRule {
                source,
                tool_pattern: pattern.clone(),
                file_pattern: None,
                action: RuleAction::Deny,
            });
        }
        for pattern in &config.ask {
            rules.push(PermissionRule {
                source,
                tool_pattern: pattern.clone(),
                file_pattern: None,
                action: RuleAction::Ask,
            });
        }
        rules
    }

    /// Evaluate rules for a tool call.
    ///
    /// Returns `None` if no rule matches (fall through to the tool's own check).
    pub fn evaluate(
        &self,
        tool_name: &str,
        file_path: Option<&Path>,
    ) -> Option<PermissionDecision> {
        let mut matching_rules: Vec<&PermissionRule> = self
            .rules
            .iter()
            .filter(|r| Self::matches_tool(&r.tool_pattern, tool_name))
            .filter(|r| Self::matches_file(&r.file_pattern, file_path))
            .collect();

        // Sort by source priority (lower ordinal = higher priority), then by
        // action severity (Deny=0 < Ask=1 < Allow=2, so most restrictive first).
        matching_rules.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(Self::action_priority(&a.action).cmp(&Self::action_priority(&b.action)))
        });

        matching_rules.first().map(|rule| match rule.action {
            RuleAction::Allow => PermissionDecision::allowed(format!(
                "Allowed by {source} rule for {tool_name}",
                source = rule.source
            ))
            .with_source(rule.source)
            .with_pattern(rule.tool_pattern.clone()),

            RuleAction::Deny => PermissionDecision::denied(format!(
                "Denied by {source} rule for {tool_name}",
                source = rule.source
            ))
            .with_source(rule.source)
            .with_pattern(rule.tool_pattern.clone()),

            RuleAction::Ask => {
                // Ask means "fall through to tool's own permission check".
                // We return Allowed here so the tool's check_permission() is
                // the one that decides whether to ask or allow.
                PermissionDecision::allowed(format!(
                    "Ask rule from {source} — delegating to tool check",
                    source = rule.source
                ))
                .with_source(rule.source)
                .with_pattern(rule.tool_pattern.clone())
            }
        })
    }

    /// Evaluate rules for a specific behavior (deny/ask/allow).
    ///
    /// Used by the permission pipeline for staged evaluation:
    /// 1. Check DENY rules → if match → Deny
    /// 2. Check ASK rules → if match → NeedsApproval
    /// 3. (tool-specific check)
    /// 4. Check ALLOW rules → if match → Allow
    ///
    /// `command_input` is the actual command string for Bash-type tools,
    /// used to match patterns like `"Bash:git *"` or `"Bash(npm run *)"`.
    ///
    /// Returns the highest-priority matching rule for the given action,
    /// or `None` if no rule of that action type matches.
    pub fn evaluate_behavior(
        &self,
        tool_name: &str,
        file_path: Option<&Path>,
        action: RuleAction,
        command_input: Option<&str>,
    ) -> Option<PermissionDecision> {
        self.rules
            .iter()
            .filter(|r| r.action == action)
            .filter(|r| Self::matches_tool_with_input(&r.tool_pattern, tool_name, command_input))
            .filter(|r| Self::matches_file(&r.file_pattern, file_path))
            .min_by_key(|r| r.source) // Highest-priority source wins
            .map(|rule| match rule.action {
                RuleAction::Allow => PermissionDecision::allowed(format!(
                    "Allowed by {source} rule for {tool_name}",
                    source = rule.source
                ))
                .with_source(rule.source)
                .with_pattern(rule.tool_pattern.clone()),

                RuleAction::Deny => PermissionDecision::denied(format!(
                    "Denied by {source} rule for {tool_name}",
                    source = rule.source
                ))
                .with_source(rule.source)
                .with_pattern(rule.tool_pattern.clone()),

                RuleAction::Ask => PermissionDecision::allowed(format!(
                    "Ask rule from {source} for {tool_name}",
                    source = rule.source
                ))
                .with_source(rule.source)
                .with_pattern(rule.tool_pattern.clone()),
            })
    }

    /// Check if `pattern` matches `tool_name`.
    fn matches_tool(pattern: &str, tool_name: &str) -> bool {
        Self::matches_tool_with_input(pattern, tool_name, None)
    }

    /// Check if `pattern` matches `tool_name`, optionally checking
    /// a command pattern against `command_input`.
    ///
    /// Pattern formats:
    /// - `"Bash"` → matches tool name "Bash"
    /// - `"Bash:git *"` → matches Bash tool + commands starting with "git "
    /// - `"Bash(npm run *)"` → parenthesized form, same as colon
    /// - `"*"` → matches all tools
    fn matches_tool_with_input(
        pattern: &str,
        tool_name: &str,
        command_input: Option<&str>,
    ) -> bool {
        if pattern == "*" {
            return true;
        }

        // Parse "Tool:command_pattern" or "Tool(command_pattern)" forms
        let (tool_part, cmd_pattern) = if pattern.contains(':') {
            let parts: Vec<&str> = pattern.splitn(2, ':').collect();
            (parts[0], Some(parts[1]))
        } else if pattern.ends_with(')') && pattern.contains('(') {
            let paren_idx = pattern.find('(').unwrap();
            let tool = &pattern[..paren_idx];
            let cmd = &pattern[paren_idx + 1..pattern.len() - 1];
            (tool, Some(cmd))
        } else {
            (pattern, None)
        };

        if tool_part != tool_name {
            return false;
        }

        // If there's a command pattern, check it against the input
        match (cmd_pattern, command_input) {
            (None, _) => true,       // No command pattern — tool name match is sufficient
            (Some(_), None) => true, // Has pattern but no input to check — match on tool name
            (Some(pat), Some(cmd)) => Self::matches_command_pattern(pat, cmd),
        }
    }

    /// Check if a command matches a wildcard pattern.
    ///
    /// Supports trailing `*` wildcards:
    /// - `"git *"` matches "git status", "git push"
    /// - `"npm run *"` matches "npm run test", "npm run build"
    /// - `"exact-command"` matches exactly
    fn matches_command_pattern(pattern: &str, command: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern.ends_with(" *") {
            let prefix = &pattern[..pattern.len() - 2];
            command == prefix || command.starts_with(&format!("{prefix} "))
        } else if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            command.starts_with(prefix)
        } else {
            command == pattern
        }
    }

    /// Check if `file_path` matches `pattern`.
    fn matches_file(pattern: &Option<String>, file_path: Option<&Path>) -> bool {
        match (pattern, file_path) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(pat), Some(path)) => {
                let path_str = path.to_string_lossy();
                if pat == "*" {
                    return true;
                }
                // Extension match: "*.rs"
                if pat.starts_with("*.") {
                    let ext = &pat[1..];
                    return path_str.ends_with(ext);
                }
                // Double-star glob: "src/**/*.ts"
                if pat.contains("**") {
                    let parts: Vec<&str> = pat.split("**").collect();
                    if parts.len() == 2 {
                        let prefix = parts[0].trim_end_matches('/');
                        let suffix = parts[1].trim_start_matches('/');
                        let prefix_ok = prefix.is_empty() || path_str.starts_with(prefix);
                        let suffix_ok = if suffix.is_empty() {
                            true
                        } else if suffix.starts_with("*.") {
                            // Extension glob in suffix: "*.ts" matches ".ts" extension
                            let ext = &suffix[1..]; // ".ts"
                            path_str.ends_with(ext)
                        } else {
                            path_str.ends_with(suffix)
                        };
                        return prefix_ok && suffix_ok;
                    }
                }
                // Substring match fallback.
                path_str.contains(pat)
            }
        }
    }

    /// Lower number = higher priority (more restrictive).
    fn action_priority(action: &RuleAction) -> i32 {
        match action {
            RuleAction::Deny => 0,
            RuleAction::Ask => 1,
            RuleAction::Allow => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::PermissionResult;

    // ── Tool name matching ───────────────────────────────────────────

    #[test]
    fn test_matches_tool_exact() {
        assert!(PermissionRuleEvaluator::matches_tool("Edit", "Edit"));
        assert!(!PermissionRuleEvaluator::matches_tool("Edit", "Write"));
    }

    #[test]
    fn test_matches_tool_wildcard() {
        assert!(PermissionRuleEvaluator::matches_tool("*", "Edit"));
        assert!(PermissionRuleEvaluator::matches_tool("*", "Bash"));
    }

    #[test]
    fn test_matches_tool_with_colon_prefix() {
        assert!(PermissionRuleEvaluator::matches_tool("Bash:git *", "Bash"));
        assert!(!PermissionRuleEvaluator::matches_tool("Bash:git *", "Edit"));
    }

    // ── File pattern matching ────────────────────────────────────────

    #[test]
    fn test_matches_file_none_pattern_always_matches() {
        assert!(PermissionRuleEvaluator::matches_file(
            &None,
            Some(Path::new("/foo/bar.rs"))
        ));
        assert!(PermissionRuleEvaluator::matches_file(&None, None));
    }

    #[test]
    fn test_matches_file_some_pattern_requires_path() {
        assert!(!PermissionRuleEvaluator::matches_file(
            &Some("*.rs".to_string()),
            None
        ));
    }

    #[test]
    fn test_matches_file_extension_glob() {
        let pat = Some("*.rs".to_string());
        assert!(PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("src/main.rs"))
        ));
        assert!(!PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("src/main.ts"))
        ));
    }

    #[test]
    fn test_matches_file_double_star_glob() {
        let pat = Some("src/**/*.ts".to_string());
        assert!(PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("src/components/App.ts"))
        ));
        assert!(!PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("lib/util.ts"))
        ));
    }

    #[test]
    fn test_matches_file_wildcard() {
        let pat = Some("*".to_string());
        assert!(PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("any/path.txt"))
        ));
    }

    #[test]
    fn test_matches_file_substring_fallback() {
        let pat = Some("secret".to_string());
        assert!(PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("/home/.secret/key"))
        ));
        assert!(!PermissionRuleEvaluator::matches_file(
            &pat,
            Some(Path::new("/home/public/key"))
        ));
    }

    // ── Rule priority ordering ───────────────────────────────────────

    #[test]
    fn test_deny_wins_over_allow_same_source() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![
            PermissionRule {
                source: RuleSource::Project,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Allow,
            },
            PermissionRule {
                source: RuleSource::Project,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Deny,
            },
        ]);

        let decision = evaluator.evaluate("Edit", None).expect("should match");
        assert!(decision.result.is_denied());
    }

    #[test]
    fn test_higher_priority_source_wins() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![
            PermissionRule {
                source: RuleSource::Session,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Allow,
            },
            PermissionRule {
                source: RuleSource::Policy,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Deny,
            },
        ]);

        // Session has highest priority — its Allow overrides Policy's Deny
        let decision = evaluator.evaluate("Edit", None).expect("should match");
        assert!(decision.result.is_allowed());
        assert_eq!(decision.source, Some(RuleSource::Session));
    }

    #[test]
    fn test_ask_action_returns_allowed_for_delegation() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::Project,
            tool_pattern: "Bash".to_string(),
            file_pattern: None,
            action: RuleAction::Ask,
        }]);

        let decision = evaluator.evaluate("Bash", None).expect("should match");
        // Ask delegates to the tool's own check, so we return Allowed.
        assert!(decision.result.is_allowed());
    }

    // ── Empty rules ──────────────────────────────────────────────────

    #[test]
    fn test_empty_rules_returns_none() {
        let evaluator = PermissionRuleEvaluator::new();
        assert!(evaluator.evaluate("Edit", None).is_none());
    }

    // ── Multiple rules — most restrictive wins ───────────────────────

    #[test]
    fn test_multiple_rules_most_restrictive_wins() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![
            PermissionRule {
                source: RuleSource::User,
                tool_pattern: "*".to_string(),
                file_pattern: None,
                action: RuleAction::Allow,
            },
            PermissionRule {
                source: RuleSource::Project,
                tool_pattern: "Edit".to_string(),
                file_pattern: Some("*.env".to_string()),
                action: RuleAction::Deny,
            },
        ]);

        // Edit on .env file — the Project deny rule wins over User allow.
        let decision = evaluator
            .evaluate("Edit", Some(Path::new("config/.env")))
            .expect("should match");
        assert!(decision.result.is_denied());
        assert_eq!(decision.source, Some(RuleSource::Project));

        // Edit on .rs file — only the User allow matches.
        let decision = evaluator
            .evaluate("Edit", Some(Path::new("src/main.rs")))
            .expect("should match");
        assert!(decision.result.is_allowed());
        assert_eq!(decision.source, Some(RuleSource::User));
    }

    #[test]
    fn test_non_matching_tool_skipped() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::Project,
            tool_pattern: "Edit".to_string(),
            file_pattern: None,
            action: RuleAction::Deny,
        }]);

        assert!(evaluator.evaluate("Bash", None).is_none());
    }

    #[test]
    fn test_decision_includes_metadata() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::Local,
            tool_pattern: "Write".to_string(),
            file_pattern: None,
            action: RuleAction::Allow,
        }]);

        let decision = evaluator.evaluate("Write", None).expect("should match");
        assert_eq!(decision.source, Some(RuleSource::Local));
        assert_eq!(decision.matched_pattern.as_deref(), Some("Write"));
    }

    // ── PermissionResult variant checks ──────────────────────────────

    #[test]
    fn test_deny_returns_denied_result() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::Policy,
            tool_pattern: "*".to_string(),
            file_pattern: None,
            action: RuleAction::Deny,
        }]);

        let decision = evaluator.evaluate("Bash", None).expect("should match");
        assert!(matches!(decision.result, PermissionResult::Denied { .. }));
    }

    #[test]
    fn test_allow_returns_allowed_result() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::User,
            tool_pattern: "Read".to_string(),
            file_pattern: None,
            action: RuleAction::Allow,
        }]);

        let decision = evaluator.evaluate("Read", None).expect("should match");
        assert!(matches!(decision.result, PermissionResult::Allowed));
    }

    // ── evaluate_behavior ───────────────────────────────────────────

    #[test]
    fn test_evaluate_behavior_deny_only() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![
            PermissionRule {
                source: RuleSource::Project,
                tool_pattern: "Bash".to_string(),
                file_pattern: None,
                action: RuleAction::Deny,
            },
            PermissionRule {
                source: RuleSource::User,
                tool_pattern: "Bash".to_string(),
                file_pattern: None,
                action: RuleAction::Allow,
            },
        ]);

        // Should find the deny rule
        let decision = evaluator
            .evaluate_behavior("Bash", None, RuleAction::Deny, None)
            .expect("should match deny");
        assert!(decision.result.is_denied());

        // Should find the allow rule
        let decision = evaluator
            .evaluate_behavior("Bash", None, RuleAction::Allow, None)
            .expect("should match allow");
        assert!(decision.result.is_allowed());

        // Should not find an ask rule
        assert!(
            evaluator
                .evaluate_behavior("Bash", None, RuleAction::Ask, None)
                .is_none()
        );
    }

    #[test]
    fn test_evaluate_behavior_highest_priority_source_wins() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![
            PermissionRule {
                source: RuleSource::Session,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Deny,
            },
            PermissionRule {
                source: RuleSource::Policy,
                tool_pattern: "Edit".to_string(),
                file_pattern: None,
                action: RuleAction::Deny,
            },
        ]);

        // Session has highest priority
        let decision = evaluator
            .evaluate_behavior("Edit", None, RuleAction::Deny, None)
            .expect("should match");
        assert_eq!(decision.source, Some(RuleSource::Session));
    }

    // ── Command pattern matching ────────────────────────────────────

    #[test]
    fn test_matches_command_pattern_trailing_wildcard() {
        assert!(PermissionRuleEvaluator::matches_command_pattern(
            "git *",
            "git status"
        ));
        assert!(PermissionRuleEvaluator::matches_command_pattern(
            "git *",
            "git push origin main"
        ));
        assert!(!PermissionRuleEvaluator::matches_command_pattern(
            "git *", "npm test"
        ));
    }

    #[test]
    fn test_matches_command_pattern_exact() {
        assert!(PermissionRuleEvaluator::matches_command_pattern(
            "npm test", "npm test"
        ));
        assert!(!PermissionRuleEvaluator::matches_command_pattern(
            "npm test",
            "npm run test"
        ));
    }

    #[test]
    fn test_matches_tool_parenthesized_form() {
        assert!(PermissionRuleEvaluator::matches_tool_with_input(
            "Bash(git status)",
            "Bash",
            Some("git status")
        ));
        assert!(!PermissionRuleEvaluator::matches_tool_with_input(
            "Bash(git status)",
            "Bash",
            Some("rm -rf /")
        ));
        assert!(PermissionRuleEvaluator::matches_tool_with_input(
            "Bash(npm *)",
            "Bash",
            Some("npm test")
        ));
    }

    // ── from_permissions_config ──────────────────────────────────────

    #[test]
    fn test_rules_from_config() {
        let config = cocode_config::PermissionsConfig {
            allow: vec!["Read".to_string(), "Bash(git *)".to_string()],
            deny: vec!["Bash(rm -rf *)".to_string()],
            ask: vec!["Bash(sudo *)".to_string()],
        };
        let rules = PermissionRuleEvaluator::rules_from_config(&config, RuleSource::User);
        assert_eq!(rules.len(), 4);

        // Check allow rules
        assert_eq!(rules[0].tool_pattern, "Read");
        assert_eq!(rules[0].action, RuleAction::Allow);
        assert_eq!(rules[0].source, RuleSource::User);
        assert_eq!(rules[1].tool_pattern, "Bash(git *)");
        assert_eq!(rules[1].action, RuleAction::Allow);

        // Check deny rule
        assert_eq!(rules[2].tool_pattern, "Bash(rm -rf *)");
        assert_eq!(rules[2].action, RuleAction::Deny);

        // Check ask rule
        assert_eq!(rules[3].tool_pattern, "Bash(sudo *)");
        assert_eq!(rules[3].action, RuleAction::Ask);
    }

    #[test]
    fn test_rules_from_config_integrated() {
        let config = cocode_config::PermissionsConfig {
            allow: vec!["Bash(git *)".to_string()],
            deny: vec!["Bash(rm *)".to_string()],
            ask: vec![],
        };
        let rules = PermissionRuleEvaluator::rules_from_config(&config, RuleSource::Project);
        let evaluator = PermissionRuleEvaluator::with_rules(rules);

        // "git status" should be allowed
        let decision =
            evaluator.evaluate_behavior("Bash", None, RuleAction::Allow, Some("git status"));
        assert!(decision.is_some());
        assert!(decision.unwrap().result.is_allowed());

        // "rm -rf /" should be denied
        let decision =
            evaluator.evaluate_behavior("Bash", None, RuleAction::Deny, Some("rm -rf /"));
        assert!(decision.is_some());
        assert!(decision.unwrap().result.is_denied());
    }

    #[test]
    fn test_evaluate_behavior_with_command_input() {
        let evaluator = PermissionRuleEvaluator::with_rules(vec![PermissionRule {
            source: RuleSource::Project,
            tool_pattern: "Bash:rm *".to_string(),
            file_pattern: None,
            action: RuleAction::Deny,
        }]);

        // "rm -rf /" should be denied
        let decision =
            evaluator.evaluate_behavior("Bash", None, RuleAction::Deny, Some("rm -rf /"));
        assert!(decision.is_some());
        assert!(decision.unwrap().result.is_denied());

        // "git status" should NOT be denied (pattern doesn't match)
        let decision =
            evaluator.evaluate_behavior("Bash", None, RuleAction::Deny, Some("git status"));
        assert!(decision.is_none());
    }
}
