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
#include "internal/quic_lcidm.h"
#include "internal/packet.h"

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
 *   u8(LCID length)
 *   Zero or more of:
 *     ENROL_ODCID          u0(0x00) u64(opaque) u8(cidl):cid
 *     RETIRE_ODCID         u8(0x01) u64(opaque)
 *     GENERATE_INITIAL     u8(0x02) u64(opaque)
 *     GENERATE             u8(0x03) u64(opaque)
 *     RETIRE               u8(0x04) u64(opaque) u64(retire_prior_to)
 *     CULL                 u8(0x05) u64(opaque)
 *     LOOKUP               u8(0x06) u8(cidl):cid
 */

enum {
    CMD_ENROL_ODCID,
    CMD_RETIRE_ODCID,
    CMD_GENERATE_INITIAL,
    CMD_GENERATE,
    CMD_RETIRE,
    CMD_CULL,
    CMD_LOOKUP
};

#define MAX_CMDS    10000

static int get_cid(PACKET *pkt, QUIC_CONN_ID *cid)
{
    unsigned int cidl;

    if (!PACKET_get_1(pkt, &cidl)
        || cidl > QUIC_MAX_CONN_ID_LEN
        || !PACKET_copy_bytes(pkt, cid->id, cidl))
        return 0;

    cid->id_len = (unsigned char)cidl;
    return 1;
}

int FuzzerTestOneInput(const uint8_t *buf, size_t len)
{
    int rc = 0;
    QUIC_LCIDM *lcidm = NULL;
    PACKET pkt;
    uint64_t arg_opaque, arg_retire_prior_to, seq_num_out;
    unsigned int cmd, lcidl;
    QUIC_CONN_ID arg_cid, cid_out;
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid_frame;
    int did_retire;
    void *opaque_out;
    size_t limit = 0;

    if (!PACKET_buf_init(&pkt, buf, len))
        goto err;

    if (!PACKET_get_1(&pkt, &lcidl)
        || lcidl > QUIC_MAX_CONN_ID_LEN) {
        rc = -1;
        goto err;
    }

    if ((lcidm = ossl_quic_lcidm_new(NULL, lcidl)) == NULL) {
        rc = -1;
        goto err;
    }

    while (PACKET_remaining(&pkt) > 0) {
        if (!PACKET_get_1(&pkt, &cmd))
            goto err;

        if (++limit > MAX_CMDS)
            goto err;

        switch (cmd) {
        case CMD_ENROL_ODCID:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)
                || !get_cid(&pkt, &arg_cid)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_enrol_odcid(lcidm, (void *)(uintptr_t)arg_opaque,
                                        &arg_cid);
            break;

        case CMD_RETIRE_ODCID:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_retire_odcid(lcidm, (void *)(uintptr_t)arg_opaque);
            break;

        case CMD_GENERATE_INITIAL:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_generate_initial(lcidm, (void *)(uintptr_t)arg_opaque,
                                             &cid_out);
            break;

        case CMD_GENERATE:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_generate(lcidm, (void *)(uintptr_t)arg_opaque,
                                     &ncid_frame);
            break;

        case CMD_RETIRE:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)
                || !PACKET_get_net_8(&pkt, &arg_retire_prior_to)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_retire(lcidm, (void *)(uintptr_t)arg_opaque,
                                   arg_retire_prior_to,
                                   NULL, &cid_out,
                                   &seq_num_out, &did_retire);
            break;

        case CMD_CULL:
            if (!PACKET_get_net_8(&pkt, &arg_opaque)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_cull(lcidm, (void *)(uintptr_t)arg_opaque);
            break;

        case CMD_LOOKUP:
            if (!get_cid(&pkt, &arg_cid)) {
                rc = -1;
                goto err;
            }

            ossl_quic_lcidm_lookup(lcidm, &arg_cid, &seq_num_out, &opaque_out);
            break;

        default:
            rc = -1;
            goto err;
        }
    }

err:
    ossl_quic_lcidm_free(lcidm);
    return rc;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
