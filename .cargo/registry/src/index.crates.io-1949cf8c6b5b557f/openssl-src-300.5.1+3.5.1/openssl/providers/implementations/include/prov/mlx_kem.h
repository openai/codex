/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_MLX_KEM_H
# define OSSL_MLX_KEM_H
# pragma once

#include <openssl/evp.h>
#include <openssl/ml_kem.h>
#include <crypto/ml_kem.h>
#include <crypto/ecx.h>

typedef struct ecdh_vinfo_st {
    const char *algorithm_name;
    const char *group_name;
    size_t      pubkey_bytes;
    size_t      prvkey_bytes;
    size_t      shsec_bytes;
    int         ml_kem_slot;
    int         ml_kem_variant;
} ECDH_VINFO;

typedef struct mlx_key_st {
    OSSL_LIB_CTX *libctx;
    char *propq;
    const ML_KEM_VINFO *minfo;
    const ECDH_VINFO *xinfo;
    EVP_PKEY *mkey;
    EVP_PKEY *xkey;
    unsigned int state;
} MLX_KEY;

#define MLX_HAVE_NOKEYS 0
#define MLX_HAVE_PUBKEY 1
#define MLX_HAVE_PRVKEY 2

/* Both key parts have whatever the ML-KEM component has */
#define mlx_kem_have_pubkey(key) ((key)->state > 0)
#define mlx_kem_have_prvkey(key) ((key)->state > 1)

#endif
