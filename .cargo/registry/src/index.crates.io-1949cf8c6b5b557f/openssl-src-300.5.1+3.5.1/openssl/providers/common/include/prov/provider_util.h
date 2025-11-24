/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/provider.h>
#include <openssl/types.h>

typedef struct {
    /*
     * References to the underlying cipher implementation.  |cipher| caches
     * the cipher, always.  |alloc_cipher| only holds a reference to an
     * explicitly fetched cipher.
     */
    const EVP_CIPHER *cipher;   /* cipher */
    EVP_CIPHER *alloc_cipher;   /* fetched cipher */

    /* Conditions for legacy EVP_CIPHER uses */
    ENGINE *engine;             /* cipher engine */
} PROV_CIPHER;

typedef struct {
    /*
     * References to the underlying digest implementation.  |md| caches
     * the digest, always.  |alloc_md| only holds a reference to an explicitly
     * fetched digest.
     */
    const EVP_MD *md;           /* digest */
    EVP_MD *alloc_md;           /* fetched digest */

    /* Conditions for legacy EVP_MD uses */
    ENGINE *engine;             /* digest engine */
} PROV_DIGEST;

/* Cipher functions */
/*
 * Load a cipher from the specified parameters with the specified context.
 * The params "properties", "engine" and "cipher" are used to determine the
 * implementation used.  If a provider cannot be found, it falls back to trying
 * non-provider based implementations.
 */
int ossl_prov_cipher_load_from_params(PROV_CIPHER *pc,
                                      const OSSL_PARAM params[],
                                      OSSL_LIB_CTX *ctx);

/* Reset the PROV_CIPHER fields and free any allocated cipher reference */
void ossl_prov_cipher_reset(PROV_CIPHER *pc);

/* Clone a PROV_CIPHER structure into a second */
int ossl_prov_cipher_copy(PROV_CIPHER *dst, const PROV_CIPHER *src);

/* Query the cipher and associated engine (if any) */
const EVP_CIPHER *ossl_prov_cipher_cipher(const PROV_CIPHER *pc);
ENGINE *ossl_prov_cipher_engine(const PROV_CIPHER *pc);

/* Digest functions */

/*
 * Fetch a digest from the specified libctx using the provided mdname and
 * propquery. Store the result in the PROV_DIGEST and return the fetched md.
 */
const EVP_MD *ossl_prov_digest_fetch(PROV_DIGEST *pd, OSSL_LIB_CTX *libctx,
                                     const char *mdname, const char *propquery);

/*
 * Load a digest from the specified parameters with the specified context.
 * The params "properties", "engine" and "digest" are used to determine the
 * implementation used.  If a provider cannot be found, it falls back to trying
 * non-provider based implementations.
 */
int ossl_prov_digest_load_from_params(PROV_DIGEST *pd,
                                      const OSSL_PARAM params[],
                                      OSSL_LIB_CTX *ctx);

/* Reset the PROV_DIGEST fields and free any allocated digest reference */
void ossl_prov_digest_reset(PROV_DIGEST *pd);

/* Clone a PROV_DIGEST structure into a second */
int ossl_prov_digest_copy(PROV_DIGEST *dst, const PROV_DIGEST *src);

/* Query the digest and associated engine (if any) */
const EVP_MD *ossl_prov_digest_md(const PROV_DIGEST *pd);
ENGINE *ossl_prov_digest_engine(const PROV_DIGEST *pd);

/* Set a specific md, resets current digests first */
void ossl_prov_digest_set_md(PROV_DIGEST *pd, EVP_MD *md);

/*
 * Set the various parameters on an EVP_MAC_CTX from the supplied arguments.
 * If any of the supplied ciphername/mdname etc are NULL then the values
 * from the supplied params (if non NULL) are used instead.
 */
int ossl_prov_set_macctx(EVP_MAC_CTX *macctx,
                         const OSSL_PARAM params[],
                         const char *ciphername,
                         const char *mdname,
                         const char *engine,
                         const char *properties,
                         const unsigned char *key,
                         size_t keylen);

/* MAC functions */
/*
 * Load an EVP_MAC_CTX* from the specified parameters with the specified
 * library context.
 * The params "mac" and "properties" are used to determine the implementation
 * used, and the parameters "digest", "cipher", "engine" and "properties" are
 * passed to the MAC via the created MAC context if they are given.
 * If there is already a created MAC context, it will be replaced if the "mac"
 * parameter is found, otherwise it will simply be used as is, and passed the
 * parameters to pilfer as it sees fit.
 *
 * As an option, a MAC name may be explicitly given, and if it is, the "mac"
 * parameter will be ignored.
 * Similarly, as an option, a cipher name or a digest name may be explicitly
 * given, and if any of them is, the "digest" and "cipher" parameters are
 * ignored.
 */
int ossl_prov_macctx_load_from_params(EVP_MAC_CTX **macctx,
                                      const OSSL_PARAM params[],
                                      const char *macname,
                                      const char *ciphername,
                                      const char *mdname,
                                      OSSL_LIB_CTX *ctx);

typedef struct ag_capable_st {
    OSSL_ALGORITHM alg;
    int (*capable)(void);
} OSSL_ALGORITHM_CAPABLE;

/*
 * Dynamically select algorithms by calling a capable() method.
 * If this method is NULL or the method returns 1 then the algorithm is added.
 */
void ossl_prov_cache_exported_algorithms(const OSSL_ALGORITHM_CAPABLE *in,
                                         OSSL_ALGORITHM *out);

/* Duplicate a lump of memory safely */
int ossl_prov_memdup(const void *src, size_t src_len,
                     unsigned char **dest, size_t *dest_len);
