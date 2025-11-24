/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PEM_H
# define OSSL_INTERNAL_PEM_H
# pragma once

# include <openssl/pem.h>
# include "crypto/types.h"

/* Found in crypto/pem/pvkfmt.c */

/* Maximum length of a blob after header */
# define BLOB_MAX_LENGTH          102400

int ossl_do_blob_header(const unsigned char **in, unsigned int length,
                        unsigned int *pmagic, unsigned int *pbitlen,
                        int *pisdss, int *pispub);
unsigned int ossl_blob_length(unsigned bitlen, int isdss, int ispub);
int ossl_do_PVK_header(const unsigned char **in, unsigned int length,
                       int skip_magic, int *isdss,
                       unsigned int *psaltlen, unsigned int *pkeylen);
# ifndef OPENSSL_NO_DEPRECATED_3_0
#  ifndef OPENSSL_NO_DSA
DSA *ossl_b2i_DSA_after_header(const unsigned char **in, unsigned int bitlen,
                               int ispub);
#  endif
RSA *ossl_b2i_RSA_after_header(const unsigned char **in, unsigned int bitlen,
                               int ispub);
# endif
EVP_PKEY *ossl_b2i(const unsigned char **in, unsigned int length, int *ispub);
EVP_PKEY *ossl_b2i_bio(BIO *in, int *ispub);

# ifndef OPENSSL_NO_DEPRECATED_3_0
#  ifndef OPENSSL_NO_DSA
DSA *b2i_DSA_PVK_bio(BIO *in, pem_password_cb *cb, void *u);
DSA *b2i_DSA_PVK_bio_ex(BIO *in, pem_password_cb *cb, void *u,
                        OSSL_LIB_CTX *libctx, const char *propq);
#  endif
RSA *b2i_RSA_PVK_bio(BIO *in, pem_password_cb *cb, void *u);
RSA *b2i_RSA_PVK_bio_ex(BIO *in, pem_password_cb *cb, void *u,
                        OSSL_LIB_CTX *libctx, const char *propq);
# endif

#endif
