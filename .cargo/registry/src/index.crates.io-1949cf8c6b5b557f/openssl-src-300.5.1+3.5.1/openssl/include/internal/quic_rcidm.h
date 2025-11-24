/*
* Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_RCIDM_H
# define OSSL_INTERNAL_QUIC_RCIDM_H
# pragma once

# include "internal/e_os.h"
# include "internal/time.h"
# include "internal/quic_types.h"
# include "internal/quic_wire.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Remote Connection ID Manager
 * =================================
 *
 * This manages connection IDs for the TX side. The RCIDM tracks remote CIDs
 * (RCIDs) which a peer has issued to us and which we can use as the DCID of
 * packets we transmit. It is entirely separate from the LCIDM, which handles
 * routing received packets by their DCIDs.
 *
 * RCIDs fall into four categories:
 *
 *   1. A client's Initial ODCID                        (0..1)
 *   2. A peer's Initial SCID                           (1)
 *   3. A server's Retry SCID                           (0..1)
 *   4. A CID issued via a NEW_CONNECTION_ID frame      (n)
 *
 * Unlike a LCIDM, which is per port, a RCIDM is per connection, as there is no
 * need for routing of outgoing packets.
 */
typedef struct quic_rcidm_st QUIC_RCIDM;

/*
 * Creates a new RCIDM. Returns NULL on failure.
 *
 * For a client, initial_odcid is the client's Initial ODCID.
 * For a server, initial_odcid is NULL.
 */
QUIC_RCIDM *ossl_quic_rcidm_new(const QUIC_CONN_ID *initial_odcid);

/* Frees a RCIDM. */
void ossl_quic_rcidm_free(QUIC_RCIDM *rcidm);

/*
 * CID Events
 * ==========
 */

/*
 * To be called by a client when a server responds to the first Initial packet
 * sent with its own Initial packet with its own SCID; or to be called by a
 * server when we first get an Initial packet from a client with the client's
 * supplied SCID. The added RCID implicitly has a sequence number of 0.
 *
 * We immediately switch to using this SCID as our preferred RCID. This SCID
 * must be enrolled using this function. May only be called once.
 */
int ossl_quic_rcidm_add_from_initial(QUIC_RCIDM *rcidm,
                                     const QUIC_CONN_ID *rcid);

/*
 * To be called by a client when a server responds to the first Initial packet
 * sent with a Retry packet with its own SCID (the "Retry ODCID"). We
 * immediately switch to using this SCID as our preferred RCID when conducting
 * the retry. This SCID must be enrolled using this function. May only be called
 * once. The added RCID has no sequence number associated with it as it is
 * essentially a new ODCID (hereafter a Retry ODCID).
 *
 * Not for server use.
 */
int ossl_quic_rcidm_add_from_server_retry(QUIC_RCIDM *rcidm,
                                          const QUIC_CONN_ID *retry_odcid);

/*
 * Processes an incoming NEW_CONN_ID frame, recording the new CID as a potential
 * RCID. The RCIDM retirement mechanism is ratcheted according to the
 * ncid->retire_prior_to field. The stateless_reset field is ignored; the caller
 * is responsible for handling it separately.
 */
int ossl_quic_rcidm_add_from_ncid(QUIC_RCIDM *rcidm,
                                  const OSSL_QUIC_FRAME_NEW_CONN_ID *ncid);

/*
 * Other Events
 * ============
 */

/*
 * Notifies the RCIDM that the handshake for a connection is complete.
 * Should only be called once; further calls are ignored.
 *
 * This may influence the RCIDM's RCID change policy.
 */
void ossl_quic_rcidm_on_handshake_complete(QUIC_RCIDM *rcidm);

/*
 * Notifies the RCIDM that one or more packets have been sent.
 *
 * This may influence the RCIDM's RCID change policy.
 */
void ossl_quic_rcidm_on_packet_sent(QUIC_RCIDM *rcidm, uint64_t num_packets);

/*
 * Manually request switching to a new RCID as soon as possible.
 */
void ossl_quic_rcidm_request_roll(QUIC_RCIDM *rcidm);

/*
 * Queries
 * =======
 */

/*
 * The RCIDM decides when it will never use a given RCID again. When it does
 * this, it outputs the sequence number of that RCID using this function, which
 * pops from a logical queue of retired RCIDs. The caller is responsible
 * for polling this function and generating Retire CID frames from the result.
 *
 * If nothing needs doing and the queue is empty, this function returns 0. If
 * there is an RCID which needs retiring, the sequence number of that RCID is
 * written to *seq_num (if seq_num is non-NULL) and this function returns 1. The
 * queue entry is popped (and the caller is thus assumed to have taken
 * responsibility for transmitting the necessary Retire CID frame).
 *
 * Note that the caller should not transmit a Retire CID frame immediately as
 * packets using the RCID may still be in flight. The caller must determine an
 * appropriate delay using knowledge of network conditions (RTT, etc.) which is
 * outside the scope of the RCIDM. The caller is responsible for implementing
 * this delay based on the last time a packet was transmitted using the RCID
 * being retired.
 */
int ossl_quic_rcidm_pop_retire_seq_num(QUIC_RCIDM *rcid, uint64_t *seq_num);

/*
 * Like ossl_quic_rcidm_pop_retire_seq_num, but does not pop the item from the
 * queue. If this call succeeds, the next call to
 * ossl_quic_rcidm_pop_retire_seq_num is guaranteed to output the same sequence
 * number.
 */
int ossl_quic_rcidm_peek_retire_seq_num(QUIC_RCIDM *rcid, uint64_t *seq_num);

/*
 * Writes the DCID preferred for a newly transmitted packet at this time to
 * *tx_dcid. This function should be called to determine what DCID to use when
 * transmitting a packet to the peer. The RCIDM may implement arbitrary policy
 * to decide when to change the preferred RCID.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_rcidm_get_preferred_tx_dcid(QUIC_RCIDM *rcidm,
                                          QUIC_CONN_ID *tx_dcid);

/*
 * Returns 1 if the value output by ossl_quic_rcidm_get_preferred_tx_dcid() has
 * changed since the last call to this function with clear set. If clear is set,
 * clears the changed flag. Returns the old value of the changed flag.
 */
int ossl_quic_rcidm_get_preferred_tx_dcid_changed(QUIC_RCIDM *rcidm,
                                                  int clear);

/*
 * Returns the number of active numbered RCIDs we have. Note that this includes
 * RCIDs on the retir*ing* queue accessed via
 * ossl_quic_rcidm_pop_retire_seq_num() as these are still active until actually
 * retired.
 */
size_t ossl_quic_rcidm_get_num_active(const QUIC_RCIDM *rcidm);

/*
 * Returns the number of retir*ing* numbered RCIDs we have.
 */
size_t ossl_quic_rcidm_get_num_retiring(const QUIC_RCIDM *rcidm);

# endif

#endif
