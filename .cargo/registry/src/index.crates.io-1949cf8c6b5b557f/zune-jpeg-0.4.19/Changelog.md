## Version 0.3.17

- Fix no-std compilation

## Version 0.3.16

- Add support for decoding to BGR and BGRA

## Version 0.3.14

- Add ability to parse exif and ICC chunk.
- Fix images with one component that were down-sampled.

### Version 0.3.13

- Allow decoding into pre-allocated buffer
- Clarify documentation

### Version 0.3.11

- Add guards for SSE and AVX code paths(allows compiling for platforms that do not support it)

### Version 0.3.0

- Overhaul to the whole decoder.
- Single threaded version
- Lightweight.

### Version 0.2.0

- New `ZuneJpegOptions` struct, this is the now recommended way to set up decoding options for
  decoding
- Deprecated previous options setting functions.
- More code cleanups
- Fixed new bugs discovered by fuzzing
- Removed dependency on `num_cpu`

### Version 0.1.5
- Allow user to set memory limits in during decoding explicitly via `set_limits`
- Fixed some bugs discovered by fuzzing
- Correctly handle small images less than 16 pixels
- Gracefully handle incorrectly sampled images.

### Version 0.1.4
- Remove all `unsafe` instances except platform dependent intrinsics.
- Numerous bug fixes identified by fuzzing.
- Expose `ImageInfo` to the crate root.

### Version 0.1.3
- Fix numerous panics found by fuzzing(thanks to @[Shnatsel] for the corpus)
- Add new method `set_num_threads` that allows one to explicitly set the number of threads to use to decode the image.

### Version 0.1.2
- Add more sub checks, contributed by @[5225225]
- Privatize some modules.

### Version 0.1.1
- Fix rgba/rgbx decoding when avx optimized functions were used
- Initial support for fuzzing 
- Remove `align_alloc` method which was unsound (Thanks to @[HeroicKatora] for pointing that out)

[Shnatsel]:https://github.com/Shnatsel
[HeroicKatora]:https://github.com/HeroicKatora
[5225225]:https://github.com/5225225