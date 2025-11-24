/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Legacy EVP_PKEY assign/set/get APIs are deprecated for public use, but
 * still ok for internal use, particularly in providers.
 */
#include "internal/deprecated.h"

#include <openssl/types.h>
#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/rsa.h>
#include <openssl/ec.h>
#include "crypto/types.h"
#include "crypto/evp.h"
#include "evp_local.h"

int EVP_PKEY_set1_RSA(EVP_PKEY *pkey, RSA *key)
{
    int ret;

    if (!RSA_up_ref(key))
        return 0;

    ret = EVP_PKEY_assign_RSA(pkey, key);

    if (!ret)
        RSA_free(key);

    return ret;
}

RSA *evp_pkey_get0_RSA_int(const EVP_PKEY *pkey)
{
    if (pkey->type != EVP_PKEY_RSA && pkey->type != EVP_PKEY_RSA_PSS) {
        ERR_raise(ERR_LIB_EVP, EVP_R_EXPECTING_AN_RSA_KEY);
        return NULL;
    }
    return evp_pkey_get_legacy((EVP_PKEY *)pkey);
}

const RSA *EVP_PKEY_get0_RSA(const EVP_PKEY *pkey)
{
    return evp_pkey_get0_RSA_int(pkey);
}

RSA *EVP_PKEY_get1_RSA(EVP_PKEY *pkey)
{
    RSA *ret = evp_pkey_get0_RSA_int(pkey);

    if (ret != NULL && !RSA_up_ref(ret))
        ret = NULL;

    return ret;
}

#ifndef OPENSSL_NO_EC
int EVP_PKEY_set1_EC_KEY(EVP_PKEY *pkey, EC_KEY *key)
{
    if (!EC_KEY_up_ref(key))
        return 0;
    if (!EVP_PKEY_assign_EC_KEY(pkey, key)) {
        EC_KEY_free(key);
        return 0;
    }
    return 1;
}

EC_KEY *evp_pkey_get0_EC_KEY_int(const EVP_PKEY *pkey)
{
    if (EVP_PKEY_get_base_id(pkey) != EVP_PKEY_EC) {
        ERR_raise(ERR_LIB_EVP, EVP_R_EXPECTING_A_EC_KEY);
        return NULL;
    }
    return evp_pkey_get_legacy((EVP_PKEY *)pkey);
}

const EC_KEY *EVP_PKEY_get0_EC_KEY(const EVP_PKEY *pkey)
{
    return evp_pkey_get0_EC_KEY_int(pkey);
}

EC_KEY *EVP_PKEY_get1_EC_KEY(EVP_PKEY *pkey)
{
    EC_KEY *ret = evp_pkey_get0_EC_KEY_int(pkey);

    if (ret != NULL && !EC_KEY_up_ref(ret))
        ret = NULL;
    return ret;
}
#endif /* OPENSSL_NO_EC */
