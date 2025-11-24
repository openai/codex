/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>

static ossl_inline ossl_unused int
shake_xof(EVP_MD_CTX *ctx, const EVP_MD *md, const uint8_t *in, size_t in_len,
          uint8_t *out, size_t out_len)
{
    return (EVP_DigestInit_ex2(ctx, md, NULL) == 1
            && EVP_DigestUpdate(ctx, in, in_len) == 1
            && EVP_DigestSqueeze(ctx, out, out_len) == 1);
}

static ossl_inline ossl_unused int
shake_xof_2(EVP_MD_CTX *ctx, const EVP_MD *md, const uint8_t *in1, size_t in1_len,
            const uint8_t *in2, size_t in2_len, uint8_t *out, size_t out_len)
{
    return EVP_DigestInit_ex2(ctx, md, NULL)
        && EVP_DigestUpdate(ctx, in1, in1_len)
        && EVP_DigestUpdate(ctx, in2, in2_len)
        && EVP_DigestSqueeze(ctx, out, out_len);
}

static ossl_inline ossl_unused int
shake_xof_3(EVP_MD_CTX *ctx, const EVP_MD *md, const uint8_t *in1, size_t in1_len,
            const uint8_t *in2, size_t in2_len,
            const uint8_t *in3, size_t in3_len, uint8_t *out, size_t out_len)
{
    return EVP_DigestInit_ex2(ctx, md, NULL)
        && EVP_DigestUpdate(ctx, in1, in1_len)
        && EVP_DigestUpdate(ctx, in2, in2_len)
        && EVP_DigestUpdate(ctx, in3, in3_len)
        && EVP_DigestSqueeze(ctx, out, out_len);
}
