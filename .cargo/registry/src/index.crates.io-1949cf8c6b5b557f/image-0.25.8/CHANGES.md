# Release Notes

## Known issues
- Many decoders will panic on malicious input.
- The color space information of pixels is not clearly communicated.

## Changes

### Version 0.25.8

Re-release of `0.25.7`

Fixes:
- Reverted a signature change to `load_from_memory` that lead to large scale
  type inference breakage despite being technically compatible.
- Color conversion Luma to Rgb used incorrect coefficients instead of broadcasting.

### Version 0.25.7 (yanked)

Features:
  - Added an API for external image format implementations to register themselves as decoders for a specific format in `image` (#2372)
  - Added [CICP](https://www.color.org/iccmax/download/CICP_tag_and_type_amendment.pdf) awarenes via [moxcms](https://crates.io/crates/moxcms) to support color spaces (#2531). The support for transforming is limited for now and will be gradually expanded.
  - You can now embed Exif metadata when writing JPEG, PNG and WebP images (#2537, #2539)
  - Added functions to extract orientation from Exif metadata and optionally clear it in the Exif chunk (#2484)
  - Serde support for more types (#2445)
  - PNM encoder now supports writing 16-bit images (#2431)

API improvements:
  - `save`, `save_with_format`, `write_to` and `write_with_encoder` methods on `DynamicImage` now automatically convert the pixel format when necessary instead of returning an error (#2501)
  - Added `DynamicImage::has_alpha()` convenience method
  - Implemented `TryFrom<ExtendedColorType>` for `ColorType` (#2444)
  - Added `const HAS_ALPHA` to trait `Pixel`
  - Unified the error for unsupported encoder colors (#2543)
  - Added a `hooks` module to customize builtin behavior, `register_format_detection_hook` and  `register_decoding_hook` for the determining format of a file and selecting an `ImageDecoder` implementation respectively. (#2372)

Performance improvements:
  - Gaussian blur (#2496) and box blur (#2515) are now faster
  - Improve compilation times by avoiding unnecessary instantiation of generic functions (#2468, #2470)

Bug fixes:
  - Many improvements to image format decoding: TIFF, WebP, AVIF, PNG, GIF, BMP, TGA
  - Fixed `GifEncoder::encode()` ignoring the speed parameter and always using the slowest speed (#2504)
  - `.pnm` is now recognized as a file extension for the PNM format (#2559)


### Version 0.25.6

Features:
  - Improved format detection ([#2418](https://github.com/image-rs/image/pull/2418))
  - Implement writing ICC profiles for JPEG and PNG images ([#2389](https://github.com/image-rs/image/pull/2389))

Bug fixes:
  - JPEG encoding bugfix ([#2387](https://github.com/image-rs/image/pull/2387))
  - Expanded ICO format detection ([#2434](https://github.com/image-rs/image/pull/2434))
  - Fixed EXR bug with NaNs ([#2381](https://github.com/image-rs/image/pull/2381))
  - Various documentation improvements

### Version 0.25.5

Features:
 - Added support for decoding 10-bit and 12-bit AVIF
 - Initial, opt-in serde support for an enum. This may be extended to other types in the future.

Bug fixes:
 - [Multiple bug fixes in AVIF decoding](https://github.com/image-rs/image/pull/2373)
 - The `rayon` feature now correctly toggles the use of `rayon` when encoding AVIF. (Previously it would be either always on or always off depending on the version of the `ravif` crate in your dependency tree.)
 - "jfif" file extension for JPEG images is now recognized

### Version 0.25.4

Features:
 - Much faster decoding of lossless WebP due to a variety of optimizations. Our benchmarks show 2x to 2.5x improvement.
 - Added support for orientation metadata, so that e.g. smartphone camera images could be displayed correctly:
   - Added `ImageDecoder::orientation()` and implemented orientation metadata extraction for JPEG, WebP and TIFF formats
   - Added `DynamicImage::apply_orientation()` to apply the orientation to an image
 - Added support for extracting Exif metadata from images via `ImageDecoder::exif_metadata()`, and implemented it for JPEG and WebP formats
 - Added `ImageEncoder::set_icc_profile()` and implemented it for WebP format. Pull requests with implementations for other formats are welcome.
 - Added `DynamicImage::fast_blur()` for a linear-time approximation of Gaussian blur, which is much faster at larger blur radii

Bug fixes:
 - Fixed some APNG images being decoded incorrectly
 - Fixed the iterator over animated WebP frames to return `None` instead of an error when the end of the animation is reached

### Version 0.25.3

Yanked! This version accidentally missed a commit that should have been
included with the release. The `Orientation` struct should be in the
appropriate module instead of the top-level. This release won't be supported.

### Version 0.25.2

Features:
- Added the HDR encoder to supported formats in generic write methods with the
  `hdr` feature enabled. Supports 32-bit float RGB color only, for now.
- When cloning `ImageBuffer`, `DynamicImage` and `Frame` the existing buffer
  will now be reused if possible.
- Added `image::ImageReader` as an alias.
- Implement `ImageEncoder` for `HdrEncoder`.

Structural changes
- Switch from `byteorder` to `byteorder-lite`, consolidating some casting
  unsafety to `bytemuck`.
- Many methods on `DynamicImage` and buffers gained `#[must_use]` indications.

Bug fixes:
- Removed test data included in the crate archive.
- The WebP animation decoder stops when reaching the indicate frame count.
- Fixed bugs in the `bmp` decoder.
- Format support gated on the `exr` feature now compiles in isolation.

### Version 0.25.1

Bug fixes:
- Fixed corrupt JPEG output when attempting to encode images containing an alpha
  channel.
- Only accept ".ff" file extension for farbfeld images.
- Correct farbfeld feature flag for `ImageFormat::{reading_enabled, writing_enabled}`.
- Disable strict mode for JPEG decoder.
- Add nasm feature to optionally enable faster AVIF encoding.


### Version 0.25.0

Breaking changes:
- Added `BufRead` + `Seek` bound on many decoders.
- Use `ExtendedColorType` instead of `ColorType` when encoding.
- Removed `ImageOutputFormat`, `GenericImageView::bounds`, and several other
  deprecated items.
- Removed incremental decoding support and changed `ImageDecoder` so the trait
  is object safe.
- Pixel types are now `repr(transparent)` rather than `repr(C)`.
- Made color_quant dependency optional.
- Renamed some feature flags.

Structural changes:
- Increased MSRV to 1.67.1

Codec changes:
- Switched to image-webp for WebP encoding.
- Switched to zune-jpeg for JPEG decoding.
- Made the HDR decoder produce f32 images.
- Removed DXT encoding and decoding support.

### Version 0.24.9

Structural changes:
- Relicense to MIT OR Apache-2.0
- Increase MSRV 1.63.0

New features:
- Support limits in PNG animation decoding.
- Added offsets to SubImage to compensate for the now-deprecated bounds call
  from GenericImageView.

Bug fixes:
- Correct limit tests for TIFF.
- Avoid overflow in gif::Decoder::buffer_size.
- Return error instead of using assertions for Avif decoder unsupported or
  invalid bit depth.

### Version 0.24.8

New features:
- Added pure-Rust lossless WebP encoding.
- Added `DynamicImage::new` method.
- Added `PngDecoder::gamma_value` method.
- Added `ImageFormat::{reading_enabled, writing_enabled, all}`.
- TGA encoder now supports RLE encoding.
- Add rayon parallel iterators behind an optional `rayon` feature.
- Support CMYK TIFF images.
- Implement `From<DynamicImage>` for all image types.

Bug fixes:
- Fix decoding pngs with invalid text chunks.
- Handle non-fatal error dav1d::Error::Again.
- Do not round floats in interpolate.
- PNM decoder now scales samples according to specified maximum.
- Fix wrong implementation of unsharpen filter.
- Fix `GifDecoder::with_limits` to raise an error when limits are exceeded.

### Version 0.24.7

New features:
- Added `{ImageBuffer, DynamicImage}::write_with_encoder` to simplify writing
  images with custom settings.
- Expose ICC profiles stored in tiff and webp files.
- Added option to set the background color of animated webp images.
- New methods for sampling and interpolation of `GenericImageView`s

Bug fixes:
- Fix panic on empty dxt.
- Fix several panics in webp decoder.
- Allow unknown chunks at the end of webp files.

### Version 0.24.6

- Add support for QOI.
- ImageDecoders now expose ICC profiles on supported formats.
- Add support for BMPs without a file header.
- Improved AVIF encoder.
- WebP decoding fixes.

### Version 0.24.5

Structural changes:
- Increased the minimum supported Rust version (MSRV) to 1.61.
- Increased the version requirement for the `tiff` crate to 0.8.0.
- Increased the version requirement for the `jpeg` crate to 0.3.0.

Bug fixes:
- The `as_rgb32f` function of `DynamicImage` is now correctly documented.
- Fixed a crash when decoding ICO images. Added a regression test.
- Fixed a panic when transforming webp images. Added a regression test.
- Added a check to prevent integer overflow when calculating file size for BMP
  images. The missing check could panic in debug mode or else set an incorrect
  file size in release mode.
- Upgraded the PNG image encoder to use the newer `PngEncoder::write_image`
  instead of the deprecated `PngEncoder::encode` which did not account for byte
  order and could result in images with incorrect colors.
- Fixed `InsufficientMemory` error when trying to decode a PNG image.
- Fix warnings and CI issues.
- Typos and links in the documentation have been corrected.

Performance:
- Added check for dynamic image dimensions before resizing. This improves
  performance in cases where the image does not need to be resized or has
  already been resized.

### Version 0.24.4

New Features:
- Encoding for `webp` is now available with the native library. This needs to
  be activate explicitly with the `web-encoder` feature.
- `exr` decoding has gained basic limit support.

Bug fixes:
- The `Iterator::size_hint` implementation of pixel iterators has been fixed to
  return the current length indicated by its `ExactSizeIterator` hint.
- Typos and bad references in the documentation have been removed.

Performance:
- `ImageBuffer::get_pixel{,_mut}` is now marked inline.
- `resize` now short-circuits when image dimensions are unchanged.

### Version 0.24.3

New Features:
- `TiffDecoder` now supports setting resource limits.

Bug fixes:
- Fix compile issues on little endian systems.
- Various panics discovered by fuzzing.

### Version 0.24.2

Structural changes:
- CI now runs `cargo-deny`, checking dependent crates to an OSS license list
  and against RUSTSEC advisories.

New Features:
- The WebP decoder recognizes and decodes images with `VP8X` header.
- The DDS decoder recognizes and decodes images with `DX10` headers.

Bug fixes:
- Calling `DynamicImage`/`ImageBuffer`'s methods `write_to` and `save` will now
  work properly even if the backing container is larger than the image layout
  requires. Only the relevant slice of pixel data is passed to the encoder.
- Fixed a OOM-panic caused by malformed images in the `gif` decoder.

### Version 0.24.1

Bug Fixes:
- ImageBuffer::get_pixel_checked would sometimes return the incorrect pixel.
- PNG encoding would sometimes not recognize unsupported color.

### Version 0.24.0

Breaking changes

Structural changes:
- Minimum Rust version is now `1.56` and may change in minor versions until
  further notice. It is now tracked in the library's `Cargo.toml`, instead, by
  the standard `[package.rust-version]` field. Note: this applies _to the
  library itself_. You may need different version resolutions for dependencies
  when using a non-stable version of Rust.
- The `math::utils::{nq, utils}` modules have been removed. These are better
  served through the `color_quant` crate and the standard library respectively.
- All codecs are now available through `image::codecs`, no longer top-level.
- `ExtendedColorType` and `DynamicImage` have been made `#[non_exhaustive]`,
  providing more methods instead of exhaustive matching.
- Reading images through the generic `io::Reader`, as well as generic
  convenience interfaces, now requires the underlying reader to be `BufRead +
  Seek`. This allows more efficient support more formats. Similarly, writing
  now requires writers to be `Write + Seek`.
- The `Bgra*` variants of buffers, which were only half-supported, have been
  removed. The owning buffer types `ImageBuffer` and `DynamicImage`
  fundamentally already make a choice in supported pixel representations. This
  allows for more consistent internal behavior. Callers are expected to convert
  formats when using those buffers, which they are required to do in any case
  already, and which is routinely performed by decoders.

Trait reworks:
- The `Pixel` trait is no longer implemented quite as liberally for structs
  defined in the crate. Instead, it is now restricted to a set of known channel
  which ensures accuracy in computations involving those channels.
- The `ImageDecoderExt` trait has been renamed to `ImageDecoderRect`, according
  to its actual functionality.
- The `Pixel` trait and its `Subpixel` field no longer require (or provide) a
  `'static` lifetime bound.
- The `Pixel` trait no longer requires specifying an associated, constant
  `ColorType`. This was of little relevance to computation but made it much
  harder to implement and extend correctly. Instead, the _private_
  `PixelWithColorType` extension is added for interfaces that require a
  properly known variant.
- Reworked how `SubImage` interacts with the `GenericImage` trait. It is now a
  default implementation. Note that `SubImage` now has _inherent_ methods that
  avoid double-indirection, the trait's method will no longer avoid this.
- The `Primitive` trait now requires implementations to provide a minimum and
  maximum logical bound for the purpose of converting to other primitive
  representations.

Additions

Image formats:
- Reading lossless WebP is now supported.
- The OpenEXR format is now supported.
- The `jpeg` decoder has been upgraded to Lossless JPEG.
- The `AvifEncoder` now correctly handles alpha-less images. Some additional
  color formats are converted to RGBA as well.
- The `Bmp` codec now decodes more valid images. It can decode a raw image
  without performing the palette mapping. It provides a method to access the
  palette. The encoder provides the inverse capabilities.
- `Tiff` is now an output format.

Buffers and Operations:
- The channel / primitive type `f32` is now supported. Currently only the
  OpenEXR codec makes full use of it but this is expected to change.
- `ImageBuffer::{get_pixel_checked, get_pixel_mut_checked}` provide panic-free
  access to pixels and channels by returning `Option<&P>` and `Option<&mut P>`.
- `ImageBuffer::write_to` has been added, encoding the buffer to a writer. This
  method already existed on `DynamicImage`.
- `DynamicImage` now implements `From<_>` for all supported buffer types.
- `DynamicImage` now implements `Default`, an empty `Rgba8` image.
- `imageops::overlay` now takes coordinates as `i64`.

Limits:
- Added `Limits` and `LimitSupport`, utilized in `io::Reader`. These can be
  configured for rudimentary protection against resource exhaustion (images
  pretending to require a very large buffer). These types are not yet
  exhaustive by design, and more and stricter limits may be added in the
  future.
- Encoders that do provide inherent support for limits, or reserve a
  significant amount of internal memory, are urged to implement the
  `set_limits` extension to `ImageDecoder`. Some strict limit are opt-in, which
  may cause decoding to fail if not supported.

Miscellaneous:
- `PNMSubtype` has been renamed to `PnmSubtype`, by Rust's naming scheme.
- Several incorrectly capitalized `PNM*` aliases have been removed.
- Several `enum` types that had previously used a hidden variant now use the
  official `#[non_exhaustive]` attribute instead.

### Version 0.23.14

- Unified gif blending in different decode methods, fixing out-of-bounds checks
  in a number of weirdly positioned frames.
- Hardened TGA decoder against a number of malicious inputs.
- Fix forward incompatible usage of the panic macro.
- Fix load_rect for gif reaching `unreachable!()` code.

- Added `ExtendedColorType::A8`.
- Allow TGA to load alpha-only images.
- Optimized load_rect to avoid unnecessary seeks.

### Version 0.23.13

- Fix an inconsistency in supported formats of different methods for encoding
  an image.
- Fix `thumbnail` choosing an empty image. It now always prefer non-empty image
  dimensions.
- Fix integer overflow in calculating requires bytes for decoded image buffers
  for farbfeld, hdr, and pnm decoders. These will now error early.
- Fix a panic decoding certain `jpeg` image without frames or meta data.
- Optimized the `jpeg` encoder.
- Optimized `GenericImage::copy_from` default impl in various cases.

- Add `avif` decoders. You must enable it explicitly and it is not covered by
  our usual MSRV policy of Rust 1.34. Instead, only latest stable is supported.
- Add `ImageFormat::{can_read, can_write}`
- Add `Frame::buffer_mut`
- Add speed and quality options on `avif` encoder.
- Add speed parameter to `gif` encoder.
- Expose control over sequence repeat to the `gif` encoder.
- Add `{contrast,brighten,huerotate}_in_place` functions in imageproc.

- Relax `Default` impl of `ImageBuffer`, removing the bound on the color type.
- Derive Debug, Hash, PartialEq, Eq for DynamicImage

### Version 0.23.12

- Fix a soundness issue affecting the impls of `Pixel::from_slice_mut`. This
  would previously reborrow the mutable input reference as a shared one but
  then proceed to construct the mutable result reference from it. While UB
  according to Rust's memory model, we're fairly certain that no miscompilation
  can happen with the LLVM codegen in practice.
  See 5cbe1e6767d11aff3f14c7ad69a06b04e8d583c7 for more details.
- Fix `imageops::blur` panicking when `sigma = 0.0`. It now defaults to `1.0`
  as all negative values.
- Fix re-exporting `png::{CompressionType, FilterType}` to maintain SemVer
  compatibility with the `0.23` releases.

- Add `ImageFormat::from_extension`
- Add copyless DynamicImage to byte slice/vec conversion.
- Add bit-depth specific `into_` and `to_` DynamicImage conversion methods.


### Version 0.23.11

- The `NeuQuant` implementation is now supplied by `color_quant`. Use of the
  type defined by this library is discouraged.
- The `jpeg` decoder can now downscale images that are decoded by 1,2,4,8.
- Optimized the jpeg encoding ~5-15%.
- Deprecated the `clamp` function. Use `num-traits` instead.
- The ICO decoder now accepts an empty mask.
- Fixed an overflow in ICO mask decoding potentially leading to panic.
- Added `ImageOutputFormat` for `AVIF`
- Updated `tiff` to `0.6` with lzw performance improvements.

### Version 0.23.10

- Added AVIF encoding capabilities using the `ravif` crate. Please note that
  the feature targets the latest stable compiler and is not enabled by default.
- Added `ImageBuffer::as_raw` to inspect the underlying container.
- Updated `gif` to `0.11` with large performance improvements.

### Version 0.23.9

- Introduced correctly capitalized aliases for some scream case types
- Introduced `imageops::{vertical_gradient, horizontal_gradient}` for writing
  simple color gradients into an image.
- Sped up methods iterating over `Pixels`, `PixelsMut`, etc. by using exact
  chunks internally. This should auto-vectorize `ImageBuffer::from_pixel`.
- Adjusted `Clone` impls of iterators to not require a bound on the pixel.
- Add `Debug` impls for iterators where the pixel's channel implements it.
- Add comparison impls for `FilterType`

### Version 0.23.8

- `flat::Error` now implements the standard `Error` trait
- The type parameter of `Map` has been relaxed to `?Sized`
- Added the `imageops::tile` function that repeats one image across another

### Version 0.23.7

- Iterators over immutable pixels of `ImageBuffer` can now be cloned
- Added a `tga` encoder
- Added `ColorMap::lookup`, an optional reversal of the map
- The `EncodableLayout` trait is now exported

### Version 0.23.6

- Added `png::ApngDecoder`, an adapter decoding the animation in an APNG.
- Fixed a bug in `jpeg` encoding that would darken output colors.
- Added a utility constructor `FlatSamples::with_monocolor`.
- Added `ImageBuffer::as_flat_samples_mut` which is a mutable variant of the
  existing ffi-helper `ImageBuffer::as_flat_samples`.

### Version 0.23.5

- The `png` encoder now allows configuring compression and filter type. The
  output is not part of stability guarantees, see its documentation.
- The `jpeg` encoder now accepts any implementor of `GenericImageView`. This
  allows images that are only partially present in memory to be encoded.
- `ImageBuffer` now derives `Hash`, `PartialEq`, `Eq`.
- The `Pixels`/`PixelsMut` iterator no longer yields out-of-bounds pixels when
  the underlying buffer is larger than required.
- The `pbm` decoder correctly decodes ascii data again, fixing a regression
  where it would use the sample value `1` as white instead of `255`.
- Fix encoding of RGBA data in `gif` frames.
- Constructing a `Rows`/`RowsMut` iterator no longer panics when the image has
  a width or height of `0`.

### Version 0.23.4

- Improved the performance of decoding animated gifs
- Added `crop_imm` which functions like `crop` but on a shared reference
- The gif `DisposalMethod::Any` is treated as `Keep`, consistent with browsers
- Most errors no longer allocate a string, instead implement Display.
- Add some implementations of `Error::source`

### Version 0.23.3

- Added `ColorType::has_alpha` to facilitate lossless conversion
- Recognize extended WebP formats for decoding
- Added decoding and encoding for the `farbfeld` format
- Export named iterator types created from various `ImageBuffer` methods
- Error in jpeg encoder for images larger than 65536 pixels, fixes panic

### Version 0.23.2

- The dependency on `jpeg-decoder` now reflects minimum requirements.

### Version 0.23.1

- Fix cmyk_to_rgb (jpeg) causing off by one rounding errors.
- A number of performance improvements for jpeg (encode and decode), bmp, vp8
- Added more details to errors for many formats

### Version 0.23.0

This major release intends to improve the interface with regards to handling of
color format data and errors for both decoding and encoding. This necessitated
many breaking changes anyways so it was used to improve the compliance to the
interface guidelines such as outstanding renaming.

It is not yet perfect with regards to color spaces but it was designed mainly
as an improvement over the current interface with regards to in-memory color
formats, first. We'll get to color spaces in a later major version.

- Heavily reworked `ColorType`:
  - This type is now used for denoting formats for which we support operations
      on buffers in these memory representations. Particularly, all channels in
      pixel types are assumed to be an integer number of bytes (In terms of the
      Rust type system, these are `Sized` and one can crate slices of channel
      values).
  - An `ExtendedColorType` is used to express more generic color formats for
      which the library has limited support but can be converted/scaled/mapped
      into a `ColorType` buffer. This operation might be fallible but, for
      example, includes sources with 1/2/4-bit components.
  - Both types are non-exhaustive to add more formats in a minor release.
  - A work-in-progress (#1085) will further separate the color model from the
      specific channel instantiation, e.g. both `8-bit RGB` and `16-bit BGR`
      are instantiations of `RGB` color model.
- Heavily rework `ImageError`:
  - The top-level enum type now serves to differentiate cause with multiple
      opaque representations for the actual error. These are no longer simple
      Strings but contains useful types. Third-party decoders that have no
      variant in `ImageFormat` have also been considered.
  - Support for `Error::source` that can be downcast to an error from a
      matching version of the underlying decoders. Note that the version is not
      part of the stable interface guarantees, this should not be relied upon
      for correctness and only be used as an optimization.
  - Added image format indications to errors.
  - The error values produced by decoder will be upgraded incrementally. See
      something that still produces plain old String messages? Feel free to
      send a PR.
- Reworked the `ImageDecoder` trait:
  - `read_image` takes an output buffer argument instead of allocating all
      memory on its own.
  - The return type of `dimensions` now aligns with `GenericImage` sizes.
  - The `colortype` method was renamed to `color_type` for conformity.
- The enums `ColorType`, `DynamicImage`, `imageops::FilterType`, `ImageFormat`
  no longer re-export all of their variants in the top-level of the crate. This
  removes the growing pollution in the documentation and usage. You can still
  insert the equivalent statement on your own:
  `use image::ImageFormat::{self, *};`
- The result of `encode` operations is now uniformly an `ImageResult<()>`.
- Removed public converters from some `tiff`, `png`, `gif`, `jpeg` types,
  mainly such as error conversion. This allows upgrading the dependency across
  major versions without a major release in `image` itself.
- On that note, the public interface of `gif` encoder no longer takes a
  `gif::Frame` but rather deals with `image::Frame` only. If you require to
  specify the disposal method, transparency, etc. then you may want to wait
  with upgrading but (see next change).
- The `gif` encoder now errors on invalid dimensions or unsupported color
  formats. It would previously silently reinterpret bytes as RGB/RGBA.
- The capitalization of  `ImageFormat` and other enum variants has been
  adjusted to adhere to the API guidelines. These variants are now spelled
  `Gif`, `Png`, etc. The same change has been made to the name of types such as
  `HDRDecoder`.
- The `Progress` type has finally received public accessor method. Strange that
  no one reported them missing.
- Introduced `PixelDensity` and `PixelDensityUnit` to store DPI information in
  formats that support encoding this form of meta data (e.g. in `jpeg`).

### Version 0.22.5

- Added `GenericImage::copy_within`, specialized for `ImageBuffer`
- Fixed decoding of interlaced `gif` files
- Prepare for future compatibility of array `IntoIterator` in example code

### Version 0.22.4

- Added in-place variants for flip and rotate operations.
- The bmp encoder now checks if dimensions are valid for the format. It would
  previously write a subset or panic.
- Removed deprecated implementations of `Error::description`
- Added `DynamicImage::into_*` which convert without an additional allocation.
- The PNG encoder errors on unsupported color types where it had previously
  silently swapped color channels.
- Enabled saving images as `gif` with `save_buffer`.

### Version 0.22.3

- Added a new module `io` containing a configurable `Reader`. It can replace
  the bunch of free functions: `image::{load_*, open, image_dimensions}` while
  enabling new combinations such as `open` but with format deduced from content
  instead of file path.
- Fixed `const_err` lint in the macro expanded implementations of `Pixel`. This
  can only affect your crate if `image` is used as a path dependency.

### Version 0.22.2

- Undeprecate `unsafe` trait accessors. Further evaluation showed that their
  deprecation should be delayed until trait `impl` specialization is available.
- Fixed magic bytes used to detect `tiff` images.
- Added `DynamicImage::from_decoder`.
- Fixed a bug in the `PNGReader` that caused an infinite loop.
- Added `ColorType::{bits_per_pixel, num_components}`.
- Added `ImageFormat::from_path`, same format deduction as the `open` method.
- Fixed a panic in the gif decoder.
- Aligned background color handling of `gif` to web browser implementations.
- Fixed handling of partial frames in animated `gif`.
- Removed unused direct `lzw` dependency, an indirect dependency in `tiff`.

### Version 0.22.1

- Fixed build without no features enabled

### Version 0.22

- The required Rust version is now `1.34.2`.
- Note the website and blog: [image-rs.org][1] and [blog.image-rs.org][2]
- `PixelMut` now only on `ImageBuffer` and removed from `GenericImage`
  interface. Prefer iterating manually in the generic case.
- Replaced an unsafe interface in the hdr decoder with a safe variant.
- Support loading 2-bit BMP images
- Add method to save an `ImageBuffer`/`DynamicImage` with specified format
- Update tiff to `0.3` with a writer
- Update png to `0.15`, fixes reading of interlaced sub-byte pixels
- Always use custom struct for `ImageDecoder::Reader`
- Added `apply_without_alpha` and `map_without_alpha` to `Pixel` trait
- Pixel information now with associated constants instead of static methods
- Changed color structs to tuple types with single component. Improves
  ergonomics of destructuring assignment and construction.
- Add lifetime parameter on `ImageDecoder` trait.
- Remove unnecessary `'static` bounds on affine operations
- Add function to retrieve image dimensions without loading full image
- Allow different image types in overlay and replace
- Iterators over rows of `ImageBuffer`, mutable variants

[1]: https://www.image-rs.org
[2]: https://blog.image-rs.org

### Version 0.21.2

- Fixed a variety of crashes and opaque errors in webp
- Updated the png limits to be less restrictive
- Reworked even more `unsafe` operations into safe alternatives
- Derived Debug on FilterType and Deref on Pixel
- Removed a restriction on DXT to always require power of two dimensions
- Change the encoding of RGBA in bmp using bitfields
- Corrected various urls

### Version 0.21.1

- A fairly important bugfix backport
- Fixed a potentially memory safety issue in the hdr and tiff decoders, see #885
- See [the full advisory](docs/2019-04-23-memory-unsafety.md) for an analysis
- Fixes `ImageBuffer` index calculation for very, very large images
- Fix some crashes while parsing specific incomplete pnm images
- Added comprehensive fuzzing for the pam image types

### Version 0.21

- Updated README to use `GenericImageView`
- Removed outdated version number from CHANGES
- Compiles now with wasm-unknown-emscripten target
- Restructured `ImageDecoder` trait
- Updated README with a more colorful example for the Julia fractal
- Use Rust 1.24.1 as minimum supported version
- Support for loading GIF frames one at a time with `animation::Frames`
- The TGA decoder now recognizes 32 bpp as RGBA(8)
- Fixed `to_bgra` document comment
- Added release test script
- Removed unsafe code blocks several places
- Fixed overlay overflow bug issues with documented proofs

### Version 0.20

- Clippy lint pass
- Updated num-rational dependency
- Added BGRA and BGR color types
- Improved performance of image resizing
- Improved PBM decoding
- PNM P4 decoding now returns bits instead of bytes
- Fixed move of overlapping buffers in BMP decoder
- Fixed some document comments
- `GenericImage` and `GenericImageView` is now object-safe
- Moved TIFF code to its own library
- Fixed README examples
- Fixed ordering of interpolated parameters in TIFF decode error string
- Thumbnail now handles upscaling
- GIF encoding for multiple frames
- Improved subimages API
- Cargo fmt fixes

### Version 0.19

- Fixed panic when blending with alpha zero.
- Made `save` consistent.
- Consistent size calculation.
- Fixed bug in `apply_with_alpha`.
- Implemented `TGADecoder::read_scanline`.
- Use deprecated attribute for `pixels_mut`.
- Fixed bug in JPEG grayscale encoding.
- Fixed multi image TIFF.
- PNM encoder.
- Added `#[derive(Hash)]` for `ColorType`.
- Use `num-derive` for `#[derive(FromPrimitive)]`.
- Added `into_frames` implementation for GIF.
- Made rayon an optional dependency.
- Fixed issue where resizing image did not give exact width/height.
- Improved downscale.
- Added a way to expose options when saving files.
- Fixed some compiler warnings.
- Switched to lzw crate instead of using built-in version.
- Added `ExactSizeIterator` implementations to buffer structs.
- Added `resize_to_fill` method.
- DXT encoding support.
- Applied clippy suggestions.

### Version 0.4
 - Various improvements.
 - Additional supported image formats (BMP and ICO).
 - GIF and PNG codec moved into separate crates.

### Version 0.3
 - Replace `std::old_io` with `std::io`.

### Version 0.2
 - Support for interlaced PNG images.
 - Writing support for GIF images (full color and paletted).
 - Color quantizer that converts 32bit images to paletted including the alpha channel.
 - Initial support for reading TGA images.
 - Reading support for TIFF images (packbits and FAX compression not supported).
 - Various bug fixes and improvements.

### Version 0.1
- Initial release
- Basic reading support for png, jpeg, gif, ppm and webp.
- Basic writing support for png and jpeg.
- A collection of basic imaging processing function like `blur` or `invert`
