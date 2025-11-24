# Unicode conformance

This document describes the regex crate's conformance to Unicode's
[UTS#18](https://unicode.org/reports/tr18/)
report, which lays out 3 levels of support: Basic, Extended and Tailored.

Full support for Level 1 ("Basic Unicode Support") is provided with two
exceptions:

1. Line boundaries are not Unicode aware. Namely, only the `\n`
   (`END OF LINE`) character is recognized as a line boundary by default.
   One can opt into `\r\n|\r|\n` being a line boundary via CRLF mode.
2. The compatibility properties specified by
   [RL1.2a](https://unicode.org/reports/tr18/#RL1.2a)
   are ASCII-only definitions.

Little to no support is provided for either Level 2 or Level 3. For the most
part, this is because the features are either complex/hard to implement, or at
the very least, very difficult to implement without sacrificing performance.
For example, tackling canonical equivalence such that matching worked as one
would expect regardless of normalization form would be a significant
undertaking. This is at least partially a result of the fact that this regex
engine is based on finite automata, which admits less flexibility normally
associated with backtracking implementations.


## RL1.1 Hex Notation

[UTS#18 RL1.1](https://unicode.org/reports/tr18/#Hex_notation)

Hex Notation refers to the ability to specify a Unicode code point in a regular
expression via its hexadecimal code point representation. This is useful in
environments that have poor Unicode font rendering or if you need to express a
code point that is not normally displayable. All forms of hexadecimal notation
are supported

    \x7F        hex character code (exactly two digits)
    \x{10FFFF}  any hex character code corresponding to a Unicode code point
    \u007F      hex character code (exactly four digits)
    \u{7F}      any hex character code corresponding to a Unicode code point
    \U0000007F  hex character code (exactly eight digits)
    \U{7F}      any hex character code corresponding to a Unicode code point

Briefly, the `\x{...}`, `\u{...}` and `\U{...}` are all exactly equivalent ways
of expressing hexadecimal code points. Any number of digits can be written
within the brackets. In contrast, `\xNN`, `\uNNNN`, `\UNNNNNNNN` are all
fixed-width variants of the same idea.

Note that when Unicode mode is disabled, any non-ASCII Unicode codepoint is
banned. Additionally, the `\xNN` syntax represents arbitrary bytes when Unicode
mode is disabled. That is, the regex `\xFF` matches the Unicode codepoint
U+00FF (encoded as `\xC3\xBF` in UTF-8) while the regex `(?-u)\xFF` matches
the literal byte `\xFF`.


## RL1.2 Properties

[UTS#18 RL1.2](https://unicode.org/reports/tr18/#Categories)

Full support for Unicode property syntax is provided. Unicode properties
provide a convenient way to construct character classes of groups of code
points specified by Unicode. The regex crate does not provide exhaustive
support, but covers a useful subset. In particular:

* [General categories](https://unicode.org/reports/tr18/#General_Category_Property)
* [Scripts and Script Extensions](https://unicode.org/reports/tr18/#Script_Property)
* [Age](https://unicode.org/reports/tr18/#Age)
* A smattering of boolean properties, including all of those specified by
  [RL1.2](https://unicode.org/reports/tr18/#RL1.2) explicitly.

In all cases, property name and value abbreviations are supported, and all
names/values are matched loosely without regard for case, whitespace or
underscores. Property name aliases can be found in Unicode's
[`PropertyAliases.txt`](https://www.unicode.org/Public/UCD/latest/ucd/PropertyAliases.txt)
file, while property value aliases can be found in Unicode's
[`PropertyValueAliases.txt`](https://www.unicode.org/Public/UCD/latest/ucd/PropertyValueAliases.txt)
file.

The syntax supported is also consistent with the UTS#18 recommendation:

* `\p{Greek}` selects the `Greek` script. Equivalent expressions follow:
  `\p{sc:Greek}`, `\p{Script:Greek}`, `\p{Sc=Greek}`, `\p{script=Greek}`,
  `\P{sc!=Greek}`. Similarly for `General_Category` (or `gc` for short) and
  `Script_Extensions` (or `scx` for short).
* `\p{age:3.2}` selects all code points in Unicode 3.2.
* `\p{Alphabetic}` selects the "alphabetic" property and can be abbreviated
  via `\p{alpha}` (for example).
* Single letter variants for properties with single letter abbreviations.
  For example, `\p{Letter}` can be equivalently written as `\pL`.

The following is a list of all properties supported by the regex crate (starred
properties correspond to properties required by RL1.2):

* `General_Category` \* (including `Any`, `ASCII` and `Assigned`)
* `Script` \*
* `Script_Extensions` \*
* `Age`
* `ASCII_Hex_Digit`
* `Alphabetic` \*
* `Bidi_Control`
* `Case_Ignorable`
* `Cased`
* `Changes_When_Casefolded`
* `Changes_When_Casemapped`
* `Changes_When_Lowercased`
* `Changes_When_Titlecased`
* `Changes_When_Uppercased`
* `Dash`
* `Default_Ignorable_Code_Point` \*
* `Deprecated`
* `Diacritic`
* `Emoji`
* `Emoji_Presentation`
* `Emoji_Modifier`
* `Emoji_Modifier_Base`
* `Emoji_Component`
* `Extended_Pictographic`
* `Extender`
* `Grapheme_Base`
* `Grapheme_Cluster_Break`
* `Grapheme_Extend`
* `Hex_Digit`
* `IDS_Binary_Operator`
* `IDS_Trinary_Operator`
* `ID_Continue`
* `ID_Start`
* `Join_Control`
* `Logical_Order_Exception`
* `Lowercase` \*
* `Math`
* `Noncharacter_Code_Point` \*
* `Pattern_Syntax`
* `Pattern_White_Space`
* `Prepended_Concatenation_Mark`
* `Quotation_Mark`
* `Radical`
* `Regional_Indicator`
* `Sentence_Break`
* `Sentence_Terminal`
* `Soft_Dotted`
* `Terminal_Punctuation`
* `Unified_Ideograph`
* `Uppercase` \*
* `Variation_Selector`
* `White_Space` \*
* `Word_Break`
* `XID_Continue`
* `XID_Start`


## RL1.2a Compatibility Properties

[UTS#18 RL1.2a](https://unicode.org/reports/tr18/#RL1.2a)

The regex crate only provides ASCII definitions of the
[compatibility properties documented in UTS#18 Annex C](https://unicode.org/reports/tr18/#Compatibility_Properties)
(sans the `\X` class, for matching grapheme clusters, which isn't provided
at all). This is because it seems to be consistent with most other regular
expression engines, and in particular, because these are often referred to as
"ASCII" or "POSIX" character classes.

Note that the `\w`, `\s` and `\d` character classes **are** Unicode aware.
Their traditional ASCII definition can be used by disabling Unicode. That is,
`[[:word:]]` and `(?-u)\w` are equivalent.


## RL1.3 Subtraction and Intersection

[UTS#18 RL1.3](https://unicode.org/reports/tr18/#Subtraction_and_Intersection)

The regex crate provides full support for nested character classes, along with
union, intersection (`&&`), difference (`--`) and symmetric difference (`~~`)
operations on arbitrary character classes.

For example, to match all non-ASCII letters, you could use either
`[\p{Letter}--\p{Ascii}]` (difference) or `[\p{Letter}&&[^\p{Ascii}]]`
(intersecting the negation).


## RL1.4 Simple Word Boundaries

[UTS#18 RL1.4](https://unicode.org/reports/tr18/#Simple_Word_Boundaries)

The regex crate provides basic Unicode aware word boundary assertions. A word
boundary assertion can be written as `\b`, or `\B` as its negation. A word
boundary negation corresponds to a zero-width match, where its adjacent
characters correspond to word and non-word, or non-word and word characters.

Conformance in this case chooses to define word character in the same way that
the `\w` character class is defined: a code point that is a member of one of
the following classes:

* `\p{Alphabetic}`
* `\p{Join_Control}`
* `\p{gc:Mark}`
* `\p{gc:Decimal_Number}`
* `\p{gc:Connector_Punctuation}`

In particular, this differs slightly from the
[prescription given in RL1.4](https://unicode.org/reports/tr18/#Simple_Word_Boundaries)
but is permissible according to
[UTS#18 Annex C](https://unicode.org/reports/tr18/#Compatibility_Properties).
Namely, it is convenient and simpler to have `\w` and `\b` be in sync with
one another.

Finally, Unicode word boundaries can be disabled, which will cause ASCII word
boundaries to be used instead. That is, `\b` is a Unicode word boundary while
`(?-u)\b` is an ASCII-only word boundary. This can occasionally be beneficial
if performance is important, since the implementation of Unicode word
boundaries is currently sub-optimal on non-ASCII text.


## RL1.5 Simple Loose Matches

[UTS#18 RL1.5](https://unicode.org/reports/tr18/#Simple_Loose_Matches)

The regex crate provides full support for case insensitive matching in
accordance with RL1.5. That is, it uses the "simple" case folding mapping. The
"simple" mapping was chosen because of a key convenient property: every
"simple" mapping is a mapping from exactly one code point to exactly one other
code point. This makes case insensitive matching of character classes, for
example, straight-forward to implement.

When case insensitive mode is enabled (e.g., `(?i)[a]` is equivalent to `a|A`),
then all characters classes are case folded as well.


## RL1.6 Line Boundaries

[UTS#18 RL1.6](https://unicode.org/reports/tr18/#Line_Boundaries)

The regex crate only provides support for recognizing the `\n` (`END OF LINE`)
character as a line boundary by default. One can also opt into treating
`\r\n|\r|\n` as a line boundary via CRLF mode. This choice was made mostly for
implementation convenience, and to avoid performance cliffs that Unicode word
boundaries are subject to.


## RL1.7 Code Points

[UTS#18 RL1.7](https://unicode.org/reports/tr18/#Supplementary_Characters)

The regex crate provides full support for Unicode code point matching. Namely,
the fundamental atom of any match is always a single code point.

Given Rust's strong ties to UTF-8, the following guarantees are also provided:

* All matches are reported on valid UTF-8 code unit boundaries. That is, any
  match range returned by the public regex API is guaranteed to successfully
  slice the string that was searched.
* By consequence of the above, it is impossible to match surrogode code points.
  No support for UTF-16 is provided, so this is never necessary.

Note that when Unicode mode is disabled, the fundamental atom of matching is
no longer a code point but a single byte. When Unicode mode is disabled, many
Unicode features are disabled as well. For example, `(?-u)\pL` is not a valid
regex but `\pL(?-u)\xFF` (matches any Unicode `Letter` followed by the literal
byte `\xFF`) is, for example.
