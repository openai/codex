/*
 * Copyright 2019-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_RSA_H
# define OSSL_INTERNAL_RSA_H
# pragma once

# include <openssl/core.h>
# include <openssl/rsa.h>
# include "crypto/types.h"

#define RSA_MIN_MODULUS_BITS    512

typedef struct rsa_pss_params_30_st {
    int hash_algorithm_nid;
    struct {
        int algorithm_nid;       /* Currently always NID_mgf1 */
        int hash_algorithm_nid;
    } mask_gen;
    int salt_len;
    int trailer_field;
} RSA_PSS_PARAMS_30;

RSA_PSS_PARAMS_30 *ossl_rsa_get0_pss_params_30(RSA *r);
int ossl_rsa_pss_params_30_set_defaults(RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_copy(RSA_PSS_PARAMS_30 *to,
                                const RSA_PSS_PARAMS_30 *from);
int ossl_rsa_pss_params_30_is_unrestricted(const RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_set_hashalg(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                       int hashalg_nid);
int ossl_rsa_pss_params_30_set_maskgenhashalg(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                              int maskgenhashalg_nid);
int ossl_rsa_pss_params_30_set_saltlen(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                       int saltlen);
int ossl_rsa_pss_params_30_set_trailerfield(RSA_PSS_PARAMS_30 *rsa_pss_params,
                                            int trailerfield);
int ossl_rsa_pss_params_30_hashalg(const RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_maskgenalg(const RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_maskgenhashalg(const RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_saltlen(const RSA_PSS_PARAMS_30 *rsa_pss_params);
int ossl_rsa_pss_params_30_trailerfield(const RSA_PSS_PARAMS_30 *rsa_pss_params);

int ossl_rsa_verify_PKCS1_PSS_mgf1(RSA *rsa, const unsigned char *mHash,
                                   const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                                   const unsigned char *EM, int *sLenOut);
int ossl_rsa_padding_add_PKCS1_PSS_mgf1(RSA *rsa, unsigned char *EM,
                                        const unsigned char *mHash,
                                        const EVP_MD *Hash, const EVP_MD *mgf1Hash,
                                        int *sLenOut);

const char *ossl_rsa_mgf_nid2name(int mgf);
int ossl_rsa_oaeppss_md2nid(const EVP_MD *md);
const char *ossl_rsa_oaeppss_nid2name(int md);

RSA *ossl_rsa_new_with_ctx(OSSL_LIB_CTX *libctx);
OSSL_LIB_CTX *ossl_rsa_get0_libctx(RSA *r);
void ossl_rsa_set0_libctx(RSA *r, OSSL_LIB_CTX *libctx);

int ossl_rsa_set0_all_params(RSA *r, STACK_OF(BIGNUM) *primes,
                             STACK_OF(BIGNUM) *exps,
                             STACK_OF(BIGNUM) *coeffs);
int ossl_rsa_get0_all_params(RSA *r, STACK_OF(BIGNUM_const) *primes,
                             STACK_OF(BIGNUM_const) *exps,
                             STACK_OF(BIGNUM_const) *coeffs);
int ossl_rsa_is_foreign(const RSA *rsa);
RSA *ossl_rsa_dup(const RSA *rsa, int selection);

int ossl_rsa_todata(RSA *rsa, OSSL_PARAM_BLD *bld, OSSL_PARAM params[],
                    int include_private);
int ossl_rsa_fromdata(RSA *rsa, const OSSL_PARAM params[], int include_private);
int ossl_rsa_pss_params_30_todata(const RSA_PSS_PARAMS_30 *pss,
                                  OSSL_PARAM_BLD *bld, OSSL_PARAM params[]);
int ossl_rsa_pss_params_30_fromdata(RSA_PSS_PARAMS_30 *pss_params,
                                    int *defaults_set,
                                    const OSSL_PARAM params[],
                                    OSSL_LIB_CTX *libctx);
int ossl_rsa_set0_pss_params(RSA *r, RSA_PSS_PARAMS *pss);
int ossl_rsa_pss_get_param_unverified(const RSA_PSS_PARAMS *pss,
                                      const EVP_MD **pmd, const EVP_MD **pmgf1md,
                                      int *psaltlen, int *ptrailerField);
RSA_PSS_PARAMS *ossl_rsa_pss_decode(const X509_ALGOR *alg);
int ossl_rsa_param_decode(RSA *rsa, const X509_ALGOR *alg);
RSA *ossl_rsa_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                             OSSL_LIB_CTX *libctx, const char *propq);

int ossl_rsa_padding_check_PKCS1_type_2(OSSL_LIB_CTX *ctx,
                                        unsigned char *to, int tlen,
                                        const unsigned char *from, int flen,
                                        int num, unsigned char *kdk);
int ossl_rsa_padding_check_PKCS1_type_2_TLS(OSSL_LIB_CTX *ctx, unsigned char *to,
                                            size_t tlen,
                                            const unsigned char *from,
                                            size_t flen, int client_version,
                                            int alt_version);
int ossl_rsa_padding_add_PKCS1_OAEP_mgf1_ex(OSSL_LIB_CTX *libctx,
                                            unsigned char *to, int tlen,
                                            const unsigned char *from, int flen,
                                            const unsigned char *param,
                                            int plen, const EVP_MD *md,
                                            const EVP_MD *mgf1md);

int ossl_rsa_validate_public(const RSA *key);
int ossl_rsa_validate_private(const RSA *key);
int ossl_rsa_validate_pairwise(const RSA *key);

int ossl_rsa_verify(int dtype, const unsigned char *m,
                    unsigned int m_len, unsigned char *rm,
                    size_t *prm_len, const unsigned char *sigbuf,
                    size_t siglen, RSA *rsa);

const unsigned char *ossl_rsa_digestinfo_encoding(int md_nid, size_t *len);

extern const char *ossl_rsa_mp_factor_names[];
extern const char *ossl_rsa_mp_exp_names[];
extern const char *ossl_rsa_mp_coeff_names[];

ASN1_STRING *ossl_rsa_ctx_to_pss_string(EVP_PKEY_CTX *pkctx);
int ossl_rsa_pss_to_ctx(EVP_MD_CTX *ctx, EVP_PKEY_CTX *pkctx,
                        const X509_ALGOR *sigalg, EVP_PKEY *pkey);

# if defined(FIPS_MODULE) && !defined(OPENSSL_NO_ACVP_TESTS)
int ossl_rsa_acvp_test_gen_params_new(OSSL_PARAM **dst, const OSSL_PARAM src[]);
void ossl_rsa_acvp_test_gen_params_free(OSSL_PARAM *dst);

int ossl_rsa_acvp_test_set_params(RSA *r, const OSSL_PARAM params[]);
int ossl_rsa_acvp_test_get_params(RSA *r, OSSL_PARAM params[]);
typedef struct rsa_acvp_test_st RSA_ACVP_TEST;
void ossl_rsa_acvp_test_free(RSA_ACVP_TEST *t);
# else
# define RSA_ACVP_TEST void
# endif
int ossl_rsa_check_factors(RSA *r);

RSA *evp_pkey_get1_RSA_PSS(EVP_PKEY *pkey);
#endif
