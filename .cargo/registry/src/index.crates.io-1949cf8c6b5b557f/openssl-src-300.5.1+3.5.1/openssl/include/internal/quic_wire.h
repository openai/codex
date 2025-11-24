/*
* Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
*
* Licensed under the Apache License 2.0 (the "License").  You may not use
* this file except in compliance with the License.  You can obtain a copy
* in the file LICENSE in the source distribution or at
* https://www.openssl.org/source/license.html
*/

#ifndef OSSL_INTERNAL_QUIC_WIRE_H
# define OSSL_INTERNAL_QUIC_WIRE_H
# pragma once

# include "internal/e_os.h"
# include "internal/time.h"
# include "internal/quic_types.h"
# include "internal/packet_quic.h"

# ifndef OPENSSL_NO_QUIC

#  define OSSL_QUIC_FRAME_TYPE_PADDING                0x00
#  define OSSL_QUIC_FRAME_TYPE_PING                   0x01
#  define OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN        0x02
#  define OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN           0x03
#  define OSSL_QUIC_FRAME_TYPE_RESET_STREAM           0x04
#  define OSSL_QUIC_FRAME_TYPE_STOP_SENDING           0x05
#  define OSSL_QUIC_FRAME_TYPE_CRYPTO                 0x06
#  define OSSL_QUIC_FRAME_TYPE_NEW_TOKEN              0x07
#  define OSSL_QUIC_FRAME_TYPE_MAX_DATA               0x10
#  define OSSL_QUIC_FRAME_TYPE_MAX_STREAM_DATA        0x11
#  define OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI       0x12
#  define OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_UNI        0x13
#  define OSSL_QUIC_FRAME_TYPE_DATA_BLOCKED           0x14
#  define OSSL_QUIC_FRAME_TYPE_STREAM_DATA_BLOCKED    0x15
#  define OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI   0x16
#  define OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_UNI    0x17
#  define OSSL_QUIC_FRAME_TYPE_NEW_CONN_ID            0x18
#  define OSSL_QUIC_FRAME_TYPE_RETIRE_CONN_ID         0x19
#  define OSSL_QUIC_FRAME_TYPE_PATH_CHALLENGE         0x1A
#  define OSSL_QUIC_FRAME_TYPE_PATH_RESPONSE          0x1B
#  define OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT   0x1C
#  define OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP         0x1D
#  define OSSL_QUIC_FRAME_TYPE_HANDSHAKE_DONE         0x1E

#  define OSSL_QUIC_FRAME_FLAG_STREAM_FIN         0x01
#  define OSSL_QUIC_FRAME_FLAG_STREAM_LEN         0x02
#  define OSSL_QUIC_FRAME_FLAG_STREAM_OFF         0x04
#  define OSSL_QUIC_FRAME_FLAG_STREAM_MASK        ((uint64_t)0x07)

/* Low 3 bits of the type contain flags */
#  define OSSL_QUIC_FRAME_TYPE_STREAM             0x08             /* base ID */
#  define OSSL_QUIC_FRAME_TYPE_STREAM_FIN         \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_FIN)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_LEN         \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_LEN)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_LEN_FIN     \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_LEN |            \
     OSSL_QUIC_FRAME_FLAG_STREAM_FIN)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_OFF         \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_OFF)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_OFF_FIN     \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_OFF |            \
     OSSL_QUIC_FRAME_FLAG_STREAM_FIN)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN     \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_OFF |            \
     OSSL_QUIC_FRAME_FLAG_STREAM_LEN)
#  define OSSL_QUIC_FRAME_TYPE_STREAM_OFF_LEN_FIN \
    (OSSL_QUIC_FRAME_TYPE_STREAM |                \
     OSSL_QUIC_FRAME_FLAG_STREAM_OFF |            \
     OSSL_QUIC_FRAME_FLAG_STREAM_LEN |            \
     OSSL_QUIC_FRAME_FLAG_STREAM_FIN)

#  define OSSL_QUIC_FRAME_TYPE_IS_STREAM(x) \
    (((x) & ~OSSL_QUIC_FRAME_FLAG_STREAM_MASK) == OSSL_QUIC_FRAME_TYPE_STREAM)
#  define OSSL_QUIC_FRAME_TYPE_IS_ACK(x) \
    (((x) & ~(uint64_t)1) == OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN)
#  define OSSL_QUIC_FRAME_TYPE_IS_MAX_STREAMS(x) \
    (((x) & ~(uint64_t)1) == OSSL_QUIC_FRAME_TYPE_MAX_STREAMS_BIDI)
#  define OSSL_QUIC_FRAME_TYPE_IS_STREAMS_BLOCKED(x) \
    (((x) & ~(uint64_t)1) == OSSL_QUIC_FRAME_TYPE_STREAMS_BLOCKED_BIDI)
#  define OSSL_QUIC_FRAME_TYPE_IS_CONN_CLOSE(x) \
    (((x) & ~(uint64_t)1) == OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT)

const char *ossl_quic_frame_type_to_string(uint64_t frame_type);

static ossl_unused ossl_inline int
ossl_quic_frame_type_is_ack_eliciting(uint64_t frame_type)
{
    switch (frame_type) {
    case OSSL_QUIC_FRAME_TYPE_PADDING:
    case OSSL_QUIC_FRAME_TYPE_ACK_WITHOUT_ECN:
    case OSSL_QUIC_FRAME_TYPE_ACK_WITH_ECN:
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_TRANSPORT:
    case OSSL_QUIC_FRAME_TYPE_CONN_CLOSE_APP:
        return 0;
    default:
        return 1;
    }
}

/* QUIC Transport Parameter Types */
#  define QUIC_TPARAM_ORIG_DCID                           0x00
#  define QUIC_TPARAM_MAX_IDLE_TIMEOUT                    0x01
#  define QUIC_TPARAM_STATELESS_RESET_TOKEN               0x02
#  define QUIC_TPARAM_MAX_UDP_PAYLOAD_SIZE                0x03
#  define QUIC_TPARAM_INITIAL_MAX_DATA                    0x04
#  define QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_LOCAL  0x05
#  define QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_BIDI_REMOTE 0x06
#  define QUIC_TPARAM_INITIAL_MAX_STREAM_DATA_UNI         0x07
#  define QUIC_TPARAM_INITIAL_MAX_STREAMS_BIDI            0x08
#  define QUIC_TPARAM_INITIAL_MAX_STREAMS_UNI             0x09
#  define QUIC_TPARAM_ACK_DELAY_EXP                       0x0A
#  define QUIC_TPARAM_MAX_ACK_DELAY                       0x0B
#  define QUIC_TPARAM_DISABLE_ACTIVE_MIGRATION            0x0C
#  define QUIC_TPARAM_PREFERRED_ADDR                      0x0D
#  define QUIC_TPARAM_ACTIVE_CONN_ID_LIMIT                0x0E
#  define QUIC_TPARAM_INITIAL_SCID                        0x0F
#  define QUIC_TPARAM_RETRY_SCID                          0x10

/*
 * QUIC Frame Logical Representations
 * ==================================
 */

/* QUIC Frame: ACK */
typedef struct ossl_quic_ack_range_st {
    /*
     * Represents an inclusive range of packet numbers [start, end].
     * start must be <= end.
     */
    QUIC_PN start, end;
} OSSL_QUIC_ACK_RANGE;

typedef struct ossl_quic_frame_ack_st {
    /*
     * A sequence of packet number ranges [[start, end]...].
     *
     * The ranges must be sorted in descending order, for example:
     *      [ 95, 100]
     *      [ 90,  92]
     *      etc.
     *
     * As such, ack_ranges[0].end is always the highest packet number
     * being acknowledged and ack_ranges[num_ack_ranges-1].start is
     * always the lowest packet number being acknowledged.
     *
     * num_ack_ranges must be greater than zero, as an ACK frame must
     * acknowledge at least one packet number.
     */
    OSSL_QUIC_ACK_RANGE        *ack_ranges;
    size_t                      num_ack_ranges;

    OSSL_TIME                   delay_time;
    uint64_t                    ect0, ect1, ecnce;
    unsigned int                ecn_present : 1;
} OSSL_QUIC_FRAME_ACK;

/* Returns 1 if the given frame contains the given PN. */
int ossl_quic_frame_ack_contains_pn(const OSSL_QUIC_FRAME_ACK *ack, QUIC_PN pn);

/* QUIC Frame: STREAM */
typedef struct ossl_quic_frame_stream_st {
    uint64_t                stream_id;  /* Stream ID */
    uint64_t                offset;     /* Logical offset in stream */
    uint64_t                len;        /* Length of data in bytes */
    const unsigned char    *data;

    /*
     * On encode, this determines whether the len field should be encoded or
     * not. If zero, the len field is not encoded and it is assumed the frame
     * runs to the end of the packet.
     *
     * On decode, this determines whether the frame had an explicitly encoded
     * length. If not set, the frame runs to the end of the packet and len has
     * been set accordingly.
     */
    unsigned int            has_explicit_len : 1;

    /* 1 if this is the end of the stream */
    unsigned int            is_fin : 1;
} OSSL_QUIC_FRAME_STREAM;

/* QUIC Frame: CRYPTO */
typedef struct ossl_quic_frame_crypto_st {
    uint64_t                offset; /* Logical offset in stream */
    uint64_t                len;    /* Length of the data in bytes */
    const unsigned char    *data;
} OSSL_QUIC_FRAME_CRYPTO;

/* QUIC Frame: RESET_STREAM */
typedef struct ossl_quic_frame_reset_stream_st {
    uint64_t    stream_id;
    uint64_t    app_error_code;
    uint64_t    final_size;
} OSSL_QUIC_FRAME_RESET_STREAM;

/* QUIC Frame: STOP_SENDING */
typedef struct ossl_quic_frame_stop_sending_st {
    uint64_t    stream_id;
    uint64_t    app_error_code;
} OSSL_QUIC_FRAME_STOP_SENDING;

/* QUIC Frame: NEW_CONNECTION_ID */
typedef struct ossl_quic_frame_new_conn_id_st {
    uint64_t                    seq_num;
    uint64_t                    retire_prior_to;
    QUIC_CONN_ID                conn_id;
    QUIC_STATELESS_RESET_TOKEN  stateless_reset;
} OSSL_QUIC_FRAME_NEW_CONN_ID;

/* QUIC Frame: CONNECTION_CLOSE */
typedef struct ossl_quic_frame_conn_close_st {
    unsigned int    is_app : 1; /* 0: transport error, 1: app error */
    uint64_t        error_code; /* 62-bit transport or app error code */
    uint64_t        frame_type; /* transport errors only */
    char            *reason;    /* UTF-8 string, not necessarily zero-terminated */
    size_t          reason_len; /* Length of reason in bytes */
} OSSL_QUIC_FRAME_CONN_CLOSE;

/*
 * QUIC Wire Format Encoding
 * =========================
 *
 * These functions return 1 on success and 0 on failure.
 */

/*
 * Encodes zero or more QUIC PADDING frames to the packet writer. Each PADDING
 * frame consumes one byte; num_bytes specifies the number of bytes of padding
 * to write.
 */
int ossl_quic_wire_encode_padding(WPACKET *pkt, size_t num_bytes);

/*
 * Encodes a QUIC PING frame to the packet writer. This frame type takes
 * no arguments.
*/
int ossl_quic_wire_encode_frame_ping(WPACKET *pkt);

/*
 * Encodes a QUIC ACK frame to the packet writer, given a logical representation
 * of the ACK frame.
 *
 * The ACK ranges passed must be sorted in descending order.
 *
 * The logical representation stores a list of packet number ranges. The wire
 * encoding is slightly different and stores the first range in the list
 * in a different manner.
 *
 * The ack_delay_exponent argument specifies the index of a power of two by
 * which the ack->ack_delay field is be divided. This exponent value must match
 * the value used when decoding.
 */
int ossl_quic_wire_encode_frame_ack(WPACKET *pkt,
                                    uint32_t ack_delay_exponent,
                                    const OSSL_QUIC_FRAME_ACK *ack);

/*
 * Encodes a QUIC RESET_STREAM frame to the packet writer, given a logical
 * representation of the RESET_STREAM frame.
 */
int ossl_quic_wire_encode_frame_reset_stream(WPACKET *pkt,
                                             const OSSL_QUIC_FRAME_RESET_STREAM *f);

/*
 * Encodes a QUIC STOP_SENDING frame to the packet writer, given a logical
 * representation of the STOP_SENDING frame.
 */
int ossl_quic_wire_encode_frame_stop_sending(WPACKET *pkt,
                                             const OSSL_QUIC_FRAME_STOP_SENDING *f);

/*
 * Encodes a QUIC CRYPTO frame header to the packet writer.
 *
 * To create a well-formed frame, the data written using this function must be
 * immediately followed by f->len bytes of data.
 */
int ossl_quic_wire_encode_frame_crypto_hdr(WPACKET *hdr,
                                           const OSSL_QUIC_FRAME_CRYPTO *f);

/*
 * Returns the number of bytes which will be required to encode the given
 * CRYPTO frame header. Does not include the payload bytes in the count.
 * Returns 0 if input is invalid.
 */
size_t ossl_quic_wire_get_encoded_frame_len_crypto_hdr(const OSSL_QUIC_FRAME_CRYPTO *f);

/*
 * Encodes a QUIC CRYPTO frame to the packet writer.
 *
 * This function returns a pointer to a buffer of f->len bytes which the caller
 * should fill however it wishes. If f->data is non-NULL, it is automatically
 * copied to the target buffer, otherwise the caller must fill the returned
 * buffer. Returns NULL on failure.
 */
void *ossl_quic_wire_encode_frame_crypto(WPACKET *pkt,
                                         const OSSL_QUIC_FRAME_CRYPTO *f);

/*
 * Encodes a QUIC NEW_TOKEN frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_new_token(WPACKET *pkt,
                                          const unsigned char *token,
                                          size_t token_len);

/*
 * Encodes a QUIC STREAM frame's header to the packet writer. The f->stream_id,
 * f->offset and f->len fields are the values for the respective Stream ID,
 * Offset and Length fields.
 *
 * If f->is_fin is non-zero, the frame is marked as the final frame in the
 * stream.
 *
 * If f->has_explicit_len is zerro, the frame is assumed to be the final frame
 * in the packet, which the caller is responsible for ensuring; the Length
 * field is then omitted.
 *
 * To create a well-formed frame, the data written using this function must be
 * immediately followed by f->len bytes of stream data.
 */
int ossl_quic_wire_encode_frame_stream_hdr(WPACKET *pkt,
                                           const OSSL_QUIC_FRAME_STREAM *f);

/*
 * Returns the number of bytes which will be required to encode the given
 * STREAM frame header. Does not include the payload bytes in the count.
 * Returns 0 if input is invalid.
 */
size_t ossl_quic_wire_get_encoded_frame_len_stream_hdr(const OSSL_QUIC_FRAME_STREAM *f);

/*
 * Functions similarly to ossl_quic_wire_encode_frame_stream_hdr, but it also
 * allocates space for f->len bytes of data after the header, creating a
 * well-formed QUIC STREAM frame in one call.
 *
 * A pointer to the bytes allocated for the framme payload is returned,
 * which the caller can fill however it wishes. If f->data is non-NULL,
 * it is automatically copied to the target buffer, otherwise the caller
 * must fill the returned buffer. Returns NULL on failure.
 */
void *ossl_quic_wire_encode_frame_stream(WPACKET *pkt,
                                         const OSSL_QUIC_FRAME_STREAM *f);

/*
 * Encodes a QUIC MAX_DATA frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_max_data(WPACKET *pkt,
                                         uint64_t max_data);

/*
 * Encodes a QUIC MAX_STREAM_DATA frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_max_stream_data(WPACKET *pkt,
                                                uint64_t stream_id,
                                                uint64_t max_data);

/*
 * Encodes a QUIC MAX_STREAMS frame to the packet writer.
 *
 * If is_uni is 0, the count specifies the maximum number of
 * bidirectional streams; else it specifies the maximum number of unidirectional
 * streams.
 */
int ossl_quic_wire_encode_frame_max_streams(WPACKET *pkt,
                                            char     is_uni,
                                            uint64_t max_streams);

/*
 * Encodes a QUIC DATA_BLOCKED frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_data_blocked(WPACKET *pkt,
                                             uint64_t max_data);

/*
 * Encodes a QUIC STREAM_DATA_BLOCKED frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_stream_data_blocked(WPACKET *pkt,
                                                    uint64_t stream_id,
                                                    uint64_t max_stream_data);
/*
 * Encodes a QUIC STREAMS_BLOCKED frame to the packet writer.
 *
 * If is_uni is 0, the count specifies the maximum number of
 * bidirectional streams; else it specifies the maximum number of unidirectional
 * streams.
 */
int ossl_quic_wire_encode_frame_streams_blocked(WPACKET *pkt,
                                                char is_uni,
                                                uint64_t max_streams);

/*
 * Encodes a QUIC NEW_CONNECTION_ID frame to the packet writer, given a logical
 * representation of the NEW_CONNECTION_ID frame.
 *
 * The buffer pointed to by the conn_id field must be valid for the duration of
 * the call.
 */
int ossl_quic_wire_encode_frame_new_conn_id(WPACKET *pkt,
                                            const OSSL_QUIC_FRAME_NEW_CONN_ID *f);

/*
 * Encodes a QUIC RETIRE_CONNECTION_ID frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_retire_conn_id(WPACKET *pkt,
                                               uint64_t seq_num);

/*
 * Encodes a QUIC PATH_CHALLENGE frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_path_challenge(WPACKET *pkt,
                                               uint64_t data);

/*
 * Encodes a QUIC PATH_RESPONSE frame to the packet writer.
 */
int ossl_quic_wire_encode_frame_path_response(WPACKET *pkt,
                                              uint64_t data);

/*
 * Encodes a QUIC CONNECTION_CLOSE frame to the packet writer, given a logical
 * representation of the CONNECTION_CLOSE frame.
 *
 * The reason field may be NULL, in which case no reason is encoded. If the
 * reason field is non-NULL, it must point to a valid UTF-8 string and
 * reason_len must be set to the length of the reason string in bytes. The
 * reason string need not be zero terminated.
 */
int ossl_quic_wire_encode_frame_conn_close(WPACKET *pkt,
                                           const OSSL_QUIC_FRAME_CONN_CLOSE *f);

/*
 * Encodes a QUIC HANDSHAKE_DONE frame to the packet writer. This frame type
 * takes no arguiments.
 */
int ossl_quic_wire_encode_frame_handshake_done(WPACKET *pkt);

/*
 * Encodes a QUIC transport parameter TLV with the given ID into the WPACKET.
 * The payload is an arbitrary buffer.
 *
 * If value is non-NULL, the value is copied into the packet.
 * If it is NULL, value_len bytes are allocated for the payload and the caller
 * should fill the buffer using the returned pointer.
 *
 * Returns a pointer to the start of the payload on success, or NULL on failure.
 */
unsigned char *ossl_quic_wire_encode_transport_param_bytes(WPACKET *pkt,
                                                           uint64_t id,
                                                           const unsigned char *value,
                                                           size_t value_len);

/*
 * Encodes a QUIC transport parameter TLV with the given ID into the WPACKET.
 * The payload is a QUIC variable-length integer with the given value.
 */
int ossl_quic_wire_encode_transport_param_int(WPACKET *pkt,
                                              uint64_t id,
                                              uint64_t value);

/*
 * Encodes a QUIC transport parameter TLV with a given ID into the WPACKET.
 * The payload is a QUIC connection ID.
 */
int ossl_quic_wire_encode_transport_param_cid(WPACKET *wpkt,
                                              uint64_t id,
                                              const QUIC_CONN_ID *cid);

/*
 * QUIC Wire Format Decoding
 * =========================
 *
 * These functions return 1 on success or 0 for failure. Typical reasons
 * why these functions may fail include:
 *
 *   - A frame decode function is called but the frame in the PACKET's buffer
 *     is not of the correct type.
 *
 *   - A variable-length field in the encoded frame appears to exceed the bounds
 *     of the PACKET's buffer.
 *
 * These functions should be called with the PACKET pointing to the start of the
 * frame (including the initial type field), and consume an entire frame
 * including its type field. The expectation is that the caller will have
 * already discerned the frame type using ossl_quic_wire_peek_frame_header().
 */

/*
 * Decodes the type field header of a QUIC frame (without advancing the current
 * position). This can be used to determine the frame type and determine which
 * frame decoding function to call.
 */
int ossl_quic_wire_peek_frame_header(PACKET *pkt, uint64_t *type,
                                     int *was_minimal);

/*
 * Like ossl_quic_wire_peek_frame_header, but advances the current position
 * so that the type field is consumed. For advanced use only.
 */
int ossl_quic_wire_skip_frame_header(PACKET *pkt, uint64_t *type);

/*
 * Determines how many ranges are needed to decode a QUIC ACK frame.
 *
 * The number of ranges which must be allocated before the call to
 * ossl_quic_wire_decode_frame_ack is written to *total_ranges.
 *
 * The PACKET is not advanced.
 */
int ossl_quic_wire_peek_frame_ack_num_ranges(const PACKET *pkt,
                                             uint64_t *total_ranges);

/*
 * Decodes a QUIC ACK frame. The ack_ranges field of the passed structure should
 * point to a preallocated array of ACK ranges and the num_ack_ranges field
 * should specify the length of allocation.
 *
 * *total_ranges is written with the number of ranges in the decoded frame,
 * which may be greater than the number of ranges which were decoded (i.e. if
 * num_ack_ranges was too small to decode all ranges).
 *
 * On success, this function modifies the num_ack_ranges field to indicate the
 * number of ranges in the decoded frame. This is the number of entries in the
 * ACK ranges array written by this function; any additional entries are not
 * modified.
 *
 * If the number of ACK ranges in the decoded frame exceeds that in
 * num_ack_ranges, as many ACK ranges as possible are decoded into the range
 * array. The caller can use the value written to *total_ranges to detect this
 * condition, as *total_ranges will exceed num_ack_ranges.
 *
 * If ack is NULL, the frame is still decoded, but only *total_ranges is
 * written. This can be used to determine the number of ranges which must be
 * allocated.
 *
 * The ack_delay_exponent argument specifies the index of a power of two used to
 * decode the ack_delay field. This must match the ack_delay_exponent value used
 * to encode the frame.
 */
int ossl_quic_wire_decode_frame_ack(PACKET *pkt,
                                    uint32_t ack_delay_exponent,
                                    OSSL_QUIC_FRAME_ACK *ack,
                                    uint64_t *total_ranges);

/*
 * Decodes a QUIC RESET_STREAM frame.
 */
int ossl_quic_wire_decode_frame_reset_stream(PACKET *pkt,
                                             OSSL_QUIC_FRAME_RESET_STREAM *f);

/*
 * Decodes a QUIC STOP_SENDING frame.
 */
int ossl_quic_wire_decode_frame_stop_sending(PACKET *pkt,
                                             OSSL_QUIC_FRAME_STOP_SENDING *f);

/*
 * Decodes a QUIC CRYPTO frame.
 *
 * f->data is set to point inside the packet buffer inside the PACKET, therefore
 * it is safe to access for as long as the packet buffer exists. If nodata is
 * set to 1 then reading the PACKET stops after the frame header and f->data is
 * set to NULL.
 */
int ossl_quic_wire_decode_frame_crypto(PACKET *pkt, int nodata,
                                       OSSL_QUIC_FRAME_CRYPTO *f);

/*
 * Decodes a QUIC NEW_TOKEN frame. *token is written with a pointer to the token
 * bytes and *token_len is written with the length of the token in bytes.
 */
int ossl_quic_wire_decode_frame_new_token(PACKET               *pkt,
                                          const unsigned char **token,
                                          size_t               *token_len);

/*
 * Decodes a QUIC STREAM frame.
 *
 * If nodata is set to 1 then reading the PACKET stops after the frame header
 * and f->data is set to NULL. In this case f->len will also be 0 in the event
 * that "has_explicit_len" is 0.
 *
 * If the frame did not contain an offset field, f->offset is set to 0, as the
 * absence of an offset field is equivalent to an offset of 0.
 *
 * If the frame contained a length field, f->has_explicit_len is set to 1 and
 * the length of the data is placed in f->len. This function ensures that the
 * length does not exceed the packet buffer, thus it is safe to access f->data.
 *
 * If the frame did not contain a length field, this means that the frame runs
 * until the end of the packet. This function sets f->has_explicit_len to zero,
 * and f->len to the amount of data remaining in the input buffer. Therefore,
 * this function should be used with a PACKET representing a single packet (and
 * not e.g. multiple packets).
 *
 * Note also that this means f->len is always valid after this function returns
 * successfully, regardless of the value of f->has_explicit_len.
 *
 * f->data points inside the packet buffer inside the PACKET, therefore it is
 * safe to access for as long as the packet buffer exists.
 *
 * f->is_fin is set according to whether the frame was marked as ending the
 * stream.
 */
int ossl_quic_wire_decode_frame_stream(PACKET *pkt, int nodata,
                                       OSSL_QUIC_FRAME_STREAM *f);

/*
 * Decodes a QUIC MAX_DATA frame. The Maximum Data field is written to
 * *max_data.
 */
int ossl_quic_wire_decode_frame_max_data(PACKET *pkt,
                                         uint64_t *max_data);

/*
 * Decodes a QUIC MAX_STREAM_DATA frame. The Stream ID is written to *stream_id
 * and Maximum Stream Data field is written to *max_stream_data.
 */
int ossl_quic_wire_decode_frame_max_stream_data(PACKET *pkt,
                                                uint64_t *stream_id,
                                          uint64_t *max_stream_data);
/*
 * Decodes a QUIC MAX_STREAMS frame. The Maximum Streams field is written to
 * *max_streams.
 *
 * Whether the limit concerns bidirectional streams or unidirectional streams is
 * denoted by the frame type; the caller should examine the frame type to
 * determine this.
 */
int ossl_quic_wire_decode_frame_max_streams(PACKET *pkt,
                                            uint64_t *max_streams);

/*
 * Decodes a QUIC DATA_BLOCKED frame. The Maximum Data field is written to
 * *max_data.
 */
int ossl_quic_wire_decode_frame_data_blocked(PACKET *pkt,
                                             uint64_t *max_data);

/*
 * Decodes a QUIC STREAM_DATA_BLOCKED frame. The Stream ID and Maximum Stream
 * Data fields are written to *stream_id and *max_stream_data respectively.
 */
int ossl_quic_wire_decode_frame_stream_data_blocked(PACKET *pkt,
                                                    uint64_t *stream_id,
                                                    uint64_t *max_stream_data);

/*
 * Decodes a QUIC STREAMS_BLOCKED frame. The Maximum Streams field is written to
 * *max_streams.
 *
 * Whether the limit concerns bidirectional streams or unidirectional streams is
 * denoted by the frame type; the caller should examine the frame type to
 * determine this.
 */
int ossl_quic_wire_decode_frame_streams_blocked(PACKET *pkt,
                                                uint64_t *max_streams);


/*
 * Decodes a QUIC NEW_CONNECTION_ID frame. The logical representation of the
 * frame is written to *f.
 *
 * The conn_id field is set to point to the connection ID string inside the
 * packet buffer; it is therefore valid for as long as the PACKET's buffer is
 * valid. The conn_id_len field is set to the length of the connection ID string
 * in bytes.
 */
int ossl_quic_wire_decode_frame_new_conn_id(PACKET *pkt,
                                            OSSL_QUIC_FRAME_NEW_CONN_ID *f);

/*
 * Decodes a QUIC RETIRE_CONNECTION_ID frame. The Sequence Number field
 * is written to *seq_num.
 */
int ossl_quic_wire_decode_frame_retire_conn_id(PACKET *pkt,
                                               uint64_t *seq_num);

/*
 * Decodes a QUIC PATH_CHALLENGE frame. The Data field is written to *data.
 */
int ossl_quic_wire_decode_frame_path_challenge(PACKET *pkt,
                                               uint64_t *data);

/*
 * Decodes a QUIC PATH_CHALLENGE frame. The Data field is written to *data.
 */
int ossl_quic_wire_decode_frame_path_response(PACKET *pkt,
                                              uint64_t *data);

/*
 * Decodes a QUIC CONNECTION_CLOSE frame. The logical representation
 * of the frame is written to *f.
 *
 * The reason field is set to point to the UTF-8 reason string inside
 * the packet buffer; it is therefore valid for as long as the PACKET's
 * buffer is valid. The reason_len field is set to the length of the
 * reason string in bytes.
 *
 * IMPORTANT: The reason string is not zero-terminated.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_quic_wire_decode_frame_conn_close(PACKET *pkt,
                                           OSSL_QUIC_FRAME_CONN_CLOSE *f);

/*
 * Decodes one or more PADDING frames. PADDING frames have no arguments.
 *
 * Returns the number of PADDING frames decoded or 0 on error.
 */
size_t ossl_quic_wire_decode_padding(PACKET *pkt);

/*
 * Decodes a PING frame. The frame has no arguments.
 */
int ossl_quic_wire_decode_frame_ping(PACKET *pkt);

/*
 * Decodes a HANDSHAKE_DONE frame. The frame has no arguments.
 */
int ossl_quic_wire_decode_frame_handshake_done(PACKET *pkt);

/*
 * Peeks at the ID of the next QUIC transport parameter TLV in the stream.
 * The ID is written to *id.
 */
int ossl_quic_wire_peek_transport_param(PACKET *pkt, uint64_t *id);

/*
 * Decodes a QUIC transport parameter TLV. A pointer to the value buffer is
 * returned on success. This points inside the PACKET's buffer and is therefore
 * valid as long as the PACKET's buffer is valid.
 *
 * The transport parameter ID is written to *id (if non-NULL) and the length of
 * the payload in bytes is written to *len.
 *
 * Returns NULL on failure.
 */
const unsigned char *ossl_quic_wire_decode_transport_param_bytes(PACKET *pkt,
                                                                 uint64_t *id,
                                                                 size_t *len);

/*
 * Decodes a QUIC transport parameter TLV containing a variable-length integer.
 *
 * The transport parameter ID is written to *id (if non-NULL) and the value is
 * written to *value.
 */
int ossl_quic_wire_decode_transport_param_int(PACKET *pkt,
                                              uint64_t *id,
                                              uint64_t *value);

/*
 * Decodes a QUIC transport parameter TLV containing a connection ID.
 *
 * The transport parameter ID is written to *id (if non-NULL) and the value is
 * written to *value.
 */
int ossl_quic_wire_decode_transport_param_cid(PACKET *pkt,
                                              uint64_t *id,
                                              QUIC_CONN_ID *cid);

/*
 * Decodes a QUIC transport parameter TLV containing a preferred_address.
 */
typedef struct quic_preferred_addr_st {
    uint16_t                    ipv4_port, ipv6_port;
    unsigned char               ipv4[4], ipv6[16];
    QUIC_STATELESS_RESET_TOKEN  stateless_reset;
    QUIC_CONN_ID                cid;
} QUIC_PREFERRED_ADDR;

int ossl_quic_wire_decode_transport_param_preferred_addr(PACKET *pkt,
                                                         QUIC_PREFERRED_ADDR *p);

# endif

#endif
