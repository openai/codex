/*
 * Copyright 2006-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/ec.h>
#include <openssl/rand.h>
#include "crypto/ecx.h"
#include "ec_local.h"
#include "curve448/curve448_local.h"
#include "ecx_backend.h"
#include "s390x_arch.h"
#include "internal/constant_time.h"

static void s390x_x25519_mod_p(unsigned char u[32])
{
    unsigned char u_red[32];
    unsigned int c = 0;
    int i;

    memcpy(u_red, u, sizeof(u_red));

    c += (unsigned int)u_red[31] + 19;
    u_red[31] = (unsigned char)c;
    c >>= 8;

    for (i = 30; i >= 0; i--) {
        c += (unsigned int)u_red[i];
        u_red[i] = (unsigned char)c;
        c >>= 8;
    }

    c = (u_red[0] & 0x80) >> 7;
    u_red[0] &= 0x7f;
    constant_time_cond_swap_buff(0 - (unsigned char)c,
                                 u, u_red, sizeof(u_red));
}

static void s390x_x448_mod_p(unsigned char u[56])
{
    unsigned char u_red[56];
    unsigned int c = 0;
    int i;

    memcpy(u_red, u, sizeof(u_red));

    c += (unsigned int)u_red[55] + 1;
    u_red[55] = (unsigned char)c;
    c >>= 8;

    for (i = 54; i >= 28; i--) {
        c += (unsigned int)u_red[i];
        u_red[i] = (unsigned char)c;
        c >>= 8;
    }

    c += (unsigned int)u_red[27] + 1;
    u_red[27] = (unsigned char)c;
    c >>= 8;

    for (i = 26; i >= 0; i--) {
        c += (unsigned int)u_red[i];
        u_red[i] = (unsigned char)c;
        c >>= 8;
    }

    constant_time_cond_swap_buff(0 - (unsigned char)c,
                                 u, u_red, sizeof(u_red));
}

int s390x_x25519_mul(unsigned char u_dst[32],
                     const unsigned char u_src[32],
                     const unsigned char d_src[32])
{
    union {
        struct {
            unsigned char u_dst[32];
            unsigned char u_src[32];
            unsigned char d_src[32];
        } x25519;
        unsigned long long buff[512];
    } param;
    int rc;

    memset(&param, 0, sizeof(param));

    s390x_flip_endian32(param.x25519.u_src, u_src);
    param.x25519.u_src[0] &= 0x7f;
    s390x_x25519_mod_p(param.x25519.u_src);

    s390x_flip_endian32(param.x25519.d_src, d_src);
    param.x25519.d_src[31] &= 248;
    param.x25519.d_src[0] &= 127;
    param.x25519.d_src[0] |= 64;

    rc = s390x_pcc(S390X_SCALAR_MULTIPLY_X25519, &param.x25519) ? 0 : 1;
    if (rc == 1)
        s390x_flip_endian32(u_dst, param.x25519.u_dst);

    OPENSSL_cleanse(param.x25519.d_src, sizeof(param.x25519.d_src));
    return rc;
}

int s390x_x448_mul(unsigned char u_dst[56],
                   const unsigned char u_src[56],
                   const unsigned char d_src[56])
{
    union {
        struct {
            unsigned char u_dst[64];
            unsigned char u_src[64];
            unsigned char d_src[64];
        } x448;
        unsigned long long buff[512];
    } param;
    int rc;

    memset(&param, 0, sizeof(param));

    memcpy(param.x448.u_src, u_src, 56);
    memcpy(param.x448.d_src, d_src, 56);

    s390x_flip_endian64(param.x448.u_src, param.x448.u_src);
    s390x_x448_mod_p(param.x448.u_src + 8);

    s390x_flip_endian64(param.x448.d_src, param.x448.d_src);
    param.x448.d_src[63] &= 252;
    param.x448.d_src[8] |= 128;

    rc = s390x_pcc(S390X_SCALAR_MULTIPLY_X448, &param.x448) ? 0 : 1;
    if (rc == 1) {
        s390x_flip_endian64(param.x448.u_dst, param.x448.u_dst);
        memcpy(u_dst, param.x448.u_dst, 56);
    }

    OPENSSL_cleanse(param.x448.d_src, sizeof(param.x448.d_src));
    return rc;
}

int s390x_ed25519_mul(unsigned char x_dst[32],
                      unsigned char y_dst[32],
                      const unsigned char x_src[32],
                      const unsigned char y_src[32],
                      const unsigned char d_src[32])
{
    union {
        struct {
            unsigned char x_dst[32];
            unsigned char y_dst[32];
            unsigned char x_src[32];
            unsigned char y_src[32];
            unsigned char d_src[32];
        } ed25519;
        unsigned long long buff[512];
    } param;
    int rc;

    memset(&param, 0, sizeof(param));

    s390x_flip_endian32(param.ed25519.x_src, x_src);
    s390x_flip_endian32(param.ed25519.y_src, y_src);
    s390x_flip_endian32(param.ed25519.d_src, d_src);

    rc = s390x_pcc(S390X_SCALAR_MULTIPLY_ED25519, &param.ed25519) ? 0 : 1;
    if (rc == 1) {
        s390x_flip_endian32(x_dst, param.ed25519.x_dst);
        s390x_flip_endian32(y_dst, param.ed25519.y_dst);
    }

    OPENSSL_cleanse(param.ed25519.d_src, sizeof(param.ed25519.d_src));
    return rc;
}

int s390x_ed448_mul(unsigned char x_dst[57],
                    unsigned char y_dst[57],
                    const unsigned char x_src[57],
                    const unsigned char y_src[57],
                    const unsigned char d_src[57])
{
    union {
        struct {
            unsigned char x_dst[64];
            unsigned char y_dst[64];
            unsigned char x_src[64];
            unsigned char y_src[64];
            unsigned char d_src[64];
        } ed448;
        unsigned long long buff[512];
    } param;
    int rc;

    memset(&param, 0, sizeof(param));

    memcpy(param.ed448.x_src, x_src, 57);
    memcpy(param.ed448.y_src, y_src, 57);
    memcpy(param.ed448.d_src, d_src, 57);
    s390x_flip_endian64(param.ed448.x_src, param.ed448.x_src);
    s390x_flip_endian64(param.ed448.y_src, param.ed448.y_src);
    s390x_flip_endian64(param.ed448.d_src, param.ed448.d_src);

    rc = s390x_pcc(S390X_SCALAR_MULTIPLY_ED448, &param.ed448) ? 0 : 1;
    if (rc == 1) {
        s390x_flip_endian64(param.ed448.x_dst, param.ed448.x_dst);
        s390x_flip_endian64(param.ed448.y_dst, param.ed448.y_dst);
        memcpy(x_dst, param.ed448.x_dst, 57);
        memcpy(y_dst, param.ed448.y_dst, 57);
    }

    OPENSSL_cleanse(param.ed448.d_src, sizeof(param.ed448.d_src));
    return rc;
}
