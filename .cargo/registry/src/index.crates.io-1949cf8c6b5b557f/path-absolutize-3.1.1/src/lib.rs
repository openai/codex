/*!
# Path Absolutize

This is a library for extending `Path` and `PathBuf` in order to get an absolute path and remove the containing dots.

The difference between `absolutize` and `canonicalize` methods is that `absolutize` does not care about whether the file exists and what the file really is.

Please read the following examples to know the parsing rules.

## Examples

There are two methods you can use.

### absolutize

Get an absolute path.

The dots in a path will be parsed even if it is already an absolute path (which means the path starts with a `MAIN_SEPARATOR` on Unix-like systems).

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("/path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/path/to/123/456", p.absolutize().unwrap().to_str().unwrap());
# }
```

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("/path/to/./123/../456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/path/to/456", p.absolutize().unwrap().to_str().unwrap());
# }
```

If a path starts with a single dot, the dot means your program's **current working directory** (CWD).

```rust
use std::path::Path;
use std::env;

use path_absolutize::*;

let p = Path::new("./path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!(Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
# }
```

If a path starts with a pair of dots, the dots means the parent of the CWD. If the CWD is **root**, the parent is still **root**.

```rust
use std::path::Path;
use std::env;

use path_absolutize::*;

let p = Path::new("../path/to/123/456");

let cwd = env::current_dir().unwrap();

let cwd_parent = cwd.parent();

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
match cwd_parent {
   Some(cwd_parent) => {
       assert_eq!(Path::join(&cwd_parent, Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
   None => {
       assert_eq!(Path::join(Path::new("/"), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
}
# }
```

A path which does not start with a `MAIN_SEPARATOR`, **Single Dot** and **Double Dots**, will act like having a single dot at the start when `absolutize` method is used.

```rust
use std::path::Path;
use std::env;

use path_absolutize::*;

let p = Path::new("path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!(Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
# }
```

```rust
use std::path::Path;
use std::env;

use path_absolutize::*;

let p = Path::new("path/../../to/123/456");

let cwd = env::current_dir().unwrap();

let cwd_parent = cwd.parent();

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
match cwd_parent {
   Some(cwd_parent) => {
       assert_eq!(Path::join(&cwd_parent, Path::new("to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
   None => {
       assert_eq!(Path::join(Path::new("/"), Path::new("to/123/456")).to_str().unwrap(), p.absolutize().unwrap().to_str().unwrap());
   }
}
# }
```

### Starting from a given current working directory

With the `absolutize_from` function, you can provide the current working directory that the relative paths should be resolved from.

```rust
use std::env;
use std::path::Path;

use path_absolutize::*;

let p = Path::new("../path/to/123/456");
let cwd = env::current_dir().unwrap();

println!("{}", p.absolutize_from(cwd).unwrap().to_str().unwrap());
```

### absolutize_virtually

Get an absolute path **only under a specific directory**.

The dots in a path will be parsed even if it is already an absolute path (which means the path starts with a `MAIN_SEPARATOR` on Unix-like systems).

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("/path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/path/to/123/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
# }
```

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("/path/to/./123/../456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/path/to/456", p.absolutize_virtually("/").unwrap().to_str().unwrap());
# }
```

Every absolute path should under the virtual root.

```rust
use std::path::Path;

use std::io::ErrorKind;

use path_absolutize::*;

let p = Path::new("/path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
# }
```

Every relative path should under the virtual root.

```rust
use std::path::Path;

use std::io::ErrorKind;

use path_absolutize::*;

let p = Path::new("./path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
# }
```

```rust
use std::path::Path;

use std::io::ErrorKind;

use path_absolutize::*;

let p = Path::new("../path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!(ErrorKind::InvalidInput, p.absolutize_virtually("/virtual/root").unwrap_err().kind());
# }
```

A path which does not start with a `MAIN_SEPARATOR`, **Single Dot** and **Double Dots**, will be located in the virtual root after the `absolutize_virtually` method is used.

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("path/to/123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/virtual/root/path/to/123/456", p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap());
# }
```

```rust
use std::path::Path;

use path_absolutize::*;

let p = Path::new("path/to/../../../../123/456");

# if cfg!(unix) {
# #[cfg(feature = "unsafe_cache")]
# {
#     unsafe {
#         update_cwd();
#     }
# }
assert_eq!("/virtual/root/123/456", p.absolutize_virtually("/virtual/root").unwrap().to_str().unwrap());
# }
```

## Caching

By default, the `absolutize` method and the `absolutize_virtually` method create a new `PathBuf` instance of the CWD every time in their operation. The overhead is obvious. Although it allows us to safely change the CWD at runtime by the program itself (e.g. using the `std::env::set_current_dir` function) or outside controls (e.g. using gdb to call `chdir`), we don't need that in most cases.

In order to parse paths with better performance, this crate provides three ways to cache the CWD.

### once_cell_cache

Enabling the `once_cell_cache` feature can let this crate use `once_cell` to cache the CWD. It's thread-safe and does not need to modify any code, but once the CWD is cached, it cannot be changed anymore at runtime.

```toml
[dependencies.path-absolutize]
version = "*"
features = ["once_cell_cache"]
```

### lazy_static_cache

Enabling the `lazy_static_cache` feature can let this crate use `lazy_static` to cache the CWD. It's thread-safe and does not need to modify any code, but once the CWD is cached, it cannot be changed anymore at runtime.

```toml
[dependencies.path-absolutize]
version = "*"
features = ["lazy_static_cache"]
```

### unsafe_cache

Enabling the `unsafe_cache` feature can let this crate use a mutable static variable to cache the CWD. It allows the program to change the CWD at runtime by the program itself, but it's not thread-safe.

You need to use the `update_cwd` function to initialize the CWD first. The function should also be used to update the CWD after the CWD is changed.

```toml
[dependencies.path-absolutize]
version = "*"
features = ["unsafe_cache"]
```

```rust
use std::path::Path;

use path_absolutize::*;

# #[cfg(feature = "unsafe_cache")]
unsafe {
    update_cwd();
}

let p = Path::new("./path/to/123/456");

println!("{}", p.absolutize().unwrap().to_str().unwrap());

std::env::set_current_dir("/").unwrap();

# #[cfg(feature = "unsafe_cache")]
unsafe {
    update_cwd();
}

println!("{}", p.absolutize().unwrap().to_str().unwrap());
```

## Benchmark

#### No-cache

```bash
cargo bench
```

#### once_cell_cache

```bash
cargo bench --features once_cell_cache
```

#### lazy_static_cache

```bash
cargo bench --features lazy_static_cache
```

#### unsafe_cache

```bash
cargo bench --features unsafe_cache
```

*/

#[cfg(any(
    all(feature = "lazy_static_cache", feature = "unsafe_cache"),
    all(feature = "once_cell_cache", feature = "unsafe_cache"),
    all(feature = "lazy_static_cache", feature = "once_cell_cache")
))]
compile_error!("You can only enable at most one caching mechanism for `path-absolutize`.");

pub extern crate path_dedot;

use std::{
    borrow::Cow,
    io,
    path::{Path, PathBuf},
};

#[cfg(feature = "unsafe_cache")]
pub use path_dedot::update_cwd;
#[cfg(any(
    feature = "once_cell_cache",
    feature = "lazy_static_cache",
    feature = "unsafe_cache"
))]
pub use path_dedot::CWD;

mod absolutize;

#[macro_use]
mod macros;

#[cfg(any(unix, all(target_family = "wasm", feature = "use_unix_paths_on_wasm")))]
mod unix;

#[cfg(windows)]
mod windows;

pub use absolutize::*;

impl Absolutize for PathBuf {
    #[inline]
    fn absolutize(&self) -> io::Result<Cow<Path>> {
        self.as_path().absolutize()
    }

    #[inline]
    fn absolutize_from(&self, cwd: impl AsRef<Path>) -> io::Result<Cow<'_, Path>> {
        self.as_path().absolutize_from(cwd)
    }

    #[inline]
    fn absolutize_virtually(&self, virtual_root: impl AsRef<Path>) -> io::Result<Cow<Path>> {
        self.as_path().absolutize_virtually(virtual_root)
    }
}
