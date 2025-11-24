/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_FFC_H
# define OSSL_INTERNAL_FFC_H
# pragma once

# include <openssl/core.h>
# include <openssl/bn.h>
# include <openssl/evp.h>
# include <openssl/dh.h> /* Uses Error codes from DH */
# include <openssl/params.h>
# include <openssl/param_build.h>
# include "internal/sizes.h"

/* Default value for gindex when canonical generation of g is not used */
# define FFC_UNVERIFIABLE_GINDEX -1

/* The different types of FFC keys */
# define FFC_PARAM_TYPE_DSA  0
# define FFC_PARAM_TYPE_DH   1

/*
 * The mode used by functions that share code for both generation and
 * verification. See ossl_ffc_params_FIPS186_4_gen_verify().
 */
#define FFC_PARAM_MODE_VERIFY   0
#define FFC_PARAM_MODE_GENERATE 1

/* Return codes for generation and validation of FFC parameters */
#define FFC_PARAM_RET_STATUS_FAILED         0
#define FFC_PARAM_RET_STATUS_SUCCESS        1
/* Returned if validating and g is only partially verifiable */
#define FFC_PARAM_RET_STATUS_UNVERIFIABLE_G 2

/* Validation flags */
# define FFC_PARAM_FLAG_VALIDATE_PQ    0x01
# define FFC_PARAM_FLAG_VALIDATE_G     0x02
# define FFC_PARAM_FLAG_VALIDATE_PQG                                           \
    (FFC_PARAM_FLAG_VALIDATE_PQ | FFC_PARAM_FLAG_VALIDATE_G)
#define FFC_PARAM_FLAG_VALIDATE_LEGACY 0x04

/*
 * NB: These values must align with the equivalently named macros in
 * openssl/dh.h. We cannot use those macros here in case DH has been disabled.
 */
# define FFC_CHECK_P_NOT_PRIME                0x00001
# define FFC_CHECK_P_NOT_SAFE_PRIME           0x00002
# define FFC_CHECK_UNKNOWN_GENERATOR          0x00004
# define FFC_CHECK_NOT_SUITABLE_GENERATOR     0x00008
# define FFC_CHECK_Q_NOT_PRIME                0x00010
# define FFC_CHECK_INVALID_Q_VALUE            0x00020
# define FFC_CHECK_INVALID_J_VALUE            0x00040

/*
 * 0x80, 0x100 reserved by include/openssl/dh.h with check bits that are not
 * relevant for FFC.
 */

# define FFC_CHECK_MISSING_SEED_OR_COUNTER    0x00200
# define FFC_CHECK_INVALID_G                  0x00400
# define FFC_CHECK_INVALID_PQ                 0x00800
# define FFC_CHECK_INVALID_COUNTER            0x01000
# define FFC_CHECK_P_MISMATCH                 0x02000
# define FFC_CHECK_Q_MISMATCH                 0x04000
# define FFC_CHECK_G_MISMATCH                 0x08000
# define FFC_CHECK_COUNTER_MISMATCH           0x10000
# define FFC_CHECK_BAD_LN_PAIR                0x20000
# define FFC_CHECK_INVALID_SEED_SIZE          0x40000

/* Validation Return codes */
# define FFC_ERROR_PUBKEY_TOO_SMALL       0x01
# define FFC_ERROR_PUBKEY_TOO_LARGE       0x02
# define FFC_ERROR_PUBKEY_INVALID         0x04
# define FFC_ERROR_NOT_SUITABLE_GENERATOR 0x08
# define FFC_ERROR_PRIVKEY_TOO_SMALL      0x10
# define FFC_ERROR_PRIVKEY_TOO_LARGE      0x20
# define FFC_ERROR_PASSED_NULL_PARAM      0x40

/*
 * Finite field cryptography (FFC) domain parameters are used by DH and DSA.
 * Refer to FIPS186_4 Appendix A & B.
 */
typedef struct ffc_params_st {
    /* Primes */
    BIGNUM *p;
    BIGNUM *q;
    /* Generator */
    BIGNUM *g;
    /* DH X9.42 Optional Subgroup factor j >= 2 where p = j * q + 1 */
    BIGNUM *j;

    /* Required for FIPS186_4 validation of p, q and optionally canonical g */
    unsigned char *seed;
    /* If this value is zero the hash size is used as the seed length */
    size_t seedlen;
    /* Required for FIPS186_4 validation of p and q */
    int pcounter;
    int nid; /* The identity of a named group */

    /*
     * Required for FIPS186_4 generation & validation of canonical g.
     * It uses unverifiable g if this value is -1.
     */
    int gindex;
    int h; /* loop counter for unverifiable g */

    unsigned int flags;
    /*
     * The digest to use for generation or validation. If this value is NULL,
     * then the digest is chosen using the value of N.
     */
    const char *mdname;
    const char *mdprops;
    /* Default key length for known named groups according to RFC7919 */
    int keylength;
} FFC_PARAMS;

void ossl_ffc_params_init(FFC_PARAMS *params);
void ossl_ffc_params_cleanup(FFC_PARAMS *params);
void ossl_ffc_params_set0_pqg(FFC_PARAMS *params, BIGNUM *p, BIGNUM *q,
                              BIGNUM *g);
void ossl_ffc_params_get0_pqg(const FFC_PARAMS *params, const BIGNUM **p,
                              const BIGNUM **q, const BIGNUM **g);
void ossl_ffc_params_set0_j(FFC_PARAMS *d, BIGNUM *j);
int ossl_ffc_params_set_seed(FFC_PARAMS *params,
                             const unsigned char *seed, size_t seedlen);
void ossl_ffc_params_set_gindex(FFC_PARAMS *params, int index);
void ossl_ffc_params_set_pcounter(FFC_PARAMS *params, int index);
void ossl_ffc_params_set_h(FFC_PARAMS *params, int index);
void ossl_ffc_params_set_flags(FFC_PARAMS *params, unsigned int flags);
void ossl_ffc_params_enable_flags(FFC_PARAMS *params, unsigned int flags,
                                  int enable);
void ossl_ffc_set_digest(FFC_PARAMS *params, const char *alg, const char *props);

int ossl_ffc_params_set_validate_params(FFC_PARAMS *params,
                                        const unsigned char *seed,
                                        size_t seedlen, int counter);
void ossl_ffc_params_get_validate_params(const FFC_PARAMS *params,
                                         unsigned char **seed, size_t *seedlen,
                                         int *pcounter);

int ossl_ffc_params_copy(FFC_PARAMS *dst, const FFC_PARAMS *src);
int ossl_ffc_params_cmp(const FFC_PARAMS *a, const FFC_PARAMS *b, int ignore_q);

#ifndef FIPS_MODULE
int ossl_ffc_params_print(BIO *bp, const FFC_PARAMS *ffc, int indent);
#endif /* FIPS_MODULE */


int ossl_ffc_params_FIPS186_4_generate(OSSL_LIB_CTX *libctx, FFC_PARAMS *params,
                                       int type, size_t L, size_t N,
                                       int *res, BN_GENCB *cb);
int ossl_ffc_params_FIPS186_2_generate(OSSL_LIB_CTX *libctx, FFC_PARAMS *params,
                                       int type, size_t L, size_t N,
                                       int *res, BN_GENCB *cb);

int ossl_ffc_params_FIPS186_4_gen_verify(OSSL_LIB_CTX *libctx,
                                         FFC_PARAMS *params, int mode, int type,
                                         size_t L, size_t N, int *res,
                                         BN_GENCB *cb);
int ossl_ffc_params_FIPS186_2_gen_verify(OSSL_LIB_CTX *libctx,
                                         FFC_PARAMS *params, int mode, int type,
                                         size_t L, size_t N, int *res,
                                         BN_GENCB *cb);

int ossl_ffc_params_simple_validate(OSSL_LIB_CTX *libctx,
                                    const FFC_PARAMS *params,
                                    int paramstype, int *res);
int ossl_ffc_params_full_validate(OSSL_LIB_CTX *libctx,
                                  const FFC_PARAMS *params,
                                  int paramstype, int *res);
int ossl_ffc_params_FIPS186_4_validate(OSSL_LIB_CTX *libctx,
                                       const FFC_PARAMS *params,
                                       int type, int *res, BN_GENCB *cb);
int ossl_ffc_params_FIPS186_2_validate(OSSL_LIB_CTX *libctx,
                                       const FFC_PARAMS *params,
                                       int type, int *res, BN_GENCB *cb);

int ossl_ffc_generate_private_key(BN_CTX *ctx, const FFC_PARAMS *params,
                                  int N, int s, BIGNUM *priv);

int ossl_ffc_params_validate_unverifiable_g(BN_CTX *ctx, BN_MONT_CTX *mont,
                                            const BIGNUM *p, const BIGNUM *q,
                                            const BIGNUM *g, BIGNUM *tmp,
                                            int *ret);

int ossl_ffc_validate_public_key(const FFC_PARAMS *params,
                                 const BIGNUM *pub_key, int *ret);
int ossl_ffc_validate_public_key_partial(const FFC_PARAMS *params,
                                         const BIGNUM *pub_key, int *ret);
int ossl_ffc_validate_private_key(const BIGNUM *upper, const BIGNUM *priv_key,
                                 int *ret);

int ossl_ffc_params_todata(const FFC_PARAMS *ffc, OSSL_PARAM_BLD *tmpl,
                           OSSL_PARAM params[]);
int ossl_ffc_params_fromdata(FFC_PARAMS *ffc, const OSSL_PARAM params[]);

typedef struct dh_named_group_st DH_NAMED_GROUP;
const DH_NAMED_GROUP *ossl_ffc_name_to_dh_named_group(const char *name);
const DH_NAMED_GROUP *ossl_ffc_uid_to_dh_named_group(int uid);
#ifndef OPENSSL_NO_DH
const DH_NAMED_GROUP *ossl_ffc_numbers_to_dh_named_group(const BIGNUM *p,
                                                         const BIGNUM *q,
                                                         const BIGNUM *g);
#endif
int ossl_ffc_named_group_get_uid(const DH_NAMED_GROUP *group);
const char *ossl_ffc_named_group_get_name(const DH_NAMED_GROUP *);
#ifndef OPENSSL_NO_DH
int ossl_ffc_named_group_get_keylength(const DH_NAMED_GROUP *group);
const BIGNUM *ossl_ffc_named_group_get_q(const DH_NAMED_GROUP *group);
int ossl_ffc_named_group_set(FFC_PARAMS *ffc, const DH_NAMED_GROUP *group);
#endif

#endif /* OSSL_INTERNAL_FFC_H */
