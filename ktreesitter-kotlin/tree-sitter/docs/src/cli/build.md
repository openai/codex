# `tree-sitter build`

The `build` command compiles your parser into a dynamically-loadable library,
either as a shared object (`.so`, `.dylib`, or `.dll`) or as a WASM module.

```bash
tree-sitter build [OPTIONS] [PATH] # Aliases: b
```

You can change the compiler executable via the `CC` environment variable and add extra flags via `CFLAGS`.
For macOS or iOS, you can set `MACOSX_DEPLOYMENT_TARGET` or `IPHONEOS_DEPLOYMENT_TARGET` respectively to define the
minimum supported version.

The path argument allows you to specify the directory of the parser to build. If you don't supply this argument, the CLI
will attempt to build the parser in the current working directory.

## Options

### `-w/--wasm`

Compile the parser as a WASM module.

### `-d/--docker`

Use Docker or Podman to supply Emscripten. This removes the need to install Emscripten on your machine locally.
Note that this flag is only available when compiling to WASM.

### `-o/--output`

Specify where to output the shared object file (native or WASM). This flag accepts either an absolute path or a relative
path. If you don't supply this flag, the CLI will attempt to figure out what the language name is based on the parent
directory name to use for the output file. If the CLI can't figure it out, it will default to `parser`, thus generating
`parser.so` or `parser.wasm` in the current working directory.

### `--reuse-allocator`

Reuse the allocator that's set in the core library for the parser's external scanner. This is useful in applications
where the author overrides the default allocator with their own, and wants to ensure every parser that allocates memory
in the external scanner does so using their allocator.

### `-0/--debug`

Compile the parser with debug flags enabled. This is useful when debugging issues that require a debugger like `gdb` or `lldb`.
