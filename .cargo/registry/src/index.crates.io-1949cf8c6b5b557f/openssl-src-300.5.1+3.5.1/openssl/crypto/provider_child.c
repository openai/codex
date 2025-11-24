/*
 * Copyright 2019-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <assert.h>
#include <openssl/crypto.h>
#include <openssl/core_dispatch.h>
#include <openssl/core_names.h>
#include <openssl/provider.h>
#include <openssl/evp.h>
#include "internal/provider.h"
#include "internal/cryptlib.h"
#include "crypto/evp.h"
#include "crypto/context.h"

DEFINE_STACK_OF(OSSL_PROVIDER)

struct child_prov_globals {
    const OSSL_CORE_HANDLE *handle;
    const OSSL_CORE_HANDLE *curr_prov;
    CRYPTO_RWLOCK *lock;
    OSSL_FUNC_core_get_libctx_fn *c_get_libctx;
    OSSL_FUNC_provider_register_child_cb_fn *c_provider_register_child_cb;
    OSSL_FUNC_provider_deregister_child_cb_fn *c_provider_deregister_child_cb;
    OSSL_FUNC_provider_name_fn *c_prov_name;
    OSSL_FUNC_provider_get0_provider_ctx_fn *c_prov_get0_provider_ctx;
    OSSL_FUNC_provider_get0_dispatch_fn *c_prov_get0_dispatch;
    OSSL_FUNC_provider_up_ref_fn *c_prov_up_ref;
    OSSL_FUNC_provider_free_fn *c_prov_free;
};

void *ossl_child_prov_ctx_new(OSSL_LIB_CTX *libctx)
{
    return OPENSSL_zalloc(sizeof(struct child_prov_globals));
}

void ossl_child_prov_ctx_free(void *vgbl)
{
    struct child_prov_globals *gbl = vgbl;

    CRYPTO_THREAD_lock_free(gbl->lock);
    OPENSSL_free(gbl);
}

static OSSL_provider_init_fn ossl_child_provider_init;

static int ossl_child_provider_init(const OSSL_CORE_HANDLE *handle,
                                    const OSSL_DISPATCH *in,
                                    const OSSL_DISPATCH **out,
                                    void **provctx)
{
    OSSL_FUNC_core_get_libctx_fn *c_get_libctx = NULL;
    OSSL_LIB_CTX *ctx;
    struct child_prov_globals *gbl;

    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GET_LIBCTX:
            c_get_libctx = OSSL_FUNC_core_get_libctx(in);
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    if (c_get_libctx == NULL)
        return 0;

    /*
     * We need an OSSL_LIB_CTX but c_get_libctx returns OPENSSL_CORE_CTX. We are
     * a built-in provider and so we can get away with this cast. Normal
     * providers can't do this.
     */
    ctx = (OSSL_LIB_CTX *)c_get_libctx(handle);

    gbl = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    *provctx = gbl->c_prov_get0_provider_ctx(gbl->curr_prov);
    *out = gbl->c_prov_get0_dispatch(gbl->curr_prov);

    return 1;
}

static int provider_create_child_cb(const OSSL_CORE_HANDLE *prov, void *cbdata)
{
    OSSL_LIB_CTX *ctx = cbdata;
    struct child_prov_globals *gbl;
    const char *provname;
    OSSL_PROVIDER *cprov;
    int ret = 0;

    gbl = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    if (!CRYPTO_THREAD_write_lock(gbl->lock))
        return 0;

    provname = gbl->c_prov_name(prov);

    /*
     * We're operating under a lock so we can store the "current" provider in
     * the global data.
     */
    gbl->curr_prov = prov;

    if ((cprov = ossl_provider_find(ctx, provname, 1)) != NULL) {
        /*
         * We free the newly created ref. We rely on the provider sticking around
         * in the provider store.
         */
        ossl_provider_free(cprov);

        /*
         * The provider already exists. It could be a previously created child,
         * or it could have been explicitly loaded. If explicitly loaded we
         * ignore it - i.e. we don't start treating it like a child.
         */
        if (!ossl_provider_activate(cprov, 0, 1))
            goto err;
    } else {
        /*
         * Create it - passing 1 as final param so we don't try and recursively
         * init children
         */
        if ((cprov = ossl_provider_new(ctx, provname, ossl_child_provider_init,
                                       NULL, 1)) == NULL)
            goto err;

        if (!ossl_provider_activate(cprov, 0, 0)) {
            ossl_provider_free(cprov);
            goto err;
        }

        if (!ossl_provider_set_child(cprov, prov)
            || !ossl_provider_add_to_store(cprov, NULL, 0)) {
            ossl_provider_deactivate(cprov, 0);
            ossl_provider_free(cprov);
            goto err;
        }
    }

    ret = 1;
 err:
    CRYPTO_THREAD_unlock(gbl->lock);
    return ret;
}

static int provider_remove_child_cb(const OSSL_CORE_HANDLE *prov, void *cbdata)
{
    OSSL_LIB_CTX *ctx = cbdata;
    struct child_prov_globals *gbl;
    const char *provname;
    OSSL_PROVIDER *cprov;

    gbl = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    provname = gbl->c_prov_name(prov);
    cprov = ossl_provider_find(ctx, provname, 1);
    if (cprov == NULL)
        return 0;
    /*
     * ossl_provider_find ups the ref count, so we free it again here. We can
     * rely on the provider store reference count.
     */
    ossl_provider_free(cprov);
    if (ossl_provider_is_child(cprov)
            && !ossl_provider_deactivate(cprov, 1))
        return 0;

    return 1;
}

static int provider_global_props_cb(const char *props, void *cbdata)
{
    OSSL_LIB_CTX *ctx = cbdata;

    return evp_set_default_properties_int(ctx, props, 0, 1);
}

int ossl_provider_init_as_child(OSSL_LIB_CTX *ctx,
                                const OSSL_CORE_HANDLE *handle,
                                const OSSL_DISPATCH *in)
{
    struct child_prov_globals *gbl;

    if (ctx == NULL)
        return 0;

    gbl = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    gbl->handle = handle;
    for (; in->function_id != 0; in++) {
        switch (in->function_id) {
        case OSSL_FUNC_CORE_GET_LIBCTX:
            gbl->c_get_libctx = OSSL_FUNC_core_get_libctx(in);
            break;
        case OSSL_FUNC_PROVIDER_REGISTER_CHILD_CB:
            gbl->c_provider_register_child_cb
                = OSSL_FUNC_provider_register_child_cb(in);
            break;
        case OSSL_FUNC_PROVIDER_DEREGISTER_CHILD_CB:
            gbl->c_provider_deregister_child_cb
                = OSSL_FUNC_provider_deregister_child_cb(in);
            break;
        case OSSL_FUNC_PROVIDER_NAME:
            gbl->c_prov_name = OSSL_FUNC_provider_name(in);
            break;
        case OSSL_FUNC_PROVIDER_GET0_PROVIDER_CTX:
            gbl->c_prov_get0_provider_ctx
                = OSSL_FUNC_provider_get0_provider_ctx(in);
            break;
        case OSSL_FUNC_PROVIDER_GET0_DISPATCH:
            gbl->c_prov_get0_dispatch = OSSL_FUNC_provider_get0_dispatch(in);
            break;
        case OSSL_FUNC_PROVIDER_UP_REF:
            gbl->c_prov_up_ref
                = OSSL_FUNC_provider_up_ref(in);
            break;
        case OSSL_FUNC_PROVIDER_FREE:
            gbl->c_prov_free = OSSL_FUNC_provider_free(in);
            break;
        default:
            /* Just ignore anything we don't understand */
            break;
        }
    }

    if (gbl->c_get_libctx == NULL
            || gbl->c_provider_register_child_cb == NULL
            || gbl->c_prov_name == NULL
            || gbl->c_prov_get0_provider_ctx == NULL
            || gbl->c_prov_get0_dispatch == NULL
            || gbl->c_prov_up_ref == NULL
            || gbl->c_prov_free == NULL)
        return 0;

    gbl->lock = CRYPTO_THREAD_lock_new();
    if (gbl->lock == NULL)
        return 0;

    if (!gbl->c_provider_register_child_cb(gbl->handle,
                                           provider_create_child_cb,
                                           provider_remove_child_cb,
                                           provider_global_props_cb,
                                           ctx))
        return 0;

    return 1;
}

void ossl_provider_deinit_child(OSSL_LIB_CTX *ctx)
{
    struct child_prov_globals *gbl
        = ossl_lib_ctx_get_data(ctx, OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return;

    gbl->c_provider_deregister_child_cb(gbl->handle);
}

/*
 * ossl_provider_up_ref_parent() and ossl_provider_free_parent() do
 * nothing in "self-referencing" child providers, i.e. when the parent
 * of the child provider is the same as the provider where this child
 * provider was created.
 * This allows the teardown function in the parent provider to be called
 * at the correct moment.
 * For child providers in other providers, the reference count is done to
 * ensure that cross referencing is recorded.  These should be cleared up
 * through that providers teardown, as part of freeing its child libctx.
 */

int ossl_provider_up_ref_parent(OSSL_PROVIDER *prov, int activate)
{
    struct child_prov_globals *gbl;
    const OSSL_CORE_HANDLE *parent_handle;

    gbl = ossl_lib_ctx_get_data(ossl_provider_libctx(prov),
                                OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    parent_handle = ossl_provider_get_parent(prov);
    if (parent_handle == gbl->handle)
        return 1;
    return gbl->c_prov_up_ref(parent_handle, activate);
}

int ossl_provider_free_parent(OSSL_PROVIDER *prov, int deactivate)
{
    struct child_prov_globals *gbl;
    const OSSL_CORE_HANDLE *parent_handle;

    gbl = ossl_lib_ctx_get_data(ossl_provider_libctx(prov),
                                OSSL_LIB_CTX_CHILD_PROVIDER_INDEX);
    if (gbl == NULL)
        return 0;

    parent_handle = ossl_provider_get_parent(prov);
    if (parent_handle == gbl->handle)
        return 1;
    return gbl->c_prov_free(ossl_provider_get_parent(prov), deactivate);
}
