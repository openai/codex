# Version 0.10.3

New features:
- Group 4 `fax` is now recognized as a compression type and decoded.
- A crate feature `fax` controls related dependencies, on by default.

Changes:
- The presence of PhotometricInterpretation::WhiteIsZero now returns an error
  if the color type can not be inverted by the library where they were
  previously silently ignored. Please report any missing color types.

Fixes:
- The PhotometricInterpretation::WhiteIsZero is now also applied to 1-bit
  images, as well as 2-bit and 4-bit sample types where previously the presence
  of this tag was ignored.

# Version 0.10.2

Republished 0.10.1, fixing an accidentally removed method (`Decoder::find_tag_unsigned_vec`).

# Version 0.10.1

New features:
 - Added a dedicated `Directory` type (#277)
 - Expose methods related to `Directory` publicly (#278)
 - Implement writing directories (#281, #283)
 - Add ICC (color profile) to the set of known tags (#287)
 - Added `Decoder.inner()` to access the underlying reader (#289)
 - Added methods for reading into a pre-allocated byte buffer (#285)

Changes:
 - No longer perform color conversion when decoding embedded JPEG. This fixes decoding for JPEG images with alpha channel. (#274)
 - More lenient LZW decoding to match libtiff (#231)

Fixes:
 - Fixed seeking multiple times in a row (#279)

# Version 0.10.0

New features:
- Added `ColorType::Multiband` and `ColorType::CMYKA`.
- Added `Value::Byte` and `Value::SignedByte` to represent `u8` and `i8` tags.
- Encoding can now use horizontal predictor.
- The `Decoder::tag_iter` method allows iterating over all tags in the image.
- Half precision floating point values are now supported during decoding.
- ZStandard compression has been added as an additional compression method
  during decoding, but requires the `zstd` feature flag.

Changes:
- Switched `u16` and `i16` values to be intepreted as `Value::Short` and
  `Value::SignedShort` respectively.
- Changed JPEG compression to use the `zune-jpeg` crate instead of
  `jpeg-decoder`.
- The `deflate`, `jpeg`, and `lzw` compression methods are now controlled by
  (enabled by default) feature flags.

Fixes:
- Fixed a bug in 1 bit-per-pixel decoding.

# Version 0.9.1

New features:
- Basic support for planar configuration.
- Allow arbitrary number of samples as long as all have the same bit depth.

Fixes:
- Don't panic when parsing metadata when custom compression is used.

# Version 0.9.0

New features:
* Added support for photometric interpretation `YCbCr` and added related
  `ColorType`.

Fixes:
* Decoding tiled images calculates padding correctly when image width or height
  is a multiple of tile size. It could previously corrupt the last tile per row
  by skipping over data.

# Version 0.8.1

Changes:
* The jpeg decoder gained to ability to utilize the Photometric Interpretation
  directly instead of relying on a custom APP segment.

Fixes:
* A spurious error within the PackBits decoder lead to the incorrect results
  (wrong bits or errors), based on the maximum size of reads from the
  underlying reader.
* Removed a panic path in jpeg decoding, when a feature such as photometric
  interpretation is not supported. An error is returned instead.

# Version 0.8.0

Changes:
* The minimum supported rust version is now indicated in `Cargo.toml`.
* The enums `TiffFormatError` and `TiffUnsupportedError` are now
  marked with the `#[non_exhaustive]` attribute. 
* Additionally, tag related enums `Value`, `Tags`, `Type`, `CompressionMethod`,
  `PhotometricInterpretation`, `PlanarConfiguration`, `Predictor`,
  `ResolutionUnit`, `SampleFormat` are also changed.

Removals:
* Removed deprecated methods of `Decoder`: `init`, `read_jpeg`,
  `read_strip_to_buffer`, `read_strip`, `read_tile`. The implicit chunk (row or
  tile) index order could not be easily tracked by the caller. New separate
  utility interfaces may be introduced at a later point, for now callers are
  obligated to choose their own.

Fixes:
* Update to `jpeg_decoder = 0.3`.

# Version 0.7.4

New features:
* Creating an encoder for invalid, zero-sized images is now rejected.

Fixes:
* Fix panic, in a case where decoding jpeg encoded images did not expect the
  jpeg decoder to return an error.
* Fix panic by validating `rows_per_strip` better, fixing a division-by-zero.

# Version 0.7.3

New features:
* Allow decoder to access specific tiles by index.
* Add support for floating point predictor.
* Tiled jpeg file support.

Changes:
* Various refactoring and performance improvements.

# Version 0.7.2

New features:
* Encoding with `ImageEncoder` now takes an optional compressor argument,
  allowing compressed encoding. See the methods 
  `TiffEncoder::{new_image,write_image}_with_compression`.
* `jpeg_decoder` has been upgraded, now supports lossless JPEG.

Changes:
* Decoding now more consistently reads and interprets the initial IFD, instead
  of performing _some_ interpretation lazily. (This change prepares fully lazy
  and backwards seeking.)

# Version 0.7.1

New features:
* Encoding signed integer formats is now supported.
* Extensive fuzzing with `cargo fuzz`.

Changes:
* Tile decoding should be a little faster, requires one less intermediate buffer.
* Images whose IFDs form a cycle due to their offsets will now raise an error
  when the cycle would be entered (jumping back should still be supported).

Fixes:
* Fixed a regression that caused conflict between strips and tile images,
  causing errors in decoding some images.
* Use checked integer arithmetic in limit calculations, fixes overflows.
* IFD Tags are now always cleared between images.
* Found by fuzzing: Several memory limit overflows; JPEG now correctly
  validates offsets and a minimum size of its table; Check upper limit of strip
  byte size correctly;

Notes:
Our CI has warned that this version no longer builds on `1.34.2` out of the
box. We're still committed to the MSRV on this major version yet one
dependency—`flate2`—has already bumped it in a SemVer compatible version of its
own. This is out-of-our-control (cargo's dependency resolution does not allow
us to address this in a reasonable manner).

This can be address this by pinning the version of `flate2` to `1.0.21` in your
own files. However, you should understand that this puts you in considerable
maintenance debt as you will no longer receive any updates for this dependency
and any package that _requires_ a new version of the `1.0` series would be
incompatible with this requirement (cargo might yell at you very loudly).

# Version 0.7.0

New features:
* Support for encoding BigTiff ([#122](https://github.com/image-rs/image-tiff/pull/122))
  * _Breaking:_ Encoder types now have a generic parameter to differentiate BigTiff and standard Tiff encoding. Defaults to standard Tiff.
* Basic tile decoding ([#125](https://github.com/image-rs/image-tiff/pull/125))
  * _Breaking:_ There is a new `TiffError::UsageError` variant.
* Support for datatypes `Int8` and `Int16` ([#114](https://github.com/image-rs/image-tiff/pull/114))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have the two new variants `I8` and `I16`.
* Support for `i32` arrays ([#118](https://github.com/image-rs/image-tiff/pull/118/files))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have a new `I32` variant.
* Support for `Ifd` and `IfdBig` tag types and `I64` data type ([#119](https://github.com/image-rs/image-tiff/pull/119))
  * _Breaking:_ `DecodingResult` and `DecodingBuffer` have a new `I64` variant.
* Add `SMinSampleValue` and `SMaxSampleValue` ([#123](https://github.com/image-rs/image-tiff/pull/123))

Changes:
* Improve deflate support ([#132](https://github.com/image-rs/image-tiff/pull/132))
  * Switch to streaming decompression via `flate2`. Aside from performance improvements and lower RAM consumption, this fixes a bug where `max_uncompressed_length` was precalculated for a single tile but then used as a hard limit on the whole data, failing to decompress any tiled images.
  * Add support for new `Deflate` tag in addition to `OldDeflate`.
* _Breaking:_ Remove `InflateError`, which is no longer needed with `flate2` ([#134](https://github.com/image-rs/image-tiff/pull/134))
* _Breaking:_ Support for `MinIsWhite` is restricted to unsigned and floating
  point values. This is expected to be be re-added once some contradictory
  interpretation regarding semantics for signed values is resolved.

Fixes:
* Validate that ASCII tags are valid ASCII and end with a null byte ([#121](https://github.com/image-rs/image-tiff/pull/121))

Internal:
* Simplify decompression logic ([#126](https://github.com/image-rs/image-tiff/pull/126))
* Simplify `expand_strip` ([#128](https://github.com/image-rs/image-tiff/pull/128))

# Version 0.6.1

New features:
* Support for reading `u16` and ascii string tags.
* Added `Limits::unlimited` for disabling all limits.
* Added `ImageEncoder::rows_per_strip` to overwrite the default.

Changes:
* The default strip size for chunked encoding is now 1MB, up from 8KB. This
  should lead to more efficient decoding and compression.

Fixes:
* Fixed a bug where LZW compressed strips could not be decoded, instead
  returning an error `Inconsistent sizes encountered`.
* Reading a tag with a complex type and a single value returns the proper Value
  variant, instead of a vector with one entry.

# Version 0.6.0

New features:
* Support for decoding BigTIFF with 64-bit offsets
* The value types byte, `f32`, `f64` are now recognized
* Support for Modern JPEG encoded images

Improvements:
* Better support for adding auxiliary tags before encoding image data
* Switched to lzw decoder library `weezl` for performance
* The `ColorType` trait now supports `SAMPLE_ENCODING` hints

Fixes:
* Fixed decoding of inline ASCII in tags
* Fixed handling after null terminator in ASCII data
* Recognize tile and sample format tags

# Version 0.5.0

* Added support for 32-bit and 64-bit decoded values.
* Added CMYK(16|32|64) color type support.
* Check many internal integer conversions to increase stability. This should
  only lead to images being reported as faulty that would previously silently
  break platform limits. If there are any false positives, please report them.
* Remove an erroneous check of decoded length in lzw compressed images.

# Version 0.4.0

* Several enumerations are now non_exhaustive for future extensions.
  These are `Tag`, `Type`, `Value`, `PhotometricInterpretation`,
  `CompressionMethod`, `Predictor`.
* Enums gained a dedicated method to convert to their TIFF variant value with
  the specified type. Performing these conversions by casting the discriminant
  with `as` is not guaranteed to be stable, except where documented explicitly.
* Removed the num-derive and num dependencies.
* Added support for decoding `deflate` compressed images.
* Make the decoder `Limits` customizable by exposing members.
* Fixed multi-page TIFF encoding writing incorrect offsets.
