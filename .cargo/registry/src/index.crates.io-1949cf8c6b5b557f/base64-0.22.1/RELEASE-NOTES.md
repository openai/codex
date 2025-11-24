# 0.22.1

- Correct the symbols used for the predefined `alphabet::BIN_HEX`.

# 0.22.0

- `DecodeSliceError::OutputSliceTooSmall` is now conservative rather than precise. That is, the error will only occur if the decoded output _cannot_ fit, meaning that `Engine::decode_slice` can now be used with exactly-sized output slices. As part of this, `Engine::internal_decode` now returns `DecodeSliceError` instead of `DecodeError`, but that is not expected to affect any external callers.
- `DecodeError::InvalidLength` now refers specifically to the _number of valid symbols_ being invalid (i.e. `len % 4 == 1`), rather than just the number of input bytes. This avoids confusing scenarios when based on interpretation you could make a case for either `InvalidLength` or `InvalidByte` being appropriate.
- Decoding is somewhat faster (5-10%)

# 0.21.7

- Support getting an alphabet's contents as a str via `Alphabet::as_str()`

# 0.21.6

- Improved introductory documentation and example

# 0.21.5

- Add `Debug` and `Clone` impls for the general purpose Engine

# 0.21.4

- Make `encoded_len` `const`, allowing the creation of arrays sized to encode compile-time-known data lengths

# 0.21.3

- Implement `source` instead of `cause` on Error types
- Roll back MSRV to 1.48.0 so Debian can continue to live in a time warp
- Slightly faster chunked encoding for short inputs
- Decrease binary size

# 0.21.2

- Rollback MSRV to 1.57.0 -- only dev dependencies need 1.60, not the main code

# 0.21.1

- Remove the possibility of panicking during decoded length calculations
- `DecoderReader` no longer sometimes erroneously ignores
  padding  [#226](https://github.com/marshallpierce/rust-base64/issues/226)

## Breaking changes

- `Engine.internal_decode` return type changed
- Update MSRV to 1.60.0

# 0.21.0

## Migration

### Functions

| < 0.20 function         | 0.21 equivalent                                                                     |
|-------------------------|-------------------------------------------------------------------------------------|
| `encode()`              | `engine::general_purpose::STANDARD.encode()` or `prelude::BASE64_STANDARD.encode()` |
| `encode_config()`       | `engine.encode()`                                                                   |
| `encode_config_buf()`   | `engine.encode_string()`                                                            |
| `encode_config_slice()` | `engine.encode_slice()`                                                             |
| `decode()`              | `engine::general_purpose::STANDARD.decode()` or `prelude::BASE64_STANDARD.decode()` |
| `decode_config()`       | `engine.decode()`                                                                   |
| `decode_config_buf()`   | `engine.decode_vec()`                                                               |
| `decode_config_slice()` | `engine.decode_slice()`                                                             |

The short-lived 0.20 functions were the 0.13 functions with `config` replaced with `engine`.

### Padding

If applicable, use the preset engines `engine::STANDARD`, `engine::STANDARD_NO_PAD`, `engine::URL_SAFE`,
or `engine::URL_SAFE_NO_PAD`.
The `NO_PAD` ones require that padding is absent when decoding, and the others require that
canonical padding is present .

If you need the < 0.20 behavior that did not care about padding, or want to recreate < 0.20.0's predefined `Config`s
precisely, see the following table.

| 0.13.1 Config   | 0.20.0+ alphabet | `encode_padding` | `decode_padding_mode` |
|-----------------|------------------|------------------|-----------------------|
| STANDARD        | STANDARD         | true             | Indifferent           |
| STANDARD_NO_PAD | STANDARD         | false            | Indifferent           |
| URL_SAFE        | URL_SAFE         | true             | Indifferent           |
| URL_SAFE_NO_PAD | URL_SAFE         | false            | Indifferent           |

# 0.21.0-rc.1

- Restore the ability to decode into a slice of precisely the correct length with `Engine.decode_slice_unchecked`.
- Add `Engine` as a `pub use` in `prelude`.

# 0.21.0-beta.2

## Breaking changes

- Re-exports of preconfigured engines in `engine` are removed in favor of `base64::prelude::...` that are better suited
  to those who wish to `use` the entire path to a name.

# 0.21.0-beta.1

## Breaking changes

- `FastPortable` was only meant to be an interim name, and shouldn't have shipped in 0.20. It is now `GeneralPurpose` to
  make its intended usage more clear.
- `GeneralPurpose` and its config are now `pub use`'d in the `engine` module for convenience.
- Change a few `from()` functions to be `new()`. `from()` causes confusing compiler errors because of confusion
  with `From::from`, and is a little misleading because some of those invocations are not very cheap as one would
  usually expect from a `from` call.
- `encode*` and `decode*` top level functions are now methods on `Engine`.
- `DEFAULT_ENGINE` was replaced by `engine::general_purpose::STANDARD`
- Predefined engine consts `engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD}`
    - These are `pub use`d into `engine` as well
- The `*_slice` decode/encode functions now return an error instead of panicking when the output slice is too small
    - As part of this, there isn't now a public way to decode into a slice _exactly_ the size needed for inputs that
      aren't multiples of 4 tokens. If adding up to 2 bytes to always be a multiple of 3 bytes for the decode buffer is
      a problem, file an issue.

## Other changes

- `decoded_len_estimate()` is provided to make it easy to size decode buffers correctly.

# 0.20.0

## Breaking changes

- Update MSRV to 1.57.0
- Decoding can now either ignore padding, require correct padding, or require no padding. The default is to require
  correct padding.
    - The `NO_PAD` config now requires that padding be absent when decoding.

## 0.20.0-alpha.1

### Breaking changes

- Extended the `Config` concept into the `Engine` abstraction, allowing the user to pick different encoding / decoding
  implementations.
    - What was formerly the only algorithm is now the `FastPortable` engine, so named because it's portable (works on
      any CPU) and relatively fast.
    - This opens the door to a portable constant-time
      implementation ([#153](https://github.com/marshallpierce/rust-base64/pull/153),
      presumably `ConstantTimePortable`?) for security-sensitive applications that need side-channel resistance, and
      CPU-specific SIMD implementations for more speed.
    - Standard base64 per the RFC is available via `DEFAULT_ENGINE`. To use different alphabets or other settings (
      padding, etc), create your own engine instance.
- `CharacterSet` is now `Alphabet` (per the RFC), and allows creating custom alphabets. The corresponding tables that
  were previously code-generated are now built dynamically.
- Since there are already multiple breaking changes, various functions are renamed to be more consistent and
  discoverable.
- MSRV is now 1.47.0 to allow various things to use `const fn`.
- `DecoderReader` now owns its inner reader, and can expose it via `into_inner()`. For symmetry, `EncoderWriter` can do
  the same with its writer.
- `encoded_len` is now public so you can size encode buffers precisely.

# 0.13.1

- More precise decode buffer sizing, avoiding unnecessary allocation in `decode_config`.

# 0.13.0

- Config methods are const
- Added `EncoderStringWriter` to allow encoding directly to a String
- `EncoderWriter` now owns its delegate writer rather than keeping a reference to it (though refs still work)
    - As a consequence, it is now possible to extract the delegate writer from an `EncoderWriter` via `finish()`, which
      returns `Result<W>` instead of `Result<()>`. If you were calling `finish()` explicitly, you will now need to
      use `let _ = foo.finish()` instead of just `foo.finish()` to avoid a warning about the unused value.
- When decoding input that has both an invalid length and an invalid symbol as the last byte, `InvalidByte` will be
  emitted instead of `InvalidLength` to make the problem more obvious.

# 0.12.2

- Add `BinHex` alphabet

# 0.12.1

- Add `Bcrypt` alphabet

# 0.12.0

- A `Read` implementation (`DecoderReader`) to let users transparently decoded data from a b64 input source
- IMAP's modified b64 alphabet
- Relaxed type restrictions to just `AsRef<[ut8]>` for main `encode*`/`decode*` functions
- A minor performance improvement in encoding

# 0.11.0

- Minimum rust version 1.34.0
- `no_std` is now supported via the two new features `alloc` and `std`.

# 0.10.1

- Minimum rust version 1.27.2
- Fix bug in streaming encoding ([#90](https://github.com/marshallpierce/rust-base64/pull/90)): if the underlying writer
  didn't write all the bytes given to it, the remaining bytes would not be retried later. See the docs
  on `EncoderWriter::write`.
- Make it configurable whether or not to return an error when decoding detects excess trailing bits.

# 0.10.0

- Remove line wrapping. Line wrapping was never a great conceptual fit in this library, and other features (streaming
  encoding, etc) either couldn't support it or could support only special cases of it with a great increase in
  complexity. Line wrapping has been pulled out into a [line-wrap](https://crates.io/crates/line-wrap) crate, so it's
  still available if you need it.
    - `Base64Display` creation no longer uses a `Result` because it can't fail, which means its helper methods for
      common
      configs that `unwrap()` for you are no longer needed
- Add a streaming encoder `Write` impl to transparently base64 as you write.
- Remove the remaining `unsafe` code.
- Remove whitespace stripping to simplify `no_std` support. No out of the box configs use it, and it's trivial to do
  yourself if needed: `filter(|b| !b" \n\t\r\x0b\x0c".contains(b)`.
- Detect invalid trailing symbols when decoding and return an error rather than silently ignoring them.

# 0.9.3

- Update safemem

# 0.9.2

- Derive `Clone` for `DecodeError`.

# 0.9.1

- Add support for `crypt(3)`'s base64 variant.

# 0.9.0

- `decode_config_slice` function for no-allocation decoding, analogous to `encode_config_slice`
- Decode performance optimization

# 0.8.0

- `encode_config_slice` function for no-allocation encoding

# 0.7.0

- `STANDARD_NO_PAD` config
- `Base64Display` heap-free wrapper for use in format strings, etc

# 0.6.0

- Decode performance improvements
- Use `unsafe` in fewer places
- Added fuzzers

# 0.5.2

- Avoid usize overflow when calculating length
- Better line wrapping performance

# 0.5.1

- Temporarily disable line wrapping
- Add Apache 2.0 license

# 0.5.0

- MIME support, including configurable line endings and line wrapping
- Removed `decode_ws`
- Renamed `Base64Error` to `DecodeError`

# 0.4.1

- Allow decoding a `AsRef<[u8]>` instead of just a `&str`

# 0.4.0

- Configurable padding
- Encode performance improvements

# 0.3.0

- Added encode/decode functions that do not allocate their own storage
- Decode performance improvements
- Extraneous padding bytes are no longer ignored. Now, an error will be returned.
