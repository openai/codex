/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OPENSSL_HEADER_ML_KEM_H
# define OPENSSL_HEADER_ML_KEM_H
# pragma once

# include <openssl/e_os2.h>
# include <openssl/bio.h>
# include <openssl/core_dispatch.h>
# include <crypto/evp.h>

# define ML_KEM_DEGREE 256
/*
 * With (q-1) an odd multiple of 256, and 17 ("zeta") as a primitive 256th root
 * of unity, the polynomial (X^256+1) splits in Z_q[X] into 128 irreducible
 * quadratic factors of the form (X^2 - zeta^(2i + 1)).  This is used to
 * implement efficient multiplication in the ring R_q via the "NTT" transform.
 */
# define ML_KEM_PRIME          (ML_KEM_DEGREE * 13 + 1)

/*
 * Various ML-KEM primitives need random input, 32-bytes at a time.  Key
 * generation consumes two random values (d, z) with "d" plus the rank (domain
 * separation) further expanded to two derived seeds "rho" and "sigma", with
 * "rho" used to generate the public matrix "A", and sigma to generate the
 * private vector "s" and error vector "e".
 *
 * Encapsulation also consumes one random value m, that is 32-bytes long.  The
 * resulting shared secret "K" (also 32 bytes) and an internal random value "r"
 * are derived from "m" concatenated with a digest of the received public key.
 * Use of the public key hash means that the derived shared secret is
 * "contributary", it uses randomness from both parties.
 *
 * The seed "rho" is appended to the public key and allows the recipient of the
 * public key to re-compute the matrix "A" when performing encapsulation.
 *
 * Note that the matrix "m" we store in the public key is the transpose of the
 * "A" matrix from FIPS 203!
 */
# define ML_KEM_RANDOM_BYTES    32 /* rho, sigma, ... */
# define ML_KEM_SEED_BYTES      (ML_KEM_RANDOM_BYTES * 2) /* Keygen (d, z) */

# define ML_KEM_PKHASH_BYTES        32 /* Salts the shared-secret */
# define ML_KEM_SHARED_SECRET_BYTES 32

# if ML_KEM_PKHASH_BYTES != ML_KEM_RANDOM_BYTES
#  error "unexpected ML-KEM public key hash size"
# endif

/*-
 * The ML-KEM specification can be found in
 * https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.203.pdf
 *
 * Section 8, Table 2, lists the parameters for the three variants:
 *
 *         Variant     n     q  k  eta1  eta2  du   dv  secbits
 *      ----------   ---  ----  -  ----  ----  --   --  -------
 *      ML-KEM-512   256  3329  2     3     2  10    4      128
 *      ML-KEM-768   256  3329  3     2     2  10    4      192
 *      ML-KEM-1024  256  3329  4     2     2  11    5      256
 *
 * where:
 *
 * - "n" (ML_KEM_DEGREE above) is the fixed degree of the quotient polynomial
 *    in the ring: "R_q" = Z[X]/(X^n + 1).
 * - "q" (ML_KEM_PRIME above) is the fixed prime (256 * 13 + 1 = 3329) used in
 *   all ML-KEM variants.
 * - "k" is the row rank of the square matrix "A", with entries in R_q, that
 *   defines the "noisy" linear equations: t = A * s + e.  Also the rank of
 *   of the associated vectors.
 * - "eta1" determines the amplitude of "s" and "e" vectors in key generation
 *   and the "y" vector in ML-KEM encapsulation (K-PKE encryption).
 * - "eta2" determines the amplitude of "e1" and "e2" noise terms in ML-KEM
 *   encapsulation (K-PKE encryption).
 * - "du" determines how many bits of each coefficient are retained in the
 *   compressed form of the "u" vector in the encapsulation ciphertext.
 * - "dv" determines how many bits of each coefficient are retained in the
 *   compressed form of the "v" value in encapsulation ciphertext
 * - "secbits" is required security strength of the RNG for the random inputs.
 */

/*
 * Variant-specific constants and structures
 * -----------------------------------------
 */
# define EVP_PKEY_ML_KEM_512    NID_ML_KEM_512
# define ML_KEM_512_BITS        512
# define ML_KEM_512_RANK        2
# define ML_KEM_512_ETA1        3
# define ML_KEM_512_ETA2        2
# define ML_KEM_512_DU          10
# define ML_KEM_512_DV          4
# define ML_KEM_512_SECBITS     128

# define EVP_PKEY_ML_KEM_768    NID_ML_KEM_768
# define ML_KEM_768_BITS        768
# define ML_KEM_768_RANK        3
# define ML_KEM_768_ETA1        2
# define ML_KEM_768_ETA2        2
# define ML_KEM_768_DU          10
# define ML_KEM_768_DV          4
# define ML_KEM_768_SECBITS     192

# define EVP_PKEY_ML_KEM_1024   NID_ML_KEM_1024
# define ML_KEM_1024_BITS       1024
# define ML_KEM_1024_RANK       4
# define ML_KEM_1024_ETA1       2
# define ML_KEM_1024_ETA2       2
# define ML_KEM_1024_DU         11
# define ML_KEM_1024_DV         5
# define ML_KEM_1024_SECBITS    256

# define ML_KEM_KEY_RANDOM_PCT  (1 << 0)
# define ML_KEM_KEY_FIXED_PCT   (1 << 1)
# define ML_KEM_KEY_PREFER_SEED (1 << 2)
# define ML_KEM_KEY_RETAIN_SEED (1 << 3)
/* Mask to check whether PCT on import is enabled */
# define ML_KEM_KEY_PCT_TYPE \
    (ML_KEM_KEY_RANDOM_PCT | ML_KEM_KEY_FIXED_PCT)
/* Default provider flags */
# define ML_KEM_KEY_PROV_FLAGS_DEFAULT \
    (ML_KEM_KEY_RANDOM_PCT | ML_KEM_KEY_PREFER_SEED | ML_KEM_KEY_RETAIN_SEED)

/*
 * External variant-specific API
 * -----------------------------
 */

typedef struct {
    const char *algorithm_name;
    size_t prvkey_bytes;
    size_t prvalloc;
    size_t pubkey_bytes;
    size_t puballoc;
    size_t ctext_bytes;
    size_t vector_bytes;
    size_t u_vector_bytes;
    int evp_type;
    int bits;
    int rank;
    int du;
    int dv;
    int secbits;
} ML_KEM_VINFO;

/* Retrive global variant-specific parameters */
const ML_KEM_VINFO *ossl_ml_kem_get_vinfo(int evp_type);

/* Known as ML_KEM_KEY via crypto/types.h */
typedef struct ossl_ml_kem_key_st {
    /* Variant metadata, for one of ML-KEM-{512,768,1024} */
    const ML_KEM_VINFO *vinfo;

    /*
     * Library context, initially used to fetch the SHA3 MDs, and later for
     * random number generation.
     */
    OSSL_LIB_CTX *libctx;

    /* Pre-fetched SHA3 */
    EVP_MD *shake128_md;
    EVP_MD *shake256_md;
    EVP_MD *sha3_256_md;
    EVP_MD *sha3_512_md;

    /*
     * Pointers into variable size storage, initially all NULL. Appropriate
     * storage is allocated once a public or private key is specified, at
     * which point the key becomes immutable.
     */
    uint8_t *rho;                           /* Public matrix seed */
    uint8_t *pkhash;                        /* Public key hash */
    struct ossl_ml_kem_scalar_st *t;        /* Public key vector */
    struct ossl_ml_kem_scalar_st *m;        /* Pre-computed pubkey matrix */
    struct ossl_ml_kem_scalar_st *s;        /* Private key secret vector */
    uint8_t *z;                             /* Private key FO failure secret */
    uint8_t *d;                             /* Private key seed */
    int prov_flags;                         /* prefer/retain seed and PCT flags */

    /*
     * Fixed-size built-in buffer, which holds the |rho| and the public key
     * |pkhash| in that order, once we have expanded key material.
     * With seed-only keys, that are not yet expanded, this instead holds the
     * |z| and |d| components in that order.
     */
    uint8_t seedbuf[64];                    /* |rho| + |pkhash| / |z| + |d| */
    uint8_t *encoded_dk;                    /* Unparsed P8 private key */
} ML_KEM_KEY;

/* The public key is always present, when the private is */
# define ossl_ml_kem_key_vinfo(key)     ((key)->vinfo)
# define ossl_ml_kem_have_pubkey(key)   ((key)->t != NULL)
# define ossl_ml_kem_have_prvkey(key)   ((key)->s != NULL)
# define ossl_ml_kem_have_seed(key)     ((key)->d != NULL)
# define ossl_ml_kem_have_dkenc(key)    ((key)->encoded_dk != NULL)
# define ossl_ml_kem_decoded_key(key)   ((key)->encoded_dk != NULL \
                                         || ((key)->s == NULL && (key)->d != NULL))

/*
 * ----- ML-KEM key lifecycle
 */

/*
 * Allocate a "bare" key for given ML-KEM variant. Initially without any public
 * or private key material.
 */
ML_KEM_KEY *ossl_ml_kem_key_new(OSSL_LIB_CTX *libctx, const char *properties,
                                int evp_type);
/* Reset a key clearing all public and private key material */
void ossl_ml_kem_key_reset(ML_KEM_KEY *key);
/* Deallocate the key */
void ossl_ml_kem_key_free(ML_KEM_KEY *key);
/*
 * Duplicate a key, optionally including some key material, per the
 * |selection|, see <openssl/core_dispatch.h>.
 */
ML_KEM_KEY *ossl_ml_kem_key_dup(const ML_KEM_KEY *key, int selection);

/*
 * ----- Import or generate key material.
 */

/*
 * Functions that augment "bare ML-KEM keys" with key material deserialised
 * from an input buffer. It is an error for any key material to already be
 * present.
 *
 * Return 1 on success, 0 otherwise.
 */
__owur
int ossl_ml_kem_parse_public_key(const uint8_t *in, size_t len,
                                 ML_KEM_KEY *key);
__owur
int ossl_ml_kem_parse_private_key(const uint8_t *in, size_t len,
                                  ML_KEM_KEY *key);
ML_KEM_KEY *ossl_ml_kem_set_seed(const uint8_t *seed, size_t seedlen,
                                 ML_KEM_KEY *key);
__owur
int ossl_ml_kem_genkey(uint8_t *pubenc, size_t publen, ML_KEM_KEY *key);

/*
 * Perform an ML-KEM operation with a given ML-KEM key.  The key can generally
 * be either a private or public key, with the exception of encoding a private
 * key or performing KEM decapsulation.
 */
__owur
int ossl_ml_kem_encode_public_key(uint8_t *out, size_t len,
                                  const ML_KEM_KEY *key);
__owur
int ossl_ml_kem_encode_private_key(uint8_t *out, size_t len,
                                   const ML_KEM_KEY *key);
__owur
int ossl_ml_kem_encode_seed(uint8_t *out, size_t len,
                            const ML_KEM_KEY *key);

__owur
int ossl_ml_kem_encap_seed(uint8_t *ctext, size_t clen,
                           uint8_t *shared_secret, size_t slen,
                           const uint8_t *entropy, size_t elen,
                           const ML_KEM_KEY *key);
__owur
int ossl_ml_kem_encap_rand(uint8_t *ctext, size_t clen,
                           uint8_t *shared_secret, size_t slen,
                           const ML_KEM_KEY *key);
__owur
int ossl_ml_kem_decap(uint8_t *shared_secret, size_t slen,
                      const uint8_t *ctext, size_t clen,
                      const ML_KEM_KEY *key);

/* Compare the public key hashes of two keys */
__owur
int ossl_ml_kem_pubkey_cmp(const ML_KEM_KEY *key1, const ML_KEM_KEY *key2);

#endif  /* OPENSSL_HEADER_ML_KEM_H */
