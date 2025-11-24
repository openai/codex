```console
$ 04_02_validate_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_02_validate_derive[EXE] <PORT>

Arguments:
  <PORT>  Network port to use

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 04_02_validate_derive 22
PORT = 22

$ 04_02_validate_derive foobar
? failed
error: invalid value 'foobar' for '<PORT>': `foobar` isn't a port number

For more information, try '--help'.

$ 04_02_validate_derive 0
? failed
error: invalid value '0' for '<PORT>': port not in range 1-65535

For more information, try '--help'.

```
