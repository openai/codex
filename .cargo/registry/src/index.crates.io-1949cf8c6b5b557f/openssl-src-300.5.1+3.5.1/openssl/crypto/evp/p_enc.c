/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use the deprecated RSA low level calls */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/rsa.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/x509.h>
#include "crypto/evp.h"

int EVP_PKEY_encrypt_old(unsigned char *ek, const unsigned char *key,
                         int key_len, EVP_PKEY *pubk)
{
    int ret = 0;
    RSA *rsa = NULL;

    if (EVP_PKEY_get_id(pubk) != EVP_PKEY_RSA) {
        ERR_raise(ERR_LIB_EVP, EVP_R_PUBLIC_KEY_NOT_RSA);
        goto err;
    }

    rsa = evp_pkey_get0_RSA_int(pubk);
    if (rsa == NULL)
        goto err;

    ret =
        RSA_public_encrypt(key_len, key, ek, rsa, RSA_PKCS1_PADDING);
 err:
    return ret;
}
