//! SGX C types definition

use crate::prelude::*;

pub type intmax_t = i64;
pub type uintmax_t = u64;

pub type size_t = usize;
pub type ptrdiff_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;
pub type ssize_t = isize;

pub const INT_MIN: c_int = -2147483648;
pub const INT_MAX: c_int = 2147483647;
