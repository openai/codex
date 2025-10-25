## `llmcc`


Use the `llmcc` shell command to indexing flies or folders first, then extract dependenciy graphs from a symbol.


*** Full help output
llmcc: llm context compiler

Usage: llmcc [OPTIONS] [FILE]...

Arguments:
  [FILE]...  Files to compile

Options:
  -d, --dir <DIR>     Load all .rs files from a directory (recursive)
      --print-ir      Print intermediate representation (IR)
      --print-graph   Print project graph
      --query <NAME>  Name of the symbol/function to query (enables find_depends mode)
      --recursive     Search recursively for transitive dependencies (default: direct dependencies only)
  -h, --help          Print help
  -V, --version       Print version

