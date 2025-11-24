//! [![Crates.io](https://img.shields.io/crates/v/askama?logo=rust&style=flat-square&logoColor=white "Crates.io")](https://crates.io/crates/askama)
//! [![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/askama-rs/askama/rust.yml?branch=master&logo=github&style=flat-square&logoColor=white "GitHub Workflow Status")](https://github.com/askama-rs/askama/actions/workflows/rust.yml)
//! [![Book](https://img.shields.io/readthedocs/askama?label=book&logo=readthedocs&style=flat-square&logoColor=white "Book")](https://askama.readthedocs.io/)
//! [![docs.rs](https://img.shields.io/docsrs/askama?logo=docsdotrs&style=flat-square&logoColor=white "docs.rs")](https://docs.rs/askama/)
//!
//! Askama implements a type-safe compiler for Jinja-like templates.
//! It lets you write templates in a Jinja-like syntax,
//! which are linked to a `struct` or an `enum` defining the template context.
//! This is done using a custom derive implementation (implemented
//! in [`askama_derive`](https://crates.io/crates/askama_derive)).
//!
//! For feature highlights and a quick start, please review the
//! [README](https://github.com/askama-rs/askama/blob/master/README.md).
//!
//! You can find the documentation about our syntax, features, configuration in our book:
//! [askama.readthedocs.io](https://askama.readthedocs.io/).
//!
//! # Creating Askama templates
//!
//! The main feature of Askama is the [`Template`] derive macro
//! which reads your template code, so your `struct` or `enum` can implement
//! the [`Template`] trait and [`Display`][std::fmt::Display], type-safe and fast:
//!
//! ```rust
//! # use askama::Template;
//! #[derive(Template)]
//! #[template(
//!     ext = "html",
//!     source = "<p>© {{ year }} {{ enterprise|upper }}</p>"
//! )]
//! struct Footer<'a> {
//!     year: u16,
//!     enterprise: &'a str,
//! }
//!
//! assert_eq!(
//!     Footer { year: 2025, enterprise: "<em>Askama</em> developers" }.to_string(),
//!     "<p>© 2025 &#60;EM&#62;ASKAMA&#60;/EM&#62; DEVELOPERS</p>",
//! );
//! // In here you see can Askama's auto-escaping. You, the developer,
//! // can easily disable the auto-escaping with the `|safe` filter,
//! // but a malicious user cannot insert e.g. HTML scripts this way.
//! ```
//!
//! An Askama template is a `struct` or `enum` definition which provides the template
//! context combined with a UTF-8 encoded text file (or inline source).
//! Askama can be used to generate any kind of text-based format.
//! The template file's extension may be used to provide content type hints.
//!
//! A template consists of **text contents**, which are passed through as-is,
//! **expressions**, which get replaced with content while being rendered, and
//! **tags**, which control the template's logic.
//! The template syntax is very similar to [Jinja](http://jinja.pocoo.org/),
//! as well as Jinja-derivatives like [Twig](https://twig.symfony.com/) or
//! [Tera](https://github.com/Keats/tera).

#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]
#![deny(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod ascii_str;
mod error;
pub mod filters;
#[doc(hidden)]
pub mod helpers;
mod html;
mod values;

#[cfg(feature = "alloc")]
use alloc::string::String;
use core::fmt;
use core::ops::Deref;
#[cfg(feature = "std")]
use std::io;

#[cfg(feature = "derive")]
pub use askama_derive::Template;

#[doc(hidden)]
pub use crate as shared;
pub use crate::error::{Error, Result};
pub use crate::helpers::PrimitiveType;
pub use crate::values::{NO_VALUES, Value, Values, get_value};

/// Main `Template` trait; implementations are generally derived
///
/// If you need an object-safe template, use [`DynTemplate`].
///
/// ## Rendering performance
///
/// When rendering a askama template, you should prefer the methods
///
/// * [`.render()`][Template::render] (to render the content into a new string),
/// * [`.render_into()`][Template::render_into] (to render the content into an [`fmt::Write`]
///   object, e.g. [`String`]) or
/// * [`.write_into()`][Template::write_into] (to render the content into an [`io::Write`] object,
///   e.g. [`Vec<u8>`][alloc::vec::Vec])
///
/// over [`.to_string()`][std::string::ToString::to_string] or [`format!()`][alloc::format].
/// While `.to_string()` and `format!()` give you the same result, they generally perform much worse
/// than askama's own methods, because [`fmt::Write`] uses [dynamic methods calls] instead of
/// monomorphised code. On average, expect `.to_string()` to be 100% to 200% slower than
/// `.render()`.
///
/// [dynamic methods calls]: <https://doc.rust-lang.org/stable/std/keyword.dyn.html>
pub trait Template: fmt::Display + FastWritable {
    /// Helper method which allocates a new `String` and renders into it.
    #[inline]
    #[cfg(feature = "alloc")]
    fn render(&self) -> Result<String> {
        self.render_with_values(NO_VALUES)
    }

    /// Helper method which allocates a new `String` and renders into it with provided [`Values`].
    #[inline]
    #[cfg(feature = "alloc")]
    fn render_with_values(&self, values: &dyn Values) -> Result<String> {
        let mut buf = String::new();
        let _ = buf.try_reserve(Self::SIZE_HINT);
        self.render_into_with_values(&mut buf, values)?;
        Ok(buf)
    }

    /// Renders the template to the given `writer` fmt buffer.
    #[inline]
    fn render_into<W: fmt::Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        self.render_into_with_values(writer, NO_VALUES)
    }

    /// Renders the template to the given `writer` fmt buffer with provided [`Values`].
    fn render_into_with_values<W: fmt::Write + ?Sized>(
        &self,
        writer: &mut W,
        values: &dyn Values,
    ) -> Result<()>;

    /// Renders the template to the given `writer` io buffer.
    #[inline]
    #[cfg(feature = "std")]
    fn write_into<W: io::Write + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.write_into_with_values(writer, NO_VALUES)
    }

    /// Renders the template to the given `writer` io buffer with provided [`Values`].
    #[cfg(feature = "std")]
    fn write_into_with_values<W: io::Write + ?Sized>(
        &self,
        writer: &mut W,
        values: &dyn Values,
    ) -> io::Result<()> {
        struct Wrapped<W: io::Write> {
            writer: W,
            err: Option<io::Error>,
        }

        impl<W: io::Write> fmt::Write for Wrapped<W> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                if let Err(err) = self.writer.write_all(s.as_bytes()) {
                    self.err = Some(err);
                    Err(fmt::Error)
                } else {
                    Ok(())
                }
            }
        }

        let mut wrapped = Wrapped { writer, err: None };
        if self.render_into_with_values(&mut wrapped, values).is_ok() {
            Ok(())
        } else {
            let err = wrapped.err.take();
            Err(err.unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, fmt::Error)))
        }
    }

    /// Provides a rough estimate of the expanded length of the rendered template. Larger
    /// values result in higher memory usage but fewer reallocations. Smaller values result in the
    /// opposite. This value only affects [`render`]. It does not take effect when calling
    /// [`render_into`], [`write_into`], the [`fmt::Display`] implementation, or the blanket
    /// [`ToString::to_string`] implementation.
    ///
    /// [`render`]: Template::render
    /// [`render_into`]: Template::render_into
    /// [`write_into`]: Template::write_into
    /// [`ToString::to_string`]: alloc::string::ToString::to_string
    const SIZE_HINT: usize;
}

impl<T: Template + ?Sized> Template for &T {
    #[inline]
    #[cfg(feature = "alloc")]
    fn render(&self) -> Result<String> {
        <T as Template>::render(self)
    }

    #[inline]
    #[cfg(feature = "alloc")]
    fn render_with_values(&self, values: &dyn Values) -> Result<String> {
        <T as Template>::render_with_values(self, values)
    }

    #[inline]
    fn render_into<W: fmt::Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        <T as Template>::render_into(self, writer)
    }

    #[inline]
    fn render_into_with_values<W: fmt::Write + ?Sized>(
        &self,
        writer: &mut W,
        values: &dyn Values,
    ) -> Result<()> {
        <T as Template>::render_into_with_values(self, writer, values)
    }

    #[inline]
    #[cfg(feature = "std")]
    fn write_into<W: io::Write + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        <T as Template>::write_into(self, writer)
    }

    #[inline]
    #[cfg(feature = "std")]
    fn write_into_with_values<W: io::Write + ?Sized>(
        &self,
        writer: &mut W,
        values: &dyn Values,
    ) -> io::Result<()> {
        <T as Template>::write_into_with_values(self, writer, values)
    }

    const SIZE_HINT: usize = T::SIZE_HINT;
}

/// [`dyn`-compatible] wrapper trait around [`Template`] implementers
///
/// This trades reduced performance (mostly due to writing into `dyn Write`) for dyn-compatibility.
///
/// [`dyn`-compatible]: https://doc.rust-lang.org/stable/reference/items/traits.html#dyn-compatibility
pub trait DynTemplate {
    /// Helper method which allocates a new `String` and renders into it.
    #[cfg(feature = "alloc")]
    fn dyn_render(&self) -> Result<String>;

    /// Helper method which allocates a new `String` and renders into it with provided [`Values`].
    #[cfg(feature = "alloc")]
    fn dyn_render_with_values(&self, values: &dyn Values) -> Result<String>;

    /// Renders the template to the given `writer` fmt buffer.
    fn dyn_render_into(&self, writer: &mut dyn fmt::Write) -> Result<()>;

    /// Renders the template to the given `writer` fmt buffer with provided [`Values`].
    fn dyn_render_into_with_values(
        &self,
        writer: &mut dyn fmt::Write,
        values: &dyn Values,
    ) -> Result<()>;

    /// Renders the template to the given `writer` io buffer.
    #[cfg(feature = "std")]
    fn dyn_write_into(&self, writer: &mut dyn io::Write) -> io::Result<()>;

    /// Renders the template to the given `writer` io buffer with provided [`Values`].
    #[cfg(feature = "std")]
    fn dyn_write_into_with_values(
        &self,
        writer: &mut dyn io::Write,
        values: &dyn Values,
    ) -> io::Result<()>;

    /// Provides a conservative estimate of the expanded length of the rendered template.
    fn size_hint(&self) -> usize;
}

impl<T: Template> DynTemplate for T {
    #[inline]
    #[cfg(feature = "alloc")]
    fn dyn_render(&self) -> Result<String> {
        <Self as Template>::render(self)
    }

    #[cfg(feature = "alloc")]
    fn dyn_render_with_values(&self, values: &dyn Values) -> Result<String> {
        <Self as Template>::render_with_values(self, values)
    }

    #[inline]
    fn dyn_render_into(&self, writer: &mut dyn fmt::Write) -> Result<()> {
        <Self as Template>::render_into(self, writer)
    }

    fn dyn_render_into_with_values(
        &self,
        writer: &mut dyn fmt::Write,
        values: &dyn Values,
    ) -> Result<()> {
        <Self as Template>::render_into_with_values(self, writer, values)
    }

    #[cfg(feature = "std")]
    fn dyn_write_into(&self, writer: &mut dyn io::Write) -> io::Result<()> {
        <Self as Template>::write_into(self, writer)
    }

    #[inline]
    #[cfg(feature = "std")]
    fn dyn_write_into_with_values(
        &self,
        writer: &mut dyn io::Write,
        values: &dyn Values,
    ) -> io::Result<()> {
        <Self as Template>::write_into_with_values(self, writer, values)
    }

    #[inline]
    fn size_hint(&self) -> usize {
        <Self as Template>::SIZE_HINT
    }
}

impl fmt::Display for dyn DynTemplate {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.dyn_render_into(f).map_err(|_| fmt::Error {})
    }
}

/// Implement the trait `$Trait` for a list of reference (wrapper) types to `$T: $Trait + ?Sized`
macro_rules! impl_for_ref {
    (impl $Trait:ident for $T:ident $body:tt) => {
        const _: () = {
            crate::impl_for_ref! {
                impl<$T> $Trait for [
                    &T
                    &mut T
                    core::cell::Ref<'_, T>
                    core::cell::RefMut<'_, T>
                ] $body
            }
        };
        #[cfg(feature = "alloc")]
        const _: () = {
            crate::impl_for_ref! {
                impl<$T> $Trait for [
                    alloc::boxed::Box<T>
                    alloc::rc::Rc<T>
                    alloc::sync::Arc<T>
                ] $body
            }
        };
        #[cfg(feature = "std")]
        const _: () = {
            crate::impl_for_ref! {
                impl<$T> $Trait for [
                    std::sync::MutexGuard<'_, T>
                    std::sync::RwLockReadGuard<'_, T>
                    std::sync::RwLockWriteGuard<'_, T>
                ] $body
            }
        };
    };
    (impl<$T:ident> $Trait:ident for [$($ty:ty)*] $body:tt) => {
        $(impl<$T: $Trait + ?Sized> $Trait for $ty $body)*
    }
}

/// Types implementing this trait can be written without needing to employ an [`fmt::Formatter`].
pub trait FastWritable {
    /// Used internally by askama to speed up writing some types.
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn Values,
    ) -> crate::Result<()>;
}

const _: () = {
    crate::impl_for_ref! {
        impl FastWritable for T {
            #[inline]
            fn write_into<W: fmt::Write + ?Sized>(
                &self,
                dest: &mut W,
                values: &dyn Values,
            ) -> crate::Result<()> {
                <T>::write_into(self, dest, values)
            }
        }
    }

    impl<T> FastWritable for core::pin::Pin<T>
    where
        T: Deref,
        <T as Deref>::Target: FastWritable,
    {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            values: &dyn Values,
        ) -> crate::Result<()> {
            self.as_ref().get_ref().write_into(dest, values)
        }
    }

    #[cfg(feature = "alloc")]
    impl<T: FastWritable + alloc::borrow::ToOwned> FastWritable for alloc::borrow::Cow<'_, T> {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            values: &dyn Values,
        ) -> crate::Result<()> {
            T::write_into(self.as_ref(), dest, values)
        }
    }

    // implement FastWritable for a list of types
    macro_rules! impl_for_int {
        ($($ty:ty)*) => { $(
            impl FastWritable for $ty {
                #[inline]
                fn write_into<W: fmt::Write + ?Sized>(
                    &self,
                    dest: &mut W,
                    values: &dyn Values,
                ) -> crate::Result<()> {
                    itoa::Buffer::new().format(*self).write_into(dest, values)
                }
            }
        )* };
    }

    impl_for_int!(
        u8 u16 u32 u64 u128 usize
        i8 i16 i32 i64 i128 isize
    );

    // implement FastWritable for a list of non-zero integral types
    macro_rules! impl_for_nz_int {
        ($($id:ident)*) => { $(
            impl FastWritable for core::num::$id {
                #[inline]
                fn write_into<W: fmt::Write + ?Sized>(
                    &self,
                    dest: &mut W,
                    values: &dyn Values,
                ) -> crate::Result<()> {
                    self.get().write_into(dest, values)
                }
            }
        )* };
    }

    impl_for_nz_int!(
        NonZeroU8 NonZeroU16 NonZeroU32 NonZeroU64 NonZeroU128 NonZeroUsize
        NonZeroI8 NonZeroI16 NonZeroI32 NonZeroI64 NonZeroI128 NonZeroIsize
    );

    impl FastWritable for str {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            _: &dyn Values,
        ) -> crate::Result<()> {
            Ok(dest.write_str(self)?)
        }
    }

    #[cfg(feature = "alloc")]
    impl FastWritable for alloc::string::String {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            values: &dyn Values,
        ) -> crate::Result<()> {
            self.as_str().write_into(dest, values)
        }
    }

    impl FastWritable for bool {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            _: &dyn Values,
        ) -> crate::Result<()> {
            Ok(dest.write_str(match self {
                true => "true",
                false => "false",
            })?)
        }
    }

    impl FastWritable for char {
        #[inline]
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            _: &dyn Values,
        ) -> crate::Result<()> {
            Ok(dest.write_char(*self)?)
        }
    }

    impl FastWritable for fmt::Arguments<'_> {
        fn write_into<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            _: &dyn Values,
        ) -> crate::Result<()> {
            Ok(match self.as_str() {
                Some(s) => dest.write_str(s),
                None => dest.write_fmt(*self),
            }?)
        }
    }

    impl<S: crate::Template + ?Sized> filters::WriteWritable for &filters::Writable<'_, S> {
        #[inline]
        fn askama_write<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            values: &dyn Values,
        ) -> crate::Result<()> {
            self.0.render_into_with_values(dest, values)
        }
    }

    impl<S: FastWritable + ?Sized> filters::WriteWritable for &&filters::Writable<'_, S> {
        #[inline]
        fn askama_write<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            values: &dyn Values,
        ) -> crate::Result<()> {
            self.0.write_into(dest, values)
        }
    }

    impl<S: fmt::Display + ?Sized> filters::WriteWritable for &&&filters::Writable<'_, S> {
        #[inline]
        fn askama_write<W: fmt::Write + ?Sized>(
            &self,
            dest: &mut W,
            _: &dyn Values,
        ) -> crate::Result<()> {
            Ok(write!(dest, "{}", self.0)?)
        }
    }
};

pub(crate) use impl_for_ref;

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use std::fmt;

    use super::*;
    use crate::{DynTemplate, Template};

    #[test]
    fn dyn_template() {
        use alloc::string::ToString;

        struct Test;

        impl Template for Test {
            fn render_into_with_values<W: fmt::Write + ?Sized>(
                &self,
                writer: &mut W,
                _values: &dyn Values,
            ) -> Result<()> {
                Ok(writer.write_str("test")?)
            }

            const SIZE_HINT: usize = 4;
        }

        impl fmt::Display for Test {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.render_into(f).map_err(|_| fmt::Error {})
            }
        }

        impl FastWritable for Test {
            #[inline]
            fn write_into<W: fmt::Write + ?Sized>(
                &self,
                f: &mut W,
                values: &dyn Values,
            ) -> crate::Result<()> {
                self.render_into_with_values(f, values)
            }
        }

        fn render(t: &dyn DynTemplate) -> String {
            t.dyn_render().unwrap()
        }

        let test = &Test as &dyn DynTemplate;

        assert_eq!(render(test), "test");

        assert_eq!(test.to_string(), "test");

        assert_eq!(alloc::format!("{test}"), "test");

        let mut vec = alloc::vec![];
        test.dyn_write_into(&mut vec).unwrap();
        assert_eq!(vec, alloc::vec![b't', b'e', b's', b't']);
    }
}
