```console
$ 03_04_subcommands help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_04_subcommands[EXE] <COMMAND>

Commands:
  add   Adds files to myapp
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_04_subcommands help add
Adds files to myapp

Usage: 03_04_subcommands[EXE] add [NAME]

Arguments:
  [NAME]  

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_04_subcommands add bob
'myapp add' was used, name is: Some("bob")

```

We set
[`Command::arg_required_else_help`][crate::Command::arg_required_else_help] to
show the help, rather than an error, when the
[required subcommand][crate::Command::subcommand_required] is missing:
```console
$ 03_04_subcommands
? failed
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_04_subcommands[EXE] <COMMAND>

Commands:
  add   Adds files to myapp
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

```

Since we specified [`Command::propagate_version`][crate::Command::propagate_version], the `--version` flag
is available in all subcommands:
```console
$ 03_04_subcommands --version
clap [..]

$ 03_04_subcommands add --version
clap-add [..]

```
