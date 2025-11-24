* **[Latest Docs.rs Here](https://docs.rs/bytemuck/)**

[![License:Zlib](https://img.shields.io/badge/License-Zlib-brightgreen.svg)](https://opensource.org/licenses/Zlib)
![Minimum Rust Version](https://img.shields.io/badge/Min%20Rust-1.34-green.svg)
[![crates.io](https://img.shields.io/crates/v/bytemuck.svg)](https://crates.io/crates/bytemuck)

# bytemuck

A crate for mucking around with piles of bytes.

This crate lets you safely perform "bit cast" operations between data types.
That's where you take a value and just reinterpret the bits as being some other
type of value, without changing the bits.

* This is **not** like the [`as` keyword][keyword-as]
* This is **not** like the [`From` trait][from-trait]
* It is **most like** [`f32::to_bits`][f32-to_bits], just generalized to let you
  convert between all sorts of data types.

[keyword-as]: https://doc.rust-lang.org/nightly/std/keyword.as.html
[from-trait]: https://doc.rust-lang.org/nightly/core/convert/trait.From.html
[f32-to_bits]: https://doc.rust-lang.org/nightly/std/primitive.f32.html#method.to_bits

### Here's the part you're more likely to care about: *you can do this with slices too!*

When a slice is involved it's not a *direct* bitcast. Instead, the `cast_slice`
and `cast_slice_mut` functions will pull apart a slice's data and give you a new
slice that's the same span of memory just viewed as the new type. If the size of
the slice's element changes then the length of the slice you get back will be
changed accordingly.

This lets you cast a slice of color values into a slice of `u8` and send it to
the GPU, or things like that. I'm sure there's other examples, but honestly this
crate is as popular as it is mostly because of Rust's 3D graphics community
wanting to cast slices of different types into byte slices for sending to the
GPU. Hi friends! Push those vertices, or whatever it is that you all do.

## See Also

While `bytemuck` is full of unsafe code, I've also started a "sibling crate"
called [bitfrob](https://docs.rs/bitfrob/latest/bitfrob/), which is where
operations that are 100% safe will be added.

## Stability

* The crate is 1.0 and I consider this it to be "basically done". New features
  are usually being accepted when other people want to put in the work, but
  myself I wanna move on to using `bytemuck` in bigger projects.
* The default build of the `bytemuck` crate will continue to work with `rustc-1.34`
  for at least the rest of the `1.y.z` versions.
* Any other cargo features of the crate **are not** held to the same standard, and
  may work only on the latest Stable or even only on latest Nightly.

**Future Plans:** Once the [Safe Transmute Project][pg-st] completes and
stabilizes ("eventually") this crate will be updated to use that as the
underlying mechanism for transmutation bounds, and a 2.0 version of `bytemuck`
will be released. The hope is for the 1.0 to 2.0 transition to be as seamless as
possible, but the future is always uncertain.

[pg-st]: https://rust-lang.github.io/rfcs/2835-project-safe-transmute.html
