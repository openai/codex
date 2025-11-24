```console
$ 03_02_option_mult --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_02_option_mult[EXE] [OPTIONS]

Options:
  -n, --name <name>  
  -h, --help         Print help
  -V, --version      Print version

$ 03_02_option_mult
names: []

$ 03_02_option_mult --name bob
names: ["bob"]

$ 03_02_option_mult --name bob --name john
names: ["bob", "john"]

$ 03_02_option_mult_derive --name bob --name=john -n tom -n=chris -nsteve
name: ["bob", "john", "tom", "chris", "steve"]

```
