//! # Documentation: FAQ
//!
//! 1. [Comparisons](#comparisons)
//!    1. [How does `clap` compare to structopt?](#how-does-clap-compare-to-structopt)
//!    2. [What are some reasons to use `clap`? (The Pitch)](#what-are-some-reasons-to-use-clap-the-pitch)
//!    3. [What are some reasons *not* to use `clap`? (The Anti Pitch)](#what-are-some-reasons-not-to-use-clap-the-anti-pitch)
//!    4. [Reasons to use `clap`](#reasons-to-use-clap)
//! 2. [How many approaches are there to create a parser?](#how-many-approaches-are-there-to-create-a-parser)
//! 3. [When should I use the builder vs derive APIs?](#when-should-i-use-the-builder-vs-derive-apis)
//! 4. [Why is there a default subcommand of help?](#why-is-there-a-default-subcommand-of-help)
//!
//! ### Comparisons
//!
//! First, let me say that these comparisons are highly subjective, and not meant
//! in a critical or harsh manner. All the argument parsing libraries out there (to
//! include `clap`) have their own strengths and weaknesses. Sometimes it just
//! comes down to personal taste when all other factors are equal. When in doubt,
//! try them all and pick one that you enjoy :). There's plenty of room in the Rust
//! community for multiple implementations!
//!
//! For less detailed but more broad comparisons, see
//! [argparse-benchmarks](https://github.com/rust-cli/argparse-benchmarks-rs).
//!
//! #### How does `clap` compare to [structopt](https://github.com/TeXitoi/structopt)?
//!
//! Simple! `clap` *is* `structopt`.  `structopt` started as a derive API built on
//! top of clap v2.  With clap v3, we've forked structopt and integrated it
//! directly into clap.  structopt is in
//! [maintenance mode](https://github.com/TeXitoi/structopt/issues/516#issuecomment-989566094)
//! with the release of `clap_derive`.
//!
//! The benefits of integrating `structopt` and `clap` are:
//! - Easier cross-linking in documentation
//! - Documentation parity
//! - Tighter design feedback loop, ensuring all new features are designed with
//!   derives in mind and easier to change `clap` in response to `structopt` bugs.
//! - Clearer endorsement of `structopt`
//!
//! See also
//! - [`clap` v3 CHANGELOG](https://github.com/clap-rs/clap/blob/v3-master/CHANGELOG.md#300---2021-12-31)
//! - [`structopt` migration guide](https://github.com/clap-rs/clap/blob/v3-master/CHANGELOG.md#migrate-structopt)
//!
//! #### What are some reasons to use `clap`? (The Pitch)
//!
//! `clap` is as fast, and as lightweight as possible while still giving all the features you'd expect from a modern argument parser. In fact, for the amount and type of features `clap` offers it remains about as fast as `getopts`. If you use `clap`, when you just need some simple arguments parsed, you'll find it's a walk in the park. `clap` also makes it possible to represent extremely complex and advanced requirements without too much thought. `clap` aims to be intuitive, easy to use, and fully capable for wide variety use cases and needs.
//!
//! #### What are some reasons *not* to use `clap`? (The Anti Pitch)
//!
//! Depending on the style in which you choose to define the valid arguments, `clap` can be very verbose. `clap` also offers so many finetuning knobs and dials, that learning everything can seem overwhelming. I strive to keep the simple cases simple, but when turning all those custom dials it can get complex. `clap` is also opinionated about parsing. Even though so much can be tweaked and tuned with `clap` (and I'm adding more all the time), there are still certain features which `clap` implements in specific ways that may be contrary to some users' use-cases.
//!
//! #### Reasons to use `clap`
//!
//!  * You want all the nice CLI features your users may expect, yet you don't want to implement them all yourself. You'd like to focus on your application, not argument parsing.
//!  * In addition to the point above, you don't want to sacrifice performance to get all those nice features.
//!  * You have complex requirements/conflicts between your various valid args.
//!  * You want to use subcommands (although other libraries also support subcommands, they are not nearly as feature rich as those provided by `clap`).
//!  * You want some sort of custom validation built into the argument parsing process, instead of as part of your application (which allows for earlier failures, better error messages, more cohesive experience, etc.).
//!
//! ### How many approaches are there to create a parser?
//!
//! The following APIs are supported:
//! - [Derive][crate::_derive::_tutorial]
//! - [Builder][crate::_tutorial]
//!
//! Previously, we supported:
//! - [YAML](https://github.com/clap-rs/clap/issues/3087)
//! - [docopt](http://docopt.org/)-inspired [usage parser](https://github.com/clap-rs/clap/issues/3086)
//! - [`clap_app!`](https://github.com/clap-rs/clap/issues/2835)
//!
//! There are also experiments with other APIs:
//! - [fncmd](https://github.com/yuhr/fncmd): function attribute
//! - [clap-serde](https://github.com/aobatact/clap-serde): create a `Command` from a deserializer
//!
//! ### When should I use the builder vs derive APIs?
//!
//! Our default answer is to use the [Derive API][crate::_derive::_tutorial]:
//! - Easier to read, write, and modify
//! - Easier to keep the argument declaration and reading of argument in sync
//! - Easier to reuse, e.g. [clap-verbosity-flag](https://crates.io/crates/clap-verbosity-flag)
//!
//! The [Builder API][crate::_tutorial] is a lower-level API that someone might want to use for
//! - Faster compile times if you aren't already using other procedural macros
//! - More flexibility, e.g. you can look up the [argument's values][crate::ArgMatches::get_many],
//!   their [ordering with other arguments][crate::ArgMatches::indices_of], and [what set
//!   them][crate::ArgMatches::value_source].  The Derive API can only report values and not
//!   indices of or other data.
//!
//! You can [interop between Derive and Builder APIs][crate::_derive#mixing-builder-and-derive-apis].
//!
//! ### Why is there a default subcommand of help?
//!
//! There is only a default subcommand of `help` when other subcommands have been defined manually. So it's opt-in(ish), being that you only get a `help` subcommand if you're actually using subcommands.
//!
//! Also, if the user defined a `help` subcommand themselves, the auto-generated one wouldn't be added (meaning it's only generated if the user hasn't defined one themselves).
//!
