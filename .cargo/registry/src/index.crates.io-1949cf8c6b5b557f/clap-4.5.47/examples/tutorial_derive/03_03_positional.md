```console
$ 03_03_positional_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_03_positional_derive[EXE] <NAME>

Arguments:
  <NAME>  

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_03_positional_derive
? 2
error: the following required arguments were not provided:
  <NAME>

Usage: 03_03_positional_derive[EXE] <NAME>

For more information, try '--help'.

$ 03_03_positional_derive bob
name: "bob"

```
