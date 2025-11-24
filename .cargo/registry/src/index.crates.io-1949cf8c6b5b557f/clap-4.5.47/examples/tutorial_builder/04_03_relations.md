```console
$ 04_03_relations --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_03_relations[EXE] [OPTIONS] <--set-ver <VER>|--major|--minor|--patch> [INPUT_FILE]

Arguments:
  [INPUT_FILE]  some regular input

Options:
      --set-ver <VER>      set version manually
      --major              auto inc major
      --minor              auto inc minor
      --patch              auto inc patch
      --spec-in <SPEC_IN>  some special input argument
  -c <CONFIG>              
  -h, --help               Print help
  -V, --version            Print version

$ 04_03_relations
? failed
error: the following required arguments were not provided:
  <--set-ver <VER>|--major|--minor|--patch>

Usage: 04_03_relations[EXE] <--set-ver <VER>|--major|--minor|--patch> [INPUT_FILE]

For more information, try '--help'.

$ 04_03_relations --major
Version: 2.2.3

$ 04_03_relations --major --minor
? failed
error: the argument '--major' cannot be used with '--minor'

Usage: 04_03_relations[EXE] <--set-ver <VER>|--major|--minor|--patch> [INPUT_FILE]

For more information, try '--help'.

$ 04_03_relations --major -c config.toml
? failed
error: the following required arguments were not provided:
  <INPUT_FILE|--spec-in <SPEC_IN>>

Usage: 04_03_relations[EXE] -c <CONFIG> <--set-ver <VER>|--major|--minor|--patch> <INPUT_FILE|--spec-in <SPEC_IN>>

For more information, try '--help'.

$ 04_03_relations --major -c config.toml --spec-in input.txt
Version: 2.2.3
Doing work using input input.txt and config config.toml

```
