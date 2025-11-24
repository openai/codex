/*
 * Copyright 2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include "internal/packet.h"
#include "internal/quic_txpim.h"
#include "testutil.h"

static int test_txpim(void)
{
    int testresult = 0;
    QUIC_TXPIM *txpim;
    size_t i, j;
    QUIC_TXPIM_PKT *pkts[10] = {NULL};
    QUIC_TXPIM_CHUNK chunks[3];
    const QUIC_TXPIM_CHUNK *rchunks;

    if (!TEST_ptr(txpim = ossl_quic_txpim_new()))
        goto err;

    for (i = 0; i < OSSL_NELEM(pkts); ++i) {
        if (!TEST_ptr(pkts[i] = ossl_quic_txpim_pkt_alloc(txpim)))
            goto err;

        if (!TEST_size_t_eq(ossl_quic_txpim_pkt_get_num_chunks(pkts[i]), 0))
            goto err;

        for (j = 0; j < OSSL_NELEM(chunks); ++j) {
            chunks[j].stream_id = 100 - j;
            chunks[j].start     = 1000 * i + j * 10;
            chunks[j].end       = chunks[j].start + 5;

            if (!TEST_true(ossl_quic_txpim_pkt_append_chunk(pkts[i], chunks + j)))
                goto err;
        }

        if (!TEST_size_t_eq(ossl_quic_txpim_pkt_get_num_chunks(pkts[i]),
                            OSSL_NELEM(chunks)))
            goto err;

        rchunks = ossl_quic_txpim_pkt_get_chunks(pkts[i]);
        if (!TEST_uint64_t_eq(rchunks[0].stream_id, 98)
            || !TEST_uint64_t_eq(rchunks[1].stream_id, 99)
            || !TEST_uint64_t_eq(rchunks[2].stream_id, 100))
            goto err;
    }

    testresult = 1;
err:
    for (i = 0; i < OSSL_NELEM(pkts); ++i)
        if (txpim != NULL && pkts[i] != NULL)
            ossl_quic_txpim_pkt_release(txpim, pkts[i]);

    ossl_quic_txpim_free(txpim);
    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_txpim);
    return 1;
}
