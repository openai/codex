use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use thiserror::Error;

use crate::{
    grammars::{InlinedProductionMap, LexicalGrammar, Production, ProductionStep, SyntaxGrammar},
    rules::SymbolType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ProductionStepId {
    // A `None` value here means that the production itself was produced via inlining,
    // and is stored in the builder's `productions` vector, as opposed to being
    // stored in one of the grammar's variables.
    variable_index: Option<usize>,
    production_index: usize,
    step_index: usize,
}

struct InlinedProductionMapBuilder {
    production_indices_by_step_id: HashMap<ProductionStepId, Vec<usize>>,
    productions: Vec<Production>,
}

impl InlinedProductionMapBuilder {
    fn build(mut self, grammar: &SyntaxGrammar) -> InlinedProductionMap {
        let mut step_ids_to_process = Vec::new();
        for (variable_index, variable) in grammar.variables.iter().enumerate() {
            for production_index in 0..variable.productions.len() {
                step_ids_to_process.push(ProductionStepId {
                    variable_index: Some(variable_index),
                    production_index,
                    step_index: 0,
                });
                while !step_ids_to_process.is_empty() {
                    let mut i = 0;
                    while i < step_ids_to_process.len() {
                        let step_id = step_ids_to_process[i];
                        if let Some(step) = self.production_step_for_id(step_id, grammar) {
                            if grammar.variables_to_inline.contains(&step.symbol) {
                                let inlined_step_ids = self
                                    .inline_production_at_step(step_id, grammar)
                                    .iter()
                                    .copied()
                                    .map(|production_index| ProductionStepId {
                                        variable_index: None,
                                        production_index,
                                        step_index: step_id.step_index,
                                    });
                                step_ids_to_process.splice(i..=i, inlined_step_ids);
                            } else {
                                step_ids_to_process[i] = ProductionStepId {
                                    variable_index: step_id.variable_index,
                                    production_index: step_id.production_index,
                                    step_index: step_id.step_index + 1,
                                };
                                i += 1;
                            }
                        } else {
                            step_ids_to_process.remove(i);
                        }
                    }
                }
            }
        }

        let productions = self.productions;
        let production_indices_by_step_id = self.production_indices_by_step_id;
        let production_map = production_indices_by_step_id
            .into_iter()
            .map(|(step_id, production_indices)| {
                let production = step_id.variable_index.map_or_else(
                    || &productions[step_id.production_index],
                    |variable_index| {
                        &grammar.variables[variable_index].productions[step_id.production_index]
                    },
                ) as *const Production;
                ((production, step_id.step_index as u32), production_indices)
            })
            .collect();

        InlinedProductionMap {
            productions,
            production_map,
        }
    }

    fn inline_production_at_step<'a>(
        &'a mut self,
        step_id: ProductionStepId,
        grammar: &'a SyntaxGrammar,
    ) -> &'a [usize] {
        // Build a list of productions produced by inlining rules.
        let mut i = 0;
        let step_index = step_id.step_index;
        let mut productions_to_add = vec![self.production_for_id(step_id, grammar).clone()];
        while i < productions_to_add.len() {
            if let Some(step) = productions_to_add[i].steps.get(step_index) {
                let symbol = step.symbol;
                if grammar.variables_to_inline.contains(&symbol) {
                    // Remove the production from the vector, replacing it with a placeholder.
                    let production = productions_to_add
                        .splice(i..=i, std::iter::once(&Production::default()).cloned())
                        .next()
                        .unwrap();

                    // Replace the placeholder with the inlined productions.
                    productions_to_add.splice(
                        i..=i,
                        grammar.variables[symbol.index].productions.iter().map(|p| {
                            let mut production = production.clone();
                            let removed_step = production
                                .steps
                                .splice(step_index..=step_index, p.steps.iter().cloned())
                                .next()
                                .unwrap();
                            let inserted_steps =
                                &mut production.steps[step_index..(step_index + p.steps.len())];
                            if let Some(alias) = removed_step.alias {
                                for inserted_step in inserted_steps.iter_mut() {
                                    inserted_step.alias = Some(alias.clone());
                                }
                            }
                            if let Some(field_name) = removed_step.field_name {
                                for inserted_step in inserted_steps.iter_mut() {
                                    inserted_step.field_name = Some(field_name.clone());
                                }
                            }
                            if let Some(last_inserted_step) = inserted_steps.last_mut() {
                                if last_inserted_step.precedence.is_none() {
                                    last_inserted_step.precedence = removed_step.precedence;
                                }
                                if last_inserted_step.associativity.is_none() {
                                    last_inserted_step.associativity = removed_step.associativity;
                                }
                            }
                            if p.dynamic_precedence.abs() > production.dynamic_precedence.abs() {
                                production.dynamic_precedence = p.dynamic_precedence;
                            }
                            production
                        }),
                    );

                    continue;
                }
            }
            i += 1;
        }

        // Store all the computed productions.
        let result = productions_to_add
            .into_iter()
            .map(|production| {
                self.productions
                    .iter()
                    .position(|p| *p == production)
                    .unwrap_or_else(|| {
                        self.productions.push(production);
                        self.productions.len() - 1
                    })
            })
            .collect();

        // Cache these productions based on the original production step.
        self.production_indices_by_step_id
            .entry(step_id)
            .or_insert(result)
    }

    fn production_for_id<'a>(
        &'a self,
        id: ProductionStepId,
        grammar: &'a SyntaxGrammar,
    ) -> &'a Production {
        id.variable_index.map_or_else(
            || &self.productions[id.production_index],
            |variable_index| &grammar.variables[variable_index].productions[id.production_index],
        )
    }

    fn production_step_for_id<'a>(
        &'a self,
        id: ProductionStepId,
        grammar: &'a SyntaxGrammar,
    ) -> Option<&'a ProductionStep> {
        self.production_for_id(id, grammar).steps.get(id.step_index)
    }
}

pub type ProcessInlinesResult<T> = Result<T, ProcessInlinesError>;

#[derive(Debug, Error, Serialize)]
pub enum ProcessInlinesError {
    #[error("External token `{0}` cannot be inlined")]
    ExternalToken(String),
    #[error("Token `{0}` cannot be inlined")]
    Token(String),
    #[error("Rule `{0}` cannot be inlined because it is the first rule")]
    FirstRule(String),
}

pub(super) fn process_inlines(
    grammar: &SyntaxGrammar,
    lexical_grammar: &LexicalGrammar,
) -> ProcessInlinesResult<InlinedProductionMap> {
    for symbol in &grammar.variables_to_inline {
        match symbol.kind {
            SymbolType::External => {
                Err(ProcessInlinesError::ExternalToken(
                    grammar.external_tokens[symbol.index].name.clone(),
                ))?;
            }
            SymbolType::Terminal => {
                Err(ProcessInlinesError::Token(
                    lexical_grammar.variables[symbol.index].name.clone(),
                ))?;
            }
            SymbolType::NonTerminal if symbol.index == 0 => {
                Err(ProcessInlinesError::FirstRule(
                    grammar.variables[symbol.index].name.clone(),
                ))?;
            }
            _ => {}
        }
    }

    Ok(InlinedProductionMapBuilder {
        productions: Vec::new(),
        production_indices_by_step_id: HashMap::new(),
    }
    .build(grammar))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        grammars::{LexicalVariable, SyntaxVariable, VariableType},
        rules::{Associativity, Precedence, Symbol},
    };

    #[test]
    fn test_basic_inlining() {
        let grammar = SyntaxGrammar {
            variables_to_inline: vec![Symbol::non_terminal(1)],
            variables: vec![
                SyntaxVariable {
                    name: "non-terminal-0".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![
                            ProductionStep::new(Symbol::terminal(10)),
                            ProductionStep::new(Symbol::non_terminal(1)), // inlined
                            ProductionStep::new(Symbol::terminal(11)),
                        ],
                    }],
                },
                SyntaxVariable {
                    name: "non-terminal-1".to_string(),
                    kind: VariableType::Named,
                    productions: vec![
                        Production {
                            dynamic_precedence: 0,
                            steps: vec![
                                ProductionStep::new(Symbol::terminal(12)),
                                ProductionStep::new(Symbol::terminal(13)),
                            ],
                        },
                        Production {
                            dynamic_precedence: -2,
                            steps: vec![ProductionStep::new(Symbol::terminal(14))],
                        },
                    ],
                },
            ],
            ..Default::default()
        };

        let inline_map = process_inlines(&grammar, &LexicalGrammar::default()).unwrap();

        // Nothing to inline at step 0.
        assert!(inline_map
            .inlined_productions(&grammar.variables[0].productions[0], 0)
            .is_none());

        // Inlining variable 1 yields two productions.
        assert_eq!(
            inline_map
                .inlined_productions(&grammar.variables[0].productions[0], 1)
                .unwrap()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(10)),
                        ProductionStep::new(Symbol::terminal(12)),
                        ProductionStep::new(Symbol::terminal(13)),
                        ProductionStep::new(Symbol::terminal(11)),
                    ],
                },
                Production {
                    dynamic_precedence: -2,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(10)),
                        ProductionStep::new(Symbol::terminal(14)),
                        ProductionStep::new(Symbol::terminal(11)),
                    ],
                },
            ]
        );
    }

    #[test]
    fn test_nested_inlining() {
        let grammar = SyntaxGrammar {
            variables: vec![
                SyntaxVariable {
                    name: "non-terminal-0".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![
                            ProductionStep::new(Symbol::terminal(10)),
                            ProductionStep::new(Symbol::non_terminal(1)), // inlined
                            ProductionStep::new(Symbol::terminal(11)),
                            ProductionStep::new(Symbol::non_terminal(2)), // inlined
                            ProductionStep::new(Symbol::terminal(12)),
                        ],
                    }],
                },
                SyntaxVariable {
                    name: "non-terminal-1".to_string(),
                    kind: VariableType::Named,
                    productions: vec![
                        Production {
                            dynamic_precedence: 0,
                            steps: vec![ProductionStep::new(Symbol::terminal(13))],
                        },
                        Production {
                            dynamic_precedence: 0,
                            steps: vec![
                                ProductionStep::new(Symbol::non_terminal(3)), // inlined
                                ProductionStep::new(Symbol::terminal(14)),
                            ],
                        },
                    ],
                },
                SyntaxVariable {
                    name: "non-terminal-2".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![ProductionStep::new(Symbol::terminal(15))],
                    }],
                },
                SyntaxVariable {
                    name: "non-terminal-3".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![ProductionStep::new(Symbol::terminal(16))],
                    }],
                },
            ],
            variables_to_inline: vec![
                Symbol::non_terminal(1),
                Symbol::non_terminal(2),
                Symbol::non_terminal(3),
            ],
            ..Default::default()
        };

        let inline_map = process_inlines(&grammar, &LexicalGrammar::default()).unwrap();

        let productions = inline_map
            .inlined_productions(&grammar.variables[0].productions[0], 1)
            .unwrap()
            .collect::<Vec<_>>();

        assert_eq!(
            productions.iter().copied().cloned().collect::<Vec<_>>(),
            vec![
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(10)),
                        ProductionStep::new(Symbol::terminal(13)),
                        ProductionStep::new(Symbol::terminal(11)),
                        ProductionStep::new(Symbol::non_terminal(2)),
                        ProductionStep::new(Symbol::terminal(12)),
                    ],
                },
                Production {
                    dynamic_precedence: 0,
                    steps: vec![
                        ProductionStep::new(Symbol::terminal(10)),
                        ProductionStep::new(Symbol::terminal(16)),
                        ProductionStep::new(Symbol::terminal(14)),
                        ProductionStep::new(Symbol::terminal(11)),
                        ProductionStep::new(Symbol::non_terminal(2)),
                        ProductionStep::new(Symbol::terminal(12)),
                    ],
                },
            ]
        );

        assert_eq!(
            inline_map
                .inlined_productions(productions[0], 3)
                .unwrap()
                .cloned()
                .collect::<Vec<_>>(),
            vec![Production {
                dynamic_precedence: 0,
                steps: vec![
                    ProductionStep::new(Symbol::terminal(10)),
                    ProductionStep::new(Symbol::terminal(13)),
                    ProductionStep::new(Symbol::terminal(11)),
                    ProductionStep::new(Symbol::terminal(15)),
                    ProductionStep::new(Symbol::terminal(12)),
                ],
            },]
        );
    }

    #[test]
    fn test_inlining_with_precedence_and_alias() {
        let grammar = SyntaxGrammar {
            variables_to_inline: vec![Symbol::non_terminal(1), Symbol::non_terminal(2)],
            variables: vec![
                SyntaxVariable {
                    name: "non-terminal-0".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![
                            // inlined
                            ProductionStep::new(Symbol::non_terminal(1))
                                .with_prec(Precedence::Integer(1), Some(Associativity::Left)),
                            ProductionStep::new(Symbol::terminal(10)),
                            // inlined
                            ProductionStep::new(Symbol::non_terminal(2))
                                .with_alias("outer_alias", true),
                        ],
                    }],
                },
                SyntaxVariable {
                    name: "non-terminal-1".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![
                            ProductionStep::new(Symbol::terminal(11))
                                .with_prec(Precedence::Integer(2), None)
                                .with_alias("inner_alias", true),
                            ProductionStep::new(Symbol::terminal(12)),
                        ],
                    }],
                },
                SyntaxVariable {
                    name: "non-terminal-2".to_string(),
                    kind: VariableType::Named,
                    productions: vec![Production {
                        dynamic_precedence: 0,
                        steps: vec![ProductionStep::new(Symbol::terminal(13))],
                    }],
                },
            ],
            ..Default::default()
        };

        let inline_map = process_inlines(&grammar, &LexicalGrammar::default()).unwrap();

        let productions = inline_map
            .inlined_productions(&grammar.variables[0].productions[0], 0)
            .unwrap()
            .collect::<Vec<_>>();

        assert_eq!(
            productions.iter().copied().cloned().collect::<Vec<_>>(),
            vec![Production {
                dynamic_precedence: 0,
                steps: vec![
                    // The first step in the inlined production retains its precedence
                    // and alias.
                    ProductionStep::new(Symbol::terminal(11))
                        .with_prec(Precedence::Integer(2), None)
                        .with_alias("inner_alias", true),
                    // The final step of the inlined production inherits the precedence of
                    // the inlined step.
                    ProductionStep::new(Symbol::terminal(12))
                        .with_prec(Precedence::Integer(1), Some(Associativity::Left)),
                    ProductionStep::new(Symbol::terminal(10)),
                    ProductionStep::new(Symbol::non_terminal(2)).with_alias("outer_alias", true),
                ]
            }],
        );

        assert_eq!(
            inline_map
                .inlined_productions(productions[0], 3)
                .unwrap()
                .cloned()
                .collect::<Vec<_>>(),
            vec![Production {
                dynamic_precedence: 0,
                steps: vec![
                    ProductionStep::new(Symbol::terminal(11))
                        .with_prec(Precedence::Integer(2), None)
                        .with_alias("inner_alias", true),
                    ProductionStep::new(Symbol::terminal(12))
                        .with_prec(Precedence::Integer(1), Some(Associativity::Left)),
                    ProductionStep::new(Symbol::terminal(10)),
                    // All steps of the inlined production inherit their alias from the
                    // inlined step.
                    ProductionStep::new(Symbol::terminal(13)).with_alias("outer_alias", true),
                ]
            }],
        );
    }

    #[test]
    fn test_error_when_inlining_tokens() {
        let lexical_grammar = LexicalGrammar {
            variables: vec![LexicalVariable {
                name: "something".to_string(),
                kind: VariableType::Named,
                implicit_precedence: 0,
                start_state: 0,
            }],
            ..Default::default()
        };

        let grammar = SyntaxGrammar {
            variables_to_inline: vec![Symbol::terminal(0)],
            variables: vec![SyntaxVariable {
                name: "non-terminal-0".to_string(),
                kind: VariableType::Named,
                productions: vec![Production {
                    dynamic_precedence: 0,
                    steps: vec![ProductionStep::new(Symbol::terminal(0))],
                }],
            }],
            ..Default::default()
        };

        if let Err(error) = process_inlines(&grammar, &lexical_grammar) {
            assert_eq!(error.to_string(), "Token `something` cannot be inlined");
        } else {
            panic!("expected an error, but got none");
        }
    }
}
