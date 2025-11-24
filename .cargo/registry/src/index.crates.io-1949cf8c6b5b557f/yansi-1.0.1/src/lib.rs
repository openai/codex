//! A dead simple ANSI terminal color painting library.
//!
//! # Features
//!
//! Why *y*et another *ANSI* terminal coloring library? Here are some reasons:
//!
//!   * This library makes simple things _simple_: `use` [`Paint`] and go!
//!   * Zero dependencies by default. It really is simple.
//!   * Zero allocations except as needed by opt-in [wrapping](#wrapping).
//!   * [Automatic Windows support] for the vast majority (95%+) of Windows
//!     users.
//!   * [Featureful `no_std`], no-`alloc`, support with `default-features =
//!     false`.
//!   * [`Style` constructors are `const`]: store styles statically, even with
//!     dynamic conditions!
//!   * _Any_ type implementing a formatting trait can be styled, not just
//!     strings.
//!   * Styling can be [enabled] and [disabled] globally and [dynamically], on
//!     the fly.
//!   * A `Style` can be predicated on arbitrary [conditions](#per-style).
//!   * Formatting specifiers like `{:x}` and `{:08b}` are supported and
//!     preserved!
//!   * [Built-in (optional) conditions] for [TTY detection] and [common
//!     environment variables].
//!   * Arbitrary items can be [_masked_] for selective disabling.
//!   * Styling can [_wrap_] to preserve styling across resets.
//!   * Styling can [_linger_] beyond a single value.
//!   * Experimental support for [hyperlinking] is included.
//!   * The name `yansi` is pretty cool 😎.
//!
//! All that said, `yansi` borrows API ideas from older libraries as well as
//! implementation details from [`ansi_term`].
//!
//! [`ansi_term`]: https://crates.io/crates/ansi_term
//! [`colored`]: https://crates.io/crates/colored
//! [`term_painter`]: https://crates.io/crates/term-painter
//! [_masked_]: #masking
//! [_wrap_]: #wrapping
//! [_linger_]: #lingering
//! [enabled]: crate::enable
//! [disabled]: crate::disable
//! [dynamically]: crate::whenever
//! [enabled conditionally]: Condition
//! [TTY detection]: Condition#impl-Condition-1
//! [common environment variables]: Condition#impl-Condition-2
//! [Automatic Windows support]: #windows
//! [Built-in (optional) conditions]: Condition#built-in-conditions
//! [`Style` constructors are `const`]: #uniform-const-builders
//! [hyperlinking]: hyperlink
//! [Featureful `no_std`]: #crate-features
//!
//! # Usage
//!
//! The [`Paint`] trait is implemented for every type. Import it and call
//! chainable builder methods:
//!
//! ```rust
//! use yansi::Paint;
//!
//! println!("Testing, {}, {}, {}!",
//!     "Ready".bold(),
//!     "Set".yellow().italic(),
//!     "STOP".white().on_red().bright().underline().bold());
//! ```
//!
//! `>` Testing,
//! <b>Ready</b>,
//! <span style="color: yellow;"><i><b>Set</b></i></span>,
//! <span style="color: white; background: red;"><u><b>STOP</b></u></span>!
//!
//! The methods return a [`Painted`] type which consists of a [`Style`] and a
//! reference to the receiver. Displaying a [`Painted`] (via `print!()`,
//! `format!()`, etc) results in emitting ANSI escape codes that effectuate the
//! style.
//!
//! ## Uniform `const` Builders
//!
//! All builder methods are uniformly available for [`Style`], [`Color`], and
//! [`Painted`], which means you can chain calls across library types. All
//! methods are `const`, allowing creations of `const` or `static` [`Style`]s. A
//! `Style` can be directly applied to values with [`.paint()`](Paint::paint()),
//! from [`Paint::paint()`], available for every type:
//!
//! ```rust
//! use yansi::{Paint, Style, Color::*};
//!
//! // `const` constructors allow static `Style`s for easy reuse
//! static ALERT: Style = White.bright().underline().italic().on_red();
//!
//! println!("Testing, {}, {}, {}!",
//!     "Ready".bold(),
//!     "Set".yellow().bold(),
//!     "STOP".paint(ALERT));
//! ```
//!
//! `>` Testing,
//! <b>Ready</b>,
//! <span style="color: yellow;"><b>Set</b></span>,
//! <span style="color: white; background: red;"><u><em>STOP</em></u></span>!
//!
//! ## Conditional Styling
//!
//! ### Globally
//!
//! Styling is enabled by default but can be enabled and disabled globally via
//! [`enable()`] and [`disable()`]. When styling is disabled, no ANSI escape
//! codes are emitted, and [_masked_] values are omitted entirely.
//!
//! Global styling can also be dynamically enabled and disabled using
//! [`whenever()`] with an arbitrary [`Condition`]: a function that returns
//! `true` or `false`. This condition is evaluated each time a [`Painted`] item
//! is displayed. The associated styling is enabled, and mask values emitted,
//! exactly when and only when the condition returns `true`.
//!
//! ### Per-`Style`
//!
//! A specific `Style` can itself be conditionally applied by using
//! [`.whenever()`](Style::whenever()):
//!
//! ```rust
//! # #[cfg(feature = "detect-tty")] {
//! use yansi::{Paint, Style, Color::*, Condition};
//!
//! static WARNING: Style = Black.bold().on_yellow().whenever(Condition::STDERR_IS_TTY);
//!
//! eprintln!("{}", "Bees can sting!".paint(WARNING));
//! # }
//! ```
//!
//! With the above, if `stderr` is a TTY, then:
//! `>` <span style="background: yellow; color: black;"><b>Bees can sting!</b></span>
//!
//! If it is not a TTY, styling is not emitted:
//! `>` Bees can sting!
//!
//! See [`Condition`] for a list of built-in conditions which require enabling
//! crate features.
//!
//! # Quirks
//!
//! As a convenience, `yansi` implements several "quirks", applicable via
//! [`Quirk`] and the respective methods, that modify if and how styling is
//! presented to the terminal. These quirks do not correspond to any ANSI
//! styling sequences.
//!
//! ## Masking
//!
//! Items can be arbitrarily _masked_ with the [`mask()`](Paint::mask()) builder
//! method. Masked values are not emitted when styling is disabled, globally or
//! for a given style. This allows selective output based on whether styling is
//! enabled.
//!
//! One use for this feature is to print certain characters only when styling is
//! enabled. For instance, you might wish to emit the 🎨 emoji when coloring is
//! enabled but not otherwise. This can be accomplished by masking the emoji:
//!
//! ```rust
//! use yansi::Paint;
//!
//! println!("I like colors!{}", " 🎨".mask());
//! ```
//!
//! When styling is enabled, this prints: `>` I like colors! 🎨
//!
//! With styling disabled, this prints: `>` I like colors!
//!
//! ## Wrapping
//!
//! **Note:** _Either the `std` or `alloc` feature is required for wrapping.
//! `std` is enabled by default. See [crate features](#crate-features)._
//!
//! Styling can _wrap_ via [`Quirk::Wrap`] or the equivalent
//! [`wrap()`](Painted::wrap()) constructor. A wrapping style modifies any
//! styling resets emitted by the internal value so that they correspond to the
//! wrapping style. In other words, the "reset" style of the wrapped item is
//! modified to be the style being `.wrap()`d.
//!
//! Wrapping is useful in situations where opaque and arbitrary values must be
//! styled consistently irrespective of any existing styling. For example, a
//! generic logger might want to style messages based on log levels
//! consistently, even when those messages may already include styling. Wrapping
//! exists to enable such consistent styling:
//!
//! ```rust
//! use yansi::Paint;
//!
//! // Imagine that `inner` is opaque and we don't know it's styling.
//! let inner = format!("{} and {}", "Stop".red(), "Go".green());
//!
//! // We can use `wrap` to ensure anything in `inner` not styled is blue.
//! println!("Hey! {}", inner.blue().wrap());
//! ```
//!
//! Thanks to wrapping, this prints:
//! `>` Hey! <span style="color: blue">
//! <span style="color: red">Stop</span> and
//! <span style="color: green">Go</span>
//! </span>
//!
//! Without wrapping, the reset after `"Stop".red()` would not be overwritten:
//! `>` Hey! <span style="color: red">Stop</span> and <span style="color: green">Go</span>
//!
//! Wrapping incurs a performance cost due to an extra allocation and
//! replacement if the wrapped item has styling applied to it. Otherwise, it
//! does not allocate nor incur a meaningful performance cost.
//!
//! ## Lingering
//!
//! Styling can _linger_ beyond a single value via [`Quirk::Linger`] or the
//! equivalent [`linger()`](Painted::linger()) constructor. A lingering style
//! does not reset itself after being applied. In other words, the style lingers
//! on beyond the value it's applied to, until something else resets the
//! respective styling.
//!
//! The complement to lingering is force resetting via [`Quirk::Resetting`] or
//! the equivalent [`resetting()`](Painted::resetting()) constructor. Force
//! resetting, as the name implies, forces a reset suffix to be emitted after
//! the value, irrespective of any lingering applied. It can be used as a way to
//! finalize a lingering style.
//!
//! Lingering itself is useful in situations where a given style is to be
//! repeated across multiple values, or when style is intended to persist even
//! across values that are not styled with `yansi`. It also allows avoiding
//! unnecessarily repeated ANSI code sequences. The examples below illustrate
//! some scenarios in which lingering is useful:
//!
//! ```rust
//! use yansi::Paint;
//!
//! println!("Hello! {} {} things with {} {}?",
//!     "How".magenta().underline().linger(),
//!     "are".italic().linger(),
//!     "you".on_yellow(), // doesn't linger, so all styling is reset here
//!     "today".blue());
//! ```
//!
//! `>` Hello!
//! <span style="color: magenta;">
//!   <u>How <i>are things with <span style="background: yellow;">you</span></i></u>
//! </span>
//! <span style="color: blue;">today</span>?
//!
//! ```rust
//! use yansi::Paint;
//!
//! println!("Hello! {} {} things with {} {}?",
//!     "How".magenta().underline().linger(),
//!     "are".italic(), // doesn't linger, so all styling is reset here
//!     "you".on_yellow().linger(),
//!     "today".blue()); // doesn't linger; styling is reset
//! ```
//!
//! `>` Hello!
//! <span style="color: magenta;">
//!   <u>How <i>are</i></u>
//! </span>
//! things with
//! <span style="background: yellow;">
//! you
//! <span style="color: blue;">today</span></span>?
//!
//! ```rust
//! use yansi::Paint;
//!
//! println!("{} B {} {} {} F",
//!     "A".red().linger(),
//!     "C".underline().linger(),
//!     "D", // doesn't linger, but no styling applied, thus no reset
//!     "E".resetting());  // explicitly reset
//! ```
//!
//! `>` <span style="color: red;"> A B <u>C D E</u> </span> F
//!
//! ## Brightening
//!
//! Most pimrary colors are available in regular and _bright_ variants, e.g.,
//! [`Color::Red`] and [`Color::BrightRed`]. The [`Quirk::Bright`] and
//! [`Quirk::OnBright`] quirks, typically applied via
//! [`.bright()`](Painted::bright()) and [`.on_bright()`](Painted::on_bright()),
//! provide an alternative, convenient mechanism to select the bright variant of
//! the selected foreground or background color, respectively. The quirk
//! provides no additional colors and is equivalent to selecting the bright
//! variants directly.
//!
//! ```rust
//! use yansi::Paint;
//!
//! // These are all equivalent.
//! print!("{}", "Regular".red());
//! print!("{}", "Bright".bright_red());
//! print!("{}", "Bright".bright().red());
//! print!("{}", "Bright".red().bright());
//!
//! # static STYLE: yansi::Style = yansi::Color::Green.bold();
//! // The `bright` quirk lets use choose the bright variants of _any_ color,
//! // even when the color or style is unknown at the call site.
//! print!("{}", "Normal".paint(STYLE));
//! print!("{}", "Bright".paint(STYLE).bright());
//! ```
//!
//! `>` <span style="color: red;">Regular</span>
//! <span style="color: hotpink;">Bright</span>
//! <span style="color: hotpink;">Bright</span>
//! <span style="color: hotpink;">Bright</span>
//! <span style="color: green;"><b>Normal</b></span>
//! <span style="color: greenyellow;"><b>Bright</b></span>
//!
//! The `bright()` quirk can be applied before or after a color is selected
//! while having the same effect.
//!
//! # Windows
//!
//! Styling is supported and enabled automatically on Windows beginning with
//! the Windows 10 Anniversary Update, or about [96% of all Windows machines
//! worldwide](https://gs.statcounter.com/os-version-market-share/windows/desktop/worldwide),
//! and likely closer to 100% of developer machines (e.g., 99% of visitors to
//! [rocket.rs](https://rocket.rs) on Windows are on Windows 10+).
//!
//! Yansi enables styling support on Windows by querying the Windows API on the
//! first attempt to color. If support is available, it is enabled. If support
//! is not available, styling is disabled and no styling sequences are emitted.
//!
//! # Crate Features
//!
//! | Feature      | Default? | Also Enables | Notes                            |
//! |--------------|----------|--------------|----------------------------------|
//! | `std`        | **Y**    | `alloc`      | Use `std` library.               |
//! | `alloc`      | **Y**    |              | Use `alloc`. Enables [wrapping]. |
//! | `detect-tty` | N        | `std`        | See [optional conditions].       |
//! | `detect-env` | N        | `std`        | See [optional conditions].       |
//! | `hyperlink`  | N        | `std`        | Enables [hyperlinking] support.  |
//!
//! With `default-features = false`, this crate is `#[no_std]`.
//!
//! Without any features enabled, all functionality except [wrapping] is
//! available. To recover wrapping _with_ `#[no_std]`, set `default-features =
//! false` and enable the `alloc` feature, which requires `alloc` support.
//!
//! [optional conditions]: Condition#built-in-conditions
//! [wrapping]: #wrapping

#![doc(html_logo_url = "https://raw.githubusercontent.com/SergioBenitez/yansi/master/.github/yansi-logo.png")]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "_nightly", feature(doc_cfg))]
#![deny(missing_docs)]

// FIXME: Remove once `clear()` and `Quirk::Clear` are removed.
#![allow(useless_deprecated, deprecated)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

#[macro_use]
mod macros;
mod windows;
mod attr_quirk;
mod style;
mod color;
mod paint;
mod global;
mod condition;
mod set;

#[cfg(feature = "hyperlink")]
#[cfg_attr(feature = "_nightly", doc(cfg(feature = "hyperlink")))]
pub mod hyperlink;

pub use paint::{Painted, Paint};
pub use attr_quirk::{Attribute, Quirk};
pub use style::Style;
pub use color::Color;
pub use condition::Condition;
pub use global::{enable, whenever, disable, is_enabled};
