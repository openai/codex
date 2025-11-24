/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Low level APIs related to EC_KEY are deprecated for public use,
 * but still ok for internal use.
 */
#include "internal/deprecated.h"

#include <openssl/core_names.h>
#include <openssl/objects.h>
#include <openssl/params.h>
#include <openssl/err.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
# include <openssl/x509.h>
#endif
#include "crypto/bn.h"
#include "crypto/ec.h"
#include "ec_local.h"
#include "internal/e_os.h"
#include "internal/nelem.h"
#include "internal/param_build_set.h"

/* Mapping between a flag and a name */
static const OSSL_ITEM encoding_nameid_map[] = {
    { OPENSSL_EC_EXPLICIT_CURVE, OSSL_PKEY_EC_ENCODING_EXPLICIT },
    { OPENSSL_EC_NAMED_CURVE, OSSL_PKEY_EC_ENCODING_GROUP },
};

static const OSSL_ITEM check_group_type_nameid_map[] = {
    { 0, OSSL_PKEY_EC_GROUP_CHECK_DEFAULT },
    { EC_FLAG_CHECK_NAMED_GROUP, OSSL_PKEY_EC_GROUP_CHECK_NAMED },
    { EC_FLAG_CHECK_NAMED_GROUP_NIST, OSSL_PKEY_EC_GROUP_CHECK_NAMED_NIST },
};

static const OSSL_ITEM format_nameid_map[] = {
    { (int)POINT_CONVERSION_UNCOMPRESSED, OSSL_PKEY_EC_POINT_CONVERSION_FORMAT_UNCOMPRESSED },
    { (int)POINT_CONVERSION_COMPRESSED, OSSL_PKEY_EC_POINT_CONVERSION_FORMAT_COMPRESSED },
    { (int)POINT_CONVERSION_HYBRID, OSSL_PKEY_EC_POINT_CONVERSION_FORMAT_HYBRID },
};

int ossl_ec_encoding_name2id(const char *name)
{
    size_t i, sz;

    /* Return the default value if there is no name */
    if (name == NULL)
        return OPENSSL_EC_NAMED_CURVE;

    for (i = 0, sz = OSSL_NELEM(encoding_nameid_map); i < sz; i++) {
        if (OPENSSL_strcasecmp(name, encoding_nameid_map[i].ptr) == 0)
            return encoding_nameid_map[i].id;
    }
    return -1;
}

static char *ec_param_encoding_id2name(int id)
{
    size_t i, sz;

    for (i = 0, sz = OSSL_NELEM(encoding_nameid_map); i < sz; i++) {
        if (id == (int)encoding_nameid_map[i].id)
            return encoding_nameid_map[i].ptr;
    }
    return NULL;
}

char *ossl_ec_check_group_type_id2name(int id)
{
    size_t i, sz;

    for (i = 0, sz = OSSL_NELEM(check_group_type_nameid_map); i < sz; i++) {
        if (id == (int)check_group_type_nameid_map[i].id)
            return check_group_type_nameid_map[i].ptr;
    }
    return NULL;
}

static int ec_check_group_type_name2id(const char *name)
{
    size_t i, sz;

    /* Return the default value if there is no name */
    if (name == NULL)
        return 0;

    for (i = 0, sz = OSSL_NELEM(check_group_type_nameid_map); i < sz; i++) {
        if (OPENSSL_strcasecmp(name, check_group_type_nameid_map[i].ptr) == 0)
            return check_group_type_nameid_map[i].id;
    }
    return -1;
}

int ossl_ec_set_check_group_type_from_name(EC_KEY *ec, const char *name)
{
    int flags = ec_check_group_type_name2id(name);

    if (flags == -1)
        return 0;
    EC_KEY_clear_flags(ec, EC_FLAG_CHECK_NAMED_GROUP_MASK);
    EC_KEY_set_flags(ec, flags);
    return 1;
}

static int ec_set_check_group_type_from_param(EC_KEY *ec, const OSSL_PARAM *p)
{
    const char *name = NULL;
    int status = 0;

    switch (p->data_type) {
    case OSSL_PARAM_UTF8_STRING:
        name = p->data;
        status = (name != NULL);
        break;
    case OSSL_PARAM_UTF8_PTR:
        status = OSSL_PARAM_get_utf8_ptr(p, &name);
        break;
    }
    if (status)
        return ossl_ec_set_check_group_type_from_name(ec, name);
    return 0;
}

int ossl_ec_pt_format_name2id(const char *name)
{
    size_t i, sz;

    /* Return the default value if there is no name */
    if (name == NULL)
        return (int)POINT_CONVERSION_UNCOMPRESSED;

    for (i = 0, sz = OSSL_NELEM(format_nameid_map); i < sz; i++) {
        if (OPENSSL_strcasecmp(name, format_nameid_map[i].ptr) == 0)
            return format_nameid_map[i].id;
    }
    return -1;
}

char *ossl_ec_pt_format_id2name(int id)
{
    size_t i, sz;

    for (i = 0, sz = OSSL_NELEM(format_nameid_map); i < sz; i++) {
        if (id == (int)format_nameid_map[i].id)
            return format_nameid_map[i].ptr;
    }
    return NULL;
}

static int ec_group_explicit_todata(const EC_GROUP *group, OSSL_PARAM_BLD *tmpl,
                                    OSSL_PARAM params[], BN_CTX *bnctx,
                                    unsigned char **genbuf)
{
    int ret = 0, fid;
    const char *field_type;
    const OSSL_PARAM *param = NULL;
    const OSSL_PARAM *param_p = NULL;
    const OSSL_PARAM *param_a = NULL;
    const OSSL_PARAM *param_b = NULL;

    fid = EC_GROUP_get_field_type(group);

    if (fid == NID_X9_62_prime_field) {
        field_type = SN_X9_62_prime_field;
    } else if (fid == NID_X9_62_characteristic_two_field) {
#ifdef OPENSSL_NO_EC2M
        ERR_raise(ERR_LIB_EC, EC_R_GF2M_NOT_SUPPORTED);
        goto err;
#else
        field_type = SN_X9_62_characteristic_two_field;
#endif
    } else {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_FIELD);
        return 0;
    }

    param_p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_P);
    param_a = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_A);
    param_b = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_B);
    if (tmpl != NULL || param_p != NULL || param_a != NULL || param_b != NULL) {
        BIGNUM *p = BN_CTX_get(bnctx);
        BIGNUM *a = BN_CTX_get(bnctx);
        BIGNUM *b = BN_CTX_get(bnctx);

        if (b == NULL) {
            ERR_raise(ERR_LIB_EC, ERR_R_BN_LIB);
            goto err;
        }

        if (!EC_GROUP_get_curve(group, p, a, b, bnctx)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_CURVE);
            goto err;
        }
        if (!ossl_param_build_set_bn(tmpl, params, OSSL_PKEY_PARAM_EC_P, p)
            || !ossl_param_build_set_bn(tmpl, params, OSSL_PKEY_PARAM_EC_A, a)
            || !ossl_param_build_set_bn(tmpl, params, OSSL_PKEY_PARAM_EC_B, b)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    param = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_ORDER);
    if (tmpl != NULL || param != NULL) {
        const BIGNUM *order = EC_GROUP_get0_order(group);

        if (order == NULL) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_GROUP_ORDER);
            goto err;
        }
        if (!ossl_param_build_set_bn(tmpl, params, OSSL_PKEY_PARAM_EC_ORDER,
                                    order)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    param = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_FIELD_TYPE);
    if (tmpl != NULL || param != NULL) {
        if (!ossl_param_build_set_utf8_string(tmpl, params,
                                              OSSL_PKEY_PARAM_EC_FIELD_TYPE,
                                              field_type)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    param = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_GENERATOR);
    if (tmpl != NULL || param != NULL) {
        size_t genbuf_len;
        const EC_POINT *genpt = EC_GROUP_get0_generator(group);
        point_conversion_form_t genform = EC_GROUP_get_point_conversion_form(group);

        if (genpt == NULL) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_GENERATOR);
            goto err;
        }
        genbuf_len = EC_POINT_point2buf(group, genpt, genform, genbuf, bnctx);
        if (genbuf_len == 0) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_GENERATOR);
            goto err;
        }
        if (!ossl_param_build_set_octet_string(tmpl, params,
                                               OSSL_PKEY_PARAM_EC_GENERATOR,
                                               *genbuf, genbuf_len)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    param = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_COFACTOR);
    if (tmpl != NULL || param != NULL) {
        const BIGNUM *cofactor = EC_GROUP_get0_cofactor(group);

        if (cofactor != NULL
            && !ossl_param_build_set_bn(tmpl, params,
                                        OSSL_PKEY_PARAM_EC_COFACTOR, cofactor)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }

    param = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_SEED);
    if (tmpl != NULL || param != NULL) {
        unsigned char *seed = EC_GROUP_get0_seed(group);
        size_t seed_len = EC_GROUP_get_seed_len(group);

        if (seed != NULL
            && seed_len > 0
            && !ossl_param_build_set_octet_string(tmpl, params,
                                                  OSSL_PKEY_PARAM_EC_SEED,
                                                  seed, seed_len)) {
            ERR_raise(ERR_LIB_EC, ERR_R_CRYPTO_LIB);
            goto err;
        }
    }
    ret = 1;
err:
    return ret;
}

int ossl_ec_group_todata(const EC_GROUP *group, OSSL_PARAM_BLD *tmpl,
                         OSSL_PARAM params[], OSSL_LIB_CTX *libctx,
                         const char *propq,
                         BN_CTX *bnctx, unsigned char **genbuf)
{
    int ret = 0, curve_nid, encoding_flag;
    const char *encoding_name, *pt_form_name;
    point_conversion_form_t genform;

    if (group == NULL) {
        ERR_raise(ERR_LIB_EC, EC_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    genform = EC_GROUP_get_point_conversion_form(group);
    pt_form_name = ossl_ec_pt_format_id2name(genform);
    if (pt_form_name == NULL
        || !ossl_param_build_set_utf8_string(
                tmpl, params,
                OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT, pt_form_name)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_FORM);
        return 0;
    }
    encoding_flag = EC_GROUP_get_asn1_flag(group) & OPENSSL_EC_NAMED_CURVE;
    encoding_name = ec_param_encoding_id2name(encoding_flag);
    if (encoding_name == NULL
        || !ossl_param_build_set_utf8_string(tmpl, params,
                                             OSSL_PKEY_PARAM_EC_ENCODING,
                                             encoding_name)) {
        ERR_raise(ERR_LIB_EC, EC_R_INVALID_ENCODING);
        return 0;
    }

    if (!ossl_param_build_set_int(tmpl, params,
                                  OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS,
                                  group->decoded_from_explicit_params))
        return 0;

    curve_nid = EC_GROUP_get_curve_name(group);

    /*
     * Get the explicit parameters in these two cases:
     * - We do not have a template, i.e. specific parameters are requested
     * - The curve is not a named curve
     */
    if (tmpl == NULL || curve_nid == NID_undef)
        if (!ec_group_explicit_todata(group, tmpl, params, bnctx, genbuf))
            goto err;

    if (curve_nid != NID_undef) {
        /* Named curve */
        const char *curve_name = OSSL_EC_curve_nid2name(curve_nid);

        if (curve_name == NULL
            || !ossl_param_build_set_utf8_string(tmpl, params,
                                                 OSSL_PKEY_PARAM_GROUP_NAME,
                                                 curve_name)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_CURVE);
            goto err;
        }
    }
    ret = 1;
err:
    return ret;
}

/*
 * The intention with the "backend" source file is to offer backend support
 * for legacy backends (EVP_PKEY_ASN1_METHOD and EVP_PKEY_METHOD) and provider
 * implementations alike.
 */
int ossl_ec_set_ecdh_cofactor_mode(EC_KEY *ec, int mode)
{
    const EC_GROUP *ecg = EC_KEY_get0_group(ec);
    const BIGNUM *cofactor;
    /*
     * mode can be only 0 for disable, or 1 for enable here.
     *
     * This is in contrast with the same parameter on an ECDH EVP_PKEY_CTX that
     * also supports mode == -1 with the meaning of "reset to the default for
     * the associated key".
     */
    if (mode < 0 || mode > 1)
        return 0;

    if ((cofactor = EC_GROUP_get0_cofactor(ecg)) == NULL )
        return 0;

    /* ECDH cofactor mode has no effect if cofactor is 1 */
    if (BN_is_one(cofactor))
        return 1;

    if (mode == 1)
        EC_KEY_set_flags(ec, EC_FLAG_COFACTOR_ECDH);
    else if (mode == 0)
        EC_KEY_clear_flags(ec, EC_FLAG_COFACTOR_ECDH);

    return 1;
}

/*
 * Callers of ossl_ec_key_fromdata MUST make sure that ec_key_params_fromdata has
 * been called before!
 *
 * This function only gets the bare keypair, domain parameters and other
 * parameters are treated separately, and domain parameters are required to
 * define a keypair.
 */
int ossl_ec_key_fromdata(EC_KEY *ec, const OSSL_PARAM params[], int include_private)
{
    const OSSL_PARAM *param_priv_key = NULL, *param_pub_key = NULL;
    BN_CTX *ctx = NULL;
    BIGNUM *priv_key = NULL;
    unsigned char *pub_key = NULL;
    size_t pub_key_len;
    const EC_GROUP *ecg = NULL;
    EC_POINT *pub_point = NULL;
    int ok = 0;

    ecg = EC_KEY_get0_group(ec);
    if (ecg == NULL)
        return 0;

    param_pub_key =
        OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (include_private)
        param_priv_key =
            OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(ec));
    if (ctx == NULL)
        goto err;

    if (param_pub_key != NULL)
        if (!OSSL_PARAM_get_octet_string(param_pub_key,
                                         (void **)&pub_key, 0, &pub_key_len)
            || (pub_point = EC_POINT_new(ecg)) == NULL
            || !EC_POINT_oct2point(ecg, pub_point, pub_key, pub_key_len, ctx))
        goto err;

    if (param_priv_key != NULL && include_private) {
        int fixed_words;
        const BIGNUM *order;

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
        order = EC_GROUP_get0_order(ecg);
        if (order == NULL || BN_is_zero(order))
            goto err;

        fixed_words = bn_get_top(order) + 2;

        if ((priv_key = BN_secure_new()) == NULL)
            goto err;
        if (bn_wexpand(priv_key, fixed_words) == NULL)
            goto err;
        BN_set_flags(priv_key, BN_FLG_CONSTTIME);

        if (!OSSL_PARAM_get_BN(param_priv_key, &priv_key))
            goto err;
    }

    if (priv_key != NULL
        && !EC_KEY_set_private_key(ec, priv_key))
        goto err;

    if (pub_point != NULL
        && !EC_KEY_set_public_key(ec, pub_point))
        goto err;

    ok = 1;

 err:
    BN_CTX_free(ctx);
    BN_clear_free(priv_key);
    OPENSSL_free(pub_key);
    EC_POINT_free(pub_point);
    return ok;
}

int ossl_ec_group_fromdata(EC_KEY *ec, const OSSL_PARAM params[])
{
    int ok = 0;
    EC_GROUP *group = NULL;

    if (ec == NULL)
        return 0;

     group = EC_GROUP_new_from_params(params, ossl_ec_key_get_libctx(ec),
                                      ossl_ec_key_get0_propq(ec));

    if (!EC_KEY_set_group(ec, group))
        goto err;
    ok = 1;
err:
    EC_GROUP_free(group);
    return ok;
}

static int ec_key_point_format_fromdata(EC_KEY *ec, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;
    int format = -1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT);
    if (p != NULL) {
        if (!ossl_ec_pt_format_param2id(p, &format)) {
            ERR_raise(ERR_LIB_EC, EC_R_INVALID_FORM);
            return 0;
        }
        EC_KEY_set_conv_form(ec, format);
    }
    return 1;
}

static int ec_key_group_check_fromdata(EC_KEY *ec, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_GROUP_CHECK_TYPE);
    if (p != NULL)
        return ec_set_check_group_type_from_param(ec, p);
    return 1;
}

static int ec_set_include_public(EC_KEY *ec, int include)
{
    int flags = EC_KEY_get_enc_flags(ec);

    if (!include)
        flags |= EC_PKEY_NO_PUBKEY;
    else
        flags &= ~EC_PKEY_NO_PUBKEY;
    EC_KEY_set_enc_flags(ec, flags);
    return 1;
}

int ossl_ec_key_otherparams_fromdata(EC_KEY *ec, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;

    if (ec == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_USE_COFACTOR_ECDH);
    if (p != NULL) {
        int mode;

        if (!OSSL_PARAM_get_int(p, &mode)
            || !ossl_ec_set_ecdh_cofactor_mode(ec, mode))
            return 0;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC);
    if (p != NULL) {
        int include = 1;

        if (!OSSL_PARAM_get_int(p, &include)
            || !ec_set_include_public(ec, include))
            return 0;
    }
    if (!ec_key_point_format_fromdata(ec, params))
        return 0;
    if (!ec_key_group_check_fromdata(ec, params))
        return 0;
    return 1;
}

int ossl_ec_key_is_foreign(const EC_KEY *ec)
{
#ifndef FIPS_MODULE
    if (ec->engine != NULL || EC_KEY_get_method(ec) != EC_KEY_OpenSSL())
        return 1;
#endif
    return 0;

}

EC_KEY *ossl_ec_key_dup(const EC_KEY *src, int selection)
{
    EC_KEY *ret;

    if (src == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if ((ret = ossl_ec_key_new_method_int(src->libctx, src->propq,
                                          src->engine)) == NULL)
        return NULL;

    /* copy the parameters */
    if (src->group != NULL
        && (selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        ret->group = ossl_ec_group_new_ex(src->libctx, src->propq,
                                          src->group->meth);
        if (ret->group == NULL
            || !EC_GROUP_copy(ret->group, src->group))
            goto err;

        if (src->meth != NULL)
            ret->meth = src->meth;
    }

    /*  copy the public key */
    if (src->pub_key != NULL
        && (selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        if (ret->group == NULL)
            /* no parameter-less keys allowed */
            goto err;
        ret->pub_key = EC_POINT_new(ret->group);
        if (ret->pub_key == NULL
            || !EC_POINT_copy(ret->pub_key, src->pub_key))
                goto err;
    }

    /* copy the private key */
    if (src->priv_key != NULL
        && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if (ret->group == NULL)
            /* no parameter-less keys allowed */
            goto err;
        ret->priv_key = BN_new();
        if (ret->priv_key == NULL || !BN_copy(ret->priv_key, src->priv_key))
            goto err;
        if (ret->group->meth->keycopy
            && ret->group->meth->keycopy(ret, src) == 0)
            goto err;
    }

    /* copy the rest */
    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0) {
        ret->enc_flag = src->enc_flag;
        ret->conv_form = src->conv_form;
    }

    ret->version = src->version;
    ret->flags = src->flags;

#ifndef FIPS_MODULE
    if (!CRYPTO_dup_ex_data(CRYPTO_EX_INDEX_EC_KEY,
                            &ret->ex_data, &src->ex_data))
        goto err;
#endif

    if (ret->meth != NULL && ret->meth->copy != NULL) {
        if ((selection
             & OSSL_KEYMGMT_SELECT_KEYPAIR) != OSSL_KEYMGMT_SELECT_KEYPAIR)
            goto err;
        if (ret->meth->copy(ret, src) == 0)
            goto err;
    }

    return ret;
 err:
    EC_KEY_free(ret);
    return NULL;
}

int ossl_ec_encoding_param2id(const OSSL_PARAM *p, int *id)
{
    const char *name = NULL;
    int status = 0;

    switch (p->data_type) {
    case OSSL_PARAM_UTF8_STRING:
        /* The OSSL_PARAM functions have no support for this */
        name = p->data;
        status = (name != NULL);
        break;
    case OSSL_PARAM_UTF8_PTR:
        status = OSSL_PARAM_get_utf8_ptr(p, &name);
        break;
    }
    if (status) {
        int i = ossl_ec_encoding_name2id(name);

        if (i >= 0) {
            *id = i;
            return 1;
        }
    }
    return 0;
}

int ossl_ec_pt_format_param2id(const OSSL_PARAM *p, int *id)
{
    const char *name = NULL;
    int status = 0;

    switch (p->data_type) {
    case OSSL_PARAM_UTF8_STRING:
        /* The OSSL_PARAM functions have no support for this */
        name = p->data;
        status = (name != NULL);
        break;
    case OSSL_PARAM_UTF8_PTR:
        status = OSSL_PARAM_get_utf8_ptr(p, &name);
        break;
    }
    if (status) {
        int i = ossl_ec_pt_format_name2id(name);

        if (i >= 0) {
            *id = i;
            return 1;
        }
    }
    return 0;
}

#ifndef FIPS_MODULE
int ossl_x509_algor_is_sm2(const X509_ALGOR *palg)
{
    int ptype = 0;
    const void *pval = NULL;

    X509_ALGOR_get0(NULL, &ptype, &pval, palg);

    if (ptype == V_ASN1_OBJECT)
        return OBJ_obj2nid((ASN1_OBJECT *)pval) == NID_sm2;

    if (ptype == V_ASN1_SEQUENCE) {
        const ASN1_STRING *str = pval;
        const unsigned char *der = str->data;
        int derlen = str->length;
        EC_GROUP *group;
        int ret;

        if ((group = d2i_ECPKParameters(NULL, &der, derlen)) == NULL)
            ret = 0;
        else
            ret = (EC_GROUP_get_curve_name(group) == NID_sm2);

        EC_GROUP_free(group);
        return ret;
    }

    return 0;
}

EC_KEY *ossl_ec_key_param_from_x509_algor(const X509_ALGOR *palg,
                                     OSSL_LIB_CTX *libctx, const char *propq)
{
    int ptype = 0;
    const void *pval = NULL;
    EC_KEY *eckey = NULL;
    EC_GROUP *group = NULL;

    X509_ALGOR_get0(NULL, &ptype, &pval, palg);
    if ((eckey = EC_KEY_new_ex(libctx, propq)) == NULL) {
        ERR_raise(ERR_LIB_EC, ERR_R_EC_LIB);
        goto ecerr;
    }

    if (ptype == V_ASN1_SEQUENCE) {
        const ASN1_STRING *pstr = pval;
        const unsigned char *pm = pstr->data;
        int pmlen = pstr->length;


        if (d2i_ECParameters(&eckey, &pm, pmlen) == NULL) {
            ERR_raise(ERR_LIB_EC, EC_R_DECODE_ERROR);
            goto ecerr;
        }
    } else if (ptype == V_ASN1_OBJECT) {
        const ASN1_OBJECT *poid = pval;

        /*
         * type == V_ASN1_OBJECT => the parameters are given by an asn1 OID
         */

        group = EC_GROUP_new_by_curve_name_ex(libctx, propq, OBJ_obj2nid(poid));
        if (group == NULL)
            goto ecerr;
        EC_GROUP_set_asn1_flag(group, OPENSSL_EC_NAMED_CURVE);
        if (EC_KEY_set_group(eckey, group) == 0)
            goto ecerr;
        EC_GROUP_free(group);
    } else {
        ERR_raise(ERR_LIB_EC, EC_R_DECODE_ERROR);
        goto ecerr;
    }

    return eckey;

 ecerr:
    EC_KEY_free(eckey);
    EC_GROUP_free(group);
    return NULL;
}

EC_KEY *ossl_ec_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                               OSSL_LIB_CTX *libctx, const char *propq)
{
    const unsigned char *p = NULL;
    int pklen;
    EC_KEY *eckey = NULL;
    const X509_ALGOR *palg;

    if (!PKCS8_pkey_get0(NULL, &p, &pklen, &palg, p8inf))
        return 0;
    eckey = ossl_ec_key_param_from_x509_algor(palg, libctx, propq);
    if (eckey == NULL)
        goto err;

    /* We have parameters now set private key */
    if (!d2i_ECPrivateKey(&eckey, &p, pklen)) {
        ERR_raise(ERR_LIB_EC, EC_R_DECODE_ERROR);
        goto err;
    }

    return eckey;
 err:
    EC_KEY_free(eckey);
    return NULL;
}
#endif
