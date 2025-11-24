/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright (c) 2019, Oracle and/or its affiliates.  All rights reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/crypto.h>
#include "internal/property.h"

typedef int OSSL_PROPERTY_IDX;

typedef enum {
    OSSL_PROPERTY_OPER_EQ, OSSL_PROPERTY_OPER_NE, OSSL_PROPERTY_OVERRIDE
} OSSL_PROPERTY_OPER;

struct ossl_property_definition_st {
    OSSL_PROPERTY_IDX name_idx;
    OSSL_PROPERTY_TYPE type;
    OSSL_PROPERTY_OPER oper;
    unsigned int optional : 1;
    union {
        int64_t             int_val;     /* Signed integer */
        OSSL_PROPERTY_IDX   str_val;     /* String */
    } v;
};

struct ossl_property_list_st {
    int num_properties;
    unsigned int has_optional : 1;
    OSSL_PROPERTY_DEFINITION properties[1];
};

#define OSSL_PROPERTY_TRUE      1
#define OSSL_PROPERTY_FALSE     2

/* Property string functions */
OSSL_PROPERTY_IDX ossl_property_name(OSSL_LIB_CTX *ctx, const char *s,
                                     int create);
const char *ossl_property_name_str(OSSL_LIB_CTX *ctx, OSSL_PROPERTY_IDX idx);
OSSL_PROPERTY_IDX ossl_property_value(OSSL_LIB_CTX *ctx, const char *s,
                                      int create);
const char *ossl_property_value_str(OSSL_LIB_CTX *ctx, OSSL_PROPERTY_IDX idx);

/* Property list functions */
void ossl_property_free(OSSL_PROPERTY_LIST *p);
int ossl_property_has_optional(const OSSL_PROPERTY_LIST *query);

/* Property definition cache functions */
OSSL_PROPERTY_LIST *ossl_prop_defn_get(OSSL_LIB_CTX *ctx, const char *prop);
int ossl_prop_defn_set(OSSL_LIB_CTX *ctx, const char *prop,
                       OSSL_PROPERTY_LIST **pl);
