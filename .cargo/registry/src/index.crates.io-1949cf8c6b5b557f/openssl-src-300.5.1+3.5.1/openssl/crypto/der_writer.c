/*
 * Copyright 2020 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <stdlib.h>
#include <string.h>
#include "internal/cryptlib.h"
#include "internal/der.h"
#include "crypto/bn.h"

static int int_start_context(WPACKET *pkt, int tag)
{
    if (tag < 0)
        return 1;
    if (!ossl_assert(tag <= 30))
        return 0;
    return WPACKET_start_sub_packet(pkt);
}

static int int_end_context(WPACKET *pkt, int tag)
{
    /*
     * If someone set the flag WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH on this
     * sub-packet and this sub-packet has nothing written to it, the DER length
     * will not be written, and the total written size will be unchanged before
     * and after WPACKET_close().  We use size1 and size2 to determine if
     * anything was written, and only write our tag if it has.
     *
     */
    size_t size1, size2;

    if (tag < 0)
        return 1;
    if (!ossl_assert(tag <= 30))
        return 0;

    /* Context specific are normally (?) constructed */
    tag |= DER_F_CONSTRUCTED | DER_C_CONTEXT;

    return WPACKET_get_total_written(pkt, &size1)
        && WPACKET_close(pkt)
        && WPACKET_get_total_written(pkt, &size2)
        && (size1 == size2 || WPACKET_put_bytes_u8(pkt, tag));
}

int ossl_DER_w_precompiled(WPACKET *pkt, int tag,
                           const unsigned char *precompiled,
                           size_t precompiled_n)
{
    return int_start_context(pkt, tag)
        && WPACKET_memcpy(pkt, precompiled, precompiled_n)
        && int_end_context(pkt, tag);
}

int ossl_DER_w_boolean(WPACKET *pkt, int tag, int b)
{
    return int_start_context(pkt, tag)
        && WPACKET_start_sub_packet(pkt)
        && (!b || WPACKET_put_bytes_u8(pkt, 0xFF))
        && !WPACKET_close(pkt)
        && !WPACKET_put_bytes_u8(pkt, DER_P_BOOLEAN)
        && int_end_context(pkt, tag);
}

int ossl_DER_w_octet_string(WPACKET *pkt, int tag,
                            const unsigned char *data, size_t data_n)
{
    return int_start_context(pkt, tag)
        && WPACKET_start_sub_packet(pkt)
        && WPACKET_memcpy(pkt, data, data_n)
        && WPACKET_close(pkt)
        && WPACKET_put_bytes_u8(pkt, DER_P_OCTET_STRING)
        && int_end_context(pkt, tag);
}

int ossl_DER_w_octet_string_uint32(WPACKET *pkt, int tag, uint32_t value)
{
    unsigned char tmp[4] = { 0, 0, 0, 0 };
    unsigned char *pbuf = tmp + (sizeof(tmp) - 1);

    while (value > 0) {
        *pbuf-- = (value & 0xFF);
        value >>= 8;
    }
    return ossl_DER_w_octet_string(pkt, tag, tmp, sizeof(tmp));
}

static int int_der_w_integer(WPACKET *pkt, int tag,
                             int (*put_bytes)(WPACKET *pkt, const void *v,
                                              unsigned int *top_byte),
                             const void *v)
{
    unsigned int top_byte = 0;

    return int_start_context(pkt, tag)
        && WPACKET_start_sub_packet(pkt)
        && put_bytes(pkt, v, &top_byte)
        && ((top_byte & 0x80) == 0 || WPACKET_put_bytes_u8(pkt, 0))
        && WPACKET_close(pkt)
        && WPACKET_put_bytes_u8(pkt, DER_P_INTEGER)
        && int_end_context(pkt, tag);
}

static int int_put_bytes_uint32(WPACKET *pkt, const void *v,
                               unsigned int *top_byte)
{
    const uint32_t *value = v;
    uint32_t tmp = *value;
    size_t n = 0;

    while (tmp != 0) {
        n++;
        *top_byte = (tmp & 0xFF);
        tmp >>= 8;
    }
    if (n == 0)
        n = 1;

    return WPACKET_put_bytes__(pkt, *value, n);
}

/* For integers, we only support unsigned values for now */
int ossl_DER_w_uint32(WPACKET *pkt, int tag, uint32_t v)
{
    return int_der_w_integer(pkt, tag, int_put_bytes_uint32, &v);
}

static int int_put_bytes_bn(WPACKET *pkt, const void *v,
                            unsigned int *top_byte)
{
    unsigned char *p = NULL;
    size_t n = BN_num_bytes(v);

    /* The BIGNUM limbs are in LE order */
    *top_byte =
        ((bn_get_words(v) [(n - 1) / BN_BYTES]) >> (8 * ((n - 1) % BN_BYTES)))
        & 0xFF;

    if (!WPACKET_allocate_bytes(pkt, n, &p))
        return 0;
    if (p != NULL)
        BN_bn2bin(v, p);
    return 1;
}

int ossl_DER_w_bn(WPACKET *pkt, int tag, const BIGNUM *v)
{
    if (v == NULL || BN_is_negative(v))
        return 0;
    if (BN_is_zero(v))
        return ossl_DER_w_uint32(pkt, tag, 0);

    return int_der_w_integer(pkt, tag, int_put_bytes_bn, v);
}

int ossl_DER_w_null(WPACKET *pkt, int tag)
{
    return int_start_context(pkt, tag)
        && WPACKET_start_sub_packet(pkt)
        && WPACKET_close(pkt)
        && WPACKET_put_bytes_u8(pkt, DER_P_NULL)
        && int_end_context(pkt, tag);
}

/* Constructed things need a start and an end */
int ossl_DER_w_begin_sequence(WPACKET *pkt, int tag)
{
    return int_start_context(pkt, tag)
        && WPACKET_start_sub_packet(pkt);
}

int ossl_DER_w_end_sequence(WPACKET *pkt, int tag)
{
    /*
     * If someone set the flag WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH on this
     * sub-packet and this sub-packet has nothing written to it, the DER length
     * will not be written, and the total written size will be unchanged before
     * and after WPACKET_close().  We use size1 and size2 to determine if
     * anything was written, and only write our tag if it has.
     *
     * Because we know that int_end_context() needs to do the same check,
     * we reproduce this flag if the written length was unchanged, or we will
     * have an erroneous context tag.
     */
    size_t size1, size2;

    return WPACKET_get_total_written(pkt, &size1)
        && WPACKET_close(pkt)
        && WPACKET_get_total_written(pkt, &size2)
        && (size1 == size2
            ? WPACKET_set_flags(pkt, WPACKET_FLAGS_ABANDON_ON_ZERO_LENGTH)
            : WPACKET_put_bytes_u8(pkt, DER_F_CONSTRUCTED | DER_P_SEQUENCE))
        && int_end_context(pkt, tag);
}
