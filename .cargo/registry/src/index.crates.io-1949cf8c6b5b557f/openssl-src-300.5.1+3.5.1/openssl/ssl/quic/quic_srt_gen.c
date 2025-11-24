/*
 * Copyright 2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "internal/quic_srt_gen.h"
#include <openssl/core_names.h>
#include <openssl/evp.h>

struct quic_srt_gen_st {
    EVP_MAC         *mac;
    EVP_MAC_CTX     *mac_ctx;
};

/*
 * Simple HMAC-SHA256-based stateless reset token generator.
 */

QUIC_SRT_GEN *ossl_quic_srt_gen_new(OSSL_LIB_CTX *libctx, const char *propq,
                                    const unsigned char *key, size_t key_len)
{
    QUIC_SRT_GEN *srt_gen;
    OSSL_PARAM params[3], *p = params;

    if ((srt_gen = OPENSSL_zalloc(sizeof(*srt_gen))) == NULL)
        return NULL;

    if ((srt_gen->mac = EVP_MAC_fetch(libctx, "HMAC", propq)) == NULL)
        goto err;

    if ((srt_gen->mac_ctx = EVP_MAC_CTX_new(srt_gen->mac)) == NULL)
        goto err;

    *p++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST, "SHA256", 7);
    if (propq != NULL)
        *p++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_PROPERTIES,
                                                (char *)propq, 0);
    *p++ = OSSL_PARAM_construct_end();

    if (!EVP_MAC_init(srt_gen->mac_ctx, key, key_len, params))
        goto err;

    return srt_gen;

err:
    ossl_quic_srt_gen_free(srt_gen);
    return NULL;
}

void ossl_quic_srt_gen_free(QUIC_SRT_GEN *srt_gen)
{
    if (srt_gen == NULL)
        return;

    EVP_MAC_CTX_free(srt_gen->mac_ctx);
    EVP_MAC_free(srt_gen->mac);
    OPENSSL_free(srt_gen);
}

int ossl_quic_srt_gen_calculate_token(QUIC_SRT_GEN *srt_gen,
                                      const QUIC_CONN_ID *dcid,
                                      QUIC_STATELESS_RESET_TOKEN *token)
{
    size_t outl = 0;
    unsigned char mac[SHA256_DIGEST_LENGTH];

    if (!EVP_MAC_init(srt_gen->mac_ctx, NULL, 0, NULL))
        return 0;

    if (!EVP_MAC_update(srt_gen->mac_ctx, (const unsigned char *)dcid->id,
                        dcid->id_len))
        return 0;

    if (!EVP_MAC_final(srt_gen->mac_ctx, mac, &outl, sizeof(mac))
        || outl != sizeof(mac))
        return 0;

    assert(sizeof(mac) >= sizeof(token->token));
    memcpy(token->token, mac, sizeof(token->token));
    return 1;
}
