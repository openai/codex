# weezl

LZW en- and decoding that goes weeeee!

## Overview

This library, written in purely safe and dependency-less Rust, provides
encoding and decoding for lzw compression in the style as it occurs in `gif`
and `tiff` image formats. It has a standalone binary that may be used to handle
those data streams but it is _not_ compatible with Spencer's `compress` and
`uncompress` binaries (though a drop-in may be developed at a later point).

Using in a `no_std` environment is also possible though an allocator is
required. This, too, may be relaxed in a later release. A feature flag already
exists but currently turns off almost all interfaces.

## License

All code is dual licensed MIT OR Apache-2.0.
