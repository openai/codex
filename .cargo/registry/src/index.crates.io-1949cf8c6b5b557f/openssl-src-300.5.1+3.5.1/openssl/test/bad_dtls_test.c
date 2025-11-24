/*
 * Copyright 2016-2024 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Unit test for Cisco DTLS1_BAD_VER session resume, as used by
 * AnyConnect VPN protocol.
 *
 * This is designed to exercise the code paths in
 * http://git.infradead.org/users/dwmw2/openconnect.git/blob/HEAD:/dtls.c
 * which have frequently been affected by regressions in DTLS1_BAD_VER
 * support.
 *
 * Note that unlike other SSL tests, we don't test against our own SSL
 * server method. Firstly because we don't have one; we *only* support
 * DTLS1_BAD_VER as a client. And secondly because even if that were
 * fixed up it's the wrong thing to test against - because if changes
 * are made in generic DTLS code which don't take DTLS1_BAD_VER into
 * account, there's plenty of scope for making those changes such that
 * they break *both* the client and the server in the same way.
 *
 * So we handle the server side manually. In a session resume there isn't
 * much to be done anyway.
 */
#include <string.h>

#include <openssl/core_names.h>
#include <openssl/params.h>
#include <openssl/opensslconf.h>
#include <openssl/bio.h>
#include <openssl/crypto.h>
#include <openssl/evp.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/rand.h>
#include <openssl/kdf.h>
#include "internal/packet.h"
#include "internal/nelem.h"
#include "testutil.h"

/* For DTLS1_BAD_VER packets the MAC doesn't include the handshake header */
#define MAC_OFFSET (DTLS1_RT_HEADER_LENGTH + DTLS1_HM_HEADER_LENGTH)

static unsigned char client_random[SSL3_RANDOM_SIZE];
static unsigned char server_random[SSL3_RANDOM_SIZE];

/* These are all generated locally, sized purely according to our own whim */
static unsigned char session_id[32];
static unsigned char master_secret[48];
static unsigned char cookie[20];

/* We've hard-coded the cipher suite; we know it's 104 bytes */
static unsigned char key_block[104];
#define mac_key (key_block + 20)
#define dec_key (key_block + 40)
#define enc_key (key_block + 56)

static EVP_MD_CTX *handshake_md;

static int do_PRF(const void *seed1, int seed1_len,
                  const void *seed2, int seed2_len,
                  const void *seed3, int seed3_len,
                  unsigned char *out, int olen)
{
    EVP_PKEY_CTX *pctx = EVP_PKEY_CTX_new_id(EVP_PKEY_TLS1_PRF, NULL);
    size_t outlen = olen;

    /* No error handling. If it all screws up, the test will fail anyway */
    EVP_PKEY_derive_init(pctx);
    EVP_PKEY_CTX_set_tls1_prf_md(pctx, EVP_md5_sha1());
    EVP_PKEY_CTX_set1_tls1_prf_secret(pctx, master_secret, sizeof(master_secret));
    EVP_PKEY_CTX_add1_tls1_prf_seed(pctx, seed1, seed1_len);
    EVP_PKEY_CTX_add1_tls1_prf_seed(pctx, seed2, seed2_len);
    EVP_PKEY_CTX_add1_tls1_prf_seed(pctx, seed3, seed3_len);
    EVP_PKEY_derive(pctx, out, &outlen);
    EVP_PKEY_CTX_free(pctx);
    return 1;
}

static SSL_SESSION *client_session(void)
{
    static unsigned char session_asn1[] = {
        0x30, 0x5F,              /* SEQUENCE, length 0x5F */
        0x02, 0x01, 0x01,        /* INTEGER, SSL_SESSION_ASN1_VERSION */
        0x02, 0x02, 0x01, 0x00,  /* INTEGER, DTLS1_BAD_VER */
        0x04, 0x02, 0x00, 0x2F,  /* OCTET_STRING, AES128-SHA */
        0x04, 0x20,              /* OCTET_STRING, session id */
#define SS_SESSID_OFS 15 /* Session ID goes here */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x04, 0x30,              /* OCTET_STRING, master secret */
#define SS_SECRET_OFS 49 /* Master secret goes here */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    };
    const unsigned char *p = session_asn1;

    /* Copy the randomly-generated fields into the above ASN1 */
    memcpy(session_asn1 + SS_SESSID_OFS, session_id, sizeof(session_id));
    memcpy(session_asn1 + SS_SECRET_OFS, master_secret, sizeof(master_secret));

    return d2i_SSL_SESSION(NULL, &p, sizeof(session_asn1));
}

/* Returns 1 for initial ClientHello, 2 for ClientHello with cookie */
static int validate_client_hello(BIO *wbio)
{
    PACKET pkt, pkt2;
    long len;
    unsigned char *data;
    int cookie_found = 0;
    unsigned int u = 0;

    if ((len = BIO_get_mem_data(wbio, (char **)&data)) < 0)
        return 0;
    if (!PACKET_buf_init(&pkt, data, len))
        return 0;

    /* Check record header type */
    if (!PACKET_get_1(&pkt, &u) || u != SSL3_RT_HANDSHAKE)
        return 0;
    /* Version */
    if (!PACKET_get_net_2(&pkt, &u) || u != DTLS1_BAD_VER)
        return 0;
    /* Skip the rest of the record header */
    if (!PACKET_forward(&pkt, DTLS1_RT_HEADER_LENGTH - 3))
        return 0;

    /* Check it's a ClientHello */
    if (!PACKET_get_1(&pkt, &u) || u != SSL3_MT_CLIENT_HELLO)
        return 0;
    /* Skip the rest of the handshake message header */
    if (!PACKET_forward(&pkt, DTLS1_HM_HEADER_LENGTH - 1))
        return 0;

    /* Check client version */
    if (!PACKET_get_net_2(&pkt, &u) || u != DTLS1_BAD_VER)
        return 0;

    /* Store random */
    if (!PACKET_copy_bytes(&pkt, client_random, SSL3_RANDOM_SIZE))
        return 0;

    /* Check session id length and content */
    if (!PACKET_get_length_prefixed_1(&pkt, &pkt2) ||
        !PACKET_equal(&pkt2, session_id, sizeof(session_id)))
        return 0;

    /* Check cookie */
    if (!PACKET_get_length_prefixed_1(&pkt, &pkt2))
        return 0;
    if (PACKET_remaining(&pkt2)) {
        if (!PACKET_equal(&pkt2, cookie, sizeof(cookie)))
            return 0;
        cookie_found = 1;
    }

    /* Skip ciphers */
    if (!PACKET_get_net_2(&pkt, &u) || !PACKET_forward(&pkt, u))
        return 0;

    /* Skip compression */
    if (!PACKET_get_1(&pkt, &u) || !PACKET_forward(&pkt, u))
        return 0;

    /* Skip extensions */
    if (!PACKET_get_net_2(&pkt, &u) || !PACKET_forward(&pkt, u))
        return 0;

    /* Now we are at the end */
    if (PACKET_remaining(&pkt))
        return 0;

    /* Update handshake MAC for second ClientHello (with cookie) */
    if (cookie_found && !EVP_DigestUpdate(handshake_md, data + MAC_OFFSET,
                                          len - MAC_OFFSET))
        return 0;

    (void)BIO_reset(wbio);

    return 1 + cookie_found;
}

static int send_hello_verify(BIO *rbio)
{
    static unsigned char hello_verify[] = {
        0x16, /* Handshake */
        0x01, 0x00, /* DTLS1_BAD_VER */
        0x00, 0x00, /* Epoch 0 */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, /* Seq# 0 */
        0x00, 0x23, /* Length */
        0x03, /* Hello Verify */
        0x00, 0x00, 0x17, /* Length */
        0x00, 0x00, /* Seq# 0 */
        0x00, 0x00, 0x00, /* Fragment offset */
        0x00, 0x00, 0x17, /* Fragment length */
        0x01, 0x00, /* DTLS1_BAD_VER */
        0x14, /* Cookie length */
#define HV_COOKIE_OFS 28 /* Cookie goes here */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    };

    memcpy(hello_verify + HV_COOKIE_OFS, cookie, sizeof(cookie));

    BIO_write(rbio, hello_verify, sizeof(hello_verify));

    return 1;
}

static int send_server_hello(BIO *rbio)
{
    static unsigned char server_hello[] = {
        0x16, /* Handshake */
        0x01, 0x00, /* DTLS1_BAD_VER */
        0x00, 0x00, /* Epoch 0 */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x01, /* Seq# 1 */
        0x00, 0x52, /* Length */
        0x02, /* Server Hello */
        0x00, 0x00, 0x46, /* Length */
        0x00, 0x01, /* Seq# */
        0x00, 0x00, 0x00, /* Fragment offset */
        0x00, 0x00, 0x46, /* Fragment length */
        0x01, 0x00, /* DTLS1_BAD_VER */
#define SH_RANDOM_OFS 27 /* Server random goes here */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x20, /* Session ID length */
#define SH_SESSID_OFS 60 /* Session ID goes here */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x2f, /* Cipher suite AES128-SHA */
        0x00, /* Compression null */
    };
    static unsigned char change_cipher_spec[] = {
        0x14, /* Change Cipher Spec */
        0x01, 0x00, /* DTLS1_BAD_VER */
        0x00, 0x00, /* Epoch 0 */
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, /* Seq# 2 */
        0x00, 0x03, /* Length */
        0x01, 0x00, 0x02, /* Message */
    };

    memcpy(server_hello + SH_RANDOM_OFS, server_random, sizeof(server_random));
    memcpy(server_hello + SH_SESSID_OFS, session_id, sizeof(session_id));

    if (!EVP_DigestUpdate(handshake_md, server_hello + MAC_OFFSET,
                          sizeof(server_hello) - MAC_OFFSET))
        return 0;

    BIO_write(rbio, server_hello, sizeof(server_hello));
    BIO_write(rbio, change_cipher_spec, sizeof(change_cipher_spec));

    return 1;
}

/* Create header, HMAC, pad, encrypt and send a record */
static int send_record(BIO *rbio, unsigned char type, uint64_t seqnr,
                       const void *msg, size_t len)
{
    /* Note that the order of the record header fields on the wire,
     * and in the HMAC, is different. So we just keep them in separate
     * variables and handle them individually. */
    static unsigned char epoch[2] = { 0x00, 0x01 };
    static unsigned char seq[6] = { 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 };
    static unsigned char ver[2] = { 0x01, 0x00 }; /* DTLS1_BAD_VER */
    unsigned char lenbytes[2];
    EVP_MAC *hmac = NULL;
    EVP_MAC_CTX *ctx = NULL;
    EVP_CIPHER_CTX *enc_ctx = NULL;
    unsigned char iv[16];
    unsigned char pad;
    unsigned char *enc;
    OSSL_PARAM params[2];
    int ret = 0;

    seq[0] = (seqnr >> 40) & 0xff;
    seq[1] = (seqnr >> 32) & 0xff;
    seq[2] = (seqnr >> 24) & 0xff;
    seq[3] = (seqnr >> 16) & 0xff;
    seq[4] = (seqnr >> 8) & 0xff;
    seq[5] = seqnr & 0xff;

    pad = 15 - ((len + SHA_DIGEST_LENGTH) % 16);
    enc = OPENSSL_malloc(len + SHA_DIGEST_LENGTH + 1 + pad);
    if (enc == NULL)
        return 0;

    /* Copy record to encryption buffer */
    memcpy(enc, msg, len);

    /* Append HMAC to data */
    if (!TEST_ptr(hmac = EVP_MAC_fetch(NULL, "HMAC", NULL))
            || !TEST_ptr(ctx = EVP_MAC_CTX_new(hmac)))
        goto end;
    params[0] = OSSL_PARAM_construct_utf8_string(OSSL_MAC_PARAM_DIGEST,
                                                 "SHA1", 0);
    params[1] = OSSL_PARAM_construct_end();
    lenbytes[0] = (unsigned char)(len >> 8);
    lenbytes[1] = (unsigned char)(len);
    if (!EVP_MAC_init(ctx, mac_key, 20, params)
            || !EVP_MAC_update(ctx, epoch, 2)
            || !EVP_MAC_update(ctx, seq, 6)
            || !EVP_MAC_update(ctx, &type, 1)
            || !EVP_MAC_update(ctx, ver, 2)      /* Version */
            || !EVP_MAC_update(ctx, lenbytes, 2) /* Length */
            || !EVP_MAC_update(ctx, enc, len)    /* Finally the data itself */
            || !EVP_MAC_final(ctx, enc + len, NULL, SHA_DIGEST_LENGTH))
        goto end;

    /* Append padding bytes */
    len += SHA_DIGEST_LENGTH;
    do {
        enc[len++] = pad;
    } while (len % 16);

    /* Generate IV, and encrypt */
    if (!TEST_int_gt(RAND_bytes(iv, sizeof(iv)), 0)
            || !TEST_ptr(enc_ctx = EVP_CIPHER_CTX_new())
            || !TEST_true(EVP_CipherInit_ex(enc_ctx, EVP_aes_128_cbc(), NULL,
                                            enc_key, iv, 1))
            || !TEST_int_ge(EVP_Cipher(enc_ctx, enc, enc, len), 0))
        goto end;

    /* Finally write header (from fragmented variables), IV and encrypted record */
    BIO_write(rbio, &type, 1);
    BIO_write(rbio, ver, 2);
    BIO_write(rbio, epoch, 2);
    BIO_write(rbio, seq, 6);
    lenbytes[0] = (unsigned char)((len + sizeof(iv)) >> 8);
    lenbytes[1] = (unsigned char)(len + sizeof(iv));
    BIO_write(rbio, lenbytes, 2);

    BIO_write(rbio, iv, sizeof(iv));
    BIO_write(rbio, enc, len);
    ret = 1;
 end:
    EVP_MAC_free(hmac);
    EVP_MAC_CTX_free(ctx);
    EVP_CIPHER_CTX_free(enc_ctx);
    OPENSSL_free(enc);
    return ret;
}

static int send_finished(SSL *s, BIO *rbio)
{
    static unsigned char finished_msg[DTLS1_HM_HEADER_LENGTH +
                                      TLS1_FINISH_MAC_LENGTH] = {
        0x14, /* Finished */
        0x00, 0x00, 0x0c, /* Length */
        0x00, 0x03, /* Seq# 3 */
        0x00, 0x00, 0x00, /* Fragment offset */
        0x00, 0x00, 0x0c, /* Fragment length */
        /* Finished MAC (12 bytes) */
    };
    unsigned char handshake_hash[EVP_MAX_MD_SIZE];
    int md_size;

    /* Derive key material */
    do_PRF(TLS_MD_KEY_EXPANSION_CONST, TLS_MD_KEY_EXPANSION_CONST_SIZE,
           server_random, SSL3_RANDOM_SIZE,
           client_random, SSL3_RANDOM_SIZE,
           key_block, sizeof(key_block));

    /* Generate Finished MAC */
    if (!EVP_DigestFinal_ex(handshake_md, handshake_hash, NULL))
        return 0;

    md_size = EVP_MD_CTX_get_size(handshake_md);
    if (md_size <= 0)
        return 0;
    do_PRF(TLS_MD_SERVER_FINISH_CONST, TLS_MD_SERVER_FINISH_CONST_SIZE,
           handshake_hash, md_size,
           NULL, 0,
           finished_msg + DTLS1_HM_HEADER_LENGTH, TLS1_FINISH_MAC_LENGTH);

    return send_record(rbio, SSL3_RT_HANDSHAKE, 0,
                       finished_msg, sizeof(finished_msg));
}

static int validate_ccs(BIO *wbio)
{
    PACKET pkt;
    long len;
    unsigned char *data;
    unsigned int u;

    len = BIO_get_mem_data(wbio, (char **)&data);
    if (len < 0)
        return 0;

    if (!PACKET_buf_init(&pkt, data, len))
        return 0;

    /* Check record header type */
    if (!PACKET_get_1(&pkt, &u) || u != SSL3_RT_CHANGE_CIPHER_SPEC)
        return 0;
    /* Version */
    if (!PACKET_get_net_2(&pkt, &u) || u != DTLS1_BAD_VER)
        return 0;
    /* Skip the rest of the record header */
    if (!PACKET_forward(&pkt, DTLS1_RT_HEADER_LENGTH - 3))
        return 0;

    /* Check ChangeCipherSpec message */
    if (!PACKET_get_1(&pkt, &u) || u != SSL3_MT_CCS)
        return 0;
    /* A DTLS1_BAD_VER ChangeCipherSpec also contains the
     * handshake sequence number (which is 2 here) */
    if (!PACKET_get_net_2(&pkt, &u) || u != 0x0002)
        return 0;

    /* Now check the Finished packet */
    if (!PACKET_get_1(&pkt, &u) || u != SSL3_RT_HANDSHAKE)
        return 0;
    if (!PACKET_get_net_2(&pkt, &u) || u != DTLS1_BAD_VER)
        return 0;

    /* Check epoch is now 1 */
    if (!PACKET_get_net_2(&pkt, &u) || u != 0x0001)
        return 0;

    /* That'll do for now. If OpenSSL accepted *our* Finished packet
     * then it's evidently remembered that DTLS1_BAD_VER doesn't
     * include the handshake header in the MAC. There's not a lot of
     * point in implementing decryption here, just to check that it
     * continues to get it right for one more packet. */

    return 1;
}

#define NODROP(x) { x##UL, 0 }
#define DROP(x)   { x##UL, 1 }

static struct {
    uint64_t seq;
    int drop;
} tests[] = {
    NODROP(1), NODROP(3), NODROP(2),
    NODROP(0x1234), NODROP(0x1230), NODROP(0x1235),
    NODROP(0xffff), NODROP(0x10001), NODROP(0xfffe), NODROP(0x10000),
    DROP(0x10001), DROP(0xff), NODROP(0x100000), NODROP(0x800000), NODROP(0x7fffe1),
    NODROP(0xffffff), NODROP(0x1000000), NODROP(0xfffffe), DROP(0xffffff), NODROP(0x1000010),
    NODROP(0xfffffd), NODROP(0x1000011), DROP(0x12), NODROP(0x1000012),
    NODROP(0x1ffffff), NODROP(0x2000000), DROP(0x1ff00fe), NODROP(0x2000001),
    NODROP(0x20fffff), NODROP(0x2105500), DROP(0x20ffffe), NODROP(0x21054ff),
    NODROP(0x211ffff), DROP(0x2110000), NODROP(0x2120000)
    /* The last test should be NODROP, because a DROP wouldn't get tested. */
};

static int test_bad_dtls(void)
{
    SSL_SESSION *sess = NULL;
    SSL_CTX *ctx = NULL;
    SSL *con = NULL;
    BIO *rbio = NULL;
    BIO *wbio = NULL;
    time_t now = 0;
    int testresult = 0;
    int ret;
    int i;

    RAND_bytes(session_id, sizeof(session_id));
    RAND_bytes(master_secret, sizeof(master_secret));
    RAND_bytes(cookie, sizeof(cookie));
    RAND_bytes(server_random + 4, sizeof(server_random) - 4);

    now = time(NULL);
    memcpy(server_random, &now, sizeof(now));

    sess = client_session();
    if (!TEST_ptr(sess))
        goto end;

    handshake_md = EVP_MD_CTX_new();
    if (!TEST_ptr(handshake_md)
            || !TEST_true(EVP_DigestInit_ex(handshake_md, EVP_md5_sha1(),
                                            NULL)))
        goto end;

    ctx = SSL_CTX_new(DTLS_client_method());
    if (!TEST_ptr(ctx)
            || !TEST_true(SSL_CTX_set_min_proto_version(ctx, DTLS1_BAD_VER))
            || !TEST_true(SSL_CTX_set_max_proto_version(ctx, DTLS1_BAD_VER))
            || !TEST_true(SSL_CTX_set_options(ctx,
                                              SSL_OP_LEGACY_SERVER_CONNECT))
            || !TEST_true(SSL_CTX_set_cipher_list(ctx, "AES128-SHA")))
        goto end;

    SSL_CTX_set_security_level(ctx, 0);
    con = SSL_new(ctx);
    if (!TEST_ptr(con)
            || !TEST_true(SSL_set_session(con, sess)))
        goto end;

    rbio = BIO_new(BIO_s_mem());
    wbio = BIO_new(BIO_s_mem());

    if (!TEST_ptr(rbio)
            || !TEST_ptr(wbio))
        goto end;

    SSL_set_bio(con, rbio, wbio);

    if (!TEST_true(BIO_up_ref(rbio))) {
        /*
         * We can't up-ref but we assigned ownership to con, so we shouldn't
         * free in the "end" block
         */
        rbio = wbio = NULL;
        goto end;
    }

    if (!TEST_true(BIO_up_ref(wbio))) {
        wbio = NULL;
        goto end;
    }

    SSL_set_connect_state(con);

    /* Send initial ClientHello */
    ret = SSL_do_handshake(con);
    if (!TEST_int_le(ret, 0)
            || !TEST_int_eq(SSL_get_error(con, ret), SSL_ERROR_WANT_READ)
            || !TEST_int_eq(validate_client_hello(wbio), 1)
            || !TEST_true(send_hello_verify(rbio)))
        goto end;

    ret = SSL_do_handshake(con);
    if (!TEST_int_le(ret, 0)
            || !TEST_int_eq(SSL_get_error(con, ret), SSL_ERROR_WANT_READ)
            || !TEST_int_eq(validate_client_hello(wbio), 2)
            || !TEST_true(send_server_hello(rbio)))
        goto end;

    ret = SSL_do_handshake(con);
    if (!TEST_int_le(ret, 0)
            || !TEST_int_eq(SSL_get_error(con, ret), SSL_ERROR_WANT_READ)
            || !TEST_true(send_finished(con, rbio)))
        goto end;

    ret = SSL_do_handshake(con);
    if (!TEST_int_gt(ret, 0)
            || !TEST_true(validate_ccs(wbio)))
        goto end;

    /* While we're here and crafting packets by hand, we might as well do a
       bit of a stress test on the DTLS record replay handling. Not Cisco-DTLS
       specific but useful anyway for the general case. It's been broken
       before, and in fact was broken even for a basic 0, 2, 1 test case
       when this test was first added.... */
    for (i = 0; i < (int)OSSL_NELEM(tests); i++) {
        uint64_t recv_buf[2];

        if (!TEST_true(send_record(rbio, SSL3_RT_APPLICATION_DATA, tests[i].seq,
                                   &tests[i].seq, sizeof(uint64_t)))) {
            TEST_error("Failed to send data seq #0x%x%08x (%d)\n",
                       (unsigned int)(tests[i].seq >> 32), (unsigned int)tests[i].seq, i);
            goto end;
        }

        if (tests[i].drop)
            continue;

        ret = SSL_read(con, recv_buf, 2 * sizeof(uint64_t));
        if (!TEST_int_eq(ret, (int)sizeof(uint64_t))) {
            TEST_error("SSL_read failed or wrong size on seq#0x%x%08x (%d)\n",
                       (unsigned int)(tests[i].seq >> 32), (unsigned int)tests[i].seq, i);
            goto end;
        }
        if (!TEST_true(recv_buf[0] == tests[i].seq))
            goto end;
    }

    /* The last test cannot be DROP() */
    if (!TEST_false(tests[i-1].drop))
        goto end;

    testresult = 1;

 end:
    SSL_SESSION_free(sess);
    BIO_free(rbio);
    BIO_free(wbio);
    SSL_free(con);
    SSL_CTX_free(ctx);
    EVP_MD_CTX_free(handshake_md);

    return testresult;
}

int setup_tests(void)
{
    ADD_TEST(test_bad_dtls);
    return 1;
}
