/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
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
#include "internal/common.h"

#include <string.h> /* strcmp */
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/bn.h>
#include <openssl/err.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "crypto/dh.h"
#include "internal/sizes.h"

static OSSL_FUNC_keymgmt_new_fn dh_newdata;
static OSSL_FUNC_keymgmt_free_fn dh_freedata;
static OSSL_FUNC_keymgmt_gen_init_fn dh_gen_init;
static OSSL_FUNC_keymgmt_gen_init_fn dhx_gen_init;
static OSSL_FUNC_keymgmt_gen_set_template_fn dh_gen_set_template;
static OSSL_FUNC_keymgmt_gen_set_params_fn dh_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn dh_gen_settable_params;
static OSSL_FUNC_keymgmt_gen_fn dh_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn dh_gen_cleanup;
static OSSL_FUNC_keymgmt_load_fn dh_load;
static OSSL_FUNC_keymgmt_get_params_fn dh_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn dh_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn dh_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn dh_settable_params;
static OSSL_FUNC_keymgmt_has_fn dh_has;
static OSSL_FUNC_keymgmt_match_fn dh_match;
static OSSL_FUNC_keymgmt_validate_fn dh_validate;
static OSSL_FUNC_keymgmt_import_fn dh_import;
static OSSL_FUNC_keymgmt_import_types_fn dh_import_types;
static OSSL_FUNC_keymgmt_export_fn dh_export;
static OSSL_FUNC_keymgmt_export_types_fn dh_export_types;
static OSSL_FUNC_keymgmt_dup_fn dh_dup;

#define DH_POSSIBLE_SELECTIONS                                                 \
    (OSSL_KEYMGMT_SELECT_KEYPAIR | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS)

struct dh_gen_ctx {
    OSSL_LIB_CTX *libctx;

    FFC_PARAMS *ffc_params;
    int selection;
    /* All these parameters are used for parameter generation only */
    /* If there is a group name then the remaining parameters are not needed */
    int group_nid;
    size_t pbits;
    size_t qbits;
    unsigned char *seed; /* optional FIPS186-4 param for testing */
    size_t seedlen;
    int gindex; /* optional  FIPS186-4 generator index (ignored if -1) */
    int gen_type; /* see dhtype2id */
    int generator; /* Used by DH_PARAMGEN_TYPE_GENERATOR in non fips mode only */
    int pcounter;
    int hindex;
    int priv_len;

    char *mdname;
    char *mdprops;
    OSSL_CALLBACK *cb;
    void *cbarg;
    int dh_type;
};

static int dh_gen_type_name2id_w_default(const char *name, int type)
{
    if (strcmp(name, "default") == 0) {
#ifdef FIPS_MODULE
        if (type == DH_FLAG_TYPE_DHX)
            return DH_PARAMGEN_TYPE_FIPS_186_4;

        return DH_PARAMGEN_TYPE_GROUP;
#else
        if (type == DH_FLAG_TYPE_DHX)
            return DH_PARAMGEN_TYPE_FIPS_186_2;

        return DH_PARAMGEN_TYPE_GENERATOR;
#endif
    }

    return ossl_dh_gen_type_name2id(name, type);
}

static void *dh_newdata(void *provctx)
{
    DH *dh = NULL;

    if (ossl_prov_is_running()) {
        dh = ossl_dh_new_ex(PROV_LIBCTX_OF(provctx));
        if (dh != NULL) {
            DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
            DH_set_flags(dh, DH_FLAG_TYPE_DH);
        }
    }
    return dh;
}

static void *dhx_newdata(void *provctx)
{
    DH *dh = NULL;

    dh = ossl_dh_new_ex(PROV_LIBCTX_OF(provctx));
    if (dh != NULL) {
        DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
        DH_set_flags(dh, DH_FLAG_TYPE_DHX);
    }
    return dh;
}

static void dh_freedata(void *keydata)
{
    DH_free(keydata);
}

static int dh_has(const void *keydata, int selection)
{
    const DH *dh = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || dh == NULL)
        return 0;
    if ((selection & DH_POSSIBLE_SELECTIONS) == 0)
        return 1; /* the selection is not missing */

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && (DH_get0_pub_key(dh) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && (DH_get0_priv_key(dh) != NULL);
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && (DH_get0_p(dh) != NULL && DH_get0_g(dh) != NULL);
    return ok;
}

static int dh_match(const void *keydata1, const void *keydata2, int selection)
{
    const DH *dh1 = keydata1;
    const DH *dh2 = keydata2;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            const BIGNUM *pa = DH_get0_pub_key(dh1);
            const BIGNUM *pb = DH_get0_pub_key(dh2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            const BIGNUM *pa = DH_get0_priv_key(dh1);
            const BIGNUM *pb = DH_get0_priv_key(dh2);

            if (pa != NULL && pb != NULL) {
                ok = ok && BN_cmp(pa, pb) == 0;
                key_checked = 1;
            }
        }
        ok = ok && key_checked;
    }
    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        FFC_PARAMS *dhparams1 = ossl_dh_get0_params((DH *)dh1);
        FFC_PARAMS *dhparams2 = ossl_dh_get0_params((DH *)dh2);

        ok = ok && ossl_ffc_params_cmp(dhparams1, dhparams2, 1);
    }
    return ok;
}

static int dh_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    DH *dh = keydata;
    int ok = 1;

    if (!ossl_prov_is_running() || dh == NULL)
        return 0;

    if ((selection & DH_POSSIBLE_SELECTIONS) == 0)
        return 0;

    /* a key without parameters is meaningless */
    ok = ok && ossl_dh_params_fromdata(dh, params);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_dh_key_fromdata(dh, params, include_private);
    }

    return ok;
}

static int dh_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
                     void *cbarg)
{
    DH *dh = keydata;
    OSSL_PARAM_BLD *tmpl = NULL;
    OSSL_PARAM *params = NULL;
    int ok = 1;

    if (!ossl_prov_is_running() || dh == NULL)
        return 0;

    if ((selection & DH_POSSIBLE_SELECTIONS) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_ALL_PARAMETERS) != 0)
        ok = ok && ossl_dh_params_todata(dh, tmpl, NULL);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private =
            selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;

        ok = ok && ossl_dh_key_todata(dh, tmpl, NULL, include_private);
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

# define DH_IMEXPORTABLE_PARAMETERS                                            \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_P, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_Q, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_G, NULL, 0),                             \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_FFC_COFACTOR, NULL, 0),                      \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_GINDEX, NULL),                          \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_PCOUNTER, NULL),                        \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_H, NULL),                               \
    OSSL_PARAM_int(OSSL_PKEY_PARAM_DH_PRIV_LEN, NULL),                         \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_FFC_SEED, NULL, 0),                \
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0)
# define DH_IMEXPORTABLE_PUBLIC_KEY                                            \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0)
# define DH_IMEXPORTABLE_PRIVATE_KEY                                           \
    OSSL_PARAM_BN(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)
static const OSSL_PARAM dh_all_types[] = {
    DH_IMEXPORTABLE_PARAMETERS,
    DH_IMEXPORTABLE_PUBLIC_KEY,
    DH_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM dh_parameter_types[] = {
    DH_IMEXPORTABLE_PARAMETERS,
    OSSL_PARAM_END
};
static const OSSL_PARAM dh_key_types[] = {
    DH_IMEXPORTABLE_PUBLIC_KEY,
    DH_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};
static const OSSL_PARAM *dh_types[] = {
    NULL,                        /* Index 0 = none of them */
    dh_parameter_types,          /* Index 1 = parameter types */
    dh_key_types,                /* Index 2 = key types */
    dh_all_types                 /* Index 3 = 1 + 2 */
};

static const OSSL_PARAM *dh_imexport_types(int selection)
{
    int type_select = 0;

    if ((selection & OSSL_KEYMGMT_SELECT_ALL_PARAMETERS) != 0)
        type_select += 1;
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        type_select += 2;
    return dh_types[type_select];
}

static const OSSL_PARAM *dh_import_types(int selection)
{
    return dh_imexport_types(selection);
}

static const OSSL_PARAM *dh_export_types(int selection)
{
    return dh_imexport_types(selection);
}

static ossl_inline int dh_get_params(void *key, OSSL_PARAM params[])
{
    DH *dh = key;
    OSSL_PARAM *p;

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, DH_bits(dh)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, DH_security_bits(dh)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && !OSSL_PARAM_set_int(p, DH_size(dh)))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY)) != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;
        p->return_size = ossl_dh_key2buf(dh, (unsigned char **)&p->data,
                                         p->data_size, 0);
        if (p->return_size == 0)
            return 0;
    }

    return ossl_dh_params_todata(dh, NULL, params)
        && ossl_dh_key_todata(dh, NULL, params, 1);
}

static const OSSL_PARAM dh_params[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    DH_IMEXPORTABLE_PARAMETERS,
    DH_IMEXPORTABLE_PUBLIC_KEY,
    DH_IMEXPORTABLE_PRIVATE_KEY,
    OSSL_PARAM_END
};

static const OSSL_PARAM *dh_gettable_params(void *provctx)
{
    return dh_params;
}

static const OSSL_PARAM dh_known_settable_params[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *dh_settable_params(void *provctx)
{
    return dh_known_settable_params;
}

static int dh_set_params(void *key, const OSSL_PARAM params[])
{
    DH *dh = key;
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL
            && (p->data_type != OSSL_PARAM_OCTET_STRING
                || !ossl_dh_buf2key(dh, p->data, p->data_size)))
        return 0;

    return 1;
}

static int dh_validate_public(const DH *dh, int checktype)
{
    const BIGNUM *pub_key = NULL;
    int res = 0;

    DH_get0_key(dh, &pub_key, NULL);
    if (pub_key == NULL)
        return 0;

    /*
     * The partial test is only valid for named group's with q = (p - 1) / 2
     * but for that case it is also fully sufficient to check the key validity.
     */
    if (ossl_dh_is_named_safe_prime_group(dh))
        return ossl_dh_check_pub_key_partial(dh, pub_key, &res);

    return DH_check_pub_key_ex(dh, pub_key);
}

static int dh_validate_private(const DH *dh)
{
    int status = 0;
    const BIGNUM *priv_key = NULL;

    DH_get0_key(dh, NULL, &priv_key);
    if (priv_key == NULL)
        return 0;
    return ossl_dh_check_priv_key(dh, priv_key, &status);
}

static int dh_validate(const void *keydata, int selection, int checktype)
{
    const DH *dh = keydata;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & DH_POSSIBLE_SELECTIONS) == 0)
        return 1; /* nothing to validate */

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
        /*
         * Both of these functions check parameters. DH_check_params_ex()
         * performs a lightweight check (e.g. it does not check that p is a
         * safe prime)
         */
        if (checktype == OSSL_KEYMGMT_VALIDATE_QUICK_CHECK)
            ok = ok && DH_check_params_ex(dh);
        else
            ok = ok && DH_check_ex(dh);
    }

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0)
        ok = ok && dh_validate_public(dh, checktype);

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        ok = ok && dh_validate_private(dh);

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR)
            == OSSL_KEYMGMT_SELECT_KEYPAIR)
        ok = ok && ossl_dh_check_pairwise(dh);
    return ok;
}

static void *dh_gen_init_base(void *provctx, int selection,
                              const OSSL_PARAM params[], int type)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct dh_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((selection & (OSSL_KEYMGMT_SELECT_KEYPAIR
                      | OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS)) == 0)
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->selection = selection;
        gctx->libctx = libctx;
        gctx->pbits = 2048;
        gctx->qbits = 224;
        gctx->mdname = NULL;
#ifdef FIPS_MODULE
        gctx->gen_type = (type == DH_FLAG_TYPE_DHX)
                         ? DH_PARAMGEN_TYPE_FIPS_186_4
                         : DH_PARAMGEN_TYPE_GROUP;
#else
        gctx->gen_type = (type == DH_FLAG_TYPE_DHX)
                         ? DH_PARAMGEN_TYPE_FIPS_186_2
                         : DH_PARAMGEN_TYPE_GENERATOR;
#endif
        gctx->gindex = -1;
        gctx->hindex = 0;
        gctx->pcounter = -1;
        gctx->generator = DH_GENERATOR_2;
        gctx->dh_type = type;
    }
    if (!dh_gen_set_params(gctx, params)) {
        OPENSSL_free(gctx);
        gctx = NULL;
    }
    return gctx;
}

static void *dh_gen_init(void *provctx, int selection,
                         const OSSL_PARAM params[])
{
    return dh_gen_init_base(provctx, selection, params, DH_FLAG_TYPE_DH);
}

static void *dhx_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[])
{
   return dh_gen_init_base(provctx, selection, params, DH_FLAG_TYPE_DHX);
}

static int dh_gen_set_template(void *genctx, void *templ)
{
    struct dh_gen_ctx *gctx = genctx;
    DH *dh = templ;

    if (!ossl_prov_is_running() || gctx == NULL || dh == NULL)
        return 0;
    gctx->ffc_params = ossl_dh_get0_params(dh);
    return 1;
}

static int dh_set_gen_seed(struct dh_gen_ctx *gctx, unsigned char *seed,
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

static int dh_gen_common_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct dh_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;
    int gen_type = -1;

    if (gctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_TYPE);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || ((gen_type =
                 dh_gen_type_name2id_w_default(p->data, gctx->dh_type)) == -1)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
        if (gen_type != -1)
            gctx->gen_type = gen_type;
    }
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_GROUP_NAME);
    if (p != NULL) {
        const DH_NAMED_GROUP *group = NULL;

        if (p->data_type != OSSL_PARAM_UTF8_STRING
            || p->data == NULL
            || (group = ossl_ffc_name_to_dh_named_group(p->data)) == NULL
            || ((gctx->group_nid =
                 ossl_ffc_named_group_get_uid(group)) == NID_undef)) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
    }
    if ((p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PBITS)) != NULL
        && !OSSL_PARAM_get_size_t(p, &gctx->pbits))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_DH_PRIV_LEN);
    if (p != NULL && !OSSL_PARAM_get_int(p, &gctx->priv_len))
        return 0;
    return 1;
}

static const OSSL_PARAM *dh_gen_settable_params(ossl_unused void *genctx,
                                                ossl_unused void *provctx)
{
    static const OSSL_PARAM dh_gen_settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_TYPE, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_DH_PRIV_LEN, NULL),
        OSSL_PARAM_size_t(OSSL_PKEY_PARAM_FFC_PBITS, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_DH_GENERATOR, NULL),
        OSSL_PARAM_END
    };
    return dh_gen_settable;
}

static const OSSL_PARAM *dhx_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static const OSSL_PARAM dhx_gen_settable[] = {
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_TYPE, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_DH_PRIV_LEN, NULL),
        OSSL_PARAM_size_t(OSSL_PKEY_PARAM_FFC_PBITS, NULL),
        OSSL_PARAM_size_t(OSSL_PKEY_PARAM_FFC_QBITS, NULL),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_FFC_DIGEST_PROPS, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_GINDEX, NULL),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_FFC_SEED, NULL, 0),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_PCOUNTER, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_FFC_H, NULL),
        OSSL_PARAM_END
    };
    return dhx_gen_settable;
}

static int dhx_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct dh_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (!dh_gen_common_set_params(genctx, params))
        return 0;

    /* Parameters related to fips186-4 and fips186-2 */
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_GINDEX);
    if (p != NULL && !OSSL_PARAM_get_int(p, &gctx->gindex))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PCOUNTER);
    if (p != NULL && !OSSL_PARAM_get_int(p, &gctx->pcounter))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_H);
    if (p != NULL && !OSSL_PARAM_get_int(p, &gctx->hindex))
        return 0;
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_SEED);
    if (p != NULL
        && (p->data_type != OSSL_PARAM_OCTET_STRING
            || !dh_set_gen_seed(gctx, p->data, p->data_size)))
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

    /* Parameters that are not allowed for DHX */
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_DH_GENERATOR);
    if (p != NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_UNSUPPORTED);
        return 0;
    }
    return 1;
}

static int dh_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct dh_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (!dh_gen_common_set_params(genctx, params))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_DH_GENERATOR);
    if (p != NULL && !OSSL_PARAM_get_int(p, &gctx->generator))
        return 0;

    /* Parameters that are not allowed for DH */
    if (OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_GINDEX) != NULL
        || OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_PCOUNTER) != NULL
        || OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_H) != NULL
        || OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_SEED) != NULL
        || OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_QBITS) != NULL
        || OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_FFC_DIGEST) != NULL
        || OSSL_PARAM_locate_const(params,
                                   OSSL_PKEY_PARAM_FFC_DIGEST_PROPS) != NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }
    return 1;
}

static int dh_gencb(int p, int n, BN_GENCB *cb)
{
    struct dh_gen_ctx *gctx = BN_GENCB_get_arg(cb);
    OSSL_PARAM params[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };

    params[0] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_POTENTIAL, &p);
    params[1] = OSSL_PARAM_construct_int(OSSL_GEN_PARAM_ITERATION, &n);

    return gctx->cb(params, gctx->cbarg);
}

static void *dh_gen(void *genctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    int ret = 0;
    struct dh_gen_ctx *gctx = genctx;
    DH *dh = NULL;
    BN_GENCB *gencb = NULL;
    FFC_PARAMS *ffc;

    if (!ossl_prov_is_running() || gctx == NULL)
        return NULL;

    /*
     * If a group name is selected then the type is group regardless of what
     * the user selected. This overrides rather than errors for backwards
     * compatibility.
     */
    if (gctx->group_nid != NID_undef)
        gctx->gen_type = DH_PARAMGEN_TYPE_GROUP;

    /*
     * Do a bounds check on context gen_type. Must be in range:
     * DH_PARAMGEN_TYPE_GENERATOR <= gen_type <= DH_PARAMGEN_TYPE_GROUP
     * Noted here as this needs to be adjusted if a new group type is
     * added.
     */
    if (!ossl_assert((gctx->gen_type >= DH_PARAMGEN_TYPE_GENERATOR)
                    && (gctx->gen_type <= DH_PARAMGEN_TYPE_GROUP))) {
        ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                       "gen_type set to unsupported value %d", gctx->gen_type);
        return NULL;
    }

    /* For parameter generation - If there is a group name just create it */
    if (gctx->gen_type == DH_PARAMGEN_TYPE_GROUP
            && gctx->ffc_params == NULL) {
        /* Select a named group if there is not one already */
        if (gctx->group_nid == NID_undef)
            gctx->group_nid = ossl_dh_get_named_group_uid_from_size(gctx->pbits);
        if (gctx->group_nid == NID_undef)
            return NULL;
        dh = ossl_dh_new_by_nid_ex(gctx->libctx, gctx->group_nid);
        if (dh == NULL)
            return NULL;
        ffc = ossl_dh_get0_params(dh);
    } else {
        dh = ossl_dh_new_ex(gctx->libctx);
        if (dh == NULL)
            return NULL;
        ffc = ossl_dh_get0_params(dh);

        /* Copy the template value if one was passed */
        if (gctx->ffc_params != NULL
            && !ossl_ffc_params_copy(ffc, gctx->ffc_params))
            goto end;

        if (!ossl_ffc_params_set_seed(ffc, gctx->seed, gctx->seedlen))
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
        gctx->cb = osslcb;
        gctx->cbarg = cbarg;
        gencb = BN_GENCB_new();
        if (gencb != NULL)
            BN_GENCB_set(gencb, dh_gencb, genctx);

        if ((gctx->selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0) {
            /*
             * NOTE: The old safe prime generator code is not used in fips mode,
             * (i.e internally it ignores the generator and chooses a named
             * group based on pbits.
             */
            if (gctx->gen_type == DH_PARAMGEN_TYPE_GENERATOR)
                ret = DH_generate_parameters_ex(dh, gctx->pbits,
                                                gctx->generator, gencb);
            else
                ret = ossl_dh_generate_ffc_parameters(dh, gctx->gen_type,
                                                      gctx->pbits, gctx->qbits,
                                                      gencb);
            if (ret <= 0)
                goto end;
        }
    }

    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        if (ffc->p == NULL || ffc->g == NULL)
            goto end;
        if (gctx->priv_len > 0)
            DH_set_length(dh, (long)gctx->priv_len);
        ossl_ffc_params_enable_flags(ffc, FFC_PARAM_FLAG_VALIDATE_LEGACY,
                                     gctx->gen_type == DH_PARAMGEN_TYPE_FIPS_186_2);
        if (DH_generate_key(dh) <= 0)
            goto end;
    }
    DH_clear_flags(dh, DH_FLAG_TYPE_MASK);
    DH_set_flags(dh, gctx->dh_type);

    ret = 1;
end:
    if (ret <= 0) {
        DH_free(dh);
        dh = NULL;
    }
    BN_GENCB_free(gencb);
    return dh;
}

static void dh_gen_cleanup(void *genctx)
{
    struct dh_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_free(gctx->mdname);
    OPENSSL_free(gctx->mdprops);
    OPENSSL_clear_free(gctx->seed, gctx->seedlen);
    OPENSSL_free(gctx);
}

static void *dh_load(const void *reference, size_t reference_sz)
{
    DH *dh = NULL;

    if (ossl_prov_is_running() && reference_sz == sizeof(dh)) {
        /* The contents of the reference is the address to our object */
        dh = *(DH **)reference;
        /* We grabbed, so we detach it */
        *(DH **)reference = NULL;
        return dh;
    }
    return NULL;
}

static void *dh_dup(const void *keydata_from, int selection)
{
    if (ossl_prov_is_running())
        return ossl_dh_dup(keydata_from, selection);
    return NULL;
}

const OSSL_DISPATCH ossl_dh_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))dh_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))dh_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE, (void (*)(void))dh_gen_set_template },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))dh_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))dh_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))dh_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))dh_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))dh_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))dh_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))dh_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))dh_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))dh_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))dh_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))dh_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))dh_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))dh_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))dh_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))dh_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))dh_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))dh_export_types },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))dh_dup },
    OSSL_DISPATCH_END
};

/* For any DH key, we use the "DH" algorithms regardless of sub-type. */
static const char *dhx_query_operation_name(int operation_id)
{
    return "DH";
}

const OSSL_DISPATCH ossl_dhx_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))dhx_newdata },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))dhx_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_TEMPLATE, (void (*)(void))dh_gen_set_template },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))dhx_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))dhx_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))dh_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))dh_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_LOAD, (void (*)(void))dh_load },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))dh_freedata },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))dh_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))dh_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))dh_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))dh_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))dh_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))dh_match },
    { OSSL_FUNC_KEYMGMT_VALIDATE, (void (*)(void))dh_validate },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))dh_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))dh_import_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))dh_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))dh_export_types },
    { OSSL_FUNC_KEYMGMT_QUERY_OPERATION_NAME,
      (void (*)(void))dhx_query_operation_name },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))dh_dup },
    OSSL_DISPATCH_END
};
