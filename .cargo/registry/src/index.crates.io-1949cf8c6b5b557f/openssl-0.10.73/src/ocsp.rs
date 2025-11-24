use bitflags::bitflags;
use foreign_types::ForeignTypeRef;
use libc::{c_int, c_long, c_ulong};
use std::mem;
use std::ptr;

use crate::asn1::Asn1GeneralizedTimeRef;
use crate::error::ErrorStack;
use crate::hash::MessageDigest;
use crate::stack::StackRef;
use crate::util::ForeignTypeRefExt;
use crate::x509::store::X509StoreRef;
use crate::x509::{X509Ref, X509};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct OcspFlag: c_ulong {
        const NO_CERTS = ffi::OCSP_NOCERTS as c_ulong;
        const NO_INTERN = ffi::OCSP_NOINTERN as c_ulong;
        const NO_CHAIN = ffi::OCSP_NOCHAIN as c_ulong;
        const NO_VERIFY = ffi::OCSP_NOVERIFY as c_ulong;
        const NO_EXPLICIT = ffi::OCSP_NOEXPLICIT as c_ulong;
        const NO_CA_SIGN = ffi::OCSP_NOCASIGN as c_ulong;
        const NO_DELEGATED = ffi::OCSP_NODELEGATED as c_ulong;
        const NO_CHECKS = ffi::OCSP_NOCHECKS as c_ulong;
        const TRUST_OTHER = ffi::OCSP_TRUSTOTHER as c_ulong;
        const RESPID_KEY = ffi::OCSP_RESPID_KEY as c_ulong;
        const NO_TIME = ffi::OCSP_NOTIME as c_ulong;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OcspResponseStatus(c_int);

impl OcspResponseStatus {
    pub const SUCCESSFUL: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_SUCCESSFUL);
    pub const MALFORMED_REQUEST: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_MALFORMEDREQUEST);
    pub const INTERNAL_ERROR: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_INTERNALERROR);
    pub const TRY_LATER: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_TRYLATER);
    pub const SIG_REQUIRED: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_SIGREQUIRED);
    pub const UNAUTHORIZED: OcspResponseStatus =
        OcspResponseStatus(ffi::OCSP_RESPONSE_STATUS_UNAUTHORIZED);

    pub fn from_raw(raw: c_int) -> OcspResponseStatus {
        OcspResponseStatus(raw)
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OcspCertStatus(c_int);

impl OcspCertStatus {
    pub const GOOD: OcspCertStatus = OcspCertStatus(ffi::V_OCSP_CERTSTATUS_GOOD);
    pub const REVOKED: OcspCertStatus = OcspCertStatus(ffi::V_OCSP_CERTSTATUS_REVOKED);
    pub const UNKNOWN: OcspCertStatus = OcspCertStatus(ffi::V_OCSP_CERTSTATUS_UNKNOWN);

    pub fn from_raw(raw: c_int) -> OcspCertStatus {
        OcspCertStatus(raw)
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OcspRevokedStatus(c_int);

impl OcspRevokedStatus {
    pub const NO_STATUS: OcspRevokedStatus = OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_NOSTATUS);
    pub const UNSPECIFIED: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_UNSPECIFIED);
    pub const KEY_COMPROMISE: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_KEYCOMPROMISE);
    pub const CA_COMPROMISE: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_CACOMPROMISE);
    pub const AFFILIATION_CHANGED: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_AFFILIATIONCHANGED);
    pub const STATUS_SUPERSEDED: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_SUPERSEDED);
    pub const STATUS_CESSATION_OF_OPERATION: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_CESSATIONOFOPERATION);
    pub const STATUS_CERTIFICATE_HOLD: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_CERTIFICATEHOLD);
    pub const REMOVE_FROM_CRL: OcspRevokedStatus =
        OcspRevokedStatus(ffi::OCSP_REVOKED_STATUS_REMOVEFROMCRL);

    pub fn from_raw(raw: c_int) -> OcspRevokedStatus {
        OcspRevokedStatus(raw)
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn as_raw(&self) -> c_int {
        self.0
    }
}

pub struct OcspStatus<'a> {
    /// The overall status of the response.
    pub status: OcspCertStatus,
    /// If `status` is `CERT_STATUS_REVOKED`, the reason for the revocation.
    pub reason: OcspRevokedStatus,
    /// If `status` is `CERT_STATUS_REVOKED`, the time at which the certificate was revoked.
    pub revocation_time: Option<&'a Asn1GeneralizedTimeRef>,
    /// The time that this revocation check was performed.
    pub this_update: &'a Asn1GeneralizedTimeRef,
    /// The time at which this revocation check expires.
    pub next_update: &'a Asn1GeneralizedTimeRef,
}

impl OcspStatus<'_> {
    /// Checks validity of the `this_update` and `next_update` fields.
    ///
    /// The `nsec` parameter specifies an amount of slack time that will be used when comparing
    /// those times with the current time to account for delays and clock skew.
    ///
    /// The `maxsec` parameter limits the maximum age of the `this_update` parameter to prohibit
    /// very old responses.
    #[corresponds(OCSP_check_validity)]
    pub fn check_validity(&self, nsec: u32, maxsec: Option<u32>) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::OCSP_check_validity(
                self.this_update.as_ptr(),
                self.next_update.as_ptr(),
                nsec as c_long,
                maxsec.map(|n| n as c_long).unwrap_or(-1),
            ))
            .map(|_| ())
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::OCSP_BASICRESP;
    fn drop = ffi::OCSP_BASICRESP_free;

    pub struct OcspBasicResponse;
    pub struct OcspBasicResponseRef;
}

impl OcspBasicResponseRef {
    /// Verifies the validity of the response.
    ///
    /// The `certs` parameter contains a set of certificates that will be searched when locating the
    /// OCSP response signing certificate. Some responders do not include this in the response.
    #[corresponds(OCSP_basic_verify)]
    pub fn verify(
        &self,
        certs: &StackRef<X509>,
        store: &X509StoreRef,
        flags: OcspFlag,
    ) -> Result<(), ErrorStack> {
        unsafe {
            cvt(ffi::OCSP_basic_verify(
                self.as_ptr(),
                certs.as_ptr(),
                store.as_ptr(),
                flags.bits(),
            ))
            .map(|_| ())
        }
    }

    /// Looks up the status for the specified certificate ID.
    #[corresponds(OCSP_resp_find_status)]
    pub fn find_status<'a>(&'a self, id: &OcspCertIdRef) -> Option<OcspStatus<'a>> {
        unsafe {
            let mut status = ffi::V_OCSP_CERTSTATUS_UNKNOWN;
            let mut reason = ffi::OCSP_REVOKED_STATUS_NOSTATUS;
            let mut revocation_time = ptr::null_mut();
            let mut this_update = ptr::null_mut();
            let mut next_update = ptr::null_mut();

            let r = ffi::OCSP_resp_find_status(
                self.as_ptr(),
                id.as_ptr(),
                &mut status,
                &mut reason,
                &mut revocation_time,
                &mut this_update,
                &mut next_update,
            );
            if r == 1 {
                let revocation_time = Asn1GeneralizedTimeRef::from_const_ptr_opt(revocation_time);

                Some(OcspStatus {
                    status: OcspCertStatus(status),
                    reason: OcspRevokedStatus(status),
                    revocation_time,
                    this_update: Asn1GeneralizedTimeRef::from_ptr(this_update),
                    next_update: Asn1GeneralizedTimeRef::from_ptr(next_update),
                })
            } else {
                None
            }
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::OCSP_CERTID;
    fn drop = ffi::OCSP_CERTID_free;

    pub struct OcspCertId;
    pub struct OcspCertIdRef;
}

impl OcspCertId {
    /// Constructs a certificate ID for certificate `subject`.
    #[corresponds(OCSP_cert_to_id)]
    pub fn from_cert(
        digest: MessageDigest,
        subject: &X509Ref,
        issuer: &X509Ref,
    ) -> Result<OcspCertId, ErrorStack> {
        unsafe {
            cvt_p(ffi::OCSP_cert_to_id(
                digest.as_ptr(),
                subject.as_ptr(),
                issuer.as_ptr(),
            ))
            .map(OcspCertId)
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::OCSP_RESPONSE;
    fn drop = ffi::OCSP_RESPONSE_free;

    pub struct OcspResponse;
    pub struct OcspResponseRef;
}

impl OcspResponse {
    /// Creates an OCSP response from the status and optional body.
    ///
    /// A body should only be provided if `status` is `RESPONSE_STATUS_SUCCESSFUL`.
    #[corresponds(OCSP_response_create)]
    pub fn create(
        status: OcspResponseStatus,
        body: Option<&OcspBasicResponseRef>,
    ) -> Result<OcspResponse, ErrorStack> {
        unsafe {
            ffi::init();

            cvt_p(ffi::OCSP_response_create(
                status.as_raw(),
                body.map(|r| r.as_ptr()).unwrap_or(ptr::null_mut()),
            ))
            .map(OcspResponse)
        }
    }

    from_der! {
        /// Deserializes a DER-encoded OCSP response.
        #[corresponds(d2i_OCSP_RESPONSE)]
        from_der,
        OcspResponse,
        ffi::d2i_OCSP_RESPONSE
    }
}

impl OcspResponseRef {
    to_der! {
        /// Serializes the response to its standard DER encoding.
        #[corresponds(i2d_OCSP_RESPONSE)]
        to_der,
        ffi::i2d_OCSP_RESPONSE
    }

    /// Returns the status of the response.
    #[corresponds(OCSP_response_status)]
    pub fn status(&self) -> OcspResponseStatus {
        unsafe { OcspResponseStatus(ffi::OCSP_response_status(self.as_ptr())) }
    }

    /// Returns the basic response.
    ///
    /// This will only succeed if `status()` returns `RESPONSE_STATUS_SUCCESSFUL`.
    #[corresponds(OCSP_response_get1_basic)]
    pub fn basic(&self) -> Result<OcspBasicResponse, ErrorStack> {
        unsafe { cvt_p(ffi::OCSP_response_get1_basic(self.as_ptr())).map(OcspBasicResponse) }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::OCSP_REQUEST;
    fn drop = ffi::OCSP_REQUEST_free;

    pub struct OcspRequest;
    pub struct OcspRequestRef;
}

impl OcspRequest {
    #[corresponds(OCSP_REQUEST_new)]
    pub fn new() -> Result<OcspRequest, ErrorStack> {
        unsafe {
            ffi::init();

            cvt_p(ffi::OCSP_REQUEST_new()).map(OcspRequest)
        }
    }

    from_der! {
        /// Deserializes a DER-encoded OCSP request.
        #[corresponds(d2i_OCSP_REQUEST)]
        from_der,
        OcspRequest,
        ffi::d2i_OCSP_REQUEST
    }
}

impl OcspRequestRef {
    to_der! {
        /// Serializes the request to its standard DER encoding.
        #[corresponds(i2d_OCSP_REQUEST)]
        to_der,
        ffi::i2d_OCSP_REQUEST
    }

    #[corresponds(OCSP_request_add0_id)]
    pub fn add_id(&mut self, id: OcspCertId) -> Result<&mut OcspOneReqRef, ErrorStack> {
        unsafe {
            let ptr = cvt_p(ffi::OCSP_request_add0_id(self.as_ptr(), id.as_ptr()))?;
            mem::forget(id);
            Ok(OcspOneReqRef::from_ptr_mut(ptr))
        }
    }
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::OCSP_ONEREQ;
    fn drop = ffi::OCSP_ONEREQ_free;

    pub struct OcspOneReq;
    pub struct OcspOneReqRef;
}
