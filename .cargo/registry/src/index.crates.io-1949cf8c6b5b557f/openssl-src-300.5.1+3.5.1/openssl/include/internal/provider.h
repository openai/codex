/*
 * Copyright 2019-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PROVIDER_H
# define OSSL_INTERNAL_PROVIDER_H
# pragma once

# include <openssl/core.h>
# include <openssl/core_dispatch.h>
# include "internal/dso.h"
# include "internal/symhacks.h"

# ifdef __cplusplus
extern "C" {
# endif

/*
 * namespaces:
 *
 * ossl_provider_       Provider Object internal API
 * OSSL_PROVIDER        Provider Object
 */

/* Provider Object finder, constructor and destructor */
OSSL_PROVIDER *ossl_provider_find(OSSL_LIB_CTX *libctx, const char *name,
                                  int noconfig);
OSSL_PROVIDER *ossl_provider_new(OSSL_LIB_CTX *libctx, const char *name,
                                 OSSL_provider_init_fn *init_function,
                                 OSSL_PARAM *params, int noconfig);
int ossl_provider_up_ref(OSSL_PROVIDER *prov);
void ossl_provider_free(OSSL_PROVIDER *prov);

/* Setters */
int ossl_provider_set_module_path(OSSL_PROVIDER *prov, const char *module_path);

int ossl_provider_is_child(const OSSL_PROVIDER *prov);
int ossl_provider_set_child(OSSL_PROVIDER *prov, const OSSL_CORE_HANDLE *handle);
const OSSL_CORE_HANDLE *ossl_provider_get_parent(OSSL_PROVIDER *prov);
int ossl_provider_up_ref_parent(OSSL_PROVIDER *prov, int activate);
int ossl_provider_free_parent(OSSL_PROVIDER *prov, int deactivate);
int ossl_provider_default_props_update(OSSL_LIB_CTX *libctx, const char *props);

/* Disable fallback loading */
int ossl_provider_disable_fallback_loading(OSSL_LIB_CTX *libctx);

/*
 * Activate the Provider
 * If the Provider is a module, the module will be loaded
 */
int ossl_provider_activate(OSSL_PROVIDER *prov, int upcalls, int aschild);
int ossl_provider_deactivate(OSSL_PROVIDER *prov, int removechildren);
int ossl_provider_add_to_store(OSSL_PROVIDER *prov, OSSL_PROVIDER **actualprov,
                               int retain_fallbacks);

/* Return pointer to the provider's context */
void *ossl_provider_ctx(const OSSL_PROVIDER *prov);

/* Force loading of fallback providers if necessary */
int ossl_provider_activate_fallbacks(OSSL_LIB_CTX *ctx);

/* Iterate over all loaded providers */
int ossl_provider_doall_activated(OSSL_LIB_CTX *,
                                  int (*cb)(OSSL_PROVIDER *provider,
                                            void *cbdata),
                                  void *cbdata);

/* Getters for other library functions */
const char *ossl_provider_name(const OSSL_PROVIDER *prov);
const DSO *ossl_provider_dso(const OSSL_PROVIDER *prov);
const char *ossl_provider_module_name(const OSSL_PROVIDER *prov);
const char *ossl_provider_module_path(const OSSL_PROVIDER *prov);
const OSSL_DISPATCH *ossl_provider_get0_dispatch(const OSSL_PROVIDER *prov);
OSSL_LIB_CTX *ossl_provider_libctx(const OSSL_PROVIDER *prov);

/* Thin wrappers around calls to the provider */
void ossl_provider_teardown(const OSSL_PROVIDER *prov);
const OSSL_PARAM *ossl_provider_gettable_params(const OSSL_PROVIDER *prov);
int ossl_provider_get_params(const OSSL_PROVIDER *prov, OSSL_PARAM params[]);
int ossl_provider_get_capabilities(const OSSL_PROVIDER *prov,
                                   const char *capability,
                                   OSSL_CALLBACK *cb,
                                   void *arg);
int ossl_provider_self_test(const OSSL_PROVIDER *prov);
int ossl_provider_random_bytes(const OSSL_PROVIDER *prov, int which,
                               void *buf, size_t n, unsigned int strength);
const OSSL_ALGORITHM *ossl_provider_query_operation(const OSSL_PROVIDER *prov,
                                                    int operation_id,
                                                    int *no_cache);
void ossl_provider_unquery_operation(const OSSL_PROVIDER *prov,
                                     int operation_id,
                                     const OSSL_ALGORITHM *algs);

/*
 * Cache of bits to see if we already added methods for an operation in
 * the "permanent" method store.
 * They should never be called for temporary method stores!
 */
int ossl_provider_set_operation_bit(OSSL_PROVIDER *provider, size_t bitnum);
int ossl_provider_test_operation_bit(OSSL_PROVIDER *provider, size_t bitnum,
                                     int *result);

/* Configuration */
void ossl_provider_add_conf_module(void);

/* Child providers */
int ossl_provider_init_as_child(OSSL_LIB_CTX *ctx,
                                const OSSL_CORE_HANDLE *handle,
                                const OSSL_DISPATCH *in);
void ossl_provider_deinit_child(OSSL_LIB_CTX *ctx);

# ifdef __cplusplus
}
# endif

#endif
