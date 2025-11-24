```console
$ 04_04_custom --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_04_custom[EXE] [OPTIONS] [INPUT_FILE]

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

$ 04_04_custom
? failed
error: Can only modify one version field

Usage: 04_04_custom[EXE] [OPTIONS] [INPUT_FILE]

For more information, try '--help'.

$ 04_04_custom --major
Version: 2.2.3

$ 04_04_custom --major --minor
? failed
error: Can only modify one version field

Usage: 04_04_custom[EXE] [OPTIONS] [INPUT_FILE]

For more information, try '--help'.

$ 04_04_custom --major -c config.toml
? failed
Version: 2.2.3
error: INPUT_FILE or --spec-in is required when using --config

Usage: 04_04_custom[EXE] [OPTIONS] [INPUT_FILE]

For more information, try '--help'.

$ 04_04_custom --major -c config.toml --spec-in input.txt
Version: 2.2.3
Doing work using input input.txt and config config.toml

```
