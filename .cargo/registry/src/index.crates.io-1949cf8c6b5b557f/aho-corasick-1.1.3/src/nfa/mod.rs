/*!
Provides direct access to NFA implementations of Aho-Corasick.

The principle characteristic of an NFA in this crate is that it may
transition through multiple states per byte of haystack. In Aho-Corasick
parlance, NFAs follow failure transitions during a search. In contrast,
a [`DFA`](crate::dfa::DFA) pre-computes all failure transitions during
compilation at the expense of a much bigger memory footprint.

Currently, there are two NFA implementations provided: noncontiguous and
contiguous. The names reflect their internal representation, and consequently,
the trade offs associated with them:

* A [`noncontiguous::NFA`] uses a separate allocation for every NFA state to
represent its transitions in a sparse format. This is ideal for building an
NFA, since it cheaply permits different states to have a different number of
transitions. A noncontiguous NFA is where the main Aho-Corasick construction
algorithm is implemented. All other Aho-Corasick implementations are built by
first constructing a noncontiguous NFA.
* A [`contiguous::NFA`] is uses a single allocation to represent all states,
while still encoding most states as sparse states but permitting states near
the starting state to have a dense representation. The dense representation
uses more memory, but permits computing transitions during a search more
quickly. By only making the most active states dense (the states near the
starting state), a contiguous NFA better balances memory usage with search
speed. The single contiguous allocation also uses less overhead per state and
enables compression tricks where most states only use 8 bytes of heap memory.

When given the choice between these two, you almost always want to pick a
contiguous NFA. It takes only a little longer to build, but both its memory
usage and search speed are typically much better than a noncontiguous NFA. A
noncontiguous NFA is useful when prioritizing build times, or when there are
so many patterns that a contiguous NFA could not be built. (Currently, because
of both memory and search speed improvements, a contiguous NFA has a smaller
internal limit on the total number of NFA states it can represent. But you
would likely need to have hundreds of thousands or even millions of patterns
before you hit this limit.)
*/
pub mod contiguous;
pub mod noncontiguous;
