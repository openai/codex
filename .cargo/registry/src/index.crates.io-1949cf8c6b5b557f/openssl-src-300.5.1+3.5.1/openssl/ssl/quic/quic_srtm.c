/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_srtm.h"
#include "internal/common.h"
#include <openssl/lhash.h>
#include <openssl/core_names.h>
#include <openssl/rand.h>

/*
 * QUIC Stateless Reset Token Manager
 * ==================================
 */
typedef struct srtm_item_st SRTM_ITEM;

#define BLINDED_SRT_LEN     16

DEFINE_LHASH_OF_EX(SRTM_ITEM);

/*
 * The SRTM is implemented using two LHASH instances, one matching opaque pointers to
 * an item structure, and another matching a SRT-derived value to an item
 * structure. Multiple items with different seq_num values under a given opaque,
 * and duplicate SRTs, are handled using sorted singly-linked lists.
 *
 * The O(n) insert and lookup performance is tolerated on the basis that the
 * total number of entries for a given opaque (total number of extant CIDs for a
 * connection) should be quite small, and the QUIC protocol allows us to place a
 * hard limit on this via the active_connection_id_limit TPARAM. Thus there is
 * no risk of a large number of SRTs needing to be registered under a given
 * opaque.
 *
 * It is expected one SRTM will exist per QUIC_PORT and track all SRTs across
 * all connections for that QUIC_PORT.
 */
struct srtm_item_st {
    SRTM_ITEM                   *next_by_srt_blinded; /* SORT BY opaque  DESC */
    SRTM_ITEM                   *next_by_seq_num;     /* SORT BY seq_num DESC */
    void                        *opaque; /* \__ unique identity for item */
    uint64_t                    seq_num; /* /                            */
    QUIC_STATELESS_RESET_TOKEN  srt;
    unsigned char               srt_blinded[BLINDED_SRT_LEN]; /* H(srt) */

#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    uint32_t                    debug_token;
#endif
};

struct quic_srtm_st {
    /* Crypto context used to calculate blinded SRTs H(srt). */
    EVP_CIPHER_CTX              *blind_ctx; /* kept with key */

    LHASH_OF(SRTM_ITEM)         *items_fwd; /* (opaque)  -> SRTM_ITEM */
    LHASH_OF(SRTM_ITEM)         *items_rev; /* (H(srt))  -> SRTM_ITEM */

    /*
     * Monotonically transitions to 1 in event of allocation failure. The only
     * valid operation on such an object is to free it.
     */
    unsigned int                alloc_failed : 1;
};

static unsigned long items_fwd_hash(const SRTM_ITEM *item)
{
    return (unsigned long)(uintptr_t)item->opaque;
}

static int items_fwd_cmp(const SRTM_ITEM *a, const SRTM_ITEM *b)
{
    return a->opaque != b->opaque;
}

static unsigned long items_rev_hash(const SRTM_ITEM *item)
{
    /*
     * srt_blinded has already been through a crypto-grade hash function, so we
     * can just use bits from that.
     */
    unsigned long l;

    memcpy(&l, item->srt_blinded, sizeof(l));
    return l;
}

static int items_rev_cmp(const SRTM_ITEM *a, const SRTM_ITEM *b)
{
    /*
     * We don't need to use CRYPTO_memcmp here as the relationship of
     * srt_blinded to srt is already cryptographically obfuscated.
     */
    return memcmp(a->srt_blinded, b->srt_blinded, sizeof(a->srt_blinded));
}

static int srtm_check_lh(QUIC_SRTM *srtm, LHASH_OF(SRTM_ITEM) *lh)
{
    if (lh_SRTM_ITEM_error(lh)) {
        srtm->alloc_failed = 1;
        return 0;
    }

    return 1;
}

QUIC_SRTM *ossl_quic_srtm_new(OSSL_LIB_CTX *libctx, const char *propq)
{
    QUIC_SRTM *srtm = NULL;
    unsigned char key[16];
    EVP_CIPHER *ecb = NULL;

    if (RAND_priv_bytes_ex(libctx, key, sizeof(key), sizeof(key) * 8) != 1)
        goto err;

    if ((srtm = OPENSSL_zalloc(sizeof(*srtm))) == NULL)
        return NULL;

    /* Use AES-128-ECB as a permutation over 128-bit SRTs. */
    if ((ecb = EVP_CIPHER_fetch(libctx, "AES-128-ECB", propq)) == NULL)
        goto err;

    if ((srtm->blind_ctx = EVP_CIPHER_CTX_new()) == NULL)
        goto err;

    if (!EVP_EncryptInit_ex2(srtm->blind_ctx, ecb, key, NULL, NULL))
        goto err;

    EVP_CIPHER_free(ecb);
    ecb = NULL;

    /* Create mappings. */
    if ((srtm->items_fwd = lh_SRTM_ITEM_new(items_fwd_hash, items_fwd_cmp)) == NULL
        || (srtm->items_rev = lh_SRTM_ITEM_new(items_rev_hash, items_rev_cmp)) == NULL)
        goto err;

    return srtm;

err:
    /*
     * No cleansing of key needed as blinding exists only for side channel
     * mitigation.
     */
    ossl_quic_srtm_free(srtm);
    EVP_CIPHER_free(ecb);
    return NULL;
}

static void srtm_free_each(SRTM_ITEM *ihead)
{
    SRTM_ITEM *inext, *item = ihead;

    for (item = item->next_by_seq_num; item != NULL; item = inext) {
        inext = item->next_by_seq_num;
        OPENSSL_free(item);
    }

    OPENSSL_free(ihead);
}

void ossl_quic_srtm_free(QUIC_SRTM *srtm)
{
    if (srtm == NULL)
        return;

    lh_SRTM_ITEM_free(srtm->items_rev);
    if (srtm->items_fwd != NULL) {
        lh_SRTM_ITEM_doall(srtm->items_fwd, srtm_free_each);
        lh_SRTM_ITEM_free(srtm->items_fwd);
    }

    EVP_CIPHER_CTX_free(srtm->blind_ctx);
    OPENSSL_free(srtm);
}

/*
 * Find a SRTM_ITEM by (opaque, seq_num). Returns NULL if no match.
 * If head is non-NULL, writes the head of the relevant opaque list to *head if
 * there is one.
 * If prev is non-NULL, writes the previous node to *prev or NULL if it is
 * the first item.
 */
static SRTM_ITEM *srtm_find(QUIC_SRTM *srtm, void *opaque, uint64_t seq_num,
                            SRTM_ITEM **head_p, SRTM_ITEM **prev_p)
{
    SRTM_ITEM key, *item = NULL, *prev = NULL;

    key.opaque  = opaque;

    item = lh_SRTM_ITEM_retrieve(srtm->items_fwd, &key);
    if (head_p != NULL)
        *head_p = item;

    for (; item != NULL; prev = item, item = item->next_by_seq_num)
        if (item->seq_num == seq_num) {
            break;
        } else if (item->seq_num < seq_num) {
            /*
             * List is sorted in descending order so there can't be any match
             * after this.
             */
            item = NULL;
            break;
        }

    if (prev_p != NULL)
        *prev_p = prev;

    return item;
}

/*
 * Inserts a SRTM_ITEM into the singly-linked by-sequence-number linked list.
 * The new head pointer is written to *new_head (which may or may not be
 * unchanged).
 */
static void sorted_insert_seq_num(SRTM_ITEM *head, SRTM_ITEM *item, SRTM_ITEM **new_head)
{
    uint64_t seq_num = item->seq_num;
    SRTM_ITEM *cur = head, **fixup = new_head;

    *new_head = head;

    while (cur != NULL && cur->seq_num > seq_num) {
        fixup = &cur->next_by_seq_num;
        cur = cur->next_by_seq_num;
    }

    item->next_by_seq_num = *fixup;
    *fixup = item;
}

/*
 * Inserts a SRTM_ITEM into the singly-linked by-SRT list.
 * The new head pointer is written to *new_head (which may or may not be
 * unchanged).
 */
static void sorted_insert_srt(SRTM_ITEM *head, SRTM_ITEM *item, SRTM_ITEM **new_head)
{
    uintptr_t opaque = (uintptr_t)item->opaque;
    SRTM_ITEM *cur = head, **fixup = new_head;

    *new_head = head;

    while (cur != NULL && (uintptr_t)cur->opaque > opaque) {
        fixup = &cur->next_by_srt_blinded;
        cur = cur->next_by_srt_blinded;
    }

    item->next_by_srt_blinded = *fixup;
    *fixup = item;
}

/*
 * Computes the blinded SRT value used for internal lookup for side channel
 * mitigation purposes. We compute this once as a cached value when an SRTM_ITEM
 * is formed.
 */
static int srtm_compute_blinded(QUIC_SRTM *srtm, SRTM_ITEM *item,
                                const QUIC_STATELESS_RESET_TOKEN *token)
{
    int outl = 0;

    /*
     * We use AES-128-ECB as a permutation using a random key to facilitate
     * blinding for side-channel purposes. Encrypt the token as a single AES
     * block.
     */
    if (!EVP_EncryptUpdate(srtm->blind_ctx, item->srt_blinded, &outl,
                           (const unsigned char *)token, sizeof(*token)))
        return 0;

    if (!ossl_assert(outl == sizeof(*token)))
        return 0;

    return 1;
}

int ossl_quic_srtm_add(QUIC_SRTM *srtm, void *opaque, uint64_t seq_num,
                       const QUIC_STATELESS_RESET_TOKEN *token)
{
    SRTM_ITEM *item = NULL, *head = NULL, *new_head, *r_item;

    if (srtm->alloc_failed)
        return 0;

    /* (opaque, seq_num) duplicates not allowed */
    if ((item = srtm_find(srtm, opaque, seq_num, &head, NULL)) != NULL)
        return 0;

    if ((item = OPENSSL_zalloc(sizeof(*item))) == NULL)
        return 0;

    item->opaque    = opaque;
    item->seq_num   = seq_num;
    item->srt       = *token;
    if (!srtm_compute_blinded(srtm, item, &item->srt)) {
        OPENSSL_free(item);
        return 0;
    }

    /* Add to forward mapping. */
    if (head == NULL) {
        /* First item under this opaque */
        lh_SRTM_ITEM_insert(srtm->items_fwd, item);
        if (!srtm_check_lh(srtm, srtm->items_fwd)) {
            OPENSSL_free(item);
            return 0;
        }
    } else {
        sorted_insert_seq_num(head, item, &new_head);
        if (new_head != head) { /* head changed, update in lhash */
            lh_SRTM_ITEM_insert(srtm->items_fwd, new_head);
            if (!srtm_check_lh(srtm, srtm->items_fwd)) {
                OPENSSL_free(item);
                return 0;
            }
        }
    }

    /* Add to reverse mapping. */
    r_item = lh_SRTM_ITEM_retrieve(srtm->items_rev, item);
    if (r_item == NULL) {
        /* First item under this blinded SRT */
        lh_SRTM_ITEM_insert(srtm->items_rev, item);
        if (!srtm_check_lh(srtm, srtm->items_rev))
            /*
             * Can't free the item now as we would have to undo the insertion
             * into the forward mapping which would require an insert operation
             * to restore the previous value. which might also fail. However,
             * the item will be freed OK when we free the entire SRTM.
             */
            return 0;
    } else {
        sorted_insert_srt(r_item, item, &new_head);
        if (new_head != r_item) { /* head changed, update in lhash */
            lh_SRTM_ITEM_insert(srtm->items_rev, new_head);
            if (!srtm_check_lh(srtm, srtm->items_rev))
                /* As above. */
                return 0;
        }
    }

    return 1;
}

/* Remove item from reverse mapping. */
static int srtm_remove_from_rev(QUIC_SRTM *srtm, SRTM_ITEM *item)
{
    SRTM_ITEM *rh_item;

    rh_item = lh_SRTM_ITEM_retrieve(srtm->items_rev, item);
    assert(rh_item != NULL);
    if (rh_item == item) {
        /*
         * Change lhash to point to item after this one, or remove the entry if
         * this is the last one.
         */
        if (item->next_by_srt_blinded != NULL) {
            lh_SRTM_ITEM_insert(srtm->items_rev, item->next_by_srt_blinded);
            if (!srtm_check_lh(srtm, srtm->items_rev))
                return 0;
        } else {
            lh_SRTM_ITEM_delete(srtm->items_rev, item);
        }
    } else {
        /* Find our entry in the SRT list */
        for (; rh_item->next_by_srt_blinded != item;
               rh_item = rh_item->next_by_srt_blinded);
        rh_item->next_by_srt_blinded = item->next_by_srt_blinded;
    }

    return 1;
}

int ossl_quic_srtm_remove(QUIC_SRTM *srtm, void *opaque, uint64_t seq_num)
{
    SRTM_ITEM *item, *prev = NULL;

    if (srtm->alloc_failed)
        return 0;

    if ((item = srtm_find(srtm, opaque, seq_num, NULL, &prev)) == NULL)
        /* No match */
        return 0;

    /* Remove from forward mapping. */
    if (prev == NULL) {
        /*
         * Change lhash to point to item after this one, or remove the entry if
         * this is the last one.
         */
        if (item->next_by_seq_num != NULL) {
            lh_SRTM_ITEM_insert(srtm->items_fwd, item->next_by_seq_num);
            if (!srtm_check_lh(srtm, srtm->items_fwd))
                return 0;
        } else {
            lh_SRTM_ITEM_delete(srtm->items_fwd, item);
        }
    } else {
        prev->next_by_seq_num = item->next_by_seq_num;
    }

    /* Remove from reverse mapping. */
    if (!srtm_remove_from_rev(srtm, item))
        return 0;

    OPENSSL_free(item);
    return 1;
}

int ossl_quic_srtm_cull(QUIC_SRTM *srtm, void *opaque)
{
    SRTM_ITEM key, *item = NULL, *inext, *ihead;

    key.opaque = opaque;

    if (srtm->alloc_failed)
        return 0;

    if ((ihead = lh_SRTM_ITEM_retrieve(srtm->items_fwd, &key)) == NULL)
        return 1; /* nothing removed is a success condition */

    for (item = ihead; item != NULL; item = inext) {
        inext = item->next_by_seq_num;
        if (item != ihead) {
            srtm_remove_from_rev(srtm, item);
            OPENSSL_free(item);
        }
    }

    lh_SRTM_ITEM_delete(srtm->items_fwd, ihead);
    srtm_remove_from_rev(srtm, ihead);
    OPENSSL_free(ihead);
    return 1;
}

int ossl_quic_srtm_lookup(QUIC_SRTM *srtm,
                          const QUIC_STATELESS_RESET_TOKEN *token,
                          size_t idx,
                          void **opaque, uint64_t *seq_num)
{
    SRTM_ITEM key, *item;

    if (srtm->alloc_failed)
        return 0;

    if (!srtm_compute_blinded(srtm, &key, token))
        return 0;

    item = lh_SRTM_ITEM_retrieve(srtm->items_rev, &key);
    for (; idx > 0 && item != NULL; --idx, item = item->next_by_srt_blinded);
    if (item == NULL)
        return 0;

    if (opaque != NULL)
        *opaque     = item->opaque;
    if (seq_num != NULL)
        *seq_num    = item->seq_num;

    return 1;
}

#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION

static uint32_t token_next = 0x5eadbeef;
static size_t tokens_seen;

struct check_args {
    uint32_t token;
    int      mode;
};

static void check_mark(SRTM_ITEM *item, void *arg)
{
    struct check_args *arg_ = arg;
    uint32_t token = arg_->token;
    uint64_t prev_seq_num = 0;
    void *prev_opaque = NULL;
    int have_prev = 0;

    assert(item != NULL);

    while (item != NULL) {
        if (have_prev) {
            assert(!(item->opaque == prev_opaque && item->seq_num == prev_seq_num));
            if (!arg_->mode)
                assert(item->opaque != prev_opaque || item->seq_num < prev_seq_num);
        }

        ++tokens_seen;
        item->debug_token = token;
        prev_opaque  = item->opaque;
        prev_seq_num = item->seq_num;
        have_prev = 1;

        if (arg_->mode)
            item = item->next_by_srt_blinded;
        else
            item = item->next_by_seq_num;
    }
}

static void check_count(SRTM_ITEM *item, void *arg)
{
    struct check_args *arg_ = arg;
    uint32_t token = arg_->token;

    assert(item != NULL);

    while (item != NULL) {
        ++tokens_seen;
        assert(item->debug_token == token);

        if (arg_->mode)
            item = item->next_by_seq_num;
        else
            item = item->next_by_srt_blinded;
    }
}

#endif

void ossl_quic_srtm_check(const QUIC_SRTM *srtm)
{
#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    struct check_args args = {0};
    size_t tokens_expected, tokens_expected_old;

    args.token = token_next;
    ++token_next;

    assert(srtm != NULL);
    assert(srtm->blind_ctx != NULL);
    assert(srtm->items_fwd != NULL);
    assert(srtm->items_rev != NULL);

    tokens_seen = 0;
    lh_SRTM_ITEM_doall_arg(srtm->items_fwd, check_mark, &args);

    tokens_expected = tokens_seen;
    tokens_seen = 0;
    lh_SRTM_ITEM_doall_arg(srtm->items_rev, check_count, &args);

    assert(tokens_seen == tokens_expected);
    tokens_expected_old = tokens_expected;

    args.token = token_next;
    ++token_next;

    args.mode = 1;
    tokens_seen = 0;
    lh_SRTM_ITEM_doall_arg(srtm->items_rev, check_mark, &args);

    tokens_expected = tokens_seen;
    tokens_seen = 0;
    lh_SRTM_ITEM_doall_arg(srtm->items_fwd, check_count, &args);

    assert(tokens_seen == tokens_expected);
    assert(tokens_seen == tokens_expected_old);
#endif
}
