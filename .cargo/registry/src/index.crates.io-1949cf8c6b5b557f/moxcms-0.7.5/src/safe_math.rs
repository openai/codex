/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::CmsError;
use std::ops::Add;

pub(crate) trait SafeAdd<T: Copy + Add<T, Output = T>> {
    fn safe_add(&self, other: T) -> Result<T, CmsError>;
}

pub(crate) trait SafeMul<T: Copy + Add<T, Output = T>> {
    fn safe_mul(&self, other: T) -> Result<T, CmsError>;
}

pub(crate) trait SafePowi<T: Copy + Add<T, Output = T>> {
    fn safe_powi(&self, power: u32) -> Result<T, CmsError>;
}

macro_rules! safe_add_impl {
    ($type_name: ident) => {
        impl SafeAdd<$type_name> for $type_name {
            #[inline(always)]
            fn safe_add(&self, other: $type_name) -> Result<$type_name, CmsError> {
                if let Some(result) = self.checked_add(other) {
                    return Ok(result);
                }
                Err(CmsError::OverflowingError)
            }
        }
    };
}

safe_add_impl!(u16);
safe_add_impl!(u32);
safe_add_impl!(i32);
safe_add_impl!(usize);
safe_add_impl!(isize);

macro_rules! safe_mul_impl {
    ($type_name: ident) => {
        impl SafeMul<$type_name> for $type_name {
            #[inline(always)]
            fn safe_mul(&self, other: $type_name) -> Result<$type_name, CmsError> {
                if let Some(result) = self.checked_mul(other) {
                    return Ok(result);
                }
                Err(CmsError::OverflowingError)
            }
        }
    };
}

safe_mul_impl!(u16);
safe_mul_impl!(u32);
safe_mul_impl!(i32);
safe_mul_impl!(usize);
safe_mul_impl!(isize);

macro_rules! safe_powi_impl {
    ($type_name: ident) => {
        impl SafePowi<$type_name> for $type_name {
            #[inline(always)]
            fn safe_powi(&self, power: u32) -> Result<$type_name, CmsError> {
                if let Some(result) = self.checked_pow(power) {
                    return Ok(result);
                }
                Err(CmsError::OverflowingError)
            }
        }
    };
}

safe_powi_impl!(u8);
safe_powi_impl!(u16);
safe_powi_impl!(u32);
safe_powi_impl!(i32);
safe_powi_impl!(usize);
safe_powi_impl!(isize);
