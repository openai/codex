/*
 * Copyright 2020-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* When removal is simulated, we still need the type internally */

#ifndef OSSL_CRYPTO_TYPES_H
# define OSSL_CRYPTO_TYPES_H
# pragma once

# ifdef OPENSSL_NO_DEPRECATED_3_0
typedef struct rsa_st RSA;
typedef struct rsa_meth_st RSA_METHOD;
#  ifndef OPENSSL_NO_EC
typedef struct ec_key_st EC_KEY;
typedef struct ec_key_method_st EC_KEY_METHOD;
#  endif
#  ifndef OPENSSL_NO_DSA
typedef struct dsa_st DSA;
#  endif
# endif

# ifndef OPENSSL_NO_EC
typedef struct ecx_key_st ECX_KEY;
# endif

typedef struct prov_skey_st PROV_SKEY;

#endif
