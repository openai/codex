/*
 * Copyright 2017-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#if defined(__s390x__) && defined(OPENSSL_CPUID_OBJ)
# include "crypto/s390x_arch.h"
#endif
#include "internal/sha3.h"

void SHA3_squeeze(uint64_t A[5][5], unsigned char *out, size_t len, size_t r, int next);

void ossl_sha3_reset(KECCAK1600_CTX *ctx)
{
#if defined(__s390x__) && defined(OPENSSL_CPUID_OBJ)
    if (!(OPENSSL_s390xcap_P.stfle[1] & S390X_CAPBIT(S390X_MSA12)))
#endif
        memset(ctx->A, 0, sizeof(ctx->A));
    ctx->bufsz = 0;
    ctx->xof_state = XOF_STATE_INIT;
}

int ossl_sha3_init(KECCAK1600_CTX *ctx, unsigned char pad, size_t bitlen)
{
    size_t bsz = SHA3_BLOCKSIZE(bitlen);

    if (bsz <= sizeof(ctx->buf)) {
        ossl_sha3_reset(ctx);
        ctx->block_size = bsz;
        ctx->md_size = bitlen / 8;
        ctx->pad = pad;
        return 1;
    }

    return 0;
}

int ossl_keccak_init(KECCAK1600_CTX *ctx, unsigned char pad, size_t bitlen, size_t mdlen)
{
    int ret = ossl_sha3_init(ctx, pad, bitlen);

    if (ret)
        ctx->md_size = mdlen / 8;
    return ret;
}

int ossl_sha3_update(KECCAK1600_CTX *ctx, const void *_inp, size_t len)
{
    const unsigned char *inp = _inp;
    size_t bsz = ctx->block_size;
    size_t num, rem;

    if (len == 0)
        return 1;

    if (ctx->xof_state == XOF_STATE_SQUEEZE
        || ctx->xof_state == XOF_STATE_FINAL)
        return 0;

    if ((num = ctx->bufsz) != 0) {      /* process intermediate buffer? */
        rem = bsz - num;

        if (len < rem) {
            memcpy(ctx->buf + num, inp, len);
            ctx->bufsz += len;
            return 1;
        }
        /*
         * We have enough data to fill or overflow the intermediate
         * buffer. So we append |rem| bytes and process the block,
         * leaving the rest for later processing...
         */
        memcpy(ctx->buf + num, inp, rem);
        inp += rem, len -= rem;
        (void)SHA3_absorb(ctx->A, ctx->buf, bsz, bsz);
        ctx->bufsz = 0;
        /* ctx->buf is processed, ctx->num is guaranteed to be zero */
    }

    if (len >= bsz)
        rem = SHA3_absorb(ctx->A, inp, len, bsz);
    else
        rem = len;

    if (rem) {
        memcpy(ctx->buf, inp + len - rem, rem);
        ctx->bufsz = rem;
    }

    return 1;
}

/*
 * ossl_sha3_final()is a single shot method
 * (Use ossl_sha3_squeeze for multiple calls).
 * outlen is the variable size output.
 */
int ossl_sha3_final(KECCAK1600_CTX *ctx, unsigned char *out, size_t outlen)
{
    size_t bsz = ctx->block_size;
    size_t num = ctx->bufsz;

    if (outlen == 0)
        return 1;
    if (ctx->xof_state == XOF_STATE_SQUEEZE
        || ctx->xof_state == XOF_STATE_FINAL)
        return 0;

    /*
     * Pad the data with 10*1. Note that |num| can be |bsz - 1|
     * in which case both byte operations below are performed on
     * same byte...
     */
    memset(ctx->buf + num, 0, bsz - num);
    ctx->buf[num] = ctx->pad;
    ctx->buf[bsz - 1] |= 0x80;

    (void)SHA3_absorb(ctx->A, ctx->buf, bsz, bsz);

    ctx->xof_state = XOF_STATE_FINAL;
    SHA3_squeeze(ctx->A, out, outlen, bsz, 0);
    return 1;
}

/*
 * This method can be called multiple times.
 * Rather than heavily modifying assembler for SHA3_squeeze(),
 * we instead just use the limitations of the existing function.
 * i.e. Only request multiples of the ctx->block_size when calling
 * SHA3_squeeze(). For output length requests smaller than the
 * ctx->block_size just request a single ctx->block_size bytes and
 * buffer the results. The next request will use the buffer first
 * to grab output bytes.
 */
int ossl_sha3_squeeze(KECCAK1600_CTX *ctx, unsigned char *out, size_t outlen)
{
    size_t bsz = ctx->block_size;
    size_t num = ctx->bufsz;
    size_t len;
    int next = 1;

    if (outlen == 0)
        return 1;

    if (ctx->xof_state == XOF_STATE_FINAL)
        return 0;

    /*
     * On the first squeeze call, finish the absorb process,
     * by adding the trailing padding and then doing
     * a final absorb.
     */
    if (ctx->xof_state != XOF_STATE_SQUEEZE) {
        /*
         * Pad the data with 10*1. Note that |num| can be |bsz - 1|
         * in which case both byte operations below are performed on
         * same byte...
         */
        memset(ctx->buf + num, 0, bsz - num);
        ctx->buf[num] = ctx->pad;
        ctx->buf[bsz - 1] |= 0x80;
        (void)SHA3_absorb(ctx->A, ctx->buf, bsz, bsz);
        ctx->xof_state = XOF_STATE_SQUEEZE;
        num = ctx->bufsz = 0;
        next = 0;
    }

    /*
     * Step 1. Consume any bytes left over from a previous squeeze
     * (See Step 4 below).
     */
    if (num != 0) {
        if (outlen > ctx->bufsz)
            len = ctx->bufsz;
        else
            len = outlen;
        memcpy(out, ctx->buf + bsz - ctx->bufsz, len);
        out += len;
        outlen -= len;
        ctx->bufsz -= len;
    }
    if (outlen == 0)
        return 1;

    /* Step 2. Copy full sized squeezed blocks to the output buffer directly */
    if (outlen >= bsz) {
        len = bsz * (outlen / bsz);
        SHA3_squeeze(ctx->A, out, len, bsz, next);
        next = 1;
        out += len;
        outlen -= len;
    }
    if (outlen > 0) {
        /* Step 3. Squeeze one more block into a buffer */
        SHA3_squeeze(ctx->A, ctx->buf, bsz, bsz, next);
        memcpy(out, ctx->buf, outlen);
        /* Step 4. Remember the leftover part of the squeezed block */
        ctx->bufsz = bsz - outlen;
    }

    return 1;
}
