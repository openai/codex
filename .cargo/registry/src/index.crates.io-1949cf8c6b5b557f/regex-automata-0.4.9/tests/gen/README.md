This directory contains tests for serialized objects from the regex-automata
crate. Currently, there are only two supported such objects: dense and sparse
DFAs.

The idea behind these tests is to commit some serialized objects and run some
basic tests by deserializing them and running searches and ensuring they are
correct. We also make sure these are run under Miri, since deserialization is
one of the biggest places where undefined behavior might occur in this crate
(at the time of writing).

The main thing we're testing is that the *current* code can still deserialize
*old* objects correctly. Generally speaking, compatibility extends to semver
compatible releases of this crate. Beyond that, no promises are made, although
in practice callers can at least depend on errors occurring. (The serialized
format always includes a version number, and incompatible changes increment
that version number such that an error will occur if an unsupported version is
detected.)

To generate the dense DFAs, I used this command:

```
$ regex-cli generate serialize dense regex \
    MULTI_PATTERN_V2 \
    tests/gen/dense/ \
    --rustfmt \
    --safe \
    --starts-for-each-pattern \
    --specialize-start-states \
    --start-kind both \
    --unicode-word-boundary \
    --minimize \
    '\b[a-zA-Z]+\b' \
    '(?m)^\S+$' \
    '(?Rm)^\S+$'
```

And to generate the sparse DFAs, I used this command, which is the same as
above, but with `s/dense/sparse/g`.

```
$ regex-cli generate serialize sparse regex \
    MULTI_PATTERN_V2 \
    tests/gen/sparse/ \
    --rustfmt \
    --safe \
    --starts-for-each-pattern \
    --specialize-start-states \
    --start-kind both \
    --unicode-word-boundary \
    --minimize \
    '\b[a-zA-Z]+\b' \
    '(?m)^\S+$' \
    '(?Rm)^\S+$'
```

The idea is to try to enable as many of the DFA's options as possible in order
to test that serialization works for all of them.

Arguably we should increase test coverage here, but this is a start. Note
that in particular, this does not need to test that serialization and
deserialization correctly roundtrips on its own. Indeed, the normal regex test
suite has a test that does a serialization round trip for every test supported
by DFAs. So that has very good coverage. What we're interested in testing here
is our compatibility promise: do DFAs generated with an older revision of the
code still deserialize correctly?
