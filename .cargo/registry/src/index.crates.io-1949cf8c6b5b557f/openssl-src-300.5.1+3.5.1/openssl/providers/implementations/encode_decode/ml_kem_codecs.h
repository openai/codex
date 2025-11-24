/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef PROV_ML_KEM_CODECS_H
# define PROV_ML_KEM_CODECS_H
# pragma once

# ifndef OPENSSL_NO_ML_KEM
#  include <openssl/e_os2.h>
#  include "crypto/ml_kem.h"
#  include "prov/provider_ctx.h"
#  include "ml_common_codecs.h"

__owur
ML_KEM_KEY *ossl_ml_kem_d2i_PUBKEY(const uint8_t *pubenc, int publen,
                                   int evp_type, PROV_CTX *provctx,
                                   const char *propq);
__owur
ML_KEM_KEY *ossl_ml_kem_d2i_PKCS8(const uint8_t *prvenc, int prvlen,
                                  int evp_type, PROV_CTX *provctx,
                                  const char *propq);
__owur
int ossl_ml_kem_key_to_text(BIO *out, const ML_KEM_KEY *key, int selection);
__owur
__owur
int ossl_ml_kem_i2d_pubkey(const ML_KEM_KEY *key, unsigned char **out);
__owur
__owur
int ossl_ml_kem_i2d_prvkey(const ML_KEM_KEY *key, unsigned char **out,
                           PROV_CTX *provctx);

# endif  /* OPENSSL_NO_ML_KEM */
#endif  /* PROV_ML_KEM_CODECS_H */
