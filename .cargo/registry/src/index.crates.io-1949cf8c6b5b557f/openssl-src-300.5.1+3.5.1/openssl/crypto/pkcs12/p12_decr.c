/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/pkcs12.h>
#include <openssl/trace.h>

/*
 * Encrypt/Decrypt a buffer based on password and algor, result in a
 * OPENSSL_malloc'ed buffer
 */
unsigned char *PKCS12_pbe_crypt_ex(const X509_ALGOR *algor,
                                   const char *pass, int passlen,
                                   const unsigned char *in, int inlen,
                                   unsigned char **data, int *datalen, int en_de,
                                   OSSL_LIB_CTX *libctx, const char *propq)
{
    unsigned char *out = NULL;
    int outlen, i;
    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    int max_out_len, mac_len = 0;
    int block_size;

    if (ctx == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_EVP_LIB);
        goto err;
    }

    /* Process data */
    if (!EVP_PBE_CipherInit_ex(algor->algorithm, pass, passlen,
                               algor->parameter, ctx, en_de, libctx, propq))
        goto err;

    /*
     * GOST algorithm specifics:
     * OMAC algorithm calculate and encrypt MAC of the encrypted objects
     * It's appended to encrypted text on encrypting
     * MAC should be processed on decrypting separately from plain text
     */
    block_size = EVP_CIPHER_CTX_get_block_size(ctx);

    if (block_size == 0) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_PASSED_NULL_PARAMETER);
        goto err;
    }

    max_out_len = inlen + block_size;
    if ((EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(ctx))
                & EVP_CIPH_FLAG_CIPHER_WITH_MAC) != 0) {
        if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_TLS1_AAD, 0, &mac_len) < 0) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_INTERNAL_ERROR);
            goto err;
        }

        if (EVP_CIPHER_CTX_is_encrypting(ctx)) {
            max_out_len += mac_len;
        } else {
            if (inlen < mac_len) {
                ERR_raise(ERR_LIB_PKCS12, PKCS12_R_UNSUPPORTED_PKCS12_MODE);
                goto err;
            }
            inlen -= mac_len;
            if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG,
                                    (int)mac_len, (unsigned char *)in+inlen) < 0) {
                ERR_raise(ERR_LIB_PKCS12, ERR_R_INTERNAL_ERROR);
                goto err;
            }
        }
    }

    if ((out = OPENSSL_malloc(max_out_len)) == NULL)
        goto err;

    if (!EVP_CipherUpdate(ctx, out, &i, in, inlen)) {
        OPENSSL_free(out);
        out = NULL;
        ERR_raise(ERR_LIB_PKCS12, ERR_R_EVP_LIB);
        goto err;
    }

    outlen = i;
    if (!EVP_CipherFinal_ex(ctx, out + i, &i)) {
        OPENSSL_free(out);
        out = NULL;
        ERR_raise_data(ERR_LIB_PKCS12, PKCS12_R_PKCS12_CIPHERFINAL_ERROR,
                       passlen == 0 ? "empty password"
                       : "maybe wrong password");
        goto err;
    }
    outlen += i;
    if ((EVP_CIPHER_get_flags(EVP_CIPHER_CTX_get0_cipher(ctx))
                & EVP_CIPH_FLAG_CIPHER_WITH_MAC) != 0) {
        if (EVP_CIPHER_CTX_is_encrypting(ctx)) {
            if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG,
                (int)mac_len, out+outlen) < 0) {
                OPENSSL_free(out);
                out = NULL;
                ERR_raise(ERR_LIB_PKCS12, ERR_R_INTERNAL_ERROR);
                goto err;
            }
            outlen += mac_len;
        }
    }
    if (datalen)
        *datalen = outlen;
    if (data)
        *data = out;
 err:
    EVP_CIPHER_CTX_free(ctx);
    return out;

}

unsigned char *PKCS12_pbe_crypt(const X509_ALGOR *algor,
                                const char *pass, int passlen,
                                const unsigned char *in, int inlen,
                                unsigned char **data, int *datalen, int en_de)
{
    return PKCS12_pbe_crypt_ex(algor, pass, passlen, in, inlen, data, datalen,
                               en_de, NULL, NULL);
}

/*
 * Decrypt an OCTET STRING and decode ASN1 structure if zbuf set zero buffer
 * after use.
 */

void *PKCS12_item_decrypt_d2i_ex(const X509_ALGOR *algor, const ASN1_ITEM *it,
                                 const char *pass, int passlen,
                                 const ASN1_OCTET_STRING *oct, int zbuf,
                                 OSSL_LIB_CTX *libctx,
                                 const char *propq)
{
    unsigned char *out = NULL;
    const unsigned char *p;
    void *ret;
    int outlen = 0;

    if (!PKCS12_pbe_crypt_ex(algor, pass, passlen, oct->data, oct->length,
                             &out, &outlen, 0, libctx, propq))
        return NULL;
    p = out;
    OSSL_TRACE_BEGIN(PKCS12_DECRYPT) {
        BIO_printf(trc_out, "\n");
        BIO_dump(trc_out, out, outlen);
        BIO_printf(trc_out, "\n");
    } OSSL_TRACE_END(PKCS12_DECRYPT);
    ret = ASN1_item_d2i(NULL, &p, outlen, it);
    if (zbuf)
        OPENSSL_cleanse(out, outlen);
    if (!ret)
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
    OPENSSL_free(out);
    return ret;
}

void *PKCS12_item_decrypt_d2i(const X509_ALGOR *algor, const ASN1_ITEM *it,
                              const char *pass, int passlen,
                              const ASN1_OCTET_STRING *oct, int zbuf)
{
    return PKCS12_item_decrypt_d2i_ex(algor, it, pass, passlen, oct, zbuf,
                                      NULL, NULL);
}

/*
 * Encode ASN1 structure and encrypt, return OCTET STRING if zbuf set zero
 * encoding.
 */

ASN1_OCTET_STRING *PKCS12_item_i2d_encrypt_ex(X509_ALGOR *algor,
                                              const ASN1_ITEM *it,
                                              const char *pass, int passlen,
                                              void *obj, int zbuf,
                                              OSSL_LIB_CTX *ctx,
                                              const char *propq)
{
    ASN1_OCTET_STRING *oct = NULL;
    unsigned char *in = NULL;
    int inlen;

    if ((oct = ASN1_OCTET_STRING_new()) == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        goto err;
    }
    inlen = ASN1_item_i2d(obj, &in, it);
    if (!in) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ENCODE_ERROR);
        goto err;
    }
    if (!PKCS12_pbe_crypt_ex(algor, pass, passlen, in, inlen, &oct->data,
                             &oct->length, 1, ctx, propq)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ENCRYPT_ERROR);
        OPENSSL_free(in);
        goto err;
    }
    if (zbuf)
        OPENSSL_cleanse(in, inlen);
    OPENSSL_free(in);
    return oct;
 err:
    ASN1_OCTET_STRING_free(oct);
    return NULL;
}

ASN1_OCTET_STRING *PKCS12_item_i2d_encrypt(X509_ALGOR *algor,
                                           const ASN1_ITEM *it,
                                           const char *pass, int passlen,
                                           void *obj, int zbuf)
{
    return PKCS12_item_i2d_encrypt_ex(algor, it, pass, passlen, obj, zbuf, NULL, NULL);
}
