/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal SLH_DSA functions for other submodules, not for application use */

#ifndef OSSL_CRYPTO_SLH_DSA_H
# define OSSL_CRYPTO_SLH_DSA_H

# pragma once
# include <openssl/e_os2.h>
# include <openssl/types.h>
# include "crypto/types.h"

# define SLH_DSA_MAX_CONTEXT_STRING_LEN 255
# define SLH_DSA_MAX_N                  32

typedef struct slh_dsa_hash_ctx_st SLH_DSA_HASH_CTX;
typedef struct slh_dsa_key_st SLH_DSA_KEY;

__owur SLH_DSA_KEY *ossl_slh_dsa_key_new(OSSL_LIB_CTX *libctx, const char *propq,
                                         const char *alg);
void ossl_slh_dsa_key_free(SLH_DSA_KEY *key);
__owur SLH_DSA_KEY *ossl_slh_dsa_key_dup(const SLH_DSA_KEY *src, int selection);
__owur int ossl_slh_dsa_key_equal(const SLH_DSA_KEY *key1, const SLH_DSA_KEY *key2,
                                  int selection);
__owur int ossl_slh_dsa_key_has(const SLH_DSA_KEY *key, int selection);
__owur int ossl_slh_dsa_key_pairwise_check(const SLH_DSA_KEY *key);
__owur int ossl_slh_dsa_key_fromdata(SLH_DSA_KEY *key, const OSSL_PARAM *params,
                                     int include_private);
__owur int ossl_slh_dsa_generate_key(SLH_DSA_HASH_CTX *ctx, SLH_DSA_KEY *out,
                                     OSSL_LIB_CTX *libctx,
                                     const uint8_t *entropy, size_t entropy_len);
__owur int ossl_slh_dsa_key_to_text(BIO *out, const SLH_DSA_KEY *key, int selection);
__owur const uint8_t *ossl_slh_dsa_key_get_pub(const SLH_DSA_KEY *key);
__owur const uint8_t *ossl_slh_dsa_key_get_priv(const SLH_DSA_KEY *key);
__owur size_t ossl_slh_dsa_key_get_pub_len(const SLH_DSA_KEY *key);
__owur int ossl_slh_dsa_set_priv(SLH_DSA_KEY *key, const uint8_t *priv,
                                 size_t priv_len);
__owur int ossl_slh_dsa_set_pub(SLH_DSA_KEY *key, const uint8_t *pub,
                                size_t pub_len);
__owur size_t ossl_slh_dsa_key_get_priv_len(const SLH_DSA_KEY *key);
__owur size_t ossl_slh_dsa_key_get_n(const SLH_DSA_KEY *key);
__owur size_t ossl_slh_dsa_key_get_sig_len(const SLH_DSA_KEY *key);
__owur const char *ossl_slh_dsa_key_get_name(const SLH_DSA_KEY *key);
__owur int ossl_slh_dsa_key_get_type(const SLH_DSA_KEY *key);
__owur int ossl_slh_dsa_key_type_matches(const SLH_DSA_KEY *key, const char *alg);
__owur SLH_DSA_HASH_CTX *ossl_slh_dsa_hash_ctx_new(const SLH_DSA_KEY *key);
void ossl_slh_dsa_hash_ctx_free(SLH_DSA_HASH_CTX *ctx);
__owur SLH_DSA_HASH_CTX *ossl_slh_dsa_hash_ctx_dup(const SLH_DSA_HASH_CTX *src);

__owur int ossl_slh_dsa_sign(SLH_DSA_HASH_CTX *slh_ctx,
                             const uint8_t *msg, size_t msg_len,
                             const uint8_t *ctx, size_t ctx_len,
                             const uint8_t *add_rand, int encode,
                             unsigned char *sig, size_t *siglen, size_t sigsize);
__owur int ossl_slh_dsa_verify(SLH_DSA_HASH_CTX *slh_ctx,
                               const uint8_t *msg, size_t msg_len,
                               const uint8_t *ctx, size_t ctx_len, int encode,
                               const uint8_t *sig, size_t sig_len);

#endif /* OSSL_CRYPTO_SLH_DSA_H */
