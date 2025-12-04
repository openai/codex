# Predicates

You can also specify arbitrary metadata and conditions associated with a pattern
by adding _predicate_ S-expressions anywhere within your pattern. Predicate S-expressions
start with a _predicate name_ beginning with a `#` character, and ending with a `?` character. After that, they can
contain an arbitrary number of `@`-prefixed capture names or strings.

Tree-sitter's CLI supports the following predicates by default:

## The `eq?` predicate

This family of predicates allows you to match against a single capture or string
value.

The first argument to this predicate must be a capture, but the second can be either a capture to
compare the two captures' text, or a string to compare first capture's text
against.

The base predicate is `#eq?`, but its complement, `#not-eq?`, can be used to _not_
match a value. Additionally, you can prefix either of these with `any-` to match
if _any_ of the nodes match the predicate. This is only useful when dealing with
quantified captures, as by default a quantified capture will only match if _all_ the captured nodes match the predicate.

Thus, there are four predicates in total:

- `#eq?`
- `#not-eq?`
- `#any-eq?`
- `#any-not-eq?`

Consider the following example targeting C:

```query
((identifier) @variable.builtin
  (#eq? @variable.builtin "self"))
```

This pattern would match any identifier that is `self`.

Now consider the following example:

```query
(
  (pair
    key: (property_identifier) @key-name
    value: (identifier) @value-name)
  (#eq? @key-name @value-name)
)
```

This pattern would match key-value pairs where the `value` is an identifier
with the same text as the key (meaning they are the same):

As mentioned earlier, the `any-` prefix is meant for use with quantified captures. Here's
an example finding an empty comment within a group of comments:

```query
((comment)+ @comment.empty
  (#any-eq? @comment.empty "//"))
```

## The `match?` predicate

These predicates are similar to the `eq?` predicates, but they use regular expressions
to match against the capture's text instead of string comparisons.

The first argument must be a capture, and the second must be a string containing
a regular expression.

Like the `eq?` predicate family, we can tack on `not-` to the beginning of the predicate
to negate the match, and `any-` to match if _any_ of the nodes in a quantified capture match the predicate.

This pattern matches identifiers written in `SCREAMING_SNAKE_CASE`.

```query
((identifier) @constant
  (#match? @constant "^[A-Z][A-Z_]+"))
```

This query identifies documentation comments in C that begin with three forward slashes (`///`).

```query
((comment)+ @comment.documentation
  (#match? @comment.documentation "^///\\s+.*"))
```

This query finds C code embedded in Go comments that appear just before a "C" import statement.
These are known as [`Cgo`][cgo] comments and are used to inject C code into Go programs.

```query
((comment)+ @injection.content
  .
  (import_declaration
    (import_spec path: (interpreted_string_literal) @_import_c))
  (#eq? @_import_c "\"C\"")
  (#match? @injection.content "^//"))
```

## The `any-of?` predicate

The `any-of?` predicate allows you to match a capture against multiple strings,
and will match if the capture's text is equal to any of the strings.

The query below will match any of the builtin variables in JavaScript.

```query
((identifier) @variable.builtin
  (#any-of? @variable.builtin
        "arguments"
        "module"
        "console"
        "window"
        "document"))
```

## The `is?` predicate

The `is?` predicate allows you to assert that a capture has a given property. This isn't widely used, but the CLI uses it
to determine whether a given node is a local variable or not, for example:

```query
((identifier) @variable.builtin
  (#match? @variable.builtin "^(arguments|module|console|window|document)$")
  (#is-not? local))
```

This pattern would match any builtin variable that is not a local variable, because the `#is-not? local` predicate is used.

# Directives

Similar to predicates, directives are a way to associate arbitrary metadata with a pattern. The only difference between predicates
and directives is that directives end in a `!` character instead of `?` character.

Tree-sitter's CLI supports the following directives by default:

## The `set!` directive

This directive allows you to associate key-value pairs with a pattern. The key and value can be any arbitrary text that you
see fit.

```query
((comment) @injection.content
  (#lua-match? @injection.content "/[*\/][!*\/]<?[^a-zA-Z]")
  (#set! injection.language "doxygen"))
```

This pattern would match any comment that contains a Doxygen-style comment, and then sets the `injection.language` key to
`"doxygen"`. Programmatically, when iterating the captures of this pattern, you can access this property to then parse the
comment with the Doxygen parser.

### The `#select-adjacent!` directive

The `#select-adjacent!` directive allows you to filter the text associated with a capture so that only nodes adjacent to
another capture are preserved. It takes two arguments, both of which are capture names.

### The `#strip!` directive

The `#strip!` directive allows you to remove text from a capture. It takes two arguments: the first is the capture to strip
text from, and the second is a regular expression to match against the text. Any text matched by the regular expression will
be removed from the text associated with the capture.

For an example on the `#select-adjacent!` and `#strip!` directives,
view the [code navigation](../../4-code-navigation.md#examples) documentation.

## Recap

To recap about the predicates and directives Tree-Sitter's bindings support:

- `#eq?` checks for a direct match against a capture or string

- `#match?` checks for a match against a regular expression

- `#any-of?` checks for a match against a list of strings

- `#is?` checks for a property on a capture

- Adding `not-` to the beginning of these predicates will negate the match

- By default, a quantified capture will only match if _all_ the nodes match the predicate

- Adding `any-` before the `eq` or `match` predicates will instead match if any of the nodes match the predicate

- `#set!` associates key-value pairs with a pattern

- `#select-adjacent!` filters the text associated with a capture so that only nodes adjacent to another capture are preserved

- `#strip!` removes text from a capture

```admonish info
Predicates and directives are not handled directly by the Tree-sitter C library.
They are just exposed in a structured form so that higher-level code can perform
the filtering. However, higher-level bindings to Tree-sitter like
[the Rust Crate][rust crate]
or the [WebAssembly binding][wasm binding]
do implement a few common predicates like those explained above. In the future, more "standard" predicates and directives
may be added.
```

[cgo]: https://pkg.go.dev/cmd/cgo
[rust crate]: https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_rust
[wasm binding]: https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_web
