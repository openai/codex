# Examples

We try to focus our documentation on the [four types of
documentation](https://documentation.divio.com/).  Examples fit into this by
providing:
- [Cookbook / How-To Guides](https://docs.rs/clap/latest/clap/_cookbook/index.html)
- Tutorials ([derive](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html), [builder](https://docs.rs/clap/latest/clap/_tutorial/index.html))

This directory contains the source for the above.

## Contributing

New examples should fit within the above structure and support their narrative
- Add the example to [Cargo.toml](../Cargo.toml) for any `required-features`
- Document how the example works with a `.md` file which will be verified using [trycmd](https://docs.rs/trycmd)
- Pull the `.rs` and `.md` file into the appropriate module doc comment to be accessible on docs.rs
