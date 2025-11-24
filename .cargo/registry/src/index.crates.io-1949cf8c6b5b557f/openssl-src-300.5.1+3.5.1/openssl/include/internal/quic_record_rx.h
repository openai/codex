/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_RECORD_RX_H
# define OSSL_QUIC_RECORD_RX_H

# include <openssl/ssl.h>
# include "internal/quic_wire_pkt.h"
# include "internal/quic_types.h"
# include "internal/quic_predef.h"
# include "internal/quic_record_util.h"
# include "internal/quic_demux.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Record Layer - RX
 * ======================
 */

typedef struct ossl_qrx_args_st {
    OSSL_LIB_CTX   *libctx;
    const char     *propq;

    /* Demux which owns the URXEs passed to us. */
    QUIC_DEMUX     *demux;

    /* Length of connection IDs used in short-header packets in bytes. */
    size_t          short_conn_id_len;

    /*
     * Maximum number of deferred datagrams buffered at any one time.
     * Suggested value: 32.
     */
    size_t          max_deferred;

    /* Initial reference PN used for RX. */
    QUIC_PN         init_largest_pn[QUIC_PN_SPACE_NUM];

    /* Initial key phase. For debugging use only; always 0 in real use. */
    unsigned char   init_key_phase_bit;
} OSSL_QRX_ARGS;

/* Instantiates a new QRX. */
OSSL_QRX *ossl_qrx_new(const OSSL_QRX_ARGS *args);

/*
 * Frees the QRX. All packets obtained using ossl_qrx_read_pkt must already
 * have been released by calling ossl_qrx_release_pkt.
 *
 * You do not need to call ossl_qrx_remove_dst_conn_id first; this function will
 * unregister the QRX from the demuxer for all registered destination connection
 * IDs (DCIDs) automatically.
 */
void ossl_qrx_free(OSSL_QRX *qrx);

/* Setters for the msg_callback and msg_callback_arg */
void ossl_qrx_set_msg_callback(OSSL_QRX *qrx, ossl_msg_cb msg_callback,
                               SSL *msg_callback_ssl);
void ossl_qrx_set_msg_callback_arg(OSSL_QRX *qrx,
                                   void *msg_callback_arg);

/*
 * Get the short header connection id len from this qrx
 */
size_t ossl_qrx_get_short_hdr_conn_id_len(OSSL_QRX *qrx);

/*
 * Secret Management
 * =================
 *
 * A QRX has several encryption levels (Initial, Handshake, 0-RTT, 1-RTT) and
 * two directions (RX, TX). At any given time, key material is managed for each
 * (EL, RX/TX) combination.
 *
 * Broadly, for a given (EL, RX/TX), the following state machine is applicable:
 *
 *   WAITING_FOR_KEYS --[Provide]--> HAVE_KEYS --[Discard]--> | DISCARDED |
 *         \-------------------------------------[Discard]--> |           |
 *
 * To transition the RX side of an EL from WAITING_FOR_KEYS to HAVE_KEYS, call
 * ossl_qrx_provide_secret (for the INITIAL EL, use of
 * ossl_quic_provide_initial_secret is recommended).
 *
 * Once keys have been provisioned for an EL, you call
 * ossl_qrx_discard_enc_level to transition the EL to the DISCARDED state. You
 * can also call this function to transition directly to the DISCARDED state
 * even before any keys have been provisioned for that EL.
 *
 * The DISCARDED state is terminal for a given EL; you cannot provide a secret
 * again for that EL after reaching it.
 *
 * Incoming packets cannot be processed and decrypted if they target an EL
 * not in the HAVE_KEYS state. However, there is a distinction between
 * the WAITING_FOR_KEYS and DISCARDED states:
 *
 *   - In the WAITING_FOR_KEYS state, the QRX assumes keys for the given
 *     EL will eventually arrive. Therefore, if it receives any packet
 *     for an EL in this state, it buffers it and tries to process it
 *     again once the EL reaches HAVE_KEYS.
 *
 *   - In the DISCARDED state, the QRX assumes no keys for the given
 *     EL will ever arrive again. If it receives any packet for an EL
 *     in this state, it is simply discarded.
 *
 * If the user wishes to instantiate a new QRX to replace an old one for
 * whatever reason, for example to take over for an already established QUIC
 * connection, it is important that all ELs no longer being used (i.e., INITIAL,
 * 0-RTT, 1-RTT) are transitioned to the DISCARDED state. Otherwise, the QRX
 * will assume that keys for these ELs will arrive in future, and will buffer
 * any received packets for those ELs perpetually. This can be done by calling
 * ossl_qrx_discard_enc_level for all non-1-RTT ELs immediately after
 * instantiating the QRX.
 *
 * The INITIAL EL is not setup automatically when the QRX is instantiated. This
 * allows the caller to instead discard it immediately after instantiation of
 * the QRX if it is not needed, for example if the QRX is being instantiated to
 * take over handling of an existing connection which has already passed the
 * INITIAL phase. This avoids the unnecessary derivation of INITIAL keys where
 * they are not needed. In the ordinary case, ossl_quic_provide_initial_secret
 * should be called immediately after instantiation.
 */

/*
 * Provides a secret to the QRX, which arises due to an encryption level change.
 * enc_level is a QUIC_ENC_LEVEL_* value. To initialise the INITIAL encryption
 * level, it is recommended to use ossl_quic_provide_initial_secret instead.
 *
 * You should seek to call this function for a given EL before packets of that
 * EL arrive and are processed by the QRX. However, if packets have already
 * arrived for a given EL, the QRX will defer processing of them and perform
 * processing of them when this function is eventually called for the EL in
 * question.
 *
 * suite_id is a QRL_SUITE_* value which determines the AEAD function used for
 * the QRX.
 *
 * The secret passed is used directly to derive the "quic key", "quic iv" and
 * "quic hp" values.
 *
 * secret_len is the length of the secret buffer in bytes. The buffer must be
 * sized correctly to the chosen suite, else the function fails.
 *
 * This function can only be called once for a given EL, except for the INITIAL
 * EL, which can need rekeying when a connection retry occurs. Subsequent calls
 * for non-INITIAL ELs fail, as do calls made after a corresponding call to
 * ossl_qrx_discard_enc_level for that EL. The secret for a non-INITIAL EL
 * cannot be changed after it is set because QUIC has no facility for
 * introducing additional key material after an EL is setup. QUIC key updates
 * are managed semi-automatically by the QRX but do require some caller handling
 * (see below).
 *
 * md is for internal use and should be NULL.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_qrx_provide_secret(OSSL_QRX              *qrx,
                            uint32_t               enc_level,
                            uint32_t               suite_id,
                            EVP_MD                *md,
                            const unsigned char   *secret,
                            size_t                 secret_len);

/*
 * Informs the QRX that it can now discard key material for a given EL. The QRX
 * will no longer be able to process incoming packets received at that
 * encryption level. This function is idempotent and succeeds if the EL has
 * already been discarded.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_qrx_discard_enc_level(OSSL_QRX *qrx, uint32_t enc_level);

/*
 * Packet Reception
 * ================
 */

/* Information about a received packet. */
struct ossl_qrx_pkt_st {
    /*
     * Points to a logical representation of the decoded QUIC packet header. The
     * data and len fields point to the decrypted QUIC payload (i.e., to a
     * sequence of zero or more (potentially malformed) frames to be decoded).
     */
    QUIC_PKT_HDR       *hdr;

    /*
     * Address the packet was received from. If this is not available for this
     * packet, this field is NULL (but this can only occur for manually injected
     * packets).
     */
    const BIO_ADDR     *peer;

    /*
     * Local address the packet was sent to. If this is not available for this
     * packet, this field is NULL.
     */
    const BIO_ADDR     *local;

    /*
     * This is the length of the datagram which contained this packet. Note that
     * the datagram may have contained other packets than this. The intended use
     * for this is so that the user can enforce minimum datagram sizes (e.g. for
     * datagrams containing INITIAL packets), as required by RFC 9000.
     */
    size_t              datagram_len;

    /* The PN which was decoded for the packet, if the packet has a PN field. */
    QUIC_PN             pn;

    /*
     * Time the packet was received, or ossl_time_zero() if the demuxer is not
     * using a now() function.
     */
    OSSL_TIME           time;

    /* The QRX which was used to receive the packet. */
    OSSL_QRX            *qrx;

    /*
     * The key epoch the packet was received with. Always 0 for non-1-RTT
     * packets.
     */
    uint64_t            key_epoch;

    /*
     * This monotonically increases with each datagram received.
     * It is for diagnostic use only.
     */
    uint64_t            datagram_id;
};

/*
 * Tries to read a new decrypted packet from the QRX.
 *
 * On success, *pkt points to a OSSL_QRX_PKT structure. The structure should be
 * freed when no longer needed by calling ossl_qrx_pkt_release(). The structure
 * is refcounted; to gain extra references, call ossl_qrx_pkt_up_ref(). This
 * will cause a corresponding number of calls to ossl_qrx_pkt_release() to be
 * ignored.
 *
 * The resources referenced by (*pkt)->hdr, (*pkt)->hdr->data and (*pkt)->peer
 * have the same lifetime as *pkt.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_qrx_read_pkt(OSSL_QRX *qrx, OSSL_QRX_PKT **pkt);

/*
 * Decrement the reference count for the given packet and frees it if the
 * reference count drops to zero. No-op if pkt is NULL.
 */
void ossl_qrx_pkt_release(OSSL_QRX_PKT *pkt);

/*
 * Like ossl_qrx_pkt_release, but just ensures that the refcount is dropped
 * on this qrx_pkt, and ensure its not on any list
 */
void ossl_qrx_pkt_orphan(OSSL_QRX_PKT *pkt);

/* Increments the reference count for the given packet. */
void ossl_qrx_pkt_up_ref(OSSL_QRX_PKT *pkt);

/*
 * Returns 1 if there are any already processed (i.e. decrypted) packets waiting
 * to be read from the QRX.
 */
int ossl_qrx_processed_read_pending(OSSL_QRX *qrx);

/*
 * Returns 1 if there are any unprocessed (i.e. not yet decrypted) packets
 * waiting to be processed by the QRX. These may or may not result in
 * successfully decrypted packets once processed. This indicates whether
 * unprocessed data is buffered by the QRX, not whether any data is available in
 * a kernel socket buffer.
 */
int ossl_qrx_unprocessed_read_pending(OSSL_QRX *qrx);

/*
 * Returns the number of UDP payload bytes received from the network so far
 * since the last time this counter was cleared. If clear is 1, clears the
 * counter and returns the old value.
 *
 * The intended use of this is to allow callers to determine how much credit to
 * add to their anti-amplification budgets. This is reported separately instead
 * of in the OSSL_QRX_PKT structure so that a caller can apply
 * anti-amplification credit as soon as a datagram is received, before it has
 * necessarily read all processed packets contained within that datagram from
 * the QRX.
 */
uint64_t ossl_qrx_get_bytes_received(OSSL_QRX *qrx, int clear);

/*
 * Sets a callback which is called when a packet is received and being validated
 * before being queued in the read queue. This is called after packet body
 * decryption and authentication to prevent exposing side channels. pn_space is
 * a QUIC_PN_SPACE_* value denoting which PN space the PN belongs to.
 *
 * If this callback returns 1, processing continues normally.
 * If this callback returns 0, the packet is discarded.
 *
 * Other packets in the same datagram will still be processed where possible.
 *
 * The callback is optional and can be unset by passing NULL for cb.
 * cb_arg is an opaque value passed to cb.
 */
typedef int (ossl_qrx_late_validation_cb)(QUIC_PN pn, int pn_space,
                                          void *arg);

int ossl_qrx_set_late_validation_cb(OSSL_QRX *qrx,
                                    ossl_qrx_late_validation_cb *cb,
                                    void *cb_arg);

/*
 * Forcibly injects a URXE which has been issued by the DEMUX into the QRX for
 * processing. This can be used to pass a received datagram to the QRX if it
 * would not be correctly routed to the QRX via standard DCID-based routing; for
 * example, when handling an incoming Initial packet which is attempting to
 * establish a new connection.
 */
void ossl_qrx_inject_urxe(OSSL_QRX *qrx, QUIC_URXE *e);
void ossl_qrx_inject_pkt(OSSL_QRX *qrx, OSSL_QRX_PKT *pkt);
int ossl_qrx_validate_initial_packet(OSSL_QRX *qrx, QUIC_URXE *urxe,
                                     const QUIC_CONN_ID *dcid);

/*
 * Decryption of 1-RTT packets must be explicitly enabled by calling this
 * function. This is to comply with the requirement that we not process 1-RTT
 * packets until the handshake is complete, even if we already have 1-RTT
 * secrets. Even if a 1-RTT secret is provisioned for the QRX, incoming 1-RTT
 * packets will be handled as though no key is available until this function is
 * called. Calling this function will then requeue any such deferred packets for
 * processing.
 */
void ossl_qrx_allow_1rtt_processing(OSSL_QRX *qrx);

/*
 * Key Update (RX)
 * ===============
 *
 * Key update on the RX side is a largely but not entirely automatic process.
 *
 * Key update is initially triggered by receiving a 1-RTT packet with a
 * different Key Phase value. This could be caused by an attacker in the network
 * flipping random bits, therefore such a key update is tentative until the
 * packet payload is successfully decrypted and authenticated by the AEAD with
 * the 'next' keys. These 'next' keys then become the 'current' keys and the
 * 'current' keys then become the 'previous' keys. The 'previous' keys must be
 * kept around temporarily as some packets may still be in flight in the network
 * encrypted with the old keys. If the old Key Phase value is X and the new Key
 * Phase Value is Y (where obviously X != Y), this creates an ambiguity as any
 * new packet received with a KP of X could either be an attempt to initiate yet
 * another key update right after the last one, or an old packet encrypted
 * before the key update.
 *
 * RFC 9001 provides some guidance on handling this issue:
 *
 *   Strategy 1:
 *      Three keys, disambiguation using packet numbers
 *
 *      "A recovered PN that is lower than any PN from the current KP uses the
 *       previous packet protection keys; a recovered PN that is higher than any
 *       PN from the current KP requires use of the next packet protection
 *       keys."
 *
 *   Strategy 2:
 *      Two keys and a timer
 *
 *      "Alternatively, endpoints can retain only two sets of packet protection
 *       keys, swapping previous keys for next after enough time has passed to
 *       allow for reordering in the network. In this case, the KP bit alone can
 *       be used to select keys."
 *
 * Strategy 2 is more efficient (we can keep fewer cipher contexts around) and
 * should cover all actually possible network conditions. It also allows a delay
 * after we make the 'next' keys our 'current' keys before we generate new
 * 'next' keys, which allows us to mitigate against malicious peers who try to
 * initiate an excessive number of key updates.
 *
 * We therefore model the following state machine:
 *
 *
 *                               PROVISIONED
 *                     _______________________________
 *                    |                               |
 *   UNPROVISIONED  --|---->  NORMAL  <----------\    |------>  DISCARDED
 *                    |          |               |    |
 *                    |          |               |    |
 *                    |          v               |    |
 *                    |      UPDATING            |    |
 *                    |          |               |    |
 *                    |          |               |    |
 *                    |          v               |    |
 *                    |       COOLDOWN           |    |
 *                    |          |               |    |
 *                    |          |               |    |
 *                    |          \---------------|    |
 *                    |_______________________________|
 *
 *
 * The RX starts (once a secret has been provisioned) in the NORMAL state. In
 * the NORMAL state, the current expected value of the Key Phase bit is
 * recorded. When a flipped Key Phase bit is detected, the RX attempts to
 * decrypt and authenticate the received packet with the 'next' keys rather than
 * the 'current' keys. If (and only if) this authentication is successful, we
 * move to the UPDATING state. (An attacker in the network could flip
 * the Key Phase bit randomly, so it is essential we do nothing until AEAD
 * authentication is complete.)
 *
 * In the UPDATING state, we know a key update is occurring and record
 * the new Key Phase bit value as the newly current value, but we still keep the
 * old keys around so that we can still process any packets which were still in
 * flight when the key update was initiated. In the UPDATING state, a
 * Key Phase bit value different to the current expected value is treated not as
 * the initiation of another key update, but a reference to our old keys.
 *
 * Eventually we will be reasonably sure we are not going to receive any more
 * packets with the old keys. At this point, we can transition to the COOLDOWN
 * state. This transition occurs automatically after a certain amount of time;
 * RFC 9001 recommends it be the PTO interval, which relates to our RTT to the
 * peer. The duration also SHOULD NOT exceed three times the PTO to assist with
 * maintaining PFS.
 *
 * In the COOLDOWN phase, the old keys have been securely erased and only one
 * set of keys can be used: the current keys. If a packet is received with a Key
 * Phase bit value different to the current Key Phase Bit value, this is treated
 * as a request for a Key Update, but this request is ignored and the packet is
 * treated as malformed. We do this to allow mitigation against malicious peers
 * trying to initiate an excessive number of Key Updates. The timeout for the
 * transition from UPDATING to COOLDOWN is recommended as adequate for
 * this purpose in itself by the RFC, so the normal additional timeout value for
 * the transition from COOLDOWN to normal is zero (immediate transition).
 *
 * A summary of each state:
 *
 *                 Epoch  Exp KP  Uses Keys KS0    KS1    If Non-Expected KP Bit
 *                 -----  ------  --------- ------ -----  ----------------------
 *      NORMAL         0  0       Keyset 0  Gen 0  Gen 1  → UPDATING
 *      UPDATING       1  1       Keyset 1  Gen 0  Gen 1  Use Keyset 0
 *      COOLDOWN       1  1       Keyset 1  Erased Gen 1  Ignore Packet (*)
 *
 *      NORMAL         1  1       Keyset 1  Gen 2  Gen 1  → UPDATING
 *      UPDATING       2  0       Keyset 0  Gen 2  Gen 1  Use Keyset 1
 *      COOLDOWN       2  0       Keyset 0  Gen 2  Erased Ignore Packet (*)
 *
 * (*) Actually implemented by attempting to decrypt the packet with the
 *     wrong keys (which ultimately has the same outcome), as recommended
 *     by RFC 9001 to avoid creating timing channels.
 *
 * Note that the key material for the next key generation ("key epoch") is
 * always kept in the NORMAL state (necessary to avoid side-channel attacks).
 * This material is derived during the transition from COOLDOWN to NORMAL.
 *
 * Note that when a peer initiates a Key Update, we MUST also initiate a Key
 * Update as per the RFC. The caller is responsible for detecting this condition
 * and making the necessary calls to the TX side by detecting changes to the
 * return value of ossl_qrx_get_key_epoch().
 *
 * The above states (NORMAL, UPDATING, COOLDOWN) can themselves be
 * considered substates of the PROVISIONED state. Providing a secret to the QRX
 * for an EL transitions from UNPROVISIONED, the initial state, to PROVISIONED
 * (NORMAL). Dropping key material for an EL transitions from whatever the
 * current substate of the PROVISIONED state is to the DISCARDED state, which is
 * the terminal state.
 *
 * Note that non-1RTT ELs cannot undergo key update, therefore a non-1RTT EL is
 * always in the NORMAL substate if it is in the PROVISIONED state.
 */

/*
 * Return the current RX key epoch for the 1-RTT encryption level. This is
 * initially zero and is incremented by one for every Key Update successfully
 * signalled by the peer. If the 1-RTT EL has not yet been provisioned or has
 * been discarded, returns UINT64_MAX.
 *
 * A necessary implication of this API is that the least significant bit of the
 * returned value corresponds to the currently expected Key Phase bit, though
 * callers are not anticipated to have any need of this information.
 *
 * It is not possible for the returned value to overflow, as a QUIC connection
 * cannot support more than 2**62 packet numbers, and a connection must be
 * terminated if this limit is reached.
 *
 * The caller should use this function to detect when the key epoch has changed
 * and use it to initiate a key update on the TX side.
 *
 * The value returned by this function increments specifically at the transition
 * from the NORMAL to the UPDATING state discussed above.
 */
uint64_t ossl_qrx_get_key_epoch(OSSL_QRX *qrx);

/*
 * Sets an optional callback which will be called when the key epoch changes.
 *
 * The callback is optional and can be unset by passing NULL for cb.
 * cb_arg is an opaque value passed to cb. pn is the PN of the packet.
 * Since key update is only supported for 1-RTT packets, the PN is always
 * in the Application Data PN space.
*/
typedef void (ossl_qrx_key_update_cb)(QUIC_PN pn, void *arg);

int ossl_qrx_set_key_update_cb(OSSL_QRX *qrx,
                               ossl_qrx_key_update_cb *cb, void *cb_arg);

/*
 * Relates to the 1-RTT encryption level. The caller should call this after the
 * UPDATING state is reached, after a timeout to be determined by the caller.
 *
 * This transitions from the UPDATING state to the COOLDOWN state (if
 * still in the UPDATING state). If normal is 1, then transitions from
 * the COOLDOWN state to the NORMAL state. Both transitions can be performed at
 * once if desired.
 *
 * If in the normal state, or if in the COOLDOWN state and normal is 0, this is
 * a no-op and returns 1. Returns 0 if the 1-RTT EL has not been provisioned or
 * has been dropped.
 *
 * It is essential that the caller call this within a few PTO intervals of a key
 * update occurring (as detected by the caller in a call to
 * ossl_qrx_key_get_key_epoch()), as otherwise the peer will not be able to
 * perform a Key Update ever again.
 */
int ossl_qrx_key_update_timeout(OSSL_QRX *qrx, int normal);


/*
 * Key Expiration
 * ==============
 */

/*
 * Returns the number of seemingly forged packets which have been received by
 * the QRX. If this value reaches the value returned by
 * ossl_qrx_get_max_epoch_forged_pkt_count() for a given EL, all further
 * received encrypted packets for that EL will be discarded without processing.
 *
 * Note that the forged packet limit is for the connection lifetime, thus it is
 * not reset by a key update. It is suggested that the caller terminate the
 * connection a reasonable margin before the limit is reached. However, the
 * exact limit imposed does vary by EL due to the possibility that different ELs
 * use different AEADs.
 */
uint64_t ossl_qrx_get_cur_forged_pkt_count(OSSL_QRX *qrx);

/*
 * Returns the maximum number of forged packets which the record layer will
 * permit to be verified using this QRX instance.
 */
uint64_t ossl_qrx_get_max_forged_pkt_count(OSSL_QRX *qrx,
                                           uint32_t enc_level);

# endif

#endif
