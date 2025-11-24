/*
 * Copyright 2006-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDH and ECDSA low level APIs are deprecated for public use, but still ok
 * for internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/x509.h>
#include <openssl/ec.h>
#include <openssl/bn.h>
#include <openssl/asn1t.h>
#include "crypto/asn1.h"
#include "crypto/evp.h"
#include "crypto/x509.h"
#include <openssl/core_names.h>
#include <openssl/param_build.h>
#include "ec_local.h"

static int eckey_param2type(int *pptype, void **ppval, const EC_KEY *ec_key)
{
    const EC_GROUP *group;
    int nid;

    if (ec_key == NULL || (group = EC_KEY_get0_group(ec_key)) == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PARAMETERS);
        return 0;
    }
    if (EC_GROUP_get_asn1_flag(group)
        && (nid = EC_GROUP_get_curve_name(group)))
        /* we have a 'named curve' => just set the OID */
    {
        ASN1_OBJECT *asn1obj = OBJ_nid2obj(nid);

        if (asn1obj == NULL || OBJ_length(asn1obj) == 0) {
            ERR_raise(ERR_LIB_EC, EC_R_MISSING_OID);
            return 0;
        }
        *ppval = asn1obj;
        *pptype = V_ASN1_OBJECT;
    } else {                    /* explicit parameters */

        ASN1_STRING *pstr = NULL;
        pstr = ASN1_STRING_new();
        if (pstr == NULL)
            return 0;
        pstr->length = i2d_ECParameters(ec_key, &pstr->data);
        if (pstr->length <= 0) {
            ASN1_STRING_free(pstr);
            ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
            return 0;
        }
        *ppval = pstr;
        *pptype = V_ASN1_SEQUENCE;
    }
    return 1;
}

static int eckey_pub_encode(X509_PUBKEY *pk, const EVP_PKEY *pkey)
{
    const EC_KEY *ec_key = pkey->pkey.ec;
    void *pval = NULL;
    int ptype;
    unsigned char *penc = NULL, *p;
    int penclen;

    if (!eckey_param2type(&ptype, &pval, ec_key)) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        return 0;
    }
    penclen = i2o_ECPublicKey(ec_key, NULL);
    if (penclen <= 0)
        goto err;
    penc = OPENSSL_malloc(penclen);
    if (penc == NULL)
        goto err;
    p = penc;
    penclen = i2o_ECPublicKey(ec_key, &p);
    if (penclen <= 0)
        goto err;
    if (X509_PUBKEY_set0_param(pk, OBJ_nid2obj(EVP_PKEY_EC),
                               ptype, pval, penc, penclen))
        return 1;
 err:
    if (ptype == V_ASN1_SEQUENCE)
        ASN1_STRING_free(pval);
    OPENSSL_free(penc);
    return 0;
}

static int eckey_pub_decode(EVP_PKEY *pkey, const X509_PUBKEY *pubkey)
{
    const unsigned char *p = NULL;
    int pklen;
    EC_KEY *eckey = NULL;
    X509_ALGOR *palg;
    OSSL_LIB_CTX *libctx = NULL;
    const char *propq = NULL;

    if (!ossl_x509_PUBKEY_get0_libctx(&libctx, &propq, pubkey)
        || !X509_PUBKEY_get0_param(NULL, &p, &pklen, &palg, pubkey))
        return 0;
    eckey = ossl_ec_key_param_from_x509_algor(palg, libctx, propq);

    if (!eckey)
        return 0;

    /* We have parameters now set public key */
    if (!o2i_ECPublicKey(&eckey, &p, pklen)) {
        ERR_raise(ERR_LIB_EC, EC_R_DECODE_ERROR);
        goto ecerr;
    }

    EVP_PKEY_assign_EC_KEY(pkey, eckey);
    return 1;

 ecerr:
    EC_KEY_free(eckey);
    return 0;
}

static int eckey_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b)
{
    int r;
    const EC_GROUP *group = EC_KEY_get0_group(b->pkey.ec);
    const EC_POINT *pa = EC_KEY_get0_public_key(a->pkey.ec),
        *pb = EC_KEY_get0_public_key(b->pkey.ec);

    if (group == NULL || pa == NULL || pb == NULL)
        return -2;
    r = EC_POINT_cmp(group, pa, pb, NULL);
    if (r == 0)
        return 1;
    if (r == 1)
        return 0;
    return -2;
}

static int eckey_priv_decode_ex(EVP_PKEY *pkey, const PKCS8_PRIV_KEY_INFO *p8,
                                OSSL_LIB_CTX *libctx, const char *propq)
{
    int ret = 0;
    EC_KEY *eckey = ossl_ec_key_from_pkcs8(p8, libctx, propq);

    if (eckey != NULL) {
        ret = 1;
        EVP_PKEY_assign_EC_KEY(pkey, eckey);
    }

    return ret;
}

static int eckey_priv_encode(PKCS8_PRIV_KEY_INFO *p8, const EVP_PKEY *pkey)
{
    EC_KEY ec_key = *(pkey->pkey.ec);
    unsigned char *ep = NULL;
    int eplen, ptype;
    void *pval;
    unsigned int old_flags;

    if (!eckey_param2type(&ptype, &pval, &ec_key)) {
        ERR_raise(ERR_LIB_EC, EC_R_DECODE_ERROR);
        return 0;
    }

    /* set the private key */

    /*
     * do not include the parameters in the SEC1 private key see PKCS#11
     * 12.11
     */
    old_flags = EC_KEY_get_enc_flags(&ec_key);
    EC_KEY_set_enc_flags(&ec_key, old_flags | EC_PKEY_NO_PARAMETERS);

    eplen = i2d_ECPrivateKey(&ec_key, &ep);
    if (eplen <= 0) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto err;
    }

    if (!PKCS8_pkey_set0(p8, OBJ_nid2obj(NID_X9_62_id_ecPublicKey), 0,
                         ptype, pval, ep, eplen)) {
        ERR_raise(ERR_LIB_EC, ERR_R_ASN1_LIB);
        OPENSSL_clear_free(ep, eplen);
        goto err;
    }

    return 1;

 err:
    if (ptype == V_ASN1_SEQUENCE)
        ASN1_STRING_free(pval);
    return 0;
}

static int int_ec_size(const EVP_PKEY *pkey)
{
    return ECDSA_size(pkey->pkey.ec);
}

static int ec_bits(const EVP_PKEY *pkey)
{
    return EC_GROUP_order_bits(EC_KEY_get0_group(pkey->pkey.ec));
}

static int ec_security_bits(const EVP_PKEY *pkey)
{
    int ecbits = ec_bits(pkey);

    if (ecbits >= 512)
        return 256;
    if (ecbits >= 384)
        return 192;
    if (ecbits >= 256)
        return 128;
    if (ecbits >= 224)
        return 112;
    if (ecbits >= 160)
        return 80;
    return ecbits / 2;
}

static int ec_missing_parameters(const EVP_PKEY *pkey)
{
    if (pkey->pkey.ec == NULL || EC_KEY_get0_group(pkey->pkey.ec) == NULL)
        return 1;
    return 0;
}

static int ec_copy_parameters(EVP_PKEY *to, const EVP_PKEY *from)
{
    EC_GROUP *group = EC_GROUP_dup(EC_KEY_get0_group(from->pkey.ec));

    if (group == NULL)
        return 0;
    if (to->pkey.ec == NULL) {
        to->pkey.ec = EC_KEY_new();
        if (to->pkey.ec == NULL)
            goto err;
    }
    if (EC_KEY_set_group(to->pkey.ec, group) == 0)
        goto err;
    EC_GROUP_free(group);
    return 1;
 err:
    EC_GROUP_free(group);
    return 0;
}

static int ec_cmp_parameters(const EVP_PKEY *a, const EVP_PKEY *b)
{
    const EC_GROUP *group_a = EC_KEY_get0_group(a->pkey.ec),
        *group_b = EC_KEY_get0_group(b->pkey.ec);

    if (group_a == NULL || group_b == NULL)
        return -2;
    if (EC_GROUP_cmp(group_a, group_b, NULL))
        return 0;
    else
        return 1;
}

static void int_ec_free(EVP_PKEY *pkey)
{
    EC_KEY_free(pkey->pkey.ec);
}

typedef enum {
    EC_KEY_PRINT_PRIVATE,
    EC_KEY_PRINT_PUBLIC,
    EC_KEY_PRINT_PARAM
} ec_print_t;

static int do_EC_KEY_print(BIO *bp, const EC_KEY *x, int off, ec_print_t ktype)
{
    const char *ecstr;
    unsigned char *priv = NULL, *pub = NULL;
    size_t privlen = 0, publen = 0;
    int ret = 0;
    const EC_GROUP *group;

    if (x == NULL || (group = EC_KEY_get0_group(x)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    if (ktype != EC_KEY_PRINT_PARAM && EC_KEY_get0_public_key(x) != NULL) {
        publen = EC_KEY_key2buf(x, EC_KEY_get_conv_form(x), &pub, NULL);
        if (publen == 0)
            goto err;
    }

    if (ktype == EC_KEY_PRINT_PRIVATE && EC_KEY_get0_private_key(x) != NULL) {
        privlen = EC_KEY_priv2buf(x, &priv);
        if (privlen == 0)
            goto err;
    }

    if (ktype == EC_KEY_PRINT_PRIVATE)
        ecstr = "Private-Key";
    else if (ktype == EC_KEY_PRINT_PUBLIC)
        ecstr = "Public-Key";
    else
        ecstr = "ECDSA-Parameters";

    if (!BIO_indent(bp, off, 128))
        goto err;
    if (BIO_printf(bp, "%s: (%d bit)\n", ecstr,
                   EC_GROUP_order_bits(group)) <= 0)
        goto err;

    if (privlen != 0) {
        if (BIO_printf(bp, "%*spriv:\n", off, "") <= 0)
            goto err;
        if (ASN1_buf_print(bp, priv, privlen, off + 4) == 0)
            goto err;
    }

    if (publen != 0) {
        if (BIO_printf(bp, "%*spub:\n", off, "") <= 0)
            goto err;
        if (ASN1_buf_print(bp, pub, publen, off + 4) == 0)
            goto err;
    }

    if (!ECPKParameters_print(bp, group, off))
        goto err;
    ret = 1;
 err:
    if (!ret)
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
    OPENSSL_clear_free(priv, privlen);
    OPENSSL_free(pub);
    return ret;
}

static int eckey_param_decode(EVP_PKEY *pkey,
                              const unsigned char **pder, int derlen)
{
    EC_KEY *eckey;

    if ((eckey = d2i_ECParameters(NULL, pder, derlen)) == NULL)
        return 0;
    EVP_PKEY_assign_EC_KEY(pkey, eckey);
    return 1;
}

static int eckey_param_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_ECParameters(pkey->pkey.ec, pder);
}

static int eckey_param_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                             ASN1_PCTX *ctx)
{
    return do_EC_KEY_print(bp, pkey->pkey.ec, indent, EC_KEY_PRINT_PARAM);
}

static int eckey_pub_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                           ASN1_PCTX *ctx)
{
    return do_EC_KEY_print(bp, pkey->pkey.ec, indent, EC_KEY_PRINT_PUBLIC);
}

static int eckey_priv_print(BIO *bp, const EVP_PKEY *pkey, int indent,
                            ASN1_PCTX *ctx)
{
    return do_EC_KEY_print(bp, pkey->pkey.ec, indent, EC_KEY_PRINT_PRIVATE);
}

static int old_ec_priv_decode(EVP_PKEY *pkey,
                              const unsigned char **pder, int derlen)
{
    EC_KEY *ec;

    if ((ec = d2i_ECPrivateKey(NULL, pder, derlen)) == NULL)
        return 0;
    EVP_PKEY_assign_EC_KEY(pkey, ec);
    return 1;
}

static int old_ec_priv_encode(const EVP_PKEY *pkey, unsigned char **pder)
{
    return i2d_ECPrivateKey(pkey->pkey.ec, pder);
}

static int ec_pkey_ctrl(EVP_PKEY *pkey, int op, long arg1, void *arg2)
{
    switch (op) {
    case ASN1_PKEY_CTRL_DEFAULT_MD_NID:
        if (EVP_PKEY_get_id(pkey) == EVP_PKEY_SM2) {
            /* For SM2, the only valid digest-alg is SM3 */
            *(int *)arg2 = NID_sm3;
            return 2;            /* Make it mandatory */
        }
        *(int *)arg2 = NID_sha256;
        return 1;

    case ASN1_PKEY_CTRL_SET1_TLS_ENCPT:
        /* We should only be here if we have a legacy key */
        if (!ossl_assert(evp_pkey_is_legacy(pkey)))
            return 0;
        return EC_KEY_oct2key(evp_pkey_get0_EC_KEY_int(pkey), arg2, arg1, NULL);

    case ASN1_PKEY_CTRL_GET1_TLS_ENCPT:
        return EC_KEY_key2buf(EVP_PKEY_get0_EC_KEY(pkey),
                              POINT_CONVERSION_UNCOMPRESSED, arg2, NULL);

    default:
        return -2;
    }
}

static int ec_pkey_check(const EVP_PKEY *pkey)
{
    EC_KEY *eckey = pkey->pkey.ec;

    /* stay consistent to what EVP_PKEY_check demands */
    if (eckey->priv_key == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PRIVATE_KEY);
        return 0;
    }

    return EC_KEY_check_key(eckey);
}

static int ec_pkey_public_check(const EVP_PKEY *pkey)
{
    EC_KEY *eckey = pkey->pkey.ec;

    /*
     * Note: it unnecessary to check eckey->pub_key here since
     * it will be checked in EC_KEY_check_key(). In fact, the
     * EC_KEY_check_key() mainly checks the public key, and checks
     * the private key optionally (only if there is one). So if
     * someone passes a whole EC key (public + private), this
     * will also work...
     */

    return EC_KEY_check_key(eckey);
}

static int ec_pkey_param_check(const EVP_PKEY *pkey)
{
    EC_KEY *eckey = pkey->pkey.ec;

    /* stay consistent to what EVP_PKEY_check demands */
    if (eckey->group == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_MISSING_PARAMETERS);
        return 0;
    }

    return EC_GROUP_check(eckey->group, NULL);
}

static
size_t ec_pkey_dirty_cnt(const EVP_PKEY *pkey)
{
    return pkey->pkey.ec->dirty_cnt;
}

static
int ec_pkey_export_to(const EVP_PKEY *from, void *to_keydata,
                      OSSL_FUNC_keymgmt_import_fn *importer,
                      OSSL_LIB_CTX *libctx, const char *propq)
{
    const EC_KEY *eckey = NULL;
    const EC_GROUP *ecg = NULL;
    unsigned char *pub_key_buf = NULL, *gen_buf = NULL;
    size_t pub_key_buflen;
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    const BIGNUM *priv_key = NULL;
    const EC_POINT *pub_point = NULL;
    int selection = 0;
    int rv = 0;
    BN_CTX *bnctx = NULL;

    if (from == NULL
            || (eckey = from->pkey.ec) == NULL
            || (ecg = EC_KEY_get0_group(eckey)) == NULL)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    /*
     * EC_POINT_point2buf() can generate random numbers in some
     * implementations so we need to ensure we use the correct libctx.
     */
    bnctx = BN_CTX_new_ex(libctx);
    if (bnctx == NULL)
        goto err;
    BN_CTX_start(bnctx);

    /* export the domain parameters */
    if (!ossl_ec_group_todata(ecg, tmpl, NULL, libctx, propq, bnctx, &gen_buf))
        goto err;
    selection |= OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS;

    priv_key = EC_KEY_get0_private_key(eckey);
    pub_point = EC_KEY_get0_public_key(eckey);

    if (pub_point != NULL) {
        /* convert pub_point to a octet string according to the SECG standard */
        point_conversion_form_t format = EC_KEY_get_conv_form(eckey);

        if ((pub_key_buflen = EC_POINT_point2buf(ecg, pub_point,
                                                 format,
                                                 &pub_key_buf, bnctx)) == 0
            || !OSSL_PARAM_BLD_push_octet_string(tmpl,
                                                 OSSL_PKEY_PARAM_PUB_KEY,
                                                 pub_key_buf,
                                                 pub_key_buflen))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    }

    if (priv_key != NULL) {
        size_t sz;
        int ecbits;
        int ecdh_cofactor_mode;

        /*
         * Key import/export should never leak the bit length of the secret
         * scalar in the key.
         *
         * For this reason, on export we use padded BIGNUMs with fixed length.
         *
         * When importing we also should make sure that, even if short lived,
         * the newly created BIGNUM is marked with the BN_FLG_CONSTTIME flag as
         * soon as possible, so that any processing of this BIGNUM might opt for
         * constant time implementations in the backend.
         *
         * Setting the BN_FLG_CONSTTIME flag alone is never enough, we also have
         * to preallocate the BIGNUM internal buffer to a fixed public size big
         * enough that operations performed during the processing never trigger
         * a realloc which would leak the size of the scalar through memory
         * accesses.
         *
         * Fixed Length
         * ------------
         *
         * The order of the large prime subgroup of the curve is our choice for
         * a fixed public size, as that is generally the upper bound for
         * generating a private key in EC cryptosystems and should fit all valid
         * secret scalars.
         *
         * For padding on export we just use the bit length of the order
         * converted to bytes (rounding up).
         *
         * For preallocating the BIGNUM storage we look at the number of "words"
         * required for the internal representation of the order, and we
         * preallocate 2 extra "words" in case any of the subsequent processing
         * might temporarily overflow the order length.
         */
        ecbits = EC_GROUP_order_bits(ecg);
        if (ecbits <= 0)
            goto err;

        sz = (ecbits + 7) / 8;
        if (!OSSL_PARAM_BLD_push_BN_pad(tmpl,
                                        OSSL_PKEY_PARAM_PRIV_KEY,
                                        priv_key, sz))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_PRIVATE_KEY;

        /*
         * The ECDH Cofactor Mode is defined only if the EC_KEY actually
         * contains a private key, so we check for the flag and export it only
         * in this case.
         */
        ecdh_cofactor_mode =
            (EC_KEY_get_flags(eckey) & EC_FLAG_COFACTOR_ECDH) ? 1 : 0;

        /* Export the ECDH_COFACTOR_MODE parameter */
        if (!OSSL_PARAM_BLD_push_int(tmpl,
                                     OSSL_PKEY_PARAM_USE_COFACTOR_ECDH,
                                     ecdh_cofactor_mode))
            goto err;
        selection |= OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS;
    }

    params = OSSL_PARAM_BLD_to_param(tmpl);

    /* We export, the provider imports */
    rv = importer(to_keydata, selection, params);

 err:
    OSSL_PARAM_BLD_free(tmpl);
    OSSL_PARAM_free(params);
    OPENSSL_free(pub_key_buf);
    OPENSSL_free(gen_buf);
    BN_CTX_end(bnctx);
    BN_CTX_free(bnctx);
    return rv;
}

static int ec_pkey_import_from(const OSSL_PARAM params[], void *vpctx)
{
    EVP_PKEY_CTX *pctx = vpctx;
    EVP_PKEY *pkey = EVP_PKEY_CTX_get0_pkey(pctx);
    EC_KEY *ec = EC_KEY_new_ex(pctx->libctx, pctx->propquery);

    if (ec == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        return 0;
    }

    if (!ossl_ec_group_fromdata(ec, params)
        || !ossl_ec_key_otherparams_fromdata(ec, params)
        || !ossl_ec_key_fromdata(ec, params, 1)
        || !EVP_PKEY_assign_EC_KEY(pkey, ec)) {
        EC_KEY_free(ec);
        return 0;
    }
    return 1;
}

static int ec_pkey_copy(EVP_PKEY *to, EVP_PKEY *from)
{
    EC_KEY *eckey = from->pkey.ec;
    EC_KEY *dupkey = NULL;
    int ret;

    if (eckey != NULL) {
        dupkey = EC_KEY_dup(eckey);
        if (dupkey == NULL)
            return 0;
    } else {
        /* necessary to properly copy empty SM2 keys */
        return EVP_PKEY_set_type(to, from->type);
    }

    ret = EVP_PKEY_assign_EC_KEY(to, dupkey);
    if (!ret)
        EC_KEY_free(dupkey);
    return ret;
}

const EVP_PKEY_ASN1_METHOD ossl_eckey_asn1_meth = {
    EVP_PKEY_EC,
    EVP_PKEY_EC,
    0,
    "EC",
    "OpenSSL EC algorithm",

    eckey_pub_decode,
    eckey_pub_encode,
    eckey_pub_cmp,
    eckey_pub_print,

    NULL,
    eckey_priv_encode,
    eckey_priv_print,

    int_ec_size,
    ec_bits,
    ec_security_bits,

    eckey_param_decode,
    eckey_param_encode,
    ec_missing_parameters,
    ec_copy_parameters,
    ec_cmp_parameters,
    eckey_param_print,
    0,

    int_ec_free,
    ec_pkey_ctrl,
    old_ec_priv_decode,
    old_ec_priv_encode,

    0, 0, 0,

    ec_pkey_check,
    ec_pkey_public_check,
    ec_pkey_param_check,

    0, /* set_priv_key */
    0, /* set_pub_key */
    0, /* get_priv_key */
    0, /* get_pub_key */

    ec_pkey_dirty_cnt,
    ec_pkey_export_to,
    ec_pkey_import_from,
    ec_pkey_copy,
    eckey_priv_decode_ex
};

#if !defined(OPENSSL_NO_SM2)
const EVP_PKEY_ASN1_METHOD ossl_sm2_asn1_meth = {
   EVP_PKEY_SM2,
   EVP_PKEY_EC,
   ASN1_PKEY_ALIAS
};
#endif

int EC_KEY_print(BIO *bp, const EC_KEY *x, int off)
{
    int private = EC_KEY_get0_private_key(x) != NULL;

    return do_EC_KEY_print(bp, x, off,
                private ? EC_KEY_PRINT_PRIVATE : EC_KEY_PRINT_PUBLIC);
}

int ECParameters_print(BIO *bp, const EC_KEY *x)
{
    return do_EC_KEY_print(bp, x, 4, EC_KEY_PRINT_PARAM);
}
