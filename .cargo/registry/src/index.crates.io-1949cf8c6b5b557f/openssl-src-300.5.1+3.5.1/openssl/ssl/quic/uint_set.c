/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/uint_set.h"
#include "internal/common.h"
#include <assert.h>

/*
 * uint64_t Integer Sets
 * =====================
 *
 * This data structure supports the following operations:
 *
 *   Insert Range: Adds an inclusive range of integers [start, end]
 *                 to the set. Equivalent to Insert for each number
 *                 in the range.
 *
 *   Remove Range: Removes an inclusive range of integers [start, end]
 *                 from the set. Not all of the range need already be in
 *                 the set, but any part of the range in the set is removed.
 *
 *   Query:        Is an integer in the data structure?
 *
 * The data structure can be iterated.
 *
 * For greater efficiency in tracking large numbers of contiguous integers, we
 * track integer ranges rather than individual integers. The data structure
 * manages a list of integer ranges [[start, end]...]. Internally this is
 * implemented as a doubly linked sorted list of range structures, which are
 * automatically split and merged as necessary.
 *
 * This data structure requires O(n) traversal of the list for insertion,
 * removal and query when we are not adding/removing ranges which are near the
 * beginning or end of the set of ranges. For the applications for which this
 * data structure is used (e.g. QUIC PN tracking for ACK generation), it is
 * expected that the number of integer ranges needed at any given time will
 * generally be small and that most operations will be close to the beginning or
 * end of the range.
 *
 * Invariant: The data structure is always sorted in ascending order by value.
 *
 * Invariant: No two adjacent ranges ever 'border' one another (have no
 *            numerical gap between them) as the data structure always ensures
 *            such ranges are merged.
 *
 * Invariant: No two ranges ever overlap.
 *
 * Invariant: No range [a, b] ever has a > b.
 *
 * Invariant: Since ranges are represented using inclusive bounds, no range
 *            item inside the data structure can represent a span of zero
 *            integers.
 */
void ossl_uint_set_init(UINT_SET *s)
{
    ossl_list_uint_set_init(s);
}

void ossl_uint_set_destroy(UINT_SET *s)
{
    UINT_SET_ITEM *x, *xnext;

    for (x = ossl_list_uint_set_head(s); x != NULL; x = xnext) {
        xnext = ossl_list_uint_set_next(x);
        OPENSSL_free(x);
    }
}

/* Possible merge of x, prev(x) */
static void uint_set_merge_adjacent(UINT_SET *s, UINT_SET_ITEM *x)
{
    UINT_SET_ITEM *xprev = ossl_list_uint_set_prev(x);

    if (xprev == NULL)
        return;

    if (x->range.start - 1 != xprev->range.end)
        return;

    x->range.start = xprev->range.start;
    ossl_list_uint_set_remove(s, xprev);
    OPENSSL_free(xprev);
}

static uint64_t u64_min(uint64_t x, uint64_t y)
{
    return x < y ? x : y;
}

static uint64_t u64_max(uint64_t x, uint64_t y)
{
    return x > y ? x : y;
}

/*
 * Returns 1 if there exists an integer x which falls within both ranges a and
 * b.
 */
static int uint_range_overlaps(const UINT_RANGE *a,
                               const UINT_RANGE *b)
{
    return u64_min(a->end, b->end)
        >= u64_max(a->start, b->start);
}

static UINT_SET_ITEM *create_set_item(uint64_t start, uint64_t end)
{
    UINT_SET_ITEM *x = OPENSSL_malloc(sizeof(UINT_SET_ITEM));

    if (x == NULL)
        return NULL;

    ossl_list_uint_set_init_elem(x);
    x->range.start = start;
    x->range.end   = end;
    return x;
}

int ossl_uint_set_insert(UINT_SET *s, const UINT_RANGE *range)
{
    UINT_SET_ITEM *x, *xnext, *z, *zprev, *f;
    uint64_t start = range->start, end = range->end;

    if (!ossl_assert(start <= end))
        return 0;

    if (ossl_list_uint_set_is_empty(s)) {
        /* Nothing in the set yet, so just add this range. */
        x = create_set_item(start, end);
        if (x == NULL)
            return 0;
        ossl_list_uint_set_insert_head(s, x);
        return 1;
    }

    z = ossl_list_uint_set_tail(s);
    if (start > z->range.end) {
        /*
         * Range is after the latest range in the set, so append.
         *
         * Note: The case where the range is before the earliest range in the
         * set is handled as a degenerate case of the final case below. See
         * optimization note (*) below.
         */
        if (z->range.end + 1 == start) {
            z->range.end = end;
            return 1;
        }

        x = create_set_item(start, end);
        if (x == NULL)
            return 0;
        ossl_list_uint_set_insert_tail(s, x);
        return 1;
    }

    f = ossl_list_uint_set_head(s);
    if (start <= f->range.start && end >= z->range.end) {
        /*
         * New range dwarfs all ranges in our set.
         *
         * Free everything except the first range in the set, which we scavenge
         * and reuse.
         */
        x = ossl_list_uint_set_head(s);
        x->range.start = start;
        x->range.end = end;
        for (x = ossl_list_uint_set_next(x); x != NULL; x = xnext) {
            xnext = ossl_list_uint_set_next(x);
            ossl_list_uint_set_remove(s, x);
        }
        return 1;
    }

    /*
     * Walk backwards since we will most often be inserting at the end. As an
     * optimization, test the head node first and skip iterating over the
     * entire list if we are inserting at the start. The assumption is that
     * insertion at the start and end of the space will be the most common
     * operations. (*)
     */
    z = end < f->range.start ? f : z;

    for (; z != NULL; z = zprev) {
        zprev = ossl_list_uint_set_prev(z);

        /* An existing range dwarfs our new range (optimisation). */
        if (z->range.start <= start && z->range.end >= end)
            return 1;

        if (uint_range_overlaps(&z->range, range)) {
            /*
             * Our new range overlaps an existing range, or possibly several
             * existing ranges.
             */
            UINT_SET_ITEM *ovend = z;

            ovend->range.end = u64_max(end, z->range.end);

            /* Get earliest overlapping range. */
            while (zprev != NULL && uint_range_overlaps(&zprev->range, range)) {
                z = zprev;
                zprev = ossl_list_uint_set_prev(z);
            }

            ovend->range.start = u64_min(start, z->range.start);

            /* Replace sequence of nodes z..ovend with updated ovend only. */
            while (z != ovend) {
                z = ossl_list_uint_set_next(x = z);
                ossl_list_uint_set_remove(s, x);
                OPENSSL_free(x);
            }
            break;
        } else if (end < z->range.start
                    && (zprev == NULL || start > zprev->range.end)) {
            if (z->range.start == end + 1) {
                /* We can extend the following range backwards. */
                z->range.start = start;

                /*
                 * If this closes a gap we now need to merge
                 * consecutive nodes.
                 */
                uint_set_merge_adjacent(s, z);
            } else if (zprev != NULL && zprev->range.end + 1 == start) {
                /* We can extend the preceding range forwards. */
                zprev->range.end = end;

                /*
                 * If this closes a gap we now need to merge
                 * consecutive nodes.
                 */
                uint_set_merge_adjacent(s, z);
            } else {
                /*
                 * The new interval is between intervals without overlapping or
                 * touching them, so insert between, preserving sort.
                 */
                x = create_set_item(start, end);
                if (x == NULL)
                    return 0;
                ossl_list_uint_set_insert_before(s, z, x);
            }
            break;
        }
    }

    return 1;
}

int ossl_uint_set_remove(UINT_SET *s, const UINT_RANGE *range)
{
    UINT_SET_ITEM *z, *zprev, *y;
    uint64_t start = range->start, end = range->end;

    if (!ossl_assert(start <= end))
        return 0;

    /* Walk backwards since we will most often be removing at the end. */
    for (z = ossl_list_uint_set_tail(s); z != NULL; z = zprev) {
        zprev = ossl_list_uint_set_prev(z);

        if (start > z->range.end)
            /* No overlapping ranges can exist beyond this point, so stop. */
            break;

        if (start <= z->range.start && end >= z->range.end) {
            /*
             * The range being removed dwarfs this range, so it should be
             * removed.
             */
            ossl_list_uint_set_remove(s, z);
            OPENSSL_free(z);
        } else if (start <= z->range.start && end >= z->range.start) {
            /*
             * The range being removed includes start of this range, but does
             * not cover the entire range (as this would be caught by the case
             * above). Shorten the range.
             */
            assert(end < z->range.end);
            z->range.start = end + 1;
        } else if (end >= z->range.end) {
            /*
             * The range being removed includes the end of this range, but does
             * not cover the entire range (as this would be caught by the case
             * above). Shorten the range. We can also stop iterating.
             */
            assert(start > z->range.start);
            assert(start > 0);
            z->range.end = start - 1;
            break;
        } else if (start > z->range.start && end < z->range.end) {
            /*
             * The range being removed falls entirely in this range, so cut it
             * into two. Cases where a zero-length range would be created are
             * handled by the above cases.
             */
            y = create_set_item(end + 1, z->range.end);
            ossl_list_uint_set_insert_after(s, z, y);
            z->range.end = start - 1;
            break;
        } else {
            /* Assert no partial overlap; all cases should be covered above. */
            assert(!uint_range_overlaps(&z->range, range));
        }
    }

    return 1;
}

int ossl_uint_set_query(const UINT_SET *s, uint64_t v)
{
    UINT_SET_ITEM *x;

    if (ossl_list_uint_set_is_empty(s))
        return 0;

    for (x = ossl_list_uint_set_tail(s); x != NULL; x = ossl_list_uint_set_prev(x))
        if (x->range.start <= v && x->range.end >= v)
            return 1;
        else if (x->range.end < v)
            return 0;

    return 0;
}
