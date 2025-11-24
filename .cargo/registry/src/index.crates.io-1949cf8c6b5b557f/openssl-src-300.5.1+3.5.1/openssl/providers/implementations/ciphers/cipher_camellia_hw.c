/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Camellia low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <openssl/camellia.h>
#include <openssl/proverr.h>
#include "cipher_camellia.h"

static int cipher_hw_camellia_initkey(PROV_CIPHER_CTX *dat,
                                      const unsigned char *key, size_t keylen)
{
    int ret, mode = dat->mode;
    PROV_CAMELLIA_CTX *adat = (PROV_CAMELLIA_CTX *)dat;
    CAMELLIA_KEY *ks = &adat->ks.ks;

    dat->ks = ks;
    ret = Camellia_set_key(key, keylen * 8, ks);
    if (ret < 0) {
        ERR_raise(ERR_LIB_PROV, PROV_R_KEY_SETUP_FAILED);
        return 0;
    }
    if (dat->enc || (mode != EVP_CIPH_ECB_MODE && mode != EVP_CIPH_CBC_MODE)) {
        dat->block = (block128_f) Camellia_encrypt;
        dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
            (cbc128_f) Camellia_cbc_encrypt : NULL;
    } else {
        dat->block = (block128_f) Camellia_decrypt;
        dat->stream.cbc = mode == EVP_CIPH_CBC_MODE ?
            (cbc128_f) Camellia_cbc_encrypt : NULL;
    }
    return 1;
}

IMPLEMENT_CIPHER_HW_COPYCTX(cipher_hw_camellia_copyctx, PROV_CAMELLIA_CTX)

# if defined(SPARC_CMLL_CAPABLE)
#  include "cipher_camellia_hw_t4.inc"
# else
/* The generic case */
#  define PROV_CIPHER_HW_declare(mode)
#  define PROV_CIPHER_HW_select(mode)
# endif /* SPARC_CMLL_CAPABLE */

#define PROV_CIPHER_HW_camellia_mode(mode)                                     \
static const PROV_CIPHER_HW camellia_##mode = {                                \
    cipher_hw_camellia_initkey,                                                \
    ossl_cipher_hw_generic_##mode,                                             \
    cipher_hw_camellia_copyctx                                                 \
};                                                                             \
PROV_CIPHER_HW_declare(mode)                                                   \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_camellia_##mode(size_t keybits)      \
{                                                                              \
    PROV_CIPHER_HW_select(mode)                                                \
    return &camellia_##mode;                                                   \
}

PROV_CIPHER_HW_camellia_mode(cbc)
PROV_CIPHER_HW_camellia_mode(ecb)
PROV_CIPHER_HW_camellia_mode(ofb128)
PROV_CIPHER_HW_camellia_mode(cfb128)
PROV_CIPHER_HW_camellia_mode(cfb1)
PROV_CIPHER_HW_camellia_mode(cfb8)
PROV_CIPHER_HW_camellia_mode(ctr)
