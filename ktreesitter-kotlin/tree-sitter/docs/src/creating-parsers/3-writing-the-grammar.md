# Writing the Grammar

Writing a grammar requires creativity. There are an infinite number of CFGs (context-free grammars) that can be used to describe
any given language. To produce a good Tree-sitter parser, you need to create a grammar with two important properties:

1. **An intuitive structure** — Tree-sitter's output is a [concrete syntax tree][cst]; each node in the tree corresponds
directly to a [terminal or non-terminal symbol][non-terminal] in the grammar. So to produce an easy-to-analyze tree, there
should be a direct correspondence between the symbols in your grammar and the recognizable constructs in the language.
This might seem obvious, but it is very different from the way that context-free grammars are often written in contexts
like [language specifications][language-spec] or [Yacc][yacc]/[Bison][bison] parsers.

2. **A close adherence to LR(1)** — Tree-sitter is based on the [GLR parsing][glr-parsing] algorithm. This means that while
it can handle any context-free grammar, it works most efficiently with a class of context-free grammars called [LR(1) Grammars][lr-grammars].
In this respect, Tree-sitter's grammars are similar to (but less restrictive than) [Yacc][yacc] and [Bison][bison] grammars,
but _different_ from [ANTLR grammars][antlr], [Parsing Expression Grammars][peg], or the [ambiguous grammars][ambiguous-grammar]
commonly used in language specifications.

It's unlikely that you'll be able to satisfy these two properties just by translating an existing context-free grammar directly
into Tree-sitter's grammar format. There are a few kinds of adjustments that are often required.
The following sections will explain these adjustments in more depth.

## The First Few Rules

It's usually a good idea to find a formal specification for the language you're trying to parse. This specification will
most likely contain a context-free grammar. As you read through the rules of this CFG, you will probably discover a complex
and cyclic graph of relationships. It might be unclear how you should navigate this graph as you define your grammar.

Although languages have very different constructs, their constructs can often be categorized in to similar groups like
_Declarations_, _Definitions_, _Statements_, _Expressions_, _Types_ and _Patterns_. In writing your grammar, a good first
step is to create just enough structure to include all of these basic _groups_ of symbols. For a language like Go,
you might start with something like this:

```js
{
  // ...

  rules: {
    source_file: $ => repeat($._definition),

    _definition: $ => choice(
      $.function_definition
      // TODO: other kinds of definitions
    ),

    function_definition: $ => seq(
      'func',
      $.identifier,
      $.parameter_list,
      $._type,
      $.block
    ),

    parameter_list: $ => seq(
      '(',
       // TODO: parameters
      ')'
    ),

    _type: $ => choice(
      'bool'
      // TODO: other kinds of types
    ),

    block: $ => seq(
      '{',
      repeat($._statement),
      '}'
    ),

    _statement: $ => choice(
      $.return_statement
      // TODO: other kinds of statements
    ),

    return_statement: $ => seq(
      'return',
      $._expression,
      ';'
    ),

    _expression: $ => choice(
      $.identifier,
      $.number
      // TODO: other kinds of expressions
    ),

    identifier: $ => /[a-z]+/,

    number: $ => /\d+/
  }
}
```

One important fact to know up front is that the start rule for the grammar is the first property in the `rules` object.
In the example above, that would correspond to `source_file`, but it can be named anything.

Some details of this grammar will be explained in more depth later on, but if you focus on the `TODO` comments, you can
see that the overall strategy is _breadth-first_. Notably, this initial skeleton does not need to directly match an exact
subset of the context-free grammar in the language specification. It just needs to touch on the major groupings of rules
in as simple and obvious a way as possible.

With this structure in place, you can now freely decide what part of the grammar to flesh out next. For example, you might
decide to start with _types_. One-by-one, you could define the rules for writing basic types and composing them into more
complex types:

```js
{
  // ...

  _type: $ => choice(
    $.primitive_type,
    $.array_type,
    $.pointer_type
  ),

  primitive_type: $ => choice(
    'bool',
    'int'
  ),

  array_type: $ => seq(
    '[',
    ']',
    $._type
  ),

  pointer_type: $ => seq(
    '*',
    $._type
  )
}
```

After developing the _type_ sublanguage a bit further, you might decide to switch to working on _statements_ or _expressions_
instead. It's often useful to check your progress by trying to parse some real code using `tree-sitter parse`.

**And remember to add tests for each rule in your `test/corpus` folder!**

## Structuring Rules Well

Imagine that you were just starting work on the [Tree-sitter JavaScript parser][tree-sitter-javascript]. Naively, you might
try to directly mirror the structure of the [ECMAScript Language Spec][ecmascript-spec]. To illustrate the problem with this
approach, consider the following line of code:

```js
return x + y;
```

According to the specification, this line is a `ReturnStatement`, the fragment `x + y` is an `AdditiveExpression`,
and `x` and `y` are both `IdentifierReferences`. The relationship between these constructs is captured by a complex series
of production rules:

```text
ReturnStatement          ->  'return' Expression
Expression               ->  AssignmentExpression
AssignmentExpression     ->  ConditionalExpression
ConditionalExpression    ->  LogicalORExpression
LogicalORExpression      ->  LogicalANDExpression
LogicalANDExpression     ->  BitwiseORExpression
BitwiseORExpression      ->  BitwiseXORExpression
BitwiseXORExpression     ->  BitwiseANDExpression
BitwiseANDExpression     ->  EqualityExpression
EqualityExpression       ->  RelationalExpression
RelationalExpression     ->  ShiftExpression
ShiftExpression          ->  AdditiveExpression
AdditiveExpression       ->  MultiplicativeExpression
MultiplicativeExpression ->  ExponentiationExpression
ExponentiationExpression ->  UnaryExpression
UnaryExpression          ->  UpdateExpression
UpdateExpression         ->  LeftHandSideExpression
LeftHandSideExpression   ->  NewExpression
NewExpression            ->  MemberExpression
MemberExpression         ->  PrimaryExpression
PrimaryExpression        ->  IdentifierReference
```

The language spec encodes the twenty different precedence levels of JavaScript expressions using twenty levels of indirection
between `IdentifierReference` and `Expression`. If we were to create a concrete syntax tree representing this statement
according to the language spec, it would have twenty levels of nesting, and it would contain nodes with names like `BitwiseXORExpression`,
which are unrelated to the actual code.

## Standard Rule Names

Tree-sitter places no restrictions on how to name the rules of your grammar. It can be helpful, however, to follow certain conventions
used by many other established grammars in the ecosystem. Some of these well-established patterns are listed below:

- `source_file`: Represents an entire source file, this rule is commonly used as the root node for a grammar,
- `expression`/`statement`: Used to represent statements and expressions for a given language. Commonly defined as a choice between several
more specific sub-expression/sub-statement rules.
- `block`: Used as the parent node for block scopes, with its children representing the block's contents.
- `type`: Represents the types of a language such as `int`, `char`, and `void`.
- `identifier`: Used for constructs like variable names, function arguments, and object fields; this rule is commonly used as the `word`
token in grammars.
- `string`: Used to represent `"string literals"`.
- `comment`: Used to represent comments, this rule is commonly used as an `extra`.

## Using Precedence

To produce a readable syntax tree, we'd like to model JavaScript expressions using a much flatter structure like this:

```js
{
  // ...

  _expression: $ => choice(
    $.identifier,
    $.unary_expression,
    $.binary_expression,
    // ...
  ),

  unary_expression: $ => choice(
    seq('-', $._expression),
    seq('!', $._expression),
    // ...
  ),

  binary_expression: $ => choice(
    seq($._expression, '*', $._expression),
    seq($._expression, '+', $._expression),
    // ...
  ),
}
```

Of course, this flat structure is highly ambiguous. If we try to generate a parser, Tree-sitter gives us an error message:

```text
Error: Unresolved conflict for symbol sequence:

  '-'  _expression  •  '*'  …

Possible interpretations:

  1:  '-'  (binary_expression  _expression  •  '*'  _expression)
  2:  (unary_expression  '-'  _expression)  •  '*'  …

Possible resolutions:

  1:  Specify a higher precedence in `binary_expression` than in the other rules.
  2:  Specify a higher precedence in `unary_expression` than in the other rules.
  3:  Specify a left or right associativity in `unary_expression`
  4:  Add a conflict for these rules: `binary_expression` `unary_expression`
```

```admonish hint
The • character in the error message indicates where exactly during
parsing the conflict occurs, or in other words, where the parser is encountering
ambiguity.
```

For an expression like `-a * b`, it's not clear whether the `-` operator applies to the `a * b` or just to the `a`. This
is where the `prec` function [described in the previous page][grammar dsl] comes into play. By wrapping a rule with `prec`,
we can indicate that certain sequence of symbols should _bind to each other more tightly_ than others. For example, the
`'-', $._expression` sequence in `unary_expression` should bind more tightly than the `$._expression, '+', $._expression`
sequence in `binary_expression`:

```js
{
  // ...

  unary_expression: $ =>
    prec(
      2,
      choice(
        seq("-", $._expression),
        seq("!", $._expression),
        // ...
      ),
    );
}
```

## Using Associativity

Applying a higher precedence in `unary_expression` fixes that conflict, but there is still another conflict:

```text
Error: Unresolved conflict for symbol sequence:

  _expression  '*'  _expression  •  '*'  …

Possible interpretations:

  1:  _expression  '*'  (binary_expression  _expression  •  '*'  _expression)
  2:  (binary_expression  _expression  '*'  _expression)  •  '*'  …

Possible resolutions:

  1:  Specify a left or right associativity in `binary_expression`
  2:  Add a conflict for these rules: `binary_expression`
```

For an expression like `a * b * c`, it's not clear whether we mean `a * (b * c)` or `(a * b) * c`.
This is where `prec.left` and `prec.right` come into use. We want to select the second interpretation, so we use `prec.left`.

```js
{
  // ...

  binary_expression: $ => choice(
    prec.left(2, seq($._expression, '*', $._expression)),
    prec.left(1, seq($._expression, '+', $._expression)),
    // ...
  ),
}
```

## Using Conflicts

Sometimes, conflicts are actually desirable. In our JavaScript grammar, expressions and patterns can create intentional ambiguity.
A construct like `[x, y]` could be legitimately parsed as both an array literal (like in `let a = [x, y]`) or as a destructuring
pattern (like in `let [x, y] = arr`).

```js
module.exports = grammar({
  name: "javascript",

  rules: {
    expression: $ => choice(
      $.identifier,
      $.array,
      $.pattern,
    ),

    array: $ => seq(
      "[",
      optional(seq(
        $.expression, repeat(seq(",", $.expression))
      )),
      "]"
    ),

    array_pattern: $ => seq(
      "[",
      optional(seq(
        $.pattern, repeat(seq(",", $.pattern))
      )),
      "]"
    ),

    pattern: $ => choice(
      $.identifier,
      $.array_pattern,
    ),
  },
})
```

In such cases, we want the parser to explore both possibilities by explicitly declaring this ambiguity:

```js
{
  name: "javascript",

  conflicts: $ => [
    [$.array, $.array_pattern],
  ],

  rules: {
    // ...
  },
}
```

```admonish note
The example is a bit contrived for the purpose of illustrating the usage of conflicts. The actual JavaScript grammar isn't
structured like that, but this conflict is actually present in the
[Tree-sitter JavaScript grammar](https://github.com/tree-sitter/tree-sitter-javascript/blob/108b2d4d17a04356a340aea809e4dd5b801eb40d/grammar.js#L100).
```

## Hiding Rules

You may have noticed in the above examples that some grammar rule name like `_expression` and `_type` began with an underscore.
Starting a rule's name with an underscore causes the rule to be _hidden_ in the syntax tree. This is useful for rules like
`_expression` in the grammars above, which always just wrap a single child node. If these nodes were not hidden, they would
add substantial depth and noise to the syntax tree without making it any easier to understand.

## Using Fields

Often, it's easier to analyze a syntax node if you can refer to its children by _name_ instead of by their position in an
ordered list. Tree-sitter grammars support this using the `field` function. This function allows you to assign unique names
to some or all of a node's children:

```js
function_definition: $ =>
  seq(
    "func",
    field("name", $.identifier),
    field("parameters", $.parameter_list),
    field("return_type", $._type),
    field("body", $.block),
  );
```

Adding fields like this allows you to retrieve nodes using the [field APIs][field-names-section].

# Lexical Analysis

Tree-sitter's parsing process is divided into two phases: parsing (which is described above) and [lexing][lexing] — the
process of grouping individual characters into the language's fundamental _tokens_. There are a few important things to
know about how Tree-sitter's lexing works.

## Conflicting Tokens

Grammars often contain multiple tokens that can match the same characters. For example, a grammar might contain the tokens
(`"if"` and `/[a-z]+/`). Tree-sitter differentiates between these conflicting tokens in a few ways.

1. **Context-aware Lexing** — Tree-sitter performs lexing on-demand, during the parsing process. At any given position
in a source document, the lexer only tries to recognize tokens that are _valid_ at that position in the document.

2. **Lexical Precedence** — When the precedence functions described [in the previous page][grammar dsl] are used _within_
the `token` function, the given explicit precedence values serve as instructions to the lexer. If there are two valid tokens
that match the characters at a given position in the document, Tree-sitter will select the one with the higher precedence.

3. **Match Length** — If multiple valid tokens with the same precedence match the characters at a given position in a document,
Tree-sitter will select the token that matches the [longest sequence of characters][longest-match].

4. **Match Specificity** — If there are two valid tokens with the same precedence, and they both match the same number
of characters, Tree-sitter will prefer a token that is specified in the grammar as a `String` over a token specified as
a `RegExp`.

5. **Rule Order** — If none of the above criteria can be used to select one token over another, Tree-sitter will prefer
the token that appears earlier in the grammar.

If there is an external scanner it may have [an additional impact][external scanner] over regular tokens
defined in the grammar.

## Lexical Precedence vs. Parse Precedence

One common mistake involves not distinguishing _lexical precedence_ from _parse precedence_. Parse precedence determines
which rule is chosen to interpret a given sequence of tokens. _Lexical precedence_ determines which token is chosen to interpret
at a given position of text, and it is a lower-level operation that is done first. The above list fully captures Tree-sitter's
lexical precedence rules, and you will probably refer back to this section of the documentation more often than any other.
Most of the time when you really get stuck, you're dealing with a lexical precedence problem. Pay particular attention to
the difference in meaning between using `prec` inside the `token` function versus outside it. The _lexical precedence_ syntax,
as mentioned in the previous page, is `token(prec(N, ...))`.

## Keywords

Many languages have a set of _keyword_ tokens (e.g. `if`, `for`, `return`), as well as a more general token (e.g. `identifier`)
that matches any word, including many of the keyword strings. For example, JavaScript has a keyword `instanceof`, which is
used as a binary operator, like this:

```js
if (a instanceof Something) b();
```

The following, however, is not valid JavaScript:

```js
if (a instanceofSomething) b();
```

A keyword like `instanceof` cannot be followed immediately by another letter, because then it would be tokenized as an `identifier`,
**even though an identifier is not valid at that position**. Because Tree-sitter uses context-aware lexing, as described
[above](#conflicting-tokens), it would not normally impose this restriction. By default, Tree-sitter would recognize `instanceofSomething`
as two separate tokens: the `instanceof` keyword followed by an `identifier`.

## Keyword Extraction

Fortunately, Tree-sitter has a feature that allows you to fix this, so that you can match the behavior of other standard
parsers: the `word` token. If you specify a `word` token in your grammar, Tree-sitter will find the set of _keyword_ tokens
that match strings also matched by the `word` token. Then, during lexing, instead of matching each of these keywords individually,
Tree-sitter will match the keywords via a two-step process where it _first_ matches the `word` token.

For example, suppose we added `identifier` as the `word` token in our JavaScript grammar:

```js
grammar({
  name: "javascript",

  word: $ => $.identifier,

  rules: {
    _expression: $ =>
      choice(
        $.identifier,
        $.unary_expression,
        $.binary_expression,
        // ...
      ),

    binary_expression: $ =>
      choice(
        prec.left(1, seq($._expression, "instanceof", $._expression)),
        // ...
      ),

    unary_expression: $ =>
      choice(
        prec.left(2, seq("typeof", $._expression)),
        // ...
      ),

    identifier: $ => /[a-z_]+/,
  },
});
```

Tree-sitter would identify `typeof` and `instanceof` as keywords. Then, when parsing the invalid code above, rather than
scanning for the `instanceof` token individually, it would scan for an `identifier` first, and find `instanceofSomething`.
It would then correctly recognize the code as invalid.

Aside from improving error detection, keyword extraction also has performance benefits. It allows Tree-sitter to generate
a smaller, simpler lexing function, which means that **the parser will compile much more quickly**.

```admonish note
The word token must be a unique token that is not reused by another rule. If you want to have a word token used in a
rule that's called something else, you should just alias the word token instead, like how the Rust grammar does it
<a href="https://github.com/tree-sitter/tree-sitter-rust/blob/1f63b33efee17e833e0ea29266dd3d713e27e321/grammar.js#L1605">here</a>
```

[ambiguous-grammar]: https://en.wikipedia.org/wiki/Ambiguous_grammar
[antlr]: https://www.antlr.org
[bison]: https://en.wikipedia.org/wiki/GNU_bison
[cst]: https://en.wikipedia.org/wiki/Parse_tree
[ecmascript-spec]: https://262.ecma-international.org/6.0/
[external scanner]: ./4-external-scanners.md#other-external-scanner-details
[glr-parsing]: https://en.wikipedia.org/wiki/GLR_parser
[grammar dsl]: ./2-the-grammar-dsl.md
[language-spec]: https://en.wikipedia.org/wiki/Programming_language_specification
[lexing]: https://en.wikipedia.org/wiki/Lexical_analysis
[longest-match]: https://en.wikipedia.org/wiki/Maximal_munch
[lr-grammars]: https://en.wikipedia.org/wiki/LR_parser
[field-names-section]: ../using-parsers/2-basic-parsing.md#node-field-names
[non-terminal]: https://en.wikipedia.org/wiki/Terminal_and_nonterminal_symbols
[peg]: https://en.wikipedia.org/wiki/Parsing_expression_grammar
[tree-sitter-javascript]: https://github.com/tree-sitter/tree-sitter-javascript
[yacc]: https://en.wikipedia.org/wiki/Yacc
