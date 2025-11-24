```console
$ 03_01_flag_bool_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_01_flag_bool_derive[EXE] [OPTIONS]

Options:
  -v, --verbose  
  -h, --help     Print help
  -V, --version  Print version

$ 03_01_flag_bool_derive
verbose: false

$ 03_01_flag_bool_derive --verbose
verbose: true

$ 03_01_flag_bool_derive --verbose --verbose
? failed
error: the argument '--verbose' cannot be used multiple times

Usage: 03_01_flag_bool_derive[EXE] [OPTIONS]

For more information, try '--help'.

```
