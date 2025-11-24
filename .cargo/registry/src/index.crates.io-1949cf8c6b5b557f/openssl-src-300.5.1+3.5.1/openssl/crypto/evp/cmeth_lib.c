/*
 * Copyright 2015-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * EVP _meth_ APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <string.h>

#include <openssl/evp.h>
#include "crypto/evp.h"
#include "internal/provider.h"
#include "evp_local.h"

EVP_CIPHER *EVP_CIPHER_meth_new(int cipher_type, int block_size, int key_len)
{
    EVP_CIPHER *cipher = evp_cipher_new();

    if (cipher != NULL) {
        cipher->nid = cipher_type;
        cipher->block_size = block_size;
        cipher->key_len = key_len;
        cipher->origin = EVP_ORIG_METH;
    }
    return cipher;
}

EVP_CIPHER *EVP_CIPHER_meth_dup(const EVP_CIPHER *cipher)
{
    EVP_CIPHER *to = NULL;

    /*
     * Non-legacy EVP_CIPHERs can't be duplicated like this.
     * Use EVP_CIPHER_up_ref() instead.
     */
    if (cipher->prov != NULL)
        return NULL;

    if ((to = EVP_CIPHER_meth_new(cipher->nid, cipher->block_size,
                                  cipher->key_len)) != NULL) {
        CRYPTO_REF_COUNT refcnt = to->refcnt;

        memcpy(to, cipher, sizeof(*to));
        to->refcnt = refcnt;
        to->origin = EVP_ORIG_METH;
    }
    return to;
}

void EVP_CIPHER_meth_free(EVP_CIPHER *cipher)
{
    if (cipher == NULL || cipher->origin != EVP_ORIG_METH)
       return;

    evp_cipher_free_int(cipher);
}

int EVP_CIPHER_meth_set_iv_length(EVP_CIPHER *cipher, int iv_len)
{
    if (cipher->iv_len != 0)
        return 0;

    cipher->iv_len = iv_len;
    return 1;
}

int EVP_CIPHER_meth_set_flags(EVP_CIPHER *cipher, unsigned long flags)
{
    if (cipher->flags != 0)
        return 0;

    cipher->flags = flags;
    return 1;
}

int EVP_CIPHER_meth_set_impl_ctx_size(EVP_CIPHER *cipher, int ctx_size)
{
    if (cipher->ctx_size != 0)
        return 0;

    cipher->ctx_size = ctx_size;
    return 1;
}

int EVP_CIPHER_meth_set_init(EVP_CIPHER *cipher,
                             int (*init) (EVP_CIPHER_CTX *ctx,
                                          const unsigned char *key,
                                          const unsigned char *iv,
                                          int enc))
{
    if (cipher->init != NULL)
        return 0;

    cipher->init = init;
    return 1;
}

int EVP_CIPHER_meth_set_do_cipher(EVP_CIPHER *cipher,
                                  int (*do_cipher) (EVP_CIPHER_CTX *ctx,
                                                    unsigned char *out,
                                                    const unsigned char *in,
                                                    size_t inl))
{
    if (cipher->do_cipher != NULL)
        return 0;

    cipher->do_cipher = do_cipher;
    return 1;
}

int EVP_CIPHER_meth_set_cleanup(EVP_CIPHER *cipher,
                                int (*cleanup) (EVP_CIPHER_CTX *))
{
    if (cipher->cleanup != NULL)
        return 0;

    cipher->cleanup = cleanup;
    return 1;
}

int EVP_CIPHER_meth_set_set_asn1_params(EVP_CIPHER *cipher,
                                        int (*set_asn1_parameters) (EVP_CIPHER_CTX *,
                                                                    ASN1_TYPE *))
{
    if (cipher->set_asn1_parameters != NULL)
        return 0;

    cipher->set_asn1_parameters = set_asn1_parameters;
    return 1;
}

int EVP_CIPHER_meth_set_get_asn1_params(EVP_CIPHER *cipher,
                                        int (*get_asn1_parameters) (EVP_CIPHER_CTX *,
                                                                    ASN1_TYPE *))
{
    if (cipher->get_asn1_parameters != NULL)
        return 0;

    cipher->get_asn1_parameters = get_asn1_parameters;
    return 1;
}

int EVP_CIPHER_meth_set_ctrl(EVP_CIPHER *cipher,
                             int (*ctrl) (EVP_CIPHER_CTX *, int type,
                                          int arg, void *ptr))
{
    if (cipher->ctrl != NULL)
        return 0;

    cipher->ctrl = ctrl;
    return 1;
}


int (*EVP_CIPHER_meth_get_init(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *ctx,
                                                          const unsigned char *key,
                                                          const unsigned char *iv,
                                                          int enc)
{
    return cipher->init;
}
int (*EVP_CIPHER_meth_get_do_cipher(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *ctx,
                                                               unsigned char *out,
                                                               const unsigned char *in,
                                                               size_t inl)
{
    return cipher->do_cipher;
}

int (*EVP_CIPHER_meth_get_cleanup(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *)
{
    return cipher->cleanup;
}

int (*EVP_CIPHER_meth_get_set_asn1_params(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *,
                                                                     ASN1_TYPE *)
{
    return cipher->set_asn1_parameters;
}

int (*EVP_CIPHER_meth_get_get_asn1_params(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *,
                                                               ASN1_TYPE *)
{
    return cipher->get_asn1_parameters;
}

int (*EVP_CIPHER_meth_get_ctrl(const EVP_CIPHER *cipher))(EVP_CIPHER_CTX *,
                                                          int type, int arg,
                                                          void *ptr)
{
    return cipher->ctrl;
}

