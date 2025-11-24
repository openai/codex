/*!
This crate provides a **lightweight** regex engine for searching strings. The
regex syntax supported by this crate is nearly identical to what is found in
the [`regex`](https://docs.rs/regex) crate. Like the `regex` crate, all regex
searches in this crate have worst case `O(m * n)` time complexity, where `m` is
proportional to the size of the regex and `n` is proportional to the size of
the string being searched.

The principal difference between the `regex` and `regex-lite` crates is that
the latter prioritizes smaller binary sizes and shorter Rust compile times
over performance and functionality. As a result, regex searches in this crate
are typically substantially slower than what is provided by the `regex` crate.
Moreover, this crate only has the most basic level of Unicode support: it
matches codepoint by codepoint but otherwise doesn't support Unicode case
insensitivity or things like `\p{Letter}`. In exchange, this crate contributes
far less to binary size and compiles much more quickly.

If you just want API documentation, then skip to the [`Regex`] type. Otherwise,
here's a quick example showing one way of parsing the output of a grep-like
program:

```rust
use regex_lite::Regex;

let re = Regex::new(r"(?m)^([^:]+):([0-9]+):(.+)$").unwrap();
let hay = "\
path/to/foo:54:Blue Harvest
path/to/bar:90:Something, Something, Something, Dark Side
path/to/baz:3:It's a Trap!
";

let mut results = vec![];
for (_, [path, lineno, line]) in re.captures_iter(hay).map(|c| c.extract()) {
    results.push((path, lineno.parse::<u64>()?, line));
}
assert_eq!(results, vec![
    ("path/to/foo", 54, "Blue Harvest"),
    ("path/to/bar", 90, "Something, Something, Something, Dark Side"),
    ("path/to/baz", 3, "It's a Trap!"),
]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Overview

The primary type in this crate is a [`Regex`]. Its most important methods are
as follows:

* [`Regex::new`] compiles a regex using the default configuration. A
[`RegexBuilder`] permits setting a non-default configuration. (For example,
case insensitive matching, verbose mode and others.)
* [`Regex::is_match`] reports whether a match exists in a particular haystack.
* [`Regex::find`] reports the byte offsets of a match in a haystack, if one
exists. [`Regex::find_iter`] returns an iterator over all such matches.
* [`Regex::captures`] returns a [`Captures`], which reports both the byte
offsets of a match in a haystack and the byte offsets of each matching capture
group from the regex in the haystack.
[`Regex::captures_iter`] returns an iterator over all such matches.

Otherwise, this top-level crate documentation is organized as follows:

* [Usage](#usage) shows how to add the `regex` crate to your Rust project.
* [Examples](#examples) provides a limited selection of regex search examples.
* [Differences with the regex crate](#differences-with-the-regex-crate)
provides a precise description of how `regex-lite` differs from `regex`.
* [Syntax](#syntax) enumerates the specific regex syntax supported by this
crate.
* [Untrusted input](#untrusted-input) discusses how this crate deals with regex
patterns or haystacks that are untrusted.

# Usage

The `regex-lite` crate is [on crates.io](https://crates.io/crates/regex-lite)
and can be used by adding `regex-lite` to your dependencies in your project's
`Cargo.toml`. Or more simply, just run `cargo add regex-lite`.

Here is a complete example that creates a new Rust project, adds a dependency
on `regex-lite`, creates the source code for a regex search and then runs the
program.

First, create the project in a new directory:

```text
$ mkdir regex-example
$ cd regex-example
$ cargo init
```

Second, add a dependency on `regex`:

```text
$ cargo add regex-lite
```

Third, edit `src/main.rs`. Delete what's there and replace it with this:

```
use regex_lite::Regex;

fn main() {
    let re = Regex::new(r"Hello (?<name>\w+)!").unwrap();
    let Some(caps) = re.captures("Hello Murphy!") else {
        println!("no match!");
        return;
    };
    println!("The name is: {}", &caps["name"]);
}
```

Fourth, run it with `cargo run`:

```text
$ cargo run
   Compiling regex-lite v0.1.0
   Compiling regex-example v0.1.0 (/tmp/regex-example)
    Finished dev [unoptimized + debuginfo] target(s) in 4.22s
     Running `target/debug/regex-example`
The name is: Murphy
```

The first time you run the program will show more output like above. But
subsequent runs shouldn't have to re-compile the dependencies.

# Examples

This section provides a few examples, in tutorial style, showing how to
search a haystack with a regex. There are more examples throughout the API
documentation.

Before starting though, it's worth defining a few terms:

* A **regex** is a Rust value whose type is `Regex`. We use `re` as a
variable name for a regex.
* A **pattern** is the string that is used to build a regex. We use `pat` as
a variable name for a pattern.
* A **haystack** is the string that is searched by a regex. We use `hay` as a
variable name for a haystack.

Sometimes the words "regex" and "pattern" are used interchangeably.

General use of regular expressions in this crate proceeds by compiling a
**pattern** into a **regex**, and then using that regex to search, split or
replace parts of a **haystack**.

### Example: find a middle initial

We'll start off with a very simple example: a regex that looks for a specific
name but uses a wildcard to match a middle initial. Our pattern serves as
something like a template that will match a particular name with *any* middle
initial.

```rust
use regex_lite::Regex;

// We use 'unwrap()' here because it would be a bug in our program if the
// pattern failed to compile to a regex. Panicking in the presence of a bug
// is okay.
let re = Regex::new(r"Homer (.)\. Simpson").unwrap();
let hay = "Homer J. Simpson";
let Some(caps) = re.captures(hay) else { return };
assert_eq!("J", &caps[1]);
```

There are a few things worth noticing here in our first example:

* The `.` is a special pattern meta character that means "match any single
character except for new lines." (More precisely, in this crate, it means
"match any UTF-8 encoding of any Unicode scalar value other than `\n`.")
* We can match an actual `.` literally by escaping it, i.e., `\.`.
* We use Rust's [raw strings] to avoid needing to deal with escape sequences in
both the regex pattern syntax and in Rust's string literal syntax. If we didn't
use raw strings here, we would have had to use `\\.` to match a literal `.`
character. That is, `r"\."` and `"\\."` are equivalent patterns.
* We put our wildcard `.` instruction in parentheses. These parentheses have a
special meaning that says, "make whatever part of the haystack matches within
these parentheses available as a capturing group." After finding a match, we
access this capture group with `&caps[1]`.

[raw strings]: https://doc.rust-lang.org/stable/reference/tokens.html#raw-string-literals

Otherwise, we execute a search using `re.captures(hay)` and return from our
function if no match occurred. We then reference the middle initial by asking
for the part of the haystack that matched the capture group indexed at `1`.
(The capture group at index 0 is implicit and always corresponds to the entire
match. In this case, that's `Homer J. Simpson`.)

### Example: named capture groups

Continuing from our middle initial example above, we can tweak the pattern
slightly to give a name to the group that matches the middle initial:

```rust
use regex_lite::Regex;

// Note that (?P<middle>.) is a different way to spell the same thing.
let re = Regex::new(r"Homer (?<middle>.)\. Simpson").unwrap();
let hay = "Homer J. Simpson";
let Some(caps) = re.captures(hay) else { return };
assert_eq!("J", &caps["middle"]);
```

Giving a name to a group can be useful when there are multiple groups in
a pattern. It makes the code referring to those groups a bit easier to
understand.

### Example: validating a particular date format

This examples shows how to confirm whether a haystack, in its entirety, matches
a particular date format:

```rust
use regex_lite::Regex;

let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
assert!(re.is_match("2010-03-14"));
```

Notice the use of the `^` and `$` anchors. In this crate, every regex search is
run with an implicit `(?s:.)*?` at the beginning of its pattern, which allows
the regex to match anywhere in a haystack. Anchors, as above, can be used to
ensure that the full haystack matches a pattern.

### Example: finding dates in a haystack

In the previous example, we showed how one might validate that a haystack,
in its entirety, corresponded to a particular date format. But what if we wanted
to extract all things that look like dates in a specific format from a haystack?
To do this, we can use an iterator API to find all matches (notice that we've
removed the anchors):

```rust
use regex_lite::Regex;

let re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
let hay = "What do 1865-04-14, 1881-07-02, 1901-09-06 and 1963-11-22 have in common?";
// 'm' is a 'Match', and 'as_str()' returns the matching part of the haystack.
let dates: Vec<&str> = re.find_iter(hay).map(|m| m.as_str()).collect();
assert_eq!(dates, vec![
    "1865-04-14",
    "1881-07-02",
    "1901-09-06",
    "1963-11-22",
]);
```

We can also iterate over [`Captures`] values instead of [`Match`] values, and
that in turn permits accessing each component of the date via capturing groups:

```rust
use regex_lite::Regex;

let re = Regex::new(r"(?<y>\d{4})-(?<m>\d{2})-(?<d>\d{2})").unwrap();
let hay = "What do 1865-04-14, 1881-07-02, 1901-09-06 and 1963-11-22 have in common?";
// 'm' is a 'Match', and 'as_str()' returns the matching part of the haystack.
let dates: Vec<(&str, &str, &str)> = re.captures_iter(hay).map(|caps| {
    // The unwraps are okay because every capture group must match if the whole
    // regex matches, and in this context, we know we have a match.
    //
    // Note that we use `caps.name("y").unwrap().as_str()` instead of
    // `&caps["y"]` because the lifetime of the former is the same as the
    // lifetime of `hay` above, but the lifetime of the latter is tied to the
    // lifetime of `caps` due to how the `Index` trait is defined.
    let year = caps.name("y").unwrap().as_str();
    let month = caps.name("m").unwrap().as_str();
    let day = caps.name("d").unwrap().as_str();
    (year, month, day)
}).collect();
assert_eq!(dates, vec![
    ("1865", "04", "14"),
    ("1881", "07", "02"),
    ("1901", "09", "06"),
    ("1963", "11", "22"),
]);
```

### Example: simpler capture group extraction

One can use [`Captures::extract`] to make the code from the previous example a
bit simpler in this case:

```rust
use regex_lite::Regex;

let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
let hay = "What do 1865-04-14, 1881-07-02, 1901-09-06 and 1963-11-22 have in common?";
let dates: Vec<(&str, &str, &str)> = re.captures_iter(hay).map(|caps| {
    let (_, [year, month, day]) = caps.extract();
    (year, month, day)
}).collect();
assert_eq!(dates, vec![
    ("1865", "04", "14"),
    ("1881", "07", "02"),
    ("1901", "09", "06"),
    ("1963", "11", "22"),
]);
```

`Captures::extract` works by ensuring that the number of matching groups match
the number of groups requested via the `[year, month, day]` syntax. If they do,
then the substrings for each corresponding capture group are automatically
returned in an appropriately sized array. Rust's syntax for pattern matching
arrays does the rest.

### Example: replacement with named capture groups

Building on the previous example, perhaps we'd like to rearrange the date
formats. This can be done by finding each match and replacing it with
something different. The [`Regex::replace_all`] routine provides a convenient
way to do this, including by supporting references to named groups in the
replacement string:

```rust
use regex_lite::Regex;

let re = Regex::new(r"(?<y>\d{4})-(?<m>\d{2})-(?<d>\d{2})").unwrap();
let before = "1973-01-05, 1975-08-25 and 1980-10-18";
let after = re.replace_all(before, "$m/$d/$y");
assert_eq!(after, "01/05/1973, 08/25/1975 and 10/18/1980");
```

The replace methods are actually polymorphic in the replacement, which
provides more flexibility than is seen here. (See the documentation for
[`Regex::replace`] for more details.)

### Example: verbose mode

When your regex gets complicated, you might consider using something other
than regex. But if you stick with regex, you can use the `x` flag to enable
insignificant whitespace mode or "verbose mode." In this mode, whitespace
is treated as insignificant and one may write comments. This may make your
patterns easier to comprehend.

```rust
use regex_lite::Regex;

let re = Regex::new(r"(?x)
  (?P<y>\d{4}) # the year
  -
  (?P<m>\d{2}) # the month
  -
  (?P<d>\d{2}) # the day
").unwrap();

let before = "1973-01-05, 1975-08-25 and 1980-10-18";
let after = re.replace_all(before, "$m/$d/$y");
assert_eq!(after, "01/05/1973, 08/25/1975 and 10/18/1980");
```

If you wish to match against whitespace in this mode, you can still use `\s`,
`\n`, `\t`, etc. For escaping a single space character, you can escape it
directly with `\ `, use its hex character code `\x20` or temporarily disable
the `x` flag, e.g., `(?-x: )`.

# Differences with the `regex` crate

As mentioned in the introduction above, the purpose of this crate is to
prioritize small binary sizes and shorter Rust compilation times as much as
possible. Namely, while the `regex` crate tends to eschew both binary size
and compilation time in favor of faster searches and features, the
`regex-lite` crate gives up faster searches and some functionality in exchange
for smaller binary sizes and faster compilation times.

The precise set of differences at the syntax level:

* The Perl character classes are limited to ASCII codepoints. That is,
`\d` is `[0-9]`, `\s` is `[\t\n\v\f\r ]` and `\w` is `[0-9A-Za-z_]`.
* Unicode character classes of the form `\p{...}` and `\P{...}` are not
supported at all. Note though that things like `[^β]` are still supported and
will match any Unicode scalar value except for `β`.
* Case insensitive searching is limited to ASCII case insensitivity.
* Character class set operations other than union are not supported. That is,
difference (`--`), intersection (`&&`) and symmetric difference (`~~`) are
not available. These tend to be most useful with Unicode classes, which also
aren't available.
* Opt-in octal support is not available in this crate.

And now at the API level:

* Currently, this crate only supports searching `&str`. It does not have APIs
for searching `&[u8]` haystacks, although it is planned to add these in the
future if there's demand.
* There is no `RegexSet` in this crate and there are no plans to add it.
* The `Error` type in this crate is completely opaque.

Other than these things, the `regex-lite` crate is intended to be a drop-in
replacement for the `regex` crate. In most cases, you can just replace `use
regex::Regex;` with `use regex_lite::Regex;` and everything will work. (Unless
you're depending on Unicode support in your regexes.)

# Syntax

The syntax supported in this crate is documented below.

### Matching one character

<pre class="rust">
.             any character except new line (includes new line with s flag)
[0-9]         any ASCII digit
\d            digit ([0-9])
\D            not digit
</pre>

### Character classes

<pre class="rust">
[xyz]         A character class matching either x, y or z (union).
[^xyz]        A character class matching any character except x, y and z.
[a-z]         A character class matching any character in range a-z.
[[:alpha:]]   ASCII character class ([A-Za-z])
[[:^alpha:]]  Negated ASCII character class ([^A-Za-z])
[\[\]]        Escaping in character classes (matching [ or ])
</pre>

Any ASCII or Perl character class may appear inside a bracketed `[...]` character
class. For example, `[\s[:digit:]]` matches any digit or space character.

Precedence in character classes, from most binding to least:

1. Ranges: `[a-cd]` == `[[a-c]d]`
2. Union: `[ab&&bc]` == `[[ab]&&[bc]]`
3. Negation: `[^a-z&&b]` == `[^[a-z&&b]]`.

### Composites

<pre class="rust">
xy    concatenation (x followed by y)
x|y   alternation (x or y, prefer x)
</pre>

This example shows how an alternation works, and what it means to prefer a
branch in the alternation over subsequent branches.

```
use regex_lite::Regex;

let haystack = "samwise";
// If 'samwise' comes first in our alternation, then it is
// preferred as a match, even if the regex engine could
// technically detect that 'sam' led to a match earlier.
let re = Regex::new(r"samwise|sam").unwrap();
assert_eq!("samwise", re.find(haystack).unwrap().as_str());
// But if 'sam' comes first, then it will match instead.
// In this case, it is impossible for 'samwise' to match
// because 'sam' is a prefix of it.
let re = Regex::new(r"sam|samwise").unwrap();
assert_eq!("sam", re.find(haystack).unwrap().as_str());
```

### Repetitions

<pre class="rust">
x*        zero or more of x (greedy)
x+        one or more of x (greedy)
x?        zero or one of x (greedy)
x*?       zero or more of x (ungreedy/lazy)
x+?       one or more of x (ungreedy/lazy)
x??       zero or one of x (ungreedy/lazy)
x{n,m}    at least n x and at most m x (greedy)
x{n,}     at least n x (greedy)
x{n}      exactly n x
x{n,m}?   at least n x and at most m x (ungreedy/lazy)
x{n,}?    at least n x (ungreedy/lazy)
x{n}?     exactly n x
</pre>

### Empty matches

<pre class="rust">
^               the beginning of a haystack (or start-of-line with multi-line mode)
$               the end of a haystack (or end-of-line with multi-line mode)
\A              only the beginning of a haystack (even with multi-line mode enabled)
\z              only the end of a haystack (even with multi-line mode enabled)
\b              an ASCII word boundary (\w on one side and \W, \A, or \z on other)
\B              not an ASCII word boundary
\b{start}, \<   an ASCII start-of-word boundary (\W|\A on the left, \w on the right)
\b{end}, \>     an ASCII end-of-word boundary (\w on the left, \W|\z on the right))
\b{start-half}  half of an ASCII start-of-word boundary (\W|\A on the left)
\b{end-half}    half of an ASCII end-of-word boundary (\W|\z on the right)
</pre>

The empty regex is valid and matches the empty string. For example, the
empty regex matches `abc` at positions `0`, `1`, `2` and `3`. When using the
top-level [`Regex`] on `&str` haystacks, an empty match that splits a codepoint
is guaranteed to never be returned. For example:

```rust
let re = regex_lite::Regex::new(r"").unwrap();
let ranges: Vec<_> = re.find_iter("💩").map(|m| m.range()).collect();
assert_eq!(ranges, vec![0..0, 4..4]);
```

Note that an empty regex is distinct from a regex that can never match. For
example, the regex `[^\s\S]` is a character class that represents the negation
of `[\s\S]`, where the union of `\s` and `\S` corresponds to all Unicode scalar
values. The negation of everything is nothing, which means the character class
is empty. Since nothing is in the empty set, `[^\s\S]` matches nothing, not
even the empty string.

### Grouping and flags

<pre class="rust">
(exp)          numbered capture group (indexed by opening parenthesis)
(?P&lt;name&gt;exp)  named (also numbered) capture group (names must be alpha-numeric)
(?&lt;name&gt;exp)   named (also numbered) capture group (names must be alpha-numeric)
(?:exp)        non-capturing group
(?flags)       set flags within current group
(?flags:exp)   set flags for exp (non-capturing)
</pre>

Capture group names must be any sequence of alpha-numeric Unicode codepoints,
in addition to `.`, `_`, `[` and `]`. Names must start with either an `_` or
an alphabetic codepoint. Alphabetic codepoints correspond to the `Alphabetic`
Unicode property, while numeric codepoints correspond to the union of the
`Decimal_Number`, `Letter_Number` and `Other_Number` general categories.

Flags are each a single character. For example, `(?x)` sets the flag `x`
and `(?-x)` clears the flag `x`. Multiple flags can be set or cleared at
the same time: `(?xy)` sets both the `x` and `y` flags and `(?x-y)` sets
the `x` flag and clears the `y` flag.

All flags are by default disabled unless stated otherwise. They are:

<pre class="rust">
i     case-insensitive: letters match both upper and lower case
m     multi-line mode: ^ and $ match begin/end of line
s     allow . to match \n
R     enables CRLF mode: when multi-line mode is enabled, \r\n is used
U     swap the meaning of x* and x*?
x     verbose mode, ignores whitespace and allow line comments (starting with `#`)
</pre>

Note that in verbose mode, whitespace is ignored everywhere, including within
character classes. To insert whitespace, use its escaped form or a hex literal.
For example, `\ ` or `\x20` for an ASCII space.

Flags can be toggled within a pattern. Here's an example that matches
case-insensitively for the first part but case-sensitively for the second part:

```rust
use regex_lite::Regex;

let re = Regex::new(r"(?i)a+(?-i)b+").unwrap();
let m = re.find("AaAaAbbBBBb").unwrap();
assert_eq!(m.as_str(), "AaAaAbb");
```

Notice that the `a+` matches either `a` or `A`, but the `b+` only matches
`b`.

Multi-line mode means `^` and `$` no longer match just at the beginning/end of
the input, but also at the beginning/end of lines:

```
use regex_lite::Regex;

let re = Regex::new(r"(?m)^line \d+").unwrap();
let m = re.find("line one\nline 2\n").unwrap();
assert_eq!(m.as_str(), "line 2");
```

Note that `^` matches after new lines, even at the end of input:

```
use regex_lite::Regex;

let re = Regex::new(r"(?m)^").unwrap();
let m = re.find_iter("test\n").last().unwrap();
assert_eq!((m.start(), m.end()), (5, 5));
```

When both CRLF mode and multi-line mode are enabled, then `^` and `$` will
match either `\r` or `\n`, but never in the middle of a `\r\n`:

```
use regex_lite::Regex;

let re = Regex::new(r"(?mR)^foo$").unwrap();
let m = re.find("\r\nfoo\r\n").unwrap();
assert_eq!(m.as_str(), "foo");
```

### Escape sequences

Note that this includes all possible escape sequences, even ones that are
documented elsewhere.

<pre class="rust">
\*              literal *, applies to all ASCII except [0-9A-Za-z<>]
\a              bell (\x07)
\f              form feed (\x0C)
\t              horizontal tab
\n              new line
\r              carriage return
\v              vertical tab (\x0B)
\A              matches at the beginning of a haystack
\z              matches at the end of a haystack
\b              word boundary assertion
\B              negated word boundary assertion
\b{start}, \<   start-of-word boundary assertion
\b{end}, \>     end-of-word boundary assertion
\b{start-half}  half of a start-of-word boundary assertion
\b{end-half}    half of a end-of-word boundary assertion
\x7F            hex character code (exactly two digits)
\x{10FFFF}      any hex character code corresponding to a Unicode code point
\u007F          hex character code (exactly four digits)
\u{7F}          any hex character code corresponding to a Unicode code point
\U0000007F      hex character code (exactly eight digits)
\U{7F}          any hex character code corresponding to a Unicode code point
\d, \s, \w      Perl character class
\D, \S, \W      negated Perl character class
</pre>

### Perl character classes (ASCII only)

These character classes are short-hands for common groups of characters. In
this crate, `\d`, `\s` and `\w` only consist of ASCII codepoints.

<pre class="rust">
\d     digit ([0-9])
\D     not digit
\s     whitespace ([\t\n\v\f\r ])
\S     not whitespace
\w     word character ([0-9A-Za-z_])
\W     not word character
</pre>

### ASCII character classes

These reflect additional groups of characters taken from POSIX regex syntax
that are sometimes useful to have. In this crate, all of these classes only
consist of ASCII codepoints.

<pre class="rust">
[[:alnum:]]    alphanumeric ([0-9A-Za-z])
[[:alpha:]]    alphabetic ([A-Za-z])
[[:ascii:]]    ASCII ([\x00-\x7F])
[[:blank:]]    blank ([\t ])
[[:cntrl:]]    control ([\x00-\x1F\x7F])
[[:digit:]]    digits ([0-9])
[[:graph:]]    graphical ([!-~])
[[:lower:]]    lower case ([a-z])
[[:print:]]    printable ([ -~])
[[:punct:]]    punctuation ([!-/:-@\[-`{-~])
[[:space:]]    whitespace ([\t\n\v\f\r ])
[[:upper:]]    upper case ([A-Z])
[[:word:]]     word characters ([0-9A-Za-z_])
[[:xdigit:]]   hex digit ([0-9A-Fa-f])
</pre>

# Untrusted input

This crate is meant to be able to run regex searches on untrusted haystacks
without fear of [ReDoS]. This crate also, to a certain extent, supports
untrusted patterns.

[ReDoS]: https://en.wikipedia.org/wiki/ReDoS

This crate differs from most (but not all) other regex engines in that it
doesn't use unbounded backtracking to run a regex search. In those cases,
one generally cannot use untrusted patterns *or* untrusted haystacks because
it can be very difficult to know whether a particular pattern will result in
catastrophic backtracking or not.

We'll first discuss how this crate deals with untrusted inputs and then wrap
it up with a realistic discussion about what practice really looks like.

### Panics

Outside of clearly documented cases, most APIs in this crate are intended to
never panic regardless of the inputs given to them. For example, `Regex::new`,
`Regex::is_match`, `Regex::find` and `Regex::captures` should never panic. That
is, it is an API promise that those APIs will never panic no matter what inputs
are given to them. With that said, regex engines are complicated beasts, and
providing a rock solid guarantee that these APIs literally never panic is
essentially equivalent to saying, "there are no bugs in this library." That is
a bold claim, and not really one that can be feasibly made with a straight
face.

Don't get the wrong impression here. This crate is extensively tested, not just
with unit and integration tests, but also via fuzz testing. For example, this
crate is part of the [OSS-fuzz project]. Panics should be incredibly rare, but
it is possible for bugs to exist, and thus possible for a panic to occur. If
you need a rock solid guarantee against panics, then you should wrap calls into
this library with [`std::panic::catch_unwind`].

It's also worth pointing out that this library will generally panic when other
regex engines would commit undefined behavior. When undefined behavior occurs,
your program might continue as if nothing bad has happened, but it also might
mean your program is open to the worst kinds of exploits. In contrast, the
worst thing a panic can do is a denial of service.

[OSS-fuzz project]: https://android.googlesource.com/platform/external/oss-fuzz/+/refs/tags/android-t-preview-1/projects/rust-regex/
[`std::panic::catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html

### Untrusted patterns

The principal way this crate deals with them is by limiting their size by
default. The size limit can be configured via [`RegexBuilder::size_limit`]. The
idea of a size limit is that compiling a pattern into a `Regex` will fail if it
becomes "too big." Namely, while *most* resources consumed by compiling a regex
are approximately proportional to the length of the pattern itself, there is
one particular exception to this: counted repetitions. Namely, this pattern:

```text
a{5}{5}{5}{5}{5}{5}
```

Is equivalent to this pattern:

```text
a{15625}
```

In both of these cases, the actual pattern string is quite small, but the
resulting `Regex` value is quite large. Indeed, as the first pattern shows,
it isn't enough to locally limit the size of each repetition because they can
be stacked in a way that results in exponential growth.

To provide a bit more context, a simplified view of regex compilation looks
like this:

* The pattern string is parsed into a structured representation called an HIR
(high-level intermediate representation). Counted repetitions are not expanded
in this stage. That is, the size of the HIR is proportional to the size
of the pattern with "reasonable" constant factors. In other words, one can
reasonably limit the memory used by an HIR by limiting the length of the
pattern string.
* The HIR is compiled into a [Thompson NFA]. This is the stage at which
something like `\w{5}` is rewritten to `\w\w\w\w\w`. Thus, this is the stage
at which [`RegexBuilder::size_limit`] is enforced. If the NFA exceeds the
configured size, then this stage will fail.

[Thompson NFA]: https://en.wikipedia.org/wiki/Thompson%27s_construction

The size limit helps avoid two different kinds of exorbitant resource usage:

* It avoids permitting exponential memory usage based on the size of the
pattern string.
* It avoids long search times. This will be discussed in more detail in the
next section, but worst case search time *is* dependent on the size of the
regex. So keeping regexes limited to a reasonable size is also a way of keeping
search times reasonable.

Finally, it's worth pointing out that regex compilation is guaranteed to take
worst case `O(m)` time, where `m` is proportional to the size of regex. The
size of the regex here is *after* the counted repetitions have been expanded.

**Advice for those using untrusted regexes**: limit the pattern length to
something small and expand it as needed. Configure [`RegexBuilder::size_limit`]
to something small and then expand it as needed.

### Untrusted haystacks

The main way this crate guards against searches from taking a long time is by
using algorithms that guarantee a `O(m * n)` worst case time and space bound.
Namely:

* `m` is proportional to the size of the regex, where the size of the regex
includes the expansion of all counted repetitions. (See the previous section on
untrusted patterns.)
* `n` is proportional to the length, in bytes, of the haystack.

In other words, if you consider `m` to be a constant (for example, the regex
pattern is a literal in the source code), then the search can be said to run
in "linear time." Or equivalently, "linear time with respect to the size of the
haystack."

But the `m` factor here is important not to ignore. If a regex is
particularly big, the search times can get quite slow. This is why, in part,
[`RegexBuilder::size_limit`] exists.

**Advice for those searching untrusted haystacks**: As long as your regexes
are not enormous, you should expect to be able to search untrusted haystacks
without fear. If you aren't sure, you should benchmark it. Unlike backtracking
engines, if your regex is so big that it's likely to result in slow searches,
this is probably something you'll be able to observe regardless of what the
haystack is made up of.

### Iterating over matches

One thing that is perhaps easy to miss is that the worst case time
complexity bound of `O(m * n)` applies to methods like [`Regex::is_match`],
[`Regex::find`] and [`Regex::captures`]. It does **not** apply to
[`Regex::find_iter`] or [`Regex::captures_iter`]. Namely, since iterating over
all matches can execute many searches, and each search can scan the entire
haystack, the worst case time complexity for iterators is `O(m * n^2)`.

One example of where this occurs is when a pattern consists of an alternation,
where an earlier branch of the alternation requires scanning the entire
haystack only to discover that there is no match. It also requires a later
branch of the alternation to have matched at the beginning of the search. For
example, consider the pattern `.*[^A-Z]|[A-Z]` and the haystack `AAAAA`. The
first search will scan to the end looking for matches of `.*[^A-Z]` even though
a finite automata engine (as in this crate) knows that `[A-Z]` has already
matched the first character of the haystack. This is due to the greedy nature
of regex searching. That first search will report a match at the first `A` only
after scanning to the end to discover that no other match exists. The next
search then begins at the second `A` and the behavior repeats.

There is no way to avoid this. This means that if both patterns and haystacks
are untrusted and you're iterating over all matches, you're susceptible
to worst case quadratic time complexity. One possible way to mitigate
this is to switch to the lower level `regex-automata` crate and use its
`meta::Regex` iterator APIs. There, you can configure the search to operate
in "earliest" mode by passing a `Input::new(haystack).earliest(true)` to
`meta::Regex::find_iter` (for example). By enabling this mode, you give up
the normal greedy match semantics of regex searches and instead ask the regex
engine to immediately stop as soon as a match has been found. Enabling this
mode will thus restore the worst case `O(m * n)` time complexity bound, but at
the cost of different semantics.

### Untrusted inputs in practice

While providing a `O(m * n)` worst case time bound on all searches goes a long
way toward preventing [ReDoS], that doesn't mean every search you can possibly
run will complete without burning CPU time. In general, there are a few ways
for the `m * n` time bound to still bite you:

* You are searching an exceptionally long haystack. No matter how you slice
it, a longer haystack will take more time to search.
* Very large regexes can searches to be quite slow due to increasing the size
`m` in the worst case `O(m * n)` bound. This is especially true when they
are combined with counted repetitions. While the regex size limit above will
protect you from the most egregious cases, the default size limit still
permits pretty big regexes that can execute more slowly than one might expect.
* While routines like [`Regex::find`] and [`Regex::captures`] guarantee
worst case `O(m * n)` search time, routines like [`Regex::find_iter`] and
[`Regex::captures_iter`] actually have worst case `O(m * n^2)` search time.
This is because `find_iter` runs many searches, and each search takes worst
case `O(m * n)` time. Thus, iteration of all matches in a haystack has
worst case `O(m * n^2)`. A good example of a pattern that exhibits this is
`(?:A+){1000}|` or even `.*[^A-Z]|[A-Z]`.

In general, untrusted haystacks are easier to stomach than untrusted patterns.
Untrusted patterns give a lot more control to the caller to impact the
performance of a search. Therefore, permitting untrusted patterns means that
your only line of defense is to put a limit on how big `m` (and perhaps also
`n`) can be in `O(m * n)`. `n` is limited by simply inspecting the length
of the haystack while `m` is limited by *both* applying a limit to the
length of the pattern *and* a limit on the compiled size of the regex via
[`RegexBuilder::size_limit`].

It bears repeating: if you're accepting untrusted patterns, it would be a good
idea to start with conservative limits on `m` and `n`, and then carefully
increase them as needed.
*/

#![no_std]
// I'm not ideologically opposed to allowing non-safe code in this crate, but
// IMO it needs really excellent justification. One likely place where this
// could show up is if and when we support a non-std alloc mode. In that case,
// we need some way to synchronize access to a PikeVM cache. That in turn will
// likely require rolling our own primitive spin-lock or similar structure.
#![forbid(unsafe_code)]
#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![warn(missing_debug_implementations)]
// When the main features are disabled, squash dead code warnings. The
// alternative is to litter conditional compilation directives everywhere,
// which is super annoying.
#![cfg_attr(not(feature = "string"), allow(dead_code))]

#[cfg(not(feature = "std"))]
compile_error!("'std' is currently a required feature, please file an issue");

#[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
compile_error!("not supported on non-{32,64}, please file an issue");

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

#[cfg(feature = "string")]
pub use self::string::*;
pub use self::{error::Error, hir::escape};

mod error;
mod hir;
mod int;
mod interpolate;
mod nfa;
mod pikevm;
mod pool;
#[cfg(feature = "string")]
mod string;
mod utf8;
