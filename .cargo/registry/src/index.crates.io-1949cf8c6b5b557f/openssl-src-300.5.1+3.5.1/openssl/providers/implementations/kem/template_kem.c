/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "prov/provider_ctx.h"
#include "prov/implementations.h"
#include "prov/securitycheck.h"
#include "prov/providercommon.h"

extern const OSSL_DISPATCH ossl_template_asym_kem_functions[];

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
    if (getenv("TEMPLATEKEM"))
        fprintf(stderr, "TEMPLATE_KEM: %s", out);
}
#endif

typedef struct {
    OSSL_LIB_CTX *libctx;
    /* some algorithm-specific key struct */
    int op;
} PROV_TEMPLATE_CTX;

static OSSL_FUNC_kem_newctx_fn template_newctx;
static OSSL_FUNC_kem_encapsulate_init_fn template_encapsulate_init;
static OSSL_FUNC_kem_encapsulate_fn template_encapsulate;
static OSSL_FUNC_kem_decapsulate_init_fn template_decapsulate_init;
static OSSL_FUNC_kem_decapsulate_fn template_decapsulate;
static OSSL_FUNC_kem_freectx_fn template_freectx;
static OSSL_FUNC_kem_set_ctx_params_fn template_set_ctx_params;
static OSSL_FUNC_kem_settable_ctx_params_fn template_settable_ctx_params;

static void *template_newctx(void *provctx)
{
    PROV_TEMPLATE_CTX *ctx = OPENSSL_zalloc(sizeof(*ctx));

    debug_print("newctx called\n");
    if (ctx == NULL)
        return NULL;
    ctx->libctx = PROV_LIBCTX_OF(provctx);

    debug_print("newctx returns %p\n", ctx);
    return ctx;
}

static void template_freectx(void *vctx)
{
    PROV_TEMPLATE_CTX *ctx = (PROV_TEMPLATE_CTX *)vctx;

    debug_print("freectx %p\n", ctx);
    OPENSSL_free(ctx);
}

static int template_init(void *vctx, int operation, void *vkey, void *vauth,
                         ossl_unused const OSSL_PARAM params[])
{
    PROV_TEMPLATE_CTX *ctx = (PROV_TEMPLATE_CTX *)vctx;

    debug_print("init %p / %p\n", ctx, vkey);
    if (!ossl_prov_is_running())
        return 0;

    /* check and fill in reference to key */
    ctx->op = operation;
    debug_print("init OK\n");
    return 1;
}

static int template_encapsulate_init(void *vctx, void *vkey,
                                     const OSSL_PARAM params[])
{
    return template_init(vctx, EVP_PKEY_OP_ENCAPSULATE, vkey, NULL, params);
}

static int template_decapsulate_init(void *vctx, void *vkey,
                                     const OSSL_PARAM params[])
{
    return template_init(vctx, EVP_PKEY_OP_DECAPSULATE, vkey, NULL, params);
}

static int template_set_ctx_params(void *vctx, const OSSL_PARAM params[])
{
    PROV_TEMPLATE_CTX *ctx = (PROV_TEMPLATE_CTX *)vctx;

    debug_print("set ctx params %p\n", ctx);
    if (ctx == NULL)
        return 0;
    if (ossl_param_is_empty(params))
        return 1;

    debug_print("set ctx params OK\n");
    return 1;
}

static const OSSL_PARAM known_settable_template_ctx_params[] = {
    /* possibly more params */
    OSSL_PARAM_END
};

static const OSSL_PARAM *template_settable_ctx_params(ossl_unused void *vctx,
                                                      ossl_unused void *provctx)
{
    return known_settable_template_ctx_params;
}

static int template_encapsulate(void *vctx, unsigned char *out, size_t *outlen,
                                unsigned char *secret, size_t *secretlen)
{
    debug_print("encaps %p to %p\n", vctx, out);

    /* add algorithm-specific length checks */

    if (outlen != NULL)
        *outlen = 0; /* replace with real encapsulated data length */
    if (secretlen != NULL)
        *secretlen = 0; /* replace with real shared secret length */

    if (out == NULL) {
        if (outlen != NULL && secretlen != NULL)
            debug_print("encaps outlens set to %zu and %zu\n", *outlen, *secretlen);
        return 1;
    }

    /* check key and perform real KEM operation */

    debug_print("encaps OK\n");
    return 1;
}

static int template_decapsulate(void *vctx, unsigned char *out, size_t *outlen,
                                const unsigned char *in, size_t inlen)
{
    debug_print("decaps %p to %p inlen at %zu\n", vctx, out, inlen);

    /* add algorithm-specific length checks */

    if (outlen != NULL)
        *outlen = 0; /* replace with shared secret length */

    if (out == NULL) {
        if (outlen != NULL)
            debug_print("decaps outlen set to %zu \n", *outlen);
        return 1;
    }

    /* check key and perform real decaps operation */

    debug_print("decaps OK\n");
    return 1;
}

const OSSL_DISPATCH ossl_template_asym_kem_functions[] = {
    { OSSL_FUNC_KEM_NEWCTX, (void (*)(void))template_newctx },
    { OSSL_FUNC_KEM_ENCAPSULATE_INIT,
      (void (*)(void))template_encapsulate_init },
    { OSSL_FUNC_KEM_ENCAPSULATE, (void (*)(void))template_encapsulate },
    { OSSL_FUNC_KEM_DECAPSULATE_INIT,
      (void (*)(void))template_decapsulate_init },
    { OSSL_FUNC_KEM_DECAPSULATE, (void (*)(void))template_decapsulate },
    { OSSL_FUNC_KEM_FREECTX, (void (*)(void))template_freectx },
    { OSSL_FUNC_KEM_SET_CTX_PARAMS,
      (void (*)(void))template_set_ctx_params },
    { OSSL_FUNC_KEM_SETTABLE_CTX_PARAMS,
      (void (*)(void))template_settable_ctx_params },
    OSSL_DISPATCH_END
};
