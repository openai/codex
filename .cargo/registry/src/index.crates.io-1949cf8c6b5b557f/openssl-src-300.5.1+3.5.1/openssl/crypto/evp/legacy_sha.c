/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * All SHA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/sha.h>         /* diverse SHA macros */
#include "internal/sha3.h"       /* KECCAK1600_WIDTH */
#include "crypto/evp.h"
/* Used by legacy methods */
#include "crypto/sha.h"
#include "legacy_meth.h"
#include "evp_local.h"

/*-
 * LEGACY methods for SHA.
 * These only remain to support engines that can get these methods.
 * Hardware support for SHA3 has been removed from these legacy cases.
 */
#define IMPLEMENT_LEGACY_EVP_MD_METH_SHA3(nm, fn, tag)                         \
static int nm##_init(EVP_MD_CTX *ctx)                                          \
{                                                                              \
    return fn##_init(EVP_MD_CTX_get0_md_data(ctx), tag, ctx->digest->md_size * 8); \
}                                                                              \
static int nm##_update(EVP_MD_CTX *ctx, const void *data, size_t count)        \
{                                                                              \
    return fn##_update(EVP_MD_CTX_get0_md_data(ctx), data, count);             \
}                                                                              \
static int nm##_final(EVP_MD_CTX *ctx, unsigned char *md)                      \
{                                                                              \
    KECCAK1600_CTX *kctx = EVP_MD_CTX_get0_md_data(ctx);                       \
    return fn##_final(kctx, md, kctx->md_size);                                \
}
#define IMPLEMENT_LEGACY_EVP_MD_METH_SHAKE(nm, fn, tag)                        \
static int nm##_init(EVP_MD_CTX *ctx)                                          \
{                                                                              \
    return fn##_init(EVP_MD_CTX_get0_md_data(ctx), tag, ctx->digest->md_size * 8); \
}                                                                              \

#define sha512_224_Init    sha512_224_init
#define sha512_256_Init    sha512_256_init

#define sha512_224_Update  SHA512_Update
#define sha512_224_Final   SHA512_Final
#define sha512_256_Update  SHA512_Update
#define sha512_256_Final   SHA512_Final

IMPLEMENT_LEGACY_EVP_MD_METH(sha1, SHA1)
IMPLEMENT_LEGACY_EVP_MD_METH(sha224, SHA224)
IMPLEMENT_LEGACY_EVP_MD_METH(sha256, SHA256)
IMPLEMENT_LEGACY_EVP_MD_METH(sha384, SHA384)
IMPLEMENT_LEGACY_EVP_MD_METH(sha512, SHA512)
IMPLEMENT_LEGACY_EVP_MD_METH(sha512_224_int, sha512_224)
IMPLEMENT_LEGACY_EVP_MD_METH(sha512_256_int, sha512_256)
IMPLEMENT_LEGACY_EVP_MD_METH_SHA3(sha3_int, ossl_sha3, '\x06')
IMPLEMENT_LEGACY_EVP_MD_METH_SHAKE(shake, ossl_sha3, '\x1f')

static int sha1_int_ctrl(EVP_MD_CTX *ctx, int cmd, int p1, void *p2)
{
    return ossl_sha1_ctrl(ctx != NULL ? EVP_MD_CTX_get0_md_data(ctx) : NULL,
                          cmd, p1, p2);
}

static int shake_ctrl(EVP_MD_CTX *evp_ctx, int cmd, int p1, void *p2)
{
    KECCAK1600_CTX *ctx;

    if (evp_ctx == NULL)
        return 0;
    ctx = evp_ctx->md_data;

    switch (cmd) {
    case EVP_MD_CTRL_XOF_LEN:
        ctx->md_size = p1;
        return 1;
    default:
        return 0;
    }
}



static const EVP_MD sha1_md = {
    NID_sha1,
    NID_sha1WithRSAEncryption,
    SHA_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha1_init, sha1_update, sha1_final, sha1_int_ctrl,
                             SHA_CBLOCK),
};

const EVP_MD *EVP_sha1(void)
{
    return &sha1_md;
}

static const EVP_MD sha224_md = {
    NID_sha224,
    NID_sha224WithRSAEncryption,
    SHA224_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha224_init, sha224_update, sha224_final, NULL,
                             SHA256_CBLOCK),
};

const EVP_MD *EVP_sha224(void)
{
    return &sha224_md;
}

static const EVP_MD sha256_md = {
    NID_sha256,
    NID_sha256WithRSAEncryption,
    SHA256_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha256_init, sha256_update, sha256_final, NULL,
                             SHA256_CBLOCK),
};

const EVP_MD *EVP_sha256(void)
{
    return &sha256_md;
}

static const EVP_MD sha512_224_md = {
    NID_sha512_224,
    NID_sha512_224WithRSAEncryption,
    SHA224_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha512_224_int_init, sha512_224_int_update,
                             sha512_224_int_final, NULL, SHA512_CBLOCK),
};

const EVP_MD *EVP_sha512_224(void)
{
    return &sha512_224_md;
}

static const EVP_MD sha512_256_md = {
    NID_sha512_256,
    NID_sha512_256WithRSAEncryption,
    SHA256_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha512_256_int_init, sha512_256_int_update,
                             sha512_256_int_final, NULL, SHA512_CBLOCK),
};

const EVP_MD *EVP_sha512_256(void)
{
    return &sha512_256_md;
}

static const EVP_MD sha384_md = {
    NID_sha384,
    NID_sha384WithRSAEncryption,
    SHA384_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha384_init, sha384_update, sha384_final, NULL,
                             SHA512_CBLOCK),
};

const EVP_MD *EVP_sha384(void)
{
    return &sha384_md;
}

static const EVP_MD sha512_md = {
    NID_sha512,
    NID_sha512WithRSAEncryption,
    SHA512_DIGEST_LENGTH,
    EVP_MD_FLAG_DIGALGID_ABSENT,
    EVP_ORIG_GLOBAL,
    LEGACY_EVP_MD_METH_TABLE(sha512_init, sha512_update, sha512_final, NULL,
                             SHA512_CBLOCK),
};

const EVP_MD *EVP_sha512(void)
{
    return &sha512_md;
}

#define EVP_MD_SHA3(bitlen)                                                    \
const EVP_MD *EVP_sha3_##bitlen(void)                                          \
{                                                                              \
    static const EVP_MD sha3_##bitlen##_md = {                                 \
        NID_sha3_##bitlen,                                                     \
        NID_RSA_SHA3_##bitlen,                                                 \
        bitlen / 8,                                                            \
        EVP_MD_FLAG_DIGALGID_ABSENT,                                           \
        EVP_ORIG_GLOBAL,                                                       \
        LEGACY_EVP_MD_METH_TABLE(sha3_int_init, sha3_int_update,               \
                                 sha3_int_final, NULL,                         \
                                 (KECCAK1600_WIDTH - bitlen * 2) / 8),         \
    };                                                                         \
    return &sha3_##bitlen##_md;                                                \
}
#define EVP_MD_SHAKE(bitlen)                                                   \
const EVP_MD *EVP_shake##bitlen(void)                                          \
{                                                                              \
    static const EVP_MD shake##bitlen##_md = {                                 \
        NID_shake##bitlen,                                                     \
        0,                                                                     \
        bitlen / 8,                                                            \
        EVP_MD_FLAG_XOF | EVP_MD_FLAG_DIGALGID_ABSENT,                         \
        EVP_ORIG_GLOBAL,                                                       \
        LEGACY_EVP_MD_METH_TABLE(shake_init, sha3_int_update, sha3_int_final,  \
                        shake_ctrl, (KECCAK1600_WIDTH - bitlen * 2) / 8),      \
    };                                                                         \
    return &shake##bitlen##_md;                                                \
}

EVP_MD_SHA3(224)
EVP_MD_SHA3(256)
EVP_MD_SHA3(384)
EVP_MD_SHA3(512)

EVP_MD_SHAKE(128)
EVP_MD_SHAKE(256)
