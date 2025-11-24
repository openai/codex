Path Dedot
====================

[![CI](https://github.com/magiclen/path-dedot/actions/workflows/ci.yml/badge.svg)](https://github.com/magiclen/path-dedot/actions/workflows/ci.yml)

This is a library for extending `Path` and `PathBuf` in order to parse the path which contains dots.

Please read the following examples to know the parsing rules.

## Examples

If a path starts with a single dot, the dot means your program's **current working directory** (CWD).

```rust
use std::path::Path;
use std::env;

use path_dedot::*;

let p = Path::new("./path/to/123/456");

assert_eq!(Path::join(env::current_dir().unwrap().as_path(), Path::new("path/to/123/456")).to_str().unwrap(), p.parse_dot().unwrap().to_str().unwrap());
```

If a path starts with a pair of dots, the dots means the parent of the CWD. If the CWD is **root**, the parent is still **root**.

```rust
use std::path::Path;
use std::env;

use path_dedot::*;

let p = Path::new("../path/to/123/456");

let cwd = env::current_dir().unwrap();

let cwd_parent = cwd.parent();

match cwd_parent {
   Some(cwd_parent) => {
      assert_eq!(Path::join(&cwd_parent, Path::new("path/to/123/456")).to_str().unwrap(), p.parse_dot().unwrap().to_str().unwrap());
   }
   None => {
      assert_eq!(Path::join(Path::new("/"), Path::new("path/to/123/456")).to_str().unwrap(), p.parse_dot().unwrap().to_str().unwrap());
   }
}
```

In addition to starting with, the **Single Dot** and **Double Dots** can also be placed to other positions. **Single Dot** means noting and will be ignored. **Double Dots** means the parent.

```rust
use std::path::Path;

use path_dedot::*;

let p = Path::new("/path/to/../123/456/./777");

assert_eq!("/path/123/456/777", p.parse_dot().unwrap().to_str().unwrap());
```

```rust
use std::path::Path;

use path_dedot::*;

let p = Path::new("/path/to/../123/456/./777/..");

assert_eq!("/path/123/456", p.parse_dot().unwrap().to_str().unwrap());
```

You should notice that `parse_dot` method does **not** aim to get an **absolute path**. A path which does not start with a `MAIN_SEPARATOR`, **Single Dot** and **Double Dots**, will not have each of them after the `parse_dot` method is used.

```rust
use std::path::Path;

use path_dedot::*;

let p = Path::new("path/to/../123/456/./777/..");

assert_eq!("path/123/456", p.parse_dot().unwrap().to_str().unwrap());
```

**Double Dots** which is not placed at the start cannot get the parent beyond the original path. Why not? With this constraint, you can insert an absolute path to the start as a virtual root in order to protect your file system from being exposed.

```rust
use std::path::Path;

use path_dedot::*;

let p = Path::new("path/to/../../../../123/456/./777/..");

assert_eq!("123/456", p.parse_dot().unwrap().to_str().unwrap());
```

```rust
use std::path::Path;

use path_dedot::*;

let p = Path::new("/path/to/../../../../123/456/./777/..");

assert_eq!("/123/456", p.parse_dot().unwrap().to_str().unwrap());
```

### Starting from a given current working directory

With the `parse_dot_from` function, you can provide the current working directory that the relative paths should be resolved from.

```rust
use std::env;
use std::path::Path;

use path_dedot::*;

let p = Path::new("../path/to/123/456");
let cwd = env::current_dir().unwrap();

println!("{}", p.parse_dot_from(cwd).unwrap().to_str().unwrap());
```

## Caching

By default, the `parse_dot` method creates a new `PathBuf` instance of the CWD every time in its operation. The overhead is obvious. Although it allows us to safely change the CWD at runtime by the program itself (e.g. using the `std::env::set_current_dir` function) or outside controls (e.g. using gdb to call `chdir`), we don't need that in most cases.

In order to parse paths with better performance, this crate provides three ways to cache the CWD.

### once_cell_cache

Enabling the `once_cell_cache` feature can let this crate use `once_cell` to cache the CWD. It's thread-safe and does not need to modify any code, but once the CWD is cached, it cannot be changed anymore at runtime.

```toml
[dependencies.path-dedot]
version = "*"
features = ["once_cell_cache"]
```

### lazy_static_cache

Enabling the `lazy_static_cache` feature can let this crate use `lazy_static` to cache the CWD. It's thread-safe and does not need to modify any code, but once the CWD is cached, it cannot be changed anymore at runtime.

```toml
[dependencies.path-dedot]
version = "*"
features = ["lazy_static_cache"]
```

### unsafe_cache

Enabling the `unsafe_cache` feature can let this crate use a mutable static variable to cache the CWD. It allows the program to change the CWD at runtime by the program itself, but it's not thread-safe.

You need to use the `update_cwd` function to initialize the CWD first. The function should also be used to update the CWD after the CWD is changed.

```toml
[dependencies.path-dedot]
version = "*"
features = ["unsafe_cache"]
```

```rust
use std::path::Path;

use path_dedot::*;

unsafe {
    update_cwd();
}

let p = Path::new("./path/to/123/456");

println!("{}", p.parse_dot().unwrap().to_str().unwrap());

std::env::set_current_dir("/").unwrap();

unsafe {
    update_cwd();
}

println!("{}", p.parse_dot().unwrap().to_str().unwrap());
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

## Crates.io

https://crates.io/crates/path-dedot

## Documentation

https://docs.rs/path-dedot

## License

[MIT](LICENSE)