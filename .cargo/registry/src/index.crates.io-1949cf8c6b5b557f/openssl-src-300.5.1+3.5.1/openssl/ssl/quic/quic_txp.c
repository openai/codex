/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/quic_txp.h"
#include "internal/quic_fifd.h"
#include "internal/quic_stream_map.h"
#include "internal/quic_error.h"
#include "internal/common.h"
#include <openssl/err.h>

#define MIN_CRYPTO_HDR_SIZE             3

#define MIN_FRAME_SIZE_HANDSHAKE_DONE   1
#define MIN_FRAME_SIZE_MAX_DATA         2
#define MIN_FRAME_SIZE_ACK              5
#define MIN_FRAME_SIZE_CRYPTO           (MIN_CRYPTO_HDR_SIZE + 1)
#define MIN_FRAME_SIZE_STREAM           3 /* minimum useful size (for non-FIN) */
#define MIN_FRAME_SIZE_MAX_STREAMS_BIDI 2
#define MIN_FRAME_SIZE_MAX_STREAMS_UNI  2

/*
 * Packet Archetypes
 * =================
 */

/* Generate normal packets containing most frame types, subject to EL. */
#define TX_PACKETISER_ARCHETYPE_NORMAL              0

/*
 * A probe packet is different in that:
 *   - It bypasses CC, but *is* counted as in flight for purposes of CC;
 *   - It must be ACK-eliciting.
 */
#define TX_PACKETISER_ARCHETYPE_PROBE               1

/*
 * An ACK-only packet is different in that:
 *   - It bypasses CC, and is considered a 'non-inflight' packet;
 *   - It may not contain anything other than an ACK frame, not even padding.
 */
#define TX_PACKETISER_ARCHETYPE_ACK_ONLY            2

#define TX_PACKETISER_ARCHETYPE_NUM                 3

struct ossl_quic_tx_packetiser_st {
    OSSL_QUIC_TX_PACKETISER_ARGS args;

    /*
     * Opaque initial token blob provided by caller. TXP frees using the
     * callback when it is no longer needed.
     */
    const unsigned char             *initial_token;
    size_t                          initial_token_len;
    ossl_quic_initial_token_free_fn *initial_token_free_cb;
    void                            *initial_token_free_cb_arg;

    /* Subcomponents of the TXP that we own. */
    QUIC_FIFD       fifd;       /* QUIC Frame-in-Flight Dispatcher */

    /* Internal state. */
    uint64_t        next_pn[QUIC_PN_SPACE_NUM]; /* Next PN to use in given PN space. */
    OSSL_TIME       last_tx_time;               /* Last time a packet was generated, or 0. */

    size_t          unvalidated_credit;         /* Limit of data we can send until validated */

    /* Internal state - frame (re)generation flags. */
    unsigned int    want_handshake_done     : 1;
    unsigned int    want_max_data           : 1;
    unsigned int    want_max_streams_bidi   : 1;
    unsigned int    want_max_streams_uni    : 1;

    /* Internal state - frame (re)generation flags - per PN space. */
    unsigned int    want_ack                : QUIC_PN_SPACE_NUM;
    unsigned int    force_ack_eliciting     : QUIC_PN_SPACE_NUM;

    /*
     * Internal state - connection close terminal state.
     * Once this is set, it is not unset unlike other want_ flags - we keep
     * sending it in every packet.
     */
    unsigned int    want_conn_close         : 1;

    /* Has the handshake been completed? */
    unsigned int    handshake_complete      : 1;

    OSSL_QUIC_FRAME_CONN_CLOSE  conn_close_frame;

    /*
     * Counts of the number of bytes received and sent while in the closing
     * state.
     */
    uint64_t                        closing_bytes_recv;
    uint64_t                        closing_bytes_xmit;

    /* Internal state - packet assembly. */
    struct txp_el {
        unsigned char   *scratch;       /* scratch buffer for packet assembly */
        size_t          scratch_len;    /* number of bytes allocated for scratch */
        OSSL_QTX_IOVEC  *iovec;         /* scratch iovec array for use with QTX */
        size_t          alloc_iovec;    /* size of iovec array */
    } el[QUIC_ENC_LEVEL_NUM];

    /* Message callback related arguments */
    ossl_msg_cb msg_callback;
    void *msg_callback_arg;
    SSL *msg_callback_ssl;

    /* Callbacks. */
    void            (*ack_tx_cb)(const OSSL_QUIC_FRAME_ACK *ack,
                                 uint32_t pn_space,
                                 void *arg);
    void            *ack_tx_cb_arg;
};

/*
 * The TX helper records state used while generating frames into packets. It
 * enables serialization into the packet to be done "transactionally" where
 * serialization of a frame can be rolled back if it fails midway (e.g. if it
 * does not fit).
 */
struct tx_helper {
    OSSL_QUIC_TX_PACKETISER *txp;
    /*
     * The Maximum Packet Payload Length in bytes. This is the amount of
     * space we have to generate frames into.
     */
    size_t max_ppl;
    /*
     * Number of bytes we have generated so far.
     */
    size_t bytes_appended;
    /*
     * Number of scratch bytes in txp->scratch we have used so far. Some iovecs
     * will reference this scratch buffer. When we need to use more of it (e.g.
     * when we need to put frame headers somewhere), we append to the scratch
     * buffer, resizing if necessary, and increase this accordingly.
     */
    size_t scratch_bytes;
    /*
     * Bytes reserved in the MaxPPL budget. We keep this number of bytes spare
     * until reserve_allowed is set to 1. Currently this is always at most 1, as
     * a PING frame takes up one byte and this mechanism is only used to ensure
     * we can encode a PING frame if we have been asked to ensure a packet is
     * ACK-eliciting and we are unusure if we are going to add any other
     * ACK-eliciting frames before we reach our MaxPPL budget.
     */
    size_t reserve;
    /*
     * Number of iovecs we have currently appended. This is the number of
     * entries valid in txp->iovec.
     */
    size_t num_iovec;
    /* The EL this TX helper is being used for. */
    uint32_t enc_level;
    /*
     * Whether we are allowed to make use of the reserve bytes in our MaxPPL
     * budget. This is used to ensure we have room to append a PING frame later
     * if we need to. Once we know we will not need to append a PING frame, this
     * is set to 1.
     */
    unsigned int reserve_allowed : 1;
    /*
     * Set to 1 if we have appended a STREAM frame with an implicit length. If
     * this happens we should never append another frame after that frame as it
     * cannot be validly encoded. This is just a safety check.
     */
    unsigned int done_implicit : 1;
    struct {
        /*
         * The fields in this structure are valid if active is set, which means
         * that a serialization transaction is currently in progress.
         */
        unsigned char   *data;
        WPACKET         wpkt;
        unsigned int    active : 1;
    } txn;
};

static void tx_helper_rollback(struct tx_helper *h);
static int txp_el_ensure_iovec(struct txp_el *el, size_t num);

/* Initialises the TX helper. */
static int tx_helper_init(struct tx_helper *h, OSSL_QUIC_TX_PACKETISER *txp,
                          uint32_t enc_level, size_t max_ppl, size_t reserve)
{
    if (reserve > max_ppl)
        return 0;

    h->txp                  = txp;
    h->enc_level            = enc_level;
    h->max_ppl              = max_ppl;
    h->reserve              = reserve;
    h->num_iovec            = 0;
    h->bytes_appended       = 0;
    h->scratch_bytes        = 0;
    h->reserve_allowed      = 0;
    h->done_implicit        = 0;
    h->txn.data             = NULL;
    h->txn.active           = 0;

    if (max_ppl > h->txp->el[enc_level].scratch_len) {
        unsigned char *scratch;

        scratch = OPENSSL_realloc(h->txp->el[enc_level].scratch, max_ppl);
        if (scratch == NULL)
            return 0;

        h->txp->el[enc_level].scratch     = scratch;
        h->txp->el[enc_level].scratch_len = max_ppl;
    }

    return 1;
}

static void tx_helper_cleanup(struct tx_helper *h)
{
    if (h->txn.active)
        tx_helper_rollback(h);

    h->txp = NULL;
}

static void tx_helper_unrestrict(struct tx_helper *h)
{
    h->reserve_allowed = 1;
}

/*
 * Append an extent of memory to the iovec list. The memory must remain
 * allocated until we finish generating the packet and call the QTX.
 *
 * In general, the buffers passed to this function will be from one of two
 * ranges:
 *
 *   - Application data contained in stream buffers managed elsewhere
 *     in the QUIC stack; or
 *
 *   - Control frame data appended into txp->scratch using tx_helper_begin and
 *     tx_helper_commit.
 *
 */
static int tx_helper_append_iovec(struct tx_helper *h,
                                  const unsigned char *buf,
                                  size_t buf_len)
{
    struct txp_el *el = &h->txp->el[h->enc_level];

    if (buf_len == 0)
        return 1;

    if (!ossl_assert(!h->done_implicit))
        return 0;

    if (!txp_el_ensure_iovec(el, h->num_iovec + 1))
        return 0;

    el->iovec[h->num_iovec].buf     = buf;
    el->iovec[h->num_iovec].buf_len = buf_len;

    ++h->num_iovec;
    h->bytes_appended += buf_len;
    return 1;
}

/*
 * How many more bytes of space do we have left in our plaintext packet payload?
 */
static size_t tx_helper_get_space_left(struct tx_helper *h)
{
    return h->max_ppl
        - (h->reserve_allowed ? 0 : h->reserve) - h->bytes_appended;
}

/*
 * Begin a control frame serialization transaction. This allows the
 * serialization of the control frame to be backed out if it turns out it won't
 * fit. Write the control frame to the returned WPACKET. Ensure you always
 * call tx_helper_rollback or tx_helper_commit (or tx_helper_cleanup). Returns
 * NULL on failure.
 */
static WPACKET *tx_helper_begin(struct tx_helper *h)
{
    size_t space_left, len;
    unsigned char *data;
    struct txp_el *el = &h->txp->el[h->enc_level];

    if (!ossl_assert(!h->txn.active))
        return NULL;

    if (!ossl_assert(!h->done_implicit))
        return NULL;

    data = (unsigned char *)el->scratch + h->scratch_bytes;
    len  = el->scratch_len - h->scratch_bytes;

    space_left = tx_helper_get_space_left(h);
    if (!ossl_assert(space_left <= len))
        return NULL;

    if (!WPACKET_init_static_len(&h->txn.wpkt, data, len, 0))
        return NULL;

    if (!WPACKET_set_max_size(&h->txn.wpkt, space_left)) {
        WPACKET_cleanup(&h->txn.wpkt);
        return NULL;
    }

    h->txn.data     = data;
    h->txn.active   = 1;
    return &h->txn.wpkt;
}

static void tx_helper_end(struct tx_helper *h, int success)
{
    if (success)
        WPACKET_finish(&h->txn.wpkt);
    else
        WPACKET_cleanup(&h->txn.wpkt);

    h->txn.active       = 0;
    h->txn.data         = NULL;
}

/* Abort a control frame serialization transaction. */
static void tx_helper_rollback(struct tx_helper *h)
{
    if (!h->txn.active)
        return;

    tx_helper_end(h, 0);
}

/* Commit a control frame. */
static int tx_helper_commit(struct tx_helper *h)
{
    size_t l = 0;

    if (!h->txn.active)
        return 0;

    if (!WPACKET_get_total_written(&h->txn.wpkt, &l)) {
        tx_helper_end(h, 0);
        return 0;
    }

    if (!tx_helper_append_iovec(h, h->txn.data, l)) {
        tx_helper_end(h, 0);
        return 0;
    }

    if (h->txp->msg_callback != NULL && l > 0) {
        uint64_t ftype;
        int ctype = SSL3_RT_QUIC_FRAME_FULL;
        PACKET pkt;

        if (!PACKET_buf_init(&pkt, h->txn.data, l)
                || !ossl_quic_wire_peek_frame_header(&pkt, &ftype, NULL)) {
            tx_helper_end(h, 0);
            return 0;
        }

        if (ftype == OSSL_QUIC_FRAME_TYPE_PADDING)
            ctype = SSL3_RT_QUIC_FRAME_PADDING;
        else if (OSSL_QUIC_FRAME_TYPE_IS_STREAM(ftype)
                || ftype == OSSL_QUIC_FRAME_TYPE_CRYPTO)
            ctype = SSL3_RT_QUIC_FRAME_HEADER;

        h->txp->msg_callback(1, OSSL_QUIC1_VERSION, ctype, h->txn.data, l,
                             h->txp->msg_callback_ssl,
                             h->txp->msg_callback_arg);
    }

    h->scratch_bytes += l;
    tx_helper_end(h, 1);
    return 1;
}

struct archetype_data {
    unsigned int allow_ack                  : 1;
    unsigned int allow_ping                 : 1;
    unsigned int allow_crypto               : 1;
    unsigned int allow_handshake_done       : 1;
    unsigned int allow_path_challenge       : 1;
    unsigned int allow_path_response        : 1;
    unsigned int allow_new_conn_id          : 1;
    unsigned int allow_retire_conn_id       : 1;
    unsigned int allow_stream_rel           : 1;
    unsigned int allow_conn_fc              : 1;
    unsigned int allow_conn_close           : 1;
    unsigned int allow_cfq_other            : 1;
    unsigned int allow_new_token            : 1;
    unsigned int allow_force_ack_eliciting  : 1;
    unsigned int allow_padding              : 1;
    unsigned int require_ack_eliciting      : 1;
    unsigned int bypass_cc                  : 1;
};

struct txp_pkt_geom {
    size_t                  cmpl, cmppl, hwm, pkt_overhead;
    uint32_t                archetype;
    struct archetype_data   adata;
};

struct txp_pkt {
    struct tx_helper    h;
    int                 h_valid;
    QUIC_TXPIM_PKT      *tpkt;
    QUIC_STREAM         *stream_head;
    QUIC_PKT_HDR        phdr;
    struct txp_pkt_geom geom;
    int                 force_pad;
};

static QUIC_SSTREAM *get_sstream_by_id(uint64_t stream_id, uint32_t pn_space,
                                       void *arg);
static void on_regen_notify(uint64_t frame_type, uint64_t stream_id,
                            QUIC_TXPIM_PKT *pkt, void *arg);
static void on_confirm_notify(uint64_t frame_type, uint64_t stream_id,
                              QUIC_TXPIM_PKT *pkt, void *arg);
static void on_sstream_updated(uint64_t stream_id, void *arg);
static int sstream_is_pending(QUIC_SSTREAM *sstream);
static int txp_should_try_staging(OSSL_QUIC_TX_PACKETISER *txp,
                                  uint32_t enc_level,
                                  uint32_t archetype,
                                  uint64_t cc_limit,
                                  uint32_t *conn_close_enc_level);
static size_t txp_determine_pn_len(OSSL_QUIC_TX_PACKETISER *txp);
static int txp_determine_ppl_from_pl(OSSL_QUIC_TX_PACKETISER *txp,
                                     size_t pl,
                                     uint32_t enc_level,
                                     size_t hdr_len,
                                     size_t *r);
static size_t txp_get_mdpl(OSSL_QUIC_TX_PACKETISER *txp);
static int txp_generate_for_el(OSSL_QUIC_TX_PACKETISER *txp,
                               struct txp_pkt *pkt,
                               int chosen_for_conn_close);
static int txp_pkt_init(struct txp_pkt *pkt, OSSL_QUIC_TX_PACKETISER *txp,
                        uint32_t enc_level, uint32_t archetype,
                        size_t running_total);
static void txp_pkt_cleanup(struct txp_pkt *pkt, OSSL_QUIC_TX_PACKETISER *txp);
static int txp_pkt_postgen_update_pkt_overhead(struct txp_pkt *pkt,
                                               OSSL_QUIC_TX_PACKETISER *txp);
static int txp_pkt_append_padding(struct txp_pkt *pkt,
                                  OSSL_QUIC_TX_PACKETISER *txp, size_t num_bytes);
static int txp_pkt_commit(OSSL_QUIC_TX_PACKETISER *txp, struct txp_pkt *pkt,
                          uint32_t archetype, int *txpim_pkt_reffed);
static uint32_t txp_determine_archetype(OSSL_QUIC_TX_PACKETISER *txp,
                                        uint64_t cc_limit);

/**
 * Sets the validated state of a QUIC TX packetiser.
 *
 * This function marks the provided QUIC TX packetiser as having its credit
 * fully validated by setting its `unvalidated_credit` field to `SIZE_MAX`.
 *
 * @param txp A pointer to the OSSL_QUIC_TX_PACKETISER structure to update.
 */
void ossl_quic_tx_packetiser_set_validated(OSSL_QUIC_TX_PACKETISER *txp)
{
    txp->unvalidated_credit = SIZE_MAX;
    return;
}

/**
 * Adds unvalidated credit to a QUIC TX packetiser.
 *
 * This function increases the unvalidated credit of the provided QUIC TX
 * packetiser. If the current unvalidated credit is not `SIZE_MAX`, the
 * function adds three times the specified `credit` value, ensuring it does
 * not exceed the maximum allowable value (`SIZE_MAX - 1`). If the addition
 * would cause an overflow, the unvalidated credit is capped at
 * `SIZE_MAX - 1`. If the current unvalidated credit is already `SIZE_MAX`,
 * the function does nothing.
 *
 * @param txp    A pointer to the OSSL_QUIC_TX_PACKETISER structure to update.
 * @param credit The amount of credit to add, multiplied by 3.
 */
void ossl_quic_tx_packetiser_add_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                    size_t credit)
{
    if (txp->unvalidated_credit != SIZE_MAX) {
        if ((SIZE_MAX - txp->unvalidated_credit) > (credit * 3))
            txp->unvalidated_credit += credit * 3;
        else
            txp->unvalidated_credit = SIZE_MAX - 1;
    }

    return;
}

/**
 * Consumes unvalidated credit from a QUIC TX packetiser.
 *
 * This function decreases the unvalidated credit of the specified
 * QUIC TX packetiser by the given `credit` value. If the unvalidated credit
 * is set to `SIZE_MAX`, the function does nothing, as `SIZE_MAX` represents
 * an unlimited credit state.
 *
 * @param txp    A pointer to the OSSL_QUIC_TX_PACKETISER structure to update.
 * @param credit The amount of credit to consume.
 */
void ossl_quic_tx_packetiser_consume_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                        size_t credit)
{
    if (txp->unvalidated_credit != SIZE_MAX) {
        if (txp->unvalidated_credit < credit)
            txp->unvalidated_credit = 0;
        else
            txp->unvalidated_credit -= credit;
    }
}

/**
 * Checks if the QUIC TX packetiser has sufficient unvalidated credit.
 *
 * This function determines whether the unvalidated credit of the specified
 * QUIC TX packetiser exceeds the required credit value (`req_credit`).
 * If the unvalidated credit is greater than `req_credit`, the function
 * returns 1 (true); otherwise, it returns 0 (false).
 *
 * @param txp        A pointer to the OSSL_QUIC_TX_PACKETISER structure to check.
 * @param req_credit The required credit value to compare against.
 *
 * @return 1 if the unvalidated credit exceeds `req_credit`, 0 otherwise.
 */
int ossl_quic_tx_packetiser_check_unvalidated_credit(OSSL_QUIC_TX_PACKETISER *txp,
                                                     size_t req_credit)
{
    return (txp->unvalidated_credit > req_credit);
}

OSSL_QUIC_TX_PACKETISER *ossl_quic_tx_packetiser_new(const OSSL_QUIC_TX_PACKETISER_ARGS *args)
{
    OSSL_QUIC_TX_PACKETISER *txp;

    if (args == NULL
        || args->qtx == NULL
        || args->txpim == NULL
        || args->cfq == NULL
        || args->ackm == NULL
        || args->qsm == NULL
        || args->conn_txfc == NULL
        || args->conn_rxfc == NULL
        || args->max_streams_bidi_rxfc == NULL
        || args->max_streams_uni_rxfc == NULL
        || args->protocol_version == 0) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return NULL;
    }

    txp = OPENSSL_zalloc(sizeof(*txp));
    if (txp == NULL)
        return NULL;

    txp->args           = *args;
    txp->last_tx_time   = ossl_time_zero();

    if (!ossl_quic_fifd_init(&txp->fifd,
                             txp->args.cfq, txp->args.ackm, txp->args.txpim,
                             get_sstream_by_id, txp,
                             on_regen_notify, txp,
                             on_confirm_notify, txp,
                             on_sstream_updated, txp,
                             args->get_qlog_cb,
                             args->get_qlog_cb_arg)) {
        OPENSSL_free(txp);
        return NULL;
    }

    return txp;
}

void ossl_quic_tx_packetiser_free(OSSL_QUIC_TX_PACKETISER *txp)
{
    uint32_t enc_level;

    if (txp == NULL)
        return;

    ossl_quic_tx_packetiser_set_initial_token(txp, NULL, 0, NULL, NULL);
    ossl_quic_fifd_cleanup(&txp->fifd);
    OPENSSL_free(txp->conn_close_frame.reason);

    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level) {
        OPENSSL_free(txp->el[enc_level].iovec);
        OPENSSL_free(txp->el[enc_level].scratch);
    }

    OPENSSL_free(txp);
}

/*
 * Determine if an Initial packet token length is reasonable based on the
 * current MDPL, returning 1 if it is OK.
 *
 * The real PMTU to the peer could differ from our (pessimistic) understanding
 * of the PMTU, therefore it is possible we could receive an Initial token from
 * a server in a Retry packet which is bigger than the MDPL. In this case it is
 * impossible for us ever to make forward progress and we need to error out
 * and fail the connection attempt.
 *
 * The specific boundary condition is complex: for example, after the size of
 * the Initial token, there are the Initial packet header overheads and then
 * encryption/AEAD tag overheads. After that, the minimum room for frame data in
 * order to guarantee forward progress must be guaranteed. For example, a crypto
 * stream needs to always be able to serialize at least one byte in a CRYPTO
 * frame in order to make forward progress. Because the offset field of a CRYPTO
 * frame uses a variable-length integer, the number of bytes needed to ensure
 * this also varies.
 *
 * Rather than trying to get this boundary condition check actually right,
 * require a reasonable amount of slack to avoid pathological behaviours. (After
 * all, transmitting a CRYPTO stream one byte at a time is probably not
 * desirable anyway.)
 *
 * We choose 160 bytes as the required margin, which is double the rough
 * estimation of the minimum we would require to guarantee forward progress
 * under worst case packet overheads.
 */
#define TXP_REQUIRED_TOKEN_MARGIN       160

static int txp_check_token_len(size_t token_len, size_t mdpl)
{
    if (token_len == 0)
        return 1;

    if (token_len >= mdpl)
        return 0;

    if (TXP_REQUIRED_TOKEN_MARGIN >= mdpl)
        /* (should not be possible because MDPL must be at least 1200) */
        return 0;

    if (token_len > mdpl - TXP_REQUIRED_TOKEN_MARGIN)
        return 0;

    return 1;
}

int ossl_quic_tx_packetiser_set_initial_token(OSSL_QUIC_TX_PACKETISER *txp,
                                              const unsigned char *token,
                                              size_t token_len,
                                              ossl_quic_initial_token_free_fn *free_cb,
                                              void *free_cb_arg)
{
    if (!txp_check_token_len(token_len, txp_get_mdpl(txp)))
        return 0;

    if (txp->initial_token != NULL && txp->initial_token_free_cb != NULL)
        txp->initial_token_free_cb(txp->initial_token, txp->initial_token_len,
                                   txp->initial_token_free_cb_arg);

    txp->initial_token              = token;
    txp->initial_token_len          = token_len;
    txp->initial_token_free_cb      = free_cb;
    txp->initial_token_free_cb_arg  = free_cb_arg;
    return 1;
}

int ossl_quic_tx_packetiser_set_protocol_version(OSSL_QUIC_TX_PACKETISER *txp,
                                                 uint32_t protocol_version)
{
    txp->args.protocol_version = protocol_version;
    return 1;
}

int ossl_quic_tx_packetiser_set_cur_dcid(OSSL_QUIC_TX_PACKETISER *txp,
                                         const QUIC_CONN_ID *dcid)
{
    if (dcid == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    txp->args.cur_dcid = *dcid;
    return 1;
}

int ossl_quic_tx_packetiser_set_cur_scid(OSSL_QUIC_TX_PACKETISER *txp,
                                         const QUIC_CONN_ID *scid)
{
    if (scid == NULL) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_NULL_PARAMETER);
        return 0;
    }

    txp->args.cur_scid = *scid;
    return 1;
}

/* Change the destination L4 address the TXP uses to send datagrams. */
int ossl_quic_tx_packetiser_set_peer(OSSL_QUIC_TX_PACKETISER *txp,
                                     const BIO_ADDR *peer)
{
    if (peer == NULL) {
        BIO_ADDR_clear(&txp->args.peer);
        return 1;
    }

    return BIO_ADDR_copy(&txp->args.peer, peer);
}

void ossl_quic_tx_packetiser_set_ack_tx_cb(OSSL_QUIC_TX_PACKETISER *txp,
                                           void (*cb)(const OSSL_QUIC_FRAME_ACK *ack,
                                                      uint32_t pn_space,
                                                      void *arg),
                                           void *cb_arg)
{
    txp->ack_tx_cb      = cb;
    txp->ack_tx_cb_arg  = cb_arg;
}

void ossl_quic_tx_packetiser_set_qlog_cb(OSSL_QUIC_TX_PACKETISER *txp,
                                         QLOG *(*get_qlog_cb)(void *arg),
                                         void *get_qlog_cb_arg)
{
    ossl_quic_fifd_set_qlog_cb(&txp->fifd, get_qlog_cb, get_qlog_cb_arg);

}

int ossl_quic_tx_packetiser_discard_enc_level(OSSL_QUIC_TX_PACKETISER *txp,
                                              uint32_t enc_level)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM) {
        ERR_raise(ERR_LIB_SSL, ERR_R_PASSED_INVALID_ARGUMENT);
        return 0;
    }

    if (enc_level != QUIC_ENC_LEVEL_0RTT)
        txp->args.crypto[ossl_quic_enc_level_to_pn_space(enc_level)] = NULL;

    return 1;
}

void ossl_quic_tx_packetiser_notify_handshake_complete(OSSL_QUIC_TX_PACKETISER *txp)
{
    txp->handshake_complete = 1;
}

void ossl_quic_tx_packetiser_schedule_handshake_done(OSSL_QUIC_TX_PACKETISER *txp)
{
    txp->want_handshake_done = 1;
}

void ossl_quic_tx_packetiser_schedule_ack_eliciting(OSSL_QUIC_TX_PACKETISER *txp,
                                                    uint32_t pn_space)
{
    txp->force_ack_eliciting |= (1UL << pn_space);
}

void ossl_quic_tx_packetiser_schedule_ack(OSSL_QUIC_TX_PACKETISER *txp,
                                          uint32_t pn_space)
{
    txp->want_ack |= (1UL << pn_space);
}

#define TXP_ERR_INTERNAL     0  /* Internal (e.g. alloc) error */
#define TXP_ERR_SUCCESS      1  /* Success */
#define TXP_ERR_SPACE        2  /* Not enough room for another packet */
#define TXP_ERR_INPUT        3  /* Invalid/malformed input */

/*
 * Generates a datagram by polling the various ELs to determine if they want to
 * generate any frames, and generating a datagram which coalesces packets for
 * any ELs which do.
 */
int ossl_quic_tx_packetiser_generate(OSSL_QUIC_TX_PACKETISER *txp,
                                     QUIC_TXP_STATUS *status)
{
    /*
     * Called to generate one or more datagrams, each containing one or more
     * packets.
     *
     * There are some tricky things to note here:
     *
     *   - The TXP is only concerned with generating encrypted packets;
     *     other packets use a different path.
     *
     *   - Any datagram containing an Initial packet must have a payload length
     *     (DPL) of at least 1200 bytes. This padding need not necessarily be
     *     found in the Initial packet.
     *
     *     - It is desirable to be able to coalesce an Initial packet
     *       with a Handshake packet. Since, before generating the Handshake
     *       packet, we do not know how long it will be, we cannot know the
     *       correct amount of padding to ensure a DPL of at least 1200 bytes.
     *       Thus this padding must added to the Handshake packet (or whatever
     *       packet is the last in the datagram).
     *
     *     - However, at the time that we generate the Initial packet,
     *       we do not actually know for sure that we will be followed
     *       in the datagram by another packet. For example, suppose we have
     *       some queued data (e.g. crypto stream data for the HANDSHAKE EL)
     *       it looks like we will want to send on the HANDSHAKE EL.
     *       We could assume padding will be placed in the Handshake packet
     *       subsequently and avoid adding any padding to the Initial packet
     *       (which would leave no room for the Handshake packet in the
     *       datagram).
     *
     *       However, this is not actually a safe assumption. Suppose that we
     *       are using a link with a MDPL of 1200 bytes, the minimum allowed by
     *       QUIC. Suppose that the Initial packet consumes 1195 bytes in total.
     *       Since it is not possible to fit a Handshake packet in just 5 bytes,
     *       upon trying to add a Handshake packet after generating the Initial
     *       packet, we will discover we have no room to fit it! This is not a
     *       problem in itself as another datagram can be sent subsequently, but
     *       it is a problem because we were counting to use that packet to hold
     *       the essential padding. But if we have already finished encrypting
     *       the Initial packet, we cannot go and add padding to it anymore.
     *       This leaves us stuck.
     *
     * Because of this, we have to plan multiple packets simultaneously, such
     * that we can start generating a Handshake (or 0-RTT or 1-RTT, or so on)
     * packet while still having the option to go back and add padding to the
     * Initial packet if it turns out to be needed.
     *
     * Trying to predict ahead of time (e.g. during Initial packet generation)
     * whether we will successfully generate a subsequent packet is fraught with
     * error as it relies on a large number of variables:
     *
     *   - Do we have room to fit a packet header? (Consider that due to
     *     variable-length integer encoding this is highly variable and can even
     *     depend on payload length due to a variable-length Length field.)
     *
     *   - Can we fit even a single one of the frames we want to put in this
     *     packet in the packet? (Each frame type has a bespoke encoding. While
     *     our encodings of some frame types are adaptive based on the available
     *     room - e.g. STREAM frames - ultimately all frame types have some
     *     absolute minimum number of bytes to be successfully encoded. For
     *     example, if after an Initial packet there is enough room to encode
     *     only one byte of frame data, it is quite likely we can't send any of
     *     the frames we wanted to send.) While this is not strictly a problem
     *     because we could just fill the packet with padding frames, this is a
     *     pointless packet and is wasteful.
     *
     * Thus we adopt a multi-phase architecture:
     *
     *   1. Archetype Selection: Determine desired packet archetype.
     *
     *   2. Packet Staging: Generation of packet information and packet payload
     *      data (frame data) into staging areas.
     *
     *   3. Packet Adjustment: Adjustment of staged packets, adding padding to
     *      the staged packets if needed.
     *
     *   4. Commit: The packets are sent to the QTX and recorded as having been
     *      sent to the FIFM.
     *
     */
    int res = 0, rc;
    uint32_t archetype, enc_level;
    uint32_t conn_close_enc_level = QUIC_ENC_LEVEL_NUM;
    struct txp_pkt pkt[QUIC_ENC_LEVEL_NUM];
    size_t pkts_done = 0;
    uint64_t cc_limit = txp->args.cc_method->get_tx_allowance(txp->args.cc_data);
    int need_padding = 0, txpim_pkt_reffed;

    memset(status, 0, sizeof(*status));

    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level)
        pkt[enc_level].h_valid = 0;


    /*
     * Should not be needed, but a sanity check in case anyone else has been
     * using the QTX.
     */
    ossl_qtx_finish_dgram(txp->args.qtx);

    /* 1. Archetype Selection */
    archetype = txp_determine_archetype(txp, cc_limit);

    /* 2. Packet Staging */
    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level) {
        size_t running_total = (enc_level > QUIC_ENC_LEVEL_INITIAL)
            ? pkt[enc_level - 1].geom.hwm : 0;

        pkt[enc_level].geom.hwm = running_total;

        if (!txp_should_try_staging(txp, enc_level, archetype, cc_limit,
                                    &conn_close_enc_level))
            continue;

        if (!txp_pkt_init(&pkt[enc_level], txp, enc_level, archetype,
                          running_total))
            /*
             * If this fails this is not a fatal error - it means the geometry
             * planning determined there was not enough space for another
             * packet. So just proceed with what we've already planned for.
             */
            break;

        rc = txp_generate_for_el(txp, &pkt[enc_level],
                                 conn_close_enc_level == enc_level);
        if (rc != TXP_ERR_SUCCESS)
            goto out;

        if (pkt[enc_level].force_pad)
            /*
             * txp_generate_for_el emitted a frame which forces packet padding.
             */
            need_padding = 1;

        pkt[enc_level].geom.hwm = running_total
            + pkt[enc_level].h.bytes_appended
            + pkt[enc_level].geom.pkt_overhead;
    }

    /* 3. Packet Adjustment */
    if (pkt[QUIC_ENC_LEVEL_INITIAL].h_valid
        && pkt[QUIC_ENC_LEVEL_INITIAL].h.bytes_appended > 0)
        /*
         * We have an Initial packet in this datagram, so we need to make sure
         * the total size of the datagram is adequate.
         */
        need_padding = 1;

    if (need_padding) {
        size_t total_dgram_size = 0;
        const size_t min_dpl = QUIC_MIN_INITIAL_DGRAM_LEN;
        uint32_t pad_el = QUIC_ENC_LEVEL_NUM;

        for (enc_level = QUIC_ENC_LEVEL_INITIAL;
             enc_level < QUIC_ENC_LEVEL_NUM;
             ++enc_level)
            if (pkt[enc_level].h_valid && pkt[enc_level].h.bytes_appended > 0) {
                if (pad_el == QUIC_ENC_LEVEL_NUM
                    /*
                     * We might not be able to add padding, for example if we
                     * are using the ACK_ONLY archetype.
                     */
                    && pkt[enc_level].geom.adata.allow_padding
                    && !pkt[enc_level].h.done_implicit)
                    pad_el = enc_level;

                txp_pkt_postgen_update_pkt_overhead(&pkt[enc_level], txp);
                total_dgram_size += pkt[enc_level].geom.pkt_overhead
                    + pkt[enc_level].h.bytes_appended;
            }

        if (pad_el != QUIC_ENC_LEVEL_NUM && total_dgram_size < min_dpl) {
            size_t deficit = min_dpl - total_dgram_size;

            if (!txp_pkt_append_padding(&pkt[pad_el], txp, deficit))
                goto out;

            total_dgram_size += deficit;

            /*
             * Padding frames make a packet ineligible for being a non-inflight
             * packet.
             */
            pkt[pad_el].tpkt->ackm_pkt.is_inflight = 1;
        }

        /*
         * If we have failed to make a datagram of adequate size, for example
         * because we have a padding requirement but are using the ACK_ONLY
         * archetype (because we are CC limited), which precludes us from
         * sending padding, give up on generating the datagram - there is
         * nothing we can do.
         */
        if (total_dgram_size < min_dpl) {
            res = 1;
            goto out;
        }
    }

    /* 4. Commit */
    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level) {

        if (!pkt[enc_level].h_valid)
            /* Did not attempt to generate a packet for this EL. */
            continue;

        if (pkt[enc_level].h.bytes_appended == 0)
            /* Nothing was generated for this EL, so skip. */
            continue;

        if (!ossl_quic_tx_packetiser_check_unvalidated_credit(txp,
                                                              pkt[enc_level].h.bytes_appended)) {
            res = TXP_ERR_SPACE;
            goto out;
        }
        ossl_quic_tx_packetiser_consume_unvalidated_credit(txp, pkt[enc_level].h.bytes_appended);

        rc = txp_pkt_commit(txp, &pkt[enc_level], archetype,
                            &txpim_pkt_reffed);
        if (rc) {
            status->sent_ack_eliciting
                = status->sent_ack_eliciting
                || pkt[enc_level].tpkt->ackm_pkt.is_ack_eliciting;

            if (enc_level == QUIC_ENC_LEVEL_HANDSHAKE)
                status->sent_handshake
                    = (pkt[enc_level].h_valid
                       && pkt[enc_level].h.bytes_appended > 0);
        }

        if (txpim_pkt_reffed)
            pkt[enc_level].tpkt = NULL; /* don't free */

        if (!rc)
            goto out;

        ++pkts_done;

    }

    /* Flush & Cleanup */
    res = 1;
out:
    ossl_qtx_finish_dgram(txp->args.qtx);

    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level)
        txp_pkt_cleanup(&pkt[enc_level], txp);

    status->sent_pkt = pkts_done;

    return res;
}

static const struct archetype_data archetypes[QUIC_ENC_LEVEL_NUM][TX_PACKETISER_ARCHETYPE_NUM] = {
    /* EL 0(INITIAL) */
    {
        /* EL 0(INITIAL) - Archetype 0(NORMAL) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 0,
        },
        /* EL 0(INITIAL) - Archetype 1(PROBE) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 1,
            /*bypass_cc                       =*/ 1,
        },
        /* EL 0(INITIAL) - Archetype 2(ACK_ONLY) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 0,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 0,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 0,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 1,
        },
    },
    /* EL 1(0RTT) */
    {
        /* EL 1(0RTT) - Archetype 0(NORMAL) */
        {
            /*allow_ack                       =*/ 0,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 1,
            /*allow_retire_conn_id            =*/ 1,
            /*allow_stream_rel                =*/ 1,
            /*allow_conn_fc                   =*/ 1,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 0,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 0,
        },
        /* EL 1(0RTT) - Archetype 1(PROBE) */
        {
            /*allow_ack                       =*/ 0,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 1,
            /*allow_retire_conn_id            =*/ 1,
            /*allow_stream_rel                =*/ 1,
            /*allow_conn_fc                   =*/ 1,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 0,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 1,
            /*bypass_cc                       =*/ 1,
        },
        /* EL 1(0RTT) - Archetype 2(ACK_ONLY) */
        {
            /*allow_ack                       =*/ 0,
            /*allow_ping                      =*/ 0,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 0,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 0,
            /*allow_padding                   =*/ 0,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 1,
        },
    },
    /* EL (HANDSHAKE) */
    {
        /* EL 2(HANDSHAKE) - Archetype 0(NORMAL) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 0,
        },
        /* EL 2(HANDSHAKE) - Archetype 1(PROBE) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 1,
            /*bypass_cc                       =*/ 1,
        },
        /* EL 2(HANDSHAKE) - Archetype 2(ACK_ONLY) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 0,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 0,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 0,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 1,
        },
    },
    /* EL 3(1RTT) */
    {
        /* EL 3(1RTT) - Archetype 0(NORMAL) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 1,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 1,
            /*allow_new_conn_id               =*/ 1,
            /*allow_retire_conn_id            =*/ 1,
            /*allow_stream_rel                =*/ 1,
            /*allow_conn_fc                   =*/ 1,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 1,
            /*allow_new_token                 =*/ 1,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 0,
        },
        /* EL 3(1RTT) - Archetype 1(PROBE) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 1,
            /*allow_crypto                    =*/ 1,
            /*allow_handshake_done            =*/ 1,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 1,
            /*allow_new_conn_id               =*/ 1,
            /*allow_retire_conn_id            =*/ 1,
            /*allow_stream_rel                =*/ 1,
            /*allow_conn_fc                   =*/ 1,
            /*allow_conn_close                =*/ 1,
            /*allow_cfq_other                 =*/ 1,
            /*allow_new_token                 =*/ 1,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 1,
            /*require_ack_eliciting           =*/ 1,
            /*bypass_cc                       =*/ 1,
        },
        /* EL 3(1RTT) - Archetype 2(ACK_ONLY) */
        {
            /*allow_ack                       =*/ 1,
            /*allow_ping                      =*/ 0,
            /*allow_crypto                    =*/ 0,
            /*allow_handshake_done            =*/ 0,
            /*allow_path_challenge            =*/ 0,
            /*allow_path_response             =*/ 0,
            /*allow_new_conn_id               =*/ 0,
            /*allow_retire_conn_id            =*/ 0,
            /*allow_stream_rel                =*/ 0,
            /*allow_conn_fc                   =*/ 0,
            /*allow_conn_close                =*/ 0,
            /*allow_cfq_other                 =*/ 0,
            /*allow_new_token                 =*/ 0,
            /*allow_force_ack_eliciting       =*/ 1,
            /*allow_padding                   =*/ 0,
            /*require_ack_eliciting           =*/ 0,
            /*bypass_cc                       =*/ 1,
        }
    }
};

static int txp_get_archetype_data(uint32_t enc_level,
                                  uint32_t archetype,
                                  struct archetype_data *a)
{
    if (enc_level >= QUIC_ENC_LEVEL_NUM
        || archetype >= TX_PACKETISER_ARCHETYPE_NUM)
        return 0;

    /* No need to avoid copying this as it should not exceed one int in size. */
    *a = archetypes[enc_level][archetype];
    return 1;
}

static int txp_determine_geometry(OSSL_QUIC_TX_PACKETISER *txp,
                                  uint32_t archetype,
                                  uint32_t enc_level,
                                  size_t running_total,
                                  QUIC_PKT_HDR *phdr,
                                  struct txp_pkt_geom *geom)
{
    size_t mdpl, cmpl, hdr_len;

    /* Get information about packet archetype. */
    if (!txp_get_archetype_data(enc_level, archetype, &geom->adata))
       return 0;

    /* Assemble packet header. */
    phdr->type          = ossl_quic_enc_level_to_pkt_type(enc_level);
    phdr->spin_bit      = 0;
    phdr->pn_len        = txp_determine_pn_len(txp);
    phdr->partial       = 0;
    phdr->fixed         = 1;
    phdr->reserved      = 0;
    phdr->version       = txp->args.protocol_version;
    phdr->dst_conn_id   = txp->args.cur_dcid;
    phdr->src_conn_id   = txp->args.cur_scid;

    /*
     * We need to know the length of the payload to get an accurate header
     * length for non-1RTT packets, because the Length field found in
     * Initial/Handshake/0-RTT packets uses a variable-length encoding. However,
     * we don't have a good idea of the length of our payload, because the
     * length of the payload depends on the room in the datagram after fitting
     * the header, which depends on the size of the header.
     *
     * In general, it does not matter if a packet is slightly shorter (because
     * e.g. we predicted use of a 2-byte length field, but ended up only needing
     * a 1-byte length field). However this does matter for Initial packets
     * which must be at least 1200 bytes, which is also the assumed default MTU;
     * therefore in many cases Initial packets will be padded to 1200 bytes,
     * which means if we overestimated the header size, we will be short by a
     * few bytes and the server will ignore the packet for being too short. In
     * this case, however, such packets always *will* be padded to meet 1200
     * bytes, which requires a 2-byte length field, so we don't actually need to
     * worry about this. Thus we estimate the header length assuming a 2-byte
     * length field here, which should in practice work well in all cases.
     */
    phdr->len           = OSSL_QUIC_VLINT_2B_MAX - phdr->pn_len;

    if (enc_level == QUIC_ENC_LEVEL_INITIAL) {
        phdr->token     = txp->initial_token;
        phdr->token_len = txp->initial_token_len;
    } else {
        phdr->token     = NULL;
        phdr->token_len = 0;
    }

    hdr_len = ossl_quic_wire_get_encoded_pkt_hdr_len(phdr->dst_conn_id.id_len,
                                                     phdr);
    if (hdr_len == 0)
        return 0;

    /* MDPL: Maximum datagram payload length. */
    mdpl = txp_get_mdpl(txp);

    /*
     * CMPL: Maximum encoded packet size we can put into this datagram given any
     * previous packets coalesced into it.
     */
    if (running_total > mdpl)
        /* Should not be possible, but if it happens: */
        cmpl = 0;
    else
        cmpl = mdpl - running_total;

    /* CMPPL: Maximum amount we can put into the current packet payload */
    if (!txp_determine_ppl_from_pl(txp, cmpl, enc_level, hdr_len, &geom->cmppl))
        return 0;

    geom->cmpl                  = cmpl;
    geom->pkt_overhead          = cmpl - geom->cmppl;
    geom->archetype             = archetype;
    return 1;
}

static uint32_t txp_determine_archetype(OSSL_QUIC_TX_PACKETISER *txp,
                                        uint64_t cc_limit)
{
    OSSL_ACKM_PROBE_INFO *probe_info
        = ossl_ackm_get0_probe_request(txp->args.ackm);
    uint32_t pn_space;

    /*
     * If ACKM has requested probe generation (e.g. due to PTO), we generate a
     * Probe-archetype packet. Actually, we determine archetype on a
     * per-datagram basis, so if any EL wants a probe, do a pass in which
     * we try and generate a probe (if needed) for all ELs.
     */
    if (probe_info->anti_deadlock_initial > 0
        || probe_info->anti_deadlock_handshake > 0)
        return TX_PACKETISER_ARCHETYPE_PROBE;

    for (pn_space = QUIC_PN_SPACE_INITIAL;
         pn_space < QUIC_PN_SPACE_NUM;
         ++pn_space)
        if (probe_info->pto[pn_space] > 0)
            return TX_PACKETISER_ARCHETYPE_PROBE;

    /*
     * If we are out of CC budget, we cannot send a normal packet,
     * but we can do an ACK-only packet (potentially, if we
     * want to send an ACK).
     */
    if (cc_limit == 0)
        return TX_PACKETISER_ARCHETYPE_ACK_ONLY;

    /* All other packets. */
    return TX_PACKETISER_ARCHETYPE_NORMAL;
}

static int txp_should_try_staging(OSSL_QUIC_TX_PACKETISER *txp,
                                  uint32_t enc_level,
                                  uint32_t archetype,
                                  uint64_t cc_limit,
                                  uint32_t *conn_close_enc_level)
{
    struct archetype_data a;
    uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    QUIC_CFQ_ITEM *cfq_item;

    if (!ossl_qtx_is_enc_level_provisioned(txp->args.qtx, enc_level))
        return 0;

    if (!txp_get_archetype_data(enc_level, archetype, &a))
        return 0;

    if (!a.bypass_cc && cc_limit == 0)
        /* CC not allowing us to send. */
        return 0;

    /*
     * We can produce CONNECTION_CLOSE frames on any EL in principle, which
     * means we need to choose which EL we would prefer to use. After a
     * connection is fully established we have only one provisioned EL and this
     * is a non-issue. Where multiple ELs are provisioned, it is possible the
     * peer does not have the keys for the EL yet, which suggests in general it
     * is preferable to use the lowest EL which is still provisioned.
     *
     * However (RFC 9000 s. 10.2.3 & 12.5) we are also required to not send
     * application CONNECTION_CLOSE frames in non-1-RTT ELs, so as to not
     * potentially leak application data on a connection which has yet to be
     * authenticated. Thus when we have an application CONNECTION_CLOSE frame
     * queued and need to send it on a non-1-RTT EL, we have to convert it
     * into a transport CONNECTION_CLOSE frame which contains no application
     * data. Since this loses information, it suggests we should use the 1-RTT
     * EL to avoid this if possible, even if a lower EL is also available.
     *
     * At the same time, just because we have the 1-RTT EL provisioned locally
     * does not necessarily mean the peer does, for example if a handshake
     * CRYPTO frame has been lost. It is fairly important that CONNECTION_CLOSE
     * is signalled in a way we know our peer can decrypt, as we stop processing
     * connection retransmission logic for real after connection close and
     * simply 'blindly' retransmit the same CONNECTION_CLOSE frame.
     *
     * This is not a major concern for clients, since if a client has a 1-RTT EL
     * provisioned the server is guaranteed to also have a 1-RTT EL provisioned.
     *
     * TODO(QUIC FUTURE): Revisit this when when have reached a decision on how
     * best to implement this
     */
    if (*conn_close_enc_level > enc_level
        && *conn_close_enc_level != QUIC_ENC_LEVEL_1RTT)
        *conn_close_enc_level = enc_level;

    /* Do we need to send a PTO probe? */
    if (a.allow_force_ack_eliciting) {
        OSSL_ACKM_PROBE_INFO *probe_info
            = ossl_ackm_get0_probe_request(txp->args.ackm);

        if ((enc_level == QUIC_ENC_LEVEL_INITIAL
             && probe_info->anti_deadlock_initial > 0)
            || (enc_level == QUIC_ENC_LEVEL_HANDSHAKE
                && probe_info->anti_deadlock_handshake > 0)
            || probe_info->pto[pn_space] > 0)
            return 1;
    }

    /* Does the crypto stream for this EL want to produce anything? */
    if (a.allow_crypto && sstream_is_pending(txp->args.crypto[pn_space]))
        return 1;

    /* Does the ACKM for this PN space want to produce anything? */
    if (a.allow_ack && (ossl_ackm_is_ack_desired(txp->args.ackm, pn_space)
                        || (txp->want_ack & (1UL << pn_space)) != 0))
        return 1;

    /* Do we need to force emission of an ACK-eliciting packet? */
    if (a.allow_force_ack_eliciting
        && (txp->force_ack_eliciting & (1UL << pn_space)) != 0)
        return 1;

    /* Does the connection-level RXFC want to produce a frame? */
    if (a.allow_conn_fc && (txp->want_max_data
        || ossl_quic_rxfc_has_cwm_changed(txp->args.conn_rxfc, 0)))
        return 1;

    /* Do we want to produce a MAX_STREAMS frame? */
    if (a.allow_conn_fc
        && (txp->want_max_streams_bidi
            || ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_bidi_rxfc,
                                              0)
            || txp->want_max_streams_uni
            || ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_uni_rxfc,
                                              0)))
        return 1;

    /* Do we want to produce a HANDSHAKE_DONE frame? */
    if (a.allow_handshake_done && txp->want_handshake_done)
        return 1;

    /* Do we want to produce a CONNECTION_CLOSE frame? */
    if (a.allow_conn_close && txp->want_conn_close &&
        *conn_close_enc_level == enc_level)
        /*
         * This is a bit of a special case since CONNECTION_CLOSE can appear in
         * most packet types, and when we decide we want to send it this status
         * isn't tied to a specific EL. So if we want to send it, we send it
         * only on the lowest non-dropped EL.
         */
        return 1;

    /* Does the CFQ have any frames queued for this PN space? */
    if (enc_level != QUIC_ENC_LEVEL_0RTT)
        for (cfq_item = ossl_quic_cfq_get_priority_head(txp->args.cfq, pn_space);
             cfq_item != NULL;
             cfq_item = ossl_quic_cfq_item_get_priority_next(cfq_item, pn_space)) {
            uint64_t frame_type = ossl_quic_cfq_item_get_frame_type(cfq_item);

            switch (frame_type) {
            case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
                if (a.allow_new_conn_id)
                    return 1;
                break;
            case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
                if (a.allow_retire_conn_id)
                    return 1;
                break;
            case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
                if (a.allow_new_token)
                    return 1;
                break;
            case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
                if (a.allow_path_response)
                    return 1;
                break;
            default:
                if (a.allow_cfq_other)
                    return 1;
                break;
            }
       }

    if (a.allow_stream_rel && txp->handshake_complete) {
        QUIC_STREAM_ITER it;

        /* If there are any active streams, 0/1-RTT wants to produce a packet.
         * Whether a stream is on the active list is required to be precise
         * (i.e., a stream is never on the active list if we cannot produce a
         * frame for it), and all stream-related frames are governed by
         * a.allow_stream_rel (i.e., if we can send one type of stream-related
         * frame, we can send any of them), so we don't need to inspect
         * individual streams on the active list, just confirm that the active
         * list is non-empty.
         */
        ossl_quic_stream_iter_init(&it, txp->args.qsm, 0);
        if (it.stream != NULL)
            return 1;
    }

    return 0;
}

static int sstream_is_pending(QUIC_SSTREAM *sstream)
{
    OSSL_QUIC_FRAME_STREAM hdr;
    OSSL_QTX_IOVEC iov[2];
    size_t num_iov = OSSL_NELEM(iov);

    return ossl_quic_sstream_get_stream_frame(sstream, 0, &hdr, iov, &num_iov);
}

/* Determine how many bytes we should use for the encoded PN. */
static size_t txp_determine_pn_len(OSSL_QUIC_TX_PACKETISER *txp)
{
    return 4; /* TODO(QUIC FUTURE) */
}

/* Determine plaintext packet payload length from payload length. */
static int txp_determine_ppl_from_pl(OSSL_QUIC_TX_PACKETISER *txp,
                                     size_t pl,
                                     uint32_t enc_level,
                                     size_t hdr_len,
                                     size_t *r)
{
    if (pl < hdr_len)
        return 0;

    pl -= hdr_len;

    if (!ossl_qtx_calculate_plaintext_payload_len(txp->args.qtx, enc_level,
                                                  pl, &pl))
        return 0;

    *r = pl;
    return 1;
}

static size_t txp_get_mdpl(OSSL_QUIC_TX_PACKETISER *txp)
{
    return ossl_qtx_get_mdpl(txp->args.qtx);
}

static QUIC_SSTREAM *get_sstream_by_id(uint64_t stream_id, uint32_t pn_space,
                                       void *arg)
{
    OSSL_QUIC_TX_PACKETISER *txp = arg;
    QUIC_STREAM *s;

    if (stream_id == UINT64_MAX)
        return txp->args.crypto[pn_space];

    s = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);
    if (s == NULL)
        return NULL;

    return s->sstream;
}

static void on_regen_notify(uint64_t frame_type, uint64_t stream_id,
                            QUIC_TXPIM_PKT *pkt, void *arg)
{
    OSSL_QUIC_TX_PACKETISER *txp = arg;

    switch (frame_type) {
        case OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE:
            txp->want_handshake_done = 1;
            break;
        case OSSL_QUIC_FRAME_TYPE_MAX_DATA:
            txp->want_max_data = 1;
            break;
        case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI:
            txp->want_max_streams_bidi = 1;
            break;
        case OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI:
            txp->want_max_streams_uni = 1;
            break;
        case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
            txp->want_ack |= (1UL << pkt->ackm_pkt.pkt_space);
            break;
        case OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA:
            {
                QUIC_STREAM *s
                    = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);

                if (s == NULL)
                    return;

                s->want_max_stream_data = 1;
                ossl_quic_stream_map_update_state(txp->args.qsm, s);
            }
            break;
        case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
            {
                QUIC_STREAM *s
                    = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);

                if (s == NULL)
                    return;

                ossl_quic_stream_map_schedule_stop_sending(txp->args.qsm, s);
            }
            break;
        case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
            {
                QUIC_STREAM *s
                    = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);

                if (s == NULL)
                    return;

                s->want_reset_stream = 1;
                ossl_quic_stream_map_update_state(txp->args.qsm, s);
            }
            break;
        default:
            assert(0);
            break;
    }
}

static int txp_need_ping(OSSL_QUIC_TX_PACKETISER *txp,
                         uint32_t pn_space,
                         const struct archetype_data *adata)
{
    return adata->allow_ping
        && (adata->require_ack_eliciting
            || (txp->force_ack_eliciting & (1UL << pn_space)) != 0);
}

static int txp_pkt_init(struct txp_pkt *pkt, OSSL_QUIC_TX_PACKETISER *txp,
                        uint32_t enc_level, uint32_t archetype,
                        size_t running_total)
{
    uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);

    if (!txp_determine_geometry(txp, archetype, enc_level,
                                running_total, &pkt->phdr, &pkt->geom))
        return 0;

    /*
     * Initialise TX helper. If we must be ACK eliciting, reserve 1 byte for
     * PING.
     */
    if (!tx_helper_init(&pkt->h, txp, enc_level,
                        pkt->geom.cmppl,
                        txp_need_ping(txp, pn_space, &pkt->geom.adata) ? 1 : 0))
        return 0;

    pkt->h_valid            = 1;
    pkt->tpkt               = NULL;
    pkt->stream_head        = NULL;
    pkt->force_pad          = 0;
    return 1;
}

static void txp_pkt_cleanup(struct txp_pkt *pkt, OSSL_QUIC_TX_PACKETISER *txp)
{
    if (!pkt->h_valid)
        return;

    tx_helper_cleanup(&pkt->h);
    pkt->h_valid = 0;

    if (pkt->tpkt != NULL) {
        ossl_quic_txpim_pkt_release(txp->args.txpim, pkt->tpkt);
        pkt->tpkt = NULL;
    }
}

static int txp_pkt_postgen_update_pkt_overhead(struct txp_pkt *pkt,
                                               OSSL_QUIC_TX_PACKETISER *txp)
{
    /*
     * After we have staged and generated our packets, but before we commit
     * them, it is possible for the estimated packet overhead (packet header +
     * AEAD tag size) to shrink slightly because we generated a short packet
     * whose which can be represented in fewer bytes as a variable-length
     * integer than we were (pessimistically) budgeting for. We need to account
     * for this to ensure that we get our padding calculation exactly right.
     *
     * Update pkt_overhead to be accurate now that we know how much data is
     * going in a packet.
     */
    size_t hdr_len, ciphertext_len;

    if (pkt->h.enc_level == QUIC_ENC_LEVEL_INITIAL)
        /*
         * Don't update overheads for the INITIAL EL - we have not finished
         * appending padding to it and would potentially miscalculate the
         * correct padding if we now update the pkt_overhead field to switch to
         * e.g. a 1-byte length field in the packet header. Since we are padding
         * to QUIC_MIN_INITIAL_DGRAM_LEN which requires a 2-byte length field,
         * this is guaranteed to be moot anyway. See comment in
         * txp_determine_geometry for more information.
         */
        return 1;

    if (!ossl_qtx_calculate_ciphertext_payload_len(txp->args.qtx, pkt->h.enc_level,
                                                   pkt->h.bytes_appended,
                                                   &ciphertext_len))
        return 0;

    pkt->phdr.len = ciphertext_len;

    hdr_len = ossl_quic_wire_get_encoded_pkt_hdr_len(pkt->phdr.dst_conn_id.id_len,
                                                     &pkt->phdr);

    pkt->geom.pkt_overhead = hdr_len + ciphertext_len - pkt->h.bytes_appended;
    return 1;
}

static void on_confirm_notify(uint64_t frame_type, uint64_t stream_id,
                              QUIC_TXPIM_PKT *pkt, void *arg)
{
    OSSL_QUIC_TX_PACKETISER *txp = arg;

    switch (frame_type) {
        case OSSL_QUIC_FRAME_TYPE_STOP_SENDING:
            {
                QUIC_STREAM *s
                    = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);

                if (s == NULL)
                    return;

                s->acked_stop_sending = 1;
                ossl_quic_stream_map_update_state(txp->args.qsm, s);
            }
            break;
        case OSSL_QUIC_FRAME_TYPE_RESET_STREAM:
            {
                QUIC_STREAM *s
                    = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);

                if (s == NULL)
                    return;

                /*
                 * We must already be in RESET_SENT or RESET_RECVD if we are
                 * here, so we don't need to check state here.
                 */
                ossl_quic_stream_map_notify_reset_stream_acked(txp->args.qsm, s);
                ossl_quic_stream_map_update_state(txp->args.qsm, s);
            }
            break;
        default:
            assert(0);
            break;
    }
}

static int txp_pkt_append_padding(struct txp_pkt *pkt,
                                  OSSL_QUIC_TX_PACKETISER *txp, size_t num_bytes)
{
    WPACKET *wpkt;

    if (num_bytes == 0)
        return 1;

    if (!ossl_assert(pkt->h_valid))
        return 0;

    if (!ossl_assert(pkt->tpkt != NULL))
        return 0;

    wpkt = tx_helper_begin(&pkt->h);
    if (wpkt == NULL)
        return 0;

    if (!ossl_quic_wire_encode_padding(wpkt, num_bytes)) {
        tx_helper_rollback(&pkt->h);
        return 0;
    }

    if (!tx_helper_commit(&pkt->h))
        return 0;

    pkt->tpkt->ackm_pkt.num_bytes      += num_bytes;
    /* Cannot be non-inflight if we have a PADDING frame */
    pkt->tpkt->ackm_pkt.is_inflight     = 1;
    return 1;
}

static void on_sstream_updated(uint64_t stream_id, void *arg)
{
    OSSL_QUIC_TX_PACKETISER *txp = arg;
    QUIC_STREAM *s;

    s = ossl_quic_stream_map_get_by_id(txp->args.qsm, stream_id);
    if (s == NULL)
        return;

    ossl_quic_stream_map_update_state(txp->args.qsm, s);
}

/*
 * Returns 1 if we can send that many bytes in closing state, 0 otherwise.
 * Also maintains the bytes sent state if it returns a success.
 */
static int try_commit_conn_close(OSSL_QUIC_TX_PACKETISER *txp, size_t n)
{
    int res;

    /* We can always send the first connection close frame */
    if (txp->closing_bytes_recv == 0)
        return 1;

    /*
     * RFC 9000 s. 10.2.1 Closing Connection State:
     *      To avoid being used for an amplification attack, such
     *      endpoints MUST limit the cumulative size of packets it sends
     *      to three times the cumulative size of the packets that are
     *      received and attributed to the connection.
     * and:
     *      An endpoint in the closing state MUST either discard packets
     *      received from an unvalidated address or limit the cumulative
     *      size of packets it sends to an unvalidated address to three
     *      times the size of packets it receives from that address.
     */
    res = txp->closing_bytes_xmit + n <= txp->closing_bytes_recv * 3;

    /*
     * Attribute the bytes to the connection, if we are allowed to send them
     * and this isn't the first closing frame.
     */
    if (res && txp->closing_bytes_recv != 0)
        txp->closing_bytes_xmit += n;
    return res;
}

void ossl_quic_tx_packetiser_record_received_closing_bytes(
        OSSL_QUIC_TX_PACKETISER *txp, size_t n)
{
    txp->closing_bytes_recv += n;
}

static int txp_generate_pre_token(OSSL_QUIC_TX_PACKETISER *txp,
                                  struct txp_pkt *pkt,
                                  int chosen_for_conn_close,
                                  int *can_be_non_inflight)
{
    const uint32_t enc_level = pkt->h.enc_level;
    const uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    const struct archetype_data *a = &pkt->geom.adata;
    QUIC_TXPIM_PKT *tpkt = pkt->tpkt;
    struct tx_helper *h = &pkt->h;
    const OSSL_QUIC_FRAME_ACK *ack;
    OSSL_QUIC_FRAME_ACK ack2;

    tpkt->ackm_pkt.largest_acked = QUIC_PN_INVALID;

    /* ACK Frames (Regenerate) */
    if (a->allow_ack
        && tx_helper_get_space_left(h) >= MIN_FRAME_SIZE_ACK
        && (((txp->want_ack & (1UL << pn_space)) != 0)
            || ossl_ackm_is_ack_desired(txp->args.ackm, pn_space))
        && (ack = ossl_ackm_get_ack_frame(txp->args.ackm, pn_space)) != NULL) {
        WPACKET *wpkt = tx_helper_begin(h);

        if (wpkt == NULL)
            return 0;

        /* We do not currently support ECN */
        ack2 = *ack;
        ack2.ecn_present = 0;

        if (ossl_quic_wire_encode_frame_ack(wpkt,
                                            txp->args.ack_delay_exponent,
                                            &ack2)) {
            if (!tx_helper_commit(h))
                return 0;

            tpkt->had_ack_frame = 1;

            if (ack->num_ack_ranges > 0)
                tpkt->ackm_pkt.largest_acked = ack->ack_ranges[0].end;

            if (txp->ack_tx_cb != NULL)
                txp->ack_tx_cb(&ack2, pn_space, txp->ack_tx_cb_arg);
        } else {
            tx_helper_rollback(h);
        }
    }

    /* CONNECTION_CLOSE Frames (Regenerate) */
    if (a->allow_conn_close && txp->want_conn_close && chosen_for_conn_close) {
        WPACKET *wpkt = tx_helper_begin(h);
        OSSL_QUIC_FRAME_CONN_CLOSE f, *pf = &txp->conn_close_frame;
        size_t l;

        if (wpkt == NULL)
            return 0;

        /*
         * Application CONNECTION_CLOSE frames may only be sent in the
         * Application PN space, as otherwise they may be sent before a
         * connection is authenticated and leak application data. Therefore, if
         * we need to send a CONNECTION_CLOSE frame in another PN space and were
         * given an application CONNECTION_CLOSE frame, convert it into a
         * transport CONNECTION_CLOSE frame, removing any sensitive application
         * data.
         *
         * RFC 9000 s. 10.2.3: "A CONNECTION_CLOSE of type 0x1d MUST be replaced
         * by a CONNECTION_CLOSE of type 0x1c when sending the frame in Initial
         * or Handshake packets. Otherwise, information about the application
         * state might be revealed. Endpoints MUST clear the value of the Reason
         * Phrase field and SHOULD use the APPLICATION_ERROR code when
         * converting to a CONNECTION_CLOSE of type 0x1c."
         */
        if (pn_space != QUIC_PN_SPACE_APP && pf->is_app) {
            pf = &f;
            pf->is_app      = 0;
            pf->frame_type  = 0;
            pf->error_code  = OSSL_QUIC_ERR_APPLICATION_ERROR;
            pf->reason      = NULL;
            pf->reason_len  = 0;
        }

        if (ossl_quic_wire_encode_frame_conn_close(wpkt, pf)
                && WPACKET_get_total_written(wpkt, &l)
                && try_commit_conn_close(txp, l)) {
            if (!tx_helper_commit(h))
                return 0;

            tpkt->had_conn_close = 1;
            *can_be_non_inflight = 0;
        } else {
            tx_helper_rollback(h);
        }
    }

    return 1;
}

static int try_len(size_t space_left, size_t orig_len,
                   size_t base_hdr_len, size_t lenbytes,
                   uint64_t maxn, size_t *hdr_len, size_t *payload_len)
{
    size_t n;
    size_t maxn_ = maxn > SIZE_MAX ? SIZE_MAX : (size_t)maxn;

    *hdr_len = base_hdr_len + lenbytes;

    if (orig_len == 0 && space_left >= *hdr_len) {
        *payload_len = 0;
        return 1;
    }

    n = orig_len;
    if (n > maxn_)
        n = maxn_;
    if (n + *hdr_len > space_left)
        n = (space_left >= *hdr_len) ? space_left - *hdr_len : 0;

    *payload_len = n;
    return n > 0;
}

static int determine_len(size_t space_left, size_t orig_len,
                         size_t base_hdr_len,
                         uint64_t *hlen, uint64_t *len)
{
    int ok = 0;
    size_t chosen_payload_len = 0;
    size_t chosen_hdr_len     = 0;
    size_t payload_len[4], hdr_len[4];
    int i, valid[4] = {0};

    valid[0] = try_len(space_left, orig_len, base_hdr_len,
                       1, OSSL_QUIC_VLINT_1B_MAX,
                       &hdr_len[0], &payload_len[0]);
    valid[1] = try_len(space_left, orig_len, base_hdr_len,
                       2, OSSL_QUIC_VLINT_2B_MAX,
                       &hdr_len[1], &payload_len[1]);
    valid[2] = try_len(space_left, orig_len, base_hdr_len,
                       4, OSSL_QUIC_VLINT_4B_MAX,
                       &hdr_len[2], &payload_len[2]);
    valid[3] = try_len(space_left, orig_len, base_hdr_len,
                       8, OSSL_QUIC_VLINT_8B_MAX,
                       &hdr_len[3], &payload_len[3]);

   for (i = OSSL_NELEM(valid) - 1; i >= 0; --i)
        if (valid[i] && payload_len[i] >= chosen_payload_len) {
            chosen_payload_len = payload_len[i];
            chosen_hdr_len     = hdr_len[i];
            ok                 = 1;
        }

    *hlen = chosen_hdr_len;
    *len  = chosen_payload_len;
    return ok;
}

/*
 * Given a CRYPTO frame header with accurate chdr->len and a budget
 * (space_left), try to find the optimal value of chdr->len to fill as much of
 * the budget as possible. This is slightly hairy because larger values of
 * chdr->len cause larger encoded sizes of the length field of the frame, which
 * in turn mean less space available for payload data. We check all possible
 * encodings and choose the optimal encoding.
 */
static int determine_crypto_len(struct tx_helper *h,
                                OSSL_QUIC_FRAME_CRYPTO *chdr,
                                size_t space_left,
                                uint64_t *hlen,
                                uint64_t *len)
{
    size_t orig_len;
    size_t base_hdr_len; /* CRYPTO header length without length field */

    if (chdr->len > SIZE_MAX)
        return 0;

    orig_len = (size_t)chdr->len;

    chdr->len = 0;
    base_hdr_len = ossl_quic_wire_get_encoded_frame_len_crypto_hdr(chdr);
    chdr->len = orig_len;
    if (base_hdr_len == 0)
        return 0;

    --base_hdr_len;

    return determine_len(space_left, orig_len, base_hdr_len, hlen, len);
}

static int determine_stream_len(struct tx_helper *h,
                                OSSL_QUIC_FRAME_STREAM *shdr,
                                size_t space_left,
                                uint64_t *hlen,
                                uint64_t *len)
{
    size_t orig_len;
    size_t base_hdr_len; /* STREAM header length without length field */

    if (shdr->len > SIZE_MAX)
        return 0;

    orig_len = (size_t)shdr->len;

    shdr->len = 0;
    base_hdr_len = ossl_quic_wire_get_encoded_frame_len_stream_hdr(shdr);
    shdr->len = orig_len;
    if (base_hdr_len == 0)
        return 0;

    if (shdr->has_explicit_len)
        --base_hdr_len;

    return determine_len(space_left, orig_len, base_hdr_len, hlen, len);
}

static int txp_generate_crypto_frames(OSSL_QUIC_TX_PACKETISER *txp,
                                      struct txp_pkt *pkt,
                                      int *have_ack_eliciting)
{
    const uint32_t enc_level = pkt->h.enc_level;
    const uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    QUIC_TXPIM_PKT *tpkt = pkt->tpkt;
    struct tx_helper *h = &pkt->h;
    size_t num_stream_iovec;
    OSSL_QUIC_FRAME_STREAM shdr = {0};
    OSSL_QUIC_FRAME_CRYPTO chdr = {0};
    OSSL_QTX_IOVEC iov[2];
    uint64_t hdr_bytes;
    WPACKET *wpkt;
    QUIC_TXPIM_CHUNK chunk = {0};
    size_t i, space_left;

    for (i = 0;; ++i) {
        space_left = tx_helper_get_space_left(h);

        if (space_left < MIN_FRAME_SIZE_CRYPTO)
            return 1; /* no point trying */

        /* Do we have any CRYPTO data waiting? */
        num_stream_iovec = OSSL_NELEM(iov);
        if (!ossl_quic_sstream_get_stream_frame(txp->args.crypto[pn_space],
                                                i, &shdr, iov,
                                                &num_stream_iovec))
            return 1; /* nothing to do */

        /* Convert STREAM frame header to CRYPTO frame header */
        chdr.offset = shdr.offset;
        chdr.len    = shdr.len;

        if (chdr.len == 0)
            return 1; /* nothing to do */

        /* Find best fit (header length, payload length) combination. */
        if (!determine_crypto_len(h, &chdr, space_left, &hdr_bytes,
                                  &chdr.len))
            return 1; /* can't fit anything */

        /*
         * Truncate IOVs to match our chosen length.
         *
         * The length cannot be more than SIZE_MAX because this length comes
         * from our send stream buffer.
         */
        ossl_quic_sstream_adjust_iov((size_t)chdr.len, iov, num_stream_iovec);

        /*
         * Ensure we have enough iovecs allocated (1 for the header, up to 2 for
         * the stream data.)
         */
        if (!txp_el_ensure_iovec(&txp->el[enc_level], h->num_iovec + 3))
            return 0; /* alloc error */

        /* Encode the header. */
        wpkt = tx_helper_begin(h);
        if (wpkt == NULL)
            return 0; /* alloc error */

        if (!ossl_quic_wire_encode_frame_crypto_hdr(wpkt, &chdr)) {
            tx_helper_rollback(h);
            return 1; /* can't fit */
        }

        if (!tx_helper_commit(h))
            return 0; /* alloc error */

        /* Add payload iovecs to the helper (infallible). */
        for (i = 0; i < num_stream_iovec; ++i)
            tx_helper_append_iovec(h, iov[i].buf, iov[i].buf_len);

        *have_ack_eliciting = 1;
        tx_helper_unrestrict(h); /* no longer need PING */

        /* Log chunk to TXPIM. */
        chunk.stream_id = UINT64_MAX; /* crypto stream */
        chunk.start     = chdr.offset;
        chunk.end       = chdr.offset + chdr.len - 1;
        chunk.has_fin   = 0; /* Crypto stream never ends */
        if (!ossl_quic_txpim_pkt_append_chunk(tpkt, &chunk))
            return 0; /* alloc error */
    }
}

struct chunk_info {
    OSSL_QUIC_FRAME_STREAM shdr;
    uint64_t orig_len;
    OSSL_QTX_IOVEC iov[2];
    size_t num_stream_iovec;
    int valid;
};

static int txp_plan_stream_chunk(OSSL_QUIC_TX_PACKETISER *txp,
                                 struct tx_helper *h,
                                 QUIC_SSTREAM *sstream,
                                 QUIC_TXFC *stream_txfc,
                                 size_t skip,
                                 struct chunk_info *chunk,
                                 uint64_t consumed)
{
    uint64_t fc_credit, fc_swm, fc_limit;

    chunk->num_stream_iovec = OSSL_NELEM(chunk->iov);
    chunk->valid = ossl_quic_sstream_get_stream_frame(sstream, skip,
                                                      &chunk->shdr,
                                                      chunk->iov,
                                                      &chunk->num_stream_iovec);
    if (!chunk->valid)
        return 1;

    if (!ossl_assert(chunk->shdr.len > 0 || chunk->shdr.is_fin))
        /* Should only have 0-length chunk if FIN */
        return 0;

    chunk->orig_len = chunk->shdr.len;

    /* Clamp according to connection and stream-level TXFC. */
    fc_credit   = ossl_quic_txfc_get_credit(stream_txfc, consumed);
    fc_swm      = ossl_quic_txfc_get_swm(stream_txfc);
    fc_limit    = fc_swm + fc_credit;

    if (chunk->shdr.len > 0 && chunk->shdr.offset + chunk->shdr.len > fc_limit) {
        chunk->shdr.len = (fc_limit <= chunk->shdr.offset)
            ? 0 : fc_limit - chunk->shdr.offset;
        chunk->shdr.is_fin = 0;
    }

    if (chunk->shdr.len == 0 && !chunk->shdr.is_fin) {
        /*
         * Nothing to do due to TXFC. Since SSTREAM returns chunks in ascending
         * order of offset we don't need to check any later chunks, so stop
         * iterating here.
         */
        chunk->valid = 0;
        return 1;
    }

    return 1;
}

/*
 * Returns 0 on fatal error (e.g. allocation failure), 1 on success.
 * *packet_full is set to 1 if there is no longer enough room for another STREAM
 * frame.
 */
static int txp_generate_stream_frames(OSSL_QUIC_TX_PACKETISER *txp,
                                      struct txp_pkt *pkt,
                                      uint64_t id,
                                      QUIC_SSTREAM *sstream,
                                      QUIC_TXFC *stream_txfc,
                                      QUIC_STREAM *next_stream,
                                      int *have_ack_eliciting,
                                      int *packet_full,
                                      uint64_t *new_credit_consumed,
                                      uint64_t conn_consumed)
{
    int rc = 0;
    struct chunk_info chunks[2] = {0};
    const uint32_t enc_level = pkt->h.enc_level;
    QUIC_TXPIM_PKT *tpkt = pkt->tpkt;
    struct tx_helper *h = &pkt->h;
    OSSL_QUIC_FRAME_STREAM *shdr;
    WPACKET *wpkt;
    QUIC_TXPIM_CHUNK chunk;
    size_t i, j, space_left;
    int can_fill_payload, use_explicit_len;
    int could_have_following_chunk;
    uint64_t orig_len;
    uint64_t hdr_len_implicit, payload_len_implicit;
    uint64_t hdr_len_explicit, payload_len_explicit;
    uint64_t fc_swm, fc_new_hwm;

    fc_swm      = ossl_quic_txfc_get_swm(stream_txfc);
    fc_new_hwm  = fc_swm;

    /*
     * Load the first two chunks if any offered by the send stream. We retrieve
     * the next chunk in advance so we can determine if we need to send any more
     * chunks from the same stream after this one, which is needed when
     * determining when we can use an implicit length in a STREAM frame.
     */
    for (i = 0; i < 2; ++i) {
        if (!txp_plan_stream_chunk(txp, h, sstream, stream_txfc, i, &chunks[i],
                                   conn_consumed))
            goto err;

        if (i == 0 && !chunks[i].valid) {
            /* No chunks, nothing to do. */
            rc = 1;
            goto err;
        }
        chunks[i].shdr.stream_id = id;
    }

    for (i = 0;; ++i) {
        space_left = tx_helper_get_space_left(h);

        if (!chunks[i % 2].valid) {
            /* Out of chunks; we're done. */
            rc = 1;
            goto err;
        }

        if (space_left < MIN_FRAME_SIZE_STREAM) {
            *packet_full = 1;
            rc = 1;
            goto err;
        }

        if (!ossl_assert(!h->done_implicit))
            /*
             * Logic below should have ensured we didn't append an
             * implicit-length unless we filled the packet or didn't have
             * another stream to handle, so this should not be possible.
             */
            goto err;

        shdr = &chunks[i % 2].shdr;
        orig_len = chunks[i % 2].orig_len;
        if (i > 0)
            /* Load next chunk for lookahead. */
            if (!txp_plan_stream_chunk(txp, h, sstream, stream_txfc, i + 1,
                                       &chunks[(i + 1) % 2], conn_consumed))
                goto err;

        /*
         * Find best fit (header length, payload length) combination for if we
         * use an implicit length.
         */
        shdr->has_explicit_len = 0;
        hdr_len_implicit = payload_len_implicit = 0;
        if (!determine_stream_len(h, shdr, space_left,
                                  &hdr_len_implicit, &payload_len_implicit)) {
            *packet_full = 1;
            rc = 1;
            goto err; /* can't fit anything */
        }

        /*
         * If there is a next stream, we don't use the implicit length so we can
         * add more STREAM frames after this one, unless there is enough data
         * for this STREAM frame to fill the packet.
         */
        can_fill_payload = (hdr_len_implicit + payload_len_implicit
                            >= space_left);

        /*
         * Is there is a stream after this one, or another chunk pending
         * transmission in this stream?
         */
        could_have_following_chunk
            = (next_stream != NULL || chunks[(i + 1) % 2].valid);

        /* Choose between explicit or implicit length representations. */
        use_explicit_len = !((can_fill_payload || !could_have_following_chunk)
                             && !pkt->force_pad);

        if (use_explicit_len) {
            /*
             * Find best fit (header length, payload length) combination for if
             * we use an explicit length.
             */
            shdr->has_explicit_len = 1;
            hdr_len_explicit = payload_len_explicit = 0;
            if (!determine_stream_len(h, shdr, space_left,
                                      &hdr_len_explicit, &payload_len_explicit)) {
                *packet_full = 1;
                rc = 1;
                goto err; /* can't fit anything */
            }

            shdr->len = payload_len_explicit;
        } else {
            *packet_full = 1;
            shdr->has_explicit_len = 0;
            shdr->len = payload_len_implicit;
        }

        /* If this is a FIN, don't keep filling the packet with more FINs. */
        if (shdr->is_fin)
            chunks[(i + 1) % 2].valid = 0;

        /*
         * We are now committed to our length (shdr->len can't change).
         * If we truncated the chunk, clear the FIN bit.
         */
        if (shdr->len < orig_len)
            shdr->is_fin = 0;

        /* Truncate IOVs to match our chosen length. */
        ossl_quic_sstream_adjust_iov((size_t)shdr->len, chunks[i % 2].iov,
                                     chunks[i % 2].num_stream_iovec);

        /*
         * Ensure we have enough iovecs allocated (1 for the header, up to 2 for
         * the stream data.)
         */
        if (!txp_el_ensure_iovec(&txp->el[enc_level], h->num_iovec + 3))
            goto err; /* alloc error */

        /* Encode the header. */
        wpkt = tx_helper_begin(h);
        if (wpkt == NULL)
            goto err; /* alloc error */

        if (!ossl_assert(ossl_quic_wire_encode_frame_stream_hdr(wpkt, shdr))) {
            /* (Should not be possible.) */
            tx_helper_rollback(h);
            *packet_full = 1;
            rc = 1;
            goto err; /* can't fit */
        }

        if (!tx_helper_commit(h))
            goto err; /* alloc error */

        /* Add payload iovecs to the helper (infallible). */
        for (j = 0; j < chunks[i % 2].num_stream_iovec; ++j)
            tx_helper_append_iovec(h, chunks[i % 2].iov[j].buf,
                                   chunks[i % 2].iov[j].buf_len);

        *have_ack_eliciting = 1;
        tx_helper_unrestrict(h); /* no longer need PING */
        if (!shdr->has_explicit_len)
            h->done_implicit = 1;

        /* Log new TXFC credit which was consumed. */
        if (shdr->len > 0 && shdr->offset + shdr->len > fc_new_hwm)
            fc_new_hwm = shdr->offset + shdr->len;

        /* Log chunk to TXPIM. */
        chunk.stream_id         = shdr->stream_id;
        chunk.start             = shdr->offset;
        chunk.end               = shdr->offset + shdr->len - 1;
        chunk.has_fin           = shdr->is_fin;
        chunk.has_stop_sending  = 0;
        chunk.has_reset_stream  = 0;
        if (!ossl_quic_txpim_pkt_append_chunk(tpkt, &chunk))
            goto err; /* alloc error */

        if (shdr->len < orig_len) {
            /*
             * If we did not serialize all of this chunk we definitely do not
             * want to try the next chunk
             */
            rc = 1;
            goto err;
        }
    }

err:
    *new_credit_consumed = fc_new_hwm - fc_swm;
    return rc;
}

static void txp_enlink_tmp(QUIC_STREAM **tmp_head, QUIC_STREAM *stream)
{
    stream->txp_next = *tmp_head;
    *tmp_head = stream;
}

static int txp_generate_stream_related(OSSL_QUIC_TX_PACKETISER *txp,
                                       struct txp_pkt *pkt,
                                       int *have_ack_eliciting,
                                       QUIC_STREAM **tmp_head)
{
    QUIC_STREAM_ITER it;
    WPACKET *wpkt;
    uint64_t cwm;
    QUIC_STREAM *stream, *snext;
    struct tx_helper *h = &pkt->h;
    uint64_t conn_consumed = 0;

    for (ossl_quic_stream_iter_init(&it, txp->args.qsm, 1);
         it.stream != NULL;) {

        stream = it.stream;
        ossl_quic_stream_iter_next(&it);
        snext = it.stream;

        stream->txp_sent_fc                  = 0;
        stream->txp_sent_stop_sending        = 0;
        stream->txp_sent_reset_stream        = 0;
        stream->txp_blocked                  = 0;
        stream->txp_txfc_new_credit_consumed = 0;

        /* Stream Abort Frames (STOP_SENDING, RESET_STREAM) */
        if (stream->want_stop_sending) {
            OSSL_QUIC_FRAME_STOP_SENDING f;

            wpkt = tx_helper_begin(h);
            if (wpkt == NULL)
                return 0; /* alloc error */

            f.stream_id         = stream->id;
            f.app_error_code    = stream->stop_sending_aec;
            if (!ossl_quic_wire_encode_frame_stop_sending(wpkt, &f)) {
                tx_helper_rollback(h); /* can't fit */
                txp_enlink_tmp(tmp_head, stream);
                break;
            }

            if (!tx_helper_commit(h))
                return 0; /* alloc error */

            *have_ack_eliciting = 1;
            tx_helper_unrestrict(h); /* no longer need PING */
            stream->txp_sent_stop_sending = 1;
        }

        if (stream->want_reset_stream) {
            OSSL_QUIC_FRAME_RESET_STREAM f;

            if (!ossl_assert(stream->send_state == QUIC_SSTREAM_STATE_RESET_SENT))
                return 0;

            wpkt = tx_helper_begin(h);
            if (wpkt == NULL)
                return 0; /* alloc error */

            f.stream_id         = stream->id;
            f.app_error_code    = stream->reset_stream_aec;
            if (!ossl_quic_stream_send_get_final_size(stream, &f.final_size))
                return 0; /* should not be possible */

            if (!ossl_quic_wire_encode_frame_reset_stream(wpkt, &f)) {
                tx_helper_rollback(h); /* can't fit */
                txp_enlink_tmp(tmp_head, stream);
                break;
            }

            if (!tx_helper_commit(h))
                return 0; /* alloc error */

            *have_ack_eliciting = 1;
            tx_helper_unrestrict(h); /* no longer need PING */
            stream->txp_sent_reset_stream = 1;

            /*
             * The final size of the stream as indicated by RESET_STREAM is used
             * to ensure a consistent view of flow control state by both
             * parties; if we happen to send a RESET_STREAM that consumes more
             * flow control credit, make sure we account for that.
             */
            if (!ossl_assert(f.final_size <= ossl_quic_txfc_get_swm(&stream->txfc)))
                return 0;

            stream->txp_txfc_new_credit_consumed
                = f.final_size - ossl_quic_txfc_get_swm(&stream->txfc);
        }

        /*
         * Stream Flow Control Frames (MAX_STREAM_DATA)
         *
         * RFC 9000 s. 13.3: "An endpoint SHOULD stop sending MAX_STREAM_DATA
         * frames when the receiving part of the stream enters a "Size Known" or
         * "Reset Recvd" state." -- In practice, RECV is the only state
         * in which it makes sense to generate more MAX_STREAM_DATA frames.
         */
        if (stream->recv_state == QUIC_RSTREAM_STATE_RECV
            && (stream->want_max_stream_data
                || ossl_quic_rxfc_has_cwm_changed(&stream->rxfc, 0))) {

            wpkt = tx_helper_begin(h);
            if (wpkt == NULL)
                return 0; /* alloc error */

            cwm = ossl_quic_rxfc_get_cwm(&stream->rxfc);

            if (!ossl_quic_wire_encode_frame_max_stream_data(wpkt, stream->id,
                                                             cwm)) {
                tx_helper_rollback(h); /* can't fit */
                txp_enlink_tmp(tmp_head, stream);
                break;
            }

            if (!tx_helper_commit(h))
                return 0; /* alloc error */

            *have_ack_eliciting = 1;
            tx_helper_unrestrict(h); /* no longer need PING */
            stream->txp_sent_fc = 1;
        }

        /*
         * Stream Data Frames (STREAM)
         *
         * RFC 9000 s. 3.3: A sender MUST NOT send a STREAM [...] frame for a
         * stream in the "Reset Sent" state [or any terminal state]. We don't
         * send any more STREAM frames if we are sending, have sent, or are
         * planning to send, RESET_STREAM. The other terminal state is Data
         * Recvd, but txp_generate_stream_frames() is guaranteed to generate
         * nothing in this case.
         */
        if (ossl_quic_stream_has_send_buffer(stream)
            && !ossl_quic_stream_send_is_reset(stream)) {
            int packet_full = 0;

            if (!ossl_assert(!stream->want_reset_stream))
                return 0;

            if (!txp_generate_stream_frames(txp, pkt,
                                            stream->id, stream->sstream,
                                            &stream->txfc,
                                            snext,
                                            have_ack_eliciting,
                                            &packet_full,
                                            &stream->txp_txfc_new_credit_consumed,
                                            conn_consumed)) {
                /* Fatal error (allocation, etc.) */
                txp_enlink_tmp(tmp_head, stream);
                return 0;
            }
            conn_consumed += stream->txp_txfc_new_credit_consumed;

            if (packet_full) {
                txp_enlink_tmp(tmp_head, stream);
                break;
            }
        }

        txp_enlink_tmp(tmp_head, stream);
    }

    return 1;
}

static int txp_generate_for_el(OSSL_QUIC_TX_PACKETISER *txp,
                               struct txp_pkt *pkt,
                               int chosen_for_conn_close)
{
    int rc = TXP_ERR_SUCCESS;
    const uint32_t enc_level = pkt->h.enc_level;
    const uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    int have_ack_eliciting = 0, done_pre_token = 0;
    const struct archetype_data a = pkt->geom.adata;
    /*
     * Cleared if we encode any non-ACK-eliciting frame type which rules out the
     * packet being a non-inflight frame. This means any non-ACK ACK-eliciting
     * frame, even PADDING frames. ACK eliciting frames always cause a packet to
     * become ineligible for non-inflight treatment so it is not necessary to
     * clear this in cases where have_ack_eliciting is set, as it is ignored in
     * that case.
     */
    int can_be_non_inflight = 1;
    QUIC_CFQ_ITEM *cfq_item;
    QUIC_TXPIM_PKT *tpkt = NULL;
    struct tx_helper *h = &pkt->h;

    /* Maximum PN reached? */
    if (!ossl_quic_pn_valid(txp->next_pn[pn_space]))
        goto fatal_err;

    if (!ossl_assert(pkt->tpkt == NULL))
        goto fatal_err;

    if ((pkt->tpkt = tpkt = ossl_quic_txpim_pkt_alloc(txp->args.txpim)) == NULL)
        goto fatal_err;

    /*
     * Frame Serialization
     * ===================
     *
     * We now serialize frames into the packet in descending order of priority.
     */

    /* HANDSHAKE_DONE (Regenerate) */
    if (a.allow_handshake_done && txp->want_handshake_done
        && tx_helper_get_space_left(h) >= MIN_FRAME_SIZE_HANDSHAKE_DONE) {
        WPACKET *wpkt = tx_helper_begin(h);

        if (wpkt == NULL)
            goto fatal_err;

        if (ossl_quic_wire_encode_frame_handshake_done(wpkt)) {
            tpkt->had_handshake_done_frame = 1;
            have_ack_eliciting             = 1;

            if (!tx_helper_commit(h))
                goto fatal_err;

            tx_helper_unrestrict(h); /* no longer need PING */
        } else {
            tx_helper_rollback(h);
        }
    }

    /* MAX_DATA (Regenerate) */
    if (a.allow_conn_fc
        && (txp->want_max_data
            || ossl_quic_rxfc_has_cwm_changed(txp->args.conn_rxfc, 0))
        && tx_helper_get_space_left(h) >= MIN_FRAME_SIZE_MAX_DATA) {
        WPACKET *wpkt = tx_helper_begin(h);
        uint64_t cwm = ossl_quic_rxfc_get_cwm(txp->args.conn_rxfc);

        if (wpkt == NULL)
            goto fatal_err;

        if (ossl_quic_wire_encode_frame_max_data(wpkt, cwm)) {
            tpkt->had_max_data_frame = 1;
            have_ack_eliciting       = 1;

            if (!tx_helper_commit(h))
                goto fatal_err;

            tx_helper_unrestrict(h); /* no longer need PING */
        } else {
            tx_helper_rollback(h);
        }
    }

    /* MAX_STREAMS_BIDI (Regenerate) */
    if (a.allow_conn_fc
        && (txp->want_max_streams_bidi
            || ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_bidi_rxfc, 0))
        && tx_helper_get_space_left(h) >= MIN_FRAME_SIZE_MAX_STREAMS_BIDI) {
        WPACKET *wpkt = tx_helper_begin(h);
        uint64_t max_streams
            = ossl_quic_rxfc_get_cwm(txp->args.max_streams_bidi_rxfc);

        if (wpkt == NULL)
            goto fatal_err;

        if (ossl_quic_wire_encode_frame_max_streams(wpkt, /*is_uni=*/0,
                                                    max_streams)) {
            tpkt->had_max_streams_bidi_frame = 1;
            have_ack_eliciting               = 1;

            if (!tx_helper_commit(h))
                goto fatal_err;

            tx_helper_unrestrict(h); /* no longer need PING */
        } else {
            tx_helper_rollback(h);
        }
    }

    /* MAX_STREAMS_UNI (Regenerate) */
    if (a.allow_conn_fc
        && (txp->want_max_streams_uni
            || ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_uni_rxfc, 0))
        && tx_helper_get_space_left(h) >= MIN_FRAME_SIZE_MAX_STREAMS_UNI) {
        WPACKET *wpkt = tx_helper_begin(h);
        uint64_t max_streams
            = ossl_quic_rxfc_get_cwm(txp->args.max_streams_uni_rxfc);

        if (wpkt == NULL)
            goto fatal_err;

        if (ossl_quic_wire_encode_frame_max_streams(wpkt, /*is_uni=*/1,
                                                    max_streams)) {
            tpkt->had_max_streams_uni_frame = 1;
            have_ack_eliciting              = 1;

            if (!tx_helper_commit(h))
                goto fatal_err;

            tx_helper_unrestrict(h); /* no longer need PING */
        } else {
            tx_helper_rollback(h);
        }
    }

    /* GCR Frames */
    for (cfq_item = ossl_quic_cfq_get_priority_head(txp->args.cfq, pn_space);
         cfq_item != NULL;
         cfq_item = ossl_quic_cfq_item_get_priority_next(cfq_item, pn_space)) {
        uint64_t frame_type = ossl_quic_cfq_item_get_frame_type(cfq_item);
        const unsigned char *encoded = ossl_quic_cfq_item_get_encoded(cfq_item);
        size_t encoded_len = ossl_quic_cfq_item_get_encoded_len(cfq_item);

        switch (frame_type) {
            case OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID:
                if (!a.allow_new_conn_id)
                    continue;
                break;
            case OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID:
                if (!a.allow_retire_conn_id)
                    continue;
                break;
            case OSSL_QUIC_FRAME_TYPE_NEW_TOKEN:
                if (!a.allow_new_token)
                    continue;

                /*
                 * NEW_TOKEN frames are handled via GCR, but some
                 * Regenerate-strategy frames should come before them (namely
                 * ACK, CONNECTION_CLOSE, PATH_CHALLENGE and PATH_RESPONSE). If
                 * we find a NEW_TOKEN frame, do these now. If there are no
                 * NEW_TOKEN frames in the GCR queue we will handle these below.
                 */
                if (!done_pre_token)
                    if (txp_generate_pre_token(txp, pkt,
                                               chosen_for_conn_close,
                                               &can_be_non_inflight))
                        done_pre_token = 1;

                break;
            case OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE:
                if (!a.allow_path_response)
                    continue;

                /*
                 * RFC 9000 s. 8.2.2: An endpoint MUST expand datagrams that
                 * contain a PATH_RESPONSE frame to at least the smallest
                 * allowed maximum datagram size of 1200 bytes.
                 */
                pkt->force_pad = 1;
                break;
            default:
                if (!a.allow_cfq_other)
                    continue;
                break;
        }

        /*
         * If the frame is too big, don't try to schedule any more GCR frames in
         * this packet rather than sending subsequent ones out of order.
         */
        if (encoded_len > tx_helper_get_space_left(h))
            break;

        if (!tx_helper_append_iovec(h, encoded, encoded_len))
            goto fatal_err;

        ossl_quic_txpim_pkt_add_cfq_item(tpkt, cfq_item);

        if (ossl_quic_frame_type_is_ack_eliciting(frame_type)) {
            have_ack_eliciting = 1;
            tx_helper_unrestrict(h); /* no longer need PING */
        }
    }

    /*
     * If we didn't generate ACK, CONNECTION_CLOSE, PATH_CHALLENGE or
     * PATH_RESPONSE (as desired) before, do so now.
     */
    if (!done_pre_token)
        if (txp_generate_pre_token(txp, pkt,
                                   chosen_for_conn_close,
                                   &can_be_non_inflight))
            done_pre_token = 1;

    /* CRYPTO Frames */
    if (a.allow_crypto)
        if (!txp_generate_crypto_frames(txp, pkt, &have_ack_eliciting))
            goto fatal_err;

    /* Stream-specific frames */
    if (a.allow_stream_rel && txp->handshake_complete)
        if (!txp_generate_stream_related(txp, pkt,
                                         &have_ack_eliciting,
                                         &pkt->stream_head))
            goto fatal_err;

    /* PING */
    tx_helper_unrestrict(h);

    if (!have_ack_eliciting && txp_need_ping(txp, pn_space, &a)) {
        WPACKET *wpkt;

        assert(h->reserve > 0);
        wpkt = tx_helper_begin(h);
        if (wpkt == NULL)
            goto fatal_err;

        if (!ossl_quic_wire_encode_frame_ping(wpkt)
            || !tx_helper_commit(h))
            /*
             * We treat a request to be ACK-eliciting as a requirement, so this
             * is an error.
             */
            goto fatal_err;

        have_ack_eliciting = 1;
    }

    /* PADDING is added by ossl_quic_tx_packetiser_generate(). */

    /*
     * ACKM Data
     * =========
     */
    if (have_ack_eliciting)
        can_be_non_inflight = 0;

    /* ACKM Data */
    tpkt->ackm_pkt.num_bytes        = h->bytes_appended + pkt->geom.pkt_overhead;
    tpkt->ackm_pkt.pkt_num          = txp->next_pn[pn_space];
    /* largest_acked is set in txp_generate_pre_token */
    tpkt->ackm_pkt.pkt_space        = pn_space;
    tpkt->ackm_pkt.is_inflight      = !can_be_non_inflight;
    tpkt->ackm_pkt.is_ack_eliciting = have_ack_eliciting;
    tpkt->ackm_pkt.is_pto_probe     = 0;
    tpkt->ackm_pkt.is_mtu_probe     = 0;
    tpkt->ackm_pkt.time             = txp->args.now(txp->args.now_arg);
    tpkt->pkt_type                  = pkt->phdr.type;

    /* Done. */
    return rc;

fatal_err:
    /*
     * Handler for fatal errors, i.e. errors causing us to abort the entire
     * packet rather than just one frame. Examples of such errors include
     * allocation errors.
     */
    if (tpkt != NULL) {
        ossl_quic_txpim_pkt_release(txp->args.txpim, tpkt);
        pkt->tpkt = NULL;
    }
    return TXP_ERR_INTERNAL;
}

/*
 * Commits and queues a packet for transmission. There is no backing out after
 * this.
 *
 * This:
 *
 *   - Sends the packet to the QTX for encryption and transmission;
 *
 *   - Records the packet as having been transmitted in FIFM. ACKM is informed,
 *     etc. and the TXPIM record is filed.
 *
 *   - Informs various subsystems of frames that were sent and clears frame
 *     wanted flags so that we do not generate the same frames again.
 *
 * Assumptions:
 *
 *   - pkt is a txp_pkt for the correct EL;
 *
 *   - pkt->tpkt is valid;
 *
 *   - pkt->tpkt->ackm_pkt has been fully filled in;
 *
 *   - Stream chunk records have been appended to pkt->tpkt for STREAM and
 *     CRYPTO frames, but not for RESET_STREAM or STOP_SENDING frames;
 *
 *   - The chosen stream list for the packet can be fully walked from
 *     pkt->stream_head using stream->txp_next;
 *
 *   - pkt->has_ack_eliciting is set correctly.
 *
 */
static int txp_pkt_commit(OSSL_QUIC_TX_PACKETISER *txp,
                          struct txp_pkt *pkt,
                          uint32_t archetype,
                          int *txpim_pkt_reffed)
{
    int rc = 1;
    uint32_t enc_level = pkt->h.enc_level;
    uint32_t pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
    QUIC_TXPIM_PKT *tpkt = pkt->tpkt;
    QUIC_STREAM *stream;
    OSSL_QTX_PKT txpkt;
    struct archetype_data a;

    *txpim_pkt_reffed = 0;

    /* Cannot send a packet with an empty payload. */
    if (pkt->h.bytes_appended == 0)
        return 0;

    if (!txp_get_archetype_data(enc_level, archetype, &a))
        return 0;

    /* Packet Information for QTX */
    txpkt.hdr       = &pkt->phdr;
    txpkt.iovec     = txp->el[enc_level].iovec;
    txpkt.num_iovec = pkt->h.num_iovec;
    txpkt.local     = NULL;
    txpkt.peer      = BIO_ADDR_family(&txp->args.peer) == AF_UNSPEC
        ? NULL : &txp->args.peer;
    txpkt.pn        = txp->next_pn[pn_space];
    txpkt.flags     = OSSL_QTX_PKT_FLAG_COALESCE; /* always try to coalesce */

    /* Generate TXPIM chunks representing STOP_SENDING and RESET_STREAM frames. */
    for (stream = pkt->stream_head; stream != NULL; stream = stream->txp_next)
        if (stream->txp_sent_stop_sending || stream->txp_sent_reset_stream) {
            /* Log STOP_SENDING/RESET_STREAM chunk to TXPIM. */
            QUIC_TXPIM_CHUNK chunk;

            chunk.stream_id         = stream->id;
            chunk.start             = UINT64_MAX;
            chunk.end               = 0;
            chunk.has_fin           = 0;
            chunk.has_stop_sending  = stream->txp_sent_stop_sending;
            chunk.has_reset_stream  = stream->txp_sent_reset_stream;
            if (!ossl_quic_txpim_pkt_append_chunk(tpkt, &chunk))
                return 0; /* alloc error */
        }

    /* Dispatch to FIFD. */
    if (!ossl_quic_fifd_pkt_commit(&txp->fifd, tpkt))
        return 0;

    /*
     * Transmission and Post-Packet Generation Bookkeeping
     * ===================================================
     *
     * No backing out anymore - at this point the ACKM has recorded the packet
     * as having been sent, so we need to increment our next PN counter, or
     * the ACKM will complain when we try to record a duplicate packet with
     * the same PN later. At this point actually sending the packet may still
     * fail. In this unlikely event it will simply be handled as though it
     * were a lost packet.
     */
    ++txp->next_pn[pn_space];
    *txpim_pkt_reffed = 1;

    /* Send the packet. */
    if (!ossl_qtx_write_pkt(txp->args.qtx, &txpkt))
        return 0;

    /*
     * Record FC and stream abort frames as sent; deactivate streams which no
     * longer have anything to do.
     */
    for (stream = pkt->stream_head; stream != NULL; stream = stream->txp_next) {
        if (stream->txp_sent_fc) {
            stream->want_max_stream_data = 0;
            ossl_quic_rxfc_has_cwm_changed(&stream->rxfc, 1);
        }

        if (stream->txp_sent_stop_sending)
            stream->want_stop_sending = 0;

        if (stream->txp_sent_reset_stream)
            stream->want_reset_stream = 0;

        if (stream->txp_txfc_new_credit_consumed > 0) {
            if (!ossl_assert(ossl_quic_txfc_consume_credit(&stream->txfc,
                                                           stream->txp_txfc_new_credit_consumed)))
                /*
                 * Should not be possible, but we should continue with our
                 * bookkeeping as we have already committed the packet to the
                 * FIFD. Just change the value we return.
                 */
                rc = 0;

            stream->txp_txfc_new_credit_consumed = 0;
        }

        /*
         * If we no longer need to generate any flow control (MAX_STREAM_DATA),
         * STOP_SENDING or RESET_STREAM frames, nor any STREAM frames (because
         * the stream is drained of data or TXFC-blocked), we can mark the
         * stream as inactive.
         */
        ossl_quic_stream_map_update_state(txp->args.qsm, stream);

        if (ossl_quic_stream_has_send_buffer(stream)
            && !ossl_quic_sstream_has_pending(stream->sstream)
            && ossl_quic_sstream_get_final_size(stream->sstream, NULL))
            /*
             * Transition to DATA_SENT if stream has a final size and we have
             * sent all data.
             */
            ossl_quic_stream_map_notify_all_data_sent(txp->args.qsm, stream);
    }

    /* We have now sent the packet, so update state accordingly. */
    if (tpkt->ackm_pkt.is_ack_eliciting)
        txp->force_ack_eliciting &= ~(1UL << pn_space);

    if (tpkt->had_handshake_done_frame)
        txp->want_handshake_done = 0;

    if (tpkt->had_max_data_frame) {
        txp->want_max_data = 0;
        ossl_quic_rxfc_has_cwm_changed(txp->args.conn_rxfc, 1);
    }

    if (tpkt->had_max_streams_bidi_frame) {
        txp->want_max_streams_bidi = 0;
        ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_bidi_rxfc, 1);
    }

    if (tpkt->had_max_streams_uni_frame) {
        txp->want_max_streams_uni = 0;
        ossl_quic_rxfc_has_cwm_changed(txp->args.max_streams_uni_rxfc, 1);
    }

    if (tpkt->had_ack_frame)
        txp->want_ack &= ~(1UL << pn_space);

    if (tpkt->had_conn_close)
        txp->want_conn_close = 0;

    /*
     * Decrement probe request counts if we have sent a packet that meets
     * the requirement of a probe, namely being ACK-eliciting.
     */
    if (tpkt->ackm_pkt.is_ack_eliciting) {
        OSSL_ACKM_PROBE_INFO *probe_info
            = ossl_ackm_get0_probe_request(txp->args.ackm);

        if (enc_level == QUIC_ENC_LEVEL_INITIAL
            && probe_info->anti_deadlock_initial > 0)
            --probe_info->anti_deadlock_initial;

        if (enc_level == QUIC_ENC_LEVEL_HANDSHAKE
            && probe_info->anti_deadlock_handshake > 0)
            --probe_info->anti_deadlock_handshake;

        if (a.allow_force_ack_eliciting /* (i.e., not for 0-RTT) */
            && probe_info->pto[pn_space] > 0)
            --probe_info->pto[pn_space];
    }

    return rc;
}

/* Ensure the iovec array is at least num elements long. */
static int txp_el_ensure_iovec(struct txp_el *el, size_t num)
{
    OSSL_QTX_IOVEC *iovec;

    if (el->alloc_iovec >= num)
        return 1;

    num = el->alloc_iovec != 0 ? el->alloc_iovec * 2 : 8;

    iovec = OPENSSL_realloc(el->iovec, sizeof(OSSL_QTX_IOVEC) * num);
    if (iovec == NULL)
        return 0;

    el->iovec          = iovec;
    el->alloc_iovec    = num;
    return 1;
}

int ossl_quic_tx_packetiser_schedule_conn_close(OSSL_QUIC_TX_PACKETISER *txp,
                                                const OSSL_QUIC_FRAME_CONN_CLOSE *f)
{
    char *reason = NULL;
    size_t reason_len = f->reason_len;
    size_t max_reason_len = txp_get_mdpl(txp) / 2;

    if (txp->want_conn_close)
        return 0;

    /*
     * Arbitrarily limit the length of the reason length string to half of the
     * MDPL.
     */
    if (reason_len > max_reason_len)
        reason_len = max_reason_len;

    if (reason_len > 0) {
        reason = OPENSSL_memdup(f->reason, reason_len);
        if (reason == NULL)
            return 0;
    }

    txp->conn_close_frame               = *f;
    txp->conn_close_frame.reason        = reason;
    txp->conn_close_frame.reason_len    = reason_len;
    txp->want_conn_close                = 1;
    return 1;
}

void ossl_quic_tx_packetiser_set_msg_callback(OSSL_QUIC_TX_PACKETISER *txp,
                                              ossl_msg_cb msg_callback,
                                              SSL *msg_callback_ssl)
{
    txp->msg_callback = msg_callback;
    txp->msg_callback_ssl = msg_callback_ssl;
}

void ossl_quic_tx_packetiser_set_msg_callback_arg(OSSL_QUIC_TX_PACKETISER *txp,
                                                  void *msg_callback_arg)
{
    txp->msg_callback_arg = msg_callback_arg;
}

QUIC_PN ossl_quic_tx_packetiser_get_next_pn(OSSL_QUIC_TX_PACKETISER *txp,
                                            uint32_t pn_space)
{
    if (pn_space >= QUIC_PN_SPACE_NUM)
        return UINT64_MAX;

    return txp->next_pn[pn_space];
}

OSSL_TIME ossl_quic_tx_packetiser_get_deadline(OSSL_QUIC_TX_PACKETISER *txp)
{
    /*
     * TXP-specific deadline computations which rely on TXP innards. This is in
     * turn relied on by the QUIC_CHANNEL code to determine the channel event
     * handling deadline.
     */
    OSSL_TIME deadline = ossl_time_infinite();
    uint32_t enc_level, pn_space;

    /*
     * ACK generation is not CC-gated - packets containing only ACKs are allowed
     * to bypass CC. We want to generate ACK frames even if we are currently
     * restricted by CC so the peer knows we have received data. The generate
     * call will take care of selecting the correct packet archetype.
     */
    for (enc_level = QUIC_ENC_LEVEL_INITIAL;
         enc_level < QUIC_ENC_LEVEL_NUM;
         ++enc_level)
        if (ossl_qtx_is_enc_level_provisioned(txp->args.qtx, enc_level)) {
            pn_space = ossl_quic_enc_level_to_pn_space(enc_level);
            deadline = ossl_time_min(deadline,
                                     ossl_ackm_get_ack_deadline(txp->args.ackm, pn_space));
        }

    /* When will CC let us send more? */
    if (txp->args.cc_method->get_tx_allowance(txp->args.cc_data) == 0)
        deadline = ossl_time_min(deadline,
                                 txp->args.cc_method->get_wakeup_deadline(txp->args.cc_data));

    return deadline;
}
