/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/ssl.h>
#include "internal/quic_record_rx.h"
#include "quic_record_shared.h"
#include "internal/common.h"
#include "internal/list.h"
#include "../ssl_local.h"

/*
 * Mark a packet in a bitfield.
 *
 * pkt_idx: index of packet within datagram.
 */
static ossl_inline void pkt_mark(uint64_t *bitf, size_t pkt_idx)
{
    assert(pkt_idx < QUIC_MAX_PKT_PER_URXE);
    *bitf |= ((uint64_t)1) << pkt_idx;
}

/* Returns 1 if a packet is in the bitfield. */
static ossl_inline int pkt_is_marked(const uint64_t *bitf, size_t pkt_idx)
{
    assert(pkt_idx < QUIC_MAX_PKT_PER_URXE);
    return (*bitf & (((uint64_t)1) << pkt_idx)) != 0;
}

/*
 * RXE
 * ===
 *
 * RX Entries (RXEs) store processed (i.e., decrypted) data received from the
 * network. One RXE is used per received QUIC packet.
 */
typedef struct rxe_st RXE;

struct rxe_st {
    OSSL_QRX_PKT        pkt;
    OSSL_LIST_MEMBER(rxe, RXE);
    size_t              data_len, alloc_len, refcount;

    /* Extra fields for per-packet information. */
    QUIC_PKT_HDR        hdr; /* data/len are decrypted payload */

    /* Decoded packet number. */
    QUIC_PN             pn;

    /* Addresses copied from URXE. */
    BIO_ADDR            peer, local;

    /* Time we received the packet (not when we processed it). */
    OSSL_TIME           time;

    /* Total length of the datagram which contained this packet. */
    size_t              datagram_len;

    /*
     * The key epoch the packet was received with. Always 0 for non-1-RTT
     * packets.
     */
    uint64_t            key_epoch;

    /*
     * Monotonically increases with each datagram received.
     * For diagnostic use only.
     */
    uint64_t            datagram_id;

    /*
     * alloc_len allocated bytes (of which data_len bytes are valid) follow this
     * structure.
     */
};

DEFINE_LIST_OF(rxe, RXE);
typedef OSSL_LIST(rxe) RXE_LIST;

static ossl_inline unsigned char *rxe_data(const RXE *e)
{
    return (unsigned char *)(e + 1);
}

/*
 * QRL
 * ===
 */
struct ossl_qrx_st {
    OSSL_LIB_CTX               *libctx;
    const char                 *propq;

    /* Demux to receive datagrams from. */
    QUIC_DEMUX                 *demux;

    /* Length of connection IDs used in short-header packets in bytes. */
    size_t                      short_conn_id_len;

    /* Maximum number of deferred datagrams buffered at any one time. */
    size_t                      max_deferred;

    /* Current count of deferred datagrams. */
    size_t                      num_deferred;

    /*
     * List of URXEs which are filled with received encrypted data.
     * These are returned to the DEMUX's free list as they are processed.
     */
    QUIC_URXE_LIST              urx_pending;

    /*
     * List of URXEs which we could not decrypt immediately and which are being
     * kept in case they can be decrypted later.
     */
    QUIC_URXE_LIST              urx_deferred;

    /*
     * List of RXEs which are not currently in use. These are moved
     * to the pending list as they are filled.
     */
    RXE_LIST                    rx_free;

    /*
     * List of RXEs which are filled with decrypted packets ready to be passed
     * to the user. A RXE is removed from all lists inside the QRL when passed
     * to the user, then returned to the free list when the user returns it.
     */
    RXE_LIST                    rx_pending;

    /* Largest PN we have received and processed in a given PN space. */
    QUIC_PN                     largest_pn[QUIC_PN_SPACE_NUM];

    /* Per encryption-level state. */
    OSSL_QRL_ENC_LEVEL_SET      el_set;

    /* Bytes we have received since this counter was last cleared. */
    uint64_t                    bytes_received;

    /*
     * Number of forged packets we have received since the QRX was instantiated.
     * Note that as per RFC 9001, this is connection-level state; it is not per
     * EL and is not reset by a key update.
     */
    uint64_t                    forged_pkt_count;

    /*
     * The PN the current key epoch started at, inclusive.
     */
    uint64_t                    cur_epoch_start_pn;

    /* Validation callback. */
    ossl_qrx_late_validation_cb    *validation_cb;
    void                           *validation_cb_arg;

    /* Key update callback. */
    ossl_qrx_key_update_cb         *key_update_cb;
    void                           *key_update_cb_arg;

    /* Initial key phase. For debugging use only; always 0 in real use. */
    unsigned char                   init_key_phase_bit;

    /* Are we allowed to process 1-RTT packets yet? */
    unsigned char                   allow_1rtt;

    /* Message callback related arguments */
    ossl_msg_cb msg_callback;
    void *msg_callback_arg;
    SSL *msg_callback_ssl;
};

static RXE *qrx_ensure_free_rxe(OSSL_QRX *qrx, size_t alloc_len);
static int qrx_validate_hdr_early(OSSL_QRX *qrx, RXE *rxe,
                                  const QUIC_CONN_ID *first_dcid);
static int qrx_relocate_buffer(OSSL_QRX *qrx, RXE **prxe, size_t *pi,
                               const unsigned char **pptr, size_t buf_len);
static int qrx_validate_hdr(OSSL_QRX *qrx, RXE *rxe);
static RXE *qrx_reserve_rxe(RXE_LIST *rxl, RXE *rxe, size_t n);
static int qrx_decrypt_pkt_body(OSSL_QRX *qrx, unsigned char *dst,
                                const unsigned char *src,
                                size_t src_len, size_t *dec_len,
                                const unsigned char *aad, size_t aad_len,
                                QUIC_PN pn, uint32_t enc_level,
                                unsigned char key_phase_bit,
                                uint64_t *rx_key_epoch);
static int qrx_validate_hdr_late(OSSL_QRX *qrx, RXE *rxe);
static uint32_t rxe_determine_pn_space(RXE *rxe);
static void ignore_res(int x);

OSSL_QRX *ossl_qrx_new(const OSSL_QRX_ARGS *args)
{
    OSSL_QRX *qrx;
    size_t i;

    if (args->demux == NULL || args->max_deferred == 0)
        return NULL;

    qrx = OPENSSL_zalloc(sizeof(OSSL_QRX));
    if (qrx == NULL)
        return NULL;

    for (i = 0; i < OSSL_NELEM(qrx->largest_pn); ++i)
        qrx->largest_pn[i] = args->init_largest_pn[i];

    qrx->libctx                 = args->libctx;
    qrx->propq                  = args->propq;
    qrx->demux                  = args->demux;
    qrx->short_conn_id_len      = args->short_conn_id_len;
    qrx->init_key_phase_bit     = args->init_key_phase_bit;
    qrx->max_deferred           = args->max_deferred;
    return qrx;
}

static void qrx_cleanup_rxl(RXE_LIST *l)
{
    RXE *e, *enext;

    for (e = ossl_list_rxe_head(l); e != NULL; e = enext) {
        enext = ossl_list_rxe_next(e);
        ossl_list_rxe_remove(l, e);
        OPENSSL_free(e);
    }
}

static void qrx_cleanup_urxl(OSSL_QRX *qrx, QUIC_URXE_LIST *l)
{
    QUIC_URXE *e, *enext;

    for (e = ossl_list_urxe_head(l); e != NULL; e = enext) {
        enext = ossl_list_urxe_next(e);
        ossl_list_urxe_remove(l, e);
        ossl_quic_demux_release_urxe(qrx->demux, e);
    }
}

void ossl_qrx_free(OSSL_QRX *qrx)
{
    uint32_t i;

    if (qrx == NULL)
        return;

    /* Free RXE queue data. */
    qrx_cleanup_rxl(&qrx->rx_free);
    qrx_cleanup_rxl(&qrx->rx_pending);
    qrx_cleanup_urxl(qrx, &qrx->urx_pending);
    qrx_cleanup_urxl(qrx, &qrx->urx_deferred);

    /* Drop keying material and crypto resources. */
    for (i = 0; i < QUIC_ENC_LEVEL_NUM; ++i)
        ossl_qrl_enc_level_set_discard(&qrx->el_set, i);

    OPENSSL_free(qrx);
}

void ossl_qrx_inject_urxe(OSSL_QRX *qrx, QUIC_URXE *urxe)
{
    /* Initialize our own fields inside the URXE and add to the pending list. */
    urxe->processed     = 0;
    urxe->hpr_removed   = 0;
    urxe->deferred      = 0;
    ossl_list_urxe_insert_tail(&qrx->urx_pending, urxe);

    if (qrx->msg_callback != NULL)
        qrx->msg_callback(0, OSSL_QUIC1_VERSION, SSL3_RT_QUIC_DATAGRAM, urxe + 1,
                          urxe->data_len, qrx->msg_callback_ssl,
                          qrx->msg_callback_arg);
}

void ossl_qrx_inject_pkt(OSSL_QRX *qrx, OSSL_QRX_PKT *pkt)
{
    RXE *rxe = (RXE *)pkt;

    /*
     * port_default_packet_handler() uses ossl_qrx_read_pkt()
     * to get pkt. Such packet has refcount 1.
     */
    ossl_qrx_pkt_orphan(pkt);
    if (ossl_assert(rxe->refcount == 0))
        ossl_list_rxe_insert_tail(&qrx->rx_pending, rxe);
}

/*
 * qrx_validate_initial_pkt() is derived from qrx_process_pkt(). Unlike
 * qrx_process_pkt() the qrx_validate_initial_pkt() function can process
 * initial packet only. All other packets should be discarded. This allows
 * port_default_packet_handler() to validate incoming packet. If packet
 * is not valid, then port_default_packet_handler() must discard the
 * packet instead of creating a new channel for it.
 */
static int qrx_validate_initial_pkt(OSSL_QRX *qrx, QUIC_URXE *urxe,
                                    const QUIC_CONN_ID *first_dcid,
                                    size_t datagram_len)
{
    PACKET pkt, orig_pkt;
    RXE *rxe;
    size_t i = 0, aad_len = 0, dec_len = 0;
    const unsigned char *sop;
    unsigned char *dst;
    QUIC_PKT_HDR_PTRS ptrs;
    uint32_t pn_space;
    OSSL_QRL_ENC_LEVEL *el = NULL;
    uint64_t rx_key_epoch = UINT64_MAX;

    if (!PACKET_buf_init(&pkt, ossl_quic_urxe_data(urxe), urxe->data_len))
        return 0;

    orig_pkt = pkt;
    sop = PACKET_data(&pkt);

    /*
     * Get a free RXE. If we need to allocate a new one, use the packet length
     * as a good ballpark figure.
     */
    rxe = qrx_ensure_free_rxe(qrx, PACKET_remaining(&pkt));
    if (rxe == NULL)
        return 0;

    /*
     * we expect INITIAL packet only, therefore it is OK to pass
     * short_conn_id_len as 0.
     */
    if (!ossl_quic_wire_decode_pkt_hdr(&pkt,
                                       0, /* short_conn_id_len */
                                       1, /* need second decode */
                                       0, /* nodata -> want to read data */
                                       &rxe->hdr, &ptrs,
                                       NULL))
        goto malformed;

    if (rxe->hdr.type != QUIC_PKT_TYPE_INITIAL)
        goto malformed;

    if (!qrx_validate_hdr_early(qrx, rxe, NULL))
        goto malformed;

    if (ossl_qrl_enc_level_set_have_el(&qrx->el_set, QUIC_ENC_LEVEL_INITIAL) != 1)
        goto malformed;

    if (rxe->hdr.type == QUIC_PKT_TYPE_INITIAL) {
        const unsigned char *token = rxe->hdr.token;

        /*
         * This may change the value of rxe and change the value of the token
         * pointer as well. So we must make a temporary copy of the pointer to
         * the token, and then copy it back into the new location of the rxe
         */
        if (!qrx_relocate_buffer(qrx, &rxe, &i, &token, rxe->hdr.token_len))
            goto malformed;

        rxe->hdr.token = token;
    }

    pkt = orig_pkt;

    el = ossl_qrl_enc_level_set_get(&qrx->el_set, QUIC_ENC_LEVEL_INITIAL, 1);
    assert(el != NULL); /* Already checked above */

    if (!ossl_quic_hdr_protector_decrypt(&el->hpr, &ptrs))
        goto malformed;

    /*
     * We have removed header protection, so don't attempt to do it again if
     * the packet gets deferred and processed again.
     */
    pkt_mark(&urxe->hpr_removed, 0);

    /* Decode the now unprotected header. */
    if (ossl_quic_wire_decode_pkt_hdr(&pkt, 0,
                                      0, 0, &rxe->hdr, NULL, NULL) != 1)
        goto malformed;

    /* Validate header and decode PN. */
    if (!qrx_validate_hdr(qrx, rxe))
        goto malformed;

    /*
     * The AAD data is the entire (unprotected) packet header including the PN.
     * The packet header has been unprotected in place, so we can just reuse the
     * PACKET buffer. The header ends where the payload begins.
     */
    aad_len = rxe->hdr.data - sop;

    /* Ensure the RXE buffer size is adequate for our payload. */
    if ((rxe = qrx_reserve_rxe(&qrx->rx_free, rxe, rxe->hdr.len + i)) == NULL)
        goto malformed;

    /*
     * We decrypt the packet body to immediately after the token at the start of
     * the RXE buffer (where present).
     *
     * Do the decryption from the PACKET (which points into URXE memory) to our
     * RXE payload (single-copy decryption), then fixup the pointers in the
     * header to point to our new buffer.
     *
     * If decryption fails this is considered a permanent error; we defer
     * packets we don't yet have decryption keys for above, so if this fails,
     * something has gone wrong with the handshake process or a packet has been
     * corrupted.
     */
    dst = (unsigned char *)rxe_data(rxe) + i;
    if (!qrx_decrypt_pkt_body(qrx, dst, rxe->hdr.data, rxe->hdr.len,
                              &dec_len, sop, aad_len, rxe->pn, QUIC_ENC_LEVEL_INITIAL,
                              rxe->hdr.key_phase, &rx_key_epoch))
        goto malformed;

    /*
     * -----------------------------------------------------
     *   IMPORTANT: ANYTHING ABOVE THIS LINE IS UNVERIFIED
     *              AND MUST BE TIMING-CHANNEL SAFE.
     * -----------------------------------------------------
     *
     * At this point, we have successfully authenticated the AEAD tag and no
     * longer need to worry about exposing the PN, PN length or Key Phase bit in
     * timing channels. Invoke any configured validation callback to allow for
     * rejection of duplicate PNs.
     */
    if (!qrx_validate_hdr_late(qrx, rxe))
        goto malformed;

    pkt_mark(&urxe->processed, 0);

    /*
     * Update header to point to the decrypted buffer, which may be shorter
     * due to AEAD tags, block padding, etc.
     */
    rxe->hdr.data       = dst;
    rxe->hdr.len        = dec_len;
    rxe->data_len       = dec_len;
    rxe->datagram_len   = datagram_len;
    rxe->key_epoch      = rx_key_epoch;

    /* We processed the PN successfully, so update largest processed PN. */
    pn_space = rxe_determine_pn_space(rxe);
    if (rxe->pn > qrx->largest_pn[pn_space])
        qrx->largest_pn[pn_space] = rxe->pn;

    /* Copy across network addresses and RX time from URXE to RXE. */
    rxe->peer           = urxe->peer;
    rxe->local          = urxe->local;
    rxe->time           = urxe->time;
    rxe->datagram_id    = urxe->datagram_id;

    /*
     * The packet is decrypted, we are going to move it from
     * rx_pending queue where it waits to be further processed
     * by ch_rx().
     */
    ossl_list_rxe_remove(&qrx->rx_free, rxe);
    ossl_list_rxe_insert_tail(&qrx->rx_pending, rxe);

    return 1;

malformed:
    /* caller (port_default_packet_handler()) should discard urxe */
    return 0;
}

int ossl_qrx_validate_initial_packet(OSSL_QRX *qrx, QUIC_URXE *urxe,
                                     const QUIC_CONN_ID *dcid)
{
    urxe->processed     = 0;
    urxe->hpr_removed   = 0;
    urxe->deferred      = 0;

    return qrx_validate_initial_pkt(qrx, urxe, dcid, urxe->data_len);
}

static void qrx_requeue_deferred(OSSL_QRX *qrx)
{
    QUIC_URXE *e;

    while ((e = ossl_list_urxe_head(&qrx->urx_deferred)) != NULL) {
        ossl_list_urxe_remove(&qrx->urx_deferred, e);
        ossl_list_urxe_insert_tail(&qrx->urx_pending, e);
    }
}

int ossl_qrx_provide_secret(OSSL_QRX *qrx, uint32_t enc_level,
                            uint32_t suite_id, EVP_MD *md,
                            const unsigned char *secret, size_t secret_len)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM)
        return 0;

    if (!ossl_qrl_enc_level_set_provide_secret(&qrx->el_set,
                                               qrx->libctx,
                                               qrx->propq,
                                               enc_level,
                                               suite_id,
                                               md,
                                               secret,
                                               secret_len,
                                               qrx->init_key_phase_bit,
                                               /*is_tx=*/0))
        return 0;

    /*
     * Any packets we previously could not decrypt, we may now be able to
     * decrypt, so move any datagrams containing deferred packets from the
     * deferred to the pending queue.
     */
    qrx_requeue_deferred(qrx);
    return 1;
}

int ossl_qrx_discard_enc_level(OSSL_QRX *qrx, uint32_t enc_level)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM)
        return 0;

    ossl_qrl_enc_level_set_discard(&qrx->el_set, enc_level);
    return 1;
}

/* Returns 1 if there are one or more pending RXEs. */
int ossl_qrx_processed_read_pending(OSSL_QRX *qrx)
{
    return !ossl_list_rxe_is_empty(&qrx->rx_pending);
}

/* Returns 1 if there are yet-unprocessed packets. */
int ossl_qrx_unprocessed_read_pending(OSSL_QRX *qrx)
{
    return !ossl_list_urxe_is_empty(&qrx->urx_pending)
           || !ossl_list_urxe_is_empty(&qrx->urx_deferred);
}

/* Pop the next pending RXE. Returns NULL if no RXE is pending. */
static RXE *qrx_pop_pending_rxe(OSSL_QRX *qrx)
{
    RXE *rxe = ossl_list_rxe_head(&qrx->rx_pending);

    if (rxe == NULL)
        return NULL;

    ossl_list_rxe_remove(&qrx->rx_pending, rxe);
    return rxe;
}

/* Allocate a new RXE. */
static RXE *qrx_alloc_rxe(size_t alloc_len)
{
    RXE *rxe;

    if (alloc_len >= SIZE_MAX - sizeof(RXE))
        return NULL;

    rxe = OPENSSL_malloc(sizeof(RXE) + alloc_len);
    if (rxe == NULL)
        return NULL;

    ossl_list_rxe_init_elem(rxe);
    rxe->alloc_len = alloc_len;
    rxe->data_len  = 0;
    rxe->refcount  = 0;
    return rxe;
}

/*
 * Ensures there is at least one RXE in the RX free list, allocating a new entry
 * if necessary. The returned RXE is in the RX free list; it is not popped.
 *
 * alloc_len is a hint which may be used to determine the RXE size if allocation
 * is necessary. Returns NULL on allocation failure.
 */
static RXE *qrx_ensure_free_rxe(OSSL_QRX *qrx, size_t alloc_len)
{
    RXE *rxe;

    if (ossl_list_rxe_head(&qrx->rx_free) != NULL)
        return ossl_list_rxe_head(&qrx->rx_free);

    rxe = qrx_alloc_rxe(alloc_len);
    if (rxe == NULL)
        return NULL;

    ossl_list_rxe_insert_tail(&qrx->rx_free, rxe);
    return rxe;
}

/*
 * Resize the data buffer attached to an RXE to be n bytes in size. The address
 * of the RXE might change; the new address is returned, or NULL on failure, in
 * which case the original RXE remains valid.
 */
static RXE *qrx_resize_rxe(RXE_LIST *rxl, RXE *rxe, size_t n)
{
    RXE *rxe2, *p;

    /* Should never happen. */
    if (rxe == NULL)
        return NULL;

    if (n >= SIZE_MAX - sizeof(RXE))
        return NULL;

    /* Remove the item from the list to avoid accessing freed memory */
    p = ossl_list_rxe_prev(rxe);
    ossl_list_rxe_remove(rxl, rxe);

    /* Should never resize an RXE which has been handed out. */
    if (!ossl_assert(rxe->refcount == 0))
        return NULL;

    /*
     * NOTE: We do not clear old memory, although it does contain decrypted
     * data.
     */
    rxe2 = OPENSSL_realloc(rxe, sizeof(RXE) + n);
    if (rxe2 == NULL) {
        /* Resize failed, restore old allocation. */
        if (p == NULL)
            ossl_list_rxe_insert_head(rxl, rxe);
        else
            ossl_list_rxe_insert_after(rxl, p, rxe);
        return NULL;
    }

    if (p == NULL)
        ossl_list_rxe_insert_head(rxl, rxe2);
    else
        ossl_list_rxe_insert_after(rxl, p, rxe2);

    rxe2->alloc_len = n;
    return rxe2;
}

/*
 * Ensure the data buffer attached to an RXE is at least n bytes in size.
 * Returns NULL on failure.
 */
static RXE *qrx_reserve_rxe(RXE_LIST *rxl,
                            RXE *rxe, size_t n)
{
    if (rxe->alloc_len >= n)
        return rxe;

    return qrx_resize_rxe(rxl, rxe, n);
}

/* Return a RXE handed out to the user back to our freelist. */
static void qrx_recycle_rxe(OSSL_QRX *qrx, RXE *rxe)
{
    /* RXE should not be in any list */
    assert(ossl_list_rxe_prev(rxe) == NULL && ossl_list_rxe_next(rxe) == NULL);
    rxe->pkt.hdr    = NULL;
    rxe->pkt.peer   = NULL;
    rxe->pkt.local  = NULL;
    ossl_list_rxe_insert_tail(&qrx->rx_free, rxe);
}

/*
 * Given a pointer to a pointer pointing to a buffer and the size of that
 * buffer, copy the buffer into *prxe, expanding the RXE if necessary (its
 * pointer may change due to realloc). *pi is the offset in bytes to copy the
 * buffer to, and on success is updated to be the offset pointing after the
 * copied buffer. *pptr is updated to point to the new location of the buffer.
 */
static int qrx_relocate_buffer(OSSL_QRX *qrx, RXE **prxe, size_t *pi,
                               const unsigned char **pptr, size_t buf_len)
{
    RXE *rxe;
    unsigned char *dst;

    if (!buf_len)
        return 1;

    if ((rxe = qrx_reserve_rxe(&qrx->rx_free, *prxe, *pi + buf_len)) == NULL)
        return 0;

    *prxe = rxe;
    dst = (unsigned char *)rxe_data(rxe) + *pi;

    memcpy(dst, *pptr, buf_len);
    *pi += buf_len;
    *pptr = dst;
    return 1;
}

static uint32_t qrx_determine_enc_level(const QUIC_PKT_HDR *hdr)
{
    switch (hdr->type) {
        case QUIC_PKT_TYPE_INITIAL:
            return QUIC_ENC_LEVEL_INITIAL;
        case QUIC_PKT_TYPE_HANDSHAKE:
            return QUIC_ENC_LEVEL_HANDSHAKE;
        case QUIC_PKT_TYPE_0RTT:
            return QUIC_ENC_LEVEL_0RTT;
        case QUIC_PKT_TYPE_1RTT:
            return QUIC_ENC_LEVEL_1RTT;

        default:
            assert(0);
        case QUIC_PKT_TYPE_RETRY:
        case QUIC_PKT_TYPE_VERSION_NEG:
            return QUIC_ENC_LEVEL_INITIAL; /* not used */
    }
}

static uint32_t rxe_determine_pn_space(RXE *rxe)
{
    uint32_t enc_level;

    enc_level = qrx_determine_enc_level(&rxe->hdr);
    return ossl_quic_enc_level_to_pn_space(enc_level);
}

static int qrx_validate_hdr_early(OSSL_QRX *qrx, RXE *rxe,
                                  const QUIC_CONN_ID *first_dcid)
{
    /* Ensure version is what we want. */
    if (rxe->hdr.version != QUIC_VERSION_1
        && rxe->hdr.version != QUIC_VERSION_NONE)
        return 0;

    /* Clients should never receive 0-RTT packets. */
    if (rxe->hdr.type == QUIC_PKT_TYPE_0RTT)
        return 0;

    /* Version negotiation and retry packets must be the first packet. */
    if (first_dcid != NULL && !ossl_quic_pkt_type_can_share_dgram(rxe->hdr.type))
        return 0;

    /*
     * If this is not the first packet in a datagram, the destination connection
     * ID must match the one in that packet.
     */
    if (first_dcid != NULL) {
        if (!ossl_assert(first_dcid->id_len < QUIC_MAX_CONN_ID_LEN)
            || !ossl_quic_conn_id_eq(first_dcid,
                                     &rxe->hdr.dst_conn_id))
        return 0;
    }

    return 1;
}

/* Validate header and decode PN. */
static int qrx_validate_hdr(OSSL_QRX *qrx, RXE *rxe)
{
    int pn_space = rxe_determine_pn_space(rxe);

    if (!ossl_quic_wire_decode_pkt_hdr_pn(rxe->hdr.pn, rxe->hdr.pn_len,
                                          qrx->largest_pn[pn_space],
                                          &rxe->pn))
        return 0;

    return 1;
}

/* Late packet header validation. */
static int qrx_validate_hdr_late(OSSL_QRX *qrx, RXE *rxe)
{
    int pn_space = rxe_determine_pn_space(rxe);

    /*
     * Allow our user to decide whether to discard the packet before we try and
     * decrypt it.
     */
    if (qrx->validation_cb != NULL
        && !qrx->validation_cb(rxe->pn, pn_space, qrx->validation_cb_arg))
        return 0;

    return 1;
}

/*
 * Retrieves the correct cipher context for an EL and key phase. Writes the key
 * epoch number actually used for packet decryption to *rx_key_epoch.
 */
static size_t qrx_get_cipher_ctx_idx(OSSL_QRX *qrx, OSSL_QRL_ENC_LEVEL *el,
                                     uint32_t enc_level,
                                     unsigned char key_phase_bit,
                                     uint64_t *rx_key_epoch,
                                     int *is_old_key)
{
    size_t idx;

    *is_old_key = 0;

    if (enc_level != QUIC_ENC_LEVEL_1RTT) {
        *rx_key_epoch = 0;
        return 0;
    }

    if (!ossl_assert(key_phase_bit <= 1))
        return SIZE_MAX;

    /*
     * RFC 9001 requires that we not create timing channels which could reveal
     * the decrypted value of the Key Phase bit. We usually handle this by
     * keeping the cipher contexts for both the current and next key epochs
     * around, so that we just select a cipher context blindly using the key
     * phase bit, which is time-invariant.
     *
     * In the COOLDOWN state, we only have one keyslot/cipher context. RFC 9001
     * suggests an implementation strategy to avoid creating a timing channel in
     * this case:
     *
     *   Endpoints can use randomized packet protection keys in place of
     *   discarded keys when key updates are not yet permitted.
     *
     * Rather than use a randomised key, we simply use our existing key as it
     * will fail AEAD verification anyway. This avoids the need to keep around a
     * dedicated garbage key.
     *
     * Note: Accessing different cipher contexts is technically not
     * timing-channel safe due to microarchitectural side channels, but this is
     * the best we can reasonably do and appears to be directly suggested by the
     * RFC.
     */
    idx = (el->state == QRL_EL_STATE_PROV_COOLDOWN ? el->key_epoch & 1
                                                   : key_phase_bit);

    /*
     * We also need to determine the key epoch number which this index
     * corresponds to. This is so we can report the key epoch number in the
     * OSSL_QRX_PKT structure, which callers need to validate whether it was OK
     * for a packet to be sent using a given key epoch's keys.
     */
    switch (el->state) {
    case QRL_EL_STATE_PROV_NORMAL:
        /*
         * If we are in the NORMAL state, usually the KP bit will match the LSB
         * of our key epoch, meaning no new key update is being signalled. If it
         * does not match, this means the packet (purports to) belong to
         * the next key epoch.
         *
         * IMPORTANT: The AEAD tag has not been verified yet when this function
         * is called, so this code must be timing-channel safe, hence use of
         * XOR. Moreover, the value output below is not yet authenticated.
         */
        *rx_key_epoch
            = el->key_epoch + ((el->key_epoch & 1) ^ (uint64_t)key_phase_bit);
        break;

    case QRL_EL_STATE_PROV_UPDATING:
        /*
         * If we are in the UPDATING state, usually the KP bit will match the
         * LSB of our key epoch. If it does not match, this means that the
         * packet (purports to) belong to the previous key epoch.
         *
         * As above, must be timing-channel safe.
         */
        *is_old_key = (el->key_epoch & 1) ^ (uint64_t)key_phase_bit;
        *rx_key_epoch = el->key_epoch - (uint64_t)*is_old_key;
        break;

    case QRL_EL_STATE_PROV_COOLDOWN:
        /*
         * If we are in COOLDOWN, there is only one key epoch we can possibly
         * decrypt with, so just try that. If AEAD decryption fails, the
         * value we output here isn't used anyway.
         */
        *rx_key_epoch = el->key_epoch;
        break;
    }

    return idx;
}

/*
 * Tries to decrypt a packet payload.
 *
 * Returns 1 on success or 0 on failure (which is permanent). The payload is
 * decrypted from src and written to dst. The buffer dst must be of at least
 * src_len bytes in length. The actual length of the output in bytes is written
 * to *dec_len on success, which will always be equal to or less than (usually
 * less than) src_len.
 */
static int qrx_decrypt_pkt_body(OSSL_QRX *qrx, unsigned char *dst,
                                const unsigned char *src,
                                size_t src_len, size_t *dec_len,
                                const unsigned char *aad, size_t aad_len,
                                QUIC_PN pn, uint32_t enc_level,
                                unsigned char key_phase_bit,
                                uint64_t *rx_key_epoch)
{
    int l = 0, l2 = 0, is_old_key, nonce_len;
    unsigned char nonce[EVP_MAX_IV_LENGTH];
    size_t i, cctx_idx;
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(&qrx->el_set,
                                                        enc_level, 1);
    EVP_CIPHER_CTX *cctx;

    if (src_len > INT_MAX || aad_len > INT_MAX)
        return 0;

    /* We should not have been called if we do not have key material. */
    if (!ossl_assert(el != NULL))
        return 0;

    if (el->tag_len >= src_len)
        return 0;

    /*
     * If we have failed to authenticate a certain number of ciphertexts, refuse
     * to decrypt any more ciphertexts.
     */
    if (qrx->forged_pkt_count >= ossl_qrl_get_suite_max_forged_pkt(el->suite_id))
        return 0;

    cctx_idx = qrx_get_cipher_ctx_idx(qrx, el, enc_level, key_phase_bit,
                                      rx_key_epoch, &is_old_key);
    if (!ossl_assert(cctx_idx < OSSL_NELEM(el->cctx)))
        return 0;

    if (is_old_key && pn >= qrx->cur_epoch_start_pn)
        /*
         * RFC 9001 s. 5.5: Once an endpoint successfully receives a packet with
         * a given PN, it MUST discard all packets in the same PN space with
         * higher PNs if they cannot be successfully unprotected with the same
         * key, or -- if there is a key update -- a subsequent packet protection
         * key.
         *
         * In other words, once a PN x triggers a KU, it is invalid for us to
         * receive a packet with a newer PN y (y > x) using the old keys.
         */
        return 0;

    cctx = el->cctx[cctx_idx];

    /* Construct nonce (nonce=IV ^ PN). */
    nonce_len = EVP_CIPHER_CTX_get_iv_length(cctx);
    if (!ossl_assert(nonce_len >= (int)sizeof(QUIC_PN)))
        return 0;

    memcpy(nonce, el->iv[cctx_idx], nonce_len);
    for (i = 0; i < sizeof(QUIC_PN); ++i)
        nonce[nonce_len - i - 1] ^= (unsigned char)(pn >> (i * 8));

    /* type and key will already have been setup; feed the IV. */
    if (EVP_CipherInit_ex(cctx, NULL,
                          NULL, NULL, nonce, /*enc=*/0) != 1)
        return 0;

    /* Feed the AEAD tag we got so the cipher can validate it. */
    if (EVP_CIPHER_CTX_ctrl(cctx, EVP_CTRL_AEAD_SET_TAG,
                            el->tag_len,
                            (unsigned char *)src + src_len - el->tag_len) != 1)
        return 0;

    /* Feed AAD data. */
    if (EVP_CipherUpdate(cctx, NULL, &l, aad, aad_len) != 1)
        return 0;

    /* Feed encrypted packet body. */
    if (EVP_CipherUpdate(cctx, dst, &l, src, src_len - el->tag_len) != 1)
        return 0;

#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
    /*
     * Throw away what we just decrypted and just use the ciphertext instead
     * (which should be unencrypted)
     */
    memcpy(dst, src, l);

    /* Pretend to authenticate the tag but ignore it */
    if (EVP_CipherFinal_ex(cctx, NULL, &l2) != 1) {
        /* We don't care */
    }
#else
    /* Ensure authentication succeeded. */
    if (EVP_CipherFinal_ex(cctx, NULL, &l2) != 1) {
        /* Authentication failed, increment failed auth counter. */
        ++qrx->forged_pkt_count;
        return 0;
    }
#endif

    *dec_len = l;
    return 1;
}

static ossl_inline void ignore_res(int x)
{
    /* No-op. */
}

static void qrx_key_update_initiated(OSSL_QRX *qrx, QUIC_PN pn)
{
    if (!ossl_qrl_enc_level_set_key_update(&qrx->el_set, QUIC_ENC_LEVEL_1RTT))
        /* We are already in RXKU, so we don't call the callback again. */
        return;

    qrx->cur_epoch_start_pn = pn;

    if (qrx->key_update_cb != NULL)
        qrx->key_update_cb(pn, qrx->key_update_cb_arg);
}

/* Process a single packet in a datagram. */
static int qrx_process_pkt(OSSL_QRX *qrx, QUIC_URXE *urxe,
                           PACKET *pkt, size_t pkt_idx,
                           QUIC_CONN_ID *first_dcid,
                           size_t datagram_len)
{
    RXE *rxe;
    const unsigned char *eop = NULL;
    size_t i, aad_len = 0, dec_len = 0;
    PACKET orig_pkt = *pkt;
    const unsigned char *sop = PACKET_data(pkt);
    unsigned char *dst;
    char need_second_decode = 0, already_processed = 0;
    QUIC_PKT_HDR_PTRS ptrs;
    uint32_t pn_space, enc_level;
    OSSL_QRL_ENC_LEVEL *el = NULL;
    uint64_t rx_key_epoch = UINT64_MAX;

    /*
     * Get a free RXE. If we need to allocate a new one, use the packet length
     * as a good ballpark figure.
     */
    rxe = qrx_ensure_free_rxe(qrx, PACKET_remaining(pkt));
    if (rxe == NULL)
        return 0;

    /* Have we already processed this packet? */
    if (pkt_is_marked(&urxe->processed, pkt_idx))
        already_processed = 1;

    /*
     * Decode the header into the RXE structure. We first decrypt and read the
     * unprotected part of the packet header (unless we already removed header
     * protection, in which case we decode all of it).
     */
    need_second_decode = !pkt_is_marked(&urxe->hpr_removed, pkt_idx);
    if (!ossl_quic_wire_decode_pkt_hdr(pkt,
                                       qrx->short_conn_id_len,
                                       need_second_decode, 0, &rxe->hdr, &ptrs,
                                       NULL))
        goto malformed;

    /*
     * Our successful decode above included an intelligible length and the
     * PACKET is now pointing to the end of the QUIC packet.
     */
    eop = PACKET_data(pkt);

    /*
     * Make a note of the first packet's DCID so we can later ensure the
     * destination connection IDs of all packets in a datagram match.
     */
    if (pkt_idx == 0)
        *first_dcid = rxe->hdr.dst_conn_id;

    /*
     * Early header validation. Since we now know the packet length, we can also
     * now skip over it if we already processed it.
     */
    if (already_processed
        || !qrx_validate_hdr_early(qrx, rxe, pkt_idx == 0 ? NULL : first_dcid))
        /*
         * Already processed packets are handled identically to malformed
         * packets; i.e., they are ignored.
         */
        goto malformed;

    if (!ossl_quic_pkt_type_is_encrypted(rxe->hdr.type)) {
        /*
         * Version negotiation and retry packets are a special case. They do not
         * contain a payload which needs decrypting and have no header
         * protection.
         */

        /* Just copy the payload from the URXE to the RXE. */
        if ((rxe = qrx_reserve_rxe(&qrx->rx_free, rxe, rxe->hdr.len)) == NULL)
            /*
             * Allocation failure. EOP will be pointing to the end of the
             * datagram so processing of this datagram will end here.
             */
            goto malformed;

        /* We are now committed to returning the packet. */
        memcpy(rxe_data(rxe), rxe->hdr.data, rxe->hdr.len);
        pkt_mark(&urxe->processed, pkt_idx);

        rxe->hdr.data   = rxe_data(rxe);
        rxe->pn         = QUIC_PN_INVALID;

        rxe->data_len       = rxe->hdr.len;
        rxe->datagram_len   = datagram_len;
        rxe->key_epoch      = 0;
        rxe->peer           = urxe->peer;
        rxe->local          = urxe->local;
        rxe->time           = urxe->time;
        rxe->datagram_id    = urxe->datagram_id;

        /* Move RXE to pending. */
        ossl_list_rxe_remove(&qrx->rx_free, rxe);
        ossl_list_rxe_insert_tail(&qrx->rx_pending, rxe);
        return 0; /* success, did not defer */
    }

    /* Determine encryption level of packet. */
    enc_level = qrx_determine_enc_level(&rxe->hdr);

    /* If we do not have keying material for this encryption level yet, defer. */
    switch (ossl_qrl_enc_level_set_have_el(&qrx->el_set, enc_level)) {
        case 1:
            /* We have keys. */
            if (enc_level == QUIC_ENC_LEVEL_1RTT && !qrx->allow_1rtt)
                /*
                 * But we cannot process 1-RTT packets until the handshake is
                 * completed (RFC 9000 s. 5.7).
                 */
                goto cannot_decrypt;

            break;
        case 0:
            /* No keys yet. */
            goto cannot_decrypt;
        default:
            /* We already discarded keys for this EL, we will never process this.*/
            goto malformed;
    }

    /*
     * We will copy any token included in the packet to the start of our RXE
     * data buffer (so that we don't reference the URXE buffer any more and can
     * recycle it). Track our position in the RXE buffer by index instead of
     * pointer as the pointer may change as reallocs occur.
     */
    i = 0;

    /*
     * rxe->hdr.data is now pointing at the (encrypted) packet payload. rxe->hdr
     * also has fields pointing into the PACKET buffer which will be going away
     * soon (the URXE will be reused for another incoming packet).
     *
     * Firstly, relocate some of these fields into the RXE as needed.
     *
     * Relocate token buffer and fix pointer.
     */
    if (rxe->hdr.type == QUIC_PKT_TYPE_INITIAL) {
        const unsigned char *token = rxe->hdr.token;

        /*
         * This may change the value of rxe and change the value of the token
         * pointer as well. So we must make a temporary copy of the pointer to
         * the token, and then copy it back into the new location of the rxe
         */
        if (!qrx_relocate_buffer(qrx, &rxe, &i, &token, rxe->hdr.token_len))
            goto malformed;

        rxe->hdr.token = token;
    }

    /* Now remove header protection. */
    *pkt = orig_pkt;

    el = ossl_qrl_enc_level_set_get(&qrx->el_set, enc_level, 1);
    assert(el != NULL); /* Already checked above */

    if (need_second_decode) {
        if (!ossl_quic_hdr_protector_decrypt(&el->hpr, &ptrs))
            goto malformed;

        /*
         * We have removed header protection, so don't attempt to do it again if
         * the packet gets deferred and processed again.
         */
        pkt_mark(&urxe->hpr_removed, pkt_idx);

        /* Decode the now unprotected header. */
        if (ossl_quic_wire_decode_pkt_hdr(pkt, qrx->short_conn_id_len,
                                          0, 0, &rxe->hdr, NULL, NULL) != 1)
            goto malformed;
    }

    /* Validate header and decode PN. */
    if (!qrx_validate_hdr(qrx, rxe))
        goto malformed;

    if (qrx->msg_callback != NULL)
        qrx->msg_callback(0, OSSL_QUIC1_VERSION, SSL3_RT_QUIC_PACKET, sop,
                          eop - sop - rxe->hdr.len, qrx->msg_callback_ssl,
                          qrx->msg_callback_arg);

    /*
     * The AAD data is the entire (unprotected) packet header including the PN.
     * The packet header has been unprotected in place, so we can just reuse the
     * PACKET buffer. The header ends where the payload begins.
     */
    aad_len = rxe->hdr.data - sop;

    /* Ensure the RXE buffer size is adequate for our payload. */
    if ((rxe = qrx_reserve_rxe(&qrx->rx_free, rxe, rxe->hdr.len + i)) == NULL) {
        /*
         * Allocation failure, treat as malformed and do not bother processing
         * any further packets in the datagram as they are likely to also
         * encounter allocation failures.
         */
        eop = NULL;
        goto malformed;
    }

    /*
     * We decrypt the packet body to immediately after the token at the start of
     * the RXE buffer (where present).
     *
     * Do the decryption from the PACKET (which points into URXE memory) to our
     * RXE payload (single-copy decryption), then fixup the pointers in the
     * header to point to our new buffer.
     *
     * If decryption fails this is considered a permanent error; we defer
     * packets we don't yet have decryption keys for above, so if this fails,
     * something has gone wrong with the handshake process or a packet has been
     * corrupted.
     */
    dst = (unsigned char *)rxe_data(rxe) + i;
    if (!qrx_decrypt_pkt_body(qrx, dst, rxe->hdr.data, rxe->hdr.len,
                              &dec_len, sop, aad_len, rxe->pn, enc_level,
                              rxe->hdr.key_phase, &rx_key_epoch))
        goto malformed;

    /*
     * -----------------------------------------------------
     *   IMPORTANT: ANYTHING ABOVE THIS LINE IS UNVERIFIED
     *              AND MUST BE TIMING-CHANNEL SAFE.
     * -----------------------------------------------------
     *
     * At this point, we have successfully authenticated the AEAD tag and no
     * longer need to worry about exposing the PN, PN length or Key Phase bit in
     * timing channels. Invoke any configured validation callback to allow for
     * rejection of duplicate PNs.
     */
    if (!qrx_validate_hdr_late(qrx, rxe))
        goto malformed;

    /* Check for a Key Phase bit differing from our expectation. */
    if (rxe->hdr.type == QUIC_PKT_TYPE_1RTT
        && rxe->hdr.key_phase != (el->key_epoch & 1))
        qrx_key_update_initiated(qrx, rxe->pn);

    /*
     * We have now successfully decrypted the packet payload. If there are
     * additional packets in the datagram, it is possible we will fail to
     * decrypt them and need to defer them until we have some key material we
     * don't currently possess. If this happens, the URXE will be moved to the
     * deferred queue. Since a URXE corresponds to one datagram, which may
     * contain multiple packets, we must ensure any packets we have already
     * processed in the URXE are not processed again (this is an RFC
     * requirement). We do this by marking the nth packet in the datagram as
     * processed.
     *
     * We are now committed to returning this decrypted packet to the user,
     * meaning we now consider the packet processed and must mark it
     * accordingly.
     */
    pkt_mark(&urxe->processed, pkt_idx);

    /*
     * Update header to point to the decrypted buffer, which may be shorter
     * due to AEAD tags, block padding, etc.
     */
    rxe->hdr.data       = dst;
    rxe->hdr.len        = dec_len;
    rxe->data_len       = dec_len;
    rxe->datagram_len   = datagram_len;
    rxe->key_epoch      = rx_key_epoch;

    /* We processed the PN successfully, so update largest processed PN. */
    pn_space = rxe_determine_pn_space(rxe);
    if (rxe->pn > qrx->largest_pn[pn_space])
        qrx->largest_pn[pn_space] = rxe->pn;

    /* Copy across network addresses and RX time from URXE to RXE. */
    rxe->peer           = urxe->peer;
    rxe->local          = urxe->local;
    rxe->time           = urxe->time;
    rxe->datagram_id    = urxe->datagram_id;

    /* Move RXE to pending. */
    ossl_list_rxe_remove(&qrx->rx_free, rxe);
    ossl_list_rxe_insert_tail(&qrx->rx_pending, rxe);
    return 0; /* success, did not defer; not distinguished from failure */

cannot_decrypt:
    /*
     * We cannot process this packet right now (but might be able to later). We
     * MUST attempt to process any other packets in the datagram, so defer it
     * and skip over it.
     */
    assert(eop != NULL && eop >= PACKET_data(pkt));
    /*
     * We don't care if this fails as it will just result in the packet being at
     * the end of the datagram buffer.
     */
    ignore_res(PACKET_forward(pkt, eop - PACKET_data(pkt)));
    return 1; /* deferred */

malformed:
    if (eop != NULL) {
        /*
         * This packet cannot be processed and will never be processable. We
         * were at least able to decode its header and determine its length, so
         * we can skip over it and try to process any subsequent packets in the
         * datagram.
         *
         * Mark as processed as an optimization.
         */
        assert(eop >= PACKET_data(pkt));
        pkt_mark(&urxe->processed, pkt_idx);
        /* We don't care if this fails (see above) */
        ignore_res(PACKET_forward(pkt, eop - PACKET_data(pkt)));
    } else {
        /*
         * This packet cannot be processed and will never be processable.
         * Because even its header is not intelligible, we cannot examine any
         * further packets in the datagram because its length cannot be
         * discerned.
         *
         * Advance over the entire remainder of the datagram, and mark it as
         * processed as an optimization.
         */
        pkt_mark(&urxe->processed, pkt_idx);
        /* We don't care if this fails (see above) */
        ignore_res(PACKET_forward(pkt, PACKET_remaining(pkt)));
    }
    return 0; /* failure, did not defer; not distinguished from success */
}

/* Process a datagram which was received. */
static int qrx_process_datagram(OSSL_QRX *qrx, QUIC_URXE *e,
                                const unsigned char *data,
                                size_t data_len)
{
    int have_deferred = 0;
    PACKET pkt;
    size_t pkt_idx = 0;
    QUIC_CONN_ID first_dcid = { 255 };

    qrx->bytes_received += data_len;

    if (!PACKET_buf_init(&pkt, data, data_len))
        return 0;

    for (; PACKET_remaining(&pkt) > 0; ++pkt_idx) {
        /*
         * A packet smaller than the minimum possible QUIC packet size is not
         * considered valid. We also ignore more than a certain number of
         * packets within the same datagram.
         */
        if (PACKET_remaining(&pkt) < QUIC_MIN_VALID_PKT_LEN
            || pkt_idx >= QUIC_MAX_PKT_PER_URXE)
            break;

        /*
         * We note whether packet processing resulted in a deferral since
         * this means we need to move the URXE to the deferred list rather
         * than the free list after we're finished dealing with it for now.
         *
         * However, we don't otherwise care here whether processing succeeded or
         * failed, as the RFC says even if a packet in a datagram is malformed,
         * we should still try to process any packets following it.
         *
         * In the case where the packet is so malformed we can't determine its
         * length, qrx_process_pkt will take care of advancing to the end of
         * the packet, so we will exit the loop automatically in this case.
         */
        if (qrx_process_pkt(qrx, e, &pkt, pkt_idx, &first_dcid, data_len))
            have_deferred = 1;
    }

    /* Only report whether there were any deferrals. */
    return have_deferred;
}

/* Process a single pending URXE. */
static int qrx_process_one_urxe(OSSL_QRX *qrx, QUIC_URXE *e)
{
    int was_deferred;

    /* The next URXE we process should be at the head of the pending list. */
    if (!ossl_assert(e == ossl_list_urxe_head(&qrx->urx_pending)))
        return 0;

    /*
     * Attempt to process the datagram. The return value indicates only if
     * processing of the datagram was deferred. If we failed to process the
     * datagram, we do not attempt to process it again and silently eat the
     * error.
     */
    was_deferred = qrx_process_datagram(qrx, e, ossl_quic_urxe_data(e),
                                        e->data_len);

    /*
     * Remove the URXE from the pending list and return it to
     * either the free or deferred list.
     */
    ossl_list_urxe_remove(&qrx->urx_pending, e);
    if (was_deferred > 0 &&
            (e->deferred || qrx->num_deferred < qrx->max_deferred)) {
        ossl_list_urxe_insert_tail(&qrx->urx_deferred, e);
        if (!e->deferred) {
            e->deferred = 1;
            ++qrx->num_deferred;
        }
    } else {
        if (e->deferred) {
            e->deferred = 0;
            --qrx->num_deferred;
        }
        ossl_quic_demux_release_urxe(qrx->demux, e);
    }

    return 1;
}

/* Process any pending URXEs to generate pending RXEs. */
static int qrx_process_pending_urxl(OSSL_QRX *qrx)
{
    QUIC_URXE *e;

    while ((e = ossl_list_urxe_head(&qrx->urx_pending)) != NULL)
        if (!qrx_process_one_urxe(qrx, e))
            return 0;

    return 1;
}

int ossl_qrx_read_pkt(OSSL_QRX *qrx, OSSL_QRX_PKT **ppkt)
{
    RXE *rxe;

    if (!ossl_qrx_processed_read_pending(qrx)) {
        if (!qrx_process_pending_urxl(qrx))
            return 0;

        if (!ossl_qrx_processed_read_pending(qrx))
            return 0;
    }

    rxe = qrx_pop_pending_rxe(qrx);
    if (!ossl_assert(rxe != NULL))
        return 0;

    assert(rxe->refcount == 0);
    rxe->refcount = 1;

    rxe->pkt.hdr            = &rxe->hdr;
    rxe->pkt.pn             = rxe->pn;
    rxe->pkt.time           = rxe->time;
    rxe->pkt.datagram_len   = rxe->datagram_len;
    rxe->pkt.peer
        = BIO_ADDR_family(&rxe->peer) != AF_UNSPEC ? &rxe->peer : NULL;
    rxe->pkt.local
        = BIO_ADDR_family(&rxe->local) != AF_UNSPEC ? &rxe->local : NULL;
    rxe->pkt.key_epoch      = rxe->key_epoch;
    rxe->pkt.datagram_id    = rxe->datagram_id;
    rxe->pkt.qrx            = qrx;
    *ppkt = &rxe->pkt;

    return 1;
}

void ossl_qrx_pkt_release(OSSL_QRX_PKT *pkt)
{
    RXE *rxe;

    if (pkt == NULL)
        return;

    rxe = (RXE *)pkt;
    assert(rxe->refcount > 0);
    if (--rxe->refcount == 0)
        qrx_recycle_rxe(pkt->qrx, rxe);
}

void ossl_qrx_pkt_orphan(OSSL_QRX_PKT *pkt)
{
    RXE *rxe;

    if (pkt == NULL)
        return;
    rxe = (RXE *)pkt;
    assert(rxe->refcount > 0);
    rxe->refcount--;
    assert(ossl_list_rxe_prev(rxe) == NULL && ossl_list_rxe_next(rxe) == NULL);
    return;
}

void ossl_qrx_pkt_up_ref(OSSL_QRX_PKT *pkt)
{
    RXE *rxe = (RXE *)pkt;

    assert(rxe->refcount > 0);
    ++rxe->refcount;
}

uint64_t ossl_qrx_get_bytes_received(OSSL_QRX *qrx, int clear)
{
    uint64_t v = qrx->bytes_received;

    if (clear)
        qrx->bytes_received = 0;

    return v;
}

int ossl_qrx_set_late_validation_cb(OSSL_QRX *qrx,
                                    ossl_qrx_late_validation_cb *cb,
                                    void *cb_arg)
{
    qrx->validation_cb       = cb;
    qrx->validation_cb_arg   = cb_arg;
    return 1;
}

int ossl_qrx_set_key_update_cb(OSSL_QRX *qrx,
                               ossl_qrx_key_update_cb *cb,
                               void *cb_arg)
{
    qrx->key_update_cb      = cb;
    qrx->key_update_cb_arg  = cb_arg;
    return 1;
}

uint64_t ossl_qrx_get_key_epoch(OSSL_QRX *qrx)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(&qrx->el_set,
                                                        QUIC_ENC_LEVEL_1RTT, 1);

    return el == NULL ? UINT64_MAX : el->key_epoch;
}

int ossl_qrx_key_update_timeout(OSSL_QRX *qrx, int normal)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(&qrx->el_set,
                                                        QUIC_ENC_LEVEL_1RTT, 1);

    if (el == NULL)
        return 0;

    if (el->state == QRL_EL_STATE_PROV_UPDATING
        && !ossl_qrl_enc_level_set_key_update_done(&qrx->el_set,
                                                   QUIC_ENC_LEVEL_1RTT))
        return 0;

    if (normal && el->state == QRL_EL_STATE_PROV_COOLDOWN
        && !ossl_qrl_enc_level_set_key_cooldown_done(&qrx->el_set,
                                                     QUIC_ENC_LEVEL_1RTT))
        return 0;

    return 1;
}

uint64_t ossl_qrx_get_cur_forged_pkt_count(OSSL_QRX *qrx)
{
    return qrx->forged_pkt_count;
}

uint64_t ossl_qrx_get_max_forged_pkt_count(OSSL_QRX *qrx,
                                           uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el = ossl_qrl_enc_level_set_get(&qrx->el_set,
                                                        enc_level, 1);

    return el == NULL ? UINT64_MAX
        : ossl_qrl_get_suite_max_forged_pkt(el->suite_id);
}

void ossl_qrx_allow_1rtt_processing(OSSL_QRX *qrx)
{
    if (qrx->allow_1rtt)
        return;

    qrx->allow_1rtt = 1;
    qrx_requeue_deferred(qrx);
}

void ossl_qrx_set_msg_callback(OSSL_QRX *qrx, ossl_msg_cb msg_callback,
                               SSL *msg_callback_ssl)
{
    qrx->msg_callback = msg_callback;
    qrx->msg_callback_ssl = msg_callback_ssl;
}

void ossl_qrx_set_msg_callback_arg(OSSL_QRX *qrx, void *msg_callback_arg)
{
    qrx->msg_callback_arg = msg_callback_arg;
}

size_t ossl_qrx_get_short_hdr_conn_id_len(OSSL_QRX *qrx)
{
    return qrx->short_conn_id_len;
}
