//! # CFG Aliases
//!
//! CFG Aliases is a tiny utility to help save you a lot of effort with long winded `#[cfg()]` checks. This crate provides a single [`cfg_aliases!`] macro that doesn't have any dependencies and specifically avoids pulling in `syn` or `quote` so that the impact on your comile times should be negligible.
//!
//! You use the the [`cfg_aliases!`] macro in your `build.rs` script to define aliases such as `x11` that could then be used in the `cfg` attribute or macro for conditional compilation: `#[cfg(x11)]`.
//!
//! ## Example
//!
//! **Cargo.toml:**
//!
//! ```toml
//! [build-dependencies]
//! cfg_aliases = "0.1.0"
//! ```
//!
//! **build.rs:**
//!
//! ```rust
//! use cfg_aliases::cfg_aliases;
//!
//! fn main() {
//!     // Setup cfg aliases
//!     cfg_aliases! {
//!         // Platforms
//!         wasm: { target_arch = "wasm32" },
//!         android: { target_os = "android" },
//!         macos: { target_os = "macos" },
//!         linux: { target_os = "linux" },
//!         // Backends
//!         surfman: { all(unix, feature = "surfman", not(wasm)) },
//!         glutin: { all(feature = "glutin", not(wasm)) },
//!         wgl: { all(windows, feature = "wgl", not(wasm)) },
//!         dummy: { not(any(wasm, glutin, wgl, surfman)) },
//!     }
//! }
//! ```
//!
//! Now that we have our aliases setup we can use them just like you would expect:
//!
//! ```rust
//! #[cfg(wasm)]
//! println!("This is running in WASM");
//!
//! #[cfg(surfman)]
//! {
//!     // Do stuff related to surfman
//! }
//!
//! #[cfg(dummy)]
//! println!("We're in dummy mode, specify another feature if you want a smarter app!");
//! ```
//!
//! This greatly improves what would otherwise look like this without the aliases:
//!
//! ```rust
//! #[cfg(target_arch = "wasm32")]
//! println!("We're running in WASM");
//!
//! #[cfg(all(unix, feature = "surfman", not(target_arch = "22")))]
//! {
//!     // Do stuff related to surfman
//! }
//!
//! #[cfg(not(any(
//!     target_arch = "wasm32",
//!     all(unix, feature = "surfman", not(target_arch = "wasm32")),
//!     all(windows, feature = "wgl", not(target_arch = "wasm32")),
//!     all(feature = "glutin", not(target_arch = "wasm32")),
//! )))]
//! println!("We're in dummy mode, specify another feature if you want a smarter app!");
//! ```
//!
//! You can also use the `cfg!` macro or combine your aliases with other checks using `all()`, `not()`, and `any()`. Your aliases are genuine `cfg` flags now!
//!
//! ```rust
//! if cfg!(glutin) {
//!     // use glutin
//! } else {
//!     // Do something else
//! }
//!
//! #[cfg(all(glutin, surfman))]
//! compile_error!("You cannot specify both `glutin` and `surfman` features");
//! ```
//!
//! ## Syntax and Error Messages
//!
//! The aliase names are restricted to the same rules as rust identifiers which, for one, means that they cannot have dashes ( `-` ) in them. Additionally, if you get certain syntax elements wrong, such as the alias name, the macro will error saying that the recursion limit was reached instead of giving a clear indication of what actually went wrong. This is due to a nuance with the macro parser and it might be fixed in a later release of this crate. It is also possible that aliases with dashes in the name might be supported in a later release. Open an issue if that is something that you would like implemented.
//!
//! Finally, you can also induce an infinite recursion by having rules that both reference each-other, but this isn't a real limitation because that doesn't make logical sense anyway:
//!
//! ```rust,ignore
//! // This causes an error!
//! cfg_aliases! {
//!     test1: { not(test2) },
//!     test2: { not(test1) },
//! }
//! ```
//!
//! ## Attribution and Thanks
//!
//! - Thanks to my God and Father who led me through figuring this out and to whome I owe everything.
//! - Thanks to @Yandros on the Rust forum for [showing me][sm] some crazy macro hacks!
//! - Thanks to @sfackler for [pointing out][po] the way to make cargo add the cfg flags.
//! - Thanks to the authors of the [`tectonic_cfg_support::target_cfg`] macro from which most of the cfg attribute parsing logic is taken from. Also thanks to @ratmice for [bringing it up][bip] on the Rust forum.
//!
//! [`tectonic_cfg_support::target_cfg`]: https://docs.rs/tectonic_cfg_support/0.0.1/src/tectonic_cfg_support/lib.rs.html#166-298
//! [po]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/2
//! [bip]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/13
//! [sm]: https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100/3

#![allow(clippy::needless_doctest_main)]

// In the `cfg_aliases!` macro below, all of the rules that start with @parser were derived from
// the `target_cfg!` macro here:
//
// https://docs.rs/tectonic_cfg_support/0.0.1/src/tectonic_cfg_support/lib.rs.html#166-298.
//
// The `target_cfg!` macro is excellently commented while the one below is not very well commented
// yet, so if you need some help understanding it you might benefit by reading that implementation.
// Also check out this forum topic for more history on the macro development:
//
// https://users.rust-lang.org/t/any-such-thing-as-cfg-aliases/40100?u=zicklag

/// Create `cfg` aliases
///
/// **build.rs:**
///
/// ```rust
/// # use cfg_aliases::cfg_aliases;
/// // Setup cfg aliases
/// cfg_aliases! {
///     // Platforms
///     wasm: { target_arch = "wasm32" },
///     android: { target_os = "android" },
///     macos: { target_os = "macos" },
///     linux: { target_os = "linux" },
///     // Backends
///     surfman: { all(unix, feature = "surfman", not(wasm)) },
///     glutin: { all(feature = "glutin", not(wasm)) },
///     wgl: { all(windows, feature = "wgl", not(wasm)) },
///     dummy: { not(any(wasm, glutin, wgl, surfman)) },
/// }
/// ```
///
/// After you put this in your build script you can then check for those conditions like so:
///
/// ```rust
/// #[cfg(surfman)]
/// {
///     // Do stuff related to surfman
/// }
///
/// #[cfg(dummy)]
/// println!("We're in dummy mode, specify another feature if you want a smarter app!");
/// ```
///
/// This greatly improves what would otherwise look like this without the aliases:
///
/// ```rust
/// #[cfg(all(unix, feature = "surfman", not(target_arch = "wasm32")))]
/// {
///     // Do stuff related to surfman
/// }
///
/// #[cfg(not(any(
///     target_arch = "wasm32",
///     all(unix, feature = "surfman", not(target_arch = "wasm32")),
///     all(windows, feature = "wgl", not(target_arch = "wasm32")),
///     all(feature = "glutin", not(target_arch = "wasm32")),
/// )))]
/// println!("We're in dummy mode, specify another feature if you want a smarter app!");
/// ```
#[macro_export]
macro_rules! cfg_aliases {
    // Helper that just checks whether the CFG environment variable is set
    (@cfg_is_set $cfgname:ident) => {
        {
            let cfg_var = stringify!($cfgname).to_uppercase().replace("-", "_");
            let result = std::env::var(format!("CARGO_CFG_{}", &cfg_var)).is_ok();

            // CARGO_CFG_DEBUG_ASSERTIONS _should_ be set for when debug assertions are enabled,
            // but as of writing is not: see https://github.com/rust-lang/cargo/issues/5777
            if !result && cfg_var == "DEBUG_ASSERTIONS" {
                std::env::var("PROFILE") == Ok("debug".to_owned())
            } else {
                result
            }
        }
    };
    // Helper to check for the presense of a feature
    (@cfg_has_feature $feature:expr) => {
        {
            std::env::var(
                format!(
                    "CARGO_FEATURE_{}",
                    &stringify!($feature).to_uppercase().replace("-", "_").replace('"', "")
                )
            ).map(|x| x == "1").unwrap_or(false)
        }
    };

    // Helper that checks whether a CFG environment contains the given value
    (@cfg_contains $cfgname:ident = $cfgvalue:expr) => {
        std::env::var(
            format!(
                "CARGO_CFG_{}",
                &stringify!($cfgname).to_uppercase().replace("-", "_")
            )
        ).unwrap_or("".to_owned()).split(",").find(|x| x == &$cfgvalue).is_some()
    };

    // Emitting `any(clause1,clause2,...)`: convert to `$crate::cfg_aliases!(clause1) && $crate::cfg_aliases!(clause2) && ...`
    (
        @parser_emit
        all
        $({$($grouped:tt)+})+
    ) => {
        ($(
            ($crate::cfg_aliases!(@parser $($grouped)+))
        )&&+)
    };

    // Likewise for `all(clause1,clause2,...)`.
    (
        @parser_emit
        any
        $({$($grouped:tt)+})+
    ) => {
        ($(
            ($crate::cfg_aliases!(@parser $($grouped)+))
        )||+)
    };

    // "@clause" rules are used to parse the comma-separated lists. They munch
    // their inputs token-by-token and finally invoke an "@emit" rule when the
    // list is all grouped. The general pattern for recording the parser state
    // is:
    //
    // ```
    // $crate::cfg_aliases!(
    //    @clause $operation
    //    [{grouped-clause-1} {grouped-clause-2...}]
    //    [not-yet-parsed-tokens...]
    //    current-clause-tokens...
    // )
    // ```

    // This rule must come first in this section. It fires when the next token
    // to parse is a comma. When this happens, we take the tokens in the
    // current clause and add them to the list of grouped clauses, adding
    // delimeters so that the grouping can be easily extracted again in the
    // emission stage.
    (
        @parser_clause
        $op:ident
        [$({$($grouped:tt)+})*]
        [, $($rest:tt)*]
        $($current:tt)+
    ) => {
        $crate::cfg_aliases!(@parser_clause $op [
            $(
                {$($grouped)+}
            )*
            {$($current)+}
        ] [
            $($rest)*
        ]);
    };

    // This rule comes next. It fires when the next un-parsed token is *not* a
    // comma. In this case, we add that token to the list of tokens in the
    // current clause, then move on to the next one.
    (
        @parser_clause
        $op:ident
        [$({$($grouped:tt)+})*]
        [$tok:tt $($rest:tt)*]
        $($current:tt)*
    ) => {
        $crate::cfg_aliases!(@parser_clause $op [
            $(
                {$($grouped)+}
            )*
        ] [
            $($rest)*
        ] $($current)* $tok);
    };

    // This rule fires when there are no more tokens to parse in this list. We
    // finish off the "current" token group, then delegate to the emission
    // rule.
    (
        @parser_clause
        $op:ident
        [$({$($grouped:tt)+})*]
        []
        $($current:tt)+
    ) => {
        $crate::cfg_aliases!(@parser_emit $op
            $(
                {$($grouped)+}
            )*
            {$($current)+}
        );
    };


    // `all(clause1, clause2...)` : we must parse this comma-separated list and
    // partner with `@emit all` to output a bunch of && terms.
    (
        @parser
        all($($tokens:tt)+)
    ) => {
        $crate::cfg_aliases!(@parser_clause all [] [$($tokens)+])
    };

    // Likewise for `any(clause1, clause2...)`
    (
        @parser
        any($($tokens:tt)+)
    ) => {
        $crate::cfg_aliases!(@parser_clause any [] [$($tokens)+])
    };

    // `not(clause)`: compute the inner clause, then just negate it.
    (
        @parser
        not($($tokens:tt)+)
    ) => {
        !($crate::cfg_aliases!(@parser $($tokens)+))
    };

    // `feature = value`: test for a feature.
    (@parser feature = $value:expr) => {
        $crate::cfg_aliases!(@cfg_has_feature $value)
    };
    // `param = value`: test for equality.
    (@parser $key:ident = $value:expr) => {
        $crate::cfg_aliases!(@cfg_contains $key = $value)
    };
    // Parse a lone identifier that might be an alias
    (@parser $e:ident) => {
        __cfg_aliases_matcher__!($e)
    };

    // Entrypoint that defines the matcher
    (
        @with_dollar[$dol:tt]
        $( $alias:ident : { $($config:tt)* } ),* $(,)?
    ) => {
        // Create a macro that expands other aliases and outputs any non
        // alias by checking whether that CFG value is set
        macro_rules! __cfg_aliases_matcher__ {
            // Parse config expression for the alias
            $(
                ( $alias ) => {
                    $crate::cfg_aliases!(@parser $($config)*)
                };
            )*
            // Anything that doesn't match evaluate the item
            ( $dol e:ident ) => {
                $crate::cfg_aliases!(@cfg_is_set $dol e)
            };
        }

        $(
            println!("cargo:rustc-check-cfg=cfg({})", stringify!($alias));
            if $crate::cfg_aliases!(@parser $($config)*) {
                println!("cargo:rustc-cfg={}", stringify!($alias));
            }
        )*
    };

    // Catch all that starts the macro
    ($($tokens:tt)*) => {
        $crate::cfg_aliases!(@with_dollar[$] $($tokens)*)
    }
}
