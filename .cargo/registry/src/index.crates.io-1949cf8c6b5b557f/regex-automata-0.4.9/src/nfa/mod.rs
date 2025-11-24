/*!
Provides non-deterministic finite automata (NFA) and regex engines that use
them.

While NFAs and DFAs (deterministic finite automata) have equivalent *theoretical*
power, their usage in practice tends to result in different engineering trade
offs. While this isn't meant to be a comprehensive treatment of the topic, here
are a few key trade offs that are, at minimum, true for this crate:

* NFAs tend to be represented sparsely where as DFAs are represented densely.
Sparse representations use less memory, but are slower to traverse. Conversely,
dense representations use more memory, but are faster to traverse. (Sometimes
these lines are blurred. For example, an `NFA` might choose to represent a
particular state in a dense fashion, and a DFA can be built using a sparse
representation via [`sparse::DFA`](crate::dfa::sparse::DFA).
* NFAs have espilon transitions and DFAs don't. In practice, this means that
handling a single byte in a haystack with an NFA at search time may require
visiting multiple NFA states. In a DFA, each byte only requires visiting
a single state. Stated differently, NFAs require a variable number of CPU
instructions to process one byte in a haystack where as a DFA uses a constant
number of CPU instructions to process one byte.
* NFAs are generally easier to amend with secondary storage. For example, the
[`thompson::pikevm::PikeVM`] uses an NFA to match, but also uses additional
memory beyond the model of a finite state machine to track offsets for matching
capturing groups. Conversely, the most a DFA can do is report the offset (and
pattern ID) at which a match occurred. This is generally why we also compile
DFAs in reverse, so that we can run them after finding the end of a match to
also find the start of a match.
* NFAs take worst case linear time to build, but DFAs take worst case
exponential time to build. The [hybrid NFA/DFA](crate::hybrid) mitigates this
challenge for DFAs in many practical cases.

There are likely other differences, but the bottom line is that NFAs tend to be
more memory efficient and give easier opportunities for increasing expressive
power, where as DFAs are faster to search with.

# Why only a Thompson NFA?

Currently, the only kind of NFA we support in this crate is a [Thompson
NFA](https://en.wikipedia.org/wiki/Thompson%27s_construction). This refers
to a specific construction algorithm that takes the syntax of a regex
pattern and converts it to an NFA. Specifically, it makes gratuitous use of
epsilon transitions in order to keep its structure simple. In exchange, its
construction time is linear in the size of the regex. A Thompson NFA also makes
the guarantee that given any state and a character in a haystack, there is at
most one transition defined for it. (Although there may be many epsilon
transitions.)

It possible that other types of NFAs will be added in the future, such as a
[Glushkov NFA](https://en.wikipedia.org/wiki/Glushkov%27s_construction_algorithm).
But currently, this crate only provides a Thompson NFA.
*/

#[cfg(feature = "nfa-thompson")]
pub mod thompson;
