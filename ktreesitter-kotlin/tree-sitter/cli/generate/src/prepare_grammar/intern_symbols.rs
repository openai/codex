use anyhow::Result;
use serde::Serialize;
use thiserror::Error;

use super::InternedGrammar;
use crate::{
    grammars::{InputGrammar, ReservedWordContext, Variable, VariableType},
    rules::{Rule, Symbol},
};

pub type InternSymbolsResult<T> = Result<T, InternSymbolsError>;

#[derive(Debug, Error, Serialize)]
pub enum InternSymbolsError {
    #[error("A grammar's start rule must be visible.")]
    HiddenStartRule,
    #[error("Undefined symbol `{0}`")]
    Undefined(String),
    #[error("Undefined symbol `{0}` in grammar's supertypes array")]
    UndefinedSupertype(String),
    #[error("Undefined symbol `{0}` in grammar's conflicts array")]
    UndefinedConflict(String),
    #[error("Undefined symbol `{0}` as grammar's word token")]
    UndefinedWordToken(String),
}

pub(super) fn intern_symbols(grammar: &InputGrammar) -> InternSymbolsResult<InternedGrammar> {
    let interner = Interner { grammar };

    if variable_type_for_name(&grammar.variables[0].name) == VariableType::Hidden {
        Err(InternSymbolsError::HiddenStartRule)?;
    }

    let mut variables = Vec::with_capacity(grammar.variables.len());
    for variable in &grammar.variables {
        variables.push(Variable {
            name: variable.name.clone(),
            kind: variable_type_for_name(&variable.name),
            rule: interner.intern_rule(&variable.rule, Some(&variable.name))?,
        });
    }

    let mut external_tokens = Vec::with_capacity(grammar.external_tokens.len());
    for external_token in &grammar.external_tokens {
        let rule = interner.intern_rule(external_token, None)?;
        let (name, kind) = if let Rule::NamedSymbol(name) = external_token {
            (name.clone(), variable_type_for_name(name))
        } else {
            (String::new(), VariableType::Anonymous)
        };
        external_tokens.push(Variable { name, kind, rule });
    }

    let mut extra_symbols = Vec::with_capacity(grammar.extra_symbols.len());
    for extra_token in &grammar.extra_symbols {
        extra_symbols.push(interner.intern_rule(extra_token, None)?);
    }

    let mut supertype_symbols = Vec::with_capacity(grammar.supertype_symbols.len());
    for supertype_symbol_name in &grammar.supertype_symbols {
        supertype_symbols.push(interner.intern_name(supertype_symbol_name).ok_or_else(|| {
            InternSymbolsError::UndefinedSupertype(supertype_symbol_name.clone())
        })?);
    }

    let mut reserved_words = Vec::with_capacity(grammar.reserved_words.len());
    for reserved_word_set in &grammar.reserved_words {
        let mut interned_set = Vec::with_capacity(reserved_word_set.reserved_words.len());
        for rule in &reserved_word_set.reserved_words {
            interned_set.push(interner.intern_rule(rule, None)?);
        }
        reserved_words.push(ReservedWordContext {
            name: reserved_word_set.name.clone(),
            reserved_words: interned_set,
        });
    }

    let mut expected_conflicts = Vec::with_capacity(grammar.expected_conflicts.len());
    for conflict in &grammar.expected_conflicts {
        let mut interned_conflict = Vec::with_capacity(conflict.len());
        for name in conflict {
            interned_conflict.push(
                interner
                    .intern_name(name)
                    .ok_or_else(|| InternSymbolsError::UndefinedConflict(name.clone()))?,
            );
        }
        expected_conflicts.push(interned_conflict);
    }

    let mut variables_to_inline = Vec::new();
    for name in &grammar.variables_to_inline {
        if let Some(symbol) = interner.intern_name(name) {
            variables_to_inline.push(symbol);
        }
    }

    let mut word_token = None;
    if let Some(name) = grammar.word_token.as_ref() {
        word_token = Some(
            interner
                .intern_name(name)
                .ok_or_else(|| InternSymbolsError::UndefinedWordToken(name.clone()))?,
        );
    }

    for (i, variable) in variables.iter_mut().enumerate() {
        if supertype_symbols.contains(&Symbol::non_terminal(i)) {
            variable.kind = VariableType::Hidden;
        }
    }

    Ok(InternedGrammar {
        variables,
        external_tokens,
        extra_symbols,
        expected_conflicts,
        variables_to_inline,
        supertype_symbols,
        word_token,
        precedence_orderings: grammar.precedence_orderings.clone(),
        reserved_word_sets: reserved_words,
    })
}

struct Interner<'a> {
    grammar: &'a InputGrammar,
}

impl Interner<'_> {
    fn intern_rule(&self, rule: &Rule, name: Option<&str>) -> InternSymbolsResult<Rule> {
        match rule {
            Rule::Choice(elements) => {
                self.check_single(elements, name);
                let mut result = Vec::with_capacity(elements.len());
                for element in elements {
                    result.push(self.intern_rule(element, name)?);
                }
                Ok(Rule::Choice(result))
            }
            Rule::Seq(elements) => {
                self.check_single(elements, name);
                let mut result = Vec::with_capacity(elements.len());
                for element in elements {
                    result.push(self.intern_rule(element, name)?);
                }
                Ok(Rule::Seq(result))
            }
            Rule::Repeat(content) => Ok(Rule::Repeat(Box::new(self.intern_rule(content, name)?))),
            Rule::Metadata { rule, params } => Ok(Rule::Metadata {
                rule: Box::new(self.intern_rule(rule, name)?),
                params: params.clone(),
            }),
            Rule::Reserved { rule, context_name } => Ok(Rule::Reserved {
                rule: Box::new(self.intern_rule(rule, name)?),
                context_name: context_name.clone(),
            }),
            Rule::NamedSymbol(name) => self.intern_name(name).map_or_else(
                || Err(InternSymbolsError::Undefined(name.clone())),
                |symbol| Ok(Rule::Symbol(symbol)),
            ),
            _ => Ok(rule.clone()),
        }
    }

    fn intern_name(&self, symbol: &str) -> Option<Symbol> {
        for (i, variable) in self.grammar.variables.iter().enumerate() {
            if variable.name == symbol {
                return Some(Symbol::non_terminal(i));
            }
        }

        for (i, external_token) in self.grammar.external_tokens.iter().enumerate() {
            if let Rule::NamedSymbol(name) = external_token {
                if name == symbol {
                    return Some(Symbol::external(i));
                }
            }
        }

        None
    }

    // In the case of a seq or choice rule of 1 element in a hidden rule, weird
    // inconsistent behavior with queries can occur. So we should warn the user about it.
    fn check_single(&self, elements: &[Rule], name: Option<&str>) {
        if elements.len() == 1 && matches!(elements[0], Rule::String(_) | Rule::Pattern(_, _)) {
            eprintln!(
                "Warning: rule {} contains a `seq` or `choice` rule with a single element. This is unnecessary.",
                name.unwrap_or_default()
            );
        }
    }
}

fn variable_type_for_name(name: &str) -> VariableType {
    if name.starts_with('_') {
        VariableType::Hidden
    } else {
        VariableType::Named
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_repeat_expansion() {
        let grammar = intern_symbols(&build_grammar(vec![
            Variable::named("x", Rule::choice(vec![Rule::named("y"), Rule::named("_z")])),
            Variable::named("y", Rule::named("_z")),
            Variable::named("_z", Rule::string("a")),
        ]))
        .unwrap();

        assert_eq!(
            grammar.variables,
            vec![
                Variable::named(
                    "x",
                    Rule::choice(vec![Rule::non_terminal(1), Rule::non_terminal(2),])
                ),
                Variable::named("y", Rule::non_terminal(2)),
                Variable::hidden("_z", Rule::string("a")),
            ]
        );
    }

    #[test]
    fn test_interning_external_token_names() {
        // Variable `y` is both an internal and an external token.
        // Variable `z` is just an external token.
        let mut input_grammar = build_grammar(vec![
            Variable::named(
                "w",
                Rule::choice(vec![Rule::named("x"), Rule::named("y"), Rule::named("z")]),
            ),
            Variable::named("x", Rule::string("a")),
            Variable::named("y", Rule::string("b")),
        ]);
        input_grammar
            .external_tokens
            .extend(vec![Rule::named("y"), Rule::named("z")]);

        let grammar = intern_symbols(&input_grammar).unwrap();

        // Variable `y` is referred to by its internal index.
        // Variable `z` is referred to by its external index.
        assert_eq!(
            grammar.variables,
            vec![
                Variable::named(
                    "w",
                    Rule::choice(vec![
                        Rule::non_terminal(1),
                        Rule::non_terminal(2),
                        Rule::external(1),
                    ])
                ),
                Variable::named("x", Rule::string("a")),
                Variable::named("y", Rule::string("b")),
            ]
        );

        // The external token for `y` refers back to its internal index.
        assert_eq!(
            grammar.external_tokens,
            vec![
                Variable::named("y", Rule::non_terminal(2)),
                Variable::named("z", Rule::external(1)),
            ]
        );
    }

    #[test]
    fn test_grammar_with_undefined_symbols() {
        let result = intern_symbols(&build_grammar(vec![Variable::named("x", Rule::named("y"))]));

        match result {
            Err(e) => assert_eq!(e.to_string(), "Undefined symbol `y`"),
            _ => panic!("Expected an error but got none"),
        }
    }

    fn build_grammar(variables: Vec<Variable>) -> InputGrammar {
        InputGrammar {
            variables,
            name: "the_language".to_string(),
            ..Default::default()
        }
    }
}
