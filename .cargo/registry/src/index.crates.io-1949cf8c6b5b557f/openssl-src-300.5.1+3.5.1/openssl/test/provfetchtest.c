/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include <openssl/provider.h>
#include <openssl/decoder.h>
#include <openssl/encoder.h>
#include <openssl/store.h>
#include <openssl/rand.h>
#include <openssl/core_names.h>
#include "testutil.h"

static int dummy_decoder_decode(void *ctx, OSSL_CORE_BIO *cin, int selection,
                                OSSL_CALLBACK *object_cb, void *object_cbarg,
                                OSSL_PASSPHRASE_CALLBACK *pw_cb, void *pw_cbarg)
{
    return 0;
}

static const OSSL_DISPATCH dummy_decoder_functions[] = {
    { OSSL_FUNC_DECODER_DECODE, (void (*)(void))dummy_decoder_decode },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM dummy_decoders[] = {
    { "DUMMY", "provider=dummy,input=pem", dummy_decoder_functions },
    { NULL, NULL, NULL }
};

static int dummy_encoder_encode(void *ctx, OSSL_CORE_BIO *out,
                                const void *obj_raw,
                                const OSSL_PARAM obj_abstract[], int selection,
                                OSSL_PASSPHRASE_CALLBACK *cb, void *cbarg)
{
    return 0;
}

static const OSSL_DISPATCH dummy_encoder_functions[] = {
    { OSSL_FUNC_DECODER_DECODE, (void (*)(void))dummy_encoder_encode },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM dummy_encoders[] = {
    { "DUMMY", "provider=dummy,output=pem", dummy_encoder_functions },
    { NULL, NULL, NULL }
};

static void *dummy_store_open(void *provctx, const char *uri)
{
    return NULL;
}

static int dummy_store_load(void *loaderctx,  OSSL_CALLBACK *object_cb,
                            void *object_cbarg, OSSL_PASSPHRASE_CALLBACK *pw_cb,
                            void *pw_cbarg)
{
    return 0;
}

static int dumm_store_eof(void *loaderctx)
{
    return 0;
}

static int dummy_store_close(void *loaderctx)
{
    return 0;
}

static const OSSL_DISPATCH dummy_store_functions[] = {
    { OSSL_FUNC_STORE_OPEN, (void (*)(void))dummy_store_open },
    { OSSL_FUNC_STORE_LOAD, (void (*)(void))dummy_store_load },
    { OSSL_FUNC_STORE_EOF, (void (*)(void))dumm_store_eof },
    { OSSL_FUNC_STORE_CLOSE, (void (*)(void))dummy_store_close },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM dummy_store[] = {
    { "DUMMY", "provider=dummy", dummy_store_functions },
    { NULL, NULL, NULL }
};

static void *dummy_rand_newctx(void *provctx, void *parent,
                               const OSSL_DISPATCH *parent_calls)
{
    return provctx;
}

static void dummy_rand_freectx(void *vctx)
{
}

static int dummy_rand_instantiate(void *vdrbg, unsigned int strength,
                                  int prediction_resistance,
                                  const unsigned char *pstr, size_t pstr_len,
                                  const OSSL_PARAM params[])
{
    return 1;
}

static int dummy_rand_uninstantiate(void *vdrbg)
{
    return 1;
}

static int dummy_rand_generate(void *vctx, unsigned char *out, size_t outlen,
                               unsigned int strength, int prediction_resistance,
                               const unsigned char *addin, size_t addin_len)
{
    size_t i;

    for (i = 0; i <outlen; i++)
        out[i] = (unsigned char)(i & 0xff);

    return 1;
}

static const OSSL_PARAM *dummy_rand_gettable_ctx_params(void *vctx, void *provctx)
{
    static const OSSL_PARAM known_gettable_ctx_params[] = {
        OSSL_PARAM_size_t(OSSL_RAND_PARAM_MAX_REQUEST, NULL),
        OSSL_PARAM_END
    };
    return known_gettable_ctx_params;
}

static int dummy_rand_get_ctx_params(void *vctx, OSSL_PARAM params[])
{
    OSSL_PARAM *p;

    p = OSSL_PARAM_locate(params, OSSL_RAND_PARAM_MAX_REQUEST);
    if (p != NULL && !OSSL_PARAM_set_size_t(p, INT_MAX))
        return 0;

    return 1;
}

static int dummy_rand_enable_locking(void *vtest)
{
    return 1;
}

static int dummy_rand_lock(void *vtest)
{
    return 1;
}

static void dummy_rand_unlock(void *vtest)
{
}

static const OSSL_DISPATCH dummy_rand_functions[] = {
    { OSSL_FUNC_RAND_NEWCTX, (void (*)(void))dummy_rand_newctx },
    { OSSL_FUNC_RAND_FREECTX, (void (*)(void))dummy_rand_freectx },
    { OSSL_FUNC_RAND_INSTANTIATE, (void (*)(void))dummy_rand_instantiate },
    { OSSL_FUNC_RAND_UNINSTANTIATE, (void (*)(void))dummy_rand_uninstantiate },
    { OSSL_FUNC_RAND_GENERATE, (void (*)(void))dummy_rand_generate },
    { OSSL_FUNC_RAND_GETTABLE_CTX_PARAMS,
      (void(*)(void))dummy_rand_gettable_ctx_params },
    { OSSL_FUNC_RAND_GET_CTX_PARAMS, (void(*)(void))dummy_rand_get_ctx_params },
    { OSSL_FUNC_RAND_ENABLE_LOCKING, (void(*)(void))dummy_rand_enable_locking },
    { OSSL_FUNC_RAND_LOCK, (void(*)(void))dummy_rand_lock },
    { OSSL_FUNC_RAND_UNLOCK, (void(*)(void))dummy_rand_unlock },
    OSSL_DISPATCH_END
};

static const OSSL_ALGORITHM dummy_rand[] = {
    { "DUMMY", "provider=dummy", dummy_rand_functions },
    { NULL, NULL, NULL }
};

static const OSSL_ALGORITHM *dummy_query(void *provctx, int operation_id,
                                         int *no_cache)
{
    *no_cache = 0;
    switch (operation_id) {
    case OSSL_OP_DECODER:
        return dummy_decoders;
    case OSSL_OP_ENCODER:
        return dummy_encoders;
    case OSSL_OP_STORE:
        return dummy_store;
    case OSSL_OP_RAND:
        return dummy_rand;
    }
    return NULL;
}

static const OSSL_DISPATCH dummy_dispatch_table[] = {
    { OSSL_FUNC_PROVIDER_QUERY_OPERATION, (void (*)(void))dummy_query },
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))OSSL_LIB_CTX_free },
    OSSL_DISPATCH_END
};

static int dummy_provider_init(const OSSL_CORE_HANDLE *handle,
                               const OSSL_DISPATCH *in,
                               const OSSL_DISPATCH **out,
                               void **provctx)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new_child(handle, in);
    unsigned char buf[32];

    *provctx = (void *)libctx;
    *out = dummy_dispatch_table;

    /*
     * Do some work using the child libctx, to make sure this is possible from
     * inside the init function.
     */
    if (RAND_bytes_ex(libctx, buf, sizeof(buf), 0) <= 0)
        return 0;

    return 1;
}

/*
 * Try fetching and freeing various things.
 * Test 0: Decoder
 * Test 1: Encoder
 * Test 2: Store loader
 * Test 3: EVP_RAND
 * Test 4-7: As above, but additionally with a query string
 */
static int fetch_test(int tst)
{
    OSSL_LIB_CTX *libctx = OSSL_LIB_CTX_new();
    OSSL_PROVIDER *dummyprov = NULL;
    OSSL_PROVIDER *nullprov = NULL;
    OSSL_DECODER *decoder = NULL;
    OSSL_ENCODER *encoder = NULL;
    OSSL_STORE_LOADER *loader = NULL;
    int testresult = 0;
    unsigned char buf[32];
    int query = tst > 3;

    if (!TEST_ptr(libctx))
        goto err;

    if (!TEST_true(OSSL_PROVIDER_add_builtin(libctx, "dummy-prov",
                                             dummy_provider_init))
            || !TEST_ptr(nullprov = OSSL_PROVIDER_load(libctx, "default"))
            || !TEST_ptr(dummyprov = OSSL_PROVIDER_load(libctx, "dummy-prov")))
        goto err;

    switch (tst % 4) {
    case 0:
        decoder = OSSL_DECODER_fetch(libctx, "DUMMY",
                                     query ? "provider=dummy" : NULL);
        if (!TEST_ptr(decoder))
            goto err;
        break;
    case 1:
        encoder = OSSL_ENCODER_fetch(libctx, "DUMMY",
                                     query ? "provider=dummy" : NULL);
        if (!TEST_ptr(encoder))
            goto err;
        break;
    case 2:
        loader = OSSL_STORE_LOADER_fetch(libctx, "DUMMY",
                                         query ? "provider=dummy" : NULL);
        if (!TEST_ptr(loader))
            goto err;
        break;
    case 3:
        if (!TEST_true(RAND_set_DRBG_type(libctx, "DUMMY",
                                          query ? "provider=dummy" : NULL,
                                          NULL, NULL))
                || !TEST_int_ge(RAND_bytes_ex(libctx, buf, sizeof(buf), 0), 1))
            goto err;
        break;
    default:
        goto err;
    }

    testresult = 1;
 err:
    OSSL_DECODER_free(decoder);
    OSSL_ENCODER_free(encoder);
    OSSL_STORE_LOADER_free(loader);
    OSSL_PROVIDER_unload(dummyprov);
    OSSL_PROVIDER_unload(nullprov);
    OSSL_LIB_CTX_free(libctx);
    return testresult;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(fetch_test, 8);

    return 1;
}
