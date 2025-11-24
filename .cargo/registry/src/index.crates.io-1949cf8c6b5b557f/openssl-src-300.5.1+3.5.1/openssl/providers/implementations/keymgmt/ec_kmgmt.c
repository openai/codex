/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * ECDH/ECDSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include <openssl/objects.h>
#include <openssl/proverr.h>
#include "crypto/bn.h"
#include "crypto/ec.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"
#include "internal/param_build_set.h"

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
#  include "crypto/sm2.h"
# endif
#endif

static OSSL_FUNC_keymgmt_new_fn ec_newdata;
static OSSL_FUNC_keymgmt_gen_init_fn ec_gen_init;
static OSSL_FUNC_keymgmt_gen_set_template_fn ec_gen_set_template;
static OSSL_FUNC_keymgmt_gen_set_params_fn ec_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn ec_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_get_params_fn ec_gen_get_params;
static OSSL_FUNC_keymgmt_gen_gettable_params_fn ec_gen_gettable_params;
static OSSL_FUNC_keymgmt_gen_fn ec_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn ec_gen_cleanup;
static OSSL_FUNC_keymgmt_load_fn ec_load;
static OSSL_FUNC_keymgmt_free_fn ec_freedata;
static OSSL_FUNC_keymgmt_get_params_fn ec_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn ec_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn ec_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn ec_settable_params;
static OSSL_FUNC_keymgmt_has_fn ec_has;
static OSSL_FUNC_keymgmt_match_fn ec_match;
static OSSL_FUNC_keymgmt_validate_fn ec_validate;
static OSSL_FUNC_keymgmt_import_fn ec_import;
static OSSL_FUNC_keymgmt_import_types_fn ec_import_types;
static OSSL_FUNC_keymgmt_export_fn ec_export;
static OSSL_FUNC_keymgmt_export_types_fn ec_export_types;
static OSSL_FUNC_keymgmt_query_operation_name_fn ec_query_operation_name;
static OSSL_FUNC_keymgmt_dup_fn ec_dup;
#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static OSSL_FUNC_keymgmt_new_fn sm2_newdata;
static OSSL_FUNC_keymgmt_gen_init_fn sm2_gen_init;
static OSSL_FUNC_keymgmt_gen_fn sm2_gen;
static OSSL_FUNC_keymgmt_get_params_fn sm2_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn sm2_gettable_params;
static OSSL_FUNC_keymgmt_settable_params_fn sm2_settable_params;
static OSSL_FUNC_keymgmt_import_fn sm2_import;
static OSSL_FUNC_keymgmt_query_operation_name_fn sm2_query_operation_name;
static OSSL_FUNC_keymgmt_validate_fn sm2_validate;
# endif
#endif

#define EC_DEFAULT_MD "SHA256"
#define EC_POSSIBLE_SELECTIONS                                                 \
    (OSSL_KEYMGMT_SELECT_KEYPAIR | OSSL_KEYMGMT_SELECT_ALL_PARAMETERS)
#define SM2_DEFAULT_MD "SM3"

static
const char *ec_query_operation_name(int operation_id)
{
    switch (operation_id) {
    case OSSL_OP_KEYEXCH:
        return "ECDH";
    case OSSL_OP_SIGNATURE:
        return "ECDSA";
    }
    return NULL;
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static
const char *sm2_query_operation_name(int operation_id)
{
    switch (operation_id) {
    case OSSL_OP_SIGNATURE:
        return "SM2";
    }
    return NULL;
}
# endif
#endif

/*
 * Callers of key_to_params MUST make sure that domparams_to_params is also
 * called!
 *
 * This function only exports the bare keypair, domain parameters and other
 * parameters are exported separately.
 */
static ossl_inline
int key_to_params(const EC_KEY *eckey, OSSL_PARAM_BLD *tmpl,
                  OSSL_PARAM params[], int include_private,
                  unsigned char **pub_key)
{
    BIGNUM *x = NULL, *y = NULL;
    const BIGNUM *priv_key = NULL;
    const EC_POINT *pub_point = NULL;
    const EC_GROUP *ecg = NULL;
    size_t pub_key_len = 0;
    int ret = 0;
    BN_CTX *bnctx = NULL;

    if (eckey == NULL
        || (ecg = EC_KEY_get0_group(eckey)) == NULL)
        return 0;

    priv_key = EC_KEY_get0_private_key(eckey);
    pub_point = EC_KEY_get0_public_key(eckey);

    if (pub_point != NULL) {
        OSSL_PARAM *p = NULL, *px = NULL, *py = NULL;
        /*
         * EC_POINT_point2buf() can generate random numbers in some
         * implementations so we need to ensure we use the correct libctx.
         */
        bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(eckey));
        if (bnctx == NULL)
            goto err;


        /* If we are doing a get then check first before decoding the point */
        if (tmpl == NULL) {
            p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PUB_KEY);
            px = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_EC_PUB_X);
            py = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_EC_PUB_Y);
        }

        if (p != NULL || tmpl != NULL) {
            /* convert pub_point to a octet string according to the SECG standard */
            point_conversion_form_t format = EC_KEY_get_conv_form(eckey);

            if ((pub_key_len = EC_POINT_point2buf(ecg, pub_point,
                                                  format,
                                                  pub_key, bnctx)) == 0
                || !ossl_param_build_set_octet_string(tmpl, p,
                                                      OSSL_PKEY_PARAM_PUB_KEY,
                                                      *pub_key, pub_key_len))
                goto err;
        }
        if (px != NULL || py != NULL) {
            if (px != NULL) {
                x = BN_CTX_get(bnctx);
                if (x == NULL)
                    goto err;
            }
            if (py != NULL) {
                y = BN_CTX_get(bnctx);
                if (y == NULL)
                    goto err;
            }

            if (!EC_POINT_get_affine_coordinates(ecg, pub_point, x, y, bnctx))
                goto err;
            if (px != NULL
                && !ossl_param_build_set_bn(tmpl, px,
                                            OSSL_PKEY_PARAM_EC_PUB_X, x))
                goto err;
            if (py != NULL
                && !ossl_param_build_set_bn(tmpl, py,
                                            OSSL_PKEY_PARAM_EC_PUB_Y, y))
                goto err;
        }
    }

    if (priv_key != NULL && include_private) {
        size_t sz;
        int ecbits;

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

        if (!ossl_param_build_set_bn_pad(tmpl, params,
                                         OSSL_PKEY_PARAM_PRIV_KEY,
                                         priv_key, sz))
            goto err;
    }
    ret = 1;
 err:
    BN_CTX_free(bnctx);
    return ret;
}

static ossl_inline
int otherparams_to_params(const EC_KEY *ec, OSSL_PARAM_BLD *tmpl,
                          OSSL_PARAM params[])
{
    int ecdh_cofactor_mode = 0, group_check = 0;
    const char *name = NULL;
    point_conversion_form_t format;

    if (ec == NULL)
        return 0;

    format = EC_KEY_get_conv_form(ec);
    name = ossl_ec_pt_format_id2name((int)format);
    if (name != NULL
        && !ossl_param_build_set_utf8_string(tmpl, params,
                                             OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                                             name))
        return 0;

    group_check = EC_KEY_get_flags(ec) & EC_FLAG_CHECK_NAMED_GROUP_MASK;
    name = ossl_ec_check_group_type_id2name(group_check);
    if (name != NULL
        && !ossl_param_build_set_utf8_string(tmpl, params,
                                             OSSL_PKEY_PARAM_EC_GROUP_CHECK_TYPE,
                                             name))
        return 0;

    if ((EC_KEY_get_enc_flags(ec) & EC_PKEY_NO_PUBKEY) != 0
            && !ossl_param_build_set_int(tmpl, params,
                                         OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC, 0))
        return 0;

    ecdh_cofactor_mode =
        (EC_KEY_get_flags(ec) & EC_FLAG_COFACTOR_ECDH) ? 1 : 0;
    return ossl_param_build_set_int(tmpl, params,
                                    OSSL_PKEY_PARAM_USE_COFACTOR_ECDH,
                                    ecdh_cofactor_mode);
}

static
void *ec_newdata(void *provctx)
{
    if (!ossl_prov_is_running())
        return NULL;
    return EC_KEY_new_ex(PROV_LIBCTX_OF(provctx), NULL);
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static
void *sm2_newdata(void *provctx)
{
    if (!ossl_prov_is_running())
        return NULL;
    return EC_KEY_new_by_curve_name_ex(PROV_LIBCTX_OF(provctx), NULL, NID_sm2);
}
# endif
#endif

static
void ec_freedata(void *keydata)
{
    EC_KEY_free(keydata);
}

static
int ec_has(const void *keydata, int selection)
{
    const EC_KEY *ec = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || ec == NULL)
        return 0;
    if ((selection & EC_POSSIBLE_SELECTIONS) == 0)
        return 1; /* the selection is not missing */

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && (EC_KEY_get0_public_key(ec) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && (EC_KEY_get0_private_key(ec) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && (EC_KEY_get0_group(ec) != NULL);
    /*
     * We consider OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS to always be
     * available, so no extra check is needed other than the previous one
     * against EC_POSSIBLE_SELECTIONS.
     */
    return ok;
}

static int ec_match(const void *keydata1, const void *keydata2, int selection)
{
    const EC_KEY *ec1 = keydata1;
    const EC_KEY *ec2 = keydata2;
    const EC_GROUP *group_a = EC_KEY_get0_group(ec1);
    const EC_GROUP *group_b = EC_KEY_get0_group(ec2);
    BN_CTX *ctx = NULL;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(ec1));
    if (ctx == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && group_a != NULL && group_b != NULL
            && EC_GROUP_cmp(group_a, group_b, ctx) == 0;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            const EC_POINT *pa = EC_KEY_get0_public_key(ec1);
            const EC_POINT *pb = EC_KEY_get0_public_key(ec2);

            if (pa != NULL && pb != NULL) {
                ok = ok && EC_POINT_cmp(group_b, pa, pb, ctx) == 0;
                key_checked = 1;
            }
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            const BIGNUM *pa = EC_KEY_get0_private_key(ec1);
            const BIGNUM *pb = EC_KEY_get0_private_key(ec2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        ok = ok && key_checked;
    }
    BN_CTX_free(ctx);
    return ok;
}

static int common_check_sm2(const EC_KEY *ec, int sm2_wanted)
{
    const EC_GROUP *ecg = NULL;

    /*
     * sm2_wanted: import the keys or domparams only on SM2 Curve
     * !sm2_wanted: import the keys or domparams only not on SM2 Curve
     */
    if ((ecg = EC_KEY_get0_group(ec)) == NULL
        || (sm2_wanted ^ (EC_GROUP_get_curve_name(ecg) == NID_sm2)))
        return 0;
    return 1;
}

static
int common_import(void *keydata, int selection, const OSSL_PARAM params[],
                  int sm2_wanted)
{
    EC_KEY *ec = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || ec == NULL)
        return 0;

    /*
     * In this implementation, we can export/import only keydata in the
     * following combinations:
     *   - domain parameters (+optional other params)
     *   - public key with associated domain parameters (+optional other params)
     *   - private key with associated domain parameters and optional public key
     *         (+optional other params)
     *
     * This means:
     *   - domain parameters must always be requested
     *   - private key must be requested alongside public key
     *   - other parameters are always optional
     */
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0)
        return 0;

    ok = ok && ossl_ec_group_fromdata(ec, params);

    if (!common_check_sm2(ec, sm2_wanted))
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_ec_key_fromdata(ec, params, include_private);
    }
    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0)
        ok = ok && ossl_ec_key_otherparams_fromdata(ec, params);

    return ok;
}

static
int ec_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    return common_import(keydata, selection, params, 0);
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static
int sm2_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    return common_import(keydata, selection, params, 1);
}
# endif
#endif

static
int ec_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
              void *cbarg)
{
    EC_KEY *ec = keydata;
    OSSL_PARAM_BLD *tmpl = NULL;
    OSSL_PARAM *params = NULL;
    unsigned char *pub_key = NULL, *genbuf = NULL;
    BN_CTX *bnctx = NULL;
    int ok = 1;

    if (!ossl_prov_is_running() || ec == NULL)
        return 0;

    /*
     * In this implementation, we can export/import only keydata in the
     * following combinations:
     *   - domain parameters (+optional other params)
     *   - public key with associated domain parameters (+optional other params)
     *   - private key with associated public key and domain parameters
     *         (+optional other params)
     *
     * This means:
     *   - domain parameters must always be requested
     *   - private key must be requested alongside public key
     *   - other parameters are always optional
     */
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) == 0)
        return 0;
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
            && (selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        bnctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(ec));
        if (bnctx == NULL) {
            ok = 0;
            goto end;
        }
        BN_CTX_start(bnctx);
        ok = ok && ossl_ec_group_todata(EC_KEY_get0_group(ec), tmpl, NULL,
                                        ossl_ec_key_get_libctx(ec),
                                        ossl_ec_key_get0_propq(ec),
                                        bnctx, &genbuf);
    }

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && key_to_params(ec, tmpl, NULL, include_private, &pub_key);
    }
    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0)
        ok = ok && otherparams_to_params(ec, tmpl, NULL);

    if (!ok || (params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL) {
        ok = 0;
        goto end;
    }

    ok = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
end:
    OSSL_PARAM_BLD_free(tmpl);
    OPENSSL_free(pub_key);
    OPENSSL_free(genbuf);
    BN_CTX_end(bnctx);
    BN_CTX_free(bnctx);
    return ok;
}

/* IMEXPORT = IMPORT + EXPORT */

# define EC_IMEXPORTABLE_DOM_PARAMETERS                                        \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),               \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_ENCODING, NULL, 0),              \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT, NULL, 0),\
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_FIELD_TYPE, NULL, 0),            \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_P, NULL, 0),                              \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_A, NULL, 0),                              \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_B, NULL, 0),                              \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_GENERATOR, NULL, 0),            \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_ORDER, NULL, 0),                          \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_COFACTOR, NULL, 0),                       \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_SEED, NULL, 0),                 \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS, NULL)

# define EC_IMEXPORTABLE_PUBLIC_KEY                                            \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0)
# define EC_IMEXPORTABLE_PRIVATE_KEY                                           \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)
# define EC_IMEXPORTABLE_OTHER_PARAMETERS                                      \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_USE_COFACTOR_ECDH, NULL),                   \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC, NULL)

/*
 * Include all the possible combinations of OSSL_PARAM arrays for
 * ec_imexport_types().
 *
 * They are in a separate file as it is ~100 lines of unreadable and
 * uninteresting machine generated stuff.
 */
#include "ec_kmgmt_imexport.inc"

static ossl_inline
const OSSL_PARAM *ec_imexport_types(int selection)
{
    int type_select = 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        type_select += 1;
    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        type_select += 2;
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        type_select += 4;
    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0)
        type_select += 8;
    return ec_types[type_select];
}

static
const OSSL_PARAM *ec_import_types(int selection)
{
    return ec_imexport_types(selection);
}

static
const OSSL_PARAM *ec_export_types(int selection)
{
    return ec_imexport_types(selection);
}

static int ec_get_ecm_params(const EC_GROUP *group, OSSL_PARAM params[])
{
#ifdef OPENSSL_NO_EC2M
    return 1;
#else
    int ret = 0, m;
    unsigned int k1 = 0, k2 = 0, k3 = 0;
    int basis_nid;
    const char *basis_name = NULL;
    int fid = EC_GROUP_get_field_type(group);

    if (fid != NID_X9_62_characteristic_two_field)
        return 1;

    basis_nid = EC_GROUP_get_basis_type(group);
    if (basis_nid == NID_X9_62_tpBasis)
        basis_name = SN_X9_62_tpBasis;
    else if (basis_nid == NID_X9_62_ppBasis)
        basis_name = SN_X9_62_ppBasis;
    else
        goto err;

    m = EC_GROUP_get_degree(group);
    if (!ossl_param_build_set_int(NULL, params, OSSL_PKEY_PARAM_EC_CHAR2_M, m)
        || !ossl_param_build_set_utf8_string(NULL, params,
                                             OSSL_PKEY_PARAM_EC_CHAR2_TYPE,
                                             basis_name))
        goto err;

    if (basis_nid == NID_X9_62_tpBasis) {
        if (!EC_GROUP_get_trinomial_basis(group, &k1)
            || !ossl_param_build_set_int(NULL, params,
                                         OSSL_PKEY_PARAM_EC_CHAR2_TP_BASIS,
                                         (int)k1))
            goto err;
    } else {
        if (!EC_GROUP_get_pentanomial_basis(group, &k1, &k2, &k3)
            || !ossl_param_build_set_int(NULL, params,
                                         OSSL_PKEY_PARAM_EC_CHAR2_PP_K1, (int)k1)
            || !ossl_param_build_set_int(NULL, params,
                                         OSSL_PKEY_PARAM_EC_CHAR2_PP_K2, (int)k2)
            || !ossl_param_build_set_int(NULL, params,
                                         OSSL_PKEY_PARAM_EC_CHAR2_PP_K3, (int)k3))
            goto err;
    }
    ret = 1;
err:
    return ret;
#endif /* OPENSSL_NO_EC2M */
}

static
int common_get_params(void *key, OSSL_PARAM params[], int sm2)
{
    int ret = 0;
    EC_KEY *eck = key;
    const EC_GROUP *ecg = NULL;
    OSSL_PARAM *p;
    unsigned char *pub_key = NULL, *genbuf = NULL;
    OSSL_LIB_CTX *libctx;
    const char *propq;
    BN_CTX *bnctx = NULL;

    ecg = EC_KEY_get0_group(eck);
    if (ecg == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_PARAMETERS_SET);
        return 0;
    }

    libctx = ossl_ec_key_get_libctx(eck);
    propq = ossl_ec_key_get0_propq(eck);

    bnctx = BN_CTX_new_ex(libctx);
    if (bnctx == NULL)
        return 0;
    BN_CTX_start(bnctx);

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && !OSSL_PARAM_set_int(p, ECDSA_size(eck)))
        goto err;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, EC_GROUP_order_bits(ecg)))
        goto err;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL) {
        int ecbits, sec_bits;

        ecbits = EC_GROUP_order_bits(ecg);

        /*
         * The following estimates are based on the values published
         * in Table 2 of "NIST Special Publication 800-57 Part 1 Revision 4"
         * at http://dx.doi.org/10.6028/NIST.SP.800-57pt1r4 .
         *
         * Note that the above reference explicitly categorizes algorithms in a
         * discrete set of values {80, 112, 128, 192, 256}, and that it is
         * relevant only for NIST approved Elliptic Curves, while OpenSSL
         * applies the same logic also to other curves.
         *
         * Classifications produced by other standardazing bodies might differ,
         * so the results provided for "bits of security" by this provider are
         * to be considered merely indicative, and it is the users'
         * responsibility to compare these values against the normative
         * references that may be relevant for their intent and purposes.
         */
        if (ecbits >= 512)
            sec_bits = 256;
        else if (ecbits >= 384)
            sec_bits = 192;
        else if (ecbits >= 256)
            sec_bits = 128;
        else if (ecbits >= 224)
            sec_bits = 112;
        else if (ecbits >= 160)
            sec_bits = 80;
        else
            sec_bits = ecbits / 2;

        if (!OSSL_PARAM_set_int(p, sec_bits))
            goto err;
    }

    if ((p = OSSL_PARAM_locate(params,
                               OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS))
            != NULL) {
        int explicitparams = EC_KEY_decoded_from_explicit_params(eck);

        if (explicitparams < 0
             || !OSSL_PARAM_set_int(p, explicitparams))
            goto err;
    }

    if (!sm2) {
        if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_DEFAULT_DIGEST)) != NULL
                && !OSSL_PARAM_set_utf8_string(p, EC_DEFAULT_MD))
            goto err;
    } else {
        if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_DEFAULT_DIGEST)) != NULL
                && !OSSL_PARAM_set_utf8_string(p, SM2_DEFAULT_MD))
            goto err;
    }

    /* SM2 doesn't support this PARAM */
    if (!sm2) {
        p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_USE_COFACTOR_ECDH);
        if (p != NULL) {
            int ecdh_cofactor_mode = 0;

            ecdh_cofactor_mode =
                (EC_KEY_get_flags(eck) & EC_FLAG_COFACTOR_ECDH) ? 1 : 0;

            if (!OSSL_PARAM_set_int(p, ecdh_cofactor_mode))
                goto err;
        }
    }
    if ((p = OSSL_PARAM_locate(params,
                               OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY)) != NULL) {
        const EC_POINT *ecp = EC_KEY_get0_public_key(key);

        if (ecp == NULL) {
            ERR_raise(ERR_LIB_PROV, PROV_R_NOT_A_PUBLIC_KEY);
            goto err;
        }
        p->return_size = EC_POINT_point2oct(ecg, ecp,
                                            POINT_CONVERSION_UNCOMPRESSED,
                                            p->data, p->data_size, bnctx);
        if (p->return_size == 0)
            goto err;
    }

    ret = ec_get_ecm_params(ecg, params)
          && ossl_ec_group_todata(ecg, NULL, params, libctx, propq, bnctx,
                                  &genbuf)
          && key_to_params(eck, NULL, params, 1, &pub_key)
          && otherparams_to_params(eck, NULL, params);
err:
    OPENSSL_free(genbuf);
    OPENSSL_free(pub_key);
    BN_CTX_end(bnctx);
    BN_CTX_free(bnctx);
    return ret;
}

static
int ec_get_params(void *key, OSSL_PARAM params[])
{
    return common_get_params(key, params, 0);
}

#ifndef OPENSSL_NO_EC2M
# define EC2M_GETTABLE_DOM_PARAMS                                              \
        OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_CHAR2_M, NULL),                      \
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_CHAR2_TYPE, NULL, 0),        \
        OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_CHAR2_TP_BASIS, NULL),               \
        OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_CHAR2_PP_K1, NULL),                  \
        OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_CHAR2_PP_K2, NULL),                  \
        OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_CHAR2_PP_K3, NULL),
#else
# define EC2M_GETTABLE_DOM_PARAMS
#endif

static const OSSL_PARAM ec_known_gettable_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_DEFAULT_DIGEST, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS, NULL),
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC2M_GETTABLE_DOM_PARAMS
    EC_IMEXPORTABLE_PUBLIC_KEY,
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_PUB_X, NULL, 0),
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_PUB_Y, NULL, 0),
    EC_IMEXPORTABLE_PRIVATE_KEY,
    EC_IMEXPORTABLE_OTHER_PARAMETERS,
    OSSL_PARAM_END
};

static
const OSSL_PARAM *ec_gettable_params(void *provctx)
{
    return ec_known_gettable_params;
}

static const OSSL_PARAM ec_known_settable_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_USE_COFACTOR_ECDH, NULL),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_ENCODING, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_SEED, NULL, 0),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_INCLUDE_PUBLIC, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_GROUP_CHECK_TYPE, NULL, 0),
    OSSL_PARAM_END
};

static
const OSSL_PARAM *ec_settable_params(void *provctx)
{
    return ec_known_settable_params;
}

static
int ec_set_params(void *key, const OSSL_PARAM params[])
{
    EC_KEY *eck = key;
    const OSSL_PARAM *p;

    if (key == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!ossl_ec_group_set_params((EC_GROUP *)EC_KEY_get0_group(key), params))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL) {
        BN_CTX *ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(key));
        int ret = 1;

        if (ctx == NULL
                || p->data_type != OSSL_PARAM_OCTET_STRING
                || !EC_KEY_oct2key(key, p->data, p->data_size, ctx))
            ret = 0;
        BN_CTX_free(ctx);
        if (!ret)
            return 0;
    }

    return ossl_ec_key_otherparams_fromdata(eck, params);
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static
int sm2_get_params(void *key, OSSL_PARAM params[])
{
    return common_get_params(key, params, 1);
}

static const OSSL_PARAM sm2_known_gettable_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_DEFAULT_DIGEST, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_EC_DECODED_FROM_EXPLICIT_PARAMS, NULL),
    EC_IMEXPORTABLE_DOM_PARAMETERS,
    EC_IMEXPORTABLE_PUBLIC_KEY,
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_PUB_X, NULL, 0),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_PUB_Y, NULL, 0),
    EC_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};

static
const OSSL_PARAM *sm2_gettable_params(ossl_unused void *provctx)
{
    return sm2_known_gettable_params;
}

static const OSSL_PARAM sm2_known_settable_params[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static
const OSSL_PARAM *sm2_settable_params(ossl_unused void *provctx)
{
    return sm2_known_settable_params;
}

static
int sm2_validate(const void *keydata, int selection, int checktype)
{
    const EC_KEY *eck = keydata;
    int ok = 1;
    BN_CTX *ctx = NULL;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & EC_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(eck));
    if  (ctx == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && EC_GROUP_check(EC_KEY_get0_group(eck), ctx);

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        if (checktype == OSSL_KEYMGMT_VALIDATE_QUICK_CHECK)
            ok = ok && ossl_ec_key_public_check_quick(eck, ctx);
        else
            ok = ok && ossl_ec_key_public_check(eck, ctx);
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && ossl_sm2_key_private_check(eck);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == OSSL_KEYMGMT_SELECT_KEYPAIR)
        ok = ok && ossl_ec_key_pairwise_check(eck, ctx);

    BN_CTX_free(ctx);
    return ok;
}
# endif
#endif

static
int ec_validate(const void *keydata, int selection, int checktype)
{
    const EC_KEY *eck = keydata;
    int ok = 1;
    BN_CTX *ctx = NULL;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & EC_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    ctx = BN_CTX_new_ex(ossl_ec_key_get_libctx(eck));
    if  (ctx == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        int flags = EC_KEY_get_flags(eck);

        if ((flags & EC_FLAG_CHECK_NAMED_GROUP) != 0)
            ok = ok && EC_GROUP_check_named_curve(EC_KEY_get0_group(eck),
                           (flags & EC_FLAG_CHECK_NAMED_GROUP_NIST) != 0, ctx) > 0;
        else
            ok = ok && EC_GROUP_check(EC_KEY_get0_group(eck), ctx);
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        if (checktype == OSSL_KEYMGMT_VALIDATE_QUICK_CHECK)
            ok = ok && ossl_ec_key_public_check_quick(eck, ctx);
        else
            ok = ok && ossl_ec_key_public_check(eck, ctx);
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && ossl_ec_key_private_check(eck);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == OSSL_KEYMGMT_SELECT_KEYPAIR)
        ok = ok && ossl_ec_key_pairwise_check(eck, ctx);

    BN_CTX_free(ctx);
    return ok;
}

struct ec_gen_ctx {
    OSSL_LIB_CTX *libctx;
    char *group_name;
    char *encoding;
    char *pt_format;
    char *group_check;
    char *field_type;
    BIGNUM *p, *a, *b, *order, *cofactor;
    unsigned char *gen, *seed;
    size_t gen_len, seed_len;
    int selection;
    int ecdh_mode;
    EC_GROUP *gen_group;
    unsigned char *dhkem_ikm;
    size_t dhkem_ikmlen;
    OSSL_FIPS_IND_DECLARE
};

static void *ec_gen_init(void *provctx, int selection,
                         const OSSL_PARAM params[])
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct ec_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running() || (selection & (EC_POSSIBLE_SELECTIONS)) == 0)
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->libctx = libctx;
        gctx->selection = selection;
        gctx->ecdh_mode = 0;
        OSSL_FIPS_IND_INIT(gctx)
        if (!ec_gen_set_params(gctx, params)) {
            OPENSSL_free(gctx);
            gctx = NULL;
        }
    }
    return gctx;
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static void *sm2_gen_init(void *provctx, int selection,
                         const OSSL_PARAM params[])
{
    struct ec_gen_ctx *gctx = ec_gen_init(provctx, selection, params);

    if (gctx != NULL) {
        if (gctx->group_name != NULL)
            return gctx;
        if ((gctx->group_name = OPENSSL_strdup("sm2")) != NULL)
            return gctx;
        ec_gen_cleanup(gctx);
    }
    return NULL;
}
# endif
#endif

static int ec_gen_set_group(void *genctx, const EC_GROUP *src)
{
    struct ec_gen_ctx *gctx = genctx;
    EC_GROUP *group;

    group = EC_GROUP_dup(src);
    if (group == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_CURVE);
        return 0;
    }
    EC_GROUP_free(gctx->gen_group);
    gctx->gen_group = group;
    return 1;
}

static int ec_gen_set_template(void *genctx, void *templ)
{
    struct ec_gen_ctx *gctx = genctx;
    EC_KEY *ec = templ;
    const EC_GROUP *ec_group;

    if (!ossl_prov_is_running() || gctx == NULL || ec == NULL)
        return 0;
    if ((ec_group = EC_KEY_get0_group(ec)) == NULL)
        return 0;
    return ec_gen_set_group(gctx, ec_group);
}

#define COPY_INT_PARAM(params, key, val)                                       \
p = OSSL_PARAM_locate_const(params, key);                                      \
if (p != NULL && !OSSL_PARAM_get_int(p, &val))                                 \
    goto err;

#define COPY_UTF8_PARAM(params, key, val)                                      \
p = OSSL_PARAM_locate_const(params, key);                                      \
if (p != NULL) {                                                               \
    if (p->data_type != OSSL_PARAM_UTF8_STRING)                                \
        goto err;                                                              \
    OPENSSL_free(val);                                                         \
    val = OPENSSL_strdup(p->data);                                             \
    if (val == NULL)                                                           \
        goto err;                                                              \
}

#define COPY_OCTET_PARAM(params, key, val, len)                                \
p = OSSL_PARAM_locate_const(params, key);                                      \
if (p != NULL) {                                                               \
    if (p->data_type != OSSL_PARAM_OCTET_STRING)                               \
        goto err;                                                              \
    OPENSSL_free(val);                                                         \
    len = p->data_size;                                                        \
    val = OPENSSL_memdup(p->data, p->data_size);                               \
    if (val == NULL)                                                           \
        goto err;                                                              \
}

#define COPY_BN_PARAM(params, key, bn)                                         \
p = OSSL_PARAM_locate_const(params, key);                                      \
if (p != NULL) {                                                               \
    if (bn == NULL)                                                            \
        bn = BN_new();                                                         \
    if (bn == NULL || !OSSL_PARAM_get_BN(p, &bn))                              \
        goto err;                                                              \
}

static int ec_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    int ret = 0;
    struct ec_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(gctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_PKEY_PARAM_FIPS_KEY_CHECK))
        goto err;

    COPY_INT_PARAM(params, OSSL_PKEY_PARAM_USE_COFACTOR_ECDH, gctx->ecdh_mode);

    COPY_UTF8_PARAM(params, OSSL_PKEY_PARAM_GROUP_NAME, gctx->group_name);
    COPY_UTF8_PARAM(params, OSSL_PKEY_PARAM_EC_FIELD_TYPE, gctx->field_type);
    COPY_UTF8_PARAM(params, OSSL_PKEY_PARAM_EC_ENCODING, gctx->encoding);
    COPY_UTF8_PARAM(params, OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT, gctx->pt_format);
    COPY_UTF8_PARAM(params, OSSL_PKEY_PARAM_EC_GROUP_CHECK_TYPE, gctx->group_check);

    COPY_BN_PARAM(params, OSSL_PKEY_PARAM_EC_P, gctx->p);
    COPY_BN_PARAM(params, OSSL_PKEY_PARAM_EC_A, gctx->a);
    COPY_BN_PARAM(params, OSSL_PKEY_PARAM_EC_B, gctx->b);
    COPY_BN_PARAM(params, OSSL_PKEY_PARAM_EC_ORDER, gctx->order);
    COPY_BN_PARAM(params, OSSL_PKEY_PARAM_EC_COFACTOR, gctx->cofactor);

    COPY_OCTET_PARAM(params, OSSL_PKEY_PARAM_EC_SEED, gctx->seed, gctx->seed_len);
    COPY_OCTET_PARAM(params, OSSL_PKEY_PARAM_EC_GENERATOR, gctx->gen,
                     gctx->gen_len);

    COPY_OCTET_PARAM(params, OSSL_PKEY_PARAM_DHKEM_IKM, gctx->dhkem_ikm,
                     gctx->dhkem_ikmlen);

    ret = 1;
err:
    return ret;
}

static int ec_gen_set_group_from_params(struct ec_gen_ctx *gctx)
{
    int ret = 0;
    OSSL_PARAM_BLD *bld;
    OSSL_PARAM *params = NULL;
    EC_GROUP *group = NULL;

    bld = OSSL_PARAM_BLD_new();
    if (bld == NULL)
        return 0;

    if (gctx->encoding != NULL
        && !OSSL_PARAM_BLD_push_utf8_string(bld, OSSL_PKEY_PARAM_EC_ENCODING,
                                            gctx->encoding, 0))
        goto err;

    if (gctx->pt_format != NULL
        && !OSSL_PARAM_BLD_push_utf8_string(bld,
                                            OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT,
                                            gctx->pt_format, 0))
        goto err;

    if (gctx->group_name != NULL) {
        if (!OSSL_PARAM_BLD_push_utf8_string(bld, OSSL_PKEY_PARAM_GROUP_NAME,
                                             gctx->group_name, 0))
            goto err;
        /* Ignore any other parameters if there is a group name */
        goto build;
    } else if (gctx->field_type != NULL) {
        if (!OSSL_PARAM_BLD_push_utf8_string(bld, OSSL_PKEY_PARAM_EC_FIELD_TYPE,
                                             gctx->field_type, 0))
            goto err;
    } else {
        goto err;
    }
    if (gctx->p == NULL
        || gctx->a == NULL
        || gctx->b == NULL
        || gctx->order == NULL
        || !OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_P, gctx->p)
        || !OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_A, gctx->a)
        || !OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_B, gctx->b)
        || !OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_ORDER, gctx->order))
        goto err;

    if (gctx->cofactor != NULL
        && !OSSL_PARAM_BLD_push_BN(bld, OSSL_PKEY_PARAM_EC_COFACTOR,
                                   gctx->cofactor))
        goto err;

    if (gctx->seed != NULL
        && !OSSL_PARAM_BLD_push_octet_string(bld, OSSL_PKEY_PARAM_EC_SEED,
                                             gctx->seed, gctx->seed_len))
        goto err;

    if (gctx->gen == NULL
        || !OSSL_PARAM_BLD_push_octet_string(bld, OSSL_PKEY_PARAM_EC_GENERATOR,
                                             gctx->gen, gctx->gen_len))
        goto err;
build:
    params = OSSL_PARAM_BLD_to_param(bld);
    if (params == NULL)
        goto err;
    group = EC_GROUP_new_from_params(params, gctx->libctx, NULL);
    if (group == NULL)
        goto err;

    EC_GROUP_free(gctx->gen_group);
    gctx->gen_group = group;

    ret = 1;
err:
    OSSL_PARAM_free(params);
    OSSL_PARAM_BLD_free(bld);
    return ret;
}

static const OSSL_PARAM *ec_gen_settable_params(ossl_unused void *genctx,
                                                ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_USE_COFACTOR_ECDH, NULL),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_ENCODING, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_POINT_CONVERSION_FORMAT, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_EC_FIELD_TYPE, NULL, 0),
        OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_P, NULL, 0),
        OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_A, NULL, 0),
        OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_B, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_GENERATOR, NULL, 0),
        OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_ORDER, NULL, 0),
        OSSL_PARAM_BN(OSSL_PKEY_PARAM_EC_COFACTOR, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_EC_SEED, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_DHKEM_IKM, NULL, 0),
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_PKEY_PARAM_FIPS_KEY_CHECK)
        OSSL_PARAM_END
    };
    return settable;
}

static const OSSL_PARAM *ec_gen_gettable_params(ossl_unused void *genctx,
                                                ossl_unused void *provctx)
{
    static const OSSL_PARAM known_ec_gen_gettable_ctx_params[] = {
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };
    return known_ec_gen_gettable_ctx_params;
}

static int ec_gen_get_params(void *genctx, OSSL_PARAM *params)
{
    struct ec_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return 0;

    if (!OSSL_FIPS_IND_GET_CTX_PARAM(gctx, params))
        return 0;

    return 1;
}

static int ec_gen_assign_group(EC_KEY *ec, EC_GROUP *group)
{
    if (group == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_NO_PARAMETERS_SET);
        return 0;
    }
    return EC_KEY_set_group(ec, group) > 0;
}

/*
 * The callback arguments (osslcb & cbarg) are not used by EC_KEY generation
 */
static void *ec_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct ec_gen_ctx *gctx = genctx;
    EC_KEY *ec = NULL;
    int ret = 0;

    if (!ossl_prov_is_running()
        || gctx == NULL
        || (ec = EC_KEY_new_ex(gctx->libctx, NULL)) == NULL)
        return NULL;

    if (gctx->gen_group == NULL) {
        if (!ec_gen_set_group_from_params(gctx))
            goto err;
    } else {
        if (gctx->encoding != NULL) {
            int flags = ossl_ec_encoding_name2id(gctx->encoding);

            if (flags < 0)
                goto err;
            EC_GROUP_set_asn1_flag(gctx->gen_group, flags);
        }
        if (gctx->pt_format != NULL) {
            int format = ossl_ec_pt_format_name2id(gctx->pt_format);

            if (format < 0)
                goto err;
            EC_GROUP_set_point_conversion_form(gctx->gen_group, format);
        }
    }
#ifdef FIPS_MODULE
    if (!ossl_fips_ind_ec_key_check(OSSL_FIPS_IND_GET(gctx),
                                    OSSL_FIPS_IND_SETTABLE0, gctx->libctx,
                                    gctx->gen_group, "EC KeyGen", 1))
        goto err;
#endif

    /* We must always assign a group, no matter what */
    ret = ec_gen_assign_group(ec, gctx->gen_group);

    /* Whether you want it or not, you get a keypair, not just one half */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
#ifndef FIPS_MODULE
        if (gctx->dhkem_ikm != NULL && gctx->dhkem_ikmlen != 0)
            ret = ret && ossl_ec_generate_key_dhkem(ec, gctx->dhkem_ikm,
                                                    gctx->dhkem_ikmlen);
        else
#endif
            ret = ret && EC_KEY_generate_key(ec);
    }

    if (gctx->ecdh_mode != -1)
        ret = ret && ossl_ec_set_ecdh_cofactor_mode(ec, gctx->ecdh_mode);

    if (gctx->group_check != NULL)
        ret = ret && ossl_ec_set_check_group_type_from_name(ec,
                                                            gctx->group_check);
    if (ret)
        return ec;
err:
    /* Something went wrong, throw the key away */
    EC_KEY_free(ec);
    return NULL;
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
/*
 * The callback arguments (osslcb & cbarg) are not used by EC_KEY generation
 */
static void *sm2_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct ec_gen_ctx *gctx = genctx;
    EC_KEY *ec = NULL;
    int ret = 1;

    if (gctx == NULL
        || (ec = EC_KEY_new_ex(gctx->libctx, NULL)) == NULL)
        return NULL;

    if (gctx->gen_group == NULL) {
        if (!ec_gen_set_group_from_params(gctx))
            goto err;
    } else {
        if (gctx->encoding) {
            int flags = ossl_ec_encoding_name2id(gctx->encoding);

            if (flags < 0)
                goto err;
            EC_GROUP_set_asn1_flag(gctx->gen_group, flags);
        }
        if (gctx->pt_format != NULL) {
            int format = ossl_ec_pt_format_name2id(gctx->pt_format);

            if (format < 0)
                goto err;
            EC_GROUP_set_point_conversion_form(gctx->gen_group, format);
        }
    }

    /* We must always assign a group, no matter what */
    ret = ec_gen_assign_group(ec, gctx->gen_group);

    /* Whether you want it or not, you get a keypair, not just one half */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        ret = ret && EC_KEY_generate_key(ec);

    if (ret)
        return ec;
err:
    /* Something went wrong, throw the key away */
    EC_KEY_free(ec);
    return NULL;
}
# endif
#endif

static void ec_gen_cleanup(void *genctx)
{
    struct ec_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_clear_free(gctx->dhkem_ikm, gctx->dhkem_ikmlen);
    EC_GROUP_free(gctx->gen_group);
    BN_free(gctx->p);
    BN_free(gctx->a);
    BN_free(gctx->b);
    BN_free(gctx->order);
    BN_free(gctx->cofactor);
    OPENSSL_free(gctx->group_name);
    OPENSSL_free(gctx->field_type);
    OPENSSL_free(gctx->pt_format);
    OPENSSL_free(gctx->encoding);
    OPENSSL_free(gctx->seed);
    OPENSSL_free(gctx->gen);
    OPENSSL_free(gctx);
}

static void *common_load(const void *reference, size_t reference_sz,
                         int sm2_wanted)
{
    EC_KEY *ec = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(ec)) {
        /* The contents of the reference is the address to our object */
        ec = *(EC_KEY **)reference;

        if (!common_check_sm2(ec, sm2_wanted))
            return NULL;

        /* We grabbed, so we detach it */
        *(EC_KEY **)reference = NULL;
        return ec;
    }
    return NULL;
}

static void *ec_load(const void *reference, size_t reference_sz)
{
    return common_load(reference, reference_sz, 0);
}

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
static void *sm2_load(const void *reference, size_t reference_sz)
{
    return common_load(reference, reference_sz, 1);
}
# endif
#endif

static void *ec_dup(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_ec_key_dup(keydata_from, selection);
    return NULL;
}

const OSSL_DISPATCH ossl_ec_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))ec_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))ec_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE,
      (void (*)(void))ec_gen_set_template },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))ec_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))ec_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN_GET_PARAMS, (void (*)(void))ec_gen_get_params },
    { OSSL_FUNC_KEYMGMT_GEN_GETTABLE_PARAMS,
      (void (*)(void))ec_gen_gettable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))ec_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))ec_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))ec_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))ec_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))ec_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))ec_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))ec_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))ec_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))ec_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))ec_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))ec_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))ec_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))ec_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))ec_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))ec_export_types },
    { OSSL_FUNC_KEYMGMT_QUERY_OPERATION_NAME,
      (void (*)(void))ec_query_operation_name },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))ec_dup },
    OSSL_DISPATCH_END
};

#ifndef FIPS_MODULE
# ifndef OPENSSL_NO_SM2
const OSSL_DISPATCH ossl_sm2_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))sm2_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))sm2_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE,
      (void (*)(void))ec_gen_set_template },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))ec_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))ec_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))sm2_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))ec_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))sm2_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))ec_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))sm2_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))sm2_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))ec_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))sm2_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))ec_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))ec_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))sm2_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))sm2_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))ec_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))ec_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))ec_export_types },
    { OSSL_FUNC_KEYMGMT_QUERY_OPERATION_NAME,
      (void (*)(void))sm2_query_operation_name },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))ec_dup },
    OSSL_DISPATCH_END
};
# endif
#endif
