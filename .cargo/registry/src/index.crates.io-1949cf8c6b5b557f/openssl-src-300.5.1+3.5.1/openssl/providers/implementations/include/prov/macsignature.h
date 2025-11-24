/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <openssl/crypto.h>
#include "internal/refcount.h"
#include "prov/provider_util.h"

struct mac_key_st {
    OSSL_LIB_CTX *libctx;
    CRYPTO_REF_COUNT refcnt;
    unsigned char *priv_key;
    size_t priv_key_len;
    PROV_CIPHER cipher;
    char *properties;
    int cmac;
};

typedef struct mac_key_st MAC_KEY;

MAC_KEY *ossl_mac_key_new(OSSL_LIB_CTX *libctx, int cmac);
void ossl_mac_key_free(MAC_KEY *mackey);
int ossl_mac_key_up_ref(MAC_KEY *mackey);
