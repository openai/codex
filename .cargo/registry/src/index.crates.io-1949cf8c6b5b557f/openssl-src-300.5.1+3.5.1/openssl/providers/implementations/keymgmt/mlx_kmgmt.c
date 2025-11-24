/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/param_build.h>
#include <openssl/params.h>
#include <openssl/proverr.h>
#include <openssl/rand.h>
#include <openssl/self_test.h>
#include "internal/nelem.h"
#include "internal/param_build_set.h"
#include "prov/implementations.h"
#include "prov/mlx_kem.h"
#include "prov/provider_ctx.h"
#include "prov/providercommon.h"
#include "prov/securitycheck.h"

static OSSL_FUNC_keymgmt_gen_fn mlx_kem_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn mlx_kem_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn mlx_kem_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn mlx_kem_gen_settable_params;
static OSSL_FUNC_keymgmt_get_params_fn mlx_kem_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn mlx_kem_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn mlx_kem_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn mlx_kem_settable_params;
static OSSL_FUNC_keymgmt_has_fn mlx_kem_has;
static OSSL_FUNC_keymgmt_match_fn mlx_kem_match;
static OSSL_FUNC_keymgmt_import_fn mlx_kem_import;
static OSSL_FUNC_keymgmt_export_fn mlx_kem_export;
static OSSL_FUNC_keymgmt_import_types_fn mlx_kem_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn mlx_kem_imexport_types;
static OSSL_FUNC_keymgmt_dup_fn mlx_kem_dup;

static const int minimal_selection = OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS
    | OSSL_KEYMGMT_SELECT_PRIVATE_KEY;

/* Must match DECLARE_DISPATCH invocations at the end of the file */
static const ECDH_VINFO hybrid_vtable[] = {
    { "EC",  "P-256", 65, 32, 32, 1, EVP_PKEY_ML_KEM_768 },
    { "EC",  "P-384", 97, 48, 48, 1, EVP_PKEY_ML_KEM_1024 },
#if !defined(OPENSSL_NO_ECX)
    { "X25519", NULL, 32, 32, 32, 0, EVP_PKEY_ML_KEM_768 },
    { "X448",   NULL, 56, 56, 56, 0, EVP_PKEY_ML_KEM_1024 },
#endif
};

typedef struct mlx_kem_gen_ctx_st {
    OSSL_LIB_CTX *libctx;
    char *propq;
    int selection;
    unsigned int evp_type;
} PROV_ML_KEM_GEN_CTX;

static void mlx_kem_key_free(void *vkey)
{
    MLX_KEY *key = vkey;

    if (key == NULL)
        return;
    OPENSSL_free(key->propq);
    EVP_PKEY_free(key->mkey);
    EVP_PKEY_free(key->xkey);
    OPENSSL_free(key);
}

/* Takes ownership of propq */
static void *
mlx_kem_key_new(unsigned int v, OSSL_LIB_CTX *libctx, char *propq)
{
    MLX_KEY *key = NULL;
    unsigned int ml_kem_variant;

    if (!ossl_prov_is_running()
        || v >= OSSL_NELEM(hybrid_vtable)
        || (key = OPENSSL_malloc(sizeof(*key))) == NULL)
        goto err;

    ml_kem_variant = hybrid_vtable[v].ml_kem_variant;
    key->libctx = libctx;
    key->minfo = ossl_ml_kem_get_vinfo(ml_kem_variant);
    key->xinfo = &hybrid_vtable[v];
    key->xkey = key->mkey = NULL;
    key->state = MLX_HAVE_NOKEYS;
    key->propq = propq;
    return key;

 err:
    OPENSSL_free(propq);
    return NULL;
}


static int mlx_kem_has(const void *vkey, int selection)
{
    const MLX_KEY *key = vkey;

    /* A NULL key MUST fail to have anything */
    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    switch (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) {
    case 0:
        return 1;
    case OSSL_KEYMGMT_SELECT_PUBLIC_KEY:
        return mlx_kem_have_pubkey(key);
    default:
        return mlx_kem_have_prvkey(key);
    }
}

static int mlx_kem_match(const void *vkey1, const void *vkey2, int selection)
{
    const MLX_KEY *key1 = vkey1;
    const MLX_KEY *key2 = vkey2;
    int have_pub1 = mlx_kem_have_pubkey(key1);
    int have_pub2 = mlx_kem_have_pubkey(key2);

    if (!ossl_prov_is_running())
        return 0;

    /* Compare domain parameters */
    if (key1->xinfo != key2->xinfo)
        return 0;

    if (!(selection & OSSL_KEYMGMT_SELECT_KEYPAIR))
        return 1;

    if (have_pub1 ^ have_pub2)
        return 0;

    /* As in other providers, equal when both have no key material. */
    if (!have_pub1)
        return 1;

    return EVP_PKEY_eq(key1->mkey, key2->mkey)
        && EVP_PKEY_eq(key1->xkey, key2->xkey);
}

typedef struct export_cb_arg_st {
    const char *algorithm_name;
    uint8_t *pubenc;
    uint8_t *prvenc;
    int      pubcount;
    int      prvcount;
    size_t   puboff;
    size_t   prvoff;
    size_t   publen;
    size_t   prvlen;
} EXPORT_CB_ARG;

/* Copy any exported key material into its storage slot */
static int export_sub_cb(const OSSL_PARAM *params, void *varg)
{
    EXPORT_CB_ARG *sub_arg = varg;
    const OSSL_PARAM *p = NULL;
    size_t len;

    /*
     * The caller will decide whether anything essential is missing, but, if
     * some key material was returned, it should have the right (parameter)
     * data type and length.
     */
    if (ossl_param_is_empty(params))
        return 1;
    if (sub_arg->pubenc != NULL
        && (p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY)) != NULL) {
        void *pub = sub_arg->pubenc + sub_arg->puboff;

        if (OSSL_PARAM_get_octet_string(p, &pub, sub_arg->publen, &len) != 1)
            return 0;
        if (len != sub_arg->publen) {
            ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                           "Unexpected %s public key length %lu != %lu",
                           sub_arg->algorithm_name, (unsigned long) len,
                           sub_arg->publen);
            return 0;
        }
        ++sub_arg->pubcount;
    }
    if (sub_arg->prvenc != NULL
        && (p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY)) != NULL) {
        void *prv = sub_arg->prvenc + sub_arg->prvoff;

        if (OSSL_PARAM_get_octet_string(p, &prv, sub_arg->prvlen, &len) != 1)
            return 0;
        if (len != sub_arg->prvlen) {
            ERR_raise_data(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR,
                           "Unexpected %s private key length %lu != %lu",
                           sub_arg->algorithm_name, (unsigned long) len,
                           (unsigned long) sub_arg->publen);
            return 0;
        }
        ++sub_arg->prvcount;
    }
    return 1;
}

static int
export_sub(EXPORT_CB_ARG *sub_arg, int selection, MLX_KEY *key)
{
    int slot;

    /*
     * The caller is responsible for initialising only the pubenc and prvenc
     * pointer fields, the rest are set here or in the callback.
     */
    sub_arg->pubcount = 0;
    sub_arg->prvcount = 0;

    for (slot = 0; slot < 2; ++slot) {
        int ml_kem_slot = key->xinfo->ml_kem_slot;
        EVP_PKEY *pkey;

        /* Export the parts of each component into its storage slot */
        if (slot == ml_kem_slot) {
            pkey = key->mkey;
            sub_arg->algorithm_name = key->minfo->algorithm_name;
            sub_arg->puboff = slot * key->xinfo->pubkey_bytes;
            sub_arg->prvoff = slot * key->xinfo->prvkey_bytes;
            sub_arg->publen = key->minfo->pubkey_bytes;
            sub_arg->prvlen = key->minfo->prvkey_bytes;
        } else {
            pkey = key->xkey;
            sub_arg->algorithm_name = key->xinfo->algorithm_name;
            sub_arg->puboff = (1 - ml_kem_slot) * key->minfo->pubkey_bytes;
            sub_arg->prvoff = (1 - ml_kem_slot) * key->minfo->prvkey_bytes;
            sub_arg->publen = key->xinfo->pubkey_bytes;
            sub_arg->prvlen = key->xinfo->prvkey_bytes;
        }
        if (!EVP_PKEY_export(pkey, selection, export_sub_cb, (void *)sub_arg))
            return 0;
    }
    return 1;
}

static int mlx_kem_export(void *vkey, int selection, OSSL_CALLBACK *param_cb,
                          void *cbarg)
{
    MLX_KEY *key = vkey;
    OSSL_PARAM_BLD *tmpl = NULL;
    OSSL_PARAM *params = NULL;
    size_t publen;
    size_t prvlen;
    int ret = 0;
    EXPORT_CB_ARG sub_arg;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    /* Fail when no key material has yet been provided */
    if (!mlx_kem_have_pubkey(key)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }
    publen = key->minfo->pubkey_bytes + key->xinfo->pubkey_bytes;
    prvlen = key->minfo->prvkey_bytes + key->xinfo->prvkey_bytes;
    memset(&sub_arg, 0, sizeof(sub_arg));

    if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
        sub_arg.pubenc = OPENSSL_malloc(publen);
        if (sub_arg.pubenc == NULL)
            goto err;
    }

    if (mlx_kem_have_prvkey(key)
        && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        /*
         * Allocated on the secure heap if configured, this is detected in
         * ossl_param_build_set_octet_string(), which will then also use the
         * secure heap.
         */
        sub_arg.prvenc = OPENSSL_secure_zalloc(prvlen);
        if (sub_arg.prvenc == NULL)
            goto err;
    }

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        goto err;

    /* Extract sub-component key material */
    if (!export_sub(&sub_arg, selection, key))
        goto err;

    if (sub_arg.pubenc != NULL && sub_arg.pubcount == 2
        && !ossl_param_build_set_octet_string(
                tmpl, NULL, OSSL_PKEY_PARAM_PUB_KEY, sub_arg.pubenc, publen))
        goto err;

    if (sub_arg.prvenc != NULL && sub_arg.prvcount == 2
        && !ossl_param_build_set_octet_string(
                tmpl, NULL, OSSL_PKEY_PARAM_PRIV_KEY, sub_arg.prvenc, prvlen))
        goto err;

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);

 err:
    OSSL_PARAM_BLD_free(tmpl);
    OPENSSL_secure_clear_free(sub_arg.prvenc, prvlen);
    OPENSSL_free(sub_arg.pubenc);
    return ret;
}

static const OSSL_PARAM *mlx_kem_imexport_types(int selection)
{
    static const OSSL_PARAM key_types[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return key_types;
    return NULL;
}

static int
load_slot(OSSL_LIB_CTX *libctx, const char *propq, const char *pname,
          int selection, MLX_KEY *key, int slot, const uint8_t *in,
          int mbytes, int xbytes)
{
    EVP_PKEY_CTX *ctx;
    EVP_PKEY **ppkey;
    OSSL_PARAM parr[] = { OSSL_PARAM_END, OSSL_PARAM_END, OSSL_PARAM_END };
    const char *alg;
    char *group = NULL;
    size_t off, len;
    void *val;
    int ml_kem_slot = key->xinfo->ml_kem_slot;
    int ret = 0;

    if (slot == ml_kem_slot) {
        alg = key->minfo->algorithm_name;
        ppkey = &key->mkey;
        off = slot * xbytes;
        len = mbytes;
    } else {
        alg = key->xinfo->algorithm_name;
        group = (char *) key->xinfo->group_name;
        ppkey = &key->xkey;
        off = (1 - ml_kem_slot) * mbytes;
        len = xbytes;
    }
    val = (void *)(in + off);

    if ((ctx = EVP_PKEY_CTX_new_from_name(libctx, alg, propq)) == NULL
        || EVP_PKEY_fromdata_init(ctx) <= 0)
        goto err;
    parr[0] = OSSL_PARAM_construct_octet_string(pname, val, len);
    if (group != NULL)
        parr[1] = OSSL_PARAM_construct_utf8_string(OSSL_PKEY_PARAM_GROUP_NAME,
                                                   group, 0);
    if (EVP_PKEY_fromdata(ctx, ppkey, selection, parr) > 0)
        ret = 1;

 err:
    EVP_PKEY_CTX_free(ctx);
    return ret;
}

static int
load_keys(MLX_KEY *key,
          const uint8_t *pubenc, size_t publen,
          const uint8_t *prvenc, size_t prvlen)
{
    int slot;

    for (slot = 0; slot < 2; ++slot) {
        if (prvlen) {
            /* Ignore public keys when private provided */
            if (!load_slot(key->libctx, key->propq, OSSL_PKEY_PARAM_PRIV_KEY,
                           minimal_selection, key, slot, prvenc,
                           key->minfo->prvkey_bytes, key->xinfo->prvkey_bytes))
                goto err;
        } else if (publen) {
            /* Absent private key data, import public keys */
            if (!load_slot(key->libctx, key->propq, OSSL_PKEY_PARAM_PUB_KEY,
                           minimal_selection, key, slot, pubenc,
                           key->minfo->pubkey_bytes, key->xinfo->pubkey_bytes))
                goto err;
        }
    }
    key->state = prvlen ? MLX_HAVE_PRVKEY : MLX_HAVE_PUBKEY;
    return 1;

 err:
    EVP_PKEY_free(key->mkey);
    EVP_PKEY_free(key->xkey);
    key->xkey = key->mkey = NULL;
    key->state = MLX_HAVE_NOKEYS;
    return 0;
}

static int mlx_kem_key_fromdata(MLX_KEY *key,
                               const OSSL_PARAM params[],
                               int include_private)
{
    const OSSL_PARAM *param_prv_key = NULL, *param_pub_key;
    const void *pubenc = NULL, *prvenc = NULL;
    size_t pubkey_bytes, prvkey_bytes;
    size_t publen = 0, prvlen = 0;

    /* Invalid attempt to mutate a key, what is the right error to report? */
    if (key == NULL || mlx_kem_have_pubkey(key))
        return 0;
    pubkey_bytes = key->minfo->pubkey_bytes + key->xinfo->pubkey_bytes;
    prvkey_bytes = key->minfo->prvkey_bytes + key->xinfo->prvkey_bytes;

    /* What does the caller want to set? */
    param_pub_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (param_pub_key != NULL &&
        OSSL_PARAM_get_octet_string_ptr(param_pub_key, &pubenc, &publen) != 1)
        return 0;
    if (include_private)
        param_prv_key = OSSL_PARAM_locate_const(params,
                                                OSSL_PKEY_PARAM_PRIV_KEY);
    if (param_prv_key != NULL &&
        OSSL_PARAM_get_octet_string_ptr(param_prv_key, &prvenc, &prvlen) != 1)
        return 0;

    /* The caller MUST specify at least one of the public or private keys. */
    if (publen == 0 && prvlen == 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    /*
     * When a pubkey is provided, its length MUST be correct, if a private key
     * is also provided, the public key will be otherwise ignored.  We could
     * look for a matching encoded block, but unclear this is useful.
     */
    if (publen != 0 && publen != pubkey_bytes) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }
    if (prvlen != 0 && prvlen != prvkey_bytes) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }

    return load_keys(key, pubenc, publen, prvenc, prvlen);
}

static int mlx_kem_import(void *vkey, int selection, const OSSL_PARAM params[])
{
    MLX_KEY *key = vkey;
    int include_private;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_private = selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;
    return mlx_kem_key_fromdata(key, params, include_private);
}

static const OSSL_PARAM *mlx_kem_gettable_params(void *provctx)
{
    static const OSSL_PARAM arr[] = {
        OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
        OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    return arr;
}

/*
 * It is assumed the key is guaranteed non-NULL here, and is from this provider
 */
static int mlx_kem_get_params(void *vkey, OSSL_PARAM params[])
{
    MLX_KEY *key = vkey;
    OSSL_PARAM *p, *pub, *prv = NULL;
    EXPORT_CB_ARG sub_arg;
    int selection;
    size_t publen = key->minfo->pubkey_bytes + key->xinfo->pubkey_bytes;
    size_t prvlen = key->minfo->prvkey_bytes + key->xinfo->prvkey_bytes;

    /* The reported "bit" count is those of the ML-KEM key */
    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, key->minfo->bits))
            return 0;

    /* The reported security bits are those of the ML-KEM key */
    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, key->minfo->secbits))
            return 0;

    /* The ciphertext sizes are additive */
    p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE);
    if (p != NULL)
        if (!OSSL_PARAM_set_int(p, key->minfo->ctext_bytes + key->xinfo->pubkey_bytes))
            return 0;

    if (!mlx_kem_have_pubkey(key))
        return 1;

    memset(&sub_arg, 0, sizeof(sub_arg));
    pub = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (pub != NULL) {
        if (pub->data_type != OSSL_PARAM_OCTET_STRING)
            return 0;
        pub->return_size = publen;
        if (pub->data == NULL) {
            pub = NULL;
        } else if (pub->data_size < publen) {
            ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                           "public key output buffer too short: %lu < %lu",
                           (unsigned long) pub->data_size,
                           (unsigned long) publen);
            return 0;
        } else {
            sub_arg.pubenc = pub->data;
        }
    }
    if (mlx_kem_have_prvkey(key)) {
        prv = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_PRIV_KEY);
        if (prv != NULL) {
            if (prv->data_type != OSSL_PARAM_OCTET_STRING)
                return 0;
            prv->return_size = prvlen;
            if (prv->data == NULL) {
                prv = NULL;
            } else if (prv->data_size < prvlen) {
                ERR_raise_data(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL,
                               "private key output buffer too short: %lu < %lu",
                               (unsigned long) prv->data_size,
                               (unsigned long) prvlen);
                return 0;
            } else {
                sub_arg.prvenc = prv->data;
            }
        }
    }
    if (pub == NULL && prv == NULL)
        return 1;

    selection = prv == NULL ? 0 : OSSL_KEYMGMT_SELECT_PRIVATE_KEY;
    selection |= pub == NULL ? 0 : OSSL_KEYMGMT_SELECT_PUBLIC_KEY;
    if (key->xinfo->group_name != NULL)
        selection |= OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS;

    /* Extract sub-component key material */
    if (!export_sub(&sub_arg, selection, key))
        return 0;

    if ((pub != NULL && sub_arg.pubcount != 2)
        || (prv != NULL && sub_arg.prvcount != 2))
        return 0;

    return 1;
}

static const OSSL_PARAM *mlx_kem_settable_params(void *provctx)
{
    static const OSSL_PARAM arr[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
        OSSL_PARAM_END
    };

    return arr;
}

static int mlx_kem_set_params(void *vkey, const OSSL_PARAM params[])
{
    MLX_KEY *key = vkey;
    const OSSL_PARAM *p;
    const void *pubenc = NULL;
    size_t publen = 0;

    if (ossl_param_is_empty(params))
        return 1;

    /* Only one settable parameter is supported */
    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p == NULL)
        return 1;

    /* Key mutation is reportedly generally not allowed */
    if (mlx_kem_have_pubkey(key)) {
        ERR_raise_data(ERR_LIB_PROV,
                       PROV_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE,
                       "keys cannot be mutated");
        return 0;
    }
    /* An unlikely failure mode is the parameter having some unexpected type */
    if (!OSSL_PARAM_get_octet_string_ptr(p, &pubenc, &publen))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        OPENSSL_free(key->propq);
        key->propq = NULL;
        if (!OSSL_PARAM_get_utf8_string(p, &key->propq, 0))
            return 0;
    }

    if (publen != key->minfo->pubkey_bytes + key->xinfo->pubkey_bytes) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        return 0;
    }

    return load_keys(key, pubenc, publen, NULL, 0);
}

static int mlx_kem_gen_set_params(void *vgctx, const OSSL_PARAM params[])
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        OPENSSL_free(gctx->propq);
        if ((gctx->propq = OPENSSL_strdup(p->data)) == NULL)
            return 0;
    }
    return 1;
}

static void *mlx_kem_gen_init(int evp_type, OSSL_LIB_CTX *libctx,
                              int selection, const OSSL_PARAM params[])
{
    PROV_ML_KEM_GEN_CTX *gctx = NULL;

    /*
     * We can only generate private keys, check that the selection is
     * appropriate.
     */
    if (!ossl_prov_is_running()
        || (selection & minimal_selection) == 0
        || (gctx = OPENSSL_zalloc(sizeof(*gctx))) == NULL)
        return NULL;

    gctx->evp_type = evp_type;
    gctx->libctx = libctx;
    gctx->selection = selection;
    if (mlx_kem_gen_set_params(gctx, params))
        return gctx;

    mlx_kem_gen_cleanup(gctx);
    return NULL;
}

static const OSSL_PARAM *mlx_kem_gen_settable_params(ossl_unused void *vgctx,
                                                     ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
        OSSL_PARAM_END
    };

    return settable;
}

static void *mlx_kem_gen(void *vgctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;
    MLX_KEY *key;
    char *propq;

    if (gctx == NULL
        || (gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) ==
            OSSL_KEYMGMT_SELECT_PUBLIC_KEY)
        return NULL;

    /* Lose ownership of propq */
    propq = gctx->propq;
    gctx->propq = NULL;
    if ((key = mlx_kem_key_new(gctx->evp_type, gctx->libctx, propq)) == NULL)
        return NULL;

    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    /* For now, using the same "propq" for all components */
    key->mkey = EVP_PKEY_Q_keygen(key->libctx, key->propq,
                                  key->minfo->algorithm_name);
    key->xkey = EVP_PKEY_Q_keygen(key->libctx, key->propq,
                                  key->xinfo->algorithm_name,
                                  key->xinfo->group_name);
    if (key->mkey != NULL && key->xkey != NULL) {
        key->state = MLX_HAVE_PRVKEY;
        return key;
    }

    mlx_kem_key_free(key);
    return NULL;
}

static void mlx_kem_gen_cleanup(void *vgctx)
{
    PROV_ML_KEM_GEN_CTX *gctx = vgctx;

    if (gctx == NULL)
        return;
    OPENSSL_free(gctx->propq);
    OPENSSL_free(gctx);
}

static void *mlx_kem_dup(const void *vkey, int selection)
{
    const MLX_KEY *key = vkey;
    MLX_KEY *ret;

    if (!ossl_prov_is_running()
        || (ret = OPENSSL_memdup(key, sizeof(*ret))) == NULL)
        return NULL;

    if (ret->propq != NULL
        && (ret->propq = OPENSSL_strdup(ret->propq)) == NULL) {
        OPENSSL_free(ret);
        return NULL;
    }

    /* Absent key material, nothing left to do */
    if (ret->mkey == NULL) {
        if (ret->xkey == NULL)
            return ret;
        /* Fail if the source key is an inconsistent state */
        OPENSSL_free(ret);
        return NULL;
    }

    switch (selection & OSSL_KEYMGMT_SELECT_KEYPAIR) {
    case 0:
        ret->xkey = ret->mkey = NULL;
        return ret;
    case OSSL_KEYMGMT_SELECT_KEYPAIR:
        ret->mkey = EVP_PKEY_dup(key->mkey);
        ret->xkey = EVP_PKEY_dup(key->xkey);
        if (ret->xkey != NULL && ret->mkey != NULL)
            return ret;
        break;
    default:
        ERR_raise_data(ERR_LIB_PROV, PROV_R_UNSUPPORTED_SELECTION,
                       "duplication of partial key material not supported");
        break;
    }

    mlx_kem_key_free(ret);
    return NULL;
}

#define DECLARE_DISPATCH(name, variant) \
    static OSSL_FUNC_keymgmt_new_fn mlx_##name##_kem_new; \
    static void *mlx_##name##_kem_new(void *provctx) \
    { \
        OSSL_LIB_CTX *libctx; \
                              \
        libctx = provctx == NULL ? NULL : PROV_LIBCTX_OF(provctx); \
        return mlx_kem_key_new(variant, libctx, NULL); \
    } \
    static OSSL_FUNC_keymgmt_gen_init_fn mlx_##name##_kem_gen_init; \
    static void *mlx_##name##_kem_gen_init(void *provctx, int selection, \
                                           const OSSL_PARAM params[]) \
    { \
        OSSL_LIB_CTX *libctx; \
                              \
        libctx = provctx == NULL ? NULL : PROV_LIBCTX_OF(provctx); \
        return mlx_kem_gen_init(variant, libctx, selection, params); \
    } \
    const OSSL_DISPATCH ossl_mlx_##name##_kem_kmgmt_functions[] = { \
        { OSSL_FUNC_KEYMGMT_NEW, (OSSL_FUNC) mlx_##name##_kem_new }, \
        { OSSL_FUNC_KEYMGMT_FREE, (OSSL_FUNC) mlx_kem_key_free }, \
        { OSSL_FUNC_KEYMGMT_GET_PARAMS, (OSSL_FUNC) mlx_kem_get_params }, \
        { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (OSSL_FUNC) mlx_kem_gettable_params }, \
        { OSSL_FUNC_KEYMGMT_SET_PARAMS, (OSSL_FUNC) mlx_kem_set_params }, \
        { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (OSSL_FUNC) mlx_kem_settable_params }, \
        { OSSL_FUNC_KEYMGMT_HAS, (OSSL_FUNC) mlx_kem_has }, \
        { OSSL_FUNC_KEYMGMT_MATCH, (OSSL_FUNC) mlx_kem_match }, \
        { OSSL_FUNC_KEYMGMT_GEN_INIT, (OSSL_FUNC) mlx_##name##_kem_gen_init }, \
        { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (OSSL_FUNC) mlx_kem_gen_set_params }, \
        { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS, (OSSL_FUNC) mlx_kem_gen_settable_params }, \
        { OSSL_FUNC_KEYMGMT_GEN, (OSSL_FUNC) mlx_kem_gen }, \
        { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (OSSL_FUNC) mlx_kem_gen_cleanup }, \
        { OSSL_FUNC_KEYMGMT_DUP, (OSSL_FUNC) mlx_kem_dup }, \
        { OSSL_FUNC_KEYMGMT_IMPORT, (OSSL_FUNC) mlx_kem_import }, \
        { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (OSSL_FUNC) mlx_kem_imexport_types }, \
        { OSSL_FUNC_KEYMGMT_EXPORT, (OSSL_FUNC) mlx_kem_export }, \
        { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (OSSL_FUNC) mlx_kem_imexport_types }, \
        OSSL_DISPATCH_END \
    }
/* See |hybrid_vtable| above */
DECLARE_DISPATCH(p256, 0);
DECLARE_DISPATCH(p384, 1);
#if !defined(OPENSSL_NO_ECX)
DECLARE_DISPATCH(x25519, 2);
DECLARE_DISPATCH(x448, 3);
#endif
