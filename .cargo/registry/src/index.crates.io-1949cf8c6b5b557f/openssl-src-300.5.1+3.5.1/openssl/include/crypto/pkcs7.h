/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_PKCS7_H
# define OSSL_CRYPTO_PKCS7_H
# pragma once

void ossl_pkcs7_resolve_libctx(PKCS7 *p7);

void ossl_pkcs7_set0_libctx(PKCS7 *p7, OSSL_LIB_CTX *ctx);
int ossl_pkcs7_set1_propq(PKCS7 *p7, const char *propq);

#endif
