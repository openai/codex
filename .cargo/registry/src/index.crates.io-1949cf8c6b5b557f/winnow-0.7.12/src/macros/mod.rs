mod dispatch;
mod seq;

#[cfg(test)]
macro_rules! assert_parse(
  ($left: expr, $right: expr) => {
     let res: $crate::error::ModalResult<_, $crate::error::InputError<_>> = $left;
     snapbox::assert_data_eq!(snapbox::data::ToDebug::to_debug(&res), $right);
  };
);

macro_rules! impl_partial_eq {
    ($lhs:ty, $rhs:ty) => {
        #[allow(unused_lifetimes)]
        impl<'a> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let l = self;
                let r: &Self = other.as_ref();
                PartialEq::eq(l, r)
            }
        }

        #[allow(unused_lifetimes)]
        impl<'a> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                PartialEq::eq(other, self)
            }
        }
    };
}

macro_rules! impl_partial_ord {
    ($lhs:ty, $rhs:ty) => {
        #[allow(unused_lifetimes)]
        impl<'a> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<Ordering> {
                let l = self;
                let r: &Self = other.as_ref();
                PartialOrd::partial_cmp(l, r)
            }
        }

        #[allow(unused_lifetimes)]
        impl<'a> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<Ordering> {
                PartialOrd::partial_cmp(other, self)
            }
        }
    };
}

#[cfg(test)]
mod tests;
