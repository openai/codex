/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
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

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include "prov/securitycheck.h"
#include "prov/providercommon.h"
#include "prov/implementations.h"
#include "prov/provider_ctx.h"
#include "crypto/dsa.h"
#include "internal/sizes.h"
#include "internal/nelem.h"
#include "internal/param_build_set.h"

static OSSL_FUNC_keymgmt_new_fn dsa_newdata;
static OSSL_FUNC_keymgmt_free_fn dsa_freedata;
static OSSL_FUNC_keymgmt_gen_init_fn dsa_gen_init;
static OSSL_FUNC_keymgmt_gen_set_template_fn dsa_gen_set_template;
static OSSL_FUNC_keymgmt_gen_set_params_fn dsa_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn dsa_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_get_params_fn dsa_gen_get_params;
static OSSL_FUNC_keymgmt_gen_gettable_params_fn dsa_gen_gettable_params;
static OSSL_FUNC_keymgmt_gen_fn dsa_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn dsa_gen_cleanup;
static OSSL_FUNC_keymgmt_load_fn dsa_load;
static OSSL_FUNC_keymgmt_get_params_fn dsa_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn dsa_gettable_params;
static OSSL_FUNC_keymgmt_has_fn dsa_has;
static OSSL_FUNC_keymgmt_match_fn dsa_match;
static OSSL_FUNC_keymgmt_validate_fn dsa_validate;
static OSSL_FUNC_keymgmt_import_fn dsa_import;
static OSSL_FUNC_keymgmt_import_types_fn dsa_import_types;
static OSSL_FUNC_keymgmt_export_fn dsa_export;
static OSSL_FUNC_keymgmt_export_types_fn dsa_export_types;
static OSSL_FUNC_keymgmt_dup_fn dsa_dup;

#define DSA_DEFAULT_MD "SHA256"
#define DSA_POSSIBLE_SELECTIONS                                                \
    (OSSL_KEYMGMT_SELECT_KEYPAIR | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS)

struct dsa_gen_ctx {
    OSSL_LIB_CTX *libctx;

    FFC_PARAMS *ffc_params;
    int selection;
    /* All these parameters are used for parameter generation only */
    size_t pbits;
    size_t qbits;
    unsigned char *seed; /* optional FIPS186-4 param for testing */
    size_t seedlen;
    int gindex; /* optional  FIPS186-4 generator index (ignored if -1) */
    int gen_type; /* DSA_PARAMGEN_TYPE_FIPS_186_2 or DSA_PARAMGEN_TYPE_FIPS_186_4 */
    int pcounter;
    int hindex;
    char *mdname;
    char *mdprops;
    OSSL_CALLBACK *cb;
    void *cbarg;
    OSSL_FIPS_IND_DECLARE
};
typedef struct dh_name2id_st{
    const char *name;
    int id;
} DSA_GENTYPE_NAME2ID;

static const DSA_GENTYPE_NAME2ID dsatype2id[] = {
#ifdef FIPS_MODULE
    { "default", DSA_PARAMGEN_TYPE_FIPS_186_4 },
#else
    { "default", DSA_PARAMGEN_TYPE_FIPS_DEFAULT },
#endif
    { "fips186_4", DSA_PARAMGEN_TYPE_FIPS_186_4 },
    { "fips186_2", DSA_PARAMGEN_TYPE_FIPS_186_2 },
};

static int dsa_gen_type_name2id(const char *name)
{
    size_t i;

    for (i = 0; i < OSSL_NELEM(dsatype2id); ++i) {
        if (OPENSSL_strcasecmp(dsatype2id[i].name, name) == 0)
            return dsatype2id[i].id;
    }
    return -1;
}

static int dsa_key_todata(DSA *dsa, OSSL_PARAM_BLD *bld, OSSL_PARAM params[],
                          int include_private)
{
    const BIGNUM *priv = NULL, *pub = NULL;

    if (dsa == NULL)
        return 0;

    DSA_get0_key(dsa, &pub, &priv);
    if (include_private
        && priv != NULL
        && !ossl_param_build_set_bn(bld, params, OSSL_PKEY_PARAM_PRIV_KEY, priv))
        return 0;
    if (pub != NULL
        && !ossl_param_build_set_bn(bld, params, OSSL_PKEY_PARAM_PUB_KEY, pub))
        return 0;

    return 1;
}

static void *dsa_newdata(void *provctx)
{
    if (!ossl_prov_is_running())
        return NULL;
    return ossl_dsa_new(PROV_LIBCTX_OF(provctx));
}

static void dsa_freedata(void *keydata)
{
    DSA_free(keydata);
}

static int dsa_has(const void *keydata, int selection)
{
    const DSA *dsa = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || dsa == NULL)
        return 0;
    if ((selection & DSA_POSSIBLE_SELECTIONS) == 0)
        return 1; /* the selection is not missing */

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && (DSA_get0_pub_key(dsa) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && (DSA_get0_priv_key(dsa) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && (DSA_get0_p(dsa) != NULL && DSA_get0_g(dsa) != NULL);
    return ok;
}

static int dsa_match(const void *keydata1, const void *keydata2, int selection)
{
    const DSA *dsa1 = keydata1;
    const DSA *dsa2 = keydata2;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            const BIGNUM *pa = DSA_get0_pub_key(dsa1);
            const BIGNUM *pb = DSA_get0_pub_key(dsa2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            const BIGNUM *pa = DSA_get0_priv_key(dsa1);
            const BIGNUM *pb = DSA_get0_priv_key(dsa2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        ok = ok && key_checked;
    }
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        FFC_PARAMS *dsaparams1 = ossl_dsa_get0_params((DSA *)dsa1);
        FFC_PARAMS *dsaparams2 = ossl_dsa_get0_params((DSA *)dsa2);

        ok = ok && ossl_ffc_params_cmp(dsaparams1, dsaparams2, 1);
    }
    return ok;
}

static int dsa_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    DSA *dsa = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || dsa == NULL)
        return 0;

    if ((selection & DSA_POSSIBLE_SELECTIONS) == 0)
        return 0;

    /* a key without parameters is meaningless */
    ok = ok && ossl_dsa_ffc_params_fromdata(dsa, params);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_dsa_key_fromdata(dsa, params, include_private);
    }

    return ok;
}

static int dsa_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
                      void *cbarg)
{
    DSA *dsa = keydata;
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ok = 1;

    if (!ossl_prov_is_running() || dsa == NULL)
        return 0;

    if ((selection & DSA_POSSIBLE_SELECTIONS) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_ALL_PARAMETERS) != 0)
        ok = ok && ossl_ffc_params_todata(ossl_dsa_get0_params(dsa), tmpl, NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && dsa_key_todata(dsa, tmpl, NULL, include_private);
    }

    if (!ok || (params = OSSL_PARAM_BLD_to_param(tmpl)) == NULL) {
        ok = 0;
        goto err;
    }

    ok = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return ok;
}

/* IMEXPORT = IMPORT + EXPORT */

# define DSA_IMEXPORTABLE_PARAMETERS                                           \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_P, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_Q, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_G, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_COFACTOR, NULL, 0),                      \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_GINDEX, NULL),                          \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_PCOUNTER, NULL),                        \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_H, NULL),                               \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_FFC_SEED, NULL, 0)
# define DSA_IMEXPORTABLE_PUBLIC_KEY                    \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0)
# define DSA_IMEXPORTABLE_PRIVATE_KEY                   \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)
static const OSSL_PARAM dsa_all_types[] = {
    DSA_IMEXPORTABLE_PARAMETERS,
    DSA_IMEXPORTABLE_PUBLIC_KEY,
    DSA_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM dsa_parameter_types[] = {
    DSA_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM dsa_key_types[] = {
    DSA_IMEXPORTABLE_PUBLIC_KEY,
    DSA_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM *dsa_types[] = {
    NULL,                        /* Index 0 = none of them */
    dsa_parameter_types,          /* Index 1 = parameter types */
    dsa_key_types,                /* Index 2 = key types */
    dsa_all_types                 /* Index 3 = 1 + 2 */
};

static const OSSL_PARAM *dsa_imexport_types(int selection)
{
    int type_select = 0;

    if ((selection & OSSL_KEYMGMT_SELECT_ALL_PARAMETERS) != 0)
        type_select += 1;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        type_select += 2;
    return dsa_types[type_select];
}

static const OSSL_PARAM *dsa_import_types(int selection)
{
    return dsa_imexport_types(selection);
}

static const OSSL_PARAM *dsa_export_types(int selection)
{
    return dsa_imexport_types(selection);
}

static ossl_inline int dsa_get_params(void *key, OSSL_PARAM params[])
{
    DSA *dsa = key;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, DSA_bits(dsa)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, DSA_security_bits(dsa)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && !OSSL_PARAM_set_int(p, DSA_size(dsa)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_DEFAULT_DIGEST)) != NULL
        && !OSSL_PARAM_set_utf8_string(p, DSA_DEFAULT_MD))
        return 0;
    return ossl_ffc_params_todata(ossl_dsa_get0_params(dsa), NULL, params)
           && dsa_key_todata(dsa, NULL, params, 1);
}

static const OSSL_PARAM dsa_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_DEFAULT_DIGEST, NULL, 0),
    DSA_IMEXPORTABLE_PARAMETERS,
    DSA_IMEXPORTABLE_PUBLIC_KEY,
    DSA_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};

static const OSSL_PARAM *dsa_gettable_params(void *provctx)
{
    return dsa_params;
}

static int dsa_validate_domparams(const DSA *dsa, int checktype)
{
    int status = 0;

    return ossl_dsa_check_params(dsa, checktype, &status);
}

static int dsa_validate_public(const DSA *dsa)
{
    int status = 0;
    const BIGNUM *pub_key = NULL;

    DSA_get0_key(dsa, &pub_key, NULL);
    if (pub_key == NULL)
        return 0;
    return ossl_dsa_check_pub_key(dsa, pub_key, &status);
}

static int dsa_validate_private(const DSA *dsa)
{
    int status = 0;
    const BIGNUM *priv_key = NULL;

    DSA_get0_key(dsa, NULL, &priv_key);
    if (priv_key == NULL)
        return 0;
    return ossl_dsa_check_priv_key(dsa, priv_key, &status);
}

static int dsa_validate(const void *keydata, int selection, int checktype)
{
    const DSA *dsa = keydata;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & DSA_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && dsa_validate_domparams(dsa, checktype);

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && dsa_validate_public(dsa);

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && dsa_validate_private(dsa);

    /* If the whole key is selected, we do a pairwise validation */
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR)
        == OSSL_KEYMGMT_SELECT_KEYPAIR)
        ok = ok && ossl_dsa_check_pairwise(dsa);
    return ok;
}

static void *dsa_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[])
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct dsa_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running() || (selection & DSA_POSSIBLE_SELECTIONS) == 0)
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->selection = selection;
        gctx->libctx = libctx;
        gctx->pbits = 2048;
        gctx->qbits = 224;
#ifdef FIPS_MODULE
        gctx->gen_type = DSA_PARAMGEN_TYPE_FIPS_186_4;
#else
        gctx->gen_type = DSA_PARAMGEN_TYPE_FIPS_DEFAULT;
#endif
        gctx->gindex = -1;
        gctx->pcounter = -1;
        gctx->hindex = 0;
        OSSL_FIPS_IND_INIT(gctx)
    }
    if (!dsa_gen_set_params(gctx, params)) {
        dsa_gen_cleanup(gctx);
        gctx = NULL;
    }
    return gctx;
}

static int dsa_gen_set_template(void *genctx, void *templ)
{
    struct dsa_gen_ctx *gctx = genctx;
    DSA *dsa = templ;

    if (!ossl_prov_is_running() || gctx == NULL || dsa == NULL)
        return 0;
    gctx->ffc_params = ossl_dsa_get0_params(dsa);
    return 1;
}

static int dsa_set_gen_seed(struct dsa_gen_ctx *gctx, unsigned char *seed,
                            size_t seedlen)
{
    OPENSSL_clear_free(gctx->seed, gctx->seedlen);
    gctx->seed = NULL;
    gctx->seedlen = 0;
    if (seed != NULL && seedlen > 0) {
        gctx->seed = OPENSSL_memdup(seed, seedlen);
        if (gctx->seed == NULL)
            return 0;
        gctx->seedlen = seedlen;
    }
    return 1;
}

static int dsa_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct dsa_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;
    int gen_type = -1;

    if (gctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    if (!OSSL_FIPS_IND_SET_CTX_PARAM(gctx, OSSL_FIPS_IND_SETTABLE0, params,
                                     OSSL_PKEY_PARAM_FIPS_SIGN_CHECK))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_TYPE);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || ((gen_type = dsa_gen_type_name2id(p->data)) == -1)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }

        /*
         * Only assign context gen_type if it was set by dsa_gen_type_name2id
         * must be in range:
         * DSA_PARAMGEN_TYPE_FIPS_186_4 <= gen_type <= DSA_PARAMGEN_TYPE_FIPS_DEFAULT
         */
        if (gen_type != -1)
            gctx->gen_type = gen_type;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_GINDEX);
    if (p != NULL
        && !OSSL_PARAM_get_int(p, &gctx->gindex))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PCOUNTER);
    if (p != NULL
        && !OSSL_PARAM_get_int(p, &gctx->pcounter))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_H);
    if (p != NULL
        && !OSSL_PARAM_get_int(p, &gctx->hindex))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_SEED);
    if (p != NULL
        && (p->data_type != OSSL_PARAM_OCTET_STRING
            || !dsa_set_gen_seed(gctx, p->data, p->data_size)))
            return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PBITS)) != NULL
        && !OSSL_PARAM_get_size_t(p, &gctx->pbits))
        return 0;
    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_QBITS)) != NULL
        && !OSSL_PARAM_get_size_t(p, &gctx->qbits))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_DIGEST);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->mdname);
        gctx->mdname = OPENSSL_strdup(p->data);
        if (gctx->mdname == NULL)
            return 0;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_DIGEST_PROPS);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->mdprops);
        gctx->mdprops = OPENSSL_strdup(p->data);
        if (gctx->mdprops == NULL)
            return 0;
    }
    return 1;
}

static const OSSL_PARAM *dsa_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_TYPE, NULL, 0),
        OSSL_PARAM_size_t(OSSL_PKEY_PARAM_FFC_PBITS, NULL),
        OSSL_PARAM_size_t(OSSL_PKEY_PARAM_FFC_QBITS, NULL),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST_PROPS, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_GINDEX, NULL),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_FFC_SEED, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_PCOUNTER, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_H, NULL),
        OSSL_FIPS_IND_SETTABLE_CTX_PARAM(OSSL_PKEY_PARAM_FIPS_SIGN_CHECK)
        OSSL_PARAM_END
    };
    return settable;
}

static int dsa_gen_get_params(void *genctx, OSSL_PARAM *params)
{
    struct dsa_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;
    if (!OSSL_FIPS_IND_GET_CTX_PARAM(gctx, params))
        return 0;
    return 1;
}

static const OSSL_PARAM *dsa_gen_gettable_params(ossl_unused void *ctx,
                                                 ossl_unused void *provctx)
{
    static const OSSL_PARAM dsa_gen_gettable_params_table[] = {
        OSSL_FIPS_IND_GETTABLE_CTX_PARAM()
        OSSL_PARAM_END
    };

    return dsa_gen_gettable_params_table;
}

static int dsa_gencb(int p, int n, BN_GENCB *cb)
{
    struct dsa_gen_ctx *gctx = BN_GENCB_get_arg(cb);
    OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };

    params[0] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_POTENTIAL, &p);
    params[1] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_ITERATION, &n);

    return gctx->cb(params, gctx->cbarg);
}

static void *dsa_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct dsa_gen_ctx *gctx = genctx;
    DSA *dsa = NULL;
    BN_GENCB *gencb = NULL;
    int ret = 0;
    FFC_PARAMS *ffc;

    if (!ossl_prov_is_running() || gctx == NULL)
        return NULL;

#ifdef FIPS_MODULE
    /*
     * DSA signing is not approved in FIPS 140-3, so there is no
     * need for DSA keygen either.
     */
    if (!OSSL_FIPS_IND_ON_UNAPPROVED(gctx, OSSL_FIPS_IND_SETTABLE0,
                                     gctx->libctx, "DSA", "Keygen",
                                     ossl_fips_config_dsa_sign_disallowed))
        return 0;
#endif

    dsa = ossl_dsa_new(gctx->libctx);
    if (dsa == NULL)
        return NULL;

    if (gctx->gen_type == DSA_PARAMGEN_TYPE_FIPS_DEFAULT)
        gctx->gen_type = (gctx->pbits >= 2048 ? DSA_PARAMGEN_TYPE_FIPS_186_4 :
                                                DSA_PARAMGEN_TYPE_FIPS_186_2);

    /*
     * Do a bounds check on context gen_type. Must be in range:
     * DSA_PARAMGEN_TYPE_FIPS_186_4 <= gen_type <= DSA_PARAMGEN_TYPE_FIPS_DEFAULT
     * Noted here as this needs to be adjusted if a new type is
     * added.
     */
    if (!ossl_assert((gctx->gen_type >= DSA_PARAMGEN_TYPE_FIPS_186_4)
                    && (gctx->gen_type <= DSA_PARAMGEN_TYPE_FIPS_DEFAULT))) {
        ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                       "gen_type set to unsupported value %d", gctx->gen_type);
        goto end;
    }

    gctx->cb = osslcb;
    gctx->cbarg = cbarg;
    gencb = BN_GENCB_new();
    if (gencb != NULL)
        BN_GENCB_set(gencb, dsa_gencb, genctx);

    ffc = ossl_dsa_get0_params(dsa);
    /* Copy the template value if one was passed */
    if (gctx->ffc_params != NULL
        && !ossl_ffc_params_copy(ffc, gctx->ffc_params))
        goto end;

    if (gctx->seed != NULL
        && !ossl_ffc_params_set_seed(ffc, gctx->seed, gctx->seedlen))
        goto end;
    if (gctx->gindex != -1) {
        ossl_ffc_params_set_gindex(ffc, gctx->gindex);
        if (gctx->pcounter != -1)
            ossl_ffc_params_set_pcounter(ffc, gctx->pcounter);
    } else if (gctx->hindex != 0) {
        ossl_ffc_params_set_h(ffc, gctx->hindex);
    }
    if (gctx->mdname != NULL)
        ossl_ffc_set_digest(ffc, gctx->mdname, gctx->mdprops);

    if ((gctx->selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {

         if (ossl_dsa_generate_ffc_parameters(dsa, gctx->gen_type,
                                              gctx->pbits, gctx->qbits,
                                              gencb) <= 0)
             goto end;
    }
    ossl_ffc_params_enable_flags(ffc, FFC_PARAM_FLAG_VALIDATE_LEGACY,
                                 gctx->gen_type == DSA_PARAMGEN_TYPE_FIPS_186_2);
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if (ffc->p == NULL
            || ffc->q == NULL
            || ffc->g == NULL)
            goto end;
        if (DSA_generate_key(dsa) <= 0)
            goto end;
    }
    ret = 1;
end:
    if (ret <= 0) {
        DSA_free(dsa);
        dsa = NULL;
    }
    BN_GENCB_free(gencb);
    return dsa;
}

static void dsa_gen_cleanup(void *genctx)
{
    struct dsa_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_free(gctx->mdname);
    OPENSSL_free(gctx->mdprops);
    OPENSSL_clear_free(gctx->seed, gctx->seedlen);
    OPENSSL_free(gctx);
}

static void *dsa_load(const void *reference, size_t reference_sz)
{
    DSA *dsa = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(dsa)) {
        /* The contents of the reference is the address to our object */
        dsa = *(DSA **)reference;
        /* We grabbed, so we detach it */
        *(DSA **)reference = NULL;
        return dsa;
    }
    return NULL;
}

static void *dsa_dup(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_dsa_dup(keydata_from, selection);
    return NULL;
}

const OSSL_DISPATCH ossl_dsa_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))dsa_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))dsa_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE, (void (*)(void))dsa_gen_set_template },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))dsa_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))dsa_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN_GET_PARAMS, (void (*)(void))dsa_gen_get_params },
    { OSSL_FUNC_KEYMGMT_GEN_GETTABLE_PARAMS,
      (void (*)(void))dsa_gen_gettable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))dsa_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))dsa_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))dsa_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))dsa_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))dsa_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))dsa_gettable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))dsa_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))dsa_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))dsa_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))dsa_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))dsa_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))dsa_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))dsa_export_types },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))dsa_dup },
    OSSL_DISPATCH_END
};
