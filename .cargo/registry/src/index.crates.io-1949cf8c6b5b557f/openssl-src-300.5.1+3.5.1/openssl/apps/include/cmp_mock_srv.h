/*
 * Copyright 2018-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2018-2020
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_APPS_CMP_MOCK_SRV_H
# define OSSL_APPS_CMP_MOCK_SRV_H

# include <openssl/opensslconf.h>
# ifndef OPENSSL_NO_CMP

#  include <openssl/cmp.h>

OSSL_CMP_SRV_CTX *ossl_cmp_mock_srv_new(OSSL_LIB_CTX *libctx,
                                        const char *propq);
void ossl_cmp_mock_srv_free(OSSL_CMP_SRV_CTX *srv_ctx);

int ossl_cmp_mock_srv_set1_refCert(OSSL_CMP_SRV_CTX *srv_ctx, X509 *cert);
int ossl_cmp_mock_srv_set1_certOut(OSSL_CMP_SRV_CTX *srv_ctx, X509 *cert);
int ossl_cmp_mock_srv_set1_keyOut(OSSL_CMP_SRV_CTX *srv_ctx, EVP_PKEY *pkey);
int ossl_cmp_mock_srv_set1_crlOut(OSSL_CMP_SRV_CTX *srv_ctx, X509_CRL *crl);
int ossl_cmp_mock_srv_set1_chainOut(OSSL_CMP_SRV_CTX *srv_ctx,
                                    STACK_OF(X509) *chain);
int ossl_cmp_mock_srv_set1_caPubsOut(OSSL_CMP_SRV_CTX *srv_ctx,
                                     STACK_OF(X509) *caPubs);
int ossl_cmp_mock_srv_set1_newWithNew(OSSL_CMP_SRV_CTX *srv_ctx, X509 *cert);
int ossl_cmp_mock_srv_set1_newWithOld(OSSL_CMP_SRV_CTX *srv_ctx, X509 *cert);
int ossl_cmp_mock_srv_set1_oldWithNew(OSSL_CMP_SRV_CTX *srv_ctx, X509 *cert);
int ossl_cmp_mock_srv_set_statusInfo(OSSL_CMP_SRV_CTX *srv_ctx, int status,
                                     int fail_info, const char *text);
int ossl_cmp_mock_srv_set_sendError(OSSL_CMP_SRV_CTX *srv_ctx, int bodytype);
int ossl_cmp_mock_srv_set_pollCount(OSSL_CMP_SRV_CTX *srv_ctx, int count);
int ossl_cmp_mock_srv_set_checkAfterTime(OSSL_CMP_SRV_CTX *srv_ctx, int sec);

# endif /* !defined(OPENSSL_NO_CMP) */
#endif /* !defined(OSSL_APPS_CMP_MOCK_SRV_H) */
