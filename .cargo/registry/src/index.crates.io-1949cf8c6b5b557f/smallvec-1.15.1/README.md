rust-smallvec
=============

[Documentation](https://docs.rs/smallvec/)

[Release notes](https://github.com/servo/rust-smallvec/releases)

"Small vector" optimization for Rust: store up to a small number of items on the stack

## Example

```rust
use smallvec::{SmallVec, smallvec};
    
// This SmallVec can hold up to 4 items on the stack:
let mut v: SmallVec<[i32; 4]> = smallvec![1, 2, 3, 4];

// It will automatically move its contents to the heap if
// contains more than four items:
v.push(5);

// SmallVec points to a slice, so you can use normal slice
// indexing and other methods to access its contents:
v[0] = v[1] + v[2];
v.sort();
```
