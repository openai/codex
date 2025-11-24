/*
 * Copyright 1999-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/core.h>
#include <openssl/core_names.h>
#include "crypto/evp.h"
#include <openssl/pkcs12.h>

/* PKCS#12 PBE algorithms now in static table */

void PKCS12_PBE_add(void)
{
}

int PKCS12_PBE_keyivgen_ex(EVP_CIPHER_CTX *ctx, const char *pass, int passlen,
                           ASN1_TYPE *param, const EVP_CIPHER *cipher,
                           const EVP_MD *md, int en_de,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    PBEPARAM *pbe;
    int saltlen, iter, ret;
    unsigned char *salt;
    unsigned char key[EVP_MAX_KEY_LENGTH], iv[EVP_MAX_IV_LENGTH];
    unsigned char *piv = iv;

    if (cipher == NULL)
        return 0;

    /* Extract useful info from parameter */

    pbe = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBEPARAM), param);
    if (pbe == NULL) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_DECODE_ERROR);
        return 0;
    }

    if (pbe->iter == NULL)
        iter = 1;
    else
        iter = ASN1_INTEGER_get(pbe->iter);
    salt = pbe->salt->data;
    saltlen = pbe->salt->length;
    if (!PKCS12_key_gen_utf8_ex(pass, passlen, salt, saltlen, PKCS12_KEY_ID,
                                iter, EVP_CIPHER_get_key_length(cipher),
                                key, md,
                                libctx, propq)) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_KEY_GEN_ERROR);
        PBEPARAM_free(pbe);
        return 0;
    }
    if (EVP_CIPHER_get_iv_length(cipher) > 0) {
        if (!PKCS12_key_gen_utf8_ex(pass, passlen, salt, saltlen, PKCS12_IV_ID,
                                    iter, EVP_CIPHER_get_iv_length(cipher),
                                    iv, md,
                                    libctx, propq)) {
            ERR_raise(ERR_LIB_PKCS12, PKCS12_R_IV_GEN_ERROR);
            PBEPARAM_free(pbe);
            return 0;
        }
    } else {
        piv = NULL;
    }
    PBEPARAM_free(pbe);
    ret = EVP_CipherInit_ex(ctx, cipher, NULL, key, piv, en_de);
    OPENSSL_cleanse(key, EVP_MAX_KEY_LENGTH);
    OPENSSL_cleanse(iv, EVP_MAX_IV_LENGTH);
    return ret;
}

int PKCS12_PBE_keyivgen(EVP_CIPHER_CTX *ctx, const char *pass, int passlen,
                        ASN1_TYPE *param, const EVP_CIPHER *cipher,
                        const EVP_MD *md, int en_de)
{
    return PKCS12_PBE_keyivgen_ex(ctx, pass, passlen, param, cipher, md, en_de,
                                  NULL, NULL);
}

