/*
 * Copyright 2020-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_DH_H
# define OSSL_CRYPTO_DH_H
# pragma once

# include <openssl/core.h>
# include <openssl/params.h>
# include <openssl/dh.h>
# include "internal/ffc.h"

DH *ossl_dh_new_by_nid_ex(OSSL_LIB_CTX *libctx, int nid);
DH *ossl_dh_new_ex(OSSL_LIB_CTX *libctx);
void ossl_dh_set0_libctx(DH *d, OSSL_LIB_CTX *libctx);
int ossl_dh_generate_ffc_parameters(DH *dh, int type, int pbits, int qbits,
                                    BN_GENCB *cb);
int ossl_dh_generate_public_key(BN_CTX *ctx, const DH *dh,
                                const BIGNUM *priv_key, BIGNUM *pub_key);
int ossl_dh_get_named_group_uid_from_size(int pbits);
const char *ossl_dh_gen_type_id2name(int id);
int ossl_dh_gen_type_name2id(const char *name, int type);
void ossl_dh_cache_named_group(DH *dh);
int ossl_dh_is_named_safe_prime_group(const DH *dh);

FFC_PARAMS *ossl_dh_get0_params(DH *dh);
int ossl_dh_get0_nid(const DH *dh);
int ossl_dh_params_fromdata(DH *dh, const OSSL_PARAM params[]);
int ossl_dh_key_fromdata(DH *dh, const OSSL_PARAM params[], int include_private);
int ossl_dh_params_todata(DH *dh, OSSL_PARAM_BLD *bld, OSSL_PARAM params[]);
int ossl_dh_key_todata(DH *dh, OSSL_PARAM_BLD *bld, OSSL_PARAM params[],
                       int include_private);
DH *ossl_dh_key_from_pkcs8(const PKCS8_PRIV_KEY_INFO *p8inf,
                           OSSL_LIB_CTX *libctx, const char *propq);
int ossl_dh_compute_key(unsigned char *key, const BIGNUM *pub_key, DH *dh);

int ossl_dh_check_pub_key_partial(const DH *dh, const BIGNUM *pub_key, int *ret);
int ossl_dh_check_priv_key(const DH *dh, const BIGNUM *priv_key, int *ret);
int ossl_dh_check_pairwise(const DH *dh);

const DH_METHOD *ossl_dh_get_method(const DH *dh);

int ossl_dh_buf2key(DH *key, const unsigned char *buf, size_t len);
size_t ossl_dh_key2buf(const DH *dh, unsigned char **pbuf, size_t size,
                       int alloc);

int ossl_dh_kdf_X9_42_asn1(unsigned char *out, size_t outlen,
                           const unsigned char *Z, size_t Zlen,
                           const char *cek_alg,
                           const unsigned char *ukm, size_t ukmlen,
                           const EVP_MD *md,
                           OSSL_LIB_CTX *libctx, const char *propq);
int ossl_dh_is_foreign(const DH *dh);
DH *ossl_dh_dup(const DH *dh, int selection);

#endif  /* OSSL_CRYPTO_DH_H */
