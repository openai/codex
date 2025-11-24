use bitflags::bitflags;
use foreign_types::{ForeignType, ForeignTypeRef};
use libc::c_int;
use std::mem;
use std::ptr;

use crate::asn1::Asn1ObjectRef;
use crate::bio::{MemBio, MemBioSlice};
use crate::error::ErrorStack;
use crate::nid::Nid;
use crate::pkey::{HasPrivate, PKeyRef};
use crate::stack::{Stack, StackRef, Stackable};
use crate::symm::Cipher;
use crate::util::ForeignTypeRefExt;
use crate::x509::store::X509StoreRef;
use crate::x509::{X509Ref, X509};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

foreign_type_and_impl_send_sync! {
    type CType = ffi::PKCS7_SIGNER_INFO;
    fn drop = ffi::PKCS7_SIGNER_INFO_free;

    pub struct Pkcs7SignerInfo;
    pub struct Pkcs7SignerInfoRef;
}

impl Stackable for Pkcs7SignerInfo {
    type StackType = ffi::stack_st_PKCS7_SIGNER_INFO;
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::PKCS7;
    fn drop = ffi::PKCS7_free;

    /// A PKCS#7 structure.
    ///
    /// Contains signed and/or encrypted data.
    pub struct Pkcs7;

    /// Reference to `Pkcs7`
    pub struct Pkcs7Ref;
}

foreign_type_and_impl_send_sync! {
    type CType = ffi::PKCS7_SIGNED;
    fn drop = ffi::PKCS7_SIGNED_free;

    /// A PKCS#7 signed data structure.
    ///
    /// Contains signed data.
    pub struct Pkcs7Signed;

    /// Reference to `Pkcs7Signed`
    pub struct Pkcs7SignedRef;
}

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct Pkcs7Flags: c_int {
        const TEXT = ffi::PKCS7_TEXT;
        const NOCERTS = ffi::PKCS7_NOCERTS;
        const NOSIGS = ffi::PKCS7_NOSIGS;
        const NOCHAIN = ffi::PKCS7_NOCHAIN;
        const NOINTERN = ffi::PKCS7_NOINTERN;
        const NOVERIFY = ffi::PKCS7_NOVERIFY;
        const DETACHED = ffi::PKCS7_DETACHED;
        const BINARY = ffi::PKCS7_BINARY;
        const NOATTR = ffi::PKCS7_NOATTR;
        const NOSMIMECAP = ffi::PKCS7_NOSMIMECAP;
        const NOOLDMIMETYPE = ffi::PKCS7_NOOLDMIMETYPE;
        const CRLFEOL = ffi::PKCS7_CRLFEOL;
        const STREAM = ffi::PKCS7_STREAM;
        const NOCRL = ffi::PKCS7_NOCRL;
        const PARTIAL = ffi::PKCS7_PARTIAL;
        const REUSE_DIGEST = ffi::PKCS7_REUSE_DIGEST;
        #[cfg(not(any(ossl101, ossl102, libressl)))]
        const NO_DUAL_CONTENT = ffi::PKCS7_NO_DUAL_CONTENT;
    }
}

impl Pkcs7 {
    from_pem! {
        /// Deserializes a PEM-encoded PKCS#7 signature
        ///
        /// The input should have a header of `-----BEGIN PKCS7-----`.
        #[corresponds(PEM_read_bio_PKCS7)]
        from_pem,
        Pkcs7,
        ffi::PEM_read_bio_PKCS7
    }

    from_der! {
        /// Deserializes a DER-encoded PKCS#7 signature
        #[corresponds(d2i_PKCS7)]
        from_der,
        Pkcs7,
        ffi::d2i_PKCS7
    }

    /// Parses a message in S/MIME format.
    ///
    /// Returns the loaded signature, along with the cleartext message (if
    /// available).
    #[corresponds(SMIME_read_PKCS7)]
    pub fn from_smime(input: &[u8]) -> Result<(Pkcs7, Option<Vec<u8>>), ErrorStack> {
        ffi::init();

        let input_bio = MemBioSlice::new(input)?;
        let mut bcont_bio = ptr::null_mut();
        unsafe {
            let pkcs7 =
                cvt_p(ffi::SMIME_read_PKCS7(input_bio.as_ptr(), &mut bcont_bio)).map(Pkcs7)?;
            let out = if !bcont_bio.is_null() {
                let bcont_bio = MemBio::from_ptr(bcont_bio);
                Some(bcont_bio.get_buf().to_vec())
            } else {
                None
            };
            Ok((pkcs7, out))
        }
    }

    /// Creates and returns a PKCS#7 `envelopedData` structure.
    ///
    /// `certs` is a list of recipient certificates. `input` is the content to be
    /// encrypted. `cipher` is the symmetric cipher to use. `flags` is an optional
    /// set of flags.
    #[corresponds(PKCS7_encrypt)]
    pub fn encrypt(
        certs: &StackRef<X509>,
        input: &[u8],
        cipher: Cipher,
        flags: Pkcs7Flags,
    ) -> Result<Pkcs7, ErrorStack> {
        let input_bio = MemBioSlice::new(input)?;

        unsafe {
            cvt_p(ffi::PKCS7_encrypt(
                certs.as_ptr(),
                input_bio.as_ptr(),
                cipher.as_ptr(),
                flags.bits(),
            ))
            .map(Pkcs7)
        }
    }

    /// Creates and returns a PKCS#7 `signedData` structure.
    ///
    /// `signcert` is the certificate to sign with, `pkey` is the corresponding
    /// private key. `certs` is an optional additional set of certificates to
    /// include in the PKCS#7 structure (for example any intermediate CAs in the
    /// chain).
    #[corresponds(PKCS7_sign)]
    pub fn sign<PT>(
        signcert: &X509Ref,
        pkey: &PKeyRef<PT>,
        certs: &StackRef<X509>,
        input: &[u8],
        flags: Pkcs7Flags,
    ) -> Result<Pkcs7, ErrorStack>
    where
        PT: HasPrivate,
    {
        let input_bio = MemBioSlice::new(input)?;
        unsafe {
            cvt_p(ffi::PKCS7_sign(
                signcert.as_ptr(),
                pkey.as_ptr(),
                certs.as_ptr(),
                input_bio.as_ptr(),
                flags.bits(),
            ))
            .map(Pkcs7)
        }
    }
}

impl Pkcs7Ref {
    /// Converts PKCS#7 structure to S/MIME format
    #[corresponds(SMIME_write_PKCS7)]
    pub fn to_smime(&self, input: &[u8], flags: Pkcs7Flags) -> Result<Vec<u8>, ErrorStack> {
        let input_bio = MemBioSlice::new(input)?;
        let output = MemBio::new()?;
        unsafe {
            cvt(ffi::SMIME_write_PKCS7(
                output.as_ptr(),
                self.as_ptr(),
                input_bio.as_ptr(),
                flags.bits(),
            ))
            .map(|_| output.get_buf().to_owned())
        }
    }

    to_pem! {
        /// Serializes the data into a PEM-encoded PKCS#7 structure.
        ///
        /// The output will have a header of `-----BEGIN PKCS7-----`.
        #[corresponds(PEM_write_bio_PKCS7)]
        to_pem,
        ffi::PEM_write_bio_PKCS7
    }

    to_der! {
        /// Serializes the data into a DER-encoded PKCS#7 structure.
        #[corresponds(i2d_PKCS7)]
        to_der,
        ffi::i2d_PKCS7
    }

    /// Decrypts data using the provided private key.
    ///
    /// `pkey` is the recipient's private key, and `cert` is the recipient's
    /// certificate.
    ///
    /// Returns the decrypted message.
    #[corresponds(PKCS7_decrypt)]
    pub fn decrypt<PT>(
        &self,
        pkey: &PKeyRef<PT>,
        cert: &X509Ref,
        flags: Pkcs7Flags,
    ) -> Result<Vec<u8>, ErrorStack>
    where
        PT: HasPrivate,
    {
        let output = MemBio::new()?;

        unsafe {
            cvt(ffi::PKCS7_decrypt(
                self.as_ptr(),
                pkey.as_ptr(),
                cert.as_ptr(),
                output.as_ptr(),
                flags.bits(),
            ))
            .map(|_| output.get_buf().to_owned())
        }
    }

    /// Verifies the PKCS#7 `signedData` structure contained by `&self`.
    ///
    /// `certs` is a set of certificates in which to search for the signer's
    /// certificate. `store` is a trusted certificate store (used for chain
    /// verification). `indata` is the signed data if the content is not present
    /// in `&self`. The content is written to `out` if it is not `None`.
    #[corresponds(PKCS7_verify)]
    pub fn verify(
        &self,
        certs: &StackRef<X509>,
        store: &X509StoreRef,
        indata: Option<&[u8]>,
        out: Option<&mut Vec<u8>>,
        flags: Pkcs7Flags,
    ) -> Result<(), ErrorStack> {
        let out_bio = MemBio::new()?;

        let indata_bio = match indata {
            Some(data) => Some(MemBioSlice::new(data)?),
            None => None,
        };
        let indata_bio_ptr = indata_bio.as_ref().map_or(ptr::null_mut(), |p| p.as_ptr());

        unsafe {
            cvt(ffi::PKCS7_verify(
                self.as_ptr(),
                certs.as_ptr(),
                store.as_ptr(),
                indata_bio_ptr,
                out_bio.as_ptr(),
                flags.bits(),
            ))
            .map(|_| ())?
        }

        if let Some(data) = out {
            data.clear();
            data.extend_from_slice(out_bio.get_buf());
        }

        Ok(())
    }

    /// Retrieve the signer's certificates from the PKCS#7 structure without verifying them.
    #[corresponds(PKCS7_get0_signers)]
    pub fn signers(
        &self,
        certs: &StackRef<X509>,
        flags: Pkcs7Flags,
    ) -> Result<Stack<X509>, ErrorStack> {
        unsafe {
            let ptr = cvt_p(ffi::PKCS7_get0_signers(
                self.as_ptr(),
                certs.as_ptr(),
                flags.bits(),
            ))?;

            // The returned stack is owned by the caller, but the certs inside are not! Our stack interface can't deal
            // with that, so instead we just manually bump the refcount of the certs so that the whole stack is properly
            // owned.
            let stack = Stack::<X509>::from_ptr(ptr);
            for cert in &stack {
                mem::forget(cert.to_owned());
            }

            Ok(stack)
        }
    }

    /// Return the type of a PKCS#7 structure as an Asn1Object
    pub fn type_(&self) -> Option<&Asn1ObjectRef> {
        unsafe {
            let ptr = (*self.as_ptr()).type_;
            Asn1ObjectRef::from_const_ptr_opt(ptr)
        }
    }

    /// Get the signed data of a PKCS#7 structure of type PKCS7_SIGNED
    pub fn signed(&self) -> Option<&Pkcs7SignedRef> {
        unsafe {
            if self.type_().map(|x| x.nid()) != Some(Nid::PKCS7_SIGNED) {
                return None;
            }
            let signed_data = (*self.as_ptr()).d.sign;
            Pkcs7SignedRef::from_const_ptr_opt(signed_data)
        }
    }
}

impl Pkcs7SignedRef {
    /// Get the stack of certificates from the PKCS7_SIGNED object
    pub fn certificates(&self) -> Option<&StackRef<X509>> {
        unsafe {
            self.as_ptr()
                .as_ref()
                .and_then(|x| x.cert.as_mut())
                .and_then(|x| StackRef::<X509>::from_const_ptr_opt(x))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::hash::MessageDigest;
    use crate::nid::Nid;
    use crate::pkcs7::{Pkcs7, Pkcs7Flags};
    use crate::pkey::PKey;
    use crate::stack::Stack;
    use crate::symm::Cipher;
    use crate::x509::store::X509StoreBuilder;
    use crate::x509::X509;

    #[test]
    fn encrypt_decrypt_test() {
        let cert = include_bytes!("../test/certs.pem");
        let cert = X509::from_pem(cert).unwrap();
        let mut certs = Stack::new().unwrap();
        certs.push(cert.clone()).unwrap();
        let message: String = String::from("foo");
        let cipher = Cipher::des_ede3_cbc();
        let flags = Pkcs7Flags::STREAM;
        let pkey = include_bytes!("../test/key.pem");
        let pkey = PKey::private_key_from_pem(pkey).unwrap();

        let pkcs7 =
            Pkcs7::encrypt(&certs, message.as_bytes(), cipher, flags).expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_ENVELOPED
        );

        let encrypted = pkcs7
            .to_smime(message.as_bytes(), flags)
            .expect("should succeed");

        let (pkcs7_decoded, _) = Pkcs7::from_smime(encrypted.as_slice()).expect("should succeed");

        let decoded = pkcs7_decoded
            .decrypt(&pkey, &cert, Pkcs7Flags::empty())
            .expect("should succeed");

        assert_eq!(decoded, message.into_bytes());
    }

    #[test]
    fn sign_verify_test_detached() {
        let cert = include_bytes!("../test/cert.pem");
        let cert = X509::from_pem(cert).unwrap();
        let certs = Stack::new().unwrap();
        let message = "foo";
        let flags = Pkcs7Flags::STREAM | Pkcs7Flags::DETACHED;
        let pkey = include_bytes!("../test/key.pem");
        let pkey = PKey::private_key_from_pem(pkey).unwrap();
        let mut store_builder = X509StoreBuilder::new().expect("should succeed");

        let root_ca = include_bytes!("../test/root-ca.pem");
        let root_ca = X509::from_pem(root_ca).unwrap();
        store_builder.add_cert(root_ca).expect("should succeed");

        let store = store_builder.build();

        let pkcs7 =
            Pkcs7::sign(&cert, &pkey, &certs, message.as_bytes(), flags).expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_SIGNED
        );

        let signed = pkcs7
            .to_smime(message.as_bytes(), flags)
            .expect("should succeed");
        println!("{:?}", String::from_utf8(signed.clone()).unwrap());
        let (pkcs7_decoded, content) =
            Pkcs7::from_smime(signed.as_slice()).expect("should succeed");

        let mut output = Vec::new();
        pkcs7_decoded
            .verify(
                &certs,
                &store,
                Some(message.as_bytes()),
                Some(&mut output),
                flags,
            )
            .expect("should succeed");

        assert_eq!(output, message.as_bytes());
        assert_eq!(content.expect("should be non-empty"), message.as_bytes());
    }

    /// https://marc.info/?l=openbsd-cvs&m=166602943014106&w=2
    #[test]
    #[cfg_attr(all(libressl360, not(libressl361)), ignore)]
    fn sign_verify_test_normal() {
        let cert = include_bytes!("../test/cert.pem");
        let cert = X509::from_pem(cert).unwrap();
        let certs = Stack::new().unwrap();
        let message = "foo";
        let flags = Pkcs7Flags::STREAM;
        let pkey = include_bytes!("../test/key.pem");
        let pkey = PKey::private_key_from_pem(pkey).unwrap();
        let mut store_builder = X509StoreBuilder::new().expect("should succeed");

        let root_ca = include_bytes!("../test/root-ca.pem");
        let root_ca = X509::from_pem(root_ca).unwrap();
        store_builder.add_cert(root_ca).expect("should succeed");

        let store = store_builder.build();

        let pkcs7 =
            Pkcs7::sign(&cert, &pkey, &certs, message.as_bytes(), flags).expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_SIGNED
        );

        let signed = pkcs7
            .to_smime(message.as_bytes(), flags)
            .expect("should succeed");

        let (pkcs7_decoded, content) =
            Pkcs7::from_smime(signed.as_slice()).expect("should succeed");

        let mut output = Vec::new();
        pkcs7_decoded
            .verify(&certs, &store, None, Some(&mut output), flags)
            .expect("should succeed");

        assert_eq!(output, message.as_bytes());
        assert!(content.is_none());
    }

    /// https://marc.info/?l=openbsd-cvs&m=166602943014106&w=2
    #[test]
    #[cfg_attr(all(libressl360, not(libressl361)), ignore)]
    fn signers() {
        let cert = include_bytes!("../test/cert.pem");
        let cert = X509::from_pem(cert).unwrap();
        let cert_digest = cert.digest(MessageDigest::sha256()).unwrap();
        let certs = Stack::new().unwrap();
        let message = "foo";
        let flags = Pkcs7Flags::STREAM;
        let pkey = include_bytes!("../test/key.pem");
        let pkey = PKey::private_key_from_pem(pkey).unwrap();
        let mut store_builder = X509StoreBuilder::new().expect("should succeed");

        let root_ca = include_bytes!("../test/root-ca.pem");
        let root_ca = X509::from_pem(root_ca).unwrap();
        store_builder.add_cert(root_ca).expect("should succeed");

        let pkcs7 =
            Pkcs7::sign(&cert, &pkey, &certs, message.as_bytes(), flags).expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_SIGNED
        );

        let signed = pkcs7
            .to_smime(message.as_bytes(), flags)
            .expect("should succeed");

        let (pkcs7_decoded, _) = Pkcs7::from_smime(signed.as_slice()).expect("should succeed");

        let empty_certs = Stack::new().unwrap();
        let signer_certs = pkcs7_decoded
            .signers(&empty_certs, flags)
            .expect("should succeed");
        assert_eq!(empty_certs.len(), 0);
        assert_eq!(signer_certs.len(), 1);
        let signer_digest = signer_certs[0].digest(MessageDigest::sha256()).unwrap();
        assert_eq!(*cert_digest, *signer_digest);
    }

    #[test]
    fn invalid_from_smime() {
        let input = String::from("Invalid SMIME Message");
        let result = Pkcs7::from_smime(input.as_bytes());

        assert!(result.is_err());
    }

    #[test]
    fn signed_data_certificates() {
        let cert = include_bytes!("../test/cert.pem");
        let cert = X509::from_pem(cert).unwrap();
        let mut extra_certs = Stack::<X509>::new().unwrap();
        for cert in
            X509::stack_from_pem(include_bytes!("../test/certs.pem")).expect("should succeed")
        {
            extra_certs.push(cert).expect("should succeed");
        }

        let message = "foo";
        let flags = Pkcs7Flags::STREAM;
        let pkey = include_bytes!("../test/key.pem");
        let pkey = PKey::private_key_from_pem(pkey).unwrap();

        let pkcs7 = Pkcs7::sign(&cert, &pkey, &extra_certs, message.as_bytes(), flags)
            .expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_SIGNED
        );
        let signed_data_certs = pkcs7.signed().and_then(|x| x.certificates());
        assert_eq!(signed_data_certs.expect("should succeed").len(), 3);
    }

    #[test]
    fn signed_data_certificates_no_signed_data() {
        let cert = include_bytes!("../test/certs.pem");
        let cert = X509::from_pem(cert).unwrap();
        let mut certs = Stack::new().unwrap();
        certs.push(cert).unwrap();
        let message: String = String::from("foo");
        let cipher = Cipher::des_ede3_cbc();
        let flags = Pkcs7Flags::STREAM;

        // Use `Pkcs7::encrypt` since it populates the PKCS7_ENVELOPE struct rather than
        // PKCS7_SIGNED
        let pkcs7 =
            Pkcs7::encrypt(&certs, message.as_bytes(), cipher, flags).expect("should succeed");
        assert_eq!(
            pkcs7.type_().expect("PKCS7 should have a type").nid(),
            Nid::PKCS7_ENVELOPED
        );

        let signed_data_certs = pkcs7.signed().and_then(|x| x.certificates());
        assert!(signed_data_certs.is_none())
    }
}
