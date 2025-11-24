/*
 * Copyright 2006-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/objects.h>
#include "obj_xref.h"
#include "internal/nelem.h"
#include "internal/thread_once.h"
#include <openssl/err.h>

static STACK_OF(nid_triple) *sig_app, *sigx_app;
static CRYPTO_RWLOCK *sig_lock;

static int sig_cmp(const nid_triple *a, const nid_triple *b)
{
    return a->sign_id - b->sign_id;
}

DECLARE_OBJ_BSEARCH_CMP_FN(nid_triple, nid_triple, sig);
IMPLEMENT_OBJ_BSEARCH_CMP_FN(nid_triple, nid_triple, sig);

static int sig_sk_cmp(const nid_triple *const *a, const nid_triple *const *b)
{
    return (*a)->sign_id - (*b)->sign_id;
}

DECLARE_OBJ_BSEARCH_CMP_FN(const nid_triple *, const nid_triple *, sigx);

static int sigx_cmp(const nid_triple *const *a, const nid_triple *const *b)
{
    int ret;

    ret = (*a)->hash_id - (*b)->hash_id;
    /* The "b" side of the comparison carries the algorithms already
     * registered. A NID_undef for 'hash_id' there means that the
     * signature algorithm doesn't need a digest to operate OK. In
     * such case, any hash_id/digest algorithm on the test side (a),
     * incl. NID_undef, is acceptable. signature algorithm NID
     * (pkey_id) must match in any case.
     */
    if ((ret != 0) && ((*b)->hash_id != NID_undef))
        return ret;
    return (*a)->pkey_id - (*b)->pkey_id;
}

IMPLEMENT_OBJ_BSEARCH_CMP_FN(const nid_triple *, const nid_triple *, sigx);

static CRYPTO_ONCE sig_init = CRYPTO_ONCE_STATIC_INIT;

DEFINE_RUN_ONCE_STATIC(o_sig_init)
{
    sig_lock = CRYPTO_THREAD_lock_new();
    return sig_lock != NULL;
}

static ossl_inline int obj_sig_init(void)
{
    return RUN_ONCE(&sig_init, o_sig_init);
}

static int ossl_obj_find_sigid_algs(int signid, int *pdig_nid, int *ppkey_nid,
                                    int lock)
{
    nid_triple tmp;
    const nid_triple *rv;
    int idx;

    if (signid == NID_undef)
        return 0;

    tmp.sign_id = signid;
    rv = OBJ_bsearch_sig(&tmp, sigoid_srt, OSSL_NELEM(sigoid_srt));
    if (rv == NULL) {
        if (!obj_sig_init())
            return 0;
        if (lock && !CRYPTO_THREAD_read_lock(sig_lock)) {
            ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
            return 0;
        }
        if (sig_app != NULL) {
            idx = sk_nid_triple_find(sig_app, &tmp);
            if (idx >= 0)
                rv = sk_nid_triple_value(sig_app, idx);
        }
        if (lock)
            CRYPTO_THREAD_unlock(sig_lock);
        if (rv == NULL)
            return 0;
    }

    if (pdig_nid != NULL)
        *pdig_nid = rv->hash_id;
    if (ppkey_nid != NULL)
        *ppkey_nid = rv->pkey_id;
    return 1;
}

int OBJ_find_sigid_algs(int signid, int *pdig_nid, int *ppkey_nid)
{
    return ossl_obj_find_sigid_algs(signid, pdig_nid, ppkey_nid, 1);
}

int OBJ_find_sigid_by_algs(int *psignid, int dig_nid, int pkey_nid)
{
    nid_triple tmp;
    const nid_triple *t = &tmp;
    const nid_triple **rv;
    int idx;

    /* permitting searches for sig algs without digest: */
    if (pkey_nid == NID_undef)
        return 0;

    tmp.hash_id = dig_nid;
    tmp.pkey_id = pkey_nid;

    rv = OBJ_bsearch_sigx(&t, sigoid_srt_xref, OSSL_NELEM(sigoid_srt_xref));
    if (rv == NULL) {
        if (!obj_sig_init())
            return 0;
        if (!CRYPTO_THREAD_read_lock(sig_lock)) {
            ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_READ_LOCK);
            return 0;
        }
        if (sigx_app != NULL) {
            idx = sk_nid_triple_find(sigx_app, &tmp);
            if (idx >= 0) {
                t = sk_nid_triple_value(sigx_app, idx);
                rv = &t;
            }
        }
        CRYPTO_THREAD_unlock(sig_lock);
        if (rv == NULL)
            return 0;
    }

    if (psignid != NULL)
        *psignid = (*rv)->sign_id;
    return 1;
}

int OBJ_add_sigid(int signid, int dig_id, int pkey_id)
{
    nid_triple *ntr;
    int dnid = NID_undef, pnid = NID_undef, ret = 0;

    if (signid == NID_undef || pkey_id == NID_undef)
        return 0;

    if (!obj_sig_init())
        return 0;

    if ((ntr = OPENSSL_malloc(sizeof(*ntr))) == NULL)
        return 0;
    ntr->sign_id = signid;
    ntr->hash_id = dig_id;
    ntr->pkey_id = pkey_id;

    if (!CRYPTO_THREAD_write_lock(sig_lock)) {
        ERR_raise(ERR_LIB_OBJ, ERR_R_UNABLE_TO_GET_WRITE_LOCK);
        OPENSSL_free(ntr);
        return 0;
    }

    /* Check that the entry doesn't exist or exists as desired */
    if (ossl_obj_find_sigid_algs(signid, &dnid, &pnid, 0)) {
        ret = dnid == dig_id && pnid == pkey_id;
        goto err;
    }

    if (sig_app == NULL) {
        sig_app = sk_nid_triple_new(sig_sk_cmp);
        if (sig_app == NULL)
            goto err;
    }
    if (sigx_app == NULL) {
        sigx_app = sk_nid_triple_new(sigx_cmp);
        if (sigx_app == NULL)
            goto err;
    }

    /*
     * Better might be to find where to insert the element and insert it there.
     * This would avoid the sorting steps below.
     */
    if (!sk_nid_triple_push(sig_app, ntr))
        goto err;
    if (!sk_nid_triple_push(sigx_app, ntr)) {
        ntr = NULL;             /* This is referenced by sig_app still */
        goto err;
    }

    sk_nid_triple_sort(sig_app);
    sk_nid_triple_sort(sigx_app);

    ntr = NULL;
    ret = 1;
 err:
    OPENSSL_free(ntr);
    CRYPTO_THREAD_unlock(sig_lock);
    return ret;
}

static void sid_free(nid_triple *tt)
{
    OPENSSL_free(tt);
}

void OBJ_sigid_free(void)
{
    sk_nid_triple_pop_free(sig_app, sid_free);
    sk_nid_triple_free(sigx_app);
    CRYPTO_THREAD_lock_free(sig_lock);
    sig_app = NULL;
    sigx_app = NULL;
    sig_lock = NULL;
}
