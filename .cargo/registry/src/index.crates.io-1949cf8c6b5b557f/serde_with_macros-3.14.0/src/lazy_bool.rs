use core::{
    mem,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not},
};

/// Not-yet evaluated boolean value.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LazyBool<T> {
    /// Like `false`.
    ///
    /// This is this type’s default.
    #[default]
    False,

    /// Like `true`.
    True,

    /// Not-yet decided.
    Lazy(T),
}

impl<T> From<bool> for LazyBool<T> {
    fn from(value: bool) -> Self {
        match value {
            false => Self::False,
            true => Self::True,
        }
    }
}

/// Helper to implement various binary operations on [`LazyBool`].
macro_rules! impl_op {
    (
        <
            $trait:ident::$method:ident,
            $assign_trait:ident::$assign_method:ident
        >($matching:pat_param) {
            $($pattern:pat => $body:expr),+ $(,)?
        }
        $(where $($bound:tt)+)?
    ) => {
        impl<L, R, T> $trait<LazyBool<R>> for LazyBool<L>
        where
            L: $trait<R, Output = T>,
            LazyBool<L>: Into<LazyBool<T>>,
            LazyBool<R>: Into<LazyBool<T>>,
            $($($bound)+)?
        {
            type Output = LazyBool<T>;

            fn $method(self, rhs: LazyBool<R>) -> Self::Output {
                match (self, rhs) {
                    (LazyBool::Lazy(lhs), LazyBool::Lazy(rhs)) => LazyBool::Lazy(lhs.$method(rhs)),
                    ($matching, rhs) => rhs.into(),
                    (lhs, $matching) => lhs.into(),
                    $($pattern => $body),+
                }
            }
        }

        impl<'a, L, R, T> $trait<&'a LazyBool<R>> for LazyBool<L>
        where
            L: $trait<&'a R, Output = T>,
            LazyBool<L>: Into<LazyBool<T>>,
            LazyBool<R>: Into<LazyBool<T>> + Clone,
            $($($bound)+)?
        {
            type Output = LazyBool<T>;

            fn $method(self, rhs: &'a LazyBool<R>) -> Self::Output {
                match (self, rhs) {
                    (LazyBool::Lazy(lhs), LazyBool::Lazy(rhs)) => LazyBool::Lazy(lhs.$method(rhs)),
                    ($matching, rhs) => rhs.clone().into(),
                    (lhs, $matching) => lhs.into(),
                    $($pattern => $body),+
                }
            }
        }

        impl<'a, L, R, T> $trait<LazyBool<R>> for &'a LazyBool<L>
        where
            LazyBool<R>: $trait<&'a LazyBool<L>, Output = LazyBool<T>>,
        {
            type Output = LazyBool<T>;

            fn $method(self, rhs: LazyBool<R>) -> Self::Output {
                rhs.$method(self)
            }
        }

        impl<L, R> $assign_trait<LazyBool<R>> for LazyBool<L>
        where
            LazyBool<L>: $trait<LazyBool<R>, Output = LazyBool<L>>,
        {
            fn $assign_method(&mut self, rhs: LazyBool<R>) {
                let lhs = mem::take(self);
                *self = lhs.$method(rhs);
            }
        }
    };
}

impl_op! { <BitAnd::bitand, BitAndAssign::bitand_assign>(LazyBool::True){ _ => LazyBool::False } }
impl_op! { <BitOr::bitor, BitOrAssign::bitor_assign>(LazyBool::False) { _ => LazyBool::True } }
impl_op! {
    <BitXor::bitxor, BitXorAssign::bitxor_assign>(LazyBool::False) {
        (LazyBool::True, rhs) => (!rhs).into(),
        (lhs, LazyBool::True) => (!lhs).into(),
    }
    where
        LazyBool<L>: Not<Output = LazyBool<L>>,
        LazyBool<R>: Not<Output = LazyBool<R>>,
}

impl<T> Not for LazyBool<T>
where
    T: Not<Output = T>,
{
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::False => Self::True,
            Self::True => Self::False,
            Self::Lazy(this) => Self::Lazy(!this),
        }
    }
}

impl<T> Not for &LazyBool<T>
where
    LazyBool<T>: Not<Output = LazyBool<T>> + Clone,
{
    type Output = LazyBool<T>;

    fn not(self) -> Self::Output {
        !self.clone()
    }
}
