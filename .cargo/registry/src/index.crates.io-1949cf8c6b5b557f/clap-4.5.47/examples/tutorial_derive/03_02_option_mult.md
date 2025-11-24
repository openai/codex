```console
$ 03_02_option_mult_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_02_option_mult_derive[EXE] [OPTIONS]

Options:
  -n, --name <NAME>  
  -h, --help         Print help
  -V, --version      Print version

$ 03_02_option_mult_derive
name: []

$ 03_02_option_mult_derive --name bob
name: ["bob"]

$ 03_02_option_mult_derive --name bob --name john
name: ["bob", "john"]

$ 03_02_option_mult_derive --name bob --name=john -n tom -n=chris -nsteve
name: ["bob", "john", "tom", "chris", "steve"]

```
