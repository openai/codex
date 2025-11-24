walkdir
=======
A cross platform Rust library for efficiently walking a directory recursively.
Comes with support for following symbolic links, controlling the number of
open file descriptors and efficient mechanisms for pruning the entries in the
directory tree.

[![Build status](https://github.com/BurntSushi/walkdir/workflows/ci/badge.svg)](https://github.com/BurntSushi/walkdir/actions)
[![](https://meritbadge.herokuapp.com/walkdir)](https://crates.io/crates/walkdir)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).

### Documentation

[docs.rs/walkdir](https://docs.rs/walkdir/)

### Usage

To use this crate, add `walkdir` as a dependency to your project's
`Cargo.toml`:

```toml
[dependencies]
walkdir = "2"
```

### Example

The following code recursively iterates over the directory given and prints
the path for each entry:

```rust,no_run
use walkdir::WalkDir;

for entry in WalkDir::new("foo") {
    let entry = entry.unwrap();
    println!("{}", entry.path().display());
}
```

Or, if you'd like to iterate over all entries and ignore any errors that may
arise, use `filter_map`. (e.g., This code below will silently skip directories
that the owner of the running process does not have permission to access.)

```rust,no_run
use walkdir::WalkDir;

for entry in WalkDir::new("foo").into_iter().filter_map(|e| e.ok()) {
    println!("{}", entry.path().display());
}
```

### Example: follow symbolic links

The same code as above, except `follow_links` is enabled:

```rust,no_run
use walkdir::WalkDir;

for entry in WalkDir::new("foo").follow_links(true) {
    let entry = entry.unwrap();
    println!("{}", entry.path().display());
}
```

### Example: skip hidden files and directories efficiently on unix

This uses the `filter_entry` iterator adapter to avoid yielding hidden files
and directories efficiently:

```rust,no_run
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.starts_with("."))
         .unwrap_or(false)
}

let walker = WalkDir::new("foo").into_iter();
for entry in walker.filter_entry(|e| !is_hidden(e)) {
    let entry = entry.unwrap();
    println!("{}", entry.path().display());
}
```

### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.34.0`.

The current policy is that the minimum Rust version required to use this crate
can be increased in minor version updates. For example, if `crate 1.0` requires
Rust 1.20.0, then `crate 1.0.z` for all values of `z` will also require Rust
1.20.0 or newer. However, `crate 1.y` for `y > 0` may require a newer minimum
version of Rust.

In general, this crate will be conservative with respect to the minimum
supported version of Rust.

### Performance

The short story is that performance is comparable with `find` and glibc's
`nftw` on both a warm and cold file cache. In fact, I cannot observe any
performance difference after running `find /`, `walkdir /` and `nftw /` on my
local file system (SSD, ~3 million entries). More precisely, I am reasonably
confident that this crate makes as few system calls and close to as few
allocations as possible.

I haven't recorded any benchmarks, but here are some things you can try with a
local checkout of `walkdir`:

```sh
# The directory you want to recursively walk:
DIR=$HOME

# If you want to observe perf on a cold file cache, run this before *each*
# command:
sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'

# To warm the caches
find $DIR

# Test speed of `find` on warm cache:
time find $DIR

# Compile and test speed of `walkdir` crate:
cargo build --release --example walkdir
time ./target/release/examples/walkdir $DIR

# Compile and test speed of glibc's `nftw`:
gcc -O3 -o nftw ./compare/nftw.c
time ./nftw $DIR

# For shits and giggles, test speed of Python's (2 or 3) os.walk:
time python ./compare/walk.py $DIR
```

On my system, the performance of `walkdir`, `find` and `nftw` is comparable.
