1.11.1 (2024-10-24)
===================
This is a new patch release of `regex` that fixes compilation on nightly
Rust when the unstable `pattern` crate feature is enabled. Users on nightly
Rust without this feature enabled are unaffected.

Bug fixes:

* [BUG #1231](https://github.com/rust-lang/regex/issues/1231):
Fix the `Pattern` trait implementation as a result of nightly API breakage.


1.11.0 (2024-09-29)
===================
This is a new minor release of `regex` that brings in an update to the
Unicode Character Database. Specifically, this updates the Unicode data
used by `regex` internally to the version 16 release.

New features:

* [FEATURE #1228](https://github.com/rust-lang/regex/pull/1228):
Add new `regex::SetMatches::matched_all` method.
* [FEATURE #1229](https://github.com/rust-lang/regex/pull/1229):
Update to Unicode Character Database (UCD) version 16.


1.10.6 (2024-08-02)
===================
This is a new patch release with a fix for the `unstable` crate feature that
enables `std::str::Pattern` trait integration.

Bug fixes:

* [BUG #1219](https://github.com/rust-lang/regex/pull/1219):
Fix the `Pattern` trait implementation as a result of nightly API breakage.


1.10.5 (2024-06-09)
===================
This is a new patch release with some minor fixes.

Bug fixes:

* [BUG #1203](https://github.com/rust-lang/regex/pull/1203):
Escape invalid UTF-8 when in the `Debug` impl of `regex::bytes::Match`.


1.10.4 (2024-03-22)
===================
This is a new patch release with some minor fixes.

* [BUG #1169](https://github.com/rust-lang/regex/issues/1169):
Fixes a bug with compiling a reverse NFA automaton in `regex-automata`.
* [BUG #1178](https://github.com/rust-lang/regex/pull/1178):
Clarifies that when `Cow::Borrowed` is returned from replace APIs, it is
equivalent to the input.


1.10.3 (2024-01-21)
===================
This is a new patch release that fixes the feature configuration of optional
dependencies, and fixes an unsound use of bounds check elision.

Bug fixes:

* [BUG #1147](https://github.com/rust-lang/regex/issues/1147):
Set `default-features=false` for the `memchr` and `aho-corasick` dependencies.
* [BUG #1154](https://github.com/rust-lang/regex/pull/1154):
Fix unsound bounds check elision.


1.10.2 (2023-10-16)
===================
This is a new patch release that fixes a search regression where incorrect
matches could be reported.

Bug fixes:

* [BUG #1110](https://github.com/rust-lang/regex/issues/1110):
Revert broadening of reverse suffix literal optimization introduced in 1.10.1.


1.10.1 (2023-10-14)
===================
This is a new patch release with a minor increase in the number of valid
patterns and a broadening of some literal optimizations.

New features:

* [FEATURE 04f5d7be](https://github.com/rust-lang/regex/commit/04f5d7be4efc542864cc400f5d43fbea4eb9bab6):
Loosen ASCII-compatible rules such that regexes like `(?-u:☃)` are now allowed.

Performance improvements:

* [PERF 8a8d599f](https://github.com/rust-lang/regex/commit/8a8d599f9d2f2d78e9ad84e4084788c2d563afa5):
Broader the reverse suffix optimization to apply in more cases.


1.10.0 (2023-10-09)
===================
This is a new minor release of `regex` that adds support for start and end
word boundary assertions. That is, `\<` and `\>`. The minimum supported Rust
version has also been raised to 1.65, which was released about one year ago.

The new word boundary assertions are:

* `\<` or `\b{start}`: a Unicode start-of-word boundary (`\W|\A` on the left,
`\w` on the right).
* `\>` or `\b{end}`: a Unicode end-of-word boundary (`\w` on the left, `\W|\z`
on the right)).
* `\b{start-half}`: half of a Unicode start-of-word boundary (`\W|\A` on the
left).
* `\b{end-half}`: half of a Unicode end-of-word boundary (`\W|\z` on the
right).

The `\<` and `\>` are GNU extensions to POSIX regexes. They have been added
to the `regex` crate because they enjoy somewhat broad support in other regex
engines as well (for example, vim). The `\b{start}` and `\b{end}` assertions
are aliases for `\<` and `\>`, respectively.

The `\b{start-half}` and `\b{end-half}` assertions are not found in any
other regex engine (although regex engines with general look-around support
can certainly express them). They were added principally to support the
implementation of word matching in grep programs, where one generally wants to
be a bit more flexible in what is considered a word boundary.

New features:

* [FEATURE #469](https://github.com/rust-lang/regex/issues/469):
Add support for `\<` and `\>` word boundary assertions.
* [FEATURE(regex-automata) #1031](https://github.com/rust-lang/regex/pull/1031):
DFAs now have a `start_state` method that doesn't use an `Input`.

Performance improvements:

* [PERF #1051](https://github.com/rust-lang/regex/pull/1051):
Unicode character class operations have been optimized in `regex-syntax`.
* [PERF #1090](https://github.com/rust-lang/regex/issues/1090):
Make patterns containing lots of literal characters use less memory.

Bug fixes:

* [BUG #1046](https://github.com/rust-lang/regex/issues/1046):
Fix a bug that could result in incorrect match spans when using a Unicode word
boundary and searching non-ASCII strings.
* [BUG(regex-syntax) #1047](https://github.com/rust-lang/regex/issues/1047):
Fix panics that can occur in `Ast->Hir` translation (not reachable from `regex`
crate).
* [BUG(regex-syntax) #1088](https://github.com/rust-lang/regex/issues/1088):
Remove guarantees in the API that connect the `u` flag with a specific HIR
representation.

`regex-automata` breaking change release:

This release includes a `regex-automata 0.4.0` breaking change release, which
was necessary in order to support the new word boundary assertions. For
example, the `Look` enum has new variants and the `LookSet` type now uses `u32`
instead of `u16` to represent a bitset of look-around assertions. These are
overall very minor changes, and most users of `regex-automata` should be able
to move to `0.4` from `0.3` without any changes at all.

`regex-syntax` breaking change release:

This release also includes a `regex-syntax 0.8.0` breaking change release,
which, like `regex-automata`, was necessary in order to support the new word
boundary assertions. This release also includes some changes to the `Ast`
type to reduce heap usage in some cases. If you are using the `Ast` type
directly, your code may require some minor modifications. Otherwise, users of
`regex-syntax 0.7` should be able to migrate to `0.8` without any code changes.

`regex-lite` release:

The `regex-lite 0.1.1` release contains support for the new word boundary
assertions. There are no breaking changes.


1.9.6 (2023-09-30)
==================
This is a patch release that fixes a panic that can occur when the default
regex size limit is increased to a large number.

* [BUG aa4e4c71](https://github.com/rust-lang/regex/commit/aa4e4c7120b0090ce0624e3c42a2ed06dd8b918a):
Fix a bug where computing the maximum haystack length for the bounded
backtracker could result underflow and thus provoke a panic later in a search
due to a broken invariant.


1.9.5 (2023-09-02)
==================
This is a patch release that hopefully mostly fixes a performance bug that
occurs when sharing a regex across multiple threads.

Issue [#934](https://github.com/rust-lang/regex/issues/934)
explains this in more detail. It is [also noted in the crate
documentation](https://docs.rs/regex/latest/regex/#sharing-a-regex-across-threads-can-result-in-contention).
The bug can appear when sharing a regex across multiple threads simultaneously,
as might be the case when using a regex from a `OnceLock`, `lazy_static` or
similar primitive. Usually high contention only results when using many threads
to execute searches on small haystacks.

One can avoid the contention problem entirely through one of two methods.
The first is to use lower level APIs from `regex-automata` that require passing
state explicitly, such as [`meta::Regex::search_with`](https://docs.rs/regex-automata/latest/regex_automata/meta/struct.Regex.html#method.search_with).
The second is to clone a regex and send it to other threads explicitly. This
will not use any additional memory usage compared to sharing the regex. The
only downside of this approach is that it may be less convenient, for example,
it won't work with things like `OnceLock` or `lazy_static` or `once_cell`.

With that said, as of this release, the contention performance problems have
been greatly reduced. This was achieved by changing the free-list so that it
was sharded across threads, and that ensuring each sharded mutex occupies a
single cache line to mitigate false sharing. So while contention may still
impact performance in some cases, it should be a lot better now.

Because of the changes to how the free-list works, please report any issues you
find with this release. That not only includes search time regressions but also
significant regressions in memory usage. Reporting improvements is also welcome
as well! If possible, provide a reproduction.

Bug fixes:

* [BUG #934](https://github.com/rust-lang/regex/issues/934):
Fix a performance bug where high contention on a single regex led to massive
slow downs.


1.9.4 (2023-08-26)
==================
This is a patch release that fixes a bug where `RegexSet::is_match(..)` could
incorrectly return false (even when `RegexSet::matches(..).matched_any()`
returns true).

Bug fixes:

* [BUG #1070](https://github.com/rust-lang/regex/issues/1070):
Fix a bug where a prefilter was incorrectly configured for a `RegexSet`.


1.9.3 (2023-08-05)
==================
This is a patch release that fixes a bug where some searches could result in
incorrect match offsets being reported. It is difficult to characterize the
types of regexes susceptible to this bug. They generally involve patterns
that contain no prefix or suffix literals, but have an inner literal along with
a regex prefix that can conditionally match.

Bug fixes:

* [BUG #1060](https://github.com/rust-lang/regex/issues/1060):
Fix a bug with the reverse inner literal optimization reporting incorrect match
offsets.


1.9.2 (2023-08-05)
==================
This is a patch release that fixes another memory usage regression. This
particular regression occurred only when using a `RegexSet`. In some cases,
much more heap memory (by one or two orders of magnitude) was allocated than in
versions prior to 1.9.0.

Bug fixes:

* [BUG #1059](https://github.com/rust-lang/regex/issues/1059):
Fix a memory usage regression when using a `RegexSet`.


1.9.1 (2023-07-07)
==================
This is a patch release which fixes a memory usage regression. In the regex
1.9 release, one of the internal engines used a more aggressive allocation
strategy than what was done previously. This patch release reverts to the
prior on-demand strategy.

Bug fixes:

* [BUG #1027](https://github.com/rust-lang/regex/issues/1027):
Change the allocation strategy for the backtracker to be less aggressive.


1.9.0 (2023-07-05)
==================
This release marks the end of a [years long rewrite of the regex crate
internals](https://github.com/rust-lang/regex/issues/656). Since this is
such a big release, please report any issues or regressions you find. We would
also love to hear about improvements as well.

In addition to many internal improvements that should hopefully result in
"my regex searches are faster," there have also been a few API additions:

* A new `Captures::extract` method for quickly accessing the substrings
that match each capture group in a regex.
* A new inline flag, `R`, which enables CRLF mode. This makes `.` match any
Unicode scalar value except for `\r` and `\n`, and also makes `(?m:^)` and
`(?m:$)` match after and before both `\r` and `\n`, respectively, but never
between a `\r` and `\n`.
* `RegexBuilder::line_terminator` was added to further customize the line
terminator used by `(?m:^)` and `(?m:$)` to be any arbitrary byte.
* The `std` Cargo feature is now actually optional. That is, the `regex` crate
can be used without the standard library.
* Because `regex 1.9` may make binary size and compile times even worse, a
new experimental crate called `regex-lite` has been published. It prioritizes
binary size and compile times over functionality (like Unicode) and
performance. It shares no code with the `regex` crate.

New features:

* [FEATURE #244](https://github.com/rust-lang/regex/issues/244):
One can opt into CRLF mode via the `R` flag.
e.g., `(?mR:$)` matches just before `\r\n`.
* [FEATURE #259](https://github.com/rust-lang/regex/issues/259):
Multi-pattern searches with offsets can be done with `regex-automata 0.3`.
* [FEATURE #476](https://github.com/rust-lang/regex/issues/476):
`std` is now an optional feature. `regex` may be used with only `alloc`.
* [FEATURE #644](https://github.com/rust-lang/regex/issues/644):
`RegexBuilder::line_terminator` configures how `(?m:^)` and `(?m:$)` behave.
* [FEATURE #675](https://github.com/rust-lang/regex/issues/675):
Anchored search APIs are now available in `regex-automata 0.3`.
* [FEATURE #824](https://github.com/rust-lang/regex/issues/824):
Add new `Captures::extract` method for easier capture group access.
* [FEATURE #961](https://github.com/rust-lang/regex/issues/961):
Add `regex-lite` crate with smaller binary sizes and faster compile times.
* [FEATURE #1022](https://github.com/rust-lang/regex/pull/1022):
Add `TryFrom` implementations for the `Regex` type.

Performance improvements:

* [PERF #68](https://github.com/rust-lang/regex/issues/68):
Added a one-pass DFA engine for faster capture group matching.
* [PERF #510](https://github.com/rust-lang/regex/issues/510):
Inner literals are now used to accelerate searches, e.g., `\w+@\w+` will scan
for `@`.
* [PERF #787](https://github.com/rust-lang/regex/issues/787),
[PERF #891](https://github.com/rust-lang/regex/issues/891):
Makes literal optimizations apply to regexes of the form `\b(foo|bar|quux)\b`.

(There are many more performance improvements as well, but not all of them have
specific issues devoted to them.)

Bug fixes:

* [BUG #429](https://github.com/rust-lang/regex/issues/429):
Fix matching bugs related to `\B` and inconsistencies across internal engines.
* [BUG #517](https://github.com/rust-lang/regex/issues/517):
Fix matching bug with capture groups.
* [BUG #579](https://github.com/rust-lang/regex/issues/579):
Fix matching bug with word boundaries.
* [BUG #779](https://github.com/rust-lang/regex/issues/779):
Fix bug where some regexes like `(re)+` were not equivalent to `(re)(re)*`.
* [BUG #850](https://github.com/rust-lang/regex/issues/850):
Fix matching bug inconsistency between NFA and DFA engines.
* [BUG #921](https://github.com/rust-lang/regex/issues/921):
Fix matching bug where literal extraction got confused by `$`.
* [BUG #976](https://github.com/rust-lang/regex/issues/976):
Add documentation to replacement routines about dealing with fallibility.
* [BUG #1002](https://github.com/rust-lang/regex/issues/1002):
Use corpus rejection in fuzz testing.


1.8.4 (2023-06-05)
==================
This is a patch release that fixes a bug where `(?-u:\B)` was allowed in
Unicode regexes, despite the fact that the current matching engines can report
match offsets between the code units of a single UTF-8 encoded codepoint. That
in turn means that match offsets that split a codepoint could be reported,
which in turn results in panicking when one uses them to slice a `&str`.

This bug occurred in the transition to `regex 1.8` because the underlying
syntactical error that prevented this regex from compiling was intentionally
removed. That's because `(?-u:\B)` will be permitted in Unicode regexes in
`regex 1.9`, but the matching engines will guarantee to never report match
offsets that split a codepoint. When the underlying syntactical error was
removed, no code was added to ensure that `(?-u:\B)` didn't compile in the
`regex 1.8` transition release. This release, `regex 1.8.4`, adds that code
such that `Regex::new(r"(?-u:\B)")` returns to the `regex <1.8` behavior of
not compiling. (A `bytes::Regex` can still of course compile it.)

Bug fixes:

* [BUG #1006](https://github.com/rust-lang/regex/issues/1006):
Fix a bug where `(?-u:\B)` was allowed in Unicode regexes, and in turn could
lead to match offsets that split a codepoint in `&str`.


1.8.3 (2023-05-25)
==================
This is a patch release that fixes a bug where the regex would report a
match at every position even when it shouldn't. This could occur in a very
small subset of regexes, usually an alternation of simple literals that
have particular properties. (See the issue linked below for a more precise
description.)

Bug fixes:

* [BUG #999](https://github.com/rust-lang/regex/issues/999):
Fix a bug where a match at every position is erroneously reported.


1.8.2 (2023-05-22)
==================
This is a patch release that fixes a bug where regex compilation could panic
in debug mode for regexes with large counted repetitions. For example,
`a{2147483516}{2147483416}{5}` resulted in an integer overflow that wrapped
in release mode but panicking in debug mode. Despite the unintended wrapping
arithmetic in release mode, it didn't cause any other logical bugs since the
errant code was for new analysis that wasn't used yet.

Bug fixes:

* [BUG #995](https://github.com/rust-lang/regex/issues/995):
Fix a bug where regex compilation with large counted repetitions could panic.


1.8.1 (2023-04-21)
==================
This is a patch release that fixes a bug where a regex match could be reported
where none was found. Specifically, the bug occurs when a pattern contains some
literal prefixes that could be extracted _and_ an optional word boundary in the
prefix.

Bug fixes:

* [BUG #981](https://github.com/rust-lang/regex/issues/981):
Fix a bug where a word boundary could interact with prefix literal
optimizations and lead to a false positive match.


1.8.0 (2023-04-20)
==================
This is a sizeable release that will be soon followed by another sizeable
release. Both of them will combined close over 40 existing issues and PRs.

This first release, despite its size, essentially represents preparatory work
for the second release, which will be even bigger. Namely, this release:

* Increases the MSRV to Rust 1.60.0, which was released about 1 year ago.
* Upgrades its dependency on `aho-corasick` to the recently released 1.0
version.
* Upgrades its dependency on `regex-syntax` to the simultaneously released
`0.7` version. The changes to `regex-syntax` principally revolve around a
rewrite of its literal extraction code and a number of simplifications and
optimizations to its high-level intermediate representation (HIR).

The second release, which will follow ~shortly after the release above, will
contain a soup-to-nuts rewrite of every regex engine. This will be done by
bringing [`regex-automata`](https://github.com/BurntSushi/regex-automata) into
this repository, and then changing the `regex` crate to be nothing but an API
shim layer on top of `regex-automata`'s API.

These tandem releases are the culmination of about 3
years of on-and-off work that [began in earnest in March
2020](https://github.com/rust-lang/regex/issues/656).

Because of the scale of changes involved in these releases, I would love to
hear about your experience. Especially if you notice undocumented changes in
behavior or performance changes (positive *or* negative).

Most changes in the first release are listed below. For more details, please
see the commit log, which reflects a linear and decently documented history
of all changes.

New features:

* [FEATURE #501](https://github.com/rust-lang/regex/issues/501):
Permit many more characters to be escaped, even if they have no significance.
More specifically, any ASCII character except for `[0-9A-Za-z<>]` can now be
escaped. Also, a new routine, `is_escapeable_character`, has been added to
`regex-syntax` to query whether a character is escapeable or not.
* [FEATURE #547](https://github.com/rust-lang/regex/issues/547):
Add `Regex::captures_at`. This fills a hole in the API, but doesn't otherwise
introduce any new expressive power.
* [FEATURE #595](https://github.com/rust-lang/regex/issues/595):
Capture group names are now Unicode-aware. They can now begin with either a `_`
or any "alphabetic" codepoint. After the first codepoint, subsequent codepoints
can be any sequence of alpha-numeric codepoints, along with `_`, `.`, `[` and
`]`. Note that replacement syntax has not changed.
* [FEATURE #810](https://github.com/rust-lang/regex/issues/810):
Add `Match::is_empty` and `Match::len` APIs.
* [FEATURE #905](https://github.com/rust-lang/regex/issues/905):
Add an `impl Default for RegexSet`, with the default being the empty set.
* [FEATURE #908](https://github.com/rust-lang/regex/issues/908):
A new method, `Regex::static_captures_len`, has been added which returns the
number of capture groups in the pattern if and only if every possible match
always contains the same number of matching groups.
* [FEATURE #955](https://github.com/rust-lang/regex/issues/955):
Named captures can now be written as `(?<name>re)` in addition to
`(?P<name>re)`.
* FEATURE: `regex-syntax` now supports empty character classes.
* FEATURE: `regex-syntax` now has an optional `std` feature. (This will come
to `regex` in the second release.)
* FEATURE: The `Hir` type in `regex-syntax` has had a number of simplifications
made to it.
* FEATURE: `regex-syntax` has support for a new `R` flag for enabling CRLF
mode. This will be supported in `regex` proper in the second release.
* FEATURE: `regex-syntax` now has proper support for "regex that never
matches" via `Hir::fail()`.
* FEATURE: The `hir::literal` module of `regex-syntax` has been completely
re-worked. It now has more documentation, examples and advice.
* FEATURE: The `allow_invalid_utf8` option in `regex-syntax` has been renamed
to `utf8`, and the meaning of the boolean has been flipped.

Performance improvements:

* PERF: The upgrade to `aho-corasick 1.0` may improve performance in some
cases. It's difficult to characterize exactly which patterns this might impact,
but if there are a small number of longish (>= 4 bytes) prefix literals, then
it might be faster than before.

Bug fixes:

* [BUG #514](https://github.com/rust-lang/regex/issues/514):
Improve `Debug` impl for `Match` so that it doesn't show the entire haystack.
* BUGS [#516](https://github.com/rust-lang/regex/issues/516),
[#731](https://github.com/rust-lang/regex/issues/731):
Fix a number of issues with printing `Hir` values as regex patterns.
* [BUG #610](https://github.com/rust-lang/regex/issues/610):
Add explicit example of `foo|bar` in the regex syntax docs.
* [BUG #625](https://github.com/rust-lang/regex/issues/625):
Clarify that `SetMatches::len` does not (regretably) refer to the number of
matches in the set.
* [BUG #660](https://github.com/rust-lang/regex/issues/660):
Clarify "verbose mode" in regex syntax documentation.
* BUG [#738](https://github.com/rust-lang/regex/issues/738),
[#950](https://github.com/rust-lang/regex/issues/950):
Fix `CaptureLocations::get` so that it never panics.
* [BUG #747](https://github.com/rust-lang/regex/issues/747):
Clarify documentation for `Regex::shortest_match`.
* [BUG #835](https://github.com/rust-lang/regex/issues/835):
Fix `\p{Sc}` so that it is equivalent to `\p{Currency_Symbol}`.
* [BUG #846](https://github.com/rust-lang/regex/issues/846):
Add more clarifying documentation to the `CompiledTooBig` error variant.
* [BUG #854](https://github.com/rust-lang/regex/issues/854):
Clarify that `regex::Regex` searches as if the haystack is a sequence of
Unicode scalar values.
* [BUG #884](https://github.com/rust-lang/regex/issues/884):
Replace `__Nonexhaustive` variants with `#[non_exhaustive]` attribute.
* [BUG #893](https://github.com/rust-lang/regex/pull/893):
Optimize case folding since it can get quite slow in some pathological cases.
* [BUG #895](https://github.com/rust-lang/regex/issues/895):
Reject `(?-u:\W)` in `regex::Regex` APIs.
* [BUG #942](https://github.com/rust-lang/regex/issues/942):
Add a missing `void` keyword to indicate "no parameters" in C API.
* [BUG #965](https://github.com/rust-lang/regex/issues/965):
Fix `\p{Lc}` so that it is equivalent to `\p{Cased_Letter}`.
* [BUG #975](https://github.com/rust-lang/regex/issues/975):
Clarify documentation for `\pX` syntax.


1.7.3 (2023-03-24)
==================
This is a small release that fixes a bug in `Regex::shortest_match_at` that
could cause it to panic, even when the offset given is valid.

Bug fixes:

* [BUG #969](https://github.com/rust-lang/regex/issues/969):
  Fix a bug in how the reverse DFA was called for `Regex::shortest_match_at`.


1.7.2 (2023-03-21)
==================
This is a small release that fixes a failing test on FreeBSD.

Bug fixes:

* [BUG #967](https://github.com/rust-lang/regex/issues/967):
  Fix "no stack overflow" test which can fail due to the small stack size.


1.7.1 (2023-01-09)
==================
This release was done principally to try and fix the doc.rs rendering for the
regex crate.

Performance improvements:

* [PERF #930](https://github.com/rust-lang/regex/pull/930):
  Optimize `replacen`. This also applies to `replace`, but not `replace_all`.

Bug fixes:

* [BUG #945](https://github.com/rust-lang/regex/issues/945):
  Maybe fix rustdoc rendering by just bumping a new release?


1.7.0 (2022-11-05)
==================
This release principally includes an upgrade to Unicode 15.

New features:

* [FEATURE #832](https://github.com/rust-lang/regex/issues/916):
  Upgrade to Unicode 15.


1.6.0 (2022-07-05)
==================
This release principally includes an upgrade to Unicode 14.

New features:

* [FEATURE #832](https://github.com/rust-lang/regex/pull/832):
  Clarify that `Captures::len` includes all groups, not just matching groups.
* [FEATURE #857](https://github.com/rust-lang/regex/pull/857):
  Add an `ExactSizeIterator` impl for `SubCaptureMatches`.
* [FEATURE #861](https://github.com/rust-lang/regex/pull/861):
  Improve `RegexSet` documentation examples.
* [FEATURE #877](https://github.com/rust-lang/regex/issues/877):
  Upgrade to Unicode 14.

Bug fixes:

* [BUG #792](https://github.com/rust-lang/regex/issues/792):
  Fix error message rendering bug.


1.5.6 (2022-05-20)
==================
This release includes a few bug fixes, including a bug that produced incorrect
matches when a non-greedy `?` operator was used.

* [BUG #680](https://github.com/rust-lang/regex/issues/680):
  Fixes a bug where `[[:alnum:][:^ascii:]]` dropped `[:alnum:]` from the class.
* [BUG #859](https://github.com/rust-lang/regex/issues/859):
  Fixes a bug where `Hir::is_match_empty` returned `false` for `\b`.
* [BUG #862](https://github.com/rust-lang/regex/issues/862):
  Fixes a bug where 'ab??' matches 'ab' instead of 'a' in 'ab'.


1.5.5 (2022-03-08)
==================
This releases fixes a security bug in the regex compiler. This bug permits a
vector for a denial-of-service attack in cases where the regex being compiled
is untrusted. There are no known problems where the regex is itself trusted,
including in cases of untrusted haystacks.

* [SECURITY #GHSA-m5pq-gvj9-9vr8](https://github.com/rust-lang/regex/security/advisories/GHSA-m5pq-gvj9-9vr8):
  Fixes a bug in the regex compiler where empty sub-expressions subverted the
  existing mitigations in place to enforce a size limit on compiled regexes.
  The Rust Security Response WG published an advisory about this:
  https://groups.google.com/g/rustlang-security-announcements/c/NcNNL1Jq7Yw


1.5.4 (2021-05-06)
==================
This release fixes another compilation failure when building regex. This time,
the fix is for when the `pattern` feature is enabled, which only works on
nightly Rust. CI has been updated to test this case.

* [BUG #772](https://github.com/rust-lang/regex/pull/772):
  Fix build when `pattern` feature is enabled.


1.5.3 (2021-05-01)
==================
This releases fixes a bug when building regex with only the `unicode-perl`
feature. It turns out that while CI was building this configuration, it wasn't
actually failing the overall build on a failed compilation.

* [BUG #769](https://github.com/rust-lang/regex/issues/769):
  Fix build in `regex-syntax` when only the `unicode-perl` feature is enabled.


1.5.2 (2021-05-01)
==================
This release fixes a performance bug when Unicode word boundaries are used.
Namely, for certain regexes on certain inputs, it's possible for the lazy DFA
to stop searching (causing a fallback to a slower engine) when it doesn't
actually need to.

[PR #768](https://github.com/rust-lang/regex/pull/768) fixes the bug, which was
originally reported in
[ripgrep#1860](https://github.com/BurntSushi/ripgrep/issues/1860).


1.5.1 (2021-04-30)
==================
This is a patch release that fixes a compilation error when the `perf-literal`
feature is not enabled.


1.5.0 (2021-04-30)
==================
This release primarily updates to Rust 2018 (finally) and bumps the MSRV to
Rust 1.41 (from Rust 1.28). Rust 1.41 was chosen because it's still reasonably
old, and is what's in Debian stable at the time of writing.

This release also drops this crate's own bespoke substring search algorithms
in favor of a new
[`memmem` implementation provided by the `memchr` crate](https://docs.rs/memchr/2.4.0/memchr/memmem/index.html).
This will change the performance profile of some regexes, sometimes getting a
little worse, and hopefully more frequently, getting a lot better. Please
report any serious performance regressions if you find them.


1.4.6 (2021-04-22)
==================
This is a small patch release that fixes the compiler's size check on how much
heap memory a regex uses. Previously, the compiler did not account for the
heap usage of Unicode character classes. Now it does. It's possible that this
may make some regexes fail to compile that previously did compile. If that
happens, please file an issue.

* [BUG OSS-fuzz#33579](https://bugs.chromium.org/p/oss-fuzz/issues/detail?id=33579):
  Some regexes can use more heap memory than one would expect.


1.4.5 (2021-03-14)
==================
This is a small patch release that fixes a regression in the size of a `Regex`
in the 1.4.4 release. Prior to 1.4.4, a `Regex` was 552 bytes. In the 1.4.4
release, it was 856 bytes due to internal changes. In this release, a `Regex`
is now 16 bytes. In general, the size of a `Regex` was never something that was
on my radar, but this increased size in the 1.4.4 release seems to have crossed
a threshold and resulted in stack overflows in some programs.

* [BUG #750](https://github.com/rust-lang/regex/pull/750):
  Fixes stack overflows seemingly caused by a large `Regex` size by decreasing
  its size.


1.4.4 (2021-03-11)
==================
This is a small patch release that contains some bug fixes. Notably, it also
drops the `thread_local` (and `lazy_static`, via transitivity) dependencies.

Bug fixes:

* [BUG #362](https://github.com/rust-lang/regex/pull/362):
  Memory leaks caused by an internal caching strategy should now be fixed.
* [BUG #576](https://github.com/rust-lang/regex/pull/576):
  All regex types now implement `UnwindSafe` and `RefUnwindSafe`.
* [BUG #728](https://github.com/rust-lang/regex/pull/749):
  Add missing `Replacer` impls for `Vec<u8>`, `String`, `Cow`, etc.


1.4.3 (2021-01-08)
==================
This is a small patch release that adds some missing standard trait
implementations for some types in the public API.

Bug fixes:

* [BUG #734](https://github.com/rust-lang/regex/pull/734):
  Add `FusedIterator` and `ExactSizeIterator` impls to iterator types.
* [BUG #735](https://github.com/rust-lang/regex/pull/735):
  Add missing `Debug` impls to public API types.


1.4.2 (2020-11-01)
==================
This is a small bug fix release that bans `\P{any}`. We previously banned empty
classes like `[^\w\W]`, but missed the `\P{any}` case. In the future, we hope
to permit empty classes.

* [BUG #722](https://github.com/rust-lang/regex/issues/722):
  Ban `\P{any}` to avoid a panic in the regex compiler. Found by OSS-Fuzz.


1.4.1 (2020-10-13)
==================
This is a small bug fix release that makes `\p{cf}` work. Previously, it would
report "property not found" even though `cf` is a valid abbreviation for the
`Format` general category.

* [BUG #719](https://github.com/rust-lang/regex/issues/719):
  Fixes bug that prevented `\p{cf}` from working.


1.4.0 (2020-10-11)
==================
This releases has a few minor documentation fixes as well as some very minor
API additions. The MSRV remains at Rust 1.28 for now, but this is intended to
increase to at least Rust 1.41.1 soon.

This release also adds support for OSS-Fuzz. Kudos to
[@DavidKorczynski](https://github.com/DavidKorczynski)
for doing the heavy lifting for that!

New features:

* [FEATURE #649](https://github.com/rust-lang/regex/issues/649):
  Support `[`, `]` and `.` in capture group names.
* [FEATURE #687](https://github.com/rust-lang/regex/issues/687):
  Add `is_empty` predicate to `RegexSet`.
* [FEATURE #689](https://github.com/rust-lang/regex/issues/689):
  Implement `Clone` for `SubCaptureMatches`.
* [FEATURE #715](https://github.com/rust-lang/regex/issues/715):
  Add `empty` constructor to `RegexSet` for convenience.

Bug fixes:

* [BUG #694](https://github.com/rust-lang/regex/issues/694):
  Fix doc example for `Replacer::replace_append`.
* [BUG #698](https://github.com/rust-lang/regex/issues/698):
  Clarify docs for `s` flag when using a `bytes::Regex`.
* [BUG #711](https://github.com/rust-lang/regex/issues/711):
  Clarify `is_match` docs to indicate that it can match anywhere in string.


1.3.9 (2020-05-28)
==================
This release fixes a MSRV (Minimum Support Rust Version) regression in the
1.3.8 release. Namely, while 1.3.8 compiles on Rust 1.28, it actually does not
compile on other Rust versions, such as Rust 1.39.

Bug fixes:

* [BUG #685](https://github.com/rust-lang/regex/issues/685):
  Remove use of `doc_comment` crate, which cannot be used before Rust 1.43.


1.3.8 (2020-05-28)
==================
This release contains a couple of important bug fixes driven
by better support for empty-subexpressions in regexes. For
example, regexes like `b|` are now allowed. Major thanks to
[@sliquister](https://github.com/sliquister) for implementing support for this
in [#677](https://github.com/rust-lang/regex/pull/677).

Bug fixes:

* [BUG #523](https://github.com/rust-lang/regex/pull/523):
  Add note to documentation that spaces can be escaped in `x` mode.
* [BUG #524](https://github.com/rust-lang/regex/issues/524):
  Add support for empty sub-expressions, including empty alternations.
* [BUG #659](https://github.com/rust-lang/regex/issues/659):
  Fix match bug caused by an empty sub-expression miscompilation.


1.3.7 (2020-04-17)
==================
This release contains a small bug fix that fixes how `regex` forwards crate
features to `regex-syntax`. In particular, this will reduce recompilations in
some cases.

Bug fixes:

* [BUG #665](https://github.com/rust-lang/regex/pull/665):
  Fix feature forwarding to `regex-syntax`.


1.3.6 (2020-03-24)
==================
This release contains a sizable (~30%) performance improvement when compiling
some kinds of large regular expressions.

Performance improvements:

* [PERF #657](https://github.com/rust-lang/regex/pull/657):
  Improvement performance of compiling large regular expressions.


1.3.5 (2020-03-12)
==================
This release updates this crate to Unicode 13.

New features:

* [FEATURE #653](https://github.com/rust-lang/regex/pull/653):
  Update `regex-syntax` to Unicode 13.


1.3.4 (2020-01-30)
==================
This is a small bug fix release that fixes a bug related to the scoping of
flags in a regex. Namely, before this fix, a regex like `((?i)a)b)` would
match `aB` despite the fact that `b` should not be matched case insensitively.

Bug fixes:

* [BUG #640](https://github.com/rust-lang/regex/issues/640):
  Fix bug related to the scoping of flags in a regex.


1.3.3 (2020-01-09)
==================
This is a small maintenance release that upgrades the dependency on
`thread_local` from `0.3` to `1.0`. The minimum supported Rust version remains
at Rust 1.28.


1.3.2 (2020-01-09)
==================
This is a small maintenance release with some house cleaning and bug fixes.

New features:

* [FEATURE #631](https://github.com/rust-lang/regex/issues/631):
  Add a `Match::range` method an a `From<Match> for Range` impl.

Bug fixes:

* [BUG #521](https://github.com/rust-lang/regex/issues/521):
  Corrects `/-/.splitn("a", 2)` to return `["a"]` instead of `["a", ""]`.
* [BUG #594](https://github.com/rust-lang/regex/pull/594):
  Improve error reporting when writing `\p\`.
* [BUG #627](https://github.com/rust-lang/regex/issues/627):
  Corrects `/-/.split("a-")` to return `["a", ""]` instead of `["a"]`.
* [BUG #633](https://github.com/rust-lang/regex/pull/633):
  Squash deprecation warnings for the `std::error::Error::description` method.


1.3.1 (2019-09-04)
==================
This is a maintenance release with no changes in order to try to work-around
a [docs.rs/Cargo issue](https://github.com/rust-lang/docs.rs/issues/400).


1.3.0 (2019-09-03)
==================
This release adds a plethora of new crate features that permit users of regex
to shrink its size considerably, in exchange for giving up either functionality
(such as Unicode support) or runtime performance. When all such features are
disabled, the dependency tree for `regex` shrinks to exactly 1 crate
(`regex-syntax`). More information about the new crate features can be
[found in the docs](https://docs.rs/regex/*/#crate-features).

Note that while this is a new minor version release, the minimum supported
Rust version for this crate remains at `1.28.0`.

New features:

* [FEATURE #474](https://github.com/rust-lang/regex/issues/474):
  The `use_std` feature has been deprecated in favor of the `std` feature.
  The `use_std` feature will be removed in regex 2. Until then, `use_std` will
  remain as an alias for the `std` feature.
* [FEATURE #583](https://github.com/rust-lang/regex/issues/583):
  Add a substantial number of crate features shrinking `regex`.


1.2.1 (2019-08-03)
==================
This release does a bit of house cleaning. Namely:

* This repository is now using rustfmt.
* License headers have been removed from all files, in following suit with the
  Rust project.
* Teddy has been removed from the `regex` crate, and is now part of the
  `aho-corasick` crate.
  [See `aho-corasick`'s new `packed` sub-module for details](https://docs.rs/aho-corasick/0.7.6/aho_corasick/packed/index.html).
* The `utf8-ranges` crate has been deprecated, with its functionality moving
  into the
  [`utf8` sub-module of `regex-syntax`](https://docs.rs/regex-syntax/0.6.11/regex_syntax/utf8/index.html).
* The `ucd-util` dependency has been dropped, in favor of implementing what
  little we need inside of `regex-syntax` itself.

In general, this is part of an ongoing (long term) effort to make optimizations
in the regex engine easier to reason about. The current code is too convoluted
and thus it is very easy to introduce new bugs. This simplification effort is
the primary motivation behind re-working the `aho-corasick` crate to not only
bundle algorithms like Teddy, but to also provide regex-like match semantics
automatically.

Moving forward, the plan is to join up with the `bstr` and `regex-automata`
crates, with the former providing more sophisticated substring search
algorithms (thereby deleting existing code in `regex`) and the latter providing
ahead-of-time compiled DFAs for cases where they are inexpensive to compute.


1.2.0 (2019-07-20)
==================
This release updates regex's minimum supported Rust version to 1.28, which was
release almost 1 year ago. This release also updates regex's Unicode data
tables to 12.1.0.


1.1.9 (2019-07-06)
==================
This release contains a bug fix that caused regex's tests to fail, due to a
dependency on an unreleased behavior in regex-syntax.

* [BUG #593](https://github.com/rust-lang/regex/issues/593):
  Move an integration-style test on error messages into regex-syntax.


1.1.8 (2019-07-04)
==================
This release contains a few small internal refactorings. One of which fixes
an instance of undefined behavior in a part of the SIMD code.

Bug fixes:

* [BUG #545](https://github.com/rust-lang/regex/issues/545):
  Improves error messages when a repetition operator is used without a number.
* [BUG #588](https://github.com/rust-lang/regex/issues/588):
  Removes use of a repr(Rust) union used for type punning in the Teddy matcher.
* [BUG #591](https://github.com/rust-lang/regex/issues/591):
  Update docs for running benchmarks and improve failure modes.


1.1.7 (2019-06-09)
==================
This release fixes up a few warnings as a result of recent deprecations.


1.1.6 (2019-04-16)
==================
This release fixes a regression introduced by a bug fix (for
[BUG #557](https://github.com/rust-lang/regex/issues/557)) which could cause
the regex engine to enter an infinite loop. This bug was originally
[reported against ripgrep](https://github.com/BurntSushi/ripgrep/issues/1247).


1.1.5 (2019-04-01)
==================
This release fixes a bug in regex's dependency specification where it requires
a newer version of regex-syntax, but this wasn't communicated correctly in the
Cargo.toml. This would have been caught by a minimal version check, but this
check was disabled because the `rand` crate itself advertises incorrect
dependency specifications.

Bug fixes:

* [BUG #570](https://github.com/rust-lang/regex/pull/570):
  Fix regex-syntax minimal version.


1.1.4 (2019-03-31)
==================
This release fixes a backwards compatibility regression where Regex was no
longer UnwindSafe. This was caused by the upgrade to aho-corasick 0.7, whose
AhoCorasick type was itself not UnwindSafe. This has been fixed in aho-corasick
0.7.4, which we now require.

Bug fixes:

* [BUG #568](https://github.com/rust-lang/regex/pull/568):
  Fix an API regression where Regex was no longer UnwindSafe.


1.1.3 (2019-03-30)
==================
This releases fixes a few bugs and adds a performance improvement when a regex
is a simple alternation of literals.

Performance improvements:

* [OPT #566](https://github.com/rust-lang/regex/pull/566):
  Upgrades `aho-corasick` to 0.7 and uses it for `foo|bar|...|quux` regexes.

Bug fixes:

* [BUG #527](https://github.com/rust-lang/regex/issues/527):
  Fix a bug where the parser would panic on patterns like `((?x))`.
* [BUG #555](https://github.com/rust-lang/regex/issues/555):
  Fix a bug where the parser would panic on patterns like `(?m){1,1}`.
* [BUG #557](https://github.com/rust-lang/regex/issues/557):
  Fix a bug where captures could lead to an incorrect match.


1.1.2 (2019-02-27)
==================
This release fixes a bug found in the fix introduced in 1.1.1.

Bug fixes:

* [BUG edf45e6f](https://github.com/rust-lang/regex/commit/edf45e6f):
  Fix bug introduced in reverse suffix literal matcher in the 1.1.1 release.


1.1.1 (2019-02-27)
==================
This is a small release with one fix for a bug caused by literal optimizations.

Bug fixes:

* [BUG 661bf53d](https://github.com/rust-lang/regex/commit/661bf53d):
  Fixes a bug in the reverse suffix literal optimization. This was originally
  reported
  [against ripgrep](https://github.com/BurntSushi/ripgrep/issues/1203).


1.1.0 (2018-11-30)
==================
This is a small release with a couple small enhancements. This release also
increases the minimal supported Rust version (MSRV) to 1.24.1 (from 1.20.0). In
accordance with this crate's MSRV policy, this release bumps the minor version
number.

Performance improvements:

* [OPT #511](https://github.com/rust-lang/regex/pull/511),
  [OPT #540](https://github.com/rust-lang/regex/pull/540):
  Improve lazy DFA construction for large regex sets.

New features:

* [FEATURE #538](https://github.com/rust-lang/regex/pull/538):
  Add Emoji and "break" Unicode properties. See [UNICODE.md](UNICODE.md).

Bug fixes:

* [BUG #530](https://github.com/rust-lang/regex/pull/530):
  Add Unicode license (for data tables).
* Various typo/doc fixups.


1.0.6 (2018-11-06)
==================
This is a small release.

Performance improvements:

* [OPT #513](https://github.com/rust-lang/regex/pull/513):
  Improve performance of compiling large Unicode classes by 8-10%.

Bug fixes:

* [BUG #533](https://github.com/rust-lang/regex/issues/533):
  Fix definition of `[[:blank:]]` class that regressed in `regex-syntax 0.5`.


1.0.5 (2018-09-06)
==================
This is a small release with an API enhancement.

New features:

* [FEATURE #509](https://github.com/rust-lang/regex/pull/509):
  Generalize impls of the `Replacer` trait.


1.0.4 (2018-08-25)
==================
This is a small release that bumps the quickcheck dependency.


1.0.3 (2018-08-24)
==================
This is a small bug fix release.

Bug fixes:

* [BUG #504](https://github.com/rust-lang/regex/pull/504):
  Fix for Cargo's "minimal version" support.
* [BUG 1e39165f](https://github.com/rust-lang/regex/commit/1e39165f):
  Fix doc examples for byte regexes.


1.0.2 (2018-07-18)
==================
This release exposes some new lower level APIs on `Regex` that permit
amortizing allocation and controlling the location at which a search is
performed in a more granular way. Most users of the regex crate will not
need or want to use these APIs.

New features:

* [FEATURE #493](https://github.com/rust-lang/regex/pull/493):
  Add a few lower level APIs for amortizing allocation and more fine grained
  searching.

Bug fixes:

* [BUG 3981d2ad](https://github.com/rust-lang/regex/commit/3981d2ad):
  Correct outdated documentation on `RegexBuilder::dot_matches_new_line`.
* [BUG 7ebe4ae0](https://github.com/rust-lang/regex/commit/7ebe4ae0):
  Correct outdated documentation on `Parser::allow_invalid_utf8` in the
  `regex-syntax` crate.
* [BUG 24c7770b](https://github.com/rust-lang/regex/commit/24c7770b):
  Fix a bug in the HIR printer where it wouldn't correctly escape meta
  characters in character classes.


1.0.1 (2018-06-19)
==================
This release upgrades regex's Unicode tables to Unicode 11, and enables SIMD
optimizations automatically on Rust stable (1.27 or newer).

New features:

* [FEATURE #486](https://github.com/rust-lang/regex/pull/486):
  Implement `size_hint` on `RegexSet` match iterators.
* [FEATURE #488](https://github.com/rust-lang/regex/pull/488):
  Update Unicode tables for Unicode 11.
* [FEATURE #490](https://github.com/rust-lang/regex/pull/490):
  SIMD optimizations are now enabled automatically in Rust stable, for versions
  1.27 and up. No compilation flags or features need to be set. CPU support
  SIMD is detected automatically at runtime.

Bug fixes:

* [BUG #482](https://github.com/rust-lang/regex/pull/482):
  Present a better compilation error when the `use_std` feature isn't used.


1.0.0 (2018-05-01)
==================
This release marks the 1.0 release of regex.

While this release includes some breaking changes, most users of older versions
of the regex library should be able to migrate to 1.0 by simply bumping the
version number. The important changes are as follows:

* We adopt Rust 1.20 as the new minimum supported version of Rust for regex.
  We also tentativley adopt a policy that permits bumping the minimum supported
  version of Rust in minor version releases of regex, but no patch releases.
  That is, with respect to semver, we do not strictly consider bumping the
  minimum version of Rust to be a breaking change, but adopt a conservative
  stance as a compromise.
* Octal syntax in regular expressions has been disabled by default. This
  permits better error messages that inform users that backreferences aren't
  available. Octal syntax can be re-enabled via the corresponding option on
  `RegexBuilder`.
* `(?-u:\B)` is no longer allowed in Unicode regexes since it can match at
  invalid UTF-8 code unit boundaries. `(?-u:\b)` is still allowed in Unicode
  regexes.
* The `From<regex_syntax::Error>` impl has been removed. This formally removes
  the public dependency on `regex-syntax`.
* A new feature, `use_std`, has been added and enabled by default. Disabling
  the feature will result in a compilation error. In the future, this may
  permit us to support `no_std` environments (w/ `alloc`) in a backwards
  compatible way.

For more information and discussion, please see
[1.0 release tracking issue](https://github.com/rust-lang/regex/issues/457).


0.2.11 (2018-05-01)
===================
This release primarily contains bug fixes. Some of them resolve bugs where
the parser could panic.

New features:

* [FEATURE #459](https://github.com/rust-lang/regex/pull/459):
  Include C++'s standard regex library and Boost's regex library in the
  benchmark harness. We now include D/libphobos, C++/std, C++/boost, Oniguruma,
  PCRE1, PCRE2, RE2 and Tcl in the harness.

Bug fixes:

* [BUG #445](https://github.com/rust-lang/regex/issues/445):
  Clarify order of indices returned by RegexSet match iterator.
* [BUG #461](https://github.com/rust-lang/regex/issues/461):
  Improve error messages for invalid regexes like `[\d-a]`.
* [BUG #464](https://github.com/rust-lang/regex/issues/464):
  Fix a bug in the error message pretty printer that could cause a panic when
  a regex contained a literal `\n` character.
* [BUG #465](https://github.com/rust-lang/regex/issues/465):
  Fix a panic in the parser that was caused by applying a repetition operator
  to `(?flags)`.
* [BUG #466](https://github.com/rust-lang/regex/issues/466):
  Fix a bug where `\pC` was not recognized as an alias for `\p{Other}`.
* [BUG #470](https://github.com/rust-lang/regex/pull/470):
  Fix a bug where literal searches did more work than necessary for anchored
  regexes.


0.2.10 (2018-03-16)
===================
This release primarily updates the regex crate to changes made in `std::arch`
on nightly Rust.

New features:

* [FEATURE #458](https://github.com/rust-lang/regex/pull/458):
  The `Hir` type in `regex-syntax` now has a printer.


0.2.9 (2018-03-12)
==================
This release introduces a new nightly only feature, `unstable`, which enables
SIMD optimizations for certain types of regexes. No additional compile time
options are necessary, and the regex crate will automatically choose the
best CPU features at run time. As a result, the `simd` (nightly only) crate
dependency has been dropped.

New features:

* [FEATURE #456](https://github.com/rust-lang/regex/pull/456):
  The regex crate now includes AVX2 optimizations in addition to the extant
  SSSE3 optimization.

Bug fixes:

* [BUG #455](https://github.com/rust-lang/regex/pull/455):
  Fix a bug where `(?x)[ / - ]` failed to parse.


0.2.8 (2018-03-12)
==================
Bug gixes:

* [BUG #454](https://github.com/rust-lang/regex/pull/454):
  Fix a bug in the nest limit checker being too aggressive.


0.2.7 (2018-03-07)
==================
This release includes a ground-up rewrite of the regex-syntax crate, which has
been in development for over a year.
731
New features:

* Error messages for invalid regexes have been greatly improved. You get these
  automatically; you don't need to do anything. In addition to better
  formatting, error messages will now explicitly call out the use of look
  around. When regex 1.0 is released, this will happen for backreferences as
  well.
* Full support for intersection, difference and symmetric difference of
  character classes. These can be used via the `&&`, `--` and `~~` binary
  operators within classes.
* A Unicode Level 1 conformat implementation of `\p{..}` character classes.
  Things like `\p{scx:Hira}`, `\p{age:3.2}` or `\p{Changes_When_Casefolded}`
  now work. All property name and value aliases are supported, and properties
  are selected via loose matching. e.g., `\p{Greek}` is the same as
  `\p{G r E e K}`.
* A new `UNICODE.md` document has been added to this repository that
  exhaustively documents support for UTS#18.
* Empty sub-expressions are now permitted in most places. That is, `()+` is
  now a valid regex.
* Almost everything in regex-syntax now uses constant stack space, even when
  performing analysis that requires structural induction. This reduces the risk
  of a user provided regular expression causing a stack overflow.
* [FEATURE #174](https://github.com/rust-lang/regex/issues/174):
  The `Ast` type in `regex-syntax` now contains span information.
* [FEATURE #424](https://github.com/rust-lang/regex/issues/424):
  Support `\u`, `\u{...}`, `\U` and `\U{...}` syntax for specifying code points
  in a regular expression.
* [FEATURE #449](https://github.com/rust-lang/regex/pull/449):
  Add a `Replace::by_ref` adapter for use of a replacer without consuming it.

Bug fixes:

* [BUG #446](https://github.com/rust-lang/regex/issues/446):
  We re-enable the Boyer-Moore literal matcher.


0.2.6 (2018-02-08)
==================
Bug fixes:

* [BUG #446](https://github.com/rust-lang/regex/issues/446):
  Fixes a bug in the new Boyer-Moore searcher that results in a match failure.
  We fix this bug by temporarily disabling Boyer-Moore.


0.2.5 (2017-12-30)
==================
Bug fixes:

* [BUG #437](https://github.com/rust-lang/regex/issues/437):
  Fixes a bug in the new Boyer-Moore searcher that results in a panic.


0.2.4 (2017-12-30)
==================
New features:

* [FEATURE #348](https://github.com/rust-lang/regex/pull/348):
  Improve performance for capture searches on anchored regex.
  (Contributed by @ethanpailes. Nice work!)
* [FEATURE #419](https://github.com/rust-lang/regex/pull/419):
  Expand literal searching to include Tuned Boyer-Moore in some cases.
  (Contributed by @ethanpailes. Nice work!)

Bug fixes:

* [BUG](https://github.com/rust-lang/regex/pull/436):
  The regex compiler plugin has been removed.
* [BUG](https://github.com/rust-lang/regex/pull/436):
  `simd` has been bumped to `0.2.1`, which fixes a Rust nightly build error.
* [BUG](https://github.com/rust-lang/regex/pull/436):
  Bring the benchmark harness up to date.


0.2.3 (2017-11-30)
==================
New features:

* [FEATURE #374](https://github.com/rust-lang/regex/pull/374):
  Add `impl From<Match> for &str`.
* [FEATURE #380](https://github.com/rust-lang/regex/pull/380):
  Derive `Clone` and `PartialEq` on `Error`.
* [FEATURE #400](https://github.com/rust-lang/regex/pull/400):
  Update to Unicode 10.

Bug fixes:

* [BUG #375](https://github.com/rust-lang/regex/issues/375):
  Fix a bug that prevented the bounded backtracker from terminating.
* [BUG #393](https://github.com/rust-lang/regex/issues/393),
  [BUG #394](https://github.com/rust-lang/regex/issues/394):
  Fix bug with `replace` methods for empty matches.


0.2.2 (2017-05-21)
==================
New features:

* [FEATURE #341](https://github.com/rust-lang/regex/issues/341):
  Support nested character classes and intersection operation.
  For example, `[\p{Greek}&&\pL]` matches greek letters and
  `[[0-9]&&[^4]]` matches every decimal digit except `4`.
  (Much thanks to @robinst, who contributed this awesome feature.)

Bug fixes:

* [BUG #321](https://github.com/rust-lang/regex/issues/321):
  Fix bug in literal extraction and UTF-8 decoding.
* [BUG #326](https://github.com/rust-lang/regex/issues/326):
  Add documentation tip about the `(?x)` flag.
* [BUG #333](https://github.com/rust-lang/regex/issues/333):
  Show additional replacement example using curly braces.
* [BUG #334](https://github.com/rust-lang/regex/issues/334):
  Fix bug when resolving captures after a match.
* [BUG #338](https://github.com/rust-lang/regex/issues/338):
  Add example that uses `Captures::get` to API documentation.
* [BUG #353](https://github.com/rust-lang/regex/issues/353):
  Fix RegexSet bug that caused match failure in some cases.
* [BUG #354](https://github.com/rust-lang/regex/pull/354):
  Fix panic in parser when `(?x)` is used.
* [BUG #358](https://github.com/rust-lang/regex/issues/358):
  Fix literal optimization bug with RegexSet.
* [BUG #359](https://github.com/rust-lang/regex/issues/359):
  Fix example code in README.
* [BUG #365](https://github.com/rust-lang/regex/pull/365):
  Fix bug in `rure_captures_len` in the C binding.
* [BUG #367](https://github.com/rust-lang/regex/issues/367):
  Fix byte class bug that caused a panic.


0.2.1
=====
One major bug with `replace_all` has been fixed along with a couple of other
touchups.

* [BUG #312](https://github.com/rust-lang/regex/issues/312):
  Fix documentation for `NoExpand` to reference correct lifetime parameter.
* [BUG #314](https://github.com/rust-lang/regex/issues/314):
  Fix a bug with `replace_all` when replacing a match with the empty string.
* [BUG #316](https://github.com/rust-lang/regex/issues/316):
  Note a missing breaking change from the `0.2.0` CHANGELOG entry.
  (`RegexBuilder::compile` was renamed to `RegexBuilder::build`.)
* [BUG #324](https://github.com/rust-lang/regex/issues/324):
  Compiling `regex` should only require one version of `memchr` crate.


0.2.0
=====
This is a new major release of the regex crate, and is an implementation of the
[regex 1.0 RFC](https://github.com/rust-lang/rfcs/blob/master/text/1620-regex-1.0.md).
We are releasing a `0.2` first, and if there are no major problems, we will
release a `1.0` shortly. For `0.2`, the minimum *supported* Rust version is
1.12.

There are a number of **breaking changes** in `0.2`. They are split into two
types. The first type correspond to breaking changes in regular expression
syntax. The second type correspond to breaking changes in the API.

Breaking changes for regex syntax:

* POSIX character classes now require double bracketing. Previously, the regex
  `[:upper:]` would parse as the `upper` POSIX character class. Now it parses
  as the character class containing the characters `:upper:`. The fix to this
  change is to use `[[:upper:]]` instead. Note that variants like
  `[[:upper:][:blank:]]` continue to work.
* The character `[` must always be escaped inside a character class.
* The characters `&`, `-` and `~` must be escaped if any one of them are
  repeated consecutively. For example, `[&]`, `[\&]`, `[\&\&]`, `[&-&]` are all
  equivalent while `[&&]` is illegal. (The motivation for this and the prior
  change is to provide a backwards compatible path for adding character class
  set notation.)
* A `bytes::Regex` now has Unicode mode enabled by default (like the main
  `Regex` type). This means regexes compiled with `bytes::Regex::new` that
  don't have the Unicode flag set should add `(?-u)` to recover the original
  behavior.

Breaking changes for the regex API:

* `find` and `find_iter` now **return `Match` values instead of
  `(usize, usize)`.** `Match` values have `start` and `end` methods, which
  return the match offsets. `Match` values also have an `as_str` method,
  which returns the text of the match itself.
* The `Captures` type now only provides a single iterator over all capturing
  matches, which should replace uses of `iter` and `iter_pos`. Uses of
  `iter_named` should use the `capture_names` method on `Regex`.
* The `at` method on the `Captures` type has been renamed to `get`, and it
  now returns a `Match`. Similarly, the `name` method on `Captures` now returns
  a `Match`.
* The `replace` methods now return `Cow` values. The `Cow::Borrowed` variant
  is returned when no replacements are made.
* The `Replacer` trait has been completely overhauled. This should only
  impact clients that implement this trait explicitly. Standard uses of
  the `replace` methods should continue to work unchanged. If you implement
  the `Replacer` trait, please consult the new documentation.
* The `quote` free function has been renamed to `escape`.
* The `Regex::with_size_limit` method has been removed. It is replaced by
  `RegexBuilder::size_limit`.
* The `RegexBuilder` type has switched from owned `self` method receivers to
  `&mut self` method receivers. Most uses will continue to work unchanged, but
  some code may require naming an intermediate variable to hold the builder.
* The `compile` method on `RegexBuilder` has been renamed to `build`.
* The free `is_match` function has been removed. It is replaced by compiling
  a `Regex` and calling its `is_match` method.
* The `PartialEq` and `Eq` impls on `Regex` have been dropped. If you relied
  on these impls, the fix is to define a wrapper type around `Regex`, impl
  `Deref` on it and provide the necessary impls.
* The `is_empty` method on `Captures` has been removed. This always returns
  `false`, so its use is superfluous.
* The `Syntax` variant of the `Error` type now contains a string instead of
  a `regex_syntax::Error`. If you were examining syntax errors more closely,
  you'll need to explicitly use the `regex_syntax` crate to re-parse the regex.
* The `InvalidSet` variant of the `Error` type has been removed since it is
  no longer used.
* Most of the iterator types have been renamed to match conventions. If you
  were using these iterator types explicitly, please consult the documentation
  for its new name. For example, `RegexSplits` has been renamed to `Split`.

A number of bugs have been fixed:

* [BUG #151](https://github.com/rust-lang/regex/issues/151):
  The `Replacer` trait has been changed to permit the caller to control
  allocation.
* [BUG #165](https://github.com/rust-lang/regex/issues/165):
  Remove the free `is_match` function.
* [BUG #166](https://github.com/rust-lang/regex/issues/166):
  Expose more knobs (available in `0.1`) and remove `with_size_limit`.
* [BUG #168](https://github.com/rust-lang/regex/issues/168):
  Iterators produced by `Captures` now have the correct lifetime parameters.
* [BUG #175](https://github.com/rust-lang/regex/issues/175):
  Fix a corner case in the parsing of POSIX character classes.
* [BUG #178](https://github.com/rust-lang/regex/issues/178):
  Drop the `PartialEq` and `Eq` impls on `Regex`.
* [BUG #179](https://github.com/rust-lang/regex/issues/179):
  Remove `is_empty` from `Captures` since it always returns false.
* [BUG #276](https://github.com/rust-lang/regex/issues/276):
  Position of named capture can now be retrieved from a `Captures`.
* [BUG #296](https://github.com/rust-lang/regex/issues/296):
  Remove winapi/kernel32-sys dependency on UNIX.
* [BUG #307](https://github.com/rust-lang/regex/issues/307):
  Fix error on emscripten.


0.1.80
======
* [PR #292](https://github.com/rust-lang/regex/pull/292):
  Fixes bug #291, which was introduced by PR #290.

0.1.79
======
* Require regex-syntax 0.3.8.

0.1.78
======
* [PR #290](https://github.com/rust-lang/regex/pull/290):
  Fixes bug #289, which caused some regexes with a certain combination
  of literals to match incorrectly.

0.1.77
======
* [PR #281](https://github.com/rust-lang/regex/pull/281):
  Fixes bug #280 by disabling all literal optimizations when a pattern
  is partially anchored.

0.1.76
======
* Tweak criteria for using the Teddy literal matcher.

0.1.75
======
* [PR #275](https://github.com/rust-lang/regex/pull/275):
  Improves match verification performance in the Teddy SIMD searcher.
* [PR #278](https://github.com/rust-lang/regex/pull/278):
  Replaces slow substring loop in the Teddy SIMD searcher with Aho-Corasick.
* Implemented DoubleEndedIterator on regex set match iterators.

0.1.74
======
* Release regex-syntax 0.3.5 with a minor bug fix.
* Fix bug #272.
* Fix bug #277.
* [PR #270](https://github.com/rust-lang/regex/pull/270):
  Fixes bugs #264, #268 and an unreported where the DFA cache size could be
  drastically under estimated in some cases (leading to high unexpected memory
  usage).

0.1.73
======
* Release `regex-syntax 0.3.4`.
* Bump `regex-syntax` dependency version for `regex` to `0.3.4`.

0.1.72
======
* [PR #262](https://github.com/rust-lang/regex/pull/262):
  Fixes a number of small bugs caught by fuzz testing (AFL).

0.1.71
======
* [PR #236](https://github.com/rust-lang/regex/pull/236):
  Fix a bug in how suffix literals were extracted, which could lead
  to invalid match behavior in some cases.

0.1.70
======
* [PR #231](https://github.com/rust-lang/regex/pull/231):
  Add SIMD accelerated multiple pattern search.
* [PR #228](https://github.com/rust-lang/regex/pull/228):
  Reintroduce the reverse suffix literal optimization.
* [PR #226](https://github.com/rust-lang/regex/pull/226):
  Implements NFA state compression in the lazy DFA.
* [PR #223](https://github.com/rust-lang/regex/pull/223):
  A fully anchored RegexSet can now short-circuit.

0.1.69
======
* [PR #216](https://github.com/rust-lang/regex/pull/216):
  Tweak the threshold for running backtracking.
* [PR #217](https://github.com/rust-lang/regex/pull/217):
  Add upper limit (from the DFA) to capture search (for the NFA).
* [PR #218](https://github.com/rust-lang/regex/pull/218):
  Add rure, a C API.

0.1.68
======
* [PR #210](https://github.com/rust-lang/regex/pull/210):
  Fixed a performance bug in `bytes::Regex::replace` where `extend` was used
  instead of `extend_from_slice`.
* [PR #211](https://github.com/rust-lang/regex/pull/211):
  Fixed a bug in the handling of word boundaries in the DFA.
* [PR #213](https://github.com/rust-lang/pull/213):
  Added RE2 and Tcl to the benchmark harness. Also added a CLI utility from
  running regexes using any of the following regex engines: PCRE1, PCRE2,
  Oniguruma, RE2, Tcl and of course Rust's own regexes.

0.1.67
======
* [PR #201](https://github.com/rust-lang/regex/pull/201):
  Fix undefined behavior in the `regex!` compiler plugin macro.
* [PR #205](https://github.com/rust-lang/regex/pull/205):
  More improvements to DFA performance. Competitive with RE2. See PR for
  benchmarks.
* [PR #209](https://github.com/rust-lang/regex/pull/209):
  Release 0.1.66 was semver incompatible since it required a newer version
  of Rust than previous releases. This PR fixes that. (And `0.1.66` was
  yanked.)

0.1.66
======
* Speculative support for Unicode word boundaries was added to the DFA. This
  should remove the last common case that disqualified use of the DFA.
* An optimization that scanned for suffix literals and then matched the regular
  expression in reverse was removed because it had worst case quadratic time
  complexity. It was replaced with a more limited optimization where, given any
  regex of the form `re$`, it will be matched in reverse from the end of the
  haystack.
* [PR #202](https://github.com/rust-lang/regex/pull/202):
  The inner loop of the DFA was heavily optimized to improve cache locality
  and reduce the overall number of instructions run on each iteration. This
  represents the first use of `unsafe` in `regex` (to elide bounds checks).
* [PR #200](https://github.com/rust-lang/regex/pull/200):
  Use of the `mempool` crate (which used thread local storage) was replaced
  with a faster version of a similar API in @Amanieu's `thread_local` crate.
  It should reduce contention when using a regex from multiple threads
  simultaneously.
* PCRE2 JIT benchmarks were added. A benchmark comparison can be found
  [here](https://gist.github.com/anonymous/14683c01993e91689f7206a18675901b).
  (Includes a comparison with PCRE1's JIT and Oniguruma.)
* A bug where word boundaries weren't being matched correctly in the DFA was
  fixed. This only affected use of `bytes::Regex`.
* [#160](https://github.com/rust-lang/regex/issues/160):
  `Captures` now has a `Debug` impl.
