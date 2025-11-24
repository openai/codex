//! Elliptic Curve
//!
//! Cryptography relies on the difficulty of solving mathematical problems, such as the factor
//! of large integers composed of two large prime numbers and the discrete logarithm of a
//! random elliptic curve.  This module provides low-level features of the latter.
//! Elliptic Curve protocols can provide the same security with smaller keys.
//!
//! There are 2 forms of elliptic curves, `Fp` and `F2^m`.  These curves use irreducible
//! trinomial or pentanomial.  Being a generic interface to a wide range of algorithms,
//! the curves are generally referenced by [`EcGroup`].  There are many built-in groups
//! found in [`Nid`].
//!
//! OpenSSL Wiki explains the fields and curves in detail at [Elliptic Curve Cryptography].
//!
//! [`EcGroup`]: struct.EcGroup.html
//! [`Nid`]: ../nid/struct.Nid.html
//! [Elliptic Curve Cryptography]: https://wiki.openssl.org/index.php/Elliptic_Curve_Cryptography
use cfg_if::cfg_if;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::c_int;
use std::fmt;
use std::ptr;

use crate::bn::{BigNum, BigNumContextRef, BigNumRef};
use crate::error::ErrorStack;
use crate::nid::Nid;
use crate::pkey::{HasParams, HasPrivate, HasPublic, Params, Private, Public};
use crate::util::ForeignTypeRefExt;
use crate::{cvt, cvt_n, cvt_p, init};
use openssl_macros::corresponds;

cfg_if! {
    if #[cfg(not(any(boringssl, awslc)))] {
        use std::ffi::CString;
        use crate::string::OpensslString;
    }
}

/// Compressed or Uncompressed conversion
///
/// Conversion from the binary value of the point on the curve is performed in one of
/// compressed, uncompressed, or hybrid conversions.  The default is compressed, except
/// for binary curves.
///
/// Further documentation is available in the [X9.62] standard.
///
/// [X9.62]: http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.202.2977&rep=rep1&type=pdf
#[derive(Copy, Clone)]
pub struct PointConversionForm(ffi::point_conversion_form_t);

impl PointConversionForm {
    /// Compressed conversion from point value.
    pub const COMPRESSED: PointConversionForm =
        PointConversionForm(ffi::point_conversion_form_t::POINT_CONVERSION_COMPRESSED);

    /// Uncompressed conversion from point value.
    pub const UNCOMPRESSED: PointConversionForm =
        PointConversionForm(ffi::point_conversion_form_t::POINT_CONVERSION_UNCOMPRESSED);

    /// Performs both compressed and uncompressed conversions.
    pub const HYBRID: PointConversionForm =
        PointConversionForm(ffi::point_conversion_form_t::POINT_CONVERSION_HYBRID);
}

/// Named Curve or Explicit
///
/// This type acts as a boolean as to whether the `EcGroup` is named or explicit.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Asn1Flag(c_int);

impl Asn1Flag {
    /// Curve defined using polynomial parameters
    ///
    /// Most applications use a named EC_GROUP curve, however, support
    /// is included to explicitly define the curve used to calculate keys
    /// This information would need to be known by both endpoint to make communication
    /// effective.
    ///
    /// OPENSSL_EC_EXPLICIT_CURVE, but that was only added in 1.1.
    /// Man page documents that 0 can be used in older versions.
    ///
    /// OpenSSL documentation at [`EC_GROUP`]
    ///
    /// [`EC_GROUP`]: https://www.openssl.org/docs/manmaster/crypto/EC_GROUP_get_seed_len.html
    pub const EXPLICIT_CURVE: Asn1Flag = Asn1Flag(0);

    /// Standard Curves
    ///
    /// Curves that make up the typical encryption use cases.  The collection of curves
    /// are well known but extensible.
    ///
    /// OpenSSL documentation at [`EC_GROUP`]
    ///
    /// [`EC_GROUP`]: https://www.openssl.org/docs/manmaster/man3/EC_GROUP_order_bits.html
    pub const NAMED_CURVE: Asn1Flag = Asn1Flag(ffi::OPENSSL_EC_NAMED_CURVE);
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::EC_GROUP;
    fn drop = ffi::EC_GROUP_free;

    /// Describes the curve
    ///
    /// A curve can be of the named curve type.  These curves can be discovered
    /// using openssl binary `openssl ecparam -list_curves`.  Other operations
    /// are available in the [wiki].  These named curves are available in the
    /// [`Nid`] module.
    ///
    /// Curves can also be generated using prime field parameters or a binary field.
    ///
    /// Prime fields use the formula `y^2 mod p = x^3 + ax + b mod p`.  Binary
    /// fields use the formula `y^2 + xy = x^3 + ax^2 + b`.  Named curves have
    /// assured security.  To prevent accidental vulnerabilities, they should
    /// be preferred.
    ///
    /// [wiki]: https://wiki.openssl.org/index.php/Command_Line_Elliptic_Curve_Operations
    /// [`Nid`]: ../nid/index.html
    pub struct EcGroup;
    /// Reference to [`EcGroup`]
    ///
    /// [`EcGroup`]: struct.EcGroup.html
    pub struct EcGroupRef;
}

impl EcGroup {
    /// Returns the group of a standard named curve.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::nid::Nid;
    /// use openssl::ec::{EcGroup, EcKey};
    ///
    /// let nid = Nid::X9_62_PRIME256V1; // NIST P-256 curve
    /// let group = EcGroup::from_curve_name(nid)?;
    /// let key = EcKey::generate(&group)?;
    /// # Ok(()) }
    /// ```
    #[corresponds(EC_GROUP_new_by_curve_name)]
    pub fn from_curve_name(nid: Nid) -> Result<EcGroup, ErrorStack> {
        unsafe {
            init();
            cvt_p(ffi::EC_GROUP_new_by_curve_name(nid.as_raw())).map(EcGroup)
        }
    }

    /// Returns the group for given parameters
    #[corresponds(EC_GROUP_new_curve_GFp)]
    pub fn from_components(
        p: BigNum,
        a: BigNum,
        b: BigNum,
        ctx: &mut BigNumContextRef,
    ) -> Result<EcGroup, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_GROUP_new_curve_GFp(
                p.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(EcGroup)
        }
    }
}

impl EcGroupRef {
    /// Places the components of a curve over a prime field in the provided `BigNum`s.
    /// The components make up the formula `y^2 mod p = x^3 + ax + b mod p`.
    #[corresponds(EC_GROUP_get_curve_GFp)]
    pub fn components_gfp(
        &self,
        p: &mut BigNumRef,
        a: &mut BigNumRef,
        b: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_GROUP_get_curve_GFp(
                self.as_ptr(),
                p.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the components of a curve over a binary field in the provided `BigNum`s.
    /// The components make up the formula `y^2 + xy = x^3 + ax^2 + b`.
    ///
    /// In this form `p` relates to the irreducible polynomial.  Each bit represents
    /// a term in the polynomial.  It will be set to 3 `1`s or 5 `1`s depending on
    /// using a trinomial or pentanomial.
    #[corresponds(EC_GROUP_get_curve_GF2m)]
    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn components_gf2m(
        &self,
        p: &mut BigNumRef,
        a: &mut BigNumRef,
        b: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_GROUP_get_curve_GF2m(
                self.as_ptr(),
                p.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the cofactor of the group in the provided `BigNum`.
    #[corresponds(EC_GROUP_get_cofactor)]
    pub fn cofactor(
        &self,
        cofactor: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_GROUP_get_cofactor(
                self.as_ptr(),
                cofactor.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Returns the degree of the curve.
    #[corresponds(EC_GROUP_get_degree)]
    pub fn degree(&self) -> u32 {
        unsafe { ffi::EC_GROUP_get_degree(self.as_ptr()) as u32 }
    }

    /// Returns the number of bits in the group order.
    #[corresponds(EC_GROUP_order_bits)]
    #[cfg(ossl110)]
    pub fn order_bits(&self) -> u32 {
        unsafe { ffi::EC_GROUP_order_bits(self.as_ptr()) as u32 }
    }

    /// Returns the generator for the given curve as an [`EcPoint`].
    #[corresponds(EC_GROUP_get0_generator)]
    pub fn generator(&self) -> &EcPointRef {
        unsafe {
            let ptr = ffi::EC_GROUP_get0_generator(self.as_ptr());
            EcPointRef::from_const_ptr(ptr)
        }
    }

    /// Sets the generator point for the given curve
    #[corresponds(EC_GROUP_set_generator)]
    pub fn set_generator(
        &mut self,
        generator: EcPoint,
        order: BigNum,
        cofactor: BigNum,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_GROUP_set_generator(
                self.as_ptr(),
                generator.as_ptr(),
                order.as_ptr(),
                cofactor.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places the order of the curve in the provided `BigNum`.
    #[corresponds(EC_GROUP_get_order)]
    pub fn order(
        &self,
        order: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_GROUP_get_order(
                self.as_ptr(),
                order.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Sets the flag determining if the group corresponds to a named curve or must be explicitly
    /// parameterized.
    ///
    /// This defaults to `EXPLICIT_CURVE` in OpenSSL 1.0.1 and 1.0.2, but `NAMED_CURVE` in OpenSSL
    /// 1.1.0.
    #[corresponds(EC_GROUP_set_asn1_flag)]
    pub fn set_asn1_flag(&mut self, flag: Asn1Flag) {
        unsafe {
            ffi::EC_GROUP_set_asn1_flag(self.as_ptr(), flag.0);
        }
    }

    /// Gets the flag determining if the group corresponds to a named curve.
    #[corresponds(EC_GROUP_get_asn1_flag)]
    pub fn asn1_flag(&self) -> Asn1Flag {
        unsafe { Asn1Flag(ffi::EC_GROUP_get_asn1_flag(self.as_ptr())) }
    }

    /// Returns the name of the curve, if a name is associated.
    #[corresponds(EC_GROUP_get_curve_name)]
    pub fn curve_name(&self) -> Option<Nid> {
        let nid = unsafe { ffi::EC_GROUP_get_curve_name(self.as_ptr()) };
        if nid > 0 {
            Some(Nid::from_raw(nid))
        } else {
            None
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::EC_POINT;
    fn drop = ffi::EC_POINT_free;

    /// Represents a point on the curve
    pub struct EcPoint;
    /// A reference a borrowed [`EcPoint`].
    pub struct EcPointRef;
}

impl EcPointRef {
    /// Computes `a + b`, storing the result in `self`.
    #[corresponds(EC_POINT_add)]
    pub fn add(
        &mut self,
        group: &EcGroupRef,
        a: &EcPointRef,
        b: &EcPointRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_add(
                group.as_ptr(),
                self.as_ptr(),
                a.as_ptr(),
                b.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Computes `q * m`, storing the result in `self`.
    #[corresponds(EC_POINT_mul)]
    pub fn mul(
        &mut self,
        group: &EcGroupRef,
        q: &EcPointRef,
        m: &BigNumRef,
        // FIXME should be &mut
        ctx: &BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_mul(
                group.as_ptr(),
                self.as_ptr(),
                ptr::null(),
                q.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Computes `generator * n`, storing the result in `self`.
    #[corresponds(EC_POINT_mul)]
    pub fn mul_generator(
        &mut self,
        group: &EcGroupRef,
        n: &BigNumRef,
        // FIXME should be &mut
        ctx: &BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_mul(
                group.as_ptr(),
                self.as_ptr(),
                n.as_ptr(),
                ptr::null(),
                ptr::null(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Computes `generator * n + q * m`, storing the result in `self`.
    #[corresponds(EC_POINT_mul)]
    pub fn mul_full(
        &mut self,
        group: &EcGroupRef,
        n: &BigNumRef,
        q: &EcPointRef,
        m: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_mul(
                group.as_ptr(),
                self.as_ptr(),
                n.as_ptr(),
                q.as_ptr(),
                m.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Inverts `self`.
    #[corresponds(EC_POINT_invert)]
    // FIXME should be mutable
    pub fn invert(&mut self, group: &EcGroupRef, ctx: &BigNumContextRef) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_invert(
                group.as_ptr(),
                self.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Serializes the point to a binary representation.
    #[corresponds(EC_POINT_point2oct)]
    pub fn to_bytes(
        &self,
        group: &EcGroupRef,
        form: PointConversionForm,
        ctx: &mut BigNumContextRef,
    ) -> Result<Vec<u8>, ErrorStack> {
        unsafe {
            let len = ffi::EC_POINT_point2oct(
                group.as_ptr(),
                self.as_ptr(),
                form.0,
                ptr::null_mut(),
                0,
                ctx.as_ptr(),
            );
            if len == 0 {
                return Err(ErrorStack::get());
            }
            let mut buf = vec![0; len];
            let len = ffi::EC_POINT_point2oct(
                group.as_ptr(),
                self.as_ptr(),
                form.0,
                buf.as_mut_ptr(),
                len,
                ctx.as_ptr(),
            );
            if len == 0 {
                Err(ErrorStack::get())
            } else {
                Ok(buf)
            }
        }
    }

    /// Serializes the point to a hexadecimal string representation.
    #[corresponds(EC_POINT_point2hex)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn to_hex_str(
        &self,
        group: &EcGroupRef,
        form: PointConversionForm,
        ctx: &mut BigNumContextRef,
    ) -> Result<OpensslString, ErrorStack> {
        unsafe {
            let buf = cvt_p(ffi::EC_POINT_point2hex(
                group.as_ptr(),
                self.as_ptr(),
                form.0,
                ctx.as_ptr(),
            ))?;
            Ok(OpensslString::from_ptr(buf))
        }
    }

    /// Creates a new point on the specified curve with the same value.
    #[corresponds(EC_POINT_dup)]
    pub fn to_owned(&self, group: &EcGroupRef) -> Result<EcPoint, ErrorStack> {
        unsafe { cvt_p(ffi::EC_POINT_dup(self.as_ptr(), group.as_ptr())).map(EcPoint) }
    }

    /// Determines if this point is equal to another.
    #[corresponds(EC_POINT_cmp)]
    pub fn eq(
        &self,
        group: &EcGroupRef,
        other: &EcPointRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<bool, ErrorStack> {
        unsafe {
            let res = cvt_n(ffi::EC_POINT_cmp(
                group.as_ptr(),
                self.as_ptr(),
                other.as_ptr(),
                ctx.as_ptr(),
            ))?;
            Ok(res == 0)
        }
    }

    /// Places affine coordinates of a curve over a prime field in the provided
    /// `x` and `y` `BigNum`s.
    #[corresponds(EC_POINT_get_affine_coordinates)]
    #[cfg(any(ossl111, boringssl, libressl350, awslc))]
    pub fn affine_coordinates(
        &self,
        group: &EcGroupRef,
        x: &mut BigNumRef,
        y: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_get_affine_coordinates(
                group.as_ptr(),
                self.as_ptr(),
                x.as_ptr(),
                y.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places affine coordinates of a curve over a prime field in the provided
    /// `x` and `y` `BigNum`s
    #[corresponds(EC_POINT_get_affine_coordinates_GFp)]
    pub fn affine_coordinates_gfp(
        &self,
        group: &EcGroupRef,
        x: &mut BigNumRef,
        y: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_get_affine_coordinates_GFp(
                group.as_ptr(),
                self.as_ptr(),
                x.as_ptr(),
                y.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Sets affine coordinates of a curve over a prime field using the provided
    /// `x` and `y` `BigNum`s
    #[corresponds(EC_POINT_set_affine_coordinates_GFp)]
    pub fn set_affine_coordinates_gfp(
        &mut self,
        group: &EcGroupRef,
        x: &BigNumRef,
        y: &BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_set_affine_coordinates_GFp(
                group.as_ptr(),
                self.as_ptr(),
                x.as_ptr(),
                y.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Places affine coordinates of a curve over a binary field in the provided
    /// `x` and `y` `BigNum`s
    #[corresponds(EC_POINT_get_affine_coordinates_GF2m)]
    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    pub fn affine_coordinates_gf2m(
        &self,
        group: &EcGroupRef,
        x: &mut BigNumRef,
        y: &mut BigNumRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::EC_POINT_get_affine_coordinates_GF2m(
                group.as_ptr(),
                self.as_ptr(),
                x.as_ptr(),
                y.as_ptr(),
                ctx.as_ptr(),
            ))
            .map(|_| ())
        }
    }

    /// Checks if point is infinity
    #[corresponds(EC_POINT_is_at_infinity)]
    pub fn is_infinity(&self, group: &EcGroupRef) -> bool {
        unsafe {
            let res = ffi::EC_POINT_is_at_infinity(group.as_ptr(), self.as_ptr());
            res == 1
        }
    }

    /// Checks if point is on a given curve
    #[corresponds(EC_POINT_is_on_curve)]
    pub fn is_on_curve(
        &self,
        group: &EcGroupRef,
        ctx: &mut BigNumContextRef,
    ) -> Result<bool, ErrorStack> {
        unsafe {
            let res = cvt_n(ffi::EC_POINT_is_on_curve(
                group.as_ptr(),
                self.as_ptr(),
                ctx.as_ptr(),
            ))?;
            Ok(res == 1)
        }
    }
}

impl EcPoint {
    /// Creates a new point on the specified curve.
    #[corresponds(EC_POINT_new)]
    pub fn new(group: &EcGroupRef) -> Result<EcPoint, ErrorStack> {
        unsafe { cvt_p(ffi::EC_POINT_new(group.as_ptr())).map(EcPoint) }
    }

    /// Creates point from a binary representation
    #[corresponds(EC_POINT_oct2point)]
    pub fn from_bytes(
        group: &EcGroupRef,
        buf: &[u8],
        ctx: &mut BigNumContextRef,
    ) -> Result<EcPoint, ErrorStack> {
        let point = EcPoint::new(group)?;
        unsafe {
            cvt(ffi::EC_POINT_oct2point(
                group.as_ptr(),
                point.as_ptr(),
                buf.as_ptr(),
                buf.len(),
                ctx.as_ptr(),
            ))?;
        }
        Ok(point)
    }

    /// Creates point from a hexadecimal string representation
    #[corresponds(EC_POINT_hex2point)]
    #[cfg(not(any(boringssl, awslc)))]
    pub fn from_hex_str(
        group: &EcGroupRef,
        s: &str,
        ctx: &mut BigNumContextRef,
    ) -> Result<EcPoint, ErrorStack> {
        let point = EcPoint::new(group)?;
        unsafe {
            let c_str = CString::new(s.as_bytes()).unwrap();
            cvt_p(ffi::EC_POINT_hex2point(
                group.as_ptr(),
                c_str.as_ptr() as *const _,
                point.as_ptr(),
                ctx.as_ptr(),
            ))?;
        }
        Ok(point)
    }
}

generic_foreign_type_and_impl_send_sync! {
    type CType = ffi::EC_KEY;
    fn drop = ffi::EC_KEY_free;

    /// Public and optional private key on the given curve.
    pub struct EcKey<T>;
    /// A reference to an [`EcKey`].
    pub struct EcKeyRef<T>;
}

impl<T> EcKeyRef<T>
where
    T: HasPrivate,
{
    private_key_to_pem! {
        /// Serializes the private key to a PEM-encoded ECPrivateKey structure.
        ///
        /// The output will have a header of `-----BEGIN EC PRIVATE KEY-----`.
        #[corresponds(PEM_write_bio_ECPrivateKey)]
        private_key_to_pem,
        /// Serializes the private key to a PEM-encoded encrypted ECPrivateKey structure.
        ///
        /// The output will have a header of `-----BEGIN EC PRIVATE KEY-----`.
        #[corresponds(PEM_write_bio_ECPrivateKey)]
        private_key_to_pem_passphrase,
        ffi::PEM_write_bio_ECPrivateKey
    }

    to_der! {
        /// Serializes the private key into a DER-encoded ECPrivateKey structure.
        #[corresponds(i2d_ECPrivateKey)]
        private_key_to_der,
        ffi::i2d_ECPrivateKey
    }

    /// Returns the private key value.
    #[corresponds(EC_KEY_get0_private_key)]
    pub fn private_key(&self) -> &BigNumRef {
        unsafe {
            let ptr = ffi::EC_KEY_get0_private_key(self.as_ptr());
            BigNumRef::from_const_ptr(ptr)
        }
    }
}

impl<T> EcKeyRef<T>
where
    T: HasPublic,
{
    /// Returns the public key.
    #[corresponds(EC_KEY_get0_public_key)]
    pub fn public_key(&self) -> &EcPointRef {
        unsafe {
            let ptr = ffi::EC_KEY_get0_public_key(self.as_ptr());
            EcPointRef::from_const_ptr(ptr)
        }
    }

    to_pem! {
        /// Serializes the public key into a PEM-encoded SubjectPublicKeyInfo structure.
        ///
        /// The output will have a header of `-----BEGIN PUBLIC KEY-----`.
        #[corresponds(PEM_write_bio_EC_PUBKEY)]
        public_key_to_pem,
        ffi::PEM_write_bio_EC_PUBKEY
    }

    to_der! {
        /// Serializes the public key into a DER-encoded SubjectPublicKeyInfo structure.
        #[corresponds(i2d_EC_PUBKEY)]
        public_key_to_der,
        ffi::i2d_EC_PUBKEY
    }
}

impl<T> EcKeyRef<T>
where
    T: HasParams,
{
    /// Returns the key's group.
    #[corresponds(EC_KEY_get0_group)]
    pub fn group(&self) -> &EcGroupRef {
        unsafe {
            let ptr = ffi::EC_KEY_get0_group(self.as_ptr());
            EcGroupRef::from_const_ptr(ptr)
        }
    }

    /// Checks the key for validity.
    #[corresponds(EC_KEY_check_key)]
    pub fn check_key(&self) -> Result<(), ErrorStack> {
        unsafe { cvt(ffi::EC_KEY_check_key(self.as_ptr())).map(|_| ()) }
    }
}

impl<T> ToOwned for EcKeyRef<T> {
    type Owned = EcKey<T>;

    fn to_owned(&self) -> EcKey<T> {
        unsafe {
            let r = ffi::EC_KEY_up_ref(self.as_ptr());
            assert!(r == 1);
            EcKey::from_ptr(self.as_ptr())
        }
    }
}

impl EcKey<Params> {
    /// Constructs an `EcKey` corresponding to a known curve.
    ///
    /// It will not have an associated public or private key. This kind of key is primarily useful
    /// to be provided to the `set_tmp_ecdh` methods on `Ssl` and `SslContextBuilder`.
    #[corresponds(EC_KEY_new_by_curve_name)]
    pub fn from_curve_name(nid: Nid) -> Result<EcKey<Params>, ErrorStack> {
        unsafe {
            init();
            cvt_p(ffi::EC_KEY_new_by_curve_name(nid.as_raw())).map(|p| EcKey::from_ptr(p))
        }
    }

    /// Constructs an `EcKey` corresponding to a curve.
    #[corresponds(EC_KEY_set_group)]
    pub fn from_group(group: &EcGroupRef) -> Result<EcKey<Params>, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_KEY_new())
                .map(|p| EcKey::from_ptr(p))
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_group(key.as_ptr(), group.as_ptr())).map(|_| key)
                })
        }
    }
}

impl EcKey<Public> {
    /// Constructs an `EcKey` from the specified group with the associated [`EcPoint`]: `public_key`.
    ///
    /// This will only have the associated `public_key`.
    ///
    /// # Example
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::bn::BigNumContext;
    /// use openssl::ec::*;
    /// use openssl::nid::Nid;
    /// use openssl::pkey::PKey;
    ///
    /// let group = EcGroup::from_curve_name(Nid::SECP384R1)?;
    /// let mut ctx = BigNumContext::new()?;
    ///
    /// // get bytes from somewhere
    /// let public_key = // ...
    /// # EcKey::generate(&group)?.public_key().to_bytes(&group,
    /// # PointConversionForm::COMPRESSED, &mut ctx)?;
    ///
    /// // create an EcKey from the binary form of a EcPoint
    /// let point = EcPoint::from_bytes(&group, &public_key, &mut ctx)?;
    /// let key = EcKey::from_public_key(&group, &point)?;
    /// key.check_key()?;
    /// # Ok(()) }
    /// ```
    #[corresponds(EC_KEY_set_public_key)]
    pub fn from_public_key(
        group: &EcGroupRef,
        public_key: &EcPointRef,
    ) -> Result<EcKey<Public>, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_KEY_new())
                .map(|p| EcKey::from_ptr(p))
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_group(key.as_ptr(), group.as_ptr())).map(|_| key)
                })
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_public_key(
                        key.as_ptr(),
                        public_key.as_ptr(),
                    ))
                    .map(|_| key)
                })
        }
    }

    /// Constructs a public key from its affine coordinates.
    #[corresponds(EC_KEY_set_public_key_affine_coordinates)]
    pub fn from_public_key_affine_coordinates(
        group: &EcGroupRef,
        x: &BigNumRef,
        y: &BigNumRef,
    ) -> Result<EcKey<Public>, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_KEY_new())
                .map(|p| EcKey::from_ptr(p))
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_group(key.as_ptr(), group.as_ptr())).map(|_| key)
                })
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_public_key_affine_coordinates(
                        key.as_ptr(),
                        x.as_ptr(),
                        y.as_ptr(),
                    ))
                    .map(|_| key)
                })
        }
    }

    from_pem! {
        /// Decodes a PEM-encoded SubjectPublicKeyInfo structure containing a EC key.
        ///
        /// The input should have a header of `-----BEGIN PUBLIC KEY-----`.
        #[corresponds(PEM_read_bio_EC_PUBKEY)]
        public_key_from_pem,
        EcKey<Public>,
        ffi::PEM_read_bio_EC_PUBKEY
    }

    from_der! {
        /// Decodes a DER-encoded SubjectPublicKeyInfo structure containing a EC key.
        #[corresponds(d2i_EC_PUBKEY)]
        public_key_from_der,
        EcKey<Public>,
        ffi::d2i_EC_PUBKEY
    }
}

impl EcKey<Private> {
    /// Generates a new public/private key pair on the specified curve.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use openssl::bn::BigNumContext;
    /// use openssl::nid::Nid;
    /// use openssl::ec::{EcGroup, EcKey, PointConversionForm};
    ///
    /// let nid = Nid::X9_62_PRIME256V1; // NIST P-256 curve
    /// let group = EcGroup::from_curve_name(nid)?;
    /// let key = EcKey::generate(&group)?;
    ///
    /// let mut ctx = BigNumContext::new()?;
    ///
    /// let public_key = &key.public_key().to_bytes(
    ///     &group,
    ///     PointConversionForm::COMPRESSED,
    ///     &mut ctx,
    /// )?;
    /// assert_eq!(public_key.len(), 33);
    /// assert_ne!(public_key[0], 0x04);
    ///
    /// let private_key = key.private_key().to_vec();
    /// assert!(private_key.len() >= 31);
    /// # Ok(()) }
    /// ```
    #[corresponds(EC_KEY_generate_key)]
    pub fn generate(group: &EcGroupRef) -> Result<EcKey<Private>, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_KEY_new())
                .map(|p| EcKey::from_ptr(p))
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_group(key.as_ptr(), group.as_ptr())).map(|_| key)
                })
                .and_then(|key| cvt(ffi::EC_KEY_generate_key(key.as_ptr())).map(|_| key))
        }
    }

    /// Constructs an public/private key pair given a curve, a private key and a public key point.
    #[corresponds(EC_KEY_set_private_key)]
    pub fn from_private_components(
        group: &EcGroupRef,
        private_number: &BigNumRef,
        public_key: &EcPointRef,
    ) -> Result<EcKey<Private>, ErrorStack> {
        unsafe {
            cvt_p(ffi::EC_KEY_new())
                .map(|p| EcKey::from_ptr(p))
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_group(key.as_ptr(), group.as_ptr())).map(|_| key)
                })
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_private_key(
                        key.as_ptr(),
                        private_number.as_ptr(),
                    ))
                    .map(|_| key)
                })
                .and_then(|key| {
                    cvt(ffi::EC_KEY_set_public_key(
                        key.as_ptr(),
                        public_key.as_ptr(),
                    ))
                    .map(|_| key)
                })
        }
    }

    private_key_from_pem! {
        /// Deserializes a private key from a PEM-encoded ECPrivateKey structure.
        ///
        /// The input should have a header of `-----BEGIN EC PRIVATE KEY-----`.
        #[corresponds(PEM_read_bio_ECPrivateKey)]
        private_key_from_pem,

        /// Deserializes a private key from a PEM-encoded encrypted ECPrivateKey structure.
        ///
        /// The input should have a header of `-----BEGIN EC PRIVATE KEY-----`.
        #[corresponds(PEM_read_bio_ECPrivateKey)]
        private_key_from_pem_passphrase,

        /// Deserializes a private key from a PEM-encoded encrypted ECPrivateKey structure.
        ///
        /// The callback should fill the password into the provided buffer and return its length.
        ///
        /// The input should have a header of `-----BEGIN EC PRIVATE KEY-----`.
        #[corresponds(PEM_read_bio_ECPrivateKey)]
        private_key_from_pem_callback,
        EcKey<Private>,
        ffi::PEM_read_bio_ECPrivateKey
    }

    from_der! {
        /// Decodes a DER-encoded elliptic curve private key structure.
        #[corresponds(d2i_ECPrivateKey)]
        private_key_from_der,
        EcKey<Private>,
        ffi::d2i_ECPrivateKey
    }
}

impl<T> Clone for EcKey<T> {
    fn clone(&self) -> EcKey<T> {
        (**self).to_owned()
    }
}

impl<T> fmt::Debug for EcKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EcKey")
    }
}

#[cfg(test)]
mod test {
    use hex::FromHex;

    use super::*;
    use crate::bn::{BigNum, BigNumContext};
    use crate::nid::Nid;

    #[test]
    fn key_new_by_curve_name() {
        EcKey::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    }

    #[test]
    fn generate() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        EcKey::generate(&group).unwrap();
    }

    #[test]
    fn ec_group_from_components() {
        // parameters are from secp256r1
        let p = BigNum::from_hex_str(
            "FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF",
        )
        .unwrap();
        let a = BigNum::from_hex_str(
            "FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFC",
        )
        .unwrap();
        let b = BigNum::from_hex_str(
            "5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B",
        )
        .unwrap();
        let mut ctx = BigNumContext::new().unwrap();

        let _curve = EcGroup::from_components(p, a, b, &mut ctx).unwrap();
    }

    #[test]
    fn ec_point_set_affine() {
        // parameters are from secp256r1
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let mut gen_point = EcPoint::new(&group).unwrap();
        let gen_x = BigNum::from_hex_str(
            "6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296",
        )
        .unwrap();
        let gen_y = BigNum::from_hex_str(
            "4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5",
        )
        .unwrap();
        gen_point
            .set_affine_coordinates_gfp(&group, &gen_x, &gen_y, &mut ctx)
            .unwrap();
        assert!(gen_point.is_on_curve(&group, &mut ctx).unwrap());
    }

    #[test]
    fn ec_group_set_generator() {
        // parameters are from secp256r1
        let mut ctx = BigNumContext::new().unwrap();
        let p = BigNum::from_hex_str(
            "FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFF",
        )
        .unwrap();
        let a = BigNum::from_hex_str(
            "FFFFFFFF00000001000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFC",
        )
        .unwrap();
        let b = BigNum::from_hex_str(
            "5AC635D8AA3A93E7B3EBBD55769886BC651D06B0CC53B0F63BCE3C3E27D2604B",
        )
        .unwrap();

        let mut group = EcGroup::from_components(p, a, b, &mut ctx).unwrap();

        let mut gen_point = EcPoint::new(&group).unwrap();
        let gen_x = BigNum::from_hex_str(
            "6B17D1F2E12C4247F8BCE6E563A440F277037D812DEB33A0F4A13945D898C296",
        )
        .unwrap();
        let gen_y = BigNum::from_hex_str(
            "4FE342E2FE1A7F9B8EE7EB4A7C0F9E162BCE33576B315ECECBB6406837BF51F5",
        )
        .unwrap();
        gen_point
            .set_affine_coordinates_gfp(&group, &gen_x, &gen_y, &mut ctx)
            .unwrap();

        let order = BigNum::from_hex_str(
            "FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551",
        )
        .unwrap();
        let cofactor = BigNum::from_hex_str("01").unwrap();
        group.set_generator(gen_point, order, cofactor).unwrap();
        let mut constructed_order = BigNum::new().unwrap();
        group.order(&mut constructed_order, &mut ctx).unwrap();

        let named_group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut named_order = BigNum::new().unwrap();
        named_group.order(&mut named_order, &mut ctx).unwrap();

        assert_eq!(
            constructed_order.ucmp(&named_order),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn cofactor() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let mut cofactor = BigNum::new().unwrap();
        group.cofactor(&mut cofactor, &mut ctx).unwrap();
        let one = BigNum::from_u32(1).unwrap();
        assert_eq!(cofactor, one);
    }

    #[test]
    #[allow(clippy::redundant_clone)]
    fn dup() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        drop(key.clone());
    }

    #[test]
    fn point_new() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        EcPoint::new(&group).unwrap();
    }

    #[test]
    fn point_bytes() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let point = key.public_key();
        let mut ctx = BigNumContext::new().unwrap();
        let bytes = point
            .to_bytes(&group, PointConversionForm::COMPRESSED, &mut ctx)
            .unwrap();
        let point2 = EcPoint::from_bytes(&group, &bytes, &mut ctx).unwrap();
        assert!(point.eq(&group, &point2, &mut ctx).unwrap());
    }

    #[test]
    #[cfg(not(any(boringssl, awslc)))]
    fn point_hex_str() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let point = key.public_key();
        let mut ctx = BigNumContext::new().unwrap();
        let hex = point
            .to_hex_str(&group, PointConversionForm::COMPRESSED, &mut ctx)
            .unwrap();
        let point2 = EcPoint::from_hex_str(&group, &hex, &mut ctx).unwrap();
        assert!(point.eq(&group, &point2, &mut ctx).unwrap());
    }

    #[test]
    fn point_owned() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let point = key.public_key();
        let owned = point.to_owned(&group).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        assert!(owned.eq(&group, point, &mut ctx).unwrap());
    }

    #[test]
    fn mul_generator() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let mut public_key = EcPoint::new(&group).unwrap();
        public_key
            .mul_generator(&group, key.private_key(), &ctx)
            .unwrap();
        assert!(public_key.eq(&group, key.public_key(), &mut ctx).unwrap());
    }

    #[test]
    fn generator() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let gen = group.generator();
        let one = BigNum::from_u32(1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let mut ecp = EcPoint::new(&group).unwrap();
        ecp.mul_generator(&group, &one, &ctx).unwrap();
        assert!(ecp.eq(&group, gen, &mut ctx).unwrap());
    }

    #[test]
    fn key_from_public_key() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let bytes = key
            .public_key()
            .to_bytes(&group, PointConversionForm::COMPRESSED, &mut ctx)
            .unwrap();

        drop(key);
        let public_key = EcPoint::from_bytes(&group, &bytes, &mut ctx).unwrap();
        let ec_key = EcKey::from_public_key(&group, &public_key).unwrap();
        assert!(ec_key.check_key().is_ok());
    }

    #[test]
    fn key_from_private_components() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let key = EcKey::generate(&group).unwrap();

        let dup_key =
            EcKey::from_private_components(&group, key.private_key(), key.public_key()).unwrap();
        dup_key.check_key().unwrap();

        assert!(key.private_key() == dup_key.private_key());
    }

    #[test]
    fn key_from_affine_coordinates() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let x = Vec::from_hex("30a0424cd21c2944838a2d75c92b37e76ea20d9f00893a3b4eee8a3c0aafec3e")
            .unwrap();
        let y = Vec::from_hex("e04b65e92456d9888b52b379bdfbd51ee869ef1f0fc65b6659695b6cce081723")
            .unwrap();

        let xbn = BigNum::from_slice(&x).unwrap();
        let ybn = BigNum::from_slice(&y).unwrap();

        let ec_key = EcKey::from_public_key_affine_coordinates(&group, &xbn, &ybn).unwrap();
        assert!(ec_key.check_key().is_ok());
    }

    #[cfg(any(ossl111, boringssl, libressl350, awslc))]
    #[test]
    fn get_affine_coordinates() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let x = Vec::from_hex("30a0424cd21c2944838a2d75c92b37e76ea20d9f00893a3b4eee8a3c0aafec3e")
            .unwrap();
        let y = Vec::from_hex("e04b65e92456d9888b52b379bdfbd51ee869ef1f0fc65b6659695b6cce081723")
            .unwrap();

        let xbn = BigNum::from_slice(&x).unwrap();
        let ybn = BigNum::from_slice(&y).unwrap();

        let ec_key = EcKey::from_public_key_affine_coordinates(&group, &xbn, &ybn).unwrap();

        let mut xbn2 = BigNum::new().unwrap();
        let mut ybn2 = BigNum::new().unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let ec_key_pk = ec_key.public_key();
        ec_key_pk
            .affine_coordinates(&group, &mut xbn2, &mut ybn2, &mut ctx)
            .unwrap();
        assert_eq!(xbn2, xbn);
        assert_eq!(ybn2, ybn);
    }

    #[test]
    fn get_affine_coordinates_gfp() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let x = Vec::from_hex("30a0424cd21c2944838a2d75c92b37e76ea20d9f00893a3b4eee8a3c0aafec3e")
            .unwrap();
        let y = Vec::from_hex("e04b65e92456d9888b52b379bdfbd51ee869ef1f0fc65b6659695b6cce081723")
            .unwrap();

        let xbn = BigNum::from_slice(&x).unwrap();
        let ybn = BigNum::from_slice(&y).unwrap();

        let ec_key = EcKey::from_public_key_affine_coordinates(&group, &xbn, &ybn).unwrap();

        let mut xbn2 = BigNum::new().unwrap();
        let mut ybn2 = BigNum::new().unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let ec_key_pk = ec_key.public_key();
        ec_key_pk
            .affine_coordinates_gfp(&group, &mut xbn2, &mut ybn2, &mut ctx)
            .unwrap();
        assert_eq!(xbn2, xbn);
        assert_eq!(ybn2, ybn);
    }

    #[test]
    fn is_infinity() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let g = group.generator();
        assert!(!g.is_infinity(&group));

        let mut order = BigNum::new().unwrap();
        group.order(&mut order, &mut ctx).unwrap();
        let mut inf = EcPoint::new(&group).unwrap();
        inf.mul_generator(&group, &order, &ctx).unwrap();
        assert!(inf.is_infinity(&group));
    }

    #[test]
    #[cfg(not(osslconf = "OPENSSL_NO_EC2M"))]
    fn is_on_curve() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let g = group.generator();
        assert!(g.is_on_curve(&group, &mut ctx).unwrap());

        let group2 = EcGroup::from_curve_name(Nid::X9_62_PRIME239V3).unwrap();
        assert!(!g.is_on_curve(&group2, &mut ctx).unwrap());
    }

    #[test]
    #[cfg(any(boringssl, ossl111, libressl350, awslc))]
    fn asn1_flag() {
        let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
        let flag = group.asn1_flag();
        assert_eq!(flag, Asn1Flag::NAMED_CURVE);
    }
}
