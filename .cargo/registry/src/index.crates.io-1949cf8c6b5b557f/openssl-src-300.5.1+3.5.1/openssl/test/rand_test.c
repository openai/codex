/*
 * Copyright 2021-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the >License>).  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/bio.h>
#include <openssl/core_names.h>
#include <openssl/params.h>
#include "crypto/rand.h"
#include "testutil.h"

static char *configfile;

static int test_rand(void)
{
    EVP_RAND_CTX *privctx;
    const OSSL_PROVIDER *prov;
    int indicator = 1;
    OSSL_PARAM params[2], *p = params;
    unsigned char entropy1[] = { 0x00, 0x01, 0x02, 0x03, 0x04, 0x05 };
    unsigned char entropy2[] = { 0xff, 0xfe, 0xfd };
    unsigned char nonce[] = { 0x00, 0x01, 0x02, 0x03, 0x04 };
    unsigned char outbuf[3];

    *p++ = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                             entropy1, sizeof(entropy1));
    *p = OSSL_PARAM_construct_end();

    if (!TEST_ptr(privctx = RAND_get0_private(NULL))
            || !TEST_true(EVP_RAND_CTX_set_params(privctx, params))
            || !TEST_int_gt(RAND_priv_bytes(outbuf, sizeof(outbuf)), 0)
            || !TEST_mem_eq(outbuf, sizeof(outbuf), entropy1, sizeof(outbuf))
            || !TEST_int_le(RAND_priv_bytes(outbuf, sizeof(outbuf) + 1), 0)
            || !TEST_int_gt(RAND_priv_bytes(outbuf, sizeof(outbuf)), 0)
            || !TEST_mem_eq(outbuf, sizeof(outbuf),
                            entropy1 + sizeof(outbuf), sizeof(outbuf)))
        return 0;

    *params = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                                entropy2, sizeof(entropy2));
    if (!TEST_true(EVP_RAND_CTX_set_params(privctx, params))
            || !TEST_int_gt(RAND_priv_bytes(outbuf, sizeof(outbuf)), 0)
            || !TEST_mem_eq(outbuf, sizeof(outbuf), entropy2, sizeof(outbuf)))
        return 0;

    *params = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_NONCE,
                                                nonce, sizeof(nonce));
    if (!TEST_true(EVP_RAND_CTX_set_params(privctx, params))
            || !TEST_true(EVP_RAND_nonce(privctx, outbuf, sizeof(outbuf)))
            || !TEST_mem_eq(outbuf, sizeof(outbuf), nonce, sizeof(outbuf)))
        return 0;

    if (fips_provider_version_lt(NULL, 3, 4, 0)) {
        /* Skip the rest and pass the test */
        return 1;
    }
    /* Verify that the FIPS indicator can be read and is false */
    prov = EVP_RAND_get0_provider(EVP_RAND_CTX_get0_rand(privctx));
    if (prov != NULL
            && strcmp(OSSL_PROVIDER_get0_name(prov), "fips") == 0) {
        params[0] = OSSL_PARAM_construct_int(OSSL_RAND_PARAM_FIPS_APPROVED_INDICATOR,
                                             &indicator);
        if (!TEST_true(EVP_RAND_CTX_get_params(privctx, params))
                || !TEST_int_eq(indicator, 0))
            return 0;
    }
    return 1;
}

static int test_rand_uniform(void)
{
    uint32_t x, i, j;
    int err = 0, res = 0;
    OSSL_LIB_CTX *ctx;

    if (!test_get_libctx(&ctx, NULL, NULL, NULL, NULL))
        goto err;

    for (i = 1; i < 100; i += 13) {
        x = ossl_rand_uniform_uint32(ctx, i, &err);
        if (!TEST_int_eq(err, 0)
                || !TEST_uint_ge(x, 0)
                || !TEST_uint_lt(x, i))
            return 0;
    }
    for (i = 1; i < 100; i += 17)
        for (j = i + 1; j < 150; j += 11) {
            x = ossl_rand_range_uint32(ctx, i, j, &err);
            if (!TEST_int_eq(err, 0)
                    || !TEST_uint_ge(x, i)
                    || !TEST_uint_lt(x, j))
                return 0;
        }

    res = 1;
 err:
    OSSL_LIB_CTX_free(ctx);
    return res;
}

/* Test the FIPS health tests */
static int fips_health_test_one(const uint8_t *buf, size_t n, size_t gen)
{
    int res = 0;
    EVP_RAND *crngt_alg = NULL, *parent_alg = NULL;
    EVP_RAND_CTX *crngt = NULL, *parent = NULL;
    OSSL_PARAM p[2];
    uint8_t out[1000];
    int indicator = -1;

    p[0] = OSSL_PARAM_construct_octet_string(OSSL_RAND_PARAM_TEST_ENTROPY,
                                             (void *)buf, n);
    p[1] = OSSL_PARAM_construct_end();

    if (!TEST_ptr(parent_alg = EVP_RAND_fetch(NULL, "TEST-RAND", "-fips"))
            || !TEST_ptr(crngt_alg = EVP_RAND_fetch(NULL, "CRNG-TEST", "-fips"))
            || !TEST_ptr(parent = EVP_RAND_CTX_new(parent_alg, NULL))
            || !TEST_ptr(crngt = EVP_RAND_CTX_new(crngt_alg, parent))
            || !TEST_true(EVP_RAND_instantiate(parent, 0, 0,
                                               (unsigned char *)"abc", 3, p))
            || !TEST_true(EVP_RAND_instantiate(crngt, 0, 0,
                                               (unsigned char *)"def", 3, NULL))
            || !TEST_size_t_le(gen, sizeof(out)))
        goto err;

    /* Verify that the FIPS indicator is negative */
    p[0] = OSSL_PARAM_construct_int(OSSL_RAND_PARAM_FIPS_APPROVED_INDICATOR,
                                    &indicator);
    if (!TEST_true(EVP_RAND_CTX_get_params(crngt, p))
            || !TEST_int_le(indicator, 0))
        goto err;

    ERR_set_mark();
    res = EVP_RAND_generate(crngt, out, gen, 0, 0, NULL, 0);
    ERR_pop_to_mark();
 err:
    EVP_RAND_CTX_free(crngt);
    EVP_RAND_CTX_free(parent);
    EVP_RAND_free(crngt_alg);
    EVP_RAND_free(parent_alg);
    return res;
}

static int fips_health_tests(void)
{
    uint8_t buf[1000];
    size_t i;

    /* Verify tests can pass */
    for (i = 0; i < sizeof(buf); i++)
        buf[i] = 0xff & i;
    if (!TEST_true(fips_health_test_one(buf, i, i)))
        return 0;

    /* Verify RCT can fail */
    for (i = 0; i < 20; i++)
        buf[i] = 0xff & (i > 10 ? 200 : i);
    if (!TEST_false(fips_health_test_one(buf, i, i)))
        return 0;

    /* Verify APT can fail */
    for (i = 0; i < sizeof(buf); i++)
        buf[i] = 0xff & (i >= 512 && i % 8 == 0 ? 0x80 : i);
    if (!TEST_false(fips_health_test_one(buf, i, i)))
        return 0;
    return 1;
}

typedef struct r_test_ctx {
    const OSSL_CORE_HANDLE *handle;
} R_TEST_CTX;

static void r_teardown(void *provctx)
{
    R_TEST_CTX *ctx = (R_TEST_CTX *)provctx;

    free(ctx);
}

static int r_random_bytes(ossl_unused void *vprov, ossl_unused int which,
                          void *buf, size_t n, ossl_unused unsigned int strength)
{
    while (n-- > 0)
        ((unsigned char *)buf)[n] = 0xff & n;
    return 1;
}

static const OSSL_DISPATCH r_test_table[] = {
    { OSSL_FUNC_PROVIDER_RANDOM_BYTES, (void (*)(void))r_random_bytes },
    { OSSL_FUNC_PROVIDER_TEARDOWN, (void (*)(void))r_teardown },
    OSSL_DISPATCH_END
};

static int r_init(const OSSL_CORE_HANDLE *handle,
                  ossl_unused const OSSL_DISPATCH *oin,
                  const OSSL_DISPATCH **out,
                  void **provctx)
{
    R_TEST_CTX *ctx;

    ctx = malloc(sizeof(*ctx));
    if (ctx == NULL)
        return 0;
    ctx->handle = handle;

    *provctx = (void *)ctx;
    *out = r_test_table;
    return 1;
}

static int test_rand_random_provider(void)
{
    OSSL_LIB_CTX *ctx = NULL;
    OSSL_PROVIDER *prov = NULL;
    int res = 0;
    static const unsigned char data[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 };
    unsigned char buf[sizeof(data)], privbuf[sizeof(data)];

    memset(buf, 255, sizeof(buf));
    memset(privbuf, 255, sizeof(privbuf));

    if (!test_get_libctx(&ctx, NULL, NULL, NULL, NULL)
            || !TEST_true(OSSL_PROVIDER_add_builtin(ctx, "r_prov", &r_init))
            || !TEST_ptr(prov = OSSL_PROVIDER_try_load(ctx, "r_prov", 1))
            || !TEST_true(RAND_set1_random_provider(ctx, prov))
            || !RAND_bytes_ex(ctx, buf, sizeof(buf), 256)
            || !TEST_mem_eq(buf, sizeof(buf), data, sizeof(data))
            || !RAND_priv_bytes_ex(ctx, privbuf, sizeof(privbuf), 256)
            || !TEST_mem_eq(privbuf, sizeof(privbuf), data, sizeof(data)))
        goto err;

    /* Test we can revert to not using the provider based randomness */
    if (!TEST_true(RAND_set1_random_provider(ctx, NULL))
            || !RAND_bytes_ex(ctx, buf, sizeof(buf), 256)
            || !TEST_mem_ne(buf, sizeof(buf), data, sizeof(data)))
        goto err;

    /* And back to the provided randomness */
    if (!TEST_true(RAND_set1_random_provider(ctx, prov))
            || !RAND_bytes_ex(ctx, buf, sizeof(buf), 256)
            || !TEST_mem_eq(buf, sizeof(buf), data, sizeof(data)))
        goto err;

    res = 1;
 err:
    OSSL_PROVIDER_unload(prov);
    OSSL_LIB_CTX_free(ctx);
    return res;
}

static int test_rand_get0_primary(void)
{
    OSSL_LIB_CTX *ctx = OSSL_LIB_CTX_new();
    int res = 0;

    if (!TEST_ptr(ctx))
        return 0;

    if (!TEST_true(OSSL_LIB_CTX_load_config(ctx, configfile)))
        goto err;

    /* We simply test that we get a valid primary */
    if (!TEST_ptr(RAND_get0_primary(ctx)))
        goto err;

    res = 1;
 err:
    OSSL_LIB_CTX_free(ctx);
    return res;
}

int setup_tests(void)
{
    if (!test_skip_common_options()) {
        TEST_error("Error parsing test options\n");
        return 0;
    }

    if (!TEST_ptr(configfile = test_get_argument(0))
            || !TEST_true(RAND_set_DRBG_type(NULL, "TEST-RAND", "fips=no",
                                             NULL, NULL))
            || (fips_provider_version_ge(NULL, 3, 0, 8)
                && !TEST_true(OSSL_LIB_CTX_load_config(NULL, configfile))))
        return 0;

    ADD_TEST(test_rand);
    ADD_TEST(test_rand_uniform);

    if (OSSL_PROVIDER_available(NULL, "fips")
            && fips_provider_version_ge(NULL, 3, 4, 0))
        ADD_TEST(fips_health_tests);

    ADD_TEST(test_rand_random_provider);

    if (!OSSL_PROVIDER_available(NULL, "fips")
            || fips_provider_version_ge(NULL, 3, 5, 1))
        ADD_TEST(test_rand_get0_primary);
    return 1;
}
