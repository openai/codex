/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include "internal/cryptlib.h"
#include "internal/refcount.h"
#include <openssl/x509.h>
#include "crypto/x509.h"
#include <openssl/x509v3.h>
#include "x509_local.h"

X509_LOOKUP *X509_LOOKUP_new(X509_LOOKUP_METHOD *method)
{
    X509_LOOKUP *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;

    ret->method = method;
    if (method->new_item != NULL && method->new_item(ret) == 0) {
        OPENSSL_free(ret);
        return NULL;
    }
    return ret;
}

void X509_LOOKUP_free(X509_LOOKUP *ctx)
{
    if (ctx == NULL)
        return;
    if ((ctx->method != NULL) && (ctx->method->free != NULL))
        (*ctx->method->free) (ctx);
    OPENSSL_free(ctx);
}

int X509_STORE_lock(X509_STORE *xs)
{
    return CRYPTO_THREAD_write_lock(xs->lock);
}

static int x509_store_read_lock(X509_STORE *xs)
{
    return CRYPTO_THREAD_read_lock(xs->lock);
}

int X509_STORE_unlock(X509_STORE *xs)
{
    return CRYPTO_THREAD_unlock(xs->lock);
}

int X509_LOOKUP_init(X509_LOOKUP *ctx)
{
    if (ctx->method == NULL)
        return 0;
    if (ctx->method->init != NULL)
        return ctx->method->init(ctx);
    else
        return 1;
}

int X509_LOOKUP_shutdown(X509_LOOKUP *ctx)
{
    if (ctx->method == NULL)
        return 0;
    if (ctx->method->shutdown != NULL)
        return ctx->method->shutdown(ctx);
    else
        return 1;
}

int X509_LOOKUP_ctrl_ex(X509_LOOKUP *ctx, int cmd, const char *argc, long argl,
                        char **ret, OSSL_LIB_CTX *libctx, const char *propq)
{
    if (ctx->method == NULL)
        return -1;
    if (ctx->method->ctrl_ex != NULL)
        return ctx->method->ctrl_ex(ctx, cmd, argc, argl, ret, libctx, propq);
    if (ctx->method->ctrl != NULL)
        return ctx->method->ctrl(ctx, cmd, argc, argl, ret);
    return 1;
}

int X509_LOOKUP_ctrl(X509_LOOKUP *ctx, int cmd, const char *argc, long argl,
                     char **ret)
{
    return X509_LOOKUP_ctrl_ex(ctx, cmd, argc, argl, ret, NULL, NULL);
}

int X509_LOOKUP_by_subject_ex(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                              const X509_NAME *name, X509_OBJECT *ret,
                              OSSL_LIB_CTX *libctx, const char *propq)
{
    if (ctx->skip
        || ctx->method == NULL
        || (ctx->method->get_by_subject == NULL
            && ctx->method->get_by_subject_ex == NULL))
        return 0;
    if (ctx->method->get_by_subject_ex != NULL)
        return ctx->method->get_by_subject_ex(ctx, type, name, ret, libctx,
                                              propq);
    else
        return ctx->method->get_by_subject(ctx, type, name, ret);
}

int X509_LOOKUP_by_subject(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                           const X509_NAME *name, X509_OBJECT *ret)
{
    return X509_LOOKUP_by_subject_ex(ctx, type, name, ret, NULL, NULL);
}

int X509_LOOKUP_by_issuer_serial(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                                 const X509_NAME *name,
                                 const ASN1_INTEGER *serial,
                                 X509_OBJECT *ret)
{
    if ((ctx->method == NULL) || (ctx->method->get_by_issuer_serial == NULL))
        return 0;
    return ctx->method->get_by_issuer_serial(ctx, type, name, serial, ret);
}

int X509_LOOKUP_by_fingerprint(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                               const unsigned char *bytes, int len,
                               X509_OBJECT *ret)
{
    if ((ctx->method == NULL) || (ctx->method->get_by_fingerprint == NULL))
        return 0;
    return ctx->method->get_by_fingerprint(ctx, type, bytes, len, ret);
}

int X509_LOOKUP_by_alias(X509_LOOKUP *ctx, X509_LOOKUP_TYPE type,
                         const char *str, int len, X509_OBJECT *ret)
{
    if ((ctx->method == NULL) || (ctx->method->get_by_alias == NULL))
        return 0;
    return ctx->method->get_by_alias(ctx, type, str, len, ret);
}

int X509_LOOKUP_set_method_data(X509_LOOKUP *ctx, void *data)
{
    ctx->method_data = data;
    return 1;
}

void *X509_LOOKUP_get_method_data(const X509_LOOKUP *ctx)
{
    return ctx->method_data;
}

X509_STORE *X509_LOOKUP_get_store(const X509_LOOKUP *ctx)
{
    return ctx->store_ctx;
}

static int x509_object_cmp(const X509_OBJECT *const *a,
                           const X509_OBJECT *const *b)
{
    int ret;

    ret = ((*a)->type - (*b)->type);
    if (ret)
        return ret;
    switch ((*a)->type) {
    case X509_LU_X509:
        ret = X509_subject_name_cmp((*a)->data.x509, (*b)->data.x509);
        break;
    case X509_LU_CRL:
        ret = X509_CRL_cmp((*a)->data.crl, (*b)->data.crl);
        break;
    case X509_LU_NONE:
        /* abort(); */
        return 0;
    }
    return ret;
}

X509_STORE *X509_STORE_new(void)
{
    X509_STORE *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;
    if ((ret->objs = sk_X509_OBJECT_new(x509_object_cmp)) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }
    ret->cache = 1;
    if ((ret->get_cert_methods = sk_X509_LOOKUP_new_null()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if ((ret->param = X509_VERIFY_PARAM_new()) == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        goto err;
    }
    if (!CRYPTO_new_ex_data(CRYPTO_EX_INDEX_X509_STORE, ret, &ret->ex_data)) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }

    ret->lock = CRYPTO_THREAD_lock_new();
    if (ret->lock == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
        goto err;
    }

    if (!CRYPTO_NEW_REF(&ret->references, 1))
        goto err;
    return ret;

err:
    X509_VERIFY_PARAM_free(ret->param);
    sk_X509_OBJECT_free(ret->objs);
    sk_X509_LOOKUP_free(ret->get_cert_methods);
    CRYPTO_THREAD_lock_free(ret->lock);
    OPENSSL_free(ret);
    return NULL;
}

void X509_STORE_free(X509_STORE *xs)
{
    int i;
    STACK_OF(X509_LOOKUP) *sk;
    X509_LOOKUP *lu;

    if (xs == NULL)
        return;
    CRYPTO_DOWN_REF(&xs->references, &i);
    REF_PRINT_COUNT("X509_STORE", i, xs);
    if (i > 0)
        return;
    REF_ASSERT_ISNT(i < 0);

    sk = xs->get_cert_methods;
    for (i = 0; i < sk_X509_LOOKUP_num(sk); i++) {
        lu = sk_X509_LOOKUP_value(sk, i);
        X509_LOOKUP_shutdown(lu);
        X509_LOOKUP_free(lu);
    }
    sk_X509_LOOKUP_free(sk);
    sk_X509_OBJECT_pop_free(xs->objs, X509_OBJECT_free);

    CRYPTO_free_ex_data(CRYPTO_EX_INDEX_X509_STORE, xs, &xs->ex_data);
    X509_VERIFY_PARAM_free(xs->param);
    CRYPTO_THREAD_lock_free(xs->lock);
    CRYPTO_FREE_REF(&xs->references);
    OPENSSL_free(xs);
}

int X509_STORE_up_ref(X509_STORE *xs)
{
    int i;

    if (CRYPTO_UP_REF(&xs->references, &i) <= 0)
        return 0;

    REF_PRINT_COUNT("X509_STORE", i, xs);
    REF_ASSERT_ISNT(i < 2);
    return i > 1 ? 1 : 0;
}

X509_LOOKUP *X509_STORE_add_lookup(X509_STORE *xs, X509_LOOKUP_METHOD *m)
{
    int i;
    STACK_OF(X509_LOOKUP) *sk;
    X509_LOOKUP *lu;

    sk = xs->get_cert_methods;
    for (i = 0; i < sk_X509_LOOKUP_num(sk); i++) {
        lu = sk_X509_LOOKUP_value(sk, i);
        if (m == lu->method) {
            return lu;
        }
    }
    /* a new one */
    lu = X509_LOOKUP_new(m);
    if (lu == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        return NULL;
    }

    lu->store_ctx = xs;
    if (sk_X509_LOOKUP_push(xs->get_cert_methods, lu))
        return lu;
    /* sk_X509_LOOKUP_push() failed */
    ERR_raise(ERR_LIB_X509, ERR_R_CRYPTO_LIB);
    X509_LOOKUP_free(lu);
    return NULL;
}

/* Also fill the cache (ctx->store->objs) with all matching certificates. */
X509_OBJECT *X509_STORE_CTX_get_obj_by_subject(X509_STORE_CTX *ctx,
                                               X509_LOOKUP_TYPE type,
                                               const X509_NAME *name)
{
    X509_OBJECT *ret = X509_OBJECT_new();

    if (ret == NULL)
        return NULL;
    if (!X509_STORE_CTX_get_by_subject(ctx, type, name, ret)) {
        X509_OBJECT_free(ret);
        return NULL;
    }
    return ret;
}

/*
 * May be called with |ret| == NULL just for the side effect of
 * caching all certs matching the given subject DN in |ctx->store->objs|.
 * Returns 1 if successful,
 * 0 if not found or X509_LOOKUP_by_subject_ex() returns an error,
 * -1 on failure
 */
int ossl_x509_store_ctx_get_by_subject(const X509_STORE_CTX *ctx, X509_LOOKUP_TYPE type,
                                       const X509_NAME *name, X509_OBJECT *ret)
{
    X509_STORE *store = ctx->store;
    X509_LOOKUP *lu;
    X509_OBJECT stmp, *tmp;
    int i, j;

    if (store == NULL)
        return 0;

    stmp.type = X509_LU_NONE;
    stmp.data.x509 = NULL;

    if (!x509_store_read_lock(store))
        return 0;
    /* Should already be sorted...but just in case */
    if (!sk_X509_OBJECT_is_sorted(store->objs)) {
        X509_STORE_unlock(store);
        /* Take a write lock instead of a read lock */
        if (!X509_STORE_lock(store))
            return 0;
        /*
         * Another thread might have sorted it in the meantime. But if so,
         * sk_X509_OBJECT_sort() exits early.
         */
        sk_X509_OBJECT_sort(store->objs);
    }
    tmp = X509_OBJECT_retrieve_by_subject(store->objs, type, name);
    X509_STORE_unlock(store);

    if (tmp == NULL || type == X509_LU_CRL) {
        for (i = 0; i < sk_X509_LOOKUP_num(store->get_cert_methods); i++) {
            lu = sk_X509_LOOKUP_value(store->get_cert_methods, i);
            if (lu->skip)
                continue;
            if (lu->method == NULL)
                return -1;
            j = X509_LOOKUP_by_subject_ex(lu, type, name, &stmp,
                                          ctx->libctx, ctx->propq);
            if (j != 0) { /* non-zero value is considered success here */
                tmp = &stmp;
                break;
            }
        }
        if (tmp == NULL)
            return 0;
    }

    if (ret != NULL) {
        if (!X509_OBJECT_up_ref_count(tmp))
            return -1;
        ret->type = tmp->type;
        ret->data = tmp->data;
    }
    return 1;
}

/* Also fill the cache |ctx->store->objs| with all matching certificates. */
int X509_STORE_CTX_get_by_subject(const X509_STORE_CTX *ctx,
                                  X509_LOOKUP_TYPE type,
                                  const X509_NAME *name, X509_OBJECT *ret)
{
    return ossl_x509_store_ctx_get_by_subject(ctx, type, name, ret) > 0;
}

static int x509_store_add(X509_STORE *store, void *x, int crl)
{
    X509_OBJECT *obj;
    int ret = 0, added = 0;

    if (x == NULL)
        return 0;
    obj = X509_OBJECT_new();
    if (obj == NULL)
        return 0;

    if (crl) {
        obj->type = X509_LU_CRL;
        obj->data.crl = (X509_CRL *)x;
    } else {
        obj->type = X509_LU_X509;
        obj->data.x509 = (X509 *)x;
    }
    if (!X509_OBJECT_up_ref_count(obj)) {
        obj->type = X509_LU_NONE;
        X509_OBJECT_free(obj);
        return 0;
    }

    if (!X509_STORE_lock(store)) {
        obj->type = X509_LU_NONE;
        X509_OBJECT_free(obj);
        return 0;
    }

    if (X509_OBJECT_retrieve_match(store->objs, obj)) {
        ret = 1;
    } else {
        added = sk_X509_OBJECT_push(store->objs, obj);
        ret = added != 0;
    }
    X509_STORE_unlock(store);

    if (added == 0)             /* obj not pushed */
        X509_OBJECT_free(obj);

    return ret;
}

int X509_STORE_add_cert(X509_STORE *xs, X509 *x)
{
    if (!x509_store_add(xs, x, 0)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        return 0;
    }
    return 1;
}

int X509_STORE_add_crl(X509_STORE *xs, X509_CRL *x)
{
    if (!x509_store_add(xs, x, 1)) {
        ERR_raise(ERR_LIB_X509, ERR_R_X509_LIB);
        return 0;
    }
    return 1;
}

int X509_OBJECT_up_ref_count(X509_OBJECT *a)
{
    switch (a->type) {
    case X509_LU_NONE:
        break;
    case X509_LU_X509:
        return X509_up_ref(a->data.x509);
    case X509_LU_CRL:
        return X509_CRL_up_ref(a->data.crl);
    }
    return 1;
}

X509 *X509_OBJECT_get0_X509(const X509_OBJECT *a)
{
    if (a == NULL || a->type != X509_LU_X509)
        return NULL;
    return a->data.x509;
}

X509_CRL *X509_OBJECT_get0_X509_CRL(const X509_OBJECT *a)
{
    if (a == NULL || a->type != X509_LU_CRL)
        return NULL;
    return a->data.crl;
}

X509_LOOKUP_TYPE X509_OBJECT_get_type(const X509_OBJECT *a)
{
    return a->type;
}

X509_OBJECT *X509_OBJECT_new(void)
{
    X509_OBJECT *ret = OPENSSL_zalloc(sizeof(*ret));

    if (ret == NULL)
        return NULL;
    ret->type = X509_LU_NONE;
    return ret;
}

static void x509_object_free_internal(X509_OBJECT *a)
{
    if (a == NULL)
        return;
    switch (a->type) {
    case X509_LU_NONE:
        break;
    case X509_LU_X509:
        X509_free(a->data.x509);
        break;
    case X509_LU_CRL:
        X509_CRL_free(a->data.crl);
        break;
    }
}

int X509_OBJECT_set1_X509(X509_OBJECT *a, X509 *obj)
{
    if (a == NULL || !X509_up_ref(obj))
        return 0;

    x509_object_free_internal(a);
    a->type = X509_LU_X509;
    a->data.x509 = obj;
    return 1;
}

int X509_OBJECT_set1_X509_CRL(X509_OBJECT *a, X509_CRL *obj)
{
    if (a == NULL || !X509_CRL_up_ref(obj))
        return 0;

    x509_object_free_internal(a);
    a->type = X509_LU_CRL;
    a->data.crl = obj;
    return 1;
}

void X509_OBJECT_free(X509_OBJECT *a)
{
    x509_object_free_internal(a);
    OPENSSL_free(a);
}

/* Returns -1 if not found, but also on error */
static int x509_object_idx_cnt(STACK_OF(X509_OBJECT) *h, X509_LOOKUP_TYPE type,
                               const X509_NAME *name, int *pnmatch)
{
    X509_OBJECT stmp;
    X509 x509_s;
    X509_CRL crl_s;

    stmp.type = type;
    switch (type) {
    case X509_LU_X509:
        stmp.data.x509 = &x509_s;
        x509_s.cert_info.subject = (X509_NAME *)name; /* won't modify it */
        break;
    case X509_LU_CRL:
        stmp.data.crl = &crl_s;
        crl_s.crl.issuer = (X509_NAME *)name; /* won't modify it */
        break;
    case X509_LU_NONE:
    default:
        /* abort(); */
        return -1;
    }

    /* Assumes h is locked for read if applicable */
    return sk_X509_OBJECT_find_all(h, &stmp, pnmatch);
}

/* Assumes h is locked for read if applicable */
int X509_OBJECT_idx_by_subject(STACK_OF(X509_OBJECT) *h, X509_LOOKUP_TYPE type,
                               const X509_NAME *name)
{
    return x509_object_idx_cnt(h, type, name, NULL);
}

/* Assumes h is locked for read if applicable */
X509_OBJECT *X509_OBJECT_retrieve_by_subject(STACK_OF(X509_OBJECT) *h,
                                             X509_LOOKUP_TYPE type,
                                             const X509_NAME *name)
{
    int idx = X509_OBJECT_idx_by_subject(h, type, name);

    if (idx == -1)
        return NULL;
    return sk_X509_OBJECT_value(h, idx);
}

STACK_OF(X509_OBJECT) *X509_STORE_get0_objects(const X509_STORE *xs)
{
    return xs->objs;
}

static X509_OBJECT *x509_object_dup(const X509_OBJECT *obj)
{
    X509_OBJECT *ret = X509_OBJECT_new();
    if (ret == NULL)
        return NULL;

    ret->type = obj->type;
    ret->data = obj->data;
    X509_OBJECT_up_ref_count(ret);
    return ret;
}

STACK_OF(X509_OBJECT) *X509_STORE_get1_objects(X509_STORE *store)
{
    STACK_OF(X509_OBJECT) *objs;

    if (store == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    if (!x509_store_read_lock(store))
        return NULL;

    objs = sk_X509_OBJECT_deep_copy(store->objs, x509_object_dup,
                                    X509_OBJECT_free);
    X509_STORE_unlock(store);
    return objs;
}

STACK_OF(X509) *X509_STORE_get1_all_certs(X509_STORE *store)
{
    STACK_OF(X509) *sk;
    STACK_OF(X509_OBJECT) *objs;
    int i;

    if (store == NULL) {
        ERR_raise(ERR_LIB_X509, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }
    if ((sk = sk_X509_new_null()) == NULL)
        return NULL;
    if (!X509_STORE_lock(store))
        goto out_free;

    sk_X509_OBJECT_sort(store->objs);
    objs = X509_STORE_get0_objects(store);
    for (i = 0; i < sk_X509_OBJECT_num(objs); i++) {
        X509 *cert = X509_OBJECT_get0_X509(sk_X509_OBJECT_value(objs, i));

        if (cert != NULL
            && !X509_add_cert(sk, cert, X509_ADD_FLAG_UP_REF))
            goto err;
    }
    X509_STORE_unlock(store);
    return sk;

 err:
    X509_STORE_unlock(store);
 out_free:
    OSSL_STACK_OF_X509_free(sk);
    return NULL;
}

/*-
 * Collect from |ctx->store| all certs with subject matching |nm|.
 * Returns NULL on internal/fatal error, empty stack if not found.
 */
STACK_OF(X509) *X509_STORE_CTX_get1_certs(X509_STORE_CTX *ctx,
                                          const X509_NAME *nm)
{
    int i, idx, cnt;
    STACK_OF(X509) *sk = NULL;
    X509 *x;
    X509_OBJECT *obj;
    X509_STORE *store = ctx->store;

    if (store == NULL)
        return sk_X509_new_null();

    if (!X509_STORE_lock(store))
        return NULL;

    sk_X509_OBJECT_sort(store->objs);
    idx = x509_object_idx_cnt(store->objs, X509_LU_X509, nm, &cnt);
    if (idx < 0) {
        /*
         * Nothing found in cache: do lookup to possibly add new objects to
         * cache
         */
        X509_STORE_unlock(store);
        i = ossl_x509_store_ctx_get_by_subject(ctx, X509_LU_X509, nm, NULL);
        if (i <= 0)
            return i < 0 ? NULL : sk_X509_new_null();
        if (!X509_STORE_lock(store))
            return NULL;
        sk_X509_OBJECT_sort(store->objs);
        idx = x509_object_idx_cnt(store->objs, X509_LU_X509, nm, &cnt);
    }

    sk = sk_X509_new_null();
    if (idx < 0 || sk == NULL)
        goto end;
    for (i = 0; i < cnt; i++, idx++) {
        obj = sk_X509_OBJECT_value(store->objs, idx);
        x = obj->data.x509;
        if (!X509_add_cert(sk, x, X509_ADD_FLAG_UP_REF)) {
            X509_STORE_unlock(store);
            OSSL_STACK_OF_X509_free(sk);
            return NULL;
        }
    }
 end:
    X509_STORE_unlock(store);
    return sk;
}

/* Returns NULL on internal/fatal error, empty stack if not found */
STACK_OF(X509_CRL) *X509_STORE_CTX_get1_crls(const X509_STORE_CTX *ctx,
                                             const X509_NAME *nm)
{
    int i = 1, idx, cnt;
    STACK_OF(X509_CRL) *sk;
    X509_CRL *x;
    X509_OBJECT *obj;
    X509_STORE *store = ctx->store;

    /* Always do lookup to possibly add new CRLs to cache */
    i = ossl_x509_store_ctx_get_by_subject(ctx, X509_LU_CRL, nm, NULL);
    if (i < 0)
        return NULL;
    sk = sk_X509_CRL_new_null();
    if (i == 0)
        return sk;
    if (!X509_STORE_lock(store)) {
        sk_X509_CRL_free(sk);
        return NULL;
    }
    sk_X509_OBJECT_sort(store->objs);
    idx = x509_object_idx_cnt(store->objs, X509_LU_CRL, nm, &cnt);
    if (idx < 0) {
        X509_STORE_unlock(store);
        return sk;
    }

    for (i = 0; i < cnt; i++, idx++) {
        obj = sk_X509_OBJECT_value(store->objs, idx);
        x = obj->data.crl;
        if (!X509_CRL_up_ref(x)) {
            X509_STORE_unlock(store);
            sk_X509_CRL_pop_free(sk, X509_CRL_free);
            return NULL;
        }
        if (!sk_X509_CRL_push(sk, x)) {
            X509_STORE_unlock(store);
            X509_CRL_free(x);
            sk_X509_CRL_pop_free(sk, X509_CRL_free);
            return NULL;
        }
    }
    X509_STORE_unlock(store);
    return sk;
}

X509_OBJECT *X509_OBJECT_retrieve_match(STACK_OF(X509_OBJECT) *h,
                                        X509_OBJECT *x)
{
    int idx, i, num;
    X509_OBJECT *obj;

    idx = sk_X509_OBJECT_find(h, x);
    if (idx < 0)
        return NULL;
    if ((x->type != X509_LU_X509) && (x->type != X509_LU_CRL))
        return sk_X509_OBJECT_value(h, idx);
    for (i = idx, num = sk_X509_OBJECT_num(h); i < num; i++) {
        obj = sk_X509_OBJECT_value(h, i);
        if (x509_object_cmp((const X509_OBJECT **)&obj,
                            (const X509_OBJECT **)&x))
            return NULL;
        if (x->type == X509_LU_X509) {
            if (!X509_cmp(obj->data.x509, x->data.x509))
                return obj;
        } else if (x->type == X509_LU_CRL) {
            if (X509_CRL_match(obj->data.crl, x->data.crl) == 0)
                return obj;
        } else {
            return obj;
        }
    }
    return NULL;
}

int X509_STORE_set_flags(X509_STORE *xs, unsigned long flags)
{
    return X509_VERIFY_PARAM_set_flags(xs->param, flags);
}

int X509_STORE_set_depth(X509_STORE *xs, int depth)
{
    X509_VERIFY_PARAM_set_depth(xs->param, depth);
    return 1;
}

int X509_STORE_set_purpose(X509_STORE *xs, int purpose)
{
    return X509_VERIFY_PARAM_set_purpose(xs->param, purpose);
}

int X509_STORE_set_trust(X509_STORE *xs, int trust)
{
    return X509_VERIFY_PARAM_set_trust(xs->param, trust);
}

int X509_STORE_set1_param(X509_STORE *xs, const X509_VERIFY_PARAM *param)
{
    return X509_VERIFY_PARAM_set1(xs->param, param);
}

X509_VERIFY_PARAM *X509_STORE_get0_param(const X509_STORE *xs)
{
    return xs->param;
}

void X509_STORE_set_verify(X509_STORE *xs, X509_STORE_CTX_verify_fn verify)
{
    xs->verify = verify;
}

X509_STORE_CTX_verify_fn X509_STORE_get_verify(const X509_STORE *xs)
{
    return xs->verify;
}

void X509_STORE_set_verify_cb(X509_STORE *xs,
                              X509_STORE_CTX_verify_cb verify_cb)
{
    xs->verify_cb = verify_cb;
}

X509_STORE_CTX_verify_cb X509_STORE_get_verify_cb(const X509_STORE *xs)
{
    return xs->verify_cb;
}

void X509_STORE_set_get_issuer(X509_STORE *xs,
                               X509_STORE_CTX_get_issuer_fn get_issuer)
{
    xs->get_issuer = get_issuer;
}

X509_STORE_CTX_get_issuer_fn X509_STORE_get_get_issuer(const X509_STORE *xs)
{
    return xs->get_issuer;
}

void X509_STORE_set_check_issued(X509_STORE *xs,
                                 X509_STORE_CTX_check_issued_fn check_issued)
{
    xs->check_issued = check_issued;
}

X509_STORE_CTX_check_issued_fn X509_STORE_get_check_issued(const X509_STORE *xs)
{
    return xs->check_issued;
}

void X509_STORE_set_check_revocation(X509_STORE *xs,
                                     X509_STORE_CTX_check_revocation_fn cb)
{
    xs->check_revocation = cb;
}

X509_STORE_CTX_check_revocation_fn X509_STORE_get_check_revocation(const X509_STORE *xs)
{
    return xs->check_revocation;
}

void X509_STORE_set_get_crl(X509_STORE *xs,
                            X509_STORE_CTX_get_crl_fn get_crl)
{
    xs->get_crl = get_crl;
}

X509_STORE_CTX_get_crl_fn X509_STORE_get_get_crl(const X509_STORE *xs)
{
    return xs->get_crl;
}

void X509_STORE_set_check_crl(X509_STORE *xs,
                              X509_STORE_CTX_check_crl_fn check_crl)
{
    xs->check_crl = check_crl;
}

X509_STORE_CTX_check_crl_fn X509_STORE_get_check_crl(const X509_STORE *xs)
{
    return xs->check_crl;
}

void X509_STORE_set_cert_crl(X509_STORE *xs,
                             X509_STORE_CTX_cert_crl_fn cert_crl)
{
    xs->cert_crl = cert_crl;
}

X509_STORE_CTX_cert_crl_fn X509_STORE_get_cert_crl(const X509_STORE *xs)
{
    return xs->cert_crl;
}

void X509_STORE_set_check_policy(X509_STORE *xs,
                                 X509_STORE_CTX_check_policy_fn check_policy)
{
    xs->check_policy = check_policy;
}

X509_STORE_CTX_check_policy_fn X509_STORE_get_check_policy(const X509_STORE *xs)
{
    return xs->check_policy;
}

void X509_STORE_set_lookup_certs(X509_STORE *xs,
                                 X509_STORE_CTX_lookup_certs_fn lookup_certs)
{
    xs->lookup_certs = lookup_certs;
}

X509_STORE_CTX_lookup_certs_fn X509_STORE_get_lookup_certs(const X509_STORE *xs)
{
    return xs->lookup_certs;
}

void X509_STORE_set_lookup_crls(X509_STORE *xs,
                                X509_STORE_CTX_lookup_crls_fn lookup_crls)
{
    xs->lookup_crls = lookup_crls;
}

X509_STORE_CTX_lookup_crls_fn X509_STORE_get_lookup_crls(const X509_STORE *xs)
{
    return xs->lookup_crls;
}

void X509_STORE_set_cleanup(X509_STORE *xs,
                            X509_STORE_CTX_cleanup_fn cleanup)
{
    xs->cleanup = cleanup;
}

X509_STORE_CTX_cleanup_fn X509_STORE_get_cleanup(const X509_STORE *xs)
{
    return xs->cleanup;
}

int X509_STORE_set_ex_data(X509_STORE *xs, int idx, void *data)
{
    return CRYPTO_set_ex_data(&xs->ex_data, idx, data);
}

void *X509_STORE_get_ex_data(const X509_STORE *xs, int idx)
{
    return CRYPTO_get_ex_data(&xs->ex_data, idx);
}

X509_STORE *X509_STORE_CTX_get0_store(const X509_STORE_CTX *ctx)
{
    return ctx->store;
}
