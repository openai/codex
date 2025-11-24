/*
 * Copyright 2019-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_DSA_H
# define OSSL_CRYPTO_DSA_H
# pragma once

# include <openssl/core.h>
# include <openssl/dsa.h>
# include "internal/ffc.h"

/*
 * DSA Paramgen types
 * Note, adding to this list requires adjustments to various checks
 * in dsa_gen range validation checks
 */
#define DSA_PARAMGEN_TYPE_FIPS_186_4   0   /* Use FIPS186-4 standard */
#define DSA_PARAMGEN_TYPE_FIPS_186_2   1   /* Use legacy FIPS186-2 standard */
#define DSA_PARAMGEN_TYPE_FIPS_DEFAULT 2

DSA *ossl_dsa_new(OSSL_LIB_CTX *libctx);
void ossl_dsa_set0_libctx(DSA *d, OSSL_LIB_CTX *libctx);

int ossl_dsa_generate_ffc_parameters(DSA *dsa, int type, int pbits, int qbits,
                                     BN_GENCB *cb);

int ossl_dsa_sign_int(int type, const unsigned char *dgst, int dlen,
                      unsigned char *sig, unsigned int *siglen, DSA *dsa,
                      unsigned int nonce_type, const char *digestname,
                      OSSL_LIB_CTX *libctx, const char *propq);

FFC_PARAMS *ossl_dsa_get0_params(DSA *dsa);
int ossl_dsa_ffc_params_fromdata(DSA *dsa, const OSSL_PARAM params[]);
int ossl_dsa_key_fromdata(DSA *dsa, const OSSL_PARAM params[],
                          int include_private);
DSA *ossl_dsa_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                             OSSL_LIB_CTX *libctx, const char *propq);

int ossl_dsa_generate_public_key(BN_CTX *ctx, const DSA *dsa,
                                 const BIGNUM *priv_key, BIGNUM *pub_key);
int ossl_dsa_check_params(const DSA *dsa, int checktype, int *ret);
int ossl_dsa_check_pub_key(const DSA *dsa, const BIGNUM *pub_key, int *ret);
int ossl_dsa_check_pub_key_partial(const DSA *dsa, const BIGNUM *pub_key,
                                   int *ret);
int ossl_dsa_check_priv_key(const DSA *dsa, const BIGNUM *priv_key, int *ret);
int ossl_dsa_check_pairwise(const DSA *dsa);
int ossl_dsa_is_foreign(const DSA *dsa);
DSA *ossl_dsa_dup(const DSA *dsa, int selection);

#endif
