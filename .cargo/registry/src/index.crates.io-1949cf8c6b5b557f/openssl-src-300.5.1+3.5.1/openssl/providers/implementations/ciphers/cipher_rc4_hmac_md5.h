/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rc4.h>
#include <openssl/md5.h>
#include "prov/ciphercommon.h"

typedef struct prov_rc4_hmac_md5_ctx_st {
    PROV_CIPHER_CTX base;      /* Must be first */
    union {
        OSSL_UNION_ALIGN;
        RC4_KEY ks;
    } ks;
    MD5_CTX head, tail, md;
    size_t payload_length;
    size_t tls_aad_pad_sz;
} PROV_RC4_HMAC_MD5_CTX;

typedef struct prov_cipher_hw_rc4_hmac_md5_st {
    PROV_CIPHER_HW base; /* Must be first */
    int (*tls_init)(PROV_CIPHER_CTX *ctx, unsigned char *aad, size_t aad_len);
    void (*init_mackey)(PROV_CIPHER_CTX *ctx, const unsigned char *key,
                        size_t len);

} PROV_CIPHER_HW_RC4_HMAC_MD5;

const PROV_CIPHER_HW *ossl_prov_cipher_hw_rc4_hmac_md5(size_t keybits);

void rc4_md5_enc(RC4_KEY *key, const void *in0, void *out,
                 MD5_CTX *ctx, const void *inp, size_t blocks);
