/*
 * Copyright 2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

/*
 * Test hashtable operation.
 */
#include <limits.h>
#include <openssl/err.h>
#include <openssl/bio.h>
#include <internal/common.h>
#include <internal/hashtable.h>
#include "fuzzer.h"

/*
 * Make the key space very small here to make lookups
 * easy to predict for the purposes of validation
 * A two byte key gives us 65536 possible entries
 * so we can allocate a flat table to compare to
 */
HT_START_KEY_DEFN(fuzzer_key)
HT_DEF_KEY_FIELD(fuzzkey, uint16_t)
HT_END_KEY_DEFN(FUZZER_KEY)

#define FZ_FLAG_ALLOCATED (1 << 0)
typedef struct fuzzer_value_st {
    uint64_t flags;
    uint64_t value;
} FUZZER_VALUE;

IMPLEMENT_HT_VALUE_TYPE_FNS(FUZZER_VALUE, fz, static)

static size_t skipped_values = 0;
static size_t inserts = 0;
static size_t replacements = 0;
static size_t deletes = 0;
static size_t flushes = 0;
static size_t lookups = 0;
static size_t foreaches = 0;
static size_t filters = 0;
static int valfound;

static FUZZER_VALUE *prediction_table = NULL;
static HT *fuzzer_table = NULL;

/*
 * Operational values
 */
#define OP_INSERT  0
#define OP_DELETE  1
#define OP_LOOKUP  2
#define OP_FLUSH   3
#define OP_FOREACH 4
#define OP_FILTER  5
#define OP_END     6 

#define OP_MASK 0x3f
#define INSERT_REPLACE_MASK 0x40
#define OPERATION(x) (((x) & OP_MASK) % OP_END)
#define IS_REPLACE(x) ((x) & INSERT_REPLACE_MASK)

static int table_iterator(HT_VALUE *v, void *arg)
{
    uint16_t keyval = (*(uint16_t *)arg);
    FUZZER_VALUE *f = ossl_ht_fz_FUZZER_VALUE_from_value(v);

    if (f != NULL && f == &prediction_table[keyval]) {
        valfound = 1;
        return 0;
    }

    return 1;
}

static int filter_iterator(HT_VALUE *v, void *arg)
{
    uint16_t keyval = (*(uint16_t *)arg);
    FUZZER_VALUE *f = ossl_ht_fz_FUZZER_VALUE_from_value(v);

    if (f != NULL && f == &prediction_table[keyval])
        return 1;

    return 0;
}

static void fuzz_free_cb(HT_VALUE *v)
{
    FUZZER_VALUE *f = ossl_ht_fz_FUZZER_VALUE_from_value(v);

    if (f != NULL)
        f->flags &= ~FZ_FLAG_ALLOCATED;
}

int FuzzerInitialize(int *argc, char ***argv)
{
    HT_CONFIG fuzz_conf = {NULL, fuzz_free_cb, NULL, 0, 1};

    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS, NULL);
    ERR_clear_error();
    prediction_table = OPENSSL_zalloc(sizeof(FUZZER_VALUE) * 65537);
    if (prediction_table == NULL)
        return -1;
    fuzzer_table = ossl_ht_new(&fuzz_conf);
    if (fuzzer_table == NULL) {
        OPENSSL_free(prediction_table);
        return -1;
    }

    return 0;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    uint8_t op_flags;
    uint16_t keyval;
    int rc;
    int rc_prediction = 1;
    size_t i;
    FUZZER_VALUE *valptr, *lval;
    FUZZER_KEY key;
    HT_VALUE *v = NULL;
    HT_VALUE tv;
    HT_VALUE_LIST *htvlist;

    /*
     * We need at least 11 bytes to be able to do anything here
     * 1 byte to detect the operation to perform, 2 bytes
     * for the lookup key, and 8 bytes of value
     */
    if (len < 11) {
        skipped_values++;
        return -1;
    }

    /*
     * parse out our operation flags and key
     */
    op_flags = buf[0];
    memcpy(&keyval, &buf[1], sizeof(uint16_t));

    /*
     * Initialize our key
     */
    HT_INIT_KEY(&key);

    /*
     * Now do our operation
     */
    switch(OPERATION(op_flags)) {
    case OP_INSERT:
        valptr = &prediction_table[keyval];

        /* reset our key */
        HT_KEY_RESET(&key);

        /* set the proper key value */
        HT_SET_KEY_FIELD(&key, fuzzkey, keyval);

        /* lock the table */
        ossl_ht_write_lock(fuzzer_table);

        /*
         * If the value to insert is already allocated
         * then we expect a conflict in the insert
         * i.e. we predict a return code of 0 instead
         * of 1. On replacement, we expect it to succeed
         * always
         */
        if (valptr->flags & FZ_FLAG_ALLOCATED) {
            if (!IS_REPLACE(op_flags))
                rc_prediction = 0;
        }

        memcpy(&valptr->value, &buf[3], sizeof(uint64_t));
        /*
         * do the insert/replace
         */
        if (IS_REPLACE(op_flags))
            rc = ossl_ht_fz_FUZZER_VALUE_insert(fuzzer_table, TO_HT_KEY(&key),
                                                valptr, &lval);
        else
            rc = ossl_ht_fz_FUZZER_VALUE_insert(fuzzer_table, TO_HT_KEY(&key),
                                                valptr, NULL);

        if (rc == -1)
            /* failed to grow the hash table due to too many collisions */
            break;

        /*
         * mark the entry as being allocated
         */
        valptr->flags |= FZ_FLAG_ALLOCATED;

        /*
         * unlock the table
         */
        ossl_ht_write_unlock(fuzzer_table);

        /*
         * Now check to make sure we did the right thing
         */
        OPENSSL_assert(rc == rc_prediction);

        /*
         * successful insertion if there wasn't a conflict
         */
        if (rc_prediction == 1)
            IS_REPLACE(op_flags) ? replacements++ : inserts++;
        break;

    case OP_DELETE:
        valptr = &prediction_table[keyval];

        /* reset our key */
        HT_KEY_RESET(&key);

        /* set the proper key value */
        HT_SET_KEY_FIELD(&key, fuzzkey, keyval);

        /* lock the table */
        ossl_ht_write_lock(fuzzer_table);

        /*
         * If the value to delete is not already allocated
         * then we expect a miss in the delete
         * i.e. we predict a return code of 0 instead
         * of 1
         */
        if (!(valptr->flags & FZ_FLAG_ALLOCATED))
            rc_prediction = 0;

        /*
         * do the delete
         */
        rc = ossl_ht_delete(fuzzer_table, TO_HT_KEY(&key));

        /*
         * unlock the table
         */
        ossl_ht_write_unlock(fuzzer_table);

        /*
         * Now check to make sure we did the right thing
         */
        OPENSSL_assert(rc == rc_prediction);

        /*
         * once the unlock is done, the table rcu will have synced
         * meaning the free function has run, so we can confirm now
         * that the valptr is no longer allocated
         */
        OPENSSL_assert(!(valptr->flags & FZ_FLAG_ALLOCATED));

        /*
         * successful deletion if there wasn't a conflict
         */
        if (rc_prediction == 1)
            deletes++;

        break;

    case OP_LOOKUP:
        valptr = &prediction_table[keyval];
        lval = NULL;

        /* reset our key */
        HT_KEY_RESET(&key);

        /* set the proper key value */
        HT_SET_KEY_FIELD(&key, fuzzkey, keyval);

        /* lock the table for reading */
        ossl_ht_read_lock(fuzzer_table);

        /*
         * If the value to find is not already allocated
         * then we expect a miss in the lookup
         * i.e. we predict a return code of NULL instead
         * of a pointer
         */
        if (!(valptr->flags & FZ_FLAG_ALLOCATED))
            valptr = NULL;

        /*
         * do the lookup
         */
        lval = ossl_ht_fz_FUZZER_VALUE_get(fuzzer_table, TO_HT_KEY(&key), &v);

        /*
         * unlock the table
         */
        ossl_ht_read_unlock(fuzzer_table);

        /*
         * Now check to make sure we did the right thing
         */
        OPENSSL_assert(lval == valptr);

        /*
         * if we expect a positive lookup, make sure that
         * we can use the _type and to_value functions
         */
        if (valptr != NULL) {
            OPENSSL_assert(ossl_ht_fz_FUZZER_VALUE_type(v) == 1);

            v = ossl_ht_fz_FUZZER_VALUE_to_value(lval, &tv);
            OPENSSL_assert(v->value == lval);
        }

        /*
         * successful lookup if we didn't expect a miss
         */
        if (valptr != NULL)
            lookups++;

        break;

    case OP_FLUSH:
        /*
         * only flush the table rarely 
         */
        if ((flushes % 100000) != 1) {
            skipped_values++;
            flushes++;
            return 0;
        }

        /*
         * lock the table
         */
        ossl_ht_write_lock(fuzzer_table);
        ossl_ht_flush(fuzzer_table);
        ossl_ht_write_unlock(fuzzer_table);

        /*
         * now check to make sure everything is free
         */
       for (i = 0; i < USHRT_MAX; i++)
            OPENSSL_assert((prediction_table[i].flags & FZ_FLAG_ALLOCATED) == 0);

        /* good flush */
        flushes++;
        break;

    case OP_FOREACH:
        valfound = 0;
        valptr = &prediction_table[keyval];

        rc_prediction = 0;
        if (valptr->flags & FZ_FLAG_ALLOCATED)
            rc_prediction = 1;

        ossl_ht_foreach_until(fuzzer_table, table_iterator, &keyval);

        OPENSSL_assert(valfound == rc_prediction);

        foreaches++;
        break;

    case OP_FILTER:
        valptr = &prediction_table[keyval];

        rc_prediction = 0;
        if (valptr->flags & FZ_FLAG_ALLOCATED)
            rc_prediction = 1;

        htvlist = ossl_ht_filter(fuzzer_table, 1, filter_iterator, &keyval);

        OPENSSL_assert(htvlist->list_len == (size_t)rc_prediction);

        ossl_ht_value_list_free(htvlist);
        filters++;
        break;

    default:
        return -1;
    }

    return 0;
}

void FuzzerCleanup(void)
{
    ossl_ht_free(fuzzer_table);
    OPENSSL_free(prediction_table);
    OPENSSL_cleanup();
}
