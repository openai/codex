/*
 * Copyright 1995-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_RAND_LOCAL_H
# define OSSL_CRYPTO_RAND_LOCAL_H

# include <openssl/aes.h>
# include <openssl/evp.h>
# include <openssl/sha.h>
# include <openssl/hmac.h>
# include <openssl/ec.h>
# include <openssl/rand.h>
# include "internal/tsan_assist.h"
# include "crypto/rand.h"

/* Default reseed intervals */
# define PRIMARY_RESEED_INTERVAL                 (1 << 8)
# define SECONDARY_RESEED_INTERVAL               (1 << 16)
# define PRIMARY_RESEED_TIME_INTERVAL            (60 * 60) /* 1 hour */
# define SECONDARY_RESEED_TIME_INTERVAL          (7 * 60)  /* 7 minutes */

# ifndef FIPS_MODULE
/* The global RAND method, and the global buffer and DRBG instance. */
extern RAND_METHOD ossl_rand_meth;
# endif

#endif
