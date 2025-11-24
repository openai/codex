# 1.2.0

* Adds `bytes` module to support operating directly on byte strings.

# 1.1.0

* Adds the `std` feature (enabled by default)
* Disabling the `std` feature makes the crate work in `#![no_std]` mode, assuming presence of the `alloc` crate

# 1.0.0

* Adds the `join` convenience function.
* Fixes parsing of `'\\n'` to match the behavior of bash/Zsh/Python `shlex`. The result was previously `\n`, now it is `\\n`.

# 0.1.1

* Adds handling of `#` comments.

# 0.1.0

This is the initial release.
