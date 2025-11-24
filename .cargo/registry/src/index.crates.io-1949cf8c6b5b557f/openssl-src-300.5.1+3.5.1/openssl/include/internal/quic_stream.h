/*
* Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_STREAM_H
# define OSSL_INTERNAL_QUIC_STREAM_H
# pragma once

#include "internal/e_os.h"
#include "internal/time.h"
#include "internal/quic_types.h"
#include "internal/quic_predef.h"
#include "internal/quic_wire.h"
#include "internal/quic_record_tx.h"
#include "internal/quic_record_rx.h"
#include "internal/quic_fc.h"
#include "internal/quic_statm.h"

# ifndef OPENSSL_NO_QUIC

/*
 * QUIC Send Stream
 * ================
 *
 * The QUIC Send Stream Manager (QUIC_SSTREAM) is responsible for:
 *
 *   - accepting octet strings of stream data;
 *
 *   - generating corresponding STREAM frames;
 *
 *   - receiving notifications of lost frames, in order to generate new STREAM
 *     frames for the lost data;
 *
 *   - receiving notifications of acknowledged frames, in order to internally
 *     reuse memory used to store acknowledged stream data;
 *
 *   - informing the caller of how much more stream data it can accept into
 *     its internal buffers, so as to ensure that the amount of unacknowledged
 *     data which can be written to a stream is not infinite and to allow the
 *     caller to manifest backpressure conditions to the user.
 *
 * The QUIC_SSTREAM is instantiated once for every stream with a send component
 * (i.e., for a unidirectional send stream or for the send component of a
 * bidirectional stream).
 *
 * Note: The terms 'TX' and 'RX' are used when referring to frames, packets and
 * datagrams. The terms 'send' and 'receive' are used when referring to the
 * stream abstraction. Applications send; we transmit.
 */

/*
 * Instantiates a new QUIC_SSTREAM. init_buf_size specifies the initial size of
 * the stream data buffer in bytes, which must be positive.
 */
QUIC_SSTREAM *ossl_quic_sstream_new(size_t init_buf_size);

/*
 * Frees a QUIC_SSTREAM and associated stream data storage.
 *
 * Any iovecs returned by ossl_quic_sstream_get_stream_frame cease to be valid after
 * calling this function.
 */
void ossl_quic_sstream_free(QUIC_SSTREAM *qss);

/*
 * (For TX packetizer use.) Retrieves information about application stream data
 * which is ready for transmission.
 *
 * *hdr is filled with the logical offset, maximum possible length of stream
 * data which can be transmitted, and a pointer to the stream data to be
 * transmitted. is_fin is set to 1 if hdr->offset + hdr->len is the final size
 * of the stream and 0 otherwise. hdr->stream_id is not set; the caller must set
 * it.
 *
 * The caller is not obligated to send all of the data. If the caller does not
 * send all of the data, the caller must reduce hdr->len before serializing the
 * header structure and must ensure that hdr->is_fin is cleared.
 *
 * hdr->has_explicit_len is always set. It is the caller's responsibility to
 * clear this if it wants to use the optimization of omitting the length field,
 * as only the caller can know when this optimization can be performed.
 *
 * *num_iov must be set to the size of the iov array at call time. When this
 * function returns successfully, it is updated to the number of iov entries
 * which have been written.
 *
 * The stream data may be split across up to two IOVs due to internal ring
 * buffer organisation. The sum of the lengths of the IOVs and the value written
 * to hdr->len will always match. If the caller decides to send less than
 * hdr->len of stream data, it must adjust the IOVs accordingly. This may be
 * done by updating hdr->len and then calling the utility function
 * ossl_quic_sstream_adjust_iov().
 *
 * After committing one or more bytes returned by ossl_quic_sstream_get_stream_frame to a
 * packet, call ossl_quic_sstream_mark_transmitted with the inclusive range of logical
 * byte numbers of the transmitted bytes (i.e., hdr->offset, hdr->offset +
 * hdr->len - 1). If you do not call ossl_quic_sstream_mark_transmitted, the next call to
 * ossl_quic_sstream_get_stream_frame will return the same data (or potentially the same
 * and more, if more data has been appended by the application).
 *
 * It is the caller's responsibility to clamp the length of data which this
 * function indicates is available according to other concerns, such as
 * stream-level flow control, connection-level flow control, or the applicable
 * maximum datagram payload length (MDPL) for a packet under construction.
 *
 * The skip argument can usually be given as zero. If it is non-zero, this
 * function outputs a range which would be output if it were called again after
 * calling ossl_quic_sstream_mark_transmitted() with the returned range, repeated 'skip'
 * times, and so on. This may be useful for callers which wish to enumerate
 * available stream frames and batch their calls to ossl_quic_sstream_mark_transmitted at
 * a later time.
 *
 * On success, this function will never write *num_iov with a value other than
 * 0, 1 or 2. A *num_iov value of 0 can only occurs when hdr->is_fin is set (for
 * example, when a stream is closed after all existing data has been sent, and
 * without sending any more data); otherwise the function returns 0 as there is
 * nothing useful to report.
 *
 * Returns 1 on success and 0 if there is no stream data available for
 * transmission, or on other error (such as if the caller provides fewer
 * than two IOVs.)
 */
int ossl_quic_sstream_get_stream_frame(QUIC_SSTREAM *qss,
                                       size_t skip,
                                       OSSL_QUIC_FRAME_STREAM *hdr,
                                       OSSL_QTX_IOVEC *iov,
                                       size_t *num_iov);

/*
 * Returns 1 if there is data pending transmission. Equivalent to calling
 * ossl_quic_sstream_get_stream_frame and seeing if it succeeds.
 */
int ossl_quic_sstream_has_pending(QUIC_SSTREAM *qss);

/*
 * Returns the current size of the stream; i.e., the number of bytes which have
 * been appended to the stream so far.
 */
uint64_t ossl_quic_sstream_get_cur_size(QUIC_SSTREAM *qss);

/*
 * (For TX packetizer use.) Marks a logical range of the send stream as having
 * been transmitted.
 *
 * 0 denotes the first byte ever sent on the stream. The start and end values
 * are both inclusive, therefore all calls to this function always mark at least
 * one byte as being transmitted; if no bytes have been transmitted, do not call
 * this function.
 *
 * If the STREAM frame sent had the FIN bit set, you must also call
 * ossl_quic_sstream_mark_transmitted_fin() after calling this function.
 *
 * If you sent a zero-length STREAM frame with the FIN bit set, you need only
 * call ossl_quic_sstream_mark_transmitted_fin() and must not call this function.
 *
 * Returns 1 on success and 0 on error (e.g. if end < start).
 */
int ossl_quic_sstream_mark_transmitted(QUIC_SSTREAM *qss,
                                       uint64_t start,
                                       uint64_t end);

/*
 * (For TX packetizer use.) Marks a STREAM frame with the FIN bit set as having
 * been transmitted. final_size is the final size of the stream (i.e., the value
 * offset + len of the transmitted STREAM frame).
 *
 * This function fails returning 0 if ossl_quic_sstream_fin() has not been called or if
 * final_size is not correct. The final_size argument is not strictly needed by
 * the QUIC_SSTREAM but is required as a sanity check.
 */
int ossl_quic_sstream_mark_transmitted_fin(QUIC_SSTREAM *qss,
                                           uint64_t final_size);

/*
 * (RX/ACKM use.) Marks a logical range of the send stream as having been lost.
 * The send stream will return the lost data for retransmission on a future call
 * to ossl_quic_sstream_get_stream_frame. The start and end values denote logical byte
 * numbers and are inclusive.
 *
 * If the lost frame had the FIN bit set, you must also call
 * ossl_quic_sstream_mark_lost_fin() after calling this function.
 *
 * Returns 1 on success and 0 on error (e.g. if end < start).
 */
int ossl_quic_sstream_mark_lost(QUIC_SSTREAM *qss,
                                uint64_t start,
                                uint64_t end);

/*
 * (RX/ACKM use.) Informs the QUIC_SSTREAM that a STREAM frame with the FIN bit
 * set was lost.
 *
 * Returns 1 on success and 0 on error.
 */
int ossl_quic_sstream_mark_lost_fin(QUIC_SSTREAM *qss);

/*
 * (RX/ACKM use.) Marks a logical range of the send stream as having been
 * acknowledged, meaning that the storage for the data in that range of the
 * stream can be now recycled and neither that logical range of the stream nor
 * any subset of it can be retransmitted again. The start and end values are
 * inclusive.
 *
 * If the acknowledged frame had the FIN bit set, you must also call
 * ossl_quic_sstream_mark_acked_fin() after calling this function.
 *
 * Returns 1 on success and 0 on error (e.g. if end < start).
 */
int ossl_quic_sstream_mark_acked(QUIC_SSTREAM *qss,
                                 uint64_t start,
                                 uint64_t end);

/*
 * (RX/ACKM use.) Informs the QUIC_SSTREAM that a STREAM frame with the FIN bit
 * set was acknowledged.
 *
 * Returns 1 on success and 0 on error.
 */
int ossl_quic_sstream_mark_acked_fin(QUIC_SSTREAM *qss);

/*
 * (Front end use.) Appends user data to the stream. The data is copied into the
 * stream. The amount of data consumed from buf is written to *consumed on
 * success (short writes are possible). The amount of data which can be written
 * can be determined in advance by calling the ossl_quic_sstream_get_buffer_avail()
 * function; data is copied into an internal ring buffer of finite size.
 *
 * If the buffer is full, this should be materialised as a backpressure
 * condition by the front end. This is not considered a failure condition;
 * *consumed is written as 0 and the function returns 1.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_quic_sstream_append(QUIC_SSTREAM *qss,
                             const unsigned char *buf,
                             size_t buf_len,
                             size_t *consumed);

/*
 * Marks a stream as finished. ossl_quic_sstream_append() may not be called anymore
 * after calling this.
 */
void ossl_quic_sstream_fin(QUIC_SSTREAM *qss);

/*
 * If the stream has had ossl_quic_sstream_fin() called, returns 1 and writes
 * the final size to *final_size. Otherwise, returns 0.
 */
int ossl_quic_sstream_get_final_size(QUIC_SSTREAM *qss, uint64_t *final_size);

/*
 * Returns 1 iff all bytes (and any FIN, if any) which have been appended to the
 * QUIC_SSTREAM so far, and any FIN (if any), have been both sent and acked.
 */
int ossl_quic_sstream_is_totally_acked(QUIC_SSTREAM *qss);

/*
 * Resizes the internal ring buffer. All stream data is preserved safely.
 *
 * This can be used to expand or contract the ring buffer, but not to contract
 * the ring buffer below the amount of stream data currently stored in it.
 * Returns 1 on success and 0 on failure.
 *
 * IMPORTANT: Any buffers referenced by iovecs output by
 * ossl_quic_sstream_get_stream_frame() cease to be valid after calling this function.
 */
int ossl_quic_sstream_set_buffer_size(QUIC_SSTREAM *qss, size_t num_bytes);

/*
 * Gets the internal ring buffer size in bytes.
 */
size_t ossl_quic_sstream_get_buffer_size(QUIC_SSTREAM *qss);

/*
 * Gets the number of bytes used in the internal ring buffer.
 */
size_t ossl_quic_sstream_get_buffer_used(QUIC_SSTREAM *qss);

/*
 * Gets the number of bytes free in the internal ring buffer.
 */
size_t ossl_quic_sstream_get_buffer_avail(QUIC_SSTREAM *qss);

/*
 * Utility function to ensure the length of an array of iovecs matches the
 * length given as len. Trailing iovecs have their length values reduced or set
 * to 0 as necessary.
 */
void ossl_quic_sstream_adjust_iov(size_t len,
                                  OSSL_QTX_IOVEC *iov,
                                  size_t num_iov);

/*
 * Sets flag to cleanse the buffered data when it is acked.
 */
void ossl_quic_sstream_set_cleanse(QUIC_SSTREAM *qss, int cleanse);

/*
 * QUIC Receive Stream Manager
 * ===========================
 *
 * The QUIC Receive Stream Manager (QUIC_RSTREAM) is responsible for
 * storing the received stream data frames until the application
 * is able to read the data.
 *
 * The QUIC_RSTREAM is instantiated once for every stream that can receive data.
 * (i.e., for a unidirectional receiving stream or for the receiving component
 * of a bidirectional stream).
 */

/*
 * Create a new instance of QUIC_RSTREAM with pointers to the flow
 * controller and statistics module. They can be NULL for unit testing.
 * If they are non-NULL, the `rxfc` is called when receive stream data
 * is read by application. `statm` is queried for current rtt.
 * `rbuf_size` is the initial size of the ring buffer to be used
 * when ossl_quic_rstream_move_to_rbuf() is called.
 */
QUIC_RSTREAM *ossl_quic_rstream_new(QUIC_RXFC *rxfc,
                                    OSSL_STATM *statm, size_t rbuf_size);

/*
 * Frees a QUIC_RSTREAM and any associated storage.
 */
void ossl_quic_rstream_free(QUIC_RSTREAM *qrs);

/*
 * Adds received stream frame data to `qrs`. The `pkt_wrap` refcount is
 * incremented if the `data` is queued directly without copying.
 * It can be NULL for unit-testing purposes, i.e. if `data` is static or
 * never released before calling ossl_quic_rstream_free().
 * The `offset` is the absolute offset of the data in the stream.
 * `data_len` can be 0 - can be useful for indicating `fin` for empty stream.
 * Or to indicate `fin` without any further data added to the stream.
 */

int ossl_quic_rstream_queue_data(QUIC_RSTREAM *qrs, OSSL_QRX_PKT *pkt,
                                 uint64_t offset,
                                 const unsigned char *data, uint64_t data_len,
                                 int fin);

/*
 * Copies the data from the stream storage to buffer `buf` of size `size`.
 * `readbytes` is set to the number of bytes actually copied.
 * `fin` is set to 1 if all the data from the stream were read so the
 * stream is finished. It is set to 0 otherwise.
 */
int ossl_quic_rstream_read(QUIC_RSTREAM *qrs, unsigned char *buf, size_t size,
                           size_t *readbytes, int *fin);

/*
 * Peeks at the data in the stream storage. It copies them to buffer `buf`
 * of size `size` and sets `readbytes` to the number of bytes actually copied.
 * `fin` is set to 1 if the copied data reach end of the stream.
 * It is set to 0 otherwise.
 */
int ossl_quic_rstream_peek(QUIC_RSTREAM *qrs, unsigned char *buf, size_t size,
                           size_t *readbytes, int *fin);

/*
 * Returns the size of the data available for reading. `fin` is set to 1 if
 * after reading all the available data the stream will be finished,
 * set to 0 otherwise.
 */
int ossl_quic_rstream_available(QUIC_RSTREAM *qrs, size_t *avail, int *fin);

/*
 * Sets *record to the beginning of the first readable stream data chunk and
 * *reclen to the size of the chunk. *fin is set to 1 if the end of the
 * chunk is the last of the stream data chunks.
 * If there is no record available *record is set to NULL and *rec_len to 0;
 * ossl_quic_rstream_release_record() should not be called in that case.
 * Returns 1 on success (including calls if no record is available, or
 * after end of the stream - in that case *fin will be set to 1 and
 * *rec_len to 0), 0 on error.
 * It is an error to call ossl_quic_rstream_get_record() multiple times
 * without calling ossl_quic_rstream_release_record() in between.
 */
int ossl_quic_rstream_get_record(QUIC_RSTREAM *qrs,
                                 const unsigned char **record, size_t *rec_len,
                                 int *fin);

/*
 * Releases (possibly partially) the record returned by
 * previous ossl_quic_rstream_get_record() call.
 * read_len between previously returned *rec_len and SIZE_MAX indicates
 * release of the whole record. Otherwise only part of the record is
 * released. The remaining part of the record is unlocked, another
 * call to ossl_quic_rstream_get_record() is needed to obtain further
 * stream data.
 * Returns 1 on success, 0 on error.
 * It is an error to call ossl_quic_rstream_release_record() multiple
 * times without calling ossl_quic_rstream_get_record() in between.
 */
int ossl_quic_rstream_release_record(QUIC_RSTREAM *qrs, size_t read_len);

/*
 * Moves received frame data from decrypted packets to ring buffer.
 * This should be called when there are too many decrypted packets allocated.
 * Returns 1 on success, 0 when it was not possible to release all
 * referenced packets due to an insufficient size of the ring buffer.
 * Exception is the packet from the record returned previously by
 * ossl_quic_rstream_get_record() - that one will be always skipped.
 */
int ossl_quic_rstream_move_to_rbuf(QUIC_RSTREAM *qrs);

/*
 * Resizes the internal ring buffer to a new `rbuf_size` size.
 * Returns 1 on success, 0 on error.
 * Possible error conditions are an allocation failure, trying to resize
 * the ring buffer when ossl_quic_rstream_get_record() was called and
 * not yet released, or trying to resize the ring buffer to a smaller size
 * than currently occupied.
 */
int ossl_quic_rstream_resize_rbuf(QUIC_RSTREAM *qrs, size_t rbuf_size);

/*
 * Sets flag to cleanse the buffered data when user reads it.
 */
void ossl_quic_rstream_set_cleanse(QUIC_RSTREAM *qrs, int cleanse);
# endif

#endif
