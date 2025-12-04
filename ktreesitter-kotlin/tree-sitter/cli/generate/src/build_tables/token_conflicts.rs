use std::{cmp::Ordering, collections::HashSet, fmt};

use crate::{
    build_tables::item::TokenSetDisplay,
    grammars::{LexicalGrammar, SyntaxGrammar},
    nfa::{CharacterSet, NfaCursor, NfaTransition},
    rules::TokenSet,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TokenConflictStatus {
    matches_prefix: bool,
    does_match_continuation: bool,
    does_match_valid_continuation: bool,
    does_match_separators: bool,
    matches_same_string: bool,
    matches_different_string: bool,
}

pub struct TokenConflictMap<'a> {
    n: usize,
    status_matrix: Vec<TokenConflictStatus>,
    following_tokens: Vec<TokenSet>,
    starting_chars_by_index: Vec<CharacterSet>,
    following_chars_by_index: Vec<CharacterSet>,
    grammar: &'a LexicalGrammar,
}

impl<'a> TokenConflictMap<'a> {
    /// Create a token conflict map based on a lexical grammar, which describes the structure
    /// each token, and a `following_token` map, which indicates which tokens may be appear
    /// immediately after each other token.
    ///
    /// This analyzes the possible kinds of overlap between each pair of tokens and stores
    /// them in a matrix.
    pub fn new(grammar: &'a LexicalGrammar, following_tokens: Vec<TokenSet>) -> Self {
        let mut cursor = NfaCursor::new(&grammar.nfa, Vec::new());
        let starting_chars = get_starting_chars(&mut cursor, grammar);
        let following_chars = get_following_chars(&starting_chars, &following_tokens);

        let n = grammar.variables.len();
        let mut status_matrix = vec![TokenConflictStatus::default(); n * n];
        for i in 0..grammar.variables.len() {
            for j in 0..i {
                let status = compute_conflict_status(&mut cursor, grammar, &following_chars, i, j);
                status_matrix[matrix_index(n, i, j)] = status.0;
                status_matrix[matrix_index(n, j, i)] = status.1;
            }
        }

        TokenConflictMap {
            n,
            status_matrix,
            following_tokens,
            starting_chars_by_index: starting_chars,
            following_chars_by_index: following_chars,
            grammar,
        }
    }

    /// Does token `i` match any strings that token `j` also matches, such that token `i`
    /// is preferred over token `j`?
    pub fn has_same_conflict_status(&self, a: usize, b: usize, other: usize) -> bool {
        let left = &self.status_matrix[matrix_index(self.n, a, other)];
        let right = &self.status_matrix[matrix_index(self.n, b, other)];
        left == right
    }

    /// Does token `i` match any strings that token `j` does *not* match?
    pub fn does_match_different_string(&self, i: usize, j: usize) -> bool {
        self.status_matrix[matrix_index(self.n, i, j)].matches_different_string
    }

    /// Does token `i` match any strings that token `j` also matches, where
    /// token `i` is preferred over token `j`?
    pub fn does_match_same_string(&self, i: usize, j: usize) -> bool {
        self.status_matrix[matrix_index(self.n, i, j)].matches_same_string
    }

    pub fn does_conflict(&self, i: usize, j: usize) -> bool {
        let entry = &self.status_matrix[matrix_index(self.n, i, j)];
        entry.does_match_valid_continuation
            || entry.does_match_separators
            || entry.matches_same_string
    }

    /// Does token `i` match any strings that are *prefixes* of strings matched by `j`?
    pub fn does_match_prefix(&self, i: usize, j: usize) -> bool {
        self.status_matrix[matrix_index(self.n, i, j)].matches_prefix
    }

    pub fn does_match_shorter_or_longer(&self, i: usize, j: usize) -> bool {
        let entry = &self.status_matrix[matrix_index(self.n, i, j)];
        let reverse_entry = &self.status_matrix[matrix_index(self.n, j, i)];
        (entry.does_match_valid_continuation || entry.does_match_separators)
            && !reverse_entry.does_match_separators
    }

    pub fn does_overlap(&self, i: usize, j: usize) -> bool {
        let status = &self.status_matrix[matrix_index(self.n, i, j)];
        status.does_match_separators
            || status.matches_prefix
            || status.matches_same_string
            || status.does_match_continuation
    }

    pub fn prefer_token(grammar: &LexicalGrammar, left: (i32, usize), right: (i32, usize)) -> bool {
        match left.0.cmp(&right.0) {
            Ordering::Less => false,
            Ordering::Greater => true,
            Ordering::Equal => match grammar.variables[left.1]
                .implicit_precedence
                .cmp(&grammar.variables[right.1].implicit_precedence)
            {
                Ordering::Less => false,
                Ordering::Greater => true,
                Ordering::Equal => left.1 < right.1,
            },
        }
    }

    pub fn prefer_transition(
        grammar: &LexicalGrammar,
        t: &NfaTransition,
        completed_id: usize,
        completed_precedence: i32,
        has_separator_transitions: bool,
    ) -> bool {
        if t.precedence < completed_precedence {
            return false;
        }
        if t.precedence == completed_precedence {
            if t.is_separator {
                return false;
            }
            if has_separator_transitions
                && !grammar
                    .variable_indices_for_nfa_states(&t.states)
                    .any(|i| i == completed_id)
            {
                return false;
            }
        }
        true
    }
}

impl fmt::Debug for TokenConflictMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "TokenConflictMap {{")?;

        let syntax_grammar = SyntaxGrammar::default();

        writeln!(f, "  following_tokens: {{")?;
        for (i, following_tokens) in self.following_tokens.iter().enumerate() {
            writeln!(
                f,
                "    follow({:?}): {},",
                self.grammar.variables[i].name,
                TokenSetDisplay(following_tokens, &syntax_grammar, self.grammar)
            )?;
        }
        writeln!(f, "  }},")?;

        writeln!(f, "  starting_characters: {{")?;
        for i in 0..self.n {
            writeln!(
                f,
                "    {:?}: {:?},",
                self.grammar.variables[i].name, self.starting_chars_by_index[i]
            )?;
        }
        writeln!(f, "  }},")?;

        writeln!(f, "  following_characters: {{")?;
        for i in 0..self.n {
            writeln!(
                f,
                "    {:?}: {:?},",
                self.grammar.variables[i].name, self.following_chars_by_index[i]
            )?;
        }
        writeln!(f, "  }},")?;

        writeln!(f, "  status_matrix: {{")?;
        for i in 0..self.n {
            writeln!(f, "    {:?}: {{", self.grammar.variables[i].name)?;
            for j in 0..self.n {
                writeln!(
                    f,
                    "      {:?}: {:?},",
                    self.grammar.variables[j].name,
                    self.status_matrix[matrix_index(self.n, i, j)]
                )?;
            }
            writeln!(f, "    }},")?;
        }
        write!(f, "  }},")?;
        write!(f, "}}")?;
        Ok(())
    }
}

const fn matrix_index(variable_count: usize, i: usize, j: usize) -> usize {
    variable_count * i + j
}

fn get_starting_chars(cursor: &mut NfaCursor, grammar: &LexicalGrammar) -> Vec<CharacterSet> {
    let mut result = Vec::with_capacity(grammar.variables.len());
    for variable in &grammar.variables {
        cursor.reset(vec![variable.start_state]);
        let mut all_chars = CharacterSet::empty();
        for (chars, _) in cursor.transition_chars() {
            all_chars = all_chars.add(chars);
        }
        result.push(all_chars);
    }
    result
}

fn get_following_chars(
    starting_chars: &[CharacterSet],
    following_tokens: &[TokenSet],
) -> Vec<CharacterSet> {
    following_tokens
        .iter()
        .map(|following_tokens| {
            let mut chars = CharacterSet::empty();
            for token in following_tokens.iter() {
                if token.is_terminal() {
                    chars = chars.add(&starting_chars[token.index]);
                }
            }
            chars
        })
        .collect()
}

fn compute_conflict_status(
    cursor: &mut NfaCursor,
    grammar: &LexicalGrammar,
    following_chars: &[CharacterSet],
    i: usize,
    j: usize,
) -> (TokenConflictStatus, TokenConflictStatus) {
    let mut visited_state_sets = HashSet::new();
    let mut state_set_queue = vec![vec![
        grammar.variables[i].start_state,
        grammar.variables[j].start_state,
    ]];
    let mut result = (
        TokenConflictStatus::default(),
        TokenConflictStatus::default(),
    );

    while let Some(state_set) = state_set_queue.pop() {
        let mut live_variable_indices = grammar.variable_indices_for_nfa_states(&state_set);

        // If only one of the two tokens could possibly match from this state, then
        // there is no reason to analyze any of its successors. Just record the fact
        // that the token matches a string that the other token does not match.
        let first_live_variable_index = live_variable_indices.next().unwrap();
        if live_variable_indices.count() == 0 {
            if first_live_variable_index == i {
                result.0.matches_different_string = true;
            } else {
                result.1.matches_different_string = true;
            }
            continue;
        }

        // Don't pursue states where there's no potential for conflict.
        cursor.reset(state_set);
        let within_separator = cursor.transition_chars().any(|(_, sep)| sep);

        // Examine each possible completed token in this state.
        let mut completion = None;
        for (id, precedence) in cursor.completions() {
            if within_separator {
                if id == i {
                    result.0.does_match_separators = true;
                } else {
                    result.1.does_match_separators = true;
                }
            }

            // If the other token has already completed, then this is
            // a same-string conflict.
            if let Some((prev_id, prev_precedence)) = completion {
                if id == prev_id {
                    continue;
                }

                // Determine which of the two tokens is preferred.
                let preferred_id;
                if TokenConflictMap::prefer_token(
                    grammar,
                    (prev_precedence, prev_id),
                    (precedence, id),
                ) {
                    preferred_id = prev_id;
                } else {
                    preferred_id = id;
                    completion = Some((id, precedence));
                }

                if preferred_id == i {
                    result.0.matches_same_string = true;
                } else {
                    result.1.matches_same_string = true;
                }
            } else {
                completion = Some((id, precedence));
            }
        }

        // Examine each possible transition from this state to detect substring conflicts.
        for transition in cursor.transitions() {
            let mut can_advance = true;

            // If there is already a completed token in this state, then determine
            // if the next state can also match the completed token. If so, then
            // this is *not* a conflict.
            if let Some((completed_id, completed_precedence)) = completion {
                let mut advanced_id = None;
                let mut successor_contains_completed_id = false;
                for variable_id in grammar.variable_indices_for_nfa_states(&transition.states) {
                    if variable_id == completed_id {
                        successor_contains_completed_id = true;
                        break;
                    }
                    advanced_id = Some(variable_id);
                }

                // Determine which action is preferred: matching the already complete
                // token, or continuing on to try and match the other longer token.
                if let (Some(advanced_id), false) = (advanced_id, successor_contains_completed_id) {
                    if TokenConflictMap::prefer_transition(
                        grammar,
                        &transition,
                        completed_id,
                        completed_precedence,
                        within_separator,
                    ) {
                        can_advance = true;
                        if advanced_id == i {
                            result.0.does_match_continuation = true;
                            if transition.characters.does_intersect(&following_chars[j]) {
                                result.0.does_match_valid_continuation = true;
                            }
                        } else {
                            result.1.does_match_continuation = true;
                            if transition.characters.does_intersect(&following_chars[i]) {
                                result.1.does_match_valid_continuation = true;
                            }
                        }
                    } else if completed_id == i {
                        result.0.matches_prefix = true;
                    } else {
                        result.1.matches_prefix = true;
                    }
                }
            }

            if can_advance && visited_state_sets.insert(transition.states.clone()) {
                state_set_queue.push(transition.states);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        grammars::{Variable, VariableType},
        prepare_grammar::{expand_tokens, ExtractedLexicalGrammar},
        rules::{Precedence, Rule, Symbol},
    };

    #[test]
    fn test_starting_characters() {
        let grammar = expand_tokens(ExtractedLexicalGrammar {
            separators: Vec::new(),
            variables: vec![
                Variable {
                    name: "token_0".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::pattern("[a-f]1|0x\\d", ""),
                },
                Variable {
                    name: "token_1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::pattern("d*ef", ""),
                },
            ],
        })
        .unwrap();

        let token_map = TokenConflictMap::new(&grammar, Vec::new());

        assert_eq!(
            token_map.starting_chars_by_index[0],
            CharacterSet::empty().add_range('a', 'f').add_char('0')
        );
        assert_eq!(
            token_map.starting_chars_by_index[1],
            CharacterSet::empty().add_range('d', 'e')
        );
    }

    #[test]
    fn test_token_conflicts() {
        let grammar = expand_tokens(ExtractedLexicalGrammar {
            separators: Vec::new(),
            variables: vec![
                Variable {
                    name: "in".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("in"),
                },
                Variable {
                    name: "identifier".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::pattern("\\w+", ""),
                },
                Variable {
                    name: "instanceof".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("instanceof"),
                },
            ],
        })
        .unwrap();

        let var = |name| index_of_var(&grammar, name);

        let token_map = TokenConflictMap::new(
            &grammar,
            vec![
                std::iter::once(&Symbol::terminal(var("identifier")))
                    .copied()
                    .collect(),
                std::iter::once(&Symbol::terminal(var("in")))
                    .copied()
                    .collect(),
                std::iter::once(&Symbol::terminal(var("identifier")))
                    .copied()
                    .collect(),
            ],
        );

        // Given the string "in", the `in` token is preferred over the `identifier` token
        assert!(token_map.does_match_same_string(var("in"), var("identifier")));
        assert!(!token_map.does_match_same_string(var("identifier"), var("in")));

        // Depending on what character follows, the string "in" may be treated as part of an
        // `identifier` token.
        assert!(token_map.does_conflict(var("identifier"), var("in")));

        // Depending on what character follows, the string "instanceof" may be treated as part of
        // an `identifier` token.
        assert!(token_map.does_conflict(var("identifier"), var("instanceof")));
        assert!(token_map.does_conflict(var("instanceof"), var("in")));
    }

    #[test]
    fn test_token_conflicts_with_separators() {
        let grammar = expand_tokens(ExtractedLexicalGrammar {
            separators: vec![Rule::pattern("\\s", "")],
            variables: vec![
                Variable {
                    name: "x".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                Variable {
                    name: "newline".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("\n"),
                },
            ],
        })
        .unwrap();

        let var = |name| index_of_var(&grammar, name);

        let token_map = TokenConflictMap::new(&grammar, vec![TokenSet::new(); 4]);

        assert!(token_map.does_conflict(var("newline"), var("x")));
        assert!(!token_map.does_conflict(var("x"), var("newline")));
    }

    #[test]
    fn test_token_conflicts_with_open_ended_tokens() {
        let grammar = expand_tokens(ExtractedLexicalGrammar {
            separators: vec![Rule::pattern("\\s", "")],
            variables: vec![
                Variable {
                    name: "x".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                Variable {
                    name: "anything".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::prec(Precedence::Integer(-1), Rule::pattern(".*", "")),
                },
            ],
        })
        .unwrap();

        let var = |name| index_of_var(&grammar, name);

        let token_map = TokenConflictMap::new(&grammar, vec![TokenSet::new(); 4]);

        assert!(token_map.does_match_shorter_or_longer(var("anything"), var("x")));
        assert!(!token_map.does_match_shorter_or_longer(var("x"), var("anything")));
    }

    fn index_of_var(grammar: &LexicalGrammar, name: &str) -> usize {
        grammar
            .variables
            .iter()
            .position(|v| v.name == name)
            .unwrap()
    }
}
