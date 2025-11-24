# Updating compile-fail test outputs

`bitflags` uses the `trybuild` crate to integration test its macros. Since Rust error messages change frequently enough that `nightly` builds produce spurious failures, we only check the compiler output in `beta` builds. If you run:

```
TRYBUILD=overwrite cargo +beta test --all
```

it will run the tests and update the `trybuild` output files.
