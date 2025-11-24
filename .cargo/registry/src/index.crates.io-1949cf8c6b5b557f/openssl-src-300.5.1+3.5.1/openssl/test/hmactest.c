/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * HMAC low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <string.h>
#include <stdlib.h>

#include "internal/nelem.h"

# include <openssl/hmac.h>
# include <openssl/sha.h>
# ifndef OPENSSL_NO_MD5
#  include <openssl/md5.h>
# endif

# ifdef CHARSET_EBCDIC
#  include <openssl/ebcdic.h>
# endif

#include "testutil.h"

# ifndef OPENSSL_NO_MD5
static struct test_st {
    const char key[16];
    int key_len;
    const unsigned char data[64];
    int data_len;
    const char *digest;
} test[8] = {
    {
        "", 0, "More text test vectors to stuff up EBCDIC machines :-)", 54,
        "e9139d1e6ee064ef8cf514fc7dc83e86",
    },
    {
        "\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b\x0b",
        16, "Hi There", 8,
        "9294727a3638bb1c13f48ef8158bfc9d",
    },
    {
        "Jefe", 4, "what do ya want for nothing?", 28,
        "750c783e6ab0b503eaa86e310a5db738",
    },
    {
        "\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa\xaa",
        16, {
            0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd,
            0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd,
            0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd,
            0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd,
            0xdd, 0xdd, 0xdd, 0xdd, 0xdd, 0xdd
        }, 50, "56be34521d144c88dbb8c733f0e8b3f6",
    },
    {
        "", 0, "My test data", 12,
        "61afdecb95429ef494d61fdee15990cabf0826fc"
    },
    {
        "", 0, "My test data", 12,
        "2274b195d90ce8e03406f4b526a47e0787a88a65479938f1a5baa3ce0f079776"
    },
    {
        "123456", 6, "My test data", 12,
        "bab53058ae861a7f191abe2d0145cbb123776a6369ee3f9d79ce455667e411dd"
    },
    {
        "12345", 5, "My test data again", 18,
        "a12396ceddd2a85f4c656bc1e0aa50c78cffde3e"
    }
};
# endif

static char *pt(unsigned char *md, unsigned int len);


# ifndef OPENSSL_NO_MD5
static int test_hmac_md5(int idx)
{
    char *p;
#  ifdef CHARSET_EBCDIC
    ebcdic2ascii(test[0].data, test[0].data, test[0].data_len);
    ebcdic2ascii(test[1].data, test[1].data, test[1].data_len);
    ebcdic2ascii(test[2].key, test[2].key, test[2].key_len);
    ebcdic2ascii(test[2].data, test[2].data, test[2].data_len);
#  endif

    p = pt(HMAC(EVP_md5(),
                test[idx].key, test[idx].key_len,
                test[idx].data, test[idx].data_len, NULL, NULL),
                MD5_DIGEST_LENGTH);

    return TEST_ptr(p) && TEST_str_eq(p, test[idx].digest);
}
# endif

static int test_hmac_bad(void)
{
    HMAC_CTX *ctx = NULL;
    int ret = 0;

    ctx = HMAC_CTX_new();
    if (!TEST_ptr(ctx)
        || !TEST_ptr_null(HMAC_CTX_get_md(ctx))
        || !TEST_false(HMAC_Init_ex(ctx, NULL, 0, NULL, NULL))
        || !TEST_false(HMAC_Update(ctx, test[4].data, test[4].data_len))
        || !TEST_false(HMAC_Init_ex(ctx, NULL, 0, EVP_sha1(), NULL))
        || !TEST_false(HMAC_Update(ctx, test[4].data, test[4].data_len)))
        goto err;

    ret = 1;
err:
    HMAC_CTX_free(ctx);
    return ret;
}

static int test_hmac_run(void)
{
    char *p;
    HMAC_CTX *ctx = NULL;
    unsigned char buf[EVP_MAX_MD_SIZE];
    unsigned int len;
    int ret = 0;

    if (!TEST_ptr(ctx = HMAC_CTX_new()))
        return 0;
    HMAC_CTX_reset(ctx);

    if (!TEST_ptr(ctx)
        || !TEST_ptr_null(HMAC_CTX_get_md(ctx))
        || !TEST_false(HMAC_Init_ex(ctx, NULL, 0, NULL, NULL))
        || !TEST_false(HMAC_Update(ctx, test[4].data, test[4].data_len))
        || !TEST_false(HMAC_Init_ex(ctx, test[4].key, -1, EVP_sha1(), NULL)))
        goto err;

    if (!TEST_true(HMAC_Init_ex(ctx, test[4].key, test[4].key_len, EVP_sha1(), NULL))
        || !TEST_true(HMAC_Update(ctx, test[4].data, test[4].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;

    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[4].digest))
        goto err;

    if (!TEST_false(HMAC_Init_ex(ctx, NULL, 0, EVP_sha256(), NULL)))
        goto err;

    if (!TEST_true(HMAC_Init_ex(ctx, test[5].key, test[5].key_len, EVP_sha256(), NULL))
        || !TEST_ptr_eq(HMAC_CTX_get_md(ctx), EVP_sha256())
        || !TEST_true(HMAC_Update(ctx, test[5].data, test[5].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;

    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[5].digest))
        goto err;

    if (!TEST_true(HMAC_Init_ex(ctx, test[6].key, test[6].key_len, NULL, NULL))
        || !TEST_true(HMAC_Update(ctx, test[6].data, test[6].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;
    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[6].digest))
        goto err;

    /* Test reusing a key */
    if (!TEST_true(HMAC_Init_ex(ctx, NULL, 0, NULL, NULL))
        || !TEST_true(HMAC_Update(ctx, test[6].data, test[6].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;
    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[6].digest))
        goto err;

    /*
     * Test reusing a key where the digest is provided again but is the same as
     * last time
     */
    if (!TEST_true(HMAC_Init_ex(ctx, NULL, 0, EVP_sha256(), NULL))
        || !TEST_true(HMAC_Update(ctx, test[6].data, test[6].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;
    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[6].digest))
        goto err;

    ret = 1;
err:
    HMAC_CTX_free(ctx);
    return ret;
}


static int test_hmac_single_shot(void)
{
    char *p;

    /* Test single-shot with NULL key. */
    p = pt(HMAC(EVP_sha1(), NULL, 0, test[4].data, test[4].data_len,
                NULL, NULL), SHA_DIGEST_LENGTH);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[4].digest))
        return 0;

    return 1;
}

/* https://github.com/openssl/openssl/issues/13210 */
static int test_hmac_final_update_fail(void)
{
    HMAC_CTX *ctx = NULL;
    unsigned char buf[EVP_MAX_MD_SIZE];
    unsigned int len;
    int ret = 0;

    /* HMAC_Update() after HMAC_Final() must return an error. */
    if (!TEST_ptr(ctx = HMAC_CTX_new()))
        goto err;
    if (!TEST_true(HMAC_Init_ex(ctx, test[5].key, test[5].key_len, EVP_sha256(), NULL))
        || !TEST_true(HMAC_Update(ctx, test[5].data, test[5].data_len))
        || !TEST_true(HMAC_Final(ctx, buf, &len))
        || !TEST_false(HMAC_Update(ctx, test[5].data, test[5].data_len))
        || !TEST_false(HMAC_Final(ctx, buf, &len)))
        goto err;

    ret = 1;
err:
    HMAC_CTX_free(ctx);
    return ret;
}

static int test_hmac_copy(void)
{
    char *p;
    HMAC_CTX *ctx = NULL, *ctx2 = NULL;
    unsigned char buf[EVP_MAX_MD_SIZE];
    unsigned int len;
    int ret = 0;

    ctx = HMAC_CTX_new();
    ctx2 = HMAC_CTX_new();
    if (!TEST_ptr(ctx) || !TEST_ptr(ctx2))
        goto err;

    if (!TEST_true(HMAC_Init_ex(ctx, test[7].key, test[7].key_len, EVP_sha1(), NULL))
        || !TEST_true(HMAC_Update(ctx, test[7].data, test[7].data_len))
        || !TEST_true(HMAC_CTX_copy(ctx2, ctx))
        || !TEST_true(HMAC_Final(ctx2, buf, &len)))
        goto err;

    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test[7].digest))
        goto err;

    ret = 1;
err:
    HMAC_CTX_free(ctx2);
    HMAC_CTX_free(ctx);
    return ret;
}

static int test_hmac_copy_uninited(void)
{
    const unsigned char key[24] = {0};
    const unsigned char ct[166] = {0};
    EVP_PKEY *pkey = NULL;
    EVP_MD_CTX *ctx = NULL;
    EVP_MD_CTX *ctx_tmp = NULL;
    int res = 0;

    if (!TEST_ptr(ctx = EVP_MD_CTX_new())
            || !TEST_ptr(pkey = EVP_PKEY_new_mac_key(EVP_PKEY_HMAC, NULL,
                                                     key, sizeof(key)))
            || !TEST_true(EVP_DigestSignInit(ctx, NULL, EVP_sha1(), NULL, pkey))
            || !TEST_ptr(ctx_tmp = EVP_MD_CTX_new())
            || !TEST_true(EVP_MD_CTX_copy(ctx_tmp, ctx)))
        goto err;
    EVP_MD_CTX_free(ctx);
    ctx = ctx_tmp;
    ctx_tmp = NULL;

    if (!TEST_true(EVP_DigestSignUpdate(ctx, ct, sizeof(ct))))
        goto err;
    res = 1;
 err:
    EVP_MD_CTX_free(ctx);
    EVP_MD_CTX_free(ctx_tmp);
    EVP_PKEY_free(pkey);
    return res;
}

#ifndef OPENSSL_NO_MD5
# define OSSL_HEX_CHARS_PER_BYTE 2
static char *pt(unsigned char *md, unsigned int len)
{
    unsigned int i;
    static char buf[201];

    if (md == NULL)
        return NULL;
    for (i = 0; i < len && (i + 1) * OSSL_HEX_CHARS_PER_BYTE < sizeof(buf); i++)
        BIO_snprintf(buf + i * OSSL_HEX_CHARS_PER_BYTE,
                     OSSL_HEX_CHARS_PER_BYTE + 1, "%02x", md[i]);
    return buf;
}
#endif

static struct test_chunks_st {
    const char *md_name;
    char key[256];
    int key_len;
    int chunks;
    int chunk_size[10];
    const char *digest;
} test_chunks[12] = {
    {
        "SHA224",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 64,
        4, { 1, 50, 200, 4000 },
        "40821a39dd54f01443b3f96b9370a15023fbdd819a074ffc4b703c77"
    },
    {
        "SHA224",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 192,
        10, { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 },
        "55ffa85e53e9a68f41c8d653c60b4ada9566d22aed3811834882661c"
    },
    {
        "SHA224", "0123456789abcdef0123456789abcdef", 32,
        4, { 100, 4096, 100, 3896 },
        "0fd18e7d8e974f401b29bf0502a71f6a9b77804e9191380ce9f48377"
    },
    {
        "SHA256",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 64,
        4, { 1, 50, 200, 4000 },
        "f67a46fa77c66d3ea5b3ffb9a10afb3e501eaadd16b15978fdee9f014a782140"
    },
    {
        "SHA256",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 192,
        10, { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 },
        "21a6f61ed6dbec30b58557a80988ff610d69b50b2e96d75863ab50f99da58c9d"
    },
    {
        "SHA256", "0123456789abcdef0123456789abcdef", 32,
        4, { 100, 4096, 100, 3896 },
        "7bfd45c1bdde9b79244816b0aea0a67ea954a182e74c60410bfbc1fdc4842660"
    },
    {
        "SHA384",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 64,
        4, { 1, 50, 200, 4000 },
        "e270e3c8ca3f2796a0c29cc7569fcec7584b04db26da64326aca0d17bd7731de"
        "938694b273f3dafe6e2dc123cde26640"
    },
    {
        "SHA384",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 192,
        10, { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 },
        "7036fd7d251298975acd18938471243e92fffe67be158f16c910c400576592d2"
        "618c3c077ef25d703312668bd2d813ff"
    },
    {
        "SHA384", "0123456789abcdef0123456789abcdef", 32,
        4, { 100, 8192, 100, 8092 },
        "0af8224145bd0812d2e34ba1f980ed4d218461271a54cce75dc43d36eda01e4e"
        "ff4299c1ebf533a7ae636fa3e6aff903"
    },
    {
        "SHA512",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 64,
        4, { 1, 50, 200, 4000 },
        "4016e960e2342553d4b9d34fb57355ab8b7f33af5dc2676fc1189e94b38f2b2c"
        "a0ec8dc3c8b95fb1109d58480cea1e8f88e02f34ad79b303e4809373c46c1b16"
    },
    {
        "SHA512",
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", 192,
        10, { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 },
        "7ceb6a421fc19434bcb7ec9c8a15ea524dbfb896c24f5f517513f06597de99b1"
        "918eb6b2472e52215ec7d1b5544766f79ff6ac6d1eb456f19a93819fa2d43c29"
    },
    {
        "SHA512", "0123456789abcdef0123456789abcdef", 32,
        4, { 100, 8192, 100, 8092 },
        "cebf722ffdff5f0e4cbfbd480cd086101d4627d30d42f1f7cf21c43251018069"
        "854d8e030b5a54cec1e2245d5b4629ff928806d4eababb427d751ec7c274047f"
    },
};

static int test_hmac_chunks(int idx)
{
    char *p;
    HMAC_CTX *ctx = NULL;
    unsigned char buf[32768];
    unsigned int len;
    const EVP_MD *md;
    int i, ret = 0;

    if (!TEST_ptr(md = EVP_get_digestbyname(test_chunks[idx].md_name)))
        goto err;

    if (!TEST_ptr(ctx = HMAC_CTX_new()))
        goto err;

#ifdef CHARSET_EBCDIC
    ebcdic2ascii(test_chunks[idx].key, test_chunks[idx].key,
                 test_chunks[idx].key_len);
#endif

    if (!TEST_true(HMAC_Init_ex(ctx, test_chunks[idx].key,
                                test_chunks[idx].key_len, md, NULL)))
        goto err;

    for (i = 0; i < test_chunks[idx].chunks; i++) {
        if (!TEST_true((test_chunks[idx].chunk_size[i] < (int)sizeof(buf))))
            goto err;
        memset(buf, i, test_chunks[idx].chunk_size[i]);
        if (!TEST_true(HMAC_Update(ctx, buf, test_chunks[idx].chunk_size[i])))
            goto err;
    }

    if (!TEST_true(HMAC_Final(ctx, buf, &len)))
        goto err;

    p = pt(buf, len);
    if (!TEST_ptr(p) || !TEST_str_eq(p, test_chunks[idx].digest))
        goto err;

    ret = 1;

err:
    HMAC_CTX_free(ctx);
    return ret;
}

int setup_tests(void)
{
    ADD_ALL_TESTS(test_hmac_md5, 4);
    ADD_TEST(test_hmac_single_shot);
    ADD_TEST(test_hmac_bad);
    ADD_TEST(test_hmac_run);
    ADD_TEST(test_hmac_final_update_fail);
    ADD_TEST(test_hmac_copy);
    ADD_TEST(test_hmac_copy_uninited);
    ADD_ALL_TESTS(test_hmac_chunks,
                  sizeof(test_chunks) / sizeof(struct test_chunks_st));
    return 1;
}

