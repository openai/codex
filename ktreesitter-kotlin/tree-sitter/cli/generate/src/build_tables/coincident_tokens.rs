use std::fmt;

use crate::{
    grammars::LexicalGrammar,
    rules::Symbol,
    tables::{ParseStateId, ParseTable},
};

pub struct CoincidentTokenIndex<'a> {
    entries: Vec<Vec<ParseStateId>>,
    grammar: &'a LexicalGrammar,
    n: usize,
}

impl<'a> CoincidentTokenIndex<'a> {
    pub fn new(table: &ParseTable, lexical_grammar: &'a LexicalGrammar) -> Self {
        let n = lexical_grammar.variables.len();
        let mut result = Self {
            n,
            grammar: lexical_grammar,
            entries: vec![Vec::new(); n * n],
        };
        for (i, state) in table.states.iter().enumerate() {
            for symbol in state.terminal_entries.keys() {
                if symbol.is_terminal() {
                    for other_symbol in state.terminal_entries.keys() {
                        if other_symbol.is_terminal() {
                            let index = result.index(symbol.index, other_symbol.index);
                            if result.entries[index].last().copied() != Some(i) {
                                result.entries[index].push(i);
                            }
                        }
                    }
                }
            }
        }
        result
    }

    pub fn states_with(&self, a: Symbol, b: Symbol) -> &[ParseStateId] {
        &self.entries[self.index(a.index, b.index)]
    }

    pub fn contains(&self, a: Symbol, b: Symbol) -> bool {
        !self.entries[self.index(a.index, b.index)].is_empty()
    }

    #[must_use]
    const fn index(&self, a: usize, b: usize) -> usize {
        if a < b {
            a * self.n + b
        } else {
            b * self.n + a
        }
    }
}

impl fmt::Debug for CoincidentTokenIndex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "CoincidentTokenIndex {{")?;

        writeln!(f, "  entries: {{")?;
        for i in 0..self.n {
            writeln!(f, "    {}: {{", self.grammar.variables[i].name)?;
            for j in 0..self.n {
                writeln!(
                    f,
                    "      {}: {:?},",
                    self.grammar.variables[j].name,
                    self.entries[self.index(i, j)].len()
                )?;
            }
            writeln!(f, "    }},")?;
        }
        write!(f, "  }},")?;
        write!(f, "}}")?;
        Ok(())
    }
}
