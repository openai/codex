# diff.rs

> An LCS based slice and string diffing implementation.

## Install

```toml
[dependencies]
diff = "0.1"
```

## Example

```rust
extern crate diff;

fn main() {
    let left = "foo\nbar\nbaz\nquux";
    let right = "foo\nbaz\nbar\nquux";

    for diff in diff::lines(left, right) {
        match diff {
            diff::Result::Left(l)    => println!("-{}", l),
            diff::Result::Both(l, _) => println!(" {}", l),
            diff::Result::Right(r)   => println!("+{}", r)
        }
    }
}
```

prints

```
 foo
-bar
 baz
+bar
 quux
```

## License

`diff` is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See LICENSE-APACHE, and LICENSE-MIT for details.
