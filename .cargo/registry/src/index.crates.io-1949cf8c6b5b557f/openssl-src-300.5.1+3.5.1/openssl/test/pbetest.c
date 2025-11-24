/*
 * Copyright 2021-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>

#include "testutil.h"

#include <openssl/evp.h>
#include <openssl/x509.h>
#include <openssl/rc4.h>
#include <openssl/md5.h>
#include <openssl/configuration.h>
#include <openssl/provider.h>

#if !defined OPENSSL_NO_RC4 && !defined OPENSSL_NO_MD5 \
    || !defined OPENSSL_NO_DES && !defined OPENSSL_NO_SHA1
static const char pbe_password[] = "MyVoiceIsMyPassport";

static unsigned char pbe_salt[] = {
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
};

static const int pbe_iter = 1000;

static unsigned char pbe_plaintext[] = {
    0x57, 0x65, 0x20, 0x61, 0x72, 0x65, 0x20, 0x61,
    0x6c, 0x6c, 0x20, 0x6d, 0x61, 0x64, 0x65, 0x20,
    0x6f, 0x66, 0x20, 0x73, 0x74, 0x61, 0x72, 0x73,
};
#endif

/* Expected output generated using OpenSSL 1.1.1 */

#if !defined OPENSSL_NO_RC4 && !defined OPENSSL_NO_MD5
static const unsigned char pbe_ciphertext_rc4_md5[] = {
    0x21, 0x90, 0xfa, 0xee, 0x95, 0x66, 0x59, 0x45,
    0xfa, 0x1e, 0x9f, 0xe2, 0x25, 0xd2, 0xf9, 0x71,
    0x94, 0xe4, 0x3d, 0xc9, 0x7c, 0xb0, 0x07, 0x23,
};
#endif

#if !defined OPENSSL_NO_DES && !defined OPENSSL_NO_SHA1
static const unsigned char pbe_ciphertext_des_sha1[] = {
    0xce, 0x4b, 0xb0, 0x0a, 0x7b, 0x48, 0xd7, 0xe3,
    0x9a, 0x9f, 0x46, 0xd6, 0x41, 0x42, 0x4b, 0x44,
    0x36, 0x45, 0x5f, 0x60, 0x8f, 0x3c, 0xd0, 0x55,
    0xd0, 0x8d, 0xa9, 0xab, 0x78, 0x5b, 0x63, 0xaf,
};
#endif

#if !defined OPENSSL_NO_RC4 && !defined OPENSSL_NO_MD5 \
    || !defined OPENSSL_NO_DES && !defined OPENSSL_NO_SHA1
static int test_pkcs5_pbe(const EVP_CIPHER *cipher, const EVP_MD *md,
                          const unsigned char *exp, const int exp_len)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx;
    X509_ALGOR *algor = NULL;
    int i, outlen;
    unsigned char out[32];

    ctx = EVP_CIPHER_CTX_new();
    if (!TEST_ptr(ctx))
        goto err;

    algor = X509_ALGOR_new();
    if (!TEST_ptr(algor))
        goto err;

    if (!TEST_true(PKCS5_pbe_set0_algor(algor, EVP_CIPHER_nid(cipher), pbe_iter,
                                        pbe_salt, sizeof(pbe_salt)))
        || !TEST_true(PKCS5_PBE_keyivgen(ctx, pbe_password, strlen(pbe_password),
                                          algor->parameter, cipher, md, 1))
        || !TEST_true(EVP_CipherUpdate(ctx, out, &i, pbe_plaintext,
                                       sizeof(pbe_plaintext))))
        goto err;
    outlen = i;

    if (!TEST_true(EVP_CipherFinal_ex(ctx, out + i, &i)))
        goto err;
    outlen += i;

    if (!TEST_mem_eq(out, outlen, exp, exp_len))
        goto err;

    /* Decrypt */

    if (!TEST_true(PKCS5_PBE_keyivgen(ctx, pbe_password, strlen(pbe_password),
                                          algor->parameter, cipher, md, 0))
        || !TEST_true(EVP_CipherUpdate(ctx, out, &i, exp, exp_len)))
        goto err;

    outlen = i;
    if (!TEST_true(EVP_CipherFinal_ex(ctx, out + i, &i)))
        goto err;

    if (!TEST_mem_eq(out, outlen, pbe_plaintext, sizeof(pbe_plaintext)))
        goto err;

    ret = 1;
err:
    EVP_CIPHER_CTX_free(ctx);
    X509_ALGOR_free(algor);
    return ret;
}
#endif

#if !defined OPENSSL_NO_RC4 && !defined OPENSSL_NO_MD5
static int test_pkcs5_pbe_rc4_md5(void)
{
    return test_pkcs5_pbe(EVP_rc4(), EVP_md5(), pbe_ciphertext_rc4_md5, sizeof(pbe_ciphertext_rc4_md5));
}
#endif

#if !defined OPENSSL_NO_DES && !defined OPENSSL_NO_SHA1
static int test_pkcs5_pbe_des_sha1(void)
{
    return test_pkcs5_pbe(EVP_des_cbc(), EVP_sha1(), pbe_ciphertext_des_sha1, sizeof(pbe_ciphertext_des_sha1));
}
#endif

#ifdef OPENSSL_NO_AUTOLOAD_CONFIG
/*
 * For configurations where we are not autoloading configuration, we need
 * to access the legacy provider.  The easiest way is to load both the
 * legacy and default providers directly and unload them on termination.
 */
static OSSL_PROVIDER *legacy, *dflt;
#endif

int setup_tests(void)
{
#ifdef OPENSSL_NO_AUTOLOAD_CONFIG
    /* Load required providers if not done via configuration */
    legacy = OSSL_PROVIDER_load(NULL, "legacy");
    dflt = OSSL_PROVIDER_load(NULL, "default");
    if (!TEST_ptr(legacy) || !TEST_ptr(dflt)) {
        cleanup_tests();
        return -1;
    }
#endif

#if !defined OPENSSL_NO_RC4 && !defined OPENSSL_NO_MD5
    ADD_TEST(test_pkcs5_pbe_rc4_md5);
#endif
#if !defined OPENSSL_NO_DES && !defined OPENSSL_NO_SHA1
    ADD_TEST(test_pkcs5_pbe_des_sha1);
#endif

    return 1;
}

#ifdef OPENSSL_NO_AUTOLOAD_CONFIG
void cleanup_tests(void)
{
    /* Dispose of providers */
    OSSL_PROVIDER_unload(legacy);
    OSSL_PROVIDER_unload(dflt);
    legacy = dflt = NULL;
}
#endif
