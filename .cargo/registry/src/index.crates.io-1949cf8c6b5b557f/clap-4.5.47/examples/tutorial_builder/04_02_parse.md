```console
$ 04_02_parse --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_02_parse[EXE] <PORT>

Arguments:
  <PORT>  Network port to use

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 04_02_parse 22
PORT = 22

$ 04_02_parse foobar
? failed
error: invalid value 'foobar' for '<PORT>': invalid digit found in string

For more information, try '--help'.

$ 04_02_parse_derive 0
? failed
error: invalid value '0' for '<PORT>': 0 is not in 1..=65535

For more information, try '--help'.

```
