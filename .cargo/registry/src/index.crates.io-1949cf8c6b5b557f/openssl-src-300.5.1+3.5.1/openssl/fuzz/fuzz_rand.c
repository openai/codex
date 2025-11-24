/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/core_names.h>
#include <openssl/rand.h>
#include <openssl/provider.h>
#include "fuzzer.h"

static OSSL_FUNC_rand_newctx_fn fuzz_rand_newctx;
static OSSL_FUNC_rand_freectx_fn fuzz_rand_freectx;
static OSSL_FUNC_rand_instantiate_fn fuzz_rand_instantiate;
static OSSL_FUNC_rand_uninstantiate_fn fuzz_rand_uninstantiate;
static OSSL_FUNC_rand_generate_fn fuzz_rand_generate;
static OSSL_FUNC_rand_gettable_ctx_params_fn fuzz_rand_gettable_ctx_params;
static OSSL_FUNC_rand_get_ctx_params_fn fuzz_rand_get_ctx_params;
static OSSL_FUNC_rand_enable_locking_fn fuzz_rand_enable_locking;

static void *fuzz_rand_newctx(
         void *provctx, void *parent, const OSSL_DISPATCH *parent_dispatch)
{
    int *st = OPENSSL_malloc(sizeof(*st));

    if (st != NULL)
        *st = EVP_RAND_STATE_UNINITIALISED;
    return st;
}

static void fuzz_rand_freectx(ossl_unused void *vrng)
{
    OPENSSL_free(vrng);
}

static int fuzz_rand_instantiate(ossl_unused void *vrng,
                                 ossl_unused unsigned int strength,
                                 ossl_unused int prediction_resistance,
                                 ossl_unused const unsigned char *pstr,
                                 ossl_unused size_t pstr_len,
                                 ossl_unused const OSSL_PARAM params[])
{
    *(int *)vrng = EVP_RAND_STATE_READY;
    return 1;
}

static int fuzz_rand_uninstantiate(ossl_unused void *vrng)
{
    *(int *)vrng = EVP_RAND_STATE_UNINITIALISED;
    return 1;
}

static int fuzz_rand_generate(ossl_unused void *vdrbg,
                              unsigned char *out, size_t outlen,
                              ossl_unused unsigned int strength,
                              ossl_unused int prediction_resistance,
                              ossl_unused const unsigned char *adin,
                              ossl_unused size_t adinlen)
{
    unsigned char val = 1;
    size_t i;

    for (i = 0; i < outlen; i++)
        out[i] = val++;
    return 1;
}

static int fuzz_rand_enable_locking(ossl_unused void *vrng)
{
    return 1;
}

static int fuzz_rand_get_ctx_params(void *vrng, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STATE);
    if (p != NULL && !OSSL_PARAM_set_int(p, *(int *)vrng))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_STRENGTH);
    if (p != NULL && !OSSL_PARAM_set_int(p, 500))
        return 0;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, INT_MAX))
        return 0;
    return 1;
}

static const OSSL_PARAM *fuzz_rand_gettable_ctx_params(ossl_unused void *vrng,
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

static const OSSL_DISPATCH fuzz_rand_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void (*)(void))fuzz_rand_newctx },
    { OSSL_FUNC_RAND_FREECTX, (void (*)(void))fuzz_rand_freectx },
    { OSSL_FUNC_RAND_INSTANTIATE, (void (*)(void))fuzz_rand_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE, (void (*)(void))fuzz_rand_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void (*)(void))fuzz_rand_generate },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void (*)(void))fuzz_rand_enable_locking },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))fuzz_rand_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))fuzz_rand_get_ctx_params },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM fuzz_rand_rand[] = {
    { "fuzz", "provider=fuzz-rand", fuzz_rand_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *fuzz_rand_query(void *provctx,
                                             int operation_id,
                                             int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_RAND:
        return fuzz_rand_rand;
    }
    return NULL;
}

/* Functions we provide to the core */
static const OSSL_DISPATCH fuzz_rand_method[] = {
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))OSSL_LIB_CTX_free },
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))fuzz_rand_query },
    OSSL_DISPATCH_END
};

static int fuzz_rand_provider_init(const OSSL_CORE_HANDLE *handle,
                                   const OSSL_DISPATCH *in,
                                   const OSSL_DISPATCH **out, void **provctx)
{
    *provctx = OSSL_LIB_CTX_new();
    if (*provctx == NULL)
        return 0;
    *out = fuzz_rand_method;
    return 1;
}

static OSSL_PROVIDER *r_prov;

void FuzzerSetRand(void)
{
    if (!OSSL_PROVIDER_add_builtin(NULL, "fuzz-rand", fuzz_rand_provider_init)
        || !RAND_set_DRBG_type(NULL, "fuzz", NULL, NULL, NULL)
        || (r_prov = OSSL_PROVIDER_try_load(NULL, "fuzz-rand", 1)) == NULL)
        exit(1);
}

void FuzzerClearRand(void)
{
    OSSL_PROVIDER_unload(r_prov);
}
