```console
$ 03_01_flag_count --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_01_flag_count[EXE] [OPTIONS]

Options:
  -v, --verbose...  
  -h, --help        Print help
  -V, --version     Print version

$ 03_01_flag_count
verbose: 0

$ 03_01_flag_count --verbose
verbose: 1

$ 03_01_flag_count --verbose --verbose
verbose: 2

```
