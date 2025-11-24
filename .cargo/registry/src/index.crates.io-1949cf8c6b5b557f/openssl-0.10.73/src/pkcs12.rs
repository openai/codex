//! PKCS #12 archives.

use foreign_types::{ForeignType, ForeignTypeRef};
use libc::c_int;
use std::ffi::CString;
use std::ptr;

use crate::error::ErrorStack;
#[cfg(not(boringssl))]
use crate::hash::MessageDigest;
use crate::nid::Nid;
use crate::pkey::{HasPrivate, PKey, PKeyRef, Private};
use crate::stack::Stack;
use crate::util::ForeignTypeExt;
use crate::x509::{X509Ref, X509};
use crate::{cvt, cvt_p};
use openssl_macros::corresponds;

foreign_type_and_impl_send_sync! {
    type CType = ffi::PKCS12;
    fn drop = ffi::PKCS12_free;

    pub struct Pkcs12;
    pub struct Pkcs12Ref;
}

impl Pkcs12Ref {
    to_der! {
        /// Serializes the `Pkcs12` to its standard DER encoding.
        #[corresponds(i2d_PKCS12)]
        to_der,
        ffi::i2d_PKCS12
    }

    /// Deprecated.
    #[deprecated(note = "Use parse2 instead", since = "0.10.46")]
    #[allow(deprecated)]
    pub fn parse(&self, pass: &str) -> Result<ParsedPkcs12, ErrorStack> {
        let parsed = self.parse2(pass)?;

        Ok(ParsedPkcs12 {
            pkey: parsed.pkey.unwrap(),
            cert: parsed.cert.unwrap(),
            chain: parsed.ca,
        })
    }

    /// Extracts the contents of the `Pkcs12`.
    #[corresponds(PKCS12_parse)]
    pub fn parse2(&self, pass: &str) -> Result<ParsedPkcs12_2, ErrorStack> {
        unsafe {
            let pass = CString::new(pass.as_bytes()).unwrap();

            let mut pkey = ptr::null_mut();
            let mut cert = ptr::null_mut();
            let mut ca = ptr::null_mut();

            cvt(ffi::PKCS12_parse(
                self.as_ptr(),
                pass.as_ptr(),
                &mut pkey,
                &mut cert,
                &mut ca,
            ))?;

            let pkey = PKey::from_ptr_opt(pkey);
            let cert = X509::from_ptr_opt(cert);
            let ca = Stack::from_ptr_opt(ca);

            Ok(ParsedPkcs12_2 { pkey, cert, ca })
        }
    }
}

impl Pkcs12 {
    from_der! {
        /// Deserializes a DER-encoded PKCS#12 archive.
        #[corresponds(d2i_PKCS12)]
        from_der,
        Pkcs12,
        ffi::d2i_PKCS12
    }

    /// Creates a new builder for a protected pkcs12 certificate.
    ///
    /// This uses the defaults from the OpenSSL library:
    ///
    /// * `nid_key` - `AES_256_CBC` (3.0.0+) or `PBE_WITHSHA1AND3_KEY_TRIPLEDES_CBC`
    /// * `nid_cert` - `AES_256_CBC` (3.0.0+) or `PBE_WITHSHA1AND40BITRC2_CBC`
    /// * `iter` - `2048`
    /// * `mac_iter` - `2048`
    /// * `mac_md` - `SHA-256` (3.0.0+) or `SHA-1` (`SHA-1` only for BoringSSL)
    pub fn builder() -> Pkcs12Builder {
        ffi::init();

        Pkcs12Builder {
            name: None,
            pkey: None,
            cert: None,
            ca: None,
            nid_key: Nid::UNDEF,
            nid_cert: Nid::UNDEF,
            iter: ffi::PKCS12_DEFAULT_ITER,
            mac_iter: ffi::PKCS12_DEFAULT_ITER,
            #[cfg(not(boringssl))]
            mac_md: None,
        }
    }
}

#[deprecated(note = "Use ParsedPkcs12_2 instead", since = "0.10.46")]
pub struct ParsedPkcs12 {
    pub pkey: PKey<Private>,
    pub cert: X509,
    pub chain: Option<Stack<X509>>,
}

pub struct ParsedPkcs12_2 {
    pub pkey: Option<PKey<Private>>,
    pub cert: Option<X509>,
    pub ca: Option<Stack<X509>>,
}

pub struct Pkcs12Builder {
    // FIXME borrow
    name: Option<CString>,
    pkey: Option<PKey<Private>>,
    cert: Option<X509>,
    ca: Option<Stack<X509>>,
    nid_key: Nid,
    nid_cert: Nid,
    iter: c_int,
    mac_iter: c_int,
    // FIXME remove
    #[cfg(not(boringssl))]
    mac_md: Option<MessageDigest>,
}

impl Pkcs12Builder {
    /// The `friendlyName` used for the certificate and private key.
    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = Some(CString::new(name).unwrap());
        self
    }

    /// The private key.
    pub fn pkey<T>(&mut self, pkey: &PKeyRef<T>) -> &mut Self
    where
        T: HasPrivate,
    {
        let new_pkey = unsafe { PKeyRef::from_ptr(pkey.as_ptr()) };
        self.pkey = Some(new_pkey.to_owned());
        self
    }

    /// The certificate.
    pub fn cert(&mut self, cert: &X509Ref) -> &mut Self {
        self.cert = Some(cert.to_owned());
        self
    }

    /// An additional set of certificates to include in the archive beyond the one provided to
    /// `build`.
    pub fn ca(&mut self, ca: Stack<X509>) -> &mut Self {
        self.ca = Some(ca);
        self
    }

    /// The encryption algorithm that should be used for the key
    pub fn key_algorithm(&mut self, nid: Nid) -> &mut Self {
        self.nid_key = nid;
        self
    }

    /// The encryption algorithm that should be used for the cert
    pub fn cert_algorithm(&mut self, nid: Nid) -> &mut Self {
        self.nid_cert = nid;
        self
    }

    /// Key iteration count, default is 2048 as of this writing
    pub fn key_iter(&mut self, iter: u32) -> &mut Self {
        self.iter = iter as c_int;
        self
    }

    /// MAC iteration count, default is the same as key_iter.
    ///
    /// Old implementations don't understand MAC iterations greater than 1, (pre 1.0.1?), if such
    /// compatibility is required this should be set to 1.
    pub fn mac_iter(&mut self, mac_iter: u32) -> &mut Self {
        self.mac_iter = mac_iter as c_int;
        self
    }

    /// MAC message digest type
    #[cfg(not(boringssl))]
    pub fn mac_md(&mut self, md: MessageDigest) -> &mut Self {
        self.mac_md = Some(md);
        self
    }

    /// Deprecated.
    #[deprecated(
        note = "Use Self::{name, pkey, cert, build2} instead.",
        since = "0.10.46"
    )]
    pub fn build<T>(
        mut self,
        password: &str,
        friendly_name: &str,
        pkey: &PKeyRef<T>,
        cert: &X509Ref,
    ) -> Result<Pkcs12, ErrorStack>
    where
        T: HasPrivate,
    {
        self.name(friendly_name)
            .pkey(pkey)
            .cert(cert)
            .build2(password)
    }

    /// Builds the PKCS#12 object.
    #[corresponds(PKCS12_create)]
    pub fn build2(&self, password: &str) -> Result<Pkcs12, ErrorStack> {
        unsafe {
            let pass = CString::new(password).unwrap();
            #[cfg(not(boringssl))]
            let pass_len = pass.as_bytes().len();
            let pass = pass.as_ptr();
            let friendly_name = self.name.as_ref().map_or(ptr::null(), |p| p.as_ptr());
            let pkey = self.pkey.as_ref().map_or(ptr::null(), |p| p.as_ptr());
            let cert = self.cert.as_ref().map_or(ptr::null(), |p| p.as_ptr());
            let ca = self
                .ca
                .as_ref()
                .map(|ca| ca.as_ptr())
                .unwrap_or(ptr::null_mut());
            let nid_key = self.nid_key.as_raw();
            let nid_cert = self.nid_cert.as_raw();

            // According to the OpenSSL docs, keytype is a non-standard extension for MSIE,
            // It's values are KEY_SIG or KEY_EX, see the OpenSSL docs for more information:
            // https://www.openssl.org/docs/manmaster/crypto/PKCS12_create.html
            let keytype = 0;

            let pkcs12 = cvt_p(ffi::PKCS12_create(
                pass as *mut _,
                friendly_name as *mut _,
                pkey as *mut _,
                cert as *mut _,
                ca,
                nid_key,
                nid_cert,
                self.iter,
                self.mac_iter,
                keytype,
            ))
            .map(Pkcs12)?;

            #[cfg(not(boringssl))]
            // BoringSSL does not support overriding the MAC and will always
            // use SHA-1.
            {
                let md_type = self
                    .mac_md
                    .map(|md_type| md_type.as_ptr())
                    .unwrap_or(ptr::null());

                cvt(ffi::PKCS12_set_mac(
                    pkcs12.as_ptr(),
                    pass,
                    pass_len.try_into().unwrap(),
                    ptr::null_mut(),
                    0,
                    self.mac_iter,
                    md_type,
                ))?;
            }

            Ok(pkcs12)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::asn1::Asn1Time;
    use crate::hash::MessageDigest;
    use crate::nid::Nid;
    use crate::pkey::PKey;
    use crate::rsa::Rsa;
    use crate::x509::extension::KeyUsage;
    use crate::x509::{X509Name, X509};

    use super::*;

    #[test]
    fn parse() {
        #[cfg(ossl300)]
        let _provider = crate::provider::Provider::try_load(None, "legacy", true).unwrap();

        let der = include_bytes!("../test/identity.p12");
        let pkcs12 = Pkcs12::from_der(der).unwrap();
        let parsed = pkcs12.parse2("mypass").unwrap();

        assert_eq!(
            hex::encode(
                parsed
                    .cert
                    .as_ref()
                    .unwrap()
                    .digest(MessageDigest::sha1())
                    .unwrap()
            ),
            "59172d9313e84459bcff27f967e79e6e9217e584"
        );
        assert_eq!(
            parsed.cert.as_ref().unwrap().alias(),
            Some(b"foobar.com" as &[u8])
        );

        let chain = parsed.ca.unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(
            hex::encode(chain[0].digest(MessageDigest::sha1()).unwrap()),
            "c0cbdf7cdd03c9773e5468e1f6d2da7d5cbb1875"
        );
    }

    #[test]
    fn parse_empty_chain() {
        #[cfg(ossl300)]
        let _provider = crate::provider::Provider::try_load(None, "legacy", true).unwrap();

        let der = include_bytes!("../test/keystore-empty-chain.p12");
        let pkcs12 = Pkcs12::from_der(der).unwrap();
        let parsed = pkcs12.parse2("cassandra").unwrap();
        if let Some(stack) = parsed.ca {
            assert_eq!(stack.len(), 0);
        }
    }

    #[test]
    fn create() {
        let subject_name = "ns.example.com";
        let rsa = Rsa::generate(2048).unwrap();
        let pkey = PKey::from_rsa(rsa).unwrap();

        let mut name = X509Name::builder().unwrap();
        name.append_entry_by_nid(Nid::COMMONNAME, subject_name)
            .unwrap();
        let name = name.build();

        let key_usage = KeyUsage::new().digital_signature().build().unwrap();

        let mut builder = X509::builder().unwrap();
        builder.set_version(2).unwrap();
        builder
            .set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        builder
            .set_not_after(&Asn1Time::days_from_now(365).unwrap())
            .unwrap();
        builder.set_subject_name(&name).unwrap();
        builder.set_issuer_name(&name).unwrap();
        builder.append_extension(key_usage).unwrap();
        builder.set_pubkey(&pkey).unwrap();
        builder.sign(&pkey, MessageDigest::sha256()).unwrap();
        let cert = builder.build();

        let pkcs12 = Pkcs12::builder()
            .name(subject_name)
            .pkey(&pkey)
            .cert(&cert)
            .build2("mypass")
            .unwrap();
        let der = pkcs12.to_der().unwrap();

        let pkcs12 = Pkcs12::from_der(&der).unwrap();
        let parsed = pkcs12.parse2("mypass").unwrap();

        assert_eq!(
            &*parsed.cert.unwrap().digest(MessageDigest::sha1()).unwrap(),
            &*cert.digest(MessageDigest::sha1()).unwrap()
        );
        assert!(parsed.pkey.unwrap().public_eq(&pkey));
    }

    #[test]
    fn create_only_ca() {
        let ca = include_bytes!("../test/root-ca.pem");
        let ca = X509::from_pem(ca).unwrap();
        let mut chain = Stack::new().unwrap();
        chain.push(ca).unwrap();

        let pkcs12 = Pkcs12::builder().ca(chain).build2("hunter2").unwrap();
        let parsed = pkcs12.parse2("hunter2").unwrap();

        assert!(parsed.cert.is_none());
        assert!(parsed.pkey.is_none());
        assert_eq!(parsed.ca.unwrap().len(), 1);
    }
}
