```console
$ 03_01_flag_bool --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_01_flag_bool[EXE] [OPTIONS]

Options:
  -v, --verbose  
  -h, --help     Print help
  -V, --version  Print version

$ 03_01_flag_bool
verbose: false

$ 03_01_flag_bool --verbose
verbose: true

$ 03_01_flag_bool --verbose --verbose
? failed
error: the argument '--verbose' cannot be used multiple times

Usage: 03_01_flag_bool[EXE] [OPTIONS]

For more information, try '--help'.

```
