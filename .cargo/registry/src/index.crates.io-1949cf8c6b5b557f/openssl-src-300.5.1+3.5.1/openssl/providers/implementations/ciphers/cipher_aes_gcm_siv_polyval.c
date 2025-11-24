/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * AES low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include <openssl/evp.h>
#include <internal/endian.h>
#include <prov/implementations.h>
#include "cipher_aes_gcm_siv.h"

static ossl_inline void mulx_ghash(uint64_t *a)
{
    uint64_t t[2], mask;
    DECLARE_IS_ENDIAN;

    if (IS_LITTLE_ENDIAN) {
        t[0] = GSWAP8(a[0]);
        t[1] = GSWAP8(a[1]);
    } else {
        t[0] = a[0];
        t[1] = a[1];
    }
    mask = -(int64_t)(t[1] & 1) & 0xe1;
    mask <<= 56;

    if (IS_LITTLE_ENDIAN) {
        a[1] = GSWAP8((t[1] >> 1) ^ (t[0] << 63));
        a[0] = GSWAP8((t[0] >> 1) ^ mask);
    } else {
        a[1] = (t[1] >> 1) ^ (t[0] << 63);
        a[0] = (t[0] >> 1) ^ mask;
    }
}

#define aligned64(p) (((uintptr_t)p & 0x07) == 0)
static ossl_inline void byte_reverse16(uint8_t *out, const uint8_t *in)
{
    if (aligned64(out) && aligned64(in)) {
        ((uint64_t *)out)[0] = GSWAP8(((uint64_t *)in)[1]);
        ((uint64_t *)out)[1] = GSWAP8(((uint64_t *)in)[0]);
    } else {
        int i;

        for (i = 0; i < 16; i++)
            out[i] = in[15 - i];
    }
}

/* Initialization of POLYVAL via existing GHASH implementation */
void ossl_polyval_ghash_init(u128 Htable[16], const uint64_t H[2])
{
    uint64_t tmp[2];
    DECLARE_IS_ENDIAN;

    byte_reverse16((uint8_t *)tmp, (const uint8_t *)H);
    mulx_ghash(tmp);
    if (IS_LITTLE_ENDIAN) {
        /* "H is stored in host byte order" */
        tmp[0] = GSWAP8(tmp[0]);
        tmp[1] = GSWAP8(tmp[1]);
    }

    ossl_gcm_init_4bit(Htable, (u64*)tmp);
}

/* Implementation of POLYVAL via existing GHASH implementation */
void ossl_polyval_ghash_hash(const u128 Htable[16], uint8_t *tag, const uint8_t *inp, size_t len)
{
    uint64_t out[2];
    uint64_t tmp[2];
    size_t i;

    byte_reverse16((uint8_t *)out, (uint8_t *)tag);

    /*
     * This implementation doesn't deal with partials, callers do,
     * so, len is a multiple of 16
     */
    for (i = 0; i < len; i += 16) {
        byte_reverse16((uint8_t *)tmp, &inp[i]);
        ossl_gcm_ghash_4bit((u64*)out, Htable, (uint8_t *)tmp, 16);
    }
    byte_reverse16(tag, (uint8_t *)out);
}
