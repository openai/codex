/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* chacha20 cipher implementation */

#include "cipher_chacha20.h"

static int chacha20_initkey(PROV_CIPHER_CTX *bctx, const uint8_t *key,
                            size_t keylen)
{
    PROV_CHACHA20_CTX *ctx = (PROV_CHACHA20_CTX *)bctx;
    unsigned int i;

    if (key != NULL) {
        for (i = 0; i < CHACHA_KEY_SIZE; i += 4)
            ctx->key.d[i / 4] = CHACHA_U8TOU32(key + i);
    }
    ctx->partial_len = 0;
    return 1;
}

static int chacha20_initiv(PROV_CIPHER_CTX *bctx)
{
    PROV_CHACHA20_CTX *ctx = (PROV_CHACHA20_CTX *)bctx;
    unsigned int i;

    if (bctx->iv_set) {
        for (i = 0; i < CHACHA_CTR_SIZE; i += 4)
            ctx->counter[i / 4] = CHACHA_U8TOU32(bctx->oiv + i);
    }
    ctx->partial_len = 0;
    return 1;
}

static int chacha20_cipher(PROV_CIPHER_CTX *bctx, unsigned char *out,
                           const unsigned char *in, size_t inl)
{
    PROV_CHACHA20_CTX *ctx = (PROV_CHACHA20_CTX *)bctx;
    unsigned int n, rem, ctr32;

    n = ctx->partial_len;
    if (n > 0) {
        while (inl > 0 && n < CHACHA_BLK_SIZE) {
            *out++ = *in++ ^ ctx->buf[n++];
            inl--;
        }
        ctx->partial_len = n;

        if (inl == 0)
            return 1;

        if (n == CHACHA_BLK_SIZE) {
            ctx->partial_len = 0;
            ctx->counter[0]++;
            if (ctx->counter[0] == 0)
                ctx->counter[1]++;
        }
    }

    rem = (unsigned int)(inl % CHACHA_BLK_SIZE);
    inl -= rem;
    ctr32 = ctx->counter[0];
    while (inl >= CHACHA_BLK_SIZE) {
        size_t blocks = inl / CHACHA_BLK_SIZE;

        /*
         * 1<<28 is just a not-so-small yet not-so-large number...
         * Below condition is practically never met, but it has to
         * be checked for code correctness.
         */
        if (sizeof(size_t) > sizeof(unsigned int) && blocks > (1U << 28))
            blocks = (1U << 28);

        /*
         * As ChaCha20_ctr32 operates on 32-bit counter, caller
         * has to handle overflow. 'if' below detects the
         * overflow, which is then handled by limiting the
         * amount of blocks to the exact overflow point...
         */
        ctr32 += (unsigned int)blocks;
        if (ctr32 < blocks) {
            blocks -= ctr32;
            ctr32 = 0;
        }
        blocks *= CHACHA_BLK_SIZE;
        ChaCha20_ctr32(out, in, blocks, ctx->key.d, ctx->counter);
        inl -= blocks;
        in += blocks;
        out += blocks;

        ctx->counter[0] = ctr32;
        if (ctr32 == 0) ctx->counter[1]++;
    }

    if (rem > 0) {
        memset(ctx->buf, 0, sizeof(ctx->buf));
        ChaCha20_ctr32(ctx->buf, ctx->buf, CHACHA_BLK_SIZE,
                       ctx->key.d, ctx->counter);
        for (n = 0; n < rem; n++)
            out[n] = in[n] ^ ctx->buf[n];
        ctx->partial_len = rem;
    }

    return 1;
}

static const PROV_CIPHER_HW_CHACHA20 chacha20_hw = {
    { chacha20_initkey, chacha20_cipher },
    chacha20_initiv
};

const PROV_CIPHER_HW *ossl_prov_cipher_hw_chacha20(size_t keybits)
{
    return (PROV_CIPHER_HW *)&chacha20_hw;
}

