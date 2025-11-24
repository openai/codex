/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 *
 *
 *
 * Notes On hash table design and layout
 * This hashtable uses a hopscotch algorithm to do indexing.  The data structure
 * looks as follows:
 *
 *   hash          +--------------+
 *   value+------->+ HT_VALUE     |
 *      +          +--------------+
 *  +-------+
 *  |       |
 *  +---------------------------------------------------------+
 *  |       |       |       |       |                         |
 *  | entry | entry | entry | entry |                         |
 *  |       |       |       |       |                         |
 *  +---------------------------------------------------------+
 *  |                               |                         |
 *  |                               |                         |
 *  +---------------------------------------------------------+
 *  |              +                             +            +
 *  |        neighborhood[0]               neighborhood[1]    |
 *  |                                                         |
 *  |                                                         |
 *  +---------------------------------------------------------+
 *                              |
 *                              +
 *                         neighborhoods
 *
 * On lookup/insert/delete, the items key is hashed to a 64 bit value
 * and the result is masked to provide an index into the neighborhoods
 * table.  Once a neighborhood is determined, an in-order search is done
 * of the elements in the neighborhood indexes entries for a matching hash
 * value, if found, the corresponding HT_VALUE is used for the respective
 * operation.  The number of entries in a neighborhood is determined at build
 * time based on the cacheline size of the target CPU.  The intent is for a
 * neighborhood to have all entries in the neighborhood fit into a single cache
 * line to speed up lookups.  If all entries in a neighborhood are in use at the
 * time of an insert, the table is expanded and rehashed.
 *
 * Lockless reads hash table is based on the same design but does not
 * allow growing and deletion. Thus subsequent neighborhoods are always
 * searched for a match until an empty entry is found.
 */

#include <string.h>
#include <internal/rcu.h>
#include <internal/hashtable.h>
#include <internal/hashfunc.h>
#include <openssl/rand.h>

/*
 * gcc defines __SANITIZE_THREAD__
 * but clang uses the feature attributes api
 * map the latter to the former
 */
#if defined(__clang__) && defined(__has_feature)
# if __has_feature(thread_sanitizer)
#  define __SANITIZE_THREADS__
# endif
#endif

#ifdef __SANITIZE_THREADS__
# include <sanitizer/tsan_interface.h>
#endif

#include "internal/numbers.h"
/*
 * When we do a lookup/insert/delete, there is a high likelihood
 * that we will iterate over at least part of the neighborhood list
 * As such, because we design a neighborhood entry to fit into a single
 * cache line it is advantageous, when supported to fetch the entire
 * structure for faster lookups
 */
#if defined(__GNUC__) || defined(__CLANG__)
# define PREFETCH_NEIGHBORHOOD(x) __builtin_prefetch(x.entries)
# define PREFETCH(x) __builtin_prefetch(x)
#else
# define PREFETCH_NEIGHBORHOOD(x)
# define PREFETCH(x)
#endif

/*
 * Define our neighborhood list length
 * Note: It should always be a power of 2
 */
#define DEFAULT_NEIGH_LEN_LOG 4
#define DEFAULT_NEIGH_LEN (1 << DEFAULT_NEIGH_LEN_LOG)

/*
 * For now assume cache line size is 64 bytes
 */
#define CACHE_LINE_BYTES 64
#define CACHE_LINE_ALIGNMENT CACHE_LINE_BYTES

#define NEIGHBORHOOD_LEN (CACHE_LINE_BYTES / sizeof(struct ht_neighborhood_entry_st))
/*
 * Defines our chains of values
 */
struct ht_internal_value_st {
    HT_VALUE value;
    HT *ht;
};

struct ht_neighborhood_entry_st {
    uint64_t hash;
    struct ht_internal_value_st *value;
};

struct ht_neighborhood_st {
    struct ht_neighborhood_entry_st entries[NEIGHBORHOOD_LEN];
};

/*
 * Updates to data in this struct
 * require an rcu sync after modification
 * prior to free
 */
struct ht_mutable_data_st {
    struct ht_neighborhood_st *neighborhoods;
    void *neighborhood_ptr_to_free;
    uint64_t neighborhood_mask;
};

/*
 * Private data may be updated on the write
 * side only, and so do not require rcu sync
 */
struct ht_write_private_data_st {
    size_t neighborhood_len;
    size_t value_count;
    int need_sync;
};

struct ht_internal_st {
    HT_CONFIG config;
    CRYPTO_RCU_LOCK *lock;
    CRYPTO_RWLOCK *atomic_lock;
    struct ht_mutable_data_st *md;
    struct ht_write_private_data_st wpd;
};

static void free_value(struct ht_internal_value_st *v);

static struct ht_neighborhood_st *alloc_new_neighborhood_list(size_t len,
                                                              void **freeptr)
{
    struct ht_neighborhood_st *ret;

    ret = OPENSSL_aligned_alloc(sizeof(struct ht_neighborhood_st) * len,
                                CACHE_LINE_BYTES, freeptr);

    /* fall back to regular malloc */
    if (ret == NULL) {
        ret = *freeptr = OPENSSL_malloc(sizeof(struct ht_neighborhood_st) * len);
        if (ret == NULL)
            return NULL;
    }
    memset(ret, 0, sizeof(struct ht_neighborhood_st) * len);
    return ret;
}

static void internal_free_nop(HT_VALUE *v)
{
    return;
}

HT *ossl_ht_new(const HT_CONFIG *conf)
{
    HT *new = OPENSSL_zalloc(sizeof(*new));

    if (new == NULL)
        return NULL;

    new->atomic_lock = CRYPTO_THREAD_lock_new();
    if (new->atomic_lock == NULL)
        goto err;

    memcpy(&new->config, conf, sizeof(*conf));

    if (new->config.init_neighborhoods != 0) {
        new->wpd.neighborhood_len = new->config.init_neighborhoods;
        /* round up to the next power of 2 */
        new->wpd.neighborhood_len--;
        new->wpd.neighborhood_len |= new->wpd.neighborhood_len >> 1;
        new->wpd.neighborhood_len |= new->wpd.neighborhood_len >> 2;
        new->wpd.neighborhood_len |= new->wpd.neighborhood_len >> 4;
        new->wpd.neighborhood_len |= new->wpd.neighborhood_len >> 8;
        new->wpd.neighborhood_len |= new->wpd.neighborhood_len >> 16;
        new->wpd.neighborhood_len++;
    } else {
        new->wpd.neighborhood_len = DEFAULT_NEIGH_LEN;
    }

    if (new->config.ht_free_fn == NULL)
        new->config.ht_free_fn = internal_free_nop;

    new->md = OPENSSL_zalloc(sizeof(*new->md));
    if (new->md == NULL)
        goto err;

    new->md->neighborhoods =
        alloc_new_neighborhood_list(new->wpd.neighborhood_len,
                                    &new->md->neighborhood_ptr_to_free);
    if (new->md->neighborhoods == NULL)
        goto err;
    new->md->neighborhood_mask = new->wpd.neighborhood_len - 1;

    new->lock = ossl_rcu_lock_new(1, conf->ctx);
    if (new->lock == NULL)
        goto err;

    if (new->config.ht_hash_fn == NULL)
        new->config.ht_hash_fn = ossl_fnv1a_hash;

    return new;

err:
    CRYPTO_THREAD_lock_free(new->atomic_lock);
    ossl_rcu_lock_free(new->lock);
    if (new->md != NULL)
        OPENSSL_free(new->md->neighborhood_ptr_to_free);
    OPENSSL_free(new->md);
    OPENSSL_free(new);
    return NULL;
}

void ossl_ht_read_lock(HT *htable)
{
    ossl_rcu_read_lock(htable->lock);
}

void ossl_ht_read_unlock(HT *htable)
{
    ossl_rcu_read_unlock(htable->lock);
}

void ossl_ht_write_lock(HT *htable)
{
    ossl_rcu_write_lock(htable->lock);
    htable->wpd.need_sync = 0;
}

void ossl_ht_write_unlock(HT *htable)
{
    int need_sync = htable->wpd.need_sync;

    htable->wpd.need_sync = 0;
    ossl_rcu_write_unlock(htable->lock);
    if (need_sync)
        ossl_synchronize_rcu(htable->lock);
}

static void free_oldmd(void *arg)
{
    struct ht_mutable_data_st *oldmd = arg;
    size_t i, j;
    size_t neighborhood_len = (size_t)oldmd->neighborhood_mask + 1;
    struct ht_internal_value_st *v;

    for (i = 0; i < neighborhood_len; i++) {
        PREFETCH_NEIGHBORHOOD(oldmd->neighborhoods[i + 1]);
        for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
            if (oldmd->neighborhoods[i].entries[j].value != NULL) {
                v = oldmd->neighborhoods[i].entries[j].value;
                v->ht->config.ht_free_fn((HT_VALUE *)v);
                free_value(v);
            }
        }
    }

    OPENSSL_free(oldmd->neighborhood_ptr_to_free);
    OPENSSL_free(oldmd);
}

static int ossl_ht_flush_internal(HT *h)
{
    struct ht_mutable_data_st *newmd = NULL;
    struct ht_mutable_data_st *oldmd = NULL;

    newmd = OPENSSL_zalloc(sizeof(*newmd));
    if (newmd == NULL)
        return 0;

    newmd->neighborhoods = alloc_new_neighborhood_list(DEFAULT_NEIGH_LEN,
                                                       &newmd->neighborhood_ptr_to_free);
    if (newmd->neighborhoods == NULL) {
        OPENSSL_free(newmd);
        return 0;
    }

    newmd->neighborhood_mask = DEFAULT_NEIGH_LEN - 1;

    /* Swap the old and new mutable data sets */
    oldmd = ossl_rcu_deref(&h->md);
    ossl_rcu_assign_ptr(&h->md, &newmd);

    /* Set the number of entries to 0 */
    h->wpd.value_count = 0;
    h->wpd.neighborhood_len = DEFAULT_NEIGH_LEN;

    ossl_rcu_call(h->lock, free_oldmd, oldmd);
    h->wpd.need_sync = 1;
    return 1;
}

int ossl_ht_flush(HT *h)
{
    return ossl_ht_flush_internal(h);
}

void ossl_ht_free(HT *h)
{
    if (h == NULL)
        return;

    ossl_ht_write_lock(h);
    ossl_ht_flush_internal(h);
    ossl_ht_write_unlock(h);
    /* Freeing the lock does a final sync for us */
    CRYPTO_THREAD_lock_free(h->atomic_lock);
    ossl_rcu_lock_free(h->lock);
    OPENSSL_free(h->md->neighborhood_ptr_to_free);
    OPENSSL_free(h->md);
    OPENSSL_free(h);
    return;
}

size_t ossl_ht_count(HT *h)
{
    size_t count;

    count = h->wpd.value_count;
    return count;
}

void ossl_ht_foreach_until(HT *h, int (*cb)(HT_VALUE *obj, void *arg),
                           void *arg)
{
    size_t i, j;
    struct ht_mutable_data_st *md;

    md = ossl_rcu_deref(&h->md);
    for (i = 0; i < md->neighborhood_mask + 1; i++) {
        PREFETCH_NEIGHBORHOOD(md->neighborhoods[i + 1]);
        for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
            if (md->neighborhoods[i].entries[j].value != NULL) {
                if (!cb((HT_VALUE *)md->neighborhoods[i].entries[j].value, arg))
                    goto out;
            }
        }
    }
out:
    return;
}

HT_VALUE_LIST *ossl_ht_filter(HT *h, size_t max_len,
                                     int (*filter)(HT_VALUE *obj, void *arg),
                                     void *arg)
{
    struct ht_mutable_data_st *md;
    HT_VALUE_LIST *list = OPENSSL_zalloc(sizeof(HT_VALUE_LIST)
                                         + (sizeof(HT_VALUE *) * max_len));
    size_t i, j;
    struct ht_internal_value_st *v;

    if (list == NULL)
        return NULL;

    /*
     * The list array lives just beyond the end of
     * the struct
     */
    list->list = (HT_VALUE **)(list + 1);

    md = ossl_rcu_deref(&h->md);
    for (i = 0; i < md->neighborhood_mask + 1; i++) {
        PREFETCH_NEIGHBORHOOD(md->neighborhoods[i+1]);
        for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
            v = md->neighborhoods[i].entries[j].value;
            if (v != NULL && filter((HT_VALUE *)v, arg)) {
                list->list[list->list_len++] = (HT_VALUE *)v;
                if (list->list_len == max_len)
                    goto out;
            }
        }
    }
out:
    return list;
}

void ossl_ht_value_list_free(HT_VALUE_LIST *list)
{
    OPENSSL_free(list);
}

static int compare_hash(uint64_t hash1, uint64_t hash2)
{
    return (hash1 == hash2);
}

static void free_old_neigh_table(void *arg)
{
    struct ht_mutable_data_st *oldmd = arg;

    OPENSSL_free(oldmd->neighborhood_ptr_to_free);
    OPENSSL_free(oldmd);
}

/*
 * Increase hash table bucket list
 * must be called with write_lock held
 */
static int grow_hashtable(HT *h, size_t oldsize)
{
    struct ht_mutable_data_st *newmd;
    struct ht_mutable_data_st *oldmd = ossl_rcu_deref(&h->md);
    int rc = 0;
    uint64_t oldi, oldj, newi, newj;
    uint64_t oldhash;
    struct ht_internal_value_st *oldv;
    int rehashed;
    size_t newsize = oldsize * 2;

    if (h->config.lockless_reads)
        goto out;

    if ((newmd = OPENSSL_zalloc(sizeof(*newmd))) == NULL)
        goto out;

    /* bucket list is always a power of 2 */
    newmd->neighborhoods = alloc_new_neighborhood_list(oldsize * 2,
                                                       &newmd->neighborhood_ptr_to_free);
    if (newmd->neighborhoods == NULL)
        goto out_free;

    /* being a power of 2 makes for easy mask computation */
    newmd->neighborhood_mask = (newsize - 1);

    /*
     * Now we need to start rehashing entries
     * Note we don't need to use atomics here as the new
     * mutable data hasn't been published
     */
    for (oldi = 0; oldi < h->wpd.neighborhood_len; oldi++) {
        PREFETCH_NEIGHBORHOOD(oldmd->neighborhoods[oldi + 1]);
        for (oldj = 0; oldj < NEIGHBORHOOD_LEN; oldj++) {
            oldv = oldmd->neighborhoods[oldi].entries[oldj].value;
            if (oldv == NULL)
                continue;
            oldhash = oldmd->neighborhoods[oldi].entries[oldj].hash;
            newi = oldhash & newmd->neighborhood_mask;
            rehashed = 0;
            for (newj = 0; newj < NEIGHBORHOOD_LEN; newj++) {
                if (newmd->neighborhoods[newi].entries[newj].value == NULL) {
                    newmd->neighborhoods[newi].entries[newj].value = oldv;
                    newmd->neighborhoods[newi].entries[newj].hash = oldhash;
                    rehashed = 1;
                    break;
                }
            }
            if (rehashed == 0) {
                /* we ran out of space in a neighborhood, grow again */
                OPENSSL_free(newmd->neighborhoods);
                OPENSSL_free(newmd);
                return grow_hashtable(h, newsize);
            }
        }
    }
    /*
     * Now that our entries are all hashed into the new bucket list
     * update our bucket_len and target_max_load
     */
    h->wpd.neighborhood_len = newsize;

    /*
     * Now we replace the old mutable data with the new
     */
    ossl_rcu_assign_ptr(&h->md, &newmd);
    ossl_rcu_call(h->lock, free_old_neigh_table, oldmd);
    h->wpd.need_sync = 1;
    /*
     * And we're done
     */
    rc = 1;

out:
    return rc;
out_free:
    OPENSSL_free(newmd->neighborhoods);
    OPENSSL_free(newmd);
    goto out;
}

static void free_old_ht_value(void *arg)
{
    HT_VALUE *h = (HT_VALUE *)arg;

    /*
     * Note, this is only called on replacement,
     * the caller is responsible for freeing the
     * held data, we just need to free the wrapping
     * struct here
     */
    OPENSSL_free(h);
}

static ossl_inline int match_key(HT_KEY *a, HT_KEY *b)
{
    /*
     * keys match if they are both present, the same size
     * and compare equal in memory
     */
    PREFETCH(a->keybuf);
    PREFETCH(b->keybuf);
    if (a->keybuf != NULL && b->keybuf != NULL && a->keysize == b->keysize)
        return !memcmp(a->keybuf, b->keybuf, a->keysize);

    return 1;
}

static int ossl_ht_insert_locked(HT *h, uint64_t hash,
                                 struct ht_internal_value_st *newval,
                                 HT_VALUE **olddata)
{
    struct ht_mutable_data_st *md = h->md;
    uint64_t neigh_idx_start = hash & md->neighborhood_mask;
    uint64_t neigh_idx = neigh_idx_start;
    size_t j;
    uint64_t ihash;
    HT_VALUE *ival;
    size_t empty_idx = SIZE_MAX;
    int lockless_reads = h->config.lockless_reads;

    do {
        PREFETCH_NEIGHBORHOOD(md->neighborhoods[neigh_idx]);

        for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
            ival = ossl_rcu_deref(&md->neighborhoods[neigh_idx].entries[j].value);
            if (ival == NULL) {
                empty_idx = j;
                /* lockless_reads implies no deletion, we can break out */
                if (lockless_reads)
                    goto not_found;
                continue;
            }
            if (!CRYPTO_atomic_load(&md->neighborhoods[neigh_idx].entries[j].hash,
                                    &ihash, h->atomic_lock))
                return 0;
            if (compare_hash(hash, ihash) && match_key(&newval->value.key,
                                                       &ival->key)) {
                if (olddata == NULL) {
                    /* This would insert a duplicate -> fail */
                    return 0;
                }
                /* Do a replacement */
                if (!CRYPTO_atomic_store(&md->neighborhoods[neigh_idx].entries[j].hash,
                                         hash, h->atomic_lock))
                    return 0;
                *olddata = (HT_VALUE *)md->neighborhoods[neigh_idx].entries[j].value;
                ossl_rcu_assign_ptr(&md->neighborhoods[neigh_idx].entries[j].value,
                                    &newval);
                ossl_rcu_call(h->lock, free_old_ht_value, *olddata);
                h->wpd.need_sync = 1;
                return 1;
            }
        }
        if (!lockless_reads)
            break;
        /* Continue search in subsequent neighborhoods */
        neigh_idx = (neigh_idx + 1) & md->neighborhood_mask;
    } while (neigh_idx != neigh_idx_start);

 not_found:
    /* If we get to here, its just an insert */
    if (empty_idx == SIZE_MAX)
        return -1; /* out of space */
    if (!CRYPTO_atomic_store(&md->neighborhoods[neigh_idx].entries[empty_idx].hash,
                             hash, h->atomic_lock))
        return 0;
    h->wpd.value_count++;
    ossl_rcu_assign_ptr(&md->neighborhoods[neigh_idx].entries[empty_idx].value,
                        &newval);
    return 1;
}

static struct ht_internal_value_st *alloc_new_value(HT *h, HT_KEY *key,
                                                    void *data,
                                                    uintptr_t *type)
{
    struct ht_internal_value_st *tmp;
    size_t nvsize = sizeof(*tmp);

    if (h->config.collision_check == 1)
        nvsize += key->keysize;

    tmp = OPENSSL_malloc(nvsize);

    if (tmp == NULL)
        return NULL;

    tmp->ht = h;
    tmp->value.value = data;
    tmp->value.type_id = type;
    tmp->value.key.keybuf = NULL;
    if (h->config.collision_check) {
        tmp->value.key.keybuf = (uint8_t *)(tmp + 1);
        tmp->value.key.keysize = key->keysize;
        memcpy(tmp->value.key.keybuf, key->keybuf, key->keysize);
    }


    return tmp;
}

static void free_value(struct ht_internal_value_st *v)
{
    OPENSSL_free(v);
}

int ossl_ht_insert(HT *h, HT_KEY *key, HT_VALUE *data, HT_VALUE **olddata)
{
    struct ht_internal_value_st *newval = NULL;
    uint64_t hash;
    int rc = 0;
    int i;

    if (data->value == NULL)
        goto out;

    newval = alloc_new_value(h, key, data->value, data->type_id);
    if (newval == NULL)
        goto out;

    /*
     * we have to take our lock here to prevent other changes
     * to the bucket list
     */
    hash = h->config.ht_hash_fn(key->keybuf, key->keysize);

    for (i = 0;
         (rc = ossl_ht_insert_locked(h, hash, newval, olddata)) == -1
         && i < 4;
         ++i)
        if (!grow_hashtable(h, h->wpd.neighborhood_len)) {
            rc = -1;
            break;
        }

    if (rc <= 0)
        free_value(newval);

out:
    return rc;
}

HT_VALUE *ossl_ht_get(HT *h, HT_KEY *key)
{
    struct ht_mutable_data_st *md;
    uint64_t hash;
    uint64_t neigh_idx_start;
    uint64_t neigh_idx;
    struct ht_internal_value_st *ival = NULL;
    size_t j;
    uint64_t ehash;
    int lockless_reads = h->config.lockless_reads;

    hash = h->config.ht_hash_fn(key->keybuf, key->keysize);

    md = ossl_rcu_deref(&h->md);
    neigh_idx = neigh_idx_start = hash & md->neighborhood_mask;
    do {
        PREFETCH_NEIGHBORHOOD(md->neighborhoods[neigh_idx]);
        for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
            ival = ossl_rcu_deref(&md->neighborhoods[neigh_idx].entries[j].value);
            if (ival == NULL) {
                if (lockless_reads)
                    /* lockless_reads implies no deletion, we can break out */
                    return NULL;
                continue;
            }
            if (!CRYPTO_atomic_load(&md->neighborhoods[neigh_idx].entries[j].hash,
                                    &ehash, h->atomic_lock))
                return NULL;
            if (compare_hash(hash, ehash) && match_key(&ival->value.key, key))
                return (HT_VALUE *)ival;
        }
        if (!lockless_reads)
            break;
        /* Continue search in subsequent neighborhoods */
        neigh_idx = (neigh_idx + 1) & md->neighborhood_mask;
    } while (neigh_idx != neigh_idx_start);

    return NULL;
}

static void free_old_entry(void *arg)
{
    struct ht_internal_value_st *v = arg;

    v->ht->config.ht_free_fn((HT_VALUE *)v);
    free_value(v);
}

int ossl_ht_delete(HT *h, HT_KEY *key)
{
    uint64_t hash;
    uint64_t neigh_idx;
    size_t j;
    struct ht_internal_value_st *v = NULL;
    HT_VALUE *nv = NULL;
    int rc = 0;

    if (h->config.lockless_reads)
        return 0;

    hash = h->config.ht_hash_fn(key->keybuf, key->keysize);

    neigh_idx = hash & h->md->neighborhood_mask;
    PREFETCH_NEIGHBORHOOD(h->md->neighborhoods[neigh_idx]);
    for (j = 0; j < NEIGHBORHOOD_LEN; j++) {
        v = (struct ht_internal_value_st *)h->md->neighborhoods[neigh_idx].entries[j].value;
        if (v == NULL)
            continue;
        if (compare_hash(hash, h->md->neighborhoods[neigh_idx].entries[j].hash)
            && match_key(key, &v->value.key)) {
            if (!CRYPTO_atomic_store(&h->md->neighborhoods[neigh_idx].entries[j].hash,
                                     0, h->atomic_lock))
                break;
            h->wpd.value_count--;
            ossl_rcu_assign_ptr(&h->md->neighborhoods[neigh_idx].entries[j].value,
                                &nv);
            rc = 1;
            break;
        }
    }
    if (rc == 1) {
        ossl_rcu_call(h->lock, free_old_entry, v);
        h->wpd.need_sync = 1;
    }
    return rc;
}
