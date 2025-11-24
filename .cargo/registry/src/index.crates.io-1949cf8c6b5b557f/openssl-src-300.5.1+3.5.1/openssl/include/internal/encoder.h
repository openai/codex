/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_ENCODER_H
# define OSSL_INTERNAL_ENCODER_H
# pragma once

# include <openssl/bio.h>
# include <openssl/buffer.h>
# include <openssl/types.h>
# include "internal/ffc.h"

int ossl_bio_print_labeled_bignum(BIO *out, const char *label,
                                  const BIGNUM *bn);
int ossl_bio_print_labeled_buf(BIO *out, const char *label,
                               const unsigned char *buf, size_t buflen);
# if !defined(OPENSSL_NO_DH) || !defined(OPENSSL_NO_DSA)
int ossl_bio_print_ffc_params(BIO *out, const FFC_PARAMS *ffc);
# endif

#endif
