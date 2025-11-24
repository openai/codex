# fancy-regex

A Rust library for compiling and matching regular expressions. It uses a hybrid
regex implementation designed to support a relatively rich set of features.
In particular, it uses backtracking to implement "fancy" features such as
look-around and backtracking, which are not supported in purely
NFA-based implementations (exemplified by
[RE2](https://github.com/google/re2), and implemented in Rust in the
[regex](https://crates.io/crates/regex) crate).

[![docs](https://docs.rs/fancy-regex/badge.svg)](https://docs.rs/fancy-regex)
[![crate](https://img.shields.io/crates/v/fancy-regex.svg)](https://crates.io/crates/fancy-regex)
[![ci](https://github.com/fancy-regex/fancy-regex/workflows/ci/badge.svg)](https://github.com/fancy-regex/fancy-regex/actions?query=workflow%3Aci)
[![codecov](https://codecov.io/gh/fancy-regex/fancy-regex/branch/main/graph/badge.svg)](https://codecov.io/gh/fancy-regex/fancy-regex)

A goal is to be as efficient as possible. For a given regex, the NFA
implementation has asymptotic running time linear in the length of the
input, while in the general case a backtracking implementation has
exponential blowup. An example given in [Static Analysis for Regular
Expression Exponential Runtime via Substructural
Logics](https://arxiv.org/pdf/1405.7058.pdf) is:

```python
import re
re.compile('(a|b|ab)*bc').match('ab' * 28 + 'ac')
```

In Python (tested on both 2.7 and 3.5), this match takes 91s, and
doubles for each additional repeat of 'ab'.

Thus, many proponents
[advocate](https://swtch.com/~rsc/regexp/regexp1.html) a purely NFA
(nondeterministic finite automaton) based approach. Even so,
backreferences and look-around do add richness to regexes, and they
are commonly used in applications such as syntax highlighting for text
editors. In particular, TextMate's [syntax
definitions](https://manual.macromates.com/en/language_grammars),
based on the [Oniguruma](https://github.com/kkos/oniguruma)
backtracking engine, are now used in a number of other popular
editors, including Sublime Text and Atom. These syntax definitions
routinely use backreferences and look-around. For example, the
following regex captures a single-line Rust raw string:

```
r(#*)".*?"\1
```

There is no NFA that can express this simple and useful pattern. Yet,
a backtracking implementation handles it efficiently.

This package is one of the first that handles both cases well. The
exponential blowup case above is run in 258ns. Thus, it should be a
very appealing alternative for applications that require both richness
and performance.

## A warning about worst-case performance

NFA-based approaches give strong guarantees about worst-case
performance. For regexes that contain "fancy" features such as
backreferences and look-around, this module gives no corresponding
guarantee. If an attacker can control the regular expressions that
will be matched against, they will be able to successfully mount a
denial-of-service attack. Be warned.

See [PERFORMANCE.md](PERFORMANCE.md) for some examples.

## A hybrid approach

One workable approach is to detect the presence of "fancy" features,
and choose either an NFA implementation or a backtracker depending on
whether they are used.

However, this module attempts to be more fine-grained. Instead, it
implements a true hybrid approach. In essence, it is a backtracking VM
(as well explained in [Regular Expression Matching: the Virtual
Machine Approach](https://swtch.com/~rsc/regexp/regexp2.html)) in
which one of the "instructions" in the VM delegates to an inner NFA
implementation (in Rust, the regex crate, though a similar approach
would certainly be possible using RE2 or the Go
[regexp](https://golang.org/pkg/regexp/) package). Then there's an
analysis which decides for each subexpression whether it is "hard", or
can be delegated to the NFA matcher. At the moment, it is eager, and
delegates as much as possible to the NFA engine.

## Theory

**(This section is written in a somewhat informal style; I hope to
expand on it)**

The fundamental idea is that it's a backtracking VM like PCRE, but as
much as possible it delegates to an "inner" RE engine like RE2 (in
this case, the Rust one). For the sublanguage not using fancy
features, the library becomes a thin wrapper.

Otherwise, you do an analysis to figure out what you can delegate and
what you have to backtrack. I was thinking it might be tricky, but
it's actually quite simple. The first phase, you just label each
subexpression as "hard" (groups that get referenced in a backref,
look-around, etc), and bubble that up. You also do a little extra
analysis, mostly determining whether an expression has constant match
length, and the minimum length.

The second phase is top down, and you carry a context, also a boolean
indicating whether it's "hard" or not. Intuitively, a hard context is
one in which the match length will affect future backtracking.

If the subexpression is easy and the context is easy, generate an
instruction in the VM that delegates to the inner NFA implementation.
Otherwise, generate VM code as in a backtracking engine. Most
expression nodes are pretty straightforward; the only interesting case
is concat (a sequence of subexpressions).

Even that one is not terribly complex. First, determine a prefix of
easy nodes of constant match length (this won't affect backtracking,
so safe to delegate to NFA). Then, if your context is easy, determine
a suffix of easy nodes. Both of these delegate to NFA. For the ones in
between, recursively compile. In an easy context, the last of these
also gets an easy context; everything else is generated in a hard
context. So, conceptually, hard context flows from right to left, and
from parents to children.

## Current status

Still in development, though the basic ideas are in place. Currently,
the following features are missing:

* Procedure calls and recursive expressions

## Acknowledgements

Many thanks to [Andrew Gallant](http://blog.burntsushi.net/about/) for
stimulating conversations that inspired this approach, as well as for
creating the excellent regex crate.

## Authors

The main author is Raph Levien, with many contributions from Robin Stocker.

## Contributions

We gladly accept contributions via GitHub pull requests. Please see
[CONTRIBUTING.md](CONTRIBUTING.md) for more details.

This project started out as a Google 20% project, but none of the authors currently
work at Google so it has been forked to be community-maintained.
