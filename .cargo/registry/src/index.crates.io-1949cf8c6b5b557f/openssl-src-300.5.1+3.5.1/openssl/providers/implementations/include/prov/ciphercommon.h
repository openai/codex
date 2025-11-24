/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_CIPHERCOMMON_H
# define OSSL_PROV_CIPHERCOMMON_H
# pragma once

# include <openssl/params.h>
# include <openssl/core_dispatch.h>
# include <openssl/core_names.h>
# include <openssl/evp.h>
# include "internal/cryptlib.h"
# include "crypto/modes.h"

# define MAXCHUNK    ((size_t)1 << 30)
# define MAXBITCHUNK ((size_t)1 << (sizeof(size_t) * 8 - 4))

# define GENERIC_BLOCK_SIZE 16
# define IV_STATE_UNINITIALISED 0  /* initial state is not initialized */
# define IV_STATE_BUFFERED      1  /* iv has been copied to the iv buffer */
# define IV_STATE_COPIED        2  /* iv has been copied from the iv buffer */
# define IV_STATE_FINISHED      3  /* the iv has been used - so don't reuse it */

# define PROV_CIPHER_FUNC(type, name, args) typedef type (* OSSL_##name##_fn)args

typedef struct prov_cipher_hw_st PROV_CIPHER_HW;
typedef struct prov_cipher_ctx_st PROV_CIPHER_CTX;

typedef int (PROV_CIPHER_HW_FN)(PROV_CIPHER_CTX *dat, unsigned char *out,
                                const unsigned char *in, size_t len);

/* Internal flags that can be queried */
# define PROV_CIPHER_FLAG_AEAD             0x0001
# define PROV_CIPHER_FLAG_CUSTOM_IV        0x0002
# define PROV_CIPHER_FLAG_CTS              0x0004
# define PROV_CIPHER_FLAG_TLS1_MULTIBLOCK  0x0008
# define PROV_CIPHER_FLAG_RAND_KEY         0x0010
/* Internal flags that are only used within the provider */
# define PROV_CIPHER_FLAG_VARIABLE_LENGTH  0x0100
# define PROV_CIPHER_FLAG_INVERSE_CIPHER   0x0200

struct prov_cipher_ctx_st {
    /* place buffer at the beginning for memory alignment */
    /* The original value of the iv */
    unsigned char oiv[GENERIC_BLOCK_SIZE];
    /* Buffer of partial blocks processed via update calls */
    unsigned char buf[GENERIC_BLOCK_SIZE];
    unsigned char iv[GENERIC_BLOCK_SIZE];

    block128_f block;
    union {
        cbc128_f cbc;
        ctr128_f ctr;
        ecb128_f ecb;
    } stream;

    unsigned int mode;
    size_t keylen;           /* key size (in bytes) */
    size_t ivlen;
    size_t blocksize;
    size_t bufsz;            /* Number of bytes in buf */
    unsigned int cts_mode;   /* Use to set the type for CTS modes */
    unsigned int pad : 1;    /* Whether padding should be used or not */
    unsigned int enc : 1;    /* Set to 1 for encrypt, or 0 otherwise */
    unsigned int iv_set : 1; /* Set when the iv is copied to the iv/oiv buffers */
    unsigned int key_set : 1; /* Set when key is set on the context */
    unsigned int updated : 1; /* Set to 1 during update for one shot ciphers */
    unsigned int variable_keylength : 1;
    unsigned int inverse_cipher : 1; /* set to 1 to use inverse cipher */
    unsigned int use_bits : 1; /* Set to 0 for cfb1 to use bits instead of bytes */

    unsigned int tlsversion; /* If TLS padding is in use the TLS version number */
    unsigned char *tlsmac;   /* tls MAC extracted from the last record */
    int alloced;             /*
                              * Whether the tlsmac data has been allocated or
                              * points into the user buffer.
                              */
    size_t tlsmacsize;       /* Size of the TLS MAC */
    int removetlspad;        /* Whether TLS padding should be removed or not */
    size_t removetlsfixed;   /*
                              * Length of the fixed size data to remove when
                              * processing TLS data (equals mac size plus
                              * IV size if applicable)
                              */

    /*
     * num contains the number of bytes of |iv| which are valid for modes that
     * manage partial blocks themselves.
     */
    unsigned int num;
    const PROV_CIPHER_HW *hw; /* hardware specific functions */
    const void *ks; /* Pointer to algorithm specific key data */
    OSSL_LIB_CTX *libctx;
};

struct prov_cipher_hw_st {
    int (*init)(PROV_CIPHER_CTX *dat, const uint8_t *key, size_t keylen);
    PROV_CIPHER_HW_FN *cipher;
    void (*copyctx)(PROV_CIPHER_CTX *dst, const PROV_CIPHER_CTX *src);
};

void ossl_cipher_generic_reset_ctx(PROV_CIPHER_CTX *ctx);
OSSL_FUNC_cipher_encrypt_init_fn ossl_cipher_generic_einit;
OSSL_FUNC_cipher_decrypt_init_fn ossl_cipher_generic_dinit;
OSSL_FUNC_cipher_update_fn ossl_cipher_generic_block_update;
OSSL_FUNC_cipher_final_fn ossl_cipher_generic_block_final;
OSSL_FUNC_cipher_update_fn ossl_cipher_generic_stream_update;
OSSL_FUNC_cipher_final_fn ossl_cipher_generic_stream_final;
OSSL_FUNC_cipher_cipher_fn ossl_cipher_generic_cipher;
OSSL_FUNC_cipher_get_ctx_params_fn ossl_cipher_generic_get_ctx_params;
OSSL_FUNC_cipher_set_ctx_params_fn ossl_cipher_generic_set_ctx_params;
OSSL_FUNC_cipher_gettable_params_fn     ossl_cipher_generic_gettable_params;
OSSL_FUNC_cipher_gettable_ctx_params_fn ossl_cipher_generic_gettable_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn ossl_cipher_generic_settable_ctx_params;
OSSL_FUNC_cipher_set_ctx_params_fn ossl_cipher_var_keylen_set_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn ossl_cipher_var_keylen_settable_ctx_params;
OSSL_FUNC_cipher_gettable_ctx_params_fn ossl_cipher_aead_gettable_ctx_params;
OSSL_FUNC_cipher_settable_ctx_params_fn ossl_cipher_aead_settable_ctx_params;
OSSL_FUNC_cipher_encrypt_skey_init_fn ossl_cipher_generic_skey_einit;
OSSL_FUNC_cipher_decrypt_skey_init_fn ossl_cipher_generic_skey_dinit;

int ossl_cipher_generic_get_params(OSSL_PARAM params[], unsigned int md,
                                   uint64_t flags,
                                   size_t kbits, size_t blkbits, size_t ivbits);
void ossl_cipher_generic_initkey(void *vctx, size_t kbits, size_t blkbits,
                                 size_t ivbits, unsigned int mode,
                                 uint64_t flags,
                                 const PROV_CIPHER_HW *hw, void *provctx);

# define IMPLEMENT_generic_cipher_func(alg, UCALG, lcmode, UCMODE, flags, kbits,\
                                      blkbits, ivbits, typ)                    \
const OSSL_DISPATCH ossl_##alg##kbits##lcmode##_functions[] = {                \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_##lcmode##_newctx },                    \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))ossl_cipher_generic_einit },   \
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))ossl_cipher_generic_dinit },   \
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_cipher_generic_##typ##_update },\
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##typ##_final },  \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },        \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lcmode##_get_params },                \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_get_ctx_params },                    \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_set_ctx_params },                    \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_gettable_ctx_params },               \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))ossl_cipher_generic_settable_ctx_params },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_SKEY_INIT, (void (*)(void))ossl_cipher_generic_skey_einit },\
    { OSSL_FUNC_CIPHER_DECRYPT_SKEY_INIT, (void (*)(void))ossl_cipher_generic_skey_dinit },\
    OSSL_DISPATCH_END                                                          \
};

# define IMPLEMENT_var_keylen_cipher_func(alg, UCALG, lcmode, UCMODE, flags,    \
                                         kbits, blkbits, ivbits, typ)          \
const OSSL_DISPATCH ossl_##alg##kbits##lcmode##_functions[] = {                \
    { OSSL_FUNC_CIPHER_NEWCTX,                                                 \
      (void (*)(void)) alg##_##kbits##_##lcmode##_newctx },                    \
    { OSSL_FUNC_CIPHER_FREECTX, (void (*)(void)) alg##_freectx },              \
    { OSSL_FUNC_CIPHER_DUPCTX, (void (*)(void)) alg##_dupctx },                \
    { OSSL_FUNC_CIPHER_ENCRYPT_INIT, (void (*)(void))ossl_cipher_generic_einit },\
    { OSSL_FUNC_CIPHER_DECRYPT_INIT, (void (*)(void))ossl_cipher_generic_dinit },\
    { OSSL_FUNC_CIPHER_UPDATE, (void (*)(void))ossl_cipher_generic_##typ##_update },\
    { OSSL_FUNC_CIPHER_FINAL, (void (*)(void))ossl_cipher_generic_##typ##_final },  \
    { OSSL_FUNC_CIPHER_CIPHER, (void (*)(void))ossl_cipher_generic_cipher },   \
    { OSSL_FUNC_CIPHER_GET_PARAMS,                                             \
      (void (*)(void)) alg##_##kbits##_##lcmode##_get_params },                \
    { OSSL_FUNC_CIPHER_GET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_generic_get_ctx_params },                    \
    { OSSL_FUNC_CIPHER_SET_CTX_PARAMS,                                         \
      (void (*)(void))ossl_cipher_var_keylen_set_ctx_params },                 \
    { OSSL_FUNC_CIPHER_GETTABLE_PARAMS,                                        \
      (void (*)(void))ossl_cipher_generic_gettable_params },                   \
    { OSSL_FUNC_CIPHER_GETTABLE_CTX_PARAMS,                                    \
      (void (*)(void))ossl_cipher_generic_gettable_ctx_params },               \
    { OSSL_FUNC_CIPHER_SETTABLE_CTX_PARAMS,                                    \
     (void (*)(void))ossl_cipher_var_keylen_settable_ctx_params },             \
    { OSSL_FUNC_CIPHER_ENCRYPT_SKEY_INIT, (void (*)(void))ossl_cipher_generic_skey_einit },\
    { OSSL_FUNC_CIPHER_DECRYPT_SKEY_INIT, (void (*)(void))ossl_cipher_generic_skey_dinit },\
    OSSL_DISPATCH_END                                                          \
};


# define IMPLEMENT_generic_cipher_genfn(alg, UCALG, lcmode, UCMODE, flags,      \
                                       kbits, blkbits, ivbits, typ)            \
static OSSL_FUNC_cipher_get_params_fn alg##_##kbits##_##lcmode##_get_params;   \
static int alg##_##kbits##_##lcmode##_get_params(OSSL_PARAM params[])          \
{                                                                              \
    return ossl_cipher_generic_get_params(params, EVP_CIPH_##UCMODE##_MODE,    \
                                          flags, kbits, blkbits, ivbits);      \
}                                                                              \
static OSSL_FUNC_cipher_newctx_fn alg##_##kbits##_##lcmode##_newctx;           \
static void * alg##_##kbits##_##lcmode##_newctx(void *provctx)                 \
{                                                                              \
     PROV_##UCALG##_CTX *ctx = ossl_prov_is_running() ? OPENSSL_zalloc(sizeof(*ctx))\
                                                     : NULL;                   \
     if (ctx != NULL) {                                                        \
         ossl_cipher_generic_initkey(ctx, kbits, blkbits, ivbits,              \
                                     EVP_CIPH_##UCMODE##_MODE, flags,          \
                                     ossl_prov_cipher_hw_##alg##_##lcmode(kbits),\
                                     provctx);                                 \
     }                                                                         \
     return ctx;                                                               \
}                                                                              \

# define IMPLEMENT_generic_cipher(alg, UCALG, lcmode, UCMODE, flags, kbits,     \
                                 blkbits, ivbits, typ)                         \
IMPLEMENT_generic_cipher_genfn(alg, UCALG, lcmode, UCMODE, flags, kbits,       \
                               blkbits, ivbits, typ)                           \
IMPLEMENT_generic_cipher_func(alg, UCALG, lcmode, UCMODE, flags, kbits,        \
                              blkbits, ivbits, typ)

# define IMPLEMENT_var_keylen_cipher(alg, UCALG, lcmode, UCMODE, flags, kbits,  \
                                    blkbits, ivbits, typ)                      \
IMPLEMENT_generic_cipher_genfn(alg, UCALG, lcmode, UCMODE, flags, kbits,       \
                               blkbits, ivbits, typ)                           \
IMPLEMENT_var_keylen_cipher_func(alg, UCALG, lcmode, UCMODE, flags, kbits,     \
                                 blkbits, ivbits, typ)

PROV_CIPHER_HW_FN ossl_cipher_hw_generic_cbc;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_ecb;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_ofb128;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_cfb128;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_cfb8;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_cfb1;
PROV_CIPHER_HW_FN ossl_cipher_hw_generic_ctr;
PROV_CIPHER_HW_FN ossl_cipher_hw_chunked_cbc;
PROV_CIPHER_HW_FN ossl_cipher_hw_chunked_cfb8;
PROV_CIPHER_HW_FN ossl_cipher_hw_chunked_cfb128;
PROV_CIPHER_HW_FN ossl_cipher_hw_chunked_ofb128;
# define ossl_cipher_hw_chunked_ecb  ossl_cipher_hw_generic_ecb
# define ossl_cipher_hw_chunked_ctr  ossl_cipher_hw_generic_ctr
# define ossl_cipher_hw_chunked_cfb1 ossl_cipher_hw_generic_cfb1

# define IMPLEMENT_CIPHER_HW_OFB(MODE, NAME, CTX_NAME, KEY_NAME, FUNC_PREFIX)   \
static int cipher_hw_##NAME##_##MODE##_cipher(PROV_CIPHER_CTX *ctx,            \
                                         unsigned char *out,                   \
                                         const unsigned char *in, size_t len)  \
{                                                                              \
    int num = ctx->num;                                                        \
    KEY_NAME *key = &(((CTX_NAME *)ctx)->ks.ks);                               \
                                                                               \
    while (len >= MAXCHUNK) {                                                  \
        FUNC_PREFIX##_encrypt(in, out, MAXCHUNK, key, ctx->iv, &num);          \
        len -= MAXCHUNK;                                                       \
        in += MAXCHUNK;                                                        \
        out += MAXCHUNK;                                                       \
    }                                                                          \
    if (len > 0) {                                                             \
        FUNC_PREFIX##_encrypt(in, out, (long)len, key, ctx->iv, &num);         \
    }                                                                          \
    ctx->num = num;                                                            \
    return 1;                                                                  \
}

# define IMPLEMENT_CIPHER_HW_ECB(MODE, NAME, CTX_NAME, KEY_NAME, FUNC_PREFIX)   \
static int cipher_hw_##NAME##_##MODE##_cipher(PROV_CIPHER_CTX *ctx,            \
                                         unsigned char *out,                   \
                                         const unsigned char *in, size_t len)  \
{                                                                              \
    size_t i, bl = ctx->blocksize;                                             \
    KEY_NAME *key = &(((CTX_NAME *)ctx)->ks.ks);                               \
                                                                               \
    if (len < bl)                                                              \
        return 1;                                                              \
    for (i = 0, len -= bl; i <= len; i += bl)                                  \
        FUNC_PREFIX##_encrypt(in + i, out + i, key, ctx->enc);                 \
    return 1;                                                                  \
}

# define IMPLEMENT_CIPHER_HW_CBC(MODE, NAME, CTX_NAME, KEY_NAME, FUNC_PREFIX)   \
static int cipher_hw_##NAME##_##MODE##_cipher(PROV_CIPHER_CTX *ctx,            \
                                         unsigned char *out,                   \
                                         const unsigned char *in, size_t len)  \
{                                                                              \
    KEY_NAME *key = &(((CTX_NAME *)ctx)->ks.ks);                               \
                                                                               \
    while (len >= MAXCHUNK) {                                                  \
        FUNC_PREFIX##_encrypt(in, out, MAXCHUNK, key, ctx->iv, ctx->enc);      \
        len -= MAXCHUNK;                                                       \
        in += MAXCHUNK;                                                        \
        out += MAXCHUNK;                                                       \
    }                                                                          \
    if (len > 0)                                                               \
        FUNC_PREFIX##_encrypt(in, out, (long)len, key, ctx->iv, ctx->enc);     \
    return 1;                                                                  \
}

# define IMPLEMENT_CIPHER_HW_CFB(MODE, NAME, CTX_NAME, KEY_NAME, FUNC_PREFIX)   \
static int cipher_hw_##NAME##_##MODE##_cipher(PROV_CIPHER_CTX *ctx,            \
                                         unsigned char *out,                   \
                                         const unsigned char *in, size_t len)  \
{                                                                              \
    size_t chunk = MAXCHUNK;                                                   \
    KEY_NAME *key = &(((CTX_NAME *)ctx)->ks.ks);                               \
    int num = ctx->num;                                                        \
                                                                               \
    if (len < chunk)                                                           \
        chunk = len;                                                           \
    while (len > 0 && len >= chunk) {                                          \
        FUNC_PREFIX##_encrypt(in, out, (long)chunk, key, ctx->iv, &num,        \
                              ctx->enc);                                       \
        len -= chunk;                                                          \
        in += chunk;                                                           \
        out += chunk;                                                          \
        if (len < chunk)                                                       \
            chunk = len;                                                       \
    }                                                                          \
    ctx->num = num;                                                            \
    return 1;                                                                  \
}

# define IMPLEMENT_CIPHER_HW_COPYCTX(name, CTX_TYPE)                            \
static void name(PROV_CIPHER_CTX *dst, const PROV_CIPHER_CTX *src)             \
{                                                                              \
    CTX_TYPE *sctx = (CTX_TYPE *)src;                                          \
    CTX_TYPE *dctx = (CTX_TYPE *)dst;                                          \
                                                                               \
    *dctx = *sctx;                                                             \
    dst->ks = &dctx->ks.ks;                                                    \
}

# define CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_START(name)                         \
static const OSSL_PARAM name##_known_gettable_ctx_params[] = {                 \
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_KEYLEN, NULL),                         \
    OSSL_PARAM_size_t(OSSL_CIPHER_PARAM_IVLEN, NULL),                          \
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_PADDING, NULL),                          \
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_NUM, NULL),                              \
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_IV, NULL, 0),                    \
    OSSL_PARAM_octet_string(OSSL_CIPHER_PARAM_UPDATED_IV, NULL, 0),

# define CIPHER_DEFAULT_GETTABLE_CTX_PARAMS_END(name)                           \
    OSSL_PARAM_END                                                             \
};                                                                             \
const OSSL_PARAM * name##_gettable_ctx_params(ossl_unused void *cctx,          \
                                              ossl_unused void *provctx)       \
{                                                                              \
    return name##_known_gettable_ctx_params;                                   \
}

# define CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_START(name)                         \
static const OSSL_PARAM name##_known_settable_ctx_params[] = {                 \
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_PADDING, NULL),                          \
    OSSL_PARAM_uint(OSSL_CIPHER_PARAM_NUM, NULL),
# define CIPHER_DEFAULT_SETTABLE_CTX_PARAMS_END(name)                           \
    OSSL_PARAM_END                                                             \
};                                                                             \
const OSSL_PARAM * name##_settable_ctx_params(ossl_unused void *cctx,          \
                                              ossl_unused void *provctx)       \
{                                                                              \
    return name##_known_settable_ctx_params;                                   \
}

int ossl_cipher_generic_initiv(PROV_CIPHER_CTX *ctx, const unsigned char *iv,
                               size_t ivlen);

size_t ossl_cipher_fillblock(unsigned char *buf, size_t *buflen,
                             size_t blocksize,
                             const unsigned char **in, size_t *inlen);
int ossl_cipher_trailingdata(unsigned char *buf, size_t *buflen,
                             size_t blocksize,
                             const unsigned char **in, size_t *inlen);

#endif
