// The re-exports are used in the generated code for macro hygiene. Even if the paths `::core` or
// `::std` are shadowed, the generated code will still be able to access the crates.
#[cfg(feature = "alloc")]
pub extern crate alloc;
pub extern crate core;
#[cfg(feature = "std")]
pub extern crate std;

use core::cell::Cell;
use core::fmt;
use core::iter::{Enumerate, Peekable};
use core::ops::Deref;
use core::pin::Pin;

pub use crate::error::{ErrorMarker, ResultConverter};
pub use crate::values::get_value;
use crate::{FastWritable, Values};

pub struct TemplateLoop<I>
where
    I: Iterator,
{
    iter: Peekable<Enumerate<I>>,
}

impl<I> TemplateLoop<I>
where
    I: Iterator,
{
    #[inline]
    pub fn new(iter: I) -> Self {
        TemplateLoop {
            iter: iter.enumerate().peekable(),
        }
    }
}

impl<I> Iterator for TemplateLoop<I>
where
    I: Iterator,
{
    type Item = (<I as Iterator>::Item, LoopItem);

    #[inline]
    fn next(&mut self) -> Option<(<I as Iterator>::Item, LoopItem)> {
        self.iter.next().map(|(index, item)| {
            (
                item,
                LoopItem {
                    index,
                    first: index == 0,
                    last: self.iter.peek().is_none(),
                },
            )
        })
    }
}

#[derive(Copy, Clone)]
pub struct LoopItem {
    pub index: usize,
    pub first: bool,
    pub last: bool,
}

pub struct FmtCell<F> {
    func: Cell<Option<F>>,
    err: Cell<Option<crate::Error>>,
}

impl<F> FmtCell<F>
where
    F: for<'a, 'b> FnOnce(&'a mut fmt::Formatter<'b>) -> crate::Result<()>,
{
    #[inline]
    pub fn new(f: F) -> Self {
        Self {
            func: Cell::new(Some(f)),
            err: Cell::new(None),
        }
    }

    #[inline]
    pub fn take_err(&self) -> crate::Result<()> {
        Err(self.err.take().unwrap_or(crate::Error::Fmt))
    }
}

impl<F> fmt::Display for FmtCell<F>
where
    F: for<'a, 'b> FnOnce(&'a mut fmt::Formatter<'b>) -> crate::Result<()>,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(func) = self.func.take() {
            if let Err(err) = func(f) {
                self.err.set(Some(err));
                return Err(fmt::Error);
            }
        }
        Ok(())
    }
}

#[inline]
pub fn get_primitive_value<T: PrimitiveType>(value: T) -> T::Value {
    value.get()
}

/// A type that is, references, or wraps a [primitive][std::primitive] type
pub trait PrimitiveType {
    type Value: Copy + Send + Sync + 'static;

    fn get(&self) -> Self::Value;
}

macro_rules! primitive_type {
    ($($ty:ty),* $(,)?) => {$(
        impl PrimitiveType for $ty {
            type Value = $ty;

            #[inline]
            fn get(&self) -> Self::Value {
                *self
            }
        }
    )*};
}

primitive_type! {
    bool,
    f32, f64,
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
}

crate::impl_for_ref! {
    impl PrimitiveType for T {
        type Value = T::Value;

        #[inline]
        fn get(&self) -> Self::Value {
            <T>::get(self)
        }
    }
}

impl<T> PrimitiveType for Pin<T>
where
    T: Deref,
    <T as Deref>::Target: PrimitiveType,
{
    type Value = <<T as Deref>::Target as PrimitiveType>::Value;

    #[inline]
    fn get(&self) -> Self::Value {
        self.as_ref().get_ref().get()
    }
}

/// Implement [`PrimitiveType`] for [`Cell<T>`]
///
/// ```
/// # use std::cell::Cell;
/// # use std::num::{NonZeroI16, Saturating};
/// # use std::rc::Rc;
/// # use std::pin::Pin;
/// # use askama::Template;
/// #[derive(Template)]
/// #[template(ext = "txt", source = "{{ value as u16 }}")]
/// struct Test<'a> {
///     value: &'a Pin<Rc<Cell<Saturating<NonZeroI16>>>>
/// }
///
/// assert_eq!(
///     Test { value: &Rc::pin(Cell::new(Saturating(NonZeroI16::new(-1).unwrap()))) }.to_string(),
///     "65535",
/// );
/// ```
impl<T: PrimitiveType + Copy> PrimitiveType for Cell<T> {
    type Value = T::Value;

    #[inline]
    fn get(&self) -> Self::Value {
        self.get().get()
    }
}

impl<T: PrimitiveType> PrimitiveType for core::num::Wrapping<T> {
    type Value = T::Value;

    #[inline]
    fn get(&self) -> Self::Value {
        self.0.get()
    }
}

impl<T: PrimitiveType> PrimitiveType for core::num::Saturating<T> {
    type Value = T::Value;

    #[inline]
    fn get(&self) -> Self::Value {
        self.0.get()
    }
}

macro_rules! primitize_nz {
    ($($nz:ty => $bare:ident,)+) => { $(
        impl PrimitiveType for $nz {
            type Value = $bare;

            #[inline]
            fn get(&self) -> Self::Value {
                <$nz>::get(*self).get()
            }
        }
    )+ };
}

primitize_nz! {
    core::num::NonZeroI8 => i8,
    core::num::NonZeroI16 => i16,
    core::num::NonZeroI32 => i32,
    core::num::NonZeroI64 => i64,
    core::num::NonZeroI128 => i128,
    core::num::NonZeroIsize => isize,
    core::num::NonZeroU8 => u8,
    core::num::NonZeroU16 => u16,
    core::num::NonZeroU32 => u32,
    core::num::NonZeroU64 => u64,
    core::num::NonZeroU128 => u128,
    core::num::NonZeroUsize => usize,
}

/// An empty element, so nothing will be written.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Empty;

impl fmt::Display for Empty {
    #[inline]
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

impl FastWritable for Empty {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(&self, _: &mut W, _: &dyn Values) -> crate::Result<()> {
        Ok(())
    }
}

impl fmt::Write for Empty {
    #[inline]
    fn write_str(&mut self, _: &str) -> fmt::Result {
        Ok(())
    }

    #[inline]
    fn write_char(&mut self, _: char) -> fmt::Result {
        Ok(())
    }
}

#[inline]
pub fn as_bool<T: PrimitiveType<Value = bool>>(value: T) -> bool {
    value.get()
}

pub struct Concat<L, R>(pub L, pub R);

impl<L: fmt::Display, R: fmt::Display> fmt::Display for Concat<L, R> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)?;
        self.1.fmt(f)
    }
}

impl<L: FastWritable, R: FastWritable> FastWritable for Concat<L, R> {
    #[inline]
    fn write_into<W: fmt::Write + ?Sized>(
        &self,
        dest: &mut W,
        values: &dyn Values,
    ) -> crate::Result<()> {
        self.0.write_into(dest, values)?;
        self.1.write_into(dest, values)
    }
}

pub trait EnumVariantTemplate {
    fn render_into_with_values<W: fmt::Write + ?Sized>(
        &self,
        writer: &mut W,
        values: &dyn crate::Values,
    ) -> crate::Result<()>;
}
