log
===

A Rust library providing a lightweight logging *facade*.

[![Build status](https://img.shields.io/github/actions/workflow/status/rust-lang/log/main.yml?branch=master)](https://github.com/rust-lang/log/actions)
[![Latest version](https://img.shields.io/crates/v/log.svg)](https://crates.io/crates/log)
[![Documentation](https://docs.rs/log/badge.svg)](https://docs.rs/log)
![License](https://img.shields.io/crates/l/log.svg)

* [`log` documentation](https://docs.rs/log)

A logging facade provides a single logging API that abstracts over the actual
logging implementation. Libraries can use the logging API provided by this
crate, and the consumer of those libraries can choose the logging
implementation that is most suitable for its use case.


## Minimum supported `rustc`

`1.61.0+`

This version is explicitly tested in CI and may be bumped in any release as needed. Maintaining compatibility with older compilers is a priority though, so the bar for bumping the minimum supported version is set very high. Any changes to the supported minimum version will be called out in the release notes.

## Usage

### In libraries

Libraries should link only to the `log` crate, and use the provided macros to
log whatever information will be useful to downstream consumers:

```toml
[dependencies]
log = "0.4"
```

```rust
use log::{info, trace, warn};

pub fn shave_the_yak(yak: &mut Yak) {
    trace!("Commencing yak shaving");

    loop {
        match find_a_razor() {
            Ok(razor) => {
                info!("Razor located: {razor}");
                yak.shave(razor);
                break;
            }
            Err(err) => {
                warn!("Unable to locate a razor: {err}, retrying");
            }
        }
    }
}
```

### In executables

In order to produce log output, executables have to use a logger implementation compatible with the facade.
There are many available implementations to choose from, here are some options:

* Simple minimal loggers:
    * [`env_logger`](https://docs.rs/env_logger/*/env_logger/)
    * [`colog`](https://docs.rs/colog/*/colog/)
    * [`simple_logger`](https://docs.rs/simple_logger/*/simple_logger/)
    * [`simplelog`](https://docs.rs/simplelog/*/simplelog/)
    * [`pretty_env_logger`](https://docs.rs/pretty_env_logger/*/pretty_env_logger/)
    * [`stderrlog`](https://docs.rs/stderrlog/*/stderrlog/)
    * [`flexi_logger`](https://docs.rs/flexi_logger/*/flexi_logger/)
    * [`call_logger`](https://docs.rs/call_logger/*/call_logger/)
    * [`std-logger`](https://docs.rs/std-logger/*/std_logger/)
    * [`structured-logger`](https://docs.rs/structured-logger/latest/structured_logger/)
    * [`clang_log`](https://docs.rs/clang_log/latest/clang_log)
    * [`ftail`](https://docs.rs/ftail/latest/ftail/)
* Complex configurable frameworks:
    * [`log4rs`](https://docs.rs/log4rs/*/log4rs/)
    * [`logforth`](https://docs.rs/logforth/*/logforth/)
    * [`fern`](https://docs.rs/fern/*/fern/)
    * [`spdlog-rs`](https://docs.rs/spdlog-rs/*/spdlog/)
* Adaptors for other facilities:
    * [`syslog`](https://docs.rs/syslog/*/syslog/)
    * [`systemd-journal-logger`](https://docs.rs/systemd-journal-logger/*/systemd_journal_logger/)
    * [`slog-stdlog`](https://docs.rs/slog-stdlog/*/slog_stdlog/)
    * [`android_log`](https://docs.rs/android_log/*/android_log/)
    * [`win_dbg_logger`](https://docs.rs/win_dbg_logger/*/win_dbg_logger/)
    * [`db_logger`](https://docs.rs/db_logger/*/db_logger/)
    * [`log-to-defmt`](https://docs.rs/log-to-defmt/*/log_to_defmt/)
    * [`logcontrol-log`](https://docs.rs/logcontrol-log/*/logcontrol_log/)
* For WebAssembly binaries:
    * [`console_log`](https://docs.rs/console_log/*/console_log/)
* For dynamic libraries:
    * You may need to construct [an FFI-safe wrapper over `log`](https://github.com/rust-lang/log/issues/421) to initialize in your libraries.
* Utilities:
    * [`log_err`](https://docs.rs/log_err/*/log_err/)
    * [`log-reload`](https://docs.rs/log-reload/*/log_reload/)
    * [`alterable_logger`](https://docs.rs/alterable_logger/*/alterable_logger)

Executables should choose a logger implementation and initialize it early in the
runtime of the program. Logger implementations will typically include a
function to do this. Any log messages generated before the logger is
initialized will be ignored.

The executable itself may use the `log` crate to log as well.

## Structured logging

If you enable the `kv` feature, you can associate structured data with your log records:

```rust
use log::{info, trace, warn};

pub fn shave_the_yak(yak: &mut Yak) {
    // `yak:serde` will capture `yak` using its `serde::Serialize` impl
    //
    // You could also use `:?` for `Debug`, or `:%` for `Display`. For a
    // full list, see the `log` crate documentation
    trace!(target = "yak_events", yak:serde; "Commencing yak shaving");

    loop {
        match find_a_razor() {
            Ok(razor) => {
                info!(razor; "Razor located");
                yak.shave(razor);
                break;
            }
            Err(e) => {
                // `e:err` will capture `e` using its `std::error::Error` impl
                warn!(e:err; "Unable to locate a razor, retrying");
            }
        }
    }
}
```
