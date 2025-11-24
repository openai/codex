/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_record_tx.h"
#include "internal/qlog_event_helpers.h"
#include "internal/bio_addr.h"
#include "internal/common.h"
#include "quic_record_shared.h"
#include "internal/list.h"
#include "../ssl_local.h"

/*
 * TXE
 * ===
 * Encrypted packets awaiting transmission are kept in TX Entries (TXEs), which
 * are queued in linked lists just like TXEs.
 */
typedef struct txe_st TXE;

struct txe_st {
    OSSL_LIST_MEMBER(txe, TXE);
    size_t              data_len, alloc_len;

    /*
     * Destination and local addresses, as applicable. Both of these are only
     * used if the family is not AF_UNSPEC.
     */
    BIO_ADDR            peer, local;

    /*
     * alloc_len allocated bytes (of which data_len bytes are valid) follow this
     * structure.
     */
};

DEFINE_LIST_OF(txe, TXE);
typedef OSSL_LIST(txe) TXE_LIST;

static ossl_inline unsigned char *txe_data(const TXE *e)
{
    return (unsigned char *)(e + 1);
}

/*
 * QTX
 * ===
 */
struct ossl_qtx_st {
    OSSL_LIB_CTX               *libctx;
    const char                 *propq;

    /* Per encryption-level state. */
    OSSL_QRL_ENC_LEVEL_SET      el_set;

    /* TX BIO. */
    BIO                        *bio;

    /* QLOG instance retrieval callback if in use, or NULL. */
    QLOG                     *(*get_qlog_cb)(void *arg);
    void                       *get_qlog_cb_arg;

    /* TX maximum datagram payload length. */
    size_t                      mdpl;

    /*
     * List of TXEs which are not currently in use. These are moved to the
     * pending list (possibly via tx_cons first) as they are filled.
     */
    TXE_LIST                    free;

    /*
     * List of TXEs which are filled with completed datagrams ready to be
     * transmitted.
     */
    TXE_LIST                    pending;
    size_t                      pending_count; /* items in list */
    size_t                      pending_bytes; /* sum(txe->data_len) in pending */

    /*
     * TXE which is under construction for coalescing purposes, if any.
     * This TXE is neither on the free nor pending list. Once the datagram
     * is completed, it is moved to the pending list.
     */
    TXE                        *cons;
    size_t                      cons_count; /* num packets */

    /*
     * Number of packets transmitted in this key epoch. Used to enforce AEAD
     * confidentiality limit.
     */
    uint64_t                    epoch_pkt_count;

    /* Datagram counter. Increases monotonically per datagram (not per packet). */
    uint64_t                    datagram_count;

    ossl_mutate_packet_cb mutatecb;
    ossl_finish_mutate_cb finishmutatecb;
    void *mutatearg;

    /* Message callback related arguments */
    ossl_msg_cb msg_callback;
    void *msg_callback_arg;
    SSL *msg_callback_ssl;
};

/* Instantiates a new QTX. */
OSSL_QTX *ossl_qtx_new(const OSSL_QTX_ARGS *args)
{
    OSSL_QTX *qtx;

    if (args->mdpl < QUIC_MIN_INITIAL_DGRAM_LEN)
        return 0;

    qtx = OPENSSL_zalloc(sizeof(OSSL_QTX));
    if (qtx == NULL)
        return 0;

    qtx->libctx             = args->libctx;
    qtx->propq              = args->propq;
    qtx->bio                = args->bio;
    qtx->mdpl               = args->mdpl;
    qtx->get_qlog_cb        = args->get_qlog_cb;
    qtx->get_qlog_cb_arg    = args->get_qlog_cb_arg;

    return qtx;
}

static void qtx_cleanup_txl(TXE_LIST *l)
{
    TXE *e, *enext;

    for (e = ossl_list_txe_head(l); e != NULL; e = enext) {
        enext = ossl_list_txe_next(e);
        OPENSSL_free(e);
    }
}

/* Frees the QTX. */
void ossl_qtx_free(OSSL_QTX *qtx)
{
    uint32_t i;

    if (qtx == NULL)
        return;

    /* Free TXE queue data. */
    qtx_cleanup_txl(&qtx->pending);
    qtx_cleanup_txl(&qtx->free);
    OPENSSL_free(qtx->cons);

    /* Drop keying material and crypto resources. */
    for (i = 0; i < QUIC_ENC_LEVEL_NUM; ++i)
        ossl_qrl_enc_level_set_discard(&qtx->el_set, i);

    OPENSSL_free(qtx);
}

/* Set mutator callbacks for test framework support */
void ossl_qtx_set_mutator(OSSL_QTX *qtx, ossl_mutate_packet_cb mutatecb,
                          ossl_finish_mutate_cb finishmutatecb, void *mutatearg)
{
    qtx->mutatecb       = mutatecb;
    qtx->finishmutatecb = finishmutatecb;
    qtx->mutatearg      = mutatearg;
}

void ossl_qtx_set_qlog_cb(OSSL_QTX *qtx, QLOG *(*get_qlog_cb)(void *arg),
                          void *get_qlog_cb_arg)
{
    qtx->get_qlog_cb        = get_qlog_cb;
    qtx->get_qlog_cb_arg    = get_qlog_cb_arg;
}

int ossl_qtx_provide_secret(OSSL_QTX              *qtx,
                            uint32_t               enc_level,
                            uint32_t               suite_id,
                            EVP_MD                *md,
                            const unsigned char   *secret,
                            size_t                 secret_len)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM)
        return 0;

    return ossl_qrl_enc_level_set_provide_secret(&qtx->el_set,
                                                 qtx->libctx,
                                                 qtx->propq,
                                                 enc_level,
                                                 suite_id,
                                                 md,
                                                 secret,
                                                 secret_len,
                                                 0,
                                                 /*is_tx=*/1);
}

int ossl_qtx_discard_enc_level(OSSL_QTX *qtx, uint32_t enc_level)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM)
        return 0;

    ossl_qrl_enc_level_set_discard(&qtx->el_set, enc_level);
    return 1;
}

int ossl_qtx_is_enc_level_provisioned(OSSL_QTX *qtx, uint32_t enc_level)
{
    return ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1) != NULL;
}

/* Allocate a new TXE. */
static TXE *qtx_alloc_txe(size_t alloc_len)
{
    TXE *txe;

    if (alloc_len >= SIZE_MAX - sizeof(TXE))
        return NULL;

    txe = OPENSSL_malloc(sizeof(TXE) + alloc_len);
    if (txe == NULL)
        return NULL;

    ossl_list_txe_init_elem(txe);
    txe->alloc_len = alloc_len;
    txe->data_len = 0;
    return txe;
}

/*
 * Ensures there is at least one TXE in the free list, allocating a new entry
 * if necessary. The returned TXE is in the free list; it is not popped.
 *
 * alloc_len is a hint which may be used to determine the TXE size if allocation
 * is necessary. Returns NULL on allocation failure.
 */
static TXE *qtx_ensure_free_txe(OSSL_QTX *qtx, size_t alloc_len)
{
    TXE *txe;

    txe = ossl_list_txe_head(&qtx->free);
    if (txe != NULL)
        return txe;

    txe = qtx_alloc_txe(alloc_len);
    if (txe == NULL)
        return NULL;

    ossl_list_txe_insert_tail(&qtx->free, txe);
    return txe;
}

/*
 * Resize the data buffer attached to an TXE to be n bytes in size. The address
 * of the TXE might change; the new address is returned, or NULL on failure, in
 * which case the original TXE remains valid.
 */
static TXE *qtx_resize_txe(OSSL_QTX *qtx, TXE_LIST *txl, TXE *txe, size_t n)
{
    TXE *txe2, *p;

    /* Should never happen. */
    if (txe == NULL)
        return NULL;

    if (n >= SIZE_MAX - sizeof(TXE))
        return NULL;

    /* Remove the item from the list to avoid accessing freed memory */
    p = ossl_list_txe_prev(txe);
    ossl_list_txe_remove(txl, txe);

    /*
     * NOTE: We do not clear old memory, although it does contain decrypted
     * data.
     */
    txe2 = OPENSSL_realloc(txe, sizeof(TXE) + n);
    if (txe2 == NULL || txe == txe2) {
        if (p == NULL)
            ossl_list_txe_insert_head(txl, txe);
        else
            ossl_list_txe_insert_after(txl, p, txe);
        return txe2;
    }

    if (p == NULL)
        ossl_list_txe_insert_head(txl, txe2);
    else
        ossl_list_txe_insert_after(txl, p, txe2);

    if (qtx->cons == txe)
        qtx->cons = txe2;

    txe2->alloc_len = n;
    return txe2;
}

/*
 * Ensure the data buffer attached to an TXE is at least n bytes in size.
 * Returns NULL on failure.
 */
static TXE *qtx_reserve_txe(OSSL_QTX *qtx, TXE_LIST *txl,
                            TXE *txe, size_t n)
{
    if (txe->alloc_len >= n)
        return txe;

    return qtx_resize_txe(qtx, txl, txe, n);
}

/* Move a TXE from pending to free. */
static void qtx_pending_to_free(OSSL_QTX *qtx)
{
    TXE *txe = ossl_list_txe_head(&qtx->pending);

    assert(txe != NULL);
    ossl_list_txe_remove(&qtx->pending, txe);
    --qtx->pending_count;
    qtx->pending_bytes -= txe->data_len;
    ossl_list_txe_insert_tail(&qtx->free, txe);
}

/* Add a TXE not currently in any list to the pending list. */
static void qtx_add_to_pending(OSSL_QTX *qtx, TXE *txe)
{
    ossl_list_txe_insert_tail(&qtx->pending, txe);
    ++qtx->pending_count;
    qtx->pending_bytes += txe->data_len;
}

struct iovec_cur {
    const OSSL_QTX_IOVEC *iovec;
    size_t                num_iovec, idx, byte_off, bytes_remaining;
};

static size_t iovec_total_bytes(const OSSL_QTX_IOVEC *iovec,
                                size_t num_iovec)
{
    size_t i, l = 0;

    for (i = 0; i < num_iovec; ++i)
        l += iovec[i].buf_len;

    return l;
}

static void iovec_cur_init(struct iovec_cur *cur,
                           const OSSL_QTX_IOVEC *iovec,
                           size_t num_iovec)
{
    cur->iovec              = iovec;
    cur->num_iovec          = num_iovec;
    cur->idx                = 0;
    cur->byte_off           = 0;
    cur->bytes_remaining    = iovec_total_bytes(iovec, num_iovec);
}

/*
 * Get an extent of bytes from the iovec cursor. *buf is set to point to the
 * buffer and the number of bytes in length of the buffer is returned. This
 * value may be less than the max_buf_len argument. If no more data is
 * available, returns 0.
 */
static size_t iovec_cur_get_buffer(struct iovec_cur *cur,
                                   const unsigned char **buf,
                                   size_t max_buf_len)
{
    size_t l;

    if (max_buf_len == 0) {
        *buf = NULL;
        return 0;
    }

    for (;;) {
        if (cur->idx >= cur->num_iovec)
            return 0;

        l = cur->iovec[cur->idx].buf_len - cur->byte_off;
        if (l > max_buf_len)
            l = max_buf_len;

        if (l > 0) {
            *buf = cur->iovec[cur->idx].buf + cur->byte_off;
            cur->byte_off += l;
            cur->bytes_remaining -= l;
            return l;
        }

        /*
         * Zero-length iovec entry or we already consumed all of it, try the
         * next iovec.
         */
        ++cur->idx;
        cur->byte_off = 0;
    }
}

/* Determines the size of the AEAD output given the input size. */
int ossl_qtx_calculate_ciphertext_payload_len(OSSL_QTX *qtx, uint32_t enc_level,
                                              size_t plaintext_len,
                                              size_t *ciphertext_len)
{
    OSSL_QRL_ENC_LEVEL *el
        = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
    size_t tag_len;

    if (el == NULL) {
        *ciphertext_len = 0;
        return 0;
    }

    /*
     * We currently only support ciphers with a 1:1 mapping between plaintext
     * and ciphertext size, save for authentication tag.
     */
    tag_len = ossl_qrl_get_suite_cipher_tag_len(el->suite_id);

    *ciphertext_len = plaintext_len + tag_len;
    return 1;
}

/* Determines the size of the AEAD input given the output size. */
int ossl_qtx_calculate_plaintext_payload_len(OSSL_QTX *qtx, uint32_t enc_level,
                                             size_t ciphertext_len,
                                             size_t *plaintext_len)
{
    OSSL_QRL_ENC_LEVEL *el
        = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
    size_t tag_len;

    if (el == NULL) {
        *plaintext_len = 0;
        return 0;
    }

    tag_len = ossl_qrl_get_suite_cipher_tag_len(el->suite_id);

    if (ciphertext_len <= tag_len) {
        *plaintext_len = 0;
        return 0;
    }

    *plaintext_len = ciphertext_len - tag_len;
    return 1;
}

/* Any other error (including packet being too big for MDPL). */
#define QTX_FAIL_GENERIC            (-1)

/*
 * Returned where there is insufficient room in the datagram to write the
 * packet.
 */
#define QTX_FAIL_INSUFFICIENT_LEN   (-2)

static int qtx_write_hdr(OSSL_QTX *qtx, const QUIC_PKT_HDR *hdr, TXE *txe,
                         QUIC_PKT_HDR_PTRS *ptrs)
{
    WPACKET wpkt;
    size_t l = 0;
    unsigned char *data = txe_data(txe) + txe->data_len;

    if (!WPACKET_init_static_len(&wpkt, data, txe->alloc_len - txe->data_len, 0))
        return 0;

    if (!ossl_quic_wire_encode_pkt_hdr(&wpkt, hdr->dst_conn_id.id_len,
                                       hdr, ptrs)
        || !WPACKET_get_total_written(&wpkt, &l)) {
        WPACKET_finish(&wpkt);
        return 0;
    }
    WPACKET_finish(&wpkt);

    if (qtx->msg_callback != NULL)
        qtx->msg_callback(1, OSSL_QUIC1_VERSION, SSL3_RT_QUIC_PACKET, data, l,
                          qtx->msg_callback_ssl, qtx->msg_callback_arg);

    txe->data_len += l;

    return 1;
}

static int qtx_encrypt_into_txe(OSSL_QTX *qtx, struct iovec_cur *cur, TXE *txe,
                                uint32_t enc_level, QUIC_PN pn,
                                const unsigned char *hdr, size_t hdr_len,
                                QUIC_PKT_HDR_PTRS *ptrs)
{
    int l = 0, l2 = 0, nonce_len;
    OSSL_QRL_ENC_LEVEL *el
        = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
    unsigned char nonce[EVP_MAX_IV_LENGTH];
    size_t i;
    EVP_CIPHER_CTX *cctx = NULL;

    /* We should not have been called if we do not have key material. */
    if (!ossl_assert(el != NULL)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /*
     * Have we already encrypted the maximum number of packets using the current
     * key?
     */
    if (el->op_count >= ossl_qrl_get_suite_max_pkt(el->suite_id)) {
        ERR_raise(ERR_LIB_SSL, SSL_R_MAXIMUM_ENCRYPTED_PKTS_REACHED);
        return 0;
    }

    /*
     * TX key update is simpler than for RX; once we initiate a key update, we
     * never need the old keys, as we never deliberately send a packet with old
     * keys. Thus the EL always uses keyslot 0 for the TX side.
     */
    cctx = el->cctx[0];
    if (!ossl_assert(cctx != NULL)) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    /* Construct nonce (nonce=IV ^ PN). */
    nonce_len = EVP_CIPHER_CTX_get_iv_length(cctx);
    if (!ossl_assert(nonce_len >= (int)sizeof(QUIC_PN))) {
        ERR_raise(ERR_LIB_SSL, ERR_R_INTERNAL_ERROR);
        return 0;
    }

    memcpy(nonce, el->iv[0], (size_t)nonce_len);
    for (i = 0; i < sizeof(QUIC_PN); ++i)
        nonce[nonce_len - i - 1] ^= (unsigned char)(pn >> (i * 8));

    /* type and key will already have been setup; feed the IV. */
    if (EVP_CipherInit_ex(cctx, NULL, NULL, NULL, nonce, /*enc=*/1) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    /* Feed AAD data. */
    if (EVP_CipherUpdate(cctx, NULL, &l, hdr, hdr_len) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    /* Encrypt plaintext directly into TXE. */
    for (;;) {
        const unsigned char *src;
        size_t src_len;

        src_len = iovec_cur_get_buffer(cur, &src, SIZE_MAX);
        if (src_len == 0)
            break;

        if (EVP_CipherUpdate(cctx, txe_data(txe) + txe->data_len,
                             &l, src, src_len) != 1) {
            ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
            return 0;
        }

#ifdef FUZZING_BUILD_MODE_UNSAFE_FOR_PRODUCTION
        /* Ignore what we just encrypted and overwrite it with the plaintext */
        memcpy(txe_data(txe) + txe->data_len, src, l);
#endif

        assert(l > 0 && src_len == (size_t)l);
        txe->data_len += src_len;
    }

    /* Finalise and get tag. */
    if (EVP_CipherFinal_ex(cctx, NULL, &l2) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    if (EVP_CIPHER_CTX_ctrl(cctx, EVP_CTRL_AEAD_GET_TAG,
                            el->tag_len, txe_data(txe) + txe->data_len) != 1) {
        ERR_raise(ERR_LIB_SSL, ERR_R_EVP_LIB);
        return 0;
    }

    txe->data_len += el->tag_len;

    /* Apply header protection. */
    if (!ossl_quic_hdr_protector_encrypt(&el->hpr, ptrs))
        return 0;

    ++el->op_count;
    return 1;
}

/*
 * Append a packet to the TXE buffer, serializing and encrypting it in the
 * process.
 */
static int qtx_write(OSSL_QTX *qtx, const OSSL_QTX_PKT *pkt, TXE *txe,
                     uint32_t enc_level, QUIC_PKT_HDR *hdr,
                     const OSSL_QTX_IOVEC *iovec, size_t num_iovec)
{
    int ret, needs_encrypt;
    size_t hdr_len, pred_hdr_len, payload_len, pkt_len, space_left;
    size_t min_len, orig_data_len;
    struct iovec_cur cur;
    QUIC_PKT_HDR_PTRS ptrs;
    unsigned char *hdr_start;
    OSSL_QRL_ENC_LEVEL *el = NULL;

    /*
     * Determine if the packet needs encryption and the minimum conceivable
     * serialization length.
     */
    if (!ossl_quic_pkt_type_is_encrypted(hdr->type)) {
        needs_encrypt = 0;
        min_len = QUIC_MIN_VALID_PKT_LEN;
    } else {
        needs_encrypt = 1;
        min_len = QUIC_MIN_VALID_PKT_LEN_CRYPTO;
        el = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
        if (!ossl_assert(el != NULL)) /* should already have been checked */
            return 0;
    }

    orig_data_len = txe->data_len;
    space_left = txe->alloc_len - txe->data_len;
    if (space_left < min_len) {
        /* Not even a possibility of it fitting. */
        ret = QTX_FAIL_INSUFFICIENT_LEN;
        goto err;
    }

    /* Set some fields in the header we are responsible for. */
    if (hdr->type == QUIC_PKT_TYPE_1RTT)
        hdr->key_phase = (unsigned char)(el->key_epoch & 1);

    /* Walk the iovecs to determine actual input payload length. */
    iovec_cur_init(&cur, iovec, num_iovec);

    if (cur.bytes_remaining == 0) {
        /* No zero-length payloads allowed. */
        ret = QTX_FAIL_GENERIC;
        goto err;
    }

    /* Determine encrypted payload length. */
    if (needs_encrypt)
        ossl_qtx_calculate_ciphertext_payload_len(qtx, enc_level,
                                                  cur.bytes_remaining,
                                                  &payload_len);
    else
        payload_len = cur.bytes_remaining;

    /* Determine header length. */
    hdr->data  = NULL;
    hdr->len   = payload_len;
    pred_hdr_len = ossl_quic_wire_get_encoded_pkt_hdr_len(hdr->dst_conn_id.id_len,
                                                          hdr);
    if (pred_hdr_len == 0) {
        ret = QTX_FAIL_GENERIC;
        goto err;
    }

    /* We now definitively know our packet length. */
    pkt_len = pred_hdr_len + payload_len;

    if (pkt_len > space_left) {
        ret = QTX_FAIL_INSUFFICIENT_LEN;
        goto err;
    }

    if (ossl_quic_pkt_type_has_pn(hdr->type)) {
        if (!ossl_quic_wire_encode_pkt_hdr_pn(pkt->pn,
                                              hdr->pn,
                                              hdr->pn_len)) {
            ret = QTX_FAIL_GENERIC;
            goto err;
        }
    }

    /* Append the header to the TXE. */
    hdr_start = txe_data(txe) + txe->data_len;
    if (!qtx_write_hdr(qtx, hdr, txe, &ptrs)) {
        ret = QTX_FAIL_GENERIC;
        goto err;
    }

    hdr_len = (txe_data(txe) + txe->data_len) - hdr_start;
    assert(hdr_len == pred_hdr_len);

    if (!needs_encrypt) {
        /* Just copy the payload across. */
        const unsigned char *src;
        size_t src_len;

        for (;;) {
            /* Buffer length has already been checked above. */
            src_len = iovec_cur_get_buffer(&cur, &src, SIZE_MAX);
            if (src_len == 0)
                break;

            memcpy(txe_data(txe) + txe->data_len, src, src_len);
            txe->data_len += src_len;
        }
    } else {
        /* Encrypt into TXE. */
        if (!qtx_encrypt_into_txe(qtx, &cur, txe, enc_level, pkt->pn,
                                  hdr_start, hdr_len, &ptrs)) {
            ret = QTX_FAIL_GENERIC;
            goto err;
        }

        assert(txe->data_len - orig_data_len == pkt_len);
    }

    return 1;

err:
    /*
     * Restore original length so we don't leave a half-written packet in the
     * TXE.
     */
    txe->data_len = orig_data_len;
    return ret;
}

static TXE *qtx_ensure_cons(OSSL_QTX *qtx)
{
    TXE *txe = qtx->cons;

    if (txe != NULL)
        return txe;

    txe = qtx_ensure_free_txe(qtx, qtx->mdpl);
    if (txe == NULL)
        return NULL;

    ossl_list_txe_remove(&qtx->free, txe);
    qtx->cons = txe;
    qtx->cons_count = 0;
    txe->data_len = 0;
    return txe;
}

static QLOG *qtx_get_qlog(OSSL_QTX *qtx)
{
    if (qtx->get_qlog_cb == NULL)
        return NULL;

    return qtx->get_qlog_cb(qtx->get_qlog_cb_arg);
}

static int qtx_mutate_write(OSSL_QTX *qtx, const OSSL_QTX_PKT *pkt, TXE *txe,
                            uint32_t enc_level)
{
    int ret;
    QUIC_PKT_HDR *hdr;
    const OSSL_QTX_IOVEC *iovec;
    size_t num_iovec;

    /* If we are running tests then mutate_packet may be non NULL */
    if (qtx->mutatecb != NULL) {
        if (!qtx->mutatecb(pkt->hdr, pkt->iovec, pkt->num_iovec, &hdr,
                           &iovec, &num_iovec, qtx->mutatearg))
            return QTX_FAIL_GENERIC;
    } else {
        hdr         = pkt->hdr;
        iovec       = pkt->iovec;
        num_iovec   = pkt->num_iovec;
    }

    ret = qtx_write(qtx, pkt, txe, enc_level,
                    hdr, iovec, num_iovec);
    if (ret == 1)
        ossl_qlog_event_transport_packet_sent(qtx_get_qlog(qtx), hdr, pkt->pn,
                                              iovec, num_iovec,
                                              qtx->datagram_count);

    if (qtx->finishmutatecb != NULL)
        qtx->finishmutatecb(qtx->mutatearg);

    return ret;
}

static int addr_eq(const BIO_ADDR *a, const BIO_ADDR *b)
{
    return ((a == NULL || BIO_ADDR_family(a) == AF_UNSPEC)
            && (b == NULL || BIO_ADDR_family(b) == AF_UNSPEC))
        || (a != NULL && b != NULL && memcmp(a, b, sizeof(*a)) == 0);
}

int ossl_qtx_write_pkt(OSSL_QTX *qtx, const OSSL_QTX_PKT *pkt)
{
    int ret;
    int coalescing = (pkt->flags & OSSL_QTX_PKT_FLAG_COALESCE) != 0;
    int was_coalescing;
    TXE *txe;
    uint32_t enc_level;

    /* Must have EL configured, must have header. */
    if (pkt->hdr == NULL)
        return 0;

    enc_level = ossl_quic_pkt_type_to_enc_level(pkt->hdr->type);

    /* Some packet types must be in a packet all by themselves. */
    if (!ossl_quic_pkt_type_can_share_dgram(pkt->hdr->type))
        ossl_qtx_finish_dgram(qtx);
    else if (enc_level >= QUIC_ENC_LEVEL_NUM
               || ossl_qrl_enc_level_set_have_el(&qtx->el_set, enc_level) != 1) {
        /* All other packet types are encrypted. */
        return 0;
    }

    was_coalescing = (qtx->cons != NULL && qtx->cons->data_len > 0);
    if (was_coalescing)
        if (!addr_eq(&qtx->cons->peer, pkt->peer)
            || !addr_eq(&qtx->cons->local, pkt->local)) {
            /* Must stop coalescing if addresses have changed */
            ossl_qtx_finish_dgram(qtx);
            was_coalescing = 0;
        }

    for (;;) {
        /*
         * Start a new coalescing session or continue using the existing one and
         * serialize/encrypt the packet. We always encrypt packets as soon as
         * our caller gives them to us, which relieves the caller of any need to
         * keep the plaintext around.
         */
        txe = qtx_ensure_cons(qtx);
        if (txe == NULL)
            return 0; /* allocation failure */

        /*
         * Ensure TXE has at least MDPL bytes allocated. This should only be
         * possible if the MDPL has increased.
         */
        if (!qtx_reserve_txe(qtx, NULL, txe, qtx->mdpl))
            return 0;

        if (!was_coalescing) {
            /* Set addresses in TXE. */
            if (pkt->peer != NULL) {
                if (!BIO_ADDR_copy(&txe->peer, pkt->peer))
                    return 0;
            } else {
                BIO_ADDR_clear(&txe->peer);
            }

            if (pkt->local != NULL) {
                if (!BIO_ADDR_copy(&txe->local, pkt->local))
                    return 0;
            } else {
                BIO_ADDR_clear(&txe->local);
            }
        }

        ret = qtx_mutate_write(qtx, pkt, txe, enc_level);
        if (ret == 1) {
            break;
        } else if (ret == QTX_FAIL_INSUFFICIENT_LEN) {
            if (was_coalescing) {
                /*
                 * We failed due to insufficient length, so end the current
                 * datagram and try again.
                 */
                ossl_qtx_finish_dgram(qtx);
                was_coalescing = 0;
            } else {
                /*
                 * We failed due to insufficient length, but we were not
                 * coalescing/started with an empty datagram, so any future
                 * attempt to write this packet must also fail.
                 */
                return 0;
            }
        } else {
            return 0; /* other error */
        }
    }

    ++qtx->cons_count;

    /*
     * Some packet types cannot have another packet come after them.
     */
    if (ossl_quic_pkt_type_must_be_last(pkt->hdr->type))
        coalescing = 0;

    if (!coalescing)
        ossl_qtx_finish_dgram(qtx);

    return 1;
}

/*
 * Finish any incomplete datagrams for transmission which were flagged for
 * coalescing. If there is no current coalescing datagram, this is a no-op.
 */
void ossl_qtx_finish_dgram(OSSL_QTX *qtx)
{
    TXE *txe = qtx->cons;

    if (txe == NULL)
        return;

    if (txe->data_len == 0)
        /*
         * If we did not put anything in the datagram, just move it back to the
         * free list.
         */
        ossl_list_txe_insert_tail(&qtx->free, txe);
    else
        qtx_add_to_pending(qtx, txe);

    qtx->cons       = NULL;
    qtx->cons_count = 0;
    ++qtx->datagram_count;
}

static void txe_to_msg(TXE *txe, BIO_MSG *msg)
{
    msg->data       = txe_data(txe);
    msg->data_len   = txe->data_len;
    msg->flags      = 0;
    msg->peer
        = BIO_ADDR_family(&txe->peer) != AF_UNSPEC ? &txe->peer : NULL;
    msg->local
        = BIO_ADDR_family(&txe->local) != AF_UNSPEC ? &txe->local : NULL;
}

#define MAX_MSGS_PER_SEND   32

int ossl_qtx_flush_net(OSSL_QTX *qtx)
{
    BIO_MSG msg[MAX_MSGS_PER_SEND];
    size_t wr, i, total_written = 0;
    TXE *txe;
    int res;

    if (ossl_list_txe_head(&qtx->pending) == NULL)
        return QTX_FLUSH_NET_RES_OK; /* Nothing to send. */

    if (qtx->bio == NULL)
        return QTX_FLUSH_NET_RES_PERMANENT_FAIL;

    for (;;) {
        for (txe = ossl_list_txe_head(&qtx->pending), i = 0;
             txe != NULL && i < OSSL_NELEM(msg);
             txe = ossl_list_txe_next(txe), ++i)
            txe_to_msg(txe, &msg[i]);

        if (!i)
            /* Nothing to send. */
            break;

        ERR_set_mark();
        res = BIO_sendmmsg(qtx->bio, msg, sizeof(BIO_MSG), i, 0, &wr);
        if (res && wr == 0) {
            /*
             * Treat 0 messages sent as a transient error and just stop for now.
             */
            ERR_clear_last_mark();
            break;
        } else if (!res) {
            /*
             * We did not get anything, so further calls will probably not
             * succeed either.
             */
            if (BIO_err_is_non_fatal(ERR_peek_last_error())) {
                /* Transient error, just stop for now, clearing the error. */
                ERR_pop_to_mark();
                break;
            } else {
                /* Non-transient error, fail and do not clear the error. */
                ERR_clear_last_mark();
                return QTX_FLUSH_NET_RES_PERMANENT_FAIL;
            }
        }

        ERR_clear_last_mark();

        /*
         * Remove everything which was successfully sent from the pending queue.
         */
        for (i = 0; i < wr; ++i) {
            if (qtx->msg_callback != NULL)
                qtx->msg_callback(1, OSSL_QUIC1_VERSION, SSL3_RT_QUIC_DATAGRAM,
                                msg[i].data, msg[i].data_len,
                                qtx->msg_callback_ssl,
                                qtx->msg_callback_arg);
            qtx_pending_to_free(qtx);
        }

        total_written += wr;
    }

    return total_written > 0
        ? QTX_FLUSH_NET_RES_OK
        : QTX_FLUSH_NET_RES_TRANSIENT_FAIL;
}

int ossl_qtx_pop_net(OSSL_QTX *qtx, BIO_MSG *msg)
{
    TXE *txe = ossl_list_txe_head(&qtx->pending);

    if (txe == NULL)
        return 0;

    txe_to_msg(txe, msg);
    qtx_pending_to_free(qtx);
    return 1;
}

void ossl_qtx_set_bio(OSSL_QTX *qtx, BIO *bio)
{
    qtx->bio = bio;
}

int ossl_qtx_set_mdpl(OSSL_QTX *qtx, size_t mdpl)
{
    if (mdpl < QUIC_MIN_INITIAL_DGRAM_LEN)
        return 0;

    qtx->mdpl = mdpl;
    return 1;
}

size_t ossl_qtx_get_mdpl(OSSL_QTX *qtx)
{
    return qtx->mdpl;
}

size_t ossl_qtx_get_queue_len_datagrams(OSSL_QTX *qtx)
{
    return qtx->pending_count;
}

size_t ossl_qtx_get_queue_len_bytes(OSSL_QTX *qtx)
{
    return qtx->pending_bytes;
}

size_t ossl_qtx_get_cur_dgram_len_bytes(OSSL_QTX *qtx)
{
    return qtx->cons != NULL ? qtx->cons->data_len : 0;
}

size_t ossl_qtx_get_unflushed_pkt_count(OSSL_QTX *qtx)
{
    return qtx->cons_count;
}

int ossl_qtx_trigger_key_update(OSSL_QTX *qtx)
{
    return ossl_qrl_enc_level_set_key_update(&qtx->el_set,
                                             QUIC_ENC_LEVEL_1RTT);
}

uint64_t ossl_qtx_get_cur_epoch_pkt_count(OSSL_QTX *qtx, uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el;

    el = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
    if (el == NULL)
        return UINT64_MAX;

    return el->op_count;
}

uint64_t ossl_qtx_get_max_epoch_pkt_count(OSSL_QTX *qtx, uint32_t enc_level)
{
    OSSL_QRL_ENC_LEVEL *el;

    el = ossl_qrl_enc_level_set_get(&qtx->el_set, enc_level, 1);
    if (el == NULL)
        return UINT64_MAX;

    return ossl_qrl_get_suite_max_pkt(el->suite_id);
}

void ossl_qtx_set_msg_callback(OSSL_QTX *qtx, ossl_msg_cb msg_callback,
                               SSL *msg_callback_ssl)
{
    qtx->msg_callback = msg_callback;
    qtx->msg_callback_ssl = msg_callback_ssl;
}

void ossl_qtx_set_msg_callback_arg(OSSL_QTX *qtx, void *msg_callback_arg)
{
    qtx->msg_callback_arg = msg_callback_arg;
}

uint64_t ossl_qtx_get_key_epoch(OSSL_QTX *qtx)
{
    OSSL_QRL_ENC_LEVEL *el;

    el = ossl_qrl_enc_level_set_get(&qtx->el_set, QUIC_ENC_LEVEL_1RTT, 1);
    if (el == NULL)
        return 0;

    return el->key_epoch;
}
