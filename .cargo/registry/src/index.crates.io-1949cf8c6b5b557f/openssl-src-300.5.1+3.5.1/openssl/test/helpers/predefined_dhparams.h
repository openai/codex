/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>

#ifndef OPENSSL_NO_DH
EVP_PKEY *get_dh512(OSSL_LIB_CTX *libctx);
EVP_PKEY *get_dhx512(OSSL_LIB_CTX *libctx);
EVP_PKEY *get_dh1024dsa(OSSL_LIB_CTX *libct);
EVP_PKEY *get_dh2048(OSSL_LIB_CTX *libctx);
EVP_PKEY *get_dh4096(OSSL_LIB_CTX *libctx);
#endif
