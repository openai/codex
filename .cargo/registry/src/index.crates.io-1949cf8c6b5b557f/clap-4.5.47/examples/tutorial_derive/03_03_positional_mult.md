```console
$ 03_03_positional_mult_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_03_positional_mult_derive[EXE] [NAME]...

Arguments:
  [NAME]...  

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_03_positional_mult_derive
name: []

$ 03_03_positional_mult_derive bob
name: ["bob"]

$ 03_03_positional_mult_derive bob john
name: ["bob", "john"]

```
