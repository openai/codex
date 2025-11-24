/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * SEED low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include "cipher_seed.h"

static int cipher_hw_seed_initkey(PROV_CIPHER_CTX *ctx,
                                  const unsigned char *key, size_t keylen)
{
    PROV_SEED_CTX *sctx =  (PROV_SEED_CTX *)ctx;

    SEED_set_key(key, &(sctx->ks.ks));
    return 1;
}

# define PROV_CIPHER_HW_seed_mode(mode, UCMODE)                                \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, seed, PROV_SEED_CTX, SEED_KEY_SCHEDULE,     \
                             SEED_##mode)                                      \
static const PROV_CIPHER_HW seed_##mode = {                                    \
    cipher_hw_seed_initkey,                                                    \
    cipher_hw_seed_##mode##_cipher                                             \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_seed_##mode(size_t keybits)          \
{                                                                              \
    return &seed_##mode;                                                       \
}

PROV_CIPHER_HW_seed_mode(cbc, CBC)
PROV_CIPHER_HW_seed_mode(ecb, ECB)
PROV_CIPHER_HW_seed_mode(ofb128, OFB)
PROV_CIPHER_HW_seed_mode(cfb128, CFB)
