# Contributing

## Code of Conduct

Contributors to Tree-sitter should abide by the [Contributor Covenant][covenant].

## Developing Web-tree-sitter

### Prerequisites

To make changes to Web-tree-sitter, you should have:

1. A [Rust toolchain][rust], for running the xtasks necessary to build the library.
2. Node.js and NPM (or an equivalent package manager).
3. Either [Emscripten][emscripten], [Docker][docker], or [podman][podman] for
compiling the library to WASM.

### Building

Clone the repository:

```sh
git clone https://github.com/tree-sitter/tree-sitter
cd tree-sitter/lib/binding_web
```

Install the necessary dependencies:

```sh
npm install
```

Build the library:

```sh
npm run build
```

Note that the build process requires a Rust toolchain to be installed. If you don't have one installed, you can install it
by visiting the [Rust website][rust] and following the instructions there.

> [!NOTE]
> By default, the build process will emit an ES6 module. If you need a CommonJS module, export `CJS` to `true`, or just
> run `CJS=true npm run build`.

> [!TIP]
> To build the library with debug information, you can run `npm run build:debug`. The `CJS` environment variable is still
> taken into account.

### Putting it together

#### The C side

There are several components that come together to build the final JS and WASM files. First, we use `emscripten` in our
xtask located at `xtask/src/build_wasm.rs` from the root directory to compile the WASM files. This WASM module is output into the
local `lib` folder, and is used only in [`src/bindings.ts`][bindings.ts] to handle loading the WASM module. The C code that
is compiled into the WASM module is located in at [`lib/tree-sitter.c`][tree-sitter.c], and contains all the necessary
glue code to interact with the JS environment. If you need to update the imported functions from the tree-sitter library,
or anywhere else, you must update [`lib/exports.txt`][exports.txt]. Lastly, the type information for the WASM module is
located at [`lib/tree-sitter.d.ts`][tree-sitter.d.ts], and can be updated by running `cargo xtask build-wasm --emit-tsd`
from the root directory.

#### The TypeScript side

The TypeScript library is a higher level abstraction over the WASM module, and is located in `src`. This is where the
public API is defined, and where the WASM module is loaded and initialized. The TypeScript library is built into a single
ES6 (or CommonJS) module, and is output into the same directory as `package.json`. If you need to update the public API,
you can do so by editing the files in `src`.

If you make changes to the library that require updating the type definitions, such as adding a new public API method,
you should run:

```sh
npm run build:dts
```

This uses [`dts-buddy`][dts-buddy] to generate `web-tree-sitter.d.ts` from the public types in `src`. Additionally, a sourcemap
is generated for the `.d.ts` file, which enables `go-to definition` and other editor integrations to take you straight
to the TypeScript source code.

This TypeScript code is then compiled into a single JavaScript file with `esbuild`. The build configuration for this can
be found in [`script/build.js`][build.js], but this shouldn't need to be updated. This step is responsible for emitting
the final JS and WASM files that are shipped with the library, as well as their sourcemaps.

### Testing

Before you can run the tests, you need to fetch and build some upstream grammars that are used for testing.
Run this in the root of the repository:

```sh
cargo xtask fetch-fixtures
```

Optionally, to update the generated parser.c files:

```sh
cargo xtask generate-fixtures
```

Then you can build the WASM modules:

```sh
cargo xtask generate-fixtures --wasm
```

Now, you can run the tests. In the `lib/binding_web` directory, run:

```sh
npm test
```

> [!NOTE]
> We use `vitest` to run the tests. If you want to run a specific test, you can use the `-t` flag to pass in a pattern.
> If you want to run a specific file, you can just pass the name of the file as is. For example, to run the `parser` tests
> in `test/parser.test.ts`, you can run `npm test parser`. To run tests that have the name `descendant` somewhere, run
> `npm test -- -t descendant`.
>
> For coverage information, you can run `npm test -- --coverage`.

### Debugging

You might have noticed that when you ran `npm build`, the build process generated a couple of [sourcemaps][sourcemap]:
`tree-sitter.js.map` and `tree-sitter.wasm.map`. These sourcemaps can be used to debug the library in the browser, and are
shipped with the library on both NPM and the GitHub releases.

#### Tweaking the Emscripten build

If you're trying to tweak the Emscripten build, or are trying to debug an issue, the code for this lies in `xtask/src/build_wasm.rs`
file mentioned earlier, namely in the `run_wasm` function.

[bindings.ts]: src/bindings.ts
[build.js]: script/build.js
[covenant]: https://www.contributor-covenant.org/version/1/4/code-of-conduct
[docker]: https://www.docker.com
[dts-buddy]: https://github.com/Rich-Harris/dts-buddy
[emscripten]: https://emscripten.org
[exports.txt]: lib/exports.txt
[podman]: https://podman.io
[rust]: https://www.rust-lang.org/tools/install
[sourcemap]: https://developer.mozilla.org/en-US/docs/Glossary/Source_map
[tree-sitter.c]: lib/tree-sitter.c
[tree-sitter.d.ts]: lib/tree-sitter.d.ts
