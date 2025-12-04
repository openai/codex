use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;
use thiserror::Error;

use super::{ExtractedLexicalGrammar, ExtractedSyntaxGrammar, InternedGrammar};
use crate::{
    grammars::{ExternalToken, ReservedWordContext, Variable, VariableType},
    rules::{MetadataParams, Rule, Symbol, SymbolType},
};

pub type ExtractTokensResult<T> = Result<T, ExtractTokensError>;

#[derive(Debug, Error, Serialize)]
pub enum ExtractTokensError {
    #[error(
        "The rule `{0}` contains an empty string.

Tree-sitter does not support syntactic rules that contain an empty string
unless they are used only as the grammar's start rule.
"
    )]
    EmptyString(String),
    #[error("Rule '{0}' cannot be used as both an external token and a non-terminal rule")]
    ExternalTokenNonTerminal(String),
    #[error("Non-symbol rules cannot be used as external tokens")]
    NonSymbolExternalToken,
    #[error(transparent)]
    WordToken(NonTerminalWordTokenError),
    #[error("Reserved word '{0}' must be a token")]
    NonTokenReservedWord(String),
}

#[derive(Debug, Error, Serialize)]
pub struct NonTerminalWordTokenError {
    pub symbol_name: String,
    pub conflicting_symbol_name: Option<String>,
}

impl std::fmt::Display for NonTerminalWordTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Non-terminal symbol '{}' cannot be used as the word token",
            self.symbol_name
        )?;
        if let Some(conflicting_name) = &self.conflicting_symbol_name {
            writeln!(
                f,
                ", because its rule is duplicated in '{conflicting_name}'",
            )
        } else {
            writeln!(f)
        }
    }
}

pub(super) fn extract_tokens(
    mut grammar: InternedGrammar,
) -> ExtractTokensResult<(ExtractedSyntaxGrammar, ExtractedLexicalGrammar)> {
    let mut extractor = TokenExtractor {
        current_variable_name: String::new(),
        current_variable_token_count: 0,
        is_first_rule: false,
        extracted_variables: Vec::new(),
        extracted_usage_counts: Vec::new(),
    };

    for (i, variable) in &mut grammar.variables.iter_mut().enumerate() {
        extractor.extract_tokens_in_variable(i == 0, variable)?;
    }

    for variable in &mut grammar.external_tokens {
        extractor.extract_tokens_in_variable(false, variable)?;
    }

    let mut lexical_variables = Vec::with_capacity(extractor.extracted_variables.len());
    for variable in extractor.extracted_variables {
        lexical_variables.push(variable);
    }

    // If a variable's entire rule was extracted as a token and that token didn't
    // appear within any other rule, then remove that variable from the syntax
    // grammar, giving its name to the token in the lexical grammar. Any symbols
    // that pointed to that variable will need to be updated to point to the
    // variable in the lexical grammar. Symbols that pointed to later variables
    // will need to have their indices decremented.
    let mut variables = Vec::with_capacity(grammar.variables.len());
    let mut symbol_replacer = SymbolReplacer {
        replacements: HashMap::new(),
    };
    for (i, variable) in grammar.variables.into_iter().enumerate() {
        if let Rule::Symbol(Symbol {
            kind: SymbolType::Terminal,
            index,
        }) = variable.rule
        {
            if i > 0 && extractor.extracted_usage_counts[index] == 1 {
                let lexical_variable = &mut lexical_variables[index];
                if lexical_variable.kind == VariableType::Auxiliary
                    || variable.kind != VariableType::Hidden
                {
                    lexical_variable.kind = variable.kind;
                    lexical_variable.name = variable.name;
                    symbol_replacer.replacements.insert(i, index);
                    continue;
                }
            }
        }
        variables.push(variable);
    }

    for variable in &mut variables {
        variable.rule = symbol_replacer.replace_symbols_in_rule(&variable.rule);
    }

    let expected_conflicts = grammar
        .expected_conflicts
        .into_iter()
        .map(|conflict| {
            let mut result = conflict
                .iter()
                .map(|symbol| symbol_replacer.replace_symbol(*symbol))
                .collect::<Vec<_>>();
            result.sort_unstable();
            result.dedup();
            result
        })
        .collect();

    let supertype_symbols = grammar
        .supertype_symbols
        .into_iter()
        .map(|symbol| symbol_replacer.replace_symbol(symbol))
        .collect();

    let variables_to_inline = grammar
        .variables_to_inline
        .into_iter()
        .map(|symbol| symbol_replacer.replace_symbol(symbol))
        .collect();

    let mut separators = Vec::new();
    let mut extra_symbols = Vec::new();
    for rule in grammar.extra_symbols {
        if let Rule::Symbol(symbol) = rule {
            extra_symbols.push(symbol_replacer.replace_symbol(symbol));
        } else if let Some(index) = lexical_variables.iter().position(|v| v.rule == rule) {
            extra_symbols.push(Symbol::terminal(index));
        } else {
            separators.push(rule);
        }
    }

    let mut external_tokens = Vec::new();
    for external_token in grammar.external_tokens {
        let rule = symbol_replacer.replace_symbols_in_rule(&external_token.rule);
        if let Rule::Symbol(symbol) = rule {
            if symbol.is_non_terminal() {
                Err(ExtractTokensError::ExternalTokenNonTerminal(
                    variables[symbol.index].name.clone(),
                ))?;
            }

            if symbol.is_external() {
                external_tokens.push(ExternalToken {
                    name: external_token.name,
                    kind: external_token.kind,
                    corresponding_internal_token: None,
                });
            } else {
                external_tokens.push(ExternalToken {
                    name: lexical_variables[symbol.index].name.clone(),
                    kind: external_token.kind,
                    corresponding_internal_token: Some(symbol),
                });
            }
        } else {
            Err(ExtractTokensError::NonSymbolExternalToken)?;
        }
    }

    let mut word_token = None;
    if let Some(token) = grammar.word_token {
        let token = symbol_replacer.replace_symbol(token);
        if token.is_non_terminal() {
            let word_token_variable = &variables[token.index];
            let conflicting_symbol_name = variables
                .iter()
                .enumerate()
                .find(|(i, v)| *i != token.index && v.rule == word_token_variable.rule)
                .map(|(_, v)| v.name.clone());

            Err(ExtractTokensError::WordToken(NonTerminalWordTokenError {
                symbol_name: word_token_variable.name.clone(),
                conflicting_symbol_name,
            }))?;
        }
        word_token = Some(token);
    }

    let mut reserved_word_contexts = Vec::with_capacity(grammar.reserved_word_sets.len());
    for reserved_word_context in grammar.reserved_word_sets {
        let mut reserved_words = Vec::with_capacity(reserved_word_contexts.len());
        for reserved_rule in reserved_word_context.reserved_words {
            if let Rule::Symbol(symbol) = reserved_rule {
                reserved_words.push(symbol_replacer.replace_symbol(symbol));
            } else if let Some(index) = lexical_variables
                .iter()
                .position(|v| v.rule == reserved_rule)
            {
                reserved_words.push(Symbol::terminal(index));
            } else {
                let token_name = match &reserved_rule {
                    Rule::String(s) => s.clone(),
                    Rule::Pattern(p, _) => p.clone(),
                    _ => "unknown".to_string(),
                };
                Err(ExtractTokensError::NonTokenReservedWord(token_name))?;
            }
        }
        reserved_word_contexts.push(ReservedWordContext {
            name: reserved_word_context.name,
            reserved_words,
        });
    }

    Ok((
        ExtractedSyntaxGrammar {
            variables,
            expected_conflicts,
            extra_symbols,
            variables_to_inline,
            supertype_symbols,
            external_tokens,
            word_token,
            precedence_orderings: grammar.precedence_orderings,
            reserved_word_sets: reserved_word_contexts,
        },
        ExtractedLexicalGrammar {
            variables: lexical_variables,
            separators,
        },
    ))
}

struct TokenExtractor {
    current_variable_name: String,
    current_variable_token_count: usize,
    is_first_rule: bool,
    extracted_variables: Vec<Variable>,
    extracted_usage_counts: Vec<usize>,
}

struct SymbolReplacer {
    replacements: HashMap<usize, usize>,
}

impl TokenExtractor {
    fn extract_tokens_in_variable(
        &mut self,
        is_first: bool,
        variable: &mut Variable,
    ) -> ExtractTokensResult<()> {
        self.current_variable_name.clear();
        self.current_variable_name.push_str(&variable.name);
        self.current_variable_token_count = 0;
        self.is_first_rule = is_first;
        variable.rule = self.extract_tokens_in_rule(&variable.rule)?;
        Ok(())
    }

    fn extract_tokens_in_rule(&mut self, input: &Rule) -> ExtractTokensResult<Rule> {
        match input {
            Rule::String(name) => Ok(self.extract_token(input, Some(name))?.into()),
            Rule::Pattern(..) => Ok(self.extract_token(input, None)?.into()),
            Rule::Metadata { params, rule } => {
                if params.is_token {
                    let mut params = params.clone();
                    params.is_token = false;

                    let mut string_value = None;
                    if let Rule::String(value) = rule.as_ref() {
                        string_value = Some(value);
                    }

                    let rule_to_extract = if params == MetadataParams::default() {
                        rule.as_ref()
                    } else {
                        input
                    };

                    Ok(self.extract_token(rule_to_extract, string_value)?.into())
                } else {
                    Ok(Rule::Metadata {
                        params: params.clone(),
                        rule: Box::new(self.extract_tokens_in_rule(rule)?),
                    })
                }
            }
            Rule::Repeat(content) => Ok(Rule::Repeat(Box::new(
                self.extract_tokens_in_rule(content)?,
            ))),
            Rule::Seq(elements) => Ok(Rule::Seq(
                elements
                    .iter()
                    .map(|e| self.extract_tokens_in_rule(e))
                    .collect::<ExtractTokensResult<Vec<_>>>()?,
            )),
            Rule::Choice(elements) => Ok(Rule::Choice(
                elements
                    .iter()
                    .map(|e| self.extract_tokens_in_rule(e))
                    .collect::<ExtractTokensResult<Vec<_>>>()?,
            )),
            Rule::Reserved { rule, context_name } => Ok(Rule::Reserved {
                rule: Box::new(self.extract_tokens_in_rule(rule)?),
                context_name: context_name.clone(),
            }),
            _ => Ok(input.clone()),
        }
    }

    fn extract_token(
        &mut self,
        rule: &Rule,
        string_value: Option<&String>,
    ) -> ExtractTokensResult<Symbol> {
        for (i, variable) in self.extracted_variables.iter_mut().enumerate() {
            if variable.rule == *rule {
                self.extracted_usage_counts[i] += 1;
                return Ok(Symbol::terminal(i));
            }
        }

        let index = self.extracted_variables.len();
        let variable = if let Some(string_value) = string_value {
            if string_value.is_empty() && !self.is_first_rule {
                Err(ExtractTokensError::EmptyString(
                    self.current_variable_name.clone(),
                ))?;
            }
            Variable {
                name: string_value.clone(),
                kind: VariableType::Anonymous,
                rule: rule.clone(),
            }
        } else {
            self.current_variable_token_count += 1;
            Variable {
                name: format!(
                    "{}_token{}",
                    self.current_variable_name, self.current_variable_token_count
                ),
                kind: VariableType::Auxiliary,
                rule: rule.clone(),
            }
        };

        self.extracted_variables.push(variable);
        self.extracted_usage_counts.push(1);
        Ok(Symbol::terminal(index))
    }
}

impl SymbolReplacer {
    fn replace_symbols_in_rule(&mut self, rule: &Rule) -> Rule {
        match rule {
            Rule::Symbol(symbol) => self.replace_symbol(*symbol).into(),
            Rule::Choice(elements) => Rule::Choice(
                elements
                    .iter()
                    .map(|e| self.replace_symbols_in_rule(e))
                    .collect(),
            ),
            Rule::Seq(elements) => Rule::Seq(
                elements
                    .iter()
                    .map(|e| self.replace_symbols_in_rule(e))
                    .collect(),
            ),
            Rule::Repeat(content) => Rule::Repeat(Box::new(self.replace_symbols_in_rule(content))),
            Rule::Metadata { rule, params } => Rule::Metadata {
                params: params.clone(),
                rule: Box::new(self.replace_symbols_in_rule(rule)),
            },
            Rule::Reserved { rule, context_name } => Rule::Reserved {
                rule: Box::new(self.replace_symbols_in_rule(rule)),
                context_name: context_name.clone(),
            },
            _ => rule.clone(),
        }
    }

    fn replace_symbol(&self, symbol: Symbol) -> Symbol {
        if !symbol.is_non_terminal() {
            return symbol;
        }

        if let Some(replacement) = self.replacements.get(&symbol.index) {
            return Symbol::terminal(*replacement);
        }

        let mut adjusted_index = symbol.index;
        for replaced_index in self.replacements.keys() {
            if *replaced_index < symbol.index {
                adjusted_index -= 1;
            }
        }

        Symbol::non_terminal(adjusted_index)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_extraction() {
        let (syntax_grammar, lexical_grammar) = extract_tokens(build_grammar(vec![
            Variable::named(
                "rule_0",
                Rule::repeat(Rule::seq(vec![
                    Rule::string("a"),
                    Rule::pattern("b", ""),
                    Rule::choice(vec![
                        Rule::non_terminal(1),
                        Rule::non_terminal(2),
                        Rule::token(Rule::repeat(Rule::choice(vec![
                            Rule::string("c"),
                            Rule::string("d"),
                        ]))),
                    ]),
                ])),
            ),
            Variable::named("rule_1", Rule::pattern("e", "")),
            Variable::named("rule_2", Rule::pattern("b", "")),
            Variable::named(
                "rule_3",
                Rule::seq(vec![Rule::non_terminal(2), Rule::Blank]),
            ),
        ]))
        .unwrap();

        assert_eq!(
            syntax_grammar.variables,
            vec![
                Variable::named(
                    "rule_0",
                    Rule::repeat(Rule::seq(vec![
                        // The string "a" was replaced by a symbol referencing the lexical grammar
                        Rule::terminal(0),
                        // The pattern "b" was replaced by a symbol referencing the lexical grammar
                        Rule::terminal(1),
                        Rule::choice(vec![
                            // The symbol referencing `rule_1` was replaced by a symbol referencing
                            // the lexical grammar.
                            Rule::terminal(3),
                            // The symbol referencing `rule_2` had its index decremented because
                            // `rule_1` was moved to the lexical grammar.
                            Rule::non_terminal(1),
                            // The rule wrapped in `token` was replaced by a symbol referencing
                            // the lexical grammar.
                            Rule::terminal(2),
                        ])
                    ]))
                ),
                // The pattern "e" was only used in once place: as the definition of `rule_1`,
                // so that rule was moved to the lexical grammar. The pattern "b" appeared in
                // two places, so it was not moved into the lexical grammar.
                Variable::named("rule_2", Rule::terminal(1)),
                Variable::named(
                    "rule_3",
                    Rule::seq(vec![Rule::non_terminal(1), Rule::Blank,])
                ),
            ]
        );

        assert_eq!(
            lexical_grammar.variables,
            vec![
                Variable::anonymous("a", Rule::string("a")),
                Variable::auxiliary("rule_0_token1", Rule::pattern("b", "")),
                Variable::auxiliary(
                    "rule_0_token2",
                    Rule::repeat(Rule::choice(vec![Rule::string("c"), Rule::string("d"),]))
                ),
                Variable::named("rule_1", Rule::pattern("e", "")),
            ]
        );
    }

    #[test]
    fn test_start_rule_is_token() {
        let (syntax_grammar, lexical_grammar) =
            extract_tokens(build_grammar(vec![Variable::named(
                "rule_0",
                Rule::string("hello"),
            )]))
            .unwrap();

        assert_eq!(
            syntax_grammar.variables,
            vec![Variable::named("rule_0", Rule::terminal(0)),]
        );
        assert_eq!(
            lexical_grammar.variables,
            vec![Variable::anonymous("hello", Rule::string("hello")),]
        );
    }

    #[test]
    fn test_extracting_extra_symbols() {
        let mut grammar = build_grammar(vec![
            Variable::named("rule_0", Rule::string("x")),
            Variable::named("comment", Rule::pattern("//.*", "")),
        ]);
        grammar.extra_symbols = vec![Rule::string(" "), Rule::non_terminal(1)];

        let (syntax_grammar, lexical_grammar) = extract_tokens(grammar).unwrap();
        assert_eq!(syntax_grammar.extra_symbols, vec![Symbol::terminal(1),]);
        assert_eq!(lexical_grammar.separators, vec![Rule::string(" "),]);
    }

    #[test]
    fn test_extract_externals() {
        let mut grammar = build_grammar(vec![
            Variable::named(
                "rule_0",
                Rule::seq(vec![
                    Rule::external(0),
                    Rule::string("a"),
                    Rule::non_terminal(1),
                    Rule::non_terminal(2),
                ]),
            ),
            Variable::named("rule_1", Rule::string("b")),
            Variable::named("rule_2", Rule::string("c")),
        ]);
        grammar.external_tokens = vec![
            Variable::named("external_0", Rule::external(0)),
            Variable::anonymous("a", Rule::string("a")),
            Variable::named("rule_2", Rule::non_terminal(2)),
        ];

        let (syntax_grammar, _) = extract_tokens(grammar).unwrap();

        assert_eq!(
            syntax_grammar.external_tokens,
            vec![
                ExternalToken {
                    name: "external_0".to_string(),
                    kind: VariableType::Named,
                    corresponding_internal_token: None,
                },
                ExternalToken {
                    name: "a".to_string(),
                    kind: VariableType::Anonymous,
                    corresponding_internal_token: Some(Symbol::terminal(0)),
                },
                ExternalToken {
                    name: "rule_2".to_string(),
                    kind: VariableType::Named,
                    corresponding_internal_token: Some(Symbol::terminal(2)),
                },
            ]
        );
    }

    #[test]
    fn test_error_on_external_with_same_name_as_non_terminal() {
        let mut grammar = build_grammar(vec![
            Variable::named(
                "rule_0",
                Rule::seq(vec![Rule::non_terminal(1), Rule::non_terminal(2)]),
            ),
            Variable::named(
                "rule_1",
                Rule::seq(vec![Rule::non_terminal(2), Rule::non_terminal(2)]),
            ),
            Variable::named("rule_2", Rule::string("a")),
        ]);
        grammar.external_tokens = vec![Variable::named("rule_1", Rule::non_terminal(1))];

        match extract_tokens(grammar) {
            Err(e) => {
                assert_eq!(e.to_string(), "Rule 'rule_1' cannot be used as both an external token and a non-terminal rule");
            }
            _ => {
                panic!("Expected an error but got no error");
            }
        }
    }

    #[test]
    fn test_extraction_on_hidden_terminal() {
        let (syntax_grammar, lexical_grammar) = extract_tokens(build_grammar(vec![
            Variable::named("rule_0", Rule::non_terminal(1)),
            Variable::hidden("_rule_1", Rule::string("a")),
        ]))
        .unwrap();

        // The rule `_rule_1` should not "absorb" the
        // terminal "a", since it is hidden,
        // so we expect two variables still
        assert_eq!(
            syntax_grammar.variables,
            vec![
                Variable::named("rule_0", Rule::non_terminal(1)),
                Variable::hidden("_rule_1", Rule::terminal(0)),
            ]
        );

        // We should not have a hidden rule in our lexical grammar, only the terminal "a"
        assert_eq!(
            lexical_grammar.variables,
            vec![Variable::anonymous("a", Rule::string("a"))]
        );
    }

    #[test]
    fn test_extraction_with_empty_string() {
        assert!(extract_tokens(build_grammar(vec![
            Variable::named("rule_0", Rule::non_terminal(1)),
            Variable::hidden("_rule_1", Rule::string("")),
        ]))
        .is_err());
    }

    fn build_grammar(variables: Vec<Variable>) -> InternedGrammar {
        InternedGrammar {
            variables,
            ..Default::default()
        }
    }
}
