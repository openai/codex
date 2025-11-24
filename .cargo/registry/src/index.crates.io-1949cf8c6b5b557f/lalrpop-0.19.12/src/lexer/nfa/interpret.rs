//! A depth-first interpreter for NFAs.

use crate::lexer::nfa::{NFAStateIndex, Noop, Other, StateKind, Test, NFA, START};
use std::cmp::max;

/// Interpret `nfa` applied to `test`, returning the longest matching
/// string that we can find (if any).
pub fn interpret<'text>(nfa: &NFA, text: &'text str) -> Option<&'text str> {
    let mut longest: Option<usize> = None;
    let mut stack: Vec<(NFAStateIndex, usize)> = vec![(START, 0)];

    while let Some((state, offset)) = stack.pop() {
        match nfa.kind(state) {
            StateKind::Accept => match longest {
                None => longest = Some(offset),
                Some(o) => longest = Some(max(o, offset)),
            },
            StateKind::Reject => {
                // the rejection state is a dead-end
                continue;
            }
            StateKind::Neither => {}
        }

        // transition the no-op edges, to start
        for edge in nfa.edges::<Noop>(state) {
            push(&mut stack, (edge.to, offset));
        }

        // check whether there is another character
        let ch = match text[offset..].chars().next() {
            Some(ch) => ch, // yep
            None => {
                continue;
            } // nope
        };

        let offset1 = offset + ch.len_utf8();

        // transition test edges
        let mut tests = 0;
        for edge in nfa.edges::<Test>(state) {
            if edge.label.contains_char(ch) {
                push(&mut stack, (edge.to, offset1));
                tests += 1;
            }
        }

        // should *never* match more than one test, because tests
        // ought to be disjoint
        assert!(tests <= 1);

        // if no tests passed, use the "Other" edge
        if tests == 0 {
            for edge in nfa.edges::<Other>(state) {
                push(&mut stack, (edge.to, offset1));
                tests += 1;
            }

            // should *never* have more than one "otherwise" edge
            assert!(tests <= 1);
        }
    }

    longest.map(|offset| &text[..offset])
}

fn push<T: Eq>(v: &mut Vec<T>, t: T) {
    if !v.contains(&t) {
        v.push(t);
    }
}
