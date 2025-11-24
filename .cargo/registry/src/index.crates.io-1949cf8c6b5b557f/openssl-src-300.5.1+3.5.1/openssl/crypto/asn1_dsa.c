/*
 * Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * A simple ASN.1 DER encoder/decoder for DSA-Sig-Value and ECDSA-Sig-Value.
 *
 * DSA-Sig-Value ::= SEQUENCE {
 *  r  INTEGER,
 *  s  INTEGER
 * }
 *
 * ECDSA-Sig-Value ::= SEQUENCE {
 *  r  INTEGER,
 *  s  INTEGER
 * }
 */

#include <openssl/crypto.h>
#include <openssl/bn.h>
#include "crypto/asn1_dsa.h"
#include "internal/packet.h"

#define ID_SEQUENCE 0x30
#define ID_INTEGER 0x02

/*
 * Outputs the encoding of the length octets for a DER value with a content
 * length of cont_len bytes to pkt. The maximum supported content length is
 * 65535 (0xffff) bytes.
 *
 * Returns 1 on success or 0 on error.
 */
int ossl_encode_der_length(WPACKET *pkt, size_t cont_len)
{
    if (cont_len > 0xffff)
        return 0; /* Too large for supported length encodings */

    if (cont_len > 0xff) {
        if (!WPACKET_put_bytes_u8(pkt, 0x82)
                || !WPACKET_put_bytes_u16(pkt, cont_len))
            return 0;
    } else {
        if (cont_len > 0x7f
                && !WPACKET_put_bytes_u8(pkt, 0x81))
            return 0;
        if (!WPACKET_put_bytes_u8(pkt, cont_len))
            return 0;
    }

    return 1;
}

/*
 * Outputs the DER encoding of a positive ASN.1 INTEGER to pkt.
 *
 * Results in an error if n is negative or too large.
 *
 * Returns 1 on success or 0 on error.
 */
int ossl_encode_der_integer(WPACKET *pkt, const BIGNUM *n)
{
    unsigned char *bnbytes;
    size_t cont_len;

    if (BN_is_negative(n))
        return 0;

    /*
     * Calculate the ASN.1 INTEGER DER content length for n.
     * This is the number of whole bytes required to represent n (i.e. rounded
     * down), plus one.
     * If n is zero then the content is a single zero byte (length = 1).
     * If the number of bits of n is a multiple of 8 then an extra zero padding
     * byte is included to ensure that the value is still treated as positive
     * in the INTEGER two's complement representation.
     */
    cont_len = BN_num_bits(n) / 8 + 1;

    if (!WPACKET_start_sub_packet(pkt)
            || !WPACKET_put_bytes_u8(pkt, ID_INTEGER)
            || !ossl_encode_der_length(pkt, cont_len)
            || !WPACKET_allocate_bytes(pkt, cont_len, &bnbytes)
            || !WPACKET_close(pkt))
        return 0;

    if (bnbytes != NULL
            && BN_bn2binpad(n, bnbytes, (int)cont_len) != (int)cont_len)
        return 0;

    return 1;
}

/*
 * Outputs the DER encoding of a DSA-Sig-Value or ECDSA-Sig-Value to pkt. pkt
 * may be initialised with a NULL buffer which enables pkt to be used to
 * calculate how many bytes would be needed.
 *
 * Returns 1 on success or 0 on error.
 */
int ossl_encode_der_dsa_sig(WPACKET *pkt, const BIGNUM *r, const BIGNUM *s)
{
    WPACKET tmppkt, *dummypkt;
    size_t cont_len;
    int isnull = WPACKET_is_null_buf(pkt);

    if (!WPACKET_start_sub_packet(pkt))
        return 0;

    if (!isnull) {
        if (!WPACKET_init_null(&tmppkt, 0))
            return 0;
        dummypkt = &tmppkt;
    } else {
        /* If the input packet has a NULL buffer, we don't need a dummy packet */
        dummypkt = pkt;
    }

    /* Calculate the content length */
    if (!ossl_encode_der_integer(dummypkt, r)
            || !ossl_encode_der_integer(dummypkt, s)
            || !WPACKET_get_length(dummypkt, &cont_len)
            || (!isnull && !WPACKET_finish(dummypkt))) {
        if (!isnull)
            WPACKET_cleanup(dummypkt);
        return 0;
    }

    /* Add the tag and length bytes */
    if (!WPACKET_put_bytes_u8(pkt, ID_SEQUENCE)
            || !ossl_encode_der_length(pkt, cont_len)
               /*
                * Really encode the integers. We already wrote to the main pkt
                * if it had a NULL buffer, so don't do it again
                */
            || (!isnull && !ossl_encode_der_integer(pkt, r))
            || (!isnull && !ossl_encode_der_integer(pkt, s))
            || !WPACKET_close(pkt))
        return 0;

    return 1;
}

/*
 * Decodes the DER length octets in pkt and initialises subpkt with the
 * following bytes of that length.
 *
 * Returns 1 on success or 0 on failure.
 */
int ossl_decode_der_length(PACKET *pkt, PACKET *subpkt)
{
    unsigned int byte;

    if (!PACKET_get_1(pkt, &byte))
        return 0;

    if (byte < 0x80)
        return PACKET_get_sub_packet(pkt, subpkt, (size_t)byte);
    if (byte == 0x81)
        return PACKET_get_length_prefixed_1(pkt, subpkt);
    if (byte == 0x82)
        return PACKET_get_length_prefixed_2(pkt, subpkt);

    /* Too large, invalid, or not DER. */
    return 0;
}

/*
 * Decodes a single ASN.1 INTEGER value from pkt, which must be DER encoded,
 * and updates n with the decoded value.
 *
 * The BIGNUM, n, must have already been allocated by calling BN_new().
 * pkt must not be NULL.
 *
 * An attempt to consume more than len bytes results in an error.
 * Returns 1 on success or 0 on error.
 *
 * If the PACKET is supposed to only contain a single INTEGER value with no
 * trailing garbage then it is up to the caller to verify that all bytes
 * were consumed.
 */
int ossl_decode_der_integer(PACKET *pkt, BIGNUM *n)
{
    PACKET contpkt, tmppkt;
    unsigned int tag, tmp;

    /* Check we have an integer and get the content bytes */
    if (!PACKET_get_1(pkt, &tag)
            || tag != ID_INTEGER
            || !ossl_decode_der_length(pkt, &contpkt))
        return 0;

    /* Peek ahead at the first bytes to check for proper encoding */
    tmppkt = contpkt;
    /* The INTEGER must be positive */
    if (!PACKET_get_1(&tmppkt, &tmp)
            || (tmp & 0x80) != 0)
        return 0;
    /* If there a zero padding byte the next byte must have the msb set */
    if (PACKET_remaining(&tmppkt) > 0 && tmp == 0) {
        if (!PACKET_get_1(&tmppkt, &tmp)
                || (tmp & 0x80) == 0)
            return 0;
    }

    if (BN_bin2bn(PACKET_data(&contpkt),
                  (int)PACKET_remaining(&contpkt), n) == NULL)
        return 0;

    return 1;
}

/*
 * Decodes a single DSA-Sig-Value or ECDSA-Sig-Value from *ppin, which must be
 * DER encoded, updates r and s with the decoded values, and increments *ppin
 * past the data that was consumed.
 *
 * The BIGNUMs, r and s, must have already been allocated by calls to BN_new().
 * ppin and *ppin must not be NULL.
 *
 * An attempt to consume more than len bytes results in an error.
 * Returns the number of bytes of input consumed or 0 if an error occurs.
 *
 * If the buffer is supposed to only contain a single [EC]DSA-Sig-Value with no
 * trailing garbage then it is up to the caller to verify that all bytes
 * were consumed.
 */
size_t ossl_decode_der_dsa_sig(BIGNUM *r, BIGNUM *s,
                               const unsigned char **ppin, size_t len)
{
    size_t consumed;
    PACKET pkt, contpkt;
    unsigned int tag;

    if (!PACKET_buf_init(&pkt, *ppin, len)
            || !PACKET_get_1(&pkt, &tag)
            || tag != ID_SEQUENCE
            || !ossl_decode_der_length(&pkt, &contpkt)
            || !ossl_decode_der_integer(&contpkt, r)
            || !ossl_decode_der_integer(&contpkt, s)
            || PACKET_remaining(&contpkt) != 0)
        return 0;

    consumed = PACKET_data(&pkt) - *ppin;
    *ppin += consumed;
    return consumed;
}
