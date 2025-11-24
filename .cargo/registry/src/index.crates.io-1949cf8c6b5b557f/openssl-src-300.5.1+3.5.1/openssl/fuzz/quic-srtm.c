/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 * https://www.openssl.org/source/license.html
 * or in the file LICENSE in the source distribution.
 */

#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/bio.h>
#include "fuzzer.h"
#include "internal/quic_srtm.h"

int FuzzerInitialize(int *argc, char ***argv)
{
    FuzzerSetRand();
    OPENSSL_init_crypto(OPENSSL_INIT_LOAD_CRYPTO_STRINGS | OPENSSL_INIT_ASYNC, NULL);
    OPENSSL_init_ssl(OPENSSL_INIT_LOAD_SSL_STRINGS, NULL);
    ERR_clear_error();
    return 1;
}

/*
 * Fuzzer input "protocol":
 *   Big endian
 *   Zero or more of:
 *     ADD    - u8(0x00) u64(opaque) u64(seq_num) u128(token)
 *     REMOVE - u8(0x01) u64(opaque) u64(seq_num)
 *     CULL   - u8(0x02) u64(opaque)
 *     LOOKUP - u8(0x03) u128(token) u64(idx)
 */
enum {
    CMD_ADD,
    CMD_REMOVE,
    CMD_CULL,
    CMD_LOOKUP,
    CMD_MAX
};

#define MAX_CMDS 10000

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    int rc = 0;
    QUIC_SRTM *srtm = NULL;
    PACKET pkt;
    unsigned int cmd;
    uint64_t arg_opaque, arg_seq_num, arg_idx;
    QUIC_STATELESS_RESET_TOKEN arg_token;
    size_t limit = 0;

    if ((srtm = ossl_quic_srtm_new(NULL, NULL)) == NULL) {
        rc = -1;
        goto err;
    }

    if (!PACKET_buf_init(&pkt, buf, len))
        goto err;

    while (PACKET_remaining(&pkt) > 0) {
        if (!PACKET_get_1(&pkt, &cmd))
            goto err;

        if (++limit > MAX_CMDS) {
            rc = 0;
            goto err;
        }

        switch (cmd % CMD_MAX) {
        case CMD_ADD:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)
                || !PACKET_get_net_8(&pkt, &arg_seq_num)
                || !PACKET_copy_bytes(&pkt, arg_token.token,
                                      sizeof(arg_token.token)))
                continue; /* just stop */

            ossl_quic_srtm_add(srtm, (void *)(uintptr_t)arg_opaque,
                               arg_seq_num, &arg_token);
            ossl_quic_srtm_check(srtm);
            break;

        case CMD_REMOVE:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)
                || !PACKET_get_net_8(&pkt, &arg_seq_num))
                continue; /* just stop */

            ossl_quic_srtm_remove(srtm, (void *)(uintptr_t)arg_opaque,
                                  arg_seq_num);
            ossl_quic_srtm_check(srtm);
            break;

        case CMD_CULL:
            if (!PACKET_get_net_8(&pkt, &arg_opaque))
                continue; /* just stop */

            ossl_quic_srtm_cull(srtm, (void *)(uintptr_t)arg_opaque);
            ossl_quic_srtm_check(srtm);
            break;

        case CMD_LOOKUP:
            if (!PACKET_copy_bytes(&pkt, arg_token.token,
                                   sizeof(arg_token.token))
                || !PACKET_get_net_8(&pkt, &arg_idx))
                continue; /* just stop */

            ossl_quic_srtm_lookup(srtm, &arg_token, (size_t)arg_idx,
                                  NULL, NULL);
            ossl_quic_srtm_check(srtm);
            break;

        default:
            /* Other bytes are treated as no-ops */
            continue;
        }
    }

    rc = 0;
err:
    ossl_quic_srtm_free(srtm);
    return rc;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
