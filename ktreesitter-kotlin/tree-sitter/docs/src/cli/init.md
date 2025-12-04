# `tree-sitter init`

The `init` command is your starting point for creating a new grammar. When you run it, it sets up a repository with all
the essential files and structure needed for grammar development. Since the command includes git-related files by default,
we recommend using git for version control of your grammar.

```bash
tree-sitter init [OPTIONS] # Aliases: i
```

## Options

### `--update`

Update outdated generated files, if needed.

## Structure of `tree-sitter.json`

The main file of interest for users to configure is `tree-sitter.json`, which tells the CLI information about your grammar,
such as the location of queries.

### The `grammars` field

This field is an array of objects, though you typically only need one object in this array unless your repo has
multiple grammars (for example, `Typescript` and `TSX`).

### Example

Typically, the objects in the `"tree-sitter"` array only needs to specify a few keys:

```json
{
  "tree-sitter": [
    {
      "scope": "source.ruby",
      "file-types": [
        "rb",
        "gemspec",
        "Gemfile",
        "Rakefile"
      ],
      "first-line-regex": "#!.*\\bruby$"
    }
  ]
}
```

#### Basic Fields

These keys specify basic information about the parser:

- `scope` (required) — A string like `"source.js"` that identifies the language.
We strive to match the scope names used by popular [TextMate grammars][textmate] and by the [Linguist][linguist] library.

- `path` — A relative path from the directory containing `tree-sitter.json` to another directory containing the `src/`
folder, which contains the actual generated parser. The default value is `"."`
(so that `src/` is in the same folder as `tree-sitter.json`), and this very rarely needs to be overridden.

- `external-files` — A list of relative paths from the root dir of a
parser to files that should be checked for modifications during recompilation.
This is useful during development to have changes to other files besides scanner.c
be picked up by the cli.

#### Language Detection

These keys help to decide whether the language applies to a given file:

- `file-types` — An array of filename suffix strings. The grammar will be used for files whose names end with one of
these suffixes. Note that the suffix may match an *entire* filename.

- `first-line-regex` — A regex pattern that will be tested against the first line of a file
to determine whether this language applies to the file. If present, this regex will be used for any file whose
language does not match any grammar's `file-types`.

- `content-regex` — A regex pattern that will be tested against the contents of the file
to break ties in cases where multiple grammars matched the file using the above two criteria. If the regex matches,
this grammar will be preferred over another grammar with no `content-regex`. If the regex does not match, a grammar with
no `content-regex` will be preferred over this one.

- `injection-regex` — A regex pattern that will be tested against a *language name* to determine whether this language
should be used for a potential *language injection* site.
Language injection is described in more detail in [the relevant section](../3-syntax-highlighting.md#language-injection).

#### Query Paths

These keys specify relative paths from the directory containing `tree-sitter.json` to the files that control syntax highlighting:

- `highlights` — Path to a *highlight query*. Default: `queries/highlights.scm`
- `locals` — Path to a *local variable query*. Default: `queries/locals.scm`.
- `injections` — Path to an *injection query*. Default: `queries/injections.scm`.
- `tags` — Path to an *tag query*. Default: `queries/tags.scm`.

### The `metadata` field

This field contains information that tree-sitter will use to populate relevant bindings' files, especially their versions.
Typically, this will all be set up when you run `tree-sitter init`, but you are welcome to update it as you see fit.

- `version` (required) — The current version of your grammar, which should follow [semver][semver]
- `license` — The license of your grammar, which should be a valid [SPDX license][spdx]
- `description` — The brief description of your grammar
- `authors` (required) — An array of objects that contain a `name` field, and optionally an `email` and `url` field.
Each field is a string
- `links` — An object that contains a `repository` field, and optionally a `homepage` field. Each field is a string
- `namespace` — The namespace for the `Java` and `Kotlin` bindings, defaults to `io.github.tree-sitter` if not provided

### The `bindings` field

This field controls what bindings are generated when the `init` command is run.
Each key is a language name, and the value is a boolean.

- `c` (default: `true`)
- `go` (default: `true`)
- `java` (default: `false`)
- `kotlin` (default: `false`)
- `node` (default: `true`)
- `python` (default: `true`)
- `rust` (default: `true`)
- `swift` (default: `false`)

## Binding Files

When you run `tree-sitter init`, the CLI will also generate a number of files in your repository that allow for your parser
to be used from different language. Here is a list of these bindings files that are generated, and what their purpose is:

### C/C++

- `Makefile` — This file tells [`make`][make] how to compile your language.
- `CMakeLists.txt` — This file tells [`cmake`][cmake] how to compile your language.
- `bindings/c/tree_sitter/tree-sitter-language.h` — This file provides the C interface of your language.
- `bindings/c/tree-sitter-language.pc` — This file provides [pkg-config][pkg-config] metadata about your language's C library.
- `src/tree_sitter/parser.h` — This file provides some basic C definitions that are used in your generated `parser.c` file.
- `src/tree_sitter/alloc.h` — This file provides some memory allocation macros that are to be used in your external scanner,
if you have one.
- `src/tree_sitter/array.h` — This file provides some array macros that are to be used in your external scanner,
if you have one.

### Go

- `go.mod` — This file is the manifest of the Go module.
- `bindings/go/binding.go` — This file wraps your language in a Go module.
- `bindings/go/binding_test.go` — This file contains a test for the Go package.

### Node

- `binding.gyp` — This file tells Node.js how to compile your language.
- `package.json` — This file is the manifest of the Node.js package.
- `bindings/node/binding.cc` — This file wraps your language in a JavaScript module for Node.js.
- `bindings/node/index.js` — This is the file that Node.js initially loads when using your language.
- `bindings/node/index.d.ts` — This file provides type hints for your parser when used in TypeScript.
- `bindings/node/binding_test.js` — This file contains a test for the Node.js package.

### Python

- `pyproject.toml` — This file is the manifest of the Python package.
- `setup.py` — This file tells Python how to compile your language.
- `bindings/python/tree_sitter_language/binding.c` — This file wraps your language in a Python module.
- `bindings/python/tree_sitter_language/__init__.py` — This file tells Python how to load your language.
 `bindings/python/tree_sitter_language/__init__.pyi` — This file provides type hints for your parser when used in Python.
- `bindings/python/tree_sitter_language/py.typed` — This file provides type hints for your parser when used in Python.
- `bindings/python/tests/test_binding.py` — This file contains a test for the Python package.

### Rust

- `Cargo.toml` — This file is the manifest of the Rust package.
- `bindings/rust/lib.rs` — This file wraps your language in a Rust crate when used in Rust.
- `bindings/rust/build.rs` — This file wraps the building process for the Rust crate.

### Swift

- `Package.swift` — This file tells Swift how to compile your language.
- `bindings/swift/TreeSitterLanguage/language.h` — This file wraps your language in a Swift module when used in Swift.
- `bindings/swift/TreeSitterLanguageTests/TreeSitterLanguageTests.swift` — This file contains a test for the Swift package.

### Additional Files

Additionally, there's a few other files that are generated when you run `tree-sitter init`,
that aim to improve the development experience:

- `.editorconfig` — This file tells your editor how to format your code. More information about this file can be found [here][editorconfig]
- `.gitattributes` — This file tells Git how to handle line endings, and tells GitHub what files are generated.
- `.gitignore` — This file tells Git what files to ignore when committing changes.

[cmake]: https://cmake.org/cmake/help/latest
[editorconfig]: https://editorconfig.org
[linguist]: https://github.com/github/linguist
[make]: https://www.gnu.org/software/make/manual/make.html
[pkg-config]: https://www.freedesktop.org/wiki/Software/pkg-config
[semver]: https://semver.org
[spdx]: https://spdx.org/licenses
[textmate]: https://macromates.com/manual/en/language_grammars
