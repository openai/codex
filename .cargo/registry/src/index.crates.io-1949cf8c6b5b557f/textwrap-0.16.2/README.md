# Textwrap

[![](https://github.com/mgeisler/textwrap/workflows/build/badge.svg)][build-status]
[![](https://codecov.io/gh/mgeisler/textwrap/branch/master/graph/badge.svg)][codecov]
[![](https://img.shields.io/crates/v/textwrap.svg)][crates-io]
[![](https://docs.rs/textwrap/badge.svg)][api-docs]

Textwrap is a library for wrapping and indenting text. It is most often used by
command-line programs to format dynamic output nicely so it looks good in a
terminal. You can also use Textwrap to wrap text set in a proportional font—such
as text used to generate PDF files, or drawn on a
[HTML5 canvas using WebAssembly][wasm-demo].

## Usage

To use the textwrap crate, add this to your `Cargo.toml` file:

```toml
[dependencies]
textwrap = "0.16"
```

By default, this enables word wrapping with support for Unicode strings. Extra
features can be enabled with Cargo features—and the Unicode support can be
disabled if needed. This allows you slim down the library and so you will only
pay for the features you actually use.

Please see the
[_Cargo Features_ in the crate
documentation](https://docs.rs/textwrap/#cargo-features) for a full list of the
available features as well as their impact on the size of your binary.

## Documentation

**[API documentation][api-docs]**

## Getting Started

Word wrapping is easy using the `wrap` and `fill` functions:

```rust
#[cfg(feature = "smawk")] {
let text = "textwrap: an efficient and powerful library for wrapping text.";
assert_eq!(
    textwrap::wrap(text, 28),
    vec![
        "textwrap: an efficient",
        "and powerful library for",
        "wrapping text.",
    ]
);
}
```

Sharp-eyed readers will notice that the first line is 22 columns wide. So why is
the word “and” put in the second line when there is space for it in the first
line?

The explanation is that textwrap does not just wrap text one line at a time.
Instead, it uses an optimal-fit algorithm which looks ahead and chooses line
breaks which minimize the gaps left at ends of lines. This is controlled with
the `smawk` Cargo feature, which is why the example is wrapped in the
`cfg`-block.

Without look-ahead, the first line would be longer and the text would look like
this:

```rust
#[cfg(not(feature = "smawk"))] {
let text = "textwrap: an efficient and powerful library for wrapping text.";
assert_eq!(
    textwrap::wrap(text, 28),
    vec![
        "textwrap: an efficient and",
        "powerful library for",
        "wrapping text.",
    ]
);
}
```

The second line is now shorter and the text is more ragged. The kind of wrapping
can be configured via `Options::wrap_algorithm`.

If you enable the `hyphenation` Cargo feature, you get support for automatic
hyphenation for [about 70 languages][patterns] via high-quality TeX hyphenation
patterns.

Your program must load the hyphenation pattern and configure
`Options::word_splitter` to use it:

```rust
#[cfg(feature = "hyphenation")] {
use hyphenation::{Language, Load, Standard};
use textwrap::{fill, Options, WordSplitter};

let dictionary = Standard::from_embedded(Language::EnglishUS).unwrap();
let options = textwrap::Options::new(28).word_splitter(WordSplitter::Hyphenation(dictionary));
let text = "textwrap: an efficient and powerful library for wrapping text.";

assert_eq!(
    textwrap::wrap(text, &options),
    vec![
        "textwrap: an efficient and",
        "powerful library for wrap-",
        "ping text."
    ]
);
}
```

The US-English hyphenation patterns are embedded when you enable the
`hyphenation` feature. They are licensed under a
[permissive license][en-us license] and take up about 88 KB in your binary. If
you need hyphenation for other languages, you need to download a
[precompiled `.bincode` file][bincode] and load it yourself. Please see the
[`hyphenation` documentation] for details.

## Wrapping Strings at Compile Time

If your strings are known at compile time, please take a look at the procedural
macros from the [`textwrap-macros` crate].

## Examples

The library comes with
[a collection](https://github.com/mgeisler/textwrap/tree/master/examples) of
small example programs that shows various features.

If you want to see Textwrap in action right away, then take a look at
[`examples/wasm/`], which shows how to wrap sans-serif, serif, and monospace
text. It uses WebAssembly and is automatically deployed to
https://mgeisler.github.io/textwrap/.

For the command-line examples, you’re invited to clone the repository and try
them out for yourself! Of special note is [`examples/interactive.rs`]. This is a
demo program which demonstrates most of the available features: you can enter
text and adjust the width at which it is wrapped interactively. You can also
adjust the `Options` used to see the effect of different `WordSplitter`s and
wrap algorithms.

Run the demo with

```sh
$ cargo run --example interactive
```

The demo needs a Linux terminal to function.

## Release History

Please see the [CHANGELOG file] for details on the changes made in each release.

## License

Textwrap can be distributed according to the [MIT license][mit]. Contributions
will be accepted under the same license.

[crates-io]: https://crates.io/crates/textwrap
[build-status]: https://github.com/mgeisler/textwrap/actions?query=workflow%3Abuild+branch%3Amaster
[codecov]: https://codecov.io/gh/mgeisler/textwrap
[wasm-demo]: https://mgeisler.github.io/textwrap/
[`textwrap-macros` crate]: https://crates.io/crates/textwrap-macros
[`hyphenation` example]: https://github.com/mgeisler/textwrap/blob/master/examples/hyphenation.rs
[`termwidth` example]: https://github.com/mgeisler/textwrap/blob/master/examples/termwidth.rs
[patterns]: https://github.com/tapeinosyne/hyphenation/tree/master/patterns
[en-us license]: https://github.com/hyphenation/tex-hyphen/blob/master/hyph-utf8/tex/generic/hyph-utf8/patterns/tex/hyph-en-us.tex
[bincode]: https://github.com/tapeinosyne/hyphenation/tree/master/dictionaries
[`hyphenation` documentation]: http://docs.rs/hyphenation
[`examples/wasm/`]: https://github.com/mgeisler/textwrap/tree/master/examples/wasm
[`examples/interactive.rs`]: https://github.com/mgeisler/textwrap/tree/master/examples/interactive.rs
[api-docs]: https://docs.rs/textwrap/
[CHANGELOG file]: https://github.com/mgeisler/textwrap/blob/master/CHANGELOG.md
[mit]: LICENSE
