//! BigNum implementation
//!
//! Large numbers are important for a cryptographic library.  OpenSSL implementation
//! of BigNum uses dynamically assigned memory to store an array of bit chunks.  This
//! allows numbers of any size to be compared and mathematical functions performed.
//!
//! OpenSSL wiki describes the [`BIGNUM`] data structure.
//!
//! # Examples
//!
//! ```
//! use openssl::bn::BigNum;
//! use openssl::error::ErrorStack;
//!
//! fn main() -> Result<(), ErrorStack> {
//!   let a = BigNum::new()?; // a = 0
//!   let b = BigNum::from_dec_str("1234567890123456789012345")?;
//!   let c = &a * &b;
//!   assert_eq!(a, c);
//!   Ok(())
//! }
//! ```
//!
//! [`BIGNUM`]: https://wiki.openssl.org/index.php/Manual:Bn_internal(3)
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::c_int;
use std::cmp::Ordering;
use std::ffi::CString;
use std::ops::{Add, Deref, Div, Mul, Neg, Rem, Shl, Shr, Sub};
use std::{fmt, ptr};

use crate::asn1::Asn1Integer;
use crate::error::ErrorStack;
use crate::string::OpensslString;
use crate::{cvt, cvt_n, cvt_p, LenType};
use openssl_macros::corresponds;

cfg_if! {
    if #[cfg(any(ossl110, libressl350, awslc))] {
        use ffi::{
            BN_get_rfc3526_prime_1536, BN_get_rfc3526_prime_2048, BN_get_rfc3526_prime_3072, BN_get_rfc3526_prime_4096,
            BN_get_rfc3526_prime_6144, BN_get_rfc3526_prime_8192, BN_is_negative,
        };
    } else if #[cfg(boringssl)] {
        use ffi::BN_is_negative;
    } else {
        use ffi::{
            get_rfc3526_prime_1536 as BN_get_rfc3526_prime_1536,
            get_rfc3526_prime_2048 as BN_get_rfc3526_prime_2048,
            get_rfc3526_prime_3072 as BN_get_rfc3526_prime_3072,
            get_rfc3526_prime_4096 as BN_get_rfc3526_prime_4096,
            get_rfc3526_prime_6144 as BN_get_rfc3526_prime_6144,
            get_rfc3526_prime_8192 as BN_get_rfc3526_prime_8192,
        };

        #[allow(bad_style)]
        unsafe fn BN_is_negative(bn: *const ffi::BIGNUM) -> c_int {
            (*bn).neg
        }
    }
}

cfg_if! {
    if #[cfg(any(ossl110, libressl350))] {
        use ffi::{
            BN_get_rfc2409_prime_1024, BN_get_rfc2409_prime_768
        };
    } else if #[cfg(not(any(boringssl, awslc)))] {
        use ffi::{
            get_rfc2409_prime_1024 as BN_get_rfc2409_prime_1024,
            get_rfc2409_prime_768 as BN_get_rfc2409_prime_768,
        };
    }
}

/// Options for the most significant bits of a randomly generated `BigNum`.
pub struct MsbOption(c_int);

impl MsbOption {
    /// The most significant bit of the number may be 0.
    pub const MAYBE_ZERO: MsbOption = MsbOption(-1);

    /// The most significant bit of the number must be 1.
    pub const ONE: MsbOption = MsbOption(0);

    /// The most significant two bits of the number must be 1.
    ///
    /// The number of bits in the product of two such numbers will always be exactly twice the
    /// number of bits in the original numbers.
    pub const TWO_ONES: MsbOption = MsbOption(1);
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::BN_CTX;
    fn drop = ffi::BN_CTX_free;

    /// Temporary storage for BigNums on the secure heap
    ///
    /// BigNum values are stored dynamically and therefore can be expensive
    /// to allocate.  BigNumContext and the OpenSSL [`BN_CTX`] structure are used
    /// internally when passing BigNum values between subroutines.
    ///
    /// [`BN_CTX`]: https://www.openssl.org/docs/manmaster/crypto/BN_CTX_new.html
    pub struct BigNumContext;
    /// Reference to [`BigNumContext`]
    ///
    /// [`BigNumContext`]: struct.BigNumContext.html
    pub struct BigNumContextRef;
}

impl BigNumContext {
    /// Returns a new `BigNumContext`.
    #[corresponds(BN_CTX_new)]
    pub fn new() -> Result<BigNumContext, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::BN_CTX_new()).map(BigNumContext)
        }
    }

    /// Returns a new secure `BigNumContext`.
    #[corresponds(BN_CTX_secure_new)]
    #[cfg(ossl110)]
    pub fn new_secure() -> Result<BigNumContext, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(ffi::BN_CTX_secure_new()).map(BigNumContext)
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::BIGNUM;
    fn drop = ffi::BN_free;

    /// Dynamically sized large number implementation
    ///
    /// Perform large number mathematics.  Create a new BigNum
    /// with [`new`].  Perform standard mathematics on large numbers using
    /// methods from [`Dref<Target = BigNumRef>`]
    ///
    /// OpenSSL documentation at [`BN_new`].
    ///
    /// [`new`]: struct.BigNum.html#method.new
    /// [`Dref<Target = BigNumRef>`]: struct.BigNum.html#deref-methods
    /// [`BN_new`]: https://www.openssl.org/docs/manmaster/crypto/BN_new.html
    ///
    /// # Examples
    /// ```
    /// use openssl::bn::BigNum;
    /// # use openssl::error::ErrorStack;
    /// # fn bignums() -> Result< (), ErrorStack > {
    /// let little_big = BigNum::from_u32(std::u32::MAX)?;
    /// assert_eq!(*&little_big.num_bytes(), 4);
    /// # Ok(())
    /// # }
    /// # fn main () { bignums(); }
    /// ```
    pub struct BigNum;
    /// Reference to a [`BigNum`]
    ///
    /// [`BigNum`]: struct.BigNum.html
    pub struct BigNumRef;
}

impl BigNumRef {
    /// Erases the memory used by this `BigNum`, resetting its value to 0.
    ///
    /// This can be used to destroy sensitive data such as keys when they are no longer needed.
    #[corresponds(BN_clear)]
    pub fn clear(&mut self) {
        unsafe { ffi::BN_clear(self.as_ptr()) }
    }

    /// Adds a `u32` to `self`.
    #[corresponds(BN_add_word)]
    pub fn add_word(&mut self, w: u32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_add_word(self.as_ptr(), w as ffi::BN_ULONG)).map(|_| ()) }
    }

    /// Subtracts a `u32` from `self`.
    #[corresponds(BN_sub_word)]
    pub fn sub_word(&mut self, w: u32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_sub_word(self.as_ptr(), w as ffi::BN_ULONG)).map(|_| ()) }
    }

    /// Multiplies a `u32` by `self`.
    #[corresponds(BN_mul_word)]
    pub fn mul_word(&mut self, w: u32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_mul_word(self.as_ptr(), w as ffi::BN_ULONG)).map(|_| ()) }
    }

    /// Divides `self` by a `u32`, returning the remainder.
    #[corresponds(BN_div_word)]
    #[allow(clippy::useless_conversion)]
    pub fn div_word(&mut self, w: u32) -> Result<u64, ErrorStack> {
        unsafe {
            let r = ffi::BN_div_word(self.as_ptr(), w.into());
            if r == ffi::BN_ULONG::MAX {
                Err(ErrorStack::get())
            } else {
                Ok(r.into())
            }
        }
    }

    /// Returns the result of `self` modulo `w`.
    #[corresponds(BN_mod_word)]
    #[allow(clippy::useless_conversion)]
    pub fn mod_word(&self, w: u32) -> Result<u64, ErrorStack> {
        unsafe {
            let r = ffi::BN_mod_word(self.as_ptr(), w.into());
            if r == ffi::BN_ULONG::MAX {
                Err(ErrorStack::get())
            } else {
                Ok(r.into())
            }
        }
    }

    /// Places a cryptographically-secure pseudo-random nonnegative
    /// number less than `self` in `rnd`.
    #[corresponds(BN_rand_range)]
    pub fn rand_range(&self, rnd: &mut BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_rand_range(rnd.as_ptr(), self.as_ptr())).map(|_| ()) }
    }

    /// The cryptographically weak counterpart to `rand_in_range`.
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[corresponds(BN_pseudo_rand_range)]
    pub fn pseudo_rand_range(&self, rnd: &mut BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_pseudo_rand_range(rnd.as_ptr(), self.as_ptr())).map(|_| ()) }
    }

    /// Sets bit `n`. Equivalent to `self |= (1 << n)`.
    ///
    /// When setting a bit outside of `self`, it is expanded.
    #[corresponds(BN_set_bit)]
    #[allow(clippy::useless_conversion)]
    pub fn set_bit(&mut self, n: i32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_set_bit(self.as_ptr(), n.into())).map(|_| ()) }
    }

    /// Clears bit `n`, setting it to 0. Equivalent to `self &= ~(1 << n)`.
    ///
    /// When clearing a bit outside of `self`, an error is returned.
    #[corresponds(BN_clear_bit)]
    #[allow(clippy::useless_conversion)]
    pub fn clear_bit(&mut self, n: i32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_clear_bit(self.as_ptr(), n.into())).map(|_| ()) }
    }

    /// Returns `true` if the `n`th bit of `self` is set to 1, `false` otherwise.
    #[corresponds(BN_is_bit_set)]
    #[allow(clippy::useless_conversion)]
    pub fn is_bit_set(&self, n: i32) -> bool {
        unsafe { ffi::BN_is_bit_set(self.as_ptr(), n.into()) == 1 }
    }

    /// Truncates `self` to the lowest `n` bits.
    ///
    /// An error occurs if `self` is already shorter than `n` bits.
    #[corresponds(BN_mask_bits)]
    #[allow(clippy::useless_conversion)]
    pub fn mask_bits(&mut self, n: i32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_mask_bits(self.as_ptr(), n.into())).map(|_| ()) }
    }

    /// Places `a << 1` in `self`.  Equivalent to `self * 2`.
    #[corresponds(BN_lshift1)]
    pub fn lshift1(&mut self, a: &BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_lshift1(self.as_ptr(), a.as_ptr())).map(|_| ()) }
    }

    /// Places `a >> 1` in `self`. Equivalent to `self / 2`.
    #[corresponds(BN_rshift1)]
    pub fn rshift1(&mut self, a: &BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_rshift1(self.as_ptr(), a.as_ptr())).map(|_| ()) }
    }

    /// Places `a + b` in `self`.  [`core::ops::Add`] is also implemented for `BigNumRef`.
    ///
    /// [`core::ops::Add`]: struct.BigNumRef.html#method.add
    #[corresponds(BN_add)]
    pub fn checked_add(&mut self, a: &BigNumRef, b: &BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_add(self.as_ptr(), a.as_ptr(), b.as_ptr())).map(|_| ()) }
    }

    /// Places `a - b` in `self`. [`core::ops::Sub`] is also implemented for `BigNumRef`.
    ///
    /// [`core::ops::Sub`]: struct.BigNumRef.html#method.sub
    #[corresponds(BN_sub)]
    pub fn checked_sub(&mut self, a: &BigNumRef, b: &BigNumRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_sub(self.as_ptr(), a.as_ptr(), b.as_ptr())).map(|_| ()) }
    }

    /// Places `a << n` in `self`.  Equivalent to `a * 2 ^ n`.
    #[corresponds(BN_lshift)]
    #[allow(clippy::useless_conversion)]
    pub fn lshift(&mut self, a: &BigNumRef, n: i32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_lshift(self.as_ptr(), a.as_ptr(), n.into())).map(|_| ()) }
    }

    /// Places `a >> n` in `self`. Equivalent to `a / 2 ^ n`.
    #[corresponds(BN_rshift)]
    #[allow(clippy::useless_conversion)]
    pub fn rshift(&mut self, a: &BigNumRef, n: i32) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_rshift(self.as_ptr(), a.as_ptr(), n.into())).map(|_| ()) }
    }

    /// Creates a new BigNum with the same value.
    #[corresponds(BN_dup)]
    pub fn to_owned(&self) -> Result<BigNum, ErrorStack> {
        unsafe { cvt_p(ffi::BN_dup(self.as_ptr())).map(|b| BigNum::from_ptr(b)) }
    }

    /// Sets the sign of `self`.  Pass true to set `self` to a negative.  False sets
    /// `self` positive.
    #[corresponds(BN_set_negative)]
    pub fn set_negative(&mut self, negative: bool) {
        unsafe { ffi::BN_set_negative(self.as_ptr(), negative as c_int) }
    }

    /// Compare the absolute values of `self` and `oth`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// # use std::cmp::Ordering;
    /// let s = -BigNum::from_u32(8).unwrap();
    /// let o = BigNum::from_u32(8).unwrap();
    ///
    /// assert_eq!(s.ucmp(&o), Ordering::Equal);
    /// ```
    #[corresponds(BN_ucmp)]
    pub fn ucmp(&self, oth: &BigNumRef) -> Ordering {
        unsafe { ffi::BN_ucmp(self.as_ptr(), oth.as_ptr()).cmp(&0) }
    }

    /// Returns `true` if `self` is negative.
    #[corresponds(BN_is_negative)]
    pub fn is_negative(&self) -> bool {
        unsafe { BN_is_negative(self.as_ptr()) == 1 }
    }

    /// Returns `true` is `self` is even.
    #[corresponds(BN_is_even)]
    #[cfg(any(ossl110, boringssl, libressl350, awslc))]
    pub fn is_even(&self) -> bool {
        !self.is_odd()
    }

    /// Returns `true` is `self` is odd.
    #[corresponds(BN_is_odd)]
    #[cfg(any(ossl110, boringssl, libressl350, awslc))]
    pub fn is_odd(&self) -> bool {
        unsafe { ffi::BN_is_odd(self.as_ptr()) == 1 }
    }

    /// Returns the number of significant bits in `self`.
    #[corresponds(BN_num_bits)]
    #[allow(clippy::unnecessary_cast)]
    pub fn num_bits(&self) -> i32 {
        unsafe { ffi::BN_num_bits(self.as_ptr()) as i32 }
    }

    /// Returns the size of `self` in bytes. Implemented natively.
    pub fn num_bytes(&self) -> i32 {
        (self.num_bits() + 7) / 8
    }

    /// Generates a cryptographically strong pseudo-random `BigNum`, placing it in `self`.
    ///
    /// # Parameters
    ///
    /// * `bits`: Length of the number in bits.
    /// * `msb`: The desired properties of the most significant bit. See [`constants`].
    /// * `odd`: If `true`, the generated number will be odd.
    ///
    /// # Examples
    ///
    /// ```
    /// use openssl::bn::{BigNum, MsbOption};
    /// use openssl::error::ErrorStack;
    ///
    /// fn generate_random() -> Result< BigNum, ErrorStack > {
    ///    let mut big = BigNum::new()?;
    ///
    ///    // Generates a 128-bit odd random number
    ///    big.rand(128, MsbOption::MAYBE_ZERO, true);
    ///    Ok((big))
    /// }
    /// ```
    ///
    /// [`constants`]: index.html#constants
    #[corresponds(BN_rand)]
    #[allow(clippy::useless_conversion)]
    pub fn rand(&mut self, bits: i32, msb: MsbOption, odd: bool) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_rand(
                self.as_ptr(),
                bits.into(),
                msb.0,
                odd as c_int,
            ))
            .map(|_| ())
        }
    }

    /// The cryptographically weak counterpart to `rand`.  Not suitable for key generation.
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[corresponds(BN_pseudo_rand)]
    #[allow(clippy::useless_conversion)]
    pub fn pseudo_rand(&mut self, bits: i32, msb: MsbOption, odd: bool) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_pseudo_rand(
                self.as_ptr(),
                bits.into(),
                msb.0,
                odd as c_int,
            ))
            .map(|_| ())
        }
    }

    /// Generates a prime number, placing it in `self`.
    ///
    /// # Parameters
    ///
    /// * `bits`: The length of the prime in bits (lower bound).
    /// * `safe`: If true, returns a "safe" prime `p` so that `(p-1)/2` is also prime.
    /// * `add`/`rem`: If `add` is set to `Some(add)`, `p % add == rem` will hold, where `p` is the
    ///   generated prime and `rem` is `1` if not specified (`None`).
    ///
    /// # Examples
    ///
    /// ```
    /// use openssl::bn::BigNum;
    /// use openssl::error::ErrorStack;
    ///
    /// fn generate_weak_prime() -> Result< BigNum, ErrorStack > {
    ///    let mut big = BigNum::new()?;
    ///
    ///    // Generates a 128-bit simple prime number
    ///    big.generate_prime(128, false, None, None);
    ///    Ok((big))
    /// }
    /// ```
    #[corresponds(BN_generate_prime_ex)]
    pub fn generate_prime(
        &mut self,
        bits: i32,
        safe: bool,
        add: Option<&BigNumRef>,
        rem: Option<&BigNumRef>,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_generate_prime_ex(
                self.as_ptr(),
                bits as c_int,
                safe as c_int,
                add.map(|n| n.as_ptr()).unwrap_or(ptr::null_mut()),
                rem.map(|n| n.as_ptr()).unwrap_or(ptr::null_mut()),
                ptr::null_mut(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a * b` in `self`.
    /// [`core::ops::Mul`] is also implemented for `BigNumRef`.
    ///
    /// [`core::ops::Mul`]: struct.BigNumRef.html#method.mul
    #[corresponds(BN_mul)]
    pub fn checked_mul(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mul(
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a / b` in `self`. The remainder is discarded.
    /// [`core::ops::Div`] is also implemented for `BigNumRef`.
    ///
    /// [`core::ops::Div`]: struct.BigNumRef.html#method.div
    #[corresponds(BN_div)]
    pub fn checked_div(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_div(
                self.as_ptr(),
                ptr::null_mut(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a % b` in `self`.
    #[corresponds(BN_div)]
    pub fn checked_rem(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_div(
                ptr::null_mut(),
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a / b` in `self` and `a % b` in `rem`.
    #[corresponds(BN_div)]
    pub fn div_rem(
        &mut self,
        rem: &mut BigNumRef,
        a: &BigNumRef,
        b: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_div(
                self.as_ptr(),
                rem.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a²` in `self`.
    #[corresponds(BN_sqr)]
    pub fn sqr(&mut self, a: &BigNumRef, ctx: &mut BigNumContextRef) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::BN_sqr(self.as_ptr(), a.as_ptr(), ctx.as_ptr())).map(|_| ()) }
    }

    /// Places the result of `a mod m` in `self`.  As opposed to `div_rem`
    /// the result is non-negative.
    #[corresponds(BN_nnmod)]
    pub fn nnmod(
        &mut self,
        a: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_nnmod(
                self.as_ptr(),
                a.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `(a + b) mod m` in `self`.
    #[corresponds(BN_mod_add)]
    pub fn mod_add(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mod_add(
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `(a - b) mod m` in `self`.
    #[corresponds(BN_mod_sub)]
    pub fn mod_sub(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mod_sub(
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `(a * b) mod m` in `self`.
    #[corresponds(BN_mod_mul)]
    pub fn mod_mul(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mod_mul(
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a² mod m` in `self`.
    #[corresponds(BN_mod_sqr)]
    pub fn mod_sqr(
        &mut self,
        a: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mod_sqr(
                self.as_ptr(),
                a.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places into `self` the modular square root of `a` such that `self^2 = a (mod p)`
    #[corresponds(BN_mod_sqrt)]
    pub fn mod_sqrt(
        &mut self,
        a: &BigNumRef,
        p: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt_p(ffi::BN_mod_sqrt(
                self.as_ptr(),
                a.as_ptr(),
                p.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a^p` in `self`.
    #[corresponds(BN_exp)]
    pub fn exp(
        &mut self,
        a: &BigNumRef,
        p: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_exp(
                self.as_ptr(),
                a.as_ptr(),
                p.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the result of `a^p mod m` in `self`.
    #[corresponds(BN_mod_exp)]
    pub fn mod_exp(
        &mut self,
        a: &BigNumRef,
        p: &BigNumRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_mod_exp(
                self.as_ptr(),
                a.as_ptr(),
                p.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the inverse of `a` modulo `n` in `self`.
    #[corresponds(BN_mod_inverse)]
    pub fn mod_inverse(
        &mut self,
        a: &BigNumRef,
        n: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt_p(ffi::BN_mod_inverse(
                self.as_ptr(),
                a.as_ptr(),
                n.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the greatest common denominator of `a` and `b` in `self`.
    #[corresponds(BN_gcd)]
    pub fn gcd(
        &mut self,
        a: &BigNumRef,
        b: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::BN_gcd(
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Checks whether `self` is prime.
    ///
    /// Performs a Miller-Rabin probabilistic primality test with `checks` iterations.
    ///
    /// # Return Value
    ///
    /// Returns `true` if `self` is prime with an error probability of less than `0.25 ^ checks`.
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[corresponds(BN_is_prime_ex)]
    #[allow(clippy::useless_conversion)]
    pub fn is_prime(&self, checks: i32, ctx: &mut BigNumContextRef) -> Result<bool, ErrorStack> {
        unsafe {
            cvt_n(ffi::BN_is_prime_ex(
                self.as_ptr(),
                checks.into(),
                ctx.as_ptr(),
                ptr::null_mut(),
            ))
            .map(|r| r != 0)
        }
    }

    /// Checks whether `self` is prime with optional trial division.
    ///
    /// If `do_trial_division` is `true`, first performs trial division by a number of small primes.
    /// Then, like `is_prime`, performs a Miller-Rabin probabilistic primality test with `checks`
    /// iterations.
    ///
    /// # Return Value
    ///
    /// Returns `true` if `self` is prime with an error probability of less than `0.25 ^ checks`.
    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[corresponds(BN_is_prime_fasttest_ex)]
    #[allow(clippy::useless_conversion)]
    pub fn is_prime_fasttest(
        &self,
        checks: i32,
        ctx: &mut BigNumContextRef,
        do_trial_division: bool,
    ) -> Result<bool, ErrorStack> {
        unsafe {
            cvt_n(ffi::BN_is_prime_fasttest_ex(
                self.as_ptr(),
                checks.into(),
                ctx.as_ptr(),
                do_trial_division as c_int,
                ptr::null_mut(),
            ))
            .map(|r| r != 0)
        }
    }

    /// Returns a big-endian byte vector representation of the absolute value of `self`.
    ///
    /// `self` can be recreated by using `from_slice`.
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let s = -BigNum::from_u32(4543).unwrap();
    /// let r = BigNum::from_u32(4543).unwrap();
    ///
    /// let s_vec = s.to_vec();
    /// assert_eq!(BigNum::from_slice(&s_vec).unwrap(), r);
    /// ```
    #[corresponds(BN_bn2bin)]
    pub fn to_vec(&self) -> Vec<u8> {
        let size = self.num_bytes() as usize;
        let mut v = Vec::with_capacity(size);
        unsafe {
            ffi::BN_bn2bin(self.as_ptr(), v.as_mut_ptr());
            v.set_len(size);
        }
        v
    }

    /// Returns a big-endian byte vector representation of the absolute value of `self` padded
    /// to `pad_to` bytes.
    ///
    /// If `pad_to` is less than `self.num_bytes()` then an error is returned.
    ///
    /// `self` can be recreated by using `from_slice`.
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let bn = BigNum::from_u32(0x4543).unwrap();
    ///
    /// let bn_vec = bn.to_vec_padded(4).unwrap();
    /// assert_eq!(&bn_vec, &[0, 0, 0x45, 0x43]);
    ///
    /// let r = bn.to_vec_padded(1);
    /// assert!(r.is_err());
    ///
    /// let bn = -BigNum::from_u32(0x4543).unwrap();
    /// let bn_vec = bn.to_vec_padded(4).unwrap();
    /// assert_eq!(&bn_vec, &[0, 0, 0x45, 0x43]);
    /// ```
    #[corresponds(BN_bn2binpad)]
    #[cfg(any(ossl110, libressl340, boringssl, awslc))]
    pub fn to_vec_padded(&self, pad_to: i32) -> Result<Vec<u8>, ErrorStack> {
        let mut v = Vec::with_capacity(pad_to as usize);
        unsafe {
            cvt(ffi::BN_bn2binpad(self.as_ptr(), v.as_mut_ptr(), pad_to))?;
            v.set_len(pad_to as usize);
        }
        Ok(v)
    }

    /// Returns a decimal string representation of `self`.
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let s = -BigNum::from_u32(12345).unwrap();
    ///
    /// assert_eq!(&**s.to_dec_str().unwrap(), "-12345");
    /// ```
    #[corresponds(BN_bn2dec)]
    pub fn to_dec_str(&self) -> Result<OpensslString, ErrorStack> {
        unsafe {
            let buf = cvt_p(ffi::BN_bn2dec(self.as_ptr()))?;
            Ok(OpensslString::from_ptr(buf))
        }
    }

    /// Returns a hexadecimal string representation of `self`.
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let s = -BigNum::from_u32(0x99ff).unwrap();
    ///
    /// assert_eq!(s.to_hex_str().unwrap().to_uppercase(), "-99FF");
    /// ```
    #[corresponds(BN_bn2hex)]
    pub fn to_hex_str(&self) -> Result<OpensslString, ErrorStack> {
        unsafe {
            let buf = cvt_p(ffi::BN_bn2hex(self.as_ptr()))?;
            Ok(OpensslString::from_ptr(buf))
        }
    }

    /// Returns an `Asn1Integer` containing the value of `self`.
    #[corresponds(BN_to_ASN1_INTEGER)]
    pub fn to_asn1_integer(&self) -> Result<Asn1Integer, ErrorStack> {
        unsafe {
            cvt_p(ffi::BN_to_ASN1_INTEGER(self.as_ptr(), ptr::null_mut()))
                .map(|p| Asn1Integer::from_ptr(p))
        }
    }

    /// Force constant time computation on this value.
    #[corresponds(BN_set_flags)]
    #[cfg(ossl110)]
    pub fn set_const_time(&mut self) {
        unsafe { ffi::BN_set_flags(self.as_ptr(), ffi::BN_FLG_CONSTTIME) }
    }

    /// Returns true if `self` is in const time mode.
    #[corresponds(BN_get_flags)]
    #[cfg(ossl110)]
    pub fn is_const_time(&self) -> bool {
        unsafe {
            let ret = ffi::BN_get_flags(self.as_ptr(), ffi::BN_FLG_CONSTTIME);
            ret == ffi::BN_FLG_CONSTTIME
        }
    }

    /// Returns true if `self` was created with [`BigNum::new_secure`].
    #[corresponds(BN_get_flags)]
    #[cfg(ossl110)]
    pub fn is_secure(&self) -> bool {
        unsafe {
            let ret = ffi::BN_get_flags(self.as_ptr(), ffi::BN_FLG_SECURE);
            ret == ffi::BN_FLG_SECURE
        }
    }
}

impl BigNum {
    /// Creates a new `BigNum` with the value 0.
    #[corresponds(BN_new)]
    pub fn new() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            let v = cvt_p(ffi::BN_new())?;
            Ok(BigNum::from_ptr(v))
        }
    }

    /// Returns a new secure `BigNum`.
    #[corresponds(BN_secure_new)]
    #[cfg(ossl110)]
    pub fn new_secure() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            let v = cvt_p(ffi::BN_secure_new())?;
            Ok(BigNum::from_ptr(v))
        }
    }

    /// Creates a new `BigNum` with the given value.
    #[corresponds(BN_set_word)]
    pub fn from_u32(n: u32) -> Result<BigNum, ErrorStack> {
        BigNum::new().and_then(|v| unsafe {
            cvt(ffi::BN_set_word(v.as_ptr(), n as ffi::BN_ULONG)).map(|_| v)
        })
    }

    /// Creates a `BigNum` from a decimal string.
    #[corresponds(BN_dec2bn)]
    pub fn from_dec_str(s: &str) -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            let c_str = CString::new(s.as_bytes()).unwrap();
            let mut bn = ptr::null_mut();
            cvt(ffi::BN_dec2bn(&mut bn, c_str.as_ptr() as *const _))?;
            Ok(BigNum::from_ptr(bn))
        }
    }

    /// Creates a `BigNum` from a hexadecimal string.
    #[corresponds(BN_hex2bn)]
    pub fn from_hex_str(s: &str) -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            let c_str = CString::new(s.as_bytes()).unwrap();
            let mut bn = ptr::null_mut();
            cvt(ffi::BN_hex2bn(&mut bn, c_str.as_ptr() as *const _))?;
            Ok(BigNum::from_ptr(bn))
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 2409`].  This prime number is in
    /// the order of magnitude of `2 ^ 768`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled Oakley group id 1.
    ///
    /// [`RFC 2409`]: https://tools.ietf.org/html/rfc2409#page-21
    #[corresponds(BN_get_rfc2409_prime_768)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn get_rfc2409_prime_768() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc2409_prime_768(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 2409`].  This prime number is in
    /// the order of magnitude of `2 ^ 1024`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled Oakly group 2.
    ///
    /// [`RFC 2409`]: https://tools.ietf.org/html/rfc2409#page-21
    #[corresponds(BN_get_rfc2409_prime_1024)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn get_rfc2409_prime_1024() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc2409_prime_1024(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 1536`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 5.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-3
    #[corresponds(BN_get_rfc3526_prime_1536)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_1536() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_1536(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 2048`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 14.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-3
    #[corresponds(BN_get_rfc3526_prime_2048)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_2048() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_2048(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 3072`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 15.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-4
    #[corresponds(BN_get_rfc3526_prime_3072)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_3072() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_3072(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 4096`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 16.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-4
    #[corresponds(BN_get_rfc3526_prime_4096)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_4096() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_4096(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 6144`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 17.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-6
    #[corresponds(BN_get_rfc3526_prime_6114)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_6144() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_6144(ptr::null_mut())).map(BigNum)
        }
    }

    /// Returns a constant used in IKE as defined in [`RFC 3526`].  The prime is in the order
    /// of magnitude of `2 ^ 8192`.  This number is used during calculated key
    /// exchanges such as Diffie-Hellman.  This number is labeled MODP group 18.
    ///
    /// [`RFC 3526`]: https://tools.ietf.org/html/rfc3526#page-6
    #[corresponds(BN_get_rfc3526_prime_8192)]
    #[cfg(not(boringssl))]
    pub fn get_rfc3526_prime_8192() -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            cvt_p(BN_get_rfc3526_prime_8192(ptr::null_mut())).map(BigNum)
        }
    }

    /// Creates a new `BigNum` from an unsigned, big-endian encoded number of arbitrary length.
    ///
    /// OpenSSL documentation at [`BN_bin2bn`]
    ///
    /// [`BN_bin2bn`]: https://www.openssl.org/docs/manmaster/crypto/BN_bin2bn.html
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let bignum = BigNum::from_slice(&[0x12, 0x00, 0x34]).unwrap();
    ///
    /// assert_eq!(bignum, BigNum::from_u32(0x120034).unwrap());
    /// ```
    #[corresponds(BN_bin2bn)]
    pub fn from_slice(n: &[u8]) -> Result<BigNum, ErrorStack> {
        unsafe {
            ffi::init();
            assert!(n.len() <= LenType::MAX as usize);

            cvt_p(ffi::BN_bin2bn(
                n.as_ptr(),
                n.len() as LenType,
                ptr::null_mut(),
            ))
            .map(|p| BigNum::from_ptr(p))
        }
    }

    /// Copies data from a slice overwriting what was in the BigNum.
    ///
    /// This function can be used to copy data from a slice to a
    /// [secure BigNum][`BigNum::new_secure`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use openssl::bn::BigNum;
    /// let mut bignum = BigNum::new().unwrap();
    /// bignum.copy_from_slice(&[0x12, 0x00, 0x34]).unwrap();
    ///
    /// assert_eq!(bignum, BigNum::from_u32(0x120034).unwrap());
    /// ```
    #[corresponds(BN_bin2bn)]
    pub fn copy_from_slice(&mut self, n: &[u8]) -> Result<(), ErrorStack> {
        unsafe {
            assert!(n.len() <= LenType::MAX as usize);

            cvt_p(ffi::BN_bin2bn(n.as_ptr(), n.len() as LenType, self.0))?;
            Ok(())
        }
    }
}

impl fmt::Debug for BigNumRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_dec_str() {
            Ok(s) => f.write_str(&s),
            Err(e) => Err(e.into()),
        }
    }
}

impl fmt::Debug for BigNum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_dec_str() {
            Ok(s) => f.write_str(&s),
            Err(e) => Err(e.into()),
        }
    }
}

impl fmt::Display for BigNumRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_dec_str() {
            Ok(s) => f.write_str(&s),
            Err(e) => Err(e.into()),
        }
    }
}

impl fmt::Display for BigNum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_dec_str() {
            Ok(s) => f.write_str(&s),
            Err(e) => Err(e.into()),
        }
    }
}

impl PartialEq<BigNumRef> for BigNumRef {
    fn eq(&self, oth: &BigNumRef) -> bool {
        self.cmp(oth) == Ordering::Equal
    }
}

impl PartialEq<BigNum> for BigNumRef {
    fn eq(&self, oth: &BigNum) -> bool {
        self.eq(oth.deref())
    }
}

impl Eq for BigNumRef {}

impl PartialEq for BigNum {
    fn eq(&self, oth: &BigNum) -> bool {
        self.deref().eq(oth)
    }
}

impl PartialEq<BigNumRef> for BigNum {
    fn eq(&self, oth: &BigNumRef) -> bool {
        self.deref().eq(oth)
    }
}

impl Eq for BigNum {}

impl PartialOrd<BigNumRef> for BigNumRef {
    fn partial_cmp(&self, oth: &BigNumRef) -> Option<Ordering> {
        Some(self.cmp(oth))
    }
}

impl PartialOrd<BigNum> for BigNumRef {
    fn partial_cmp(&self, oth: &BigNum) -> Option<Ordering> {
        Some(self.cmp(oth.deref()))
    }
}

impl Ord for BigNumRef {
    fn cmp(&self, oth: &BigNumRef) -> Ordering {
        unsafe { ffi::BN_cmp(self.as_ptr(), oth.as_ptr()).cmp(&0) }
    }
}

impl PartialOrd for BigNum {
    fn partial_cmp(&self, oth: &BigNum) -> Option<Ordering> {
        Some(self.cmp(oth))
    }
}

impl PartialOrd<BigNumRef> for BigNum {
    fn partial_cmp(&self, oth: &BigNumRef) -> Option<Ordering> {
        self.deref().partial_cmp(oth)
    }
}

impl Ord for BigNum {
    fn cmp(&self, oth: &BigNum) -> Ordering {
        self.deref().cmp(oth.deref())
    }
}

macro_rules! delegate {
    ($t:ident, $m:ident) => {
        impl<'a, 'b> $t<&'b BigNum> for &'a BigNumRef {
            type Output = BigNum;

            fn $m(self, oth: &BigNum) -> BigNum {
                $t::$m(self, oth.deref())
            }
        }

        impl<'a, 'b> $t<&'b BigNumRef> for &'a BigNum {
            type Output = BigNum;

            fn $m(self, oth: &BigNumRef) -> BigNum {
                $t::$m(self.deref(), oth)
            }
        }

        impl<'a, 'b> $t<&'b BigNum> for &'a BigNum {
            type Output = BigNum;

            fn $m(self, oth: &BigNum) -> BigNum {
                $t::$m(self.deref(), oth.deref())
            }
        }
    };
}

impl Add<&BigNumRef> for &BigNumRef {
    type Output = BigNum;

    fn add(self, oth: &BigNumRef) -> BigNum {
        let mut r = BigNum::new().unwrap();
        r.checked_add(self, oth).unwrap();
        r
    }
}

delegate!(Add, add);

impl Sub<&BigNumRef> for &BigNumRef {
    type Output = BigNum;

    fn sub(self, oth: &BigNumRef) -> BigNum {
        let mut r = BigNum::new().unwrap();
        r.checked_sub(self, oth).unwrap();
        r
    }
}

delegate!(Sub, sub);

impl Mul<&BigNumRef> for &BigNumRef {
    type Output = BigNum;

    fn mul(self, oth: &BigNumRef) -> BigNum {
        let mut ctx = BigNumContext::new().unwrap();
        let mut r = BigNum::new().unwrap();
        r.checked_mul(self, oth, &mut ctx).unwrap();
        r
    }
}

delegate!(Mul, mul);

impl<'b> Div<&'b BigNumRef> for &BigNumRef {
    type Output = BigNum;

    fn div(self, oth: &'b BigNumRef) -> BigNum {
        let mut ctx = BigNumContext::new().unwrap();
        let mut r = BigNum::new().unwrap();
        r.checked_div(self, oth, &mut ctx).unwrap();
        r
    }
}

delegate!(Div, div);

impl<'b> Rem<&'b BigNumRef> for &BigNumRef {
    type Output = BigNum;

    fn rem(self, oth: &'b BigNumRef) -> BigNum {
        let mut ctx = BigNumContext::new().unwrap();
        let mut r = BigNum::new().unwrap();
        r.checked_rem(self, oth, &mut ctx).unwrap();
        r
    }
}

delegate!(Rem, rem);

impl Shl<i32> for &BigNumRef {
    type Output = BigNum;

    fn shl(self, n: i32) -> BigNum {
        let mut r = BigNum::new().unwrap();
        r.lshift(self, n).unwrap();
        r
    }
}

impl Shl<i32> for &BigNum {
    type Output = BigNum;

    fn shl(self, n: i32) -> BigNum {
        self.deref().shl(n)
    }
}

impl Shr<i32> for &BigNumRef {
    type Output = BigNum;

    fn shr(self, n: i32) -> BigNum {
        let mut r = BigNum::new().unwrap();
        r.rshift(self, n).unwrap();
        r
    }
}

impl Shr<i32> for &BigNum {
    type Output = BigNum;

    fn shr(self, n: i32) -> BigNum {
        self.deref().shr(n)
    }
}

impl Neg for &BigNumRef {
    type Output = BigNum;

    fn neg(self) -> BigNum {
        self.to_owned().unwrap().neg()
    }
}

impl Neg for &BigNum {
    type Output = BigNum;

    fn neg(self) -> BigNum {
        self.deref().neg()
    }
}

impl Neg for BigNum {
    type Output = BigNum;

    fn neg(mut self) -> BigNum {
        let negative = self.is_negative();
        self.set_negative(!negative);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::bn::{BigNum, BigNumContext};

    #[test]
    fn test_to_from_slice() {
        let v0 = BigNum::from_u32(10_203_004).unwrap();
        let vec = v0.to_vec();
        let v1 = BigNum::from_slice(&vec).unwrap();

        assert_eq!(v0, v1);
    }

    #[test]
    fn test_negation() {
        let a = BigNum::from_u32(909_829_283).unwrap();

        assert!(!a.is_negative());
        assert!((-a).is_negative());
    }

    #[test]
    fn test_shift() {
        let a = BigNum::from_u32(909_829_283).unwrap();

        assert_eq!(a, &(&a << 1) >> 1);
    }

    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[test]
    fn test_rand_range() {
        let range = BigNum::from_u32(909_829_283).unwrap();
        let mut result = BigNum::from_dec_str(&range.to_dec_str().unwrap()).unwrap();
        range.rand_range(&mut result).unwrap();
        assert!(result >= BigNum::from_u32(0).unwrap() && result < range);
    }

    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[test]
    fn test_pseudo_rand_range() {
        let range = BigNum::from_u32(909_829_283).unwrap();
        let mut result = BigNum::from_dec_str(&range.to_dec_str().unwrap()).unwrap();
        range.pseudo_rand_range(&mut result).unwrap();
        assert!(result >= BigNum::from_u32(0).unwrap() && result < range);
    }

    #[cfg(not(osslconf = "OPENSSL_NO_DEPRECATED_3_0"))]
    #[test]
    fn test_prime_numbers() {
        let a = BigNum::from_u32(19_029_017).unwrap();
        let mut p = BigNum::new().unwrap();
        p.generate_prime(128, true, None, Some(&a)).unwrap();

        let mut ctx = BigNumContext::new().unwrap();
        assert!(p.is_prime(100, &mut ctx).unwrap());
        assert!(p.is_prime_fasttest(100, &mut ctx, true).unwrap());
    }

    #[cfg(ossl110)]
    #[test]
    fn test_secure_bn_ctx() {
        let mut cxt = BigNumContext::new_secure().unwrap();
        let a = BigNum::from_u32(8).unwrap();
        let b = BigNum::from_u32(3).unwrap();

        let mut remainder = BigNum::new().unwrap();
        remainder.nnmod(&a, &b, &mut cxt).unwrap();

        assert!(remainder.eq(&BigNum::from_u32(2).unwrap()));
    }

    #[cfg(ossl110)]
    #[test]
    fn test_secure_bn() {
        let a = BigNum::new().unwrap();
        assert!(!a.is_secure());

        let b = BigNum::new_secure().unwrap();
        assert!(b.is_secure())
    }

    #[cfg(ossl110)]
    #[test]
    fn test_const_time_bn() {
        let a = BigNum::new().unwrap();
        assert!(!a.is_const_time());

        let mut b = BigNum::new().unwrap();
        b.set_const_time();
        assert!(b.is_const_time())
    }

    #[test]
    fn test_mod_sqrt() {
        let mut ctx = BigNumContext::new().unwrap();

        let s = BigNum::from_hex_str("2").unwrap();
        let p = BigNum::from_hex_str("7DEB1").unwrap();
        let mut sqrt = BigNum::new().unwrap();
        let mut out = BigNum::new().unwrap();

        // Square the root because OpenSSL randomly returns one of 2E42C or 4FA85
        sqrt.mod_sqrt(&s, &p, &mut ctx).unwrap();
        out.mod_sqr(&sqrt, &p, &mut ctx).unwrap();
        assert!(out == s);

        let s = BigNum::from_hex_str("3").unwrap();
        let p = BigNum::from_hex_str("5").unwrap();
        assert!(out.mod_sqrt(&s, &p, &mut ctx).is_err());
    }

    #[test]
    #[cfg(any(ossl110, boringssl, libressl350, awslc))]
    fn test_odd_even() {
        let a = BigNum::from_u32(17).unwrap();
        let b = BigNum::from_u32(18).unwrap();

        assert!(a.is_odd());
        assert!(!b.is_odd());

        assert!(!a.is_even());
        assert!(b.is_even());
    }
}
