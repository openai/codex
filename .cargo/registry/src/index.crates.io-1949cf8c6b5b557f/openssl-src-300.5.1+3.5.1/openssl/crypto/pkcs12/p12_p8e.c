/*
 * Copyright 2001-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/core.h>
#include <openssl/pkcs12.h>
#include "crypto/x509.h"

X509_SIG *PKCS8_encrypt_ex(int pbe_nid, const EVP_CIPHER *cipher,
                           const char *pass, int passlen,
                           unsigned char *salt, int saltlen, int iter,
                           PKCS8_PRIV_KEY_INFO *p8inf,
                           OSSL_LIB_CTX *libctx, const char *propq)
{
    X509_SIG *p8 = NULL;
    X509_ALGOR *pbe;

    if (pbe_nid == -1) {
        if (cipher == NULL) {
            ERR_raise(ERR_LIB_PKCS12, ERR_R_PASSED_NULL_PARAMETER);
            return NULL;
        }
        pbe = PKCS5_pbe2_set_iv_ex(cipher, iter, salt, saltlen, NULL, -1,
                                   libctx);
    } else {
        ERR_set_mark();
        if (EVP_PBE_find(EVP_PBE_TYPE_PRF, pbe_nid, NULL, NULL, 0)) {
            ERR_clear_last_mark();
            if (cipher == NULL) {
                ERR_raise(ERR_LIB_PKCS12, ERR_R_PASSED_NULL_PARAMETER);
                return NULL;
            }
            pbe = PKCS5_pbe2_set_iv_ex(cipher, iter, salt, saltlen, NULL,
                                       pbe_nid, libctx);
        } else {
            ERR_pop_to_mark();
            pbe = PKCS5_pbe_set_ex(pbe_nid, iter, salt, saltlen, libctx);
        }
    }
    if (pbe == NULL) {
        ERR_raise(ERR_LIB_PKCS12, ERR_R_ASN1_LIB);
        return NULL;
    }
    p8 = PKCS8_set0_pbe_ex(pass, passlen, p8inf, pbe, libctx, propq);
    if (p8 == NULL) {
        X509_ALGOR_free(pbe);
        return NULL;
    }

    return p8;
}

X509_SIG *PKCS8_encrypt(int pbe_nid, const EVP_CIPHER *cipher,
                        const char *pass, int passlen,
                        unsigned char *salt, int saltlen, int iter,
                        PKCS8_PRIV_KEY_INFO *p8inf)
{
    return PKCS8_encrypt_ex(pbe_nid, cipher, pass, passlen, salt, saltlen, iter,
                            p8inf, NULL, NULL);
}

X509_SIG *PKCS8_set0_pbe_ex(const char *pass, int passlen,
                            PKCS8_PRIV_KEY_INFO *p8inf, X509_ALGOR *pbe,
                            OSSL_LIB_CTX *ctx, const char *propq)
{
    X509_SIG *p8;
    ASN1_OCTET_STRING *enckey;

    enckey =
        PKCS12_item_i2d_encrypt_ex(pbe, ASN1_ITEM_rptr(PKCS8_PRIV_KEY_INFO),
                                   pass, passlen, p8inf, 1, ctx, propq);
    if (!enckey) {
        ERR_raise(ERR_LIB_PKCS12, PKCS12_R_ENCRYPT_ERROR);
        return NULL;
    }

    p8 = OPENSSL_zalloc(sizeof(*p8));

    if (p8 == NULL) {
        ASN1_OCTET_STRING_free(enckey);
        return NULL;
    }
    p8->algor = pbe;
    p8->digest = enckey;

    return p8;
}

X509_SIG *PKCS8_set0_pbe(const char *pass, int passlen,
                         PKCS8_PRIV_KEY_INFO *p8inf, X509_ALGOR *pbe)
{
    return PKCS8_set0_pbe_ex(pass, passlen, p8inf, pbe, NULL, NULL);
}
