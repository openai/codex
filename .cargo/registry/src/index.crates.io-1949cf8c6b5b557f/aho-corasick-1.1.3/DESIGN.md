This document describes the internal design of this crate, which is an object
lesson in what happens when you take a fairly simple old algorithm like
Aho-Corasick and make it fast and production ready.

The target audience of this document is Rust programmers that have some
familiarity with string searching, however, one does not need to know the
Aho-Corasick algorithm in order to read this (it is explained below). One
should, however, know what a trie is. (If you don't, go read its Wikipedia
article.)

The center-piece of this crate is an implementation of Aho-Corasick. On its
own, Aho-Corasick isn't that complicated. The complex pieces come from the
different variants of Aho-Corasick implemented in this crate. Specifically,
they are:

* Aho-Corasick as a noncontiguous NFA. States have their transitions
  represented sparsely, and each state puts its transitions in its own separate
  allocation. Hence the same "noncontiguous."
* Aho-Corasick as a contiguous NFA. This NFA uses a single allocation to
  represent the transitions of all states. That is, transitions are laid out
  contiguously in memory. Moreover, states near the starting state are
  represented densely, such that finding the next state ID takes a constant
  number of instructions.
* Aho-Corasick as a DFA. In this case, all states are represented densely in
  a transition table that uses one allocation.
* Supporting "standard" match semantics, along with its overlapping variant,
  in addition to leftmost-first and leftmost-longest semantics. The "standard"
  semantics are typically what you see in a textbook description of
  Aho-Corasick. However, Aho-Corasick is also useful as an optimization in
  regex engines, which often use leftmost-first or leftmost-longest semantics.
  Thus, it is useful to implement those semantics here. The "standard" and
  "leftmost" search algorithms are subtly different, and also require slightly
  different construction algorithms.
* Support for ASCII case insensitive matching.
* Support for accelerating searches when the patterns all start with a small
  number of fixed bytes. Or alternatively, when the  patterns all contain a
  small number of rare bytes. (Searching for these bytes uses SIMD vectorized
  code courtesy of `memchr`.)
* Transparent support for alternative SIMD vectorized search routines for
  smaller number of literals, such as the Teddy algorithm. We called these
  "packed" search routines because they use SIMD. They can often be an order of
  magnitude faster than just Aho-Corasick, but don't scale as well.
* Support for searching streams. This can reuse most of the underlying code,
  but does require careful buffering support.
* Support for anchored searches, which permit efficient "is prefix" checks for
  a large number of patterns.

When you combine all of this together along with trying to make everything as
fast as possible, what you end up with is enitrely too much code with too much
`unsafe`. Alas, I was not smart enough to figure out how to reduce it. Instead,
we will explain it.


# Basics

The fundamental problem this crate is trying to solve is to determine the
occurrences of possibly many patterns in a haystack. The naive way to solve
this is to look for a match for each pattern at each position in the haystack:

    for i in 0..haystack.len():
      for p in patterns.iter():
        if haystack[i..].starts_with(p.bytes()):
          return Match(p.id(), i, i + p.bytes().len())

Those four lines are effectively all this crate does. The problem with those
four lines is that they are very slow, especially when you're searching for a
large number of patterns.

While there are many different algorithms available to solve this, a popular
one is Aho-Corasick. It's a common solution because it's not too hard to
implement, scales quite well even when searching for thousands of patterns and
is generally pretty fast. Aho-Corasick does well here because, regardless of
the number of patterns you're searching for, it always visits each byte in the
haystack exactly once. This means, generally speaking, adding more patterns to
an Aho-Corasick automaton does not make it slower. (Strictly speaking, however,
this is not true, since a larger automaton will make less effective use of the
CPU's cache.)

Aho-Corasick can be succinctly described as a trie with state transitions
between some of the nodes that efficiently instruct the search algorithm to
try matching alternative keys in the trie. The trick is that these state
transitions are arranged such that each byte of input needs to be inspected
only once. These state transitions are typically called "failure transitions,"
because they instruct the searcher (the thing traversing the automaton while
reading from the haystack) what to do when a byte in the haystack does not
correspond to a valid transition in the current state of the trie.

More formally, a failure transition points to a state in the automaton that may
lead to a match whose prefix is a proper suffix of the path traversed through
the trie so far. (If no such proper suffix exists, then the failure transition
points back to the start state of the trie, effectively restarting the search.)
This is perhaps simpler to explain pictorally. For example, let's say we built
an Aho-Corasick automaton with the following patterns: 'abcd' and 'cef'. The
trie looks like this:

         a - S1 - b - S2 - c - S3 - d - S4*
        /
    S0 - c - S5 - e - S6 - f - S7*

where states marked with a `*` are match states (meaning, the search algorithm
should stop and report a match to the caller).

So given this trie, it should be somewhat straight-forward to see how it can
be used to determine whether any particular haystack *starts* with either
`abcd` or `cef`. It's easy to express this in code:

    fn has_prefix(trie: &Trie, haystack: &[u8]) -> bool {
      let mut state_id = trie.start();
      // If the empty pattern is in trie, then state_id is a match state.
      if trie.is_match(state_id) {
        return true;
      }
      for (i, &b) in haystack.iter().enumerate() {
        state_id = match trie.next_state(state_id, b) {
          Some(id) => id,
          // If there was no transition for this state and byte, then we know
          // the haystack does not start with one of the patterns in our trie.
          None => return false,
        };
        if trie.is_match(state_id) {
          return true;
        }
      }
      false
    }

And that's pretty much it. All we do is move through the trie starting with the
bytes at the beginning of the haystack. If we find ourselves in a position
where we can't move, or if we've looked through the entire haystack without
seeing a match state, then we know the haystack does not start with any of the
patterns in the trie.

The meat of the Aho-Corasick algorithm is in how we add failure transitions to
our trie to keep searching efficient. Specifically, it permits us to not only
check whether a haystack *starts* with any one of a number of patterns, but
rather, whether the haystack contains any of a number of patterns *anywhere* in
the haystack.

As mentioned before, failure transitions connect a proper suffix of the path
traversed through the trie before, with a path that leads to a match that has a
prefix corresponding to that proper suffix. So in our case, for patterns `abcd`
and `cef`, with a haystack `abcef`, we want to transition to state `S5` (from
the diagram above) from `S3` upon seeing that the byte following `c` is not
`d`. Namely, the proper suffix in this example is `c`, which is a prefix of
`cef`. So the modified diagram looks like this:


         a - S1 - b - S2 - c - S3 - d - S4*
        /                      /
       /       ----------------
      /       /
    S0 - c - S5 - e - S6 - f - S7*

One thing that isn't shown in this diagram is that *all* states have a failure
transition, but only `S3` has a *non-trivial* failure transition. That is, all
other states have a failure transition back to the start state. So if our
haystack was `abzabcd`, then the searcher would transition back to `S0` after
seeing `z`, which effectively restarts the search. (Because there is no pattern
in our trie that has a prefix of `bz` or `z`.)

The code for traversing this *automaton* or *finite state machine* (it is no
longer just a trie) is not that much different from the `has_prefix` code
above:

    fn contains(fsm: &FiniteStateMachine, haystack: &[u8]) -> bool {
      let mut state_id = fsm.start();
      // If the empty pattern is in fsm, then state_id is a match state.
      if fsm.is_match(state_id) {
        return true;
      }
      for (i, &b) in haystack.iter().enumerate() {
        // While the diagram above doesn't show this, we may wind up needing
        // to follow multiple failure transitions before we land on a state
        // in which we can advance. Therefore, when searching for the next
        // state, we need to loop until we don't see a failure transition.
        //
        // This loop terminates because the start state has no empty
        // transitions. Every transition from the start state either points to
        // another state, or loops back to the start state.
        loop {
          match fsm.next_state(state_id, b) {
            Some(id) => {
              state_id = id;
              break;
            }
            // Unlike our code above, if there was no transition for this
            // state, then we don't quit. Instead, we look for this state's
            // failure transition and follow that instead.
            None => {
              state_id = fsm.next_fail_state(state_id);
            }
          };
        }
        if fsm.is_match(state_id) {
          return true;
        }
      }
      false
    }

Other than the complication around traversing failure transitions, this code
is still roughly "traverse the automaton with bytes from the haystack, and quit
when a match is seen."

And that concludes our section on the basics. While we didn't go deep into how
the automaton is built (see `src/nfa/noncontiguous.rs`, which has detailed
comments about that), the basic structure of Aho-Corasick should be reasonably
clear.


# NFAs and DFAs

There are generally two types of finite automata: non-deterministic finite
automata (NFA) and deterministic finite automata (DFA). The difference between
them is, principally, that an NFA can be in multiple states at once. This is
typically accomplished by things called _epsilon_ transitions, where one could
move to a new state without consuming any bytes from the input. (The other
mechanism by which NFAs can be in more than one state is where the same byte in
a particular state transitions to multiple distinct states.) In contrast, a DFA
can only ever be in one state at a time. A DFA has no epsilon transitions, and
for any given state, a byte transitions to at most one other state.

By this formulation, the Aho-Corasick automaton described in the previous
section is an NFA. This is because failure transitions are, effectively,
epsilon transitions. That is, whenever the automaton is in state `S`, it is
actually in the set of states that are reachable by recursively following
failure transitions from `S` until you reach the start state. (This means
that, for example, the start state is always active since the start state is
reachable via failure transitions from any state in the automaton.)

NFAs have a lot of nice properties. They tend to be easier to construct, and
also tend to use less memory. However, their primary downside is that they are
typically slower to execute a search with. For example, the code above showing
how to search with an Aho-Corasick automaton needs to potentially iterate
through many failure transitions for every byte of input. While this is a
fairly small amount of overhead, this can add up, especially if the automaton
has a lot of overlapping patterns with a lot of failure transitions.

A DFA's search code, by contrast, looks like this:

    fn contains(dfa: &DFA, haystack: &[u8]) -> bool {
      let mut state_id = dfa.start();
      // If the empty pattern is in dfa, then state_id is a match state.
      if dfa.is_match(state_id) {
        return true;
      }
      for (i, &b) in haystack.iter().enumerate() {
        // An Aho-Corasick DFA *never* has a missing state that requires
        // failure transitions to be followed. One byte of input advances the
        // automaton by one state. Always.
        state_id = dfa.next_state(state_id, b);
        if dfa.is_match(state_id) {
          return true;
        }
      }
      false
    }

The search logic here is much simpler than for the NFA, and this tends to
translate into significant performance benefits as well, since there's a lot
less work being done for each byte in the haystack. How is this accomplished?
It's done by pre-following all failure transitions for all states for all bytes
in the alphabet, and then building a single state transition table. Building
this DFA can be much more costly than building the NFA, and use much more
memory, but the better performance can be worth it.

Users of this crate can actually choose between using one of two possible NFAs
(noncontiguous or contiguous) or a DFA. By default, a contiguous NFA is used,
in most circumstances, but if the number of patterns is small enough a DFA will
be used. A contiguous NFA is chosen because it uses orders of magnitude less
memory than a DFA, takes only a little longer to build than a noncontiguous
NFA and usually gets pretty close to the search speed of a DFA. (Callers can
override this automatic selection via the `AhoCorasickBuilder::start_kind`
configuration.)


# More DFA tricks

As described in the previous section, one of the downsides of using a DFA
is that it uses more memory and can take longer to build. One small way of
mitigating these concerns is to map the alphabet used by the automaton into
a smaller space. Typically, the alphabet of a DFA has 256 elements in it:
one element for each possible value that fits into a byte. However, in many
cases, one does not need the full alphabet. For example, if all patterns in an
Aho-Corasick automaton are ASCII letters, then this only uses up 52 distinct
bytes. As far as the automaton is concerned, the rest of the 204 bytes are
indistinguishable from one another: they will never disrciminate between a
match or a non-match. Therefore, in cases like that, the alphabet can be shrunk
to just 53 elements. One for each ASCII letter, and then another to serve as a
placeholder for every other unused byte.

In practice, this library doesn't quite compute the optimal set of equivalence
classes, but it's close enough in most cases. The key idea is that this then
allows the transition table for the DFA to be potentially much smaller. The
downside of doing this, however, is that since the transition table is defined
in terms of this smaller alphabet space, every byte in the haystack must be
re-mapped to this smaller space. This requires an additional 256-byte table.
In practice, this can lead to a small search time hit, but it can be difficult
to measure. Moreover, it can sometimes lead to faster search times for bigger
automata, since it could be difference between more parts of the automaton
staying in the CPU cache or not.

One other trick for DFAs employed by this crate is the notion of premultiplying
state identifiers. Specifically, the normal way to compute the next transition
in a DFA is via the following (assuming that the transition table is laid out
sequentially in memory, in row-major order, where the rows are states):

    next_state_id = dfa.transitions[current_state_id * 256 + current_byte]

However, since the value `256` is a fixed constant, we can actually premultiply
the state identifiers in the table when we build the table initially. Then, the
next transition computation simply becomes:

    next_state_id = dfa.transitions[current_state_id + current_byte]

This doesn't seem like much, but when this is being executed for every byte of
input that you're searching, saving that extra multiplication instruction can
add up.

The same optimization works even when equivalence classes are enabled, as
described above. The only difference is that the premultiplication is by the
total number of equivalence classes instead of 256.

There isn't much downside to premultiplying state identifiers, other than it
imposes a smaller limit on the total number of states in the DFA. Namely, with
premultiplied state identifiers, you run out of room in your state identifier
representation more rapidly than if the identifiers are just state indices.

Both equivalence classes and premultiplication are always enabled. There is a
`AhoCorasickBuilder::byte_classes` configuration, but disabling this just makes
it so there are always 256 equivalence classes, i.e., every class corresponds
to precisely one byte. When it's disabled, the equivalence class map itself is
still used. The purpose of disabling it is when one is debugging the underlying
automaton. It can be easier to comprehend when it uses actual byte values for
its transitions instead of equivalence classes.


# Match semantics

One of the more interesting things about this implementation of Aho-Corasick
that (as far as this author knows) separates it from other implementations, is
that it natively supports leftmost-first and leftmost-longest match semantics.
Briefly, match semantics refer to the decision procedure by which searching
will disambiguate matches when there are multiple to choose from:

* **standard** match semantics emits matches as soon as they are detected by
  the automaton. This is typically equivalent to the textbook non-overlapping
  formulation of Aho-Corasick.
* **leftmost-first** match semantics means that 1) the next match is the match
  starting at the leftmost position and 2) among multiple matches starting at
  the same leftmost position, the match corresponding to the pattern provided
  first by the caller is reported.
* **leftmost-longest** is like leftmost-first, except when there are multiple
  matches starting at the same leftmost position, the pattern corresponding to
  the longest match is returned.

(The crate API documentation discusses these differences, with examples, in
more depth on the `MatchKind` type.)

The reason why supporting these match semantics is important is because it
gives the user more control over the match procedure. For example,
leftmost-first permits users to implement match priority by simply putting the
higher priority patterns first. Leftmost-longest, on the other hand, permits
finding the longest possible match, which might be useful when trying to find
words matching a dictionary. Additionally, regex engines often want to use
Aho-Corasick as an optimization when searching for an alternation of literals.
In order to preserve correct match semantics, regex engines typically can't use
the standard textbook definition directly, since regex engines will implement
either leftmost-first (Perl-like) or leftmost-longest (POSIX) match semantics.

Supporting leftmost semantics requires a couple key changes:

* Constructing the Aho-Corasick automaton changes a bit in both how the trie is
  constructed and how failure transitions are found. Namely, only a subset
  of the failure transitions are added. Specifically, only the failure
  transitions that either do not occur after a match or do occur after a match
  but preserve that match are kept. (More details on this can be found in
  `src/nfa/noncontiguous.rs`.)
* The search algorithm changes slightly. Since we are looking for the leftmost
  match, we cannot quit as soon as a match is detected. Instead, after a match
  is detected, we must keep searching until either the end of the input or
  until a dead state is seen. (Dead states are not used for standard match
  semantics. Dead states mean that searching should stop after a match has been
  found.)

Most other implementations of Aho-Corasick do support leftmost match semantics,
but they do it with more overhead at search time, or even worse, with a queue
of matches and sophisticated hijinks to disambiguate the matches. While our
construction algorithm becomes a bit more complicated, the correct match
semantics fall out from the structure of the automaton itself.


# Overlapping matches

One of the nice properties of an Aho-Corasick automaton is that it can report
all possible matches, even when they overlap with one another. In this mode,
the match semantics don't matter, since all possible matches are reported.
Overlapping searches work just like regular searches, except the state
identifier at which the previous search left off is carried over to the next
search, so that it can pick up where it left off. If there are additional
matches at that state, then they are reported before resuming the search.

Enabling leftmost-first or leftmost-longest match semantics causes the
automaton to use a subset of all failure transitions, which means that
overlapping searches cannot be used. Therefore, if leftmost match semantics are
used, attempting to do an overlapping search will return an error (or panic
when using the infallible APIs). Thus, to get overlapping searches, the caller
must use the default standard match semantics. This behavior was chosen because
there are only two alternatives, which were deemed worse:

* Compile two automatons internally, one for standard semantics and one for
  the semantics requested by the caller (if not standard).
* Create a new type, distinct from the `AhoCorasick` type, which has different
  capabilities based on the configuration options.

The first is untenable because of the amount of memory used by the automaton.
The second increases the complexity of the API too much by adding too many
types that do similar things. It is conceptually much simpler to keep all
searching isolated to a single type.


# Stream searching

Since Aho-Corasick is an automaton, it is possible to do partial searches on
partial parts of the haystack, and then resume that search on subsequent pieces
of the haystack. This is useful when the haystack you're trying to search is
not stored contiguously in memory, or if one does not want to read the entire
haystack into memory at once.

Currently, only standard semantics are supported for stream searching. This is
some of the more complicated code in this crate, and is something I would very
much like to improve. In particular, it currently has the restriction that it
must buffer at least enough of the haystack in memory in order to fit the
longest possible match. The difficulty in getting stream searching right is
that the implementation choices (such as the buffer size) often impact what the
API looks like and what it's allowed to do.


# Prefilters

In some cases, Aho-Corasick is not the fastest way to find matches containing
multiple patterns. Sometimes, the search can be accelerated using highly
optimized SIMD routines. For example, consider searching the following
patterns:

    Sherlock
    Moriarty
    Watson

It is plausible that it would be much faster to quickly look for occurrences of
the leading bytes, `S`, `M` or `W`, before trying to start searching via the
automaton. Indeed, this is exactly what this crate will do.

When there are more than three distinct starting bytes, then this crate will
look for three distinct bytes occurring at any position in the patterns, while
preferring bytes that are heuristically determined to be rare over others. For
example:

    Abuzz
    Sanchez
    Vasquez
    Topaz
    Waltz

Here, we have more than 3 distinct starting bytes, but all of the patterns
contain `z`, which is typically a rare byte. In this case, the prefilter will
scan for `z`, back up a bit, and then execute the Aho-Corasick automaton.

If all of that fails, then a packed multiple substring algorithm will be
attempted. Currently, the only algorithm available for this is Teddy, but more
may be added in the future. Teddy is unlike the above prefilters in that it
confirms its own matches, so when Teddy is active, it might not be necessary
for Aho-Corasick to run at all. However, the current Teddy implementation
only works in `x86_64` when SSSE3 or AVX2 are available or in `aarch64`
(using NEON), and moreover, only works _well_ when there are a small number
of patterns (say, less than 100). Teddy also requires the haystack to be of a
certain length (more than 16-34 bytes). When the haystack is shorter than that,
Rabin-Karp is used instead. (See `src/packed/rabinkarp.rs`.)

There is a more thorough description of Teddy at
[`src/packed/teddy/README.md`](src/packed/teddy/README.md).
