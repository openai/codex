/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include "internal/endian.h"
#include "crypto/modes.h"

#ifndef STRICT_ALIGNMENT
# ifdef __GNUC__
typedef u64 u64_a1 __attribute((__aligned__(1)));
# else
typedef u64 u64_a1;
# endif
#endif

int ossl_crypto_xts128gb_encrypt(const XTS128_CONTEXT *ctx,
                                 const unsigned char iv[16],
                                 const unsigned char *inp, unsigned char *out,
                                 size_t len, int enc)
{
    DECLARE_IS_ENDIAN;
    union {
        u64 u[2];
        u32 d[4];
        u8 c[16];
    } tweak, scratch;
    unsigned int i;

    if (len < 16)
        return -1;

    memcpy(tweak.c, iv, 16);

    (*ctx->block2) (tweak.c, tweak.c, ctx->key2);

    if (!enc && (len % 16))
        len -= 16;

    while (len >= 16) {
#if defined(STRICT_ALIGNMENT)
        memcpy(scratch.c, inp, 16);
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
#else
        scratch.u[0] = ((u64_a1 *)inp)[0] ^ tweak.u[0];
        scratch.u[1] = ((u64_a1 *)inp)[1] ^ tweak.u[1];
#endif
        (*ctx->block1) (scratch.c, scratch.c, ctx->key1);
#if defined(STRICT_ALIGNMENT)
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
        memcpy(out, scratch.c, 16);
#else
        ((u64_a1 *)out)[0] = scratch.u[0] ^= tweak.u[0];
        ((u64_a1 *)out)[1] = scratch.u[1] ^= tweak.u[1];
#endif
        inp += 16;
        out += 16;
        len -= 16;

        if (len == 0)
            return 0;

        if (IS_LITTLE_ENDIAN) {
            u8 res;
            u64 hi, lo;
#ifdef BSWAP8
            hi = BSWAP8(tweak.u[0]);
            lo = BSWAP8(tweak.u[1]);
#else
            u8 *p = tweak.c;

            hi = (u64)GETU32(p) << 32 | GETU32(p + 4);
            lo = (u64)GETU32(p + 8) << 32 | GETU32(p + 12);
#endif
            res = (u8)lo & 1;
            tweak.u[0] = (lo >> 1) | (hi << 63);
            tweak.u[1] = hi >> 1;
            if (res)
                tweak.c[15] ^= 0xe1;
#ifdef BSWAP8
            hi = BSWAP8(tweak.u[0]);
            lo = BSWAP8(tweak.u[1]);
#else
            p = tweak.c;

            hi = (u64)GETU32(p) << 32 | GETU32(p + 4);
            lo = (u64)GETU32(p + 8) << 32 | GETU32(p + 12);
#endif
            tweak.u[0] = lo;
            tweak.u[1] = hi;
        } else {
            u8 carry, res;
            carry = 0;
            for (i = 0; i < 16; ++i) {
                res = (tweak.c[i] << 7) & 0x80;
                tweak.c[i] = ((tweak.c[i] >> 1) + carry) & 0xff;
                carry = res;
            }
            if (res)
                tweak.c[0] ^= 0xe1;
        }
    }
    if (enc) {
        for (i = 0; i < len; ++i) {
            u8 c = inp[i];
            out[i] = scratch.c[i];
            scratch.c[i] = c;
        }
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
        (*ctx->block1) (scratch.c, scratch.c, ctx->key1);
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
        memcpy(out - 16, scratch.c, 16);
    } else {
        union {
            u64 u[2];
            u8 c[16];
        } tweak1;

        if (IS_LITTLE_ENDIAN) {
            u8 res;
            u64 hi, lo;
#ifdef BSWAP8
            hi = BSWAP8(tweak.u[0]);
            lo = BSWAP8(tweak.u[1]);
#else
            u8 *p = tweak.c;

            hi = (u64)GETU32(p) << 32 | GETU32(p + 4);
            lo = (u64)GETU32(p + 8) << 32 | GETU32(p + 12);
#endif
            res = (u8)lo & 1;
            tweak1.u[0] = (lo >> 1) | (hi << 63);
            tweak1.u[1] = hi >> 1;
            if (res)
                tweak1.c[15] ^= 0xe1;
#ifdef BSWAP8
            hi = BSWAP8(tweak1.u[0]);
            lo = BSWAP8(tweak1.u[1]);
#else
            p = tweak1.c;

            hi = (u64)GETU32(p) << 32 | GETU32(p + 4);
            lo = (u64)GETU32(p + 8) << 32 | GETU32(p + 12);
#endif
            tweak1.u[0] = lo;
            tweak1.u[1] = hi;
        } else {
            u8 carry, res;
            carry = 0;
            for (i = 0; i < 16; ++i) {
                res = (tweak.c[i] << 7) & 0x80;
                tweak1.c[i] = ((tweak.c[i] >> 1) + carry) & 0xff;
                carry = res;
            }
            if (res)
                tweak1.c[0] ^= 0xe1;
        }
#if defined(STRICT_ALIGNMENT)
        memcpy(scratch.c, inp, 16);
        scratch.u[0] ^= tweak1.u[0];
        scratch.u[1] ^= tweak1.u[1];
#else
        scratch.u[0] = ((u64_a1 *)inp)[0] ^ tweak1.u[0];
        scratch.u[1] = ((u64_a1 *)inp)[1] ^ tweak1.u[1];
#endif
        (*ctx->block1) (scratch.c, scratch.c, ctx->key1);
        scratch.u[0] ^= tweak1.u[0];
        scratch.u[1] ^= tweak1.u[1];

        for (i = 0; i < len; ++i) {
            u8 c = inp[16 + i];
            out[16 + i] = scratch.c[i];
            scratch.c[i] = c;
        }
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
        (*ctx->block1) (scratch.c, scratch.c, ctx->key1);
#if defined(STRICT_ALIGNMENT)
        scratch.u[0] ^= tweak.u[0];
        scratch.u[1] ^= tweak.u[1];
        memcpy(out, scratch.c, 16);
#else
        ((u64_a1 *)out)[0] = scratch.u[0] ^ tweak.u[0];
        ((u64_a1 *)out)[1] = scratch.u[1] ^ tweak.u[1];
#endif
    }

    return 0;
}
