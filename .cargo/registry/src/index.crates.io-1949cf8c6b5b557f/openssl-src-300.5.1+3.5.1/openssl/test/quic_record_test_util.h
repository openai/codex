/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_RECORD_TEST_UTIL_H
# define OSSL_RECORD_TEST_UTIL_H

static int cmp_pkt_hdr(const QUIC_PKT_HDR *a, const QUIC_PKT_HDR *b,
                       const unsigned char *b_data, size_t b_len,
                       int cmp_data)
{
    int ok = 1;

    if (b_data == NULL) {
        b_data = b->data;
        b_len  = b->len;
    }

    if (!TEST_int_eq(a->type, b->type)
        || !TEST_int_eq(a->spin_bit, b->spin_bit)
        || !TEST_int_eq(a->key_phase, b->key_phase)
        || !TEST_int_eq(a->pn_len, b->pn_len)
        || !TEST_int_eq(a->partial, b->partial)
        || !TEST_int_eq(a->fixed, b->fixed)
        || !TEST_int_eq(a->unused, b->unused)
        || !TEST_int_eq(a->reserved, b->reserved)
        || !TEST_uint_eq(a->version, b->version)
        || !TEST_true(ossl_quic_conn_id_eq(&a->dst_conn_id, &b->dst_conn_id))
        || !TEST_true(ossl_quic_conn_id_eq(&a->src_conn_id, &b->src_conn_id))
        || !TEST_mem_eq(a->pn, sizeof(a->pn), b->pn, sizeof(b->pn))
        || !TEST_size_t_eq(a->token_len, b->token_len)
        || !TEST_uint64_t_eq(a->len, b->len))
        ok = 0;

    if (a->token_len > 0 && b->token_len > 0
        && !TEST_mem_eq(a->token, a->token_len, b->token, b->token_len))
        ok = 0;

    if ((a->token_len == 0 && !TEST_ptr_null(a->token))
        || (b->token_len == 0 && !TEST_ptr_null(b->token)))
        ok = 0;

    if (cmp_data && !TEST_mem_eq(a->data, a->len, b_data, b_len))
        ok = 0;

    return ok;
}

#endif
