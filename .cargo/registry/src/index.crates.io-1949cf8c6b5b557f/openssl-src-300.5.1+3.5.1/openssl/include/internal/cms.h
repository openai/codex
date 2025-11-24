/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_INTERNAL_CMS_H
# define OSSL_INTERNAL_CMS_H
# pragma once

# include <openssl/cms.h>

# ifndef OPENSSL_NO_CMS
CMS_EnvelopedData *ossl_cms_sign_encrypt(BIO *data, X509 *sign_cert, STACK_OF(X509) *certs,
                                         EVP_PKEY *sign_key, unsigned int sign_flags,
                                         STACK_OF(X509) *enc_recip, const EVP_CIPHER *cipher,
                                         unsigned int enc_flags, OSSL_LIB_CTX *libctx,
                                         const char *propq);
# endif /* OPENSSL_NO_CMS */
#endif /* OSSL_INTERNAL_CMS_H */
