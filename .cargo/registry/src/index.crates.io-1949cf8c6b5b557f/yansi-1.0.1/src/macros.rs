macro_rules! set_enum {
    ($T:ident { $($v:ident),+ $(,)? }) => {
        impl $T {
            pub(crate) const fn bit_mask(self) -> u16 {
                1 << self as u16
            }

            pub(crate) const fn from_bit_mask(value: u16) -> Option<Self> {
                $(if (value == $T::$v.bit_mask()) { return Some($T::$v); })+
                None
            }
        }

        impl crate::set::Set<$T> {
            #[must_use]
            pub const fn insert(mut self, value: $T) -> Self {
                self.1 |= value.bit_mask();
                self
            }
        }

        impl crate::set::SetMember for $T {
            const MAX_VALUE: u8 = { $($T::$v as u8);+ };
            fn bit_mask(self) -> u16 { <$T>::bit_mask(self) }
            fn from_bit_mask(v: u16) -> Option<Self> { <$T>::from_bit_mask(v) }
        }
    };
}

macro_rules! constructor {
    (
        [$($q:tt)*] $r:ty, $R:ty, $p:ident,
        $(#[$pattr:meta])* $prop:ident => $V:path $([$($a:ident : $T:ty),+])?
    ) => {
        /// Returns `self` with the
        #[doc = concat!("[`", stringify!($p), "()`](Self::", stringify!($p), "())")]
        /// set to
        #[doc = concat!("[`", stringify!($V), "`].")]
        ///
        /// # Example
        ///
        /// ```rust
        #[doc = concat!(
            "# let value = yansi::Painted::new(0);",
            $($("\n# let ", stringify!($a), " = 0;"),+)?
        )]
        #[doc = concat!(
            "println!(\"{}\", value.", stringify!($prop), "(", $(stringify!($($a),+),)? "));"
        )]
        /// ```
        #[inline]
        $(#[$pattr])*
        $($q)* fn $prop(self: $r $($(,$a: $T)+)?) -> $R {
            let v = $V $(($($a),*))?;
            self.apply(crate::style::Application::$p(v))
        }
    };

    ([$($q:tt)*] $(#[$attr:meta])* $r:ty, $R:ty, $kind:ident ($A:ty)) => {
        $(#[$attr])*
        #[inline]
        $($q)* fn $kind(self: $r, value: $A) -> $R {
            self.apply(crate::style::Application::$kind(value))
        }
    };
}

macro_rules! signature {
    (
        [$($q:tt)*] $r:ty, $R:ty, $p:ident,
        $(#[$pattr:meta])* $prop:ident => $V:path $([$($a:ident : $T:ty),+])?
    ) => {
        /// Returns `self` with the
        #[doc = concat!("[`", stringify!($p), "()`](Self::", stringify!($p), "())")]
        /// set to
        #[doc = concat!("[`", stringify!($V), "`].")]
        ///
        /// # Example
        ///
        /// ```rust
        #[doc = concat!(
            "# let value = yansi::Painted::new(0);",
            $($("\n# let ", stringify!($a), " = 0;"),+)?
        )]
        #[doc = concat!(
            "println!(\"{}\", value.", stringify!($prop), "(", $(stringify!($($a),+),)? "));"
        )]
        /// ```
        $(#[$pattr])*
        $($q)* fn $prop(self: $r $($(,$a: $T)+)?) -> $R;
    };

    ([$($q:tt)*] $(#[$attr:meta])* $r:ty, $R:ty, $kind:ident ($A:ty)) => {
        $(#[$attr])*
        $($q)* fn $kind(self: $r, value: $A) -> $R;
    };
}

macro_rules! define_property {
    ([$d:tt] $(#[$attr:meta])* $kind:ident ($A:ty) {
        $($(#[$pattr:meta])* $prop:ident => $V:path $([$($a:tt)*])?),* $(,)?
    }) => {
        macro_rules! $kind {
            ($d ([$d ($qual:tt)*])? $cont:ident ($r:ty) -> $R:ty) => (
                $cont!([$d ($d ($qual)*)?] $(#[$attr])* $r, $R, $kind($A));

                $(
                    $cont!(
                        [$d ($d ($qual)*)?]
                        $r, $R, $kind, $(#[$pattr])* $prop => $V $([$($a)*] )?
                    );
                )*
            )
        }
    };

    ($(#[$attr:meta])* $kind:ident ($A:ty)) => {
        define_property!([$] $(#[$attr])* $kind ($A) {});
    };

    ($($t:tt)*) => { define_property!([$] $($t)*); }
}

// Check that every variant of a property is covered.
macro_rules! check_property_exhaustiveness {
    ($A:ident $({ $($(#[$pattr:meta])* $p:ident => $V:path $([ $($a:tt)* ])?),* $(,)? })? ) => {
        const _: () = {$(
            use crate::*;
            fn _check() {
                #[allow(unreachable_code)]
                match { let _v: $A = todo!(); _v } {
                    $($V { .. } => { },)*
                }
            }
        )?};
    }
}

macro_rules! define_properties {
    ($($(#[$attr:meta])* $kind:ident ($A:ident) $({ $($t:tt)* })?),* $(,)?) => {
        $(define_property!($(#[$attr])* $kind($A) $({ $($t)* })?);)*
        $(check_property_exhaustiveness!($A $({ $($t)* })?);)*
    }
}

macro_rules! properties {
    ($([$($qual:tt)*])? $cont:ident ($r:ty) -> $R:ty) => (
        fg!($([$($qual)*])? $cont ($r) -> $R);
        bg!($([$($qual)*])? $cont ($r) -> $R);
        attr!($([$($qual)*])? $cont ($r) -> $R);
        quirk!($([$($qual)*])? $cont ($r) -> $R);
        whenever!($([$($qual)*])? $cont ($r) -> $R);
    )
}

define_properties! {
    /// Returns a styled value derived from `self` with the foreground set to
    /// `value`.
    ///
    /// This method should be used rarely. Instead, prefer to use color-specific
    /// builder methods like [`red()`](Self::red()) and
    /// [`green()`](Self::green()), which have the same functionality but are
    /// pithier.
    ///
    /// # Example
    ///
    /// Set foreground color to white using `fg()`:
    ///
    /// ```rust
    /// use yansi::{Paint, Color};
    ///
    /// # let painted = ();
    /// painted.fg(Color::White);
    /// ```
    ///
    /// Set foreground color to white using [`white()`](Self::white()).
    ///
    /// ```rust
    /// use yansi::Paint;
    ///
    /// # let painted = ();
    /// painted.white();
    /// ```
    fg(Color) {
        primary => Color::Primary,
        fixed => Color::Fixed[color: u8],
        rgb => Color::Rgb[r: u8, g: u8, b: u8],
        black => Color::Black,
        red => Color::Red,
        green => Color::Green,
        yellow => Color::Yellow,
        blue => Color::Blue,
        magenta => Color::Magenta,
        cyan => Color::Cyan,
        white => Color::White,
        bright_black => Color::BrightBlack,
        bright_red => Color::BrightRed,
        bright_green => Color::BrightGreen,
        bright_yellow => Color::BrightYellow,
        bright_blue => Color::BrightBlue,
        bright_magenta => Color::BrightMagenta,
        bright_cyan => Color::BrightCyan,
        bright_white => Color::BrightWhite,
    },

    /// Returns a styled value derived from `self` with the background set to
    /// `value`.
    ///
    /// This method should be used rarely. Instead, prefer to use color-specific
    /// builder methods like [`on_red()`](Self::on_red()) and
    /// [`on_green()`](Self::on_green()), which have the same functionality but
    /// are pithier.
    ///
    /// # Example
    ///
    /// Set background color to red using `fg()`:
    ///
    /// ```rust
    /// use yansi::{Paint, Color};
    ///
    /// # let painted = ();
    /// painted.bg(Color::Red);
    /// ```
    ///
    /// Set background color to red using [`on_red()`](Self::on_red()).
    ///
    /// ```rust
    /// use yansi::Paint;
    ///
    /// # let painted = ();
    /// painted.on_red();
    /// ```
    bg(Color) {
        on_primary => Color::Primary,
        on_fixed => Color::Fixed[color: u8],
        on_rgb => Color::Rgb[r: u8, g: u8, b: u8],
        on_black => Color::Black,
        on_red => Color::Red,
        on_green => Color::Green,
        on_yellow => Color::Yellow,
        on_blue => Color::Blue,
        on_magenta => Color::Magenta,
        on_cyan => Color::Cyan,
        on_white => Color::White,
        on_bright_black => Color::BrightBlack,
        on_bright_red => Color::BrightRed,
        on_bright_green => Color::BrightGreen,
        on_bright_yellow => Color::BrightYellow,
        on_bright_blue => Color::BrightBlue,
        on_bright_magenta => Color::BrightMagenta,
        on_bright_cyan => Color::BrightCyan,
        on_bright_white => Color::BrightWhite,
    },

    /// Enables the styling [`Attribute`] `value`.
    ///
    /// This method should be used rarely. Instead, prefer to use
    /// attribute-specific builder methods like [`bold()`](Self::bold()) and
    /// [`underline()`](Self::underline()), which have the same functionality
    /// but are pithier.
    ///
    /// # Example
    ///
    /// Make text bold using `attr()`:
    ///
    /// ```rust
    /// use yansi::{Paint, Attribute};
    ///
    /// # let painted = ();
    /// painted.attr(Attribute::Bold);
    /// ```
    ///
    /// Make text bold using using [`bold()`](Self::bold()).
    ///
    /// ```rust
    /// use yansi::Paint;
    ///
    /// # let painted = ();
    /// painted.bold();
    /// ```
    attr(Attribute) {
        bold => Attribute::Bold,
        dim => Attribute::Dim,
        italic => Attribute::Italic,
        underline => Attribute::Underline,
        blink => Attribute::Blink,
        rapid_blink => Attribute::RapidBlink,
        invert => Attribute::Invert,
        conceal => Attribute::Conceal,
        strike => Attribute::Strike,
    },

    /// Enables the `yansi` [`Quirk`] `value`.
    ///
    /// This method should be used rarely. Instead, prefer to use quirk-specific
    /// builder methods like [`mask()`](Self::mask()) and
    /// [`wrap()`](Self::wrap()), which have the same functionality but are
    /// pithier.
    ///
    /// # Example
    ///
    /// Enable wrapping using `.quirk()`:
    ///
    /// ```rust
    /// use yansi::{Paint, Quirk};
    ///
    /// # let painted = ();
    /// painted.quirk(Quirk::Wrap);
    /// ```
    ///
    /// Enable wrapping using [`wrap()`](Self::wrap()).
    ///
    /// ```rust
    /// use yansi::Paint;
    ///
    /// # let painted = ();
    /// painted.wrap();
    /// ```
    quirk(Quirk) {
        mask => Quirk::Mask,
        wrap => Quirk::Wrap,
        linger => Quirk::Linger,
        #[deprecated(
            since = "1.0.1",
            note = "renamed to `resetting()` due to conflicts with `Vec::clear()`.\n\
                The `clear()` method will be removed in a future release."
        )]
        clear => Quirk::Clear,
        resetting => Quirk::Resetting,
        bright => Quirk::Bright,
        on_bright => Quirk::OnBright,
    },

    /// Conditionally enable styling based on whether the [`Condition`] `value`
    /// applies. Replaces any previous condition.
    ///
    /// See the [crate level docs](crate#per-style) for more details.
    ///
    /// # Example
    ///
    /// Enable styling `painted` only when both `stdout` and `stderr` are TTYs:
    ///
    /// ```rust
    /// # #[cfg(feature = "detect-tty")] {
    /// use yansi::{Paint, Condition};
    ///
    /// # let painted = ();
    /// painted.red().on_yellow().whenever(Condition::STDOUTERR_ARE_TTY);
    /// # }
    /// ```
    whenever(Condition),
}

macro_rules! impl_fmt_trait {
    ($F:path, $f:literal <$G:ident> $T:ty => $s:ident.$v:ident ($V:ty)) => {
        impl<$G: $F> $F for $T {
            fn fmt(&$s, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                $s.fmt_args(&<$V>::fmt, f, format_args!($f, $s.$v))
            }
        }
    };
}

macro_rules! impl_fmt_traits {
    ($($t:tt)*) => {
        impl_fmt_trait!(core::fmt::Display, "{}" $($t)*);
        impl_fmt_trait!(core::fmt::Debug, "{:?}" $($t)*);
        impl_fmt_trait!(core::fmt::Octal, "{:o}" $($t)*);
        impl_fmt_trait!(core::fmt::LowerHex, "{:x}" $($t)*);
        impl_fmt_trait!(core::fmt::UpperHex, "{:X}" $($t)*);
        impl_fmt_trait!(core::fmt::Pointer, "{:p}" $($t)*);
        impl_fmt_trait!(core::fmt::Binary, "{:b}" $($t)*);
        impl_fmt_trait!(core::fmt::LowerExp, "{:e}" $($t)*);
        impl_fmt_trait!(core::fmt::UpperExp, "{:E}" $($t)*);
    };
}
