//! Password masker.

use core::fmt::{self, Write as _};
use core::ops::Range;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::borrow::ToOwned;
#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

use crate::components::AuthorityComponents;
#[cfg(feature = "alloc")]
use crate::format::ToDedicatedString;
use crate::spec::Spec;
use crate::types::{RiAbsoluteStr, RiReferenceStr, RiRelativeStr, RiStr};
#[cfg(feature = "alloc")]
use crate::types::{RiAbsoluteString, RiReferenceString, RiRelativeString, RiString};

/// Returns the range of the password to hide.
pub(crate) fn password_range_to_hide<S: Spec>(iri: &RiReferenceStr<S>) -> Option<Range<usize>> {
    /// Spec-agnostic internal implementation of `password_range_to_hide`.
    fn inner(iri: &str, userinfo: &str) -> Option<Range<usize>> {
        // Length (including `//`) before the `authority` compontent.
        // 2: `"//".len()`.
        let authority_start = 2 + iri
            .find("//")
            .expect("[validity] `authority` component must be prefixed with `//`");
        let end = authority_start + userinfo.len();
        let start = authority_start + userinfo.find(':').map_or_else(|| userinfo.len(), |v| v + 1);
        Some(start..end)
    }

    let authority_components = AuthorityComponents::from_iri(iri)?;
    let userinfo = authority_components.userinfo()?;
    inner(iri.as_str(), userinfo)
}

/// Writes the URI with the password part replaced.
fn write_with_masked_password<D>(
    f: &mut fmt::Formatter<'_>,
    s: &str,
    pw_range: Range<usize>,
    alt: &D,
) -> fmt::Result
where
    D: ?Sized + fmt::Display,
{
    debug_assert!(
        s.len() >= pw_range.end,
        "[consistency] password range must be inside the IRI"
    );

    f.write_str(&s[..pw_range.start])?;
    alt.fmt(f)?;
    f.write_str(&s[pw_range.end..])?;
    Ok(())
}

/// Writes an IRI with the password part trimmed.
fn write_trim_password(f: &mut fmt::Formatter<'_>, s: &str, pw_range: Range<usize>) -> fmt::Result {
    write_with_masked_password(f, s, pw_range, "")
}

/// A wrapper of an IRI string that masks the non-empty password when `Display`ed.
///
/// This is a retrun type of `mask_password` method of IRI string types (such as
/// [`RiStr::mask_password`]).
///
/// # Examples
///
/// ```
/// # use iri_string::validate::Error;
/// # #[cfg(feature = "alloc")] {
/// use iri_string::types::UriReferenceStr;
///
/// let iri = UriReferenceStr::new("http://user:password@example.com/path?query")?;
/// let masked = iri.mask_password();
/// assert_eq!(masked.to_string(), "http://user:@example.com/path?query");
///
/// assert_eq!(
///     masked.replace_password("${password}").to_string(),
///     "http://user:${password}@example.com/path?query"
/// );
/// # }
/// # Ok::<_, Error>(())
/// ```
///
/// [`RiStr::mask_password`]: `crate::types::RiStr::mask_password`
#[derive(Clone, Copy)]
pub struct PasswordMasked<'a, T: ?Sized> {
    /// IRI reference.
    iri_ref: &'a T,
}

impl<'a, T: ?Sized> PasswordMasked<'a, T> {
    /// Creates a new `PasswordMasked` object.
    #[inline]
    #[must_use]
    pub(crate) fn new(iri_ref: &'a T) -> Self {
        Self { iri_ref }
    }
}

/// Implements traits for `PasswordMasked`.
macro_rules! impl_mask {
    ($borrowed:ident, $owned:ident) => {
        impl<'a, S: Spec> PasswordMasked<'a, $borrowed<S>> {
            /// Replaces the password with the given arbitrary content.
            ///
            /// Note that the result might be invalid as an IRI since arbitrary string
            /// can go to the place of the password.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// # #[cfg(feature = "alloc")] {
            /// use iri_string::format::ToDedicatedString;
            /// use iri_string::types::IriReferenceStr;
            ///
            /// let iri = IriReferenceStr::new("http://user:password@example.com/path?query")?;
            /// let masked = iri.mask_password();
            ///
            /// assert_eq!(
            ///     masked.replace_password("${password}").to_string(),
            ///     "http://user:${password}@example.com/path?query"
            /// );
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            #[must_use]
            pub fn replace_password<D>(&self, alt: D) -> PasswordReplaced<'a, $borrowed<S>, D>
            where
                D: fmt::Display,
            {
                PasswordReplaced::with_replacer(self.iri_ref, move |_| alt)
            }

            /// Replaces the password with the given arbitrary content.
            ///
            /// Note that the result might be invalid as an IRI since arbitrary string
            /// can go to the place of the password.
            ///
            /// # Examples
            ///
            /// ```
            /// # use iri_string::validate::Error;
            /// # #[cfg(feature = "alloc")] {
            /// use iri_string::format::ToDedicatedString;
            /// use iri_string::types::IriReferenceStr;
            ///
            /// let iri = IriReferenceStr::new("http://user:password@example.com/path?query")?;
            /// let masked = iri.mask_password();
            ///
            /// let replaced = masked
            ///     .replace_password_with(|password| format!("{{{} chars}}", password.len()));
            /// assert_eq!(
            ///     replaced.to_string(),
            ///     "http://user:{8 chars}@example.com/path?query"
            /// );
            /// # }
            /// # Ok::<_, Error>(())
            /// ```
            #[inline]
            #[must_use]
            pub fn replace_password_with<F, D>(
                &self,
                replace: F,
            ) -> PasswordReplaced<'a, $borrowed<S>, D>
            where
                F: FnOnce(&str) -> D,
                D: fmt::Display,
            {
                PasswordReplaced::with_replacer(self.iri_ref, replace)
            }
        }

        impl<S: Spec> fmt::Display for PasswordMasked<'_, $borrowed<S>> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match password_range_to_hide(self.iri_ref.as_ref()) {
                    Some(pw_range) => write_trim_password(f, self.iri_ref.as_str(), pw_range),
                    None => self.iri_ref.fmt(f),
                }
            }
        }

        impl<S: Spec> fmt::Debug for PasswordMasked<'_, $borrowed<S>> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_char('<')?;
                fmt::Display::fmt(self, f)?;
                f.write_char('>')
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: Spec> ToDedicatedString for PasswordMasked<'_, $borrowed<S>> {
            type Target = $owned<S>;

            fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError> {
                let pw_range = match password_range_to_hide(self.iri_ref.as_ref()) {
                    Some(pw_range) => pw_range,
                    None => return Ok(self.iri_ref.to_owned()),
                };
                let mut s = String::new();
                let iri_ref = self.iri_ref.as_str();
                s.try_reserve(iri_ref.len() - (pw_range.end - pw_range.start))?;
                s.push_str(&iri_ref[..pw_range.start]);
                s.push_str(&iri_ref[pw_range.end..]);
                // SAFETY: IRI remains valid and type does not change if
                // the password is trimmed.
                let iri = unsafe { <$owned<S>>::new_maybe_unchecked(s) };
                Ok(iri)
            }
        }
    };
}

impl_mask!(RiReferenceStr, RiReferenceString);
impl_mask!(RiStr, RiString);
impl_mask!(RiAbsoluteStr, RiAbsoluteString);
impl_mask!(RiRelativeStr, RiRelativeString);

/// A wrapper of an IRI string that replaces the non-empty password when `Display`ed.
///
/// This is a retrun type of `mask_password` method of IRI string types (such as
/// [`RiStr::mask_password`]).
///
/// Note that the result might be invalid as an IRI since arbitrary string can
/// go to the place of the password.
#[cfg_attr(
    feature = "alloc",
    doc = "Because of this, [`ToDedicatedString`] trait is not implemented for this type."
)]
///
/// [`PasswordMasked::replace_password`]: `PasswordMasked::replace_password`
pub struct PasswordReplaced<'a, T: ?Sized, D> {
    /// IRI reference.
    iri_ref: &'a T,
    /// Password range and alternative content.
    password: Option<(Range<usize>, D)>,
}

impl<'a, T, D> PasswordReplaced<'a, T, D>
where
    T: ?Sized,
    D: fmt::Display,
{
    /// Creates a new `PasswordMasked` object.
    ///
    /// # Precondition
    ///
    /// The given string must be a valid IRI reference.
    #[inline]
    #[must_use]
    pub(crate) fn with_replacer<S, F>(iri_ref: &'a T, replace: F) -> Self
    where
        S: Spec,
        T: AsRef<RiReferenceStr<S>>,
        F: FnOnce(&str) -> D,
    {
        let iri_ref_asref = iri_ref.as_ref();
        let password = password_range_to_hide(iri_ref_asref)
            .map(move |pw_range| (pw_range.clone(), replace(&iri_ref_asref.as_str()[pw_range])));
        Self { iri_ref, password }
    }
}

/// Implements traits for `PasswordReplaced`.
macro_rules! impl_replace {
    ($borrowed:ident, $owned:ident) => {
        impl<S: Spec, D: fmt::Display> fmt::Display for PasswordReplaced<'_, $borrowed<S>, D> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match &self.password {
                    Some((pw_range, alt)) => {
                        write_with_masked_password(f, self.iri_ref.as_str(), pw_range.clone(), alt)
                    }
                    None => self.iri_ref.fmt(f),
                }
            }
        }

        impl<S: Spec, D: fmt::Display> fmt::Debug for PasswordReplaced<'_, $borrowed<S>, D> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_char('<')?;
                fmt::Display::fmt(self, f)?;
                f.write_char('>')
            }
        }
    };
}

impl_replace!(RiReferenceStr, RiReferenceString);
impl_replace!(RiStr, RiString);
impl_replace!(RiAbsoluteStr, RiAbsoluteString);
impl_replace!(RiRelativeStr, RiRelativeString);
