# The Grammar DSL

The following is a complete list of built-in functions you can use in your `grammar.js` to define rules. Use-cases for some
of these functions will be explained in more detail in later sections.

- **Symbols (the `$` object)** — Every grammar rule is written as a JavaScript function that takes a parameter conventionally
called `$`. The syntax `$.identifier` is how you refer to another grammar symbol within a rule. Names starting with `$.MISSING`
or `$.UNEXPECTED` should be avoided as they have special meaning for the `tree-sitter test` command.
- **String and Regex literals** — The terminal symbols in a grammar are described using JavaScript strings and regular
expressions. Of course during parsing, Tree-sitter does not actually use JavaScript's regex engine to evaluate these regexes;
it generates its own regex-matching logic based on the Rust regex syntax as part of each parser. Regex literals are just
used as a convenient way of writing regular expressions in your grammar. You can use Rust regular expressions in your grammar
DSL through the `RustRegex` class. Simply pass your regex pattern as a string:

```js
new RustRegex('(?i)[a-z_][a-z0-9_]*') // matches a simple identifier
```

Unlike JavaScript's builtin `RegExp` class, which takes a pattern and flags as separate arguments, `RustRegex` only
accepts a single pattern string. While it doesn't support separate flags, you can use inline flags within the pattern itself.
For more details about Rust's regex syntax and capabilities, check out the [Rust regex documentation][rust regex].

- **Regex Limitations** — Only a subset of the Regex engine is actually
supported. This is due to certain features like lookahead and lookaround assertions
not feasible to use in an LR(1) grammar, as well as certain flags being unnecessary
for tree-sitter. However, plenty of features are supported by default:

  - Character classes
  - Character ranges
  - Character sets
  - Quantifiers
  - Alternation
  - Grouping
  - Unicode character escapes
  - Unicode property escapes

- **Sequences : `seq(rule1, rule2, ...)`** — This function creates a rule that matches any number of other rules, one after
another. It is analogous to simply writing multiple symbols next to each other in [EBNF notation][ebnf].

- **Alternatives : `choice(rule1, rule2, ...)`** — This function creates a rule that matches *one* of a set of possible
rules. The order of the arguments does not matter. This is analogous to the `|` (pipe) operator in EBNF notation.

- **Repetitions : `repeat(rule)`** — This function creates a rule that matches *zero-or-more* occurrences of a given rule.
It is analogous to the `{x}` (curly brace) syntax in EBNF notation.

- **Repetitions : `repeat1(rule)`** — This function creates a rule that matches *one-or-more* occurrences of a given rule.
The previous `repeat` rule is implemented in `repeat1` but is included because it is very commonly used.

- **Options : `optional(rule)`** — This function creates a rule that matches *zero or one* occurrence of a given rule.
It is analogous to the `[x]` (square bracket) syntax in EBNF notation.

- **Precedence : `prec(number, rule)`** — This function marks the given rule with a numerical precedence, which will be used
to resolve [*LR(1) Conflicts*][lr-conflict] at parser-generation time. When two rules overlap in a way that represents either
a true ambiguity or a *local* ambiguity given one token of lookahead, Tree-sitter will try to resolve the conflict by matching
the rule with the higher precedence. The default precedence of all rules is zero. This works similarly to the
[precedence directives][yacc-prec] in Yacc grammars.

  This function can also be used to assign lexical precedence to a given
  token, but it must be wrapped in a `token` call, such as `token(prec(1, 'foo'))`. This reads as "the token `foo` has a
  lexical precedence of 1". The purpose of lexical precedence is to solve the issue where multiple tokens can match the same
  set of characters, but one token should be preferred over the other. See [Lexical Precedence vs Parse Precedence][lexical vs parse]
  for a more detailed explanation.

- **Left Associativity : `prec.left([number], rule)`** — This function marks the given rule as left-associative (and optionally
applies a numerical precedence). When an LR(1) conflict arises in which all the rules have the same numerical precedence,
Tree-sitter will consult the rules' associativity. If there is a left-associative rule, Tree-sitter will prefer matching
a rule that ends *earlier*. This works similarly to [associativity directives][yacc-prec] in Yacc grammars.

- **Right Associativity : `prec.right([number], rule)`** — This function is like `prec.left`, but it instructs Tree-sitter
to prefer matching a rule that ends *later*.

- **Dynamic Precedence : `prec.dynamic(number, rule)`** — This function is similar to `prec`, but the given numerical precedence
is applied at *runtime* instead of at parser generation time. This is only necessary when handling a conflict dynamically
using the `conflicts` field in the grammar, and when there is a genuine *ambiguity*: multiple rules correctly match a given
piece of code. In that event, Tree-sitter compares the total dynamic precedence associated with each rule, and selects the
one with the highest total. This is similar to [dynamic precedence directives][bison-dprec] in Bison grammars.

- **Tokens : `token(rule)`** — This function marks the given rule as producing only
a single token. Tree-sitter's default is to treat each String or RegExp literal
in the grammar as a separate token. Each token is matched separately by the lexer
and returned as its own leaf node in the tree. The `token` function allows you to
express a complex rule using the functions described above (rather than as a single
regular expression) but still have Tree-sitter treat it as a single token.
The token function will only accept terminal rules, so `token($.foo)` will not work.
You can think of it as a shortcut for squashing complex rules of strings or regexes
down to a single token.

- **Immediate Tokens : `token.immediate(rule)`** — Usually, whitespace (and any other extras, such as comments) is optional
before each token. This function means that the token will only match if there is no whitespace.

- **Aliases : `alias(rule, name)`** — This function causes the given rule to *appear* with an alternative name in the syntax
tree. If `name` is a *symbol*, as in `alias($.foo, $.bar)`, then the aliased rule will *appear* as a [named node][named-vs-anonymous-nodes]
called `bar`. And if `name` is a *string literal*, as in `alias($.foo, 'bar')`, then the aliased rule will appear as an
[anonymous node][named-vs-anonymous-nodes], as if the rule had been written as the simple string.

- **Field Names : `field(name, rule)`** — This function assigns a *field name* to the child node(s) matched by the given
rule. In the resulting syntax tree, you can then use that field name to access specific children.

- **Reserved Keywords : `reserved(wordset, rule)`**  — This function will override the global reserved word set with the
one passed into the `wordset` parameter. This is useful for contextual keywords, such as `if` in JavaScript, which cannot
be used as a variable name in most contexts, but can be used as a property name.

In addition to the `name` and `rules` fields, grammars have a few other optional public fields that influence the behavior
of the parser. Each of these fields is a function that accepts the grammar object (`$`) as its only parameter, like the
grammar rules themselves. These fields are:

- **`extras`** — an array of tokens that may appear *anywhere* in the language. This is often used for whitespace and
comments. The default value of `extras` is to accept whitespace. To control whitespace explicitly, specify
`extras: $ => []` in your grammar.

- **`inline`** — an array of rule names that should be automatically *removed* from the grammar by replacing all of their
usages with a copy of their definition. This is useful for rules that are used in multiple places but for which you *don't*
want to create syntax tree nodes at runtime.

- **`conflicts`** — an array of arrays of rule names. Each inner array represents a set of rules that's involved in an
*LR(1) conflict* that is *intended to exist* in the grammar. When these conflicts occur at runtime, Tree-sitter will use
the GLR algorithm to explore all the possible interpretations. If *multiple* parses end up succeeding, Tree-sitter will pick
the subtree whose corresponding rule has the highest total *dynamic precedence*.

- **`externals`** — an array of token names which can be returned by an
[*external scanner*][external-scanners]. External scanners allow you to write custom C code which runs during the lexing
process to handle lexical rules (e.g. Python's indentation tokens) that cannot be described by regular expressions.

- **`precedences`** — an array of arrays of strings, where each array of strings defines named precedence levels in descending
order. These names can be used in the `prec` functions to define precedence relative only to other names in the array, rather
than globally. Can only be used with parse precedence, not lexical precedence.

- **`word`** — the name of a token that will match keywords to the
[keyword extraction][keyword-extraction] optimization.

- **`supertypes`** — an array of hidden rule names which should be considered to be 'supertypes' in the generated
[*node types* file][static-node-types].

- **`reserved`** — similar in structure to the main `rules` property, an object of reserved word sets associated with an
array of reserved rules. The reserved rule in the array must be a terminal token meaning it must be a string, regex, or token,
or a terminal rule. The *first* reserved word set in the object is the global word set, meaning it applies to every rule
in every parse state. However, certain keywords are contextual, depending on the rule. For example, in JavaScript, keywords
are typically not allowed as ordinary variables, however, they *can* be used as a property name. In this situation, the `reserved`
function would be used, and the word set to pass in would be the name of the word set that is declared in the `reserved`
object that coreesponds an empty array, signifying *no* keywords are reserved.

[bison-dprec]: https://www.gnu.org/software/bison/manual/html_node/Generalized-LR-Parsing.html
[ebnf]: https://en.wikipedia.org/wiki/Extended_Backus%E2%80%93Naur_form
[external-scanners]: ./4-external-scanners.md
[keyword-extraction]: ./3-writing-the-grammar.md#keyword-extraction
[lexical vs parse]: ./3-writing-the-grammar.md#lexical-precedence-vs-parse-precedence
[lr-conflict]: https://en.wikipedia.org/wiki/LR_parser#Conflicts_in_the_constructed_tables
[named-vs-anonymous-nodes]: ../using-parsers/2-basic-parsing.md#named-vs-anonymous-nodes
[rust regex]: https://docs.rs/regex/1.1.8/regex/#grouping-and-flags
[static-node-types]: ../using-parsers/6-static-node-types.md
[yacc-prec]: https://docs.oracle.com/cd/E19504-01/802-5880/6i9k05dh3/index.html
