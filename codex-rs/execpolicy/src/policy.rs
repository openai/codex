use crate::decision::Decision;
use crate::error::Error;
use crate::error::Result;
use crate::executable_name::executable_path_lookup_key;
use crate::rule::NetworkRule;
use crate::rule::NetworkRuleProtocol;
use crate::rule::PatternToken;
use crate::rule::PrefixPattern;
use crate::rule::PrefixRule;
use crate::rule::RuleMatch;
use crate::rule::RuleRef;
use crate::rule::normalize_network_rule_host;
use codex_utils_absolute_path::AbsolutePathBuf;
use multimap::MultiMap;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

type HeuristicsFallback<'a> = Option<&'a dyn Fn(&[String]) -> Decision>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MatchOptions {
    pub resolve_host_executables: bool,
}

#[derive(Clone, Debug)]
pub struct Policy {
    rules_by_program: MultiMap<String, RuleRef>,
    network_rules: Vec<NetworkRule>,
    host_executables_by_name: HashMap<String, Arc<[AbsolutePathBuf]>>,
}

impl Policy {
    pub fn new(rules_by_program: MultiMap<String, RuleRef>) -> Self {
        Self::from_parts(rules_by_program, Vec::new(), HashMap::new())
    }

    pub fn from_parts(
        rules_by_program: MultiMap<String, RuleRef>,
        network_rules: Vec<NetworkRule>,
        host_executables_by_name: HashMap<String, Arc<[AbsolutePathBuf]>>,
    ) -> Self {
        Self {
            rules_by_program,
            network_rules,
            host_executables_by_name,
        }
    }

    pub fn empty() -> Self {
        Self::new(MultiMap::new())
    }

    pub fn rules(&self) -> &MultiMap<String, RuleRef> {
        &self.rules_by_program
    }

    pub fn network_rules(&self) -> &[NetworkRule] {
        &self.network_rules
    }

    pub fn host_executables(&self) -> &HashMap<String, Arc<[AbsolutePathBuf]>> {
        &self.host_executables_by_name
    }

    pub fn get_allowed_prefixes(&self) -> Vec<Vec<String>> {
        let mut prefixes = Vec::new();

        for (_program, rules) in self.rules_by_program.iter_all() {
            for rule in rules {
                let Some(prefix_rule) = rule.as_any().downcast_ref::<PrefixRule>() else {
                    continue;
                };
                if prefix_rule.decision != Decision::Allow {
                    continue;
                }

                let mut prefix = Vec::with_capacity(prefix_rule.pattern.rest.len() + 1);
                prefix.push(prefix_rule.pattern.first.as_ref().to_string());
                prefix.extend(prefix_rule.pattern.rest.iter().map(render_pattern_token));
                prefixes.push(prefix);
            }
        }

        prefixes.sort();
        prefixes.dedup();
        prefixes
    }

    pub fn add_prefix_rule(&mut self, prefix: &[String], decision: Decision) -> Result<()> {
        let (first_token, rest) = prefix
            .split_first()
            .ok_or_else(|| Error::InvalidPattern("prefix cannot be empty".to_string()))?;

        let rule: RuleRef = Arc::new(PrefixRule {
            pattern: PrefixPattern {
                first: Arc::from(first_token.as_str()),
                rest: rest
                    .iter()
                    .map(|token| PatternToken::Single(token.clone()))
                    .collect::<Vec<_>>()
                    .into(),
            },
            decision,
            justification: None,
        });

        self.rules_by_program.insert(first_token.clone(), rule);
        Ok(())
    }

    pub fn add_network_rule(
        &mut self,
        host: &str,
        protocol: NetworkRuleProtocol,
        decision: Decision,
        justification: Option<String>,
    ) -> Result<()> {
        let host = normalize_network_rule_host(host)?;
        if let Some(raw) = justification.as_deref()
            && raw.trim().is_empty()
        {
            return Err(Error::InvalidRule(
                "justification cannot be empty".to_string(),
            ));
        }
        self.network_rules.push(NetworkRule {
            host,
            protocol,
            decision,
            justification,
        });
        Ok(())
    }

    pub fn set_host_executable_paths(&mut self, name: String, paths: Vec<AbsolutePathBuf>) {
        self.host_executables_by_name.insert(name, paths.into());
    }

    pub fn merge_overlay(&self, overlay: &Policy) -> Policy {
        let mut combined_rules = self.rules_by_program.clone();
        for (program, rules) in overlay.rules_by_program.iter_all() {
            for rule in rules {
                combined_rules.insert(program.clone(), rule.clone());
            }
        }

        let mut combined_network_rules = self.network_rules.clone();
        combined_network_rules.extend(overlay.network_rules.iter().cloned());

        let mut host_executables_by_name = self.host_executables_by_name.clone();
        host_executables_by_name.extend(
            overlay
                .host_executables_by_name
                .iter()
                .map(|(name, paths)| (name.clone(), paths.clone())),
        );

        Policy::from_parts(
            combined_rules,
            combined_network_rules,
            host_executables_by_name,
        )
    }

    pub fn compiled_network_domains(&self) -> (Vec<String>, Vec<String>) {
        let mut allowed = Vec::new();
        let mut denied = Vec::new();

        for rule in &self.network_rules {
            match rule.decision {
                Decision::Allow => {
                    denied.retain(|entry| entry != &rule.host);
                    upsert_domain(&mut allowed, &rule.host);
                }
                Decision::Forbidden => {
                    allowed.retain(|entry| entry != &rule.host);
                    upsert_domain(&mut denied, &rule.host);
                }
                Decision::Prompt => {}
            }
        }

        (allowed, denied)
    }

    pub fn check<F>(&self, cmd: &[String], heuristics_fallback: &F) -> Evaluation
    where
        F: Fn(&[String]) -> Decision,
    {
        let matched_rules = self.matches_for_command_with_options(
            cmd,
            Some(heuristics_fallback),
            &MatchOptions::default(),
        );
        Evaluation::from_matches(matched_rules)
    }

    pub fn check_with_options<F>(
        &self,
        cmd: &[String],
        heuristics_fallback: &F,
        options: &MatchOptions,
    ) -> Evaluation
    where
        F: Fn(&[String]) -> Decision,
    {
        let matched_rules =
            self.matches_for_command_with_options(cmd, Some(heuristics_fallback), options);
        Evaluation::from_matches(matched_rules)
    }

    /// Checks multiple commands and aggregates the results.
    pub fn check_multiple<Commands, F>(
        &self,
        commands: Commands,
        heuristics_fallback: &F,
    ) -> Evaluation
    where
        Commands: IntoIterator,
        Commands::Item: AsRef<[String]>,
        F: Fn(&[String]) -> Decision,
    {
        self.check_multiple_with_options(commands, heuristics_fallback, &MatchOptions::default())
    }

    pub fn check_multiple_with_options<Commands, F>(
        &self,
        commands: Commands,
        heuristics_fallback: &F,
        options: &MatchOptions,
    ) -> Evaluation
    where
        Commands: IntoIterator,
        Commands::Item: AsRef<[String]>,
        F: Fn(&[String]) -> Decision,
    {
        let matched_rules: Vec<RuleMatch> = commands
            .into_iter()
            .flat_map(|command| {
                self.matches_for_command_with_options(
                    command.as_ref(),
                    Some(heuristics_fallback),
                    options,
                )
            })
            .collect();

        Evaluation::from_matches(matched_rules)
    }

    /// Returns matching rules for the given command. If no rules match and
    /// `heuristics_fallback` is provided, returns a single
    /// `HeuristicsRuleMatch` with the decision rendered by
    /// `heuristics_fallback`.
    ///
    /// If `heuristics_fallback.is_some()`, then the returned vector is
    /// guaranteed to be non-empty.
    pub fn matches_for_command(
        &self,
        cmd: &[String],
        heuristics_fallback: HeuristicsFallback<'_>,
    ) -> Vec<RuleMatch> {
        self.matches_for_command_with_options(cmd, heuristics_fallback, &MatchOptions::default())
    }

    pub fn matches_for_command_with_options(
        &self,
        cmd: &[String],
        heuristics_fallback: HeuristicsFallback<'_>,
        options: &MatchOptions,
    ) -> Vec<RuleMatch> {
        let matched_rules = self
            .match_exact_rules(cmd)
            .filter(|matched_rules| !matched_rules.is_empty())
            .or_else(|| {
                options
                    .resolve_host_executables
                    .then(|| self.match_host_executable_rules(cmd))
                    .filter(|matched_rules| !matched_rules.is_empty())
            })
            .unwrap_or_default();

        if matched_rules.is_empty()
            && let Some(heuristics_fallback) = heuristics_fallback
        {
            vec![RuleMatch::HeuristicsRuleMatch {
                command: cmd.to_vec(),
                decision: heuristics_fallback(cmd),
            }]
        } else {
            matched_rules
        }
    }

    fn match_exact_rules(&self, cmd: &[String]) -> Option<Vec<RuleMatch>> {
        let first = cmd.first()?;
        Some(
            self.rules_by_program
                .get_vec(first)
                .map(|rules| rules.iter().filter_map(|rule| rule.matches(cmd)).collect())
                .unwrap_or_default(),
        )
    }

    fn match_host_executable_rules(&self, cmd: &[String]) -> Vec<RuleMatch> {
        let Some(first) = cmd.first() else {
            return Vec::new();
        };
        let Ok(program) = AbsolutePathBuf::try_from(first.clone()) else {
            return Vec::new();
        };
        let Some(basename) = executable_path_lookup_key(program.as_path()) else {
            return Vec::new();
        };
        let Some(rules) = self.rules_by_program.get_vec(&basename) else {
            return Vec::new();
        };
        if let Some(paths) = self.host_executables_by_name.get(&basename)
            && !paths.iter().any(|path| path == &program)
        {
            return Vec::new();
        }

        let basename_command = std::iter::once(basename)
            .chain(cmd.iter().skip(1).cloned())
            .collect::<Vec<_>>();
        rules
            .iter()
            .filter_map(|rule| rule.matches(&basename_command))
            .map(|rule_match| rule_match.with_resolved_program(&program))
            .collect()
    }
}

fn upsert_domain(entries: &mut Vec<String>, host: &str) {
    entries.retain(|entry| entry != host);
    entries.push(host.to_string());
}

fn render_pattern_token(token: &PatternToken) -> String {
    match token {
        PatternToken::Single(value) => value.clone(),
        PatternToken::Alts(alternatives) => format!("[{}]", alternatives.join("|")),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub decision: Decision,
    #[serde(rename = "matchedRules")]
    pub matched_rules: Vec<RuleMatch>,
}

impl Evaluation {
    pub fn is_match(&self) -> bool {
        self.matched_rules
            .iter()
            .any(|rule_match| !matches!(rule_match, RuleMatch::HeuristicsRuleMatch { .. }))
    }

    /// Caller is responsible for ensuring that `matched_rules` is non-empty.
    fn from_matches(matched_rules: Vec<RuleMatch>) -> Self {
        let decision = matched_rules.iter().map(RuleMatch::decision).max();
        #[expect(clippy::expect_used)]
        let decision = decision.expect("invariant failed: matched_rules must be non-empty");

        Self {
            decision,
            matched_rules,
        }
    }
}
