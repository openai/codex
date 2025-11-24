/*!
This crate provides routines for searching strings for matches of a [regular
expression] (aka "regex"). The regex syntax supported by this crate is similar
to other regex engines, but it lacks several features that are not known how to
implement efficiently. This includes, but is not limited to, look-around and
backreferences. In exchange, all regex searches in this crate have worst case
`O(m * n)` time complexity, where `m` is proportional to the size of the regex
and `n` is proportional to the size of the string being searched.

[regular expression]: https://en.wikipedia.org/wiki/Regular_expression

If you just want API documentation, then skip to the [`Regex`] type. Otherwise,
here's a quick example showing one way of parsing the output of a grep-like
program:

```rust
use regex::Regex;

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

There is also a [`RegexSet`], which permits searching for multiple regex
patterns simultaneously in a single search. However, it currently only reports
which patterns match and *not* the byte offsets of a match.

Otherwise, this top-level crate documentation is organized as follows:

* [Usage](#usage) shows how to add the `regex` crate to your Rust project.
* [Examples](#examples) provides a limited selection of regex search examples.
* [Performance](#performance) provides a brief summary of how to optimize regex
searching speed.
* [Unicode](#unicode) discusses support for non-ASCII patterns.
* [Syntax](#syntax) enumerates the specific regex syntax supported by this
crate.
* [Untrusted input](#untrusted-input) discusses how this crate deals with regex
patterns or haystacks that are untrusted.
* [Crate features](#crate-features) documents the Cargo features that can be
enabled or disabled for this crate.
* [Other crates](#other-crates) links to other crates in the `regex` family.

# Usage

The `regex` crate is [on crates.io](https://crates.io/crates/regex) and can be
used by adding `regex` to your dependencies in your project's `Cargo.toml`.
Or more simply, just run `cargo add regex`.

Here is a complete example that creates a new Rust project, adds a dependency
on `regex`, creates the source code for a regex search and then runs the
program.

First, create the project in a new directory:

```text
$ mkdir regex-example
$ cd regex-example
$ cargo init
```

Second, add a dependency on `regex`:

```text
$ cargo add regex
```

Third, edit `src/main.rs`. Delete what's there and replace it with this:

```
use regex::Regex;

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
   Compiling memchr v2.5.0
   Compiling regex-syntax v0.7.1
   Compiling aho-corasick v1.0.1
   Compiling regex v1.8.1
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
use regex::Regex;

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
use regex::Regex;

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
use regex::Regex;

let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
assert!(re.is_match("2010-03-14"));
```

Notice the use of the `^` and `$` anchors. In this crate, every regex search is
run with an implicit `(?s:.)*?` at the beginning of its pattern, which allows
the regex to match anywhere in a haystack. Anchors, as above, can be used to
ensure that the full haystack matches a pattern.

This crate is also Unicode aware by default, which means that `\d` might match
more than you might expect it to. For example:

```rust
use regex::Regex;

let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
assert!(re.is_match("𝟚𝟘𝟙𝟘-𝟘𝟛-𝟙𝟜"));
```

To only match an ASCII decimal digit, all of the following are equivalent:

* `[0-9]`
* `(?-u:\d)`
* `[[:digit:]]`
* `[\d&&\p{ascii}]`

### Example: finding dates in a haystack

In the previous example, we showed how one might validate that a haystack,
in its entirety, corresponded to a particular date format. But what if we wanted
to extract all things that look like dates in a specific format from a haystack?
To do this, we can use an iterator API to find all matches (notice that we've
removed the anchors and switched to looking for ASCII-only digits):

```rust
use regex::Regex;

let re = Regex::new(r"[0-9]{4}-[0-9]{2}-[0-9]{2}").unwrap();
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
use regex::Regex;

let re = Regex::new(r"(?<y>[0-9]{4})-(?<m>[0-9]{2})-(?<d>[0-9]{2})").unwrap();
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
use regex::Regex;

let re = Regex::new(r"([0-9]{4})-([0-9]{2})-([0-9]{2})").unwrap();
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
use regex::Regex;

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
use regex::Regex;

let re = Regex::new(r"(?x)
  (?P<y>\d{4}) # the year, including all Unicode digits
  -
  (?P<m>\d{2}) # the month, including all Unicode digits
  -
  (?P<d>\d{2}) # the day, including all Unicode digits
").unwrap();

let before = "1973-01-05, 1975-08-25 and 1980-10-18";
let after = re.replace_all(before, "$m/$d/$y");
assert_eq!(after, "01/05/1973, 08/25/1975 and 10/18/1980");
```

If you wish to match against whitespace in this mode, you can still use `\s`,
`\n`, `\t`, etc. For escaping a single space character, you can escape it
directly with `\ `, use its hex character code `\x20` or temporarily disable
the `x` flag, e.g., `(?-x: )`.

### Example: match multiple regular expressions simultaneously

This demonstrates how to use a [`RegexSet`] to match multiple (possibly
overlapping) regexes in a single scan of a haystack:

```rust
use regex::RegexSet;

let set = RegexSet::new(&[
    r"\w+",
    r"\d+",
    r"\pL+",
    r"foo",
    r"bar",
    r"barfoo",
    r"foobar",
]).unwrap();

// Iterate over and collect all of the matches. Each match corresponds to the
// ID of the matching pattern.
let matches: Vec<_> = set.matches("foobar").into_iter().collect();
assert_eq!(matches, vec![0, 2, 3, 4, 6]);

// You can also test whether a particular regex matched:
let matches = set.matches("foobar");
assert!(!matches.matched(5));
assert!(matches.matched(6));
```

# Performance

This section briefly discusses a few concerns regarding the speed and resource
usage of regexes.

### Only ask for what you need

When running a search with a regex, there are generally three different types
of information one can ask for:

1. Does a regex match in a haystack?
2. Where does a regex match in a haystack?
3. Where do each of the capturing groups match in a haystack?

Generally speaking, this crate could provide a function to answer only #3,
which would subsume #1 and #2 automatically. However, it can be significantly
more expensive to compute the location of capturing group matches, so it's best
not to do it if you don't need to.

Therefore, only ask for what you need. For example, don't use [`Regex::find`]
if you only need to test if a regex matches a haystack. Use [`Regex::is_match`]
instead.

### Unicode can impact memory usage and search speed

This crate has first class support for Unicode and it is **enabled by default**.
In many cases, the extra memory required to support it will be negligible and
it typically won't impact search speed. But it can in some cases.

With respect to memory usage, the impact of Unicode principally manifests
through the use of Unicode character classes. Unicode character classes
tend to be quite large. For example, `\w` by default matches around 140,000
distinct codepoints. This requires additional memory, and tends to slow down
regex compilation. While a `\w` here and there is unlikely to be noticed,
writing `\w{100}` will for example result in quite a large regex by default.
Indeed, `\w` is considerably larger than its ASCII-only version, so if your
requirements are satisfied by ASCII, it's probably a good idea to stick to
ASCII classes. The ASCII-only version of `\w` can be spelled in a number of
ways. All of the following are equivalent:

* `[0-9A-Za-z_]`
* `(?-u:\w)`
* `[[:word:]]`
* `[\w&&\p{ascii}]`

With respect to search speed, Unicode tends to be handled pretty well, even when
using large Unicode character classes. However, some of the faster internal
regex engines cannot handle a Unicode aware word boundary assertion. So if you
don't need Unicode-aware word boundary assertions, you might consider using
`(?-u:\b)` instead of `\b`, where the former uses an ASCII-only definition of
a word character.

### Literals might accelerate searches

This crate tends to be quite good at recognizing literals in a regex pattern
and using them to accelerate a search. If it is at all possible to include
some kind of literal in your pattern, then it might make search substantially
faster. For example, in the regex `\w+@\w+`, the engine will look for
occurrences of `@` and then try a reverse match for `\w+` to find the start
position.

### Avoid re-compiling regexes, especially in a loop

It is an anti-pattern to compile the same pattern in a loop since regex
compilation is typically expensive. (It takes anywhere from a few microseconds
to a few **milliseconds** depending on the size of the pattern.) Not only is
compilation itself expensive, but this also prevents optimizations that reuse
allocations internally to the regex engine.

In Rust, it can sometimes be a pain to pass regexes around if they're used from
inside a helper function. Instead, we recommend using crates like [`once_cell`]
and [`lazy_static`] to ensure that patterns are compiled exactly once.

[`once_cell`]: https://crates.io/crates/once_cell
[`lazy_static`]: https://crates.io/crates/lazy_static

This example shows how to use `once_cell`:

```rust
use {
    once_cell::sync::Lazy,
    regex::Regex,
};

fn some_helper_function(haystack: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"...").unwrap());
    RE.is_match(haystack)
}

fn main() {
    assert!(some_helper_function("abc"));
    assert!(!some_helper_function("ac"));
}
```

Specifically, in this example, the regex will be compiled when it is used for
the first time. On subsequent uses, it will reuse the previously built `Regex`.
Notice how one can define the `Regex` locally to a specific function.

### Sharing a regex across threads can result in contention

While a single `Regex` can be freely used from multiple threads simultaneously,
there is a small synchronization cost that must be paid. Generally speaking,
one shouldn't expect to observe this unless the principal task in each thread
is searching with the regex *and* most searches are on short haystacks. In this
case, internal contention on shared resources can spike and increase latency,
which in turn may slow down each individual search.

One can work around this by cloning each `Regex` before sending it to another
thread. The cloned regexes will still share the same internal read-only portion
of its compiled state (it's reference counted), but each thread will get
optimized access to the mutable space that is used to run a search. In general,
there is no additional cost in memory to doing this. The only cost is the added
code complexity required to explicitly clone the regex. (If you share the same
`Regex` across multiple threads, each thread still gets its own mutable space,
but accessing that space is slower.)

# Unicode

This section discusses what kind of Unicode support this regex library has.
Before showing some examples, we'll summarize the relevant points:

* This crate almost fully implements "Basic Unicode Support" (Level 1) as
specified by the [Unicode Technical Standard #18][UTS18]. The full details
of what is supported are documented in [UNICODE.md] in the root of the regex
crate repository. There is virtually no support for "Extended Unicode Support"
(Level 2) from UTS#18.
* The top-level [`Regex`] runs searches *as if* iterating over each of the
codepoints in the haystack. That is, the fundamental atom of matching is a
single codepoint.
* [`bytes::Regex`], in contrast, permits disabling Unicode mode for part of all
of your pattern in all cases. When Unicode mode is disabled, then a search is
run *as if* iterating over each byte in the haystack. That is, the fundamental
atom of matching is a single byte. (A top-level `Regex` also permits disabling
Unicode and thus matching *as if* it were one byte at a time, but only when
doing so wouldn't permit matching invalid UTF-8.)
* When Unicode mode is enabled (the default), `.` will match an entire Unicode
scalar value, even when it is encoded using multiple bytes. When Unicode mode
is disabled (e.g., `(?-u:.)`), then `.` will match a single byte in all cases.
* The character classes `\w`, `\d` and `\s` are all Unicode-aware by default.
Use `(?-u:\w)`, `(?-u:\d)` and `(?-u:\s)` to get their ASCII-only definitions.
* Similarly, `\b` and `\B` use a Unicode definition of a "word" character.
To get ASCII-only word boundaries, use `(?-u:\b)` and `(?-u:\B)`. This also
applies to the special word boundary assertions. (That is, `\b{start}`,
`\b{end}`, `\b{start-half}`, `\b{end-half}`.)
* `^` and `$` are **not** Unicode-aware in multi-line mode. Namely, they only
recognize `\n` (assuming CRLF mode is not enabled) and not any of the other
forms of line terminators defined by Unicode.
* Case insensitive searching is Unicode-aware and uses simple case folding.
* Unicode general categories, scripts and many boolean properties are available
by default via the `\p{property name}` syntax.
* In all cases, matches are reported using byte offsets. Or more precisely,
UTF-8 code unit offsets. This permits constant time indexing and slicing of the
haystack.

[UTS18]: https://unicode.org/reports/tr18/
[UNICODE.md]: https://github.com/rust-lang/regex/blob/master/UNICODE.md

Patterns themselves are **only** interpreted as a sequence of Unicode scalar
values. This means you can use Unicode characters directly in your pattern:

```rust
use regex::Regex;

let re = Regex::new(r"(?i)Δ+").unwrap();
let m = re.find("ΔδΔ").unwrap();
assert_eq!((0, 6), (m.start(), m.end()));
// alternatively:
assert_eq!(0..6, m.range());
```

As noted above, Unicode general categories, scripts, script extensions, ages
and a smattering of boolean properties are available as character classes. For
example, you can match a sequence of numerals, Greek or Cherokee letters:

```rust
use regex::Regex;

let re = Regex::new(r"[\pN\p{Greek}\p{Cherokee}]+").unwrap();
let m = re.find("abcΔᎠβⅠᏴγδⅡxyz").unwrap();
assert_eq!(3..23, m.range());
```

While not specific to Unicode, this library also supports character class set
operations. Namely, one can nest character classes arbitrarily and perform set
operations on them. Those set operations are union (the default), intersection,
difference and symmetric difference. These set operations tend to be most
useful with Unicode character classes. For example, to match any codepoint
that is both in the `Greek` script and in the `Letter` general category:

```rust
use regex::Regex;

let re = Regex::new(r"[\p{Greek}&&\pL]+").unwrap();
let subs: Vec<&str> = re.find_iter("ΔδΔ𐅌ΔδΔ").map(|m| m.as_str()).collect();
assert_eq!(subs, vec!["ΔδΔ", "ΔδΔ"]);

// If we just matches on Greek, then all codepoints would match!
let re = Regex::new(r"\p{Greek}+").unwrap();
let subs: Vec<&str> = re.find_iter("ΔδΔ𐅌ΔδΔ").map(|m| m.as_str()).collect();
assert_eq!(subs, vec!["ΔδΔ𐅌ΔδΔ"]);
```

### Opt out of Unicode support

The [`bytes::Regex`] type that can be used to search `&[u8]` haystacks. By
default, haystacks are conventionally treated as UTF-8 just like it is with the
main `Regex` type. However, this behavior can be disabled by turning off the
`u` flag, even if doing so could result in matching invalid UTF-8. For example,
when the `u` flag is disabled, `.` will match any byte instead of any Unicode
scalar value.

Disabling the `u` flag is also possible with the standard `&str`-based `Regex`
type, but it is only allowed where the UTF-8 invariant is maintained. For
example, `(?-u:\w)` is an ASCII-only `\w` character class and is legal in an
`&str`-based `Regex`, but `(?-u:\W)` will attempt to match *any byte* that
isn't in `(?-u:\w)`, which in turn includes bytes that are invalid UTF-8.
Similarly, `(?-u:\xFF)` will attempt to match the raw byte `\xFF` (instead of
`U+00FF`), which is invalid UTF-8 and therefore is illegal in `&str`-based
regexes.

Finally, since Unicode support requires bundling large Unicode data
tables, this crate exposes knobs to disable the compilation of those
data tables, which can be useful for shrinking binary size and reducing
compilation times. For details on how to do that, see the section on [crate
features](#crate-features).

# Syntax

The syntax supported in this crate is documented below.

Note that the regular expression parser and abstract syntax are exposed in
a separate crate, [`regex-syntax`](https://docs.rs/regex-syntax).

### Matching one character

<pre class="rust">
.             any character except new line (includes new line with s flag)
[0-9]         any ASCII digit
\d            digit (\p{Nd})
\D            not digit
\pX           Unicode character class identified by a one-letter name
\p{Greek}     Unicode character class (general category or script)
\PX           Negated Unicode character class identified by a one-letter name
\P{Greek}     negated Unicode character class (general category or script)
</pre>

### Character classes

<pre class="rust">
[xyz]         A character class matching either x, y or z (union).
[^xyz]        A character class matching any character except x, y and z.
[a-z]         A character class matching any character in range a-z.
[[:alpha:]]   ASCII character class ([A-Za-z])
[[:^alpha:]]  Negated ASCII character class ([^A-Za-z])
[x[^xyz]]     Nested/grouping character class (matching any character except y and z)
[a-y&&xyz]    Intersection (matching x or y)
[0-9&&[^4]]   Subtraction using intersection and negation (matching 0-9 except 4)
[0-9--4]      Direct subtraction (matching 0-9 except 4)
[a-g~~b-h]    Symmetric difference (matching `a` and `h` only)
[\[\]]        Escaping in character classes (matching [ or ])
[a&&b]        An empty character class matching nothing
</pre>

Any named character class may appear inside a bracketed `[...]` character
class. For example, `[\p{Greek}[:digit:]]` matches any ASCII digit or any
codepoint in the `Greek` script. `[\p{Greek}&&\pL]` matches Greek letters.

Precedence in character classes, from most binding to least:

1. Ranges: `[a-cd]` == `[[a-c]d]`
2. Union: `[ab&&bc]` == `[[ab]&&[bc]]`
3. Intersection, difference, symmetric difference. All three have equivalent
precedence, and are evaluated in left-to-right order. For example,
`[\pL--\p{Greek}&&\p{Uppercase}]` == `[[\pL--\p{Greek}]&&\p{Uppercase}]`.
4. Negation: `[^a-z&&b]` == `[^[a-z&&b]]`.

### Composites

<pre class="rust">
xy    concatenation (x followed by y)
x|y   alternation (x or y, prefer x)
</pre>

This example shows how an alternation works, and what it means to prefer a
branch in the alternation over subsequent branches.

```
use regex::Regex;

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
\b              a Unicode word boundary (\w on one side and \W, \A, or \z on other)
\B              not a Unicode word boundary
\b{start}, \<   a Unicode start-of-word boundary (\W|\A on the left, \w on the right)
\b{end}, \>     a Unicode end-of-word boundary (\w on the left, \W|\z on the right))
\b{start-half}  half of a Unicode start-of-word boundary (\W|\A on the left)
\b{end-half}    half of a Unicode end-of-word boundary (\W|\z on the right)
</pre>

The empty regex is valid and matches the empty string. For example, the
empty regex matches `abc` at positions `0`, `1`, `2` and `3`. When using the
top-level [`Regex`] on `&str` haystacks, an empty match that splits a codepoint
is guaranteed to never be returned. However, such matches are permitted when
using a [`bytes::Regex`]. For example:

```rust
let re = regex::Regex::new(r"").unwrap();
let ranges: Vec<_> = re.find_iter("💩").map(|m| m.range()).collect();
assert_eq!(ranges, vec![0..0, 4..4]);

let re = regex::bytes::Regex::new(r"").unwrap();
let ranges: Vec<_> = re.find_iter("💩".as_bytes()).map(|m| m.range()).collect();
assert_eq!(ranges, vec![0..0, 1..1, 2..2, 3..3, 4..4]);
```

Note that an empty regex is distinct from a regex that can never match.
For example, the regex `[a&&b]` is a character class that represents the
intersection of `a` and `b`. That intersection is empty, which means the
character class is empty. Since nothing is in the empty set, `[a&&b]` matches
nothing, not even the empty string.

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
u     Unicode support (enabled by default)
x     verbose mode, ignores whitespace and allow line comments (starting with `#`)
</pre>

Note that in verbose mode, whitespace is ignored everywhere, including within
character classes. To insert whitespace, use its escaped form or a hex literal.
For example, `\ ` or `\x20` for an ASCII space.

Flags can be toggled within a pattern. Here's an example that matches
case-insensitively for the first part but case-sensitively for the second part:

```rust
use regex::Regex;

let re = Regex::new(r"(?i)a+(?-i)b+").unwrap();
let m = re.find("AaAaAbbBBBb").unwrap();
assert_eq!(m.as_str(), "AaAaAbb");
```

Notice that the `a+` matches either `a` or `A`, but the `b+` only matches
`b`.

Multi-line mode means `^` and `$` no longer match just at the beginning/end of
the input, but also at the beginning/end of lines:

```
use regex::Regex;

let re = Regex::new(r"(?m)^line \d+").unwrap();
let m = re.find("line one\nline 2\n").unwrap();
assert_eq!(m.as_str(), "line 2");
```

Note that `^` matches after new lines, even at the end of input:

```
use regex::Regex;

let re = Regex::new(r"(?m)^").unwrap();
let m = re.find_iter("test\n").last().unwrap();
assert_eq!((m.start(), m.end()), (5, 5));
```

When both CRLF mode and multi-line mode are enabled, then `^` and `$` will
match either `\r` and `\n`, but never in the middle of a `\r\n`:

```
use regex::Regex;

let re = Regex::new(r"(?mR)^foo$").unwrap();
let m = re.find("\r\nfoo\r\n").unwrap();
assert_eq!(m.as_str(), "foo");
```

Unicode mode can also be selectively disabled, although only when the result
*would not* match invalid UTF-8. One good example of this is using an ASCII
word boundary instead of a Unicode word boundary, which might make some regex
searches run faster:

```rust
use regex::Regex;

let re = Regex::new(r"(?-u:\b).+(?-u:\b)").unwrap();
let m = re.find("$$abc$$").unwrap();
assert_eq!(m.as_str(), "abc");
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
\123            octal character code, up to three digits (when enabled)
\x7F            hex character code (exactly two digits)
\x{10FFFF}      any hex character code corresponding to a Unicode code point
\u007F          hex character code (exactly four digits)
\u{7F}          any hex character code corresponding to a Unicode code point
\U0000007F      hex character code (exactly eight digits)
\U{7F}          any hex character code corresponding to a Unicode code point
\p{Letter}      Unicode character class
\P{Letter}      negated Unicode character class
\d, \s, \w      Perl character class
\D, \S, \W      negated Perl character class
</pre>

### Perl character classes (Unicode friendly)

These classes are based on the definitions provided in
[UTS#18](https://www.unicode.org/reports/tr18/#Compatibility_Properties):

<pre class="rust">
\d     digit (\p{Nd})
\D     not digit
\s     whitespace (\p{White_Space})
\S     not whitespace
\w     word character (\p{Alphabetic} + \p{M} + \d + \p{Pc} + \p{Join_Control})
\W     not word character
</pre>

### ASCII character classes

These classes are based on the definitions provided in
[UTS#18](https://www.unicode.org/reports/tr18/#Compatibility_Properties):

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

It's also worth pointing out that this library will *generally* panic when
other regex engines would commit undefined behavior. When undefined behavior
occurs, your program might continue as if nothing bad has happened, but it also
might mean your program is open to the worst kinds of exploits. In contrast,
the worst thing a panic can do is a denial of service.

[OSS-fuzz project]: https://android.googlesource.com/platform/external/oss-fuzz/+/refs/tags/android-t-preview-1/projects/rust-regex/
[`std::panic::catch_unwind`]: https://doc.rust-lang.org/std/panic/fn.catch_unwind.html

### Untrusted patterns

The principal way this crate deals with them is by limiting their size by
default. The size limit can be configured via [`RegexBuilder::size_limit`]. The
idea of a size limit is that compiling a pattern into a `Regex` will fail if it
becomes "too big." Namely, while *most* resources consumed by compiling a regex
are approximately proportional (albeit with some high constant factors in some
cases, such as with Unicode character classes) to the length of the pattern
itself, there is one particular exception to this: counted repetitions. Namely,
this pattern:

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

* The pattern string is parsed into a structured representation called an AST.
Counted repetitions are not expanded and Unicode character classes are not
looked up in this stage. That is, the size of the AST is proportional to the
size of the pattern with "reasonable" constant factors. In other words, one
can reasonably limit the memory used by an AST by limiting the length of the
pattern string.
* The AST is translated into an HIR. Counted repetitions are still *not*
expanded at this stage, but Unicode character classes are embedded into the
HIR. The memory usage of a HIR is still proportional to the length of the
original pattern string, but the constant factors---mostly as a result of
Unicode character classes---can be quite high. Still though, the memory used by
an HIR can be reasonably limited by limiting the length of the pattern string.
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
are untrusted and you're iterating over all matches, you're susceptible to
worst case quadratic time complexity. One possible way to mitigate this
is to drop down to the lower level `regex-automata` crate and use its
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
it, a longer haystack will take more time to search. This crate may often make
very quick work of even long haystacks because of its literal optimizations,
but those aren't available for all regexes.
* Unicode character classes can cause searches to be quite slow in some cases.
This is especially true when they are combined with counted repetitions. While
the regex size limit above will protect you from the most egregious cases,
the default size limit still permits pretty big regexes that can execute more
slowly than one might expect.
* While routines like [`Regex::find`] and [`Regex::captures`] guarantee
worst case `O(m * n)` search time, routines like [`Regex::find_iter`] and
[`Regex::captures_iter`] actually have worst case `O(m * n^2)` search time.
This is because `find_iter` runs many searches, and each search takes worst
case `O(m * n)` time. Thus, iteration of all matches in a haystack has
worst case `O(m * n^2)`. A good example of a pattern that exhibits this is
`(?:A+){1000}|` or even `.*[^A-Z]|[A-Z]`.

In general, unstrusted haystacks are easier to stomach than untrusted patterns.
Untrusted patterns give a lot more control to the caller to impact the
performance of a search. In many cases, a regex search will actually execute in
average case `O(n)` time (i.e., not dependent on the size of the regex), but
this can't be guaranteed in general. Therefore, permitting untrusted patterns
means that your only line of defense is to put a limit on how big `m` (and
perhaps also `n`) can be in `O(m * n)`. `n` is limited by simply inspecting
the length of the haystack while `m` is limited by *both* applying a limit to
the length of the pattern *and* a limit on the compiled size of the regex via
[`RegexBuilder::size_limit`].

It bears repeating: if you're accepting untrusted patterns, it would be a good
idea to start with conservative limits on `m` and `n`, and then carefully
increase them as needed.

# Crate features

By default, this crate tries pretty hard to make regex matching both as fast
as possible and as correct as it can be. This means that there is a lot of
code dedicated to performance, the handling of Unicode data and the Unicode
data itself. Overall, this leads to more dependencies, larger binaries and
longer compile times. This trade off may not be appropriate in all cases, and
indeed, even when all Unicode and performance features are disabled, one is
still left with a perfectly serviceable regex engine that will work well in
many cases. (Note that code is not arbitrarily reducible, and for this reason,
the [`regex-lite`](https://docs.rs/regex-lite) crate exists to provide an even
more minimal experience by cutting out Unicode and performance, but still
maintaining the linear search time bound.)

This crate exposes a number of features for controlling that trade off. Some
of these features are strictly performance oriented, such that disabling them
won't result in a loss of functionality, but may result in worse performance.
Other features, such as the ones controlling the presence or absence of Unicode
data, can result in a loss of functionality. For example, if one disables the
`unicode-case` feature (described below), then compiling the regex `(?i)a`
will fail since Unicode case insensitivity is enabled by default. Instead,
callers must use `(?i-u)a` to disable Unicode case folding. Stated differently,
enabling or disabling any of the features below can only add or subtract from
the total set of valid regular expressions. Enabling or disabling a feature
will never modify the match semantics of a regular expression.

Most features below are enabled by default. Features that aren't enabled by
default are noted.

### Ecosystem features

* **std** -
  When enabled, this will cause `regex` to use the standard library. In terms
  of APIs, `std` causes error types to implement the `std::error::Error`
  trait. Enabling `std` will also result in performance optimizations,
  including SIMD and faster synchronization primitives. Notably, **disabling
  the `std` feature will result in the use of spin locks**. To use a regex
  engine without `std` and without spin locks, you'll need to drop down to
  the [`regex-automata`](https://docs.rs/regex-automata) crate.
* **logging** -
  When enabled, the `log` crate is used to emit messages about regex
  compilation and search strategies. This is **disabled by default**. This is
  typically only useful to someone working on this crate's internals, but might
  be useful if you're doing some rabbit hole performance hacking. Or if you're
  just interested in the kinds of decisions being made by the regex engine.

### Performance features

* **perf** -
  Enables all performance related features except for `perf-dfa-full`. This
  feature is enabled by default is intended to cover all reasonable features
  that improve performance, even if more are added in the future.
* **perf-dfa** -
  Enables the use of a lazy DFA for matching. The lazy DFA is used to compile
  portions of a regex to a very fast DFA on an as-needed basis. This can
  result in substantial speedups, usually by an order of magnitude on large
  haystacks. The lazy DFA does not bring in any new dependencies, but it can
  make compile times longer.
* **perf-dfa-full** -
  Enables the use of a full DFA for matching. Full DFAs are problematic because
  they have worst case `O(2^n)` construction time. For this reason, when this
  feature is enabled, full DFAs are only used for very small regexes and a
  very small space bound is used during determinization to avoid the DFA
  from blowing up. This feature is not enabled by default, even as part of
  `perf`, because it results in fairly sizeable increases in binary size and
  compilation time. It can result in faster search times, but they tend to be
  more modest and limited to non-Unicode regexes.
* **perf-onepass** -
  Enables the use of a one-pass DFA for extracting the positions of capture
  groups. This optimization applies to a subset of certain types of NFAs and
  represents the fastest engine in this crate for dealing with capture groups.
* **perf-backtrack** -
  Enables the use of a bounded backtracking algorithm for extracting the
  positions of capture groups. This usually sits between the slowest engine
  (the PikeVM) and the fastest engine (one-pass DFA) for extracting capture
  groups. It's used whenever the regex is not one-pass and is small enough.
* **perf-inline** -
  Enables the use of aggressive inlining inside match routines. This reduces
  the overhead of each match. The aggressive inlining, however, increases
  compile times and binary size.
* **perf-literal** -
  Enables the use of literal optimizations for speeding up matches. In some
  cases, literal optimizations can result in speedups of _several_ orders of
  magnitude. Disabling this drops the `aho-corasick` and `memchr` dependencies.
* **perf-cache** -
  This feature used to enable a faster internal cache at the cost of using
  additional dependencies, but this is no longer an option. A fast internal
  cache is now used unconditionally with no additional dependencies. This may
  change in the future.

### Unicode features

* **unicode** -
  Enables all Unicode features. This feature is enabled by default, and will
  always cover all Unicode features, even if more are added in the future.
* **unicode-age** -
  Provide the data for the
  [Unicode `Age` property](https://www.unicode.org/reports/tr44/tr44-24.html#Character_Age).
  This makes it possible to use classes like `\p{Age:6.0}` to refer to all
  codepoints first introduced in Unicode 6.0
* **unicode-bool** -
  Provide the data for numerous Unicode boolean properties. The full list
  is not included here, but contains properties like `Alphabetic`, `Emoji`,
  `Lowercase`, `Math`, `Uppercase` and `White_Space`.
* **unicode-case** -
  Provide the data for case insensitive matching using
  [Unicode's "simple loose matches" specification](https://www.unicode.org/reports/tr18/#Simple_Loose_Matches).
* **unicode-gencat** -
  Provide the data for
  [Unicode general categories](https://www.unicode.org/reports/tr44/tr44-24.html#General_Category_Values).
  This includes, but is not limited to, `Decimal_Number`, `Letter`,
  `Math_Symbol`, `Number` and `Punctuation`.
* **unicode-perl** -
  Provide the data for supporting the Unicode-aware Perl character classes,
  corresponding to `\w`, `\s` and `\d`. This is also necessary for using
  Unicode-aware word boundary assertions. Note that if this feature is
  disabled, the `\s` and `\d` character classes are still available if the
  `unicode-bool` and `unicode-gencat` features are enabled, respectively.
* **unicode-script** -
  Provide the data for
  [Unicode scripts and script extensions](https://www.unicode.org/reports/tr24/).
  This includes, but is not limited to, `Arabic`, `Cyrillic`, `Hebrew`,
  `Latin` and `Thai`.
* **unicode-segment** -
  Provide the data necessary to provide the properties used to implement the
  [Unicode text segmentation algorithms](https://www.unicode.org/reports/tr29/).
  This enables using classes like `\p{gcb=Extend}`, `\p{wb=Katakana}` and
  `\p{sb=ATerm}`.

# Other crates

This crate has two required dependencies and several optional dependencies.
This section briefly describes them with the goal of raising awareness of how
different components of this crate may be used independently.

It is somewhat unusual for a regex engine to have dependencies, as most regex
libraries are self contained units with no dependencies other than a particular
environment's standard library. Indeed, for other similarly optimized regex
engines, most or all of the code in the dependencies of this crate would
normally just be unseparable or coupled parts of the crate itself. But since
Rust and its tooling ecosystem make the use of dependencies so easy, it made
sense to spend some effort de-coupling parts of this crate and making them
independently useful.

We only briefly describe each crate here.

* [`regex-lite`](https://docs.rs/regex-lite) is not a dependency of `regex`,
but rather, a standalone zero-dependency simpler version of `regex` that
prioritizes compile times and binary size. In exchange, it eschews Unicode
support and performance. Its match semantics are as identical as possible to
the `regex` crate, and for the things it supports, its APIs are identical to
the APIs in this crate. In other words, for a lot of use cases, it is a drop-in
replacement.
* [`regex-syntax`](https://docs.rs/regex-syntax) provides a regular expression
parser via `Ast` and `Hir` types. It also provides routines for extracting
literals from a pattern. Folks can use this crate to do analysis, or even to
build their own regex engine without having to worry about writing a parser.
* [`regex-automata`](https://docs.rs/regex-automata) provides the regex engines
themselves. One of the downsides of finite automata based regex engines is that
they often need multiple internal engines in order to have similar or better
performance than an unbounded backtracking engine in practice. `regex-automata`
in particular provides public APIs for a PikeVM, a bounded backtracker, a
one-pass DFA, a lazy DFA, a fully compiled DFA and a meta regex engine that
combines all them together. It also has native multi-pattern support and
provides a way to compile and serialize full DFAs such that they can be loaded
and searched in a no-std no-alloc environment. `regex-automata` itself doesn't
even have a required dependency on `regex-syntax`!
* [`memchr`](https://docs.rs/memchr) provides low level SIMD vectorized
routines for quickly finding the location of single bytes or even substrings
in a haystack. In other words, it provides fast `memchr` and `memmem` routines.
These are used by this crate in literal optimizations.
* [`aho-corasick`](https://docs.rs/aho-corasick) provides multi-substring
search. It also provides SIMD vectorized routines in the case where the number
of substrings to search for is relatively small. The `regex` crate also uses
this for literal optimizations.
*/

#![no_std]
#![deny(missing_docs)]
#![cfg_attr(feature = "pattern", feature(pattern))]
#![warn(missing_debug_implementations)]

#[cfg(doctest)]
doc_comment::doctest!("../README.md");

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub use crate::error::Error;

pub use crate::{builders::string::*, regex::string::*, regexset::string::*};

mod builders;
pub mod bytes;
mod error;
mod find_byte;
#[cfg(feature = "pattern")]
mod pattern;
mod regex;
mod regexset;

/// Escapes all regular expression meta characters in `pattern`.
///
/// The string returned may be safely used as a literal in a regular
/// expression.
pub fn escape(pattern: &str) -> alloc::string::String {
    regex_syntax::escape(pattern)
}
