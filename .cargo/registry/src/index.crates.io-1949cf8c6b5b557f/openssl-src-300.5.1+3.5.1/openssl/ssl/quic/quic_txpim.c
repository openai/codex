/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_txpim.h"
#include <stdlib.h>

typedef struct quic_txpim_pkt_ex_st QUIC_TXPIM_PKT_EX;

struct quic_txpim_pkt_ex_st {
    QUIC_TXPIM_PKT              public;
    QUIC_TXPIM_PKT_EX          *prev, *next;
    QUIC_TXPIM_CHUNK           *chunks;
    size_t                      num_chunks, alloc_chunks;
    unsigned int                chunks_need_sort : 1;
};

typedef struct quic_txpim_pkt_ex_list {
    QUIC_TXPIM_PKT_EX          *head, *tail;
} QUIC_TXPIM_PKT_EX_LIST;

struct quic_txpim_st {
    QUIC_TXPIM_PKT_EX_LIST  free_list;
    size_t                  in_use;
};

#define MAX_ALLOC_CHUNKS 512

QUIC_TXPIM *ossl_quic_txpim_new(void)
{
    QUIC_TXPIM *txpim = OPENSSL_zalloc(sizeof(*txpim));

    if (txpim == NULL)
        return NULL;

    return txpim;
}

static void free_list(QUIC_TXPIM_PKT_EX_LIST *l)
{
    QUIC_TXPIM_PKT_EX *n, *nnext;

    for (n = l->head; n != NULL; n = nnext) {
        nnext = n->next;

        OPENSSL_free(n->chunks);
        OPENSSL_free(n);
    }

    l->head = l->tail = NULL;
}

void ossl_quic_txpim_free(QUIC_TXPIM *txpim)
{
    if (txpim == NULL)
        return;

    assert(txpim->in_use == 0);
    free_list(&txpim->free_list);
    OPENSSL_free(txpim);
}

static void list_remove(QUIC_TXPIM_PKT_EX_LIST *l, QUIC_TXPIM_PKT_EX *n)
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

static void list_insert_tail(QUIC_TXPIM_PKT_EX_LIST *l, QUIC_TXPIM_PKT_EX *n)
{
    n->prev = l->tail;
    n->next = NULL;
    l->tail = n;
    if (n->prev != NULL)
        n->prev->next = n;
    if (l->head == NULL)
        l->head = n;
}

static QUIC_TXPIM_PKT_EX *txpim_get_free(QUIC_TXPIM *txpim)
{
    QUIC_TXPIM_PKT_EX *ex = txpim->free_list.head;

    if (ex != NULL)
        return ex;

    ex = OPENSSL_zalloc(sizeof(*ex));
    if (ex == NULL)
        return NULL;

    list_insert_tail(&txpim->free_list, ex);
    return ex;
}

static void txpim_clear(QUIC_TXPIM_PKT_EX *ex)
{
    memset(&ex->public.ackm_pkt, 0, sizeof(ex->public.ackm_pkt));
    ossl_quic_txpim_pkt_clear_chunks(&ex->public);
    ex->public.retx_head                   = NULL;
    ex->public.fifd                        = NULL;
    ex->public.had_handshake_done_frame    = 0;
    ex->public.had_max_data_frame          = 0;
    ex->public.had_max_streams_bidi_frame  = 0;
    ex->public.had_max_streams_uni_frame   = 0;
    ex->public.had_ack_frame               = 0;
    ex->public.had_conn_close              = 0;
}

QUIC_TXPIM_PKT *ossl_quic_txpim_pkt_alloc(QUIC_TXPIM *txpim)
{
    QUIC_TXPIM_PKT_EX *ex = txpim_get_free(txpim);

    if (ex == NULL)
        return NULL;

    txpim_clear(ex);
    list_remove(&txpim->free_list, ex);
    ++txpim->in_use;
    return &ex->public;
}

void ossl_quic_txpim_pkt_release(QUIC_TXPIM *txpim, QUIC_TXPIM_PKT *fpkt)
{
    QUIC_TXPIM_PKT_EX *ex = (QUIC_TXPIM_PKT_EX *)fpkt;

    assert(txpim->in_use > 0);
    --txpim->in_use;
    list_insert_tail(&txpim->free_list, ex);
}

void ossl_quic_txpim_pkt_add_cfq_item(QUIC_TXPIM_PKT *fpkt,
                                      QUIC_CFQ_ITEM *item)
{
    item->pkt_next = fpkt->retx_head;
    item->pkt_prev = NULL;
    fpkt->retx_head = item;
}

void ossl_quic_txpim_pkt_clear_chunks(QUIC_TXPIM_PKT *fpkt)
{
    QUIC_TXPIM_PKT_EX *ex = (QUIC_TXPIM_PKT_EX *)fpkt;

    ex->num_chunks = 0;
}

int ossl_quic_txpim_pkt_append_chunk(QUIC_TXPIM_PKT *fpkt,
                                     const QUIC_TXPIM_CHUNK *chunk)
{
    QUIC_TXPIM_PKT_EX *ex = (QUIC_TXPIM_PKT_EX *)fpkt;
    QUIC_TXPIM_CHUNK *new_chunk;
    size_t new_alloc_chunks = ex->alloc_chunks;

    if (ex->num_chunks == ex->alloc_chunks) {
        new_alloc_chunks = (ex->alloc_chunks == 0) ? 4 : ex->alloc_chunks * 8 / 5;
        if (new_alloc_chunks > MAX_ALLOC_CHUNKS)
            new_alloc_chunks = MAX_ALLOC_CHUNKS;
        if (ex->num_chunks == new_alloc_chunks)
            return 0;

        new_chunk = OPENSSL_realloc(ex->chunks,
                                    new_alloc_chunks * sizeof(QUIC_TXPIM_CHUNK));
        if (new_chunk == NULL)
            return 0;

        ex->chunks          = new_chunk;
        ex->alloc_chunks    = new_alloc_chunks;
    }

    ex->chunks[ex->num_chunks++]    = *chunk;
    ex->chunks_need_sort            = 1;
    return 1;
}

static int compare(const void *a, const void *b)
{
    const QUIC_TXPIM_CHUNK *ac = a, *bc = b;

    if (ac->stream_id < bc->stream_id)
        return -1;
    else if (ac->stream_id > bc->stream_id)
        return 1;

    if (ac->start < bc->start)
        return -1;
    else if (ac->start > bc->start)
        return 1;

    return 0;
}

const QUIC_TXPIM_CHUNK *ossl_quic_txpim_pkt_get_chunks(const QUIC_TXPIM_PKT *fpkt)
{
    QUIC_TXPIM_PKT_EX *ex = (QUIC_TXPIM_PKT_EX *)fpkt;

    if (ex->chunks_need_sort) {
        /*
         * List of chunks will generally be very small so there is no issue
         * simply sorting here.
         */
        qsort(ex->chunks, ex->num_chunks, sizeof(QUIC_TXPIM_CHUNK), compare);
        ex->chunks_need_sort = 0;
    }

    return ex->chunks;
}

size_t ossl_quic_txpim_pkt_get_num_chunks(const QUIC_TXPIM_PKT *fpkt)
{
    QUIC_TXPIM_PKT_EX *ex = (QUIC_TXPIM_PKT_EX *)fpkt;

    return ex->num_chunks;
}

size_t ossl_quic_txpim_get_in_use(const QUIC_TXPIM *txpim)
{
    return txpim->in_use;
}
