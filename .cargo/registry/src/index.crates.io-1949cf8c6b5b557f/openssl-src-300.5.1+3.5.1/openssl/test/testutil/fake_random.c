/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <string.h>
#include <openssl/core_names.h>
#include <openssl/rand.h>
#include <openssl/provider.h>
#include "../include/crypto/evp.h"
#include "../../crypto/evp/evp_local.h"
#include "../testutil.h"

typedef struct {
    fake_random_generate_cb *cb;
    int state;
    const char *name;
    EVP_RAND_CTX *ctx;
} FAKE_RAND;

static OSSL_FUNC_rand_newctx_fn fake_rand_newctx;
static OSSL_FUNC_rand_freectx_fn fake_rand_freectx;
static OSSL_FUNC_rand_instantiate_fn fake_rand_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn fake_rand_uninstantiate;
static OSSL_FUNC_rand_generate_fn fake_rand_generate;
static OSSL_FUNC_rand_gettable_ctx_params_fn fake_rand_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn fake_rand_get_ctx_params;
static OSSL_FUNC_rand_enable_locking_fn fake_rand_enable_locking;

static void *fake_rand_newctx(void *provctx, void *parent,
                              const OSSL_DISPATCH *parent_dispatch)
{
    FAKE_RAND *r = OPENSSL_zalloc(sizeof(*r));

    if (r != NULL)
        r->state = EVP_RAND_STATE_UNINITIALISED;
    return r;
}

static void fake_rand_freectx(void *vrng)
{
    OPENSSL_free(vrng);
}

static int fake_rand_instantiate(void *vrng, ossl_unused unsigned int strength,
                                 ossl_unused  int prediction_resistance,
                                 ossl_unused const unsigned char *pstr,
                                 size_t pstr_len,
                                 ossl_unused const OSSL_PARAM params[])
{
    FAKE_RAND *frng = (FAKE_RAND *)vrng;

    frng->state = EVP_RAND_STATE_READY;
    return 1;
}

static int fake_rand_uninstantiate(void *vrng)
{
    FAKE_RAND *frng = (FAKE_RAND *)vrng;

    frng->state = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static int fake_rand_generate(void *vrng, unsigned char *out, size_t outlen,
                              unsigned int strength, int prediction_resistance,
                              const unsigned char *adin, size_t adinlen)
{
    FAKE_RAND *frng = (FAKE_RAND *)vrng;
    size_t l;
    uint32_t r;

    if (frng->cb != NULL)
        return (*frng->cb)(out, outlen, frng->name, frng->ctx);
    while (outlen > 0) {
        r = test_random();
        l = outlen < sizeof(r) ? outlen : sizeof(r);

        memcpy(out, &r, l);
        out += l;
        outlen -= l;
    }
    return 1;
}

static int fake_rand_enable_locking(void *vrng)
{
    return 1;
}

static int fake_rand_get_ctx_params(ossl_unused void *vrng, OSSL_PARAM params[])
{
    FAKE_RAND *frng = (FAKE_RAND *)vrng;
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STATE);
    if (p != NULL && !OSSL_PARAM_set_int(p, frng->state))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_set_int(p, 256))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, INT_MAX))
        return 0;
    return 1;
}

static const OSSL_PARAM *fake_rand_gettable_ctx_params(ossl_unused void *vrng,
                                                       ossl_unused void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_int(OSSL_RAND_PARAM_STATE, NULL),
        OSSL_PARAM_uint(OSSL_RAND_PARAM_STRENGTH, NULL),
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static const OSSL_DISPATCH fake_rand_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void (*)(void))fake_rand_newctx },
    { OSSL_FUNC_RAND_FREECTX, (void (*)(void))fake_rand_freectx },
    { OSSL_FUNC_RAND_INSTANTIATE, (void (*)(void))fake_rand_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE, (void (*)(void))fake_rand_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void (*)(void))fake_rand_generate },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void (*)(void))fake_rand_enable_locking },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))fake_rand_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))fake_rand_get_ctx_params },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM fake_rand_rand[] = {
    { "FAKE", "provider=fake", fake_rand_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *fake_rand_query(void *provctx,
                                             int operation_id,
                                             int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_RAND:
        return fake_rand_rand;
    }
    return NULL;
}

/* Functions we provide to the core */
static const OSSL_DISPATCH fake_rand_method[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))OSSL_LIB_CTX_free },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fake_rand_query },
    OSSL_DISPATCH_END
};

static int fake_rand_provider_init(const OSSL_CORE_HANDLE *handle,
                                   const OSSL_DISPATCH *in,
                                   const OSSL_DISPATCH **out, void **provctx)
{
    if (!TEST_ptr(*provctx = OSSL_LIB_CTX_new()))
        return 0;
    *out = fake_rand_method;
    return 1;
}

static int check_rng(EVP_RAND_CTX *rng, const char *name)
{
    FAKE_RAND *f;

    if (!TEST_ptr(rng)) {
        TEST_info("random: %s", name);
        return 0;
    }
    f = rng->algctx;
    f->name = name;
    f->ctx = rng;
    return 1;
}

OSSL_PROVIDER *fake_rand_start(OSSL_LIB_CTX *libctx)
{
    OSSL_PROVIDER *p;

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, "fake-rand",
                                             fake_rand_provider_init))
            || !TEST_true(RAND_set_DRBG_type(libctx, "fake", NULL, NULL, NULL))
            || !TEST_ptr(p = OSSL_PROVIDER_try_load(libctx, "fake-rand", 1)))
        return NULL;

    /* Ensure that the fake rand is initialized. */
    if (!TEST_true(check_rng(RAND_get0_primary(libctx), "primary"))
            || !TEST_true(check_rng(RAND_get0_private(libctx), "private"))
            || !TEST_true(check_rng(RAND_get0_public(libctx), "public"))) {
        OSSL_PROVIDER_unload(p);
        return NULL;
    }

    return p;
}

void fake_rand_finish(OSSL_PROVIDER *p)
{
    OSSL_PROVIDER_unload(p);
}

void fake_rand_set_callback(EVP_RAND_CTX *rng,
                            int (*cb)(unsigned char *out, size_t outlen,
                                      const char *name, EVP_RAND_CTX *ctx))
{
    if (rng != NULL)
        ((FAKE_RAND *)rng->algctx)->cb = cb;
}

void fake_rand_set_public_private_callbacks(OSSL_LIB_CTX *libctx,
                                            int (*cb)(unsigned char *out,
                                                      size_t outlen,
                                                      const char *name,
                                                      EVP_RAND_CTX *ctx))
{
    fake_rand_set_callback(RAND_get0_private(libctx), cb);
    fake_rand_set_callback(RAND_get0_public(libctx), cb);
}

