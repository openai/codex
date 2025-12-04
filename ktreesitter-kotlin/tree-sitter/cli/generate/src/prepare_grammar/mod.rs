mod expand_repeats;
mod expand_tokens;
mod extract_default_aliases;
mod extract_tokens;
mod flatten_grammar;
mod intern_symbols;
mod process_inlines;

use std::{
    cmp::Ordering,
    collections::{hash_map, HashMap, HashSet},
    mem,
};

use anyhow::Result;
pub use expand_tokens::ExpandTokensError;
pub use extract_tokens::ExtractTokensError;
pub use flatten_grammar::FlattenGrammarError;
pub use intern_symbols::InternSymbolsError;
pub use process_inlines::ProcessInlinesError;
use serde::Serialize;
use thiserror::Error;

pub use self::expand_tokens::expand_tokens;
use self::{
    expand_repeats::expand_repeats, extract_default_aliases::extract_default_aliases,
    extract_tokens::extract_tokens, flatten_grammar::flatten_grammar,
    intern_symbols::intern_symbols, process_inlines::process_inlines,
};
use super::{
    grammars::{
        ExternalToken, InlinedProductionMap, InputGrammar, LexicalGrammar, PrecedenceEntry,
        SyntaxGrammar, Variable,
    },
    rules::{AliasMap, Precedence, Rule, Symbol},
};
use crate::grammars::ReservedWordContext;

pub struct IntermediateGrammar<T, U> {
    variables: Vec<Variable>,
    extra_symbols: Vec<T>,
    expected_conflicts: Vec<Vec<Symbol>>,
    precedence_orderings: Vec<Vec<PrecedenceEntry>>,
    external_tokens: Vec<U>,
    variables_to_inline: Vec<Symbol>,
    supertype_symbols: Vec<Symbol>,
    word_token: Option<Symbol>,
    reserved_word_sets: Vec<ReservedWordContext<T>>,
}

pub type InternedGrammar = IntermediateGrammar<Rule, Variable>;

pub type ExtractedSyntaxGrammar = IntermediateGrammar<Symbol, ExternalToken>;

#[derive(Debug, PartialEq, Eq)]
pub struct ExtractedLexicalGrammar {
    pub variables: Vec<Variable>,
    pub separators: Vec<Rule>,
}

impl<T, U> Default for IntermediateGrammar<T, U> {
    fn default() -> Self {
        Self {
            variables: Vec::default(),
            extra_symbols: Vec::default(),
            expected_conflicts: Vec::default(),
            precedence_orderings: Vec::default(),
            external_tokens: Vec::default(),
            variables_to_inline: Vec::default(),
            supertype_symbols: Vec::default(),
            word_token: Option::default(),
            reserved_word_sets: Vec::default(),
        }
    }
}

pub type PrepareGrammarResult<T> = Result<T, PrepareGrammarError>;

#[derive(Debug, Error, Serialize)]
#[error(transparent)]
pub enum PrepareGrammarError {
    ValidatePrecedences(#[from] ValidatePrecedenceError),
    InternSymbols(#[from] InternSymbolsError),
    ExtractTokens(#[from] ExtractTokensError),
    FlattenGrammar(#[from] FlattenGrammarError),
    ExpandTokens(#[from] ExpandTokensError),
    ProcessInlines(#[from] ProcessInlinesError),
}

pub type ValidatePrecedenceResult<T> = Result<T, ValidatePrecedenceError>;

#[derive(Debug, Error, Serialize)]
#[error(transparent)]
pub enum ValidatePrecedenceError {
    Undeclared(#[from] UndeclaredPrecedenceError),
    Ordering(#[from] ConflictingPrecedenceOrderingError),
}

#[derive(Debug, Error, Serialize)]
pub struct UndeclaredPrecedenceError {
    pub precedence: String,
    pub rule: String,
}

impl std::fmt::Display for UndeclaredPrecedenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Undeclared precedence '{}' in rule '{}'",
            self.precedence, self.rule
        )?;
        Ok(())
    }
}

#[derive(Debug, Error, Serialize)]
pub struct ConflictingPrecedenceOrderingError {
    pub precedence_1: String,
    pub precedence_2: String,
}

impl std::fmt::Display for ConflictingPrecedenceOrderingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Conflicting orderings for precedences {} and {}",
            self.precedence_1, self.precedence_2
        )?;
        Ok(())
    }
}

/// Transform an input grammar into separate components that are ready
/// for parse table construction.
pub fn prepare_grammar(
    input_grammar: &InputGrammar,
) -> PrepareGrammarResult<(
    SyntaxGrammar,
    LexicalGrammar,
    InlinedProductionMap,
    AliasMap,
)> {
    validate_precedences(input_grammar)?;

    let interned_grammar = intern_symbols(input_grammar)?;
    let (syntax_grammar, lexical_grammar) = extract_tokens(interned_grammar)?;
    let syntax_grammar = expand_repeats(syntax_grammar);
    let mut syntax_grammar = flatten_grammar(syntax_grammar)?;
    let lexical_grammar = expand_tokens(lexical_grammar)?;
    let default_aliases = extract_default_aliases(&mut syntax_grammar, &lexical_grammar);
    let inlines = process_inlines(&syntax_grammar, &lexical_grammar)?;
    Ok((syntax_grammar, lexical_grammar, inlines, default_aliases))
}

/// Check that all of the named precedences used in the grammar are declared
/// within the `precedences` lists, and also that there are no conflicting
/// precedence orderings declared in those lists.
fn validate_precedences(grammar: &InputGrammar) -> ValidatePrecedenceResult<()> {
    // Check that no rule contains a named precedence that is not present in
    // any of the `precedences` lists.
    fn validate(
        rule_name: &str,
        rule: &Rule,
        names: &HashSet<&String>,
    ) -> ValidatePrecedenceResult<()> {
        match rule {
            Rule::Repeat(rule) => validate(rule_name, rule, names),
            Rule::Seq(elements) | Rule::Choice(elements) => elements
                .iter()
                .try_for_each(|e| validate(rule_name, e, names)),
            Rule::Metadata { rule, params } => {
                if let Precedence::Name(n) = &params.precedence {
                    if !names.contains(n) {
                        Err(UndeclaredPrecedenceError {
                            precedence: n.to_string(),
                            rule: rule_name.to_string(),
                        })?;
                    }
                }
                validate(rule_name, rule, names)?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    // For any two precedence names `a` and `b`, if `a` comes before `b`
    // in some list, then it cannot come *after* `b` in any list.
    let mut pairs = HashMap::new();
    for list in &grammar.precedence_orderings {
        for (i, mut entry1) in list.iter().enumerate() {
            for mut entry2 in list.iter().skip(i + 1) {
                if entry2 == entry1 {
                    continue;
                }
                let mut ordering = Ordering::Greater;
                if entry1 > entry2 {
                    ordering = Ordering::Less;
                    mem::swap(&mut entry1, &mut entry2);
                }
                match pairs.entry((entry1, entry2)) {
                    hash_map::Entry::Vacant(e) => {
                        e.insert(ordering);
                    }
                    hash_map::Entry::Occupied(e) => {
                        if e.get() != &ordering {
                            Err(ConflictingPrecedenceOrderingError {
                                precedence_1: entry1.to_string(),
                                precedence_2: entry2.to_string(),
                            })?;
                        }
                    }
                }
            }
        }
    }

    let precedence_names = grammar
        .precedence_orderings
        .iter()
        .flat_map(|l| l.iter())
        .filter_map(|p| {
            if let PrecedenceEntry::Name(n) = p {
                Some(n)
            } else {
                None
            }
        })
        .collect::<HashSet<&String>>();
    for variable in &grammar.variables {
        validate(&variable.name, &variable.rule, &precedence_names)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammars::VariableType;

    #[test]
    fn test_validate_precedences_with_undeclared_precedence() {
        let grammar = InputGrammar {
            precedence_orderings: vec![
                vec![
                    PrecedenceEntry::Name("a".to_string()),
                    PrecedenceEntry::Name("b".to_string()),
                ],
                vec![
                    PrecedenceEntry::Name("b".to_string()),
                    PrecedenceEntry::Name("c".to_string()),
                    PrecedenceEntry::Name("d".to_string()),
                ],
            ],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::Seq(vec![
                        Rule::prec_left(Precedence::Name("b".to_string()), Rule::string("w")),
                        Rule::prec(Precedence::Name("c".to_string()), Rule::string("x")),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::repeat(Rule::Choice(vec![
                        Rule::prec_left(Precedence::Name("omg".to_string()), Rule::string("y")),
                        Rule::prec(Precedence::Name("c".to_string()), Rule::string("z")),
                    ])),
                },
            ],
            ..Default::default()
        };

        let result = validate_precedences(&grammar);
        assert_eq!(
            result.unwrap_err().to_string(),
            "Undeclared precedence 'omg' in rule 'v2'",
        );
    }

    #[test]
    fn test_validate_precedences_with_conflicting_order() {
        let grammar = InputGrammar {
            precedence_orderings: vec![
                vec![
                    PrecedenceEntry::Name("a".to_string()),
                    PrecedenceEntry::Name("b".to_string()),
                ],
                vec![
                    PrecedenceEntry::Name("b".to_string()),
                    PrecedenceEntry::Name("c".to_string()),
                    PrecedenceEntry::Name("a".to_string()),
                ],
            ],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::Seq(vec![
                        Rule::prec_left(Precedence::Name("b".to_string()), Rule::string("w")),
                        Rule::prec(Precedence::Name("c".to_string()), Rule::string("x")),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::repeat(Rule::Choice(vec![
                        Rule::prec_left(Precedence::Name("a".to_string()), Rule::string("y")),
                        Rule::prec(Precedence::Name("c".to_string()), Rule::string("z")),
                    ])),
                },
            ],
            ..Default::default()
        };

        let result = validate_precedences(&grammar);
        assert_eq!(
            result.unwrap_err().to_string(),
            "Conflicting orderings for precedences 'a' and 'b'",
        );
    }
}
