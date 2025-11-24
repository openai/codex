#![allow(dead_code)]

use crate::collections::{map, Map};
use crate::grammar::consts::INLINE;
use crate::grammar::repr::*;
use crate::normalize::{NormError, NormResult};
use petgraph::graph::{Graph, NodeIndex};
use string_cache::DefaultAtom as Atom;

#[cfg(test)]
mod test;

/// Computes the proper order to inline the various nonterminals in
/// `grammar`. Reports an error if there is an inline
/// cycle. Otherwise, yields an ordering such that we inline X before
/// Y if Y references X.  I actually think it doesn't matter what
/// order we do the inlining, really, but this order seems better
/// somehow. :) (That is, inline into something before we inline it.)
pub fn inline_order(grammar: &Grammar) -> NormResult<Vec<NonterminalString>> {
    let mut graph = NonterminalGraph::new(grammar);
    graph.create_nodes();
    graph.add_edges();
    graph.inline_order()
}

struct NonterminalGraph<'grammar> {
    grammar: &'grammar Grammar,
    graph: Graph<NonterminalString, ()>,
    nonterminal_map: Map<NonterminalString, NodeIndex>,
}

#[derive(Copy, Clone)]
enum WalkState {
    NotVisited,
    Visiting,
    Visited,
}

impl<'grammar> NonterminalGraph<'grammar> {
    fn new(grammar: &'grammar Grammar) -> NonterminalGraph<'grammar> {
        NonterminalGraph {
            grammar,
            graph: Graph::new(),
            nonterminal_map: map(),
        }
    }

    fn create_nodes(&mut self) {
        let inline = Atom::from(INLINE);
        for (name, data) in &self.grammar.nonterminals {
            if data.annotations.iter().any(|a| a.id == inline) {
                let index = self.graph.add_node(name.clone());
                self.nonterminal_map.insert(name.clone(), index);
            }
        }
    }

    fn add_edges(&mut self) {
        for production in self
            .grammar
            .nonterminals
            .values()
            .flat_map(|d| &d.productions)
        {
            let from_index = match self.nonterminal_map.get(&production.nonterminal) {
                Some(&index) => index,
                None => continue, // this is not an inlined nonterminal
            };

            for symbol in &production.symbols {
                match *symbol {
                    Symbol::Nonterminal(ref to) => {
                        if let Some(&to_index) = self.nonterminal_map.get(to) {
                            self.graph.add_edge(from_index, to_index, ());
                        }
                    }
                    Symbol::Terminal(_) => {}
                }
            }
        }
    }

    fn inline_order(&self) -> NormResult<Vec<NonterminalString>> {
        let mut states = vec![WalkState::NotVisited; self.graph.node_count()];
        let mut result = vec![];
        for node in self.nonterminal_map.values().cloned() {
            self.walk(&mut states, &mut result, node)?;
        }
        Ok(result)
    }

    fn walk(
        &self,
        states: &mut Vec<WalkState>,
        result: &mut Vec<NonterminalString>,
        source: NodeIndex,
    ) -> NormResult<()> {
        let nt = self.graph.node_weight(source).unwrap();

        match states[source.index()] {
            WalkState::NotVisited => {
                states[source.index()] = WalkState::Visiting;
                for target in self.graph.neighbors(source) {
                    self.walk(states, result, target)?;
                }
                states[source.index()] = WalkState::Visited;
                result.push(nt.clone());
                Ok(())
            }
            WalkState::Visited => Ok(()),
            WalkState::Visiting => {
                return_err!(
                    self.grammar.nonterminals[nt].span,
                    "cyclic inline directive: `{}` would have to be inlined into itself",
                    nt
                );
            }
        }
    }
}
