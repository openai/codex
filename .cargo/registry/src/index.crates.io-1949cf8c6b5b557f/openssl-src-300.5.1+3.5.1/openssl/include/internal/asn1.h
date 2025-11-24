/*
 * Copyright 2020-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_ASN1_H
# define OSSL_INTERNAL_ASN1_H
# pragma once

# include <openssl/bio.h>

int asn1_d2i_read_bio(BIO *in, BUF_MEM **pb);

#endif
