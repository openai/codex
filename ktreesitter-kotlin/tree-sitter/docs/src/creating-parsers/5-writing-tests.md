# Writing Tests

For each rule that you add to the grammar, you should first create a *test* that describes how the syntax trees should look
when parsing that rule. These tests are written using specially-formatted text files in the `test/corpus/` directory within
your parser's root folder.

For example, you might have a file called `test/corpus/statements.txt` that contains a series of entries like this:

```text
==================
Return statements
==================

func x() int {
  return 1;
}

---

(source_file
  (function_definition
    (identifier)
    (parameter_list)
    (primitive_type)
    (block
      (return_statement (number)))))
```

* The **name** of each test is written between two lines containing only `=` (equal sign) characters.

* Then the **input source code** is written, followed by a line containing three or more `-` (dash) characters.

* Then, the **expected output syntax tree** is written as an [S-expression][s-exp]. The exact placement of whitespace in
the S-expression doesn't matter, but ideally the syntax tree should be legible.

```admonish tip
The S-expression does not show syntax nodes like `func`, `(` and `;`, which are expressed as strings and regexes in the grammar.
It only shows the *named* nodes, as described in [this section][named-vs-anonymous-nodes] of the page on parser usage.
```

  The expected output section can also *optionally* show the [*field names*][node-field-names] associated with each child
  node. To include field names in your tests, you write a node's field name followed by a colon, before the node itself in
  the S-expression:

```query
(source_file
  (function_definition
    name: (identifier)
    parameters: (parameter_list)
    result: (primitive_type)
    body: (block
      (return_statement (number)))))
```

* If your language's syntax conflicts with the `===` and `---` test separators, you can optionally add an arbitrary identical
suffix (in the below example, `|||`) to disambiguate them:

```text
==================|||
Basic module
==================|||

---- MODULE Test ----
increment(n) == n + 1
====

---|||

(source_file
  (module (identifier)
    (operator (identifier)
      (parameter_list (identifier))
      (plus (identifier_ref) (number)))))
```

These tests are important. They serve as the parser's API documentation, and they can be run every time you change the grammar
to verify that everything still parses correctly.

By default, the `tree-sitter test` command runs all the tests in your `test/corpus/` folder. To run a particular test, you
can use the `-f` flag:

```sh
tree-sitter test -f 'Return statements'
```

The recommendation is to be comprehensive in adding tests. If it's a visible node, add it to a test file in your `test/corpus`
directory. It's typically a good idea to test all the permutations of each language construct. This increases test coverage,
but doubly acquaints readers with a way to examine expected outputs and understand the "edges" of a language.

## Attributes

Tests can be annotated with a few `attributes`. Attributes must be put in the header, below the test name, and start with
a `:`. A couple of attributes also take in a parameter, which require the use of parenthesis.

```admonish tip
If you'd like to supply in multiple parameters, e.g. to run tests on multiple platforms or to test multiple languages,
you can repeat the attribute on a new line.
```

The following attributes are available:

* `:skip` — This attribute will skip the test when running `tree-sitter test`.
  This is useful when you want to temporarily disable running a test without deleting it.
* `:error` — This attribute will assert that the parse tree contains an error. It's useful to just validate that a certain
input is invalid without displaying the whole parse tree, as such you should omit the parse tree below the `---` line.
* `:fail-fast` — This attribute will stop the testing additional tests if the test marked with this attribute fails.
* `:language(LANG)` — This attribute will run the tests using the parser for the specified language. This is useful for
multi-parser repos, such as XML and DTD, or Typescript and TSX. The default parser used will always be the first entry in
the `grammars` field in the `tree-sitter.json` config file, so having a way to pick a second or even third parser is useful.
* `:platform(PLATFORM)` — This attribute specifies the platform on which the test should run. It is useful to test platform-specific
behavior (e.g. Windows newlines are different from Unix). This attribute must match up with Rust's [`std::env::consts::OS`][constants].

Examples using attributes:

```text
=========================
Test that will be skipped
:skip
=========================

int main() {}

-------------------------

====================================
Test that will run on Linux or macOS

:platform(linux)
:platform(macos)
====================================

int main() {}

------------------------------------

========================================================================
Test that expects an error, and will fail fast if there's no parse error
:fail-fast
:error
========================================================================

int main ( {}

------------------------------------------------------------------------

=================================================
Test that will parse with both Typescript and TSX
:language(typescript)
:language(tsx)
=================================================

console.log('Hello, world!');

-------------------------------------------------
```

### Automatic Compilation

You might notice that the first time you run `tree-sitter test` after regenerating your parser, it takes some extra time.
This is because Tree-sitter automatically compiles your C code into a dynamically-loadable library. It recompiles your parser
as-needed whenever you update it by re-running `tree-sitter generate`, or whenever the [external scanner][external-scanners]
file is changed.

[constants]: https://doc.rust-lang.org/std/env/consts/constant.OS.html
[external-scanners]: ./4-external-scanners.md
[node-field-names]: ../using-parsers/2-basic-parsing.md#node-field-names
[s-exp]: https://en.wikipedia.org/wiki/S-expression
