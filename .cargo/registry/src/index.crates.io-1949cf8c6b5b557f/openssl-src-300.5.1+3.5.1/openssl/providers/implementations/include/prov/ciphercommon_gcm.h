
/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_CIPHERCOMMON_GCM_H
# define OSSL_PROV_CIPHERCOMMON_GCM_H
# pragma once

# include <openssl/aes.h>
# include "ciphercommon_aead.h"

typedef struct prov_gcm_hw_st PROV_GCM_HW;

# define GCM_IV_DEFAULT_SIZE 12 /* IV's for AES_GCM should normally be 12 bytes */
# define GCM_IV_MAX_SIZE     (1024 / 8)
# define GCM_TAG_MAX_SIZE    16

# if defined(OPENSSL_CPUID_OBJ) && defined(__s390__)
/*-
 * KMA-GCM-AES parameter block - begin
 * (see z/Architecture Principles of Operation >= SA22-7832-11)
 */
typedef struct S390X_kma_params_st {
    unsigned char reserved[12];
    union {
        unsigned int w;
        unsigned char b[4];
    } cv; /* 32 bit counter value */
    union {
        unsigned long long g[2];
        unsigned char b[16];
    } t; /* tag */
    unsigned char h[16]; /* hash subkey */
    unsigned long long taadl; /* total AAD length */
    unsigned long long tpcl; /* total plaintxt/ciphertxt len */
    union {
        unsigned long long g[2];
        unsigned int w[4];
    } j0;                   /* initial counter value */
    unsigned char k[32];    /* key */
} S390X_KMA_PARAMS;

# endif

typedef struct prov_gcm_ctx_st {
    unsigned int mode;          /* The mode that we are using */
    size_t keylen;
    size_t ivlen;
    size_t taglen;
    size_t tls_aad_pad_sz;
    size_t tls_aad_len;         /* TLS AAD length */
    uint64_t tls_enc_records;   /* Number of TLS records encrypted */

    /*
     * num contains the number of bytes of |iv| which are valid for modes that
     * manage partial blocks themselves.
     */
    size_t num;
    size_t bufsz;               /* Number of bytes in buf */
    uint64_t flags;

    unsigned int iv_state;      /* set to one of IV_STATE_XXX */
    unsigned int enc:1;         /* Set to 1 if we are encrypting or 0 otherwise */
    unsigned int pad:1;         /* Whether padding should be used or not */
    unsigned int key_set:1;     /* Set if key initialised */
    unsigned int iv_gen_rand:1; /* No IV was specified, so generate a rand IV */
    unsigned int iv_gen:1;      /* It is OK to generate IVs */

    unsigned char iv[GCM_IV_MAX_SIZE]; /* Buffer to use for IV's */
    unsigned char buf[AES_BLOCK_SIZE]; /* Buffer of partial blocks processed via update calls */

    OSSL_LIB_CTX *libctx;    /* needed for rand calls */
    const PROV_GCM_HW *hw;  /* hardware specific methods */
    GCM128_CONTEXT gcm;
    ctr128_f ctr;
} PROV_GCM_CTX;

PROV_CIPHER_FUNC(int, GCM_setkey, (PROV_GCM_CTX *ctx, const unsigned char *key,
                                   size_t keylen));
PROV_CIPHER_FUNC(int, GCM_setiv, (PROV_GCM_CTX *dat, const unsigned char *iv,
                                  size_t ivlen));
PROV_CIPHER_FUNC(int, GCM_aadupdate, (PROV_GCM_CTX *ctx,
                                      const unsigned char *aad, size_t aadlen));
PROV_CIPHER_FUNC(int, GCM_cipherupdate, (PROV_GCM_CTX *ctx,
                                         const unsigned char *in, size_t len,
                                         unsigned char *out));
PROV_CIPHER_FUNC(int, GCM_cipherfinal, (PROV_GCM_CTX *ctx, unsigned char *tag));
PROV_CIPHER_FUNC(int, GCM_oneshot, (PROV_GCM_CTX *ctx, unsigned char *aad,
                                    size_t aad_len, const unsigned char *in,
                                    size_t in_len, unsigned char *out,
                                    unsigned char *tag, size_t taglen));
struct prov_gcm_hw_st {
  OSSL_GCM_setkey_fn setkey;
  OSSL_GCM_setiv_fn setiv;
  OSSL_GCM_aadupdate_fn aadupdate;
  OSSL_GCM_cipherupdate_fn cipherupdate;
  OSSL_GCM_cipherfinal_fn cipherfinal;
  OSSL_GCM_oneshot_fn oneshot;
};

OSSL_FUNC_cipher_encrypt_init_fn ossl_gcm_einit;
OSSL_FUNC_cipher_decrypt_init_fn ossl_gcm_dinit;
OSSL_FUNC_cipher_get_ctx_params_fn ossl_gcm_get_ctx_params;
OSSL_FUNC_cipher_set_ctx_params_fn ossl_gcm_set_ctx_params;
OSSL_FUNC_cipher_cipher_fn ossl_gcm_cipher;
OSSL_FUNC_cipher_update_fn ossl_gcm_stream_update;
OSSL_FUNC_cipher_final_fn ossl_gcm_stream_final;
void ossl_gcm_initctx(void *provctx, PROV_GCM_CTX *ctx, size_t keybits,
                      const PROV_GCM_HW *hw);

int ossl_gcm_setiv(PROV_GCM_CTX *ctx, const unsigned char *iv, size_t ivlen);
int ossl_gcm_aad_update(PROV_GCM_CTX *ctx, const unsigned char *aad,
                        size_t aad_len);
int ossl_gcm_cipher_final(PROV_GCM_CTX *ctx, unsigned char *tag);
int ossl_gcm_one_shot(PROV_GCM_CTX *ctx, unsigned char *aad, size_t aad_len,
                      const unsigned char *in, size_t in_len,
                      unsigned char *out, unsigned char *tag, size_t tag_len);
int ossl_gcm_cipher_update(PROV_GCM_CTX *ctx, const unsigned char *in,
                           size_t len, unsigned char *out);

# define GCM_HW_SET_KEY_CTR_FN(ks, fn_set_enc_key, fn_block, fn_ctr)            \
    fn_set_enc_key(key, keylen * 8, ks);                                       \
    CRYPTO_gcm128_init(&ctx->gcm, ks, (block128_f)fn_block);                   \
    ctx->ctr = (ctr128_f)fn_ctr;                                               \
    ctx->key_set = 1;

#endif
