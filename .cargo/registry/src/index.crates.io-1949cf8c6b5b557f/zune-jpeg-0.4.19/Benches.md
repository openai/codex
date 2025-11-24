# Benchmarks of popular jpeg libraries

Here I compare how long it takes popular JPEG decoders to decode the below 7680*4320 image
of (now defunct ?) [Cutefish OS](https://en.cutefishos.com/) default wallpaper.
![img](benches/images/speed_bench.jpg)

## About benchmarks

Benchmarks are weird, especially IO & multi-threaded programs. This library uses both of the above hence performance may
vary.

For best results shut down your machine, go take coffee, think about life and how it came to be and why people should
save the environment.

Then power up your machine, if it's a laptop connect it to a power supply and if there is a setting for performance
mode, tweak it.

Then run.

## Benchmarks vs real world usage

Real world usage may vary.

Notice that I'm using a large image but probably most decoding will be small to medium images.

To make the library thread safe, we do about 1.5-1.7x more allocations than libjpeg-turbo. Although, do note that the
allocations do not occur at ago, we allocate when needed and deallocate when not needed.

Do note if memory bandwidth is a limitation. This is not for you.

## Reproducibility

The benchmarks are carried out on my local machine with an AMD Ryzen 5 4500u

The benchmarks are reproducible.

To reproduce them

1. Clone this repository
2. Install rust(if you don't have it yet)
3. `cd` into the directory.
4. Run `cargo bench`

## Performance features of the three libraries

| feature                      | image-rs/jpeg-decoder | libjpeg-turbo | zune-jpeg |
|------------------------------|-----------------------|---------------|-----------|
| multithreaded                | ✅                     | ❌             | ❌         |
| platform specific intrinsics | ✅                     | ✅             | ✅         |

- Image-rs/jpeg-decoder uses [rayon] under the hood but it's under a feature
  flag.

- libjpeg-turbo uses hand-written asm for platform specific intrinsics, ported to
  the most common architectures out there but falls back to scalar
  code if it can't run in a platform.

# Finally benchmarks

[here]

## Notes

Benchmarks are ran at least once a week to catch regressions early and
are uploaded to Github pages.

Machine specs can be found on the other [landing page]

Benchmarks may not reflect real world usage(threads, other I/O machine bottlenecks)

[landing page]:https://etemesi254.github.io/posts/Zune-Benchmarks/

[here]:https://etemesi254.github.io/assets/criterion/report/index.html

[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo

[jpeg-decoder]:https://github.com/image-rs/jpeg-decoder

[rayon]:https://github.com/rayon-rs/rayon