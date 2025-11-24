/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#ifndef OSSL_UINT_SET_H
# define OSSL_UINT_SET_H

#include "openssl/params.h"
#include "internal/list.h"

/*
 * uint64_t Integer Sets
 * =====================
 *
 * Utilities for managing a logical set of unsigned 64-bit integers. The
 * structure tracks each contiguous range of integers using one allocation and
 * is thus optimised for cases where integers tend to appear consecutively.
 * Queries are optimised under the assumption that they will generally be made
 * on integers near the end of the set.
 *
 * Discussion of implementation details can be found in uint_set.c.
 */
typedef struct uint_range_st {
    uint64_t    start, end;
} UINT_RANGE;

typedef struct uint_set_item_st UINT_SET_ITEM;
struct uint_set_item_st {
    OSSL_LIST_MEMBER(uint_set, UINT_SET_ITEM);
    UINT_RANGE                  range;
};

DEFINE_LIST_OF(uint_set, UINT_SET_ITEM);

typedef OSSL_LIST(uint_set) UINT_SET;

void ossl_uint_set_init(UINT_SET *s);
void ossl_uint_set_destroy(UINT_SET *s);

/*
 * Insert a range into a integer set. Returns 0 on allocation failure, in which
 * case the integer set is in a valid but undefined state. Otherwise, returns 1.
 * Ranges can overlap existing ranges without limitation. If a range is a subset
 * of an existing range in the set, this is a no-op and returns 1.
 */
int ossl_uint_set_insert(UINT_SET *s, const UINT_RANGE *range);

/*
 * Remove a range from the set. Returns 0 on allocation failure, in which case
 * the integer set is unchanged. Otherwise, returns 1. Ranges which are not
 * already in the set can be removed without issue. If a passed range is not in
 * the integer set at all, this is a no-op and returns 1.
 */
int ossl_uint_set_remove(UINT_SET *s, const UINT_RANGE *range);

/* Returns 1 iff the given integer is in the integer set. */
int ossl_uint_set_query(const UINT_SET *s, uint64_t v);

#endif
