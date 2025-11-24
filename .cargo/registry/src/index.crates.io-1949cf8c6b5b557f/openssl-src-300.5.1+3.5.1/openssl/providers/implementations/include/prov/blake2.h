/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_PROV_BLAKE2_H
# define OSSL_PROV_BLAKE2_H

# include <openssl/opensslconf.h>

# include <openssl/e_os2.h>
# include <stddef.h>
# include <crypto/evp.h>

# define BLAKE2S_BLOCKBYTES    64
# define BLAKE2S_OUTBYTES      32
# define BLAKE2S_KEYBYTES      32
# define BLAKE2S_SALTBYTES     8
# define BLAKE2S_PERSONALBYTES 8

# define BLAKE2B_BLOCKBYTES    128
# define BLAKE2B_OUTBYTES      64
# define BLAKE2B_KEYBYTES      64
# define BLAKE2B_SALTBYTES     16
# define BLAKE2B_PERSONALBYTES 16

struct blake2s_param_st {
    uint8_t  digest_length; /* 1 */
    uint8_t  key_length;    /* 2 */
    uint8_t  fanout;        /* 3 */
    uint8_t  depth;         /* 4 */
    uint8_t  leaf_length[4];/* 8 */
    uint8_t  node_offset[6];/* 14 */
    uint8_t  node_depth;    /* 15 */
    uint8_t  inner_length;  /* 16 */
    uint8_t  salt[BLAKE2S_SALTBYTES]; /* 24 */
    uint8_t  personal[BLAKE2S_PERSONALBYTES];  /* 32 */
};

typedef struct blake2s_param_st BLAKE2S_PARAM;

struct blake2s_ctx_st {
    uint32_t h[8];
    uint32_t t[2];
    uint32_t f[2];
    uint8_t  buf[BLAKE2S_BLOCKBYTES];
    size_t   buflen;
    size_t   outlen;
};

struct blake2b_param_st {
    uint8_t  digest_length; /* 1 */
    uint8_t  key_length;    /* 2 */
    uint8_t  fanout;        /* 3 */
    uint8_t  depth;         /* 4 */
    uint8_t  leaf_length[4];/* 8 */
    uint8_t  node_offset[8];/* 16 */
    uint8_t  node_depth;    /* 17 */
    uint8_t  inner_length;  /* 18 */
    uint8_t  reserved[14];  /* 32 */
    uint8_t  salt[BLAKE2B_SALTBYTES]; /* 48 */
    uint8_t  personal[BLAKE2B_PERSONALBYTES];  /* 64 */
};

typedef struct blake2b_param_st BLAKE2B_PARAM;

struct blake2b_ctx_st {
    uint64_t h[8];
    uint64_t t[2];
    uint64_t f[2];
    uint8_t  buf[BLAKE2B_BLOCKBYTES];
    size_t   buflen;
    size_t   outlen;
};

#define BLAKE2B_DIGEST_LENGTH 64
#define BLAKE2S_DIGEST_LENGTH 32

typedef struct blake2s_ctx_st BLAKE2S_CTX;
typedef struct blake2b_ctx_st BLAKE2B_CTX;

struct blake2b_md_data_st {
    BLAKE2B_CTX ctx;
    BLAKE2B_PARAM params;
};

struct blake2s_md_data_st {
    BLAKE2S_CTX ctx;
    BLAKE2S_PARAM params;
};

int ossl_blake2b_init(BLAKE2B_CTX *c, const BLAKE2B_PARAM *P);
int ossl_blake2b_init_key(BLAKE2B_CTX *c, const BLAKE2B_PARAM *P,
                          const void *key);
int ossl_blake2b_update(BLAKE2B_CTX *c, const void *data, size_t datalen);
int ossl_blake2b_final(unsigned char *md, BLAKE2B_CTX *c);

OSSL_FUNC_digest_get_ctx_params_fn ossl_blake2b_get_ctx_params;
OSSL_FUNC_digest_set_ctx_params_fn ossl_blake2b_set_ctx_params;
OSSL_FUNC_digest_gettable_ctx_params_fn ossl_blake2b_gettable_ctx_params;
OSSL_FUNC_digest_settable_ctx_params_fn ossl_blake2b_settable_ctx_params;

/*
 * These setters are internal and do not check the validity of their parameters.
 * See blake2b_mac_ctrl for validation logic.
 */

void ossl_blake2b_param_init(BLAKE2B_PARAM *P);
void ossl_blake2b_param_set_digest_length(BLAKE2B_PARAM *P, uint8_t outlen);
void ossl_blake2b_param_set_key_length(BLAKE2B_PARAM *P, uint8_t keylen);
void ossl_blake2b_param_set_personal(BLAKE2B_PARAM *P, const uint8_t *personal,
                                     size_t length);
void ossl_blake2b_param_set_salt(BLAKE2B_PARAM *P, const uint8_t *salt,
                                 size_t length);
int ossl_blake2s_init(BLAKE2S_CTX *c, const BLAKE2S_PARAM *P);
int ossl_blake2s_init_key(BLAKE2S_CTX *c, const BLAKE2S_PARAM *P,
                          const void *key);
int ossl_blake2s_update(BLAKE2S_CTX *c, const void *data, size_t datalen);
int ossl_blake2s_final(unsigned char *md, BLAKE2S_CTX *c);

void ossl_blake2s_param_init(BLAKE2S_PARAM *P);
void ossl_blake2s_param_set_digest_length(BLAKE2S_PARAM *P, uint8_t outlen);
void ossl_blake2s_param_set_key_length(BLAKE2S_PARAM *P, uint8_t keylen);
void ossl_blake2s_param_set_personal(BLAKE2S_PARAM *P, const uint8_t *personal,
                                     size_t length);
void ossl_blake2s_param_set_salt(BLAKE2S_PARAM *P, const uint8_t *salt,
                                 size_t length);

OSSL_FUNC_digest_get_ctx_params_fn ossl_blake2s_get_ctx_params;
OSSL_FUNC_digest_set_ctx_params_fn ossl_blake2s_set_ctx_params;
OSSL_FUNC_digest_gettable_ctx_params_fn ossl_blake2s_gettable_ctx_params;
OSSL_FUNC_digest_settable_ctx_params_fn ossl_blake2s_settable_ctx_params;

#endif /* OSSL_PROV_BLAKE2_H */
