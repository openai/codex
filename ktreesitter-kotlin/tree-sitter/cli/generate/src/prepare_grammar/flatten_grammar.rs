use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use thiserror::Error;

use super::ExtractedSyntaxGrammar;
use crate::{
    grammars::{
        Production, ProductionStep, ReservedWordSetId, SyntaxGrammar, SyntaxVariable, Variable,
    },
    rules::{Alias, Associativity, Precedence, Rule, Symbol, TokenSet},
};

pub type FlattenGrammarResult<T> = Result<T, FlattenGrammarError>;

#[derive(Debug, Error, Serialize)]
pub enum FlattenGrammarError {
    #[error("No such reserved word set: {0}")]
    NoReservedWordSet(String),
    #[error(
        "The rule `{0}` matches the empty string.

Tree-sitter does not support syntactic rules that match the empty string
unless they are used only as the grammar's start rule.
"
    )]
    EmptyString(String),
    #[error("Rule `{0}` cannot be inlined because it contains a reference to itself")]
    RecursiveInline(String),
}

struct RuleFlattener {
    production: Production,
    reserved_word_set_ids: HashMap<String, ReservedWordSetId>,
    precedence_stack: Vec<Precedence>,
    associativity_stack: Vec<Associativity>,
    reserved_word_stack: Vec<ReservedWordSetId>,
    alias_stack: Vec<Alias>,
    field_name_stack: Vec<String>,
}

impl RuleFlattener {
    const fn new(reserved_word_set_ids: HashMap<String, ReservedWordSetId>) -> Self {
        Self {
            production: Production {
                steps: Vec::new(),
                dynamic_precedence: 0,
            },
            reserved_word_set_ids,
            precedence_stack: Vec::new(),
            associativity_stack: Vec::new(),
            reserved_word_stack: Vec::new(),
            alias_stack: Vec::new(),
            field_name_stack: Vec::new(),
        }
    }

    fn flatten_variable(&mut self, variable: Variable) -> FlattenGrammarResult<SyntaxVariable> {
        let choices = extract_choices(variable.rule);
        let mut productions = Vec::with_capacity(choices.len());
        for rule in choices {
            let production = self.flatten_rule(rule)?;
            if !productions.contains(&production) {
                productions.push(production);
            }
        }
        Ok(SyntaxVariable {
            name: variable.name,
            kind: variable.kind,
            productions,
        })
    }

    fn flatten_rule(&mut self, rule: Rule) -> FlattenGrammarResult<Production> {
        self.production = Production::default();
        self.alias_stack.clear();
        self.reserved_word_stack.clear();
        self.precedence_stack.clear();
        self.associativity_stack.clear();
        self.field_name_stack.clear();
        self.apply(rule, true)?;
        Ok(self.production.clone())
    }

    fn apply(&mut self, rule: Rule, at_end: bool) -> FlattenGrammarResult<bool> {
        match rule {
            Rule::Seq(members) => {
                let mut result = false;
                let last_index = members.len() - 1;
                for (i, member) in members.into_iter().enumerate() {
                    result |= self.apply(member, i == last_index && at_end)?;
                }
                Ok(result)
            }
            Rule::Metadata { rule, params } => {
                let mut has_precedence = false;
                if !params.precedence.is_none() {
                    has_precedence = true;
                    self.precedence_stack.push(params.precedence);
                }

                let mut has_associativity = false;
                if let Some(associativity) = params.associativity {
                    has_associativity = true;
                    self.associativity_stack.push(associativity);
                }

                let mut has_alias = false;
                if let Some(alias) = params.alias {
                    has_alias = true;
                    self.alias_stack.push(alias);
                }

                let mut has_field_name = false;
                if let Some(field_name) = params.field_name {
                    has_field_name = true;
                    self.field_name_stack.push(field_name);
                }

                if params.dynamic_precedence.abs() > self.production.dynamic_precedence.abs() {
                    self.production.dynamic_precedence = params.dynamic_precedence;
                }

                let did_push = self.apply(*rule, at_end)?;

                if has_precedence {
                    self.precedence_stack.pop();
                    if did_push && !at_end {
                        self.production.steps.last_mut().unwrap().precedence = self
                            .precedence_stack
                            .last()
                            .cloned()
                            .unwrap_or(Precedence::None);
                    }
                }

                if has_associativity {
                    self.associativity_stack.pop();
                    if did_push && !at_end {
                        self.production.steps.last_mut().unwrap().associativity =
                            self.associativity_stack.last().copied();
                    }
                }

                if has_alias {
                    self.alias_stack.pop();
                }

                if has_field_name {
                    self.field_name_stack.pop();
                }

                Ok(did_push)
            }
            Rule::Reserved { rule, context_name } => {
                self.reserved_word_stack.push(
                    self.reserved_word_set_ids
                        .get(&context_name)
                        .copied()
                        .ok_or_else(|| {
                            FlattenGrammarError::NoReservedWordSet(context_name.clone())
                        })?,
                );
                let did_push = self.apply(*rule, at_end)?;
                self.reserved_word_stack.pop();
                Ok(did_push)
            }
            Rule::Symbol(symbol) => {
                self.production.steps.push(ProductionStep {
                    symbol,
                    precedence: self
                        .precedence_stack
                        .last()
                        .cloned()
                        .unwrap_or(Precedence::None),
                    associativity: self.associativity_stack.last().copied(),
                    reserved_word_set_id: self
                        .reserved_word_stack
                        .last()
                        .copied()
                        .unwrap_or(ReservedWordSetId::default()),
                    alias: self.alias_stack.last().cloned(),
                    field_name: self.field_name_stack.last().cloned(),
                });
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

fn extract_choices(rule: Rule) -> Vec<Rule> {
    match rule {
        Rule::Seq(elements) => {
            let mut result = vec![Rule::Blank];
            for element in elements {
                let extraction = extract_choices(element);
                let mut next_result = Vec::with_capacity(result.len());
                for entry in result {
                    for extraction_entry in &extraction {
                        next_result.push(Rule::Seq(vec![entry.clone(), extraction_entry.clone()]));
                    }
                }
                result = next_result;
            }
            result
        }
        Rule::Choice(elements) => {
            let mut result = Vec::with_capacity(elements.len());
            for element in elements {
                for rule in extract_choices(element) {
                    result.push(rule);
                }
            }
            result
        }
        Rule::Metadata { rule, params } => extract_choices(*rule)
            .into_iter()
            .map(|rule| Rule::Metadata {
                rule: Box::new(rule),
                params: params.clone(),
            })
            .collect(),
        Rule::Reserved { rule, context_name } => extract_choices(*rule)
            .into_iter()
            .map(|rule| Rule::Reserved {
                rule: Box::new(rule),
                context_name: context_name.clone(),
            })
            .collect(),
        _ => vec![rule],
    }
}

fn symbol_is_used(variables: &[SyntaxVariable], symbol: Symbol) -> bool {
    for variable in variables {
        for production in &variable.productions {
            for step in &production.steps {
                if step.symbol == symbol {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn flatten_grammar(
    grammar: ExtractedSyntaxGrammar,
) -> FlattenGrammarResult<SyntaxGrammar> {
    let mut reserved_word_set_ids_by_name = HashMap::new();
    for (ix, set) in grammar.reserved_word_sets.iter().enumerate() {
        reserved_word_set_ids_by_name.insert(set.name.clone(), ReservedWordSetId(ix));
    }

    let mut flattener = RuleFlattener::new(reserved_word_set_ids_by_name);
    let variables = grammar
        .variables
        .into_iter()
        .map(|variable| flattener.flatten_variable(variable))
        .collect::<FlattenGrammarResult<Vec<_>>>()?;

    for (i, variable) in variables.iter().enumerate() {
        let symbol = Symbol::non_terminal(i);
        let used = symbol_is_used(&variables, symbol);

        for production in &variable.productions {
            if used && production.steps.is_empty() {
                Err(FlattenGrammarError::EmptyString(variable.name.clone()))?;
            }

            if grammar.variables_to_inline.contains(&symbol)
                && production.steps.iter().any(|step| step.symbol == symbol)
            {
                Err(FlattenGrammarError::RecursiveInline(variable.name.clone()))?;
            }
        }
    }
    let mut reserved_word_sets = grammar
        .reserved_word_sets
        .into_iter()
        .map(|set| set.reserved_words.into_iter().collect())
        .collect::<Vec<_>>();

    // If no default reserved word set is specified, there are no reserved words.
    if reserved_word_sets.is_empty() {
        reserved_word_sets.push(TokenSet::default());
    }

    Ok(SyntaxGrammar {
        extra_symbols: grammar.extra_symbols,
        expected_conflicts: grammar.expected_conflicts,
        variables_to_inline: grammar.variables_to_inline,
        precedence_orderings: grammar.precedence_orderings,
        external_tokens: grammar.external_tokens,
        supertype_symbols: grammar.supertype_symbols,
        word_token: grammar.word_token,
        reserved_word_sets,
        variables,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammars::VariableType;

    #[test]
    fn test_flatten_grammar() {
        let mut flattener = RuleFlattener::new(HashMap::default());
        let result = flattener
            .flatten_variable(Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::seq(vec![
                    Rule::non_terminal(1),
                    Rule::prec_left(
                        Precedence::Integer(101),
                        Rule::seq(vec![
                            Rule::non_terminal(2),
                            Rule::choice(vec![
                                Rule::prec_right(
                                    Precedence::Integer(102),
                                    Rule::seq(vec![Rule::non_terminal(3), Rule::non_terminal(4)]),
                                ),
                                Rule::non_terminal(5),
                            ]),
                            Rule::non_terminal(6),
                        ]),
                    ),
                    Rule::non_terminal(7),
                ]),
            })
            .unwrap();

        assert_eq!(
            result.productions,
            vec![
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::non_terminal(1)),
                        ProductionStep::new(Symbol::non_terminal(2))
                            .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                        ProductionStep::new(Symbol::non_terminal(3))
                            .with_prec(Precedence::Integer(102), Some(Associativity::Right)),
                        ProductionStep::new(Symbol::non_terminal(4))
                            .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                        ProductionStep::new(Symbol::non_terminal(6)),
                        ProductionStep::new(Symbol::non_terminal(7)),
                    ]
                },
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::non_terminal(1)),
                        ProductionStep::new(Symbol::non_terminal(2))
                            .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                        ProductionStep::new(Symbol::non_terminal(5))
                            .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                        ProductionStep::new(Symbol::non_terminal(6)),
                        ProductionStep::new(Symbol::non_terminal(7)),
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_flatten_grammar_with_maximum_dynamic_precedence() {
        let mut flattener = RuleFlattener::new(HashMap::default());
        let result = flattener
            .flatten_variable(Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::seq(vec![
                    Rule::non_terminal(1),
                    Rule::prec_dynamic(
                        101,
                        Rule::seq(vec![
                            Rule::non_terminal(2),
                            Rule::choice(vec![
                                Rule::prec_dynamic(
                                    102,
                                    Rule::seq(vec![Rule::non_terminal(3), Rule::non_terminal(4)]),
                                ),
                                Rule::non_terminal(5),
                            ]),
                            Rule::non_terminal(6),
                        ]),
                    ),
                    Rule::non_terminal(7),
                ]),
            })
            .unwrap();

        assert_eq!(
            result.productions,
            vec![
                Production {
                    dynamic_precedence: 102,
                    steps: vec![
                        ProductionStep::new(Symbol::non_terminal(1)),
                        ProductionStep::new(Symbol::non_terminal(2)),
                        ProductionStep::new(Symbol::non_terminal(3)),
                        ProductionStep::new(Symbol::non_terminal(4)),
                        ProductionStep::new(Symbol::non_terminal(6)),
                        ProductionStep::new(Symbol::non_terminal(7)),
                    ],
                },
                Production {
                    dynamic_precedence: 101,
                    steps: vec![
                        ProductionStep::new(Symbol::non_terminal(1)),
                        ProductionStep::new(Symbol::non_terminal(2)),
                        ProductionStep::new(Symbol::non_terminal(5)),
                        ProductionStep::new(Symbol::non_terminal(6)),
                        ProductionStep::new(Symbol::non_terminal(7)),
                    ],
                },
            ]
        );
    }

    #[test]
    fn test_flatten_grammar_with_final_precedence() {
        let mut flattener = RuleFlattener::new(HashMap::default());
        let result = flattener
            .flatten_variable(Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::prec_left(
                    Precedence::Integer(101),
                    Rule::seq(vec![Rule::non_terminal(1), Rule::non_terminal(2)]),
                ),
            })
            .unwrap();

        assert_eq!(
            result.productions,
            vec![Production {
                dynamic_precedence: 0,
                steps: vec![
                    ProductionStep::new(Symbol::non_terminal(1))
                        .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                    ProductionStep::new(Symbol::non_terminal(2))
                        .with_prec(Precedence::Integer(101), Some(Associativity::Left)),
                ]
            }]
        );

        let result = flattener
            .flatten_variable(Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::prec_left(
                    Precedence::Integer(101),
                    Rule::seq(vec![Rule::non_terminal(1)]),
                ),
            })
            .unwrap();

        assert_eq!(
            result.productions,
            vec![Production {
                dynamic_precedence: 0,
                steps: vec![ProductionStep::new(Symbol::non_terminal(1))
                    .with_prec(Precedence::Integer(101), Some(Associativity::Left)),]
            }]
        );
    }

    #[test]
    fn test_flatten_grammar_with_field_names() {
        let mut flattener = RuleFlattener::new(HashMap::default());
        let result = flattener
            .flatten_variable(Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::seq(vec![
                    Rule::field("first-thing".to_string(), Rule::terminal(1)),
                    Rule::terminal(2),
                    Rule::choice(vec![
                        Rule::Blank,
                        Rule::field("second-thing".to_string(), Rule::terminal(3)),
                    ]),
                ]),
            })
            .unwrap();

        assert_eq!(
            result.productions,
            vec![
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(1)).with_field_name("first-thing"),
                        ProductionStep::new(Symbol::terminal(2))
                    ]
                },
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(1)).with_field_name("first-thing"),
                        ProductionStep::new(Symbol::terminal(2)),
                        ProductionStep::new(Symbol::terminal(3)).with_field_name("second-thing"),
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_flatten_grammar_with_recursive_inline_variable() {
        let result = flatten_grammar(ExtractedSyntaxGrammar {
            extra_symbols: Vec::new(),
            expected_conflicts: Vec::new(),
            variables_to_inline: vec![Symbol::non_terminal(0)],
            precedence_orderings: Vec::new(),
            external_tokens: Vec::new(),
            supertype_symbols: Vec::new(),
            word_token: None,
            reserved_word_sets: Vec::new(),
            variables: vec![Variable {
                name: "test".to_string(),
                kind: VariableType::Named,
                rule: Rule::seq(vec![
                    Rule::non_terminal(0),
                    Rule::non_terminal(1),
                    Rule::non_terminal(2),
                ]),
            }],
        });

        assert_eq!(
            result.unwrap_err().to_string(),
            "Rule `test` cannot be inlined because it contains a reference to itself",
        );
    }
}
