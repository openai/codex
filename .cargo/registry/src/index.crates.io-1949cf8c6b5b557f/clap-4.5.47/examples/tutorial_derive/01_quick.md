```console
$ 01_quick_derive --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 01_quick_derive[EXE] [OPTIONS] [NAME] [COMMAND]

Commands:
  test  does testing things
  help  Print this message or the help of the given subcommand(s)

Arguments:
  [NAME]  Optional name to operate on

Options:
  -c, --config <FILE>  Sets a custom config file
  -d, --debug...       Turn debugging information on
  -h, --help           Print help
  -V, --version        Print version

```

By default, the program does nothing:
```console
$ 01_quick_derive
Debug mode is off

```

But you can mix and match the various features
```console
$ 01_quick_derive -dd test
Debug mode is on
Not printing testing lists...

```
