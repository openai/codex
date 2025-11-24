use core::fmt::{self, Debug};
use core::marker::PhantomData;

pub trait FnOnce1<A> {
    type Output;
    fn call_once(self, arg: A) -> Self::Output;
}

impl<T, A, R> FnOnce1<A> for T
where
    T: FnOnce(A) -> R,
{
    type Output = R;
    fn call_once(self, arg: A) -> R {
        self(arg)
    }
}

pub trait FnMut1<A>: FnOnce1<A> {
    fn call_mut(&mut self, arg: A) -> Self::Output;
}

impl<T, A, R> FnMut1<A> for T
where
    T: FnMut(A) -> R,
{
    fn call_mut(&mut self, arg: A) -> R {
        self(arg)
    }
}

// Not used, but present for completeness
#[allow(unreachable_pub)]
pub trait Fn1<A>: FnMut1<A> {
    fn call(&self, arg: A) -> Self::Output;
}

impl<T, A, R> Fn1<A> for T
where
    T: Fn(A) -> R,
{
    fn call(&self, arg: A) -> R {
        self(arg)
    }
}

macro_rules! trivial_fn_impls {
    ($name:ident <$($arg:ident),*> $t:ty = $debug:literal) => {
        impl<$($arg),*> Copy for $t {}
        impl<$($arg),*> Clone for $t {
            fn clone(&self) -> Self { *self }
        }
        impl<$($arg),*> Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str($debug)
            }
        }
        impl<$($arg,)* A> FnMut1<A> for $t where Self: FnOnce1<A> {
            fn call_mut(&mut self, arg: A) -> Self::Output {
                self.call_once(arg)
            }
        }
        impl<$($arg,)* A> Fn1<A> for $t where Self: FnOnce1<A> {
            fn call(&self, arg: A) -> Self::Output {
                self.call_once(arg)
            }
        }
        pub(crate) fn $name<$($arg),*>() -> $t {
            Default::default()
        }
    }
}

pub struct OkFn<E>(PhantomData<fn(E)>);

impl<E> Default for OkFn<E> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A, E> FnOnce1<A> for OkFn<E> {
    type Output = Result<A, E>;
    fn call_once(self, arg: A) -> Self::Output {
        Ok(arg)
    }
}

trivial_fn_impls!(ok_fn <T> OkFn<T> = "Ok");

#[derive(Debug, Copy, Clone, Default)]
pub struct ChainFn<F, G>(F, G);

impl<F, G, A> FnOnce1<A> for ChainFn<F, G>
where
    F: FnOnce1<A>,
    G: FnOnce1<F::Output>,
{
    type Output = G::Output;
    fn call_once(self, arg: A) -> Self::Output {
        self.1.call_once(self.0.call_once(arg))
    }
}
impl<F, G, A> FnMut1<A> for ChainFn<F, G>
where
    F: FnMut1<A>,
    G: FnMut1<F::Output>,
{
    fn call_mut(&mut self, arg: A) -> Self::Output {
        self.1.call_mut(self.0.call_mut(arg))
    }
}
impl<F, G, A> Fn1<A> for ChainFn<F, G>
where
    F: Fn1<A>,
    G: Fn1<F::Output>,
{
    fn call(&self, arg: A) -> Self::Output {
        self.1.call(self.0.call(arg))
    }
}
pub(crate) fn chain_fn<F, G>(f: F, g: G) -> ChainFn<F, G> {
    ChainFn(f, g)
}

#[derive(Default)]
pub struct MergeResultFn;

impl<T> FnOnce1<Result<T, T>> for MergeResultFn {
    type Output = T;
    fn call_once(self, arg: Result<T, T>) -> Self::Output {
        match arg {
            Ok(x) => x,
            Err(x) => x,
        }
    }
}
trivial_fn_impls!(merge_result_fn <> MergeResultFn = "merge_result");

#[derive(Debug, Copy, Clone, Default)]
pub struct InspectFn<F>(F);

#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> FnOnce1<A> for InspectFn<F>
where
    F: for<'a> FnOnce1<&'a A, Output = ()>,
{
    type Output = A;
    fn call_once(self, arg: A) -> Self::Output {
        self.0.call_once(&arg);
        arg
    }
}
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> FnMut1<A> for InspectFn<F>
where
    F: for<'a> FnMut1<&'a A, Output = ()>,
{
    fn call_mut(&mut self, arg: A) -> Self::Output {
        self.0.call_mut(&arg);
        arg
    }
}
#[allow(single_use_lifetimes)] // https://github.com/rust-lang/rust/issues/55058
impl<F, A> Fn1<A> for InspectFn<F>
where
    F: for<'a> Fn1<&'a A, Output = ()>,
{
    fn call(&self, arg: A) -> Self::Output {
        self.0.call(&arg);
        arg
    }
}
pub(crate) fn inspect_fn<F>(f: F) -> InspectFn<F> {
    InspectFn(f)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MapOkFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for MapOkFn<F>
where
    F: FnOnce1<T>,
{
    type Output = Result<F::Output, E>;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for MapOkFn<F>
where
    F: FnMut1<T>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for MapOkFn<F>
where
    F: Fn1<T>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.map(|x| self.0.call(x))
    }
}
pub(crate) fn map_ok_fn<F>(f: F) -> MapOkFn<F> {
    MapOkFn(f)
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MapErrFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for MapErrFn<F>
where
    F: FnOnce1<E>,
{
    type Output = Result<T, F::Output>;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for MapErrFn<F>
where
    F: FnMut1<E>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for MapErrFn<F>
where
    F: Fn1<E>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.map_err(|x| self.0.call(x))
    }
}
pub(crate) fn map_err_fn<F>(f: F) -> MapErrFn<F> {
    MapErrFn(f)
}

#[derive(Debug, Copy, Clone)]
pub struct InspectOkFn<F>(F);

impl<'a, F, T, E> FnOnce1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: FnOnce1<&'a T, Output = ()>,
{
    type Output = ();
    fn call_once(self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg {
            self.0.call_once(x)
        }
    }
}
impl<'a, F, T, E> FnMut1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: FnMut1<&'a T, Output = ()>,
{
    fn call_mut(&mut self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg {
            self.0.call_mut(x)
        }
    }
}
impl<'a, F, T, E> Fn1<&'a Result<T, E>> for InspectOkFn<F>
where
    F: Fn1<&'a T, Output = ()>,
{
    fn call(&self, arg: &'a Result<T, E>) -> Self::Output {
        if let Ok(x) = arg {
            self.0.call(x)
        }
    }
}
pub(crate) fn inspect_ok_fn<F>(f: F) -> InspectOkFn<F> {
    InspectOkFn(f)
}

#[derive(Debug, Copy, Clone)]
pub struct InspectErrFn<F>(F);

impl<'a, F, T, E> FnOnce1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: FnOnce1<&'a E, Output = ()>,
{
    type Output = ();
    fn call_once(self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg {
            self.0.call_once(x)
        }
    }
}
impl<'a, F, T, E> FnMut1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: FnMut1<&'a E, Output = ()>,
{
    fn call_mut(&mut self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg {
            self.0.call_mut(x)
        }
    }
}
impl<'a, F, T, E> Fn1<&'a Result<T, E>> for InspectErrFn<F>
where
    F: Fn1<&'a E, Output = ()>,
{
    fn call(&self, arg: &'a Result<T, E>) -> Self::Output {
        if let Err(x) = arg {
            self.0.call(x)
        }
    }
}
pub(crate) fn inspect_err_fn<F>(f: F) -> InspectErrFn<F> {
    InspectErrFn(f)
}

pub(crate) type MapOkOrElseFn<F, G> = ChainFn<MapOkFn<F>, ChainFn<MapErrFn<G>, MergeResultFn>>;
pub(crate) fn map_ok_or_else_fn<F, G>(f: F, g: G) -> MapOkOrElseFn<F, G> {
    chain_fn(map_ok_fn(f), chain_fn(map_err_fn(g), merge_result_fn()))
}

#[derive(Debug, Copy, Clone, Default)]
pub struct UnwrapOrElseFn<F>(F);

impl<F, T, E> FnOnce1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: FnOnce1<E, Output = T>,
{
    type Output = T;
    fn call_once(self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call_once(x))
    }
}
impl<F, T, E> FnMut1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: FnMut1<E, Output = T>,
{
    fn call_mut(&mut self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call_mut(x))
    }
}
impl<F, T, E> Fn1<Result<T, E>> for UnwrapOrElseFn<F>
where
    F: Fn1<E, Output = T>,
{
    fn call(&self, arg: Result<T, E>) -> Self::Output {
        arg.unwrap_or_else(|x| self.0.call(x))
    }
}
pub(crate) fn unwrap_or_else_fn<F>(f: F) -> UnwrapOrElseFn<F> {
    UnwrapOrElseFn(f)
}

pub struct IntoFn<T>(PhantomData<fn() -> T>);

impl<T> Default for IntoFn<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}
impl<A, T> FnOnce1<A> for IntoFn<T>
where
    A: Into<T>,
{
    type Output = T;
    fn call_once(self, arg: A) -> Self::Output {
        arg.into()
    }
}

trivial_fn_impls!(into_fn <T> IntoFn<T> = "Into::into");
