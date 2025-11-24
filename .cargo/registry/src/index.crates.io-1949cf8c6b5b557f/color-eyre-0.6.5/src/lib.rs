//! An error report handler for panics and the [`eyre`] crate for colorful, consistent, and well
//! formatted error reports for all kinds of errors.
//!
//! ## TLDR
//!
//! `color_eyre` helps you build error reports that look like this:
//!
//! <pre><span style="color: #06989A"><b>color-eyre</b></span> on <span style="color: #75507B"><b> hooked</b></span> <span style="color: #CC0000"><b>[$!] </b></span>is <span style="color: #FF8700"><b>📦 v0.5.0</b></span> via <span style="color: #CC0000"><b>🦀 v1.44.0</b></span>
//! <span style="color: #4E9A06"><b>❯</b></span> cargo run --example custom_section
//! <span style="color: #4E9A06"><b>    Finished</b></span> dev [unoptimized + debuginfo] target(s) in 0.04s
//! <span style="color: #4E9A06"><b>     Running</b></span> `target/debug/examples/custom_section`
//! Error:
//!    0: <span style="color: #F15D22">Unable to read config</span>
//!    1: <span style="color: #F15D22">cmd exited with non-zero status code</span>
//!
//! Stderr:
//!    cat: fake_file: No such file or directory
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ SPANTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//!    0: <span style="color: #F15D22">custom_section::output2</span> with <span style="color: #34E2E2">self=&quot;cat&quot; &quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">14</span>
//!    1: <span style="color: #F15D22">custom_section::read_file</span> with <span style="color: #34E2E2">path=&quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">58</span>
//!    2: <span style="color: #F15D22">custom_section::read_config</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">63</span>
//!
//! <span style="color: #34E2E2">Suggestion</span>: try using a file that exists next time</pre>
//!
//! ## Setup
//!
//! Add the following to your toml file:
//!
//! ```toml
//! [dependencies]
//! color-eyre = "0.6"
//! ```
//!
//! And install the panic and error report handlers:
//!
//! ```rust
//! use color_eyre::eyre::Result;
//!
//! fn main() -> Result<()> {
//!     color_eyre::install()?;
//!
//!     // ...
//!     # Ok(())
//! }
//! ```
//!
//! ### Disabling tracing support
//!
//! If you don't plan on using `tracing_error` and `SpanTrace` you can disable the
//! tracing integration to cut down on unused dependencies:
//!
//! ```toml
//! [dependencies]
//! color-eyre = { version = "0.6", default-features = false }
//! ```
//!
//! ### Disabling SpanTrace capture by default
//!
//! color-eyre defaults to capturing span traces. This is because `SpanTrace`
//! capture is significantly cheaper than `Backtrace` capture. However, like
//! backtraces, span traces are most useful for debugging applications, and it's
//! not uncommon to want to disable span trace capture by default to keep noise out
//! developer.
//!
//! To disable span trace capture you must explicitly set one of the env variables
//! that regulate `SpanTrace` capture to `"0"`:
//!
//! ```rust
//! if std::env::var("RUST_SPANTRACE").is_err() {
//!     std::env::set_var("RUST_SPANTRACE", "0");
//! }
//! ```
//!
//! ### Improving perf on debug builds
//!
//! In debug mode `color-eyre` behaves noticably worse than `eyre`. This is caused
//! by the fact that `eyre` uses `std::backtrace::Backtrace` instead of
//! `backtrace::Backtrace`. The std version of backtrace is precompiled with
//! optimizations, this means that whether or not you're in debug mode doesn't
//! matter much for how expensive backtrace capture is, it will always be in the
//! 10s of milliseconds to capture. A debug version of `backtrace::Backtrace`
//! however isn't so lucky, and can take an order of magnitude more time to capture
//! a backtrace compared to its std counterpart.
//!
//! Cargo [profile
//! overrides](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides)
//! can be used to mitigate this problem. By configuring your project to always
//! build `backtrace` with optimizations you should get the same performance from
//! `color-eyre` that you're used to with `eyre`. To do so add the following to
//! your Cargo.toml:
//!
//! ```toml
//! [profile.dev.package.backtrace]
//! opt-level = 3
//! ```
//!
//! ## Features
//!
//! ### Multiple report format verbosity levels
//!
//! `color-eyre` provides 3 different report formats for how it formats the captured `SpanTrace`
//! and `Backtrace`, minimal, short, and full. Take the below snippets of the output produced by [`examples/usage.rs`]:
//!
//! ---
//!
//! Running `cargo run --example usage` without `RUST_LIB_BACKTRACE` set will produce a minimal
//! report like this:
//!
//! <pre><span style="color: #06989A"><b>color-eyre</b></span> on <span style="color: #75507B"><b> hooked</b></span> <span style="color: #CC0000"><b>[$!] </b></span>is <span style="color: #FF8700"><b>📦 v0.5.0</b></span> via <span style="color: #CC0000"><b>🦀 v1.44.0</b></span> took <span style="color: #C4A000"><b>2s</b></span>
//! <span style="color: #CC0000"><b>❯</b></span> cargo run --example usage
//! <span style="color: #4E9A06"><b>    Finished</b></span> dev [unoptimized + debuginfo] target(s) in 0.04s
//! <span style="color: #4E9A06"><b>     Running</b></span> `target/debug/examples/usage`
//! <span style="color: #A1A1A1">Jul 05 19:15:58.026 </span><span style="color: #4E9A06"> INFO</span> <b>read_config</b>:<b>read_file{</b>path=&quot;fake_file&quot;<b>}</b>: Reading file
//! Error:
//!    0: <span style="color: #F15D22">Unable to read config</span>
//!    1: <span style="color: #F15D22">No such file or directory (os error 2)</span>
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ SPANTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//!    0: <span style="color: #F15D22">usage::read_file</span> with <span style="color: #34E2E2">path=&quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">32</span>
//!    1: <span style="color: #F15D22">usage::read_config</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">38</span>
//!
//! <span style="color: #34E2E2">Suggestion</span>: try using a file that exists next time</pre>
//!
//! <br>
//!
//! Running `RUST_LIB_BACKTRACE=1 cargo run --example usage` tells `color-eyre` to use the short
//! format, which additionally capture a [`backtrace::Backtrace`]:
//!
//! <pre><span style="color: #06989A"><b>color-eyre</b></span> on <span style="color: #75507B"><b> hooked</b></span> <span style="color: #CC0000"><b>[$!] </b></span>is <span style="color: #FF8700"><b>📦 v0.5.0</b></span> via <span style="color: #CC0000"><b>🦀 v1.44.0</b></span>
//! <span style="color: #CC0000"><b>❯</b></span> RUST_LIB_BACKTRACE=1 cargo run --example usage
//! <span style="color: #4E9A06"><b>    Finished</b></span> dev [unoptimized + debuginfo] target(s) in 0.04s
//! <span style="color: #4E9A06"><b>     Running</b></span> `target/debug/examples/usage`
//! <span style="color: #A1A1A1">Jul 05 19:16:02.853 </span><span style="color: #4E9A06"> INFO</span> <b>read_config</b>:<b>read_file{</b>path=&quot;fake_file&quot;<b>}</b>: Reading file
//! Error:
//!    0: <span style="color: #F15D22">Unable to read config</span>
//!    1: <span style="color: #F15D22">No such file or directory (os error 2)</span>
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ SPANTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//!    0: <span style="color: #F15D22">usage::read_file</span> with <span style="color: #34E2E2">path=&quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">32</span>
//!    1: <span style="color: #F15D22">usage::read_config</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">38</span>
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ BACKTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!   <span style="color: #34E2E2">                              ⋮ 5 frames hidden ⋮                               </span>
//!    6: <span style="color: #F15D22">usage::read_file</span><span style="color: #88807C">::haee210cb22460af3</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">35</span>
//!    7: <span style="color: #F15D22">usage::read_config</span><span style="color: #88807C">::ha649ef4ec333524d</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">40</span>
//!    8: <span style="color: #F15D22">usage::main</span><span style="color: #88807C">::hbe443b50eac38236</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">11</span>
//!   <span style="color: #34E2E2">                              ⋮ 10 frames hidden ⋮                              </span>
//!
//! <span style="color: #34E2E2">Suggestion</span>: try using a file that exists next time</pre>
//!
//! <br>
//!
//! Finally, running `RUST_LIB_BACKTRACE=full cargo run --example usage` tells `color-eyre` to use
//! the full format, which in addition to the above will attempt to include source lines where the
//! error originated from, assuming it can find them on the disk.
//!
//! <pre><span style="color: #06989A"><b>color-eyre</b></span> on <span style="color: #75507B"><b> hooked</b></span> <span style="color: #CC0000"><b>[$!] </b></span>is <span style="color: #FF8700"><b>📦 v0.5.0</b></span> via <span style="color: #CC0000"><b>🦀 v1.44.0</b></span>
//! <span style="color: #CC0000"><b>❯</b></span> RUST_LIB_BACKTRACE=full cargo run --example usage
//! <span style="color: #4E9A06"><b>    Finished</b></span> dev [unoptimized + debuginfo] target(s) in 0.05s
//! <span style="color: #4E9A06"><b>     Running</b></span> `target/debug/examples/usage`
//! <span style="color: #A1A1A1">Jul 05 19:16:06.335 </span><span style="color: #4E9A06"> INFO</span> <b>read_config</b>:<b>read_file{</b>path=&quot;fake_file&quot;<b>}</b>: Reading file
//! Error:
//!    0: <span style="color: #F15D22">Unable to read config</span>
//!    1: <span style="color: #F15D22">No such file or directory (os error 2)</span>
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ SPANTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//!    0: <span style="color: #F15D22">usage::read_file</span> with <span style="color: #34E2E2">path=&quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">32</span>
//!         30 │ }
//!         31 │
//!   <b>      32 &gt; #[instrument]</b>
//!         33 │ fn read_file(path: &amp;str) -&gt; Result&lt;(), Report&gt; {
//!         34 │     info!(&quot;Reading file&quot;);
//!    1: <span style="color: #F15D22">usage::read_config</span>
//!       at <span style="color: #75507B">examples/usage.rs</span>:<span style="color: #75507B">38</span>
//!         36 │ }
//!         37 │
//!   <b>      38 &gt; #[instrument]</b>
//!         39 │ fn read_config() -&gt; Result&lt;(), Report&gt; {
//!         40 │     read_file(&quot;fake_file&quot;)
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ BACKTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!   <span style="color: #34E2E2">                              ⋮ 5 frames hidden ⋮                               </span>
//!    6: <span style="color: #F15D22">usage::read_file</span><span style="color: #88807C">::haee210cb22460af3</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">35</span>
//!         33 │ fn read_file(path: &amp;str) -&gt; Result&lt;(), Report&gt; {
//!         34 │     info!(&quot;Reading file&quot;);
//!   <span style="color: #D3D7CF"><b>      35 &gt;     Ok(std::fs::read_to_string(path).map(drop)?)</b></span>
//!         36 │ }
//!         37 │
//!    7: <span style="color: #F15D22">usage::read_config</span><span style="color: #88807C">::ha649ef4ec333524d</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">40</span>
//!         38 │ #[instrument]
//!         39 │ fn read_config() -&gt; Result&lt;(), Report&gt; {
//!   <span style="color: #D3D7CF"><b>      40 &gt;     read_file(&quot;fake_file&quot;)</b></span>
//!         41 │         .wrap_err(&quot;Unable to read config&quot;)
//!         42 │         .suggestion(&quot;try using a file that exists next time&quot;)
//!    8: <span style="color: #F15D22">usage::main</span><span style="color: #88807C">::hbe443b50eac38236</span>
//!       at <span style="color: #75507B">/home/jlusby/git/yaahc/color-eyre/examples/usage.rs</span>:<span style="color: #75507B">11</span>
//!          9 │     color_eyre::install()?;
//!         10 │
//!   <span style="color: #D3D7CF"><b>      11 &gt;     Ok(read_config()?)</b></span>
//!         12 │ }
//!         13 │
//!   <span style="color: #34E2E2">                              ⋮ 10 frames hidden ⋮                              </span>
//!
//! <span style="color: #34E2E2">Suggestion</span>: try using a file that exists next time</pre>
//!
//! ### Custom `Section`s for error reports via [`Section`] trait
//!
//! The `section` module provides helpers for adding extra sections to error
//! reports. Sections are disinct from error messages and are displayed
//! independently from the chain of errors. Take this example of adding sections
//! to contain `stderr` and `stdout` from a failed command, taken from
//! [`examples/custom_section.rs`]:
//!
//! ```rust
//! use color_eyre::{eyre::eyre, SectionExt, Section, eyre::Report};
//! use std::process::Command;
//! use tracing::instrument;
//!
//! trait Output {
//!     fn output2(&mut self) -> Result<String, Report>;
//! }
//!
//! impl Output for Command {
//!     #[instrument]
//!     fn output2(&mut self) -> Result<String, Report> {
//!         let output = self.output()?;
//!
//!         let stdout = String::from_utf8_lossy(&output.stdout);
//!
//!         if !output.status.success() {
//!             let stderr = String::from_utf8_lossy(&output.stderr);
//!             Err(eyre!("cmd exited with non-zero status code"))
//!                 .with_section(move || stdout.trim().to_string().header("Stdout:"))
//!                 .with_section(move || stderr.trim().to_string().header("Stderr:"))
//!         } else {
//!             Ok(stdout.into())
//!         }
//!     }
//! }
//! ```
//!
//! ---
//!
//! Here we have an function that, if the command exits unsuccessfully, creates a
//! report indicating the failure and attaches two sections, one for `stdout` and
//! one for `stderr`.
//!
//! Running `cargo run --example custom_section` shows us how these sections are
//! included in the output:
//!
//! <pre><span style="color: #06989A"><b>color-eyre</b></span> on <span style="color: #75507B"><b> hooked</b></span> <span style="color: #CC0000"><b>[$!] </b></span>is <span style="color: #FF8700"><b>📦 v0.5.0</b></span> via <span style="color: #CC0000"><b>🦀 v1.44.0</b></span> took <span style="color: #C4A000"><b>2s</b></span>
//! <span style="color: #CC0000"><b>❯</b></span> cargo run --example custom_section
//! <span style="color: #4E9A06"><b>    Finished</b></span> dev [unoptimized + debuginfo] target(s) in 0.04s
//! <span style="color: #4E9A06"><b>     Running</b></span> `target/debug/examples/custom_section`
//! Error:
//!    0: <span style="color: #F15D22">Unable to read config</span>
//!    1: <span style="color: #F15D22">cmd exited with non-zero status code</span>
//!
//! Stderr:
//!    cat: fake_file: No such file or directory
//!
//!   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ SPANTRACE ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!
//!    0: <span style="color: #F15D22">custom_section::output2</span> with <span style="color: #34E2E2">self=&quot;cat&quot; &quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">14</span>
//!    1: <span style="color: #F15D22">custom_section::read_file</span> with <span style="color: #34E2E2">path=&quot;fake_file&quot;</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">58</span>
//!    2: <span style="color: #F15D22">custom_section::read_config</span>
//!       at <span style="color: #75507B">examples/custom_section.rs</span>:<span style="color: #75507B">63</span>
//!
//! <span style="color: #34E2E2">Suggestion</span>: try using a file that exists next time</pre>
//!
//! Only the `Stderr:` section actually gets included. The `cat` command fails,
//! so stdout ends up being empty and is skipped in the final report. This gives
//! us a short and concise error report indicating exactly what was attempted and
//! how it failed.
//!
//! ### Aggregating multiple errors into one report
//!
//! It's not uncommon for programs like batched task runners or parsers to want
//! to return an error with multiple sources. The current version of the error
//! trait does not support this use case very well, though there is [work being
//! done](https://github.com/rust-lang/rfcs/pull/2895) to improve this.
//!
//! For now however one way to work around this is to compose errors outside the
//! error trait. `color-eyre` supports such composition in its error reports via
//! the `Section` trait.
//!
//! For an example of how to aggregate errors check out [`examples/multiple_errors.rs`].
//!
//! ### Custom configuration for `color-backtrace` for setting custom filters and more
//!
//! The pretty printing for backtraces and span traces isn't actually provided by
//! `color-eyre`, but instead comes from its dependencies [`color-backtrace`] and
//! [`color-spantrace`]. `color-backtrace` in particular has many more features
//! than are exported by `color-eyre`, such as customized color schemes, panic
//! hooks, and custom frame filters. The custom frame filters are particularly
//! useful when combined with `color-eyre`, so to enable their usage we provide
//! the `install` fn for setting up a custom `BacktracePrinter` with custom
//! filters installed.
//!
//! For an example of how to setup custom filters, check out [`examples/custom_filter.rs`].
//!
//! [`eyre`]: https://docs.rs/eyre
//! [`tracing-error`]: https://docs.rs/tracing-error
//! [`color-backtrace`]: https://docs.rs/color-backtrace
//! [`eyre::EyreHandler`]: https://docs.rs/eyre/*/eyre/trait.EyreHandler.html
//! [`backtrace::Backtrace`]: https://docs.rs/backtrace/*/backtrace/struct.Backtrace.html
//! [`tracing_error::SpanTrace`]: https://docs.rs/tracing-error/*/tracing_error/struct.SpanTrace.html
//! [`color-spantrace`]: https://github.com/yaahc/color-spantrace
//! [`Section`]: https://docs.rs/color-eyre/*/color_eyre/trait.Section.html
//! [`eyre::Report`]: https://docs.rs/eyre/*/eyre/struct.Report.html
//! [`eyre::Result`]: https://docs.rs/eyre/*/eyre/type.Result.html
//! [`Handler`]: https://docs.rs/color-eyre/*/color_eyre/struct.Handler.html
//! [`examples/usage.rs`]: https://github.com/yaahc/color-eyre/blob/master/examples/usage.rs
//! [`examples/custom_filter.rs`]: https://github.com/yaahc/color-eyre/blob/master/examples/custom_filter.rs
//! [`examples/custom_section.rs`]: https://github.com/yaahc/color-eyre/blob/master/examples/custom_section.rs
//! [`examples/multiple_errors.rs`]: https://github.com/yaahc/color-eyre/blob/master/examples/multiple_errors.rs
#![doc(html_root_url = "https://docs.rs/color-eyre/0.6.2")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(
    missing_docs,
    rustdoc::missing_doc_code_examples,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    unused,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
#![allow(clippy::try_err)]

use std::sync::Arc;

use backtrace::Backtrace;
pub use eyre;
#[doc(hidden)]
pub use eyre::Report;
#[doc(hidden)]
pub use eyre::Result;
pub use owo_colors;
use section::help::HelpInfo;
#[doc(hidden)]
pub use section::Section as Help;
pub use section::{IndentedSection, Section, SectionExt};
#[cfg(feature = "capture-spantrace")]
use tracing_error::SpanTrace;
#[doc(hidden)]
pub use Handler as Context;

pub mod config;
mod fmt;
mod handler;
pub(crate) mod private;
pub mod section;
mod writers;

/// A custom handler type for [`eyre::Report`] which provides colorful error
/// reports and [`tracing-error`] support.
///
/// # Details
///
/// This type is not intended to be used directly, prefer using it via the
/// [`color_eyre::Report`] and [`color_eyre::Result`] type aliases.
///
/// [`eyre::Report`]: https://docs.rs/eyre/*/eyre/struct.Report.html
/// [`tracing-error`]: https://docs.rs/tracing-error
/// [`color_eyre::Report`]: type.Report.html
/// [`color_eyre::Result`]: type.Result.html
pub struct Handler {
    filters: Arc<[Box<config::FilterCallback>]>,
    backtrace: Option<Backtrace>,
    suppress_backtrace: bool,
    #[cfg(feature = "capture-spantrace")]
    span_trace: Option<SpanTrace>,
    sections: Vec<HelpInfo>,
    display_env_section: bool,
    #[cfg(feature = "track-caller")]
    display_location_section: bool,
    #[cfg(feature = "issue-url")]
    issue_url: Option<String>,
    #[cfg(feature = "issue-url")]
    issue_metadata:
        std::sync::Arc<Vec<(String, Box<dyn std::fmt::Display + Send + Sync + 'static>)>>,
    #[cfg(feature = "issue-url")]
    issue_filter: std::sync::Arc<config::IssueFilterCallback>,
    theme: crate::config::Theme,
    #[cfg(feature = "track-caller")]
    location: Option<&'static std::panic::Location<'static>>,
}

/// The kind of type erased error being reported
#[cfg(feature = "issue-url")]
#[cfg_attr(docsrs, doc(cfg(feature = "issue-url")))]
pub enum ErrorKind<'a> {
    /// A non recoverable error aka `panic!`
    NonRecoverable(&'a dyn std::any::Any),
    /// A recoverable error aka `impl std::error::Error`
    Recoverable(&'a (dyn std::error::Error + 'static)),
}

/// Install the default panic and error report hooks
///
/// # Details
///
/// This function must be called to enable the customization of `eyre::Report`
/// provided by `color-eyre`. This function should be called early, ideally
/// before any errors could be encountered.
///
/// Only the first install will succeed. Calling this function after another
/// report handler has been installed will cause an error. **Note**: This
/// function _must_ be called before any `eyre::Report`s are constructed to
/// prevent the default handler from being installed.
///
/// Installing a global theme in `color_spantrace` manually (by calling
/// `color_spantrace::set_theme` or `color_spantrace::colorize` before
/// `install` is called) will result in an error if this function is called.
///
/// # Examples
///
/// ```rust
/// use color_eyre::eyre::Result;
///
/// fn main() -> Result<()> {
///     color_eyre::install()?;
///
///     // ...
///     # Ok(())
/// }
/// ```
pub fn install() -> Result<(), crate::eyre::Report> {
    config::HookBuilder::default().install()
}
