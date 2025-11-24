/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_FIPS_H
# define OSSL_INTERNAL_FIPS_H
# pragma once

# ifdef FIPS_MODULE

/* Return 1 if the FIPS self tests are running and 0 otherwise */
int ossl_fips_self_testing(void);

# endif /* FIPS_MODULE */

#endif
