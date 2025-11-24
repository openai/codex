/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/* We need to use some engine deprecated APIs */
#define OPENSSL_SUPPRESS_DEPRECATED

#include <openssl/evp.h>
#include <openssl/core_names.h>
#include <openssl/err.h>
#include <openssl/proverr.h>
#ifndef FIPS_MODULE
# include <openssl/engine.h>
# include "crypto/evp.h"
#endif
#include "prov/providercommon.h"
#include "prov/provider_util.h"

void ossl_prov_cipher_reset(PROV_CIPHER *pc)
{
    EVP_CIPHER_free(pc->alloc_cipher);
    pc->alloc_cipher = NULL;
    pc->cipher = NULL;
#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    ENGINE_finish(pc->engine);
#endif
    pc->engine = NULL;
}

int ossl_prov_cipher_copy(PROV_CIPHER *dst, const PROV_CIPHER *src)
{
    if (src->alloc_cipher != NULL && !EVP_CIPHER_up_ref(src->alloc_cipher))
        return 0;
#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    if (src->engine != NULL && !ENGINE_init(src->engine)) {
        EVP_CIPHER_free(src->alloc_cipher);
        return 0;
    }
#endif
    dst->engine = src->engine;
    dst->cipher = src->cipher;
    dst->alloc_cipher = src->alloc_cipher;
    return 1;
}

static int load_common(const OSSL_PARAM params[], const char **propquery,
                       ENGINE **engine)
{
    const OSSL_PARAM *p;

    *propquery = NULL;
    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_PROPERTIES);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        *propquery = p->data;
    }

#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    ENGINE_finish(*engine);
#endif
    *engine = NULL;
    /* Inside the FIPS module, we don't support legacy ciphers */
#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_ENGINE);
    if (p != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        /* Get a structural reference */
        *engine = ENGINE_by_id(p->data);
        if (*engine == NULL)
            return 0;
        /* Get a functional reference */
        if (!ENGINE_init(*engine)) {
            ENGINE_free(*engine);
            *engine = NULL;
            return 0;
        }
        /* Free the structural reference */
        ENGINE_free(*engine);
    }
#endif
    return 1;
}

int ossl_prov_cipher_load_from_params(PROV_CIPHER *pc,
                                      const OSSL_PARAM params[],
                                      OSSL_LIB_CTX *ctx)
{
    const OSSL_PARAM *p;
    const char *propquery;

    if (ossl_param_is_empty(params))
        return 1;

    if (!load_common(params, &propquery, &pc->engine))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_CIPHER);
    if (p == NULL)
        return 1;
    if (p->data_type != OSSL_PARAM_UTF8_STRING)
        return 0;

    EVP_CIPHER_free(pc->alloc_cipher);
    ERR_set_mark();
    pc->cipher = pc->alloc_cipher = EVP_CIPHER_fetch(ctx, p->data, propquery);
#ifndef FIPS_MODULE /* Inside the FIPS module, we don't support legacy ciphers */
    if (pc->cipher == NULL) {
        const EVP_CIPHER *cipher;

        cipher = EVP_get_cipherbyname(p->data);
        /* Do not use global EVP_CIPHERs */
        if (cipher != NULL && cipher->origin != EVP_ORIG_GLOBAL)
            pc->cipher = cipher;
    }
#endif
    if (pc->cipher != NULL)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return pc->cipher != NULL;
}

const EVP_CIPHER *ossl_prov_cipher_cipher(const PROV_CIPHER *pc)
{
    return pc->cipher;
}

ENGINE *ossl_prov_cipher_engine(const PROV_CIPHER *pc)
{
    return pc->engine;
}

void ossl_prov_digest_reset(PROV_DIGEST *pd)
{
    EVP_MD_free(pd->alloc_md);
    pd->alloc_md = NULL;
    pd->md = NULL;
#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    ENGINE_finish(pd->engine);
#endif
    pd->engine = NULL;
}

int ossl_prov_digest_copy(PROV_DIGEST *dst, const PROV_DIGEST *src)
{
    if (src->alloc_md != NULL && !EVP_MD_up_ref(src->alloc_md))
        return 0;
#if !defined(FIPS_MODULE) && !defined(OPENSSL_NO_ENGINE)
    if (src->engine != NULL && !ENGINE_init(src->engine)) {
        EVP_MD_free(src->alloc_md);
        return 0;
    }
#endif
    dst->engine = src->engine;
    dst->md = src->md;
    dst->alloc_md = src->alloc_md;
    return 1;
}

const EVP_MD *ossl_prov_digest_fetch(PROV_DIGEST *pd, OSSL_LIB_CTX *libctx,
                                     const char *mdname, const char *propquery)
{
    EVP_MD_free(pd->alloc_md);
    pd->md = pd->alloc_md = EVP_MD_fetch(libctx, mdname, propquery);

    return pd->md;
}

int ossl_prov_digest_load_from_params(PROV_DIGEST *pd,
                                      const OSSL_PARAM params[],
                                      OSSL_LIB_CTX *ctx)
{
    const OSSL_PARAM *p;
    const char *propquery;

    if (ossl_param_is_empty(params))
        return 1;

    if (!load_common(params, &propquery, &pd->engine))
        return 0;

    p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_DIGEST);
    if (p == NULL)
        return 1;
    if (p->data_type != OSSL_PARAM_UTF8_STRING)
        return 0;

    ERR_set_mark();
    ossl_prov_digest_fetch(pd, ctx, p->data, propquery);
#ifndef FIPS_MODULE /* Inside the FIPS module, we don't support legacy digests */
    if (pd->md == NULL) {
        const EVP_MD *md;

        md = EVP_get_digestbyname(p->data);
        /* Do not use global EVP_MDs */
        if (md != NULL && md->origin != EVP_ORIG_GLOBAL)
            pd->md = md;
    }
#endif
    if (pd->md != NULL)
        ERR_pop_to_mark();
    else
        ERR_clear_last_mark();
    return pd->md != NULL;
}

void ossl_prov_digest_set_md(PROV_DIGEST *pd, EVP_MD *md)
{
    ossl_prov_digest_reset(pd);
    pd->md = pd->alloc_md = md;
}

const EVP_MD *ossl_prov_digest_md(const PROV_DIGEST *pd)
{
    return pd->md;
}

ENGINE *ossl_prov_digest_engine(const PROV_DIGEST *pd)
{
    return pd->engine;
}

int ossl_prov_set_macctx(EVP_MAC_CTX *macctx,
                         const OSSL_PARAM params[],
                         const char *ciphername,
                         const char *mdname,
                         const char *engine,
                         const char *properties,
                         const unsigned char *key,
                         size_t keylen)
{
    const OSSL_PARAM *p;
    OSSL_PARAM mac_params[6], *mp = mac_params;

    if (params != NULL) {
        if (mdname == NULL) {
            if ((p = OSSL_PARAM_locate_const(params,
                                            OSSL_ALG_PARAM_DIGEST)) != NULL) {
                if (p->data_type != OSSL_PARAM_UTF8_STRING)
                    return 0;
                mdname = p->data;
            }
        }
        if (ciphername == NULL) {
            if ((p = OSSL_PARAM_locate_const(params,
                                            OSSL_ALG_PARAM_CIPHER)) != NULL) {
                if (p->data_type != OSSL_PARAM_UTF8_STRING)
                    return 0;
                ciphername = p->data;
            }
        }
        if (engine == NULL) {
            if ((p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_ENGINE))
                    != NULL) {
                if (p->data_type != OSSL_PARAM_UTF8_STRING)
                    return 0;
                engine = p->data;
            }
        }
    }

    if (mdname != NULL)
        *mp++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                                 (char *)mdname, 0);
    if (ciphername != NULL)
        *mp++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_CIPHER,
                                                 (char *)ciphername, 0);
    if (properties != NULL)
        *mp++ = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_PROPERTIES,
                                                 (char *)properties, 0);

#if !defined(OPENSSL_NO_ENGINE) && !defined(FIPS_MODULE)
    if (engine != NULL)
        *mp++ = OSSL_PARAM_construct_utf8_string(OSSL_ALG_PARAM_ENGINE,
                                                 (char *) engine, 0);
#endif

    if (key != NULL)
        *mp++ = OSSL_PARAM_construct_octet_string(OSSL_MAC_PARAM_KEY,
                                                  (unsigned char *)key,
                                                  keylen);

    *mp = OSSL_PARAM_construct_end();

    return EVP_MAC_CTX_set_params(macctx, mac_params);

}

int ossl_prov_macctx_load_from_params(EVP_MAC_CTX **macctx,
                                      const OSSL_PARAM params[],
                                      const char *macname,
                                      const char *ciphername,
                                      const char *mdname,
                                      OSSL_LIB_CTX *libctx)
{
    const OSSL_PARAM *p;
    const char *properties = NULL;

    if (macname == NULL
        && (p = OSSL_PARAM_locate_const(params, OSSL_ALG_PARAM_MAC)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        macname = p->data;
    }
    if ((p = OSSL_PARAM_locate_const(params,
                                     OSSL_ALG_PARAM_PROPERTIES)) != NULL) {
        if (p->data_type != OSSL_PARAM_UTF8_STRING)
            return 0;
        properties = p->data;
    }

    /* If we got a new mac name, we make a new EVP_MAC_CTX */
    if (macname != NULL) {
        EVP_MAC *mac = EVP_MAC_fetch(libctx, macname, properties);

        EVP_MAC_CTX_free(*macctx);
        *macctx = mac == NULL ? NULL : EVP_MAC_CTX_new(mac);
        /* The context holds on to the MAC */
        EVP_MAC_free(mac);
        if (*macctx == NULL)
            return 0;
    }

    /*
     * If there is no MAC yet (and therefore, no MAC context), we ignore
     * all other parameters.
     */
    if (*macctx == NULL)
        return 1;

    if (ossl_prov_set_macctx(*macctx, params, ciphername, mdname, NULL,
                             properties, NULL, 0))
        return 1;

    EVP_MAC_CTX_free(*macctx);
    *macctx = NULL;
    return 0;
}

void ossl_prov_cache_exported_algorithms(const OSSL_ALGORITHM_CAPABLE *in,
                                         OSSL_ALGORITHM *out)
{
    int i, j;

    if (out[0].algorithm_names == NULL) {
        for (i = j = 0; in[i].alg.algorithm_names != NULL; ++i) {
            if (in[i].capable == NULL || in[i].capable())
                out[j++] = in[i].alg;
        }
        out[j++] = in[i].alg;
    }
}

/* Duplicate a lump of memory safely */
int ossl_prov_memdup(const void *src, size_t src_len,
                     unsigned char **dest, size_t *dest_len)
{
    if (src != NULL) {
        if ((*dest = OPENSSL_memdup(src, src_len)) == NULL)
            return 0;
        *dest_len = src_len;
    } else {
        *dest = NULL;
        *dest_len = 0;
    }
    return 1;
}
