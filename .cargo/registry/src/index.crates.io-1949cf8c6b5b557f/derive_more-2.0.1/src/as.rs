//! Type glue for [autoref-based specialization][0], used in [`AsRef`]/[`AsMut`] macro expansion.
//!
//! Allows tp specialize the `impl<T> AsRef<T> for T` case over the default
//! `impl<Inner: AsRef<B>, B> AsRef<B> for Outer<Inner>` one.
//!
//! [0]: https://lukaskalbertodt.github.io/2019/12/05/generalized-autoref-based-specialization.html

use core::marker::PhantomData;

/// Container to specialize over.
pub struct Conv<Frm: ?Sized, To: ?Sized>(PhantomData<(*const Frm, *const To)>);

impl<Frm: ?Sized, To: ?Sized> Default for Conv<Frm, To> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Trait performing the specialization.
pub trait ExtractRef {
    /// Input reference type.
    type Frm;
    /// Output reference type.
    type To;

    /// Extracts the output type from the input one.
    fn __extract_ref(&self, frm: Self::Frm) -> Self::To;
}

impl<'a, T> ExtractRef for &Conv<&'a T, T>
where
    T: ?Sized,
{
    type Frm = &'a T;
    type To = &'a T;

    fn __extract_ref(&self, frm: Self::Frm) -> Self::To {
        frm
    }
}

impl<'a, Frm, To> ExtractRef for Conv<&'a Frm, To>
where
    Frm: AsRef<To> + ?Sized,
    To: ?Sized + 'a,
{
    type Frm = &'a Frm;
    type To = &'a To;

    fn __extract_ref(&self, frm: Self::Frm) -> Self::To {
        frm.as_ref()
    }
}

impl<'a, T> ExtractRef for &Conv<&'a mut T, T>
where
    T: ?Sized,
{
    type Frm = &'a mut T;
    type To = &'a mut T;

    fn __extract_ref(&self, frm: Self::Frm) -> Self::To {
        frm
    }
}

impl<'a, Frm, To> ExtractRef for Conv<&'a mut Frm, To>
where
    Frm: AsMut<To> + ?Sized,
    To: ?Sized + 'a,
{
    type Frm = &'a mut Frm;
    type To = &'a mut To;

    fn __extract_ref(&self, frm: Self::Frm) -> Self::To {
        frm.as_mut()
    }
}
