**This requires enabling the [`cargo` feature flag][crate::_features].**

You can use `--` to escape further arguments.

Let's see what this looks like in the help:
```console
$ escaped-positional --help
A simple to use, efficient, and full-featured Command Line Argument Parser

Usage: escaped-positional[EXE] [OPTIONS] [-- <SLOP>...]

Arguments:
  [SLOP]...  

Options:
  -f             
  -p <PEAR>      
  -h, --help     Print help
  -V, --version  Print version

```

Here is a baseline without any arguments:
```console
$ escaped-positional
-f used: false
-p's value: None
'slops' values: []

```

Notice that we can't pass positional arguments before `--`:
```console
$ escaped-positional foo bar
? failed
error: unexpected argument 'foo' found

Usage: escaped-positional[EXE] [OPTIONS] [-- <SLOP>...]

For more information, try '--help'.

```

But you can after:
```console
$ escaped-positional -f -p=bob -- sloppy slop slop
-f used: true
-p's value: Some("bob")
'slops' values: ["sloppy", "slop", "slop"]

```

As mentioned, the parser will directly pass everything through:
```console
$ escaped-positional -- -f -p=bob sloppy slop slop
-f used: false
-p's value: None
'slops' values: ["-f", "-p=bob", "sloppy", "slop", "slop"]

```
