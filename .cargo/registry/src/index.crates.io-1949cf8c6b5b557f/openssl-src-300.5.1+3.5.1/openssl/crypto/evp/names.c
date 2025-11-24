/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <openssl/evp.h>
#include <openssl/kdf.h>
#include <openssl/x509.h>
#include "internal/cryptlib.h"
#include "internal/namemap.h"
#include "crypto/objects.h"
#include "crypto/evp.h"

int EVP_add_cipher(const EVP_CIPHER *c)
{
    int r;

    if (c == NULL)
        return 0;

    r = OBJ_NAME_add(OBJ_nid2sn(c->nid), OBJ_NAME_TYPE_CIPHER_METH,
                     (const char *)c);
    if (r == 0)
        return 0;
    r = OBJ_NAME_add(OBJ_nid2ln(c->nid), OBJ_NAME_TYPE_CIPHER_METH,
                     (const char *)c);
    return r;
}

int EVP_add_digest(const EVP_MD *md)
{
    int r;
    const char *name;

    name = OBJ_nid2sn(md->type);
    r = OBJ_NAME_add(name, OBJ_NAME_TYPE_MD_METH, (const char *)md);
    if (r == 0)
        return 0;
    r = OBJ_NAME_add(OBJ_nid2ln(md->type), OBJ_NAME_TYPE_MD_METH,
                     (const char *)md);
    if (r == 0)
        return 0;

    if (md->pkey_type && md->type != md->pkey_type) {
        r = OBJ_NAME_add(OBJ_nid2sn(md->pkey_type),
                         OBJ_NAME_TYPE_MD_METH | OBJ_NAME_ALIAS, name);
        if (r == 0)
            return 0;
        r = OBJ_NAME_add(OBJ_nid2ln(md->pkey_type),
                         OBJ_NAME_TYPE_MD_METH | OBJ_NAME_ALIAS, name);
    }
    return r;
}

static void cipher_from_name(const char *name, void *data)
{
    const EVP_CIPHER **cipher = data;

    if (*cipher != NULL)
        return;

    *cipher = (const EVP_CIPHER *)OBJ_NAME_get(name, OBJ_NAME_TYPE_CIPHER_METH);
}

const EVP_CIPHER *EVP_get_cipherbyname(const char *name)
{
    return evp_get_cipherbyname_ex(NULL, name);
}

const EVP_CIPHER *evp_get_cipherbyname_ex(OSSL_LIB_CTX *libctx,
                                          const char *name)
{
    const EVP_CIPHER *cp;
    OSSL_NAMEMAP *namemap;
    int id;
    int do_retry = 1;

    if (!OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_CIPHERS, NULL))
        return NULL;

    cp = (const EVP_CIPHER *)OBJ_NAME_get(name, OBJ_NAME_TYPE_CIPHER_METH);

    if (cp != NULL)
        return cp;

    /*
     * It's not in the method database, but it might be there under a different
     * name. So we check for aliases in the EVP namemap and try all of those
     * in turn.
     */

    namemap = ossl_namemap_stored(libctx);
 retry:
    id = ossl_namemap_name2num(namemap, name);
    if (id == 0) {
        EVP_CIPHER *fetched_cipher;

        /* Try to fetch it because the name might not be known yet. */
        if (!do_retry)
            return NULL;
        do_retry = 0;
        ERR_set_mark();
        fetched_cipher = EVP_CIPHER_fetch(libctx, name, NULL);
        EVP_CIPHER_free(fetched_cipher);
        ERR_pop_to_mark();
        goto retry;
    }

    if (!ossl_namemap_doall_names(namemap, id, cipher_from_name, &cp))
        return NULL;

    return cp;
}

static void digest_from_name(const char *name, void *data)
{
    const EVP_MD **md = data;

    if (*md != NULL)
        return;

    *md = (const EVP_MD *)OBJ_NAME_get(name, OBJ_NAME_TYPE_MD_METH);
}

const EVP_MD *EVP_get_digestbyname(const char *name)
{
    return evp_get_digestbyname_ex(NULL, name);
}

const EVP_MD *evp_get_digestbyname_ex(OSSL_LIB_CTX *libctx, const char *name)
{
    const EVP_MD *dp;
    OSSL_NAMEMAP *namemap;
    int id;
    int do_retry = 1;

    if (!OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_DIGESTS, NULL))
        return NULL;

    dp = (const EVP_MD *)OBJ_NAME_get(name, OBJ_NAME_TYPE_MD_METH);

    if (dp != NULL)
        return dp;

    /*
     * It's not in the method database, but it might be there under a different
     * name. So we check for aliases in the EVP namemap and try all of those
     * in turn.
     */

    namemap = ossl_namemap_stored(libctx);
 retry:
    id = ossl_namemap_name2num(namemap, name);
    if (id == 0) {
        EVP_MD *fetched_md;

        /* Try to fetch it because the name might not be known yet. */
        if (!do_retry)
            return NULL;
        do_retry = 0;
        ERR_set_mark();
        fetched_md = EVP_MD_fetch(libctx, name, NULL);
        EVP_MD_free(fetched_md);
        ERR_pop_to_mark();
        goto retry;
    }

    if (!ossl_namemap_doall_names(namemap, id, digest_from_name, &dp))
        return NULL;

    return dp;
}

void evp_cleanup_int(void)
{
    OBJ_NAME_cleanup(OBJ_NAME_TYPE_KDF_METH);
    OBJ_NAME_cleanup(OBJ_NAME_TYPE_CIPHER_METH);
    OBJ_NAME_cleanup(OBJ_NAME_TYPE_MD_METH);
    /*
     * The above calls will only clean out the contents of the name hash
     * table, but not the hash table itself.  The following line does that
     * part.  -- Richard Levitte
     */
    OBJ_NAME_cleanup(-1);

    EVP_PBE_cleanup();
    OBJ_sigid_free();

    evp_app_cleanup_int();
}

struct doall_cipher {
    void *arg;
    void (*fn) (const EVP_CIPHER *ciph,
                const char *from, const char *to, void *arg);
};

static void do_all_cipher_fn(const OBJ_NAME *nm, void *arg)
{
    struct doall_cipher *dc = arg;
    if (nm->alias)
        dc->fn(NULL, nm->name, nm->data, dc->arg);
    else
        dc->fn((const EVP_CIPHER *)nm->data, nm->name, NULL, dc->arg);
}

void EVP_CIPHER_do_all(void (*fn) (const EVP_CIPHER *ciph,
                                   const char *from, const char *to, void *x),
                       void *arg)
{
    struct doall_cipher dc;

    /* Ignore errors */
    OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_CIPHERS, NULL);

    dc.fn = fn;
    dc.arg = arg;
    OBJ_NAME_do_all(OBJ_NAME_TYPE_CIPHER_METH, do_all_cipher_fn, &dc);
}

void EVP_CIPHER_do_all_sorted(void (*fn) (const EVP_CIPHER *ciph,
                                          const char *from, const char *to,
                                          void *x), void *arg)
{
    struct doall_cipher dc;

    /* Ignore errors */
    OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_CIPHERS, NULL);

    dc.fn = fn;
    dc.arg = arg;
    OBJ_NAME_do_all_sorted(OBJ_NAME_TYPE_CIPHER_METH, do_all_cipher_fn, &dc);
}

struct doall_md {
    void *arg;
    void (*fn) (const EVP_MD *ciph,
                const char *from, const char *to, void *arg);
};

static void do_all_md_fn(const OBJ_NAME *nm, void *arg)
{
    struct doall_md *dc = arg;
    if (nm->alias)
        dc->fn(NULL, nm->name, nm->data, dc->arg);
    else
        dc->fn((const EVP_MD *)nm->data, nm->name, NULL, dc->arg);
}

void EVP_MD_do_all(void (*fn) (const EVP_MD *md,
                               const char *from, const char *to, void *x),
                   void *arg)
{
    struct doall_md dc;

    /* Ignore errors */
    OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_DIGESTS, NULL);

    dc.fn = fn;
    dc.arg = arg;
    OBJ_NAME_do_all(OBJ_NAME_TYPE_MD_METH, do_all_md_fn, &dc);
}

void EVP_MD_do_all_sorted(void (*fn) (const EVP_MD *md,
                                      const char *from, const char *to,
                                      void *x), void *arg)
{
    struct doall_md dc;

    OPENSSL_init_crypto(OPENSSL_INIT_ADD_ALL_DIGESTS, NULL);

    dc.fn = fn;
    dc.arg = arg;
    OBJ_NAME_do_all_sorted(OBJ_NAME_TYPE_MD_METH, do_all_md_fn, &dc);
}
