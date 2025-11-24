/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs are deprecated for public use, but still ok for internal use.
 */
#include "internal/deprecated.h"

#include <openssl/byteorder.h>
#include <openssl/core.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/crypto.h>
#include <openssl/params.h>
#include <openssl/asn1.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/x509.h>
#include <openssl/pkcs12.h>      /* PKCS8_encrypt() */
#include <openssl/dh.h>
#include <openssl/dsa.h>
#include <openssl/ec.h>
#include <openssl/proverr.h>
#include "internal/passphrase.h"
#include "internal/cryptlib.h"
#include "crypto/ecx.h"
#include "crypto/ml_kem.h"
#include "crypto/rsa.h"
#include "crypto/ml_dsa.h"
#include "crypto/slh_dsa.h"
#include "prov/implementations.h"
#include "prov/bio.h"
#include "prov/provider_ctx.h"
#include "prov/der_rsa.h"
#include "endecoder_local.h"
#include "ml_dsa_codecs.h"
#include "ml_kem_codecs.h"

#if defined(OPENSSL_NO_DH) && defined(OPENSSL_NO_DSA) && defined(OPENSSL_NO_EC)
# define OPENSSL_NO_KEYPARAMS
#endif

typedef struct key2any_ctx_st {
    PROV_CTX *provctx;

    /* Set to 0 if parameters should not be saved (dsa only) */
    int save_parameters;

    /* Set to 1 if intending to encrypt/decrypt, otherwise 0 */
    int cipher_intent;

    EVP_CIPHER *cipher;

    struct ossl_passphrase_data_st pwdata;
} KEY2ANY_CTX;

typedef int check_key_type_fn(const void *key, int nid);
typedef int key_to_paramstring_fn(const void *key, int nid, int save,
                                  void **str, int *strtype);
typedef int key_to_der_fn(BIO *out, const void *key,
                          int key_nid, const char *pemname,
                          key_to_paramstring_fn *p2s,
                          OSSL_i2d_of_void_ctx *k2d, KEY2ANY_CTX *ctx);
typedef int write_bio_of_void_fn(BIO *bp, const void *x);


/* Free the blob allocated during key_to_paramstring_fn */
static void free_asn1_data(int type, void *data)
{
    switch (type) {
    case V_ASN1_OBJECT:
        ASN1_OBJECT_free(data);
        break;
    case V_ASN1_SEQUENCE:
        ASN1_STRING_free(data);
        break;
    }
}

static PKCS8_PRIV_KEY_INFO *key_to_p8info(const void *key, int key_nid,
                                          void *params, int params_type,
                                          OSSL_i2d_of_void_ctx *k2d,
                                          KEY2ANY_CTX *ctx)
{
    /* der, derlen store the key DER output and its length */
    unsigned char *der = NULL;
    int derlen;
    /* The final PKCS#8 info */
    PKCS8_PRIV_KEY_INFO *p8info = NULL;

    if ((p8info = PKCS8_PRIV_KEY_INFO_new()) == NULL
        || (derlen = k2d(key, &der, (void *)ctx)) <= 0
        || !PKCS8_pkey_set0(p8info, OBJ_nid2obj(key_nid), 0,
                            params_type, params, der, derlen)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        PKCS8_PRIV_KEY_INFO_free(p8info);
        OPENSSL_free(der);
        p8info = NULL;
    }

    return p8info;
}

static X509_SIG *p8info_to_encp8(PKCS8_PRIV_KEY_INFO *p8info,
                                 KEY2ANY_CTX *ctx)
{
    X509_SIG *p8 = NULL;
    char kstr[PEM_BUFSIZE];
    size_t klen = 0;
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(ctx->provctx);

    if (ctx->cipher == NULL)
        return NULL;

    if (!ossl_pw_get_passphrase(kstr, sizeof(kstr), &klen, NULL, 1,
                                &ctx->pwdata)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_UNABLE_TO_GET_PASSPHRASE);
        return NULL;
    }
    /* First argument == -1 means "standard" */
    p8 = PKCS8_encrypt_ex(-1, ctx->cipher, kstr, klen, NULL, 0, 0, p8info, libctx, NULL);
    OPENSSL_cleanse(kstr, klen);
    return p8;
}

static X509_SIG *key_to_encp8(const void *key, int key_nid,
                              void *params, int params_type,
                              OSSL_i2d_of_void_ctx *k2d,
                              KEY2ANY_CTX *ctx)
{
    PKCS8_PRIV_KEY_INFO *p8info =
        key_to_p8info(key, key_nid, params, params_type, k2d, ctx);
    X509_SIG *p8 = NULL;

    if (p8info == NULL) {
        free_asn1_data(params_type, params);
    } else {
        p8 = p8info_to_encp8(p8info, ctx);
        PKCS8_PRIV_KEY_INFO_free(p8info);
    }
    return p8;
}

static X509_PUBKEY *key_to_pubkey(const void *key, int key_nid,
                                  void *params, int params_type,
                                  OSSL_i2d_of_void_ctx *k2d,
                                  KEY2ANY_CTX *ctx)
{
    /* der, derlen store the key DER output and its length */
    unsigned char *der = NULL;
    int derlen;
    /* The final X509_PUBKEY */
    X509_PUBKEY *xpk = NULL;


    if ((xpk = X509_PUBKEY_new()) == NULL
        || (derlen = k2d(key, &der, (void *)ctx)) <= 0
        || !X509_PUBKEY_set0_param(xpk, OBJ_nid2obj(key_nid),
                                   params_type, params, der, derlen)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_X509_LIB);
        X509_PUBKEY_free(xpk);
        OPENSSL_free(der);
        xpk = NULL;
    }

    return xpk;
}

/*
 * key_to_epki_* produce encoded output with the private key data in a
 * EncryptedPrivateKeyInfo structure (defined by PKCS#8).  They require
 * that there's an intent to encrypt, anything else is an error.
 *
 * key_to_pki_* primarily produce encoded output with the private key data
 * in a PrivateKeyInfo structure (also defined by PKCS#8).  However, if
 * there is an intent to encrypt the data, the corresponding key_to_epki_*
 * function is used instead.
 *
 * key_to_spki_* produce encoded output with the public key data in an
 * X.509 SubjectPublicKeyInfo.
 *
 * Key parameters don't have any defined envelopment of this kind, but are
 * included in some manner in the output from the functions described above,
 * either in the AlgorithmIdentifier's parameter field, or as part of the
 * key data itself.
 */

static int key_to_epki_der_priv_bio(BIO *out, const void *key,
                                    int key_nid,
                                    ossl_unused const char *pemname,
                                    key_to_paramstring_fn *p2s,
                                    OSSL_i2d_of_void_ctx *k2d,
                                    KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_SIG *p8;

    if (!ctx->cipher_intent)
        return 0;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8 = key_to_encp8(key, key_nid, str, strtype, k2d, ctx);
    if (p8 != NULL)
        ret = i2d_PKCS8_bio(out, p8);

    X509_SIG_free(p8);

    return ret;
}

static int key_to_epki_pem_priv_bio(BIO *out, const void *key,
                                    int key_nid,
                                    ossl_unused const char *pemname,
                                    key_to_paramstring_fn *p2s,
                                    OSSL_i2d_of_void_ctx *k2d,
                                    KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_SIG *p8;

    if (!ctx->cipher_intent)
        return 0;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8 = key_to_encp8(key, key_nid, str, strtype, k2d, ctx);
    if (p8 != NULL)
        ret = PEM_write_bio_PKCS8(out, p8);

    X509_SIG_free(p8);

    return ret;
}

static int key_to_pki_der_priv_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   OSSL_i2d_of_void_ctx *k2d,
                                   KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    PKCS8_PRIV_KEY_INFO *p8info;

    if (ctx->cipher_intent)
        return key_to_epki_der_priv_bio(out, key, key_nid, pemname,
                                        p2s, k2d, ctx);

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8info = key_to_p8info(key, key_nid, str, strtype, k2d, ctx);

    if (p8info != NULL)
        ret = i2d_PKCS8_PRIV_KEY_INFO_bio(out, p8info);
    else
        free_asn1_data(strtype, str);

    PKCS8_PRIV_KEY_INFO_free(p8info);

    return ret;
}

static int key_to_pki_pem_priv_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   OSSL_i2d_of_void_ctx *k2d,
                                   KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    PKCS8_PRIV_KEY_INFO *p8info;

    if (ctx->cipher_intent)
        return key_to_epki_pem_priv_bio(out, key, key_nid, pemname,
                                        p2s, k2d, ctx);

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    p8info = key_to_p8info(key, key_nid, str, strtype, k2d, ctx);

    if (p8info != NULL)
        ret = PEM_write_bio_PKCS8_PRIV_KEY_INFO(out, p8info);
    else
        free_asn1_data(strtype, str);

    PKCS8_PRIV_KEY_INFO_free(p8info);

    return ret;
}

static int key_to_spki_der_pub_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   OSSL_i2d_of_void_ctx *k2d,
                                   KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_PUBKEY *xpk = NULL;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    xpk = key_to_pubkey(key, key_nid, str, strtype, k2d, ctx);

    if (xpk != NULL)
        ret = i2d_X509_PUBKEY_bio(out, xpk);

    /* Also frees |str| */
    X509_PUBKEY_free(xpk);
    return ret;
}

static int key_to_spki_pem_pub_bio(BIO *out, const void *key,
                                   int key_nid,
                                   ossl_unused const char *pemname,
                                   key_to_paramstring_fn *p2s,
                                   OSSL_i2d_of_void_ctx *k2d,
                                   KEY2ANY_CTX *ctx)
{
    int ret = 0;
    void *str = NULL;
    int strtype = V_ASN1_UNDEF;
    X509_PUBKEY *xpk = NULL;

    if (p2s != NULL && !p2s(key, key_nid, ctx->save_parameters,
                            &str, &strtype))
        return 0;

    xpk = key_to_pubkey(key, key_nid, str, strtype, k2d, ctx);

    if (xpk != NULL)
        ret = PEM_write_bio_X509_PUBKEY(out, xpk);
    else
        free_asn1_data(strtype, str);

    /* Also frees |str| */
    X509_PUBKEY_free(xpk);
    return ret;
}

/*
 * key_to_type_specific_* produce encoded output with type specific key data,
 * no envelopment; the same kind of output as the type specific i2d_ and
 * PEM_write_ functions, which is often a simple SEQUENCE of INTEGER.
 *
 * OpenSSL tries to discourage production of new keys in this form, because
 * of the ambiguity when trying to recognise them, but can't deny that PKCS#1
 * et al still are live standards.
 *
 * Note that these functions completely ignore p2s, and rather rely entirely
 * on k2d to do the complete work.
 */
static int key_to_type_specific_der_bio(BIO *out, const void *key,
                                        int key_nid,
                                        ossl_unused const char *pemname,
                                        key_to_paramstring_fn *p2s,
                                        OSSL_i2d_of_void_ctx *k2d,
                                        KEY2ANY_CTX *ctx)
{
    unsigned char *der = NULL;
    int derlen;
    int ret;

    if ((derlen = k2d(key, &der, (void *)ctx)) <= 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PROV_LIB);
        return 0;
    }

    ret = BIO_write(out, der, derlen);
    OPENSSL_free(der);
    return ret > 0;
}
#define key_to_type_specific_der_priv_bio key_to_type_specific_der_bio
#define key_to_type_specific_der_pub_bio key_to_type_specific_der_bio
#define key_to_type_specific_der_param_bio key_to_type_specific_der_bio

static int key_to_type_specific_pem_bio_cb(BIO *out, const void *key,
                                           int key_nid, const char *pemname,
                                           key_to_paramstring_fn *p2s,
                                           OSSL_i2d_of_void_ctx *k2d,
                                           KEY2ANY_CTX *ctx,
                                           pem_password_cb *cb, void *cbarg)
{
    return PEM_ASN1_write_bio_ctx(k2d, (void *)ctx, pemname, out, key,
                                  ctx->cipher, NULL, 0, cb, cbarg) > 0;
}

static int key_to_type_specific_pem_priv_bio(BIO *out, const void *key,
                                             int key_nid, const char *pemname,
                                             key_to_paramstring_fn *p2s,
                                             OSSL_i2d_of_void_ctx *k2d,
                                             KEY2ANY_CTX *ctx)
{
    return key_to_type_specific_pem_bio_cb(out, key, key_nid, pemname,
                                           p2s, k2d, ctx,
                                           ossl_pw_pem_password, &ctx->pwdata);
}

static int key_to_type_specific_pem_pub_bio(BIO *out, const void *key,
                                            int key_nid, const char *pemname,
                                            key_to_paramstring_fn *p2s,
                                            OSSL_i2d_of_void_ctx *k2d,
                                            KEY2ANY_CTX *ctx)
{
    return key_to_type_specific_pem_bio_cb(out, key, key_nid, pemname,
                                           p2s, k2d, ctx, NULL, NULL);
}

#ifndef OPENSSL_NO_KEYPARAMS
static int key_to_type_specific_pem_param_bio(BIO *out, const void *key,
                                              int key_nid, const char *pemname,
                                              key_to_paramstring_fn *p2s,
                                              OSSL_i2d_of_void_ctx *k2d,
                                              KEY2ANY_CTX *ctx)
{
    return key_to_type_specific_pem_bio_cb(out, key, key_nid, pemname,
                                           p2s, k2d, ctx, NULL, NULL);
}
#endif

/* ---------------------------------------------------------------------- */

#define k2d_NOCTX(n, f)                             \
    static int                                      \
    n##_k2d(const void *key, unsigned char **pder,  \
            ossl_unused void *ctx)                  \
    {                                               \
        return f(key, pder);                        \
    }

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_DH
static int prepare_dh_params(const void *dh, int nid, int save,
                             void **pstr, int *pstrtype)
{
    ASN1_STRING *params = ASN1_STRING_new();

    if (params == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        return 0;
    }

    if (nid == EVP_PKEY_DHX)
        params->length = i2d_DHxparams(dh, &params->data);
    else
        params->length = i2d_DHparams(dh, &params->data);

    if (params->length <= 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        ASN1_STRING_free(params);
        return 0;
    }
    params->type = V_ASN1_SEQUENCE;

    *pstr = params;
    *pstrtype = V_ASN1_SEQUENCE;
    return 1;
}

static int dh_spki_pub_to_der(const void *dh, unsigned char **pder,
                              ossl_unused void *ctx)
{
    const BIGNUM *bn = NULL;
    ASN1_INTEGER *pub_key = NULL;
    int ret;

    if ((bn = DH_get0_pub_key(dh)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
        return 0;
    }
    if ((pub_key = BN_to_ASN1_INTEGER(bn, NULL)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BN_ERROR);
        return 0;
    }

    ret = i2d_ASN1_INTEGER(pub_key, pder);

    ASN1_STRING_clear_free(pub_key);
    return ret;
}

static int dh_pki_priv_to_der(const void *dh, unsigned char **pder,
                              ossl_unused void *ctx)
{
    const BIGNUM *bn = NULL;
    ASN1_INTEGER *priv_key = NULL;
    int ret;

    if ((bn = DH_get0_priv_key(dh)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
        return 0;
    }
    if ((priv_key = BN_to_ASN1_INTEGER(bn, NULL)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BN_ERROR);
        return 0;
    }

    ret = i2d_ASN1_INTEGER(priv_key, pder);

    ASN1_STRING_clear_free(priv_key);
    return ret;
}

# define dh_epki_priv_to_der dh_pki_priv_to_der

static int
dh_type_specific_params_to_der(const void *dh, unsigned char **pder,
                               ossl_unused void *ctx)
{
    if (DH_test_flags(dh, DH_FLAG_TYPE_DHX))
        return i2d_DHxparams(dh, pder);
    return i2d_DHparams(dh, pder);
}

/*
 * DH doesn't have i2d_DHPrivateKey or i2d_DHPublicKey, so we can't make
 * corresponding functions here.
 */
# define dh_type_specific_priv_to_der   NULL
# define dh_type_specific_pub_to_der    NULL

static int dh_check_key_type(const void *dh, int expected_type)
{
    int type =
        DH_test_flags(dh, DH_FLAG_TYPE_DHX) ? EVP_PKEY_DHX : EVP_PKEY_DH;

    return type == expected_type;
}

# define dh_evp_type            EVP_PKEY_DH
# define dhx_evp_type           EVP_PKEY_DHX
# define dh_pem_type            "DH"
# define dhx_pem_type           "X9.42 DH"
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_DSA
static int encode_dsa_params(const void *dsa, int nid,
                             void **pstr, int *pstrtype)
{
    ASN1_STRING *params = ASN1_STRING_new();

    if (params == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        return 0;
    }

    params->length = i2d_DSAparams(dsa, &params->data);

    if (params->length <= 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        ASN1_STRING_free(params);
        return 0;
    }

    *pstrtype = V_ASN1_SEQUENCE;
    *pstr = params;
    return 1;
}

static int prepare_dsa_params(const void *dsa, int nid, int save,
                              void **pstr, int *pstrtype)
{
    const BIGNUM *p = DSA_get0_p(dsa);
    const BIGNUM *q = DSA_get0_q(dsa);
    const BIGNUM *g = DSA_get0_g(dsa);

    if (save && p != NULL && q != NULL && g != NULL)
        return encode_dsa_params(dsa, nid, pstr, pstrtype);

    *pstr = NULL;
    *pstrtype = V_ASN1_UNDEF;
    return 1;
}

static int dsa_spki_pub_to_der(const void *dsa, unsigned char **pder,
                               ossl_unused void *ctx)
{
    const BIGNUM *bn = NULL;
    ASN1_INTEGER *pub_key = NULL;
    int ret;

    if ((bn = DSA_get0_pub_key(dsa)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
        return 0;
    }
    if ((pub_key = BN_to_ASN1_INTEGER(bn, NULL)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BN_ERROR);
        return 0;
    }

    ret = i2d_ASN1_INTEGER(pub_key, pder);

    ASN1_STRING_clear_free(pub_key);
    return ret;
}

static int dsa_pki_priv_to_der(const void *dsa, unsigned char **pder,
                               ossl_unused void *ctx)
{
    const BIGNUM *bn = NULL;
    ASN1_INTEGER *priv_key = NULL;
    int ret;

    if ((bn = DSA_get0_priv_key(dsa)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PRIVATE_KEY);
        return 0;
    }
    if ((priv_key = BN_to_ASN1_INTEGER(bn, NULL)) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_BN_ERROR);
        return 0;
    }

    ret = i2d_ASN1_INTEGER(priv_key, pder);

    ASN1_STRING_clear_free(priv_key);
    return ret;
}

k2d_NOCTX(dsa_prv, i2d_DSAPrivateKey)
k2d_NOCTX(dsa_pub, i2d_DSAPublicKey)
k2d_NOCTX(dsa_param, i2d_DSAparams)

# define dsa_epki_priv_to_der dsa_pki_priv_to_der

# define dsa_type_specific_priv_to_der   dsa_prv_k2d
# define dsa_type_specific_pub_to_der    dsa_pub_k2d
# define dsa_type_specific_params_to_der dsa_param_k2d

# define dsa_check_key_type     NULL
# define dsa_evp_type           EVP_PKEY_DSA
# define dsa_pem_type           "DSA"
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_EC
static int prepare_ec_explicit_params(const void *eckey,
                                      void **pstr, int *pstrtype)
{
    ASN1_STRING *params = ASN1_STRING_new();

    if (params == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        return 0;
    }

    params->length = i2d_ECParameters(eckey, &params->data);
    if (params->length <= 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        ASN1_STRING_free(params);
        return 0;
    }

    *pstrtype = V_ASN1_SEQUENCE;
    *pstr = params;
    return 1;
}

/*
 * This implements EcpkParameters, where the CHOICE is based on whether there
 * is a curve name (curve nid) to be found or not.  See RFC 3279 for details.
 */
static int prepare_ec_params(const void *eckey, int nid, int save,
                             void **pstr, int *pstrtype)
{
    int curve_nid;
    const EC_GROUP *group = EC_KEY_get0_group(eckey);
    ASN1_OBJECT *params = NULL;

    if (group == NULL)
        return 0;
    curve_nid = EC_GROUP_get_curve_name(group);
    if (curve_nid != NID_undef) {
        params = OBJ_nid2obj(curve_nid);
        if (params == NULL)
            return 0;
    }

    if (curve_nid != NID_undef
        && (EC_GROUP_get_asn1_flag(group) & OPENSSL_EC_NAMED_CURVE)) {
        /* The CHOICE came to namedCurve */
        if (OBJ_length(params) == 0) {
            /* Some curves might not have an associated OID */
            ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_OID);
            ASN1_OBJECT_free(params);
            return 0;
        }
        *pstr = params;
        *pstrtype = V_ASN1_OBJECT;
        return 1;
    } else {
        /* The CHOICE came to ecParameters */
        return prepare_ec_explicit_params(eckey, pstr, pstrtype);
    }
}

static int ec_spki_pub_to_der(const void *eckey, unsigned char **pder,
                              ossl_unused void *ctx)
{
    if (EC_KEY_get0_public_key(eckey) == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
        return 0;
    }
    return i2o_ECPublicKey(eckey, pder);
}

static int ec_pki_priv_to_der(const void *veckey, unsigned char **pder,
                              ossl_unused void *ctx)
{
    EC_KEY *eckey = (EC_KEY *)veckey;
    unsigned int old_flags;
    int ret = 0;

    /*
     * For PKCS8 the curve name appears in the PKCS8_PRIV_KEY_INFO object
     * as the pkeyalg->parameter field. (For a named curve this is an OID)
     * The pkey field is an octet string that holds the encoded
     * ECPrivateKey SEQUENCE with the optional parameters field omitted.
     * We omit this by setting the EC_PKEY_NO_PARAMETERS flag.
     */
    old_flags = EC_KEY_get_enc_flags(eckey); /* save old flags */
    EC_KEY_set_enc_flags(eckey, old_flags | EC_PKEY_NO_PARAMETERS);
    ret = i2d_ECPrivateKey(eckey, pder);
    EC_KEY_set_enc_flags(eckey, old_flags); /* restore old flags */
    return ret; /* return the length of the der encoded data */
}

k2d_NOCTX(ec_param, i2d_ECParameters)
k2d_NOCTX(ec_prv, i2d_ECPrivateKey)

# define ec_epki_priv_to_der ec_pki_priv_to_der

# define ec_type_specific_params_to_der ec_param_k2d
/* No ec_type_specific_pub_to_der, there simply is no such thing */
# define ec_type_specific_priv_to_der   ec_prv_k2d

# define ec_check_key_type      NULL
# define ec_evp_type            EVP_PKEY_EC
# define ec_pem_type            "EC"

# ifndef OPENSSL_NO_SM2
/*
 * Albeit SM2 is a slightly different algorithm than ECDSA, the key type
 * encoding (in all places where an AlgorithmIdentifier is produced, such
 * as PrivateKeyInfo and SubjectPublicKeyInfo) is the same as for ECC keys
 * according to the example in GM/T 0015-2012, appendix D.2.
 * This leaves the distinction of SM2 keys to the EC group (which is found
 * in AlgorithmIdentified.params).
 */
#  define sm2_evp_type          ec_evp_type
#  define sm2_pem_type          "SM2"
# endif
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ECX
# define prepare_ecx_params NULL

static int ecx_spki_pub_to_der(const void *vecxkey, unsigned char **pder,
                               ossl_unused void *ctx)
{
    const ECX_KEY *ecxkey = vecxkey;
    unsigned char *keyblob;

    if (ecxkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    keyblob = OPENSSL_memdup(ecxkey->pubkey, ecxkey->keylen);
    if (keyblob == NULL)
        return 0;

    *pder = keyblob;
    return ecxkey->keylen;
}

static int ecx_pki_priv_to_der(const void *vecxkey, unsigned char **pder,
                               ossl_unused void *ctx)
{
    const ECX_KEY *ecxkey = vecxkey;
    ASN1_OCTET_STRING oct;
    int keybloblen;

    if (ecxkey == NULL || ecxkey->privkey == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    oct.data = ecxkey->privkey;
    oct.length = ecxkey->keylen;
    oct.flags = 0;

    keybloblen = i2d_ASN1_OCTET_STRING(&oct, pder);
    if (keybloblen < 0) {
        ERR_raise(ERR_LIB_PROV, ERR_R_ASN1_LIB);
        return 0;
    }

    return keybloblen;
}

# define ecx_epki_priv_to_der ecx_pki_priv_to_der

/*
 * ED25519, ED448, X25519 and X448 only has PKCS#8 / SubjectPublicKeyInfo
 * representation, so we don't define ecx_type_specific_[priv,pub,params]_to_der.
 */

# define ecx_check_key_type     NULL

# define ed25519_evp_type       EVP_PKEY_ED25519
# define ed448_evp_type         EVP_PKEY_ED448
# define x25519_evp_type        EVP_PKEY_X25519
# define x448_evp_type          EVP_PKEY_X448
# define ed25519_pem_type       "ED25519"
# define ed448_pem_type         "ED448"
# define x25519_pem_type        "X25519"
# define x448_pem_type          "X448"
#endif

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ML_DSA
static int ml_dsa_spki_pub_to_der(const void *vkey, unsigned char **pder,
                                  ossl_unused void *ctx)
{
    return ossl_ml_dsa_i2d_pubkey(vkey, pder);
}

static int ml_dsa_pki_priv_to_der(const void *vkey, unsigned char **pder,
                                  void *vctx)
{
    KEY2ANY_CTX *ctx = vctx;

    return ossl_ml_dsa_i2d_prvkey(vkey, pder, ctx->provctx);
}

# define ml_dsa_epki_priv_to_der ml_dsa_pki_priv_to_der
# define prepare_ml_dsa_params   NULL
# define ml_dsa_check_key_type   NULL

# define ml_dsa_44_evp_type        EVP_PKEY_ML_DSA_44
# define ml_dsa_44_pem_type        "ML-DSA-44"
# define ml_dsa_65_evp_type        EVP_PKEY_ML_DSA_65
# define ml_dsa_65_pem_type        "ML-DSA-65"
# define ml_dsa_87_evp_type        EVP_PKEY_ML_DSA_87
# define ml_dsa_87_pem_type        "ML-DSA-87"
#endif /* OPENSSL_NO_ML_DSA */

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_ML_KEM

static int ml_kem_spki_pub_to_der(const void *vkey, unsigned char **pder,
                                  ossl_unused void *ctx)
{
    return ossl_ml_kem_i2d_pubkey(vkey, pder);
}

static int ml_kem_pki_priv_to_der(const void *vkey, unsigned char **pder,
                                  void *vctx)
{
    KEY2ANY_CTX *ctx = vctx;

    return ossl_ml_kem_i2d_prvkey(vkey, pder, ctx->provctx);
}

# define ml_kem_epki_priv_to_der ml_kem_pki_priv_to_der
# define prepare_ml_kem_params   NULL
# define ml_kem_check_key_type   NULL

# define ml_kem_512_evp_type        EVP_PKEY_ML_KEM_512
# define ml_kem_512_pem_type        "ML-KEM-512"
# define ml_kem_768_evp_type        EVP_PKEY_ML_KEM_768
# define ml_kem_768_pem_type        "ML-KEM-768"
# define ml_kem_1024_evp_type       EVP_PKEY_ML_KEM_1024
# define ml_kem_1024_pem_type       "ML-KEM-1024"
#endif

/* ---------------------------------------------------------------------- */

/*
 * Helper functions to prepare RSA-PSS params for encoding.  We would
 * have simply written the whole AlgorithmIdentifier, but existing libcrypto
 * functionality doesn't allow that.
 */

static int prepare_rsa_params(const void *rsa, int nid, int save,
                              void **pstr, int *pstrtype)
{
    const RSA_PSS_PARAMS_30 *pss = ossl_rsa_get0_pss_params_30((RSA *)rsa);

    *pstr = NULL;

    switch (RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK)) {
    case RSA_FLAG_TYPE_RSA:
        /* If plain RSA, the parameters shall be NULL */
        *pstrtype = V_ASN1_NULL;
        return 1;
    case RSA_FLAG_TYPE_RSASSAPSS:
        if (ossl_rsa_pss_params_30_is_unrestricted(pss)) {
            *pstrtype = V_ASN1_UNDEF;
            return 1;
        } else {
            ASN1_STRING *astr = NULL;
            WPACKET pkt;
            unsigned char *str = NULL;
            size_t str_sz = 0;
            int i;

            for (i = 0; i < 2; i++) {
                switch (i) {
                case 0:
                    if (!WPACKET_init_null_der(&pkt))
                        goto err;
                    break;
                case 1:
                    if ((str = OPENSSL_malloc(str_sz)) == NULL
                        || !WPACKET_init_der(&pkt, str, str_sz)) {
                        WPACKET_cleanup(&pkt);
                        goto err;
                    }
                    break;
                }
                if (!ossl_DER_w_RSASSA_PSS_params(&pkt, -1, pss)
                    || !WPACKET_finish(&pkt)
                    || !WPACKET_get_total_written(&pkt, &str_sz)) {
                    WPACKET_cleanup(&pkt);
                    goto err;
                }
                WPACKET_cleanup(&pkt);

                /*
                 * If no PSS parameters are going to be written, there's no
                 * point going for another iteration.
                 * This saves us from getting |str| allocated just to have it
                 * immediately de-allocated.
                 */
                if (str_sz == 0)
                    break;
            }

            if ((astr = ASN1_STRING_new()) == NULL)
                goto err;
            *pstrtype = V_ASN1_SEQUENCE;
            ASN1_STRING_set0(astr, str, (int)str_sz);
            *pstr = astr;

            return 1;
         err:
            OPENSSL_free(str);
            return 0;
        }
    }

    /* Currently unsupported RSA key type */
    return 0;
}

k2d_NOCTX(rsa_prv, i2d_RSAPrivateKey)
k2d_NOCTX(rsa_pub, i2d_RSAPublicKey)

/*
 * RSA is extremely simple, as PKCS#1 is used for the PKCS#8 |privateKey|
 * field as well as the SubjectPublicKeyInfo |subjectPublicKey| field.
 */
#define rsa_pki_priv_to_der             rsa_type_specific_priv_to_der
#define rsa_epki_priv_to_der            rsa_type_specific_priv_to_der
#define rsa_spki_pub_to_der             rsa_type_specific_pub_to_der
#define rsa_type_specific_priv_to_der   rsa_prv_k2d
#define rsa_type_specific_pub_to_der    rsa_pub_k2d
#define rsa_type_specific_params_to_der NULL

static int rsa_check_key_type(const void *rsa, int expected_type)
{
    switch (RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK)) {
    case RSA_FLAG_TYPE_RSA:
        return expected_type == EVP_PKEY_RSA;
    case RSA_FLAG_TYPE_RSASSAPSS:
        return expected_type == EVP_PKEY_RSA_PSS;
    }

    /* Currently unsupported RSA key type */
    return EVP_PKEY_NONE;
}

#define rsa_evp_type            EVP_PKEY_RSA
#define rsapss_evp_type         EVP_PKEY_RSA_PSS
#define rsa_pem_type            "RSA"
#define rsapss_pem_type         "RSA-PSS"

/* ---------------------------------------------------------------------- */

#ifndef OPENSSL_NO_SLH_DSA
# define prepare_slh_dsa_params NULL

static int slh_dsa_spki_pub_to_der(const void *vkey, unsigned char **pder,
                                   ossl_unused void *ctx)
{
    const SLH_DSA_KEY *key = vkey;
    uint8_t *key_blob;
    size_t key_len;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    key_len = ossl_slh_dsa_key_get_pub_len(key);
    key_blob = OPENSSL_memdup(ossl_slh_dsa_key_get_pub(key), key_len);
    if (key_blob == NULL)
        return 0;

    *pder = key_blob;
    return key_len;
}

static int slh_dsa_pki_priv_to_der(const void *vkey, unsigned char **pder,
                                   ossl_unused void *ctx)
{
    const SLH_DSA_KEY *key = vkey;
    size_t len;

    if (ossl_slh_dsa_key_get_priv(key) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }
    len = ossl_slh_dsa_key_get_priv_len(key);

    if (pder != NULL
            && ((*pder = OPENSSL_memdup(ossl_slh_dsa_key_get_priv(key), len)) == NULL))
        return 0;

    return len;
}
# define slh_dsa_epki_priv_to_der slh_dsa_pki_priv_to_der

/* SLH_DSA only has PKCS#8 / SubjectPublicKeyInfo representations. */

# define slh_dsa_check_key_type NULL
# define slh_dsa_sha2_128s_evp_type EVP_PKEY_SLH_DSA_SHA2_128S
# define slh_dsa_sha2_128f_evp_type EVP_PKEY_SLH_DSA_SHA2_128F
# define slh_dsa_sha2_192s_evp_type EVP_PKEY_SLH_DSA_SHA2_192S
# define slh_dsa_sha2_192f_evp_type EVP_PKEY_SLH_DSA_SHA2_192F
# define slh_dsa_sha2_256s_evp_type EVP_PKEY_SLH_DSA_SHA2_256S
# define slh_dsa_sha2_256f_evp_type EVP_PKEY_SLH_DSA_SHA2_256F
# define slh_dsa_shake_128s_evp_type EVP_PKEY_SLH_DSA_SHAKE_128S
# define slh_dsa_shake_128f_evp_type EVP_PKEY_SLH_DSA_SHAKE_128F
# define slh_dsa_shake_192s_evp_type EVP_PKEY_SLH_DSA_SHAKE_192S
# define slh_dsa_shake_192f_evp_type EVP_PKEY_SLH_DSA_SHAKE_192F
# define slh_dsa_shake_256s_evp_type EVP_PKEY_SLH_DSA_SHAKE_256S
# define slh_dsa_shake_256f_evp_type EVP_PKEY_SLH_DSA_SHAKE_256F
# define slh_dsa_sha2_128s_input_type "SLH-DSA-SHA2-128s"
# define slh_dsa_sha2_128f_input_type "SLH-DSA-SHA2-128f"
# define slh_dsa_sha2_192s_input_type "SLH-DSA-SHA2-192s"
# define slh_dsa_sha2_192f_input_type "SLH-DSA-SHA2-192f"
# define slh_dsa_sha2_256s_input_type "SLH-DSA-SHA2-256s"
# define slh_dsa_sha2_256f_input_type "SLH-DSA-SHA2-256f"
# define slh_dsa_shake_128s_input_type "SLH-DSA-SHAKE-128s"
# define slh_dsa_shake_128f_input_type "SLH-DSA-SHAKE-128f"
# define slh_dsa_shake_192s_input_type "SLH-DSA-SHAKE-192s"
# define slh_dsa_shake_192f_input_type "SLH-DSA-SHAKE-192f"
# define slh_dsa_shake_256s_input_type "SLH-DSA-SHAKE-256s"
# define slh_dsa_shake_256f_input_type "SLH-DSA-SHAKE-256f"
# define slh_dsa_sha2_128s_pem_type "SLH-DSA-SHA2-128s"
# define slh_dsa_sha2_128f_pem_type "SLH-DSA-SHA2-128f"
# define slh_dsa_sha2_192s_pem_type "SLH-DSA-SHA2-192s"
# define slh_dsa_sha2_192f_pem_type "SLH-DSA-SHA2-192f"
# define slh_dsa_sha2_256s_pem_type "SLH-DSA-SHA2-256s"
# define slh_dsa_sha2_256f_pem_type "SLH-DSA-SHA2-256f"
# define slh_dsa_shake_128s_pem_type "SLH-DSA-SHAKE-128s"
# define slh_dsa_shake_128f_pem_type "SLH-DSA-SHAKE-128f"
# define slh_dsa_shake_192s_pem_type "SLH-DSA-SHAKE-192s"
# define slh_dsa_shake_192f_pem_type "SLH-DSA-SHAKE-192f"
# define slh_dsa_shake_256s_pem_type "SLH-DSA-SHAKE-256s"
# define slh_dsa_shake_256f_pem_type "SLH-DSA-SHAKE-256f"
#endif /* OPENSSL_NO_SLH_DSA */

/* ---------------------------------------------------------------------- */

static OSSL_FUNC_decoder_newctx_fn key2any_newctx;
static OSSL_FUNC_decoder_freectx_fn key2any_freectx;

static void *key2any_newctx(void *provctx)
{
    KEY2ANY_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->provctx = provctx;
        ctx->save_parameters = 1;
    }

    return ctx;
}

static void key2any_freectx(void *vctx)
{
    KEY2ANY_CTX *ctx = vctx;

    ossl_pw_clear_passphrase_data(&ctx->pwdata);
    EVP_CIPHER_free(ctx->cipher);
    OPENSSL_free(ctx);
}

static const OSSL_PARAM *key2any_settable_ctx_params(ossl_unused void *provctx)
{
    static const OSSL_PARAM settables[] = {
        OSSL_PARAM_utf8_string(OSSL_ENCODER_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_ENCODER_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END,
    };

    return settables;
}

static int key2any_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    KEY2ANY_CTX *ctx = vctx;
    OSSL_LIB_CTX *libctx = ossl_prov_ctx_get0_libctx(ctx->provctx);
    const OSSL_PARAM *cipherp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_CIPHER);
    const OSSL_PARAM *propsp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_PROPERTIES);
    const OSSL_PARAM *save_paramsp =
        OSSL_PARAM_locate_const(params, OSSL_ENCODER_PARAM_SAVE_PARAMETERS);

    if (cipherp != NULL) {
        const char *ciphername = NULL;
        const char *props = NULL;

        if (!OSSL_PARAM_get_utf8_string_ptr(cipherp, &ciphername))
            return 0;
        if (propsp != NULL && !OSSL_PARAM_get_utf8_string_ptr(propsp, &props))
            return 0;

        EVP_CIPHER_free(ctx->cipher);
        ctx->cipher = NULL;
        ctx->cipher_intent = ciphername != NULL;
        if (ciphername != NULL
            && ((ctx->cipher =
                 EVP_CIPHER_fetch(libctx, ciphername, props)) == NULL))
            return 0;
    }

    if (save_paramsp != NULL) {
        if (!OSSL_PARAM_get_int(save_paramsp, &ctx->save_parameters))
            return 0;
    }
    return 1;
}

static int key2any_check_selection(int selection, int selection_mask)
{
    /*
     * The selections are kinda sorta "levels", i.e. each selection given
     * here is assumed to include those following.
     */
    int checks[] = {
        OSSL_KEYMGMT_SELECT_PRIVATE_KEY,
        OSSL_KEYMGMT_SELECT_PUBLIC_KEY,
        OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
    };
    size_t i;

    /* The decoder implementations made here support guessing */
    if (selection == 0)
        return 1;

    for (i = 0; i < OSSL_NELEM(checks); i++) {
        int check1 = (selection & checks[i]) != 0;
        int check2 = (selection_mask & checks[i]) != 0;

        /*
         * If the caller asked for the currently checked bit(s), return
         * whether the decoder description says it's supported.
         */
        if (check1)
            return check2;
    }

    /* This should be dead code, but just to be safe... */
    return 0;
}

static int key2any_encode(KEY2ANY_CTX *ctx, OSSL_CORE_BIO *cout,
                          const void *key, int type, const char *pemname,
                          check_key_type_fn *checker,
                          key_to_der_fn *writer,
                          OSSL_PASSPHRASE_CALLBACK *pwcb, void *pwcbarg,
                          key_to_paramstring_fn *key2paramstring,
                          OSSL_i2d_of_void_ctx *key2der)
{
    int ret = 0;

    if (key == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_NULL_PARAMETER);
    } else if (writer != NULL
               && (checker == NULL || checker(key, type))) {
        BIO *out = ossl_bio_new_from_core_bio(ctx->provctx, cout);

        if (out != NULL
            && (pwcb == NULL
                || ossl_pw_set_ossl_passphrase_cb(&ctx->pwdata, pwcb, pwcbarg)))
            ret =
                writer(out, key, type, pemname, key2paramstring, key2der, ctx);

        BIO_free(out);
    } else {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
    }
    return ret;
}

#define DO_PRIVATE_KEY_selection_mask OSSL_KEYMGMT_SELECT_PRIVATE_KEY
#define DO_PRIVATE_KEY(impl, type, kind, output)                            \
    if ((selection & DO_PRIVATE_KEY_selection_mask) != 0)                   \
        return key2any_encode(ctx, cout, key, impl##_evp_type,              \
                              impl##_pem_type " PRIVATE KEY",               \
                              type##_check_key_type,                        \
                              key_to_##kind##_##output##_priv_bio,          \
                              cb, cbarg, prepare_##type##_params,           \
                              type##_##kind##_priv_to_der);

#define DO_PUBLIC_KEY_selection_mask OSSL_KEYMGMT_SELECT_PUBLIC_KEY
#define DO_PUBLIC_KEY(impl, type, kind, output)                             \
    if ((selection & DO_PUBLIC_KEY_selection_mask) != 0)                    \
        return key2any_encode(ctx, cout, key, impl##_evp_type,              \
                              impl##_pem_type " PUBLIC KEY",                \
                              type##_check_key_type,                        \
                              key_to_##kind##_##output##_pub_bio,           \
                              cb, cbarg, prepare_##type##_params,           \
                              type##_##kind##_pub_to_der);

#define DO_PARAMETERS_selection_mask OSSL_KEYMGMT_SELECT_ALL_PARAMETERS
#define DO_PARAMETERS(impl, type, kind, output)                             \
    if ((selection & DO_PARAMETERS_selection_mask) != 0)                    \
        return key2any_encode(ctx, cout, key, impl##_evp_type,              \
                              impl##_pem_type " PARAMETERS",                \
                              type##_check_key_type,                        \
                              key_to_##kind##_##output##_param_bio,         \
                              NULL, NULL, NULL,                             \
                              type##_##kind##_params_to_der);

/*-
 * Implement the kinds of output structure that can be produced.  They are
 * referred to by name, and for each name, the following macros are defined
 * (braces not included):
 *
 * DO_{kind}_selection_mask
 *
 *      A mask of selection bits that must not be zero.  This is used as a
 *      selection criterion for each implementation.
 *      This mask must never be zero.
 *
 * DO_{kind}
 *
 *      The performing macro.  It must use the DO_ macros defined above,
 *      always in this order:
 *
 *      - DO_PRIVATE_KEY
 *      - DO_PUBLIC_KEY
 *      - DO_PARAMETERS
 *
 *      Any of those may be omitted, but the relative order must still be
 *      the same.
 */

/*
 * PKCS#8 defines two structures for private keys only:
 * - PrivateKeyInfo             (raw unencrypted form)
 * - EncryptedPrivateKeyInfo    (encrypted wrapping)
 *
 * To allow a certain amount of flexibility, we allow the routines
 * for PrivateKeyInfo to also produce EncryptedPrivateKeyInfo if a
 * passphrase callback has been passed to them.
 */
#define DO_PrivateKeyInfo_selection_mask DO_PRIVATE_KEY_selection_mask
#define DO_PrivateKeyInfo(impl, type, output)                               \
    DO_PRIVATE_KEY(impl, type, pki, output)

#define DO_EncryptedPrivateKeyInfo_selection_mask DO_PRIVATE_KEY_selection_mask
#define DO_EncryptedPrivateKeyInfo(impl, type, output)                      \
    DO_PRIVATE_KEY(impl, type, epki, output)

/* SubjectPublicKeyInfo is a structure for public keys only */
#define DO_SubjectPublicKeyInfo_selection_mask DO_PUBLIC_KEY_selection_mask
#define DO_SubjectPublicKeyInfo(impl, type, output)                         \
    DO_PUBLIC_KEY(impl, type, spki, output)

/*
 * "type-specific" is a uniform name for key type specific output for private
 * and public keys as well as key parameters.  This is used internally in
 * libcrypto so it doesn't have to have special knowledge about select key
 * types, but also when no better name has been found.  If there are more
 * expressive DO_ names above, those are preferred.
 *
 * Three forms exist:
 *
 * - type_specific_keypair              Only supports private and public key
 * - type_specific_params               Only supports parameters
 * - type_specific                      Supports all parts of an EVP_PKEY
 * - type_specific_no_pub               Supports all parts of an EVP_PKEY
 *                                      except public key
 */
#define DO_type_specific_params_selection_mask DO_PARAMETERS_selection_mask
#define DO_type_specific_params(impl, type, output)                         \
    DO_PARAMETERS(impl, type, type_specific, output)
#define DO_type_specific_keypair_selection_mask                             \
    ( DO_PRIVATE_KEY_selection_mask | DO_PUBLIC_KEY_selection_mask )
#define DO_type_specific_keypair(impl, type, output)                        \
    DO_PRIVATE_KEY(impl, type, type_specific, output)                       \
    DO_PUBLIC_KEY(impl, type, type_specific, output)
#define DO_type_specific_selection_mask                                     \
    ( DO_type_specific_keypair_selection_mask                               \
      | DO_type_specific_params_selection_mask )
#define DO_type_specific(impl, type, output)                                \
    DO_type_specific_keypair(impl, type, output)                            \
    DO_type_specific_params(impl, type, output)
#define DO_type_specific_no_pub_selection_mask \
    ( DO_PRIVATE_KEY_selection_mask |  DO_PARAMETERS_selection_mask)
#define DO_type_specific_no_pub(impl, type, output)                         \
    DO_PRIVATE_KEY(impl, type, type_specific, output)                       \
    DO_type_specific_params(impl, type, output)

/*
 * Type specific aliases for the cases where we need to refer to them by
 * type name.
 * This only covers key types that are represented with i2d_{TYPE}PrivateKey,
 * i2d_{TYPE}PublicKey and i2d_{TYPE}params / i2d_{TYPE}Parameters.
 */
#define DO_RSA_selection_mask DO_type_specific_keypair_selection_mask
#define DO_RSA(impl, type, output) DO_type_specific_keypair(impl, type, output)

#define DO_DH_selection_mask DO_type_specific_params_selection_mask
#define DO_DH(impl, type, output) DO_type_specific_params(impl, type, output)

#define DO_DHX_selection_mask DO_type_specific_params_selection_mask
#define DO_DHX(impl, type, output) DO_type_specific_params(impl, type, output)

#define DO_DSA_selection_mask DO_type_specific_selection_mask
#define DO_DSA(impl, type, output) DO_type_specific(impl, type, output)

#define DO_EC_selection_mask DO_type_specific_no_pub_selection_mask
#define DO_EC(impl, type, output) DO_type_specific_no_pub(impl, type, output)

#define DO_SM2_selection_mask DO_type_specific_no_pub_selection_mask
#define DO_SM2(impl, type, output) DO_type_specific_no_pub(impl, type, output)

/* PKCS#1 defines a structure for RSA private and public keys */
#define DO_PKCS1_selection_mask DO_RSA_selection_mask
#define DO_PKCS1(impl, type, output) DO_RSA(impl, type, output)

/* PKCS#3 defines a structure for DH parameters */
#define DO_PKCS3_selection_mask DO_DH_selection_mask
#define DO_PKCS3(impl, type, output) DO_DH(impl, type, output)
/* X9.42 defines a structure for DHx parameters */
#define DO_X9_42_selection_mask DO_DHX_selection_mask
#define DO_X9_42(impl, type, output) DO_DHX(impl, type, output)

/* X9.62 defines a structure for EC keys and parameters */
#define DO_X9_62_selection_mask DO_EC_selection_mask
#define DO_X9_62(impl, type, output) DO_EC(impl, type, output)

/*
 * MAKE_ENCODER is the single driver for creating OSSL_DISPATCH tables.
 * It takes the following arguments:
 *
 * impl         This is the key type name that's being implemented.
 * type         This is the type name for the set of functions that implement
 *              the key type.  For example, ed25519, ed448, x25519 and x448
 *              are all implemented with the exact same set of functions.
 * kind         What kind of support to implement.  These translate into
 *              the DO_##kind macros above.
 * output       The output type to implement.  may be der or pem.
 *
 * The resulting OSSL_DISPATCH array gets the following name (expressed in
 * C preprocessor terms) from those arguments:
 *
 * ossl_##impl##_to_##kind##_##output##_encoder_functions
 */
#define MAKE_ENCODER(impl, type, kind, output)                              \
    static OSSL_FUNC_encoder_import_object_fn                               \
    impl##_to_##kind##_##output##_import_object;                            \
    static OSSL_FUNC_encoder_free_object_fn                                 \
    impl##_to_##kind##_##output##_free_object;                              \
    static OSSL_FUNC_encoder_encode_fn                                      \
    impl##_to_##kind##_##output##_encode;                                   \
                                                                            \
    static void *                                                           \
    impl##_to_##kind##_##output##_import_object(void *vctx, int selection,  \
                                                const OSSL_PARAM params[])  \
    {                                                                       \
        KEY2ANY_CTX *ctx = vctx;                                            \
                                                                            \
        return ossl_prov_import_key(ossl_##impl##_keymgmt_functions,        \
                                    ctx->provctx, selection, params);       \
    }                                                                       \
    static void impl##_to_##kind##_##output##_free_object(void *key)        \
    {                                                                       \
        ossl_prov_free_key(ossl_##impl##_keymgmt_functions, key);           \
    }                                                                       \
    static int impl##_to_##kind##_##output##_does_selection(void *ctx,      \
                                                            int selection)  \
    {                                                                       \
        return key2any_check_selection(selection,                           \
                                       DO_##kind##_selection_mask);         \
    }                                                                       \
    static int                                                              \
    impl##_to_##kind##_##output##_encode(void *ctx, OSSL_CORE_BIO *cout,    \
                                         const void *key,                   \
                                         const OSSL_PARAM key_abstract[],   \
                                         int selection,                     \
                                         OSSL_PASSPHRASE_CALLBACK *cb,      \
                                         void *cbarg)                       \
    {                                                                       \
        /* We don't deal with abstract objects */                           \
        if (key_abstract != NULL) {                                         \
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);         \
            return 0;                                                       \
        }                                                                   \
        DO_##kind(impl, type, output)                                       \
                                                                            \
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);             \
        return 0;                                                           \
    }                                                                       \
    const OSSL_DISPATCH                                                     \
    ossl_##impl##_to_##kind##_##output##_encoder_functions[] = {            \
        { OSSL_FUNC_ENCODER_NEWCTX,                                         \
          (void (*)(void))key2any_newctx },                                 \
        { OSSL_FUNC_ENCODER_FREECTX,                                        \
          (void (*)(void))key2any_freectx },                                \
        { OSSL_FUNC_ENCODER_SETTABLE_CTX_PARAMS,                            \
          (void (*)(void))key2any_settable_ctx_params },                    \
        { OSSL_FUNC_ENCODER_SET_CTX_PARAMS,                                 \
          (void (*)(void))key2any_set_ctx_params },                         \
        { OSSL_FUNC_ENCODER_DOES_SELECTION,                                 \
          (void (*)(void))impl##_to_##kind##_##output##_does_selection },   \
        { OSSL_FUNC_ENCODER_IMPORT_OBJECT,                                  \
          (void (*)(void))impl##_to_##kind##_##output##_import_object },    \
        { OSSL_FUNC_ENCODER_FREE_OBJECT,                                    \
          (void (*)(void))impl##_to_##kind##_##output##_free_object },      \
        { OSSL_FUNC_ENCODER_ENCODE,                                         \
          (void (*)(void))impl##_to_##kind##_##output##_encode },           \
        OSSL_DISPATCH_END                                                   \
    }

/*
 * Replacements for i2d_{TYPE}PrivateKey, i2d_{TYPE}PublicKey,
 * i2d_{TYPE}params, as they exist.
 */
MAKE_ENCODER(rsa, rsa, type_specific_keypair, der);
#ifndef OPENSSL_NO_DH
MAKE_ENCODER(dh, dh, type_specific_params, der);
MAKE_ENCODER(dhx, dh, type_specific_params, der);
#endif
#ifndef OPENSSL_NO_DSA
MAKE_ENCODER(dsa, dsa, type_specific, der);
#endif
#ifndef OPENSSL_NO_EC
MAKE_ENCODER(ec, ec, type_specific_no_pub, der);
# ifndef OPENSSL_NO_SM2
MAKE_ENCODER(sm2, ec, type_specific_no_pub, der);
# endif
#endif

/*
 * Replacements for PEM_write_bio_{TYPE}PrivateKey,
 * PEM_write_bio_{TYPE}PublicKey, PEM_write_bio_{TYPE}params, as they exist.
 */
MAKE_ENCODER(rsa, rsa, type_specific_keypair, pem);
#ifndef OPENSSL_NO_DH
MAKE_ENCODER(dh, dh, type_specific_params, pem);
MAKE_ENCODER(dhx, dh, type_specific_params, pem);
#endif
#ifndef OPENSSL_NO_DSA
MAKE_ENCODER(dsa, dsa, type_specific, pem);
#endif
#ifndef OPENSSL_NO_EC
MAKE_ENCODER(ec, ec, type_specific_no_pub, pem);
# ifndef OPENSSL_NO_SM2
MAKE_ENCODER(sm2, ec, type_specific_no_pub, pem);
# endif
#endif

/*
 * PKCS#8 and SubjectPublicKeyInfo support.  This may duplicate some of the
 * implementations specified above, but are more specific.
 * The SubjectPublicKeyInfo implementations also replace the
 * PEM_write_bio_{TYPE}_PUBKEY functions.
 * For PEM, these are expected to be used by PEM_write_bio_PrivateKey(),
 * PEM_write_bio_PUBKEY() and PEM_write_bio_Parameters().
 */
MAKE_ENCODER(rsa, rsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(rsa, rsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(rsa, rsa, PrivateKeyInfo, der);
MAKE_ENCODER(rsa, rsa, PrivateKeyInfo, pem);
MAKE_ENCODER(rsa, rsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(rsa, rsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(rsapss, rsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(rsapss, rsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(rsapss, rsa, PrivateKeyInfo, der);
MAKE_ENCODER(rsapss, rsa, PrivateKeyInfo, pem);
MAKE_ENCODER(rsapss, rsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(rsapss, rsa, SubjectPublicKeyInfo, pem);
#ifndef OPENSSL_NO_DH
MAKE_ENCODER(dh, dh, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(dh, dh, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(dh, dh, PrivateKeyInfo, der);
MAKE_ENCODER(dh, dh, PrivateKeyInfo, pem);
MAKE_ENCODER(dh, dh, SubjectPublicKeyInfo, der);
MAKE_ENCODER(dh, dh, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(dhx, dh, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(dhx, dh, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(dhx, dh, PrivateKeyInfo, der);
MAKE_ENCODER(dhx, dh, PrivateKeyInfo, pem);
MAKE_ENCODER(dhx, dh, SubjectPublicKeyInfo, der);
MAKE_ENCODER(dhx, dh, SubjectPublicKeyInfo, pem);
#endif
#ifndef OPENSSL_NO_DSA
MAKE_ENCODER(dsa, dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(dsa, dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(dsa, dsa, PrivateKeyInfo, der);
MAKE_ENCODER(dsa, dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(dsa, dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(dsa, dsa, SubjectPublicKeyInfo, pem);
#endif
#ifndef OPENSSL_NO_EC
MAKE_ENCODER(ec, ec, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ec, ec, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ec, ec, PrivateKeyInfo, der);
MAKE_ENCODER(ec, ec, PrivateKeyInfo, pem);
MAKE_ENCODER(ec, ec, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ec, ec, SubjectPublicKeyInfo, pem);
# ifndef OPENSSL_NO_SM2
MAKE_ENCODER(sm2, ec, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(sm2, ec, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(sm2, ec, PrivateKeyInfo, der);
MAKE_ENCODER(sm2, ec, PrivateKeyInfo, pem);
MAKE_ENCODER(sm2, ec, SubjectPublicKeyInfo, der);
MAKE_ENCODER(sm2, ec, SubjectPublicKeyInfo, pem);
# endif
# ifndef OPENSSL_NO_ECX
MAKE_ENCODER(ed25519, ecx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ed25519, ecx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ed25519, ecx, PrivateKeyInfo, der);
MAKE_ENCODER(ed25519, ecx, PrivateKeyInfo, pem);
MAKE_ENCODER(ed25519, ecx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ed25519, ecx, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(ed448, ecx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ed448, ecx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ed448, ecx, PrivateKeyInfo, der);
MAKE_ENCODER(ed448, ecx, PrivateKeyInfo, pem);
MAKE_ENCODER(ed448, ecx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ed448, ecx, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(x25519, ecx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(x25519, ecx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(x25519, ecx, PrivateKeyInfo, der);
MAKE_ENCODER(x25519, ecx, PrivateKeyInfo, pem);
MAKE_ENCODER(x25519, ecx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(x25519, ecx, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(x448, ecx, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(x448, ecx, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(x448, ecx, PrivateKeyInfo, der);
MAKE_ENCODER(x448, ecx, PrivateKeyInfo, pem);
MAKE_ENCODER(x448, ecx, SubjectPublicKeyInfo, der);
MAKE_ENCODER(x448, ecx, SubjectPublicKeyInfo, pem);
# endif
#endif
#ifndef OPENSSL_NO_SLH_DSA
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_sha2_128s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_128f, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_192f, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_sha2_256f, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(slh_dsa_shake_128s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_128f, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_192f, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256s, slh_dsa, SubjectPublicKeyInfo, pem);
MAKE_ENCODER(slh_dsa_shake_256f, slh_dsa, SubjectPublicKeyInfo, pem);
#endif /* OPENSSL_NO_SLH_DSA */

#ifndef OPENSSL_NO_ML_KEM
MAKE_ENCODER(ml_kem_512, ml_kem, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_512, ml_kem, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_512, ml_kem, PrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_512, ml_kem, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_512, ml_kem, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_kem_512, ml_kem, SubjectPublicKeyInfo, pem);

MAKE_ENCODER(ml_kem_768, ml_kem, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_768, ml_kem, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_768, ml_kem, PrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_768, ml_kem, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_768, ml_kem, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_kem_768, ml_kem, SubjectPublicKeyInfo, pem);

MAKE_ENCODER(ml_kem_1024, ml_kem, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_1024, ml_kem, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_1024, ml_kem, PrivateKeyInfo, der);
MAKE_ENCODER(ml_kem_1024, ml_kem, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_kem_1024, ml_kem, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_kem_1024, ml_kem, SubjectPublicKeyInfo, pem);
#endif

/*
 * Support for key type specific output formats.  Not all key types have
 * this, we only aim to duplicate what is available in 1.1.1 as
 * i2d_TYPEPrivateKey(), i2d_TYPEPublicKey() and i2d_TYPEparams().
 * For example, there are no publicly available i2d_ function for
 * ED25519, ED448, X25519 or X448, and they therefore only have PKCS#8
 * and SubjectPublicKeyInfo implementations as implemented above.
 */
MAKE_ENCODER(rsa, rsa, RSA, der);
MAKE_ENCODER(rsa, rsa, RSA, pem);
#ifndef OPENSSL_NO_DH
MAKE_ENCODER(dh, dh, DH, der);
MAKE_ENCODER(dh, dh, DH, pem);
MAKE_ENCODER(dhx, dh, DHX, der);
MAKE_ENCODER(dhx, dh, DHX, pem);
#endif
#ifndef OPENSSL_NO_DSA
MAKE_ENCODER(dsa, dsa, DSA, der);
MAKE_ENCODER(dsa, dsa, DSA, pem);
#endif
#ifndef OPENSSL_NO_EC
MAKE_ENCODER(ec, ec, EC, der);
MAKE_ENCODER(ec, ec, EC, pem);
# ifndef OPENSSL_NO_SM2
MAKE_ENCODER(sm2, ec, SM2, der);
MAKE_ENCODER(sm2, ec, SM2, pem);
# endif
#endif

/* Convenience structure names */
MAKE_ENCODER(rsa, rsa, PKCS1, der);
MAKE_ENCODER(rsa, rsa, PKCS1, pem);
MAKE_ENCODER(rsapss, rsa, PKCS1, der);
MAKE_ENCODER(rsapss, rsa, PKCS1, pem);
#ifndef OPENSSL_NO_DH
MAKE_ENCODER(dh, dh, PKCS3, der); /* parameters only */
MAKE_ENCODER(dh, dh, PKCS3, pem); /* parameters only */
MAKE_ENCODER(dhx, dh, X9_42, der); /* parameters only */
MAKE_ENCODER(dhx, dh, X9_42, pem); /* parameters only */
#endif
#ifndef OPENSSL_NO_EC
MAKE_ENCODER(ec, ec, X9_62, der);
MAKE_ENCODER(ec, ec, X9_62, pem);
#endif

#ifndef OPENSSL_NO_ML_DSA
MAKE_ENCODER(ml_dsa_44, ml_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_44, ml_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_44, ml_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_44, ml_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_44, ml_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_dsa_44, ml_dsa, SubjectPublicKeyInfo, pem);

MAKE_ENCODER(ml_dsa_65, ml_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_65, ml_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_65, ml_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_65, ml_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_65, ml_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_dsa_65, ml_dsa, SubjectPublicKeyInfo, pem);

MAKE_ENCODER(ml_dsa_87, ml_dsa, EncryptedPrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_87, ml_dsa, EncryptedPrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_87, ml_dsa, PrivateKeyInfo, der);
MAKE_ENCODER(ml_dsa_87, ml_dsa, PrivateKeyInfo, pem);
MAKE_ENCODER(ml_dsa_87, ml_dsa, SubjectPublicKeyInfo, der);
MAKE_ENCODER(ml_dsa_87, ml_dsa, SubjectPublicKeyInfo, pem);
#endif /* OPENSSL_NO_ML_DSA */
