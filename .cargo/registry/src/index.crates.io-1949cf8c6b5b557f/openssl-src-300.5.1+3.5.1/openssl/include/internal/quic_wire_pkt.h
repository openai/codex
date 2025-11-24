/*
 * Copyright 2022-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_QUIC_WIRE_PKT_H
# define OSSL_QUIC_WIRE_PKT_H

# include <openssl/ssl.h>
# include "internal/packet_quic.h"
# include "internal/quic_types.h"

# ifndef OPENSSL_NO_QUIC

#  define QUIC_VERSION_NONE   ((uint32_t)0)   /* Used for version negotiation */
#  define QUIC_VERSION_1      ((uint32_t)1)   /* QUIC v1 */

/* QUIC logical packet type. These do not match wire values. */
#  define QUIC_PKT_TYPE_INITIAL        1
#  define QUIC_PKT_TYPE_0RTT           2
#  define QUIC_PKT_TYPE_HANDSHAKE      3
#  define QUIC_PKT_TYPE_RETRY          4
#  define QUIC_PKT_TYPE_1RTT           5
#  define QUIC_PKT_TYPE_VERSION_NEG    6

/*
 * Determine encryption level from packet type. Returns QUIC_ENC_LEVEL_NUM if
 * the packet is not of a type which is encrypted.
 */
static ossl_inline ossl_unused uint32_t
ossl_quic_pkt_type_to_enc_level(uint32_t pkt_type)
{
    switch (pkt_type) {
        case QUIC_PKT_TYPE_INITIAL:
            return QUIC_ENC_LEVEL_INITIAL;
        case QUIC_PKT_TYPE_HANDSHAKE:
            return QUIC_ENC_LEVEL_HANDSHAKE;
        case QUIC_PKT_TYPE_0RTT:
            return QUIC_ENC_LEVEL_0RTT;
        case QUIC_PKT_TYPE_1RTT:
            return QUIC_ENC_LEVEL_1RTT;
        default:
            return QUIC_ENC_LEVEL_NUM;
    }
}

static ossl_inline ossl_unused uint32_t
ossl_quic_enc_level_to_pkt_type(uint32_t enc_level)
{
    switch (enc_level) {
        case QUIC_ENC_LEVEL_INITIAL:
            return QUIC_PKT_TYPE_INITIAL;
        case QUIC_ENC_LEVEL_HANDSHAKE:
            return QUIC_PKT_TYPE_HANDSHAKE;
        case QUIC_ENC_LEVEL_0RTT:
            return QUIC_PKT_TYPE_0RTT;
        case QUIC_ENC_LEVEL_1RTT:
            return QUIC_PKT_TYPE_1RTT;
        default:
            return UINT32_MAX;
    }
}

/* Determine if a packet type contains an encrypted payload. */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_is_encrypted(uint32_t pkt_type)
{
    switch (pkt_type) {
        case QUIC_PKT_TYPE_RETRY:
        case QUIC_PKT_TYPE_VERSION_NEG:
            return 0;
        default:
            return 1;
    }
}

/* Determine if a packet type contains a PN field. */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_has_pn(uint32_t pkt_type)
{
    /*
     * Currently a packet has a PN iff it is encrypted. This could change
     * someday.
     */
    return ossl_quic_pkt_type_is_encrypted(pkt_type);
}

/*
 * Determine if a packet type can appear with other packets in a datagram. Some
 * packet types must be the sole packet in a datagram.
 */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_can_share_dgram(uint32_t pkt_type)
{
    /*
     * Currently only the encrypted packet types can share a datagram. This
     * could change someday.
     */
    return ossl_quic_pkt_type_is_encrypted(pkt_type);
}

/*
 * Determine if the packet type must come at the end of the datagram (due to the
 * lack of a length field).
 */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_must_be_last(uint32_t pkt_type)
{
    /*
     * Any packet type which cannot share a datagram obviously must come last.
     * 1-RTT also must come last as it lacks a length field.
     */
    return !ossl_quic_pkt_type_can_share_dgram(pkt_type)
        || pkt_type == QUIC_PKT_TYPE_1RTT;
}

/*
 * Determine if the packet type has a version field.
 */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_has_version(uint32_t pkt_type)
{
    return pkt_type != QUIC_PKT_TYPE_1RTT && pkt_type != QUIC_PKT_TYPE_VERSION_NEG;
}

/*
 * Determine if the packet type has a SCID field.
 */
static ossl_inline ossl_unused int
ossl_quic_pkt_type_has_scid(uint32_t pkt_type)
{
    return pkt_type != QUIC_PKT_TYPE_1RTT;
}

/*
 * Smallest possible QUIC packet size as per RFC (aside from version negotiation
 * packets).
 */
#  define QUIC_MIN_VALID_PKT_LEN_CRYPTO      21
#  define QUIC_MIN_VALID_PKT_LEN_VERSION_NEG  7
#  define QUIC_MIN_VALID_PKT_LEN              QUIC_MIN_VALID_PKT_LEN_VERSION_NEG

typedef struct quic_pkt_hdr_ptrs_st QUIC_PKT_HDR_PTRS;

/*
 * QUIC Packet Header Protection
 * =============================
 *
 * Functions to apply and remove QUIC packet header protection. A header
 * protector is initialised using ossl_quic_hdr_protector_init and must be
 * destroyed using ossl_quic_hdr_protector_cleanup when no longer needed.
 */
typedef struct quic_hdr_protector_st {
    OSSL_LIB_CTX       *libctx;
    const char         *propq;
    EVP_CIPHER_CTX     *cipher_ctx;
    EVP_CIPHER         *cipher;
    uint32_t            cipher_id;
} QUIC_HDR_PROTECTOR;

#  define QUIC_HDR_PROT_CIPHER_AES_128    1
#  define QUIC_HDR_PROT_CIPHER_AES_256    2
#  define QUIC_HDR_PROT_CIPHER_CHACHA     3

/*
 * Initialises a header protector.
 *
 *   cipher_id:
 *      The header protection cipher method to use. One of
 *      QUIC_HDR_PROT_CIPHER_*. Must be chosen based on negotiated TLS cipher
 *      suite.
 *
 *   quic_hp_key:
 *      This must be the "quic hp" key derived from a traffic secret.
 *
 *      The length of the quic_hp_key must correspond to that expected for the
 *      given cipher ID.
 *
 * The header protector performs amortisable initialisation in this function,
 * therefore a header protector should be used for as long as possible.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_hdr_protector_init(QUIC_HDR_PROTECTOR *hpr,
                                 OSSL_LIB_CTX *libctx,
                                 const char *propq,
                                 uint32_t cipher_id,
                                 const unsigned char *quic_hp_key,
                                 size_t quic_hp_key_len);

/*
 * Destroys a header protector. This is also safe to call on a zero-initialized
 * OSSL_QUIC_HDR_PROTECTOR structure which has not been initialized, or which
 * has already been destroyed.
 */
void ossl_quic_hdr_protector_cleanup(QUIC_HDR_PROTECTOR *hpr);

/*
 * Removes header protection from a packet. The packet payload must currently be
 * encrypted (i.e., you must remove header protection before decrypting packets
 * received). The function examines the header buffer to determine which bytes
 * of the header need to be decrypted.
 *
 * If this function fails, no data is modified.
 *
 * This is implemented as a call to ossl_quic_hdr_protector_decrypt_fields().
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_hdr_protector_decrypt(QUIC_HDR_PROTECTOR *hpr,
                                    QUIC_PKT_HDR_PTRS *ptrs);

/*
 * Applies header protection to a packet. The packet payload must already have
 * been encrypted (i.e., you must apply header protection after encrypting
 * a packet). The function examines the header buffer to determine which bytes
 * of the header need to be encrypted.
 *
 * This is implemented as a call to ossl_quic_hdr_protector_encrypt_fields().
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_hdr_protector_encrypt(QUIC_HDR_PROTECTOR *hpr,
                                    QUIC_PKT_HDR_PTRS *ptrs);

/*
 * Removes header protection from a packet. The packet payload must currently
 * be encrypted. This is a low-level function which assumes you have already
 * determined which parts of the packet header need to be decrypted.
 *
 * sample:
 *   The range of bytes in the packet to be used to generate the header
 *   protection mask. It is permissible to set sample_len to the size of the
 *   remainder of the packet; this function will only use as many bytes as
 *   needed. If not enough sample bytes are provided, this function fails.
 *
 * first_byte:
 *   The first byte of the QUIC packet header to be decrypted.
 *
 * pn:
 *   Pointer to the start of the PN field. The caller is responsible
 *   for ensuring at least four bytes follow this pointer.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_hdr_protector_decrypt_fields(QUIC_HDR_PROTECTOR *hpr,
                                           const unsigned char *sample,
                                           size_t sample_len,
                                           unsigned char *first_byte,
                                           unsigned char *pn_bytes);

/*
 * Works analogously to ossl_hdr_protector_decrypt_fields, but applies header
 * protection instead of removing it.
 */
int ossl_quic_hdr_protector_encrypt_fields(QUIC_HDR_PROTECTOR *hpr,
                                           const unsigned char *sample,
                                           size_t sample_len,
                                           unsigned char *first_byte,
                                           unsigned char *pn_bytes);

/*
 * QUIC Packet Header
 * ==================
 *
 * This structure provides a logical representation of a QUIC packet header.
 *
 * QUIC packet formats fall into the following categories:
 *
 *   Long Packets, which is subdivided into five possible packet types:
 *     Version Negotiation (a special case);
 *     Initial;
 *     0-RTT;
 *     Handshake; and
 *     Retry
 *
 *   Short Packets, which comprises only a single packet type (1-RTT).
 *
 * The packet formats vary and common fields are found in some packets but
 * not others. The below table indicates which fields are present in which
 * kinds of packet. * indicates header protection is applied.
 *
 *   SLLLLL         Legend: 1=1-RTT, i=Initial, 0=0-RTT, h=Handshake
 *   1i0hrv                 r=Retry, v=Version Negotiation
 *   ------
 *   1i0hrv         Header Form (0=Short, 1=Long)
 *   1i0hr          Fixed Bit (always 1)
 *   1              Spin Bit
 *   1       *      Reserved Bits
 *   1       *      Key Phase
 *   1i0h    *      Packet Number Length
 *    i0hr?         Long Packet Type
 *    i0h           Type-Specific Bits
 *    i0hr          Version (note: always 0 for Version Negotiation packets)
 *   1i0hrv         Destination Connection ID
 *    i0hrv         Source Connection ID
 *   1i0h    *      Packet Number
 *    i             Token
 *    i0h           Length
 *       r          Retry Token
 *       r          Retry Integrity Tag
 *
 * For each field below, the conditions under which the field is valid are
 * specified. If a field is not currently valid, it is initialized to a zero or
 * NULL value.
 */
typedef struct quic_pkt_hdr_st {
    /* [ALL] A QUIC_PKT_TYPE_* value. Always valid. */
    unsigned int    type        :8;

    /* [S] Value of the spin bit. Valid if (type == 1RTT). */
    unsigned int    spin_bit    :1;

    /*
     * [S] Value of the Key Phase bit in the short packet.
     * Valid if (type == 1RTT && !partial).
     */
    unsigned int    key_phase   :1;

    /*
     * [1i0h] Length of packet number in bytes. This is the decoded value.
     * Valid if ((type == 1RTT || (version && type != RETRY)) && !partial).
     */
    unsigned int    pn_len      :4;

    /*
     * [ALL] Set to 1 if this is a partial decode because the packet header
     * has not yet been deprotected. pn_len, pn and key_phase are not valid if
     * this is set.
     */
    unsigned int    partial     :1;

    /*
     * [ALL] Whether the fixed bit was set. Note that only Version Negotiation
     * packets are allowed to have this unset, so this will always be 1 for all
     * other packet types (decode will fail if it is not set). Ignored when
     * encoding unless encoding a Version Negotiation packet.
     */
    unsigned int    fixed       :1;

    /*
     * The unused bits in the low 4 bits of a Retry packet header's first byte.
     * This is used to ensure that Retry packets have the same bit-for-bit
     * representation in their header when decoding and encoding them again.
     * This is necessary to validate Retry packet headers.
     */
    unsigned int    unused      :4;

    /*
     * The 'Reserved' bits in an Initial, Handshake, 0-RTT or 1-RTT packet
     * header's first byte. These are provided so that the caller can validate
     * that they are zero, as this must be done after packet protection is
     * successfully removed to avoid creating a timing channel.
     */
    unsigned int    reserved    :2;

    /* [L] Version field. Valid if (type != 1RTT). */
    uint32_t        version;

    /* [ALL] The destination connection ID. Always valid. */
    QUIC_CONN_ID    dst_conn_id;

    /*
     * [L] The source connection ID.
     * Valid if (type != 1RTT).
     */
    QUIC_CONN_ID    src_conn_id;

    /*
     * [1i0h] Relatively-encoded packet number in raw, encoded form. The correct
     * decoding of this value is context-dependent. The number of bytes valid in
     * this buffer is determined by pn_len above. If the decode was partial,
     * this field is not valid.
     *
     * Valid if ((type == 1RTT || (version && type != RETRY)) && !partial).
     */
    unsigned char           pn[4];

    /*
     * [i] Token field in Initial packet. Points to memory inside the decoded
     * PACKET, and therefore is valid for as long as the PACKET's buffer is
     * valid. token_len is the length of the token in bytes.
     *
     * Valid if (type == INITIAL).
     */
    const unsigned char    *token;
    size_t                  token_len;

    /*
     * [ALL] Payload length in bytes.
     *
     * Though 1-RTT, Retry and Version Negotiation packets do not contain an
     * explicit length field, this field is always valid and is used by the
     * packet header encoding and decoding routines to describe the payload
     * length, regardless of whether the packet type encoded or decoded uses an
     * explicit length indication.
     */
    size_t                  len;

    /*
     * Pointer to start of payload data in the packet. Points to memory inside
     * the decoded PACKET, and therefore is valid for as long as the PACKET'S
     * buffer is valid. The length of the buffer in bytes is in len above.
     *
     * For Version Negotiation packets, points to the array of supported
     * versions.
     *
     * For Retry packets, points to the Retry packet payload, which comprises
     * the Retry Token followed by a 16-byte Retry Integrity Tag.
     *
     * Regardless of whether a packet is a Version Negotiation packet (where the
     * payload contains a list of supported versions), a Retry packet (where the
     * payload contains a Retry Token and Retry Integrity Tag), or any other
     * packet type (where the payload contains frames), the payload is not
     * validated and the user must parse the payload bearing this in mind.
     *
     * If the decode was partial (partial is set), this points to the start of
     * the packet number field, rather than the protected payload, as the length
     * of the packet number field is unknown. The len field reflects this in
     * this case (i.e., the len field is the number of payload bytes plus the
     * number of bytes comprising the PN).
     */
    const unsigned char    *data;
} QUIC_PKT_HDR;

/*
 * Extra information which can be output by the packet header decode functions
 * for the assistance of the header protector. This avoids the header protector
 * needing to partially re-decode the packet header.
 */
struct quic_pkt_hdr_ptrs_st {
    unsigned char    *raw_start;        /* start of packet */
    unsigned char    *raw_sample;       /* start of sampling range */
    size_t            raw_sample_len;   /* maximum length of sampling range */

    /*
     * Start of PN field. Guaranteed to be NULL unless at least four bytes are
     * available via this pointer.
     */
    unsigned char    *raw_pn;
};

/*
 * If partial is 1, reads the unprotected parts of a protected packet header
 * from a PACKET, performing a partial decode.
 *
 * If partial is 0, the input is assumed to have already had header protection
 * removed, and all header fields are decoded.
 *
 * If nodata is 1, the input is assumed to have no payload data in it. Otherwise
 * payload data must be present.
 *
 * On success, the logical decode of the packet header is written to *hdr.
 * hdr->partial is set or cleared according to whether a partial decode was
 * performed. *ptrs is filled with pointers to various parts of the packet
 * buffer.
 *
 * In order to decode short packets, the connection ID length being used must be
 * known contextually, and should be passed as short_conn_id_len. If
 * short_conn_id_len is set to an invalid value (a value greater than
 * QUIC_MAX_CONN_ID_LEN), this function fails when trying to decode a short
 * packet, but succeeds for long packets.
 *
 * fail_cause is a bitmask of the reasons decode might have failed
 * as defined below, useful when you need to interrogate parts of
 * a header even if its otherwise undecodeable.  May be NULL.
 *
 * Returns 1 on success and 0 on failure.
 */

#  define QUIC_PKT_HDR_DECODE_DECODE_ERR  (1 << 0)
#  define QUIC_PKT_HDR_DECODE_BAD_VERSION (1 << 1)

int ossl_quic_wire_decode_pkt_hdr(PACKET *pkt,
                                  size_t short_conn_id_len,
                                  int partial,
                                  int nodata,
                                  QUIC_PKT_HDR *hdr,
                                  QUIC_PKT_HDR_PTRS *ptrs,
                                  uint64_t *fail_cause);

/*
 * Encodes a packet header. The packet is written to pkt.
 *
 * The length of the (encrypted) packet payload should be written to hdr->len
 * and will be placed in the serialized packet header. The payload data itself
 * is not copied; the caller should write hdr->len bytes of encrypted payload to
 * the WPACKET immediately after the call to this function. However,
 * WPACKET_reserve_bytes is called for the payload size.
 *
 * This function does not apply header protection. You must apply header
 * protection yourself after calling this function. *ptrs is filled with
 * pointers which can be passed to a header protector, but this must be
 * performed after the encrypted payload is written.
 *
 * The pointers in *ptrs are direct pointers into the WPACKET buffer. If more
 * data is written to the WPACKET buffer, WPACKET buffer reallocations may
 * occur, causing these pointers to become invalid. Therefore, you must not call
 * any write WPACKET function between this call and the call to
 * ossl_quic_hdr_protector_encrypt. This function calls WPACKET_reserve_bytes
 * for the payload length, so you may assume hdr->len bytes are already free to
 * write at the WPACKET cursor location once this function returns successfully.
 * It is recommended that you call this function, write the encrypted payload,
 * call ossl_quic_hdr_protector_encrypt, and then call
 * WPACKET_allocate_bytes(hdr->len).
 *
 * Version Negotiation and Retry packets do not use header protection; for these
 * header types, the fields in *ptrs are all written as zero. Version
 * Negotiation, Retry and 1-RTT packets do not contain a Length field, but
 * hdr->len bytes of data are still reserved in the WPACKET.
 *
 * If serializing a short packet and short_conn_id_len does not match the DCID
 * specified in hdr, the function fails.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_wire_encode_pkt_hdr(WPACKET *pkt,
                                  size_t short_conn_id_len,
                                  const QUIC_PKT_HDR *hdr,
                                  QUIC_PKT_HDR_PTRS *ptrs);

/*
 * Retrieves only the DCID from a packet header. This is intended for demuxer
 * use. It avoids the need to parse the rest of the packet header twice.
 *
 * Information on packet length is not decoded, as this only needs to be used on
 * the first packet in a datagram, therefore this takes a buffer and not a
 * PACKET.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_wire_get_pkt_hdr_dst_conn_id(const unsigned char *buf,
                                           size_t buf_len,
                                           size_t short_conn_id_len,
                                           QUIC_CONN_ID *dst_conn_id);

/*
 * Precisely predicts the encoded length of a packet header structure.
 *
 * May return 0 if the packet header is not valid, but the fact that this
 * function returns non-zero does not guarantee that
 * ossl_quic_wire_encode_pkt_hdr() will succeed.
 */
int ossl_quic_wire_get_encoded_pkt_hdr_len(size_t short_conn_id_len,
                                           const QUIC_PKT_HDR *hdr);

/*
 * Packet Number Encoding
 * ======================
 */

/*
 * Decode an encoded packet header QUIC PN.
 *
 * enc_pn is the raw encoded PN to decode. enc_pn_len is its length in bytes as
 * indicated by packet headers. largest_pn is the largest PN successfully
 * processed in the relevant PN space.
 *
 * The resulting PN is written to *res_pn.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_quic_wire_decode_pkt_hdr_pn(const unsigned char *enc_pn,
                                     size_t enc_pn_len,
                                     QUIC_PN largest_pn,
                                     QUIC_PN *res_pn);

/*
 * Determine how many bytes should be used to encode a PN. Returns the number of
 * bytes (which will be in range [1, 4]).
 */
int ossl_quic_wire_determine_pn_len(QUIC_PN pn, QUIC_PN largest_acked);

/*
 * Encode a PN for a packet header using the specified number of bytes, which
 * should have been determined by calling ossl_quic_wire_determine_pn_len. The
 * PN encoding process is done in two parts to allow the caller to override PN
 * encoding length if it wishes.
 *
 * Returns 1 on success and 0 on failure.
 */
int ossl_quic_wire_encode_pkt_hdr_pn(QUIC_PN pn,
                                     unsigned char *enc_pn,
                                     size_t enc_pn_len);

/*
 * Retry Integrity Tags
 * ====================
 */

#  define QUIC_RETRY_INTEGRITY_TAG_LEN    16

/*
 * Validate a retry integrity tag. Returns 1 if the tag is valid.
 *
 * Must be called on a hdr with a type of QUIC_PKT_TYPE_RETRY with a valid data
 * pointer.
 *
 * client_initial_dcid must be the original DCID used by the client in its first
 * Initial packet, as this is used to calculate the Retry Integrity Tag.
 *
 * Returns 0 if the tag is invalid, if called on any other type of packet or if
 * the body is too short.
 */
int ossl_quic_validate_retry_integrity_tag(OSSL_LIB_CTX *libctx,
                                           const char *propq,
                                           const QUIC_PKT_HDR *hdr,
                                           const QUIC_CONN_ID *client_initial_dcid);

/*
 * Calculates a retry integrity tag. Returns 0 on error, for example if hdr does
 * not have a type of QUIC_PKT_TYPE_RETRY.
 *
 * client_initial_dcid must be the original DCID used by the client in its first
 * Initial packet, as this is used to calculate the Retry Integrity Tag.
 *
 * tag must point to a buffer of QUIC_RETRY_INTEGRITY_TAG_LEN bytes in size.
 *
 * Note that hdr->data must point to the Retry packet body, and hdr->len must
 * include the space for the Retry Integrity Tag. (This means that you can
 * easily fill in a tag in a Retry packet you are generating by calling this
 * function and passing (hdr->data + hdr->len - QUIC_RETRY_INTEGRITY_TAG_LEN) as
 * the tag argument.) This function fails if hdr->len is too short to contain a
 * Retry Integrity Tag.
 */
int ossl_quic_calculate_retry_integrity_tag(OSSL_LIB_CTX *libctx,
                                            const char *propq,
                                            const QUIC_PKT_HDR *hdr,
                                            const QUIC_CONN_ID *client_initial_dcid,
                                            unsigned char *tag);

# endif

#endif
