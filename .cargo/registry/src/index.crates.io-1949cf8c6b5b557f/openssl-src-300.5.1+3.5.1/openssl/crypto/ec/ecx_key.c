/*
 * Copyright 2020-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <string.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#include "crypto/ecx.h"
#include "internal/common.h" /* for ossl_assert() */

#ifdef S390X_EC_ASM
# include "s390x_arch.h"
#endif

ECX_KEY *ossl_ecx_key_new(OSSL_LIB_CTX *libctx, ECX_KEY_TYPE type, int haspubkey,
                          const char *propq)
{
    ECX_KEY *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->libctx = libctx;
    ret->haspubkey = haspubkey;
    switch (type) {
    case ECX_KEY_TYPE_X25519:
        ret->keylen = X25519_KEYLEN;
        break;
    case ECX_KEY_TYPE_X448:
        ret->keylen = X448_KEYLEN;
        break;
    case ECX_KEY_TYPE_ED25519:
        ret->keylen = ED25519_KEYLEN;
        break;
    case ECX_KEY_TYPE_ED448:
        ret->keylen = ED448_KEYLEN;
        break;
    }
    ret->type = type;

    if (!CRYPTO_NEW_REF(&ret->references, 1))
        goto err;

    if (propq != NULL) {
        ret->propq = OPENSSL_strdup(propq);
        if (ret->propq == NULL)
            goto err;
    }
    return ret;
err:
    if (ret != NULL) {
        OPENSSL_free(ret->propq);
        CRYPTO_FREE_REF(&ret->references);
    }
    OPENSSL_free(ret);
    return NULL;
}

void ossl_ecx_key_free(ECX_KEY *key)
{
    int i;

    if (key == NULL)
        return;

    CRYPTO_DOWN_REF(&key->references, &i);
    REF_PRINT_COUNT("ECX_KEY", i, key);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    OPENSSL_free(key->propq);
#ifdef OPENSSL_PEDANTIC_ZEROIZATION
    OPENSSL_cleanse(&key->pubkey, sizeof(key->pubkey));
#endif
    OPENSSL_secure_clear_free(key->privkey, key->keylen);
    CRYPTO_FREE_REF(&key->references);
    OPENSSL_free(key);
}

void ossl_ecx_key_set0_libctx(ECX_KEY *key, OSSL_LIB_CTX *libctx)
{
    key->libctx = libctx;
}

int ossl_ecx_key_up_ref(ECX_KEY *key)
{
    int i;

    if (CRYPTO_UP_REF(&key->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("ECX_KEY", i, key);
    REF_ASSERT_ISNT(i < 2);
    return ((i > 1) ? 1 : 0);
}

unsigned char *ossl_ecx_key_allocate_privkey(ECX_KEY *key)
{
    key->privkey = OPENSSL_secure_zalloc(key->keylen);

    return key->privkey;
}

int ossl_ecx_compute_key(ECX_KEY *peer, ECX_KEY *priv, size_t keylen,
                         unsigned char *secret, size_t *secretlen, size_t outlen)
{
    if (priv == NULL
            || priv->privkey == NULL
            || peer == NULL) {
        ERR_raise(ERR_LIB_PROV, PROV_R_MISSING_KEY);
        return 0;
    }

    if (!ossl_assert(keylen == X25519_KEYLEN
            || keylen == X448_KEYLEN)) {
        ERR_raise(ERR_LIB_PROV, PROV_R_INVALID_KEY_LENGTH);
        return 0;
    }

    if (secret == NULL) {
        *secretlen = keylen;
        return 1;
    }
    if (outlen < keylen) {
        ERR_raise(ERR_LIB_PROV, PROV_R_OUTPUT_BUFFER_TOO_SMALL);
        return 0;
    }

    if (keylen == X25519_KEYLEN) {
#ifdef S390X_EC_ASM
        if (OPENSSL_s390xcap_P.pcc[1]
                & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X25519)) {
            if (s390x_x25519_mul(secret, peer->pubkey, priv->privkey) == 0) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_DURING_DERIVATION);
                return 0;
            }
        } else
#endif
        if (ossl_x25519(secret, priv->privkey, peer->pubkey) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_DURING_DERIVATION);
            return 0;
        }
    } else {
#ifdef S390X_EC_ASM
        if (OPENSSL_s390xcap_P.pcc[1]
                & S390X_CAPBIT(S390X_SCALAR_MULTIPLY_X448)) {
            if (s390x_x448_mul(secret, peer->pubkey, priv->privkey) == 0) {
                ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_DURING_DERIVATION);
                return 0;
            }
        } else
#endif
        if (ossl_x448(secret, priv->privkey, peer->pubkey) == 0) {
            ERR_raise(ERR_LIB_PROV, PROV_R_FAILED_DURING_DERIVATION);
            return 0;
        }
    }
    *secretlen = keylen;
    return 1;
}
