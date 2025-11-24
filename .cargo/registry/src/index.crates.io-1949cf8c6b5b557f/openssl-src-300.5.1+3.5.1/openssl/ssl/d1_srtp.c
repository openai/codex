/*
 * Copyright 2011-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * DTLS code by Eric Rescorla <ekr@rtfm.com>
 *
 * Copyright (C) 2006, Network Resonance, Inc. Copyright (C) 2011, RTFM, Inc.
 */

#include <stdio.h>
#include <openssl/objects.h>
#include "ssl_local.h"
#include "internal/ssl_unwrap.h"

#ifndef OPENSSL_NO_SRTP

static const SRTP_PROTECTION_PROFILE srtp_known_profiles[] = {
    {
     "SRTP_AES128_CM_SHA1_80",
     SRTP_AES128_CM_SHA1_80,
     },
    {
     "SRTP_AES128_CM_SHA1_32",
     SRTP_AES128_CM_SHA1_32,
     },
    {
     "SRTP_AEAD_AES_128_GCM",
     SRTP_AEAD_AES_128_GCM,
     },
    {
     "SRTP_AEAD_AES_256_GCM",
     SRTP_AEAD_AES_256_GCM,
     },
    {
     "SRTP_DOUBLE_AEAD_AES_128_GCM_AEAD_AES_128_GCM",
     SRTP_DOUBLE_AEAD_AES_128_GCM_AEAD_AES_128_GCM,
     },
    {
     "SRTP_DOUBLE_AEAD_AES_256_GCM_AEAD_AES_256_GCM",
     SRTP_DOUBLE_AEAD_AES_256_GCM_AEAD_AES_256_GCM,
     },
    {
     "SRTP_ARIA_128_CTR_HMAC_SHA1_80",
     SRTP_ARIA_128_CTR_HMAC_SHA1_80,
     },
    {
     "SRTP_ARIA_128_CTR_HMAC_SHA1_32",
     SRTP_ARIA_128_CTR_HMAC_SHA1_32,
     },
    {
     "SRTP_ARIA_256_CTR_HMAC_SHA1_80",
     SRTP_ARIA_256_CTR_HMAC_SHA1_80,
     },
    {
     "SRTP_ARIA_256_CTR_HMAC_SHA1_32",
     SRTP_ARIA_256_CTR_HMAC_SHA1_32,
     },
    {
     "SRTP_AEAD_ARIA_128_GCM",
     SRTP_AEAD_ARIA_128_GCM,
     },
    {
     "SRTP_AEAD_ARIA_256_GCM",
     SRTP_AEAD_ARIA_256_GCM,
     },
    {0}
};

static int find_profile_by_name(char *profile_name,
                                const SRTP_PROTECTION_PROFILE **pptr, size_t len)
{
    const SRTP_PROTECTION_PROFILE *p;

    p = srtp_known_profiles;
    while (p->name) {
        if ((len == strlen(p->name))
            && strncmp(p->name, profile_name, len) == 0) {
            *pptr = p;
            return 0;
        }

        p++;
    }

    return 1;
}

static int ssl_ctx_make_profiles(const char *profiles_string,
                                 STACK_OF(SRTP_PROTECTION_PROFILE) **out)
{
    STACK_OF(SRTP_PROTECTION_PROFILE) *profiles;

    char *col;
    char *ptr = (char *)profiles_string;
    const SRTP_PROTECTION_PROFILE *p;

    if ((profiles = sk_SRTP_PROTECTION_PROFILE_new_null()) == NULL) {
        ERR_raise(ERR_LIB_SSL, SSL_R_SRTP_COULD_NOT_ALLOCATE_PROFILES);
        return 1;
    }

    do {
        col = strchr(ptr, ':');

        if (!find_profile_by_name(ptr, &p, col ? (size_t)(col - ptr)
                                               : strlen(ptr))) {
            if (sk_SRTP_PROTECTION_PROFILE_find(profiles,
                                                (SRTP_PROTECTION_PROFILE *)p) >= 0) {
                ERR_raise(ERR_LIB_SSL, SSL_R_BAD_SRTP_PROTECTION_PROFILE_LIST);
                goto err;
            }

            if (!sk_SRTP_PROTECTION_PROFILE_push(profiles,
                                                 (SRTP_PROTECTION_PROFILE *)p)) {
                ERR_raise(ERR_LIB_SSL, SSL_R_SRTP_COULD_NOT_ALLOCATE_PROFILES);
                goto err;
            }
        } else {
            ERR_raise(ERR_LIB_SSL, SSL_R_SRTP_UNKNOWN_PROTECTION_PROFILE);
            goto err;
        }

        if (col)
            ptr = col + 1;
    } while (col);

    sk_SRTP_PROTECTION_PROFILE_free(*out);

    *out = profiles;

    return 0;
 err:
    sk_SRTP_PROTECTION_PROFILE_free(profiles);
    return 1;
}

int SSL_CTX_set_tlsext_use_srtp(SSL_CTX *ctx, const char *profiles)
{
    if (IS_QUIC_METHOD(ctx->method))
        return 1;

    return ssl_ctx_make_profiles(profiles, &ctx->srtp_profiles);
}

int SSL_set_tlsext_use_srtp(SSL *s, const char *profiles)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 1;

    return ssl_ctx_make_profiles(profiles, &sc->srtp_profiles);
}

STACK_OF(SRTP_PROTECTION_PROFILE) *SSL_get_srtp_profiles(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc != NULL) {
        if (sc->srtp_profiles != NULL) {
            return sc->srtp_profiles;
        } else if ((s->ctx != NULL) && (s->ctx->srtp_profiles != NULL)) {
            return s->ctx->srtp_profiles;
        }
    }

    return NULL;
}

SRTP_PROTECTION_PROFILE *SSL_get_selected_srtp_profile(SSL *s)
{
    SSL_CONNECTION *sc = SSL_CONNECTION_FROM_SSL_ONLY(s);

    if (sc == NULL)
        return 0;

    return sc->srtp_profile;
}
#endif
