/*!
A collection of modules that provide APIs that are useful across many regex
engines.

While one should explore the sub-modules directly to get a sense of what's
there, here are some highlights that tie the sub-modules to higher level
use cases:

* `alphabet` contains APIs that are useful if you're doing low level things
with the DFAs in this crate. For example, implementing determinization or
walking its state graph directly.
* `captures` contains APIs for dealing with capture group matches and their
mapping to "slots" used inside an NFA graph. This is also where you can find
iterators over capture group names.
* `escape` contains types for pretty-printing raw byte slices as strings.
* `iter` contains API helpers for writing regex iterators.
* `lazy` contains a no-std and no-alloc variant of `lazy_static!` and
`once_cell`.
* `look` contains APIs for matching and configuring look-around assertions.
* `pool` provides a way to reuse mutable memory allocated in a thread safe
manner.
* `prefilter` provides APIs for building prefilters and using them in searches.
* `primitives` are what you might use if you're doing lower level work on
automata, such as walking an NFA state graph.
* `syntax` provides some higher level convenience functions for interacting
with the `regex-syntax` crate.
* `wire` is useful if you're working with DFA serialization.
*/

pub mod alphabet;
#[cfg(feature = "alloc")]
pub mod captures;
pub mod escape;
#[cfg(feature = "alloc")]
pub mod interpolate;
pub mod iter;
pub mod lazy;
pub mod look;
#[cfg(feature = "alloc")]
pub mod pool;
pub mod prefilter;
pub mod primitives;
pub mod start;
#[cfg(feature = "syntax")]
pub mod syntax;
pub mod wire;

#[cfg(any(feature = "dfa-build", feature = "hybrid"))]
pub(crate) mod determinize;
pub(crate) mod empty;
pub(crate) mod int;
pub(crate) mod memchr;
pub(crate) mod search;
#[cfg(feature = "alloc")]
pub(crate) mod sparse_set;
pub(crate) mod unicode_data;
pub(crate) mod utf8;
