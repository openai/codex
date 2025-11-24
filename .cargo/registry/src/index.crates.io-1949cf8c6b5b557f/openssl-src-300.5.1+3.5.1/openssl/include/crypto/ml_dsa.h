/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* Internal ML_DSA functions for other submodules, not for application use */

#ifndef OSSL_CRYPTO_ML_DSA_H
# define OSSL_CRYPTO_ML_DSA_H

# pragma once
# include <openssl/e_os2.h>
# include <openssl/types.h>
# include "crypto/types.h"

# define ML_DSA_MAX_CONTEXT_STRING_LEN 255
# define ML_DSA_SEED_BYTES 32

# define ML_DSA_ENTROPY_LEN 32

/* See FIPS 204 Section 4 Table 1 & Table 2 */
# define ML_DSA_44_PRIV_LEN 2560
# define ML_DSA_44_PUB_LEN 1312
# define ML_DSA_44_SIG_LEN 2420

/* See FIPS 204 Section 4 Table 1 & Table 2 */
# define ML_DSA_65_PRIV_LEN 4032
# define ML_DSA_65_PUB_LEN 1952
# define ML_DSA_65_SIG_LEN 3309

/* See FIPS 204 Section 4 Table 1 & Table 2 */
# define ML_DSA_87_PRIV_LEN 4896
# define ML_DSA_87_PUB_LEN 2592
# define ML_DSA_87_SIG_LEN 4627

/* Key and signature size maxima taken from values above */
# define MAX_ML_DSA_PRIV_LEN ML_DSA_87_PRIV_LEN
# define MAX_ML_DSA_PUB_LEN ML_DSA_87_PUB_LEN
# define MAX_ML_DSA_SIG_LEN ML_DSA_87_SIG_LEN

# define ML_DSA_KEY_PREFER_SEED (1 << 0)
# define ML_DSA_KEY_RETAIN_SEED (1 << 1)
/* Default provider flags */
# define ML_DSA_KEY_PROV_FLAGS_DEFAULT \
    (ML_DSA_KEY_PREFER_SEED | ML_DSA_KEY_RETAIN_SEED)

/*
 * Refer to FIPS 204 Section 4 Parameter sets.
 * Fields that are shared between all algorithms (such as q & d) have been omitted.
 */
typedef struct ml_dsa_params_st {
    const char *alg;
    int evp_type;
    int tau;    /* Number of +/-1's in polynomial c */
    int bit_strength; /* The collision strength (lambda) */
    int gamma1; /* coefficient range of y */
    int gamma2; /* low-order rounding range */
    size_t k, l; /* matrix dimensions of 'A' */
    int eta;    /* Private key range */
    int beta;   /* tau * eta */
    int omega;  /* Number of 1's in the hint 'h' */
    int security_category; /* Category is related to Security strength */
    size_t sk_len; /* private key size */
    size_t pk_len; /* public key size */
    size_t sig_len; /* signature size */
} ML_DSA_PARAMS;

/* NOTE - any changes to this struct may require updates to ossl_ml_dsa_dup() */
typedef struct ml_dsa_key_st ML_DSA_KEY;

const ML_DSA_PARAMS *ossl_ml_dsa_params_get(int evp_type);
const ML_DSA_PARAMS *ossl_ml_dsa_key_params(const ML_DSA_KEY *key);
__owur ML_DSA_KEY *ossl_ml_dsa_key_new(OSSL_LIB_CTX *libctx, const char *propq,
                                       int evp_type);
/* Factory reset for keys that fail initialisation */
void ossl_ml_dsa_key_reset(ML_DSA_KEY *key);
__owur int ossl_ml_dsa_key_pub_alloc(ML_DSA_KEY *key);
__owur int ossl_ml_dsa_key_priv_alloc(ML_DSA_KEY *key);
void ossl_ml_dsa_key_free(ML_DSA_KEY *key);
__owur ML_DSA_KEY *ossl_ml_dsa_key_dup(const ML_DSA_KEY *src, int selection);
__owur int ossl_ml_dsa_key_equal(const ML_DSA_KEY *key1, const ML_DSA_KEY *key2,
                                 int selection);
__owur int ossl_ml_dsa_key_has(const ML_DSA_KEY *key, int selection);
__owur int ossl_ml_dsa_key_pairwise_check(const ML_DSA_KEY *key);
__owur int ossl_ml_dsa_generate_key(ML_DSA_KEY *out);
__owur const uint8_t *ossl_ml_dsa_key_get_pub(const ML_DSA_KEY *key);
__owur size_t ossl_ml_dsa_key_get_pub_len(const ML_DSA_KEY *key);
__owur const uint8_t *ossl_ml_dsa_key_get_priv(const ML_DSA_KEY *key);
__owur size_t ossl_ml_dsa_key_get_priv_len(const ML_DSA_KEY *key);
__owur const uint8_t *ossl_ml_dsa_key_get_seed(const ML_DSA_KEY *key);
__owur int ossl_ml_dsa_key_get_prov_flags(const ML_DSA_KEY *key);
int ossl_ml_dsa_set_prekey(ML_DSA_KEY *key, int flags_set, int flags_clr,
                           const uint8_t *seed, size_t seed_len,
                           const uint8_t *sk, size_t sk_len);
__owur size_t ossl_ml_dsa_key_get_collision_strength_bits(const ML_DSA_KEY *key);
__owur size_t ossl_ml_dsa_key_get_sig_len(const ML_DSA_KEY *key);
__owur int ossl_ml_dsa_key_matches(const ML_DSA_KEY *key, int evp_type);
__owur const char *ossl_ml_dsa_key_get_name(const ML_DSA_KEY *key);
OSSL_LIB_CTX *ossl_ml_dsa_key_get0_libctx(const ML_DSA_KEY *key);

__owur int ossl_ml_dsa_key_public_from_private(ML_DSA_KEY *key);
__owur int ossl_ml_dsa_pk_decode(ML_DSA_KEY *key, const uint8_t *in, size_t in_len);
__owur int ossl_ml_dsa_sk_decode(ML_DSA_KEY *key, const uint8_t *in, size_t in_len);

__owur int ossl_ml_dsa_sign(const ML_DSA_KEY *priv, int msg_is_mu,
                            const uint8_t *msg, size_t msg_len,
                            const uint8_t *context, size_t context_len,
                            const uint8_t *rand, size_t rand_len, int encode,
                            unsigned char *sig, size_t *siglen, size_t sigsize);
__owur int ossl_ml_dsa_verify(const ML_DSA_KEY *pub, int msg_is_mu,
                              const uint8_t *msg, size_t msg_len,
                              const uint8_t *context, size_t context_len,
                              int encode, const uint8_t *sig, size_t sig_len);

#endif /* OSSL_CRYPTO_SLH_DSA_H */
