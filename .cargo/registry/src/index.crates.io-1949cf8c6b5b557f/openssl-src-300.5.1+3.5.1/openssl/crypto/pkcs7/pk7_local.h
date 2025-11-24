/*
 * Copyright 2020-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/pkcs7.h"

STACK_OF(X509) *pkcs7_get0_certificates(const PKCS7 *p7);
const PKCS7_CTX *ossl_pkcs7_get0_ctx(const PKCS7 *p7);
OSSL_LIB_CTX *ossl_pkcs7_ctx_get0_libctx(const PKCS7_CTX *ctx);
const char *ossl_pkcs7_ctx_get0_propq(const PKCS7_CTX *ctx);

int ossl_pkcs7_ctx_propagate(const PKCS7 *from, PKCS7 *to);
