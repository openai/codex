/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/des.h>
#include <openssl/core_dispatch.h>
#include "prov/securitycheck.h"
#include "crypto/des_platform.h"

#define DES_BLOCK_SIZE 8
#define TDES_IVLEN 8
#define TDES_FLAGS PROV_CIPHER_FLAG_RAND_KEY

typedef struct prov_tdes_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        DES_key_schedule ks[3];
    } tks;
    union {
        void (*cbc) (const void *, void *, size_t,
                     const DES_key_schedule *, unsigned char *);
    } tstream;
    OSSL_FIPS_IND_DECLARE

} PROV_TDES_CTX;

#define IMPLEMENT_tdes_cipher(type, UCTYPE, lcmode, UCMODE, flags,             \
                              kbits, blkbits, ivbits, block)                   \
static OSSL_FUNC_cipher_newctx_fn tdes_##type##_##lcmode##_newctx;             \
static void *tdes_##type##_##lcmode##_newctx(void *provctx)                    \
{                                                                              \
    return ossl_tdes_newctx(provctx, EVP_CIPH_##UCMODE##_MODE, kbits, blkbits, \
                       ivbits, flags,                                          \
                       ossl_prov_cipher_hw_tdes_##type##_##lcmode());          \
}                                                                              \
static OSSL_FUNC_cipher_get_params_fn tdes_##type##_##lcmode##_get_params;     \
static int tdes_##type##_##lcmode##_get_params(OSSL_PARAM params[])            \
{                                                                              \
    return ossl_tdes_get_params(params, EVP_CIPH_##UCMODE##_MODE,              \
                                flags, kbits, blkbits, ivbits);                \
}                                                                              \
const OSSL_DISPATCH ossl_tdes_##type##_##lcmode##_functions[] = {              \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))ossl_tdes_einit },        \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))ossl_tdes_dinit },        \
    { OSSL_FUNC_CIPHER_UPDATE,                                                 \
      (void (*)(void))ossl_cipher_generic_##block##_update },                  \
    { OSSL_FUNC_CIPHER_FINAL,                                                  \
      (void (*)(void))ossl_cipher_generic_##block##_final },                   \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void))tdes_##type##_##lcmode##_newctx },                       \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void))ossl_tdes_dupctx },             \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void))ossl_tdes_freectx },           \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void))tdes_##type##_##lcmode##_get_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_tdes_get_ctx_params },                              \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_tdes_gettable_ctx_params },                         \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_tdes_set_ctx_params },                              \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_tdes_settable_ctx_params },                         \
    OSSL_DISPATCH_END                                                          \
}

void *ossl_tdes_newctx(void *provctx, int mode, size_t kbits, size_t blkbits,
                       size_t ivbits, uint64_t flags, const PROV_CIPHER_HW *hw);
int ossl_tdes_get_params(OSSL_PARAM params[], unsigned int md, uint64_t flags,
                         size_t kbits, size_t blkbits, size_t ivbits);

OSSL_FUNC_cipher_dupctx_fn ossl_tdes_dupctx;
OSSL_FUNC_cipher_freectx_fn ossl_tdes_freectx;
OSSL_FUNC_cipher_encrypt_init_fn ossl_tdes_einit;
OSSL_FUNC_cipher_decrypt_init_fn ossl_tdes_dinit;
OSSL_FUNC_cipher_get_ctx_params_fn ossl_tdes_get_ctx_params;
OSSL_FUNC_cipher_gettable_ctx_params_fn ossl_tdes_gettable_ctx_params;
OSSL_FUNC_cipher_set_ctx_params_fn ossl_tdes_set_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn ossl_tdes_settable_ctx_params;

#define PROV_CIPHER_HW_tdes_mode(type, mode)                                   \
static const PROV_CIPHER_HW type##_##mode = {                                  \
    ossl_cipher_hw_tdes_##type##_initkey,                                      \
    ossl_cipher_hw_tdes_##mode,                                                \
    ossl_cipher_hw_tdes_copyctx                                                \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_tdes_##type##_##mode(void)           \
{                                                                              \
    return &type##_##mode;                                                     \
}

int ossl_cipher_hw_tdes_ede3_initkey(PROV_CIPHER_CTX *ctx,
                                     const unsigned char *key, size_t keylen);
void ossl_cipher_hw_tdes_copyctx(PROV_CIPHER_CTX *dst,
                                 const PROV_CIPHER_CTX *src);
int ossl_cipher_hw_tdes_cbc(PROV_CIPHER_CTX *ctx, unsigned char *out,
                            const unsigned char *in, size_t inl);
int ossl_cipher_hw_tdes_ecb(PROV_CIPHER_CTX *ctx, unsigned char *out,
                            const unsigned char *in, size_t len);

const PROV_CIPHER_HW *ossl_prov_cipher_hw_tdes_ede3_cbc(void);
const PROV_CIPHER_HW *ossl_prov_cipher_hw_tdes_ede3_ecb(void);
