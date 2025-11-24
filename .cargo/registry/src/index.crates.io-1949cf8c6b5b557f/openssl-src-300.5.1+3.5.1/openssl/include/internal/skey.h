/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_CRYPTO_SKEY_H
# define OSSL_CRYPTO_SKEY_H

/* Known symmetric key type definitions */
# define SKEY_TYPE_GENERIC 1 /* generic bytes container unknown key types */
# define SKEY_TYPE_AES 2 /* AES keys */

struct prov_skey_st {
    /*
     * Internal skey implementation,
     * A symmetric key is basically just a buffer of bytes of
     * defined length, and a type, that defines, what
     * cryptosystem the key is meant for (AES, HMAC, etc...)
     */
    OSSL_LIB_CTX *libctx;

    int type;

    unsigned char *data;
    size_t length;
};

#endif /* OSSL_CRYPTO_SKEY_H */
