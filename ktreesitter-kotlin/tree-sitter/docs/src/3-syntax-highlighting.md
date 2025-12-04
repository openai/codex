# Syntax Highlighting

Syntax highlighting is a very common feature in applications that deal with code. Tree-sitter has built-in support for
syntax highlighting via the [`tree-sitter-highlight`][highlight crate] library, which is now used on GitHub.com for highlighting
code written in several languages. You can also perform syntax highlighting at the command line using the
`tree-sitter highlight` command.

This document explains how the Tree-sitter syntax highlighting system works, using the command line interface. If you are
using `tree-sitter-highlight` library (either from C or from Rust), all of these concepts are still applicable, but the
configuration data is provided using in-memory objects, rather than files.

## Overview

All the files needed to highlight a given language are normally included in the same git repository as the Tree-sitter
grammar for that language (for example, [`tree-sitter-javascript`][js grammar], [`tree-sitter-ruby`][ruby grammar]).
To run syntax highlighting from the command-line, three types of files are needed:

1. Per-user configuration in `~/.config/tree-sitter/config.json` (see the [init-config][init-config] page for more info).
2. Language configuration in grammar repositories' `tree-sitter.json` files (see the [init][init] page for more info).
3. Tree queries in the grammars repositories' `queries` folders.

For an example of the language-specific files, see the [`tree-sitter.json` file][ts json] and [`queries` directory][queries]
in the `tree-sitter-ruby` repository. The following sections describe the behavior of each file.

## Language Configuration

The `tree-sitter.json` file is used by the Tree-sitter CLI. Within this file, the CLI looks for data nested under the
top-level `"grammars"` key. This key is expected to contain an array of objects with the following keys:

### Basics

These keys specify basic information about the parser:

- `scope` (required) — A string like `"source.js"` that identifies the language. We strive to match the scope names used
by popular [TextMate grammars][textmate] and by the [Linguist][linguist] library.

- `path` (optional) — A relative path from the directory containing `tree-sitter.json` to another directory containing
the `src/` folder, which contains the actual generated parser. The default value is `"."` (so that `src/` is in the same
folder as `tree-sitter.json`), and this very rarely needs to be overridden.

- `external-files` (optional) — A list of relative paths from the root dir of a
parser to files that should be checked for modifications during recompilation.
This is useful during development to have changes to other files besides scanner.c
be picked up by the cli.

### Language Detection

These keys help to decide whether the language applies to a given file:

- `file-types` — An array of filename suffix strings. The grammar will be used for files whose names end with one of these
suffixes. Note that the suffix may match an *entire* filename.

- `first-line-regex` — A regex pattern that will be tested against the first line of a file to determine whether this language
applies to the file. If present, this regex will be used for any file whose language does not match any grammar's `file-types`.

- `content-regex` — A regex pattern that will be tested against the contents of the file to break ties in cases where
multiple grammars matched the file using the above two criteria. If the regex matches, this grammar will be preferred over
another grammar with no `content-regex`. If the regex does not match, a grammar with no `content-regex` will be preferred
over this one.

- `injection-regex` — A regex pattern that will be tested against a *language name* ito determine whether this language
should be used for a potential *language injection* site. Language injection is described in more detail in [a later section](#language-injection).

### Query Paths

These keys specify relative paths from the directory containing `tree-sitter.json` to the files that control syntax highlighting:

- `highlights` — Path to a *highlight query*. Default: `queries/highlights.scm`
- `locals` — Path to a *local variable query*. Default: `queries/locals.scm`.
- `injections` — Path to an *injection query*. Default: `queries/injections.scm`.

The behaviors of these three files are described in the next section.

## Queries

Tree-sitter's syntax highlighting system is based on *tree queries*, which are a general system for pattern-matching on Tree-sitter's
syntax trees. See [this section][pattern matching] of the documentation for more information
about tree queries.

Syntax highlighting is controlled by *three* different types of query files that are usually included in the `queries` folder.
The default names for the query files use the `.scm` file. We chose this extension because it commonly used for files written
in [Scheme][scheme], a popular dialect of Lisp, and these query files use a Lisp-like syntax.

### Highlights

The most important query is called the highlights query. The highlights query uses *captures* to assign arbitrary
*highlight names* to different nodes in the tree. Each highlight name can then be mapped to a color
(as described in the [init-config command][theme]). Commonly used highlight names include
`keyword`, `function`, `type`, `property`, and `string`. Names can also be dot-separated like `function.builtin`.

#### Example Go Snippet

For example, consider the following Go code:

```go
func increment(a int) int {
    return a + 1
}
```

With this syntax tree:

```scheme
(source_file
  (function_declaration
    name: (identifier)
    parameters: (parameter_list
      (parameter_declaration
        name: (identifier)
        type: (type_identifier)))
    result: (type_identifier)
    body: (block
      (return_statement
        (expression_list
          (binary_expression
            left: (identifier)
            right: (int_literal)))))))
```

#### Example Query

Suppose we wanted to render this code with the following colors:

- keywords `func` and `return` in purple
- function `increment` in blue
- type `int` in green
- number `5` brown

We can assign each of these categories a *highlight name* using a query like this:

```scheme
; highlights.scm

"func" @keyword
"return" @keyword
(type_identifier) @type
(int_literal) @number
(function_declaration name: (identifier) @function)
```

Then, in our config file, we could map each of these highlight names to a color:

```json
{
  "theme": {
    "keyword": "purple",
    "function": "blue",
    "type": "green",
    "number": "brown"
  }
}
```

#### Highlights Result

Running `tree-sitter highlight` on this Go file would produce output like this:

```admonish example collapsible=true, title='Output'
<pre class='highlight'>
<span style='color: purple;'>func</span> <span style='color: #005fd7;'>increment</span>(<span>a</span> <span style='color: green;'>int</span>) <span style='color: green;'>int</span> {
    <span style='color: purple;'>return</span> <span>a</span> <span style='font-weight: bold;color: #4e4e4e;'>+</span> <span style='font-weight: bold;color: #875f00;'>1</span>
}
</pre>
```

### Local Variables

Good syntax highlighting helps the reader to quickly distinguish between the different types of *entities* in their code.
Ideally, if a given entity appears in *multiple* places, it should be colored the same in each place. The Tree-sitter syntax
highlighting system can help you to achieve this by keeping track of local scopes and variables.

The *local variables* query is different from the highlights query in that, while the highlights query uses *arbitrary*
capture names, which can then be mapped to colors, the locals variable query uses a fixed set of capture names, each of
which has a special meaning.

The capture names are as follows:

- `@local.scope` — indicates that a syntax node introduces a new local scope.
- `@local.definition` — indicates that a syntax node contains the *name* of a definition within the current local scope.
- `@local.reference` — indicates that a syntax node contains the *name*, which *may* refer to an earlier definition within
some enclosing scope.

When highlighting a file, Tree-sitter will keep track of the set of scopes that contains any given position, and the set
of definitions within each scope. When processing a syntax node that is captured as a `local.reference`, Tree-sitter will
try to find a definition for a name that matches the node's text. If it finds a match, Tree-sitter will ensure that the
*reference*, and the *definition* are colored the same.

The information produced by this query can also be *used* by the highlights query. You can *disable* a pattern for nodes,
which have been identified as local variables by adding the predicate `(#is-not? local)` to the pattern. This is used in
the example below:

#### Example Ruby Snippet

Consider this Ruby code:

```ruby
def process_list(list)
  context = current_context
  list.map do |item|
    process_item(item, context)
  end
end

item = 5
list = [item]
```

With this syntax tree:

```scheme
(program
  (method
    name: (identifier)
    parameters: (method_parameters
      (identifier))
    (assignment
      left: (identifier)
      right: (identifier))
    (method_call
      method: (call
        receiver: (identifier)
        method: (identifier))
      block: (do_block
        (block_parameters
          (identifier))
        (method_call
          method: (identifier)
          arguments: (argument_list
            (identifier)
            (identifier))))))
  (assignment
    left: (identifier)
    right: (integer))
  (assignment
    left: (identifier)
    right: (array
      (identifier))))
```

There are several types of names within this method:

- `process_list` is a method.
- Within this method, `list` is a formal parameter
- `context` is a local variable.
- `current_context` is *not* a local variable, so it must be a method.
- Within the `do` block, `item` is a formal parameter
- Later on, `item` and `list` are both local variables (not formal parameters).

#### Example Queries

Let's write some queries that let us clearly distinguish between these types of names. First, set up the highlighting query,
as described in the previous section. We'll assign distinct colors to method calls, method definitions, and formal parameters:

```scheme
; highlights.scm

(call method: (identifier) @function.method)
(method_call method: (identifier) @function.method)

(method name: (identifier) @function.method)

(method_parameters (identifier) @variable.parameter)
(block_parameters (identifier) @variable.parameter)

((identifier) @function.method
 (#is-not? local))
```

Then, we'll set up a local variable query to keep track of the variables and scopes. Here, we're indicating that methods
and blocks create local *scopes*, parameters and assignments create *definitions*, and other identifiers should be considered
*references*:

```scheme
; locals.scm

(method) @local.scope
(do_block) @local.scope

(method_parameters (identifier) @local.definition)
(block_parameters (identifier) @local.definition)

(assignment left:(identifier) @local.definition)

(identifier) @local.reference
```

#### Locals Result

Running `tree-sitter highlight` on this ruby file would produce output like this:

```admonish example collapsible=true, title='Output'
<pre class='highlight'>
<span style='color: purple;'>def</span> <span style='color: #005fd7;'>process_list</span><span style='color: #4e4e4e;'>(</span><span style='text-decoration: underline;'>list</span><span style='color: #4e4e4e;'>)</span>
  <span>context</span> <span style='font-weight: bold;color: #4e4e4e;'>=</span> <span style='color: #005fd7;'>current_context</span>
  <span style='text-decoration: underline;'>list</span><span style='color: #4e4e4e;'>.</span><span style='color: #005fd7;'>map</span> <span style='color: purple;'>do</span> |<span style='text-decoration: underline;'>item</span>|
    <span style='color: #005fd7;'>process_item</span>(<span style='text-decoration: underline;'>item</span><span style='color: #4e4e4e;'>,</span> <span>context</span><span style='color: #4e4e4e;'>)</span>
  <span style='color: purple;'>end</span>
<span style='color: purple;'>end</span>

<span>item</span> <span style='font-weight: bold;color: #4e4e4e;'>=</span> <span style='font-weight: bold;color: #875f00;'>5</span>
<span>list</span> <span style='font-weight: bold;color: #4e4e4e;'>=</span> [<span>item</span><span style='color: #4e4e4e;'>]</span>
</pre>
```

### Language Injection

Some source files contain code written in multiple different languages. Examples include:

- HTML files, which can contain JavaScript inside `<script>` tags and CSS inside `<style>` tags
- [ERB][erb] files, which contain Ruby inside `<% %>` tags, and HTML outside those tags
- PHP files, which can contain  HTML between the `<php` tags
- JavaScript files, which contain regular expression syntax within regex literals
- Ruby, which can contain snippets of code inside heredoc literals, where the heredoc delimiter often indicates the language

All of these examples can be modeled in terms a *parent* syntax tree and one or more *injected* syntax trees, which reside
*inside* of certain nodes in the parent tree. The language injection query allows you to specify these "injections" using
the following captures:

- `@injection.content` — indicates that the captured node should have its contents re-parsed using another language.
- `@injection.language` — indicates that the captured node's text may contain the *name* of a language that should be used
to re-parse the `@injection.content`.

The language injection behavior can also be configured by some properties associated with patterns:

- `injection.language` — can be used to hard-code the name of a specific language.
- `injection.combined` — indicates that *all* the matching nodes in the tree
  should have their content parsed as *one* nested document.
- `injection.include-children` — indicates that the `@injection.content` node's
  *entire* text should be re-parsed, including the text of its child nodes. By default,
child nodes' text will be *excluded* from the injected document.
- `injection.self` — indicates that the `@injection.content` node should be parsed
  using the same language as the node itself. This is useful for cases where the
  node's language is not known until runtime (e.g. via inheriting another language)
- `injection.parent` indicates that the `@injection.content` node should be parsed
  using the same language as the node's parent language. This is only meant for injections
  that need to refer back to the parent language to parse the node's text inside
  the injected language.

#### Examples

Consider this ruby code:

```ruby
system <<-BASH.strip!
  abc --def | ghi > jkl
BASH
```

With this syntax tree:

```scheme
(program
  (method_call
    method: (identifier)
    arguments: (argument_list
      (call
        receiver: (heredoc_beginning)
        method: (identifier))))
  (heredoc_body
    (heredoc_end)))
```

The following query would specify that the contents of the heredoc should be parsed using a language named "BASH"
(because that is the text of the `heredoc_end` node):

```scheme
(heredoc_body
  (heredoc_end) @injection.language) @injection.content
```

You can also force the language using the `#set!` predicate.
For example, this will force the language to be always `ruby`.

```scheme
((heredoc_body) @injection.content
 (#set! injection.language "ruby"))
```

## Unit Testing

Tree-sitter has a built-in way to verify the results of syntax highlighting. The interface is based on [Sublime Text's system][sublime]
for testing highlighting.

Tests are written as normal source code files that contain specially-formatted *comments* that make assertions about the
surrounding syntax highlighting. These files are stored in the `test/highlight` directory in a grammar repository.

Here is an example of a syntax highlighting test for JavaScript:

```js
var abc = function(d) {
  // <- keyword
  //          ^ keyword
  //               ^ variable.parameter
  // ^ function

  if (a) {
  // <- keyword
  // ^ punctuation.bracket

    foo(`foo ${bar}`);
    // <- function
    //    ^ string
    //          ^ variable
  }

  baz();
  // <- !variable
};
```

```admonish cite title='From the Sublime text docs'
The two types of tests are:

**Caret**: ^ this will test the following selector against the scope on the most recent non-test line. It will test it
at the same column the ^ is in. Consecutive ^s will test each column against the selector.

**Arrow**: <- this will test the following selector against the scope on the most recent non-test line. It will test it
at the same column as the comment character is in.
```
```admonish note
An exclamation mark (`!`) can be used to negate a selector. For example, `!keyword` will match any scope that is
not the `keyword` class.
```

[erb]: https://en.wikipedia.org/wiki/ERuby
[highlight crate]: https://github.com/tree-sitter/tree-sitter/tree/master/highlight
[init-config]: ./cli/init-config.md
[init]: ./cli/init.md#structure-of-tree-sitterjson
[js grammar]: https://github.com/tree-sitter/tree-sitter-javascript
[linguist]: https://github.com/github/linguist
[pattern matching]: ./using-parsers/queries/index.md
[queries]: https://github.com/tree-sitter/tree-sitter-ruby/tree/master/queries
[ruby grammar]: https://github.com/tree-sitter/tree-sitter-ruby
[scheme]: https://en.wikipedia.org/wiki/Scheme_%28programming_language%29
[sublime]: https://www.sublimetext.com/docs/3/syntax.html#testing
[textmate]: https://macromates.com/manual/en/language_grammars
[theme]: ./cli/init-config.md#theme
[ts json]: https://github.com/tree-sitter/tree-sitter-ruby/blob/master/tree-sitter.json
