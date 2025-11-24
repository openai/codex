/*
 * Copyright 2016-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include <openssl/opensslconf.h>

#include <string.h>
#include <openssl/engine.h>
#include <openssl/evp.h>
#include <openssl/rand.h>
#include "testutil.h"

/* Use a buffer size which is not aligned to block size */
#define BUFFER_SIZE     17

#ifndef OPENSSL_NO_ENGINE
static ENGINE *e;

static int test_afalg_aes_cbc(int keysize_idx)
{
    EVP_CIPHER_CTX *ctx;
    const EVP_CIPHER *cipher;
    unsigned char ebuf[BUFFER_SIZE + 32];
    unsigned char dbuf[BUFFER_SIZE + 32];
    const unsigned char *enc_result = NULL;
    int encl, encf, decl, decf;
    int ret = 0;
    static const unsigned char key[] =
        "\x06\xa9\x21\x40\x36\xb8\xa1\x5b\x51\x2e\x03\xd5\x34\x12\x00\x06"
        "\x06\xa9\x21\x40\x36\xb8\xa1\x5b\x51\x2e\x03\xd5\x34\x12\x00\x06";
    static const unsigned char iv[] =
        "\x3d\xaf\xba\x42\x9d\x9e\xb4\x30\xb4\x22\xda\x80\x2c\x9f\xac\x41";
    /* input = "Single block msg\n" 17 Bytes*/
    static const unsigned char in[BUFFER_SIZE] =
        "\x53\x69\x6e\x67\x6c\x65\x20\x62\x6c\x6f\x63\x6b\x20\x6d\x73\x67"
        "\x0a";
    static const unsigned char encresult_128[BUFFER_SIZE] =
        "\xe3\x53\x77\x9c\x10\x79\xae\xb8\x27\x08\x94\x2d\xbe\x77\x18\x1a"
        "\x2d";
    static const unsigned char encresult_192[BUFFER_SIZE] =
        "\xf7\xe4\x26\xd1\xd5\x4f\x8f\x39\xb1\x9e\xe0\xdf\x61\xb9\xc2\x55"
        "\xeb";
    static const unsigned char encresult_256[BUFFER_SIZE] =
        "\xa0\x76\x85\xfd\xc1\x65\x71\x9d\xc7\xe9\x13\x6e\xae\x55\x49\xb4"
        "\x13";

#ifdef OSSL_SANITIZE_MEMORY
    /*
     * Initialise the encryption & decryption buffers to pacify the memory
     * sanitiser.  The sanitiser doesn't know that this memory is modified
     * by the engine, this tells it that all is good.
     */
    OPENSSL_cleanse(ebuf, sizeof(ebuf));
    OPENSSL_cleanse(dbuf, sizeof(dbuf));
#endif

    switch (keysize_idx) {
        case 0:
            cipher = EVP_aes_128_cbc();
            enc_result = &encresult_128[0];
            break;
        case 1:
            cipher = EVP_aes_192_cbc();
            enc_result = &encresult_192[0];
            break;
        case 2:
            cipher = EVP_aes_256_cbc();
            enc_result = &encresult_256[0];
            break;
        default:
            cipher = NULL;
    }
    if (!TEST_ptr(ctx = EVP_CIPHER_CTX_new()))
            return 0;

    if (!TEST_true(EVP_CipherInit_ex(ctx, cipher, e, key, iv, 1))
            || !TEST_true(EVP_CipherUpdate(ctx, ebuf, &encl, in, BUFFER_SIZE))
            || !TEST_true(EVP_CipherFinal_ex(ctx, ebuf + encl, &encf)))
        goto end;
    encl += encf;

    if (!TEST_mem_eq(enc_result, BUFFER_SIZE, ebuf, BUFFER_SIZE))
        goto end;

    if (!TEST_true(EVP_CIPHER_CTX_reset(ctx))
            || !TEST_true(EVP_CipherInit_ex(ctx, cipher, e, key, iv, 0))
            || !TEST_true(EVP_CipherUpdate(ctx, dbuf, &decl, ebuf, encl))
            || !TEST_true(EVP_CipherFinal_ex(ctx, dbuf + decl, &decf)))
        goto end;
    decl += decf;

    if (!TEST_int_eq(decl, BUFFER_SIZE)
            || !TEST_mem_eq(dbuf, BUFFER_SIZE, in, BUFFER_SIZE))
        goto end;

    ret = 1;

 end:
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int test_pr16743(void)
{
    int ret = 0;
    const EVP_CIPHER *cipher;
    EVP_CIPHER_CTX *ctx;

    if (!TEST_true(ENGINE_init(e)))
        return 0;
    cipher = ENGINE_get_cipher(e, NID_aes_128_cbc);
    ctx = EVP_CIPHER_CTX_new();
    if (cipher != NULL && ctx != NULL)
        ret = EVP_EncryptInit_ex(ctx, cipher, e, NULL, NULL);
    TEST_true(ret);
    EVP_CIPHER_CTX_free(ctx);
    ENGINE_finish(e);
    return ret;
}

int global_init(void)
{
    ENGINE_load_builtin_engines();
# ifndef OPENSSL_NO_STATIC_ENGINE
    OPENSSL_init_crypto(OPENSSL_INIT_ENGINE_AFALG, NULL);
# endif
    return 1;
}
#endif

int setup_tests(void)
{
#ifndef OPENSSL_NO_ENGINE
    if ((e = ENGINE_by_id("afalg")) == NULL) {
        /* Probably a platform env issue, not a test failure. */
        TEST_info("Can't load AFALG engine");
    } else {
        ADD_ALL_TESTS(test_afalg_aes_cbc, 3);
        ADD_TEST(test_pr16743);
    }
#endif

    return 1;
}

#ifndef OPENSSL_NO_ENGINE
void cleanup_tests(void)
{
    ENGINE_free(e);
}
#endif
