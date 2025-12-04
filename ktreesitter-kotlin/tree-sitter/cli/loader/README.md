# Tree-sitter Loader

The `tree-sitter` command-line program will dynamically find and build grammars
at runtime, if you have cloned the grammars' repositories to your local
filesystem. This helper crate implements that logic, so that you can use it in
your own program analysis tools, as well.
