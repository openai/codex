use std::collections::HashSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use super::{
    grammars::{InputGrammar, PrecedenceEntry, Variable, VariableType},
    rules::{Precedence, Rule},
};
use crate::grammars::ReservedWordContext;

#[derive(Deserialize)]
#[serde(tag = "type")]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
enum RuleJSON {
    ALIAS {
        content: Box<RuleJSON>,
        named: bool,
        value: String,
    },
    BLANK,
    STRING {
        value: String,
    },
    PATTERN {
        value: String,
        flags: Option<String>,
    },
    SYMBOL {
        name: String,
    },
    CHOICE {
        members: Vec<RuleJSON>,
    },
    FIELD {
        name: String,
        content: Box<RuleJSON>,
    },
    SEQ {
        members: Vec<RuleJSON>,
    },
    REPEAT {
        content: Box<RuleJSON>,
    },
    REPEAT1 {
        content: Box<RuleJSON>,
    },
    PREC_DYNAMIC {
        value: i32,
        content: Box<RuleJSON>,
    },
    PREC_LEFT {
        value: PrecedenceValueJSON,
        content: Box<RuleJSON>,
    },
    PREC_RIGHT {
        value: PrecedenceValueJSON,
        content: Box<RuleJSON>,
    },
    PREC {
        value: PrecedenceValueJSON,
        content: Box<RuleJSON>,
    },
    TOKEN {
        content: Box<RuleJSON>,
    },
    IMMEDIATE_TOKEN {
        content: Box<RuleJSON>,
    },
    RESERVED {
        context_name: String,
        content: Box<RuleJSON>,
    },
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PrecedenceValueJSON {
    Integer(i32),
    Name(String),
}

#[derive(Deserialize)]
pub struct GrammarJSON {
    pub name: String,
    rules: Map<String, Value>,
    #[serde(default)]
    precedences: Vec<Vec<RuleJSON>>,
    #[serde(default)]
    conflicts: Vec<Vec<String>>,
    #[serde(default)]
    externals: Vec<RuleJSON>,
    #[serde(default)]
    extras: Vec<RuleJSON>,
    #[serde(default)]
    inline: Vec<String>,
    #[serde(default)]
    supertypes: Vec<String>,
    #[serde(default)]
    word: Option<String>,
    #[serde(default)]
    reserved: Map<String, Value>,
}

pub type ParseGrammarResult<T> = Result<T, ParseGrammarError>;

#[derive(Debug, Error, Serialize)]
pub enum ParseGrammarError {
    #[error("{0}")]
    Serialization(String),
    #[error("Rules in the `extras` array must not contain empty strings")]
    InvalidExtra,
    #[error("Invalid rule in precedences array. Only strings and symbols are allowed")]
    Unexpected,
    #[error("Reserved word sets must be arrays")]
    InvalidReservedWordSet,
    #[error("Grammar Error: Unexpected rule `{0}` in `token()` call")]
    UnexpectedRule(String),
}

impl From<serde_json::Error> for ParseGrammarError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}

/// Check if a rule is referenced by another rule.
///
/// This function is used to determine if a variable is used in a given rule,
/// and `is_other` indicates if the rule is an external, and if it is,
/// to not assume that a named symbol that is equal to itself means it's being referenced.
///
/// For example, if we have an external rule **and** a normal rule both called `foo`,
/// `foo` should not be thought of as directly used unless it's used within another rule.
fn rule_is_referenced(rule: &Rule, target: &str, is_external: bool) -> bool {
    match rule {
        Rule::NamedSymbol(name) => name == target && !is_external,
        Rule::Choice(rules) | Rule::Seq(rules) => {
            rules.iter().any(|r| rule_is_referenced(r, target, false))
        }
        Rule::Metadata { rule, .. } | Rule::Reserved { rule, .. } => {
            rule_is_referenced(rule, target, is_external)
        }
        Rule::Repeat(inner) => rule_is_referenced(inner, target, false),
        Rule::Blank | Rule::String(_) | Rule::Pattern(_, _) | Rule::Symbol(_) => false,
    }
}

fn variable_is_used(
    grammar_rules: &[(String, Rule)],
    extras: &[Rule],
    externals: &[Rule],
    target_name: &str,
    in_progress: &mut HashSet<String>,
) -> bool {
    let root = &grammar_rules.first().unwrap().0;
    if target_name == root {
        return true;
    }

    if extras
        .iter()
        .any(|rule| rule_is_referenced(rule, target_name, false))
    {
        return true;
    }

    if externals
        .iter()
        .any(|rule| rule_is_referenced(rule, target_name, true))
    {
        return true;
    }

    in_progress.insert(target_name.to_string());
    let result = grammar_rules
        .iter()
        .filter(|(key, _)| *key != target_name)
        .any(|(name, rule)| {
            if !rule_is_referenced(rule, target_name, false) || in_progress.contains(name) {
                return false;
            }
            variable_is_used(grammar_rules, extras, externals, name, in_progress)
        });
    in_progress.remove(target_name);

    result
}

pub(crate) fn parse_grammar(input: &str) -> ParseGrammarResult<InputGrammar> {
    let mut grammar_json = serde_json::from_str::<GrammarJSON>(input)?;

    let mut extra_symbols =
        grammar_json
            .extras
            .into_iter()
            .try_fold(Vec::<Rule>::new(), |mut acc, item| {
                let rule = parse_rule(item, false)?;
                if let Rule::String(ref value) = rule {
                    if value.is_empty() {
                        Err(ParseGrammarError::InvalidExtra)?;
                    }
                }
                acc.push(rule);
                ParseGrammarResult::Ok(acc)
            })?;

    let mut external_tokens = grammar_json
        .externals
        .into_iter()
        .map(|e| parse_rule(e, false))
        .collect::<ParseGrammarResult<Vec<_>>>()?;

    let mut precedence_orderings = Vec::with_capacity(grammar_json.precedences.len());
    for list in grammar_json.precedences {
        let mut ordering = Vec::with_capacity(list.len());
        for entry in list {
            ordering.push(match entry {
                RuleJSON::STRING { value } => PrecedenceEntry::Name(value),
                RuleJSON::SYMBOL { name } => PrecedenceEntry::Symbol(name),
                _ => Err(ParseGrammarError::Unexpected)?,
            });
        }
        precedence_orderings.push(ordering);
    }

    let mut variables = Vec::with_capacity(grammar_json.rules.len());

    let rules = grammar_json
        .rules
        .into_iter()
        .map(|(n, r)| Ok((n, parse_rule(serde_json::from_value(r)?, false)?)))
        .collect::<ParseGrammarResult<Vec<_>>>()?;

    let mut in_progress = HashSet::new();

    for (name, rule) in &rules {
        if grammar_json.word.as_ref().is_none_or(|w| w != name)
            && !variable_is_used(
                &rules,
                &extra_symbols,
                &external_tokens,
                name,
                &mut in_progress,
            )
        {
            grammar_json.conflicts.retain(|r| !r.contains(name));
            grammar_json.supertypes.retain(|r| r != name);
            grammar_json.inline.retain(|r| r != name);
            extra_symbols.retain(|r| !rule_is_referenced(r, name, true));
            external_tokens.retain(|r| !rule_is_referenced(r, name, true));
            precedence_orderings.retain(|r| {
                !r.iter().any(|e| {
                    let PrecedenceEntry::Symbol(s) = e else {
                        return false;
                    };
                    s == name
                })
            });
            continue;
        }
        variables.push(Variable {
            name: name.clone(),
            kind: VariableType::Named,
            rule: rule.clone(),
        });
    }

    let reserved_words = grammar_json
        .reserved
        .into_iter()
        .map(|(name, rule_values)| {
            let Value::Array(rule_values) = rule_values else {
                Err(ParseGrammarError::InvalidReservedWordSet)?
            };

            let mut reserved_words = Vec::with_capacity(rule_values.len());
            for value in rule_values {
                reserved_words.push(parse_rule(serde_json::from_value(value)?, false)?);
            }
            Ok(ReservedWordContext {
                name,
                reserved_words,
            })
        })
        .collect::<ParseGrammarResult<Vec<_>>>()?;

    Ok(InputGrammar {
        name: grammar_json.name,
        word_token: grammar_json.word,
        expected_conflicts: grammar_json.conflicts,
        supertype_symbols: grammar_json.supertypes,
        variables_to_inline: grammar_json.inline,
        precedence_orderings,
        variables,
        extra_symbols,
        external_tokens,
        reserved_words,
    })
}

fn parse_rule(json: RuleJSON, is_token: bool) -> ParseGrammarResult<Rule> {
    match json {
        RuleJSON::ALIAS {
            content,
            value,
            named,
        } => parse_rule(*content, is_token).map(|r| Rule::alias(r, value, named)),
        RuleJSON::BLANK => Ok(Rule::Blank),
        RuleJSON::STRING { value } => Ok(Rule::String(value)),
        RuleJSON::PATTERN { value, flags } => Ok(Rule::Pattern(
            value,
            flags.map_or(String::new(), |f| {
                f.matches(|c| {
                    if c == 'i' {
                        true
                    } else {
                        // silently ignore unicode flags
                        if c != 'u' && c != 'v' {
                            eprintln!("Warning: unsupported flag {c}");
                        }
                        false
                    }
                })
                .collect()
            }),
        )),
        RuleJSON::SYMBOL { name } => {
            if is_token {
                Err(ParseGrammarError::UnexpectedRule(name))?
            } else {
                Ok(Rule::NamedSymbol(name))
            }
        }
        RuleJSON::CHOICE { members } => members
            .into_iter()
            .map(|m| parse_rule(m, is_token))
            .collect::<ParseGrammarResult<Vec<_>>>()
            .map(Rule::choice),
        RuleJSON::FIELD { content, name } => {
            parse_rule(*content, is_token).map(|r| Rule::field(name, r))
        }
        RuleJSON::SEQ { members } => members
            .into_iter()
            .map(|m| parse_rule(m, is_token))
            .collect::<ParseGrammarResult<Vec<_>>>()
            .map(Rule::seq),
        RuleJSON::REPEAT1 { content } => parse_rule(*content, is_token).map(Rule::repeat),
        RuleJSON::REPEAT { content } => {
            parse_rule(*content, is_token).map(|m| Rule::choice(vec![Rule::repeat(m), Rule::Blank]))
        }
        RuleJSON::PREC { value, content } => {
            parse_rule(*content, is_token).map(|r| Rule::prec(value.into(), r))
        }
        RuleJSON::PREC_LEFT { value, content } => {
            parse_rule(*content, is_token).map(|r| Rule::prec_left(value.into(), r))
        }
        RuleJSON::PREC_RIGHT { value, content } => {
            parse_rule(*content, is_token).map(|r| Rule::prec_right(value.into(), r))
        }
        RuleJSON::PREC_DYNAMIC { value, content } => {
            parse_rule(*content, is_token).map(|r| Rule::prec_dynamic(value, r))
        }
        RuleJSON::RESERVED {
            content,
            context_name,
        } => parse_rule(*content, is_token).map(|r| Rule::Reserved {
            rule: Box::new(r),
            context_name,
        }),
        RuleJSON::TOKEN { content } => parse_rule(*content, true).map(Rule::token),
        RuleJSON::IMMEDIATE_TOKEN { content } => {
            parse_rule(*content, is_token).map(Rule::immediate_token)
        }
    }
}

impl From<PrecedenceValueJSON> for Precedence {
    fn from(val: PrecedenceValueJSON) -> Self {
        match val {
            PrecedenceValueJSON::Integer(i) => Self::Integer(i),
            PrecedenceValueJSON::Name(i) => Self::Name(i),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_grammar() {
        let grammar = parse_grammar(
            r#"{
            "name": "my_lang",
            "rules": {
                "file": {
                    "type": "REPEAT1",
                    "content": {
                        "type": "SYMBOL",
                        "name": "statement"
                    }
                },
                "statement": {
                    "type": "STRING",
                    "value": "foo"
                }
            }
        }"#,
        )
        .unwrap();

        assert_eq!(grammar.name, "my_lang");
        assert_eq!(
            grammar.variables,
            vec![
                Variable {
                    name: "file".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::repeat(Rule::NamedSymbol("statement".to_string()))
                },
                Variable {
                    name: "statement".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::String("foo".to_string())
                },
            ]
        );
    }
}
