//! Definition of [`SmartDisplay`] and its related items.
//!
//! [`SmartDisplay`] is a trait that allows authors to provide additional information to both the
//! formatter and other users. This information is provided in the form of a metadata type. The only
//! required piece of metadata is the width of the value. This is _before_ it is passed to the
//! formatter (i.e. it does not include any padding added by the formatter). Other information
//! can be stored in a custom metadata type as needed. This information may be made available to
//! downstream users, but it is not required.
//!
//! This module contains the [`SmartDisplay`] and associated items.
//!
//! # Example
//!
//! ```rust
//! use std::fmt;
//!
//! use powerfmt::ext::FormatterExt as _;
//! use powerfmt::smart_display::{self, FormatterOptions, Metadata, SmartDisplay};
//!
//! #[derive(Debug)]
//! struct User {
//!     id: usize,
//! }
//!
//! // If you try to use `UserMetadata` in the `SmartDisplay` implementation, you will get a
//! // compiler error about a private type being used publicly. To avoid this, use this attribute to
//! // declare a private metadata type. You shouldn't need to worry about how this works, but be
//! // aware that any public fields or methods remain usable by downstream users.
//! #[smart_display::private_metadata]
//! struct UserMetadata {
//!     username: String,
//!     legal_name: String,
//! }
//!
//! // This attribute can be applied to `SmartDisplay` implementations. It will generate an
//! // implementation of `Display` that delegates to `SmartDisplay`, avoiding the need to write
//! // boilerplate.
//! #[smart_display::delegate]
//! impl SmartDisplay for User {
//!     type Metadata = UserMetadata;
//!
//!     fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
//!         // This could be obtained from a database, for example.
//!         let legal_name = "John Doe".to_owned();
//!         let username = "jdoe".to_owned();
//!
//!         // Note that this must be kept in sync with the implementation of `fmt_with_metadata`.
//!         let width = smart_display::padded_width_of!(username, " (", legal_name, ")",);
//!
//!         Metadata::new(
//!             width,
//!             self,
//!             UserMetadata {
//!                 username,
//!                 legal_name,
//!             },
//!         )
//!     }
//!
//!     // Use the now-generated metadata to format the value. Here we use the `pad_with_width`
//!     // method to use the alignment and desired width from the formatter.
//!     fn fmt_with_metadata(
//!         &self,
//!         f: &mut fmt::Formatter<'_>,
//!         metadata: Metadata<Self>,
//!     ) -> fmt::Result {
//!         f.pad_with_width(
//!             metadata.unpadded_width(),
//!             format_args!("{} ({})", metadata.username, metadata.legal_name),
//!         )
//!     }
//! }
//!
//! let user = User { id: 42 };
//! assert_eq!(user.to_string(), "jdoe (John Doe)");
//! assert_eq!(format!("{user:>20}"), "     jdoe (John Doe)");
//! ```

use core::cmp;
use core::convert::Infallible;
use core::fmt::{Alignment, Debug, Display, Formatter, Result};
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::Deref;

/// Compute the width of multiple items while optionally declaring the options for each item.
///
/// ```rust
/// # use powerfmt::smart_display;
/// let alpha = 0;
/// let beta = 1;
/// let gamma = 100;
///
/// let width = smart_display::padded_width_of!(
///     alpha, // use the default options
///     beta => width(2), // use the specified options
///     gamma => width(2) sign_plus(true), // use multiple options
/// );
/// assert_eq!(width, 7);
///
/// let formatted = format!("{alpha}{beta:2}{gamma:+2}");
/// assert_eq!(formatted.len(), width);
/// ```
///
/// Supported options are:
///
/// Option                      | Method called
/// ---                         | ---
/// `fill(char)`                | [`FormatterOptions::with_fill`]
/// `sign_plus(bool)`           | [`FormatterOptions::with_sign_plus`]
/// `sign_minus(bool)`          | [`FormatterOptions::with_sign_minus`]
/// `align(Alignment)`          | [`FormatterOptions::with_align`]
/// `width(usize)`              | [`FormatterOptions::with_width`]
/// `precision(usize)`          | [`FormatterOptions::with_precision`]
/// `alternate(bool)`           | [`FormatterOptions::with_alternate`]
/// `sign_aware_zero_pad(bool)` | [`FormatterOptions::with_sign_aware_zero_pad`]
///
/// If there are future additions to [`FormatterOptions`], they will be added to this macro as well.
///
/// Options may be provided in any order and will be called in the order they are provided. The
/// ordering matters if providing both `sign_plus` and `sign_minus`.
#[cfg(doc)]
#[doc(hidden)] // Don't show at crate root.
#[macro_export]
macro_rules! padded_width_of {
    ($($t:tt)*) => {};
}

#[cfg(not(doc))]
#[allow(missing_docs)] // This is done with `#[cfg(doc)]` to avoid showing the various rules.
#[macro_export]
macro_rules! __not_public_at_root__padded_width_of {
    // Base case
    (@inner [] [$($output:tt)+]) => { $($output)+ };
    (@inner [$e:expr $(, $($remaining:tt)*)?] [$($expansion:tt)+]) => {
        $crate::smart_display::padded_width_of!(@inner [$($($remaining)*)?] [
            $($expansion)+ + $crate::smart_display::Metadata::padded_width_of(
                &$e,
                $crate::smart_display::padded_width_of!(@options)
            )
        ])
    };
    (@inner
        [$e:expr => $($call:ident($call_expr:expr))+ $(, $($remaining:tt)*)?]
        [$($expansion:tt)+]
    ) => {
        $crate::smart_display::padded_width_of!(@inner [$($($remaining)*)?] [
            $($expansion)+ + $crate::smart_display::Metadata::padded_width_of(
                &$e,
                *$crate::smart_display::padded_width_of!(@options $($call($call_expr))+)
            )
        ])
    };

    // Options base case
    (@options_inner [] [$($output:tt)+]) => { $($output)+ };
    (@options_inner [fill($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_fill($e)
        ])
    };
    (@options_inner [sign_plus($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_sign_plus($e)
        ])
    };
    (@options_inner [sign_minus($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_sign_minus($e)
        ])
    };
    (@options_inner [align($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_align(Some($e))
        ])
    };
    (@options_inner [width($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_width(Some($e))
        ])
    };
    (@options_inner [precision($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_precision(Some($e))
        ])
    };
    (@options_inner [alternate($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_width($e)
        ])
    };
    (@options_inner [sign_aware_zero_pad($e:expr) $($remaining:tt)*] [$($expansion:tt)*]) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($remaining)*] [
            $($expansion)*.with_sign_aware_zero_pad($e)
        ])
    };
    // Options entry point
    (@options $($e:tt)*) => {
        $crate::smart_display::padded_width_of!(@options_inner [$($e)*] [
            $crate::smart_display::FormatterOptions::default()
        ])
    };

    // Entry point
    ($($t:tt)*) => {
        $crate::smart_display::padded_width_of!(
            @inner [$($t)*] [0]
        )
    };
}

#[cfg(not(doc))]
pub use __not_public_at_root__padded_width_of as padded_width_of;
#[cfg(doc)]
#[doc(inline)] // Show in this module.
pub use padded_width_of;
/// Implement [`Display`] for a type by using its implementation of [`SmartDisplay`].
///
/// This attribute is applied to the `SmartDisplay` implementation.
///
/// ```rust,no_run
/// # use powerfmt::smart_display::{self, SmartDisplay, Metadata, FormatterOptions};
/// # struct Foo;
/// #[smart_display::delegate]
/// impl SmartDisplay for Foo {
/// #   type Metadata = ();
/// #   fn metadata(&self, f: FormatterOptions) -> Metadata<Self> {
/// #       todo!()
/// #   }
///     // ...
/// }
/// ```
#[cfg(feature = "macros")]
pub use powerfmt_macros::smart_display_delegate as delegate;
/// Declare a private metadata type for `SmartDisplay`.
///
/// Use this attribute if you want to provide metadata for a type that is not public. Doing
/// this will avoid a compiler error about a private type being used publicly. Keep in mind
/// that any public fields, public methods, and trait implementations _will_ be able to be used
/// by downstream users.
///
/// To avoid accidentally exposing details, such as when all fields are public or if the type
/// is a unit struct, the type is annotated with `#[non_exhaustive]` automatically.
///
/// ```rust,no_run
/// # use powerfmt::smart_display;
/// /// Metadata for `Foo`
/// #[smart_display::private_metadata]
/// #[derive(Debug)]
/// pub(crate) struct FooMetadata {
///     pub(crate) expensive_to_calculate: usize,
/// }
/// ```
#[cfg(feature = "macros")]
pub use powerfmt_macros::smart_display_private_metadata as private_metadata;

#[derive(Debug)]
enum FlagBit {
    SignPlus,
    SignMinus,
    Alternate,
    SignAwareZeroPad,
    WidthIsInitialized,
    PrecisionIsInitialized,
}

/// Configuration for formatting.
///
/// This struct is obtained from a [`Formatter`]. It provides the same functionality as that of a
/// reference to a `Formatter`. However, it is not possible to construct a `Formatter`, which is
/// necessary for some use cases of [`SmartDisplay`]. `FormatterOptions` implements [`Default`] and
/// has builder methods to alleviate this.
#[derive(Clone, Copy)]
pub struct FormatterOptions {
    flags: u8,
    fill: char,
    align: Option<Alignment>,
    width: MaybeUninit<usize>,
    precision: MaybeUninit<usize>,
}

impl Debug for FormatterOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("FormatterOptions")
            .field("fill", &self.fill)
            .field("align", &self.align())
            .field("width", &self.width())
            .field("precision", &self.precision())
            .field("sign_plus", &self.sign_plus())
            .field("sign_minus", &self.sign_minus())
            .field("alternate", &self.alternate())
            .field("sign_aware_zero_pad", &self.sign_aware_zero_pad())
            .finish()
    }
}

impl Default for FormatterOptions {
    #[inline]
    fn default() -> Self {
        Self {
            flags: 0,
            fill: ' ',
            align: None,
            width: MaybeUninit::uninit(),
            precision: MaybeUninit::uninit(),
        }
    }
}

impl FormatterOptions {
    /// Sets the fill character to use whenever there is alignment.
    #[inline]
    pub fn with_fill(&mut self, c: char) -> &mut Self {
        self.fill = c;
        self
    }

    /// Set whether the `+` flag is specified.
    #[inline]
    pub fn with_sign_plus(&mut self, b: bool) -> &mut Self {
        if b {
            self.flags |= 1 << FlagBit::SignPlus as u8;
            self.flags &= !(1 << FlagBit::SignMinus as u8);
        } else {
            self.flags &= !(1 << FlagBit::SignPlus as u8);
        }
        self
    }

    /// Set whether the `-` flag is specified.
    #[inline]
    pub fn with_sign_minus(&mut self, b: bool) -> &mut Self {
        if b {
            self.flags |= 1 << FlagBit::SignMinus as u8;
            self.flags &= !(1 << FlagBit::SignPlus as u8);
        } else {
            self.flags &= !(1 << FlagBit::SignMinus as u8);
        }
        self
    }

    /// Set the flag indicating what form of alignment is requested, if any.
    #[inline]
    pub fn with_align(&mut self, align: Option<Alignment>) -> &mut Self {
        self.align = align;
        self
    }

    /// Set the optional integer width that the output should be.
    #[inline]
    pub fn with_width(&mut self, width: Option<usize>) -> &mut Self {
        if let Some(width) = width {
            self.flags |= 1 << FlagBit::WidthIsInitialized as u8;
            self.width = MaybeUninit::new(width);
        } else {
            self.flags &= !(1 << FlagBit::WidthIsInitialized as u8);
        }
        self
    }

    /// Set the optional precision for numeric types. Alternatively, the maximum width for string
    /// types.
    #[inline]
    pub fn with_precision(&mut self, precision: Option<usize>) -> &mut Self {
        if let Some(precision) = precision {
            self.flags |= 1 << FlagBit::PrecisionIsInitialized as u8;
            self.precision = MaybeUninit::new(precision);
        } else {
            self.flags &= !(1 << FlagBit::PrecisionIsInitialized as u8);
        }
        self
    }

    /// Set whether the `#` flag is specified.
    #[inline]
    pub fn with_alternate(&mut self, b: bool) -> &mut Self {
        if b {
            self.flags |= 1 << FlagBit::Alternate as u8;
        } else {
            self.flags &= !(1 << FlagBit::Alternate as u8);
        }
        self
    }

    /// Set whether the `0` flag is specified.
    #[inline]
    pub fn with_sign_aware_zero_pad(&mut self, b: bool) -> &mut Self {
        if b {
            self.flags |= 1 << FlagBit::SignAwareZeroPad as u8;
        } else {
            self.flags &= !(1 << FlagBit::SignAwareZeroPad as u8);
        }
        self
    }
}

impl FormatterOptions {
    /// Character used as 'fill' whenever there is alignment.
    #[inline]
    #[must_use]
    pub const fn fill(&self) -> char {
        self.fill
    }

    /// Flag indicating what form of alignment was requested.
    #[inline]
    #[must_use]
    pub const fn align(&self) -> Option<Alignment> {
        self.align
    }

    /// Optionally specified integer width that the output should be.
    #[inline]
    #[must_use]
    pub const fn width(&self) -> Option<usize> {
        if (self.flags >> FlagBit::WidthIsInitialized as u8) & 1 == 1 {
            // Safety: `width` is initialized if the flag is set.
            Some(unsafe { self.width.assume_init() })
        } else {
            None
        }
    }

    /// Optionally specified precision for numeric types. Alternatively, the maximum width for
    /// string types.
    #[inline]
    #[must_use]
    pub const fn precision(&self) -> Option<usize> {
        if (self.flags >> FlagBit::PrecisionIsInitialized as u8) & 1 == 1 {
            // Safety: `precision` is initialized if the flag is set.
            Some(unsafe { self.precision.assume_init() })
        } else {
            None
        }
    }

    /// Determines if the `+` flag was specified.
    #[inline]
    #[must_use]
    pub const fn sign_plus(&self) -> bool {
        (self.flags >> FlagBit::SignPlus as u8) & 1 == 1
    }

    /// Determines if the `-` flag was specified.
    #[inline]
    #[must_use]
    pub const fn sign_minus(&self) -> bool {
        (self.flags >> FlagBit::SignMinus as u8) & 1 == 1
    }

    /// Determines if the `#` flag was specified.
    #[inline]
    #[must_use]
    pub const fn alternate(&self) -> bool {
        (self.flags >> FlagBit::Alternate as u8) & 1 == 1
    }

    /// Determines if the `0` flag was specified.
    #[inline]
    #[must_use]
    pub const fn sign_aware_zero_pad(&self) -> bool {
        (self.flags >> FlagBit::SignAwareZeroPad as u8) & 1 == 1
    }
}

impl From<&Formatter<'_>> for FormatterOptions {
    fn from(value: &Formatter<'_>) -> Self {
        *Self::default()
            .with_fill(value.fill())
            .with_sign_plus(value.sign_plus())
            .with_sign_minus(value.sign_minus())
            .with_align(value.align())
            .with_width(value.width())
            .with_precision(value.precision())
            .with_alternate(value.alternate())
            .with_sign_aware_zero_pad(value.sign_aware_zero_pad())
    }
}

impl From<&mut Formatter<'_>> for FormatterOptions {
    #[inline]
    fn from(value: &mut Formatter<'_>) -> Self {
        (&*value).into()
    }
}

/// Information used to format a value. This is returned by [`SmartDisplay::metadata`].
///
/// This type is generic over any user-provided type. This allows the author to store any
/// information that is needed. For example, a type's implementation of [`SmartDisplay`] may need
/// to calculate something before knowing its width. This calculation can be performed, with the
/// result being stored in the custom metadata type.
///
/// Note that `Metadata` _always_ contains the width of the type. Authors do not need to store this
/// information in their custom metadata type.
///
/// Generally speaking, a type should be able to be formatted using only its metadata, fields, and
/// the formatter. Any other information should be stored in the metadata type.
pub struct Metadata<'a, T>
where
    T: SmartDisplay + ?Sized,
{
    unpadded_width: usize,
    metadata: T::Metadata,
    _value: PhantomData<&'a T>, // variance
}

// manual impls for bounds
impl<T> Debug for Metadata<'_, T>
where
    T: SmartDisplay,
    T::Metadata: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Metadata")
            .field("unpadded_width", &self.unpadded_width)
            .field("metadata", &self.metadata)
            .finish()
    }
}

impl<T> Clone for Metadata<'_, T>
where
    T: SmartDisplay,
    T::Metadata: Clone,
{
    fn clone(&self) -> Self {
        Self {
            unpadded_width: self.unpadded_width,
            metadata: self.metadata.clone(),
            _value: self._value,
        }
    }
}

impl<T> Copy for Metadata<'_, T>
where
    T: SmartDisplay,
    T::Metadata: Copy,
{
}

impl<'a, T> Metadata<'a, T>
where
    T: SmartDisplay + ?Sized,
{
    /// Creates a new `Metadata` with the given width and metadata. While the width _should_ be
    /// exact, this is not a requirement for soundness.
    pub const fn new(unpadded_width: usize, _value: &T, metadata: T::Metadata) -> Self {
        Self {
            unpadded_width,
            metadata,
            _value: PhantomData,
        }
    }

    /// Reuse the metadata for another type. This is useful when implementing [`SmartDisplay`] for a
    /// type that wraps another type. Both type's metadata type must be the same.
    pub fn reuse<'b, U>(self) -> Metadata<'b, U>
    where
        'a: 'b,
        U: SmartDisplay<Metadata = T::Metadata> + ?Sized,
    {
        Metadata {
            unpadded_width: self.unpadded_width,
            metadata: self.metadata,
            _value: PhantomData,
        }
    }

    /// Obtain the width of the value before padding.
    pub const fn unpadded_width(&self) -> usize {
        self.unpadded_width
    }

    /// Obtain the width of the value after padding.
    pub fn padded_width(&self, f: FormatterOptions) -> usize {
        match f.width() {
            Some(requested_width) => cmp::max(self.unpadded_width(), requested_width),
            None => self.unpadded_width(),
        }
    }
}

impl Metadata<'_, Infallible> {
    /// Obtain the width of the value before padding, given the formatter options.
    pub fn unpadded_width_of<T>(value: T, f: FormatterOptions) -> usize
    where
        T: SmartDisplay,
    {
        value.metadata(f).unpadded_width
    }

    /// Obtain the width of the value after padding, given the formatter options.
    pub fn padded_width_of<T>(value: T, f: FormatterOptions) -> usize
    where
        T: SmartDisplay,
    {
        value.metadata(f).padded_width(f)
    }
}

/// Permit using `Metadata` as a smart pointer to the user-provided metadata.
impl<T> Deref for Metadata<'_, T>
where
    T: SmartDisplay + ?Sized,
{
    type Target = T::Metadata;

    fn deref(&self) -> &T::Metadata {
        &self.metadata
    }
}

/// Format trait that allows authors to provide additional information.
///
/// This trait is similar to [`Display`], but allows the author to provide additional information
/// to the formatter. This information is provided in the form of a custom metadata type.
///
/// The only required piece of metadata is the width of the value. This is _before_ it is passed to
/// the formatter (i.e. it does not include any padding added by the formatter). Other information
/// can be stored in a custom metadata type as needed. This information may be made available to
/// downstream users, but it is not required.
///
/// **Note**: While both `fmt_with_metadata` and `fmt` have default implementations, it is strongly
/// recommended to implement only `fmt_with_metadata`. `fmt` should be implemented if and only if
/// the type does not require any of the calculated metadata. In that situation, `fmt_with_metadata`
/// should be omitted.
#[cfg_attr(__powerfmt_docs, rustc_must_implement_one_of(fmt, fmt_with_metadata))]
pub trait SmartDisplay: Display {
    /// User-provided metadata type.
    type Metadata;

    /// Compute any information needed to format the value. This must, at a minimum, determine the
    /// width of the value before any padding is added by the formatter.
    ///
    /// If the type uses other types that implement `SmartDisplay` verbatim, the inner types should
    /// have their metadata calculated and included in the returned value.
    ///
    /// # Lifetimes
    ///
    /// This method's return type contains a lifetime to `self`. This ensures that the metadata will
    /// neither outlive the value nor be invalidated by a mutation of the value (barring interior
    /// mutability).
    ///
    /// ```rust,compile_fail
    /// # use std::fmt;
    /// # use std::fmt::Write;
    /// # use powerfmt::buf::WriteBuffer;
    /// # use powerfmt::smart_display::{self, FormatterOptions, Metadata, SmartDisplay};
    /// #[derive(Debug)]
    /// struct WrappedBuffer(WriteBuffer<128>);
    ///
    /// #[smart_display::delegate]
    /// impl SmartDisplay for WrappedBuffer {
    ///     type Metadata = ();
    ///
    ///     fn metadata(&self, _: FormatterOptions) -> Metadata<'_, Self> {
    ///         Metadata::new(self.0.len(), self, ())
    ///     }
    ///
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         f.pad(self.0.as_str())
    ///     }
    /// }
    ///
    /// let mut buf = WrappedBuffer(WriteBuffer::new());
    /// let metadata = buf.metadata(FormatterOptions::default());
    /// // We cannot mutate the buffer while it is borrowed and use its previous metadata on the
    /// // following line.
    /// write!(buf.0, "Hello, world!")?;
    /// assert_eq!(metadata.width(), 13);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn metadata(&self, f: FormatterOptions) -> Metadata<'_, Self>;

    /// Format the value using the given formatter and metadata. The formatted output should have
    /// the width indicated by the metadata. This is before any padding is added by the
    /// formatter.
    ///
    /// If the metadata is not needed, you should implement the `fmt` method instead.
    fn fmt_with_metadata(&self, f: &mut Formatter<'_>, _metadata: Metadata<'_, Self>) -> Result {
        SmartDisplay::fmt(self, f)
    }

    /// Format the value using the given formatter. This is the same as [`Display::fmt`].
    ///
    /// The default implementation of this method calls `fmt_with_metadata` with the result of
    /// `metadata`. Generally speaking, this method should not be implemented. You should implement
    /// the `fmt_with_metadata` method instead.
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let metadata = self.metadata(f.into());
        self.fmt_with_metadata(f, metadata)
    }
}
