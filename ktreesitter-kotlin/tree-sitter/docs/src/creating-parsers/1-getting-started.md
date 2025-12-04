# Getting Started

## Dependencies

To develop a Tree-sitter parser, there are two dependencies that you need to install:

- **A JavaScript runtime** — Tree-sitter grammars are written in JavaScript, and Tree-sitter uses a JavaScript runtime
(the default being [Node.js][node.js]) to interpret JavaScript files. It requires this runtime command (default: `node`)
to be in one of the directories in your [`PATH`][path-env].

- **A C Compiler** — Tree-sitter creates parsers that are written in C. To run and test these parsers with the
`tree-sitter parse` or `tree-sitter test` commands, you must have a C/C++ compiler installed. Tree-sitter will try to look
for these compilers in the standard places for each platform.

## Installation

To create a Tree-sitter parser, you need to use [the `tree-sitter` CLI][tree-sitter-cli]. You can install the CLI in a few
different ways:

- Build the `tree-sitter-cli` [Rust crate][crate] from source using [`cargo`][cargo], the Rust package manager. This works
on any platform. See [the contributing docs](../6-contributing.md#developing-tree-sitter) for more information.

- Install the `tree-sitter-cli` [Rust crate][crate] from [crates.io][crates.io] using [`cargo`][cargo]. You can do so by
running the following command: `cargo install tree-sitter-cli --locked`

- Install the `tree-sitter-cli` [Node.js module][node-module] using [`npm`][npm], the Node package manager. This approach
is fast, but is only works on certain platforms, because it relies on pre-built binaries.

- Download a binary for your platform from [the latest GitHub release][releases], and put it into a directory on your `PATH`.

## Project Setup

The preferred convention is to name the parser repository "tree-sitter-" followed by the name of the language, in lowercase.

```sh
mkdir tree-sitter-${LOWER_PARSER_NAME}
cd tree-sitter-${LOWER_PARSER_NAME}
```

```admonish note
The `LOWER-` prefix here means the "lowercase" name of the language.
```

### Init

Once you've installed the `tree-sitter` CLI tool, you can start setting up your project, which will allow your parser to
be used from multiple languages.

```sh
# This will prompt you for input
tree-sitter init
```

The `init` command will create a bunch of files in the project.
There should be a file called `grammar.js` with the following contents:

```js
/**
 * @file PARSER_DESCRIPTION
 * @author PARSER_AUTHOR_NAME PARSER_AUTHOR_EMAIL
 * @license PARSER_LICENSE
 */

/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: 'LOWER_PARSER_NAME',

  rules: {
    // TODO: add the actual grammar rules
    source_file: $ => 'hello'
  }
});
```

```admonish info
The placeholders shown above would be replaced with the corresponding data you provided in the `init` sub-command's
prompts.
```

To learn more about this command, check the [reference page](../cli/init.md).

### Generate

Next, run the following command:

```sh
tree-sitter generate
```

This will generate the C code required to parse this trivial language.

You can test this parser by creating a source file with the contents "hello" and parsing it:

```sh
echo 'hello' > example-file
tree-sitter parse example-file
```

Alternatively, in Windows PowerShell:

```pwsh
"hello" | Out-File example-file -Encoding utf8
tree-sitter parse example-file
```

This should print the following:

```text
(source_file [0, 0] - [1, 0])
```

You now have a working parser.

Finally, look back at the [triple-slash][] and [`@ts-check`][ts-check] comments in `grammar.js`; these tell your editor
to provide documentation and type information as you edit your grammar. For these to work, you must download Tree-sitter's
TypeScript API from npm into a `node_modules` directory in your project:

```sh
npm install # or your package manager of choice
```

To learn more about this command, check the [reference page](../cli/generate.md).

[cargo]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[crate]: https://crates.io/crates/tree-sitter-cli
[crates.io]: https://crates.io/crates/tree-sitter-cli
[node-module]: https://www.npmjs.com/package/tree-sitter-cli
[node.js]: https://nodejs.org
[npm]: https://docs.npmjs.com
[path-env]: https://en.wikipedia.org/wiki/PATH_(variable)
[releases]: https://github.com/tree-sitter/tree-sitter/releases/latest
[tree-sitter-cli]: https://github.com/tree-sitter/tree-sitter/tree/master/cli
[triple-slash]: https://www.typescriptlang.org/docs/handbook/triple-slash-directives.html
[ts-check]: https://www.typescriptlang.org/docs/handbook/intro-to-js-ts.html
