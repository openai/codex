/*
 * Copyright 2008-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/crypto.h>
#include "crypto/modes.h"

#if defined(__GNUC__) && !defined(STRICT_ALIGNMENT)
typedef size_t size_t_aX __attribute((__aligned__(1)));
#else
typedef size_t size_t_aX;
#endif

/*
 * The input and output encrypted as though 128bit ofb mode is being used.
 * The extra state information to record how much of the 128bit block we have
 * used is contained in *num;
 */
void CRYPTO_ofb128_encrypt(const unsigned char *in, unsigned char *out,
                           size_t len, const void *key,
                           unsigned char ivec[16], int *num, block128_f block)
{
    unsigned int n;
    size_t l = 0;

    if (*num < 0) {
        /* There is no good way to signal an error return from here */
        *num = -1;
        return;
    }
    n = *num;

#if !defined(OPENSSL_SMALL_FOOTPRINT)
    if (16 % sizeof(size_t) == 0) { /* always true actually */
        do {
            while (n && len) {
                *(out++) = *(in++) ^ ivec[n];
                --len;
                n = (n + 1) % 16;
            }
# if defined(STRICT_ALIGNMENT)
            if (((size_t)in | (size_t)out | (size_t)ivec) % sizeof(size_t) !=
                0)
                break;
# endif
            while (len >= 16) {
                (*block) (ivec, ivec, key);
                for (; n < 16; n += sizeof(size_t))
                    *(size_t_aX *)(out + n) =
                        *(size_t_aX *)(in + n)
                        ^ *(size_t_aX *)(ivec + n);
                len -= 16;
                out += 16;
                in += 16;
                n = 0;
            }
            if (len) {
                (*block) (ivec, ivec, key);
                while (len--) {
                    out[n] = in[n] ^ ivec[n];
                    ++n;
                }
            }
            *num = n;
            return;
        } while (0);
    }
    /* the rest would be commonly eliminated by x86* compiler */
#endif
    while (l < len) {
        if (n == 0) {
            (*block) (ivec, ivec, key);
        }
        out[l] = in[l] ^ ivec[n];
        ++l;
        n = (n + 1) % 16;
    }

    *num = n;
}
