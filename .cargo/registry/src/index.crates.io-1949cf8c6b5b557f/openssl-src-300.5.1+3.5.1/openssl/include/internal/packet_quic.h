/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_PACKET_QUIC_H
# define OSSL_INTERNAL_PACKET_QUIC_H
# pragma once

# include "internal/packet.h"
# include "internal/quic_vlint.h"

# ifndef OPENSSL_NO_QUIC
/*
 * Decodes a QUIC variable-length integer in |pkt| and stores the result in
 * |data|.
 */
__owur static ossl_inline int PACKET_get_quic_vlint(PACKET *pkt,
                                                    uint64_t *data)
{
    size_t enclen;

    if (PACKET_remaining(pkt) < 1)
        return 0;

    enclen = ossl_quic_vlint_decode_len(*pkt->curr);

    if (PACKET_remaining(pkt) < enclen)
        return 0;

    *data = ossl_quic_vlint_decode_unchecked(pkt->curr);
    packet_forward(pkt, enclen);
    return 1;
}

/*
 * Decodes a QUIC variable-length integer in |pkt| and stores the result in
 * |data|. Unlike PACKET_get_quic_vlint, this does not advance the current
 * position. If was_minimal is non-NULL, *was_minimal is set to 1 if the integer
 * was encoded using the minimal possible number of bytes and 0 otherwise.
 */
__owur static ossl_inline int PACKET_peek_quic_vlint_ex(PACKET *pkt,
                                                        uint64_t *data,
                                                        int *was_minimal)
{
    size_t enclen;

    if (PACKET_remaining(pkt) < 1)
        return 0;

    enclen = ossl_quic_vlint_decode_len(*pkt->curr);

    if (PACKET_remaining(pkt) < enclen)
        return 0;

    *data = ossl_quic_vlint_decode_unchecked(pkt->curr);

    if (was_minimal != NULL)
        *was_minimal = (enclen == ossl_quic_vlint_encode_len(*data));

    return 1;
}

__owur static ossl_inline int PACKET_peek_quic_vlint(PACKET *pkt,
                                                     uint64_t *data)
{
    return PACKET_peek_quic_vlint_ex(pkt, data, NULL);
}

/*
 * Skips over a QUIC variable-length integer in |pkt| without decoding it.
 */
__owur static ossl_inline int PACKET_skip_quic_vlint(PACKET *pkt)
{
    size_t enclen;

    if (PACKET_remaining(pkt) < 1)
        return 0;

    enclen = ossl_quic_vlint_decode_len(*pkt->curr);

    if (PACKET_remaining(pkt) < enclen)
        return 0;

    packet_forward(pkt, enclen);
    return 1;
}

/*
 * Reads a variable-length vector prefixed with a QUIC variable-length integer
 * denoting the length, and stores the contents in |subpkt|. |pkt| can equal
 * |subpkt|. Data is not copied: the |subpkt| packet will share its underlying
 * buffer with the original |pkt|, so data wrapped by |pkt| must outlive the
 * |subpkt|. Upon failure, the original |pkt| and |subpkt| are not modified.
 */
__owur static ossl_inline int PACKET_get_quic_length_prefixed(PACKET *pkt,
                                                              PACKET *subpkt)
{
    uint64_t length;
    const unsigned char *data;
    PACKET tmp = *pkt;

    if (!PACKET_get_quic_vlint(&tmp, &length) ||
        length > SIZE_MAX ||
        !PACKET_get_bytes(&tmp, &data, (size_t)length)) {
        return 0;
    }

    *pkt = tmp;
    subpkt->curr = data;
    subpkt->remaining = (size_t)length;

    return 1;
}

/*
 * Starts a QUIC sub-packet headed by a QUIC variable-length integer. A 4-byte
 * representation is used.
 */
__owur int WPACKET_start_quic_sub_packet(WPACKET *pkt);

/*
 * Starts a QUIC sub-packet headed by a QUIC variable-length integer. max_len
 * specifies the upper bound for the sub-packet size at the time the sub-packet
 * is closed, which determines the encoding size for the variable-length
 * integer header. max_len can be a precise figure or a worst-case bound
 * if a precise figure is not available.
 */
__owur int WPACKET_start_quic_sub_packet_bound(WPACKET *pkt, size_t max_len);

/*
 * Allocates a QUIC sub-packet with exactly len bytes of payload, headed by a
 * QUIC variable-length integer. The pointer to the payload buffer is output and
 * must be filled by the caller. This function assures optimal selection of
 * variable-length integer encoding length.
 */
__owur int WPACKET_quic_sub_allocate_bytes(WPACKET *pkt, size_t len,
                                           unsigned char **bytes);

/*
 * Write a QUIC variable-length integer to the packet.
 */
__owur int WPACKET_quic_write_vlint(WPACKET *pkt, uint64_t v);

# endif                         /* OPENSSL_NO_QUIC */
#endif                          /* OSSL_INTERNAL_PACKET_QUIC_H */
