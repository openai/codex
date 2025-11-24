![Tracing — Structured, application-level diagnostics][splash]

[splash]: https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/splash.svg

# tracing-appender

Writers for logging events and spans

[![Crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (master)][docs-master-badge]][docs-master-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Documentation][docs-url] | [Chat][discord-url]

[crates-badge]: https://img.shields.io/crates/v/tracing-appender.svg
[crates-url]: https://crates.io/crates/tracing-appender/0.2.2
[docs-badge]: https://docs.rs/tracing-appender/badge.svg
[docs-url]: https://docs.rs/tracing-appender/0.2.2
[docs-master-badge]: https://img.shields.io/badge/docs-master-blue
[docs-master-url]: https://tracing.rs/tracing-appender
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: ../LICENSE
[actions-badge]: https://github.com/tokio-rs/tracing/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/tracing/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white
[discord-url]: https://discord.gg/EeF3cQw

## Overview

[`tracing`][tracing] is a framework for instrumenting Rust programs to 
collect structured, event-based diagnostic information. `tracing-appender` 
allows events and spans to be recorded in a non-blocking manner through a 
dedicated logging thread. It also provides a [`RollingFileAppender`][file_appender] 
that can be used with _or_ without the non-blocking writer.

*Compiler support: [requires `rustc` 1.63+][msrv]*

[msrv]: #supported-rust-versions

## Usage

Add the following to your `Cargo.toml`:
```toml
tracing-appender = "0.2"
```

This crate can be used in a few ways to record spans/events:
 - Using a [`RollingFileAppender`][file_appender] to write to a log file. 
 This is a blocking operation.
 - Using *any* type implementing [`std::io::Write`][write] in a 
 non-blocking fashion.
 - Using [`NonBlocking`][non_blocking] and [`RollingFileAppender`][file_appender] 
 together to write to log files in a non-blocking fashion.

## Rolling File Appender

```rust
fn main(){
    let file_appender = tracing_appender::rolling::hourly("/some/directory", "prefix.log");
}
```
This creates an hourly rotating file appender that writes to 
`/some/directory/prefix.log.YYYY-MM-DD-HH`. [`Rotation::DAILY`] and 
[`Rotation::NEVER`] are the other available options.

The file appender implements [`std::io::Write`][write]. To be used with 
[`tracing_subscriber::FmtSubscriber`][fmt_subscriber], it must be combined 
with a [`MakeWriter`][make_writer] implementation to be able to record 
tracing spans/event.

The [rolling] module's documentation provides more detail on how to use 
this file appender.

## Non-Blocking Writer
The example below demonstrates the construction of a `non_blocking` writer 
with an implementation of [`std::io::Writer`][write].

```rust
use std::io::Error;

struct TestWriter;

impl std::io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let buf_len = buf.len();
    
        println!("{:?}", buf);
        Ok(buf_len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn main() {
    let (non_blocking, _guard) = tracing_appender::non_blocking(TestWriter);
    tracing_subscriber::fmt().with_writer(non_blocking).init();
}
```
**Note:** `_guard` is a [`WorkerGuard`][guard] which is returned by 
`tracing_appender::non_blocking` to ensure buffered logs are flushed to 
their output in the case of abrupt terminations of a process. See 
[`WorkerGuard`][guard] module for more details.

The example below demonstrates the construction of a 
[`tracing_appender::non_blocking`][non_blocking] writer constructed with 
a [`std::io::Write`][write]:

```rust
fn main() {
    let (non_blocking, _guard) = tracing_appender::non_blocking(std::io::stdout());
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .init();
}
```

The [non_blocking] module's documentation provides more detail on how to 
use `non_blocking`.

## Non-Blocking Rolling File Appender

```rust
fn main() {
    let file_appender = tracing_appender::rolling::hourly("/some/directory", "prefix.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
   tracing_subscriber::fmt()
       .with_writer(non_blocking)
       .init();
}
```

[tracing]: https://docs.rs/tracing/latest/tracing/
[make_writer]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/trait.MakeWriter.html
[write]: https://doc.rust-lang.org/std/io/trait.Write.html
[non_blocking]: https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/index.html
[rolling]: https://docs.rs/tracing-appender/latest/tracing_appender/rolling/index.html
[guard]: https://docs.rs/tracing-appender/latest/tracing_appender/non_blocking/struct.WorkerGuard.html
[file_appender]: https://docs.rs/tracing-appender/latest/tracing_appender/rolling/struct.RollingFileAppender.html
[fmt_subscriber]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/struct.Subscriber.html

## Supported Rust Versions

`tracing-appender` is built against the latest stable release. The minimum supported
version is 1.63. The current `tracing-appender` version is not guaranteed to build on
Rust versions earlier than the minimum supported version.

Tracing follows the same compiler support policies as the rest of the Tokio
project. The current stable Rust compiler and the three most recent minor
versions before it will always be supported. For example, if the current
stable compiler version is 1.69, the minimum supported version will not be
increased past 1.66, three minor versions prior. Increasing the minimum
supported compiler version is not considered a semver breaking change as
long as doing so complies with this policy.

## License

This project is licensed under the [MIT license](../LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
