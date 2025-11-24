/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "prov/ciphercommon.h"
#include "crypto/aes_platform.h"
#include "crypto/siv.h"

typedef struct prov_cipher_hw_aes_siv_st {
    int (*initkey)(void *ctx, const uint8_t *key, size_t keylen);
    int (*cipher)(void *ctx, unsigned char *out, const unsigned char *in,
                  size_t len);
    void (*setspeed)(void *ctx, int speed);
    int (*settag)(void *ctx, const unsigned char *tag, size_t tagl);
    void (*cleanup)(void *ctx);
    int (*dupctx)(void *src, void *dst);
} PROV_CIPHER_HW_AES_SIV;

typedef struct prov_siv_ctx_st {
    unsigned int mode;       /* The mode that we are using */
    unsigned int enc : 1;    /* Set to 1 if we are encrypting or 0 otherwise */
    size_t keylen;           /* The input keylength (twice the alg key length) */
    size_t taglen;           /* the taglen is the same as the sivlen */
    SIV128_CONTEXT siv;
    EVP_CIPHER *ctr;        /* These are fetched - so we need to free them */
    EVP_CIPHER *cbc;
    const PROV_CIPHER_HW_AES_SIV *hw;
    OSSL_LIB_CTX *libctx;
} PROV_AES_SIV_CTX;

const PROV_CIPHER_HW_AES_SIV *ossl_prov_cipher_hw_aes_siv(size_t keybits);
