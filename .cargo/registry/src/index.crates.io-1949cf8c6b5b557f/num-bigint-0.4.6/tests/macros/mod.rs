#![allow(unused)]

/// Assert that an op works for all val/ref combinations
macro_rules! assert_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {
        assert_eq!((&$left) $op (&$right), $expected);
        assert_eq!((&$left) $op $right.clone(), $expected);
        assert_eq!($left.clone() $op (&$right), $expected);
        assert_eq!($left.clone() $op $right.clone(), $expected);
    };
}

/// Assert that an assign-op works for all val/ref combinations
macro_rules! assert_assign_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {{
        let mut left = $left.clone();
        assert_eq!({ left $op &$right; left}, $expected);

        let mut left = $left.clone();
        assert_eq!({ left $op $right.clone(); left}, $expected);
    }};
}

/// Assert that an op works for scalar left or right
macro_rules! assert_scalar_op {
    (($($to:ident),*) $left:ident $op:tt $right:ident == $expected:expr) => {
        $(
            if let Some(left) = $left.$to() {
                assert_op!(left $op $right == $expected);
            }
            if let Some(right) = $right.$to() {
                assert_op!($left $op right == $expected);
            }
        )*
    };
}

macro_rules! assert_unsigned_scalar_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {
        assert_scalar_op!((to_u8, to_u16, to_u32, to_u64, to_usize, to_u128)
                          $left $op $right == $expected);
    };
}

macro_rules! assert_signed_scalar_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {
        assert_scalar_op!((to_u8, to_u16, to_u32, to_u64, to_usize, to_u128,
                           to_i8, to_i16, to_i32, to_i64, to_isize, to_i128)
                          $left $op $right == $expected);
    };
}

/// Assert that an op works for scalar right
macro_rules! assert_scalar_assign_op {
    (($($to:ident),*) $left:ident $op:tt $right:ident == $expected:expr) => {
        $(
            if let Some(right) = $right.$to() {
                let mut left = $left.clone();
                assert_eq!({ left $op right; left}, $expected);
            }
        )*
    };
}

macro_rules! assert_unsigned_scalar_assign_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {
        assert_scalar_assign_op!((to_u8, to_u16, to_u32, to_u64, to_usize, to_u128)
                                 $left $op $right == $expected);
    };
}

macro_rules! assert_signed_scalar_assign_op {
    ($left:ident $op:tt $right:ident == $expected:expr) => {
        assert_scalar_assign_op!((to_u8, to_u16, to_u32, to_u64, to_usize, to_u128,
                                  to_i8, to_i16, to_i32, to_i64, to_isize, to_i128)
                                 $left $op $right == $expected);
    };
}
