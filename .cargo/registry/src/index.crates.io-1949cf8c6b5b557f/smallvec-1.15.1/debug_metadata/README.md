## Debugger Visualizers

Many languages and debuggers enable developers to control how a type is
displayed in a debugger. These are called "debugger visualizations" or "debugger
views".

The Windows debuggers (WinDbg\CDB) support defining custom debugger visualizations using
the `Natvis` framework. To use Natvis, developers write XML documents using the natvis
schema that describe how debugger types should be displayed with the `.natvis` extension.
(See: https://docs.microsoft.com/en-us/visualstudio/debugger/create-custom-views-of-native-objects?view=vs-2019)
The Natvis files provide patterns which match type names a description of how to display
those types.

The Natvis schema can be found either online (See: https://code.visualstudio.com/docs/cpp/natvis#_schema)
or locally at `<VS Installation Folder>\Xml\Schemas\1033\natvis.xsd`.

The GNU debugger (GDB) supports defining custom debugger views using Pretty Printers.
Pretty printers are written as python scripts that describe how a type should be displayed
when loaded up in GDB/LLDB. (See: https://sourceware.org/gdb/onlinedocs/gdb/Pretty-Printing.html#Pretty-Printing)
The pretty printers provide patterns, which match type names, and for matching
types, describe how to display those types. (For writing a pretty printer, see: https://sourceware.org/gdb/onlinedocs/gdb/Writing-a-Pretty_002dPrinter.html#Writing-a-Pretty_002dPrinter).

### Embedding Visualizers

Through the use of the currently unstable `#[debugger_visualizer]` attribute, the `smallvec`
crate can embed debugger visualizers into the crate metadata.

Currently the two types of visualizers supported are Natvis and Pretty printers.

For Natvis files, when linking an executable with a crate that includes Natvis files,
the MSVC linker will embed the contents of all Natvis files into the generated `PDB`.

For pretty printers, the compiler will encode the contents of the pretty printer
in the `.debug_gdb_scripts` section of the `ELF` generated.

### Testing Visualizers

The `smallvec` crate supports testing debugger visualizers defined for this crate. The entry point for
these tests are `tests/debugger_visualizer.rs`. These tests are defined using the `debugger_test` and
`debugger_test_parser` crates. The `debugger_test` crate is a proc macro crate which defines a
single proc macro attribute, `#[debugger_test]`. For more detailed information about this crate,
see https://crates.io/crates/debugger_test. The CI pipeline for the `smallvec` crate has been updated
to run the debugger visualizer tests to ensure debugger visualizers do not become broken/stale.

The `#[debugger_test]` proc macro attribute may only be used on test functions and will run the
function under the debugger specified by the `debugger` meta item.

This proc macro attribute has 3 required values:

1. The first required meta item, `debugger`, takes a string value which specifies the debugger to launch.
2. The second required meta item, `commands`, takes a string of new line (`\n`) separated list of debugger
commands to run.
3. The third required meta item, `expected_statements`, takes a string of new line (`\n`) separated list of
statements that must exist in the debugger output. Pattern matching through regular expressions is also
supported by using the `pattern:` prefix for each expected statement.

#### Example:

```rust
#[debugger_test(
    debugger = "cdb",
    commands = "command1\ncommand2\ncommand3",
    expected_statements = "statement1\nstatement2\nstatement3")]
fn test() {

}
```

Using a multiline string is also supported, with a single debugger command/expected statement per line:

```rust
#[debugger_test(
    debugger = "cdb",
    commands = "
command1
command2
command3",
    expected_statements = "
statement1
pattern:statement[0-9]+
statement3")]
fn test() {
    
}
```

In the example above, the second expected statement uses pattern matching through a regular expression
by using the `pattern:` prefix.

#### Testing Locally

Currently, only Natvis visualizations have been defined for the `smallvec` crate via `debug_metadata/smallvec.natvis`,
which means the `tests/debugger_visualizer.rs` tests need to be run on Windows using the `*-pc-windows-msvc` targets.
To run these tests locally, first ensure the debugging tools for Windows are installed or install them following
the steps listed here, [Debugging Tools for Windows](https://docs.microsoft.com/en-us/windows-hardware/drivers/debugger/).
Once the debugging tools have been installed, the tests can be run in the same manner as they are in the CI
pipeline.

#### Note

When running the debugger visualizer tests, `tests/debugger_visualizer.rs`, they need to be run consecutively
and not in parallel. This can be achieved by passing the flag `--test-threads=1` to rustc. This is due to
how the debugger tests are run. Each test marked with the `#[debugger_test]` attribute launches a debugger
and attaches it to the current test process. If tests are running in parallel, the test will try to attach
a debugger to the current process which may already have a debugger attached causing the test to fail.

For example:

```
cargo test --test debugger_visualizer --features debugger_visualizer -- --test-threads=1
```
