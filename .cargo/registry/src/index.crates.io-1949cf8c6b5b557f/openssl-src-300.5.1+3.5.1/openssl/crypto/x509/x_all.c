/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/buffer.h>
#include <openssl/asn1.h>
#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/x509_acert.h>
#include <openssl/http.h>
#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include <openssl/x509v3.h>
#include "internal/asn1.h"
#include "crypto/pkcs7.h"
#include "crypto/x509.h"
#include "crypto/x509_acert.h"
#include "crypto/rsa.h"

int X509_verify(X509 *a, EVP_PKEY *r)
{
    if (X509_ALGOR_cmp(&a->sig_alg, &a->cert_info.signature) != 0)
        return 0;

    return ASN1_item_verify_ex(ASN1_ITEM_rptr(X509_CINF), &a->sig_alg,
                               &a->signature, &a->cert_info,
                               a->distinguishing_id, r, a->libctx, a->propq);
}

int X509_REQ_verify_ex(X509_REQ *a, EVP_PKEY *r, OSSL_LIB_CTX *libctx,
                       const char *propq)
{
    if (X509_REQ_get_version(a) != X509_REQ_VERSION_1) {
        ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_VERSION);
        return -1;
    }

    return ASN1_item_verify_ex(ASN1_ITEM_rptr(X509_REQ_INFO), &a->sig_alg,
                               a->signature, &a->req_info, a->distinguishing_id,
                               r, libctx, propq);
}

int X509_REQ_verify(X509_REQ *a, EVP_PKEY *r)
{
    return X509_REQ_verify_ex(a, r, NULL, NULL);
}

int X509_ACERT_verify(X509_ACERT *a, EVP_PKEY *r)
{
    if (X509_ALGOR_cmp(&a->sig_alg, &a->acinfo->signature) != 0)
        return 0;

    return ASN1_item_verify_ex(ASN1_ITEM_rptr(X509_ACERT_INFO), &a->sig_alg,
                               &a->signature, a->acinfo,
                               NULL, r, NULL, NULL);
}

int NETSCAPE_SPKI_verify(NETSCAPE_SPKI *a, EVP_PKEY *r)
{
    return ASN1_item_verify(ASN1_ITEM_rptr(NETSCAPE_SPKAC),
                            &a->sig_algor, a->signature, a->spkac, r);
}

int X509_sign(X509 *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (sk_X509_EXTENSION_num(X509_get0_extensions(x)) > 0
            && !X509_set_version(x, X509_VERSION_3))
        return 0;

    /*
     * Setting the modified flag before signing it. This makes the cached
     * encoding to be ignored, so even if the certificate fields have changed,
     * they are signed correctly.
     * The X509_sign_ctx, X509_REQ_sign{,_ctx}, X509_CRL_sign{,_ctx} functions
     * which exist below are the same.
     */
    x->cert_info.enc.modified = 1;
    return ASN1_item_sign_ex(ASN1_ITEM_rptr(X509_CINF), &x->cert_info.signature,
                             &x->sig_alg, &x->signature, &x->cert_info, NULL,
                             pkey, md, x->libctx, x->propq);
}

int X509_sign_ctx(X509 *x, EVP_MD_CTX *ctx)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (sk_X509_EXTENSION_num(X509_get0_extensions(x)) > 0
            && !X509_set_version(x, X509_VERSION_3))
        return 0;
    x->cert_info.enc.modified = 1;
    return ASN1_item_sign_ctx(ASN1_ITEM_rptr(X509_CINF),
                              &x->cert_info.signature,
                              &x->sig_alg, &x->signature, &x->cert_info, ctx);
}

static ASN1_VALUE *simple_get_asn1(const char *url, BIO *bio, BIO *rbio,
                                   int timeout, const ASN1_ITEM *it)
{
#ifndef OPENSSL_NO_HTTP
    size_t max_resp_len = (it == ASN1_ITEM_rptr(X509_CRL)) ?
        OSSL_HTTP_DEFAULT_MAX_CRL_LEN : OSSL_HTTP_DEFAULT_MAX_RESP_LEN;
    BIO *mem = OSSL_HTTP_get(url, NULL /* proxy */, NULL /* no_proxy */,
                             bio, rbio, NULL /* cb */, NULL /* arg */,
                             1024 /* buf_size */, NULL /* headers */,
                             NULL /* expected_ct */, 1 /* expect_asn1 */,
                             max_resp_len, timeout);
    ASN1_VALUE *res = ASN1_item_d2i_bio(it, mem, NULL);

    BIO_free(mem);
    return res;
#else
    return 0;
#endif
}

X509 *X509_load_http(const char *url, BIO *bio, BIO *rbio, int timeout)
{
    return (X509 *)simple_get_asn1(url, bio, rbio, timeout,
                                   ASN1_ITEM_rptr(X509));
}

int X509_REQ_sign(X509_REQ *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    x->req_info.enc.modified = 1;
    return ASN1_item_sign_ex(ASN1_ITEM_rptr(X509_REQ_INFO), &x->sig_alg, NULL,
                             x->signature, &x->req_info, NULL,
                             pkey, md, x->libctx, x->propq);
}

int X509_REQ_sign_ctx(X509_REQ *x, EVP_MD_CTX *ctx)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    x->req_info.enc.modified = 1;
    return ASN1_item_sign_ctx(ASN1_ITEM_rptr(X509_REQ_INFO),
                              &x->sig_alg, NULL, x->signature, &x->req_info,
                              ctx);
}

int X509_CRL_sign(X509_CRL *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    x->crl.enc.modified = 1;
    return ASN1_item_sign_ex(ASN1_ITEM_rptr(X509_CRL_INFO), &x->crl.sig_alg,
                             &x->sig_alg, &x->signature, &x->crl, NULL,
                             pkey, md, x->libctx, x->propq);
}

int X509_CRL_sign_ctx(X509_CRL *x, EVP_MD_CTX *ctx)
{
    if (x == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    x->crl.enc.modified = 1;
    return ASN1_item_sign_ctx(ASN1_ITEM_rptr(X509_CRL_INFO),
                              &x->crl.sig_alg, &x->sig_alg, &x->signature,
                              &x->crl, ctx);
}

X509_CRL *X509_CRL_load_http(const char *url, BIO *bio, BIO *rbio, int timeout)
{
    return (X509_CRL *)simple_get_asn1(url, bio, rbio, timeout,
                                       ASN1_ITEM_rptr(X509_CRL));
}

int X509_ACERT_sign(X509_ACERT *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    return ASN1_item_sign_ex(ASN1_ITEM_rptr(X509_ACERT_INFO), &x->sig_alg,
                             &x->acinfo->signature,
                             &x->signature, x->acinfo, NULL,
                             pkey, md, NULL, NULL);
}

int X509_ACERT_sign_ctx(X509_ACERT *x, EVP_MD_CTX *ctx)
{
    return ASN1_item_sign_ctx(ASN1_ITEM_rptr(X509_ACERT_INFO),
                              &x->sig_alg, &x->acinfo->signature, &x->signature,
                              x->acinfo, ctx);
}

int NETSCAPE_SPKI_sign(NETSCAPE_SPKI *x, EVP_PKEY *pkey, const EVP_MD *md)
{
    return
        ASN1_item_sign_ex(ASN1_ITEM_rptr(NETSCAPE_SPKAC), &x->sig_algor, NULL,
                          x->signature, x->spkac, NULL, pkey, md, NULL, NULL);
}

#ifndef OPENSSL_NO_STDIO
X509 *d2i_X509_fp(FILE *fp, X509 **x509)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(X509), fp, x509);
}

int i2d_X509_fp(FILE *fp, const X509 *x509)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(X509), fp, x509);
}
#endif

X509 *d2i_X509_bio(BIO *bp, X509 **x509)
{
    return ASN1_item_d2i_bio(ASN1_ITEM_rptr(X509), bp, x509);
}

int i2d_X509_bio(BIO *bp, const X509 *x509)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(X509), bp, x509);
}

#ifndef OPENSSL_NO_STDIO
X509_CRL *d2i_X509_CRL_fp(FILE *fp, X509_CRL **crl)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(X509_CRL), fp, crl);
}

int i2d_X509_CRL_fp(FILE *fp, const X509_CRL *crl)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(X509_CRL), fp, crl);
}
#endif

X509_CRL *d2i_X509_CRL_bio(BIO *bp, X509_CRL **crl)
{
    return ASN1_item_d2i_bio(ASN1_ITEM_rptr(X509_CRL), bp, crl);
}

int i2d_X509_CRL_bio(BIO *bp, const X509_CRL *crl)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(X509_CRL), bp, crl);
}

#ifndef OPENSSL_NO_STDIO
PKCS7 *d2i_PKCS7_fp(FILE *fp, PKCS7 **p7)
{
    PKCS7 *ret;
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (p7 != NULL && *p7 != NULL) {
        libctx = (*p7)->ctx.libctx;
        propq = (*p7)->ctx.propq;
    }

    ret = ASN1_item_d2i_fp_ex(ASN1_ITEM_rptr(PKCS7), fp, p7, libctx, propq);
    if (ret != NULL)
        ossl_pkcs7_resolve_libctx(ret);
    return ret;
}

int i2d_PKCS7_fp(FILE *fp, const PKCS7 *p7)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(PKCS7), fp, p7);
}
#endif

PKCS7 *d2i_PKCS7_bio(BIO *bp, PKCS7 **p7)
{
    PKCS7 *ret;
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (p7 != NULL && *p7 != NULL) {
        libctx = (*p7)->ctx.libctx;
        propq = (*p7)->ctx.propq;
    }

    ret = ASN1_item_d2i_bio_ex(ASN1_ITEM_rptr(PKCS7), bp, p7, libctx, propq);
    if (ret != NULL)
        ossl_pkcs7_resolve_libctx(ret);
    return ret;
}

int i2d_PKCS7_bio(BIO *bp, const PKCS7 *p7)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(PKCS7), bp, p7);
}

#ifndef OPENSSL_NO_STDIO
X509_REQ *d2i_X509_REQ_fp(FILE *fp, X509_REQ **req)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(X509_REQ), fp, req);
}

int i2d_X509_REQ_fp(FILE *fp, const X509_REQ *req)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(X509_REQ), fp, req);
}
#endif

X509_REQ *d2i_X509_REQ_bio(BIO *bp, X509_REQ **req)
{
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (req != NULL && *req != NULL) {
        libctx = (*req)->libctx;
        propq = (*req)->propq;
    }

    return
        ASN1_item_d2i_bio_ex(ASN1_ITEM_rptr(X509_REQ), bp, req, libctx, propq);
}

int i2d_X509_REQ_bio(BIO *bp, const X509_REQ *req)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(X509_REQ), bp, req);
}

#ifndef OPENSSL_NO_STDIO
RSA *d2i_RSAPrivateKey_fp(FILE *fp, RSA **rsa)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(RSAPrivateKey), fp, rsa);
}

int i2d_RSAPrivateKey_fp(FILE *fp, const RSA *rsa)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(RSAPrivateKey), fp, rsa);
}

RSA *d2i_RSAPublicKey_fp(FILE *fp, RSA **rsa)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(RSAPublicKey), fp, rsa);
}

RSA *d2i_RSA_PUBKEY_fp(FILE *fp, RSA **rsa)
{
    return ASN1_d2i_fp((void *(*)(void))
                       RSA_new, (D2I_OF(void)) d2i_RSA_PUBKEY, fp,
                       (void **)rsa);
}

int i2d_RSAPublicKey_fp(FILE *fp, const RSA *rsa)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(RSAPublicKey), fp, rsa);
}

int i2d_RSA_PUBKEY_fp(FILE *fp, const RSA *rsa)
{
    return ASN1_i2d_fp((I2D_OF(void))i2d_RSA_PUBKEY, fp, rsa);
}
#endif

RSA *d2i_RSAPrivateKey_bio(BIO *bp, RSA **rsa)
{
    return ASN1_item_d2i_bio(ASN1_ITEM_rptr(RSAPrivateKey), bp, rsa);
}

int i2d_RSAPrivateKey_bio(BIO *bp, const RSA *rsa)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(RSAPrivateKey), bp, rsa);
}

RSA *d2i_RSAPublicKey_bio(BIO *bp, RSA **rsa)
{
    return ASN1_item_d2i_bio(ASN1_ITEM_rptr(RSAPublicKey), bp, rsa);
}

RSA *d2i_RSA_PUBKEY_bio(BIO *bp, RSA **rsa)
{
    return ASN1_d2i_bio_of(RSA, RSA_new, d2i_RSA_PUBKEY, bp, rsa);
}

int i2d_RSAPublicKey_bio(BIO *bp, const RSA *rsa)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(RSAPublicKey), bp, rsa);
}

int i2d_RSA_PUBKEY_bio(BIO *bp, const RSA *rsa)
{
    return ASN1_i2d_bio_of(RSA, i2d_RSA_PUBKEY, bp, rsa);
}

#ifndef OPENSSL_NO_DSA
# ifndef OPENSSL_NO_STDIO
DSA *d2i_DSAPrivateKey_fp(FILE *fp, DSA **dsa)
{
    return ASN1_d2i_fp_of(DSA, DSA_new, d2i_DSAPrivateKey, fp, dsa);
}

int i2d_DSAPrivateKey_fp(FILE *fp, const DSA *dsa)
{
    return ASN1_i2d_fp_of(DSA, i2d_DSAPrivateKey, fp, dsa);
}

DSA *d2i_DSA_PUBKEY_fp(FILE *fp, DSA **dsa)
{
    return ASN1_d2i_fp_of(DSA, DSA_new, d2i_DSA_PUBKEY, fp, dsa);
}

int i2d_DSA_PUBKEY_fp(FILE *fp, const DSA *dsa)
{
    return ASN1_i2d_fp_of(DSA, i2d_DSA_PUBKEY, fp, dsa);
}
# endif

DSA *d2i_DSAPrivateKey_bio(BIO *bp, DSA **dsa)
{
    return ASN1_d2i_bio_of(DSA, DSA_new, d2i_DSAPrivateKey, bp, dsa);
}

int i2d_DSAPrivateKey_bio(BIO *bp, const DSA *dsa)
{
    return ASN1_i2d_bio_of(DSA, i2d_DSAPrivateKey, bp, dsa);
}

DSA *d2i_DSA_PUBKEY_bio(BIO *bp, DSA **dsa)
{
    return ASN1_d2i_bio_of(DSA, DSA_new, d2i_DSA_PUBKEY, bp, dsa);
}

int i2d_DSA_PUBKEY_bio(BIO *bp, const DSA *dsa)
{
    return ASN1_i2d_bio_of(DSA, i2d_DSA_PUBKEY, bp, dsa);
}

#endif

#ifndef OPENSSL_NO_EC
# ifndef OPENSSL_NO_STDIO
EC_KEY *d2i_EC_PUBKEY_fp(FILE *fp, EC_KEY **eckey)
{
    return ASN1_d2i_fp_of(EC_KEY, EC_KEY_new, d2i_EC_PUBKEY, fp, eckey);
}

int i2d_EC_PUBKEY_fp(FILE *fp, const EC_KEY *eckey)
{
    return ASN1_i2d_fp_of(EC_KEY, i2d_EC_PUBKEY, fp, eckey);
}

EC_KEY *d2i_ECPrivateKey_fp(FILE *fp, EC_KEY **eckey)
{
    return ASN1_d2i_fp_of(EC_KEY, EC_KEY_new, d2i_ECPrivateKey, fp, eckey);
}

int i2d_ECPrivateKey_fp(FILE *fp, const EC_KEY *eckey)
{
    return ASN1_i2d_fp_of(EC_KEY, i2d_ECPrivateKey, fp, eckey);
}
# endif
EC_KEY *d2i_EC_PUBKEY_bio(BIO *bp, EC_KEY **eckey)
{
    return ASN1_d2i_bio_of(EC_KEY, EC_KEY_new, d2i_EC_PUBKEY, bp, eckey);
}

int i2d_EC_PUBKEY_bio(BIO *bp, const EC_KEY *ecdsa)
{
    return ASN1_i2d_bio_of(EC_KEY, i2d_EC_PUBKEY, bp, ecdsa);
}

EC_KEY *d2i_ECPrivateKey_bio(BIO *bp, EC_KEY **eckey)
{
    return ASN1_d2i_bio_of(EC_KEY, EC_KEY_new, d2i_ECPrivateKey, bp, eckey);
}

int i2d_ECPrivateKey_bio(BIO *bp, const EC_KEY *eckey)
{
    return ASN1_i2d_bio_of(EC_KEY, i2d_ECPrivateKey, bp, eckey);
}
#endif

int X509_pubkey_digest(const X509 *data, const EVP_MD *type,
                       unsigned char *md, unsigned int *len)
{
    ASN1_BIT_STRING *key = X509_get0_pubkey_bitstr(data);

    if (key == NULL)
        return 0;
    return EVP_Digest(key->data, key->length, md, len, type, NULL);
}

int X509_digest(const X509 *cert, const EVP_MD *md, unsigned char *data,
                unsigned int *len)
{
    if (EVP_MD_is_a(md, SN_sha1) && (cert->ex_flags & EXFLAG_SET) != 0
            && (cert->ex_flags & EXFLAG_NO_FINGERPRINT) == 0) {
        /* Asking for SHA1 and we already computed it. */
        if (len != NULL)
            *len = sizeof(cert->sha1_hash);
        memcpy(data, cert->sha1_hash, sizeof(cert->sha1_hash));
        return 1;
    }
    return ossl_asn1_item_digest_ex(ASN1_ITEM_rptr(X509), md, (char *)cert,
                                    data, len, cert->libctx, cert->propq);
}

/* calculate cert digest using the same hash algorithm as in its signature */
ASN1_OCTET_STRING *X509_digest_sig(const X509 *cert,
                                   EVP_MD **md_used, int *md_is_fallback)
{
    unsigned int len;
    unsigned char hash[EVP_MAX_MD_SIZE];
    int mdnid, pknid;
    EVP_MD *md = NULL;
    const char *md_name;
    ASN1_OCTET_STRING *new;

    if (md_used != NULL)
        *md_used = NULL;
    if (md_is_fallback != NULL)
        *md_is_fallback = 0;

    if (cert == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (!OBJ_find_sigid_algs(X509_get_signature_nid(cert), &mdnid, &pknid)) {
        ERR_raise(ERR_LIB_X509, X509_R_UNKNOWN_SIGID_ALGS);
        return NULL;
    }

    if (mdnid == NID_undef) {
        if (pknid == EVP_PKEY_RSA_PSS) {
            RSA_PSS_PARAMS *pss = ossl_rsa_pss_decode(&cert->sig_alg);
            const EVP_MD *mgf1md, *mmd = NULL;
            int saltlen, trailerfield;

            if (pss == NULL
                || !ossl_rsa_pss_get_param_unverified(pss, &mmd, &mgf1md,
                                                      &saltlen,
                                                      &trailerfield)
                || mmd == NULL) {
                RSA_PSS_PARAMS_free(pss);
                ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_ALGORITHM);
                return NULL;
            }
            RSA_PSS_PARAMS_free(pss);
            /* Fetch explicitly and do not fallback */
            if ((md = EVP_MD_fetch(cert->libctx, EVP_MD_get0_name(mmd),
                                   cert->propq)) == NULL)
                /* Error code from fetch is sufficient */
                return NULL;
        } else if (pknid != NID_undef) {
            /* A known algorithm, but without a digest */
            switch (pknid) {
            case NID_ED25519: /* Follow CMS default given in RFC8419 */
                md_name = "SHA512";
                break;
            case NID_ED448: /* Follow CMS default given in RFC8419 */
                md_name = "SHAKE256";
                break;
            default: /* Fall back to SHA-256 */
                md_name = "SHA256";
                break;
            }
            if ((md = EVP_MD_fetch(cert->libctx, md_name,
                                   cert->propq)) == NULL)
                return NULL;
            if (md_is_fallback != NULL)
                *md_is_fallback = 1;
        } else {
            /* A completely unknown algorithm */
            ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_ALGORITHM);
            return NULL;
        }
    } else if ((md = EVP_MD_fetch(cert->libctx, OBJ_nid2sn(mdnid),
                                  cert->propq)) == NULL
               && (md = (EVP_MD *)EVP_get_digestbynid(mdnid)) == NULL) {
        ERR_raise(ERR_LIB_X509, X509_R_UNSUPPORTED_ALGORITHM);
        return NULL;
    }
    if (!X509_digest(cert, md, hash, &len)
            || (new = ASN1_OCTET_STRING_new()) == NULL)
        goto err;
    if (ASN1_OCTET_STRING_set(new, hash, len)) {
        if (md_used != NULL)
            *md_used = md;
        else
            EVP_MD_free(md);
        return new;
    }
    ASN1_OCTET_STRING_free(new);
 err:
    EVP_MD_free(md);
    return NULL;
}

int X509_CRL_digest(const X509_CRL *data, const EVP_MD *type,
                    unsigned char *md, unsigned int *len)
{
    if (type == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    if (EVP_MD_is_a(type, SN_sha1)
            && (data->flags & EXFLAG_SET) != 0
            && (data->flags & EXFLAG_NO_FINGERPRINT) == 0) {
        /* Asking for SHA1; always computed in CRL d2i. */
        if (len != NULL)
            *len = sizeof(data->sha1_hash);
        memcpy(md, data->sha1_hash, sizeof(data->sha1_hash));
        return 1;
    }
    return
        ossl_asn1_item_digest_ex(ASN1_ITEM_rptr(X509_CRL), type, (char *)data,
                                 md, len, data->libctx, data->propq);
}

int X509_REQ_digest(const X509_REQ *data, const EVP_MD *type,
                    unsigned char *md, unsigned int *len)
{
    return
        ossl_asn1_item_digest_ex(ASN1_ITEM_rptr(X509_REQ), type, (char *)data,
                                 md, len, data->libctx, data->propq);
}

int X509_NAME_digest(const X509_NAME *data, const EVP_MD *type,
                     unsigned char *md, unsigned int *len)
{
    return ASN1_item_digest(ASN1_ITEM_rptr(X509_NAME), type, (char *)data,
                            md, len);
}

int PKCS7_ISSUER_AND_SERIAL_digest(PKCS7_ISSUER_AND_SERIAL *data,
                                   const EVP_MD *type, unsigned char *md,
                                   unsigned int *len)
{
    return ASN1_item_digest(ASN1_ITEM_rptr(PKCS7_ISSUER_AND_SERIAL), type,
                            (char *)data, md, len);
}

#ifndef OPENSSL_NO_STDIO
X509_SIG *d2i_PKCS8_fp(FILE *fp, X509_SIG **p8)
{
    return ASN1_d2i_fp_of(X509_SIG, X509_SIG_new, d2i_X509_SIG, fp, p8);
}

int i2d_PKCS8_fp(FILE *fp, const X509_SIG *p8)
{
    return ASN1_i2d_fp_of(X509_SIG, i2d_X509_SIG, fp, p8);
}
#endif

X509_SIG *d2i_PKCS8_bio(BIO *bp, X509_SIG **p8)
{
    return ASN1_d2i_bio_of(X509_SIG, X509_SIG_new, d2i_X509_SIG, bp, p8);
}

int i2d_PKCS8_bio(BIO *bp, const X509_SIG *p8)
{
    return ASN1_i2d_bio_of(X509_SIG, i2d_X509_SIG, bp, p8);
}

#ifndef OPENSSL_NO_STDIO
X509_PUBKEY *d2i_X509_PUBKEY_fp(FILE *fp, X509_PUBKEY **xpk)
{
    return ASN1_d2i_fp_of(X509_PUBKEY, X509_PUBKEY_new, d2i_X509_PUBKEY,
                          fp, xpk);
}

int i2d_X509_PUBKEY_fp(FILE *fp, const X509_PUBKEY *xpk)
{
    return ASN1_i2d_fp_of(X509_PUBKEY, i2d_X509_PUBKEY, fp, xpk);
}
#endif

X509_PUBKEY *d2i_X509_PUBKEY_bio(BIO *bp, X509_PUBKEY **xpk)
{
    return ASN1_d2i_bio_of(X509_PUBKEY, X509_PUBKEY_new, d2i_X509_PUBKEY,
                           bp, xpk);
}

int i2d_X509_PUBKEY_bio(BIO *bp, const X509_PUBKEY *xpk)
{
    return ASN1_i2d_bio_of(X509_PUBKEY, i2d_X509_PUBKEY, bp, xpk);
}

#ifndef OPENSSL_NO_STDIO
PKCS8_PRIV_KEY_INFO *d2i_PKCS8_PRIV_KEY_INFO_fp(FILE *fp,
                                                PKCS8_PRIV_KEY_INFO **p8inf)
{
    return ASN1_d2i_fp_of(PKCS8_PRIV_KEY_INFO, PKCS8_PRIV_KEY_INFO_new,
                          d2i_PKCS8_PRIV_KEY_INFO, fp, p8inf);
}

int i2d_PKCS8_PRIV_KEY_INFO_fp(FILE *fp, const PKCS8_PRIV_KEY_INFO *p8inf)
{
    return ASN1_i2d_fp_of(PKCS8_PRIV_KEY_INFO, i2d_PKCS8_PRIV_KEY_INFO, fp,
                          p8inf);
}

int i2d_PKCS8PrivateKeyInfo_fp(FILE *fp, const EVP_PKEY *key)
{
    PKCS8_PRIV_KEY_INFO *p8inf;
    int ret;

    p8inf = EVP_PKEY2PKCS8(key);
    if (p8inf == NULL)
        return 0;
    ret = i2d_PKCS8_PRIV_KEY_INFO_fp(fp, p8inf);
    PKCS8_PRIV_KEY_INFO_free(p8inf);
    return ret;
}

int i2d_PrivateKey_fp(FILE *fp, const EVP_PKEY *pkey)
{
    return ASN1_i2d_fp_of(EVP_PKEY, i2d_PrivateKey, fp, pkey);
}

EVP_PKEY *d2i_PrivateKey_fp(FILE *fp, EVP_PKEY **a)
{
    return ASN1_d2i_fp_of(EVP_PKEY, EVP_PKEY_new, d2i_AutoPrivateKey, fp, a);
}

EVP_PKEY *d2i_PrivateKey_ex_fp(FILE *fp, EVP_PKEY **a, OSSL_LIB_CTX *libctx,
                               const char *propq)
{
    BIO *b;
    void *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_BUF_LIB);
        return NULL;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = d2i_PrivateKey_ex_bio(b, a, libctx, propq);
    BIO_free(b);
    return ret;
}

int i2d_PUBKEY_fp(FILE *fp, const EVP_PKEY *pkey)
{
    return ASN1_i2d_fp_of(EVP_PKEY, i2d_PUBKEY, fp, pkey);
}

EVP_PKEY *d2i_PUBKEY_ex_fp(FILE *fp, EVP_PKEY **a, OSSL_LIB_CTX *libctx,
                           const char *propq)
{
    BIO *b;
    void *ret;

    if ((b = BIO_new(BIO_s_file())) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_BUF_LIB);
        return NULL;
    }
    BIO_set_fp(b, fp, BIO_NOCLOSE);
    ret = d2i_PUBKEY_ex_bio(b, a, libctx, propq);
    BIO_free(b);
    return ret;
}

EVP_PKEY *d2i_PUBKEY_fp(FILE *fp, EVP_PKEY **a)
{
    return ASN1_d2i_fp_of(EVP_PKEY, EVP_PKEY_new, d2i_PUBKEY, fp, a);
}

#endif

PKCS8_PRIV_KEY_INFO *d2i_PKCS8_PRIV_KEY_INFO_bio(BIO *bp,
                                                 PKCS8_PRIV_KEY_INFO **p8inf)
{
    return ASN1_d2i_bio_of(PKCS8_PRIV_KEY_INFO, PKCS8_PRIV_KEY_INFO_new,
                           d2i_PKCS8_PRIV_KEY_INFO, bp, p8inf);
}

int i2d_PKCS8_PRIV_KEY_INFO_bio(BIO *bp, const PKCS8_PRIV_KEY_INFO *p8inf)
{
    return ASN1_i2d_bio_of(PKCS8_PRIV_KEY_INFO, i2d_PKCS8_PRIV_KEY_INFO, bp,
                           p8inf);
}

int i2d_PKCS8PrivateKeyInfo_bio(BIO *bp, const EVP_PKEY *key)
{
    PKCS8_PRIV_KEY_INFO *p8inf;
    int ret;

    p8inf = EVP_PKEY2PKCS8(key);
    if (p8inf == NULL)
        return 0;
    ret = i2d_PKCS8_PRIV_KEY_INFO_bio(bp, p8inf);
    PKCS8_PRIV_KEY_INFO_free(p8inf);
    return ret;
}

int i2d_PrivateKey_bio(BIO *bp, const EVP_PKEY *pkey)
{
    return ASN1_i2d_bio_of(EVP_PKEY, i2d_PrivateKey, bp, pkey);
}

EVP_PKEY *d2i_PrivateKey_bio(BIO *bp, EVP_PKEY **a)
{
    return ASN1_d2i_bio_of(EVP_PKEY, EVP_PKEY_new, d2i_AutoPrivateKey, bp, a);
}

EVP_PKEY *d2i_PrivateKey_ex_bio(BIO *bp, EVP_PKEY **a, OSSL_LIB_CTX *libctx,
                                const char *propq)
{
    BUF_MEM *b = NULL;
    const unsigned char *p;
    void *ret = NULL;
    int len;

    len = asn1_d2i_read_bio(bp, &b);
    if (len < 0)
        goto err;

    p = (unsigned char *)b->data;
    ret = d2i_AutoPrivateKey_ex(a, &p, len, libctx, propq);
 err:
    BUF_MEM_free(b);
    return ret;
}

int i2d_PUBKEY_bio(BIO *bp, const EVP_PKEY *pkey)
{
    return ASN1_i2d_bio_of(EVP_PKEY, i2d_PUBKEY, bp, pkey);
}

EVP_PKEY *d2i_PUBKEY_ex_bio(BIO *bp, EVP_PKEY **a, OSSL_LIB_CTX *libctx,
                            const char *propq)
{
    BUF_MEM *b = NULL;
    const unsigned char *p;
    void *ret = NULL;
    int len;

    len = asn1_d2i_read_bio(bp, &b);
    if (len < 0)
        goto err;

    p = (unsigned char *)b->data;
    ret = d2i_PUBKEY_ex(a, &p, len, libctx, propq);
 err:
    BUF_MEM_free(b);
    return ret;
}

EVP_PKEY *d2i_PUBKEY_bio(BIO *bp, EVP_PKEY **a)
{
    return ASN1_d2i_bio_of(EVP_PKEY, EVP_PKEY_new, d2i_PUBKEY, bp, a);
}

#ifndef OPENSSL_NO_STDIO
X509_ACERT *d2i_X509_ACERT_fp(FILE *fp, X509_ACERT **acert)
{
    return ASN1_item_d2i_fp(ASN1_ITEM_rptr(X509_ACERT), fp, acert);
}

int i2d_X509_ACERT_fp(FILE *fp, const X509_ACERT *acert)
{
    return ASN1_item_i2d_fp(ASN1_ITEM_rptr(X509_ACERT), fp, acert);
}
#endif

X509_ACERT *d2i_X509_ACERT_bio(BIO *bp, X509_ACERT **acert)
{
    return ASN1_item_d2i_bio(ASN1_ITEM_rptr(X509_ACERT), bp, acert);
}

int i2d_X509_ACERT_bio(BIO *bp, const X509_ACERT *acert)
{
    return ASN1_item_i2d_bio(ASN1_ITEM_rptr(X509_ACERT), bp, acert);
}
