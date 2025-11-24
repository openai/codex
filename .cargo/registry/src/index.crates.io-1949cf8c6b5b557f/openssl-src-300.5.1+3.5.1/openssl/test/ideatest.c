/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * IDEA low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include <string.h>

#include "internal/nelem.h"
#include "testutil.h"

#ifndef OPENSSL_NO_IDEA
# include <openssl/idea.h>

static const unsigned char k[16] = {
    0x00, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x04,
    0x00, 0x05, 0x00, 0x06, 0x00, 0x07, 0x00, 0x08
};

static const  unsigned char in[8] = { 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x03 };
static const unsigned char c[8] = { 0x11, 0xFB, 0xED, 0x2B, 0x01, 0x98, 0x6D, 0xE5 };

static unsigned char out[80];

static const unsigned char text[] = "Hello to all people out there";

static const unsigned char cfb_key[16] = {
    0xe1, 0xf0, 0xc3, 0xd2, 0xa5, 0xb4, 0x87, 0x96,
    0x69, 0x78, 0x4b, 0x5a, 0x2d, 0x3c, 0x0f, 0x1e,
};
static const unsigned char cfb_iv[80] = {
    0x34, 0x12, 0x78, 0x56, 0xab, 0x90, 0xef, 0xcd
};
static unsigned char cfb_buf1[40], cfb_buf2[40], cfb_tmp[8];
# define CFB_TEST_SIZE 24
static const unsigned char plain[CFB_TEST_SIZE] = {
    0x4e, 0x6f, 0x77, 0x20, 0x69, 0x73,
    0x20, 0x74, 0x68, 0x65, 0x20, 0x74,
    0x69, 0x6d, 0x65, 0x20, 0x66, 0x6f,
    0x72, 0x20, 0x61, 0x6c, 0x6c, 0x20
};

static const unsigned char cfb_cipher64[CFB_TEST_SIZE] = {
    0x59, 0xD8, 0xE2, 0x65, 0x00, 0x58, 0x6C, 0x3F,
    0x2C, 0x17, 0x25, 0xD0, 0x1A, 0x38, 0xB7, 0x2A,
    0x39, 0x61, 0x37, 0xDC, 0x79, 0xFB, 0x9F, 0x45
/*- 0xF9,0x78,0x32,0xB5,0x42,0x1A,0x6B,0x38,
    0x9A,0x44,0xD6,0x04,0x19,0x43,0xC4,0xD9,
    0x3D,0x1E,0xAE,0x47,0xFC,0xCF,0x29,0x0B,*/
};

static int test_idea_ecb(void)
{
    IDEA_KEY_SCHEDULE key, dkey;

    IDEA_set_encrypt_key(k, &key);
    IDEA_ecb_encrypt(in, out, &key);
    if (!TEST_mem_eq(out, IDEA_BLOCK, c, sizeof(c)))
        return 0;

    IDEA_set_decrypt_key(&key, &dkey);
    IDEA_ecb_encrypt(c, out, &dkey);
    return TEST_mem_eq(out, IDEA_BLOCK, in, sizeof(in));
}

static int test_idea_cbc(void)
{
    IDEA_KEY_SCHEDULE key, dkey;
    unsigned char iv[IDEA_BLOCK];
    const size_t text_len = sizeof(text);

    IDEA_set_encrypt_key(k, &key);
    IDEA_set_decrypt_key(&key, &dkey);
    memcpy(iv, k, sizeof(iv));
    IDEA_cbc_encrypt(text, out, text_len, &key, iv, 1);
    memcpy(iv, k, sizeof(iv));
    IDEA_cbc_encrypt(out, out, IDEA_BLOCK, &dkey, iv, 0);
    IDEA_cbc_encrypt(&out[8], &out[8], text_len - 8, &dkey, iv, 0);
    return TEST_mem_eq(text, text_len, out, text_len);
}

static int test_idea_cfb64(void)
{
    IDEA_KEY_SCHEDULE eks, dks;
    int n;

    IDEA_set_encrypt_key(cfb_key, &eks);
    IDEA_set_decrypt_key(&eks, &dks);
    memcpy(cfb_tmp, cfb_iv, sizeof(cfb_tmp));
    n = 0;
    IDEA_cfb64_encrypt(plain, cfb_buf1, (long)12, &eks,
                       cfb_tmp, &n, IDEA_ENCRYPT);
    IDEA_cfb64_encrypt(&plain[12], &cfb_buf1[12],
                       (long)CFB_TEST_SIZE - 12, &eks,
                       cfb_tmp, &n, IDEA_ENCRYPT);
    if (!TEST_mem_eq(cfb_cipher64, CFB_TEST_SIZE, cfb_buf1, CFB_TEST_SIZE))
        return 0;
    memcpy(cfb_tmp, cfb_iv, sizeof(cfb_tmp));
    n = 0;
    IDEA_cfb64_encrypt(cfb_buf1, cfb_buf2, (long)13, &eks,
                       cfb_tmp, &n, IDEA_DECRYPT);
    IDEA_cfb64_encrypt(&cfb_buf1[13], &cfb_buf2[13],
                       (long)CFB_TEST_SIZE - 13, &eks,
                       cfb_tmp, &n, IDEA_DECRYPT);
    return TEST_mem_eq(plain, CFB_TEST_SIZE, cfb_buf2, CFB_TEST_SIZE);
}
#endif

int setup_tests(void)
{
#ifndef OPENSSL_NO_IDEA
    ADD_TEST(test_idea_ecb);
    ADD_TEST(test_idea_cbc);
    ADD_TEST(test_idea_cfb64);
#endif
    return 1;
}
