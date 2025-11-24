/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * RSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/proverr.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "crypto/rsa.h"
#include "crypto/cryptlib.h"
#include "internal/param_build_set.h"

static OSSL_FUNC_keymgmt_new_fn rsa_newdata;
static OSSL_FUNC_keymgmt_new_fn rsapss_newdata;
static OSSL_FUNC_keymgmt_gen_init_fn rsa_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn rsapss_gen_init;
static OSSL_FUNC_keymgmt_gen_set_params_fn rsa_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn rsa_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn rsapss_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_fn rsa_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn rsa_gen_cleanup;
static OSSL_FUNC_keymgmt_load_fn rsa_load;
static OSSL_FUNC_keymgmt_load_fn rsapss_load;
static OSSL_FUNC_keymgmt_free_fn rsa_freedata;
static OSSL_FUNC_keymgmt_get_params_fn rsa_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn rsa_gettable_params;
static OSSL_FUNC_keymgmt_has_fn rsa_has;
static OSSL_FUNC_keymgmt_match_fn rsa_match;
static OSSL_FUNC_keymgmt_validate_fn rsa_validate;
static OSSL_FUNC_keymgmt_import_fn rsa_import;
static OSSL_FUNC_keymgmt_import_types_fn rsa_import_types;
static OSSL_FUNC_keymgmt_export_fn rsa_export;
static OSSL_FUNC_keymgmt_export_types_fn rsa_export_types;
static OSSL_FUNC_keymgmt_query_operation_name_fn rsa_query_operation_name;
static OSSL_FUNC_keymgmt_dup_fn rsa_dup;

#define RSA_DEFAULT_MD "SHA256"
#define RSA_POSSIBLE_SELECTIONS                                        \
    (OSSL_KEYMGMT_SELECT_KEYPAIR | OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS)

DEFINE_STACK_OF(BIGNUM)
DEFINE_SPECIAL_STACK_OF_CONST(BIGNUM_const, BIGNUM)

static int pss_params_fromdata(RSA_PSS_PARAMS_30 *pss_params, int *defaults_set,
                               const OSSL_PARAM params[], int rsa_type,
                               OSSL_LIB_CTX *libctx)
{
    if (!ossl_rsa_pss_params_30_fromdata(pss_params, defaults_set,
                                         params, libctx))
        return 0;

    /* If not a PSS type RSA, sending us PSS parameters is wrong */
    if (rsa_type != RSA_FLAG_TYPE_RSASSAPSS
        && !ossl_rsa_pss_params_30_is_unrestricted(pss_params))
        return 0;

    return 1;
}

static void *rsa_newdata(void *provctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    RSA *rsa;

    if (!ossl_prov_is_running())
        return NULL;

    rsa = ossl_rsa_new_with_ctx(libctx);
    if (rsa != NULL) {
        RSA_clear_flags(rsa, RSA_FLAG_TYPE_MASK);
        RSA_set_flags(rsa, RSA_FLAG_TYPE_RSA);
    }
    return rsa;
}

static void *rsapss_newdata(void *provctx)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    RSA *rsa;

    if (!ossl_prov_is_running())
        return NULL;

    rsa = ossl_rsa_new_with_ctx(libctx);
    if (rsa != NULL) {
        RSA_clear_flags(rsa, RSA_FLAG_TYPE_MASK);
        RSA_set_flags(rsa, RSA_FLAG_TYPE_RSASSAPSS);
    }
    return rsa;
}

static void rsa_freedata(void *keydata)
{
    RSA_free(keydata);
}

static int rsa_has(const void *keydata, int selection)
{
    const RSA *rsa = keydata;
    int ok = 1;

    if (rsa == NULL || !ossl_prov_is_running())
        return 0;
    if ((selection & RSA_POSSIBLE_SELECTIONS) == 0)
        return 1; /* the selection is not missing */

    /* OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS are always available even if empty */
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        ok = ok && (RSA_get0_n(rsa) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && (RSA_get0_e(rsa) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && (RSA_get0_d(rsa) != NULL);
    return ok;
}

static int rsa_match(const void *keydata1, const void *keydata2, int selection)
{
    const RSA *rsa1 = keydata1;
    const RSA *rsa2 = keydata2;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    /* There is always an |e| */
    ok = ok && BN_cmp(RSA_get0_e(rsa1), RSA_get0_e(rsa2)) == 0;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            const BIGNUM *pa = RSA_get0_n(rsa1);
            const BIGNUM *pb = RSA_get0_n(rsa2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            const BIGNUM *pa = RSA_get0_d(rsa1);
            const BIGNUM *pb = RSA_get0_d(rsa2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        ok = ok && key_checked;
    }
    return ok;
}

static int rsa_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    RSA *rsa = keydata;
    int rsa_type;
    int ok = 1;
    int pss_defaults_set = 0;

    if (!ossl_prov_is_running() || rsa == NULL)
        return 0;

    if ((selection & RSA_POSSIBLE_SELECTIONS) == 0)
        return 0;

    rsa_type = RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK);

    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0)
        ok = ok && pss_params_fromdata(ossl_rsa_get0_pss_params_30(rsa),
                                       &pss_defaults_set,
                                       params, rsa_type,
                                       ossl_rsa_get0_libctx(rsa));
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_rsa_fromdata(rsa, params, include_private);
    }

    return ok;
}

static int rsa_export(void *keydata, int selection,
                      OSSL_CALLBACK *param_callback, void *cbarg)
{
    RSA *rsa = keydata;
    const RSA_PSS_PARAMS_30 *pss_params = ossl_rsa_get0_pss_params_30(rsa);
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ok = 1;

    if (!ossl_prov_is_running() || rsa == NULL)
        return 0;

    if ((selection & RSA_POSSIBLE_SELECTIONS) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_OTHER_PARAMETERS) != 0)
        ok = ok && (ossl_rsa_pss_params_30_is_unrestricted(pss_params)
                    || ossl_rsa_pss_params_30_todata(pss_params, tmpl, NULL));
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_rsa_todata(rsa, tmpl, NULL, include_private);
    }

    if (!ok || (params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL) {
        ok = 0;
        goto err;
    }

    ok = param_callback(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return ok;
}

#ifdef FIPS_MODULE
/* In fips mode there are no multi-primes. */
# define RSA_KEY_MP_TYPES()                                                    \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR1, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR2, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT1, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT2, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT1, NULL, 0),
#else
/*
 * We allow up to 10 prime factors (starting with p, q).
 * NOTE: there is only 9 OSSL_PKEY_PARAM_RSA_COEFFICIENT
 */
# define RSA_KEY_MP_TYPES()                                                    \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR1, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR2, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR3, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR4, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR5, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR6, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR7, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR8, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR9, NULL, 0),                           \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_FACTOR10, NULL, 0),                          \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT1, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT2, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT3, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT4, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT5, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT6, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT7, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT8, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT9, NULL, 0),                         \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_EXPONENT10, NULL, 0),                        \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT1, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT2, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT3, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT4, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT5, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT6, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT7, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT8, NULL, 0),                      \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_COEFFICIENT9, NULL, 0),
#endif

#define RSA_KEY_TYPES()                                                        \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_N, NULL, 0),                                 \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_E, NULL, 0),                                 \
OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_D, NULL, 0),                                 \
RSA_KEY_MP_TYPES()

/*
 * This provider can export everything in an RSA key, so we use the exact
 * same type description for export as for import.  Other providers might
 * choose to import full keys, but only export the public parts, and will
 * therefore have the importkey_types and importkey_types functions return
 * different arrays.
 */
static const OSSL_PARAM rsa_key_types[] = {
    RSA_KEY_TYPES()
    OSSL_PARAM_END
};
/*
 * We lied about the amount of factors, exponents and coefficients, the
 * export and import functions can really deal with an infinite amount
 * of these numbers.  However, RSA keys with too many primes are futile,
 * so we at least pretend to have some limits.
 */

static const OSSL_PARAM *rsa_imexport_types(int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return rsa_key_types;
    return NULL;
}

static const OSSL_PARAM *rsa_import_types(int selection)
{
    return rsa_imexport_types(selection);
}

static const OSSL_PARAM *rsa_export_types(int selection)
{
    return rsa_imexport_types(selection);
}

static int rsa_get_params(void *key, OSSL_PARAM params[])
{
    RSA *rsa = key;
    const RSA_PSS_PARAMS_30 *pss_params = ossl_rsa_get0_pss_params_30(rsa);
    int rsa_type = RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK);
    OSSL_PARAM *p;
    int empty = RSA_get0_n(rsa) == NULL;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && (empty || !OSSL_PARAM_set_int(p, RSA_bits(rsa))))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && (empty || !OSSL_PARAM_set_int(p, RSA_security_bits(rsa))))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && (empty || !OSSL_PARAM_set_int(p, RSA_size(rsa))))
        return 0;

    /*
     * For restricted RSA-PSS keys, we ignore the default digest request.
     * With RSA-OAEP keys, this may need to be amended.
     */
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_DEFAULT_DIGEST)) != NULL
        && (rsa_type != RSA_FLAG_TYPE_RSASSAPSS
            || ossl_rsa_pss_params_30_is_unrestricted(pss_params))) {
        if (!OSSL_PARAM_set_utf8_string(p, RSA_DEFAULT_MD))
            return 0;
    }

    /*
     * For non-RSA-PSS keys, we ignore the mandatory digest request.
     * With RSA-OAEP keys, this may need to be amended.
     */
    if ((p = OSSL_PARAM_locate(params,
                               OSSL_PKEY_PARAM_MANDATORY_DIGEST)) != NULL
        && rsa_type == RSA_FLAG_TYPE_RSASSAPSS
        && !ossl_rsa_pss_params_30_is_unrestricted(pss_params)) {
        const char *mdname =
            ossl_rsa_oaeppss_nid2name(ossl_rsa_pss_params_30_hashalg(pss_params));

        if (mdname == NULL || !OSSL_PARAM_set_utf8_string(p, mdname))
            return 0;
    }
    return (rsa_type != RSA_FLAG_TYPE_RSASSAPSS
            || ossl_rsa_pss_params_30_todata(pss_params, NULL, params))
        && ossl_rsa_todata(rsa, NULL, params, 1);
}

static const OSSL_PARAM rsa_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_DEFAULT_DIGEST, NULL, 0),
    RSA_KEY_TYPES()
    OSSL_PARAM_END
};

static const OSSL_PARAM *rsa_gettable_params(void *provctx)
{
    return rsa_params;
}

static int rsa_validate(const void *keydata, int selection, int checktype)
{
    const RSA *rsa = keydata;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & RSA_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    /* If the whole key is selected, we do a pairwise validation */
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR)
        == OSSL_KEYMGMT_SELECT_KEYPAIR) {
        ok = ok && ossl_rsa_validate_pairwise(rsa);
    } else {
        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
            ok = ok && ossl_rsa_validate_private(rsa);
        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
            ok = ok && ossl_rsa_validate_public(rsa);
    }
    return ok;
}

struct rsa_gen_ctx {
    OSSL_LIB_CTX *libctx;
    const char *propq;

    int rsa_type;

    size_t nbits;
    BIGNUM *pub_exp;
    size_t primes;

    /* For PSS */
    RSA_PSS_PARAMS_30 pss_params;
    int pss_defaults_set;

    /* For generation callback */
    OSSL_CALLBACK *cb;
    void *cbarg;

#if defined(FIPS_MODULE) && !defined(OPENSSL_NO_ACVP_TESTS)
    /* ACVP test parameters */
    OSSL_PARAM *acvp_test_params;
#endif
};

static int rsa_gencb(int p, int n, BN_GENCB *cb)
{
    struct rsa_gen_ctx *gctx = BN_GENCB_get_arg(cb);
    OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };

    params[0] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_POTENTIAL, &p);
    params[1] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_ITERATION, &n);
    return gctx->cb(params, gctx->cbarg);
}

static void *gen_init(void *provctx, int selection, int rsa_type,
                      const OSSL_PARAM params[])
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct rsa_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->libctx = libctx;
        if ((gctx->pub_exp = BN_new()) == NULL
            || !BN_set_word(gctx->pub_exp, RSA_F4)) {
            goto err;
        }
        gctx->nbits = 2048;
        gctx->primes = RSA_DEFAULT_PRIME_NUM;
        gctx->rsa_type = rsa_type;
    } else {
        goto err;
    }

    if (!rsa_gen_set_params(gctx, params))
        goto err;
    return gctx;

err:
    if (gctx != NULL)
        BN_free(gctx->pub_exp);
    OPENSSL_free(gctx);
    return NULL;
}

static void *rsa_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[])
{
    return gen_init(provctx, selection, RSA_FLAG_TYPE_RSA, params);
}

static void *rsapss_gen_init(void *provctx, int selection,
                             const OSSL_PARAM params[])
{
    return gen_init(provctx, selection, RSA_FLAG_TYPE_RSASSAPSS, params);
}

/*
 * This function is common for all RSA sub-types, to detect possible
 * misuse, such as PSS parameters being passed when a plain RSA key
 * is generated.
 */
static int rsa_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct rsa_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (ossl_param_is_empty(params))
        return 1;

    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_BITS)) != NULL) {
        if (!OSSL_PARAM_get_size_t(p, &gctx->nbits))
            return 0;
        if (gctx->nbits < RSA_MIN_MODULUS_BITS) {
            ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SIZE_TOO_SMALL);
            return 0;
        }
    }
    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_PRIMES)) != NULL
        && !OSSL_PARAM_get_size_t(p, &gctx->primes))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_RSA_E)) != NULL
        && !OSSL_PARAM_get_BN(p, &gctx->pub_exp))
        return 0;
    /* Only attempt to get PSS parameters when generating an RSA-PSS key */
    if (gctx->rsa_type == RSA_FLAG_TYPE_RSASSAPSS
        && !pss_params_fromdata(&gctx->pss_params, &gctx->pss_defaults_set, params,
                                gctx->rsa_type, gctx->libctx))
        return 0;
#if defined(FIPS_MODULE) && !defined(OPENSSL_NO_ACVP_TESTS)
    /* Any ACVP test related parameters are copied into a params[] */
    if (!ossl_rsa_acvp_test_gen_params_new(&gctx->acvp_test_params, params))
        return 0;
#endif
    return 1;
}

#define rsa_gen_basic                                           \
    OSSL_PARAM_size_t(OSSL_PKEY_PARAM_RSA_BITS, NULL),          \
    OSSL_PARAM_size_t(OSSL_PKEY_PARAM_RSA_PRIMES, NULL),        \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_RSA_E, NULL, 0)

/*
 * The following must be kept in sync with ossl_rsa_pss_params_30_fromdata()
 * in crypto/rsa/rsa_backend.c
 */
#define rsa_gen_pss                                                     \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_RSA_DIGEST, NULL, 0),        \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_RSA_DIGEST_PROPS, NULL, 0),  \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_RSA_MASKGENFUNC, NULL, 0),   \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_RSA_MGF1_DIGEST, NULL, 0),   \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_RSA_PSS_SALTLEN, NULL)

static const OSSL_PARAM *rsa_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        rsa_gen_basic,
        OSSL_PARAM_END
    };

    return settable;
}

static const OSSL_PARAM *rsapss_gen_settable_params(ossl_unused void *genctx,
                                                    ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        rsa_gen_basic,
        rsa_gen_pss,
        OSSL_PARAM_END
    };

    return settable;
}

static void *rsa_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct rsa_gen_ctx *gctx = genctx;
    RSA *rsa = NULL, *rsa_tmp = NULL;
    BN_GENCB *gencb = NULL;

    if (!ossl_prov_is_running() || gctx == NULL)
        return NULL;

    switch (gctx->rsa_type) {
    case RSA_FLAG_TYPE_RSA:
        /* For plain RSA keys, PSS parameters must not be set */
        if (!ossl_rsa_pss_params_30_is_unrestricted(&gctx->pss_params))
            goto err;
        break;
    case RSA_FLAG_TYPE_RSASSAPSS:
        /*
         * For plain RSA-PSS keys, PSS parameters may be set but don't have
         * to, so not check.
         */
        break;
    default:
        /* Unsupported RSA key sub-type... */
        return NULL;
    }

    if ((rsa_tmp = ossl_rsa_new_with_ctx(gctx->libctx)) == NULL)
        return NULL;

    gctx->cb = osslcb;
    gctx->cbarg = cbarg;
    gencb = BN_GENCB_new();
    if (gencb != NULL)
        BN_GENCB_set(gencb, rsa_gencb, genctx);

#if defined(FIPS_MODULE) && !defined(OPENSSL_NO_ACVP_TESTS)
    if (gctx->acvp_test_params != NULL) {
        if (!ossl_rsa_acvp_test_set_params(rsa_tmp, gctx->acvp_test_params))
            goto err;
    }
#endif

    if (!RSA_generate_multi_prime_key(rsa_tmp,
                                      (int)gctx->nbits, (int)gctx->primes,
                                      gctx->pub_exp, gencb))
        goto err;

    if (!ossl_rsa_pss_params_30_copy(ossl_rsa_get0_pss_params_30(rsa_tmp),
                                     &gctx->pss_params))
        goto err;

    RSA_clear_flags(rsa_tmp, RSA_FLAG_TYPE_MASK);
    RSA_set_flags(rsa_tmp, gctx->rsa_type);

    rsa = rsa_tmp;
    rsa_tmp = NULL;
 err:
    BN_GENCB_free(gencb);
    RSA_free(rsa_tmp);
    return rsa;
}

static void rsa_gen_cleanup(void *genctx)
{
    struct rsa_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;
#if defined(FIPS_MODULE) && !defined(OPENSSL_NO_ACVP_TESTS)
    ossl_rsa_acvp_test_gen_params_free(gctx->acvp_test_params);
    gctx->acvp_test_params = NULL;
#endif
    BN_clear_free(gctx->pub_exp);
    OPENSSL_free(gctx);
}

static void *common_load(const void *reference, size_t reference_sz,
                         int expected_rsa_type)
{
    RSA *rsa = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(rsa)) {
        /* The contents of the reference is the address to our object */
        rsa = *(RSA **)reference;

        if (RSA_test_flags(rsa, RSA_FLAG_TYPE_MASK) != expected_rsa_type)
            return NULL;

        /* We grabbed, so we detach it */
        *(RSA **)reference = NULL;
        return rsa;
    }
    return NULL;
}

static void *rsa_load(const void *reference, size_t reference_sz)
{
    return common_load(reference, reference_sz, RSA_FLAG_TYPE_RSA);
}

static void *rsapss_load(const void *reference, size_t reference_sz)
{
    return common_load(reference, reference_sz, RSA_FLAG_TYPE_RSASSAPSS);
}

static void *rsa_dup(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running()
        /* do not allow creating empty keys by duplication */
        && (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return ossl_rsa_dup(keydata_from, selection);
    return NULL;
}

/* For any RSA key, we use the "RSA" algorithms regardless of sub-type. */
static const char *rsa_query_operation_name(int operation_id)
{
    return "RSA";
}

const OSSL_DISPATCH ossl_rsa_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))rsa_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))rsa_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS,
      (void (*)(void))rsa_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))rsa_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))rsa_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))rsa_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))rsa_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))rsa_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))rsa_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))rsa_gettable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))rsa_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))rsa_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))rsa_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))rsa_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))rsa_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))rsa_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))rsa_export_types },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))rsa_dup },
    OSSL_DISPATCH_END
};

const OSSL_DISPATCH ossl_rsapss_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))rsapss_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))rsapss_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))rsa_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))rsapss_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))rsa_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))rsa_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))rsapss_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))rsa_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))rsa_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))rsa_gettable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))rsa_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))rsa_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))rsa_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))rsa_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))rsa_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))rsa_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))rsa_export_types },
    { OSSL_FUNC_KEYMGMT_QUERY_OPERATION_NAME,
      (void (*)(void))rsa_query_operation_name },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))rsa_dup },
    OSSL_DISPATCH_END
};
