/*
 * Copyright 1998-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <openssl/err.h>
#include <openssl/lhash.h>
#include <openssl/objects.h>
#include <openssl/safestack.h>
#include <openssl/e_os2.h>
#include "internal/thread_once.h"
#include "crypto/lhash.h"
#include "obj_local.h"
#include "internal/e_os.h"

/*
 * I use the ex_data stuff to manage the identifiers for the obj_name_types
 * that applications may define.  I only really use the free function field.
 */
static LHASH_OF(OBJ_NAME) *names_lh = NULL;
static int names_type_num = OBJ_NAME_TYPE_NUM;
static CRYPTO_RWLOCK *obj_lock = NULL;

struct name_funcs_st {
    unsigned long (*hash_func) (const char *name);
    int (*cmp_func) (const char *a, const char *b);
    void (*free_func) (const char *, int, const char *);
};

static STACK_OF(NAME_FUNCS) *name_funcs_stack;

/*
 * The LHASH callbacks now use the raw "void *" prototypes and do
 * per-variable casting in the functions. This prevents function pointer
 * casting without the need for macro-generated wrapper functions.
 */

static unsigned long obj_name_hash(const OBJ_NAME *a);
static int obj_name_cmp(const OBJ_NAME *a, const OBJ_NAME *b);

static CRYPTO_ONCE init = CRYPTO_ONCE_STATIC_INIT;
DEFINE_RUN_ONCE_STATIC(o_names_init)
{
    names_lh = NULL;
    obj_lock = CRYPTO_THREAD_lock_new();
    if (obj_lock != NULL)
        names_lh = lh_OBJ_NAME_new(obj_name_hash, obj_name_cmp);
    if (names_lh == NULL) {
        CRYPTO_THREAD_lock_free(obj_lock);
        obj_lock = NULL;
    }
    return names_lh != NULL && obj_lock != NULL;
}

int OBJ_NAME_init(void)
{
    return RUN_ONCE(&init, o_names_init);
}

int OBJ_NAME_new_index(unsigned long (*hash_func) (const char *),
                       int (*cmp_func) (const char *, const char *),
                       void (*free_func) (const char *, int, const char *))
{
    int ret = 0, i, push;
    NAME_FUNCS *name_funcs;

    if (!OBJ_NAME_init())
        return 0;

    if (!CRYPTO_THREAD_write_lock(obj_lock))
        return 0;

    if (name_funcs_stack == NULL)
        name_funcs_stack = sk_NAME_FUNCS_new_null();
    if (name_funcs_stack == NULL) {
        /* ERROR */
        goto out;
    }
    ret = names_type_num;
    names_type_num++;
    for (i = sk_NAME_FUNCS_num(name_funcs_stack); i < names_type_num; i++) {
        name_funcs = OPENSSL_zalloc(sizeof(*name_funcs));
        if (name_funcs == NULL) {
            ret = 0;
            goto out;
        }
        name_funcs->hash_func = ossl_lh_strcasehash;
        name_funcs->cmp_func = OPENSSL_strcasecmp;
        push = sk_NAME_FUNCS_push(name_funcs_stack, name_funcs);

        if (!push) {
            ERR_raise(ERR_LIB_OBJ, ERR_R_CRYPTO_LIB);
            OPENSSL_free(name_funcs);
            ret = 0;
            goto out;
        }
    }
    name_funcs = sk_NAME_FUNCS_value(name_funcs_stack, ret);
    if (hash_func != NULL)
        name_funcs->hash_func = hash_func;
    if (cmp_func != NULL)
        name_funcs->cmp_func = cmp_func;
    if (free_func != NULL)
        name_funcs->free_func = free_func;

out:
    CRYPTO_THREAD_unlock(obj_lock);
    return ret;
}

static int obj_name_cmp(const OBJ_NAME *a, const OBJ_NAME *b)
{
    int ret;

    ret = a->type - b->type;
    if (ret == 0) {
        if ((name_funcs_stack != NULL)
            && (sk_NAME_FUNCS_num(name_funcs_stack) > a->type)) {
            ret = sk_NAME_FUNCS_value(name_funcs_stack,
                                      a->type)->cmp_func(a->name, b->name);
        } else
            ret = OPENSSL_strcasecmp(a->name, b->name);
    }
    return ret;
}

static unsigned long obj_name_hash(const OBJ_NAME *a)
{
    unsigned long ret;

    if ((name_funcs_stack != NULL)
        && (sk_NAME_FUNCS_num(name_funcs_stack) > a->type)) {
        ret =
            sk_NAME_FUNCS_value(name_funcs_stack,
                                a->type)->hash_func(a->name);
    } else {
        ret = ossl_lh_strcasehash(a->name);
    }
    ret ^= a->type;
    return ret;
}

const char *OBJ_NAME_get(const char *name, int type)
{
    OBJ_NAME on, *ret;
    int num = 0, alias;
    const char *value = NULL;

    if (name == NULL)
        return NULL;
    if (!OBJ_NAME_init())
        return NULL;
    if (!CRYPTO_THREAD_read_lock(obj_lock))
        return NULL;

    alias = type & OBJ_NAME_ALIAS;
    type &= ~OBJ_NAME_ALIAS;

    on.name = name;
    on.type = type;

    for (;;) {
        ret = lh_OBJ_NAME_retrieve(names_lh, &on);
        if (ret == NULL)
            break;
        if ((ret->alias) && !alias) {
            if (++num > 10)
                break;
            on.name = ret->data;
        } else {
            value = ret->data;
            break;
        }
    }

    CRYPTO_THREAD_unlock(obj_lock);
    return value;
}

int OBJ_NAME_add(const char *name, int type, const char *data)
{
    OBJ_NAME *onp, *ret;
    int alias, ok = 0;

    if (!OBJ_NAME_init())
        return 0;

    alias = type & OBJ_NAME_ALIAS;
    type &= ~OBJ_NAME_ALIAS;

    onp = OPENSSL_malloc(sizeof(*onp));
    if (onp == NULL)
        return 0;

    onp->name = name;
    onp->alias = alias;
    onp->type = type;
    onp->data = data;

    if (!CRYPTO_THREAD_write_lock(obj_lock)) {
        OPENSSL_free(onp);
        return 0;
    }

    ret = lh_OBJ_NAME_insert(names_lh, onp);
    if (ret != NULL) {
        /* free things */
        if ((name_funcs_stack != NULL)
            && (sk_NAME_FUNCS_num(name_funcs_stack) > ret->type)) {
            /*
             * XXX: I'm not sure I understand why the free function should
             * get three arguments... -- Richard Levitte
             */
            sk_NAME_FUNCS_value(name_funcs_stack,
                                ret->type)->free_func(ret->name, ret->type,
                                                      ret->data);
        }
        OPENSSL_free(ret);
    } else {
        if (lh_OBJ_NAME_error(names_lh)) {
            /* ERROR */
            OPENSSL_free(onp);
            goto unlock;
        }
    }

    ok = 1;

unlock:
    CRYPTO_THREAD_unlock(obj_lock);
    return ok;
}

int OBJ_NAME_remove(const char *name, int type)
{
    OBJ_NAME on, *ret;
    int ok = 0;

    if (!OBJ_NAME_init())
        return 0;

    if (!CRYPTO_THREAD_write_lock(obj_lock))
        return 0;

    type &= ~OBJ_NAME_ALIAS;
    on.name = name;
    on.type = type;
    ret = lh_OBJ_NAME_delete(names_lh, &on);
    if (ret != NULL) {
        /* free things */
        if ((name_funcs_stack != NULL)
            && (sk_NAME_FUNCS_num(name_funcs_stack) > ret->type)) {
            /*
             * XXX: I'm not sure I understand why the free function should
             * get three arguments... -- Richard Levitte
             */
            sk_NAME_FUNCS_value(name_funcs_stack,
                                ret->type)->free_func(ret->name, ret->type,
                                                      ret->data);
        }
        OPENSSL_free(ret);
        ok = 1;
    }

    CRYPTO_THREAD_unlock(obj_lock);
    return ok;
}

typedef struct {
    int type;
    void (*fn) (const OBJ_NAME *, void *arg);
    void *arg;
} OBJ_DOALL;

static void do_all_fn(const OBJ_NAME *name, OBJ_DOALL *d)
{
    if (name->type == d->type)
        d->fn(name, d->arg);
}

IMPLEMENT_LHASH_DOALL_ARG_CONST(OBJ_NAME, OBJ_DOALL);

void OBJ_NAME_do_all(int type, void (*fn) (const OBJ_NAME *, void *arg),
                     void *arg)
{
    OBJ_DOALL d;

    d.type = type;
    d.fn = fn;
    d.arg = arg;

    lh_OBJ_NAME_doall_OBJ_DOALL(names_lh, do_all_fn, &d);
}

struct doall_sorted {
    int type;
    int n;
    const OBJ_NAME **names;
};

static void do_all_sorted_fn(const OBJ_NAME *name, void *d_)
{
    struct doall_sorted *d = d_;

    if (name->type != d->type)
        return;

    d->names[d->n++] = name;
}

static int do_all_sorted_cmp(const void *n1_, const void *n2_)
{
    const OBJ_NAME *const *n1 = n1_;
    const OBJ_NAME *const *n2 = n2_;

    return strcmp((*n1)->name, (*n2)->name);
}

void OBJ_NAME_do_all_sorted(int type,
                            void (*fn) (const OBJ_NAME *, void *arg),
                            void *arg)
{
    struct doall_sorted d;
    int n;

    d.type = type;
    d.names =
        OPENSSL_malloc(sizeof(*d.names) * lh_OBJ_NAME_num_items(names_lh));
    /* Really should return an error if !d.names...but its a void function! */
    if (d.names != NULL) {
        d.n = 0;
        OBJ_NAME_do_all(type, do_all_sorted_fn, &d);

        qsort((void *)d.names, d.n, sizeof(*d.names), do_all_sorted_cmp);

        for (n = 0; n < d.n; ++n)
            fn(d.names[n], arg);

        OPENSSL_free((void *)d.names);
    }
}

static int free_type;

static void names_lh_free_doall(OBJ_NAME *onp)
{
    if (onp == NULL)
        return;

    if (free_type < 0 || free_type == onp->type)
        OBJ_NAME_remove(onp->name, onp->type);
}

static void name_funcs_free(NAME_FUNCS *ptr)
{
    OPENSSL_free(ptr);
}

void OBJ_NAME_cleanup(int type)
{
    unsigned long down_load;

    if (names_lh == NULL)
        return;

    free_type = type;
    down_load = lh_OBJ_NAME_get_down_load(names_lh);
    lh_OBJ_NAME_set_down_load(names_lh, 0);

    lh_OBJ_NAME_doall(names_lh, names_lh_free_doall);
    if (type < 0) {
        lh_OBJ_NAME_free(names_lh);
        sk_NAME_FUNCS_pop_free(name_funcs_stack, name_funcs_free);
        CRYPTO_THREAD_lock_free(obj_lock);
        names_lh = NULL;
        name_funcs_stack = NULL;
        obj_lock = NULL;
    } else
        lh_OBJ_NAME_set_down_load(names_lh, down_load);
}
