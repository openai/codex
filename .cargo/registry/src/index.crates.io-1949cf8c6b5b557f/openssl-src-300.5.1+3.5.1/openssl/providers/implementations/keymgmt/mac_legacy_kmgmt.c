/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <string.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/evp.h>
#include <openssl/proverr.h>
#include <openssl/param_build.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
#endif
#include "internal/param_build_set.h"
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/macsignature.h"

static OSSL_FUNC_keymgmt_new_fn mac_new;
static OSSL_FUNC_keymgmt_free_fn mac_free;
static OSSL_FUNC_keymgmt_gen_init_fn mac_gen_init;
static OSSL_FUNC_keymgmt_gen_fn mac_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn mac_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn mac_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn mac_gen_settable_params;
static OSSL_FUNC_keymgmt_get_params_fn mac_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn mac_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn mac_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn mac_settable_params;
static OSSL_FUNC_keymgmt_has_fn mac_has;
static OSSL_FUNC_keymgmt_match_fn mac_match;
static OSSL_FUNC_keymgmt_import_fn mac_import;
static OSSL_FUNC_keymgmt_import_types_fn mac_imexport_types;
static OSSL_FUNC_keymgmt_export_fn mac_export;
static OSSL_FUNC_keymgmt_export_types_fn mac_imexport_types;

static OSSL_FUNC_keymgmt_new_fn mac_new_cmac;
static OSSL_FUNC_keymgmt_gettable_params_fn cmac_gettable_params;
static OSSL_FUNC_keymgmt_import_types_fn cmac_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn cmac_imexport_types;
static OSSL_FUNC_keymgmt_gen_init_fn cmac_gen_init;
static OSSL_FUNC_keymgmt_gen_set_params_fn cmac_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn cmac_gen_settable_params;

struct mac_gen_ctx {
    OSSL_LIB_CTX *libctx;
    int selection;
    unsigned char *priv_key;
    size_t priv_key_len;
    PROV_CIPHER cipher;
};

MAC_KEY *ossl_mac_key_new(OSSL_LIB_CTX *libctx, int cmac)
{
    MAC_KEY *mackey;

    if (!ossl_prov_is_running())
        return NULL;

    mackey = OPENSSL_zalloc(sizeof(*mackey));
    if (mackey == NULL)
        return NULL;

    if (!CRYPTO_NEW_REF(&mackey->refcnt, 1)) {
        OPENSSL_free(mackey);
        return NULL;
    }
    mackey->libctx = libctx;
    mackey->cmac = cmac;

    return mackey;
}

void ossl_mac_key_free(MAC_KEY *mackey)
{
    int ref = 0;

    if (mackey == NULL)
        return;

    CRYPTO_DOWN_REF(&mackey->refcnt, &ref);
    if (ref > 0)
        return;

    OPENSSL_secure_clear_free(mackey->priv_key, mackey->priv_key_len);
    OPENSSL_free(mackey->properties);
    ossl_prov_cipher_reset(&mackey->cipher);
    CRYPTO_FREE_REF(&mackey->refcnt);
    OPENSSL_free(mackey);
}

int ossl_mac_key_up_ref(MAC_KEY *mackey)
{
    int ref = 0;

    /* This is effectively doing a new operation on the MAC_KEY and should be
     * adequately guarded again modules' error states.  However, both current
     * calls here are guarded properly in signature/mac_legacy.c.  Thus, it
     * could be removed here.  The concern is that something in the future
     * might call this function without adequate guards.  It's a cheap call,
     * it seems best to leave it even though it is currently redundant.
     */
    if (!ossl_prov_is_running())
        return 0;

    CRYPTO_UP_REF(&mackey->refcnt, &ref);
    return 1;
}

static void *mac_new(void *provctx)
{
    return ossl_mac_key_new(PROV_LIBCTX_OF(provctx), 0);
}

static void *mac_new_cmac(void *provctx)
{
    return ossl_mac_key_new(PROV_LIBCTX_OF(provctx), 1);
}

static void mac_free(void *mackey)
{
    ossl_mac_key_free(mackey);
}

static int mac_has(const void *keydata, int selection)
{
    const MAC_KEY *key = keydata;
    int ok = 0;

    if (ossl_prov_is_running() && key != NULL) {
        /*
         * MAC keys always have all the parameters they need (i.e. none).
         * Therefore we always return with 1, if asked about parameters.
         * Similarly for public keys.
         */
        ok = 1;

        if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
            ok = key->priv_key != NULL;
    }
    return ok;
}

static int mac_match(const void *keydata1, const void *keydata2, int selection)
{
    const MAC_KEY *key1 = keydata1;
    const MAC_KEY *key2 = keydata2;
    int ok = 1;

    if (!ossl_prov_is_running())
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
        if ((key1->priv_key == NULL && key2->priv_key != NULL)
                || (key1->priv_key != NULL && key2->priv_key == NULL)
                || key1->priv_key_len != key2->priv_key_len
                || (key1->cipher.cipher == NULL && key2->cipher.cipher != NULL)
                || (key1->cipher.cipher != NULL && key2->cipher.cipher == NULL))
            ok = 0;
        else
            ok = ok && (key1->priv_key == NULL /* implies key2->privkey == NULL */
                        || CRYPTO_memcmp(key1->priv_key, key2->priv_key,
                                         key1->priv_key_len) == 0);
        if (key1->cipher.cipher != NULL)
            ok = ok && EVP_CIPHER_is_a(key1->cipher.cipher,
                                       EVP_CIPHER_get0_name(key2->cipher.cipher));
    }
    return ok;
}

static int mac_key_fromdata(MAC_KEY *key, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
        OPENSSL_secure_clear_free(key->priv_key, key->priv_key_len);
        /* allocate at least one byte to distinguish empty key from no key set */
        key->priv_key = OPENSSL_secure_malloc(p->data_size > 0 ? p->data_size : 1);
        if (key->priv_key == NULL)
            return 0;
        memcpy(key->priv_key, p->data, p->data_size);
        key->priv_key_len = p->data_size;
    }

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
        OPENSSL_free(key->properties);
        key->properties = OPENSSL_strdup(p->data);
        if (key->properties == NULL)
            return 0;
    }

    if (key->cmac && !ossl_prov_cipher_load_from_params(&key->cipher, params,
                                                        key->libctx)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (key->priv_key != NULL)
        return 1;

    return 0;
}

static int mac_import(void *keydata, int selection, const OSSL_PARAM params[])
{
    MAC_KEY *key = keydata;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) == 0)
        return 0;

    return mac_key_fromdata(key, params);
}

static int key_to_params(MAC_KEY *key, OSSL_PARAM_BLD *tmpl,
                         OSSL_PARAM params[])
{
    if (key == NULL)
        return 0;

    if (key->priv_key != NULL
        && !ossl_param_build_set_octet_string(tmpl, params,
                                              OSSL_PKEY_PARAM_PRIV_KEY,
                                              key->priv_key, key->priv_key_len))
        return 0;

    if (key->cipher.cipher != NULL
        && !ossl_param_build_set_utf8_string(tmpl, params,
                                             OSSL_PKEY_PARAM_CIPHER,
                                             EVP_CIPHER_get0_name(key->cipher.cipher)))
        return 0;

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    if (key->cipher.engine != NULL
        && !ossl_param_build_set_utf8_string(tmpl, params,
                                             OSSL_PKEY_PARAM_ENGINE,
                                             ENGINE_get_id(key->cipher.engine)))
        return 0;
#endif

    return 1;
}

static int mac_export(void *keydata, int selection, OSSL_CALLBACK *param_cb,
                      void *cbarg)
{
    MAC_KEY *key = keydata;
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ret = 0;

    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0
         && !key_to_params(key, tmpl, NULL))
        goto err;

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    return ret;
}

static const OSSL_PARAM mac_key_types[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *mac_imexport_types(int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        return mac_key_types;
    return NULL;
}

static const OSSL_PARAM cmac_key_types[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_CIPHER, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_ENGINE, NULL, 0),
    OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_PROPERTIES, NULL, 0),
    OSSL_PARAM_END
};
static const OSSL_PARAM *cmac_imexport_types(int selection)
{
    if ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0)
        return cmac_key_types;
    return NULL;
}

static int mac_get_params(void *key, OSSL_PARAM params[])
{
    return key_to_params(key, NULL, params);
}

static const OSSL_PARAM *mac_gettable_params(void *provctx)
{
    static const OSSL_PARAM gettable_params[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_END
    };
    return gettable_params;
}

static const OSSL_PARAM *cmac_gettable_params(void *provctx)
{
    static const OSSL_PARAM gettable_params[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_ENGINE, NULL, 0),
        OSSL_PARAM_END
    };
    return gettable_params;
}

static int mac_set_params(void *keydata, const OSSL_PARAM params[])
{
    MAC_KEY *key = keydata;
    const OSSL_PARAM *p;

    if (key == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    if (p != NULL)
        return mac_key_fromdata(key, params);

    return 1;
}

static const OSSL_PARAM *mac_settable_params(void *provctx)
{
    static const OSSL_PARAM settable_params[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_END
    };
    return settable_params;
}

static void *mac_gen_init_common(void *provctx, int selection)
{
    OSSL_LIB_CTX *libctx = PROV_LIBCTX_OF(provctx);
    struct mac_gen_ctx *gctx = NULL;

    if (!ossl_prov_is_running())
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->libctx = libctx;
        gctx->selection = selection;
    }
    return gctx;
}

static void *mac_gen_init(void *provctx, int selection,
                          const OSSL_PARAM params[])
{
    struct mac_gen_ctx *gctx = mac_gen_init_common(provctx, selection);

    if (gctx != NULL && !mac_gen_set_params(gctx, params)) {
        mac_gen_cleanup(gctx);
        gctx = NULL;
    }
    return gctx;
}

static void *cmac_gen_init(void *provctx, int selection,
                           const OSSL_PARAM params[])
{
    struct mac_gen_ctx *gctx = mac_gen_init_common(provctx, selection);

    if (gctx != NULL && !cmac_gen_set_params(gctx, params)) {
        mac_gen_cleanup(gctx);
        gctx = NULL;
    }
    return gctx;
}

static int mac_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct mac_gen_ctx *gctx = genctx;
    const OSSL_PARAM *p;

    if (gctx == NULL)
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_OCTET_STRING) {
            ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
            return 0;
        }
        gctx->priv_key = OPENSSL_secure_malloc(p->data_size);
        if (gctx->priv_key == NULL)
            return 0;
        memcpy(gctx->priv_key, p->data, p->data_size);
        gctx->priv_key_len = p->data_size;
    }

    return 1;
}

static int cmac_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct mac_gen_ctx *gctx = genctx;

    if (!mac_gen_set_params(genctx, params))
        return 0;

    if (!ossl_prov_cipher_load_from_params(&gctx->cipher, params,
                                           gctx->libctx)) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    return 1;
}

static const OSSL_PARAM *mac_gen_settable_params(ossl_unused void *genctx,
                                                 ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static const OSSL_PARAM *cmac_gen_settable_params(ossl_unused void *genctx,
                                                  ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0),
        OSSL_PARAM_utf8_string(OSSL_PKEY_PARAM_CIPHER, NULL, 0),
        OSSL_PARAM_END
    };
    return settable;
}

static void *mac_gen(void *genctx, OSSL_CALLBACK *cb, void *cbarg)
{
    struct mac_gen_ctx *gctx = genctx;
    MAC_KEY *key;

    if (!ossl_prov_is_running() || gctx == NULL)
        return NULL;

    if ((key = ossl_mac_key_new(gctx->libctx, 0)) == NULL) {
        ERR_raise(ERR_LIB_PROV, ERR_R_PROV_LIB);
        return NULL;
    }

    /* If we're doing parameter generation then we just return a blank key */
    if ((gctx->selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return key;

    if (gctx->priv_key == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY);
        ossl_mac_key_free(key);
        return NULL;
    }

    /*
     * This is horrible but required for backwards compatibility. We don't
     * actually do real key generation at all. We simply copy the key that was
     * previously set in the gctx. Hopefully at some point in the future all
     * of this can be removed and we will only support the EVP_KDF APIs.
     */
    if (!ossl_prov_cipher_copy(&key->cipher, &gctx->cipher)) {
        ossl_mac_key_free(key);
        ERR_raise(ERR_LIB_PROV, ERR_R_INTERNAL_ERROR);
        return NULL;
    }
    ossl_prov_cipher_reset(&gctx->cipher);
    key->priv_key = gctx->priv_key;
    key->priv_key_len = gctx->priv_key_len;
    gctx->priv_key_len = 0;
    gctx->priv_key = NULL;

    return key;
}

static void mac_gen_cleanup(void *genctx)
{
    struct mac_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    OPENSSL_secure_clear_free(gctx->priv_key, gctx->priv_key_len);
    ossl_prov_cipher_reset(&gctx->cipher);
    OPENSSL_free(gctx);
}

const OSSL_DISPATCH ossl_mac_legacy_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))mac_new },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))mac_free },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))mac_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))mac_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))mac_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))mac_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))mac_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))mac_match },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))mac_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))mac_imexport_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))mac_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))mac_imexport_types },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))mac_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))mac_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
        (void (*)(void))mac_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))mac_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))mac_gen_cleanup },
    OSSL_DISPATCH_END
};

const OSSL_DISPATCH ossl_cmac_legacy_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))mac_new_cmac },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))mac_free },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))mac_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))cmac_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))mac_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))mac_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))mac_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))mac_match },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))mac_import },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))cmac_imexport_types },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))mac_export },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))cmac_imexport_types },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))cmac_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))cmac_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
        (void (*)(void))cmac_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))mac_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))mac_gen_cleanup },
    OSSL_DISPATCH_END
};

