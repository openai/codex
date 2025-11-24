/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_cfq.h"
#include "internal/numbers.h"

typedef struct quic_cfq_item_ex_st QUIC_CFQ_ITEM_EX;

struct quic_cfq_item_ex_st {
    QUIC_CFQ_ITEM           public;
    QUIC_CFQ_ITEM_EX       *prev, *next;
    unsigned char          *encoded;
    cfq_free_cb            *free_cb;
    void                   *free_cb_arg;
    uint64_t                frame_type;
    size_t                  encoded_len;
    uint32_t                priority, pn_space, flags;
    int                     state;
};

uint64_t ossl_quic_cfq_item_get_frame_type(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return ex->frame_type;
}

const unsigned char *ossl_quic_cfq_item_get_encoded(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return ex->encoded;
}

size_t ossl_quic_cfq_item_get_encoded_len(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return ex->encoded_len;
}

int ossl_quic_cfq_item_get_state(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return ex->state;
}

uint32_t ossl_quic_cfq_item_get_pn_space(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return ex->pn_space;
}

int ossl_quic_cfq_item_is_unreliable(const QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    return (ex->flags & QUIC_CFQ_ITEM_FLAG_UNRELIABLE) != 0;
}

typedef struct quic_cfq_item_list_st {
    QUIC_CFQ_ITEM_EX *head, *tail;
} QUIC_CFQ_ITEM_LIST;

struct quic_cfq_st {
    /*
     * Invariant: A CFQ item is always in exactly one of these lists, never more
     * or less than one.
     *
     * Invariant: The list the CFQ item is determined exactly by the state field
     * of the item.
     */
    QUIC_CFQ_ITEM_LIST                      new_list, tx_list, free_list;
};

static int compare(const QUIC_CFQ_ITEM_EX *a, const QUIC_CFQ_ITEM_EX *b)
{
    if (a->pn_space < b->pn_space)
        return -1;
    else if (a->pn_space > b->pn_space)
        return 1;

    if (a->priority > b->priority)
        return -1;
    else if (a->priority < b->priority)
        return 1;

    return 0;
}

static void list_remove(QUIC_CFQ_ITEM_LIST *l, QUIC_CFQ_ITEM_EX *n)
{
    if (l->head == n)
        l->head = n->next;
    if (l->tail == n)
        l->tail = n->prev;
    if (n->prev != NULL)
        n->prev->next = n->next;
    if (n->next != NULL)
        n->next->prev = n->prev;
    n->prev = n->next = NULL;
}

static void list_insert_head(QUIC_CFQ_ITEM_LIST *l, QUIC_CFQ_ITEM_EX *n)
{
    n->next = l->head;
    n->prev = NULL;
    l->head = n;
    if (n->next != NULL)
        n->next->prev = n;
    if (l->tail == NULL)
        l->tail = n;
}

static void list_insert_tail(QUIC_CFQ_ITEM_LIST *l, QUIC_CFQ_ITEM_EX *n)
{
    n->prev = l->tail;
    n->next = NULL;
    l->tail = n;
    if (n->prev != NULL)
        n->prev->next = n;
    if (l->head == NULL)
        l->head = n;
}

static void list_insert_after(QUIC_CFQ_ITEM_LIST *l,
                              QUIC_CFQ_ITEM_EX *ref,
                              QUIC_CFQ_ITEM_EX *n)
{
    n->prev = ref;
    n->next = ref->next;
    if (ref->next != NULL)
        ref->next->prev = n;
    ref->next = n;
    if (l->tail == ref)
        l->tail = n;
}

static void list_insert_sorted(QUIC_CFQ_ITEM_LIST *l, QUIC_CFQ_ITEM_EX *n,
                               int (*cmp)(const QUIC_CFQ_ITEM_EX *a,
                                          const QUIC_CFQ_ITEM_EX *b))
{
    QUIC_CFQ_ITEM_EX *p = l->head, *pprev = NULL;

    if (p == NULL) {
        l->head = l->tail = n;
        n->prev = n->next = NULL;
        return;
    }

    for (; p != NULL && cmp(p, n) < 0; pprev = p, p = p->next);

    if (p == NULL)
        list_insert_tail(l, n);
    else if (pprev == NULL)
        list_insert_head(l, n);
    else
        list_insert_after(l, pprev, n);
}

QUIC_CFQ *ossl_quic_cfq_new(void)
{
    QUIC_CFQ *cfq = OPENSSL_zalloc(sizeof(*cfq));

    if (cfq == NULL)
        return NULL;

    return cfq;
}

static void clear_item(QUIC_CFQ_ITEM_EX *item)
{
    if (item->free_cb != NULL) {
        item->free_cb(item->encoded, item->encoded_len, item->free_cb_arg);

        item->free_cb       = NULL;
        item->encoded       = NULL;
        item->encoded_len   = 0;
    }

    item->state = -1;
}

static void free_list_items(QUIC_CFQ_ITEM_LIST *l)
{
    QUIC_CFQ_ITEM_EX *p, *pnext;

    for (p = l->head; p != NULL; p = pnext) {
        pnext = p->next;
        clear_item(p);
        OPENSSL_free(p);
    }
}

void ossl_quic_cfq_free(QUIC_CFQ *cfq)
{
    if (cfq == NULL)
        return;

    free_list_items(&cfq->new_list);
    free_list_items(&cfq->tx_list);
    free_list_items(&cfq->free_list);
    OPENSSL_free(cfq);
}

static QUIC_CFQ_ITEM_EX *cfq_get_free(QUIC_CFQ *cfq)
{
    QUIC_CFQ_ITEM_EX *item = cfq->free_list.head;

    if (item != NULL)
        return item;

    item = OPENSSL_zalloc(sizeof(*item));
    if (item == NULL)
        return NULL;

    item->state = -1;
    list_insert_tail(&cfq->free_list, item);
    return item;
}

QUIC_CFQ_ITEM *ossl_quic_cfq_add_frame(QUIC_CFQ            *cfq,
                                       uint32_t             priority,
                                       uint32_t             pn_space,
                                       uint64_t             frame_type,
                                       uint32_t             flags,
                                       const unsigned char *encoded,
                                       size_t               encoded_len,
                                       cfq_free_cb         *free_cb,
                                       void                *free_cb_arg)
{
    QUIC_CFQ_ITEM_EX *item = cfq_get_free(cfq);

    if (item == NULL)
        return NULL;

    item->priority      = priority;
    item->frame_type    = frame_type;
    item->pn_space      = pn_space;
    item->encoded       = (unsigned char *)encoded;
    item->encoded_len   = encoded_len;
    item->free_cb       = free_cb;
    item->free_cb_arg   = free_cb_arg;

    item->state = QUIC_CFQ_STATE_NEW;
    item->flags = flags;
    list_remove(&cfq->free_list, item);
    list_insert_sorted(&cfq->new_list, item, compare);
    return &item->public;
}

void ossl_quic_cfq_mark_tx(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    switch (ex->state) {
    case QUIC_CFQ_STATE_NEW:
        list_remove(&cfq->new_list, ex);
        list_insert_tail(&cfq->tx_list, ex);
        ex->state = QUIC_CFQ_STATE_TX;
        break;
    case QUIC_CFQ_STATE_TX:
        break; /* nothing to do */
    default:
        assert(0); /* invalid state (e.g. in free state) */
        break;
    }
}

void ossl_quic_cfq_mark_lost(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item,
                             uint32_t priority)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    if (ossl_quic_cfq_item_is_unreliable(item)) {
        ossl_quic_cfq_release(cfq, item);
        return;
    }

    switch (ex->state) {
    case QUIC_CFQ_STATE_NEW:
        if (priority != UINT32_MAX && priority != ex->priority) {
            list_remove(&cfq->new_list, ex);
            ex->priority = priority;
            list_insert_sorted(&cfq->new_list, ex, compare);
        }
        break; /* nothing to do */
    case QUIC_CFQ_STATE_TX:
        if (priority != UINT32_MAX)
            ex->priority = priority;
        list_remove(&cfq->tx_list, ex);
        list_insert_sorted(&cfq->new_list, ex, compare);
        ex->state = QUIC_CFQ_STATE_NEW;
        break;
    default:
        assert(0); /* invalid state (e.g. in free state) */
        break;
    }
}

/*
 * Releases a CFQ item. The item may be in either state (NEW or TX) prior to the
 * call. The QUIC_CFQ_ITEM pointer must not be used following this call.
 */
void ossl_quic_cfq_release(QUIC_CFQ *cfq, QUIC_CFQ_ITEM *item)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    switch (ex->state) {
    case QUIC_CFQ_STATE_NEW:
        list_remove(&cfq->new_list, ex);
        list_insert_tail(&cfq->free_list, ex);
        clear_item(ex);
        break;
    case QUIC_CFQ_STATE_TX:
        list_remove(&cfq->tx_list, ex);
        list_insert_tail(&cfq->free_list, ex);
        clear_item(ex);
        break;
    default:
        assert(0); /* invalid state (e.g. in free state) */
        break;
    }
}

QUIC_CFQ_ITEM *ossl_quic_cfq_get_priority_head(const QUIC_CFQ *cfq,
                                               uint32_t pn_space)
{
    QUIC_CFQ_ITEM_EX *item = cfq->new_list.head;

    for (; item != NULL && item->pn_space != pn_space; item = item->next);

    if (item == NULL)
        return NULL;

    return &item->public;
}

QUIC_CFQ_ITEM *ossl_quic_cfq_item_get_priority_next(const QUIC_CFQ_ITEM *item,
                                                    uint32_t pn_space)
{
    QUIC_CFQ_ITEM_EX *ex = (QUIC_CFQ_ITEM_EX *)item;

    if (ex == NULL)
        return NULL;

     ex = ex->next;

     for (; ex != NULL && ex->pn_space != pn_space; ex = ex->next);

     if (ex == NULL)
         return NULL; /* ubsan */

     return &ex->public;
}
