/*
 * Copyright 2006-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <openssl/x509.h>
#include <openssl/ec.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include <openssl/rand.h>
#include "internal/cryptlib.h"
#include "internal/provider.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/ecx.h"
#include "ec_local.h"
#include "curve448/curve448_local.h"
#include "ecx_backend.h"

static int ecx_pub_encode(X509_PUBKEY *pk, const EVP_PKEY *pkey)
{
    const ECX_KEY *ecxkey = pkey->pkey.ecx;
    unsigned char *penc;

    if (ecxkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    penc = OPENSSL_memdup(ecxkey->pubkey, KEYLEN(pkey));
    if (penc == NULL)
        return 0;

    if (!X509_PUBKEY_set0_param(pk, OBJ_nid2obj(pkey->ameth->pkey_id),
                                V_ASN1_UNDEF, NULL, penc, KEYLEN(pkey))) {
        OPENSSL_free(penc);
        ERR_raise(ERR_LIB_EC, ERR_R_X509_LIB);
        return 0;
    }
    return 1;
}

static int ecx_pub_decode(EVP_PKEY *pkey, const X509_PUBKEY *pubkey)
{
    const unsigned char *p;
    int pklen;
    X509_ALGOR *palg;
    ECX_KEY *ecx;
    int ret = 0;

    if (!X509_PUBKEY_get0_param(NULL, &p, &pklen, &palg, pubkey))
        return 0;
    ecx = ossl_ecx_key_op(palg, p, pklen, pkey->ameth->pkey_id,
                          KEY_OP_PUBLIC, NULL, NULL);
    if (ecx != NULL) {
        ret = 1;
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, ecx);
    }
    return ret;
}

static int ecx_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b)
{
    const ECX_KEY *akey = a->pkey.ecx;
    const ECX_KEY *bkey = b->pkey.ecx;

    if (akey == NULL || bkey == NULL)
        return -2;

    return CRYPTO_memcmp(akey->pubkey, bkey->pubkey, KEYLEN(a)) == 0;
}

static int ecx_priv_decode_ex(EVP_PKEY *pkey, const PKCS8_PRIV_KEY_INFO *p8,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    int ret = 0;
    ECX_KEY *ecx = ossl_ecx_key_from_pkcs8(p8, libctx, propq);

    if (ecx != NULL) {
        ret = 1;
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, ecx);
    }

    return ret;
}

static int ecx_priv_encode(PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pkey)
{
    const ECX_KEY *ecxkey = pkey->pkey.ecx;
    ASN1_OCTET_STRING oct;
    unsigned char *penc = NULL;
    int penclen;

    if (ecxkey == NULL || ecxkey->privkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_PRIVATE_KEY);
        return 0;
    }

    oct.data = ecxkey->privkey;
    oct.length = KEYLEN(pkey);
    oct.flags = 0;

    penclen = i2d_ASN1_OCTET_STRING(&oct, &penc);
    if (penclen < 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_ASN1_LIB);
        return 0;
    }

    if (!PKCS8_pkey_set0(p8, OBJ_nid2obj(pkey->ameth->pkey_id), 0,
                         V_ASN1_UNDEF, NULL, penc, penclen)) {
        OPENSSL_clear_free(penc, penclen);
        ERR_raise(ERR_LIB_EC, ERR_R_ASN1_LIB);
        return 0;
    }

    return 1;
}

static int ecx_size(const EVP_PKEY *pkey)
{
    return KEYLEN(pkey);
}

static int ecx_bits(const EVP_PKEY *pkey)
{
    if (IS25519(pkey->ameth->pkey_id)) {
        return X25519_BITS;
    } else if (ISX448(pkey->ameth->pkey_id)) {
        return X448_BITS;
    } else {
        return ED448_BITS;
    }
}

static int ecx_security_bits(const EVP_PKEY *pkey)
{
    if (IS25519(pkey->ameth->pkey_id)) {
        return X25519_SECURITY_BITS;
    } else {
        return X448_SECURITY_BITS;
    }
}

static void ecx_free(EVP_PKEY *pkey)
{
    ossl_ecx_key_free(pkey->pkey.ecx);
}

/* "parameters" are always equal */
static int ecx_cmp_parameters(const EVP_PKEY *a, const EVP_PKEY *b)
{
    return 1;
}

static int ecx_key_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                         ASN1_PCTX *ctx, ecx_key_op_t op)
{
    const ECX_KEY *ecxkey = pkey->pkey.ecx;
    const char *nm = OBJ_nid2ln(pkey->ameth->pkey_id);

    if (op == KEY_OP_PRIVATE) {
        if (ecxkey == NULL || ecxkey->privkey == NULL) {
            if (BIO_printf(bp, "%*s<INVALID PRIVATE KEY>\n", indent, "") <= 0)
                return 0;
            return 1;
        }
        if (BIO_printf(bp, "%*s%s Private-Key:\n", indent, "", nm) <= 0)
            return 0;
        if (BIO_printf(bp, "%*spriv:\n", indent, "") <= 0)
            return 0;
        if (ASN1_buf_print(bp, ecxkey->privkey, KEYLEN(pkey),
                           indent + 4) == 0)
            return 0;
    } else {
        if (ecxkey == NULL) {
            if (BIO_printf(bp, "%*s<INVALID PUBLIC KEY>\n", indent, "") <= 0)
                return 0;
            return 1;
        }
        if (BIO_printf(bp, "%*s%s Public-Key:\n", indent, "", nm) <= 0)
            return 0;
    }
    if (BIO_printf(bp, "%*spub:\n", indent, "") <= 0)
        return 0;

    if (ASN1_buf_print(bp, ecxkey->pubkey, KEYLEN(pkey),
                       indent + 4) == 0)
        return 0;
    return 1;
}

static int ecx_priv_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                          ASN1_PCTX *ctx)
{
    return ecx_key_print(bp, pkey, indent, ctx, KEY_OP_PRIVATE);
}

static int ecx_pub_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                         ASN1_PCTX *ctx)
{
    return ecx_key_print(bp, pkey, indent, ctx, KEY_OP_PUBLIC);
}

static int ecx_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    switch (op) {

    case ASN1_PKEY_CTRL_SET1_TLS_ENCPT: {
        ECX_KEY *ecx = ossl_ecx_key_op(NULL, arg2, arg1, pkey->ameth->pkey_id,
                                       KEY_OP_PUBLIC, NULL, NULL);

        if (ecx != NULL) {
            EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, ecx);
            return 1;
        }
        return 0;
    }
    case ASN1_PKEY_CTRL_GET1_TLS_ENCPT:
        if (pkey->pkey.ecx != NULL) {
            unsigned char **ppt = arg2;

            *ppt = OPENSSL_memdup(pkey->pkey.ecx->pubkey, KEYLEN(pkey));
            if (*ppt != NULL)
                return KEYLEN(pkey);
        }
        return 0;

    default:
        return -2;

    }
}

static int ecd_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    switch (op) {
    case ASN1_PKEY_CTRL_DEFAULT_MD_NID:
        /* We currently only support Pure EdDSA which takes no digest */
        *(int *)arg2 = NID_undef;
        return 2;

    default:
        return -2;

    }
}

static int ecx_set_priv_key(EVP_PKEY *pkey, const unsigned char *priv,
                            size_t len)
{
    OSSL_LIB_CTX *libctx = NULL;
    ECX_KEY *ecx = NULL;

    if (pkey->keymgmt != NULL)
        libctx = ossl_provider_libctx(EVP_KEYMGMT_get0_provider(pkey->keymgmt));

    ecx = ossl_ecx_key_op(NULL, priv, len, pkey->ameth->pkey_id,
                          KEY_OP_PRIVATE, libctx, NULL);

    if (ecx != NULL) {
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, ecx);
        return 1;
    }
    return 0;
}

static int ecx_set_pub_key(EVP_PKEY *pkey, const unsigned char *pub, size_t len)
{
    OSSL_LIB_CTX *libctx = NULL;
    ECX_KEY *ecx = NULL;

    if (pkey->keymgmt != NULL)
        libctx = ossl_provider_libctx(EVP_KEYMGMT_get0_provider(pkey->keymgmt));

    ecx = ossl_ecx_key_op(NULL, pub, len, pkey->ameth->pkey_id,
                          KEY_OP_PUBLIC, libctx, NULL);

    if (ecx != NULL) {
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, ecx);
        return 1;
    }
    return 0;
}

static int ecx_get_priv_key(const EVP_PKEY *pkey, unsigned char *priv,
                            size_t *len)
{
    const ECX_KEY *key = pkey->pkey.ecx;

    if (priv == NULL) {
        *len = KEYLENID(pkey->ameth->pkey_id);
        return 1;
    }

    if (key == NULL
            || key->privkey == NULL
            || *len < (size_t)KEYLENID(pkey->ameth->pkey_id))
        return 0;

    *len = KEYLENID(pkey->ameth->pkey_id);
    memcpy(priv, key->privkey, *len);

    return 1;
}

static int ecx_get_pub_key(const EVP_PKEY *pkey, unsigned char *pub,
                           size_t *len)
{
    const ECX_KEY *key = pkey->pkey.ecx;

    if (pub == NULL) {
        *len = KEYLENID(pkey->ameth->pkey_id);
        return 1;
    }

    if (key == NULL
            || *len < (size_t)KEYLENID(pkey->ameth->pkey_id))
        return 0;

    *len = KEYLENID(pkey->ameth->pkey_id);
    memcpy(pub, key->pubkey, *len);

    return 1;
}

static size_t ecx_pkey_dirty_cnt(const EVP_PKEY *pkey)
{
    /*
     * We provide no mechanism to "update" an ECX key once it has been set,
     * therefore we do not have to maintain a dirty count.
     */
    return 1;
}

static int ecx_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                              OSSL_FUNC_keymgmt_import_fn *importer,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    const ECX_KEY *key = from->pkey.ecx;
    OSSL_PARAM_BLD *tmpl = OSSL_PARAM_BLD_new();
    OSSL_PARAM *params = NULL;
    int selection = 0;
    int rv = 0;

    if (tmpl == NULL)
        return 0;

    /* A key must at least have a public part */
    if (!OSSL_PARAM_BLD_push_octet_string(tmpl, OSSL_PKEY_PARAM_PUB_KEY,
                                          key->pubkey, key->keylen))
        goto err;
    selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;

    if (key->privkey != NULL) {
        if (!OSSL_PARAM_BLD_push_octet_string(tmpl,
                                              OSSL_PKEY_PARAM_PRIV_KEY,
                                              key->privkey, key->keylen))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_PRIVATE_KEY;
    }

    params = OSSL_PARAM_BLD_to_param(tmpl);

    /* We export, the provider imports */
    rv = importer(to_keydata, selection, params);

 err:
    OSSL_PARAM_BLD_free(tmpl);
    OSSL_PARAM_free(params);
    return rv;
}

static int ecx_generic_import_from(const OSSL_PARAM params[], void *vpctx,
                                   int keytype)
{
    EVP_PKEY_CTX *pctx = vpctx;
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    ECX_KEY *ecx = ossl_ecx_key_new(pctx->libctx, KEYNID2TYPE(keytype), 0,
                                    pctx->propquery);

    if (ecx == NULL) {
        ERR_raise(ERR_LIB_DH, ERR_R_EC_LIB);
        return 0;
    }

    if (!ossl_ecx_key_fromdata(ecx, params, 1)
        || !EVP_PKEY_assign(pkey, keytype, ecx)) {
        ossl_ecx_key_free(ecx);
        return 0;
    }
    return 1;
}

static int ecx_pkey_copy(EVP_PKEY *to, EVP_PKEY *from)
{
    ECX_KEY *ecx = from->pkey.ecx, *dupkey = NULL;
    int ret;

    if (ecx != NULL) {
        dupkey = ossl_ecx_key_dup(ecx, OSSL_KEYMGMT_SELECT_ALL);
        if (dupkey == NULL)
            return 0;
    }

    ret = EVP_PKEY_assign(to, from->type, dupkey);
    if (!ret)
        ossl_ecx_key_free(dupkey);
    return ret;
}

static int x25519_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return ecx_generic_import_from(params, vpctx, EVP_PKEY_X25519);
}

const EVP_PKEY_ASN1_METHOD ossl_ecx25519_asn1_meth = {
    EVP_PKEY_X25519,
    EVP_PKEY_X25519,
    0,
    "X25519",
    "OpenSSL X25519 algorithm",

    ecx_pub_decode,
    ecx_pub_encode,
    ecx_pub_cmp,
    ecx_pub_print,

    NULL,
    ecx_priv_encode,
    ecx_priv_print,

    ecx_size,
    ecx_bits,
    ecx_security_bits,

    0, 0, 0, 0,
    ecx_cmp_parameters,
    0, 0,

    ecx_free,
    ecx_ctrl,
    NULL,
    NULL,

    NULL,
    NULL,
    NULL,

    NULL,
    NULL,
    NULL,

    ecx_set_priv_key,
    ecx_set_pub_key,
    ecx_get_priv_key,
    ecx_get_pub_key,
    ecx_pkey_dirty_cnt,
    ecx_pkey_export_to,
    x25519_import_from,
    ecx_pkey_copy,

    ecx_priv_decode_ex
};

static int x448_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return ecx_generic_import_from(params, vpctx, EVP_PKEY_X448);
}

const EVP_PKEY_ASN1_METHOD ossl_ecx448_asn1_meth = {
    EVP_PKEY_X448,
    EVP_PKEY_X448,
    0,
    "X448",
    "OpenSSL X448 algorithm",

    ecx_pub_decode,
    ecx_pub_encode,
    ecx_pub_cmp,
    ecx_pub_print,

    NULL,
    ecx_priv_encode,
    ecx_priv_print,

    ecx_size,
    ecx_bits,
    ecx_security_bits,

    0, 0, 0, 0,
    ecx_cmp_parameters,
    0, 0,

    ecx_free,
    ecx_ctrl,
    NULL,
    NULL,

    NULL,
    NULL,
    NULL,

    NULL,
    NULL,
    NULL,

    ecx_set_priv_key,
    ecx_set_pub_key,
    ecx_get_priv_key,
    ecx_get_pub_key,
    ecx_pkey_dirty_cnt,
    ecx_pkey_export_to,
    x448_import_from,
    ecx_pkey_copy,

    ecx_priv_decode_ex
};

static int ecd_size25519(const EVP_PKEY *pkey)
{
    return ED25519_SIGSIZE;
}

static int ecd_size448(const EVP_PKEY *pkey)
{
    return ED448_SIGSIZE;
}

static int ecd_item_verify(EVP_MD_CTX *ctx, const ASN1_ITEM *it,
                           const void *asn, const X509_ALGOR *sigalg,
                           const ASN1_BIT_STRING *str, EVP_PKEY *pkey)
{
    const ASN1_OBJECT *obj;
    int ptype;
    int nid;

    /* Sanity check: make sure it is ED25519/ED448 with absent parameters */
    X509_ALGOR_get0(&obj, &ptype, NULL, sigalg);
    nid = OBJ_obj2nid(obj);
    if ((nid != NID_ED25519 && nid != NID_ED448) || ptype != V_ASN1_UNDEF) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
        return 0;
    }

    if (!EVP_DigestVerifyInit(ctx, NULL, NULL, NULL, pkey))
        return 0;

    return 2;
}

static int ecd_item_sign(X509_ALGOR *alg1, X509_ALGOR *alg2, int nid)
{
    /* Note that X509_ALGOR_set0(..., ..., V_ASN1_UNDEF, ...) cannot fail */
    /* Set algorithms identifiers */
    (void)X509_ALGOR_set0(alg1, OBJ_nid2obj(nid), V_ASN1_UNDEF, NULL);
    if (alg2 != NULL)
        (void)X509_ALGOR_set0(alg2, OBJ_nid2obj(nid), V_ASN1_UNDEF, NULL);
    /* Algorithm identifiers set: carry on as normal */
    return 3;
}

static int ecd_item_sign25519(EVP_MD_CTX *ctx, const ASN1_ITEM *it,
                              const void *asn,
                              X509_ALGOR *alg1, X509_ALGOR *alg2,
                              ASN1_BIT_STRING *str)
{
    return ecd_item_sign(alg1, alg2, NID_ED25519);
}

static int ecd_sig_info_set25519(X509_SIG_INFO *siginf, const X509_ALGOR *alg,
                                 const ASN1_STRING *sig)
{
    X509_SIG_INFO_set(siginf, NID_undef, NID_ED25519, X25519_SECURITY_BITS,
                      X509_SIG_INFO_TLS);
    return 1;
}

static int ecd_item_sign448(EVP_MD_CTX *ctx, const ASN1_ITEM *it,
                            const void *asn,
                            X509_ALGOR *alg1, X509_ALGOR *alg2,
                            ASN1_BIT_STRING *str)
{
    return ecd_item_sign(alg1, alg2, NID_ED448);
}

static int ecd_sig_info_set448(X509_SIG_INFO *siginf, const X509_ALGOR *alg,
                               const ASN1_STRING *sig)
{
    X509_SIG_INFO_set(siginf, NID_undef, NID_ED448, X448_SECURITY_BITS,
                      X509_SIG_INFO_TLS);
    return 1;
}

static int ed25519_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return ecx_generic_import_from(params, vpctx, EVP_PKEY_ED25519);
}

const EVP_PKEY_ASN1_METHOD ossl_ed25519_asn1_meth = {
    EVP_PKEY_ED25519,
    EVP_PKEY_ED25519,
    0,
    "ED25519",
    "OpenSSL ED25519 algorithm",

    ecx_pub_decode,
    ecx_pub_encode,
    ecx_pub_cmp,
    ecx_pub_print,

    NULL,
    ecx_priv_encode,
    ecx_priv_print,

    ecd_size25519,
    ecx_bits,
    ecx_security_bits,

    0, 0, 0, 0,
    ecx_cmp_parameters,
    0, 0,

    ecx_free,
    ecd_ctrl,
    NULL,
    NULL,
    ecd_item_verify,
    ecd_item_sign25519,
    ecd_sig_info_set25519,

    NULL,
    NULL,
    NULL,

    ecx_set_priv_key,
    ecx_set_pub_key,
    ecx_get_priv_key,
    ecx_get_pub_key,
    ecx_pkey_dirty_cnt,
    ecx_pkey_export_to,
    ed25519_import_from,
    ecx_pkey_copy,

    ecx_priv_decode_ex
};

static int ed448_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return ecx_generic_import_from(params, vpctx, EVP_PKEY_ED448);
}

const EVP_PKEY_ASN1_METHOD ossl_ed448_asn1_meth = {
    EVP_PKEY_ED448,
    EVP_PKEY_ED448,
    0,
    "ED448",
    "OpenSSL ED448 algorithm",

    ecx_pub_decode,
    ecx_pub_encode,
    ecx_pub_cmp,
    ecx_pub_print,

    NULL,
    ecx_priv_encode,
    ecx_priv_print,

    ecd_size448,
    ecx_bits,
    ecx_security_bits,

    0, 0, 0, 0,
    ecx_cmp_parameters,
    0, 0,

    ecx_free,
    ecd_ctrl,
    NULL,
    NULL,
    ecd_item_verify,
    ecd_item_sign448,
    ecd_sig_info_set448,

    NULL,
    NULL,
    NULL,

    ecx_set_priv_key,
    ecx_set_pub_key,
    ecx_get_priv_key,
    ecx_get_pub_key,
    ecx_pkey_dirty_cnt,
    ecx_pkey_export_to,
    ed448_import_from,
    ecx_pkey_copy,

    ecx_priv_decode_ex
};

static int pkey_ecx_keygen(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    ECX_KEY *ecx = ossl_ecx_key_op(NULL, NULL, 0, ctx->pmeth->pkey_id,
                                   KEY_OP_KEYGEN, NULL, NULL);

    if (ecx != NULL) {
        EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, ecx);
        return 1;
    }
    return 0;
}

static int validate_ecx_derive(EVP_PKEY_CTX *ctx, unsigned char *key,
                                          size_t *keylen,
                                          const unsigned char **privkey,
                                          const unsigned char **pubkey)
{
    const ECX_KEY *ecxkey, *peerkey;

    if (ctx->pkey == NULL || ctx->peerkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_KEYS_NOT_SET);
        return 0;
    }
    ecxkey = evp_pkey_get_legacy(ctx->pkey);
    peerkey = evp_pkey_get_legacy(ctx->peerkey);
    if (ecxkey == NULL || ecxkey->privkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_PRIVATE_KEY);
        return 0;
    }
    if (peerkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_PEER_KEY);
        return 0;
    }
    *privkey = ecxkey->privkey;
    *pubkey = peerkey->pubkey;

    return 1;
}

static int pkey_ecx_derive25519(EVP_PKEY_CTX *ctx, unsigned char *key,
                                size_t *keylen)
{
    const unsigned char *privkey, *pubkey;

    if (!validate_ecx_derive(ctx, key, keylen, &privkey, &pubkey)
            || (key != NULL
                && ossl_x25519(key, privkey, pubkey) == 0))
        return 0;
    *keylen = X25519_KEYLEN;
    return 1;
}

static int pkey_ecx_derive448(EVP_PKEY_CTX *ctx, unsigned char *key,
                              size_t *keylen)
{
    const unsigned char *privkey, *pubkey;

    if (!validate_ecx_derive(ctx, key, keylen, &privkey, &pubkey)
            || (key != NULL
                && ossl_x448(key, privkey, pubkey) == 0))
        return 0;
    *keylen = X448_KEYLEN;
    return 1;
}

static int pkey_ecx_ctrl(EVP_PKEY_CTX *ctx, int type, int p1, void *p2)
{
    /* Only need to handle peer key for derivation */
    if (type == EVP_PKEY_CTRL_PEER_KEY)
        return 1;
    return -2;
}

static const EVP_PKEY_METHOD ecx25519_pkey_meth = {
    EVP_PKEY_X25519,
    0, 0, 0, 0, 0, 0, 0,
    pkey_ecx_keygen,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecx_derive25519,
    pkey_ecx_ctrl,
    0
};

static const EVP_PKEY_METHOD ecx448_pkey_meth = {
    EVP_PKEY_X448,
    0, 0, 0, 0, 0, 0, 0,
    pkey_ecx_keygen,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecx_derive448,
    pkey_ecx_ctrl,
    0
};

static int pkey_ecd_digestsign25519(EVP_MD_CTX *ctx, unsigned char *sig,
                                    size_t *siglen, const unsigned char *tbs,
                                    size_t tbslen)
{
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (sig == NULL) {
        *siglen = ED25519_SIGSIZE;
        return 1;
    }
    if (*siglen < ED25519_SIGSIZE) {
        ERR_raise(ERR_LIB_EC, EC_R_BUFFER_TOO_SMALL);
        return 0;
    }

    if (ossl_ed25519_sign(sig, tbs, tbslen, edkey->pubkey, edkey->privkey,
                          0, 0, 0,
                          NULL, 0,
                          NULL, NULL) == 0)
        return 0;
    *siglen = ED25519_SIGSIZE;
    return 1;
}

static int pkey_ecd_digestsign448(EVP_MD_CTX *ctx, unsigned char *sig,
                                  size_t *siglen, const unsigned char *tbs,
                                  size_t tbslen)
{
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (sig == NULL) {
        *siglen = ED448_SIGSIZE;
        return 1;
    }
    if (*siglen < ED448_SIGSIZE) {
        ERR_raise(ERR_LIB_EC, EC_R_BUFFER_TOO_SMALL);
        return 0;
    }

    if (ossl_ed448_sign(edkey->libctx, sig, tbs, tbslen, edkey->pubkey,
                        edkey->privkey, NULL, 0, 0, edkey->propq) == 0)
        return 0;
    *siglen = ED448_SIGSIZE;
    return 1;
}

static int pkey_ecd_digestverify25519(EVP_MD_CTX *ctx, const unsigned char *sig,
                                      size_t siglen, const unsigned char *tbs,
                                      size_t tbslen)
{
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (siglen != ED25519_SIGSIZE)
        return 0;

    return ossl_ed25519_verify(tbs, tbslen, sig, edkey->pubkey,
                               0, 0, 0,
                               NULL, 0,
                               edkey->libctx, edkey->propq);
}

static int pkey_ecd_digestverify448(EVP_MD_CTX *ctx, const unsigned char *sig,
                                    size_t siglen, const unsigned char *tbs,
                                    size_t tbslen)
{
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (siglen != ED448_SIGSIZE)
        return 0;

    return ossl_ed448_verify(edkey->libctx, tbs, tbslen, sig, edkey->pubkey,
                             NULL, 0, 0, edkey->propq);
}

static int pkey_ecd_ctrl(EVP_PKEY_CTX *ctx, int type, int p1, void *p2)
{
    switch (type) {
    case EVP_PKEY_CTRL_MD:
        /* Only NULL allowed as digest */
        if (p2 == NULL || (const EVP_MD *)p2 == EVP_md_null())
            return 1;
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_DIGEST_TYPE);
        return 0;

    case EVP_PKEY_CTRL_DIGESTINIT:
        return 1;
    }
    return -2;
}

static const EVP_PKEY_METHOD ed25519_pkey_meth = {
    EVP_PKEY_ED25519, EVP_PKEY_FLAG_SIGCTX_CUSTOM,
    0, 0, 0, 0, 0, 0,
    pkey_ecx_keygen,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecd_ctrl,
    0,
    pkey_ecd_digestsign25519,
    pkey_ecd_digestverify25519
};

static const EVP_PKEY_METHOD ed448_pkey_meth = {
    EVP_PKEY_ED448, EVP_PKEY_FLAG_SIGCTX_CUSTOM,
    0, 0, 0, 0, 0, 0,
    pkey_ecx_keygen,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecd_ctrl,
    0,
    pkey_ecd_digestsign448,
    pkey_ecd_digestverify448
};

#ifdef S390X_EC_ASM
# include "s390x_arch.h"

static int s390x_pkey_ecx_keygen25519(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static const unsigned char generator[] = {
        0x09, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    ECX_KEY *key = ossl_ecx_key_new(ctx->libctx, ECX_KEY_TYPE_X25519, 1,
                                    ctx->propquery);
    unsigned char *privkey = NULL, *pubkey;

    if (key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (RAND_priv_bytes_ex(ctx->libctx, privkey, X25519_KEYLEN, 0) <= 0)
        goto err;

    privkey[0] &= 248;
    privkey[31] &= 127;
    privkey[31] |= 64;

    if (s390x_x25519_mul(pubkey, generator, privkey) != 1)
        goto err;

    EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, key);
    return 1;
 err:
    ossl_ecx_key_free(key);
    return 0;
}

static int s390x_pkey_ecx_keygen448(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static const unsigned char generator[] = {
        0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
    };
    ECX_KEY *key = ossl_ecx_key_new(ctx->libctx, ECX_KEY_TYPE_X448, 1,
                                    ctx->propquery);
    unsigned char *privkey = NULL, *pubkey;

    if (key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (RAND_priv_bytes_ex(ctx->libctx, privkey, X448_KEYLEN, 0) <= 0)
        goto err;

    privkey[0] &= 252;
    privkey[55] |= 128;

    if (s390x_x448_mul(pubkey, generator, privkey) != 1)
        goto err;

    EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, key);
    return 1;
 err:
    ossl_ecx_key_free(key);
    return 0;
}

static int s390x_pkey_ecd_keygen25519(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static const unsigned char generator_x[] = {
        0x1a, 0xd5, 0x25, 0x8f, 0x60, 0x2d, 0x56, 0xc9, 0xb2, 0xa7, 0x25, 0x95,
        0x60, 0xc7, 0x2c, 0x69, 0x5c, 0xdc, 0xd6, 0xfd, 0x31, 0xe2, 0xa4, 0xc0,
        0xfe, 0x53, 0x6e, 0xcd, 0xd3, 0x36, 0x69, 0x21
    };
    static const unsigned char generator_y[] = {
        0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
        0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    };
    unsigned char x_dst[32], buff[SHA512_DIGEST_LENGTH];
    ECX_KEY *key = ossl_ecx_key_new(ctx->libctx, ECX_KEY_TYPE_ED25519, 1,
                                    ctx->propquery);
    unsigned char *privkey = NULL, *pubkey;
    unsigned int sz;
    EVP_MD *md = NULL;
    int rv;

    if (key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (RAND_priv_bytes_ex(ctx->libctx, privkey, ED25519_KEYLEN, 0) <= 0)
        goto err;

    md = EVP_MD_fetch(ctx->libctx, "SHA512", ctx->propquery);
    if (md == NULL)
        goto err;

    rv = EVP_Digest(privkey, 32, buff, &sz, md, NULL);
    EVP_MD_free(md);
    if (!rv)
        goto err;

    buff[0] &= 248;
    buff[31] &= 63;
    buff[31] |= 64;

    if (s390x_ed25519_mul(x_dst, pubkey,
                          generator_x, generator_y, buff) != 1)
        goto err;

    pubkey[31] |= ((x_dst[0] & 0x01) << 7);

    EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, key);
    return 1;
 err:
    ossl_ecx_key_free(key);
    return 0;
}

static int s390x_pkey_ecd_keygen448(EVP_PKEY_CTX *ctx, EVP_PKEY *pkey)
{
    static const unsigned char generator_x[] = {
        0x5e, 0xc0, 0x0c, 0xc7, 0x2b, 0xa8, 0x26, 0x26, 0x8e, 0x93, 0x00, 0x8b,
        0xe1, 0x80, 0x3b, 0x43, 0x11, 0x65, 0xb6, 0x2a, 0xf7, 0x1a, 0xae, 0x12,
        0x64, 0xa4, 0xd3, 0xa3, 0x24, 0xe3, 0x6d, 0xea, 0x67, 0x17, 0x0f, 0x47,
        0x70, 0x65, 0x14, 0x9e, 0xda, 0x36, 0xbf, 0x22, 0xa6, 0x15, 0x1d, 0x22,
        0xed, 0x0d, 0xed, 0x6b, 0xc6, 0x70, 0x19, 0x4f, 0x00
    };
    static const unsigned char generator_y[] = {
        0x14, 0xfa, 0x30, 0xf2, 0x5b, 0x79, 0x08, 0x98, 0xad, 0xc8, 0xd7, 0x4e,
        0x2c, 0x13, 0xbd, 0xfd, 0xc4, 0x39, 0x7c, 0xe6, 0x1c, 0xff, 0xd3, 0x3a,
        0xd7, 0xc2, 0xa0, 0x05, 0x1e, 0x9c, 0x78, 0x87, 0x40, 0x98, 0xa3, 0x6c,
        0x73, 0x73, 0xea, 0x4b, 0x62, 0xc7, 0xc9, 0x56, 0x37, 0x20, 0x76, 0x88,
        0x24, 0xbc, 0xb6, 0x6e, 0x71, 0x46, 0x3f, 0x69, 0x00
    };
    unsigned char x_dst[57], buff[114];
    ECX_KEY *key = ossl_ecx_key_new(ctx->libctx, ECX_KEY_TYPE_ED448, 1,
                                    ctx->propquery);
    unsigned char *privkey = NULL, *pubkey;
    EVP_MD_CTX *hashctx = NULL;
    EVP_MD *md = NULL;
    int rv;

    if (key == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    pubkey = key->pubkey;

    privkey = ossl_ecx_key_allocate_privkey(key);
    if (privkey == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (RAND_priv_bytes_ex(ctx->libctx, privkey, ED448_KEYLEN, 0) <= 0)
        goto err;

    hashctx = EVP_MD_CTX_new();
    if (hashctx == NULL)
        goto err;

    md = EVP_MD_fetch(ctx->libctx, "SHAKE256", ctx->propquery);
    if (md == NULL)
        goto err;

    rv = EVP_DigestInit_ex(hashctx, md, NULL);
    EVP_MD_free(md);
    if (rv != 1)
        goto err;

    if (EVP_DigestUpdate(hashctx, privkey, 57) != 1)
        goto err;
    if (EVP_DigestFinalXOF(hashctx, buff, sizeof(buff)) != 1)
        goto err;

    buff[0] &= -4;
    buff[55] |= 0x80;
    buff[56] = 0;

    if (s390x_ed448_mul(x_dst, pubkey,
                        generator_x, generator_y, buff) != 1)
        goto err;

    pubkey[56] |= ((x_dst[0] & 0x01) << 7);

    EVP_PKEY_assign(pkey, ctx->pmeth->pkey_id, key);
    EVP_MD_CTX_free(hashctx);
    return 1;
 err:
    ossl_ecx_key_free(key);
    EVP_MD_CTX_free(hashctx);
    return 0;
}

static int s390x_pkey_ecx_derive25519(EVP_PKEY_CTX *ctx, unsigned char *key,
                                      size_t *keylen)
{
    const unsigned char *privkey, *pubkey;

    if (!validate_ecx_derive(ctx, key, keylen, &privkey, &pubkey)
        || (key != NULL
            && s390x_x25519_mul(key, privkey, pubkey) == 0))
        return 0;
    *keylen = X25519_KEYLEN;
    return 1;
}

static int s390x_pkey_ecx_derive448(EVP_PKEY_CTX *ctx, unsigned char *key,
                                      size_t *keylen)
{
    const unsigned char *privkey, *pubkey;

    if (!validate_ecx_derive(ctx, key, keylen, &privkey, &pubkey)
        || (key != NULL
            && s390x_x448_mul(key, pubkey, privkey) == 0))
        return 0;
    *keylen = X448_KEYLEN;
    return 1;
}

static int s390x_pkey_ecd_digestsign25519(EVP_MD_CTX *ctx,
                                          unsigned char *sig, size_t *siglen,
                                          const unsigned char *tbs,
                                          size_t tbslen)
{
    union {
        struct {
            unsigned char sig[64];
            unsigned char priv[32];
        } ed25519;
        unsigned long long buff[512];
    } param;
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);
    int rc;

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (sig == NULL) {
        *siglen = ED25519_SIGSIZE;
        return 1;
    }

    if (*siglen < ED25519_SIGSIZE) {
        ERR_raise(ERR_LIB_EC, EC_R_BUFFER_TOO_SMALL);
        return 0;
    }

    memset(&param, 0, sizeof(param));
    memcpy(param.ed25519.priv, edkey->privkey, sizeof(param.ed25519.priv));

    rc = s390x_kdsa(S390X_EDDSA_SIGN_ED25519, &param.ed25519, tbs, tbslen);
    OPENSSL_cleanse(param.ed25519.priv, sizeof(param.ed25519.priv));
    if (rc != 0)
        return 0;

    s390x_flip_endian32(sig, param.ed25519.sig);
    s390x_flip_endian32(sig + 32, param.ed25519.sig + 32);

    *siglen = ED25519_SIGSIZE;
    return 1;
}

static int s390x_pkey_ecd_digestsign448(EVP_MD_CTX *ctx,
                                        unsigned char *sig, size_t *siglen,
                                        const unsigned char *tbs,
                                        size_t tbslen)
{
    union {
        struct {
            unsigned char sig[128];
            unsigned char priv[64];
        } ed448;
        unsigned long long buff[512];
    } param;
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);
    int rc;

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (sig == NULL) {
        *siglen = ED448_SIGSIZE;
        return 1;
    }

    if (*siglen < ED448_SIGSIZE) {
        ERR_raise(ERR_LIB_EC, EC_R_BUFFER_TOO_SMALL);
        return 0;
    }

    memset(&param, 0, sizeof(param));
    memcpy(param.ed448.priv + 64 - 57, edkey->privkey, 57);

    rc = s390x_kdsa(S390X_EDDSA_SIGN_ED448, &param.ed448, tbs, tbslen);
    OPENSSL_cleanse(param.ed448.priv, sizeof(param.ed448.priv));
    if (rc != 0)
        return 0;

    s390x_flip_endian64(param.ed448.sig, param.ed448.sig);
    s390x_flip_endian64(param.ed448.sig + 64, param.ed448.sig + 64);
    memcpy(sig, param.ed448.sig, 57);
    memcpy(sig + 57, param.ed448.sig + 64, 57);

    *siglen = ED448_SIGSIZE;
    return 1;
}

static int s390x_pkey_ecd_digestverify25519(EVP_MD_CTX *ctx,
                                            const unsigned char *sig,
                                            size_t siglen,
                                            const unsigned char *tbs,
                                            size_t tbslen)
{
    union {
        struct {
            unsigned char sig[64];
            unsigned char pub[32];
        } ed25519;
        unsigned long long buff[512];
    } param;
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (siglen != ED25519_SIGSIZE)
        return 0;

    memset(&param, 0, sizeof(param));
    s390x_flip_endian32(param.ed25519.sig, sig);
    s390x_flip_endian32(param.ed25519.sig + 32, sig + 32);
    s390x_flip_endian32(param.ed25519.pub, edkey->pubkey);

    return s390x_kdsa(S390X_EDDSA_VERIFY_ED25519,
                      &param.ed25519, tbs, tbslen) == 0 ? 1 : 0;
}

static int s390x_pkey_ecd_digestverify448(EVP_MD_CTX *ctx,
                                          const unsigned char *sig,
                                          size_t siglen,
                                          const unsigned char *tbs,
                                          size_t tbslen)
{
    union {
        struct {
            unsigned char sig[128];
            unsigned char pub[64];
        } ed448;
        unsigned long long buff[512];
    } param;
    const ECX_KEY *edkey = evp_pkey_get_legacy(EVP_MD_CTX_get_pkey_ctx(ctx)->pkey);

    if (edkey == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_KEY);
        return 0;
    }

    if (siglen != ED448_SIGSIZE)
        return 0;

    memset(&param, 0, sizeof(param));
    memcpy(param.ed448.sig, sig, 57);
    s390x_flip_endian64(param.ed448.sig, param.ed448.sig);
    memcpy(param.ed448.sig + 64, sig + 57, 57);
    s390x_flip_endian64(param.ed448.sig + 64, param.ed448.sig + 64);
    memcpy(param.ed448.pub, edkey->pubkey, 57);
    s390x_flip_endian64(param.ed448.pub, param.ed448.pub);

    return s390x_kdsa(S390X_EDDSA_VERIFY_ED448,
                      &param.ed448, tbs, tbslen) == 0 ? 1 : 0;
}

static const EVP_PKEY_METHOD ecx25519_s390x_pkey_meth = {
    EVP_PKEY_X25519,
    0, 0, 0, 0, 0, 0, 0,
    s390x_pkey_ecx_keygen25519,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    s390x_pkey_ecx_derive25519,
    pkey_ecx_ctrl,
    0
};

static const EVP_PKEY_METHOD ecx448_s390x_pkey_meth = {
    EVP_PKEY_X448,
    0, 0, 0, 0, 0, 0, 0,
    s390x_pkey_ecx_keygen448,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    s390x_pkey_ecx_derive448,
    pkey_ecx_ctrl,
    0
};
static const EVP_PKEY_METHOD ed25519_s390x_pkey_meth = {
    EVP_PKEY_ED25519, EVP_PKEY_FLAG_SIGCTX_CUSTOM,
    0, 0, 0, 0, 0, 0,
    s390x_pkey_ecd_keygen25519,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecd_ctrl,
    0,
    s390x_pkey_ecd_digestsign25519,
    s390x_pkey_ecd_digestverify25519
};

static const EVP_PKEY_METHOD ed448_s390x_pkey_meth = {
    EVP_PKEY_ED448, EVP_PKEY_FLAG_SIGCTX_CUSTOM,
    0, 0, 0, 0, 0, 0,
    s390x_pkey_ecd_keygen448,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    pkey_ecd_ctrl,
    0,
    s390x_pkey_ecd_digestsign448,
    s390x_pkey_ecd_digestverify448
};
#endif

const EVP_PKEY_METHOD *ossl_ecx25519_pkey_method(void)
{
#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X25519))
        return &ecx25519_s390x_pkey_meth;
#endif
    return &ecx25519_pkey_meth;
}

const EVP_PKEY_METHOD *ossl_ecx448_pkey_method(void)
{
#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X448))
        return &ecx448_s390x_pkey_meth;
#endif
    return &ecx448_pkey_meth;
}

const EVP_PKEY_METHOD *ossl_ed25519_pkey_method(void)
{
#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED25519)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_SIGN_ED25519)
        && OPENSSL_s390xcap_P.kdsa[0]
            & S390X_CAPBIT(S390X_EDDSA_VERIFY_ED25519))
        return &ed25519_s390x_pkey_meth;
#endif
    return &ed25519_pkey_meth;
}

const EVP_PKEY_METHOD *ossl_ed448_pkey_method(void)
{
#ifdef S390X_EC_ASM
    if (OPENSSL_s390xcap_P.pcc[1] & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_ED448)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_SIGN_ED448)
        && OPENSSL_s390xcap_P.kdsa[0] & S390X_CAPBIT(S390X_EDDSA_VERIFY_ED448))
        return &ed448_s390x_pkey_meth;
#endif
    return &ed448_pkey_meth;
}
