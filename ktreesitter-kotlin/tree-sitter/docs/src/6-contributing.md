# Contributing

## Code of Conduct

Contributors to Tree-sitter should abide by the [Contributor Covenant][covenant].

## Developing Tree-sitter

### Prerequisites

To make changes to Tree-sitter, you should have:

1. A C compiler, for compiling the core library and the generated parsers.
2. A [Rust toolchain][rust], for compiling the Rust bindings, the highlighting library, and the CLI.
3. Node.js and NPM, for generating parsers from `grammar.js` files.
4. Either [Emscripten][emscripten], [Docker][docker], or [podman][podman] for
compiling the library to WASM.

### Building

Clone the repository:

```sh
git clone https://github.com/tree-sitter/tree-sitter
cd tree-sitter
```

Optionally, build the WASM library. If you skip this step, then the `tree-sitter playground` command will require an internet
connection. If you have Emscripten installed, this will use your `emcc` compiler. Otherwise, it will use Docker or Podman:

```sh
cd lib/binding_web
npm install # or your JS package manager of choice
npm run build
```

Build the Rust libraries and the CLI:

```sh
cargo build --release
```

This will create the `tree-sitter` CLI executable in the `target/release` folder.

If you want to automatically install the `tree-sitter` CLI in your system, you can run:

```sh
cargo install --path cli
```

If you're going to be in a fast iteration cycle and would like the CLI to build faster, you can use the `release-dev` profile:

```sh
cargo build --release --profile release-dev
# or
cargo install --path cli --profile release-dev
```

### Testing

Before you can run the tests, you need to fetch some upstream grammars that are used for testing:

```sh
cargo xtask fetch-fixtures
```

To test any changes you've made to the CLI, you can regenerate these parsers using your current CLI code:

```sh
cargo xtask generate-fixtures
```

Then you can run the tests:

```sh
cargo xtask test
```

Similarly, to test the WASM binding, you need to compile these parsers to WASM:

```sh
cargo xtask generate-fixtures --wasm
cargo xtask test-wasm
```

### Debugging

The test script has a number of useful flags. You can list them all by running `cargo xtask test -h`.
Here are some of the main flags:

If you want to run a specific unit test, pass its name (or part of its name) as an argument:

```sh
cargo xtask test test_does_something
```

You can run the tests under the debugger (either `lldb` or `gdb`) using the `-g` flag:

```sh
cargo xtask test -g test_does_something
```

Part of the Tree-sitter test suite involves parsing the _corpus_ tests for several languages and performing randomized edits
to each example in the corpus. If you just want to run the tests for a particular _language_, you can pass the `-l` flag.
Additionally, if you want to run a particular _example_ from the corpus, you can pass the `-e` flag:

```sh
cargo xtask test -l javascript -e Arrays
```

## Published Packages

The main [`tree-sitter/tree-sitter`][ts repo] repository contains the source code for
several packages that are published to package registries for different languages:

* Rust crates on [crates.io][crates]:
  * [`tree-sitter`][lib crate] — A Rust binding to the core library
  * [`tree-sitter-highlight`][highlight crate] — The syntax-highlighting library
  * [`tree-sitter-cli`][cli crate] — The command-line tool

* JavaScript modules on [npmjs.com][npmjs]:
  * [`web-tree-sitter`][web-ts] — A WASM-based JavaScript binding to the core library
  * [`tree-sitter-cli`][cli package] — The command-line tool

There are also several other dependent repositories that contain other published packages:

* [`tree-sitter/node-tree-sitter`][node ts] — Node.js bindings to the core library,
published as [`tree-sitter`][node package] on npmjs.com
* [`tree-sitter/py-tree-sitter`][py ts] — Python bindings to the core library,
published as [`tree-sitter`][py package] on [PyPI.org][pypi].
* [`tree-sitter/go-tree-sitter`][go ts] — Go bindings to the core library,
published as [`tree_sitter`][go package] on [pkg.go.dev][go.dev].

## Developing Documentation

Our current static site generator for documentation is [`mdBook`][mdBook], with a little bit of custom JavaScript to handle
the playground page. Most of the documentation is written in Markdown, including this file! You can find these files
at [`docs/src`][docs src]. If you'd like to submit a PR to improve the documentation, navigate to the page you'd like to
edit and hit the edit icon at the top right of the page.

### Prerequisites for Local Development

```admonish note
We're assuming you have `cargo` installed, the Rust package manager.
```

To run and iterate on the docs locally, the
[`mdbook`][mdbook cli] CLI tool is required, which can be installed with

```sh
cargo install mdbook
```

You might have noticed we have some fancy admonitions sprinkled throughout the documentation, like the note above.
These are created using [`mdbook-admonish`][admonish], a [preprocessor][preprocessor] for `mdBook`. As such, this is also
a requirement for developing the documentation locally. To install it, run:

```sh
cargo install mdbook-admonish
```

Once you've installed it, you can begin using admonitions in your markdown files. See the [reference][admonish reference]
for more information.

### Spinning it up

Now that you've installed the prerequisites, you can run the following command to start a local server:

```sh
cd docs
mdbook serve --open
```

`mdbook` has a live-reload feature, so any changes you make to the markdown files will be reflected in the browser after
a short delay. Once you've made a change that you're happy with, you can submit a PR with your changes.

### Improving the Playground

The playground page is a little more complicated, but if you know some basic JavaScript and CSS you should be able to make
changes. The playground code can be found in [`docs/src/assets/js/playground.js`][playground], and its corresponding css
at [`docs/src/assets/css/playground.css`][playground css]. The editor of choice we use for the playground is [CodeMirror][codemirror],
and the tree-sitter module is fetched from [here][js url]. This, along with the wasm module and wasm parsers, live in the
[.github.io repo][gh.io repo].

[admonish]: https://github.com/tommilligan/mdbook-admonish
[admonish reference]: https://tommilligan.github.io/mdbook-admonish/reference.html
[cli crate]: https://crates.io/crates/tree-sitter-cli
[cli package]: https://www.npmjs.com/package/tree-sitter-cli
[codemirror]: https://codemirror.net
[covenant]: https://www.contributor-covenant.org/version/1/4/code-of-conduct
[crates]: https://crates.io
[docker]: https://www.docker.com
[docs src]: https://github.com/tree-sitter/tree-sitter/tree/master/docs/src
[emscripten]: https://emscripten.org
[gh.io repo]: https://github.com/tree-sitter/tree-sitter.github.io
[go.dev]: https://pkg.go.dev
[go package]: https://pkg.go.dev/github.com/tree-sitter/go-tree-sitter
[go ts]: https://github.com/tree-sitter/go-tree-sitter
[highlight crate]: https://crates.io/crates/tree-sitter-highlight
[js url]: https://tree-sitter.github.io/tree-sitter.js
[lib crate]: https://crates.io/crates/tree-sitter
[mdBook]: https://rust-lang.github.io/mdBook
[mdbook cli]: https://rust-lang.github.io/mdBook/guide/installation.html
[node package]: https://www.npmjs.com/package/tree-sitter
[node ts]: https://github.com/tree-sitter/node-tree-sitter
[npmjs]: https://npmjs.com
[playground]: https://github.com/tree-sitter/tree-sitter/blob/master/docs/src/assets/js/playground.js
[playground css]: https://github.com/tree-sitter/tree-sitter/blob/master/docs/src/assets/css/playground.css
[podman]: https://podman.io
[preprocessor]: https://rust-lang.github.io/mdBook/for_developers/preprocessors.html
[py package]: https://pypi.org/project/tree-sitter
[py ts]: https://github.com/tree-sitter/py-tree-sitter
[pypi]: https://pypi.org
[rust]: https://rustup.rs
[ts repo]: https://github.com/tree-sitter/tree-sitter
[web-ts]: https://www.npmjs.com/package/web-tree-sitter
