use super::super::*;
use libc::*;

pub type pem_password_cb = Option<
    unsafe extern "C" fn(
        buf: *mut c_char,
        size: c_int,
        rwflag: c_int,
        user_data: *mut c_void,
    ) -> c_int,
>;

const_ptr_api! {
    extern "C" {
        pub fn PEM_write_bio_X509(bio: *mut BIO, x509: #[const_ptr_if(ossl300)] X509) -> c_int;
        pub fn PEM_write_bio_X509_REQ(bio: *mut BIO, x509: #[const_ptr_if(ossl300)] X509_REQ) -> c_int;
        pub fn PEM_write_bio_X509_CRL(bio: *mut BIO, x509: #[const_ptr_if(ossl300)] X509_CRL) -> c_int;
        pub fn PEM_write_bio_RSAPrivateKey(
            bp: *mut BIO,
            rsa: #[const_ptr_if(ossl300)] RSA,
            cipher: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_uchar,
            klen: c_int,
            callback: pem_password_cb,
            user_data: *mut c_void,
        ) -> c_int;
        pub fn PEM_write_bio_RSA_PUBKEY(bp: *mut BIO, rsa: #[const_ptr_if(ossl300)] RSA) -> c_int;
        pub fn PEM_write_bio_DSAPrivateKey(
            bp: *mut BIO,
            dsa: #[const_ptr_if(ossl300)] DSA,
            cipher: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_uchar,
            klen: c_int,
            callback: pem_password_cb,
            user_data: *mut c_void,
        ) -> c_int;
        pub fn PEM_write_bio_ECPrivateKey(
            bio: *mut BIO,
            key: #[const_ptr_if(ossl300)] EC_KEY,
            cipher: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_uchar,
            klen: c_int,
            callback: pem_password_cb,
            user_data: *mut c_void,
        ) -> c_int;
        pub fn PEM_write_bio_DSA_PUBKEY(bp: *mut BIO, dsa: #[const_ptr_if(ossl300)] DSA) -> c_int;
        pub fn PEM_write_bio_PrivateKey(
            bio: *mut BIO,
            pkey: #[const_ptr_if(ossl300)] EVP_PKEY,
            cipher: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_uchar,
            klen: c_int,
            callback: pem_password_cb,
            user_data: *mut c_void,
        ) -> c_int;
        pub fn PEM_write_bio_PUBKEY(bp: *mut BIO, x: #[const_ptr_if(ossl300)] EVP_PKEY) -> c_int;
        pub fn PEM_write_bio_PKCS8PrivateKey(
            bio: *mut BIO,
            pkey: #[const_ptr_if(ossl300)] EVP_PKEY,
            cipher: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_char,
            klen: c_int,
            callback: pem_password_cb,
            user_data: *mut c_void,
        ) -> c_int;
        pub fn PEM_write_bio_PKCS7(bp: *mut BIO, x: #[const_ptr_if(ossl300)] PKCS7) -> c_int;
        pub fn PEM_write_bio_EC_PUBKEY(bp: *mut BIO, ec: #[const_ptr_if(ossl300)] EC_KEY) -> c_int;
        pub fn i2d_PKCS8PrivateKey_bio(
            bp: *mut BIO,
            x: #[const_ptr_if(ossl300)] EVP_PKEY,
            enc: *const EVP_CIPHER,
            kstr: #[const_ptr_if(ossl300)] c_char,
            klen: c_int,
            cb: pem_password_cb,
            u: *mut c_void,
        ) -> c_int;
    }
}

extern "C" {
    pub fn PEM_read_bio_X509(
        bio: *mut BIO,
        out: *mut *mut X509,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut X509;
    pub fn PEM_read_bio_X509_REQ(
        bio: *mut BIO,
        out: *mut *mut X509_REQ,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut X509_REQ;
    pub fn PEM_read_bio_X509_CRL(
        bio: *mut BIO,
        out: *mut *mut X509_CRL,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut X509_CRL;
    pub fn PEM_read_bio_RSAPrivateKey(
        bio: *mut BIO,
        rsa: *mut *mut RSA,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut RSA;
    pub fn PEM_read_bio_RSAPublicKey(
        bio: *mut BIO,
        rsa: *mut *mut RSA,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut RSA;
    pub fn PEM_write_bio_RSAPublicKey(bp: *mut BIO, rsa: *const RSA) -> c_int;
    pub fn PEM_read_bio_RSA_PUBKEY(
        bio: *mut BIO,
        rsa: *mut *mut RSA,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut RSA;
    pub fn PEM_read_bio_DSAPrivateKey(
        bp: *mut BIO,
        dsa: *mut *mut DSA,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut DSA;
    pub fn PEM_read_bio_DSA_PUBKEY(
        bp: *mut BIO,
        dsa: *mut *mut DSA,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut DSA;
    pub fn PEM_read_bio_ECPrivateKey(
        bio: *mut BIO,
        key: *mut *mut EC_KEY,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut EC_KEY;
    pub fn PEM_read_bio_EC_PUBKEY(
        bp: *mut BIO,
        ec: *mut *mut EC_KEY,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut EC_KEY;
    pub fn PEM_read_bio_DHparams(
        bio: *mut BIO,
        out: *mut *mut DH,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut DH;
    pub fn PEM_write_bio_DHparams(bio: *mut BIO, x: *const DH) -> c_int;
    pub fn PEM_read_bio_PrivateKey(
        bio: *mut BIO,
        out: *mut *mut EVP_PKEY,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut EVP_PKEY;
    pub fn PEM_read_bio_PUBKEY(
        bio: *mut BIO,
        out: *mut *mut EVP_PKEY,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut EVP_PKEY;

    pub fn d2i_PKCS8PrivateKey_bio(
        bp: *mut BIO,
        x: *mut *mut EVP_PKEY,
        cb: pem_password_cb,
        u: *mut c_void,
    ) -> *mut EVP_PKEY;
    pub fn d2i_PKCS8_PRIV_KEY_INFO(
        k: *mut *mut PKCS8_PRIV_KEY_INFO,
        buf: *mut *const u8,
        length: c_long,
    ) -> *mut PKCS8_PRIV_KEY_INFO;
    pub fn PKCS8_PRIV_KEY_INFO_free(p8inf: *mut PKCS8_PRIV_KEY_INFO);

    pub fn PEM_read_bio_PKCS7(
        bio: *mut BIO,
        out: *mut *mut PKCS7,
        cb: pem_password_cb,
        u: *mut c_void,
    ) -> *mut PKCS7;

    #[cfg(ossl101)]
    pub fn PEM_read_bio_CMS(
        bio: *mut BIO,
        out: *mut *mut CMS_ContentInfo,
        callback: pem_password_cb,
        user_data: *mut c_void,
    ) -> *mut CMS_ContentInfo;
    #[cfg(ossl101)]
    pub fn PEM_write_bio_CMS(bio: *mut BIO, cms: *const CMS_ContentInfo) -> c_int;
}
