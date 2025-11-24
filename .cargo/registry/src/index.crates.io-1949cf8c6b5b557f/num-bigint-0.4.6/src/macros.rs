#![allow(unused_macros)]

macro_rules! cfg_32 {
    ($($any:tt)+) => {
        #[cfg(not(target_pointer_width = "64"))] $($any)+
    }
}

macro_rules! cfg_32_or_test {
    ($($any:tt)+) => {
        #[cfg(any(not(target_pointer_width = "64"), test))] $($any)+
    }
}

macro_rules! cfg_64 {
    ($($any:tt)+) => {
        #[cfg(target_pointer_width = "64")] $($any)+
    }
}

macro_rules! cfg_digit {
    ($item32:item $item64:item) => {
        cfg_32!($item32);
        cfg_64!($item64);
    };
}

macro_rules! cfg_digit_expr {
    ($expr32:expr, $expr64:expr) => {
        cfg_32!($expr32);
        cfg_64!($expr64);
    };
}

macro_rules! forward_val_val_binop {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<$res> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                // forward to val-ref
                $imp::$method(self, &other)
            }
        }
    };
}

macro_rules! forward_val_val_binop_commutative {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<$res> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                // forward to val-ref, with the larger capacity as val
                if self.capacity() >= other.capacity() {
                    $imp::$method(self, &other)
                } else {
                    $imp::$method(other, &self)
                }
            }
        }
    };
}

macro_rules! forward_ref_val_binop {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<$res> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                // forward to ref-ref
                $imp::$method(self, &other)
            }
        }
    };
}

macro_rules! forward_ref_val_binop_commutative {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<$res> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                // reverse, forward to val-ref
                $imp::$method(other, self)
            }
        }
    };
}

macro_rules! forward_val_ref_binop {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<&$res> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                // forward to ref-ref
                $imp::$method(&self, other)
            }
        }
    };
}

macro_rules! forward_ref_ref_binop {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<&$res> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                // forward to val-ref
                $imp::$method(self.clone(), other)
            }
        }
    };
}

macro_rules! forward_ref_ref_binop_commutative {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<&$res> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                // forward to val-ref, choosing the larger to clone
                if self.len() >= other.len() {
                    $imp::$method(self.clone(), other)
                } else {
                    $imp::$method(other.clone(), self)
                }
            }
        }
    };
}

macro_rules! forward_val_assign {
    (impl $imp:ident for $res:ty, $method:ident) => {
        impl $imp<$res> for $res {
            #[inline]
            fn $method(&mut self, other: $res) {
                self.$method(&other);
            }
        }
    };
}

macro_rules! forward_val_assign_scalar {
    (impl $imp:ident for $res:ty, $scalar:ty, $method:ident) => {
        impl $imp<$res> for $scalar {
            #[inline]
            fn $method(&mut self, other: $res) {
                self.$method(&other);
            }
        }
    };
}

/// use this if val_val_binop is already implemented and the reversed order is required
macro_rules! forward_scalar_val_val_binop_commutative {
    (impl $imp:ident < $scalar:ty > for $res:ty, $method:ident) => {
        impl $imp<$res> for $scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                $imp::$method(other, self)
            }
        }
    };
}

// Forward scalar to ref-val, when reusing storage is not helpful
macro_rules! forward_scalar_val_val_binop_to_ref_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        impl $imp<$scalar> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $scalar) -> $res {
                $imp::$method(&self, other)
            }
        }

        impl $imp<$res> for $scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                $imp::$method(self, &other)
            }
        }
    };
}

macro_rules! forward_scalar_ref_ref_binop_to_ref_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        impl $imp<&$scalar> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$scalar) -> $res {
                $imp::$method(self, *other)
            }
        }

        impl $imp<&$res> for &$scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                $imp::$method(*self, other)
            }
        }
    };
}

macro_rules! forward_scalar_val_ref_binop_to_ref_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        impl $imp<&$scalar> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$scalar) -> $res {
                $imp::$method(&self, *other)
            }
        }

        impl $imp<$res> for &$scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                $imp::$method(*self, &other)
            }
        }
    };
}

macro_rules! forward_scalar_val_ref_binop_to_val_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        impl $imp<&$scalar> for $res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$scalar) -> $res {
                $imp::$method(self, *other)
            }
        }

        impl $imp<$res> for &$scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: $res) -> $res {
                $imp::$method(*self, other)
            }
        }
    };
}

macro_rules! forward_scalar_ref_val_binop_to_val_val {
    (impl $imp:ident < $scalar:ty > for $res:ty, $method:ident) => {
        impl $imp<$scalar> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: $scalar) -> $res {
                $imp::$method(self.clone(), other)
            }
        }

        impl $imp<&$res> for $scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                $imp::$method(self, other.clone())
            }
        }
    };
}

macro_rules! forward_scalar_ref_ref_binop_to_val_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        impl $imp<&$scalar> for &$res {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$scalar) -> $res {
                $imp::$method(self.clone(), *other)
            }
        }

        impl $imp<&$res> for &$scalar {
            type Output = $res;

            #[inline]
            fn $method(self, other: &$res) -> $res {
                $imp::$method(*self, other.clone())
            }
        }
    };
}

macro_rules! promote_scalars {
    (impl $imp:ident<$promo:ty> for $res:ty, $method:ident, $( $scalar:ty ),*) => {
        $(
            forward_all_scalar_binop_to_val_val!(impl $imp<$scalar> for $res, $method);

            impl $imp<$scalar> for $res {
                type Output = $res;

                #[allow(clippy::cast_lossless)]
                #[inline]
                fn $method(self, other: $scalar) -> $res {
                    $imp::$method(self, other as $promo)
                }
            }

            impl $imp<$res> for $scalar {
                type Output = $res;

                #[allow(clippy::cast_lossless)]
                #[inline]
                fn $method(self, other: $res) -> $res {
                    $imp::$method(self as $promo, other)
                }
            }
        )*
    }
}
macro_rules! promote_scalars_assign {
    (impl $imp:ident<$promo:ty> for $res:ty, $method:ident, $( $scalar:ty ),*) => {
        $(
            impl $imp<$scalar> for $res {
                #[allow(clippy::cast_lossless)]
                #[inline]
                fn $method(&mut self, other: $scalar) {
                    self.$method(other as $promo);
                }
            }
        )*
    }
}

macro_rules! promote_unsigned_scalars {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_scalars!(impl $imp<u32> for $res, $method, u8, u16);
        promote_scalars!(impl $imp<UsizePromotion> for $res, $method, usize);
    }
}

macro_rules! promote_unsigned_scalars_assign {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_scalars_assign!(impl $imp<u32> for $res, $method, u8, u16);
        promote_scalars_assign!(impl $imp<UsizePromotion> for $res, $method, usize);
    }
}

macro_rules! promote_signed_scalars {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_scalars!(impl $imp<i32> for $res, $method, i8, i16);
        promote_scalars!(impl $imp<IsizePromotion> for $res, $method, isize);
    }
}

macro_rules! promote_signed_scalars_assign {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_scalars_assign!(impl $imp<i32> for $res, $method, i8, i16);
        promote_scalars_assign!(impl $imp<IsizePromotion> for $res, $method, isize);
    }
}

// Forward everything to ref-ref, when reusing storage is not helpful
macro_rules! forward_all_binop_to_ref_ref {
    (impl $imp:ident for $res:ty, $method:ident) => {
        forward_val_val_binop!(impl $imp for $res, $method);
        forward_val_ref_binop!(impl $imp for $res, $method);
        forward_ref_val_binop!(impl $imp for $res, $method);
    };
}

// Forward everything to val-ref, so LHS storage can be reused
macro_rules! forward_all_binop_to_val_ref {
    (impl $imp:ident for $res:ty, $method:ident) => {
        forward_val_val_binop!(impl $imp for $res, $method);
        forward_ref_val_binop!(impl $imp for $res, $method);
        forward_ref_ref_binop!(impl $imp for $res, $method);
    };
}

// Forward everything to val-ref, commutatively, so either LHS or RHS storage can be reused
macro_rules! forward_all_binop_to_val_ref_commutative {
    (impl $imp:ident for $res:ty, $method:ident) => {
        forward_val_val_binop_commutative!(impl $imp for $res, $method);
        forward_ref_val_binop_commutative!(impl $imp for $res, $method);
        forward_ref_ref_binop_commutative!(impl $imp for $res, $method);
    };
}

macro_rules! forward_all_scalar_binop_to_ref_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        forward_scalar_val_val_binop_to_ref_val!(impl $imp<$scalar> for $res, $method);
        forward_scalar_val_ref_binop_to_ref_val!(impl $imp<$scalar> for $res, $method);
        forward_scalar_ref_ref_binop_to_ref_val!(impl $imp<$scalar> for $res, $method);
    }
}

macro_rules! forward_all_scalar_binop_to_val_val {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        forward_scalar_val_ref_binop_to_val_val!(impl $imp<$scalar> for $res, $method);
        forward_scalar_ref_val_binop_to_val_val!(impl $imp<$scalar> for $res, $method);
        forward_scalar_ref_ref_binop_to_val_val!(impl $imp<$scalar> for $res, $method);
    }
}

macro_rules! forward_all_scalar_binop_to_val_val_commutative {
    (impl $imp:ident<$scalar:ty> for $res:ty, $method:ident) => {
        forward_scalar_val_val_binop_commutative!(impl $imp<$scalar> for $res, $method);
        forward_all_scalar_binop_to_val_val!(impl $imp<$scalar> for $res, $method);
    }
}

macro_rules! promote_all_scalars {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_unsigned_scalars!(impl $imp for $res, $method);
        promote_signed_scalars!(impl $imp for $res, $method);
    }
}

macro_rules! promote_all_scalars_assign {
    (impl $imp:ident for $res:ty, $method:ident) => {
        promote_unsigned_scalars_assign!(impl $imp for $res, $method);
        promote_signed_scalars_assign!(impl $imp for $res, $method);
    }
}

macro_rules! impl_sum_iter_type {
    ($res:ty) => {
        impl<T> Sum<T> for $res
        where
            $res: Add<T, Output = $res>,
        {
            fn sum<I>(iter: I) -> Self
            where
                I: Iterator<Item = T>,
            {
                iter.fold(Self::ZERO, <$res>::add)
            }
        }
    };
}

macro_rules! impl_product_iter_type {
    ($res:ty) => {
        impl<T> Product<T> for $res
        where
            $res: Mul<T, Output = $res>,
        {
            fn product<I>(iter: I) -> Self
            where
                I: Iterator<Item = T>,
            {
                iter.fold(One::one(), <$res>::mul)
            }
        }
    };
}
