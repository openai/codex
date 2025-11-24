//! A crate with utilities to get and set the system's host name.
//!
//! ## Examples
#![cfg_attr(
    feature = "set",
    doc = r#"
Set and get the host name:
```rust,no_run
# use std::io;
# use std::ffi::OsStr;
# fn main() -> io::Result<()> {
hostname::set("potato")?;
let new_name = hostname::get()?;
assert_eq!(new_name, OsStr::new("potato"));
# Ok(())
# }
```
"#
)]
#![cfg_attr(
    not(feature = "set"),
    doc = r#"
Get the host name:
```rust,no_run
# use std::io;
# use std::ffi::OsStr;
# fn main() -> io::Result<()> {
let name = hostname::get()?;
println!("{:?}", name);
# Ok(())
# }
```
"#
)]
#![doc(html_root_url = "https://docs.rs/hostname/0.4.1")]
#![deny(
    unused,
    unused_imports,
    unused_features,
    bare_trait_objects,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    dead_code,
    deprecated,
    rust_2018_idioms,
    trivial_casts,
    unused_import_braces,
    unused_results
)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use cfg_if::cfg_if;

#[cfg(feature = "set")]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;

cfg_if! {
    if #[cfg(any(unix, target_os = "redox"))] {
        mod nix;
        use crate::nix as sys;
    } else if #[cfg(target_os = "windows")] {
        mod windows;
        use crate::windows as sys;
    } else {
        compile_error!("Unsupported target OS! Create an issue: https://github.com/svartalf/hostname/issues/new");
    }
}

/// Return the system hostname.
///
/// ## Example
///
/// ```rust
/// # use std::io;
/// # fn main() -> io::Result<()> {
/// let name = hostname::get()?;
/// # Ok(())
/// # }
/// ```
///
/// ## Errors
///
/// If this function encounters any form of error, an error
/// variant will be returned; in practice it is rare to be happen.
pub fn get() -> io::Result<OsString> {
    sys::get()
}

/// Set the system hostname.
///
/// This function is available only with `set` feature enabled (**disabled** by
/// default).
#[cfg_attr(
    feature = "set",
    doc = r#"
## Example

```rust,no_run
# use std::io;
# fn main() -> io::Result<()> {
hostname::set("potato")?;
# Ok(())
# }
```
"#
)]
/// ## Errors
///
/// In order to set new hostname, caller might need
/// to have the corresponding privilege
/// (`CAP_SYS_ADMIN` capability for Linux, administrator privileges for Windows,
/// and so on).\
/// An error variant will be returned if this function
/// will encounter a permission error or any other form of error.
///
/// ## Compatibility
///
/// * Will fail with a linkage error for Android API < 23 (see [#9](https://github.com/svartalf/hostname/issues/9#issuecomment-563991112))
#[cfg(feature = "set")]
#[cfg_attr(docsrs, doc(cfg(feature = "set")))]
pub fn set<T>(hostname: T) -> io::Result<()>
where
    T: AsRef<OsStr>,
{
    sys::set(hostname.as_ref())
}
