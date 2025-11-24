/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DSA low level APIs are deprecated for public use, but still ok for
 * internal use.
 */
#include "internal/deprecated.h"

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/bn.h>
#include <openssl/evp.h>
#include <openssl/objects.h>
#include <openssl/asn1.h>
#include <openssl/rsa.h>
#include <openssl/dsa.h>
#include <openssl/ec.h>

#include "crypto/evp.h"

EVP_PKEY *d2i_PublicKey(int type, EVP_PKEY **a, const unsigned char **pp,
                        long length)
{
    EVP_PKEY *ret;
    EVP_PKEY *copy = NULL;

    if ((a == NULL) || (*a == NULL)) {
        if ((ret = EVP_PKEY_new()) == NULL) {
            ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
            return NULL;
        }
    } else {
        ret = *a;

#ifndef OPENSSL_NO_EC
        if (evp_pkey_is_provided(ret)
            && EVP_PKEY_get_base_id(ret) == EVP_PKEY_EC) {
            if (!evp_pkey_copy_downgraded(&copy, ret))
                goto err;
        }
#endif
    }

    if ((type != EVP_PKEY_get_id(ret) || copy != NULL)
        && !EVP_PKEY_set_type(ret, type)) {
        ERR_raise(ERR_LIB_ASN1, ERR_R_EVP_LIB);
        goto err;
    }

    switch (EVP_PKEY_get_base_id(ret)) {
    case EVP_PKEY_RSA:
        if ((ret->pkey.rsa = d2i_RSAPublicKey(NULL, pp, length)) == NULL) {
            ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
            goto err;
        }
        break;
#ifndef OPENSSL_NO_DSA
    case EVP_PKEY_DSA:
        if (!d2i_DSAPublicKey(&ret->pkey.dsa, pp, length)) {
            ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
            goto err;
        }
        break;
#endif
#ifndef OPENSSL_NO_EC
    case EVP_PKEY_EC:
        if (copy != NULL) {
            /* use downgraded parameters from copy */
            ret->pkey.ec = copy->pkey.ec;
            copy->pkey.ec = NULL;
        }
        if (!o2i_ECPublicKey(&ret->pkey.ec, pp, length)) {
            ERR_raise(ERR_LIB_ASN1, ERR_R_ASN1_LIB);
            goto err;
        }
        break;
#endif
    default:
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNKNOWN_PUBLIC_KEY_TYPE);
        goto err;
    }
    if (a != NULL)
        (*a) = ret;
    EVP_PKEY_free(copy);
    return ret;
 err:
    if (a == NULL || *a != ret)
        EVP_PKEY_free(ret);
    EVP_PKEY_free(copy);
    return NULL;
}
