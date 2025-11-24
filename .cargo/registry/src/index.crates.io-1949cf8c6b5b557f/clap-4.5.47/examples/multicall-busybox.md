See the documentation for [`Command::multicall`][crate::Command::multicall] for rationale.

This example omits every command except true and false,
which are the most trivial to implement,
```console
$ busybox true
? 0

$ busybox false
? 1

```
*Note: without the links setup, we can't demonstrate the multicall behavior*

But includes the `--install` option as an example of why it can be useful
for the main program to take arguments that aren't applet subcommands.
```console
$ busybox --install
? failed
...

```

Though users must pass something:
```console
$ busybox
? failed
Usage: busybox [OPTIONS] [APPLET]

APPLETS:
  true   does nothing successfully
  false  does nothing unsuccessfully
  help   Print this message or the help of the given subcommand(s)

Options:
      --install <install>  Install hardlinks for all subcommands in path
  -h, --help               Print help

```
