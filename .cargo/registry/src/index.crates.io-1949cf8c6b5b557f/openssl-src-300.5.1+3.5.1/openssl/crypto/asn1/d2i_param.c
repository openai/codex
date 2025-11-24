/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include <openssl/evp.h>
#include <openssl/asn1.h>
#include "internal/asn1.h"
#include "crypto/asn1.h"
#include "crypto/evp.h"

EVP_PKEY *d2i_KeyParams(int type, EVP_PKEY **a, const unsigned char **pp,
                        long length)
{
    EVP_PKEY *ret = NULL;

    if ((a == NULL) || (*a == NULL)) {
        if ((ret = EVP_PKEY_new()) == NULL)
            return NULL;
    } else
        ret = *a;

    if (type != EVP_PKEY_get_id(ret) && !EVP_PKEY_set_type(ret, type))
        goto err;

    if (ret->ameth == NULL || ret->ameth->param_decode == NULL) {
        ERR_raise(ERR_LIB_ASN1, ASN1_R_UNSUPPORTED_TYPE);
        goto err;
    }

    if (!ret->ameth->param_decode(ret, pp, length))
        goto err;

    if (a != NULL)
        (*a) = ret;
    return ret;
err:
    if (a == NULL || *a != ret)
        EVP_PKEY_free(ret);
    return NULL;
}

EVP_PKEY *d2i_KeyParams_bio(int type, EVP_PKEY **a, BIO *in)
{
    BUF_MEM *b = NULL;
    const unsigned char *p;
    void *ret = NULL;
    int len;

    len = asn1_d2i_read_bio(in, &b);
    if (len < 0)
        goto err;

    p = (unsigned char *)b->data;
    ret = d2i_KeyParams(type, a, &p, len);
err:
    BUF_MEM_free(b);
    return ret;
}
