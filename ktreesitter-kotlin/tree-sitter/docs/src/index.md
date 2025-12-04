<div style="display: flex; justify-content: center; margin-left: 32px;">
    <a href="./"><img src="assets/images/tree-sitter-small.png" width="200" height="200" alt="Tree-sitter logo" /></a>
</div>

# Introduction

Tree-sitter is a parser generator tool and an incremental parsing library. It can build a concrete syntax tree for a source
file and efficiently update the syntax tree as the source file is edited. Tree-sitter aims to be:

- **General** enough to parse any programming language
- **Fast** enough to parse on every keystroke in a text editor
- **Robust** enough to provide useful results even in the presence of syntax errors
- **Dependency-free** so that the runtime library (which is written in pure [C11](https://github.com/tree-sitter/tree-sitter/tree/master/lib)) can be embedded in any application

## Language Bindings

There are bindings that allow Tree-sitter to be used from the following languages:

### Official

- [C#](https://github.com/tree-sitter/csharp-tree-sitter)
- [Go](https://github.com/tree-sitter/go-tree-sitter)
- [Haskell](https://github.com/tree-sitter/haskell-tree-sitter)
- [Java (JDK 22)](https://github.com/tree-sitter/java-tree-sitter)
- [JavaScript (Node.js)](https://github.com/tree-sitter/node-tree-sitter)
- [JavaScript (Wasm)](https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_web)
- [Kotlin](https://github.com/tree-sitter/kotlin-tree-sitter)
- [Python](https://github.com/tree-sitter/py-tree-sitter)
- [Rust](https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_rust)
- [Zig](https://github.com/tree-sitter/zig-tree-sitter)

### Third-party

- [Delphi](https://github.com/modersohn/delphi-tree-sitter)
- [ELisp](https://www.gnu.org/software/emacs/manual/html_node/elisp/Parsing-Program-Source.html)
- [Guile](https://github.com/Z572/guile-ts)
- [Java (JDK 8+)](https://github.com/bonede/tree-sitter-ng)
- [Java (JDK 11+)](https://github.com/seart-group/java-tree-sitter)
- [Julia](https://github.com/MichaelHatherly/TreeSitter.jl)
- [Lua](https://github.com/euclidianAce/ltreesitter)
- [Lua](https://github.com/xcb-xwii/lua-tree-sitter)
- [OCaml](https://github.com/returntocorp/ocaml-tree-sitter-core)
- [Odin](https://github.com/laytan/odin-tree-sitter)
- [Perl](https://metacpan.org/pod/Text::Treesitter)
- [R](https://github.com/DavisVaughan/r-tree-sitter)
- [Ruby](https://github.com/Faveod/ruby-tree-sitter)
- [Ruby](https://github.com/calicoday/ruby-tree-sitter-ffi)
- [Swift](https://github.com/ChimeHQ/SwiftTreeSitter)

## Parsers

The following parsers can be found in the upstream organization:

- [Agda](https://github.com/tree-sitter/tree-sitter-agda)
- [Bash](https://github.com/tree-sitter/tree-sitter-bash)
- [C](https://github.com/tree-sitter/tree-sitter-c)
- [C++](https://github.com/tree-sitter/tree-sitter-cpp)
- [C#](https://github.com/tree-sitter/tree-sitter-c-sharp)
- [CSS](https://github.com/tree-sitter/tree-sitter-css)
- [ERB / EJS](https://github.com/tree-sitter/tree-sitter-embedded-template)
- [Go](https://github.com/tree-sitter/tree-sitter-go)
- [Haskell](https://github.com/tree-sitter/tree-sitter-haskell)
- [HTML](https://github.com/tree-sitter/tree-sitter-html)
- [Java](https://github.com/tree-sitter/tree-sitter-java)
- [JavaScript](https://github.com/tree-sitter/tree-sitter-javascript)
- [JSDoc](https://github.com/tree-sitter/tree-sitter-jsdoc)
- [JSON](https://github.com/tree-sitter/tree-sitter-json)
- [Julia](https://github.com/tree-sitter/tree-sitter-julia)
- [OCaml](https://github.com/tree-sitter/tree-sitter-ocaml)
- [PHP](https://github.com/tree-sitter/tree-sitter-php)
- [Python](https://github.com/tree-sitter/tree-sitter-python)
- [Regex](https://github.com/tree-sitter/tree-sitter-regex)
- [Ruby](https://github.com/tree-sitter/tree-sitter-ruby)
- [Rust](https://github.com/tree-sitter/tree-sitter-rust)
- [Scala](https://github.com/tree-sitter/tree-sitter-scala)
- [TypeScript](https://github.com/tree-sitter/tree-sitter-typescript)
- [Verilog](https://github.com/tree-sitter/tree-sitter-verilog)

A list of known parsers can be found in the [wiki](https://github.com/tree-sitter/tree-sitter/wiki/List-of-parsers).

## Talks on Tree-sitter

- [Strange Loop 2018](https://www.thestrangeloop.com/2018/tree-sitter---a-new-parsing-system-for-programming-tools.html)
- [FOSDEM 2018](https://www.youtube.com/watch?v=0CGzC_iss-8)
- [GitHub Universe 2017](https://www.youtube.com/watch?v=a1rC79DHpmY)

## Underlying Research

The design of Tree-sitter was greatly influenced by the following research papers:

- [Practical Algorithms for Incremental Software Development Environments](https://www2.eecs.berkeley.edu/Pubs/TechRpts/1997/CSD-97-946.pdf)
- [Context Aware Scanning for Parsing Extensible Languages](https://www-users.cse.umn.edu/~evw/pubs/vanwyk07gpce/vanwyk07gpce.pdf)
- [Efficient and Flexible Incremental Parsing](https://harmonia.cs.berkeley.edu/papers/twagner-parsing.pdf)
- [Incremental Analysis of Real Programming Languages](https://harmonia.cs.berkeley.edu/papers/twagner-glr.pdf)
- [Error Detection and Recovery in LR Parsers](https://web.archive.org/web/20240302031213/https://what-when-how.com/compiler-writing/bottom-up-parsing-compiler-writing-part-13)
- [Error Recovery for LR Parsers](https://apps.dtic.mil/sti/pdfs/ADA043470.pdf)
