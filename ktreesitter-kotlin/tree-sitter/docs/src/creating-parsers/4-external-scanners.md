# External Scanners

Many languages have some tokens whose structure is impossible or inconvenient to describe with a regular expression.
Some examples:

- [Indent and dedent][indent-tokens] tokens in Python
- [Heredocs][heredoc] in Bash and Ruby
- [Percent strings][percent-string] in Ruby

Tree-sitter allows you to handle these kinds of tokens using _external scanners_. An external scanner is a set of C functions
that you, the grammar author, can write by hand to add custom logic for recognizing certain tokens.

To use an external scanner, there are a few steps. First, add an `externals` section to your grammar. This section should
list the names of all of your external tokens. These names can then be used elsewhere in your grammar.

```js
grammar({
  name: "my_language",

  externals: $ => [$.indent, $.dedent, $.newline],

  // ...
});
```

Then, add another C source file to your project. Its path must be src/scanner.c for the CLI to recognize it. Be sure to add
this file to the sources section of your `binding.gyp` file so that it will be included when your project is compiled by
Node.js and uncomment the appropriate block in your bindings/rust/build.rs file so that it will be included in your Rust
crate.

In this new source file, define an [`enum`][enum] type containing the names of all of your external tokens. The ordering
of this enum must match the order in your grammar's `externals` array; the actual names do not matter.

```c
#include "tree_sitter/parser.h"
#include "tree_sitter/alloc.h"
#include "tree_sitter/array.h"

enum TokenType {
  INDENT,
  DEDENT,
  NEWLINE
}
```

Finally, you must define five functions with specific names, based on your language's name and five actions:
_create_, _destroy_, _serialize_, _deserialize_, and _scan_.

## Create

```c
void * tree_sitter_my_language_external_scanner_create() {
  // ...
}
```

This function should create your scanner object. It will only be called once anytime your language is set on a parser.
Often, you will want to allocate memory on the heap and return a pointer to it. If your external scanner doesn't need to
maintain any state, it's ok to return `NULL`.

## Destroy

```c
void tree_sitter_my_language_external_scanner_destroy(void *payload) {
  // ...
}
```

This function should free any memory used by your scanner. It is called once when a parser is deleted or assigned a different
language. It receives as an argument the same pointer that was returned from the _create_ function. If your _create_ function
didn't allocate any memory, this function can be a no-op.

## Serialize

```c
unsigned tree_sitter_my_language_external_scanner_serialize(
  void *payload,
  char *buffer
) {
  // ...
}
```

This function should copy the complete state of your scanner into a given byte buffer, and return the number of bytes written.
The function is called every time the external scanner successfully recognizes a token. It receives a pointer to your scanner
and a pointer to a buffer. The maximum number of bytes that you can write is given by the `TREE_SITTER_SERIALIZATION_BUFFER_SIZE`
constant, defined in the `tree_sitter/parser.h` header file.

The data that this function writes will ultimately be stored in the syntax tree so that the scanner can be restored to the
right state when handling edits or ambiguities. For your parser to work correctly, the `serialize` function must store its
entire state, and `deserialize` must restore the entire state. For good performance, you should design your scanner so that
its state can be serialized as quickly and compactly as possible.

## Deserialize

```c
void tree_sitter_my_language_external_scanner_deserialize(
  void *payload,
  const char *buffer,
  unsigned length
) {
  // ...
}
```

This function should _restore_ the state of your scanner based the bytes that were previously written by the `serialize`
function. It is called with a pointer to your scanner, a pointer to the buffer of bytes, and the number of bytes that should
be read. It is good practice to explicitly erase your scanner state variables at the start of this function, before restoring
their values from the byte buffer.

## Scan

Typically, one will

- Call `lexer->advance` several times, if the characters are valid for the token being lexed.

- Optionally, call `lexer->mark_end` to mark the end of the token, and "peek ahead"
to check if the next character (or set of characters) invalidates the token.

- Set `lexer->result_symbol` to the token type.

- Return `true` from the scanning function, indicating that a token was successfully lexed.

Tree-sitter will then push resulting node to the parse stack, and the input position will remain where it reached at the
point `lexer->mark_end` was called.

```c
bool tree_sitter_my_language_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  // ...
}
```

The second parameter to this function is the lexer, of type `TSLexer`. The `TSLexer` struct has the following fields:

- **`int32_t lookahead`** — The current next character in the input stream, represented as a 32-bit unicode code point.

- **`TSSymbol result_symbol`** — The symbol that was recognized. Your scan function should _assign_ to this field one of
the values from the `TokenType` enum, described above.

- **`void (*advance)(TSLexer *, bool skip)`** — A function for advancing to the next character. If you pass `true` for
the second argument, the current character will be treated as whitespace; whitespace won't be included in the text range
associated with tokens emitted by the external scanner.

- **`void (*mark_end)(TSLexer *)`** — A function for marking the end of the recognized token. This allows matching tokens
that require multiple characters of lookahead. By default, (if you don't call `mark_end`), any character that you moved past
using the `advance` function will be included in the size of the token. But once you call `mark_end`, then any later calls
to `advance` will _not_ increase the size of the returned token. You can call `mark_end` multiple times to increase the size
of the token.

- **`uint32_t (*get_column)(TSLexer *)`** — A function for querying the current column position of the lexer. It returns
the number of codepoints since the start of the current line. The codepoint position is recalculated on every call to this
function by reading from the start of the line.

- **`bool (*is_at_included_range_start)(const TSLexer *)`** — A function for checking whether the parser has just skipped
some characters in the document. When parsing an embedded document using the `ts_parser_set_included_ranges` function
(described in the [multi-language document section][multi-language-section]), the scanner may want to apply some special
behavior when moving to a disjoint part of the document. For example, in [EJS documents][ejs], the JavaScript parser uses
this function to enable inserting automatic semicolon tokens in between the code directives, delimited by `<%` and `%>`.

- **`bool (*eof)(const TSLexer *)`** — A function for determining whether the lexer is at the end of the file. The value
of `lookahead` will be `0` at the end of a file, but this function should be used instead of checking for that value because
the `0` or "NUL" value is also a valid character that could be present in the file being parsed.

The third argument to the `scan` function is an array of booleans that indicates which of external tokens are expected by
the parser. You should only look for a given token if it is valid according to this array. At the same time, you cannot
backtrack, so you may need to combine certain pieces of logic.

```c
if (valid_symbols[INDENT] || valid_symbols[DEDENT]) {

  // ... logic that is common to both `INDENT` and `DEDENT`

  if (valid_symbols[INDENT]) {

    // ... logic that is specific to `INDENT`

    lexer->result_symbol = INDENT;
    return true;
  }
}
```

## External Scanner Helpers

### Allocator

Instead of using libc's `malloc`, `calloc`, `realloc`, and `free`, you should use the versions prefixed with `ts_` from `tree_sitter/alloc.h`.
These macros can allow a potential consumer to override the default allocator with their own implementation, but by default
will use the libc functions.

As a consumer of the tree-sitter core library as well as any parser libraries that might use allocations, you can enable
overriding the default allocator and have it use the same one as the library allocator, of which you can set with `ts_set_allocator`.
To enable this overriding in scanners, you must compile them with the `TREE_SITTER_REUSE_ALLOCATOR` macro defined, and tree-sitter
the library must be linked into your final app dynamically, since it needs to resolve the internal functions at runtime.
If you are compiling an executable binary that uses the core library, but want to load parsers dynamically at runtime, then
you will have to use a special linker flag on Unix. For non-Darwin systems, that would be `--dynamic-list` and for Darwin
systems, that would be `-exported_symbols_list`. The CLI does exactly this, so you can use it as a reference (check out `cli/build.rs`).

For example, assuming you wanted to allocate 100 bytes for your scanner, you'd do so like the following example:

```c
#include "tree_sitter/parser.h"
#include "tree_sitter/alloc.h"

// ...

void* tree_sitter_my_language_external_scanner_create() {
  return ts_calloc(100, 1); // or ts_malloc(100)
}

// ...

```

### Arrays

If you need to use array-like types in your scanner, such as tracking a stack of indentations or tags, you should use the
array macros from `tree_sitter/array.h`.

There are quite a few of them provided for you, but here's how you could get started tracking some . Check out the header
itself for more detailed documentation.

```admonish attention
Do not use any of the array functions or macros that are prefixed with an underscore and have comments saying
that it is not what you are looking for. These are internal functions used as helpers by other macros that are public.
They are not meant to be used directly, nor are they what you want.
```

```c
#include "tree_sitter/parser.h"
#include "tree_sitter/array.h"

enum TokenType {
  INDENT,
  DEDENT,
  NEWLINE,
  STRING,
}

// Create the array in your create function

void* tree_sitter_my_language_external_scanner_create() {
  return ts_calloc(1, sizeof(Array(int)));

  // or if you want to zero out the memory yourself

  Array(int) *stack = ts_malloc(sizeof(Array(int)));
  array_init(&stack);
  return stack;
}

bool tree_sitter_my_language_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  Array(int) *stack = payload;
  if (valid_symbols[INDENT]) {
    array_push(stack, lexer->get_column(lexer));
    lexer->result_symbol = INDENT;
    return true;
  }
  if (valid_symbols[DEDENT]) {
    array_pop(stack); // this returns the popped element by value, but we don't need it
    lexer->result_symbol = DEDENT;
    return true;
  }

  // we can also use an array on the stack to keep track of a string

  Array(char) next_string = array_new();

  if (valid_symbols[STRING] && lexer->lookahead == '"') {
    lexer->advance(lexer, false);
    while (lexer->lookahead != '"' && lexer->lookahead != '\n' && !lexer->eof(lexer)) {
      array_push(&next_string, lexer->lookahead);
      lexer->advance(lexer, false);
    }

    // assume we have some arbitrary constraint of not having more than 100 characters in a string
    if (lexer->lookahead == '"' && next_string.size <= 100) {
      lexer->advance(lexer, false);
      lexer->result_symbol = STRING;
      return true;
    }
  }

  return false;
}

```

## Other External Scanner Details

External scanners have priority over Tree-sitter's normal lexing process. When a token listed in the externals array is valid
at a given position, the external scanner is called first. This makes external scanners a powerful way to override Tree-sitter's
default lexing behavior, especially for cases that can't be handled with regular lexical rules, parsing, or dynamic precedence.

During error recovery, Tree-sitter's first step is to call the external scanner's scan function with all tokens marked as
valid. Your scanner should detect and handle this case appropriately. One simple approach is to add an unused "sentinel"
token at the end of your externals array:

```js
{
  name: "my_language",

  externals: $ => [$.token1, $.token2, $.error_sentinel]

  // ...
}
```

You can then check if this sentinel token is marked valid to determine if Tree-sitter is in error recovery mode.

If you would rather not handle the error recovery case explicitly, the easiest way to "opt-out" and let tree-sitter's internal
lexer handle it is to return `false` from your scan function when `valid_symbols` contains the error sentinel.

```c
bool tree_sitter_my_language_external_scanner_scan(
  void *payload,
  TSLexer *lexer,
  const bool *valid_symbols
) {
  if (valid_symbols[ERROR_SENTINEL]) {
    return false;
  }
  // ...
}
```

When you include literal keywords in the externals array, for example:

```js
externals: $ => ['if', 'then', 'else']
```

_those_ keywords will
be tokenized by the external scanner whenever they appear in the grammar.

This is equivalent to declaring named tokens and aliasing them:

```js
{
  name: "my_language",

  externals: $ => [$.if_keyword, $.then_keyword, $.else_keyword],

  rules: {

    // then using it in a rule like so:
    if_statement: $ => seq(alias($.if_keyword, 'if'), ...),

    // ...
  }
}
```

The tokenization process for external keywords works in two stages:

1. The external scanner attempts to recognize the token first
2. If the scanner returns true and sets a token, that token is used
3. If the scanner returns false, Tree-sitter falls back to its internal lexer

However, when you use rule references (like `$.if_keyword`) in the externals array without defining the corresponding rules
in the grammar, Tree-sitter cannot fall back to its internal lexer. In this case, the external scanner is solely responsible
for recognizing these tokens.

```admonish danger
- External scanners can easily create infinite loops

- Be extremely careful when emitting zero-width tokens

- Always use the `eof` function when looping through characters
```

[ejs]: https://ejs.co
[enum]: https://en.wikipedia.org/wiki/Enumerated_type#C
[heredoc]: https://en.wikipedia.org/wiki/Here_document
[indent-tokens]: https://en.wikipedia.org/wiki/Off-side_rule
[multi-language-section]: ../using-parsers/3-advanced-parsing.md#multi-language-documents
[percent-string]: https://docs.ruby-lang.org/en/2.5.0/doc/syntax/literals_rdoc.html#label-Percent+Strings
