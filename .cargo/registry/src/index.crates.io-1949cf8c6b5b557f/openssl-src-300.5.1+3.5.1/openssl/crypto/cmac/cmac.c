/*
 * Copyright 2010-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * CMAC low level APIs are deprecated for public use, but still ok for internal
 * use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "internal/cryptlib.h"
#include <openssl/cmac.h>
#include <openssl/err.h>
#include "crypto/cmac.h"

#define LOCAL_BUF_SIZE 2048
struct CMAC_CTX_st {
    /* Cipher context to use */
    EVP_CIPHER_CTX *cctx;
    /* Keys k1 and k2 */
    unsigned char k1[EVP_MAX_BLOCK_LENGTH];
    unsigned char k2[EVP_MAX_BLOCK_LENGTH];
    /* Temporary block */
    unsigned char tbl[EVP_MAX_BLOCK_LENGTH];
    /* Last (possibly partial) block */
    unsigned char last_block[EVP_MAX_BLOCK_LENGTH];
    /* Number of bytes in last block: -1 means context not initialised */
    int nlast_block;
};

/* Make temporary keys K1 and K2 */

static void make_kn(unsigned char *k1, const unsigned char *l, int bl)
{
    int i;
    unsigned char c = l[0], carry = c >> 7, cnext;

    /* Shift block to left, including carry */
    for (i = 0; i < bl - 1; i++, c = cnext)
        k1[i] = (c << 1) | ((cnext = l[i + 1]) >> 7);

    /* If MSB set fixup with R */
    k1[i] = (c << 1) ^ ((0 - carry) & (bl == 16 ? 0x87 : 0x1b));
}

CMAC_CTX *CMAC_CTX_new(void)
{
    CMAC_CTX *ctx;

    if ((ctx = OPENSSL_malloc(sizeof(*ctx))) == NULL)
        return NULL;
    ctx->cctx = EVP_CIPHER_CTX_new();
    if (ctx->cctx == NULL) {
        OPENSSL_free(ctx);
        return NULL;
    }
    ctx->nlast_block = -1;
    return ctx;
}

void CMAC_CTX_cleanup(CMAC_CTX *ctx)
{
    EVP_CIPHER_CTX_reset(ctx->cctx);
    OPENSSL_cleanse(ctx->tbl, EVP_MAX_BLOCK_LENGTH);
    OPENSSL_cleanse(ctx->k1, EVP_MAX_BLOCK_LENGTH);
    OPENSSL_cleanse(ctx->k2, EVP_MAX_BLOCK_LENGTH);
    OPENSSL_cleanse(ctx->last_block, EVP_MAX_BLOCK_LENGTH);
    ctx->nlast_block = -1;
}

EVP_CIPHER_CTX *CMAC_CTX_get0_cipher_ctx(CMAC_CTX *ctx)
{
    return ctx->cctx;
}

void CMAC_CTX_free(CMAC_CTX *ctx)
{
    if (!ctx)
        return;
    CMAC_CTX_cleanup(ctx);
    EVP_CIPHER_CTX_free(ctx->cctx);
    OPENSSL_free(ctx);
}

int CMAC_CTX_copy(CMAC_CTX *out, const CMAC_CTX *in)
{
    int bl;

    if (in->nlast_block == -1)
        return 0;
    if ((bl = EVP_CIPHER_CTX_get_block_size(in->cctx)) == 0)
        return 0;
    if (!EVP_CIPHER_CTX_copy(out->cctx, in->cctx))
        return 0;
    memcpy(out->k1, in->k1, bl);
    memcpy(out->k2, in->k2, bl);
    memcpy(out->tbl, in->tbl, bl);
    memcpy(out->last_block, in->last_block, bl);
    out->nlast_block = in->nlast_block;
    return 1;
}

int ossl_cmac_init(CMAC_CTX *ctx, const void *key, size_t keylen,
                   const EVP_CIPHER *cipher, ENGINE *impl,
                   const OSSL_PARAM param[])
{
    static const unsigned char zero_iv[EVP_MAX_BLOCK_LENGTH] = { 0 };
    int block_len;

    /* All zeros means restart */
    if (!key && !cipher && !impl && keylen == 0) {
        /* Not initialised */
        if (ctx->nlast_block == -1)
            return 0;
        if (!EVP_EncryptInit_ex2(ctx->cctx, NULL, NULL, zero_iv, param))
            return 0;
        block_len = EVP_CIPHER_CTX_get_block_size(ctx->cctx);
        if (block_len == 0)
            return 0;
        memset(ctx->tbl, 0, block_len);
        ctx->nlast_block = 0;
        return 1;
    }
    /* Initialise context */
    if (cipher != NULL) {
        /* Ensure we can't use this ctx until we also have a key */
        ctx->nlast_block = -1;
        if (impl != NULL) {
            if (!EVP_EncryptInit_ex(ctx->cctx, cipher, impl, NULL, NULL))
                return 0;
        } else {
            if (!EVP_EncryptInit_ex2(ctx->cctx, cipher, NULL, NULL, param))
                return 0;
        }
    }
    /* Non-NULL key means initialisation complete */
    if (key != NULL) {
        int bl;

        /* If anything fails then ensure we can't use this ctx */
        ctx->nlast_block = -1;
        if (EVP_CIPHER_CTX_get0_cipher(ctx->cctx) == NULL)
            return 0;
        if (EVP_CIPHER_CTX_set_key_length(ctx->cctx, keylen) <= 0)
            return 0;
        if (!EVP_EncryptInit_ex2(ctx->cctx, NULL, key, zero_iv, param))
            return 0;
        if ((bl = EVP_CIPHER_CTX_get_block_size(ctx->cctx)) < 0)
            return 0;
        if (EVP_Cipher(ctx->cctx, ctx->tbl, zero_iv, bl) <= 0)
            return 0;
        make_kn(ctx->k1, ctx->tbl, bl);
        make_kn(ctx->k2, ctx->k1, bl);
        OPENSSL_cleanse(ctx->tbl, bl);
        /* Reset context again ready for first data block */
        if (!EVP_EncryptInit_ex2(ctx->cctx, NULL, NULL, zero_iv, param))
            return 0;
        /* Zero tbl so resume works */
        memset(ctx->tbl, 0, bl);
        ctx->nlast_block = 0;
    }
    return 1;
}

int CMAC_Init(CMAC_CTX *ctx, const void *key, size_t keylen,
              const EVP_CIPHER *cipher, ENGINE *impl)
{
    return ossl_cmac_init(ctx, key, keylen, cipher, impl, NULL);
}

int CMAC_Update(CMAC_CTX *ctx, const void *in, size_t dlen)
{
    const unsigned char *data = in;
    int bl;
    size_t max_burst_blocks, cipher_blocks;
    unsigned char buf[LOCAL_BUF_SIZE];

    if (ctx->nlast_block == -1)
        return 0;
    if (dlen == 0)
        return 1;
    if ((bl = EVP_CIPHER_CTX_get_block_size(ctx->cctx)) == 0)
        return 0;
    /* Copy into partial block if we need to */
    if (ctx->nlast_block > 0) {
        size_t nleft;

        nleft = bl - ctx->nlast_block;
        if (dlen < nleft)
            nleft = dlen;
        memcpy(ctx->last_block + ctx->nlast_block, data, nleft);
        dlen -= nleft;
        ctx->nlast_block += nleft;
        /* If no more to process return */
        if (dlen == 0)
            return 1;
        data += nleft;
        /* Else not final block so encrypt it */
        if (EVP_Cipher(ctx->cctx, ctx->tbl, ctx->last_block, bl) <= 0)
            return 0;
    }
    /* Encrypt all but one of the complete blocks left */

    max_burst_blocks = LOCAL_BUF_SIZE / bl;
    cipher_blocks = (dlen - 1) / bl;
    if (max_burst_blocks == 0) {
        /*
         * When block length is greater than local buffer size,
         * use ctx->tbl as cipher output.
         */
        while (dlen > (size_t)bl) {
            if (EVP_Cipher(ctx->cctx, ctx->tbl, data, bl) <= 0)
                return 0;
            dlen -= bl;
            data += bl;
        }
    } else {
        while (cipher_blocks > max_burst_blocks) {
            if (EVP_Cipher(ctx->cctx, buf, data, max_burst_blocks * bl) <= 0)
                return 0;
            dlen -= max_burst_blocks * bl;
            data += max_burst_blocks * bl;
            cipher_blocks -= max_burst_blocks;
        }
        if (cipher_blocks > 0) {
            if (EVP_Cipher(ctx->cctx, buf, data, cipher_blocks * bl) <= 0)
                return 0;
            dlen -= cipher_blocks * bl;
            data += cipher_blocks * bl;
            memcpy(ctx->tbl, &buf[(cipher_blocks - 1) * bl], bl);
        }
    }
    /* Copy any data left to last block buffer */
    memcpy(ctx->last_block, data, dlen);
    ctx->nlast_block = dlen;
    return 1;

}

int CMAC_Final(CMAC_CTX *ctx, unsigned char *out, size_t *poutlen)
{
    int i, bl, lb;

    if (ctx->nlast_block == -1)
        return 0;
    if ((bl = EVP_CIPHER_CTX_get_block_size(ctx->cctx)) == 0)
        return 0;
    if (poutlen != NULL)
        *poutlen = (size_t)bl;
    if (!out)
        return 1;
    lb = ctx->nlast_block;
    /* Is last block complete? */
    if (lb == bl) {
        for (i = 0; i < bl; i++)
            out[i] = ctx->last_block[i] ^ ctx->k1[i];
    } else {
        ctx->last_block[lb] = 0x80;
        if (bl - lb > 1)
            memset(ctx->last_block + lb + 1, 0, bl - lb - 1);
        for (i = 0; i < bl; i++)
            out[i] = ctx->last_block[i] ^ ctx->k2[i];
    }
    if (EVP_Cipher(ctx->cctx, out, out, bl) <= 0) {
        OPENSSL_cleanse(out, bl);
        return 0;
    }
    return 1;
}

int CMAC_resume(CMAC_CTX *ctx)
{
    if (ctx->nlast_block == -1)
        return 0;
    /*
     * The buffer "tbl" contains the last fully encrypted block which is the
     * last IV (or all zeroes if no last encrypted block). The last block has
     * not been modified since CMAC_final(). So reinitialising using the last
     * decrypted block will allow CMAC to continue after calling
     * CMAC_Final().
     */
    return EVP_EncryptInit_ex(ctx->cctx, NULL, NULL, NULL, ctx->tbl);
}
