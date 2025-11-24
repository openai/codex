```console
$ 03_02_option_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_02_option_derive[EXE] --name <NAME>

Options:
  -n, --name <NAME>  
  -h, --help         Print help
  -V, --version      Print version

$ 03_02_option_derive
? 2
error: the following required arguments were not provided:
  --name <NAME>

Usage: 03_02_option_derive[EXE] --name <NAME>

For more information, try '--help'.

$ 03_02_option_derive --name bob
name: "bob"

$ 03_02_option_derive --name=bob
name: "bob"

$ 03_02_option_derive -n bob
name: "bob"

$ 03_02_option_derive -n=bob
name: "bob"

$ 03_02_option_derive -nbob
name: "bob"

```
