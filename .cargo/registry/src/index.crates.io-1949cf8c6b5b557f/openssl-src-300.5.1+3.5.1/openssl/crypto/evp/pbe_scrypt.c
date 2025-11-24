/*
 * Copyright 2015-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/evp.h>
#include <openssl/err.h>
#include <openssl/kdf.h>
#include <openssl/core_names.h>
#include "internal/numbers.h"

#ifndef OPENSSL_NO_SCRYPT

/*
 * Maximum permitted memory allow this to be overridden with Configuration
 * option: e.g. -DSCRYPT_MAX_MEM=0 for maximum possible.
 */

#ifdef SCRYPT_MAX_MEM
# if SCRYPT_MAX_MEM == 0
#  undef SCRYPT_MAX_MEM
/*
 * Although we could theoretically allocate SIZE_MAX memory that would leave
 * no memory available for anything else so set limit as half that.
 */
#  define SCRYPT_MAX_MEM (SIZE_MAX/2)
# endif
#else
/* Default memory limit: 32 MB */
# define SCRYPT_MAX_MEM  (1024 * 1024 * 32)
#endif

int EVP_PBE_scrypt_ex(const char *pass, size_t passlen,
                      const unsigned char *salt, size_t saltlen,
                      uint64_t N, uint64_t r, uint64_t p, uint64_t maxmem,
                      unsigned char *key, size_t keylen,
                      OSSL_LIB_CTX *ctx, const char *propq)
{
    const char *empty = "";
    int rv = 1;
    EVP_KDF *kdf;
    EVP_KDF_CTX *kctx;
    OSSL_PARAM params[7], *z = params;

    if (r > UINT32_MAX || p > UINT32_MAX) {
        ERR_raise(ERR_LIB_EVP, EVP_R_PARAMETER_TOO_LARGE);
        return 0;
    }

    /* Maintain existing behaviour. */
    if (pass == NULL) {
        pass = empty;
        passlen = 0;
    }
    if (salt == NULL) {
        salt = (const unsigned char *)empty;
        saltlen = 0;
    }
    if (maxmem == 0)
        maxmem = SCRYPT_MAX_MEM;

    /* Use OSSL_LIB_CTX_set0_default() if you need a library context */
    kdf = EVP_KDF_fetch(ctx, OSSL_KDF_NAME_SCRYPT, propq);
    kctx = EVP_KDF_CTX_new(kdf);
    EVP_KDF_free(kdf);
    if (kctx == NULL)
        return 0;

    *z++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_PASSWORD,
                                              (unsigned char *)pass,
                                                      passlen);
    *z++ = OSSL_PARAM_construct_octet_string(OSSL_KDF_PARAM_SALT,
                                             (unsigned char *)salt, saltlen);
    *z++ = OSSL_PARAM_construct_uint64(OSSL_KDF_PARAM_SCRYPT_N, &N);
    *z++ = OSSL_PARAM_construct_uint64(OSSL_KDF_PARAM_SCRYPT_R, &r);
    *z++ = OSSL_PARAM_construct_uint64(OSSL_KDF_PARAM_SCRYPT_P, &p);
    *z++ = OSSL_PARAM_construct_uint64(OSSL_KDF_PARAM_SCRYPT_MAXMEM, &maxmem);
    *z = OSSL_PARAM_construct_end();
    if (EVP_KDF_derive(kctx, key, keylen, params) != 1)
        rv = 0;

    EVP_KDF_CTX_free(kctx);
    return rv;
}

int EVP_PBE_scrypt(const char *pass, size_t passlen,
                   const unsigned char *salt, size_t saltlen,
                   uint64_t N, uint64_t r, uint64_t p, uint64_t maxmem,
                   unsigned char *key, size_t keylen)
{
    return EVP_PBE_scrypt_ex(pass, passlen, salt, saltlen, N, r, p, maxmem,
                             key, keylen, NULL, NULL);
}

#endif
