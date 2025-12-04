# `tree-sitter version`

The `version` command upgrades the version of your grammar.

```bash
tree-sitter version <VERSION> # Aliases: publish
```

This will update the version in several files, if they exist:

* tree-sitter.json
* Cargo.toml
* Cargo.lock
* package.json
* package-lock.json
* Makefile
* CMakeLists.txt
* pyproject.toml

As a grammar author, you should keep the version of your grammar in sync across
different bindings. However, doing so manually is error-prone and tedious, so
this command takes care of the burden. If you are using a version control system,
it is recommended to commit the changes made by this command, and to tag the
commit with the new version.
