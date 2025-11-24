/*
 * Copyright 1995-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include <openssl/crypto.h>

#include "internal/tsan_assist.h"

struct lhash_node_st {
    void *data;
    struct lhash_node_st *next;
    unsigned long hash;
};

struct lhash_st {
    OPENSSL_LH_NODE **b;
    OPENSSL_LH_COMPFUNC comp;
    OPENSSL_LH_HASHFUNC hash;
    OPENSSL_LH_HASHFUNCTHUNK hashw;
    OPENSSL_LH_COMPFUNCTHUNK compw;
    OPENSSL_LH_DOALL_FUNC_THUNK daw;
    OPENSSL_LH_DOALL_FUNCARG_THUNK daaw;
    unsigned int num_nodes;
    unsigned int num_alloc_nodes;
    unsigned int p;
    unsigned int pmax;
    unsigned long up_load;      /* load times 256 */
    unsigned long down_load;    /* load times 256 */
    unsigned long num_items;
    int error;
};
