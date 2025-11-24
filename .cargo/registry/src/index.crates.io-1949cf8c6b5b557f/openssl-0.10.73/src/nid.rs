//! A collection of numerical identifiers for OpenSSL objects.
use libc::{c_char, c_int};

use std::ffi::CStr;
use std::ffi::CString;
use std::str;

use crate::cvt_p;
use crate::error::ErrorStack;
use openssl_macros::corresponds;

/// The digest and public-key algorithms associated with a signature.
pub struct SignatureAlgorithms {
    /// The signature's digest.
    ///
    /// If the signature does not specify a digest, this will be `NID::UNDEF`.
    pub digest: Nid,

    /// The signature's public-key.
    pub pkey: Nid,
}

/// A numerical identifier for an OpenSSL object.
///
/// Objects in OpenSSL can have a short name, a long name, and
/// a numerical identifier (NID). For convenience, objects
/// are usually represented in source code using these numeric
/// identifiers.
///
/// Users should generally not need to create new `Nid`s.
///
/// # Examples
///
/// To view the integer representation of a `Nid`:
///
/// ```
/// use openssl::nid::Nid;
///
/// assert!(Nid::AES_256_GCM.as_raw() == 901);
/// ```
///
/// # External Documentation
///
/// The following documentation provides context about `Nid`s and their usage
/// in OpenSSL.
///
/// - [Obj_nid2obj](https://www.openssl.org/docs/manmaster/crypto/OBJ_create.html)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Nid(c_int);

#[allow(non_snake_case)]
impl Nid {
    /// Create a `Nid` from an integer representation.
    pub const fn from_raw(raw: c_int) -> Nid {
        Nid(raw)
    }

    /// Return the integer representation of a `Nid`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub const fn as_raw(&self) -> c_int {
        self.0
    }

    /// Creates a new `Nid` for the `oid` with short name `sn` and long name `ln`.
    #[corresponds(OBJ_create)]
    pub fn create(oid: &str, sn: &str, ln: &str) -> Result<Nid, ErrorStack> {
        unsafe {
            ffi::init();
            let oid = CString::new(oid).unwrap();
            let sn = CString::new(sn).unwrap();
            let ln = CString::new(ln).unwrap();
            let raw = ffi::OBJ_create(oid.as_ptr(), sn.as_ptr(), ln.as_ptr());
            if raw == ffi::NID_undef {
                Err(ErrorStack::get())
            } else {
                Ok(Nid(raw))
            }
        }
    }

    /// Returns the `Nid`s of the digest and public key algorithms associated with a signature ID.
    #[corresponds(OBJ_find_sigid_algs)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn signature_algorithms(&self) -> Option<SignatureAlgorithms> {
        unsafe {
            let mut digest = 0;
            let mut pkey = 0;
            if ffi::OBJ_find_sigid_algs(self.0, &mut digest, &mut pkey) == 1 {
                Some(SignatureAlgorithms {
                    digest: Nid(digest),
                    pkey: Nid(pkey),
                })
            } else {
                None
            }
        }
    }

    /// Returns the string representation of a `Nid` (long).
    #[corresponds(OBJ_nid2ln)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn long_name(&self) -> Result<&'static str, ErrorStack> {
        unsafe {
            cvt_p(ffi::OBJ_nid2ln(self.0) as *mut c_char)
                .map(|nameptr| str::from_utf8(CStr::from_ptr(nameptr).to_bytes()).unwrap())
        }
    }

    /// Returns the string representation of a `Nid` (short).
    #[corresponds(OBJ_nid2sn)]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn short_name(&self) -> Result<&'static str, ErrorStack> {
        unsafe {
            cvt_p(ffi::OBJ_nid2sn(self.0) as *mut c_char)
                .map(|nameptr| str::from_utf8(CStr::from_ptr(nameptr).to_bytes()).unwrap())
        }
    }

    pub const UNDEF: Nid = Nid(ffi::NID_undef);
    pub const ITU_T: Nid = Nid(ffi::NID_itu_t);
    #[cfg(not(any(boringssl, awslc)))]
    pub const CCITT: Nid = Nid(ffi::NID_ccitt);
    pub const ISO: Nid = Nid(ffi::NID_iso);
    pub const JOINT_ISO_ITU_T: Nid = Nid(ffi::NID_joint_iso_itu_t);
    #[cfg(not(any(boringssl, awslc)))]
    pub const JOINT_ISO_CCITT: Nid = Nid(ffi::NID_joint_iso_ccitt);
    pub const MEMBER_BODY: Nid = Nid(ffi::NID_member_body);
    pub const IDENTIFIED_ORGANIZATION: Nid = Nid(ffi::NID_identified_organization);
    pub const HMAC_MD5: Nid = Nid(ffi::NID_hmac_md5);
    pub const HMAC_SHA1: Nid = Nid(ffi::NID_hmac_sha1);
    pub const CERTICOM_ARC: Nid = Nid(ffi::NID_certicom_arc);
    pub const INTERNATIONAL_ORGANIZATIONS: Nid = Nid(ffi::NID_international_organizations);
    pub const WAP: Nid = Nid(ffi::NID_wap);
    pub const WAP_WSG: Nid = Nid(ffi::NID_wap_wsg);
    pub const SELECTED_ATTRIBUTE_TYPES: Nid = Nid(ffi::NID_selected_attribute_types);
    pub const CLEARANCE: Nid = Nid(ffi::NID_clearance);
    pub const ISO_US: Nid = Nid(ffi::NID_ISO_US);
    pub const X9_57: Nid = Nid(ffi::NID_X9_57);
    pub const X9CM: Nid = Nid(ffi::NID_X9cm);
    pub const DSA: Nid = Nid(ffi::NID_dsa);
    pub const DSAWITHSHA1: Nid = Nid(ffi::NID_dsaWithSHA1);
    pub const ANSI_X9_62: Nid = Nid(ffi::NID_ansi_X9_62);
    pub const X9_62_PRIME_FIELD: Nid = Nid(ffi::NID_X9_62_prime_field);
    pub const X9_62_CHARACTERISTIC_TWO_FIELD: Nid = Nid(ffi::NID_X9_62_characteristic_two_field);
    pub const X9_62_ID_CHARACTERISTIC_TWO_BASIS: Nid =
        Nid(ffi::NID_X9_62_id_characteristic_two_basis);
    pub const X9_62_ONBASIS: Nid = Nid(ffi::NID_X9_62_onBasis);
    pub const X9_62_TPBASIS: Nid = Nid(ffi::NID_X9_62_tpBasis);
    pub const X9_62_PPBASIS: Nid = Nid(ffi::NID_X9_62_ppBasis);
    pub const X9_62_ID_ECPUBLICKEY: Nid = Nid(ffi::NID_X9_62_id_ecPublicKey);
    pub const X9_62_C2PNB163V1: Nid = Nid(ffi::NID_X9_62_c2pnb163v1);
    pub const X9_62_C2PNB163V2: Nid = Nid(ffi::NID_X9_62_c2pnb163v2);
    pub const X9_62_C2PNB163V3: Nid = Nid(ffi::NID_X9_62_c2pnb163v3);
    pub const X9_62_C2PNB176V1: Nid = Nid(ffi::NID_X9_62_c2pnb176v1);
    pub const X9_62_C2TNB191V1: Nid = Nid(ffi::NID_X9_62_c2tnb191v1);
    pub const X9_62_C2TNB191V2: Nid = Nid(ffi::NID_X9_62_c2tnb191v2);
    pub const X9_62_C2TNB191V3: Nid = Nid(ffi::NID_X9_62_c2tnb191v3);
    pub const X9_62_C2ONB191V4: Nid = Nid(ffi::NID_X9_62_c2onb191v4);
    pub const X9_62_C2ONB191V5: Nid = Nid(ffi::NID_X9_62_c2onb191v5);
    pub const X9_62_C2PNB208W1: Nid = Nid(ffi::NID_X9_62_c2pnb208w1);
    pub const X9_62_C2TNB239V1: Nid = Nid(ffi::NID_X9_62_c2tnb239v1);
    pub const X9_62_C2TNB239V2: Nid = Nid(ffi::NID_X9_62_c2tnb239v2);
    pub const X9_62_C2TNB239V3: Nid = Nid(ffi::NID_X9_62_c2tnb239v3);
    pub const X9_62_C2ONB239V4: Nid = Nid(ffi::NID_X9_62_c2onb239v4);
    pub const X9_62_C2ONB239V5: Nid = Nid(ffi::NID_X9_62_c2onb239v5);
    pub const X9_62_C2PNB272W1: Nid = Nid(ffi::NID_X9_62_c2pnb272w1);
    pub const X9_62_C2PNB304W1: Nid = Nid(ffi::NID_X9_62_c2pnb304w1);
    pub const X9_62_C2TNB359V1: Nid = Nid(ffi::NID_X9_62_c2tnb359v1);
    pub const X9_62_C2PNB368W1: Nid = Nid(ffi::NID_X9_62_c2pnb368w1);
    pub const X9_62_C2TNB431R1: Nid = Nid(ffi::NID_X9_62_c2tnb431r1);
    pub const X9_62_PRIME192V1: Nid = Nid(ffi::NID_X9_62_prime192v1);
    pub const X9_62_PRIME192V2: Nid = Nid(ffi::NID_X9_62_prime192v2);
    pub const X9_62_PRIME192V3: Nid = Nid(ffi::NID_X9_62_prime192v3);
    pub const X9_62_PRIME239V1: Nid = Nid(ffi::NID_X9_62_prime239v1);
    pub const X9_62_PRIME239V2: Nid = Nid(ffi::NID_X9_62_prime239v2);
    pub const X9_62_PRIME239V3: Nid = Nid(ffi::NID_X9_62_prime239v3);
    pub const X9_62_PRIME256V1: Nid = Nid(ffi::NID_X9_62_prime256v1);
    pub const ECDSA_WITH_SHA1: Nid = Nid(ffi::NID_ecdsa_with_SHA1);
    pub const ECDSA_WITH_RECOMMENDED: Nid = Nid(ffi::NID_ecdsa_with_Recommended);
    pub const ECDSA_WITH_SPECIFIED: Nid = Nid(ffi::NID_ecdsa_with_Specified);
    pub const ECDSA_WITH_SHA224: Nid = Nid(ffi::NID_ecdsa_with_SHA224);
    pub const ECDSA_WITH_SHA256: Nid = Nid(ffi::NID_ecdsa_with_SHA256);
    pub const ECDSA_WITH_SHA384: Nid = Nid(ffi::NID_ecdsa_with_SHA384);
    pub const ECDSA_WITH_SHA512: Nid = Nid(ffi::NID_ecdsa_with_SHA512);
    pub const SECP112R1: Nid = Nid(ffi::NID_secp112r1);
    pub const SECP112R2: Nid = Nid(ffi::NID_secp112r2);
    pub const SECP128R1: Nid = Nid(ffi::NID_secp128r1);
    pub const SECP128R2: Nid = Nid(ffi::NID_secp128r2);
    pub const SECP160K1: Nid = Nid(ffi::NID_secp160k1);
    pub const SECP160R1: Nid = Nid(ffi::NID_secp160r1);
    pub const SECP160R2: Nid = Nid(ffi::NID_secp160r2);
    pub const SECP192K1: Nid = Nid(ffi::NID_secp192k1);
    pub const SECP224K1: Nid = Nid(ffi::NID_secp224k1);
    pub const SECP224R1: Nid = Nid(ffi::NID_secp224r1);
    pub const SECP256K1: Nid = Nid(ffi::NID_secp256k1);
    pub const SECP384R1: Nid = Nid(ffi::NID_secp384r1);
    pub const SECP521R1: Nid = Nid(ffi::NID_secp521r1);
    pub const SECT113R1: Nid = Nid(ffi::NID_sect113r1);
    pub const SECT113R2: Nid = Nid(ffi::NID_sect113r2);
    pub const SECT131R1: Nid = Nid(ffi::NID_sect131r1);
    pub const SECT131R2: Nid = Nid(ffi::NID_sect131r2);
    pub const SECT163K1: Nid = Nid(ffi::NID_sect163k1);
    pub const SECT163R1: Nid = Nid(ffi::NID_sect163r1);
    pub const SECT163R2: Nid = Nid(ffi::NID_sect163r2);
    pub const SECT193R1: Nid = Nid(ffi::NID_sect193r1);
    pub const SECT193R2: Nid = Nid(ffi::NID_sect193r2);
    pub const SECT233K1: Nid = Nid(ffi::NID_sect233k1);
    pub const SECT233R1: Nid = Nid(ffi::NID_sect233r1);
    pub const SECT239K1: Nid = Nid(ffi::NID_sect239k1);
    pub const SECT283K1: Nid = Nid(ffi::NID_sect283k1);
    pub const SECT283R1: Nid = Nid(ffi::NID_sect283r1);
    pub const SECT409K1: Nid = Nid(ffi::NID_sect409k1);
    pub const SECT409R1: Nid = Nid(ffi::NID_sect409r1);
    pub const SECT571K1: Nid = Nid(ffi::NID_sect571k1);
    pub const SECT571R1: Nid = Nid(ffi::NID_sect571r1);
    #[cfg(any(ossl110, libressl))]
    pub const BRAINPOOL_P256R1: Nid = Nid(ffi::NID_brainpoolP256r1);
    #[cfg(any(ossl110, libressl))]
    pub const BRAINPOOL_P320R1: Nid = Nid(ffi::NID_brainpoolP320r1);
    #[cfg(any(ossl110, libressl))]
    pub const BRAINPOOL_P384R1: Nid = Nid(ffi::NID_brainpoolP384r1);
    #[cfg(any(ossl110, libressl))]
    pub const BRAINPOOL_P512R1: Nid = Nid(ffi::NID_brainpoolP512r1);
    pub const WAP_WSG_IDM_ECID_WTLS1: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls1);
    pub const WAP_WSG_IDM_ECID_WTLS3: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls3);
    pub const WAP_WSG_IDM_ECID_WTLS4: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls4);
    pub const WAP_WSG_IDM_ECID_WTLS5: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls5);
    pub const WAP_WSG_IDM_ECID_WTLS6: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls6);
    pub const WAP_WSG_IDM_ECID_WTLS7: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls7);
    pub const WAP_WSG_IDM_ECID_WTLS8: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls8);
    pub const WAP_WSG_IDM_ECID_WTLS9: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls9);
    pub const WAP_WSG_IDM_ECID_WTLS10: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls10);
    pub const WAP_WSG_IDM_ECID_WTLS11: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls11);
    pub const WAP_WSG_IDM_ECID_WTLS12: Nid = Nid(ffi::NID_wap_wsg_idm_ecid_wtls12);
    pub const CAST5_CBC: Nid = Nid(ffi::NID_cast5_cbc);
    pub const CAST5_ECB: Nid = Nid(ffi::NID_cast5_ecb);
    pub const CAST5_CFB64: Nid = Nid(ffi::NID_cast5_cfb64);
    pub const CAST5_OFB64: Nid = Nid(ffi::NID_cast5_ofb64);
    pub const PBEWITHMD5ANDCAST5_CBC: Nid = Nid(ffi::NID_pbeWithMD5AndCast5_CBC);
    pub const ID_PASSWORDBASEDMAC: Nid = Nid(ffi::NID_id_PasswordBasedMAC);
    pub const ID_DHBASEDMAC: Nid = Nid(ffi::NID_id_DHBasedMac);
    pub const RSADSI: Nid = Nid(ffi::NID_rsadsi);
    pub const PKCS: Nid = Nid(ffi::NID_pkcs);
    pub const PKCS1: Nid = Nid(ffi::NID_pkcs1);
    pub const RSAENCRYPTION: Nid = Nid(ffi::NID_rsaEncryption);
    pub const MD2WITHRSAENCRYPTION: Nid = Nid(ffi::NID_md2WithRSAEncryption);
    pub const MD4WITHRSAENCRYPTION: Nid = Nid(ffi::NID_md4WithRSAEncryption);
    pub const MD5WITHRSAENCRYPTION: Nid = Nid(ffi::NID_md5WithRSAEncryption);
    pub const SHA1WITHRSAENCRYPTION: Nid = Nid(ffi::NID_sha1WithRSAEncryption);
    pub const RSAESOAEP: Nid = Nid(ffi::NID_rsaesOaep);
    pub const MGF1: Nid = Nid(ffi::NID_mgf1);
    pub const RSASSAPSS: Nid = Nid(ffi::NID_rsassaPss);
    pub const SHA256WITHRSAENCRYPTION: Nid = Nid(ffi::NID_sha256WithRSAEncryption);
    pub const SHA384WITHRSAENCRYPTION: Nid = Nid(ffi::NID_sha384WithRSAEncryption);
    pub const SHA512WITHRSAENCRYPTION: Nid = Nid(ffi::NID_sha512WithRSAEncryption);
    pub const SHA224WITHRSAENCRYPTION: Nid = Nid(ffi::NID_sha224WithRSAEncryption);
    pub const PKCS3: Nid = Nid(ffi::NID_pkcs3);
    pub const DHKEYAGREEMENT: Nid = Nid(ffi::NID_dhKeyAgreement);
    pub const PKCS5: Nid = Nid(ffi::NID_pkcs5);
    pub const PBEWITHMD2ANDDES_CBC: Nid = Nid(ffi::NID_pbeWithMD2AndDES_CBC);
    pub const PBEWITHMD5ANDDES_CBC: Nid = Nid(ffi::NID_pbeWithMD5AndDES_CBC);
    pub const PBEWITHMD2ANDRC2_CBC: Nid = Nid(ffi::NID_pbeWithMD2AndRC2_CBC);
    pub const PBEWITHMD5ANDRC2_CBC: Nid = Nid(ffi::NID_pbeWithMD5AndRC2_CBC);
    pub const PBEWITHSHA1ANDDES_CBC: Nid = Nid(ffi::NID_pbeWithSHA1AndDES_CBC);
    pub const PBEWITHSHA1ANDRC2_CBC: Nid = Nid(ffi::NID_pbeWithSHA1AndRC2_CBC);
    pub const ID_PBKDF2: Nid = Nid(ffi::NID_id_pbkdf2);
    pub const PBES2: Nid = Nid(ffi::NID_pbes2);
    pub const PBMAC1: Nid = Nid(ffi::NID_pbmac1);
    pub const PKCS7: Nid = Nid(ffi::NID_pkcs7);
    pub const PKCS7_DATA: Nid = Nid(ffi::NID_pkcs7_data);
    pub const PKCS7_SIGNED: Nid = Nid(ffi::NID_pkcs7_signed);
    pub const PKCS7_ENVELOPED: Nid = Nid(ffi::NID_pkcs7_enveloped);
    pub const PKCS7_SIGNEDANDENVELOPED: Nid = Nid(ffi::NID_pkcs7_signedAndEnveloped);
    pub const PKCS7_DIGEST: Nid = Nid(ffi::NID_pkcs7_digest);
    pub const PKCS7_ENCRYPTED: Nid = Nid(ffi::NID_pkcs7_encrypted);
    pub const PKCS9: Nid = Nid(ffi::NID_pkcs9);
    pub const PKCS9_EMAILADDRESS: Nid = Nid(ffi::NID_pkcs9_emailAddress);
    pub const PKCS9_UNSTRUCTUREDNAME: Nid = Nid(ffi::NID_pkcs9_unstructuredName);
    pub const PKCS9_CONTENTTYPE: Nid = Nid(ffi::NID_pkcs9_contentType);
    pub const PKCS9_MESSAGEDIGEST: Nid = Nid(ffi::NID_pkcs9_messageDigest);
    pub const PKCS9_SIGNINGTIME: Nid = Nid(ffi::NID_pkcs9_signingTime);
    pub const PKCS9_COUNTERSIGNATURE: Nid = Nid(ffi::NID_pkcs9_countersignature);
    pub const PKCS9_CHALLENGEPASSWORD: Nid = Nid(ffi::NID_pkcs9_challengePassword);
    pub const PKCS9_UNSTRUCTUREDADDRESS: Nid = Nid(ffi::NID_pkcs9_unstructuredAddress);
    pub const PKCS9_EXTCERTATTRIBUTES: Nid = Nid(ffi::NID_pkcs9_extCertAttributes);
    pub const EXT_REQ: Nid = Nid(ffi::NID_ext_req);
    pub const SMIMECAPABILITIES: Nid = Nid(ffi::NID_SMIMECapabilities);
    pub const SMIME: Nid = Nid(ffi::NID_SMIME);
    pub const ID_SMIME_MOD: Nid = Nid(ffi::NID_id_smime_mod);
    pub const ID_SMIME_CT: Nid = Nid(ffi::NID_id_smime_ct);
    pub const ID_SMIME_AA: Nid = Nid(ffi::NID_id_smime_aa);
    pub const ID_SMIME_ALG: Nid = Nid(ffi::NID_id_smime_alg);
    pub const ID_SMIME_CD: Nid = Nid(ffi::NID_id_smime_cd);
    pub const ID_SMIME_SPQ: Nid = Nid(ffi::NID_id_smime_spq);
    pub const ID_SMIME_CTI: Nid = Nid(ffi::NID_id_smime_cti);
    pub const ID_SMIME_MOD_CMS: Nid = Nid(ffi::NID_id_smime_mod_cms);
    pub const ID_SMIME_MOD_ESS: Nid = Nid(ffi::NID_id_smime_mod_ess);
    pub const ID_SMIME_MOD_OID: Nid = Nid(ffi::NID_id_smime_mod_oid);
    pub const ID_SMIME_MOD_MSG_V3: Nid = Nid(ffi::NID_id_smime_mod_msg_v3);
    pub const ID_SMIME_MOD_ETS_ESIGNATURE_88: Nid = Nid(ffi::NID_id_smime_mod_ets_eSignature_88);
    pub const ID_SMIME_MOD_ETS_ESIGNATURE_97: Nid = Nid(ffi::NID_id_smime_mod_ets_eSignature_97);
    pub const ID_SMIME_MOD_ETS_ESIGPOLICY_88: Nid = Nid(ffi::NID_id_smime_mod_ets_eSigPolicy_88);
    pub const ID_SMIME_MOD_ETS_ESIGPOLICY_97: Nid = Nid(ffi::NID_id_smime_mod_ets_eSigPolicy_97);
    pub const ID_SMIME_CT_RECEIPT: Nid = Nid(ffi::NID_id_smime_ct_receipt);
    pub const ID_SMIME_CT_AUTHDATA: Nid = Nid(ffi::NID_id_smime_ct_authData);
    pub const ID_SMIME_CT_PUBLISHCERT: Nid = Nid(ffi::NID_id_smime_ct_publishCert);
    pub const ID_SMIME_CT_TSTINFO: Nid = Nid(ffi::NID_id_smime_ct_TSTInfo);
    pub const ID_SMIME_CT_TDTINFO: Nid = Nid(ffi::NID_id_smime_ct_TDTInfo);
    pub const ID_SMIME_CT_CONTENTINFO: Nid = Nid(ffi::NID_id_smime_ct_contentInfo);
    pub const ID_SMIME_CT_DVCSREQUESTDATA: Nid = Nid(ffi::NID_id_smime_ct_DVCSRequestData);
    pub const ID_SMIME_CT_DVCSRESPONSEDATA: Nid = Nid(ffi::NID_id_smime_ct_DVCSResponseData);
    pub const ID_SMIME_CT_COMPRESSEDDATA: Nid = Nid(ffi::NID_id_smime_ct_compressedData);
    pub const ID_CT_ASCIITEXTWITHCRLF: Nid = Nid(ffi::NID_id_ct_asciiTextWithCRLF);
    pub const ID_SMIME_AA_RECEIPTREQUEST: Nid = Nid(ffi::NID_id_smime_aa_receiptRequest);
    pub const ID_SMIME_AA_SECURITYLABEL: Nid = Nid(ffi::NID_id_smime_aa_securityLabel);
    pub const ID_SMIME_AA_MLEXPANDHISTORY: Nid = Nid(ffi::NID_id_smime_aa_mlExpandHistory);
    pub const ID_SMIME_AA_CONTENTHINT: Nid = Nid(ffi::NID_id_smime_aa_contentHint);
    pub const ID_SMIME_AA_MSGSIGDIGEST: Nid = Nid(ffi::NID_id_smime_aa_msgSigDigest);
    pub const ID_SMIME_AA_ENCAPCONTENTTYPE: Nid = Nid(ffi::NID_id_smime_aa_encapContentType);
    pub const ID_SMIME_AA_CONTENTIDENTIFIER: Nid = Nid(ffi::NID_id_smime_aa_contentIdentifier);
    pub const ID_SMIME_AA_MACVALUE: Nid = Nid(ffi::NID_id_smime_aa_macValue);
    pub const ID_SMIME_AA_EQUIVALENTLABELS: Nid = Nid(ffi::NID_id_smime_aa_equivalentLabels);
    pub const ID_SMIME_AA_CONTENTREFERENCE: Nid = Nid(ffi::NID_id_smime_aa_contentReference);
    pub const ID_SMIME_AA_ENCRYPKEYPREF: Nid = Nid(ffi::NID_id_smime_aa_encrypKeyPref);
    pub const ID_SMIME_AA_SIGNINGCERTIFICATE: Nid = Nid(ffi::NID_id_smime_aa_signingCertificate);
    pub const ID_SMIME_AA_SMIMEENCRYPTCERTS: Nid = Nid(ffi::NID_id_smime_aa_smimeEncryptCerts);
    pub const ID_SMIME_AA_TIMESTAMPTOKEN: Nid = Nid(ffi::NID_id_smime_aa_timeStampToken);
    pub const ID_SMIME_AA_ETS_SIGPOLICYID: Nid = Nid(ffi::NID_id_smime_aa_ets_sigPolicyId);
    pub const ID_SMIME_AA_ETS_COMMITMENTTYPE: Nid = Nid(ffi::NID_id_smime_aa_ets_commitmentType);
    pub const ID_SMIME_AA_ETS_SIGNERLOCATION: Nid = Nid(ffi::NID_id_smime_aa_ets_signerLocation);
    pub const ID_SMIME_AA_ETS_SIGNERATTR: Nid = Nid(ffi::NID_id_smime_aa_ets_signerAttr);
    pub const ID_SMIME_AA_ETS_OTHERSIGCERT: Nid = Nid(ffi::NID_id_smime_aa_ets_otherSigCert);
    pub const ID_SMIME_AA_ETS_CONTENTTIMESTAMP: Nid =
        Nid(ffi::NID_id_smime_aa_ets_contentTimestamp);
    pub const ID_SMIME_AA_ETS_CERTIFICATEREFS: Nid = Nid(ffi::NID_id_smime_aa_ets_CertificateRefs);
    pub const ID_SMIME_AA_ETS_REVOCATIONREFS: Nid = Nid(ffi::NID_id_smime_aa_ets_RevocationRefs);
    pub const ID_SMIME_AA_ETS_CERTVALUES: Nid = Nid(ffi::NID_id_smime_aa_ets_certValues);
    pub const ID_SMIME_AA_ETS_REVOCATIONVALUES: Nid =
        Nid(ffi::NID_id_smime_aa_ets_revocationValues);
    pub const ID_SMIME_AA_ETS_ESCTIMESTAMP: Nid = Nid(ffi::NID_id_smime_aa_ets_escTimeStamp);
    pub const ID_SMIME_AA_ETS_CERTCRLTIMESTAMP: Nid =
        Nid(ffi::NID_id_smime_aa_ets_certCRLTimestamp);
    pub const ID_SMIME_AA_ETS_ARCHIVETIMESTAMP: Nid =
        Nid(ffi::NID_id_smime_aa_ets_archiveTimeStamp);
    pub const ID_SMIME_AA_SIGNATURETYPE: Nid = Nid(ffi::NID_id_smime_aa_signatureType);
    pub const ID_SMIME_AA_DVCS_DVC: Nid = Nid(ffi::NID_id_smime_aa_dvcs_dvc);
    pub const ID_SMIME_ALG_ESDHWITH3DES: Nid = Nid(ffi::NID_id_smime_alg_ESDHwith3DES);
    pub const ID_SMIME_ALG_ESDHWITHRC2: Nid = Nid(ffi::NID_id_smime_alg_ESDHwithRC2);
    pub const ID_SMIME_ALG_3DESWRAP: Nid = Nid(ffi::NID_id_smime_alg_3DESwrap);
    pub const ID_SMIME_ALG_RC2WRAP: Nid = Nid(ffi::NID_id_smime_alg_RC2wrap);
    pub const ID_SMIME_ALG_ESDH: Nid = Nid(ffi::NID_id_smime_alg_ESDH);
    pub const ID_SMIME_ALG_CMS3DESWRAP: Nid = Nid(ffi::NID_id_smime_alg_CMS3DESwrap);
    pub const ID_SMIME_ALG_CMSRC2WRAP: Nid = Nid(ffi::NID_id_smime_alg_CMSRC2wrap);
    pub const ID_ALG_PWRI_KEK: Nid = Nid(ffi::NID_id_alg_PWRI_KEK);
    pub const ID_SMIME_CD_LDAP: Nid = Nid(ffi::NID_id_smime_cd_ldap);
    pub const ID_SMIME_SPQ_ETS_SQT_URI: Nid = Nid(ffi::NID_id_smime_spq_ets_sqt_uri);
    pub const ID_SMIME_SPQ_ETS_SQT_UNOTICE: Nid = Nid(ffi::NID_id_smime_spq_ets_sqt_unotice);
    pub const ID_SMIME_CTI_ETS_PROOFOFORIGIN: Nid = Nid(ffi::NID_id_smime_cti_ets_proofOfOrigin);
    pub const ID_SMIME_CTI_ETS_PROOFOFRECEIPT: Nid = Nid(ffi::NID_id_smime_cti_ets_proofOfReceipt);
    pub const ID_SMIME_CTI_ETS_PROOFOFDELIVERY: Nid =
        Nid(ffi::NID_id_smime_cti_ets_proofOfDelivery);
    pub const ID_SMIME_CTI_ETS_PROOFOFSENDER: Nid = Nid(ffi::NID_id_smime_cti_ets_proofOfSender);
    pub const ID_SMIME_CTI_ETS_PROOFOFAPPROVAL: Nid =
        Nid(ffi::NID_id_smime_cti_ets_proofOfApproval);
    pub const ID_SMIME_CTI_ETS_PROOFOFCREATION: Nid =
        Nid(ffi::NID_id_smime_cti_ets_proofOfCreation);
    pub const FRIENDLYNAME: Nid = Nid(ffi::NID_friendlyName);
    pub const LOCALKEYID: Nid = Nid(ffi::NID_localKeyID);
    pub const MS_CSP_NAME: Nid = Nid(ffi::NID_ms_csp_name);
    pub const LOCALKEYSET: Nid = Nid(ffi::NID_LocalKeySet);
    pub const X509CERTIFICATE: Nid = Nid(ffi::NID_x509Certificate);
    pub const SDSICERTIFICATE: Nid = Nid(ffi::NID_sdsiCertificate);
    pub const X509CRL: Nid = Nid(ffi::NID_x509Crl);
    pub const PBE_WITHSHA1AND128BITRC4: Nid = Nid(ffi::NID_pbe_WithSHA1And128BitRC4);
    pub const PBE_WITHSHA1AND40BITRC4: Nid = Nid(ffi::NID_pbe_WithSHA1And40BitRC4);
    pub const PBE_WITHSHA1AND3_KEY_TRIPLEDES_CBC: Nid =
        Nid(ffi::NID_pbe_WithSHA1And3_Key_TripleDES_CBC);
    pub const PBE_WITHSHA1AND2_KEY_TRIPLEDES_CBC: Nid =
        Nid(ffi::NID_pbe_WithSHA1And2_Key_TripleDES_CBC);
    pub const PBE_WITHSHA1AND128BITRC2_CBC: Nid = Nid(ffi::NID_pbe_WithSHA1And128BitRC2_CBC);
    pub const PBE_WITHSHA1AND40BITRC2_CBC: Nid = Nid(ffi::NID_pbe_WithSHA1And40BitRC2_CBC);
    pub const KEYBAG: Nid = Nid(ffi::NID_keyBag);
    pub const PKCS8SHROUDEDKEYBAG: Nid = Nid(ffi::NID_pkcs8ShroudedKeyBag);
    pub const CERTBAG: Nid = Nid(ffi::NID_certBag);
    pub const CRLBAG: Nid = Nid(ffi::NID_crlBag);
    pub const SECRETBAG: Nid = Nid(ffi::NID_secretBag);
    pub const SAFECONTENTSBAG: Nid = Nid(ffi::NID_safeContentsBag);
    pub const MD2: Nid = Nid(ffi::NID_md2);
    pub const MD4: Nid = Nid(ffi::NID_md4);
    pub const MD5: Nid = Nid(ffi::NID_md5);
    pub const MD5_SHA1: Nid = Nid(ffi::NID_md5_sha1);
    pub const HMACWITHMD5: Nid = Nid(ffi::NID_hmacWithMD5);
    pub const HMACWITHSHA1: Nid = Nid(ffi::NID_hmacWithSHA1);
    pub const HMACWITHSHA224: Nid = Nid(ffi::NID_hmacWithSHA224);
    pub const HMACWITHSHA256: Nid = Nid(ffi::NID_hmacWithSHA256);
    pub const HMACWITHSHA384: Nid = Nid(ffi::NID_hmacWithSHA384);
    pub const HMACWITHSHA512: Nid = Nid(ffi::NID_hmacWithSHA512);
    pub const RC2_CBC: Nid = Nid(ffi::NID_rc2_cbc);
    pub const RC2_ECB: Nid = Nid(ffi::NID_rc2_ecb);
    pub const RC2_CFB64: Nid = Nid(ffi::NID_rc2_cfb64);
    pub const RC2_OFB64: Nid = Nid(ffi::NID_rc2_ofb64);
    pub const RC2_40_CBC: Nid = Nid(ffi::NID_rc2_40_cbc);
    pub const RC2_64_CBC: Nid = Nid(ffi::NID_rc2_64_cbc);
    pub const RC4: Nid = Nid(ffi::NID_rc4);
    pub const RC4_40: Nid = Nid(ffi::NID_rc4_40);
    pub const DES_EDE3_CBC: Nid = Nid(ffi::NID_des_ede3_cbc);
    pub const RC5_CBC: Nid = Nid(ffi::NID_rc5_cbc);
    pub const RC5_ECB: Nid = Nid(ffi::NID_rc5_ecb);
    pub const RC5_CFB64: Nid = Nid(ffi::NID_rc5_cfb64);
    pub const RC5_OFB64: Nid = Nid(ffi::NID_rc5_ofb64);
    pub const MS_EXT_REQ: Nid = Nid(ffi::NID_ms_ext_req);
    pub const MS_CODE_IND: Nid = Nid(ffi::NID_ms_code_ind);
    pub const MS_CODE_COM: Nid = Nid(ffi::NID_ms_code_com);
    pub const MS_CTL_SIGN: Nid = Nid(ffi::NID_ms_ctl_sign);
    pub const MS_SGC: Nid = Nid(ffi::NID_ms_sgc);
    pub const MS_EFS: Nid = Nid(ffi::NID_ms_efs);
    pub const MS_SMARTCARD_LOGIN: Nid = Nid(ffi::NID_ms_smartcard_login);
    pub const MS_UPN: Nid = Nid(ffi::NID_ms_upn);
    pub const IDEA_CBC: Nid = Nid(ffi::NID_idea_cbc);
    pub const IDEA_ECB: Nid = Nid(ffi::NID_idea_ecb);
    pub const IDEA_CFB64: Nid = Nid(ffi::NID_idea_cfb64);
    pub const IDEA_OFB64: Nid = Nid(ffi::NID_idea_ofb64);
    pub const BF_CBC: Nid = Nid(ffi::NID_bf_cbc);
    pub const BF_ECB: Nid = Nid(ffi::NID_bf_ecb);
    pub const BF_CFB64: Nid = Nid(ffi::NID_bf_cfb64);
    pub const BF_OFB64: Nid = Nid(ffi::NID_bf_ofb64);
    pub const ID_PKIX: Nid = Nid(ffi::NID_id_pkix);
    pub const ID_PKIX_MOD: Nid = Nid(ffi::NID_id_pkix_mod);
    pub const ID_PE: Nid = Nid(ffi::NID_id_pe);
    pub const ID_QT: Nid = Nid(ffi::NID_id_qt);
    pub const ID_KP: Nid = Nid(ffi::NID_id_kp);
    pub const ID_IT: Nid = Nid(ffi::NID_id_it);
    pub const ID_PKIP: Nid = Nid(ffi::NID_id_pkip);
    pub const ID_ALG: Nid = Nid(ffi::NID_id_alg);
    pub const ID_CMC: Nid = Nid(ffi::NID_id_cmc);
    pub const ID_ON: Nid = Nid(ffi::NID_id_on);
    pub const ID_PDA: Nid = Nid(ffi::NID_id_pda);
    pub const ID_ACA: Nid = Nid(ffi::NID_id_aca);
    pub const ID_QCS: Nid = Nid(ffi::NID_id_qcs);
    pub const ID_CCT: Nid = Nid(ffi::NID_id_cct);
    pub const ID_PPL: Nid = Nid(ffi::NID_id_ppl);
    pub const ID_AD: Nid = Nid(ffi::NID_id_ad);
    pub const ID_PKIX1_EXPLICIT_88: Nid = Nid(ffi::NID_id_pkix1_explicit_88);
    pub const ID_PKIX1_IMPLICIT_88: Nid = Nid(ffi::NID_id_pkix1_implicit_88);
    pub const ID_PKIX1_EXPLICIT_93: Nid = Nid(ffi::NID_id_pkix1_explicit_93);
    pub const ID_PKIX1_IMPLICIT_93: Nid = Nid(ffi::NID_id_pkix1_implicit_93);
    pub const ID_MOD_CRMF: Nid = Nid(ffi::NID_id_mod_crmf);
    pub const ID_MOD_CMC: Nid = Nid(ffi::NID_id_mod_cmc);
    pub const ID_MOD_KEA_PROFILE_88: Nid = Nid(ffi::NID_id_mod_kea_profile_88);
    pub const ID_MOD_KEA_PROFILE_93: Nid = Nid(ffi::NID_id_mod_kea_profile_93);
    pub const ID_MOD_CMP: Nid = Nid(ffi::NID_id_mod_cmp);
    pub const ID_MOD_QUALIFIED_CERT_88: Nid = Nid(ffi::NID_id_mod_qualified_cert_88);
    pub const ID_MOD_QUALIFIED_CERT_93: Nid = Nid(ffi::NID_id_mod_qualified_cert_93);
    pub const ID_MOD_ATTRIBUTE_CERT: Nid = Nid(ffi::NID_id_mod_attribute_cert);
    pub const ID_MOD_TIMESTAMP_PROTOCOL: Nid = Nid(ffi::NID_id_mod_timestamp_protocol);
    pub const ID_MOD_OCSP: Nid = Nid(ffi::NID_id_mod_ocsp);
    pub const ID_MOD_DVCS: Nid = Nid(ffi::NID_id_mod_dvcs);
    pub const ID_MOD_CMP2000: Nid = Nid(ffi::NID_id_mod_cmp2000);
    pub const INFO_ACCESS: Nid = Nid(ffi::NID_info_access);
    pub const BIOMETRICINFO: Nid = Nid(ffi::NID_biometricInfo);
    pub const QCSTATEMENTS: Nid = Nid(ffi::NID_qcStatements);
    pub const AC_AUDITENTITY: Nid = Nid(ffi::NID_ac_auditEntity);
    pub const AC_TARGETING: Nid = Nid(ffi::NID_ac_targeting);
    pub const AACONTROLS: Nid = Nid(ffi::NID_aaControls);
    pub const SBGP_IPADDRBLOCK: Nid = Nid(ffi::NID_sbgp_ipAddrBlock);
    pub const SBGP_AUTONOMOUSSYSNUM: Nid = Nid(ffi::NID_sbgp_autonomousSysNum);
    pub const SBGP_ROUTERIDENTIFIER: Nid = Nid(ffi::NID_sbgp_routerIdentifier);
    pub const AC_PROXYING: Nid = Nid(ffi::NID_ac_proxying);
    pub const SINFO_ACCESS: Nid = Nid(ffi::NID_sinfo_access);
    pub const PROXYCERTINFO: Nid = Nid(ffi::NID_proxyCertInfo);
    pub const ID_QT_CPS: Nid = Nid(ffi::NID_id_qt_cps);
    pub const ID_QT_UNOTICE: Nid = Nid(ffi::NID_id_qt_unotice);
    pub const TEXTNOTICE: Nid = Nid(ffi::NID_textNotice);
    pub const SERVER_AUTH: Nid = Nid(ffi::NID_server_auth);
    pub const CLIENT_AUTH: Nid = Nid(ffi::NID_client_auth);
    pub const CODE_SIGN: Nid = Nid(ffi::NID_code_sign);
    pub const EMAIL_PROTECT: Nid = Nid(ffi::NID_email_protect);
    pub const IPSECENDSYSTEM: Nid = Nid(ffi::NID_ipsecEndSystem);
    pub const IPSECTUNNEL: Nid = Nid(ffi::NID_ipsecTunnel);
    pub const IPSECUSER: Nid = Nid(ffi::NID_ipsecUser);
    pub const TIME_STAMP: Nid = Nid(ffi::NID_time_stamp);
    pub const OCSP_SIGN: Nid = Nid(ffi::NID_OCSP_sign);
    pub const DVCS: Nid = Nid(ffi::NID_dvcs);
    pub const ID_IT_CAPROTENCCERT: Nid = Nid(ffi::NID_id_it_caProtEncCert);
    pub const ID_IT_SIGNKEYPAIRTYPES: Nid = Nid(ffi::NID_id_it_signKeyPairTypes);
    pub const ID_IT_ENCKEYPAIRTYPES: Nid = Nid(ffi::NID_id_it_encKeyPairTypes);
    pub const ID_IT_PREFERREDSYMMALG: Nid = Nid(ffi::NID_id_it_preferredSymmAlg);
    pub const ID_IT_CAKEYUPDATEINFO: Nid = Nid(ffi::NID_id_it_caKeyUpdateInfo);
    pub const ID_IT_CURRENTCRL: Nid = Nid(ffi::NID_id_it_currentCRL);
    pub const ID_IT_UNSUPPORTEDOIDS: Nid = Nid(ffi::NID_id_it_unsupportedOIDs);
    pub const ID_IT_SUBSCRIPTIONREQUEST: Nid = Nid(ffi::NID_id_it_subscriptionRequest);
    pub const ID_IT_SUBSCRIPTIONRESPONSE: Nid = Nid(ffi::NID_id_it_subscriptionResponse);
    pub const ID_IT_KEYPAIRPARAMREQ: Nid = Nid(ffi::NID_id_it_keyPairParamReq);
    pub const ID_IT_KEYPAIRPARAMREP: Nid = Nid(ffi::NID_id_it_keyPairParamRep);
    pub const ID_IT_REVPASSPHRASE: Nid = Nid(ffi::NID_id_it_revPassphrase);
    pub const ID_IT_IMPLICITCONFIRM: Nid = Nid(ffi::NID_id_it_implicitConfirm);
    pub const ID_IT_CONFIRMWAITTIME: Nid = Nid(ffi::NID_id_it_confirmWaitTime);
    pub const ID_IT_ORIGPKIMESSAGE: Nid = Nid(ffi::NID_id_it_origPKIMessage);
    pub const ID_IT_SUPPLANGTAGS: Nid = Nid(ffi::NID_id_it_suppLangTags);
    pub const ID_REGCTRL: Nid = Nid(ffi::NID_id_regCtrl);
    pub const ID_REGINFO: Nid = Nid(ffi::NID_id_regInfo);
    pub const ID_REGCTRL_REGTOKEN: Nid = Nid(ffi::NID_id_regCtrl_regToken);
    pub const ID_REGCTRL_AUTHENTICATOR: Nid = Nid(ffi::NID_id_regCtrl_authenticator);
    pub const ID_REGCTRL_PKIPUBLICATIONINFO: Nid = Nid(ffi::NID_id_regCtrl_pkiPublicationInfo);
    pub const ID_REGCTRL_PKIARCHIVEOPTIONS: Nid = Nid(ffi::NID_id_regCtrl_pkiArchiveOptions);
    pub const ID_REGCTRL_OLDCERTID: Nid = Nid(ffi::NID_id_regCtrl_oldCertID);
    pub const ID_REGCTRL_PROTOCOLENCRKEY: Nid = Nid(ffi::NID_id_regCtrl_protocolEncrKey);
    pub const ID_REGINFO_UTF8PAIRS: Nid = Nid(ffi::NID_id_regInfo_utf8Pairs);
    pub const ID_REGINFO_CERTREQ: Nid = Nid(ffi::NID_id_regInfo_certReq);
    pub const ID_ALG_DES40: Nid = Nid(ffi::NID_id_alg_des40);
    pub const ID_ALG_NOSIGNATURE: Nid = Nid(ffi::NID_id_alg_noSignature);
    pub const ID_ALG_DH_SIG_HMAC_SHA1: Nid = Nid(ffi::NID_id_alg_dh_sig_hmac_sha1);
    pub const ID_ALG_DH_POP: Nid = Nid(ffi::NID_id_alg_dh_pop);
    pub const ID_CMC_STATUSINFO: Nid = Nid(ffi::NID_id_cmc_statusInfo);
    pub const ID_CMC_IDENTIFICATION: Nid = Nid(ffi::NID_id_cmc_identification);
    pub const ID_CMC_IDENTITYPROOF: Nid = Nid(ffi::NID_id_cmc_identityProof);
    pub const ID_CMC_DATARETURN: Nid = Nid(ffi::NID_id_cmc_dataReturn);
    pub const ID_CMC_TRANSACTIONID: Nid = Nid(ffi::NID_id_cmc_transactionId);
    pub const ID_CMC_SENDERNONCE: Nid = Nid(ffi::NID_id_cmc_senderNonce);
    pub const ID_CMC_RECIPIENTNONCE: Nid = Nid(ffi::NID_id_cmc_recipientNonce);
    pub const ID_CMC_ADDEXTENSIONS: Nid = Nid(ffi::NID_id_cmc_addExtensions);
    pub const ID_CMC_ENCRYPTEDPOP: Nid = Nid(ffi::NID_id_cmc_encryptedPOP);
    pub const ID_CMC_DECRYPTEDPOP: Nid = Nid(ffi::NID_id_cmc_decryptedPOP);
    pub const ID_CMC_LRAPOPWITNESS: Nid = Nid(ffi::NID_id_cmc_lraPOPWitness);
    pub const ID_CMC_GETCERT: Nid = Nid(ffi::NID_id_cmc_getCert);
    pub const ID_CMC_GETCRL: Nid = Nid(ffi::NID_id_cmc_getCRL);
    pub const ID_CMC_REVOKEREQUEST: Nid = Nid(ffi::NID_id_cmc_revokeRequest);
    pub const ID_CMC_REGINFO: Nid = Nid(ffi::NID_id_cmc_regInfo);
    pub const ID_CMC_RESPONSEINFO: Nid = Nid(ffi::NID_id_cmc_responseInfo);
    pub const ID_CMC_QUERYPENDING: Nid = Nid(ffi::NID_id_cmc_queryPending);
    pub const ID_CMC_POPLINKRANDOM: Nid = Nid(ffi::NID_id_cmc_popLinkRandom);
    pub const ID_CMC_POPLINKWITNESS: Nid = Nid(ffi::NID_id_cmc_popLinkWitness);
    pub const ID_CMC_CONFIRMCERTACCEPTANCE: Nid = Nid(ffi::NID_id_cmc_confirmCertAcceptance);
    pub const ID_ON_PERSONALDATA: Nid = Nid(ffi::NID_id_on_personalData);
    pub const ID_ON_PERMANENTIDENTIFIER: Nid = Nid(ffi::NID_id_on_permanentIdentifier);
    pub const ID_PDA_DATEOFBIRTH: Nid = Nid(ffi::NID_id_pda_dateOfBirth);
    pub const ID_PDA_PLACEOFBIRTH: Nid = Nid(ffi::NID_id_pda_placeOfBirth);
    pub const ID_PDA_GENDER: Nid = Nid(ffi::NID_id_pda_gender);
    pub const ID_PDA_COUNTRYOFCITIZENSHIP: Nid = Nid(ffi::NID_id_pda_countryOfCitizenship);
    pub const ID_PDA_COUNTRYOFRESIDENCE: Nid = Nid(ffi::NID_id_pda_countryOfResidence);
    pub const ID_ACA_AUTHENTICATIONINFO: Nid = Nid(ffi::NID_id_aca_authenticationInfo);
    pub const ID_ACA_ACCESSIDENTITY: Nid = Nid(ffi::NID_id_aca_accessIdentity);
    pub const ID_ACA_CHARGINGIDENTITY: Nid = Nid(ffi::NID_id_aca_chargingIdentity);
    pub const ID_ACA_GROUP: Nid = Nid(ffi::NID_id_aca_group);
    pub const ID_ACA_ROLE: Nid = Nid(ffi::NID_id_aca_role);
    pub const ID_ACA_ENCATTRS: Nid = Nid(ffi::NID_id_aca_encAttrs);
    pub const ID_QCS_PKIXQCSYNTAX_V1: Nid = Nid(ffi::NID_id_qcs_pkixQCSyntax_v1);
    pub const ID_CCT_CRS: Nid = Nid(ffi::NID_id_cct_crs);
    pub const ID_CCT_PKIDATA: Nid = Nid(ffi::NID_id_cct_PKIData);
    pub const ID_CCT_PKIRESPONSE: Nid = Nid(ffi::NID_id_cct_PKIResponse);
    pub const ID_PPL_ANYLANGUAGE: Nid = Nid(ffi::NID_id_ppl_anyLanguage);
    pub const ID_PPL_INHERITALL: Nid = Nid(ffi::NID_id_ppl_inheritAll);
    pub const INDEPENDENT: Nid = Nid(ffi::NID_Independent);
    pub const AD_OCSP: Nid = Nid(ffi::NID_ad_OCSP);
    pub const AD_CA_ISSUERS: Nid = Nid(ffi::NID_ad_ca_issuers);
    pub const AD_TIMESTAMPING: Nid = Nid(ffi::NID_ad_timeStamping);
    pub const AD_DVCS: Nid = Nid(ffi::NID_ad_dvcs);
    pub const CAREPOSITORY: Nid = Nid(ffi::NID_caRepository);
    pub const ID_PKIX_OCSP_BASIC: Nid = Nid(ffi::NID_id_pkix_OCSP_basic);
    pub const ID_PKIX_OCSP_NONCE: Nid = Nid(ffi::NID_id_pkix_OCSP_Nonce);
    pub const ID_PKIX_OCSP_CRLID: Nid = Nid(ffi::NID_id_pkix_OCSP_CrlID);
    pub const ID_PKIX_OCSP_ACCEPTABLERESPONSES: Nid =
        Nid(ffi::NID_id_pkix_OCSP_acceptableResponses);
    pub const ID_PKIX_OCSP_NOCHECK: Nid = Nid(ffi::NID_id_pkix_OCSP_noCheck);
    pub const ID_PKIX_OCSP_ARCHIVECUTOFF: Nid = Nid(ffi::NID_id_pkix_OCSP_archiveCutoff);
    pub const ID_PKIX_OCSP_SERVICELOCATOR: Nid = Nid(ffi::NID_id_pkix_OCSP_serviceLocator);
    pub const ID_PKIX_OCSP_EXTENDEDSTATUS: Nid = Nid(ffi::NID_id_pkix_OCSP_extendedStatus);
    pub const ID_PKIX_OCSP_VALID: Nid = Nid(ffi::NID_id_pkix_OCSP_valid);
    pub const ID_PKIX_OCSP_PATH: Nid = Nid(ffi::NID_id_pkix_OCSP_path);
    pub const ID_PKIX_OCSP_TRUSTROOT: Nid = Nid(ffi::NID_id_pkix_OCSP_trustRoot);
    pub const ALGORITHM: Nid = Nid(ffi::NID_algorithm);
    pub const MD5WITHRSA: Nid = Nid(ffi::NID_md5WithRSA);
    pub const DES_ECB: Nid = Nid(ffi::NID_des_ecb);
    pub const DES_CBC: Nid = Nid(ffi::NID_des_cbc);
    pub const DES_OFB64: Nid = Nid(ffi::NID_des_ofb64);
    pub const DES_CFB64: Nid = Nid(ffi::NID_des_cfb64);
    pub const RSASIGNATURE: Nid = Nid(ffi::NID_rsaSignature);
    pub const DSA_2: Nid = Nid(ffi::NID_dsa_2);
    pub const DSAWITHSHA: Nid = Nid(ffi::NID_dsaWithSHA);
    pub const SHAWITHRSAENCRYPTION: Nid = Nid(ffi::NID_shaWithRSAEncryption);
    pub const DES_EDE_ECB: Nid = Nid(ffi::NID_des_ede_ecb);
    pub const DES_EDE3_ECB: Nid = Nid(ffi::NID_des_ede3_ecb);
    pub const DES_EDE_CBC: Nid = Nid(ffi::NID_des_ede_cbc);
    pub const DES_EDE_CFB64: Nid = Nid(ffi::NID_des_ede_cfb64);
    pub const DES_EDE3_CFB64: Nid = Nid(ffi::NID_des_ede3_cfb64);
    pub const DES_EDE_OFB64: Nid = Nid(ffi::NID_des_ede_ofb64);
    pub const DES_EDE3_OFB64: Nid = Nid(ffi::NID_des_ede3_ofb64);
    pub const DESX_CBC: Nid = Nid(ffi::NID_desx_cbc);
    pub const SHA: Nid = Nid(ffi::NID_sha);
    pub const SHA1: Nid = Nid(ffi::NID_sha1);
    pub const DSAWITHSHA1_2: Nid = Nid(ffi::NID_dsaWithSHA1_2);
    pub const SHA1WITHRSA: Nid = Nid(ffi::NID_sha1WithRSA);
    pub const RIPEMD160: Nid = Nid(ffi::NID_ripemd160);
    pub const RIPEMD160WITHRSA: Nid = Nid(ffi::NID_ripemd160WithRSA);
    pub const SXNET: Nid = Nid(ffi::NID_sxnet);
    pub const X500: Nid = Nid(ffi::NID_X500);
    pub const X509: Nid = Nid(ffi::NID_X509);
    pub const COMMONNAME: Nid = Nid(ffi::NID_commonName);
    pub const SURNAME: Nid = Nid(ffi::NID_surname);
    pub const SERIALNUMBER: Nid = Nid(ffi::NID_serialNumber);
    pub const COUNTRYNAME: Nid = Nid(ffi::NID_countryName);
    pub const LOCALITYNAME: Nid = Nid(ffi::NID_localityName);
    pub const STATEORPROVINCENAME: Nid = Nid(ffi::NID_stateOrProvinceName);
    pub const STREETADDRESS: Nid = Nid(ffi::NID_streetAddress);
    pub const ORGANIZATIONNAME: Nid = Nid(ffi::NID_organizationName);
    pub const ORGANIZATIONALUNITNAME: Nid = Nid(ffi::NID_organizationalUnitName);
    pub const TITLE: Nid = Nid(ffi::NID_title);
    pub const DESCRIPTION: Nid = Nid(ffi::NID_description);
    pub const SEARCHGUIDE: Nid = Nid(ffi::NID_searchGuide);
    pub const BUSINESSCATEGORY: Nid = Nid(ffi::NID_businessCategory);
    pub const POSTALADDRESS: Nid = Nid(ffi::NID_postalAddress);
    pub const POSTALCODE: Nid = Nid(ffi::NID_postalCode);
    pub const POSTOFFICEBOX: Nid = Nid(ffi::NID_postOfficeBox);
    pub const PHYSICALDELIVERYOFFICENAME: Nid = Nid(ffi::NID_physicalDeliveryOfficeName);
    pub const TELEPHONENUMBER: Nid = Nid(ffi::NID_telephoneNumber);
    pub const TELEXNUMBER: Nid = Nid(ffi::NID_telexNumber);
    pub const TELETEXTERMINALIDENTIFIER: Nid = Nid(ffi::NID_teletexTerminalIdentifier);
    pub const FACSIMILETELEPHONENUMBER: Nid = Nid(ffi::NID_facsimileTelephoneNumber);
    pub const X121ADDRESS: Nid = Nid(ffi::NID_x121Address);
    pub const INTERNATIONALISDNNUMBER: Nid = Nid(ffi::NID_internationaliSDNNumber);
    pub const REGISTEREDADDRESS: Nid = Nid(ffi::NID_registeredAddress);
    pub const DESTINATIONINDICATOR: Nid = Nid(ffi::NID_destinationIndicator);
    pub const PREFERREDDELIVERYMETHOD: Nid = Nid(ffi::NID_preferredDeliveryMethod);
    pub const PRESENTATIONADDRESS: Nid = Nid(ffi::NID_presentationAddress);
    pub const SUPPORTEDAPPLICATIONCONTEXT: Nid = Nid(ffi::NID_supportedApplicationContext);
    pub const MEMBER: Nid = Nid(ffi::NID_member);
    pub const OWNER: Nid = Nid(ffi::NID_owner);
    pub const ROLEOCCUPANT: Nid = Nid(ffi::NID_roleOccupant);
    pub const SEEALSO: Nid = Nid(ffi::NID_seeAlso);
    pub const USERPASSWORD: Nid = Nid(ffi::NID_userPassword);
    pub const USERCERTIFICATE: Nid = Nid(ffi::NID_userCertificate);
    pub const CACERTIFICATE: Nid = Nid(ffi::NID_cACertificate);
    pub const AUTHORITYREVOCATIONLIST: Nid = Nid(ffi::NID_authorityRevocationList);
    pub const CERTIFICATEREVOCATIONLIST: Nid = Nid(ffi::NID_certificateRevocationList);
    pub const CROSSCERTIFICATEPAIR: Nid = Nid(ffi::NID_crossCertificatePair);
    pub const NAME: Nid = Nid(ffi::NID_name);
    pub const GIVENNAME: Nid = Nid(ffi::NID_givenName);
    pub const INITIALS: Nid = Nid(ffi::NID_initials);
    pub const GENERATIONQUALIFIER: Nid = Nid(ffi::NID_generationQualifier);
    pub const X500UNIQUEIDENTIFIER: Nid = Nid(ffi::NID_x500UniqueIdentifier);
    pub const DNQUALIFIER: Nid = Nid(ffi::NID_dnQualifier);
    pub const ENHANCEDSEARCHGUIDE: Nid = Nid(ffi::NID_enhancedSearchGuide);
    pub const PROTOCOLINFORMATION: Nid = Nid(ffi::NID_protocolInformation);
    pub const DISTINGUISHEDNAME: Nid = Nid(ffi::NID_distinguishedName);
    pub const UNIQUEMEMBER: Nid = Nid(ffi::NID_uniqueMember);
    pub const HOUSEIDENTIFIER: Nid = Nid(ffi::NID_houseIdentifier);
    pub const SUPPORTEDALGORITHMS: Nid = Nid(ffi::NID_supportedAlgorithms);
    pub const DELTAREVOCATIONLIST: Nid = Nid(ffi::NID_deltaRevocationList);
    pub const DMDNAME: Nid = Nid(ffi::NID_dmdName);
    pub const PSEUDONYM: Nid = Nid(ffi::NID_pseudonym);
    pub const ROLE: Nid = Nid(ffi::NID_role);
    pub const X500ALGORITHMS: Nid = Nid(ffi::NID_X500algorithms);
    pub const RSA: Nid = Nid(ffi::NID_rsa);
    pub const MDC2WITHRSA: Nid = Nid(ffi::NID_mdc2WithRSA);
    pub const MDC2: Nid = Nid(ffi::NID_mdc2);
    pub const ID_CE: Nid = Nid(ffi::NID_id_ce);
    pub const SUBJECT_DIRECTORY_ATTRIBUTES: Nid = Nid(ffi::NID_subject_directory_attributes);
    pub const SUBJECT_KEY_IDENTIFIER: Nid = Nid(ffi::NID_subject_key_identifier);
    pub const KEY_USAGE: Nid = Nid(ffi::NID_key_usage);
    pub const PRIVATE_KEY_USAGE_PERIOD: Nid = Nid(ffi::NID_private_key_usage_period);
    pub const SUBJECT_ALT_NAME: Nid = Nid(ffi::NID_subject_alt_name);
    pub const ISSUER_ALT_NAME: Nid = Nid(ffi::NID_issuer_alt_name);
    pub const BASIC_CONSTRAINTS: Nid = Nid(ffi::NID_basic_constraints);
    pub const CRL_NUMBER: Nid = Nid(ffi::NID_crl_number);
    pub const CRL_REASON: Nid = Nid(ffi::NID_crl_reason);
    pub const INVALIDITY_DATE: Nid = Nid(ffi::NID_invalidity_date);
    pub const DELTA_CRL: Nid = Nid(ffi::NID_delta_crl);
    pub const ISSUING_DISTRIBUTION_POINT: Nid = Nid(ffi::NID_issuing_distribution_point);
    pub const CERTIFICATE_ISSUER: Nid = Nid(ffi::NID_certificate_issuer);
    pub const NAME_CONSTRAINTS: Nid = Nid(ffi::NID_name_constraints);
    pub const CRL_DISTRIBUTION_POINTS: Nid = Nid(ffi::NID_crl_distribution_points);
    pub const CERTIFICATE_POLICIES: Nid = Nid(ffi::NID_certificate_policies);
    pub const ANY_POLICY: Nid = Nid(ffi::NID_any_policy);
    pub const POLICY_MAPPINGS: Nid = Nid(ffi::NID_policy_mappings);
    pub const AUTHORITY_KEY_IDENTIFIER: Nid = Nid(ffi::NID_authority_key_identifier);
    pub const POLICY_CONSTRAINTS: Nid = Nid(ffi::NID_policy_constraints);
    pub const EXT_KEY_USAGE: Nid = Nid(ffi::NID_ext_key_usage);
    pub const FRESHEST_CRL: Nid = Nid(ffi::NID_freshest_crl);
    pub const INHIBIT_ANY_POLICY: Nid = Nid(ffi::NID_inhibit_any_policy);
    pub const TARGET_INFORMATION: Nid = Nid(ffi::NID_target_information);
    pub const NO_REV_AVAIL: Nid = Nid(ffi::NID_no_rev_avail);
    pub const ANYEXTENDEDKEYUSAGE: Nid = Nid(ffi::NID_anyExtendedKeyUsage);
    pub const NETSCAPE: Nid = Nid(ffi::NID_netscape);
    pub const NETSCAPE_CERT_EXTENSION: Nid = Nid(ffi::NID_netscape_cert_extension);
    pub const NETSCAPE_DATA_TYPE: Nid = Nid(ffi::NID_netscape_data_type);
    pub const NETSCAPE_CERT_TYPE: Nid = Nid(ffi::NID_netscape_cert_type);
    pub const NETSCAPE_BASE_URL: Nid = Nid(ffi::NID_netscape_base_url);
    pub const NETSCAPE_REVOCATION_URL: Nid = Nid(ffi::NID_netscape_revocation_url);
    pub const NETSCAPE_CA_REVOCATION_URL: Nid = Nid(ffi::NID_netscape_ca_revocation_url);
    pub const NETSCAPE_RENEWAL_URL: Nid = Nid(ffi::NID_netscape_renewal_url);
    pub const NETSCAPE_CA_POLICY_URL: Nid = Nid(ffi::NID_netscape_ca_policy_url);
    pub const NETSCAPE_SSL_SERVER_NAME: Nid = Nid(ffi::NID_netscape_ssl_server_name);
    pub const NETSCAPE_COMMENT: Nid = Nid(ffi::NID_netscape_comment);
    pub const NETSCAPE_CERT_SEQUENCE: Nid = Nid(ffi::NID_netscape_cert_sequence);
    pub const NS_SGC: Nid = Nid(ffi::NID_ns_sgc);
    pub const ORG: Nid = Nid(ffi::NID_org);
    pub const DOD: Nid = Nid(ffi::NID_dod);
    pub const IANA: Nid = Nid(ffi::NID_iana);
    pub const DIRECTORY: Nid = Nid(ffi::NID_Directory);
    pub const MANAGEMENT: Nid = Nid(ffi::NID_Management);
    pub const EXPERIMENTAL: Nid = Nid(ffi::NID_Experimental);
    pub const PRIVATE: Nid = Nid(ffi::NID_Private);
    pub const SECURITY: Nid = Nid(ffi::NID_Security);
    pub const SNMPV2: Nid = Nid(ffi::NID_SNMPv2);
    pub const MAIL: Nid = Nid(ffi::NID_Mail);
    pub const ENTERPRISES: Nid = Nid(ffi::NID_Enterprises);
    pub const DCOBJECT: Nid = Nid(ffi::NID_dcObject);
    pub const MIME_MHS: Nid = Nid(ffi::NID_mime_mhs);
    pub const MIME_MHS_HEADINGS: Nid = Nid(ffi::NID_mime_mhs_headings);
    pub const MIME_MHS_BODIES: Nid = Nid(ffi::NID_mime_mhs_bodies);
    pub const ID_HEX_PARTIAL_MESSAGE: Nid = Nid(ffi::NID_id_hex_partial_message);
    pub const ID_HEX_MULTIPART_MESSAGE: Nid = Nid(ffi::NID_id_hex_multipart_message);
    pub const ZLIB_COMPRESSION: Nid = Nid(ffi::NID_zlib_compression);
    pub const AES_128_ECB: Nid = Nid(ffi::NID_aes_128_ecb);
    pub const AES_128_CBC: Nid = Nid(ffi::NID_aes_128_cbc);
    pub const AES_128_OFB128: Nid = Nid(ffi::NID_aes_128_ofb128);
    pub const AES_128_CFB128: Nid = Nid(ffi::NID_aes_128_cfb128);
    pub const ID_AES128_WRAP: Nid = Nid(ffi::NID_id_aes128_wrap);
    pub const AES_128_GCM: Nid = Nid(ffi::NID_aes_128_gcm);
    pub const AES_128_CCM: Nid = Nid(ffi::NID_aes_128_ccm);
    pub const ID_AES128_WRAP_PAD: Nid = Nid(ffi::NID_id_aes128_wrap_pad);
    pub const AES_192_ECB: Nid = Nid(ffi::NID_aes_192_ecb);
    pub const AES_192_CBC: Nid = Nid(ffi::NID_aes_192_cbc);
    pub const AES_192_OFB128: Nid = Nid(ffi::NID_aes_192_ofb128);
    pub const AES_192_CFB128: Nid = Nid(ffi::NID_aes_192_cfb128);
    pub const ID_AES192_WRAP: Nid = Nid(ffi::NID_id_aes192_wrap);
    pub const AES_192_GCM: Nid = Nid(ffi::NID_aes_192_gcm);
    pub const AES_192_CCM: Nid = Nid(ffi::NID_aes_192_ccm);
    pub const ID_AES192_WRAP_PAD: Nid = Nid(ffi::NID_id_aes192_wrap_pad);
    pub const AES_256_ECB: Nid = Nid(ffi::NID_aes_256_ecb);
    pub const AES_256_CBC: Nid = Nid(ffi::NID_aes_256_cbc);
    pub const AES_256_OFB128: Nid = Nid(ffi::NID_aes_256_ofb128);
    pub const AES_256_CFB128: Nid = Nid(ffi::NID_aes_256_cfb128);
    pub const ID_AES256_WRAP: Nid = Nid(ffi::NID_id_aes256_wrap);
    pub const AES_256_GCM: Nid = Nid(ffi::NID_aes_256_gcm);
    pub const AES_256_CCM: Nid = Nid(ffi::NID_aes_256_ccm);
    pub const ID_AES256_WRAP_PAD: Nid = Nid(ffi::NID_id_aes256_wrap_pad);
    pub const AES_128_CFB1: Nid = Nid(ffi::NID_aes_128_cfb1);
    pub const AES_192_CFB1: Nid = Nid(ffi::NID_aes_192_cfb1);
    pub const AES_256_CFB1: Nid = Nid(ffi::NID_aes_256_cfb1);
    pub const AES_128_CFB8: Nid = Nid(ffi::NID_aes_128_cfb8);
    pub const AES_192_CFB8: Nid = Nid(ffi::NID_aes_192_cfb8);
    pub const AES_256_CFB8: Nid = Nid(ffi::NID_aes_256_cfb8);
    pub const AES_128_CTR: Nid = Nid(ffi::NID_aes_128_ctr);
    pub const AES_192_CTR: Nid = Nid(ffi::NID_aes_192_ctr);
    pub const AES_256_CTR: Nid = Nid(ffi::NID_aes_256_ctr);
    pub const AES_128_XTS: Nid = Nid(ffi::NID_aes_128_xts);
    pub const AES_256_XTS: Nid = Nid(ffi::NID_aes_256_xts);
    pub const DES_CFB1: Nid = Nid(ffi::NID_des_cfb1);
    pub const DES_CFB8: Nid = Nid(ffi::NID_des_cfb8);
    pub const DES_EDE3_CFB1: Nid = Nid(ffi::NID_des_ede3_cfb1);
    pub const DES_EDE3_CFB8: Nid = Nid(ffi::NID_des_ede3_cfb8);
    pub const SHA256: Nid = Nid(ffi::NID_sha256);
    pub const SHA384: Nid = Nid(ffi::NID_sha384);
    pub const SHA512: Nid = Nid(ffi::NID_sha512);
    pub const SHA224: Nid = Nid(ffi::NID_sha224);
    pub const DSA_WITH_SHA224: Nid = Nid(ffi::NID_dsa_with_SHA224);
    pub const DSA_WITH_SHA256: Nid = Nid(ffi::NID_dsa_with_SHA256);
    pub const HOLD_INSTRUCTION_CODE: Nid = Nid(ffi::NID_hold_instruction_code);
    pub const HOLD_INSTRUCTION_NONE: Nid = Nid(ffi::NID_hold_instruction_none);
    pub const HOLD_INSTRUCTION_CALL_ISSUER: Nid = Nid(ffi::NID_hold_instruction_call_issuer);
    pub const HOLD_INSTRUCTION_REJECT: Nid = Nid(ffi::NID_hold_instruction_reject);
    pub const DATA: Nid = Nid(ffi::NID_data);
    pub const PSS: Nid = Nid(ffi::NID_pss);
    pub const UCL: Nid = Nid(ffi::NID_ucl);
    pub const PILOT: Nid = Nid(ffi::NID_pilot);
    pub const PILOTATTRIBUTETYPE: Nid = Nid(ffi::NID_pilotAttributeType);
    pub const PILOTATTRIBUTESYNTAX: Nid = Nid(ffi::NID_pilotAttributeSyntax);
    pub const PILOTOBJECTCLASS: Nid = Nid(ffi::NID_pilotObjectClass);
    pub const PILOTGROUPS: Nid = Nid(ffi::NID_pilotGroups);
    pub const IA5STRINGSYNTAX: Nid = Nid(ffi::NID_iA5StringSyntax);
    pub const CASEIGNOREIA5STRINGSYNTAX: Nid = Nid(ffi::NID_caseIgnoreIA5StringSyntax);
    pub const PILOTOBJECT: Nid = Nid(ffi::NID_pilotObject);
    pub const PILOTPERSON: Nid = Nid(ffi::NID_pilotPerson);
    pub const ACCOUNT: Nid = Nid(ffi::NID_account);
    pub const DOCUMENT: Nid = Nid(ffi::NID_document);
    pub const ROOM: Nid = Nid(ffi::NID_room);
    pub const DOCUMENTSERIES: Nid = Nid(ffi::NID_documentSeries);
    pub const DOMAIN: Nid = Nid(ffi::NID_Domain);
    pub const RFC822LOCALPART: Nid = Nid(ffi::NID_rFC822localPart);
    pub const DNSDOMAIN: Nid = Nid(ffi::NID_dNSDomain);
    pub const DOMAINRELATEDOBJECT: Nid = Nid(ffi::NID_domainRelatedObject);
    pub const FRIENDLYCOUNTRY: Nid = Nid(ffi::NID_friendlyCountry);
    pub const SIMPLESECURITYOBJECT: Nid = Nid(ffi::NID_simpleSecurityObject);
    pub const PILOTORGANIZATION: Nid = Nid(ffi::NID_pilotOrganization);
    pub const PILOTDSA: Nid = Nid(ffi::NID_pilotDSA);
    pub const QUALITYLABELLEDDATA: Nid = Nid(ffi::NID_qualityLabelledData);
    pub const USERID: Nid = Nid(ffi::NID_userId);
    pub const TEXTENCODEDORADDRESS: Nid = Nid(ffi::NID_textEncodedORAddress);
    pub const RFC822MAILBOX: Nid = Nid(ffi::NID_rfc822Mailbox);
    pub const INFO: Nid = Nid(ffi::NID_info);
    pub const FAVOURITEDRINK: Nid = Nid(ffi::NID_favouriteDrink);
    pub const ROOMNUMBER: Nid = Nid(ffi::NID_roomNumber);
    pub const PHOTO: Nid = Nid(ffi::NID_photo);
    pub const USERCLASS: Nid = Nid(ffi::NID_userClass);
    pub const HOST: Nid = Nid(ffi::NID_host);
    pub const MANAGER: Nid = Nid(ffi::NID_manager);
    pub const DOCUMENTIDENTIFIER: Nid = Nid(ffi::NID_documentIdentifier);
    pub const DOCUMENTTITLE: Nid = Nid(ffi::NID_documentTitle);
    pub const DOCUMENTVERSION: Nid = Nid(ffi::NID_documentVersion);
    pub const DOCUMENTAUTHOR: Nid = Nid(ffi::NID_documentAuthor);
    pub const DOCUMENTLOCATION: Nid = Nid(ffi::NID_documentLocation);
    pub const HOMETELEPHONENUMBER: Nid = Nid(ffi::NID_homeTelephoneNumber);
    pub const SECRETARY: Nid = Nid(ffi::NID_secretary);
    pub const OTHERMAILBOX: Nid = Nid(ffi::NID_otherMailbox);
    pub const LASTMODIFIEDTIME: Nid = Nid(ffi::NID_lastModifiedTime);
    pub const LASTMODIFIEDBY: Nid = Nid(ffi::NID_lastModifiedBy);
    pub const DOMAINCOMPONENT: Nid = Nid(ffi::NID_domainComponent);
    pub const ARECORD: Nid = Nid(ffi::NID_aRecord);
    pub const PILOTATTRIBUTETYPE27: Nid = Nid(ffi::NID_pilotAttributeType27);
    pub const MXRECORD: Nid = Nid(ffi::NID_mXRecord);
    pub const NSRECORD: Nid = Nid(ffi::NID_nSRecord);
    pub const SOARECORD: Nid = Nid(ffi::NID_sOARecord);
    pub const CNAMERECORD: Nid = Nid(ffi::NID_cNAMERecord);
    pub const ASSOCIATEDDOMAIN: Nid = Nid(ffi::NID_associatedDomain);
    pub const ASSOCIATEDNAME: Nid = Nid(ffi::NID_associatedName);
    pub const HOMEPOSTALADDRESS: Nid = Nid(ffi::NID_homePostalAddress);
    pub const PERSONALTITLE: Nid = Nid(ffi::NID_personalTitle);
    pub const MOBILETELEPHONENUMBER: Nid = Nid(ffi::NID_mobileTelephoneNumber);
    pub const PAGERTELEPHONENUMBER: Nid = Nid(ffi::NID_pagerTelephoneNumber);
    pub const FRIENDLYCOUNTRYNAME: Nid = Nid(ffi::NID_friendlyCountryName);
    pub const ORGANIZATIONALSTATUS: Nid = Nid(ffi::NID_organizationalStatus);
    pub const JANETMAILBOX: Nid = Nid(ffi::NID_janetMailbox);
    pub const MAILPREFERENCEOPTION: Nid = Nid(ffi::NID_mailPreferenceOption);
    pub const BUILDINGNAME: Nid = Nid(ffi::NID_buildingName);
    pub const DSAQUALITY: Nid = Nid(ffi::NID_dSAQuality);
    pub const SINGLELEVELQUALITY: Nid = Nid(ffi::NID_singleLevelQuality);
    pub const SUBTREEMINIMUMQUALITY: Nid = Nid(ffi::NID_subtreeMinimumQuality);
    pub const SUBTREEMAXIMUMQUALITY: Nid = Nid(ffi::NID_subtreeMaximumQuality);
    pub const PERSONALSIGNATURE: Nid = Nid(ffi::NID_personalSignature);
    pub const DITREDIRECT: Nid = Nid(ffi::NID_dITRedirect);
    pub const AUDIO: Nid = Nid(ffi::NID_audio);
    pub const DOCUMENTPUBLISHER: Nid = Nid(ffi::NID_documentPublisher);
    pub const ID_SET: Nid = Nid(ffi::NID_id_set);
    pub const SET_CTYPE: Nid = Nid(ffi::NID_set_ctype);
    pub const SET_MSGEXT: Nid = Nid(ffi::NID_set_msgExt);
    pub const SET_ATTR: Nid = Nid(ffi::NID_set_attr);
    pub const SET_POLICY: Nid = Nid(ffi::NID_set_policy);
    pub const SET_CERTEXT: Nid = Nid(ffi::NID_set_certExt);
    pub const SET_BRAND: Nid = Nid(ffi::NID_set_brand);
    pub const SETCT_PANDATA: Nid = Nid(ffi::NID_setct_PANData);
    pub const SETCT_PANTOKEN: Nid = Nid(ffi::NID_setct_PANToken);
    pub const SETCT_PANONLY: Nid = Nid(ffi::NID_setct_PANOnly);
    pub const SETCT_OIDATA: Nid = Nid(ffi::NID_setct_OIData);
    pub const SETCT_PI: Nid = Nid(ffi::NID_setct_PI);
    pub const SETCT_PIDATA: Nid = Nid(ffi::NID_setct_PIData);
    pub const SETCT_PIDATAUNSIGNED: Nid = Nid(ffi::NID_setct_PIDataUnsigned);
    pub const SETCT_HODINPUT: Nid = Nid(ffi::NID_setct_HODInput);
    pub const SETCT_AUTHRESBAGGAGE: Nid = Nid(ffi::NID_setct_AuthResBaggage);
    pub const SETCT_AUTHREVREQBAGGAGE: Nid = Nid(ffi::NID_setct_AuthRevReqBaggage);
    pub const SETCT_AUTHREVRESBAGGAGE: Nid = Nid(ffi::NID_setct_AuthRevResBaggage);
    pub const SETCT_CAPTOKENSEQ: Nid = Nid(ffi::NID_setct_CapTokenSeq);
    pub const SETCT_PINITRESDATA: Nid = Nid(ffi::NID_setct_PInitResData);
    pub const SETCT_PI_TBS: Nid = Nid(ffi::NID_setct_PI_TBS);
    pub const SETCT_PRESDATA: Nid = Nid(ffi::NID_setct_PResData);
    pub const SETCT_AUTHREQTBS: Nid = Nid(ffi::NID_setct_AuthReqTBS);
    pub const SETCT_AUTHRESTBS: Nid = Nid(ffi::NID_setct_AuthResTBS);
    pub const SETCT_AUTHRESTBSX: Nid = Nid(ffi::NID_setct_AuthResTBSX);
    pub const SETCT_AUTHTOKENTBS: Nid = Nid(ffi::NID_setct_AuthTokenTBS);
    pub const SETCT_CAPTOKENDATA: Nid = Nid(ffi::NID_setct_CapTokenData);
    pub const SETCT_CAPTOKENTBS: Nid = Nid(ffi::NID_setct_CapTokenTBS);
    pub const SETCT_ACQCARDCODEMSG: Nid = Nid(ffi::NID_setct_AcqCardCodeMsg);
    pub const SETCT_AUTHREVREQTBS: Nid = Nid(ffi::NID_setct_AuthRevReqTBS);
    pub const SETCT_AUTHREVRESDATA: Nid = Nid(ffi::NID_setct_AuthRevResData);
    pub const SETCT_AUTHREVRESTBS: Nid = Nid(ffi::NID_setct_AuthRevResTBS);
    pub const SETCT_CAPREQTBS: Nid = Nid(ffi::NID_setct_CapReqTBS);
    pub const SETCT_CAPREQTBSX: Nid = Nid(ffi::NID_setct_CapReqTBSX);
    pub const SETCT_CAPRESDATA: Nid = Nid(ffi::NID_setct_CapResData);
    pub const SETCT_CAPREVREQTBS: Nid = Nid(ffi::NID_setct_CapRevReqTBS);
    pub const SETCT_CAPREVREQTBSX: Nid = Nid(ffi::NID_setct_CapRevReqTBSX);
    pub const SETCT_CAPREVRESDATA: Nid = Nid(ffi::NID_setct_CapRevResData);
    pub const SETCT_CREDREQTBS: Nid = Nid(ffi::NID_setct_CredReqTBS);
    pub const SETCT_CREDREQTBSX: Nid = Nid(ffi::NID_setct_CredReqTBSX);
    pub const SETCT_CREDRESDATA: Nid = Nid(ffi::NID_setct_CredResData);
    pub const SETCT_CREDREVREQTBS: Nid = Nid(ffi::NID_setct_CredRevReqTBS);
    pub const SETCT_CREDREVREQTBSX: Nid = Nid(ffi::NID_setct_CredRevReqTBSX);
    pub const SETCT_CREDREVRESDATA: Nid = Nid(ffi::NID_setct_CredRevResData);
    pub const SETCT_PCERTREQDATA: Nid = Nid(ffi::NID_setct_PCertReqData);
    pub const SETCT_PCERTRESTBS: Nid = Nid(ffi::NID_setct_PCertResTBS);
    pub const SETCT_BATCHADMINREQDATA: Nid = Nid(ffi::NID_setct_BatchAdminReqData);
    pub const SETCT_BATCHADMINRESDATA: Nid = Nid(ffi::NID_setct_BatchAdminResData);
    pub const SETCT_CARDCINITRESTBS: Nid = Nid(ffi::NID_setct_CardCInitResTBS);
    pub const SETCT_MEAQCINITRESTBS: Nid = Nid(ffi::NID_setct_MeAqCInitResTBS);
    pub const SETCT_REGFORMRESTBS: Nid = Nid(ffi::NID_setct_RegFormResTBS);
    pub const SETCT_CERTREQDATA: Nid = Nid(ffi::NID_setct_CertReqData);
    pub const SETCT_CERTREQTBS: Nid = Nid(ffi::NID_setct_CertReqTBS);
    pub const SETCT_CERTRESDATA: Nid = Nid(ffi::NID_setct_CertResData);
    pub const SETCT_CERTINQREQTBS: Nid = Nid(ffi::NID_setct_CertInqReqTBS);
    pub const SETCT_ERRORTBS: Nid = Nid(ffi::NID_setct_ErrorTBS);
    pub const SETCT_PIDUALSIGNEDTBE: Nid = Nid(ffi::NID_setct_PIDualSignedTBE);
    pub const SETCT_PIUNSIGNEDTBE: Nid = Nid(ffi::NID_setct_PIUnsignedTBE);
    pub const SETCT_AUTHREQTBE: Nid = Nid(ffi::NID_setct_AuthReqTBE);
    pub const SETCT_AUTHRESTBE: Nid = Nid(ffi::NID_setct_AuthResTBE);
    pub const SETCT_AUTHRESTBEX: Nid = Nid(ffi::NID_setct_AuthResTBEX);
    pub const SETCT_AUTHTOKENTBE: Nid = Nid(ffi::NID_setct_AuthTokenTBE);
    pub const SETCT_CAPTOKENTBE: Nid = Nid(ffi::NID_setct_CapTokenTBE);
    pub const SETCT_CAPTOKENTBEX: Nid = Nid(ffi::NID_setct_CapTokenTBEX);
    pub const SETCT_ACQCARDCODEMSGTBE: Nid = Nid(ffi::NID_setct_AcqCardCodeMsgTBE);
    pub const SETCT_AUTHREVREQTBE: Nid = Nid(ffi::NID_setct_AuthRevReqTBE);
    pub const SETCT_AUTHREVRESTBE: Nid = Nid(ffi::NID_setct_AuthRevResTBE);
    pub const SETCT_AUTHREVRESTBEB: Nid = Nid(ffi::NID_setct_AuthRevResTBEB);
    pub const SETCT_CAPREQTBE: Nid = Nid(ffi::NID_setct_CapReqTBE);
    pub const SETCT_CAPREQTBEX: Nid = Nid(ffi::NID_setct_CapReqTBEX);
    pub const SETCT_CAPRESTBE: Nid = Nid(ffi::NID_setct_CapResTBE);
    pub const SETCT_CAPREVREQTBE: Nid = Nid(ffi::NID_setct_CapRevReqTBE);
    pub const SETCT_CAPREVREQTBEX: Nid = Nid(ffi::NID_setct_CapRevReqTBEX);
    pub const SETCT_CAPREVRESTBE: Nid = Nid(ffi::NID_setct_CapRevResTBE);
    pub const SETCT_CREDREQTBE: Nid = Nid(ffi::NID_setct_CredReqTBE);
    pub const SETCT_CREDREQTBEX: Nid = Nid(ffi::NID_setct_CredReqTBEX);
    pub const SETCT_CREDRESTBE: Nid = Nid(ffi::NID_setct_CredResTBE);
    pub const SETCT_CREDREVREQTBE: Nid = Nid(ffi::NID_setct_CredRevReqTBE);
    pub const SETCT_CREDREVREQTBEX: Nid = Nid(ffi::NID_setct_CredRevReqTBEX);
    pub const SETCT_CREDREVRESTBE: Nid = Nid(ffi::NID_setct_CredRevResTBE);
    pub const SETCT_BATCHADMINREQTBE: Nid = Nid(ffi::NID_setct_BatchAdminReqTBE);
    pub const SETCT_BATCHADMINRESTBE: Nid = Nid(ffi::NID_setct_BatchAdminResTBE);
    pub const SETCT_REGFORMREQTBE: Nid = Nid(ffi::NID_setct_RegFormReqTBE);
    pub const SETCT_CERTREQTBE: Nid = Nid(ffi::NID_setct_CertReqTBE);
    pub const SETCT_CERTREQTBEX: Nid = Nid(ffi::NID_setct_CertReqTBEX);
    pub const SETCT_CERTRESTBE: Nid = Nid(ffi::NID_setct_CertResTBE);
    pub const SETCT_CRLNOTIFICATIONTBS: Nid = Nid(ffi::NID_setct_CRLNotificationTBS);
    pub const SETCT_CRLNOTIFICATIONRESTBS: Nid = Nid(ffi::NID_setct_CRLNotificationResTBS);
    pub const SETCT_BCIDISTRIBUTIONTBS: Nid = Nid(ffi::NID_setct_BCIDistributionTBS);
    pub const SETEXT_GENCRYPT: Nid = Nid(ffi::NID_setext_genCrypt);
    pub const SETEXT_MIAUTH: Nid = Nid(ffi::NID_setext_miAuth);
    pub const SETEXT_PINSECURE: Nid = Nid(ffi::NID_setext_pinSecure);
    pub const SETEXT_PINANY: Nid = Nid(ffi::NID_setext_pinAny);
    pub const SETEXT_TRACK2: Nid = Nid(ffi::NID_setext_track2);
    pub const SETEXT_CV: Nid = Nid(ffi::NID_setext_cv);
    pub const SET_POLICY_ROOT: Nid = Nid(ffi::NID_set_policy_root);
    pub const SETCEXT_HASHEDROOT: Nid = Nid(ffi::NID_setCext_hashedRoot);
    pub const SETCEXT_CERTTYPE: Nid = Nid(ffi::NID_setCext_certType);
    pub const SETCEXT_MERCHDATA: Nid = Nid(ffi::NID_setCext_merchData);
    pub const SETCEXT_CCERTREQUIRED: Nid = Nid(ffi::NID_setCext_cCertRequired);
    pub const SETCEXT_TUNNELING: Nid = Nid(ffi::NID_setCext_tunneling);
    pub const SETCEXT_SETEXT: Nid = Nid(ffi::NID_setCext_setExt);
    pub const SETCEXT_SETQUALF: Nid = Nid(ffi::NID_setCext_setQualf);
    pub const SETCEXT_PGWYCAPABILITIES: Nid = Nid(ffi::NID_setCext_PGWYcapabilities);
    pub const SETCEXT_TOKENIDENTIFIER: Nid = Nid(ffi::NID_setCext_TokenIdentifier);
    pub const SETCEXT_TRACK2DATA: Nid = Nid(ffi::NID_setCext_Track2Data);
    pub const SETCEXT_TOKENTYPE: Nid = Nid(ffi::NID_setCext_TokenType);
    pub const SETCEXT_ISSUERCAPABILITIES: Nid = Nid(ffi::NID_setCext_IssuerCapabilities);
    pub const SETATTR_CERT: Nid = Nid(ffi::NID_setAttr_Cert);
    pub const SETATTR_PGWYCAP: Nid = Nid(ffi::NID_setAttr_PGWYcap);
    pub const SETATTR_TOKENTYPE: Nid = Nid(ffi::NID_setAttr_TokenType);
    pub const SETATTR_ISSCAP: Nid = Nid(ffi::NID_setAttr_IssCap);
    pub const SET_ROOTKEYTHUMB: Nid = Nid(ffi::NID_set_rootKeyThumb);
    pub const SET_ADDPOLICY: Nid = Nid(ffi::NID_set_addPolicy);
    pub const SETATTR_TOKEN_EMV: Nid = Nid(ffi::NID_setAttr_Token_EMV);
    pub const SETATTR_TOKEN_B0PRIME: Nid = Nid(ffi::NID_setAttr_Token_B0Prime);
    pub const SETATTR_ISSCAP_CVM: Nid = Nid(ffi::NID_setAttr_IssCap_CVM);
    pub const SETATTR_ISSCAP_T2: Nid = Nid(ffi::NID_setAttr_IssCap_T2);
    pub const SETATTR_ISSCAP_SIG: Nid = Nid(ffi::NID_setAttr_IssCap_Sig);
    pub const SETATTR_GENCRYPTGRM: Nid = Nid(ffi::NID_setAttr_GenCryptgrm);
    pub const SETATTR_T2ENC: Nid = Nid(ffi::NID_setAttr_T2Enc);
    pub const SETATTR_T2CLEARTXT: Nid = Nid(ffi::NID_setAttr_T2cleartxt);
    pub const SETATTR_TOKICCSIG: Nid = Nid(ffi::NID_setAttr_TokICCsig);
    pub const SETATTR_SECDEVSIG: Nid = Nid(ffi::NID_setAttr_SecDevSig);
    pub const SET_BRAND_IATA_ATA: Nid = Nid(ffi::NID_set_brand_IATA_ATA);
    pub const SET_BRAND_DINERS: Nid = Nid(ffi::NID_set_brand_Diners);
    pub const SET_BRAND_AMERICANEXPRESS: Nid = Nid(ffi::NID_set_brand_AmericanExpress);
    pub const SET_BRAND_JCB: Nid = Nid(ffi::NID_set_brand_JCB);
    pub const SET_BRAND_VISA: Nid = Nid(ffi::NID_set_brand_Visa);
    pub const SET_BRAND_MASTERCARD: Nid = Nid(ffi::NID_set_brand_MasterCard);
    pub const SET_BRAND_NOVUS: Nid = Nid(ffi::NID_set_brand_Novus);
    pub const DES_CDMF: Nid = Nid(ffi::NID_des_cdmf);
    pub const RSAOAEPENCRYPTIONSET: Nid = Nid(ffi::NID_rsaOAEPEncryptionSET);
    pub const IPSEC3: Nid = Nid(ffi::NID_ipsec3);
    pub const IPSEC4: Nid = Nid(ffi::NID_ipsec4);
    pub const WHIRLPOOL: Nid = Nid(ffi::NID_whirlpool);
    pub const CRYPTOPRO: Nid = Nid(ffi::NID_cryptopro);
    pub const CRYPTOCOM: Nid = Nid(ffi::NID_cryptocom);
    pub const ID_GOSTR3411_94_WITH_GOSTR3410_2001: Nid =
        Nid(ffi::NID_id_GostR3411_94_with_GostR3410_2001);
    pub const ID_GOSTR3411_94_WITH_GOSTR3410_94: Nid =
        Nid(ffi::NID_id_GostR3411_94_with_GostR3410_94);
    pub const ID_GOSTR3411_94: Nid = Nid(ffi::NID_id_GostR3411_94);
    pub const ID_HMACGOSTR3411_94: Nid = Nid(ffi::NID_id_HMACGostR3411_94);
    pub const ID_GOSTR3410_2001: Nid = Nid(ffi::NID_id_GostR3410_2001);
    pub const ID_GOSTR3410_94: Nid = Nid(ffi::NID_id_GostR3410_94);
    pub const ID_GOST28147_89: Nid = Nid(ffi::NID_id_Gost28147_89);
    pub const GOST89_CNT: Nid = Nid(ffi::NID_gost89_cnt);
    pub const ID_GOST28147_89_MAC: Nid = Nid(ffi::NID_id_Gost28147_89_MAC);
    pub const ID_GOSTR3411_94_PRF: Nid = Nid(ffi::NID_id_GostR3411_94_prf);
    pub const ID_GOSTR3410_2001DH: Nid = Nid(ffi::NID_id_GostR3410_2001DH);
    pub const ID_GOSTR3410_94DH: Nid = Nid(ffi::NID_id_GostR3410_94DH);
    pub const ID_GOST28147_89_CRYPTOPRO_KEYMESHING: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_KeyMeshing);
    pub const ID_GOST28147_89_NONE_KEYMESHING: Nid = Nid(ffi::NID_id_Gost28147_89_None_KeyMeshing);
    pub const ID_GOSTR3411_94_TESTPARAMSET: Nid = Nid(ffi::NID_id_GostR3411_94_TestParamSet);
    pub const ID_GOSTR3411_94_CRYPTOPROPARAMSET: Nid =
        Nid(ffi::NID_id_GostR3411_94_CryptoProParamSet);
    pub const ID_GOST28147_89_TESTPARAMSET: Nid = Nid(ffi::NID_id_Gost28147_89_TestParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_A_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_A_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_B_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_B_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_C_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_C_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_D_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_D_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_OSCAR_1_1_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_Oscar_1_1_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_OSCAR_1_0_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_Oscar_1_0_ParamSet);
    pub const ID_GOST28147_89_CRYPTOPRO_RIC_1_PARAMSET: Nid =
        Nid(ffi::NID_id_Gost28147_89_CryptoPro_RIC_1_ParamSet);
    pub const ID_GOSTR3410_94_TESTPARAMSET: Nid = Nid(ffi::NID_id_GostR3410_94_TestParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_A_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_A_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_B_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_B_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_C_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_C_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_D_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_D_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_XCHA_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_XchA_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_XCHB_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_XchB_ParamSet);
    pub const ID_GOSTR3410_94_CRYPTOPRO_XCHC_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_94_CryptoPro_XchC_ParamSet);
    pub const ID_GOSTR3410_2001_TESTPARAMSET: Nid = Nid(ffi::NID_id_GostR3410_2001_TestParamSet);
    pub const ID_GOSTR3410_2001_CRYPTOPRO_A_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_2001_CryptoPro_A_ParamSet);
    pub const ID_GOSTR3410_2001_CRYPTOPRO_B_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_2001_CryptoPro_B_ParamSet);
    pub const ID_GOSTR3410_2001_CRYPTOPRO_C_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_2001_CryptoPro_C_ParamSet);
    pub const ID_GOSTR3410_2001_CRYPTOPRO_XCHA_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_2001_CryptoPro_XchA_ParamSet);
    pub const ID_GOSTR3410_2001_CRYPTOPRO_XCHB_PARAMSET: Nid =
        Nid(ffi::NID_id_GostR3410_2001_CryptoPro_XchB_ParamSet);
    pub const ID_GOSTR3410_94_A: Nid = Nid(ffi::NID_id_GostR3410_94_a);
    pub const ID_GOSTR3410_94_ABIS: Nid = Nid(ffi::NID_id_GostR3410_94_aBis);
    pub const ID_GOSTR3410_94_B: Nid = Nid(ffi::NID_id_GostR3410_94_b);
    pub const ID_GOSTR3410_94_BBIS: Nid = Nid(ffi::NID_id_GostR3410_94_bBis);
    pub const ID_GOST28147_89_CC: Nid = Nid(ffi::NID_id_Gost28147_89_cc);
    pub const ID_GOSTR3410_94_CC: Nid = Nid(ffi::NID_id_GostR3410_94_cc);
    pub const ID_GOSTR3410_2001_CC: Nid = Nid(ffi::NID_id_GostR3410_2001_cc);
    pub const ID_GOSTR3411_94_WITH_GOSTR3410_94_CC: Nid =
        Nid(ffi::NID_id_GostR3411_94_with_GostR3410_94_cc);
    pub const ID_GOSTR3411_94_WITH_GOSTR3410_2001_CC: Nid =
        Nid(ffi::NID_id_GostR3411_94_with_GostR3410_2001_cc);
    pub const ID_GOSTR3410_2001_PARAMSET_CC: Nid = Nid(ffi::NID_id_GostR3410_2001_ParamSet_cc);
    pub const CAMELLIA_128_CBC: Nid = Nid(ffi::NID_camellia_128_cbc);
    pub const CAMELLIA_192_CBC: Nid = Nid(ffi::NID_camellia_192_cbc);
    pub const CAMELLIA_256_CBC: Nid = Nid(ffi::NID_camellia_256_cbc);
    pub const ID_CAMELLIA128_WRAP: Nid = Nid(ffi::NID_id_camellia128_wrap);
    pub const ID_CAMELLIA192_WRAP: Nid = Nid(ffi::NID_id_camellia192_wrap);
    pub const ID_CAMELLIA256_WRAP: Nid = Nid(ffi::NID_id_camellia256_wrap);
    pub const CAMELLIA_128_ECB: Nid = Nid(ffi::NID_camellia_128_ecb);
    pub const CAMELLIA_128_OFB128: Nid = Nid(ffi::NID_camellia_128_ofb128);
    pub const CAMELLIA_128_CFB128: Nid = Nid(ffi::NID_camellia_128_cfb128);
    pub const CAMELLIA_192_ECB: Nid = Nid(ffi::NID_camellia_192_ecb);
    pub const CAMELLIA_192_OFB128: Nid = Nid(ffi::NID_camellia_192_ofb128);
    pub const CAMELLIA_192_CFB128: Nid = Nid(ffi::NID_camellia_192_cfb128);
    pub const CAMELLIA_256_ECB: Nid = Nid(ffi::NID_camellia_256_ecb);
    pub const CAMELLIA_256_OFB128: Nid = Nid(ffi::NID_camellia_256_ofb128);
    pub const CAMELLIA_256_CFB128: Nid = Nid(ffi::NID_camellia_256_cfb128);
    pub const CAMELLIA_128_CFB1: Nid = Nid(ffi::NID_camellia_128_cfb1);
    pub const CAMELLIA_192_CFB1: Nid = Nid(ffi::NID_camellia_192_cfb1);
    pub const CAMELLIA_256_CFB1: Nid = Nid(ffi::NID_camellia_256_cfb1);
    pub const CAMELLIA_128_CFB8: Nid = Nid(ffi::NID_camellia_128_cfb8);
    pub const CAMELLIA_192_CFB8: Nid = Nid(ffi::NID_camellia_192_cfb8);
    pub const CAMELLIA_256_CFB8: Nid = Nid(ffi::NID_camellia_256_cfb8);
    pub const KISA: Nid = Nid(ffi::NID_kisa);
    pub const SEED_ECB: Nid = Nid(ffi::NID_seed_ecb);
    pub const SEED_CBC: Nid = Nid(ffi::NID_seed_cbc);
    pub const SEED_CFB128: Nid = Nid(ffi::NID_seed_cfb128);
    pub const SEED_OFB128: Nid = Nid(ffi::NID_seed_ofb128);
    pub const HMAC: Nid = Nid(ffi::NID_hmac);
    pub const CMAC: Nid = Nid(ffi::NID_cmac);
    pub const RC4_HMAC_MD5: Nid = Nid(ffi::NID_rc4_hmac_md5);
    pub const AES_128_CBC_HMAC_SHA1: Nid = Nid(ffi::NID_aes_128_cbc_hmac_sha1);
    pub const AES_192_CBC_HMAC_SHA1: Nid = Nid(ffi::NID_aes_192_cbc_hmac_sha1);
    pub const AES_256_CBC_HMAC_SHA1: Nid = Nid(ffi::NID_aes_256_cbc_hmac_sha1);
    #[cfg(ossl111)]
    pub const SM2: Nid = Nid(ffi::NID_sm2);
    #[cfg(any(ossl111, libressl291))]
    pub const SM3: Nid = Nid(ffi::NID_sm3);
    #[cfg(any(ossl111, libressl380, awslc))]
    pub const SHA3_224: Nid = Nid(ffi::NID_sha3_224);
    #[cfg(any(ossl111, libressl380, awslc))]
    pub const SHA3_256: Nid = Nid(ffi::NID_sha3_256);
    #[cfg(any(ossl111, libressl380, awslc))]
    pub const SHA3_384: Nid = Nid(ffi::NID_sha3_384);
    #[cfg(any(ossl111, libressl380, awslc))]
    pub const SHA3_512: Nid = Nid(ffi::NID_sha3_512);
    #[cfg(any(ossl111, awslc))]
    pub const SHAKE128: Nid = Nid(ffi::NID_shake128);
    #[cfg(any(ossl111, awslc))]
    pub const SHAKE256: Nid = Nid(ffi::NID_shake256);
    #[cfg(any(ossl110, libressl271, awslc))]
    pub const CHACHA20_POLY1305: Nid = Nid(ffi::NID_chacha20_poly1305);
}

#[cfg(test)]
mod test {
    use super::Nid;

    #[test]
    fn signature_digest() {
        let algs = Nid::SHA256WITHRSAENCRYPTION.signature_algorithms().unwrap();
        assert_eq!(algs.digest, Nid::SHA256);
        assert_eq!(algs.pkey, Nid::RSAENCRYPTION);
    }

    #[test]
    fn test_long_name_conversion() {
        let common_name = Nid::COMMONNAME;
        let organizational_unit_name = Nid::ORGANIZATIONALUNITNAME;
        let aes256_cbc_hmac_sha1 = Nid::AES_256_CBC_HMAC_SHA1;
        let id_cmc_lrapopwitness = Nid::ID_CMC_LRAPOPWITNESS;
        let ms_ctl_sign = Nid::MS_CTL_SIGN;
        let undefined_nid = Nid::from_raw(118);

        assert_eq!(common_name.long_name().unwrap(), "commonName");
        assert_eq!(
            organizational_unit_name.long_name().unwrap(),
            "organizationalUnitName"
        );
        assert_eq!(
            aes256_cbc_hmac_sha1.long_name().unwrap(),
            "aes-256-cbc-hmac-sha1"
        );
        assert_eq!(
            id_cmc_lrapopwitness.long_name().unwrap(),
            "id-cmc-lraPOPWitness"
        );
        assert_eq!(
            ms_ctl_sign.long_name().unwrap(),
            "Microsoft Trust List Signing"
        );
        assert!(
            undefined_nid.long_name().is_err(),
            "undefined_nid should not return a valid value"
        );
    }

    #[test]
    fn test_short_name_conversion() {
        let common_name = Nid::COMMONNAME;
        let organizational_unit_name = Nid::ORGANIZATIONALUNITNAME;
        let aes256_cbc_hmac_sha1 = Nid::AES_256_CBC_HMAC_SHA1;
        let id_cmc_lrapopwitness = Nid::ID_CMC_LRAPOPWITNESS;
        let ms_ctl_sign = Nid::MS_CTL_SIGN;
        let undefined_nid = Nid::from_raw(118);

        assert_eq!(common_name.short_name().unwrap(), "CN");
        assert_eq!(organizational_unit_name.short_name().unwrap(), "OU");
        assert_eq!(
            aes256_cbc_hmac_sha1.short_name().unwrap(),
            "AES-256-CBC-HMAC-SHA1"
        );
        assert_eq!(
            id_cmc_lrapopwitness.short_name().unwrap(),
            "id-cmc-lraPOPWitness"
        );
        assert_eq!(ms_ctl_sign.short_name().unwrap(), "msCTLSign");
        assert!(
            undefined_nid.short_name().is_err(),
            "undefined_nid should not return a valid value"
        );
    }

    #[test]
    fn test_create() {
        let nid = Nid::create("1.2.3.4", "foo", "foobar").unwrap();
        assert_eq!(nid.short_name().unwrap(), "foo");
        assert_eq!(nid.long_name().unwrap(), "foobar");

        // Due to a bug in OpenSSL 3.1.0, this test crashes on Windows
        if !cfg!(ossl310) {
            let invalid_oid = Nid::create("invalid_oid", "invalid", "invalid");
            assert!(
                invalid_oid.is_err(),
                "invalid_oid should not return a valid value"
            );
        }
    }
}
