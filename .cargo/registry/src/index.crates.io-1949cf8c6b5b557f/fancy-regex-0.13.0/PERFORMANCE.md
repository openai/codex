The README has a quick introduction to the performance of this crate.
This will look at some examples and compare them to the Oniguruma engine.

## Catastrophic backtracking

Backtracking engines can have worst-case performance when the regular
expression forces the engine to consider an exponentially increasing
number of sub-cases.

For a good explanation of that, read
[Runaway Regular Expressions: Catastrophic Backtracking][].

Let's look at the regex from the README again:

    (a|b|ab)*bc

And the input text:

    ababababababababababababababababababababababababababababac

Python's engine has exponential runtime. The regex crate and fancy-regex
however have no problem with it.

## Oniguruma

[Oniguruma][] implements a backtracking
engine. So we'd expect it to have a problem with the above regex too.

However, in the above case, it quickly finds that there's no match. How
is that possible? The answer is that it has optimizations which
sometimes help it avoid having to do any matching at all:

In the pattern `(a|b|ab)*bc`, you might notice that if the input doesn't
contain `bc`, the pattern will never match. Oniguruma detects that and,
before it tries to do any matching, tries to find `bc` in the input.

But what happens if we add `bc` at the end of the input, like this:

    ababababababababababababababababababababababababababababacbc

Now the optimization doesn't help anymore, and Oniguruma is slow too.

## fancy-regex

For `(a|b|ab)*bc` fancy-regex is fast in all cases because it can
delegate to the regex crate which matches it in linear runtime.

Let's look at another regex, one that makes use of a "fancy" look-ahead:

    (a|b|ab)*(?=c)

When fancy-regex matches it against this input:

    abababababababababababababababababababababababababababab

It's slow! The reason is that `(?=c)` is not supported by the regex
crate, so we need to handle it with backtracking. And because
`(a|b|ab)*` is before it, we need to do it with backtracking as well.

Oniguruma doesn't have a problem with this particular case because its
optimization saves it again: It checks if there's a `c` in the input
before doing any matching.

There's nothing preventing fancy-regex from adding similar optimizations
in the future, but it's not done yet.

Note that how much fancy-regex can do without backtracking depends on
the structure of the regex. For example, with `(?=(a|b|ab)*bc)`, the
inner part of the look-ahead can be delegated to regex entirely.

### Summary

* If the regex doesn't use fancy features, fancy-regex should have
  linear runtime compared to Oniguruma's exponential worst-case.
* Even if the regex doesn't use any fancy features, Oniguruma can be
  faster because it is a mature and highly optimized engine.
* With fancy features, Oniguruma can be faster because of optimizations.

[Runaway Regular Expressions: Catastrophic Backtracking]: https://www.regular-expressions.info/catastrophic.html
[Oniguruma]: https://github.com/kkos/oniguruma
