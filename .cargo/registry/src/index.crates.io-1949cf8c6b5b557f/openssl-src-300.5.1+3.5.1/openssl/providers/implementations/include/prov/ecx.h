/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "crypto/types.h"

#ifndef OPENSSL_NO_EC

/* RFC 9180 Labels used for Extract and Expand operations */

/* ASCII: "eae_prk", in hex for EBCDIC compatibility */
#define OSSL_DHKEM_LABEL_EAE_PRK "\x65\x61\x65\x5F\x70\x72\x6B"
/* ASCII: "shared_secret", in hex for EBCDIC compatibility */
#define OSSL_DHKEM_LABEL_SHARED_SECRET "\x73\x68\x61\x72\x65\x64\x5F\x73\x65\x63\x72\x65\x74"
/* ASCII: "dkp_prk", in hex for EBCDIC compatibility */
#define OSSL_DHKEM_LABEL_DKP_PRK "\x64\x6B\x70\x5F\x70\x72\x6B"
/* ASCII: "candidate", in hex for EBCDIC compatibility */
#define OSSL_DHKEM_LABEL_CANDIDATE "\x63\x61\x6E\x64\x69\x64\x61\x74\x65"
/* ASCII: "sk", in hex for EBCDIC compatibility */
#define OSSL_DHKEM_LABEL_SK "\x73\x6B"

int ossl_ecx_dhkem_derive_private(ECX_KEY *ecx, unsigned char *privout,
                                  const unsigned char *ikm, size_t ikmlen);
int ossl_ec_dhkem_derive_private(EC_KEY *ec, BIGNUM *privout,
                                 const unsigned char *ikm, size_t ikmlen);
#endif
