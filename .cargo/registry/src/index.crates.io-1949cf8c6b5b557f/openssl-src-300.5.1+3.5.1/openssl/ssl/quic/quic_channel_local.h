#ifndef OSSL_QUIC_CHANNEL_LOCAL_H
# define OSSL_QUIC_CHANNEL_LOCAL_H

# include "internal/quic_channel.h"

# ifndef OPENSSL_NO_QUIC

#  include <openssl/lhash.h>
#  include "internal/list.h"
#  include "internal/quic_predef.h"
#  include "internal/quic_fc.h"
#  include "internal/quic_stream_map.h"
#  include "internal/quic_tls.h"

/*
 * QUIC Channel Structure
 * ======================
 *
 * QUIC channel internals. It is intended that only the QUIC_CHANNEL
 * implementation and the RX depacketiser be allowed to access this structure
 * directly. As the RX depacketiser has no state of its own and computes over a
 * QUIC_CHANNEL structure, it can be viewed as an extension of the QUIC_CHANNEL
 * implementation. While the RX depacketiser could be provided with adequate
 * accessors to do what it needs, this would weaken the abstraction provided by
 * the QUIC_CHANNEL to other components; moreover the coupling of the RX
 * depacketiser to QUIC_CHANNEL internals is too deep and bespoke to make this
 * desirable.
 *
 * Other components should not include this header.
 */
struct quic_channel_st {
    QUIC_PORT                       *port;

    /*
     * QUIC_PORT keeps the channels which belong to it on a list for bookkeeping
     * purposes.
     */
    OSSL_LIST_MEMBER(ch,            QUIC_CHANNEL);
    OSSL_LIST_MEMBER(incoming_ch,   QUIC_CHANNEL);

    /*
     * The associated TLS 1.3 connection data. Used to provide the handshake
     * layer; its 'network' side is plugged into the crypto stream for each EL
     * (other than the 0-RTT EL). Note that the `tls` SSL object is not "owned"
     * by this channel. It is created and managed elsewhere and is guaranteed
     * to be valid for the lifetime of the channel. Therefore we do not free it
     * when we free the channel.
     */
    QUIC_TLS                        *qtls;
    SSL                             *tls;

    /* Port LCIDM we use to register LCIDs. */
    QUIC_LCIDM                      *lcidm;
    /* SRTM we register SRTs with. */
    QUIC_SRTM                       *srtm;

    /* Optional QLOG instance (or NULL). */
    QLOG                            *qlog;

    /*
     * The transport parameter block we will send or have sent.
     * Freed after sending or when connection is freed.
     */
    unsigned char                   *local_transport_params;

    /*
     * Pending new token to send once handshake is complete
     */
    uint8_t                         *pending_new_token;
    size_t                          pending_new_token_len;

    /* Our current L4 peer address, if any. */
    BIO_ADDR                        cur_peer_addr;

    /*
     * Subcomponents of the connection. All of these components are instantiated
     * and owned by us.
     */
    OSSL_QUIC_TX_PACKETISER         *txp;
    QUIC_TXPIM                      *txpim;
    QUIC_CFQ                        *cfq;
    /*
     * Connection level FC. The stream_count RXFCs is used to manage
     * MAX_STREAMS signalling.
     */
    QUIC_TXFC                       conn_txfc;
    QUIC_RXFC                       conn_rxfc, crypto_rxfc[QUIC_PN_SPACE_NUM];
    QUIC_RXFC                       max_streams_bidi_rxfc, max_streams_uni_rxfc;
    QUIC_STREAM_MAP                 qsm;
    OSSL_STATM                      statm;
    OSSL_CC_DATA                    *cc_data;
    const OSSL_CC_METHOD            *cc_method;
    OSSL_ACKM                       *ackm;

    /* Record layers in the TX and RX directions. */
    OSSL_QTX                        *qtx;
    OSSL_QRX                        *qrx;

    /* Message callback related arguments */
    ossl_msg_cb                     msg_callback;
    void                            *msg_callback_arg;
    SSL                             *msg_callback_ssl;

    /*
     * Send and receive parts of the crypto streams.
     * crypto_send[QUIC_PN_SPACE_APP] is the 1-RTT crypto stream. There is no
     * 0-RTT crypto stream.
     */
    QUIC_SSTREAM                    *crypto_send[QUIC_PN_SPACE_NUM];
    QUIC_RSTREAM                    *crypto_recv[QUIC_PN_SPACE_NUM];

    /* Internal state. */
    /*
     * Client: The DCID used in the first Initial packet we transmit as a client.
     * Server: The DCID used in the first Initial packet the client transmitted.
     * Randomly generated and required by RFC to be at least 8 bytes.
     */
    QUIC_CONN_ID                    init_dcid;

    /*
     * Server: If this channel is created in response to an init packet sent
     * after the server has sent a retry packet to do address validation, this
     * field stores the original connection id from the first init packet sent
     */
    QUIC_CONN_ID                    odcid;

    /*
     * Client: The SCID found in the first Initial packet from the server.
     * Not valid for servers.
     * Valid if have_received_enc_pkt is set.
     */
    QUIC_CONN_ID                    init_scid;

    /*
     * Client only: The SCID found in an incoming Retry packet we handled.
     * Not valid for servers.
     */
    QUIC_CONN_ID                    retry_scid;

    /* Server only: The DCID we currently expect the peer to use to talk to us. */
    QUIC_CONN_ID                    cur_local_cid;

    /*
     * The DCID we currently use to talk to the peer and its sequence num.
     */
    QUIC_CONN_ID                    cur_remote_dcid;
    uint64_t                        cur_remote_seq_num;
    uint64_t                        cur_retire_prior_to;

    /* Transport parameter values we send to our peer. */
    uint64_t                        tx_init_max_stream_data_bidi_local;
    uint64_t                        tx_init_max_stream_data_bidi_remote;
    uint64_t                        tx_init_max_stream_data_uni;
    uint64_t                        tx_max_ack_delay; /* ms */

    /* Transport parameter values received from server. */
    uint64_t                        rx_init_max_stream_data_bidi_local;
    uint64_t                        rx_init_max_stream_data_bidi_remote;
    uint64_t                        rx_init_max_stream_data_uni;
    uint64_t                        rx_max_ack_delay; /* ms */
    unsigned char                   rx_ack_delay_exp;

    /* Diagnostic counters for testing purposes only. May roll over. */
    uint16_t                        diag_num_rx_ack; /* Number of ACK frames received */

    /*
     * Temporary staging area to store information about the incoming packet we
     * are currently processing.
     */
    OSSL_QRX_PKT                    *qrx_pkt;

    /*
     * Current limit on number of streams we may create. Set by transport
     * parameters initially and then by MAX_STREAMS frames.
     */
    uint64_t                        max_local_streams_bidi;
    uint64_t                        max_local_streams_uni;

    /* The idle timeout values we and our peer requested. */
    uint64_t                        max_idle_timeout_local_req;
    uint64_t                        max_idle_timeout_remote_req;

    /* The negotiated maximum idle timeout in milliseconds. */
    uint64_t                        max_idle_timeout;

    /*
     * Maximum payload size in bytes for datagrams sent to our peer, as
     * negotiated by transport parameters.
     */
    uint64_t                        rx_max_udp_payload_size;
    /* Maximum active CID limit, as negotiated by transport parameters. */
    uint64_t                        rx_active_conn_id_limit;

    /*
     * Used to allocate stream IDs. This is a stream ordinal, i.e., a stream ID
     * without the low two bits designating type and initiator. Shift and or in
     * the type bits to convert to a stream ID.
     */
    uint64_t                        next_local_stream_ordinal_bidi;
    uint64_t                        next_local_stream_ordinal_uni;

    /*
     * Used to track which stream ordinals within a given stream type have been
     * used by the remote peer. This is an optimisation used to determine
     * which streams should be implicitly created due to usage of a higher
     * stream ordinal.
     */
    uint64_t                        next_remote_stream_ordinal_bidi;
    uint64_t                        next_remote_stream_ordinal_uni;

    /*
     * Application error code to be used for STOP_SENDING/RESET_STREAM frames
     * used to autoreject incoming streams.
     */
    uint64_t                        incoming_stream_auto_reject_aec;

    /*
     * Override packet count threshold at which we do a spontaneous TXKU.
     * Usually UINT64_MAX in which case a suitable value is chosen based on AEAD
     * limit advice from the QRL utility functions. This is intended for testing
     * use only. Usually set to UINT64_MAX.
     */
    uint64_t                        txku_threshold_override;

    /* Valid if we are in the TERMINATING or TERMINATED states. */
    QUIC_TERMINATE_CAUSE            terminate_cause;

    /*
     * Deadline at which we move to TERMINATING state. Valid if in the
     * TERMINATING state.
     */
    OSSL_TIME                       terminate_deadline;

    /*
     * Deadline at which connection dies due to idle timeout if no further
     * events occur.
     */
    OSSL_TIME                       idle_deadline;

    /*
     * Deadline at which we should send an ACK-eliciting packet to ensure
     * idle timeout does not occur.
     */
    OSSL_TIME                       ping_deadline;

    /*
     * The deadline at which the period in which it is RECOMMENDED that we not
     * initiate any spontaneous TXKU ends. This is zero if no such deadline
     * applies.
     */
    OSSL_TIME                       txku_cooldown_deadline;

    /*
     * The deadline at which we take the QRX out of UPDATING and back to NORMAL.
     * Valid if rxku_in_progress in 1.
     */
    OSSL_TIME                       rxku_update_end_deadline;

    /*
     * The first (application space) PN sent with a new key phase. Valid if the
     * QTX key epoch is greater than 0. Once a packet we sent with a PN p (p >=
     * txku_pn) is ACKed, the TXKU is considered completed and txku_in_progress
     * becomes 0. For sanity's sake, such a PN p should also be <= the highest
     * PN we have ever sent, of course.
     */
    QUIC_PN                         txku_pn;

    /*
     * The (application space) PN which triggered RXKU detection. Valid if
     * rxku_pending_confirm.
     */
    QUIC_PN                         rxku_trigger_pn;

    /*
     * State tracking. QUIC connection-level state is best represented based on
     * whether various things have happened yet or not, rather than as an
     * explicit FSM. We do have a coarse state variable which tracks the basic
     * state of the connection's lifecycle, but more fine-grained conditions of
     * the Active state are tracked via flags below. For more details, see
     * doc/designs/quic-design/connection-state-machine.md. We are in the Open
     * state if the state is QUIC_CHANNEL_STATE_ACTIVE and handshake_confirmed is
     * set.
     */
    unsigned int                    state                   : 3;

    /*
     * Have we received at least one encrypted packet from the peer?
     * (If so, Retry and Version Negotiation messages should no longer
     *  be received and should be ignored if they do occur.)
     */
    unsigned int                    have_received_enc_pkt   : 1;

    /*
     * Have we successfully processed any packet, including a Version
     * Negotiation packet? If so, further Version Negotiation packets should be
     * ignored.
     */
    unsigned int                    have_processed_any_pkt  : 1;

    /*
     * Have we sent literally any packet yet? If not, there is no point polling
     * RX.
     */
    unsigned int                    have_sent_any_pkt       : 1;

    /*
     * Are we currently doing proactive version negotiation?
     */
    unsigned int                    doing_proactive_ver_neg : 1;

    /* We have received transport parameters from the peer. */
    unsigned int                    got_remote_transport_params    : 1;
    /* We have generated our local transport parameters. */
    unsigned int                    got_local_transport_params     : 1;

    /*
     * This monotonically transitions to 1 once the TLS state machine is
     * 'complete', meaning that it has both sent a Finished and successfully
     * verified the peer's Finished (see RFC 9001 s. 4.1.1). Note that it
     * does not transition to 1 at both peers simultaneously.
     *
     * Handshake completion is not the same as handshake confirmation (see
     * below).
     */
    unsigned int                    handshake_complete      : 1;

    /*
     * This monotonically transitions to 1 once the handshake is confirmed.
     * This happens on the client when we receive a HANDSHAKE_DONE frame.
     * At our option, we may also take acknowledgement of any 1-RTT packet
     * we sent as a handshake confirmation.
     */
    unsigned int                    handshake_confirmed     : 1;

    /*
     * We are sending Initial packets based on a Retry. This means we definitely
     * should not receive another Retry, and if we do it is an error.
     */
    unsigned int                    doing_retry             : 1;

    /*
     * We don't store the current EL here; the TXP asks the QTX which ELs
     * are provisioned to determine which ELs to use.
     */

    /* Have statm, qsm been initialised? Used to track cleanup. */
    unsigned int                    have_statm              : 1;
    unsigned int                    have_qsm                : 1;

    /*
     * Preferred ELs for transmission and reception. This is not strictly needed
     * as it can be inferred from what keys we have provisioned, but makes
     * determining the current EL simpler and faster. A separate EL for
     * transmission and reception is not strictly necessary but makes things
     * easier for interoperation with the handshake layer, which likes to invoke
     * the yield secret callback at different times for TX and RX.
     */
    unsigned int                    tx_enc_level            : 3;
    unsigned int                    rx_enc_level            : 3;

    /* If bit n is set, EL n has been discarded. */
    unsigned int                    el_discarded            : 4;

    /*
     * While in TERMINATING - CLOSING, set when we should generate a connection
     * close frame.
     */
    unsigned int                    conn_close_queued       : 1;

    /* Are we in server mode? Never changes after instantiation. */
    unsigned int                    is_server               : 1;

    /*
     * Set temporarily when the handshake layer has given us a new RX secret.
     * Used to determine if we need to check our RX queues again.
     */
    unsigned int                    have_new_rx_secret      : 1;

    /* Have we ever called QUIC_TLS yet during RX processing? */
    unsigned int                    did_tls_tick            : 1;
    /* Has any CRYPTO frame been processed during this tick? */
    unsigned int                    did_crypto_frame        : 1;

    /*
     * Have we sent an ack-eliciting packet since the last successful packet
     * reception? Used to determine when to bump idle timer (see RFC 9000 s.
     * 10.1).
     */
    unsigned int                    have_sent_ack_eliciting_since_rx    : 1;

    /* Should incoming streams automatically be rejected? */
    unsigned int                    incoming_stream_auto_reject         : 1;

    /*
     * 1 if a key update sequence was locally initiated, meaning we sent the
     * TXKU first and the resultant RXKU shouldn't result in our triggering
     * another TXKU. 0 if a key update sequence was initiated by the peer,
     * meaning we detect a RXKU first and have to generate a TXKU in response.
     */
    unsigned int                    ku_locally_initiated                : 1;

    /*
     * 1 if we have triggered TXKU (whether spontaneous or solicited) but are
     * waiting for any PN using that new KP to be ACKed. While this is set, we
     * are not allowed to trigger spontaneous TXKU (but solicited TXKU is
     * potentially still possible).
     */
    unsigned int                    txku_in_progress                    : 1;

    /*
     * We have received an RXKU event and currently are going through
     * UPDATING/COOLDOWN on the QRX. COOLDOWN is currently not used. Since RXKU
     * cannot be detected in this state, this doesn't cause a protocol error or
     * anything similar if a peer tries TXKU in this state. That traffic would
     * simply be dropped. It's only used to track that our UPDATING timer is
     * active so we know when to take the QRX out of UPDATING and back to
     * NORMAL.
     */
    unsigned int                    rxku_in_progress                    : 1;

    /*
     * We have received an RXKU but have yet to send an ACK for it, which means
     * no further RXKUs are allowed yet. Note that we cannot detect further
     * RXKUs anyway while the QRX remains in the UPDATING/COOLDOWN states, so
     * this restriction comes into play if we take more than PTO time to send
     * an ACK for it (not likely).
     */
    unsigned int                    rxku_pending_confirm                : 1;

    /* Temporary variable indicating rxku_pending_confirm is to become 0. */
    unsigned int                    rxku_pending_confirm_done           : 1;

    /*
     * If set, RXKU is expected (because we initiated a spontaneous TXKU).
     */
    unsigned int                    rxku_expected                       : 1;

    /* Permanent net error encountered */
    unsigned int                    net_error                           : 1;

    /*
     * Protocol error encountered. Note that you should refer to the state field
     * rather than this. This is only used so we can ignore protocol errors
     * after the first protocol error, but still record the first protocol error
     * if it happens during the TERMINATING state.
     */
    unsigned int                    protocol_error                      : 1;

    /* Are we using addressed mode? */
    unsigned int                    addressed_mode                      : 1;

    /* Are we on the QUIC_PORT linked list of channels? */
    unsigned int                    on_port_list                        : 1;

    /* Has qlog been requested? */
    unsigned int                    use_qlog                            : 1;

    /* Has qlog been requested? */
    unsigned int                    is_tserver_ch                       : 1;

    /* Saved error stack in case permanent error was encountered */
    ERR_STATE                       *err_state;

    /* Scratch area for use by RXDP to store decoded ACK ranges. */
    OSSL_QUIC_ACK_RANGE             *ack_range_scratch;
    size_t                          num_ack_range_scratch;

    /* Title for qlog purposes. We own this copy. */
    char                            *qlog_title;
};

# endif

#endif
