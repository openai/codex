# pulldown-cmark-escape

[![Tests](https://github.com/pulldown-cmark/pulldown-cmark/actions/workflows/rust.yml/badge.svg)](https://github.com/pulldown-cmark/pulldown-cmark/actions/workflows/rust.yml)
[![Docs](https://docs.rs/pulldown-cmark/badge.svg)](https://docs.rs/pulldown-cmark)
[![Crates.io](https://img.shields.io/crates/v/pulldown-cmark-escape.svg?maxAge=2592000)](https://crates.io/crates/pulldown-cmark-escape)

[Documentation](https://docs.rs/pulldown-cmark/)

This crate allows to escape HTML and links and it is part of the pulldown-cmark
project, by providing `escape_html`, `escape_html_body_text` (for
a less long output in body HTML strings) and `escape_href` functions.

## Authors

The main author is Raph Levien. The implementation of the new design (v0.3+) was
completed by Marcus Klaas de Vries. Since 2023, the development has been driven
by Martín Pozo, Michael Howell, Roope Salmi and Martin Geisler.

## License

This software is under the MIT license. See details in [license file](./LICENSE).

## Contributions

We gladly accept contributions via GitHub pull requests. Please see
[CONTRIBUTING.md](../CONTRIBUTING.md) for more details.
