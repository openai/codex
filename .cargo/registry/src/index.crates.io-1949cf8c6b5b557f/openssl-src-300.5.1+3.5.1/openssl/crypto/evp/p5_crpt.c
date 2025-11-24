/*
 * Copyright 1999-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include "internal/cryptlib.h"
#include <openssl/x509.h>
#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/kdf.h>

/*
 * Doesn't do anything now: Builtin PBE algorithms in static table.
 */

void PKCS5_PBE_add(void)
{
}

int PKCS5_PBE_keyivgen_ex(EVP_CIPHER_CTX *cctx, const char *pass, int passlen,
                          ASN1_TYPE *param, const EVP_CIPHER *cipher,
                          const EVP_MD *md, int en_de, OSSL_LIB_CTX *libctx,
                          const char *propq)
{
    unsigned char md_tmp[EVP_MAX_MD_SIZE];
    unsigned char key[EVP_MAX_KEY_LENGTH], iv[EVP_MAX_IV_LENGTH];
    int ivl, kl;
    PBEPARAM *pbe = NULL;
    int saltlen, iter;
    unsigned char *salt;
    int mdsize;
    int rv = 0;
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx = NULL;
    OSSL_PARAM params[5], *p = params;
    const char *mdname = EVP_MD_name(md);

    /* Extract useful info from parameter */
    if (param == NULL || param->type != V_ASN1_SEQUENCE ||
        param->value.sequence == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DECODE_ERROR);
        return 0;
    }

    pbe = ASN1_TYPE_unpack_sequence(ASN1_ITEM_rptr(PBEPARAM), param);
    if (pbe == NULL) {
        ERR_raise(ERR_LIB_EVP, EVP_R_DECODE_ERROR);
        return 0;
    }

    ivl = EVP_CIPHER_get_iv_length(cipher);
    if (ivl < 0 || ivl > 16) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_IV_LENGTH);
        goto err;
    }
    kl = EVP_CIPHER_get_key_length(cipher);
    if (kl < 0 || kl > (int)sizeof(md_tmp)) {
        ERR_raise(ERR_LIB_EVP, EVP_R_INVALID_KEY_LENGTH);
        goto err;
    }

    if (pbe->iter == NULL)
        iter = 1;
    else
        iter = ASN1_INTEGER_get(pbe->iter);
    salt = pbe->salt->data;
    saltlen = pbe->salt->length;

    if (pass == NULL)
        passlen = 0;
    else if (passlen == -1)
        passlen = strlen(pass);

    mdsize = EVP_MD_get_size(md);
    if (mdsize <= 0)
        goto err;

    kdf = EVP_KDF_fetch(libctx, OSSL_KDF_NAME_PBKDF1, propq);
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL)
        goto err;
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                             (char *)pass, (size_t)passlen);
    *p++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             salt, saltlen);
    *p++ = OSSL_PARAM_construct_int(OSSL_KDF_PARAM_ITER, &iter);
    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_KDF_PARAM_DIGEST,
                                            (char *)mdname, 0);
    *p = OSSL_PARAM_construct_end();
    if (EVP_KDF_derive(kctx, md_tmp, mdsize, params) != 1)
        goto err;
    memcpy(key, md_tmp, kl);
    memcpy(iv, md_tmp + (16 - ivl), ivl);
    if (!EVP_CipherInit_ex(cctx, cipher, NULL, key, iv, en_de))
        goto err;
    OPENSSL_cleanse(md_tmp, EVP_MAX_MD_SIZE);
    OPENSSL_cleanse(key, EVP_MAX_KEY_LENGTH);
    OPENSSL_cleanse(iv, EVP_MAX_IV_LENGTH);
    rv = 1;
 err:
    EVP_KDF_CTX_free(kctx);
    PBEPARAM_free(pbe);
    return rv;
}

int PKCS5_PBE_keyivgen(EVP_CIPHER_CTX *cctx, const char *pass, int passlen,
                       ASN1_TYPE *param, const EVP_CIPHER *cipher,
                       const EVP_MD *md, int en_de)
{
    return PKCS5_PBE_keyivgen_ex(cctx, pass, passlen, param, cipher, md, en_de,
                                 NULL, NULL);
}

