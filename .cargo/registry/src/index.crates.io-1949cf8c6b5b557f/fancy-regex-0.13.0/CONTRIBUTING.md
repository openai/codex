# Contributing

The fancy-regex project is committed to fostering and preserving a
diverse, welcoming community; all participants are expected to
follow the [Rust Code of Conduct](https://www.rust-lang.org/en-US/conduct.html).

Patching processes for this project are somewhat informal, as it's
maintained by a small group. No Contributor License Agreement is needed.

If this is your first substantive pull request in this repo, feel free
to add yourself to the AUTHORS file.

Make sure to run `cargo test` and `cargo fmt` to make sure your changes
pass the tests and are formatted as expected.

When adding support for new syntax (i.e. for better compatibility with
oniguruma syntax), remember to update the "Syntax" section of the
documentation comments in `lib.rs`.

## Manual testing

Sometimes it's useful to manually check how regexes are matched, e.g.
to compare to other engines.

### fancy-regex

The toy example is useful for playing around with different regexes:

    cargo run --example toy run '[a-z]' 'input text'

### Oniguruma

Set up rust-onig which is based on Oniguruma:

    git clone --recurse-submodules https://github.com/rust-onig/rust-onig

Then match some text:

    echo 'input text' | cargo run --example capturedump -- '[a-z]'
