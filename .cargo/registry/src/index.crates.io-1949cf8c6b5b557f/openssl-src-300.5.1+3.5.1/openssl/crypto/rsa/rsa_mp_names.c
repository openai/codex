/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/core_names.h>
#include "crypto/rsa.h"

/*
 * The following tables are constants used during RSA parameter building
 * operations. It is easier to point to one of these fixed strings than have
 * to dynamically add and generate the names on the fly.
 */

/*
 * A fixed table of names for the RSA prime factors starting with
 * P,Q and up to 8 additional primes.
 */
const char *ossl_rsa_mp_factor_names[] = {
    OSSL_PKEY_PARAM_RSA_FACTOR1,
    OSSL_PKEY_PARAM_RSA_FACTOR2,
#ifndef FIPS_MODULE
    OSSL_PKEY_PARAM_RSA_FACTOR3,
    OSSL_PKEY_PARAM_RSA_FACTOR4,
    OSSL_PKEY_PARAM_RSA_FACTOR5,
    OSSL_PKEY_PARAM_RSA_FACTOR6,
    OSSL_PKEY_PARAM_RSA_FACTOR7,
    OSSL_PKEY_PARAM_RSA_FACTOR8,
    OSSL_PKEY_PARAM_RSA_FACTOR9,
    OSSL_PKEY_PARAM_RSA_FACTOR10,
#endif
    NULL
};

/*
 * A fixed table of names for the RSA exponents starting with
 * DP,DQ and up to 8 additional exponents.
 */
const char *ossl_rsa_mp_exp_names[] = {
    OSSL_PKEY_PARAM_RSA_EXPONENT1,
    OSSL_PKEY_PARAM_RSA_EXPONENT2,
#ifndef FIPS_MODULE
    OSSL_PKEY_PARAM_RSA_EXPONENT3,
    OSSL_PKEY_PARAM_RSA_EXPONENT4,
    OSSL_PKEY_PARAM_RSA_EXPONENT5,
    OSSL_PKEY_PARAM_RSA_EXPONENT6,
    OSSL_PKEY_PARAM_RSA_EXPONENT7,
    OSSL_PKEY_PARAM_RSA_EXPONENT8,
    OSSL_PKEY_PARAM_RSA_EXPONENT9,
    OSSL_PKEY_PARAM_RSA_EXPONENT10,
#endif
    NULL
};

/*
 * A fixed table of names for the RSA coefficients starting with
 * QINV and up to 8 additional exponents.
 */
const char *ossl_rsa_mp_coeff_names[] = {
    OSSL_PKEY_PARAM_RSA_COEFFICIENT1,
#ifndef FIPS_MODULE
    OSSL_PKEY_PARAM_RSA_COEFFICIENT2,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT3,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT4,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT5,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT6,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT7,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT8,
    OSSL_PKEY_PARAM_RSA_COEFFICIENT9,
#endif
    NULL
};
