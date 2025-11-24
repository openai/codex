/*
 * Copyright 2018-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Here is an STORE loader for ENGINE backed keys.  It relies on deprecated
 * functions, and therefore need to have deprecation warnings suppressed.
 * This file is not compiled at all in a '--api=3 no-deprecated' configuration.
 */
#define OPENSSL_SUPPRESS_DEPRECATED

#include "internal/e_os.h"
#include "apps.h"

#ifndef OPENSSL_NO_ENGINE

# include <stdarg.h>
# include <string.h>
# include <openssl/engine.h>
# include <openssl/store.h>

/*
 * Support for legacy private engine keys via the 'org.openssl.engine:' scheme
 *
 * org.openssl.engine:{engineid}:{keyid}
 *
 * Note: we ONLY support ENGINE_load_private_key() and ENGINE_load_public_key()
 * Note 2: This scheme has a precedent in code in PKIX-SSH. for exactly
 * this sort of purpose.
 */

/* Local definition of OSSL_STORE_LOADER_CTX */
struct ossl_store_loader_ctx_st {
    ENGINE *e;                   /* Structural reference */
    char *keyid;
    int expected;
    int loaded;                  /* 0 = key not loaded yet, 1 = key loaded */
};

static OSSL_STORE_LOADER_CTX *OSSL_STORE_LOADER_CTX_new(ENGINE *e, char *keyid)
{
    OSSL_STORE_LOADER_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    if (ctx != NULL) {
        ctx->e = e;
        ctx->keyid = keyid;
    }
    return ctx;
}

static void OSSL_STORE_LOADER_CTX_free(OSSL_STORE_LOADER_CTX *ctx)
{
    if (ctx != NULL) {
        ENGINE_free(ctx->e);
        OPENSSL_free(ctx->keyid);
        OPENSSL_free(ctx);
    }
}

static OSSL_STORE_LOADER_CTX *engine_open(const OSSL_STORE_LOADER *loader,
                                          const char *uri,
                                          const UI_METHOD *ui_method,
                                          void *ui_data)
{
    const char *p = uri, *q;
    ENGINE *e = NULL;
    char *keyid = NULL;
    OSSL_STORE_LOADER_CTX *ctx = NULL;

    if (!CHECK_AND_SKIP_CASE_PREFIX(p, ENGINE_SCHEME_COLON))
        return NULL;

    /* Look for engine ID */
    q = strchr(p, ':');
    if (q != NULL                /* There is both an engine ID and a key ID */
        && p[0] != ':'           /* The engine ID is at least one character */
        && q[1] != '\0') {       /* The key ID is at least one character */
        char engineid[256];
        size_t engineid_l = q - p;

        strncpy(engineid, p, engineid_l);
        engineid[engineid_l] = '\0';
        e = ENGINE_by_id(engineid);

        keyid = OPENSSL_strdup(q + 1);
    }

    if (e != NULL && keyid != NULL)
        ctx = OSSL_STORE_LOADER_CTX_new(e, keyid);

    if (ctx == NULL) {
        OPENSSL_free(keyid);
        ENGINE_free(e);
    }

    return ctx;
}

static int engine_expect(OSSL_STORE_LOADER_CTX *ctx, int expected)
{
    if (expected == 0
        || expected == OSSL_STORE_INFO_PUBKEY
        || expected == OSSL_STORE_INFO_PKEY) {
        ctx->expected = expected;
        return 1;
    }
    return 0;
}

static OSSL_STORE_INFO *engine_load(OSSL_STORE_LOADER_CTX *ctx,
                                    const UI_METHOD *ui_method, void *ui_data)
{
    EVP_PKEY *pkey = NULL, *pubkey = NULL;
    OSSL_STORE_INFO *info = NULL;

    if (ctx->loaded == 0) {
        if (ENGINE_init(ctx->e)) {
            if (ctx->expected == 0
                || ctx->expected == OSSL_STORE_INFO_PKEY)
                pkey =
                    ENGINE_load_private_key(ctx->e, ctx->keyid,
                                            (UI_METHOD *)ui_method, ui_data);
            if ((pkey == NULL && ctx->expected == 0)
                || ctx->expected == OSSL_STORE_INFO_PUBKEY)
                pubkey =
                    ENGINE_load_public_key(ctx->e, ctx->keyid,
                                           (UI_METHOD *)ui_method, ui_data);
            ENGINE_finish(ctx->e);
        }
    }

    ctx->loaded = 1;

    if (pubkey != NULL)
        info = OSSL_STORE_INFO_new_PUBKEY(pubkey);
    else if (pkey != NULL)
        info = OSSL_STORE_INFO_new_PKEY(pkey);
    if (info == NULL) {
        EVP_PKEY_free(pkey);
        EVP_PKEY_free(pubkey);
    }
    return info;
}

static int engine_eof(OSSL_STORE_LOADER_CTX *ctx)
{
    return ctx->loaded != 0;
}

static int engine_error(OSSL_STORE_LOADER_CTX *ctx)
{
    return 0;
}

static int engine_close(OSSL_STORE_LOADER_CTX *ctx)
{
    OSSL_STORE_LOADER_CTX_free(ctx);
    return 1;
}

int setup_engine_loader(void)
{
    OSSL_STORE_LOADER *loader = NULL;

    if ((loader = OSSL_STORE_LOADER_new(NULL, ENGINE_SCHEME)) == NULL
        || !OSSL_STORE_LOADER_set_open(loader, engine_open)
        || !OSSL_STORE_LOADER_set_expect(loader, engine_expect)
        || !OSSL_STORE_LOADER_set_load(loader, engine_load)
        || !OSSL_STORE_LOADER_set_eof(loader, engine_eof)
        || !OSSL_STORE_LOADER_set_error(loader, engine_error)
        || !OSSL_STORE_LOADER_set_close(loader, engine_close)
        || !OSSL_STORE_register_loader(loader)) {
        OSSL_STORE_LOADER_free(loader);
        loader = NULL;
    }

    return loader != NULL;
}

void destroy_engine_loader(void)
{
    OSSL_STORE_LOADER *loader = OSSL_STORE_unregister_loader(ENGINE_SCHEME);
    OSSL_STORE_LOADER_free(loader);
}

#else  /* !OPENSSL_NO_ENGINE */

int setup_engine_loader(void)
{
    return 0;
}

void destroy_engine_loader(void)
{
}

#endif
