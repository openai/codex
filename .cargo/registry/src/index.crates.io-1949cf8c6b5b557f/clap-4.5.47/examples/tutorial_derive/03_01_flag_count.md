```console
$ 03_01_flag_count_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_01_flag_count_derive[EXE] [OPTIONS]

Options:
  -v, --verbose...  
  -h, --help        Print help
  -V, --version     Print version

$ 03_01_flag_count_derive
verbose: 0

$ 03_01_flag_count_derive --verbose
verbose: 1

$ 03_01_flag_count_derive --verbose --verbose
verbose: 2

```
