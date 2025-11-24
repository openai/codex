# `object`

The `object` crate provides a unified interface to working with object files
across platforms. It supports reading relocatable object files and executable files,
and writing COFF/ELF/Mach-O/XCOFF relocatable object files and ELF/PE executable files.

For reading files, it provides multiple levels of support:

* raw struct definitions suitable for zero copy access
* low level APIs for accessing the raw structs ([example](crates/examples/src/readobj/))
* a higher level unified API for accessing common features of object files, such
  as sections and symbols ([example](crates/examples/src/objdump.rs))

Supported file formats for reading: ELF, Mach-O, Windows PE/COFF, Wasm, XCOFF, and Unix archive.

For writing files, it provides:

* low level writers for ELF, PE, and COFF
* higher level builder for ELF ([example](crates/rewrite/src))
* a unified API for writing relocatable object files (ELF, Mach-O, COFF, XCOFF)
  ([example](crates/examples/src/bin/simple_write.rs))

## Example for unified read API
```rust
use object::{Object, ObjectSection};
use std::error::Error;
use std::fs;

/// Reads a file and displays the name of each section.
fn main() -> Result<(), Box<dyn Error>> {
    let binary_data = fs::read("path/to/binary")?;
    let file = object::File::parse(&*binary_data)?;
    for section in file.sections() {
        println!("{}", section.name()?);
    }
    Ok(())
}
```

See [`crates/examples`](crates/examples) for more examples.

## Minimum Supported Rust Version (MSRV)

Changes to MSRV are considered breaking changes. We are conservative about changing the MSRV,
but sometimes are required to due to dependencies. The MSRV is 1.65.0.

## License

Licensed under either of

  * Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([`LICENSE-MIT`](./LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
