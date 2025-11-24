/*
 * Copyright 2023-2024 The OpenSSL Project Authors. All Rights Reserved.
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
#include "internal/quic_rcidm.h"
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
 *   Zero or more of:
 *     RESET_WITH_ODCID                 u8(0x00) u8(cidl):cid
 *     RESET_WITHOUT_ODCID              u8(0x01)
 *       (free and reallocate)
 *     ADD_FROM_INITIAL                 u8(0x02) u8(cidl):cid
 *     ADD_FROM_SERVER_RETRY            u8(0x03) u8(cidl):cid
 *     ADD_FROM_NCID                    u8(0x04) u64(seq_num)
 *                                        u64(retire_prior_to) u8(cidl):cid
 *     ON_HANDSHAKE_COMPLETE            u8(0x05)
 *     ON_PACKET_SENT                   u8(0x06) u64(num_pkt)
 *     REQUEST_ROLL                     u8(0x07)
 *     POP_RETIRE_SEQ_NUM               u8(0x08)
 *     PEEK_RETIRE_SEQ_NUM              u8(0x09)
 *     GET_PREFERRED_TX_DCID            u8(0x0A)
 *     GET_PREFERRED_TX_DCID_CHANGED    u8(0x0B) u8(clear)
 */

enum {
    CMD_RESET_WITH_ODCID,
    CMD_RESET_WITHOUT_ODCID,
    CMD_ADD_FROM_INITIAL,
    CMD_ADD_FROM_SERVER_RETRY,
    CMD_ADD_FROM_NCID,
    CMD_ON_HANDSHAKE_COMPLETE,
    CMD_ON_PACKET_SENT,
    CMD_REQUEST_ROLL,
    CMD_POP_RETIRE_SEQ_NUM,
    CMD_PEEK_RETIRE_SEQ_NUM,
    CMD_GET_PREFERRED_TX_DCID,
    CMD_GET_PREFERRED_TX_DCID_CHANGED
};

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
    QUIC_RCIDM *rcidm = NULL;
    PACKET pkt;
    uint64_t seq_num_out, arg_num_pkt;
    unsigned int cmd, arg_clear;
    QUIC_CONN_ID arg_cid, cid_out;
    OSSL_QUIC_FRAME_NEW_CONN_ID ncid_frame;

    if (!PACKET_buf_init(&pkt, buf, len))
        goto err;

    if ((rcidm = ossl_quic_rcidm_new(NULL)) == NULL)
        goto err;

    while (PACKET_remaining(&pkt) > 0) {
        if (!PACKET_get_1(&pkt, &cmd))
            goto err;

        switch (cmd) {
        case CMD_RESET_WITH_ODCID:
            if (!get_cid(&pkt, &arg_cid)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_free(rcidm);

            if ((rcidm = ossl_quic_rcidm_new(&arg_cid)) == NULL)
                goto err;

            break;

        case CMD_RESET_WITHOUT_ODCID:
            ossl_quic_rcidm_free(rcidm);

            if ((rcidm = ossl_quic_rcidm_new(NULL)) == NULL)
                goto err;

            break;

        case CMD_ADD_FROM_INITIAL:
            if (!get_cid(&pkt, &arg_cid)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_add_from_initial(rcidm, &arg_cid);
            break;

        case CMD_ADD_FROM_SERVER_RETRY:
            if (!get_cid(&pkt, &arg_cid)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_add_from_server_retry(rcidm, &arg_cid);
            break;

        case CMD_ADD_FROM_NCID:
            if (!PACKET_get_net_8(&pkt, &ncid_frame.seq_num)
                || !PACKET_get_net_8(&pkt, &ncid_frame.retire_prior_to)
                || !get_cid(&pkt, &ncid_frame.conn_id)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_add_from_ncid(rcidm, &ncid_frame);
            break;

        case CMD_ON_HANDSHAKE_COMPLETE:
            ossl_quic_rcidm_on_handshake_complete(rcidm);
            break;

        case CMD_ON_PACKET_SENT:
            if (!PACKET_get_net_8(&pkt, &arg_num_pkt)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_on_packet_sent(rcidm, arg_num_pkt);
            break;

        case CMD_REQUEST_ROLL:
            ossl_quic_rcidm_request_roll(rcidm);
            break;

        case CMD_POP_RETIRE_SEQ_NUM:
            ossl_quic_rcidm_pop_retire_seq_num(rcidm, &seq_num_out);
            break;

        case CMD_PEEK_RETIRE_SEQ_NUM:
            ossl_quic_rcidm_peek_retire_seq_num(rcidm, &seq_num_out);
            break;

        case CMD_GET_PREFERRED_TX_DCID:
            ossl_quic_rcidm_get_preferred_tx_dcid(rcidm, &cid_out);
            break;

        case CMD_GET_PREFERRED_TX_DCID_CHANGED:
            if (!PACKET_get_1(&pkt, &arg_clear)) {
                rc = -1;
                goto err;
            }

            ossl_quic_rcidm_get_preferred_tx_dcid_changed(rcidm, arg_clear);
            break;

        default:
            rc = -1;
            goto err;
        }
    }

err:
    ossl_quic_rcidm_free(rcidm);
    return rc;
}

void FuzzerCleanup(void)
{
    FuzzerClearRand();
}
