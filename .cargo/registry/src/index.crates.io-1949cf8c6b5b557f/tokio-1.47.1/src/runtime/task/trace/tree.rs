use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};

use super::{Backtrace, Symbol, SymbolTrace, Trace};

/// An adjacency list representation of an execution tree.
///
/// This tree provides a convenient intermediate representation for formatting
/// [`Trace`] as a tree.
pub(super) struct Tree {
    /// The roots of the trees.
    ///
    /// There should only be one root, but the code is robust to multiple roots.
    roots: HashSet<Symbol>,

    /// The adjacency list of symbols in the execution tree(s).
    edges: HashMap<Symbol, HashSet<Symbol>>,
}

impl Tree {
    /// Constructs a [`Tree`] from [`Trace`]
    pub(super) fn from_trace(trace: Trace) -> Self {
        let mut roots: HashSet<Symbol> = HashSet::default();
        let mut edges: HashMap<Symbol, HashSet<Symbol>> = HashMap::default();

        for trace in trace.backtraces {
            let trace = to_symboltrace(trace);

            if let Some(first) = trace.first() {
                roots.insert(first.to_owned());
            }

            let mut trace = trace.into_iter().peekable();
            while let Some(frame) = trace.next() {
                let subframes = edges.entry(frame).or_default();
                if let Some(subframe) = trace.peek() {
                    subframes.insert(subframe.clone());
                }
            }
        }

        Tree { roots, edges }
    }

    /// Produces the sub-symbols of a given symbol.
    fn consequences(&self, frame: &Symbol) -> Option<impl ExactSizeIterator<Item = &Symbol>> {
        Some(self.edges.get(frame)?.iter())
    }

    /// Format this [`Tree`] as a textual tree.
    fn display<W: fmt::Write>(
        &self,
        f: &mut W,
        root: &Symbol,
        is_last: bool,
        prefix: &str,
    ) -> fmt::Result {
        let root_fmt = format!("{}", root);

        let current;
        let next;

        if is_last {
            current = format!("{prefix}└╼\u{a0}{root_fmt}");
            next = format!("{}\u{a0}\u{a0}\u{a0}", prefix);
        } else {
            current = format!("{prefix}├╼\u{a0}{root_fmt}");
            next = format!("{}│\u{a0}\u{a0}", prefix);
        }

        write!(f, "{}", {
            let mut current = current.chars();
            current.next().unwrap();
            current.next().unwrap();
            &current.as_str()
        })?;

        if let Some(consequences) = self.consequences(root) {
            let len = consequences.len();
            for (i, consequence) in consequences.enumerate() {
                let is_last = i == len - 1;
                writeln!(f)?;
                self.display(f, consequence, is_last, &next)?;
            }
        }

        Ok(())
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for root in &self.roots {
            self.display(f, root, true, " ")?;
        }
        Ok(())
    }
}

/// Resolve a sequence of [`backtrace::BacktraceFrame`]s into a sequence of
/// [`Symbol`]s.
fn to_symboltrace(backtrace: Backtrace) -> SymbolTrace {
    // Resolve the backtrace frames to symbols.
    let backtrace: Backtrace = {
        let mut backtrace = backtrace::Backtrace::from(backtrace);
        backtrace.resolve();
        backtrace.into()
    };

    // Accumulate the symbols in descending order into `symboltrace`.
    let mut symboltrace: SymbolTrace = vec![];
    let mut state = DefaultHasher::new();
    for frame in backtrace.into_iter().rev() {
        for symbol in frame.symbols().iter().rev() {
            let symbol = Symbol {
                symbol: symbol.clone(),
                parent_hash: state.finish(),
            };
            symbol.hash(&mut state);
            symboltrace.push(symbol);
        }
    }

    symboltrace
}
