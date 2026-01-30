use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_execpolicy::rule::PatternToken;
use codex_execpolicy::rule::PrefixPattern;
use codex_execpolicy::rule::PrefixRule;
use codex_execpolicy::rule::RuleRef;
use multimap::MultiMap;
use serde::Deserialize;
use std::sync::Arc;
use thiserror::Error;

/// TOML types for expressing exec policy requirements.
///
/// These types are kept separate from `ConfigRequirementsToml` and are
/// converted into `codex-execpolicy` rules.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequirementsExecPolicyTomlRoot {
    pub exec_policy: RequirementsExecPolicyToml,
}

/// TOML representation of `[exec_policy]` within `requirements.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequirementsExecPolicyToml {
    pub prefix_rules: Vec<RequirementsExecPolicyPrefixRuleToml>,
}

/// A TOML representation of the `prefix_rule(...)` Starlark builtin.
///
/// This mirrors the builtin defined in `execpolicy/src/parser.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequirementsExecPolicyPrefixRuleToml {
    pub pattern: Vec<RequirementsExecPolicyPatternTokenToml>,
    pub decision: Option<RequirementsExecPolicyDecisionToml>,
    pub justification: Option<String>,
}

/// TOML-friendly representation of a pattern token.
///
/// Starlark supports either a string token or a list of alternative tokens at
/// each position, but TOML arrays cannot mix strings and arrays. Using an
/// array of tables sidesteps that restriction.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RequirementsExecPolicyPatternTokenToml {
    pub token: Option<String>,
    pub any_of: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequirementsExecPolicyDecisionToml {
    Allow,
    Prompt,
    Forbidden,
}

impl RequirementsExecPolicyDecisionToml {
    fn as_decision(self) -> Decision {
        match self {
            Self::Allow => Decision::Allow,
            Self::Prompt => Decision::Prompt,
            Self::Forbidden => Decision::Forbidden,
        }
    }
}

#[derive(Debug, Error)]
pub enum RequirementsExecPolicyParseError {
    #[error("exec policy prefix_rules cannot be empty")]
    EmptyPrefixRules,

    #[error("exec policy prefix_rule at index {rule_index} has an empty pattern")]
    EmptyPattern { rule_index: usize },

    #[error(
        "exec policy prefix_rule at index {rule_index} has an invalid pattern token at index {token_index}: {reason}"
    )]
    InvalidPatternToken {
        rule_index: usize,
        token_index: usize,
        reason: String,
    },

    #[error("exec policy prefix_rule at index {rule_index} has an empty justification")]
    EmptyJustification { rule_index: usize },
}

impl RequirementsExecPolicyToml {
    /// Convert requirements TOML exec policy rules into the internal `.rules`
    /// representation used by `codex-execpolicy`.
    pub fn to_policy(&self) -> Result<Policy, RequirementsExecPolicyParseError> {
        if self.prefix_rules.is_empty() {
            return Err(RequirementsExecPolicyParseError::EmptyPrefixRules);
        }

        let mut rules_by_program: MultiMap<String, RuleRef> = MultiMap::new();

        for (rule_index, rule) in self.prefix_rules.iter().enumerate() {
            if let Some(justification) = &rule.justification
                && justification.trim().is_empty()
            {
                return Err(RequirementsExecPolicyParseError::EmptyJustification { rule_index });
            }

            if rule.pattern.is_empty() {
                return Err(RequirementsExecPolicyParseError::EmptyPattern { rule_index });
            }

            let pattern_tokens = rule
                .pattern
                .iter()
                .enumerate()
                .map(|(token_index, token)| parse_pattern_token(token, rule_index, token_index))
                .collect::<Result<Vec<_>, _>>()?;

            let decision = rule
                .decision
                .map(RequirementsExecPolicyDecisionToml::as_decision)
                .unwrap_or(Decision::Allow);
            let justification = rule.justification.clone();

            let (first_token, remaining_tokens) = pattern_tokens
                .split_first()
                .ok_or(RequirementsExecPolicyParseError::EmptyPattern { rule_index })?;

            let rest: Arc<[PatternToken]> = remaining_tokens.to_vec().into();

            for head in first_token.alternatives() {
                let rule: RuleRef = Arc::new(PrefixRule {
                    pattern: PrefixPattern {
                        first: Arc::from(head.as_str()),
                        rest: rest.clone(),
                    },
                    decision,
                    justification: justification.clone(),
                });
                rules_by_program.insert(head.clone(), rule);
            }
        }

        Ok(Policy::new(rules_by_program))
    }
}

fn parse_pattern_token(
    token: &RequirementsExecPolicyPatternTokenToml,
    rule_index: usize,
    token_index: usize,
) -> Result<PatternToken, RequirementsExecPolicyParseError> {
    match (&token.token, &token.any_of) {
        (Some(single), None) => {
            if single.trim().is_empty() {
                return Err(RequirementsExecPolicyParseError::InvalidPatternToken {
                    rule_index,
                    token_index,
                    reason: "token cannot be empty".to_string(),
                });
            }
            Ok(PatternToken::Single(single.clone()))
        }
        (None, Some(alternatives)) => {
            if alternatives.is_empty() {
                return Err(RequirementsExecPolicyParseError::InvalidPatternToken {
                    rule_index,
                    token_index,
                    reason: "any_of cannot be empty".to_string(),
                });
            }
            if alternatives.iter().any(|alt| alt.trim().is_empty()) {
                return Err(RequirementsExecPolicyParseError::InvalidPatternToken {
                    rule_index,
                    token_index,
                    reason: "any_of cannot include empty tokens".to_string(),
                });
            }
            Ok(PatternToken::Alts(alternatives.clone()))
        }
        (Some(_), Some(_)) => Err(RequirementsExecPolicyParseError::InvalidPatternToken {
            rule_index,
            token_index,
            reason: "set either token or any_of, not both".to_string(),
        }),
        (None, None) => Err(RequirementsExecPolicyParseError::InvalidPatternToken {
            rule_index,
            token_index,
            reason: "set either token or any_of".to_string(),
        }),
    }
}
