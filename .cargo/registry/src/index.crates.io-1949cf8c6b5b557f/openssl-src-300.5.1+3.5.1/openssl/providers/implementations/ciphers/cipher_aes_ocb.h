/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/aes.h>
#include "prov/ciphercommon.h"
#include "crypto/aes_platform.h"

#define OCB_MAX_TAG_LEN     AES_BLOCK_SIZE
#define OCB_MAX_DATA_LEN    AES_BLOCK_SIZE
#define OCB_MAX_AAD_LEN     AES_BLOCK_SIZE

typedef struct prov_aes_ocb_ctx_st {
    PROV_CIPHER_CTX base;       /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ksenc;                    /* AES key schedule to use for encryption/aad */
    union {
        OSSL_UNION_ALIGN;
        AES_KEY ks;
    } ksdec;                    /* AES key schedule to use for decryption */
    OCB128_CONTEXT ocb;
    unsigned int iv_state;      /* set to one of IV_STATE_XXX */
    unsigned int key_set : 1;
    size_t taglen;
    size_t data_buf_len;
    size_t aad_buf_len;
    unsigned char tag[OCB_MAX_TAG_LEN];
    unsigned char data_buf[OCB_MAX_DATA_LEN]; /* Store partial data blocks */
    unsigned char aad_buf[OCB_MAX_AAD_LEN];   /* Store partial AAD blocks */
} PROV_AES_OCB_CTX;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_aes_ocb(size_t keybits);
