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
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/self_test.h>
#include "internal/param_build_set.h"
#include <openssl/param_build.h>
#include "prov/implementations.h"
#include "prov/providercommon.h"
#include "prov/provider_ctx.h"
#include "prov/securitycheck.h"

extern const OSSL_DISPATCH ossl_template_keymgmt_functions[];

#define BUFSIZE 1000
#if defined(NDEBUG) || defined(OPENSSL_NO_STDIO)
static void debug_print(char *fmt, ...)
{
}
#else
static void debug_print(char *fmt, ...)
{
    char out[BUFSIZE];
    va_list argptr;

    va_start(argptr, fmt);
    vsnprintf(out, BUFSIZE, fmt, argptr);
    va_end(argptr);
    if (getenv("TEMPLATEKM"))
        fprintf(stderr, "TEMPLATE_KM: %s", out);
}
#endif

static OSSL_FUNC_keymgmt_new_fn template_new;
static OSSL_FUNC_keymgmt_free_fn template_free;
static OSSL_FUNC_keymgmt_gen_init_fn template_gen_init;
static OSSL_FUNC_keymgmt_gen_fn template_gen;
static OSSL_FUNC_keymgmt_gen_cleanup_fn template_gen_cleanup;
static OSSL_FUNC_keymgmt_gen_set_params_fn template_gen_set_params;
static OSSL_FUNC_keymgmt_gen_settable_params_fn template_gen_settable_params;
static OSSL_FUNC_keymgmt_get_params_fn template_get_params;
static OSSL_FUNC_keymgmt_gettable_params_fn template_gettable_params;
static OSSL_FUNC_keymgmt_set_params_fn template_set_params;
static OSSL_FUNC_keymgmt_settable_params_fn template_settable_params;
static OSSL_FUNC_keymgmt_has_fn template_has;
static OSSL_FUNC_keymgmt_match_fn template_match;
static OSSL_FUNC_keymgmt_import_fn template_import;
static OSSL_FUNC_keymgmt_export_fn template_export;
static OSSL_FUNC_keymgmt_import_types_fn template_imexport_types;
static OSSL_FUNC_keymgmt_export_types_fn template_imexport_types;

static OSSL_FUNC_keymgmt_dup_fn template_dup;

struct template_gen_ctx {
    void *provctx;
    int selection;
};

static void *template_new(void *provctx)
{
    void *key = NULL;

    debug_print("new key req\n");
    if (!ossl_prov_is_running())
        return 0;

    /* add logic to create new key */

    debug_print("new key = %p\n", key);
    return key;
}

static void template_free(void *vkey)
{
    debug_print("free key %p\n", vkey);
    if (vkey == NULL)
        return;

    /* add logic to free all key components */

    OPENSSL_free(vkey);
}

static int template_has(const void *keydata, int selection)
{
    int ok = 0;

    debug_print("has %p\n", keydata);
    if (ossl_prov_is_running() && keydata != NULL) {
        /* add logic to check whether this key has the requested parameters */
    }
    debug_print("has result %d\n", ok);
    return ok;
}

static int template_match(const void *keydata1, const void *keydata2, int selection)
{
    int ok = 1;

    debug_print("matching %p and %p\n", keydata1, keydata2);
    if (!ossl_prov_is_running())
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_DOMAIN_PARAMETERS) != 0)
        ok = ok && keydata1 != NULL && keydata2 != NULL;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int key_checked = 0;

        if ((selection & OSSL_KEYMGMT_SELECT_PUBLIC_KEY) != 0) {
            /* validate whether the public keys match */
        }
        if (!key_checked
            && (selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0) {
            /* validate whether the private keys match */
        }
        ok = ok && key_checked;
    }
    debug_print("match result %d\n", ok);
    return ok;
}

static int key_to_params(void *key, OSSL_PARAM_BLD *tmpl,
                         OSSL_PARAM params[], int include_private)
{
    if (key == NULL)
        return 0;

    /* add public and/or private key parts to templ as possible */

    return 1;
}

static int template_export(void *key, int selection, OSSL_CALLBACK *param_cb,
                           void *cbarg)
{
    OSSL_PARAM_BLD *tmpl;
    OSSL_PARAM *params = NULL;
    int ret = 0;

    debug_print("export %p\n", key);
    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    tmpl = OSSL_PARAM_BLD_new();
    if (tmpl == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0) {
        int include_private = ((selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY) != 0);

        if (!key_to_params(key, tmpl, NULL, include_private))
            goto err;
    }

    params = OSSL_PARAM_BLD_to_param(tmpl);
    if (params == NULL)
        goto err;

    ret = param_cb(params, cbarg);
    OSSL_PARAM_free(params);
err:
    OSSL_PARAM_BLD_free(tmpl);
    debug_print("export result %d\n", ret);
    return ret;
}

static int ossl_template_key_fromdata(void *key,
                                      const OSSL_PARAM params[],
                                      int include_private)
{
    const OSSL_PARAM *param_priv_key = NULL, *param_pub_key;

    if (key == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 0;

    /* validate integrity of key (algorithm type specific) */

    param_pub_key = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PUB_KEY);
    if (include_private)
        param_priv_key =
            OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_PRIV_KEY);

    if (param_pub_key == NULL && param_priv_key == NULL)
        return 0;

    if (param_priv_key != NULL) {
        /* retrieve private key and check integrity */
    }

    if (param_pub_key != NULL) {
        /* retrieve public key and check integrity */
    }

    return 1;
}

static int template_import(void *key, int selection, const OSSL_PARAM params[])
{
    int ok = 1;
    int include_private;

    debug_print("import %p\n", key);
    if (!ossl_prov_is_running() || key == NULL)
        return 0;

    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) == 0)
        return 0;

    include_private = selection & OSSL_KEYMGMT_SELECT_PRIVATE_KEY ? 1 : 0;
    ok = ok && ossl_template_key_fromdata(key, params, include_private);

    debug_print("import result %d\n", ok);
    return ok;
}

#define TEMPLATE_KEY_TYPES()                                     \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PUB_KEY, NULL, 0),   \
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_PRIV_KEY, NULL, 0)

static const OSSL_PARAM template_key_types[] = {
    TEMPLATE_KEY_TYPES(),
    OSSL_PARAM_END
};

static const OSSL_PARAM *template_imexport_types(int selection)
{
    debug_print("getting imexport types\n");
    if ((selection & OSSL_KEYMGMT_SELECT_KEYPAIR) != 0)
        return template_key_types;
    return NULL;
}

static int template_get_params(void *key, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    debug_print("get params %p\n", key);

    if (ossl_param_is_empty(params))
        return 0;

    /* return sensible values for at least these parameters */

    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, 0))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_SECURITY_BITS)) != NULL
        && !OSSL_PARAM_set_int(p, 0))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_MAX_SIZE)) != NULL
        && !OSSL_PARAM_set_int(p, 0))
        return 0;
    if ((p = OSSL_PARAM_locate(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY)) != NULL) {
        if (!OSSL_PARAM_set_octet_string(p, NULL, 0))
            return 0;
    }

    debug_print("get params OK\n");
    return 1;
}

static const OSSL_PARAM template_gettable_params_arr[] = {
    OSSL_PARAM_int(OSSL_PKEY_PARAM_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_SECURITY_BITS, NULL),
    OSSL_PARAM_int(OSSL_PKEY_PARAM_MAX_SIZE, NULL),
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *template_gettable_params(void *provctx)
{
    debug_print("gettable params called\n");
    return template_gettable_params_arr;
}

static int template_set_params(void *key, const OSSL_PARAM params[])
{
    const OSSL_PARAM *p;

    debug_print("set params called for %p\n", key);
    if (ossl_param_is_empty(params))
        return 1; /* OK not to set anything */

    p = OSSL_PARAM_locate_const(params, OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY);
    if (p != NULL) {
        /* load public key structure */
    }

    debug_print("set params OK\n");
    return 1;
}

static const OSSL_PARAM template_settable_params_arr[] = {
    OSSL_PARAM_octet_string(OSSL_PKEY_PARAM_ENCODED_PUBLIC_KEY, NULL, 0),
    OSSL_PARAM_END
};

static const OSSL_PARAM *template_settable_params(void *provctx)
{
    debug_print("settable params called\n");
    return template_settable_params_arr;
}

static int template_gen_set_params(void *genctx, const OSSL_PARAM params[])
{
    struct template_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return 0;

    debug_print("empty gen_set params called for %p\n", gctx);
    return 1;
}

static void *template_gen_init(void *provctx, int selection,
                               const OSSL_PARAM params[])
{
    struct template_gen_ctx *gctx = NULL;

    debug_print("gen init called for %p\n", provctx);

    /* perform algorithm type specific sanity checks */

    if (!ossl_prov_is_running())
        return NULL;

    if ((gctx = OPENSSL_zalloc(sizeof(*gctx))) != NULL) {
        gctx->provctx = provctx;
        gctx->selection = selection;
    }
    if (!template_gen_set_params(gctx, params)) {
        OPENSSL_free(gctx);
        gctx = NULL;
    }
    debug_print("gen init returns %p\n", gctx);
    return gctx;
}

static const OSSL_PARAM *template_gen_settable_params(ossl_unused void *genctx,
                                                      ossl_unused void *provctx)
{
    static OSSL_PARAM settable[] = {
        OSSL_PARAM_END
    };
    return settable;
}

static void *template_gen(void *vctx, OSSL_CALLBACK *osslcb, void *cbarg)
{
    struct template_gen_ctx *gctx = (struct template_gen_ctx *)vctx;
    void *key = NULL;

    debug_print("gen called for %p\n", gctx);

    if (gctx == NULL)
        goto err;

    /* generate and return new key */

    debug_print("gen returns set %p\n", key);
    return key;
err:
    template_free(key);
    debug_print("gen returns NULL\n");
    return NULL;
}

static void template_gen_cleanup(void *genctx)
{
    struct template_gen_ctx *gctx = genctx;

    if (gctx == NULL)
        return;

    debug_print("gen cleanup for %p\n", gctx);
    OPENSSL_free(gctx);
}

static void *template_dup(const void *vsrckey, int selection)
{
    void *dstkey = NULL;

    debug_print("dup called for %p\n", vsrckey);
    if (!ossl_prov_is_running())
        return NULL;

    dstkey = template_new(NULL);
    if (dstkey == NULL)
        goto err;

    /* populate dstkey from vsrckey material */

    debug_print("dup returns %p\n", dstkey);
    return dstkey;
 err:
    template_free(dstkey);
    debug_print("dup returns NULL\n");
    return NULL;
}

const OSSL_DISPATCH ossl_template_keymgmt_functions[] = {
    { OSSL_FUNC_KEYMGMT_NEW, (void (*)(void))template_new },
    { OSSL_FUNC_KEYMGMT_FREE, (void (*)(void))template_free },
    { OSSL_FUNC_KEYMGMT_GET_PARAMS, (void (*) (void))template_get_params },
    { OSSL_FUNC_KEYMGMT_GETTABLE_PARAMS, (void (*) (void))template_gettable_params },
    { OSSL_FUNC_KEYMGMT_SET_PARAMS, (void (*) (void))template_set_params },
    { OSSL_FUNC_KEYMGMT_SETTABLE_PARAMS, (void (*) (void))template_settable_params },
    { OSSL_FUNC_KEYMGMT_HAS, (void (*)(void))template_has },
    { OSSL_FUNC_KEYMGMT_MATCH, (void (*)(void))template_match },
    { OSSL_FUNC_KEYMGMT_IMPORT_TYPES, (void (*)(void))template_imexport_types },
    { OSSL_FUNC_KEYMGMT_EXPORT_TYPES, (void (*)(void))template_imexport_types },
    { OSSL_FUNC_KEYMGMT_IMPORT, (void (*)(void))template_import },
    { OSSL_FUNC_KEYMGMT_EXPORT, (void (*)(void))template_export },
    { OSSL_FUNC_KEYMGMT_GEN_INIT, (void (*)(void))template_gen_init },
    { OSSL_FUNC_KEYMGMT_GEN_SET_PARAMS, (void (*)(void))template_gen_set_params },
    { OSSL_FUNC_KEYMGMT_GEN_SETTABLE_PARAMS,
      (void (*)(void))template_gen_settable_params },
    { OSSL_FUNC_KEYMGMT_GEN, (void (*)(void))template_gen },
    { OSSL_FUNC_KEYMGMT_GEN_CLEANUP, (void (*)(void))template_gen_cleanup },
    { OSSL_FUNC_KEYMGMT_DUP, (void (*)(void))template_dup },
    OSSL_DISPATCH_END
};
