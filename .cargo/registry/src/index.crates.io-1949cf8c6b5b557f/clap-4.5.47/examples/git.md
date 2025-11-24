Git is an example of several common subcommand patterns.

Help:
```console
$ git
? failed
A fictional versioning CLI

Usage: git[EXE] <COMMAND>

Commands:
  clone  Clones repos
  diff   Compare two commits
  push   pushes things
  add    adds things
  stash  
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ git help
A fictional versioning CLI

Usage: git[EXE] <COMMAND>

Commands:
  clone  Clones repos
  diff   Compare two commits
  push   pushes things
  add    adds things
  stash  
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

$ git help add
adds things

Usage: git[EXE] add <PATH>...

Arguments:
  <PATH>...  Stuff to add

Options:
  -h, --help  Print help

```

A basic argument:
```console
$ git add
? failed
adds things

Usage: git[EXE] add <PATH>...

Arguments:
  <PATH>...  Stuff to add

Options:
  -h, --help  Print help

$ git add Cargo.toml Cargo.lock
Adding ["Cargo.toml", "Cargo.lock"]

```

Default subcommand:
```console
$ git stash -h
Usage: git[EXE] stash [OPTIONS]
       git[EXE] stash push [OPTIONS]
       git[EXE] stash pop [STASH]
       git[EXE] stash apply [STASH]
       git[EXE] stash help [COMMAND]...

Options:
  -m, --message <MESSAGE>  
  -h, --help               Print help

git[EXE] stash push:
  -m, --message <MESSAGE>  
  -h, --help               Print help

git[EXE] stash pop:
  -h, --help  Print help
  [STASH]     

git[EXE] stash apply:
  -h, --help  Print help
  [STASH]     

git[EXE] stash help:
Print this message or the help of the given subcommand(s)
  [COMMAND]...  Print help for the subcommand(s)

$ git stash push -h
Usage: git[EXE] stash push [OPTIONS]

Options:
  -m, --message <MESSAGE>  
  -h, --help               Print help

$ git stash pop -h
Usage: git[EXE] stash pop [STASH]

Arguments:
  [STASH]  

Options:
  -h, --help  Print help

$ git stash -m "Prototype"
Pushing Some("Prototype")

$ git stash pop
Popping None

$ git stash push -m "Prototype"
Pushing Some("Prototype")

$ git stash pop
Popping None

```

External subcommands:
```console
$ git custom-tool arg1 --foo bar
Calling out to "custom-tool" with ["arg1", "--foo", "bar"]

```

Last argument:
```console
$ git diff --help
Compare two commits

Usage: git[EXE] diff [OPTIONS] [COMMIT] [COMMIT] [-- <PATH>]

Arguments:
  [COMMIT]  
  [COMMIT]  
  [PATH]    

Options:
      --color[=<WHEN>]  [default: auto] [possible values: always, auto, never]
  -h, --help            Print help

$ git diff
Diffing stage..worktree  (color=auto)

$ git diff ./src
Diffing stage..worktree ./src (color=auto)

$ git diff HEAD ./src
Diffing HEAD..worktree ./src (color=auto)

$ git diff HEAD~~ -- HEAD
Diffing HEAD~~..worktree HEAD (color=auto)

$ git diff --color
Diffing stage..worktree  (color=always)

$ git diff --color=never
Diffing stage..worktree  (color=never)

```
