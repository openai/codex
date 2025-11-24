@rem Copyright 2024 The Fuchsia Authors

@rem Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
@rem <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
@rem license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
@rem This file may not be copied, modified, or distributed except according to
@rem those terms.

@rem Build `cargo-zerocopy` without any RUSTFLAGS set in the environment
@set TEMP_RUSTFLAGS=%RUSTFLAGS%
@set RUSTFLAGS=
@cargo +stable build --manifest-path tools/Cargo.toml -p cargo-zerocopy -q
@set RUSTFLAGS=%TEMP_RUSTFLAGS%
@set TEMP_RUSTFLAGS=
@rem Thin wrapper around the `cargo-zerocopy` binary in `tools/cargo-zerocopy`
@tools\target\debug\cargo-zerocopy %*
