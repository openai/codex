```console
$ 03_02_option --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_02_option[EXE] [OPTIONS]

Options:
  -n, --name <name>  
  -h, --help         Print help
  -V, --version      Print version

$ 03_02_option
name: None

$ 03_02_option --name bob
name: Some("bob")

$ 03_02_option --name=bob
name: Some("bob")

$ 03_02_option -n bob
name: Some("bob")

$ 03_02_option -n=bob
name: Some("bob")

$ 03_02_option -nbob
name: Some("bob")

```
