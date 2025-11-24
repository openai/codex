/*
 * Copyright 2022-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_FC_H
# define OSSL_QUIC_FC_H

# include <openssl/ssl.h>
# include "internal/time.h"

# ifndef OPENSSL_NO_QUIC

/*
 * TX Flow Controller (TXFC)
 * =========================
 *
 * For discussion, see doc/designs/quic-design/quic-fc.md.
 */
typedef struct quic_txfc_st QUIC_TXFC;

struct quic_txfc_st {
    QUIC_TXFC   *parent; /* stream-level iff non-NULL */
    uint64_t    swm, cwm;
    char        has_become_blocked;
};

/*
 * Initialises a TX flow controller. conn_txfc should be non-NULL and point to
 * the connection-level flow controller if the TXFC is for stream-level flow
 * control, and NULL otherwise.
 */
int ossl_quic_txfc_init(QUIC_TXFC *txfc, QUIC_TXFC *conn_txfc);

/*
 * Gets the parent (i.e., connection-level) TX flow controller. Returns NULL if
 * called on a connection-level TX flow controller.
 */
QUIC_TXFC *ossl_quic_txfc_get_parent(QUIC_TXFC *txfc);

/*
 * Bump the credit watermark (CWM) value. This is the 'On TX Window Updated'
 * operation. This function is a no-op if it has already been called with an
 * equal or higher CWM value.
 *
 * It returns 1 iff the call resulted in the CWM being bumped and 0 if it was
 * not increased because it has already been called with an equal or higher CWM
 * value. This is not an error per se but may indicate a local programming error
 * or a protocol error in a remote peer.
 */
int ossl_quic_txfc_bump_cwm(QUIC_TXFC *txfc, uint64_t cwm);

/*
 * Get the number of bytes by which we are in credit. This is the number of
 * controlled bytes we are allowed to send. (Thus if this function returns 0, we
 * are currently blocked.)
 *
 * If called on a stream-level TXFC, ossl_quic_txfc_get_credit is called on
 * the connection-level TXFC as well, and the lesser of the two values is
 * returned. The consumed value is the amount already consumed on the connection
 * level TXFC.
 */
uint64_t ossl_quic_txfc_get_credit(QUIC_TXFC *txfc, uint64_t consumed);

/*
 * Like ossl_quic_txfc_get_credit(), but when called on a stream-level TXFC,
 * retrieves only the stream-level credit value and does not clamp it based on
 * connection-level flow control. Any credit value is reduced by the consumed
 * amount.
 */
uint64_t ossl_quic_txfc_get_credit_local(QUIC_TXFC *txfc, uint64_t consumed);

/*
 * Consume num_bytes of credit. This is the 'On TX' operation. This should be
 * called when we transmit any controlled bytes. Calling this with an argument
 * of 0 is a no-op.
 *
 * We must never transmit more controlled bytes than we are in credit for (see
 * the return value of ossl_quic_txfc_get_credit()). If you call this function
 * with num_bytes greater than our current credit, this function consumes the
 * remainder of the credit and returns 0. This indicates a serious programming
 * error on the caller's part. Otherwise, the function returns 1.
 *
 * If called on a stream-level TXFC, ossl_quic_txfc_consume_credit() is called
 * on the connection-level TXFC also. If the call to that function on the
 * connection-level TXFC returns zero, this function will also return zero.
 */
int ossl_quic_txfc_consume_credit(QUIC_TXFC *txfc, uint64_t num_bytes);

/*
 * Like ossl_quic_txfc_consume_credit(), but when called on a stream-level TXFC,
 * consumes only from the stream-level credit and does not inform the
 * connection-level TXFC.
 */
int ossl_quic_txfc_consume_credit_local(QUIC_TXFC *txfc, uint64_t num_bytes);

/*
 * This flag is provided for convenience. A caller is not required to use it. It
 * is a boolean flag set whenever our credit drops to zero. If clear is 1, the
 * flag is cleared. The old value of the flag is returned. Callers may use this
 * to determine if they need to send a DATA_BLOCKED or STREAM_DATA_BLOCKED
 * frame, which should contain the value returned by ossl_quic_txfc_get_cwm().
 */
int ossl_quic_txfc_has_become_blocked(QUIC_TXFC *txfc, int clear);

/*
 * Get the current CWM value. This is mainly only needed when generating a
 * DATA_BLOCKED or STREAM_DATA_BLOCKED frame, or for diagnostic purposes.
 */
uint64_t ossl_quic_txfc_get_cwm(QUIC_TXFC *txfc);

/*
 * Get the current spent watermark (SWM) value. This is purely for diagnostic
 * use and should not be needed in normal circumstances.
 */
uint64_t ossl_quic_txfc_get_swm(QUIC_TXFC *txfc);

/*
 * RX Flow Controller (RXFC)
 * =========================
 */
typedef struct quic_rxfc_st QUIC_RXFC;

struct quic_rxfc_st {
    /*
     * swm is the sent/received watermark, which tracks how much we have
     * received from the peer. rwm is the retired watermark, which tracks how
     * much has been passed to the application. esrwm is the rwm value at which
     * the current auto-tuning epoch started. hwm is the highest stream length
     * (STREAM frame offset + payload length) we have seen from a STREAM frame
     * yet.
     */
    uint64_t        cwm, swm, rwm, esrwm, hwm, cur_window_size, max_window_size;
    OSSL_TIME       epoch_start;
    OSSL_TIME       (*now)(void *arg);
    void            *now_arg;
    QUIC_RXFC       *parent;
    unsigned char   error_code, has_cwm_changed, is_fin, standalone;
};

/*
 * Initialises an RX flow controller. conn_rxfc should be non-NULL and point to
 * a connection-level RXFC if the RXFC is for stream-level flow control, and
 * NULL otherwise. initial_window_size and max_window_size specify the initial
 * and absolute maximum window sizes, respectively. Window size values are
 * expressed in bytes and determine how much credit the RXFC extends to the peer
 * to transmit more data at a time.
 */
int ossl_quic_rxfc_init(QUIC_RXFC *rxfc, QUIC_RXFC *conn_rxfc,
                        uint64_t initial_window_size,
                        uint64_t max_window_size,
                        OSSL_TIME (*now)(void *arg),
                        void *now_arg);

/*
 * Initialises an RX flow controller which is used by itself and not under a
 * connection-level RX flow controller. This can be used for stream count
 * enforcement as well as CRYPTO buffer enforcement.
 */
int ossl_quic_rxfc_init_standalone(QUIC_RXFC *rxfc,
                                   uint64_t initial_window_size,
                                   OSSL_TIME (*now)(void *arg),
                                   void *now_arg);

/*
 * Gets the parent (i.e., connection-level) RXFC. Returns NULL if called on a
 * connection-level RXFC.
 */
QUIC_RXFC *ossl_quic_rxfc_get_parent(QUIC_RXFC *rxfc);

/*
 * Changes the current maximum window size value.
 */
void ossl_quic_rxfc_set_max_window_size(QUIC_RXFC *rxfc,
                                        size_t max_window_size);

/*
 * To be called whenever a STREAM frame is received.
 *
 * end is the value (offset + len), where offset is the offset field of the
 * STREAM frame and len is the length of the STREAM frame's payload in bytes.
 *
 * is_fin should be 1 if the STREAM frame had the FIN flag set and 0 otherwise.
 *
 * This function may be used on a stream-level RXFC only. The connection-level
 * RXFC will have its state updated by the stream-level RXFC.
 *
 * You should check ossl_quic_rxfc_has_error() on both connection-level and
 * stream-level RXFCs after calling this function, as an incoming STREAM frame
 * may cause flow control limits to be exceeded by an errant peer. This
 * function still returns 1 in this case, as this is not a caller error.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_quic_rxfc_on_rx_stream_frame(QUIC_RXFC *rxfc,
                                      uint64_t end, int is_fin);

/*
 * To be called whenever controlled bytes are retired, i.e. when bytes are
 * dequeued from a QUIC stream and passed to the application. num_bytes
 * is the number of bytes which were passed to the application.
 *
 * You should call this only on a stream-level RXFC. This function will update
 * the connection-level RXFC automatically.
 *
 * rtt should be the current best understanding of the RTT to the peer, as
 * offered by the Statistics Manager.
 *
 * You should check ossl_quic_rxfc_has_cwm_changed() after calling this
 * function, as it may have caused the RXFC to decide to grant more flow control
 * credit to the peer.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_rxfc_on_retire(QUIC_RXFC *rxfc,
                             uint64_t num_bytes,
                             OSSL_TIME rtt);

/*
 * Returns the current CWM which the RXFC thinks the peer should have.
 *
 * Note that the RXFC will increase this value in response to events, at which
 * time a MAX_DATA or MAX_STREAM_DATA frame must be generated. Use
 * ossl_quic_rxfc_has_cwm_changed() to detect this condition.
 *
 * This value increases monotonically.
 */
uint64_t ossl_quic_rxfc_get_cwm(const QUIC_RXFC *rxfc);

/*
 * Returns the current SWM. This is the total number of bytes the peer has
 * transmitted to us. This is intended for diagnostic use only; you should
 * not need it.
 */
uint64_t ossl_quic_rxfc_get_swm(const QUIC_RXFC *rxfc);

/*
 * Returns the current RWM. This is the total number of bytes that has been
 * retired. This is intended for diagnostic use only; you should not need it.
 */
uint64_t ossl_quic_rxfc_get_rwm(const QUIC_RXFC *rxfc);

/*
 * Returns the current credit. This is the CWM minus the SWM. This is intended
 * for diagnostic use only; you should not need it.
 */
uint64_t ossl_quic_rxfc_get_credit(const QUIC_RXFC *rxfc);

/*
 * Returns the CWM changed flag. If clear is 1, the flag is cleared and the old
 * value is returned.
 */
int ossl_quic_rxfc_has_cwm_changed(QUIC_RXFC *rxfc, int clear);

/*
 * Returns a QUIC_ERR_* error code if a flow control error has been detected.
 * Otherwise, returns QUIC_ERR_NO_ERROR. If clear is 1, the error is cleared
 * and the old value is returned.
 *
 * May return one of the following values:
 *
 * QUIC_ERR_FLOW_CONTROL_ERROR:
 *   This indicates a flow control protocol violation by the remote peer; the
 *   connection should be terminated in this event.
 * QUIC_ERR_FINAL_SIZE:
 *   The peer attempted to change the stream length after ending the stream.
 */
int ossl_quic_rxfc_get_error(QUIC_RXFC *rxfc, int clear);

/*
 * Returns 1 if the RXFC is a stream-level RXFC and the RXFC knows the final
 * size for the stream in bytes. If this is the case and final_size is non-NULL,
 * writes the final size to *final_size. Otherwise, returns 0.
 */
int ossl_quic_rxfc_get_final_size(const QUIC_RXFC *rxfc, uint64_t *final_size);

# endif

#endif
