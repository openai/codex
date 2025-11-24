This directory contains the results of compilation tests. Specifically,
the results are from testing both the from scratch compilation time and
relative binary size increases of various features for both the `regex` and
`regex-automata` crates.

Here's an example of how to run these tests for just the `regex` crate. You'll
need the `regex-cli` command installed, which can be found in the `regex-cli`
directory in the root of this repository.

This must be run in the root of a checkout of this repository.

```
$ mkdir /tmp/regex-compile-test
$ regex-cli compile-test ./ /tmp/regex-compile-test | tee record/compile-test/2023-04-19_1.7.3.csv
```

You can then look at the results using a tool like [`xsv`][xsv]:

```
$ xsv table record/compile-test/2023-04-19_1.7.3.csv
```

Note that the relative binary size is computed by building a "baseline" hello
world program, and then subtracting that from the size of a binary that uses
the regex crate.

[xsv]: https://github.com/BurntSushi/xsv
