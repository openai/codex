/*
 * Copyright 2006-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <openssl/x509.h>
#include <openssl/asn1.h>
#include <openssl/bn.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include "internal/cryptlib.h"
#include "crypto/asn1.h"
#include "crypto/dsa.h"
#include "crypto/evp.h"
#include "internal/ffc.h"
#include "dsa_local.h"

static int dsa_pub_decode(EVP_PKEY *pkey, const X509_PUBKEY *pubkey)
{
    const unsigned char *p, *pm;
    int pklen, pmlen;
    int ptype;
    const void *pval;
    const ASN1_STRING *pstr;
    X509_ALGOR *palg;
    ASN1_INTEGER *public_key = NULL;

    DSA *dsa = NULL;

    if (!X509_PUBKEY_get0_param(NULL, &p, &pklen, &palg, pubkey))
        return 0;
    X509_ALGOR_get0(NULL, &ptype, &pval, palg);

    if (ptype == V_ASN1_SEQUENCE) {
        pstr = pval;
        pm = pstr->data;
        pmlen = pstr->length;

        if ((dsa = d2i_DSAparams(NULL, &pm, pmlen)) == NULL) {
            ERR_raise(ERR_LIB_DSA, DSA_R_DECODE_ERROR);
            goto err;
        }

    } else if ((ptype == V_ASN1_NULL) || (ptype == V_ASN1_UNDEF)) {
        if ((dsa = DSA_new()) == NULL) {
            ERR_raise(ERR_LIB_DSA, ERR_R_DSA_LIB);
            goto err;
        }
    } else {
        ERR_raise(ERR_LIB_DSA, DSA_R_PARAMETER_ENCODING_ERROR);
        goto err;
    }

    if ((public_key = d2i_ASN1_INTEGER(NULL, &p, pklen)) == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_DECODE_ERROR);
        goto err;
    }

    if ((dsa->pub_key = ASN1_INTEGER_to_BN(public_key, NULL)) == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BN_DECODE_ERROR);
        goto err;
    }

    dsa->dirty_cnt++;
    ASN1_INTEGER_free(public_key);
    EVP_PKEY_assign_DSA(pkey, dsa);
    return 1;

 err:
    ASN1_INTEGER_free(public_key);
    DSA_free(dsa);
    return 0;

}

static int dsa_pub_encode(X509_PUBKEY *pk, const EVP_PKEY *pkey)
{
    DSA *dsa;
    int ptype;
    unsigned char *penc = NULL;
    int penclen;
    ASN1_STRING *str = NULL;
    ASN1_INTEGER *pubint = NULL;
    ASN1_OBJECT *aobj;

    dsa = pkey->pkey.dsa;
    if (pkey->save_parameters
        && dsa->params.p != NULL
        && dsa->params.q != NULL
        && dsa->params.g != NULL) {
        str = ASN1_STRING_new();
        if (str == NULL) {
            ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
            goto err;
        }
        str->length = i2d_DSAparams(dsa, &str->data);
        if (str->length <= 0) {
            ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
            goto err;
        }
        ptype = V_ASN1_SEQUENCE;
    } else
        ptype = V_ASN1_UNDEF;

    pubint = BN_to_ASN1_INTEGER(dsa->pub_key, NULL);

    if (pubint == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
        goto err;
    }

    penclen = i2d_ASN1_INTEGER(pubint, &penc);
    ASN1_INTEGER_free(pubint);

    if (penclen <= 0) {
        ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
        goto err;
    }

    aobj = OBJ_nid2obj(EVP_PKEY_DSA);
    if (aobj == NULL)
        goto err;

    if (X509_PUBKEY_set0_param(pk, aobj, ptype, str, penc, penclen))
        return 1;

 err:
    OPENSSL_free(penc);
    ASN1_STRING_free(str);

    return 0;
}

/*
 * In PKCS#8 DSA: you just get a private key integer and parameters in the
 * AlgorithmIdentifier the pubkey must be recalculated.
 */

static int dsa_priv_decode(EVP_PKEY *pkey, const PKCS8_PRIV_KEY_INFO *p8)
{
    int ret = 0;
    DSA *dsa = ossl_dsa_key_from_pkcs8(p8, NULL, NULL);

    if (dsa != NULL) {
        ret = 1;
        EVP_PKEY_assign_DSA(pkey, dsa);
    }

    return ret;
}

static int dsa_priv_encode(PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pkey)
{
    ASN1_STRING *params = NULL;
    ASN1_INTEGER *prkey = NULL;
    unsigned char *dp = NULL;
    int dplen;

    if (pkey->pkey.dsa  == NULL|| pkey->pkey.dsa->priv_key == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_MISSING_PARAMETERS);
        goto err;
    }

    params = ASN1_STRING_new();

    if (params == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
        goto err;
    }

    params->length = i2d_DSAparams(pkey->pkey.dsa, &params->data);
    if (params->length <= 0) {
        ERR_raise(ERR_LIB_DSA, ERR_R_ASN1_LIB);
        goto err;
    }
    params->type = V_ASN1_SEQUENCE;

    /* Get private key into integer */
    prkey = BN_to_ASN1_INTEGER(pkey->pkey.dsa->priv_key, NULL);

    if (prkey == NULL) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BN_ERROR);
        goto err;
    }

    dplen = i2d_ASN1_INTEGER(prkey, &dp);

    ASN1_STRING_clear_free(prkey);

    if (dplen <= 0) {
        ERR_raise(ERR_LIB_DSA, DSA_R_BN_ERROR);
        goto err;
    }

    if (!PKCS8_pkey_set0(p8, OBJ_nid2obj(NID_dsa), 0,
                         V_ASN1_SEQUENCE, params, dp, dplen)) {
        OPENSSL_clear_free(dp, dplen);
        goto err;
    }
    return 1;

 err:
    ASN1_STRING_free(params);
    return 0;
}

static int int_dsa_size(const EVP_PKEY *pkey)
{
    return DSA_size(pkey->pkey.dsa);
}

static int dsa_bits(const EVP_PKEY *pkey)
{
    return DSA_bits(pkey->pkey.dsa);
}

static int dsa_security_bits(const EVP_PKEY *pkey)
{
    return DSA_security_bits(pkey->pkey.dsa);
}

static int dsa_missing_parameters(const EVP_PKEY *pkey)
{
    DSA *dsa;
    dsa = pkey->pkey.dsa;
    return dsa == NULL
        || dsa->params.p == NULL
        || dsa->params.q == NULL
        || dsa->params.g == NULL;
}

static int dsa_copy_parameters(EVP_PKEY *to, const EVP_PKEY *from)
{
    if (to->pkey.dsa == NULL) {
        to->pkey.dsa = DSA_new();
        if (to->pkey.dsa == NULL)
            return 0;
    }
    if (!ossl_ffc_params_copy(&to->pkey.dsa->params, &from->pkey.dsa->params))
        return 0;

    to->pkey.dsa->dirty_cnt++;
    return 1;
}

static int dsa_cmp_parameters(const EVP_PKEY *a, const EVP_PKEY *b)
{
    return ossl_ffc_params_cmp(&a->pkey.dsa->params, &b->pkey.dsa->params, 1);
}

static int dsa_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b)
{
    return BN_cmp(b->pkey.dsa->pub_key, a->pkey.dsa->pub_key) == 0;
}

static void int_dsa_free(EVP_PKEY *pkey)
{
    DSA_free(pkey->pkey.dsa);
}

static int do_dsa_print(BIO *bp, const DSA *x, int off, int ptype)
{
    int ret = 0;
    const char *ktype = NULL;
    const BIGNUM *priv_key, *pub_key;
    int mod_len = 0;

    if (x->params.p != NULL)
        mod_len = DSA_bits(x);

    if (ptype == 2)
        priv_key = x->priv_key;
    else
        priv_key = NULL;

    if (ptype > 0)
        pub_key = x->pub_key;
    else
        pub_key = NULL;

    if (ptype == 2)
        ktype = "Private-Key";
    else if (ptype == 1)
        ktype = "Public-Key";
    else
        ktype = "DSA-Parameters";

    if (priv_key != NULL) {
        if (!BIO_indent(bp, off, 128))
            goto err;
        if (BIO_printf(bp, "%s: (%d bit)\n", ktype, mod_len) <= 0)
            goto err;
    } else {
        if (BIO_printf(bp, "Public-Key: (%d bit)\n", mod_len) <= 0)
            goto err;
    }

    if (!ASN1_bn_print(bp, "priv:", priv_key, NULL, off))
        goto err;
    if (!ASN1_bn_print(bp, "pub: ", pub_key, NULL, off))
        goto err;
    if (!ossl_ffc_params_print(bp, &x->params, off))
        goto err;
    ret = 1;
 err:
    return ret;
}

static int dsa_param_decode(EVP_PKEY *pkey,
                            const unsigned char **pder, int derlen)
{
    DSA *dsa;

    if ((dsa = d2i_DSAparams(NULL, pder, derlen)) == NULL)
        return 0;

    dsa->dirty_cnt++;
    EVP_PKEY_assign_DSA(pkey, dsa);
    return 1;
}

static int dsa_param_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_DSAparams(pkey->pkey.dsa, pder);
}

static int dsa_param_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                           ASN1_PCTX *ctx)
{
    return do_dsa_print(bp, pkey->pkey.dsa, indent, 0);
}

static int dsa_pub_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                         ASN1_PCTX *ctx)
{
    return do_dsa_print(bp, pkey->pkey.dsa, indent, 1);
}

static int dsa_priv_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                          ASN1_PCTX *ctx)
{
    return do_dsa_print(bp, pkey->pkey.dsa, indent, 2);
}

static int old_dsa_priv_decode(EVP_PKEY *pkey,
                               const unsigned char **pder, int derlen)
{
    DSA *dsa;

    if ((dsa = d2i_DSAPrivateKey(NULL, pder, derlen)) == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_DSA_LIB);
        return 0;
    }
    dsa->dirty_cnt++;
    EVP_PKEY_assign_DSA(pkey, dsa);
    return 1;
}

static int old_dsa_priv_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_DSAPrivateKey(pkey->pkey.dsa, pder);
}

static int dsa_sig_print(BIO *bp, const X509_ALGOR *sigalg,
                         const ASN1_STRING *sig, int indent, ASN1_PCTX *pctx)
{
    DSA_SIG *dsa_sig;
    const unsigned char *p;

    if (sig == NULL) {
        if (BIO_puts(bp, "\n") <= 0)
            return 0;
        else
            return 1;
    }
    p = sig->data;
    dsa_sig = d2i_DSA_SIG(NULL, &p, sig->length);
    if (dsa_sig != NULL) {
        int rv = 0;
        const BIGNUM *r, *s;

        DSA_SIG_get0(dsa_sig, &r, &s);

        if (BIO_write(bp, "\n", 1) != 1)
            goto err;

        if (!ASN1_bn_print(bp, "r:   ", r, NULL, indent))
            goto err;
        if (!ASN1_bn_print(bp, "s:   ", s, NULL, indent))
            goto err;
        rv = 1;
 err:
        DSA_SIG_free(dsa_sig);
        return rv;
    }
    if (BIO_puts(bp, "\n") <= 0)
        return 0;
    return X509_signature_dump(bp, sig, indent);
}

static int dsa_pkey_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    switch (op) {
    case ASN1_PKEY_CTRL_DEFAULT_MD_NID:
        *(int *)arg2 = NID_sha256;
        return 1;

    default:
        return -2;
    }
}

static size_t dsa_pkey_dirty_cnt(const EVP_PKEY *pkey)
{
    return pkey->pkey.dsa->dirty_cnt;
}

static int dsa_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                              OSSL_FUNC_keymgmt_import_fn *importer,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    DSA *dsa = from->pkey.dsa;
    OSSL_PARAM_BLD *tmpl;
    const BIGNUM *p = DSA_get0_p(dsa), *g = DSA_get0_g(dsa);
    const BIGNUM *q = DSA_get0_q(dsa), *pub_key = DSA_get0_pub_key(dsa);
    const BIGNUM *priv_key = DSA_get0_priv_key(dsa);
    OSSL_PARAM *params;
    int selection = 0;
    int rv = 0;

    if (p == NULL || q == NULL || g == NULL)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_P, p)
        || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_Q, q)
        || !OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_FFC_G, g))
        goto err;
    selection |= OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS;
    if (pub_key != NULL) {
        if (!OSSL_PARAM_BLD_push_BN(tmpl, OSSL_PKEY_PARAM_PUB_KEY,
                                    pub_key))
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

static int dsa_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    EVP_PKEY_CTX *pctx = vpctx;
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    DSA *dsa = ossl_dsa_new(pctx->libctx);

    if (dsa == NULL) {
        ERR_raise(ERR_LIB_DSA, ERR_R_DSA_LIB);
        return 0;
    }

    if (!ossl_dsa_ffc_params_fromdata(dsa, params)
        || !ossl_dsa_key_fromdata(dsa, params, 1)
        || !EVP_PKEY_assign_DSA(pkey, dsa)) {
        DSA_free(dsa);
        return 0;
    }
    return 1;
}

static int dsa_pkey_copy(EVP_PKEY *to, EVP_PKEY *from)
{
    DSA *dsa = from->pkey.dsa;
    DSA *dupkey = NULL;
    int ret;

    if (dsa != NULL) {
        dupkey = ossl_dsa_dup(dsa, OSSL_KEYMGMT_SELECT_ALL);
        if (dupkey == NULL)
            return 0;
    }

    ret = EVP_PKEY_assign_DSA(to, dupkey);
    if (!ret)
        DSA_free(dupkey);
    return ret;
}

/* NB these are sorted in pkey_id order, lowest first */

const EVP_PKEY_ASN1_METHOD ossl_dsa_asn1_meths[4] = {

    /* This aliases NID_dsa with NID_dsa_2 */
    {
     EVP_PKEY_DSA1,
     EVP_PKEY_DSA,
     ASN1_PKEY_ALIAS},

    /* This aliases NID_dsaWithSHA with NID_dsaWithSHA_2 */
    {
     EVP_PKEY_DSA4,
     EVP_PKEY_DSA2,
     ASN1_PKEY_ALIAS},

    /* This aliases NID_dsaWithSHA with NID_dsaWithSHA1 */
    {
     EVP_PKEY_DSA3,
     EVP_PKEY_DSA2,
     ASN1_PKEY_ALIAS},

    {
     EVP_PKEY_DSA,
     EVP_PKEY_DSA,
     0,

     "DSA",
     "OpenSSL DSA method",

     dsa_pub_decode,
     dsa_pub_encode,
     dsa_pub_cmp,
     dsa_pub_print,

     dsa_priv_decode,
     dsa_priv_encode,
     dsa_priv_print,

     int_dsa_size,
     dsa_bits,
     dsa_security_bits,

     dsa_param_decode,
     dsa_param_encode,
     dsa_missing_parameters,
     dsa_copy_parameters,
     dsa_cmp_parameters,
     dsa_param_print,
     dsa_sig_print,

     int_dsa_free,
     dsa_pkey_ctrl,
     old_dsa_priv_decode,
     old_dsa_priv_encode,

     NULL, NULL, NULL,
     NULL, NULL, NULL,
     NULL, NULL, NULL, NULL,

     dsa_pkey_dirty_cnt,
     dsa_pkey_export_to,
     dsa_pkey_import_from,
     dsa_pkey_copy
    }
};
