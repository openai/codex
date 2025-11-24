/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#define ISX448(id)      ((id) == EVP_PKEY_X448)
#define IS25519(id)     ((id) == EVP_PKEY_X25519 || (id) == EVP_PKEY_ED25519)
#define KEYLENID(id)    (IS25519(id) ? X25519_KEYLEN \
                                     : ((id) == EVP_PKEY_X448 ? X448_KEYLEN \
                                                              : ED448_KEYLEN))
#define KEYNID2TYPE(id) \
    (IS25519(id) ? ((id) == EVP_PKEY_X25519 ? ECX_KEY_TYPE_X25519 \
                                            : ECX_KEY_TYPE_ED25519) \
                 : ((id) == EVP_PKEY_X448 ? ECX_KEY_TYPE_X448 \
                                          : ECX_KEY_TYPE_ED448))
#define KEYLEN(p)       KEYLENID((p)->ameth->pkey_id)
