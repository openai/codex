**This requires enabling the [`derive` feature flag][crate::_features].**

Git is an example of several common subcommand patterns.

Help:
```console
$ git-derive
? failed
A fictional versioning CLI

Usage: git-derive[EXE] <COMMAND>

Commands:
  clone  Clones repos
  diff   Compare two commits
  push   pushes things
  add    adds things
  stash  
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ git-derive help
A fictional versioning CLI

Usage: git-derive[EXE] <COMMAND>

Commands:
  clone  Clones repos
  diff   Compare two commits
  push   pushes things
  add    adds things
  stash  
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ git-derive help add
adds things

Usage: git-derive[EXE] add <PATH>...

Arguments:
  <PATH>...  Stuff to add

Options:
  -h, --help  Print help

```

A basic argument:
```console
$ git-derive add
? failed
adds things

Usage: git-derive[EXE] add <PATH>...

Arguments:
  <PATH>...  Stuff to add

Options:
  -h, --help  Print help

$ git-derive add Cargo.toml Cargo.lock
Adding ["Cargo.toml", "Cargo.lock"]

```

Default subcommand:
```console
$ git-derive stash -h
Usage: git-derive[EXE] stash [OPTIONS]
       git-derive[EXE] stash push [OPTIONS]
       git-derive[EXE] stash pop [STASH]
       git-derive[EXE] stash apply [STASH]
       git-derive[EXE] stash help [COMMAND]...

Options:
  -m, --message <MESSAGE>  
  -h, --help               Print help

git-derive[EXE] stash push:
  -m, --message <MESSAGE>  
  -h, --help               Print help

git-derive[EXE] stash pop:
  -h, --help  Print help
  [STASH]     

git-derive[EXE] stash apply:
  -h, --help  Print help
  [STASH]     

git-derive[EXE] stash help:
Print this message or the help of the given subcommand(s)
  [COMMAND]...  Print help for the subcommand(s)

$ git-derive stash push -h
Usage: git-derive[EXE] stash push [OPTIONS]

Options:
  -m, --message <MESSAGE>  
  -h, --help               Print help

$ git-derive stash pop -h
Usage: git-derive[EXE] stash pop [STASH]

Arguments:
  [STASH]  

Options:
  -h, --help  Print help

$ git-derive stash -m "Prototype"
Pushing StashPushArgs { message: Some("Prototype") }

$ git-derive stash pop
Popping None

$ git-derive stash push -m "Prototype"
Pushing StashPushArgs { message: Some("Prototype") }

$ git-derive stash pop
Popping None

```

External subcommands:
```console
$ git-derive custom-tool arg1 --foo bar
Calling out to "custom-tool" with ["arg1", "--foo", "bar"]

```

Last argument:
```console
$ git-derive diff --help
Compare two commits

Usage: git-derive[EXE] diff [OPTIONS] [COMMIT] [COMMIT] [-- <PATH>]

Arguments:
  [COMMIT]  
  [COMMIT]  
  [PATH]    

Options:
      --color[=<WHEN>]  [default: auto] [possible values: always, auto, never]
  -h, --help            Print help

$ git-derive diff
Diffing stage..worktree  (color=auto)

$ git-derive diff ./src
Diffing stage..worktree ./src (color=auto)

$ git-derive diff HEAD ./src
Diffing HEAD..worktree ./src (color=auto)

$ git-derive diff HEAD~~ -- HEAD
Diffing HEAD~~..worktree HEAD (color=auto)

$ git-derive diff --color
Diffing stage..worktree  (color=always)

$ git-derive diff --color=never
Diffing stage..worktree  (color=never)

```
