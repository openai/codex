```console
$ 03_04_subcommands_derive help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_04_subcommands_derive[EXE] <COMMAND>

Commands:
  add   Adds files to myapp
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_04_subcommands_derive help add
Adds files to myapp

Usage: 03_04_subcommands_derive[EXE] add [NAME]

Arguments:
  [NAME]  

Options:
  -h, --help     Print help
  -V, --version  Print version

$ 03_04_subcommands_derive add bob
'myapp add' was used, name is: Some("bob")

```

When specifying commands with `command: Commands`, they are [required][crate::Command::subcommand_required].
By default, a missing subcommand will [show help rather than error][crate::Command::arg_required_else_help].
```console
$ 03_04_subcommands_derive
? failed
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: 03_04_subcommands_derive[EXE] <COMMAND>

Commands:
  add   Adds files to myapp
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

```
To make a subcommand optional, wrap it in an `Option` (e.g. `command: Option<Commands>`).

Since we specified [`#[command(propagate_version = true)]`][crate::Command::propagate_version],
the `--version` flag is available in all subcommands:
```console
$ 03_04_subcommands_derive --version
clap [..]

$ 03_04_subcommands_derive add --version
clap-add [..]

```
