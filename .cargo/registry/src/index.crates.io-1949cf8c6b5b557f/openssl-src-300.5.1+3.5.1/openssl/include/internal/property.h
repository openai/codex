/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PROPERTY_H
# define OSSL_INTERNAL_PROPERTY_H
# pragma once

# include "internal/cryptlib.h"

typedef struct ossl_method_store_st OSSL_METHOD_STORE;
typedef struct ossl_property_list_st OSSL_PROPERTY_LIST;

typedef enum {
    OSSL_PROPERTY_TYPE_STRING, OSSL_PROPERTY_TYPE_NUMBER,
    OSSL_PROPERTY_TYPE_VALUE_UNDEFINED
} OSSL_PROPERTY_TYPE;
typedef struct ossl_property_definition_st OSSL_PROPERTY_DEFINITION;

/* Initialisation */
int ossl_property_parse_init(OSSL_LIB_CTX *ctx);

/* Property definition parser */
OSSL_PROPERTY_LIST *ossl_parse_property(OSSL_LIB_CTX *ctx, const char *defn);
/* Property query parser */
OSSL_PROPERTY_LIST *ossl_parse_query(OSSL_LIB_CTX *ctx, const char *s,
                                     int create_values);
/* Property checker of query vs definition */
int ossl_property_match_count(const OSSL_PROPERTY_LIST *query,
                              const OSSL_PROPERTY_LIST *defn);
int ossl_property_is_enabled(OSSL_LIB_CTX *ctx,  const char *property_name,
                             const OSSL_PROPERTY_LIST *prop_list);
/* Free a parsed property list */
void ossl_property_free(OSSL_PROPERTY_LIST *p);

/* Get a property from a property list */
const OSSL_PROPERTY_DEFINITION *
ossl_property_find_property(const OSSL_PROPERTY_LIST *list,
                            OSSL_LIB_CTX *libctx, const char *name);
OSSL_PROPERTY_TYPE ossl_property_get_type(const OSSL_PROPERTY_DEFINITION *prop);
const char *ossl_property_get_string_value(OSSL_LIB_CTX *libctx,
                                           const OSSL_PROPERTY_DEFINITION *prop);
int64_t ossl_property_get_number_value(const OSSL_PROPERTY_DEFINITION *prop);


/* Implementation store functions */
OSSL_METHOD_STORE *ossl_method_store_new(OSSL_LIB_CTX *ctx);
void ossl_method_store_free(OSSL_METHOD_STORE *store);

int ossl_method_lock_store(OSSL_METHOD_STORE *store);
int ossl_method_unlock_store(OSSL_METHOD_STORE *store);

int ossl_method_store_add(OSSL_METHOD_STORE *store, const OSSL_PROVIDER *prov,
                          int nid, const char *properties, void *method,
                          int (*method_up_ref)(void *),
                          void (*method_destruct)(void *));
int ossl_method_store_remove(OSSL_METHOD_STORE *store, int nid,
                             const void *method);
void ossl_method_store_do_all(OSSL_METHOD_STORE *store,
                              void (*fn)(int id, void *method, void *fnarg),
                              void *fnarg);
int ossl_method_store_fetch(OSSL_METHOD_STORE *store,
                            int nid, const char *prop_query,
                            const OSSL_PROVIDER **prov, void **method);
int ossl_method_store_remove_all_provided(OSSL_METHOD_STORE *store,
                                          const OSSL_PROVIDER *prov);

/* Get the global properties associate with the specified library context */
OSSL_PROPERTY_LIST **ossl_ctx_global_properties(OSSL_LIB_CTX *ctx,
                                                int loadconfig);

/* property query cache functions */
int ossl_method_store_cache_get(OSSL_METHOD_STORE *store, OSSL_PROVIDER *prov,
                                int nid, const char *prop_query, void **result);
int ossl_method_store_cache_set(OSSL_METHOD_STORE *store, OSSL_PROVIDER *prov,
                                int nid, const char *prop_query, void *result,
                                int (*method_up_ref)(void *),
                                void (*method_destruct)(void *));

__owur int ossl_method_store_cache_flush_all(OSSL_METHOD_STORE *store);

/* Merge two property queries together */
OSSL_PROPERTY_LIST *ossl_property_merge(const OSSL_PROPERTY_LIST *a,
                                        const OSSL_PROPERTY_LIST *b);

size_t ossl_property_list_to_string(OSSL_LIB_CTX *ctx,
                                    const OSSL_PROPERTY_LIST *list, char *buf,
                                    size_t bufsize);

int ossl_global_properties_no_mirrored(OSSL_LIB_CTX *libctx);
void ossl_global_properties_stop_mirroring(OSSL_LIB_CTX *libctx);

#endif
