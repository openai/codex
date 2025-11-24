use core::mem;

use alloc::{vec, vec::Vec};

use crate::{
    nfa::thompson::{self, compiler::ThompsonRef, BuildError, Builder},
    util::primitives::{IteratorIndexExt, StateID},
};

/// A trie that preserves leftmost-first match semantics.
///
/// This is a purpose-built data structure for optimizing 'lit1|lit2|..|litN'
/// patterns. It can *only* handle alternations of literals, which makes it
/// somewhat restricted in its scope, but literal alternations are fairly
/// common.
///
/// At a 5,000 foot level, the main idea of this trie is make an alternation of
/// literals look more like a DFA than an NFA via epsilon removal.
///
/// More precisely, the main issue is in how alternations are compiled into
/// a Thompson NFA. Namely, each alternation gets a single NFA "union" state
/// with an epsilon transition for every branch of the alternation pointing to
/// an NFA state corresponding to the start of that branch. The main problem
/// with this representation is the cost of computing an epsilon closure. Once
/// you hit the alternation's start state, it acts as a sort of "clog" that
/// requires you to traverse all of the epsilon transitions to compute the full
/// closure.
///
/// While fixing such clogs in the general case is pretty tricky without going
/// to a DFA (or perhaps a Glushkov NFA, but that comes with other problems).
/// But at least in the case of an alternation of literals, we can convert
/// that to a prefix trie without too much cost. In theory, that's all you
/// really need to do: build the trie and then compile it to a Thompson NFA.
/// For example, if you have the pattern 'bar|baz|foo', then using a trie, it
/// is transformed to something like 'b(a(r|z))|f'. This reduces the clog by
/// reducing the number of epsilon transitions out of the alternation's start
/// state from 3 to 2 (it actually gets down to 1 when you use a sparse state,
/// which we do below). It's a small effect here, but when your alternation is
/// huge, the savings is also huge.
///
/// And that is... essentially what a LiteralTrie does. But there is one
/// hiccup. Consider a regex like 'sam|samwise'. How does a prefix trie compile
/// that when leftmost-first semantics are used? If 'sam|samwise' was the
/// entire regex, then you could just drop the 'samwise' branch entirely since
/// it is impossible to match ('sam' will always take priority, and since it
/// is a prefix of 'samwise', 'samwise' will never match). But what about the
/// regex '\b(sam|samwise)\b'? In that case, you can't remove 'samwise' because
/// it might match when 'sam' doesn't fall on a word boundary.
///
/// The main idea is that 'sam|samwise' can be translated to 'sam(?:|wise)',
/// which is a precisely equivalent regex that also gets rid of the clog.
///
/// Another example is 'zapper|z|zap'. That gets translated to
/// 'z(?:apper||ap)'.
///
/// We accomplish this by giving each state in the trie multiple "chunks" of
/// transitions. Each chunk barrier represents a match. The idea is that once
/// you know a match occurs, none of the transitions after the match can be
/// re-ordered and mixed in with the transitions before the match. Otherwise,
/// the match semantics could be changed.
///
/// See the 'State' data type for a bit more detail.
///
/// Future work:
///
/// * In theory, it would be nice to generalize the idea of removing clogs and
/// apply it to the NFA graph itself. Then this could in theory work for
/// case insensitive alternations of literals, or even just alternations where
/// each branch starts with a non-epsilon transition.
/// * Could we instead use the Aho-Corasick algorithm here? The aho-corasick
/// crate deals with leftmost-first matches correctly, but I think this implies
/// encoding failure transitions into a Thompson NFA somehow. Which seems fine,
/// because failure transitions are just unconditional epsilon transitions?
/// * Or perhaps even better, could we use an aho_corasick::AhoCorasick
/// directly? At time of writing, 0.7 is the current version of the
/// aho-corasick crate, and that definitely cannot be used as-is. But if we
/// expose the underlying finite state machine API, then could we use it? That
/// would be super. If we could figure that out, it might also lend itself to
/// more general composition of finite state machines.
#[derive(Clone)]
pub(crate) struct LiteralTrie {
    /// The set of trie states. Each state contains one or more chunks, where
    /// each chunk is a sparse set of transitions to other states. A leaf state
    /// is always a match state that contains only empty chunks (i.e., no
    /// transitions).
    states: Vec<State>,
    /// Whether to add literals in reverse to the trie. Useful when building
    /// a reverse NFA automaton.
    rev: bool,
}

impl LiteralTrie {
    /// Create a new literal trie that adds literals in the forward direction.
    pub(crate) fn forward() -> LiteralTrie {
        let root = State::default();
        LiteralTrie { states: vec![root], rev: false }
    }

    /// Create a new literal trie that adds literals in reverse.
    pub(crate) fn reverse() -> LiteralTrie {
        let root = State::default();
        LiteralTrie { states: vec![root], rev: true }
    }

    /// Add the given literal to this trie.
    ///
    /// If the literal could not be added because the `StateID` space was
    /// exhausted, then an error is returned. If an error returns, the trie
    /// is in an unspecified state.
    pub(crate) fn add(&mut self, bytes: &[u8]) -> Result<(), BuildError> {
        let mut prev = StateID::ZERO;
        let mut it = bytes.iter().copied();
        while let Some(b) = if self.rev { it.next_back() } else { it.next() } {
            prev = self.get_or_add_state(prev, b)?;
        }
        self.states[prev].add_match();
        Ok(())
    }

    /// If the given transition is defined, then return the next state ID.
    /// Otherwise, add the transition to `from` and point it to a new state.
    ///
    /// If a new state ID could not be allocated, then an error is returned.
    fn get_or_add_state(
        &mut self,
        from: StateID,
        byte: u8,
    ) -> Result<StateID, BuildError> {
        let active = self.states[from].active_chunk();
        match active.binary_search_by_key(&byte, |t| t.byte) {
            Ok(i) => Ok(active[i].next),
            Err(i) => {
                // Add a new state and get its ID.
                let next = StateID::new(self.states.len()).map_err(|_| {
                    BuildError::too_many_states(self.states.len())
                })?;
                self.states.push(State::default());
                // Offset our position to account for all transitions and not
                // just the ones in the active chunk.
                let i = self.states[from].active_chunk_start() + i;
                let t = Transition { byte, next };
                self.states[from].transitions.insert(i, t);
                Ok(next)
            }
        }
    }

    /// Compile this literal trie to the NFA builder given.
    ///
    /// This forwards any errors that may occur while using the given builder.
    pub(crate) fn compile(
        &self,
        builder: &mut Builder,
    ) -> Result<ThompsonRef, BuildError> {
        // Compilation proceeds via depth-first traversal of the trie.
        //
        // This is overall pretty brutal. The recursive version of this is
        // deliciously simple. (See 'compile_to_hir' below for what it might
        // look like.) But recursion on a trie means your call stack grows
        // in accordance with the longest literal, which just does not seem
        // appropriate. So we push the call stack to the heap. But as a result,
        // the trie traversal becomes pretty brutal because we essentially
        // have to encode the state of a double for-loop into an explicit call
        // frame. If someone can simplify this without using recursion, that'd
        // be great.

        // 'end' is our match state for this trie, but represented in the the
        // NFA. Any time we see a match in the trie, we insert a transition
        // from the current state we're in to 'end'.
        let end = builder.add_empty()?;
        let mut stack = vec![];
        let mut f = Frame::new(&self.states[StateID::ZERO]);
        loop {
            if let Some(t) = f.transitions.next() {
                if self.states[t.next].is_leaf() {
                    f.sparse.push(thompson::Transition {
                        start: t.byte,
                        end: t.byte,
                        next: end,
                    });
                } else {
                    f.sparse.push(thompson::Transition {
                        start: t.byte,
                        end: t.byte,
                        // This is a little funny, but when the frame we create
                        // below completes, it will pop this parent frame off
                        // and modify this transition to point to the correct
                        // state.
                        next: StateID::ZERO,
                    });
                    stack.push(f);
                    f = Frame::new(&self.states[t.next]);
                }
                continue;
            }
            // At this point, we have visited all transitions in f.chunk, so
            // add it as a sparse NFA state. Unless the chunk was empty, in
            // which case, we don't do anything.
            if !f.sparse.is_empty() {
                let chunk_id = if f.sparse.len() == 1 {
                    builder.add_range(f.sparse.pop().unwrap())?
                } else {
                    let sparse = mem::replace(&mut f.sparse, vec![]);
                    builder.add_sparse(sparse)?
                };
                f.union.push(chunk_id);
            }
            // Now we need to look to see if there are other chunks to visit.
            if let Some(chunk) = f.chunks.next() {
                // If we're here, it means we're on the second (or greater)
                // chunk, which implies there is a match at this point. So
                // connect this state to the final end state.
                f.union.push(end);
                // Advance to the next chunk.
                f.transitions = chunk.iter();
                continue;
            }
            // Now that we are out of chunks, we have completely visited
            // this state. So turn our union of chunks into an NFA union
            // state, and add that union state to the parent state's current
            // sparse state. (If there is no parent, we're done.)
            let start = builder.add_union(f.union)?;
            match stack.pop() {
                None => {
                    return Ok(ThompsonRef { start, end });
                }
                Some(mut parent) => {
                    // OK because the only way a frame gets pushed on to the
                    // stack (aside from the root) is when a transition has
                    // been added to 'sparse'.
                    parent.sparse.last_mut().unwrap().next = start;
                    f = parent;
                }
            }
        }
    }

    /// Converts this trie to an equivalent HIR expression.
    ///
    /// We don't actually use this, but it's useful for tests. In particular,
    /// it provides a (somewhat) human readable representation of the trie
    /// itself.
    #[cfg(test)]
    fn compile_to_hir(&self) -> regex_syntax::hir::Hir {
        self.compile_state_to_hir(StateID::ZERO)
    }

    /// The recursive implementation of 'to_hir'.
    ///
    /// Notice how simple this is compared to 'compile' above. 'compile' could
    /// be similarly simple, but we opt to not use recursion in order to avoid
    /// overflowing the stack in the case of a longer literal.
    #[cfg(test)]
    fn compile_state_to_hir(&self, sid: StateID) -> regex_syntax::hir::Hir {
        use regex_syntax::hir::Hir;

        let mut alt = vec![];
        for (i, chunk) in self.states[sid].chunks().enumerate() {
            if i > 0 {
                alt.push(Hir::empty());
            }
            if chunk.is_empty() {
                continue;
            }
            let mut chunk_alt = vec![];
            for t in chunk.iter() {
                chunk_alt.push(Hir::concat(vec![
                    Hir::literal(vec![t.byte]),
                    self.compile_state_to_hir(t.next),
                ]));
            }
            alt.push(Hir::alternation(chunk_alt));
        }
        Hir::alternation(alt)
    }
}

impl core::fmt::Debug for LiteralTrie {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeln!(f, "LiteralTrie(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            writeln!(f, "{:06?}: {:?}", sid.as_usize(), state)?;
        }
        writeln!(f, ")")?;
        Ok(())
    }
}

/// An explicit stack frame used for traversing the trie without using
/// recursion.
///
/// Each frame is tied to the traversal of a single trie state. The frame is
/// dropped once the entire state (and all of its children) have been visited.
/// The "output" of compiling a state is the 'union' vector, which is turn
/// converted to a NFA union state. Each branch of the union corresponds to a
/// chunk in the trie state.
///
/// 'sparse' corresponds to the set of transitions for a particular chunk in a
/// trie state. It is ultimately converted to an NFA sparse state. The 'sparse'
/// field, after being converted to a sparse NFA state, is reused for any
/// subsequent chunks in the trie state, if any exist.
#[derive(Debug)]
struct Frame<'a> {
    /// The remaining chunks to visit for a trie state.
    chunks: StateChunksIter<'a>,
    /// The transitions of the current chunk that we're iterating over. Since
    /// every trie state has at least one chunk, every frame is initialized
    /// with the first chunk's transitions ready to be consumed.
    transitions: core::slice::Iter<'a, Transition>,
    /// The NFA state IDs pointing to the start of each chunk compiled by
    /// this trie state. This ultimately gets converted to an NFA union once
    /// the entire trie state (and all of its children) have been compiled.
    /// The order of these matters for leftmost-first match semantics, since
    /// earlier matches in the union are preferred over later ones.
    union: Vec<StateID>,
    /// The actual NFA transitions for a single chunk in a trie state. This
    /// gets converted to an NFA sparse state, and its corresponding NFA state
    /// ID should get added to 'union'.
    sparse: Vec<thompson::Transition>,
}

impl<'a> Frame<'a> {
    /// Create a new stack frame for trie traversal. This initializes the
    /// 'transitions' iterator to the transitions for the first chunk, with the
    /// 'chunks' iterator being every chunk after the first one.
    fn new(state: &'a State) -> Frame<'a> {
        let mut chunks = state.chunks();
        // every state has at least 1 chunk
        let chunk = chunks.next().unwrap();
        let transitions = chunk.iter();
        Frame { chunks, transitions, union: vec![], sparse: vec![] }
    }
}

/// A state in a trie.
///
/// This uses a sparse representation. Since we don't use literal tries
/// for searching, and ultimately (and compilation requires visiting every
/// transition anyway), we use a sparse representation for transitions. This
/// means we save on memory, at the expense of 'LiteralTrie::add' being perhaps
/// a bit slower.
///
/// While 'transitions' is pretty standard as far as tries goes, the 'chunks'
/// piece here is more unusual. In effect, 'chunks' defines a partitioning
/// of 'transitions', where each chunk corresponds to a distinct set of
/// transitions. The key invariant is that a transition in one chunk cannot
/// be moved to another chunk. This is the secret sauce that preserve
/// leftmost-first match semantics.
///
/// A new chunk is added whenever we mark a state as a match state. Once a
/// new chunk is added, the old active chunk is frozen and is never mutated
/// again. The new chunk becomes the active chunk, which is defined as
/// '&transitions[chunks.last().map_or(0, |c| c.1)..]'. Thus, a state where
/// 'chunks' is empty actually contains one chunk. Thus, every state contains
/// at least one (possibly empty) chunk.
///
/// A "leaf" state is a state that has no outgoing transitions (so
/// 'transitions' is empty). Note that there is no way for a leaf state to be a
/// non-matching state. (Although while building the trie, within 'add', a leaf
/// state may exist while not containing any matches. But this invariant is
/// only broken within 'add'. Once 'add' returns, the invariant is upheld.)
#[derive(Clone, Default)]
struct State {
    transitions: Vec<Transition>,
    chunks: Vec<(usize, usize)>,
}

impl State {
    /// Mark this state as a match state and freeze the active chunk such that
    /// it can not be further mutated.
    fn add_match(&mut self) {
        // This is not strictly necessary, but there's no point in recording
        // another match by adding another chunk if the state has no
        // transitions. Note though that we only skip this if we already know
        // this is a match state, which is only true if 'chunks' is not empty.
        // Basically, if we didn't do this, nothing semantically would change,
        // but we'd end up pushing another chunk and potentially triggering an
        // alloc.
        if self.transitions.is_empty() && !self.chunks.is_empty() {
            return;
        }
        let chunk_start = self.active_chunk_start();
        let chunk_end = self.transitions.len();
        self.chunks.push((chunk_start, chunk_end));
    }

    /// Returns true if and only if this state is a leaf state. That is, a
    /// state that has no outgoing transitions.
    fn is_leaf(&self) -> bool {
        self.transitions.is_empty()
    }

    /// Returns an iterator over all of the chunks (including the currently
    /// active chunk) in this state. Since the active chunk is included, the
    /// iterator is guaranteed to always yield at least one chunk (although the
    /// chunk may be empty).
    fn chunks(&self) -> StateChunksIter<'_> {
        StateChunksIter {
            transitions: &*self.transitions,
            chunks: self.chunks.iter(),
            active: Some(self.active_chunk()),
        }
    }

    /// Returns the active chunk as a slice of transitions.
    fn active_chunk(&self) -> &[Transition] {
        let start = self.active_chunk_start();
        &self.transitions[start..]
    }

    /// Returns the index into 'transitions' where the active chunk starts.
    fn active_chunk_start(&self) -> usize {
        self.chunks.last().map_or(0, |&(_, end)| end)
    }
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut spacing = " ";
        for (i, chunk) in self.chunks().enumerate() {
            if i > 0 {
                write!(f, "{}MATCH", spacing)?;
            }
            spacing = "";
            for (j, t) in chunk.iter().enumerate() {
                spacing = " ";
                if j == 0 && i > 0 {
                    write!(f, " ")?;
                } else if j > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:?}", t)?;
            }
        }
        Ok(())
    }
}

/// An iterator over all of the chunks in a state, including the active chunk.
///
/// This iterator is created by `State::chunks`. We name this iterator so that
/// we can include it in the `Frame` type for non-recursive trie traversal.
#[derive(Debug)]
struct StateChunksIter<'a> {
    transitions: &'a [Transition],
    chunks: core::slice::Iter<'a, (usize, usize)>,
    active: Option<&'a [Transition]>,
}

impl<'a> Iterator for StateChunksIter<'a> {
    type Item = &'a [Transition];

    fn next(&mut self) -> Option<&'a [Transition]> {
        if let Some(&(start, end)) = self.chunks.next() {
            return Some(&self.transitions[start..end]);
        }
        if let Some(chunk) = self.active.take() {
            return Some(chunk);
        }
        None
    }
}

/// A single transition in a trie to another state.
#[derive(Clone, Copy)]
struct Transition {
    byte: u8,
    next: StateID,
}

impl core::fmt::Debug for Transition {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{:?} => {}",
            crate::util::escape::DebugByte(self.byte),
            self.next.as_usize()
        )
    }
}

#[cfg(test)]
mod tests {
    use bstr::B;
    use regex_syntax::hir::Hir;

    use super::*;

    #[test]
    fn zap() {
        let mut trie = LiteralTrie::forward();
        trie.add(b"zapper").unwrap();
        trie.add(b"z").unwrap();
        trie.add(b"zap").unwrap();

        let got = trie.compile_to_hir();
        let expected = Hir::concat(vec![
            Hir::literal(B("z")),
            Hir::alternation(vec![
                Hir::literal(B("apper")),
                Hir::empty(),
                Hir::literal(B("ap")),
            ]),
        ]);
        assert_eq!(expected, got);
    }

    #[test]
    fn maker() {
        let mut trie = LiteralTrie::forward();
        trie.add(b"make").unwrap();
        trie.add(b"maple").unwrap();
        trie.add(b"maker").unwrap();

        let got = trie.compile_to_hir();
        let expected = Hir::concat(vec![
            Hir::literal(B("ma")),
            Hir::alternation(vec![
                Hir::concat(vec![
                    Hir::literal(B("ke")),
                    Hir::alternation(vec![Hir::empty(), Hir::literal(B("r"))]),
                ]),
                Hir::literal(B("ple")),
            ]),
        ]);
        assert_eq!(expected, got);
    }
}
