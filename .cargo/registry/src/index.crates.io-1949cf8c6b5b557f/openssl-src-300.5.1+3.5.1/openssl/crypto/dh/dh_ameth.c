/*
 * Copyright 2006-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DH low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <openssl/x509.h>
#include <openssl/asn1.h>
#include <openssl/bn.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include "internal/ffc.h"
#include "internal/cryptlib.h"
#include "crypto/asn1.h"
#include "crypto/dh.h"
#include "crypto/evp.h"
#include "dh_local.h"

/*
 * i2d/d2i like DH parameter functions which use the appropriate routine for
 * PKCS#3 DH or X9.42 DH.
 */

static DH *d2i_dhp(const EVP_PKEY *pkey, const unsigned char **pp,
                   long length)
{
    DH *dh = NULL;
    int is_dhx = (pkey->ameth == &ossl_dhx_asn1_meth);

    if (is_dhx)
        dh = d2i_DHxparams(NULL, pp, length);
    else
        dh = d2i_DHparams(NULL, pp, length);

    return dh;
}

static int i2d_dhp(const EVP_PKEY *pkey, const DH *a, unsigned char **pp)
{
    if (pkey->ameth == &ossl_dhx_asn1_meth)
        return i2d_DHxparams(a, pp);
    return i2d_DHparams(a, pp);
}

static void int_dh_free(EVP_PKEY *pkey)
{
    DH_free(pkey->pkey.dh);
}

static int dh_pub_decode(EVP_PKEY *pkey, const X509_PUBKEY *pubkey)
{
    const unsigned char *p, *pm;
    int pklen, pmlen;
    int ptype;
    const void *pval;
    const ASN1_STRING *pstr;
    X509_ALGOR *palg;
    ASN1_INTEGER *public_key = NULL;

    DH *dh = NULL;

    if (!X509_PUBKEY_get0_param(NULL, &p, &pklen, &palg, pubkey))
        return 0;
    X509_ALGOR_get0(NULL, &ptype, &pval, palg);

    if (ptype != V_ASN1_SEQUENCE) {
        ERR_raise(ERR_LIB_DH, DH_R_PARAMETER_ENCODING_ERROR);
        goto err;
    }

    pstr = pval;
    pm = pstr->data;
    pmlen = pstr->length;

    if ((dh = d2i_dhp(pkey, &pm, pmlen)) == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_DECODE_ERROR);
        goto err;
    }

    if ((public_key = d2i_ASN1_INTEGER(NULL, &p, pklen)) == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_DECODE_ERROR);
        goto err;
    }

    /* We have parameters now set public key */
    if ((dh->pub_key = ASN1_INTEGER_to_BN(public_key, NULL)) == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_BN_DECODE_ERROR);
        goto err;
    }

    ASN1_INTEGER_free(public_key);
    EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, dh);
    return 1;

 err:
    ASN1_INTEGER_free(public_key);
    DH_free(dh);
    return 0;
}

static int dh_pub_encode(X509_PUBKEY *pk, const EVP_PKEY *pkey)
{
    DH *dh;
    int ptype;
    unsigned char *penc = NULL;
    int penclen;
    ASN1_STRING *str;
    ASN1_INTEGER *pub_key = NULL;

    dh = pkey->pkey.dh;

    str = ASN1_STRING_new();
    if (str == NULL) {
        ERR_raise(ERR_LIB_DH, ERR_R_ASN1_LIB);
        goto err;
    }
    str->length = i2d_dhp(pkey, dh, &str->data);
    if (str->length <= 0) {
        ERR_raise(ERR_LIB_DH, ERR_R_ASN1_LIB);
        goto err;
    }
    ptype = V_ASN1_SEQUENCE;

    pub_key = BN_to_ASN1_INTEGER(dh->pub_key, NULL);
    if (pub_key == NULL)
        goto err;

    penclen = i2d_ASN1_INTEGER(pub_key, &penc);

    ASN1_INTEGER_free(pub_key);

    if (penclen <= 0) {
        ERR_raise(ERR_LIB_DH, ERR_R_ASN1_LIB);
        goto err;
    }

    if (X509_PUBKEY_set0_param(pk, OBJ_nid2obj(pkey->ameth->pkey_id),
                               ptype, str, penc, penclen))
        return 1;

 err:
    OPENSSL_free(penc);
    ASN1_STRING_free(str);

    return 0;
}

/*
 * PKCS#8 DH is defined in PKCS#11 of all places. It is similar to DH in that
 * the AlgorithmIdentifier contains the parameters, the private key is
 * explicitly included and the pubkey must be recalculated.
 */

static int dh_priv_decode(EVP_PKEY *pkey, const PKCS8_PRIV_KEY_INFO *p8)
{
    int ret = 0;
    DH *dh = ossl_dh_key_from_pkcs8(p8, NULL, NULL);

    if (dh != NULL) {
        ret = 1;
        EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, dh);
    }

    return ret;
}

static int dh_priv_encode(PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pkey)
{
    ASN1_STRING *params = NULL;
    ASN1_INTEGER *prkey = NULL;
    unsigned char *dp = NULL;
    int dplen;

    params = ASN1_STRING_new();

    if (params == NULL) {
        ERR_raise(ERR_LIB_DH, ERR_R_ASN1_LIB);
        goto err;
    }

    params->length = i2d_dhp(pkey, pkey->pkey.dh, &params->data);
    if (params->length <= 0) {
        ERR_raise(ERR_LIB_DH, ERR_R_ASN1_LIB);
        goto err;
    }
    params->type = V_ASN1_SEQUENCE;

    /* Get private key into integer */
    prkey = BN_to_ASN1_INTEGER(pkey->pkey.dh->priv_key, NULL);

    if (prkey == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_BN_ERROR);
        goto err;
    }

    dplen = i2d_ASN1_INTEGER(prkey, &dp);

    ASN1_STRING_clear_free(prkey);

    if (dplen <= 0) {
        ERR_raise(ERR_LIB_DH, DH_R_BN_ERROR);
        goto err;
    }

    if (!PKCS8_pkey_set0(p8, OBJ_nid2obj(pkey->ameth->pkey_id), 0,
                         V_ASN1_SEQUENCE, params, dp, dplen)) {
        OPENSSL_clear_free(dp, dplen);
        goto err;
    }
    return 1;

 err:
    ASN1_STRING_free(params);
    return 0;
}

static int dh_param_decode(EVP_PKEY *pkey,
                           const unsigned char **pder, int derlen)
{
    DH *dh;

    if ((dh = d2i_dhp(pkey, pder, derlen)) == NULL)
        return 0;
    dh->dirty_cnt++;
    EVP_PKEY_assign(pkey, pkey->ameth->pkey_id, dh);
    return 1;
}

static int dh_param_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_dhp(pkey, pkey->pkey.dh, pder);
}

static int do_dh_print(BIO *bp, const DH *x, int indent, int ptype)
{
    int reason = ERR_R_BUF_LIB;
    const char *ktype = NULL;
    BIGNUM *priv_key, *pub_key;

    if (ptype == 2)
        priv_key = x->priv_key;
    else
        priv_key = NULL;

    if (ptype > 0)
        pub_key = x->pub_key;
    else
        pub_key = NULL;

    if (x->params.p == NULL || (ptype == 2 && priv_key == NULL)
            || (ptype > 0 && pub_key == NULL)) {
        reason = ERR_R_PASSED_NULL_PARAMETER;
        goto err;
    }

    if (ptype == 2)
        ktype = "DH Private-Key";
    else if (ptype == 1)
        ktype = "DH Public-Key";
    else
        ktype = "DH Parameters";

    if (!BIO_indent(bp, indent, 128)
            || BIO_printf(bp, "%s: (%d bit)\n", ktype, DH_bits(x)) <= 0)
        goto err;
    indent += 4;

    if (!ASN1_bn_print(bp, "private-key:", priv_key, NULL, indent))
        goto err;
    if (!ASN1_bn_print(bp, "public-key:", pub_key, NULL, indent))
        goto err;

    if (!ossl_ffc_params_print(bp, &x->params, indent))
        goto err;

    if (x->length != 0) {
        if (!BIO_indent(bp, indent, 128)
                || BIO_printf(bp, "recommended-private-length: %d bits\n",
                              (int)x->length) <= 0)
            goto err;
    }

    return 1;

 err:
    ERR_raise(ERR_LIB_DH, reason);
    return 0;
}

static int int_dh_size(const EVP_PKEY *pkey)
{
    return DH_size(pkey->pkey.dh);
}

static int dh_bits(const EVP_PKEY *pkey)
{
    return DH_bits(pkey->pkey.dh);
}

static int dh_security_bits(const EVP_PKEY *pkey)
{
    return DH_security_bits(pkey->pkey.dh);
}

static int dh_cmp_parameters(const EVP_PKEY *a, const EVP_PKEY *b)
{
    return ossl_ffc_params_cmp(&a->pkey.dh->params, &b->pkey.dh->params,
                               a->ameth != &ossl_dhx_asn1_meth);
}

static int int_dh_param_copy(DH *to, const DH *from, int is_x942)
{
    if (is_x942 == -1)
        is_x942 = (from->params.q != NULL);
    if (!ossl_ffc_params_copy(&to->params, &from->params))
        return 0;
    if (!is_x942)
        to->length = from->length;
    to->dirty_cnt++;
    return 1;
}

DH *DHparams_dup(const DH *dh)
{
    DH *ret;
    ret = DH_new();
    if (ret == NULL)
        return NULL;
    if (!int_dh_param_copy(ret, dh, -1)) {
        DH_free(ret);
        return NULL;
    }
    return ret;
}

static int dh_copy_parameters(EVP_PKEY *to, const EVP_PKEY *from)
{
    if (to->pkey.dh == NULL) {
        to->pkey.dh = DH_new();
        if (to->pkey.dh == NULL)
            return 0;
    }
    return int_dh_param_copy(to->pkey.dh, from->pkey.dh,
                             from->ameth == &ossl_dhx_asn1_meth);
}

static int dh_missing_parameters(const EVP_PKEY *a)
{
    return a->pkey.dh == NULL
        || a->pkey.dh->params.p == NULL
        || a->pkey.dh->params.g == NULL;
}

static int dh_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b)
{
    if (dh_cmp_parameters(a, b) == 0)
        return 0;
    if (BN_cmp(b->pkey.dh->pub_key, a->pkey.dh->pub_key) != 0)
        return 0;
    else
        return 1;
}

static int dh_param_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                          ASN1_PCTX *ctx)
{
    return do_dh_print(bp, pkey->pkey.dh, indent, 0);
}

static int dh_public_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                           ASN1_PCTX *ctx)
{
    return do_dh_print(bp, pkey->pkey.dh, indent, 1);
}

static int dh_private_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                            ASN1_PCTX *ctx)
{
    return do_dh_print(bp, pkey->pkey.dh, indent, 2);
}

int DHparams_print(BIO *bp, const DH *x)
{
    return do_dh_print(bp, x, 4, 0);
}

static int dh_pkey_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    DH *dh;
    switch (op) {
    case ASN1_PKEY_CTRL_SET1_TLS_ENCPT:
        /* We should only be here if we have a legacy key */
        if (!ossl_assert(evp_pkey_is_legacy(pkey)))
            return 0;
        dh = (DH *) evp_pkey_get0_DH_int(pkey);
        if (dh == NULL)
            return 0;
        return ossl_dh_buf2key(dh, arg2, arg1);
    case ASN1_PKEY_CTRL_GET1_TLS_ENCPT:
        dh = (DH *) EVP_PKEY_get0_DH(pkey);
        if (dh == NULL)
            return 0;
        return ossl_dh_key2buf(dh, arg2, 0, 1);
    default:
        return -2;
    }
}

static int dhx_pkey_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    switch (op) {
    default:
        return -2;
    }

}

static int dh_pkey_public_check(const EVP_PKEY *pkey)
{
    DH *dh = pkey->pkey.dh;

    if (dh->pub_key == NULL) {
        ERR_raise(ERR_LIB_DH, DH_R_MISSING_PUBKEY);
        return 0;
    }

    return DH_check_pub_key_ex(dh, dh->pub_key);
}

static int dh_pkey_param_check(const EVP_PKEY *pkey)
{
    DH *dh = pkey->pkey.dh;

    return DH_check_ex(dh);
}

static size_t dh_pkey_dirty_cnt(const EVP_PKEY *pkey)
{
    return pkey->pkey.dh->dirty_cnt;
}

static int dh_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                             OSSL_FUNC_keymgmt_import_fn *importer,
                             OSSL_LIB_CTX *libctx, const char *propq)
{
    DH *dh = from->pkey.dh;
    OSSL_PARAM_BLD *tmpl;
    const BIGNUM *p = DH_get0_p(dh), *g = DH_get0_g(dh), *q = DH_get0_q(dh);
    long l = DH_get_length(dh);
    const BIGNUM *pub_key = DH_get0_pub_key(dh);
    const BIGNUM *priv_key = DH_get0_priv_key(dh);
    OSSL_PARAM *params = NULL;
    int selection = 0;
    int rv = 0;

    if (p == NULL || g == NULL)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;
    if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_P, p)
        || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_G, g))
        goto err;
    if (q != NULL) {
        if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_Q, q))
            goto err;
    }
    selection |= OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS;
    if (l > 0) {
        if (!OSSL_PARAM_BLD_push_long(tmpl, OSSL_PKEY_PARAM_DH_PRIV_LEN, l))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS;
    }
    if (pub_key != NULL) {
        if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_PUB_KEY, pub_key))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    }
    if (priv_key != NULL) {
        if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_PRIV_KEY,
                                    priv_key))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_PRIVATE_KEY;
    }

    if ((params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL)
        goto err;

    /* We export, the provider imports */
    rv = importer(to_keydata, selection, params);

    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return rv;
}

static int dh_pkey_import_from_type(const OSSL_PARAM params[], void *vpctx,
                                    int type)
{
    EVP_PKEY_CTX *pctx = vpctx;
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    DH *dh = ossl_dh_new_ex(pctx->libctx);

    if (dh == NULL) {
        ERR_raise(ERR_LIB_DH, ERR_R_DH_LIB);
        return 0;
    }
    DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
    DH_set_flags(dh, type == EVP_PKEY_DH ? DH_FLAG_TYPE_DH : DH_FLAG_TYPE_DHX);

    if (!ossl_dh_params_fromdata(dh, params)
        || !ossl_dh_key_fromdata(dh, params, 1)
        || !EVP_PKEY_assign(pkey, type, dh)) {
        DH_free(dh);
        return 0;
    }
    return 1;
}

static int dh_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return dh_pkey_import_from_type(params, vpctx, EVP_PKEY_DH);
}

static int dhx_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    return dh_pkey_import_from_type(params, vpctx, EVP_PKEY_DHX);
}

static int dh_pkey_copy(EVP_PKEY *to, EVP_PKEY *from)
{
    DH *dh = from->pkey.dh;
    DH *dupkey = NULL;
    int ret;

    if (dh != NULL) {
        dupkey = ossl_dh_dup(dh, OSSL_KEYMGMT_SELECT_ALL);
        if (dupkey == NULL)
            return 0;
    }

    ret = EVP_PKEY_assign(to, from->type, dupkey);
    if (!ret)
        DH_free(dupkey);
    return ret;
}

const EVP_PKEY_ASN1_METHOD ossl_dh_asn1_meth = {
    EVP_PKEY_DH,
    EVP_PKEY_DH,
    0,

    "DH",
    "OpenSSL PKCS#3 DH method",

    dh_pub_decode,
    dh_pub_encode,
    dh_pub_cmp,
    dh_public_print,

    dh_priv_decode,
    dh_priv_encode,
    dh_private_print,

    int_dh_size,
    dh_bits,
    dh_security_bits,

    dh_param_decode,
    dh_param_encode,
    dh_missing_parameters,
    dh_copy_parameters,
    dh_cmp_parameters,
    dh_param_print,
    0,

    int_dh_free,
    dh_pkey_ctrl,

    0, 0, 0, 0, 0,

    0,
    dh_pkey_public_check,
    dh_pkey_param_check,

    0, 0, 0, 0,

    dh_pkey_dirty_cnt,
    dh_pkey_export_to,
    dh_pkey_import_from,
    dh_pkey_copy
};

const EVP_PKEY_ASN1_METHOD ossl_dhx_asn1_meth = {
    EVP_PKEY_DHX,
    EVP_PKEY_DHX,
    0,

    "X9.42 DH",
    "OpenSSL X9.42 DH method",

    dh_pub_decode,
    dh_pub_encode,
    dh_pub_cmp,
    dh_public_print,

    dh_priv_decode,
    dh_priv_encode,
    dh_private_print,

    int_dh_size,
    dh_bits,
    dh_security_bits,

    dh_param_decode,
    dh_param_encode,
    dh_missing_parameters,
    dh_copy_parameters,
    dh_cmp_parameters,
    dh_param_print,
    0,

    int_dh_free,
    dhx_pkey_ctrl,

    0, 0, 0, 0, 0,

    0,
    dh_pkey_public_check,
    dh_pkey_param_check,
    0, 0, 0, 0,
    dh_pkey_dirty_cnt,
    dh_pkey_export_to,
    dhx_pkey_import_from,
    dh_pkey_copy
};
