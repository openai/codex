```console
$ 03_06_required --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_06_required[EXE] <name>

Arguments:
  <name>  

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_06_required
? 2
error: the following required arguments were not provided:
  <name>

Usage: 03_06_required[EXE] <name>

For more information, try '--help'.

$ 03_06_required bob
name: "bob"

```
