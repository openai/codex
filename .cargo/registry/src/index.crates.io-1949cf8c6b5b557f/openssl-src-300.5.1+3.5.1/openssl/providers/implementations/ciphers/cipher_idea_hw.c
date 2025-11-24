/*
 * Copyright 2019-2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * IDEA low level APIs are deprecated for public use, but still ok for internal
 * use where we're using them to implement the higher level EVP interface, as is
 * the case here.
 */
#include "internal/deprecated.h"

#include "cipher_idea.h"

static int cipher_hw_idea_initkey(PROV_CIPHER_CTX *ctx,
                                  const unsigned char *key, size_t keylen)
{
    PROV_IDEA_CTX *ictx =  (PROV_IDEA_CTX *)ctx;
    IDEA_KEY_SCHEDULE *ks = &(ictx->ks.ks);

    if (ctx->enc
            || ctx->mode == EVP_CIPH_OFB_MODE
            || ctx->mode == EVP_CIPH_CFB_MODE) {
        IDEA_set_encrypt_key(key, ks);
    } else {
        IDEA_KEY_SCHEDULE tmp;

        IDEA_set_encrypt_key(key, &tmp);
        IDEA_set_decrypt_key(&tmp, ks);
        OPENSSL_cleanse((unsigned char *)&tmp, sizeof(IDEA_KEY_SCHEDULE));
    }
    return 1;
}

# define PROV_CIPHER_HW_idea_mode_ex(mode, UCMODE, fname)                      \
IMPLEMENT_CIPHER_HW_##UCMODE(mode, idea, PROV_IDEA_CTX, IDEA_KEY_SCHEDULE,     \
                             fname)                                            \
static const PROV_CIPHER_HW idea_##mode = {                                    \
    cipher_hw_idea_initkey,                                                    \
    cipher_hw_idea_##mode##_cipher                                             \
};                                                                             \
const PROV_CIPHER_HW *ossl_prov_cipher_hw_idea_##mode(size_t keybits)          \
{                                                                              \
    return &idea_##mode;                                                       \
}

# define PROV_CIPHER_HW_idea_mode(mode, UCMODE)                                \
    PROV_CIPHER_HW_idea_mode_ex(mode, UCMODE, IDEA_##mode)

PROV_CIPHER_HW_idea_mode(cbc, CBC)
PROV_CIPHER_HW_idea_mode(ofb64, OFB)
PROV_CIPHER_HW_idea_mode(cfb64, CFB)
/*
 * IDEA_ecb_encrypt() does not have a enc parameter  - so we create a macro
 * that ignores this parameter when IMPLEMENT_CIPHER_HW_ecb() is called.
 */
#define IDEA2_ecb_encrypt(in, out, ks, enc) IDEA_ecb_encrypt(in, out, ks)

PROV_CIPHER_HW_idea_mode_ex(ecb, ECB, IDEA2_ecb)
