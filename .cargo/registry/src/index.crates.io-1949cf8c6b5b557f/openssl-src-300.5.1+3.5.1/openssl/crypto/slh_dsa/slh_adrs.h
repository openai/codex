/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/e_os2.h>

/*
 * An Address object is used to store a blob of data that is used by hash
 * functions. It stores information related to the type of operation, as well as
 * information related to tree addresses and heights.
 * SHAKE based algorithms use 32 bytes for this object, whereas SHA2 based
 * algorithms use a compressed format of 22 bytes. For this reason there are
 * different method tables to support the different formats.
 * FIPS 205 Section 4.2 describes the SHAKE related functions.
 * The compressed format is discussed in Section 11.2.
 */

#define SLH_ADRS_SIZE     32 /* size of the ADRS blob */
#define SLH_ADRSC_SIZE    22 /* size of a compact ADRS blob */
#define SLH_ADRS_SIZE_MAX SLH_ADRS_SIZE

/* 7 Different types of addresses */
#define SLH_ADRS_TYPE_WOTS_HASH  0
#define SLH_ADRS_TYPE_WOTS_PK    1
#define SLH_ADRS_TYPE_TREE       2
#define SLH_ADRS_TYPE_FORS_TREE  3
#define SLH_ADRS_TYPE_FORS_ROOTS 4
#define SLH_ADRS_TYPE_WOTS_PRF   5
#define SLH_ADRS_TYPE_FORS_PRF   6

#define SLH_ADRS_DECLARE(a) uint8_t a[SLH_ADRS_SIZE_MAX]
#define SLH_ADRS_FUNC_DECLARE(ctx, adrsf) \
    const SLH_ADRS_FUNC *adrsf = ctx->adrs_func
#define SLH_ADRS_FN_DECLARE(adrsf, t) OSSL_SLH_ADRS_FUNC_##t *t = adrsf->t

typedef void (OSSL_SLH_ADRS_FUNC_zero)(uint8_t *adrs);
typedef void (OSSL_SLH_ADRS_FUNC_copy)(uint8_t *dst, const uint8_t *src);
typedef void (OSSL_SLH_ADRS_FUNC_copy_keypair_address)(uint8_t *dst, const uint8_t *src);
/*
 * Note that the tree address is actually 12 bytes in uncompressed format,
 * but we only use 8 bytes
 */
typedef void (OSSL_SLH_ADRS_FUNC_set_tree_address)(uint8_t *adrs, uint64_t in);
typedef void (OSSL_SLH_ADRS_FUNC_set_layer_address)(uint8_t *adrs, uint32_t layer);
typedef void (OSSL_SLH_ADRS_FUNC_set_type_and_clear)(uint8_t *adrs, uint32_t type);
typedef void (OSSL_SLH_ADRS_FUNC_set_keypair_address)(uint8_t *adrs, uint32_t in);
typedef void (OSSL_SLH_ADRS_FUNC_set_chain_address)(uint8_t *adrs, uint32_t in);
typedef void (OSSL_SLH_ADRS_FUNC_set_tree_height)(uint8_t *adrs, uint32_t in);
typedef void (OSSL_SLH_ADRS_FUNC_set_hash_address)(uint8_t *adrs, uint32_t in);
typedef void (OSSL_SLH_ADRS_FUNC_set_tree_index)(uint8_t *adrs, uint32_t in);

typedef struct slh_adrs_func_st {
    OSSL_SLH_ADRS_FUNC_set_layer_address *set_layer_address;
    OSSL_SLH_ADRS_FUNC_set_tree_address *set_tree_address;
    OSSL_SLH_ADRS_FUNC_set_type_and_clear *set_type_and_clear;
    OSSL_SLH_ADRS_FUNC_set_keypair_address *set_keypair_address;
    OSSL_SLH_ADRS_FUNC_copy_keypair_address *copy_keypair_address;
    OSSL_SLH_ADRS_FUNC_set_chain_address *set_chain_address;
    OSSL_SLH_ADRS_FUNC_set_tree_height *set_tree_height;
    OSSL_SLH_ADRS_FUNC_set_hash_address *set_hash_address;
    OSSL_SLH_ADRS_FUNC_set_tree_index *set_tree_index;
    OSSL_SLH_ADRS_FUNC_zero *zero;
    OSSL_SLH_ADRS_FUNC_copy *copy;
} SLH_ADRS_FUNC;

const SLH_ADRS_FUNC *ossl_slh_get_adrs_fn(int is_compressed);
