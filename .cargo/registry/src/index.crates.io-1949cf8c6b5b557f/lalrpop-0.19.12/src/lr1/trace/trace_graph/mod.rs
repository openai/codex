use crate::collections::{map, Map};
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::lr1::example::*;
use crate::lr1::first::*;
use crate::lr1::lookahead::*;
use petgraph::graph::{EdgeReference, Edges, NodeIndex};
use petgraph::prelude::*;
use petgraph::{Directed, EdgeDirection, Graph};
use std::fmt::{Debug, Error, Formatter};

#[cfg(test)]
mod test;

/// Trace graphs are used to summarize how it is that we came to be in
/// a state where we can take some particular shift/reduce action; put
/// another way, how it is that we came to be in a state with some
/// particular LR(1) item.
///
/// The nodes in the graph are each labeled with a TraceGraphNode and
/// hence take one of two forms:
///
/// - TraceGraphNode::Item -- represents an LR0 item. These nodes are
///   used for the starting/end points in the graph only.  Basically a
///   complete trace stretches from the start item to some end item,
///   and all intermediate nodes are nonterminals.
/// - TraceGraphNode::Nonterminal -- if this graph is for a shift,
///   then these represent items where the cursor is at the beginning:
///   `X = (*) ...`. If the graph is for a reduce, they represent
///   items where a reduce is possible without shifting any more
///   terminals (though further reductions may be needed): `X =
///   ... (*) ...s` where `FIRST(...s)` includes `\epsilon`.
///
/// The edges in the graph are also important. They are labeled with
/// `SymbolSets` instances, meaning that each carries a (prefix,
/// cursor, and suffix) tuple. The label on an edge `A -> B` means
/// that transitioning from a state containing `A` to a state
/// containing `B` is possible if you:
///
/// - shift the symbols in `prefix`
/// - `B` will produce the symbol in `cursor`
/// - shift the symbols in `suffix` after `B` is popped
pub struct TraceGraph<'grammar> {
    // A -L-> B means:
    //
    //     Transition from a state containing A to a state containing
    //     B by (pushing|popping) the symbols L.
    //
    // If this trace graph represents a shift backtrace, then the
    // labels are symbols that are pushed. Otherwise they are labels
    // that are popped.
    graph: Graph<TraceGraphNode<'grammar>, SymbolSets<'grammar>>,
    indices: Map<TraceGraphNode<'grammar>, NodeIndex>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum TraceGraphNode<'grammar> {
    Nonterminal(NonterminalString),
    Item(LR0Item<'grammar>),
}

impl<'grammar> TraceGraph<'grammar> {
    pub fn new() -> Self {
        TraceGraph {
            graph: Graph::new(),
            indices: map(),
        }
    }

    pub fn add_node<T>(&mut self, node: T) -> NodeIndex
    where
        T: Into<TraceGraphNode<'grammar>>,
    {
        let node = node.into();
        let graph = &mut self.graph;
        *self
            .indices
            .entry(node.clone())
            .or_insert_with(|| graph.add_node(node))
    }

    pub fn add_edge<F, T>(&mut self, from: F, to: T, labels: SymbolSets<'grammar>)
    where
        F: Into<TraceGraphNode<'grammar>>,
        T: Into<TraceGraphNode<'grammar>>,
    {
        let from = self.add_node(from.into());
        let to = self.add_node(to.into());
        if !self
            .graph
            .edges_directed(from, EdgeDirection::Outgoing)
            .any(|edge| edge.target() == to && *edge.weight() == labels)
        {
            self.graph.add_edge(from, to, labels);
        }
    }

    pub fn lr0_examples<'graph>(
        &'graph self,
        lr0_item: LR0Item<'grammar>,
    ) -> PathEnumerator<'graph, 'grammar> {
        PathEnumerator::new(self, lr0_item)
    }

    pub fn lr1_examples<'trace>(
        &'trace self,
        first_sets: &'trace FirstSets,
        item: &LR1Item<'grammar>,
    ) -> FilteredPathEnumerator<'trace, 'grammar> {
        FilteredPathEnumerator::new(first_sets, self, item.to_lr0(), item.lookahead.clone())
    }
}

impl<'grammar> Into<TraceGraphNode<'grammar>> for NonterminalString {
    fn into(self) -> TraceGraphNode<'grammar> {
        TraceGraphNode::Nonterminal(self)
    }
}

impl<'grammar, L: Lookahead> Into<TraceGraphNode<'grammar>> for Item<'grammar, L> {
    fn into(self) -> TraceGraphNode<'grammar> {
        (&self).into()
    }
}

impl<'a, 'grammar, L: Lookahead> Into<TraceGraphNode<'grammar>> for &'a Item<'grammar, L> {
    fn into(self) -> TraceGraphNode<'grammar> {
        TraceGraphNode::Item(self.to_lr0())
    }
}

// This just exists to help with the `Debug` impl
struct TraceGraphEdge<'grammar> {
    from: TraceGraphNode<'grammar>,
    to: TraceGraphNode<'grammar>,
    label: (
        &'grammar [Symbol],
        Option<&'grammar Symbol>,
        &'grammar [Symbol],
    ),
}

impl<'grammar> Debug for TraceGraphEdge<'grammar> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "({:?} -{:?}-> {:?})", self.from, self.label, self.to)
    }
}

impl<'grammar> Debug for TraceGraph<'grammar> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        let mut s = fmt.debug_list();
        for (node, &index) in &self.indices {
            for edge in self.graph.edges_directed(index, EdgeDirection::Outgoing) {
                let label = edge.weight();
                s.entry(&TraceGraphEdge {
                    from: node.clone(),
                    to: self.graph[edge.target()].clone(),
                    label: (label.prefix, label.cursor, label.suffix),
                });
            }
        }
        s.finish()
    }
}

///////////////////////////////////////////////////////////////////////////
// PathEnumerator
//
// The path enumerater walks a trace graph searching for paths that
// start at a given item and terminate at another item. If such a path
// is found, you can then find the complete list of symbols by calling
// `symbols_and_cursor` and also get access to the state.

pub struct PathEnumerator<'graph, 'grammar: 'graph> {
    graph: &'graph TraceGraph<'grammar>,
    stack: Vec<EnumeratorState<'graph, 'grammar>>,
}

struct EnumeratorState<'graph, 'grammar: 'graph> {
    index: NodeIndex,
    symbol_sets: SymbolSets<'grammar>,
    edges: Edges<'graph, SymbolSets<'grammar>, Directed>,
}

impl<'graph, 'grammar> PathEnumerator<'graph, 'grammar> {
    fn new(graph: &'graph TraceGraph<'grammar>, lr0_item: LR0Item<'grammar>) -> Self {
        let start_state = graph.indices[&TraceGraphNode::Item(lr0_item)];
        let mut enumerator = PathEnumerator {
            graph,
            stack: vec![],
        };
        let edges = enumerator.incoming_edges(start_state);
        enumerator.stack.push(EnumeratorState {
            index: start_state,
            symbol_sets: SymbolSets::new(),
            edges,
        });
        enumerator.find_next_trace();
        enumerator
    }

    /// Advance to the next example. Returns false if there are no more
    /// examples.
    pub fn advance(&mut self) -> bool {
        // If we have not yet exhausted all the examples, then the top
        // of the stack should be the last target item that we
        // found. Pop it off.
        match self.stack.pop() {
            Some(top_state) => {
                assert!(match self.graph.graph[top_state.index] {
                    TraceGraphNode::Item(_) => true,
                    TraceGraphNode::Nonterminal(_) => false,
                });

                self.find_next_trace()
            }
            None => false,
        }
    }

    fn incoming_edges(&self, index: NodeIndex) -> Edges<'graph, SymbolSets<'grammar>, Directed> {
        self.graph
            .graph
            .edges_directed(index, EdgeDirection::Incoming)
    }

    /// This is the main operation, written in CPS style and hence it
    /// can seem a bit confusing. The idea is that `find_next_trace`
    /// is called when we are ready to consider the next child of
    /// whatever is on the top of the stack. It simply withdraws
    /// that next child (if any) and hands it to `push_next`.
    fn find_next_trace(&mut self) -> bool {
        if !self.stack.is_empty() {
            let next_edge = {
                let top_of_stack = self.stack.last_mut().unwrap();
                top_of_stack.edges.next()
            };
            self.push_next_child_if_any(next_edge)
        } else {
            false
        }
    }

    /// Invoked with the next child (if any) of the node on the top of
    /// the stack.
    ///
    /// If `next` is `Some`, we simply call `push_next_child`.
    ///
    /// If `next` is `None`, then the node on the top of
    /// the stack *has* no next child, and so it is popped, and then
    /// we call `find_next_trace` again to start with the next child
    /// of the new top of the stack.
    fn push_next_child_if_any(
        &mut self,
        next: Option<EdgeReference<'graph, SymbolSets<'grammar>>>,
    ) -> bool {
        if let Some(edge) = next {
            let index = edge.source();
            let symbol_sets = *edge.weight();
            self.push_next_child(index, symbol_sets)
        } else {
            self.stack.pop();
            self.find_next_trace()
        }
    }

    /// Push the next child of the top of the stack onto the stack,
    /// making the child the new top.
    ///
    /// If the child is an `Item` node, we have found the next trace,
    /// and hence our search terminates. We push the symbols from this
    /// item node into the symbols vector and then return true.
    ///
    /// Otherwise, we check whether this new node would cause a cycle.
    /// If so, we do *not* push it, and instead just call
    /// `find_next_trace` again to proceed to the next child of the
    /// current top.
    ///
    /// Finally, if the new node would NOT cause a cycle, then we can
    /// push it onto the stack so that it becomes the new top, and
    /// call `find_next_trace` to start searching its children.
    fn push_next_child(&mut self, index: NodeIndex, symbol_sets: SymbolSets<'grammar>) -> bool {
        match self.graph.graph[index] {
            TraceGraphNode::Item(_) => {
                // If we reached an item like
                //
                //     X = ...p (*) ...s
                //
                // then we are done, but we still need to push on the
                // symbols `...p`.
                let edges = self.incoming_edges(index);
                self.stack.push(EnumeratorState {
                    index,
                    symbol_sets,
                    edges,
                });
                true
            }
            TraceGraphNode::Nonterminal(_) => {
                // If this node already appears on the stack, do not
                // visit its children.
                if !self.stack.iter().any(|state| state.index == index) {
                    let edges = self.incoming_edges(index);
                    self.stack.push(EnumeratorState {
                        index,
                        symbol_sets,
                        edges,
                    });
                }
                self.find_next_trace()
            }
        }
    }

    pub fn found_trace(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Returns the 1-context for the current trace. In other words,
    /// the set of tokens that may appear next in the input. If this
    /// trace was derived from a shiftable item, this will always be
    /// the terminal that was to be shifted; if derived from a reduce
    /// item, this constitutes the set of lookaheads that will trigger
    /// a reduce.
    pub fn first0(&self, first_sets: &FirstSets) -> TokenSet {
        assert!(self.found_trace());
        first_sets.first0(
            self.stack[1]
                .symbol_sets
                .cursor
                .into_iter()
                .chain(self.stack.iter().flat_map(|s| s.symbol_sets.suffix)),
        )
    }

    pub fn example(&self) -> Example {
        assert!(self.found_trace());

        let mut symbols = vec![];

        symbols.extend(
            self.stack
                .iter()
                .rev()
                .flat_map(|s| s.symbol_sets.prefix)
                .cloned()
                .map(ExampleSymbol::Symbol),
        );

        let cursor = symbols.len();

        match self.stack[1].symbol_sets.cursor {
            Some(s) => symbols.push(ExampleSymbol::Symbol(s.clone())),
            None => {
                if self.stack[1].symbol_sets.prefix.is_empty() {
                    symbols.push(ExampleSymbol::Epsilon)
                } else {
                }
            }
        }

        symbols.extend(
            self.stack
                .iter()
                .flat_map(|s| s.symbol_sets.suffix)
                .cloned()
                .map(ExampleSymbol::Symbol),
        );

        let mut cursors = (0, symbols.len());

        let mut reductions: Vec<_> = self.stack[1..]
            .iter()
            .rev()
            .map(|state| {
                let nonterminal = match self.graph.graph[state.index] {
                    TraceGraphNode::Nonterminal(ref nonterminal) => nonterminal.clone(),
                    TraceGraphNode::Item(ref item) => item.production.nonterminal.clone(),
                };
                let reduction = Reduction {
                    start: cursors.0,
                    end: cursors.1,
                    nonterminal,
                };
                cursors.0 += state.symbol_sets.prefix.len();
                cursors.1 -= state.symbol_sets.suffix.len();
                reduction
            })
            .collect();
        reductions.reverse();

        Example {
            symbols,
            cursor,
            reductions,
        }
    }
}

impl<'graph, 'grammar> Iterator for PathEnumerator<'graph, 'grammar> {
    type Item = Example;

    fn next(&mut self) -> Option<Example> {
        if self.found_trace() {
            let example = self.example();
            self.advance();
            Some(example)
        } else {
            None
        }
    }
}

///////////////////////////////////////////////////////////////////////////
// FilteredPathEnumerator
//
// Like the path enumerator, but tests for examples with some specific
// lookahead

pub struct FilteredPathEnumerator<'graph, 'grammar: 'graph> {
    base: PathEnumerator<'graph, 'grammar>,
    first_sets: &'graph FirstSets,
    lookahead: TokenSet,
}

impl<'graph, 'grammar> FilteredPathEnumerator<'graph, 'grammar> {
    fn new(
        first_sets: &'graph FirstSets,
        graph: &'graph TraceGraph<'grammar>,
        lr0_item: LR0Item<'grammar>,
        lookahead: TokenSet,
    ) -> Self {
        FilteredPathEnumerator {
            base: PathEnumerator::new(graph, lr0_item),
            first_sets,
            lookahead,
        }
    }
}

impl<'graph, 'grammar> Iterator for FilteredPathEnumerator<'graph, 'grammar> {
    type Item = Example;

    fn next(&mut self) -> Option<Example> {
        while self.base.found_trace() {
            let firsts = self.base.first0(self.first_sets);
            if firsts.is_intersecting(&self.lookahead) {
                let example = self.base.example();
                self.base.advance();
                return Some(example);
            }
            self.base.advance();
        }
        None
    }
}
