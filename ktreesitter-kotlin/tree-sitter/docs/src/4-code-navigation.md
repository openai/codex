# Code Navigation Systems

Tree-sitter can be used in conjunction with its [query language][query language] as a part of code navigation systems.
An example of such a system can be seen in the `tree-sitter tags` command, which emits a textual dump of the interesting
syntactic nodes in its file argument. A notable application of this is GitHub's support for [search-based code navigation][gh search].
This document exists to describe how to integrate with such systems, and how to extend this functionality to any language with a Tree-sitter grammar.

## Tagging and captures

_Tagging_ is the act of identifying the entities that can be named in a program. We use Tree-sitter queries to find those
entities. Having found them, you use a syntax capture to label the entity and its name.

The essence of a given tag lies in two pieces of data: the _role_ of the entity that is matched
(i.e. whether it is a definition or a reference) and the _kind_ of that entity, which describes how the entity is used
(i.e. whether it's a class definition, function call, variable reference, and so on). Our convention is to use a syntax capture
following the `@role.kind` capture name format, and another inner capture, always called `@name`, that pulls out the name
of a given identifier.

You may optionally include a capture named `@doc` to bind a docstring. For convenience purposes, the tagging system provides
two built-in functions, `#select-adjacent!` and `#strip!` that are convenient for removing comment syntax from a docstring.
`#strip!` takes a capture as its first argument and a regular expression as its second, expressed as a quoted string.
Any text patterns matched by the regular expression will be removed from the text associated with the passed capture.
`#select-adjacent!`, when passed two capture names, filters the text associated with the first capture so that only nodes
adjacent to the second capture are preserved. This can be useful when writing queries that would otherwise include too much
information in matched comments.

## Examples

This [query][query] recognizes Python function definitions and captures their declared name. The `function_definition`
syntax node is defined in the [Python Tree-sitter grammar][node].

```query
(function_definition
  name: (identifier) @name) @definition.function
```

A more sophisticated query can be found in the [JavaScript Tree-sitter repository][js query]:

```query
(assignment_expression
  left: [
    (identifier) @name
    (member_expression
      property: (property_identifier) @name)
  ]
  right: [(arrow_function) (function)]
) @definition.function
```

An even more sophisticated query is in the [Ruby Tree-sitter repository][ruby query], which uses built-in functions to
strip the Ruby comment character (`#`) from the docstrings associated with a class or singleton-class declaration, then
selects only the docstrings adjacent to the node matched as `@definition.class`.

```query
(
  (comment)* @doc
  .
  [
    (class
      name: [
        (constant) @name
        (scope_resolution
          name: (_) @name)
      ]) @definition.class
    (singleton_class
      value: [
        (constant) @name
        (scope_resolution
          name: (_) @name)
      ]) @definition.class
  ]
  (#strip! @doc "^#\\s*")
  (#select-adjacent! @doc @definition.class)
)
```

The below table describes a standard vocabulary for kinds and roles during the tagging process. New applications may extend
(or only recognize a subset of) these capture names, but it is desirable to standardize on the names below.

| Category                 | Tag                         |
| ------------------------ | --------------------------- |
| Class definitions        | `@definition.class`         |
| Function definitions     | `@definition.function`      |
| Interface definitions    | `@definition.interface`     |
| Method definitions       | `@definition.method`        |
| Module definitions       | `@definition.module`        |
| Function/method calls    | `@reference.call`           |
| Class reference          | `@reference.class`          |
| Interface implementation | `@reference.implementation` |

## Command-line invocation

You can use the `tree-sitter tags` command to test out a tags query file, passing as arguments one or more files to tag.
We can run this tool from within the Tree-sitter Ruby repository, over code in a file called `test.rb`:

```ruby
module Foo
  class Bar
    # won't be included

    # is adjacent, will be
    def baz
    end
  end
end
```

Invoking `tree-sitter tags test.rb` produces the following console output, representing matched entities' name, role, location,
first line, and docstring:

```text
    test.rb
        Foo              | module       def (0, 7) - (0, 10) `module Foo`
        Bar              | class        def (1, 8) - (1, 11) `class Bar`
        baz              | method       def (2, 8) - (2, 11) `def baz`  "is adjacent, will be"
```

It is expected that tag queries for a given language are located at `queries/tags.scm` in that language's repository.

## Unit Testing

Tags queries may be tested with `tree-sitter test`. Files under `test/tags/` are checked using the same comment system as
[highlights queries][unit testing]. For example, the above Ruby tags can be tested with these comments:

```ruby
module Foo
  #     ^ definition.module
  class Bar
    #    ^ definition.class

    def baz
      #  ^ definition.method
    end
  end
end
```

[gh search]: https://docs.github.com/en/repositories/working-with-files/using-files/navigating-code-on-github#precise-and-search-based-navigation
[js query]: https://github.com/tree-sitter/tree-sitter-javascript/blob/fdeb68ac8d2bd5a78b943528bb68ceda3aade2eb/queries/tags.scm#L63-L70
[node]: https://github.com/tree-sitter/tree-sitter-python/blob/78c4e9b6b2f08e1be23b541ffced47b15e2972ad/grammar.js#L354
[query]: https://github.com/tree-sitter/tree-sitter-python/blob/78c4e9b6b2f08e1be23b541ffced47b15e2972ad/queries/tags.scm#L4-L5
[ruby query]: https://github.com/tree-sitter/tree-sitter-ruby/blob/1ebfdb288842dae5a9233e2509a135949023dd82/queries/tags.scm#L24-L43
[query language]: ./using-parsers/queries/index.md
[unit testing]: ./3-syntax-highlighting.md#unit-testing
