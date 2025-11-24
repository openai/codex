/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include "testutil.h"

static const unsigned char gcm_key[] = {
    0xee, 0xbc, 0x1f, 0x57, 0x48, 0x7f, 0x51, 0x92, 0x1c, 0x04, 0x65, 0x66,
    0x5f, 0x8a, 0xe6, 0xd1, 0x65, 0x8b, 0xb2, 0x6d, 0xe6, 0xf8, 0xa0, 0x69,
    0xa3, 0x52, 0x02, 0x93, 0xa5, 0x72, 0x07, 0x8f
};
static const unsigned char gcm_iv[] = {
    0x99, 0xaa, 0x3e, 0x68, 0xed, 0x81, 0x73, 0xa0, 0xee, 0xd0, 0x66, 0x84
};
static const unsigned char gcm_pt[] = {
    0xf5, 0x6e, 0x87, 0x05, 0x5b, 0xc3, 0x2d, 0x0e, 0xeb, 0x31, 0xb2, 0xea,
    0xcc, 0x2b, 0xf2, 0xa5
};
static const unsigned char gcm_aad[] = {
    0x4d, 0x23, 0xc3, 0xce, 0xc3, 0x34, 0xb4, 0x9b, 0xdb, 0x37, 0x0c, 0x43,
    0x7f, 0xec, 0x78, 0xde
};
static const unsigned char gcm_ct[] = {
    0xf7, 0x26, 0x44, 0x13, 0xa8, 0x4c, 0x0e, 0x7c, 0xd5, 0x36, 0x86, 0x7e,
    0xb9, 0xf2, 0x17, 0x36
};
static const unsigned char gcm_tag[] = {
    0x67, 0xba, 0x05, 0x10, 0x26, 0x2a, 0xe4, 0x87, 0xd7, 0x37, 0xee, 0x62,
    0x98, 0xf7, 0x7e, 0x0c
};

static int do_encrypt(unsigned char *iv_gen, unsigned char *ct, int *ct_len,
                      unsigned char *tag, int *tag_len)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    int outlen;
    unsigned char outbuf[64];

    *tag_len = 16;
    ret = TEST_ptr(ctx = EVP_CIPHER_CTX_new())
          && TEST_true(EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL,
                                          NULL) > 0)
          && TEST_true(EVP_EncryptInit_ex(ctx, NULL, NULL, gcm_key,
                                          iv_gen != NULL ? NULL : gcm_iv) > 0)
          && TEST_true(EVP_EncryptUpdate(ctx, NULL, &outlen, gcm_aad,
                                         sizeof(gcm_aad)) > 0)
          && TEST_true(EVP_EncryptUpdate(ctx, ct, ct_len, gcm_pt,
                                         sizeof(gcm_pt)) > 0)
          && TEST_true(EVP_EncryptFinal_ex(ctx, outbuf, &outlen) > 0)
          && TEST_int_eq(EVP_CIPHER_CTX_get_tag_length(ctx), 16)
          && TEST_true(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG, 16,
                                           tag) > 0)
          && TEST_true(iv_gen == NULL
                  || EVP_CIPHER_CTX_get_original_iv(ctx, iv_gen, 12));
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int do_decrypt(const unsigned char *iv, const unsigned char *ct,
                      int ct_len, const unsigned char *tag, int tag_len)
{
    int ret = 0;
    EVP_CIPHER_CTX *ctx = NULL;
    int outlen, ptlen;
    unsigned char pt[32];
    unsigned char outbuf[32];

    ret = TEST_ptr(ctx = EVP_CIPHER_CTX_new())
              && TEST_true(EVP_DecryptInit_ex(ctx, EVP_aes_256_gcm(), NULL,
                                              NULL, NULL) > 0)
              && TEST_true(EVP_DecryptInit_ex(ctx, NULL, NULL, gcm_key, iv) > 0)
              && TEST_int_eq(EVP_CIPHER_CTX_get_tag_length(ctx), 16)
              && TEST_true(EVP_DecryptUpdate(ctx, NULL, &outlen, gcm_aad,
                                             sizeof(gcm_aad)) > 0)
              && TEST_true(EVP_DecryptUpdate(ctx, pt, &ptlen, ct,
                                             ct_len) > 0)
              && TEST_true(EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                               tag_len, (void *)tag) > 0)
              && TEST_true(EVP_DecryptFinal_ex(ctx, outbuf, &outlen) > 0)
              && TEST_mem_eq(gcm_pt, sizeof(gcm_pt), pt, ptlen);

    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int kat_test(void)
{
    unsigned char tag[32];
    unsigned char ct[32];
    int ctlen = 0, taglen = 0;

    return do_encrypt(NULL, ct, &ctlen, tag, &taglen)
           && TEST_mem_eq(gcm_ct, sizeof(gcm_ct), ct, ctlen)
           && TEST_mem_eq(gcm_tag, sizeof(gcm_tag), tag, taglen)
           && do_decrypt(gcm_iv, ct, ctlen, tag, taglen);
}

static int badkeylen_test(void)
{
    int ret;
    EVP_CIPHER_CTX *ctx = NULL;
    const EVP_CIPHER *cipher;

    ret = TEST_ptr(cipher = EVP_aes_192_gcm())
          && TEST_ptr(ctx = EVP_CIPHER_CTX_new())
          && TEST_true(EVP_EncryptInit_ex(ctx, cipher, NULL, NULL, NULL))
          && TEST_int_le(EVP_CIPHER_CTX_set_key_length(ctx, 2), 0);
    EVP_CIPHER_CTX_free(ctx);
    return ret;
}

static int ivgen_test(void)
{
    unsigned char iv_gen[16];
    unsigned char tag[32];
    unsigned char ct[32];
    int ctlen = 0, taglen = 0;

    return do_encrypt(iv_gen, ct, &ctlen, tag, &taglen)
           && do_decrypt(iv_gen, ct, ctlen, tag, taglen);
}

int setup_tests(void)
{
    ADD_TEST(kat_test);
    ADD_TEST(badkeylen_test);
    ADD_TEST(ivgen_test);
    return 1;
}
