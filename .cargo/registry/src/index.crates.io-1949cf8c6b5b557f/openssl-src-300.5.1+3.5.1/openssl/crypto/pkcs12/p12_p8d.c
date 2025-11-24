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
#include <openssl/pkcs12.h>

PKCS8_PRIV_KEY_INFO *PKCS8_decrypt_ex(const X509_SIG *p8, const char *pass,
                                      int passlen, OSSL_LIB_CTX *ctx,
                                      const char *propq)
{
    const X509_ALGOR *dalg;
    const ASN1_OCTET_STRING *doct;

    X509_SIG_get0(p8, &dalg, &doct);
    return PKCS12_item_decrypt_d2i_ex(dalg,
                                   ASN1_ITEM_rptr(PKCS8_PRIV_KEY_INFO), pass,
                                   passlen, doct, 1, ctx, propq);
}

PKCS8_PRIV_KEY_INFO *PKCS8_decrypt(const X509_SIG *p8, const char *pass,
                                   int passlen)
{
    return PKCS8_decrypt_ex(p8, pass, passlen, NULL, NULL);
}

