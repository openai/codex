```console
$ 04_01_enum --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_01_enum[EXE] <MODE>

Arguments:
  <MODE>
          What mode to run the program in

          Possible values:
          - fast: Run swiftly
          - slow: Crawl slowly but steadily

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

$ 04_01_enum -h
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 04_01_enum[EXE] <MODE>

Arguments:
  <MODE>  What mode to run the program in [possible values: fast, slow]

Options:
  -h, --help     Print help (see more with '--help')
  -V, --version  Print version

$ 04_01_enum fast
Hare

$ 04_01_enum slow
Tortoise

$ 04_01_enum medium
? failed
error: invalid value 'medium' for '<MODE>'
  [possible values: fast, slow]

For more information, try '--help'.

```
