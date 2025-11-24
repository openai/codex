/*!
Defines a Thompson NFA and provides the [`PikeVM`](pikevm::PikeVM) and
[`BoundedBacktracker`](backtrack::BoundedBacktracker) regex engines.

A Thompson NFA (non-deterministic finite automaton) is arguably _the_ central
data type in this library. It is the result of what is commonly referred to as
"regex compilation." That is, turning a regex pattern from its concrete syntax
string into something that can run a search looks roughly like this:

* A `&str` is parsed into a [`regex-syntax::ast::Ast`](regex_syntax::ast::Ast).
* An `Ast` is translated into a [`regex-syntax::hir::Hir`](regex_syntax::hir::Hir).
* An `Hir` is compiled into a [`NFA`].
* The `NFA` is then used to build one of a few different regex engines:
  * An `NFA` is used directly in the `PikeVM` and `BoundedBacktracker` engines.
  * An `NFA` is used by a [hybrid NFA/DFA](crate::hybrid) to build out a DFA's
  transition table at search time.
  * An `NFA`, assuming it is one-pass, is used to build a full
  [one-pass DFA](crate::dfa::onepass) ahead of time.
  * An `NFA` is used to build a [full DFA](crate::dfa) ahead of time.

The [`meta`](crate::meta) regex engine makes all of these choices for you based
on various criteria. However, if you have a lower level use case, _you_ can
build any of the above regex engines and use them directly. But you must start
here by building an `NFA`.

# Details

It is perhaps worth expanding a bit more on what it means to go through the
`&str`->`Ast`->`Hir`->`NFA` process.

* Parsing a string into an `Ast` gives it a structured representation.
Crucially, the size and amount of work done in this step is proportional to the
size of the original string. No optimization or Unicode handling is done at
this point. This means that parsing into an `Ast` has very predictable costs.
Moreover, an `Ast` can be roundtripped back to its original pattern string as
written.
* Translating an `Ast` into an `Hir` is a process by which the structured
representation is simplified down to its most fundamental components.
Translation deals with flags such as case insensitivity by converting things
like `(?i:a)` to `[Aa]`. Translation is also where Unicode tables are consulted
to resolve things like `\p{Emoji}` and `\p{Greek}`. It also flattens each
character class, regardless of how deeply nested it is, into a single sequence
of non-overlapping ranges. All the various literal forms are thrown out in
favor of one common representation. Overall, the `Hir` is small enough to fit
into your head and makes analysis and other tasks much simpler.
* Compiling an `Hir` into an `NFA` formulates the regex into a finite state
machine whose transitions are defined over bytes. For example, an `Hir` might
have a Unicode character class corresponding to a sequence of ranges defined
in terms of `char`. Compilation is then responsible for turning those ranges
into a UTF-8 automaton. That is, an automaton that matches the UTF-8 encoding
of just the codepoints specified by those ranges. Otherwise, the main job of
an `NFA` is to serve as a byte-code of sorts for a virtual machine. It can be
seen as a sequence of instructions for how to match a regex.
*/

#[cfg(feature = "nfa-backtrack")]
pub mod backtrack;
mod builder;
#[cfg(feature = "syntax")]
mod compiler;
mod error;
#[cfg(feature = "syntax")]
mod literal_trie;
#[cfg(feature = "syntax")]
mod map;
mod nfa;
#[cfg(feature = "nfa-pikevm")]
pub mod pikevm;
#[cfg(feature = "syntax")]
mod range_trie;

pub use self::{
    builder::Builder,
    error::BuildError,
    nfa::{
        DenseTransitions, PatternIter, SparseTransitions, State, Transition,
        NFA,
    },
};
#[cfg(feature = "syntax")]
pub use compiler::{Compiler, Config, WhichCaptures};
