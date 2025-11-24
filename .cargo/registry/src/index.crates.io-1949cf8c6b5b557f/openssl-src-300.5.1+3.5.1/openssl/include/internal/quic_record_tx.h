/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_RECORD_TX_H
# define OSSL_QUIC_RECORD_TX_H

# include <openssl/ssl.h>
# include "internal/quic_wire_pkt.h"
# include "internal/quic_types.h"
# include "internal/quic_predef.h"
# include "internal/quic_record_util.h"
# include "internal/qlog.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Record Layer - TX
 * ======================
 */
typedef struct ossl_qtx_iovec_st {
    const unsigned char    *buf;
    size_t                  buf_len;
} OSSL_QTX_IOVEC;

typedef struct ossl_qtx_st OSSL_QTX;

typedef int (*ossl_mutate_packet_cb)(const QUIC_PKT_HDR *hdrin,
                                     const OSSL_QTX_IOVEC *iovecin, size_t numin,
                                     QUIC_PKT_HDR **hdrout,
                                     const OSSL_QTX_IOVEC **iovecout,
                                     size_t *numout,
                                     void *arg);

typedef void (*ossl_finish_mutate_cb)(void *arg);

typedef struct ossl_qtx_args_st {
    OSSL_LIB_CTX   *libctx;
    const char     *propq;

    /* BIO to transmit to. */
    BIO            *bio;

    /* Maximum datagram payload length (MDPL) for TX purposes. */
    size_t          mdpl;

    /* Callback returning QLOG instance to use, or NULL. */
    QLOG           *(*get_qlog_cb)(void *arg);
    void           *get_qlog_cb_arg;
} OSSL_QTX_ARGS;

/* Instantiates a new QTX. */
OSSL_QTX *ossl_qtx_new(const OSSL_QTX_ARGS *args);

/* Frees the QTX. */
void ossl_qtx_free(OSSL_QTX *qtx);

/* Set mutator callbacks for test framework support */
void ossl_qtx_set_mutator(OSSL_QTX *qtx, ossl_mutate_packet_cb mutatecb,
                          ossl_finish_mutate_cb finishmutatecb, void *mutatearg);

/* Setters for the msg_callback and the msg_callback_arg */
void ossl_qtx_set_msg_callback(OSSL_QTX *qtx, ossl_msg_cb msg_callback,
                               SSL *msg_callback_ssl);
void ossl_qtx_set_msg_callback_arg(OSSL_QTX *qtx, void *msg_callback_arg);

/* Change QLOG instance retrieval callback in use after instantiation. */
void ossl_qtx_set_qlog_cb(OSSL_QTX *qtx, QLOG *(*get_qlog_cb)(void *arg),
                          void *get_qlog_cb_arg);

/*
 * Secret Management
 * -----------------
 */

/*
 * Provides a secret to the QTX, which arises due to an encryption level change.
 * enc_level is a QUIC_ENC_LEVEL_* value.
 *
 * This function can be used to initialise the INITIAL encryption level, but you
 * should not do so directly; see the utility function
 * ossl_qrl_provide_initial_secret() instead, which can initialise the INITIAL
 * encryption level of a QRX and QTX simultaneously without duplicating certain
 * key derivation steps.
 *
 * You must call this function for a given EL before transmitting packets at
 * that EL using this QTX, otherwise ossl_qtx_write_pkt will fail.
 *
 * suite_id is a QRL_SUITE_* value which determines the AEAD function used for
 * the QTX.
 *
 * The secret passed is used directly to derive the "quic key", "quic iv" and
 * "quic hp" values.
 *
 * secret_len is the length of the secret buffer in bytes. The buffer must be
 * sized correctly to the chosen suite, else the function fails.
 *
 * This function can only be called once for a given EL, except for the INITIAL
 * EL, as the INITIAL EL can need to be rekeyed if connection retry occurs.
 * Subsequent calls for non-INITIAL ELs fail. Calls made after a corresponding
 * call to ossl_qtx_discard_enc_level for a given EL also fail, including for
 * the INITIAL EL. The secret for a non-INITIAL EL cannot be changed after it is
 * set because QUIC has no facility for introducing additional key material
 * after an EL is setup. (QUIC key updates generate new keys from existing key
 * material and do not introduce new entropy into a connection's key material.)
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_qtx_provide_secret(OSSL_QTX              *qtx,
                            uint32_t               enc_level,
                            uint32_t               suite_id,
                            EVP_MD                *md,
                            const unsigned char   *secret,
                            size_t                 secret_len);

/*
 * Informs the QTX that it can now discard key material for a given EL. The QTX
 * will no longer be able to generate packets at that EL. This function is
 * idempotent and succeeds if the EL has already been discarded.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_qtx_discard_enc_level(OSSL_QTX *qtx, uint32_t enc_level);

/* Returns 1 if the given encryption level is provisioned. */
int ossl_qtx_is_enc_level_provisioned(OSSL_QTX *qtx, uint32_t enc_level);

/*
 * Given the value ciphertext_len representing an encrypted packet payload
 * length in bytes, determines how many plaintext bytes it will decrypt to.
 * Returns 0 if the specified EL is not provisioned or ciphertext_len is too
 * small. The result is written to *plaintext_len.
 */
int ossl_qtx_calculate_plaintext_payload_len(OSSL_QTX *qtx, uint32_t enc_level,
                                             size_t ciphertext_len,
                                             size_t *plaintext_len);

/*
 * Given the value plaintext_len represented a plaintext packet payload length
 * in bytes, determines how many ciphertext bytes it will encrypt to. The value
 * output does not include packet headers. Returns 0 if the specified EL is not
 * provisioned. The result is written to *ciphertext_len.
 */
int ossl_qtx_calculate_ciphertext_payload_len(OSSL_QTX *qtx, uint32_t enc_level,
                                              size_t plaintext_len,
                                              size_t *ciphertext_len);

uint32_t ossl_qrl_get_suite_cipher_tag_len(uint32_t suite_id);


/*
 * Packet Transmission
 * -------------------
 */

struct ossl_qtx_pkt_st {
    /* Logical packet header to be serialized. */
    QUIC_PKT_HDR               *hdr;

    /*
     * iovecs expressing the logical packet payload buffer. Zero-length entries
     * are permitted.
     */
    const OSSL_QTX_IOVEC       *iovec;
    size_t                      num_iovec;

    /* Destination address. Will be passed through to the BIO if non-NULL. */
    const BIO_ADDR             *peer;

    /*
     * Local address (optional). Specify as non-NULL only if TX BIO
     * has local address support enabled.
     */
    const BIO_ADDR             *local;

    /*
     * Logical PN. Used for encryption. This will automatically be encoded to
     * hdr->pn, which need not be initialized.
     */
    QUIC_PN                     pn;

    /* Packet flags. Zero or more OSSL_QTX_PKT_FLAG_* values. */
    uint32_t                    flags;
};

/*
 * More packets will be written which should be coalesced into a single
 * datagram; do not send this packet yet. To use this, set this flag for all
 * packets but the final packet in a datagram, then send the final packet
 * without this flag set.
 *
 * This flag is not a guarantee and the QTX may transmit immediately anyway if
 * it is not possible to fit any more packets in the current datagram.
 *
 * If the caller change its mind and needs to cause a packet queued with
 * COALESCE after having passed it to this function but without writing another
 * packet, it should call ossl_qtx_flush_pkt().
 */
#define OSSL_QTX_PKT_FLAG_COALESCE       (1U << 0)

/*
 * Writes a packet.
 *
 * *pkt need be valid only for the duration of the call to this function.
 *
 * pkt->hdr->data and pkt->hdr->len are unused. The payload buffer is specified
 * via an array of OSSL_QTX_IOVEC structures. The API is designed to support
 * single-copy transmission; data is copied from the iovecs as it is encrypted
 * into an internal staging buffer for transmission.
 *
 * The function may modify and clobber pkt->hdr->data, pkt->hdr->len,
 * pkt->hdr->key_phase and pkt->hdr->pn for its own internal use. No other
 * fields of pkt or pkt->hdr will be modified.
 *
 * It is the callers responsibility to determine how long the PN field in the
 * encoded packet should be by setting pkt->hdr->pn_len. This function takes
 * care of the PN encoding. Set pkt->pn to the desired PN.
 *
 * Note that 1-RTT packets do not have a DCID Length field, therefore the DCID
 * length must be understood contextually. This function assumes the caller
 * knows what it is doing and will serialize a DCID of whatever length is given.
 * It is the caller's responsibility to ensure it uses a consistent DCID length
 * for communication with any given set of remote peers.
 *
 * The packet is queued regardless of whether it is able to be sent immediately.
 * This enables packets to be batched and sent at once on systems which support
 * system calls to send multiple datagrams in a single system call (see
 * BIO_sendmmsg). To flush queued datagrams to the network, see
 * ossl_qtx_flush_net().
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_qtx_write_pkt(OSSL_QTX *qtx, const OSSL_QTX_PKT *pkt);

/*
 * Finish any incomplete datagrams for transmission which were flagged for
 * coalescing. If there is no current coalescing datagram, this is a no-op.
 */
void ossl_qtx_finish_dgram(OSSL_QTX *qtx);

/*
 * (Attempt to) flush any datagrams which are queued for transmission. Note that
 * this does not cancel coalescing; call ossl_qtx_finish_dgram() first if that
 * is desired. The queue is drained into the OS's sockets as much as possible.
 * To determine if there is still data to be sent after calling this function,
 * use ossl_qtx_get_queue_len_bytes().
 *
 * Returns one of the following values:
 *
 *   QTX_FLUSH_NET_RES_OK
 *      Either no packets are currently queued for transmission,
 *      or at least one packet was successfully submitted.
 *
 *   QTX_FLUSH_NET_RES_TRANSIENT_FAIL
 *      The underlying network write BIO indicated a transient error
 *      (e.g. buffers full).
 *
 *   QTX_FLUSH_NET_RES_PERMANENT_FAIL
 *      Internal error (e.g. assertion or allocation error)
 *      or the underlying network write BIO indicated a non-transient
 *      error.
 */
#define QTX_FLUSH_NET_RES_OK                1
#define QTX_FLUSH_NET_RES_TRANSIENT_FAIL    (-1)
#define QTX_FLUSH_NET_RES_PERMANENT_FAIL    (-2)

int ossl_qtx_flush_net(OSSL_QTX *qtx);

/*
 * Diagnostic function. If there is any datagram pending transmission, pops it
 * and writes the details of the datagram as they would have been passed to
 * *msg. Returns 1, or 0 if there are no datagrams pending. For test use only.
 */
int ossl_qtx_pop_net(OSSL_QTX *qtx, BIO_MSG *msg);

/* Returns number of datagrams which are fully-formed but not yet sent. */
size_t ossl_qtx_get_queue_len_datagrams(OSSL_QTX *qtx);

/*
 * Returns number of payload bytes across all datagrams which are fully-formed
 * but not yet sent. Does not count any incomplete coalescing datagram.
 */
size_t ossl_qtx_get_queue_len_bytes(OSSL_QTX *qtx);

/*
 * Returns number of bytes in the current coalescing datagram, or 0 if there is
 * no current coalescing datagram. Returns 0 after a call to
 * ossl_qtx_finish_dgram().
 */
size_t ossl_qtx_get_cur_dgram_len_bytes(OSSL_QTX *qtx);

/*
 * Returns number of queued coalesced packets which have not been put into a
 * datagram yet. If this is non-zero, ossl_qtx_flush_pkt() needs to be called.
 */
size_t ossl_qtx_get_unflushed_pkt_count(OSSL_QTX *qtx);

/*
 * Change the BIO being used by the QTX. May be NULL if actual transmission is
 * not currently required. Does not up-ref the BIO; the caller is responsible
 * for ensuring the lifetime of the BIO exceeds the lifetime of the QTX.
 */
void ossl_qtx_set_bio(OSSL_QTX *qtx, BIO *bio);

/* Changes the MDPL. */
int ossl_qtx_set_mdpl(OSSL_QTX *qtx, size_t mdpl);

/* Retrieves the current MDPL. */
size_t ossl_qtx_get_mdpl(OSSL_QTX *qtx);


/*
 * Key Update
 * ----------
 *
 * For additional discussion of key update considerations, see QRX header file.
 */

/*
 * Triggers a key update. The key update will be started by inverting the Key
 * Phase bit of the next packet transmitted; no key update occurs until the next
 * packet is transmitted. Thus, this function should generally be called
 * immediately before queueing the next packet.
 *
 * There are substantial requirements imposed by RFC 9001 on under what
 * circumstances a key update can be initiated. The caller is responsible for
 * meeting most of these requirements. For example, this function cannot be
 * called too soon after a previous key update has occurred. Key updates also
 * cannot be initiated until the 1-RTT encryption level is reached.
 *
 * As a sanity check, this function will fail and return 0 if the non-1RTT
 * encryption levels have not yet been dropped.
 *
 * The caller may decide itself to initiate a key update, but it also MUST
 * initiate a key update where it detects that the peer has initiated a key
 * update. The caller is responsible for initiating a TX key update by calling
 * this function in this circumstance; thus, the caller is responsible for
 * coupling the RX and TX QUIC record layers in this way.
 */
int ossl_qtx_trigger_key_update(OSSL_QTX *qtx);


/*
 * Key Expiration
 * --------------
 */

/*
 * Returns the number of packets which have been encrypted for transmission with
 * the current set of TX keys (the current "TX key epoch"). Reset to zero after
 * a key update and incremented for each packet queued. If enc_level is not
 * valid or relates to an EL which is not currently available, returns
 * UINT64_MAX.
 */
uint64_t ossl_qtx_get_cur_epoch_pkt_count(OSSL_QTX *qtx, uint32_t enc_level);

/*
 * Returns the maximum number of packets which the record layer will permit to
 * be encrypted using the current set of TX keys. If this limit is reached (that
 * is, if the counter returned by ossl_qrx_tx_get_cur_epoch_pkt_count() reaches
 * this value), as a safety measure, the QTX will not permit any further packets
 * to be queued. All calls to ossl_qrx_write_pkt that try to send packets of a
 * kind which need to be encrypted will fail. It is not possible to recover from
 * this condition and the QTX must then be destroyed; therefore, callers should
 * ensure they always trigger a key update well in advance of reaching this
 * limit.
 *
 * The value returned by this function is based on the ciphersuite configured
 * for the given encryption level. If keys have not been provisioned for the
 * specified enc_level or the enc_level argument is invalid, this function
 * returns UINT64_MAX, which is not a valid value. Note that it is not possible
 * to perform a key update at any encryption level other than 1-RTT, therefore
 * if this limit is reached at earlier encryption levels (which should not be
 * possible) the connection must be terminated. Since this condition precludes
 * the transmission of further packets, the only possible signalling of such an
 * error condition to a peer is a Stateless Reset packet.
 */
uint64_t ossl_qtx_get_max_epoch_pkt_count(OSSL_QTX *qtx, uint32_t enc_level);

/*
 * Get the 1-RTT EL key epoch number for the QTX. This is intended for
 * diagnostic purposes. Returns 0 if 1-RTT EL is not provisioned yet.
 */
uint64_t ossl_qtx_get_key_epoch(OSSL_QTX *qtx);

# endif

#endif
