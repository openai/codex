/*
 * Copyright 2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */
#include "internal/sockets.h"
#include <openssl/rand.h>

static const unsigned char alpn_ossltest[] = {
    /* "\x08ossltest" (hex for EBCDIC resilience) */
    0x08, 0x6f, 0x73, 0x73, 0x6c, 0x74, 0x65, 0x73, 0x74
};

DEF_FUNC(hf_unbind)
{
    int ok = 0;
    const char *name;

    F_POP(name);
    RADIX_PROCESS_set_obj(RP(), name, NULL);

    ok = 1;
err:
    return ok;
}

static int ssl_ctx_select_alpn(SSL *ssl,
                               const unsigned char **out, unsigned char *out_len,
                               const unsigned char *in, unsigned int in_len,
                               void *arg)
{
    if (SSL_select_next_proto((unsigned char **)out, out_len,
                              alpn_ossltest, sizeof(alpn_ossltest), in, in_len)
            != OPENSSL_NPN_NEGOTIATED)
        return SSL_TLSEXT_ERR_ALERT_FATAL;

    return SSL_TLSEXT_ERR_OK;
}

static void keylog_cb(const SSL *ssl, const char *line)
{
    ossl_crypto_mutex_lock(RP()->gm);
    BIO_printf(RP()->keylog_out, "%s", line);
    (void)BIO_flush(RP()->keylog_out);
    ossl_crypto_mutex_unlock(RP()->gm);
}

static int ssl_ctx_configure(SSL_CTX *ctx, int is_server)
{
    if (!TEST_true(ossl_quic_set_diag_title(ctx, "quic_radix_test")))
        return 0;

    if (!is_server)
        return 1;

    if (RP()->keylog_out != NULL)
        SSL_CTX_set_keylog_callback(ctx, keylog_cb);

    if (!TEST_int_eq(SSL_CTX_use_certificate_file(ctx, cert_file,
                                                  SSL_FILETYPE_PEM), 1)
        || !TEST_int_eq(SSL_CTX_use_PrivateKey_file(ctx, key_file,
                                                    SSL_FILETYPE_PEM), 1))
        return 0;

    SSL_CTX_set_alpn_select_cb(ctx, ssl_ctx_select_alpn, NULL);
    return 1;
}

static int ssl_create_bound_socket(uint16_t listen_port,
                                   int *p_fd, uint16_t *p_result_port)
{
    int ok = 0;
    int fd = -1;
    BIO_ADDR *addr = NULL;
    union BIO_sock_info_u info;
    struct in_addr ina;

    ina.s_addr = htonl(INADDR_LOOPBACK);

    fd = BIO_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP, 0);
    if (!TEST_int_ge(fd, 0))
        goto err;

    if (!TEST_true(BIO_socket_nbio(fd, 1)))
        goto err;

    if (!TEST_ptr(addr = BIO_ADDR_new()))
        goto err;

    if (!TEST_true(BIO_ADDR_rawmake(addr, AF_INET,
                                    &ina, sizeof(ina), 0)))
        goto err;

    if (!TEST_true(BIO_bind(fd, addr, 0)))
        goto err;

    info.addr = addr;
    if (!TEST_true(BIO_sock_info(fd, BIO_SOCK_INFO_ADDRESS, &info)))
        goto err;

    if (!TEST_int_gt(BIO_ADDR_rawport(addr), 0))
        goto err;

    ok = 1;
err:
    if (!ok && fd >= 0)
        BIO_closesocket(fd);
    else if (ok) {
        *p_fd = fd;
        if (p_result_port != NULL)
            *p_result_port = BIO_ADDR_rawport(addr);
    }
    BIO_ADDR_free(addr);
    return ok;
}

static int ssl_attach_bio_dgram(SSL *ssl,
                                uint16_t local_port, uint16_t *actual_port)
{
    int s_fd = -1;
    BIO *bio;

    if (!TEST_true(ssl_create_bound_socket(local_port, &s_fd, actual_port)))
        return 0;

    if (!TEST_ptr(bio = BIO_new_dgram(s_fd, BIO_CLOSE))) {
        BIO_closesocket(s_fd);
        return 0;
    }

    SSL_set0_rbio(ssl, bio);
    if (!TEST_true(BIO_up_ref(bio)))
        return 0;

    SSL_set0_wbio(ssl, bio);

    return 1;
}

/*
 * Test to make sure that SSL_accept_connection returns the same ssl object
 * that is used in the various TLS callbacks
 *
 * Unlike TCP, QUIC processes new connections independently from their
 * acceptance, and so we need to pre-allocate tls objects to return during
 * connection acceptance via the user_ssl.  This is just a quic test to validate
 * that:
 * 1) The new callback to inform the user of a new pending ssl acceptance works
 *    properly
 * 2) That the object returned from SSL_accept_connection matches the one passed
 *    to various callbacks
 *
 * It would be better as its own test, but currently the tserver used in the
 * other quic_tests doesn't actually accept connections (it pre-creates them
 * and fixes them up in place), so testing there is not feasible at the moment
 *
 * For details on this issue see:
 * https://github.com/openssl/project/issues/918
 */
static SSL *pending_ssl_obj = NULL;
static SSL *client_hello_ssl_obj = NULL;
static int check_pending_match = 0;
static int pending_cb_called = 0;
static int hello_cb_called = 0;
static int new_pending_cb(SSL_CTX *ctx, SSL *new_ssl, void *arg)
{
    pending_ssl_obj = new_ssl;
    pending_cb_called = 1;
    return 1;
}

static int client_hello_cb(SSL *s, int *al, void *arg)
{
    client_hello_ssl_obj = s;
    hello_cb_called = 1;
    return 1;
}

DEF_FUNC(hf_new_ssl)
{
    int ok = 0;
    const char *name;
    SSL_CTX *ctx = NULL;
    const SSL_METHOD *method;
    SSL *ssl;
    uint64_t flags;
    int is_server, is_domain;

    F_POP2(name, flags);

    is_domain   = ((flags & 2) != 0);
    is_server   = ((flags & 1) != 0);

    method = is_server ? OSSL_QUIC_server_method() : OSSL_QUIC_client_method();
    if (!TEST_ptr(ctx = SSL_CTX_new(method)))
        goto err;

#if defined(OPENSSL_THREADS)
    if (!TEST_true(SSL_CTX_set_domain_flags(ctx,
                                            SSL_DOMAIN_FLAG_MULTI_THREAD
                                            | SSL_DOMAIN_FLAG_BLOCKING)))
        goto err;
#endif

    if (!TEST_true(ssl_ctx_configure(ctx, is_server)))
        goto err;

    if (is_domain) {
        if (!TEST_ptr(ssl = SSL_new_domain(ctx, 0)))
            goto err;

    } else if (is_server) {
        SSL_CTX_set_new_pending_conn_cb(ctx, new_pending_cb, NULL);
        SSL_CTX_set_client_hello_cb(ctx, client_hello_cb, NULL);
        check_pending_match = 1;
        if (!TEST_ptr(ssl = SSL_new_listener(ctx, 0)))
            goto err;
    } else {
        if (!TEST_ptr(ssl = SSL_new(ctx)))
            goto err;
    }

    if (!is_domain && !TEST_true(ssl_attach_bio_dgram(ssl, 0, NULL)))
        goto err;

    if (!TEST_true(RADIX_PROCESS_set_ssl(RP(), name, ssl))) {
        SSL_free(ssl);
        goto err;
    }

    ok = 1;
err:
    /* SSL object will hold ref, we don't need it */
    SSL_CTX_free(ctx);
    return ok;
}

DEF_FUNC(hf_new_ssl_listener_from)
{
    int ok = 0;
    SSL *domain, *listener;
    const char *listener_name;
    uint64_t flags;

    REQUIRE_SSL(domain);
    F_POP2(listener_name, flags);

    if (!TEST_ptr_null(RADIX_PROCESS_get_obj(RP(), listener_name)))
        goto err;

    if (!TEST_ptr(listener = SSL_new_listener_from(domain, flags)))
        goto err;

    if (!TEST_true(ssl_attach_bio_dgram(listener, 0, NULL)))
        goto err;

    if (!TEST_true(RADIX_PROCESS_set_ssl(RP(), listener_name, listener))) {
        SSL_free(listener);
        goto err;
    }

    radix_activate_slot(0);

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_listen)
{
    int ok = 0, r;
    SSL *ssl;

    REQUIRE_SSL(ssl);

    r = SSL_listen(ssl);
    if (!TEST_true(r))
        goto err;

    if (SSL_get0_domain(ssl) == NULL)
        radix_activate_slot(0);

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_new_stream)
{
    int ok = 0;
    const char *stream_name;
    SSL *conn, *stream;
    uint64_t flags, do_accept;

    F_POP2(flags, do_accept);
    F_POP(stream_name);
    REQUIRE_SSL(conn);

    if (!TEST_ptr_null(RADIX_PROCESS_get_obj(RP(), stream_name)))
        goto err;

    if (do_accept) {
        stream = SSL_accept_stream(conn, flags);

        if (stream == NULL)
            F_SPIN_AGAIN();
    } else {
        stream = SSL_new_stream(conn, flags);
    }

    if (!TEST_ptr(stream))
        goto err;

    /* TODO(QUIC RADIX): Implement wait behaviour */

    if (stream != NULL
        && !TEST_true(RADIX_PROCESS_set_ssl(RP(), stream_name, stream))) {
        SSL_free(stream);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_accept_conn)
{
    int ok = 0;
    const char *conn_name;
    uint64_t flags;
    SSL *listener, *conn;

    F_POP2(conn_name, flags);
    REQUIRE_SSL(listener);

    if (!TEST_ptr_null(RADIX_PROCESS_get_obj(RP(), conn_name)))
        goto err;

    conn = SSL_accept_connection(listener, flags);
    if (conn == NULL)
        F_SPIN_AGAIN();

    if (!TEST_true(RADIX_PROCESS_set_ssl(RP(), conn_name, conn))) {
        SSL_free(conn);
        goto err;
    }

    if (check_pending_match) {
        if (!pending_cb_called || !hello_cb_called) {
            TEST_info("Callbacks not called, skipping user_ssl check\n");
        } else {
            if (!TEST_ptr_eq(pending_ssl_obj, client_hello_ssl_obj)) {
                SSL_free(conn);
                goto err;
            }
            if (!TEST_ptr_eq(pending_ssl_obj, conn)) {
                SSL_free(conn);
                goto err;
            }
        }
        pending_ssl_obj = client_hello_ssl_obj = NULL;
        check_pending_match = 0;
        pending_cb_called = hello_cb_called = 0;
    }
    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_accept_conn_none)
{
    int ok = 0;
    SSL *listener, *conn;

    REQUIRE_SSL(listener);

    conn = SSL_accept_connection(listener, SSL_ACCEPT_CONNECTION_NO_BLOCK);
    if (!TEST_ptr_null(conn)) {
        SSL_free(conn);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_accept_stream_none)
{
    int ok = 0;
    const char *conn_name;
    uint64_t flags;
    SSL *conn, *stream;

    F_POP2(conn_name, flags);

    if (!TEST_ptr(conn = RADIX_PROCESS_get_ssl(RP(), conn_name)))
        goto err;

    stream = SSL_accept_stream(conn, flags);
    if (!TEST_ptr_null(stream)) {
        SSL_free(stream);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_pop_err)
{
    ERR_pop();

    return 1;
}

DEF_FUNC(hf_stream_reset)
{
    int ok = 0;
    const char *name;
    SSL_STREAM_RESET_ARGS args = {0};
    SSL *ssl;

    F_POP2(name, args.quic_error_code);
    REQUIRE_SSL(ssl);

    if (!TEST_true(SSL_stream_reset(ssl, &args, sizeof(args))))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_set_default_stream_mode)
{
    int ok = 0;
    uint64_t mode;
    SSL *ssl;

    F_POP(mode);
    REQUIRE_SSL(ssl);

    if (!TEST_true(SSL_set_default_stream_mode(ssl, (uint32_t)mode)))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_set_incoming_stream_policy)
{
    int ok = 0;
    uint64_t policy, error_code;
    SSL *ssl;

    F_POP(error_code);
    F_POP(policy);
    REQUIRE_SSL(ssl);

    if (!TEST_true(SSL_set_incoming_stream_policy(ssl, (int)policy, error_code)))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_shutdown_wait)
{
    int ok = 0, ret;
    uint64_t flags;
    SSL *ssl;
    SSL_SHUTDOWN_EX_ARGS args = {0};
    QUIC_CHANNEL *ch;

    F_POP(args.quic_reason);
    F_POP(args.quic_error_code);
    F_POP(flags);
    REQUIRE_SSL(ssl);

    ch = ossl_quic_conn_get_channel(ssl);
    ossl_quic_engine_set_inhibit_tick(ossl_quic_channel_get0_engine(ch), 0);

    ret = SSL_shutdown_ex(ssl, flags, &args, sizeof(args));
    if (!TEST_int_ge(ret, 0))
        goto err;

    if (ret == 0)
        F_SPIN_AGAIN();

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_conclude)
{
    int ok = 0;
    SSL *ssl;

    REQUIRE_SSL(ssl);

    if (!TEST_true(SSL_stream_conclude(ssl, 0)))
        goto err;

    ok = 1;
err:
    return ok;
}

static int is_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);

    return ec == SSL_ERROR_WANT_READ || ec == SSL_ERROR_WANT_WRITE;
}

static int check_consistent_want(SSL *s, int ret)
{
    int ec = SSL_get_error(s, ret);
    int w = SSL_want(s);

    int ok = TEST_true(
        (ec == SSL_ERROR_NONE                 && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_ZERO_RETURN          && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_SSL                  && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_SYSCALL              && w == SSL_NOTHING)
    ||  (ec == SSL_ERROR_WANT_READ            && w == SSL_READING)
    ||  (ec == SSL_ERROR_WANT_WRITE           && w == SSL_WRITING)
    ||  (ec == SSL_ERROR_WANT_CLIENT_HELLO_CB && w == SSL_CLIENT_HELLO_CB)
    ||  (ec == SSL_ERROR_WANT_X509_LOOKUP     && w == SSL_X509_LOOKUP)
    ||  (ec == SSL_ERROR_WANT_RETRY_VERIFY    && w == SSL_RETRY_VERIFY)
    );

    if (!ok)
        TEST_error("got error=%d, want=%d", ec, w);

    return ok;
}

DEF_FUNC(hf_write)
{
    int ok = 0, r;
    SSL *ssl;
    const void *buf;
    size_t buf_len, bytes_written = 0;

    F_POP2(buf, buf_len);
    REQUIRE_SSL(ssl);

    r = SSL_write_ex(ssl, buf, buf_len, &bytes_written);
    if (!TEST_true(r)
        || !check_consistent_want(ssl, r)
        || !TEST_size_t_eq(bytes_written, buf_len))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_write_rand)
{
    int ok = 0, r;
    SSL *ssl;
    void *buf = NULL;
    size_t buf_len, bytes_written = 0;

    F_POP(buf_len);
    REQUIRE_SSL(ssl);

    while (buf_len > 0) {
        size_t thislen = buf_len > 1024 ? 1024 : buf_len;

        if (buf == NULL)
            buf = OPENSSL_malloc(thislen);
        if (!TEST_ptr(buf))
            goto err;
        if (!TEST_int_eq(RAND_bytes(buf, thislen), 1))
            goto err;
        r = SSL_write_ex(ssl, buf, thislen, &bytes_written);
        if (!TEST_true(r)
            || !check_consistent_want(ssl, r)
            || !TEST_size_t_eq(bytes_written, thislen))
            goto err;

        buf_len -= thislen;
    }

    ok = 1;
err:
    OPENSSL_free(buf);
    return ok;
}

DEF_FUNC(hf_write_ex2)
{
    int ok = 0, r;
    SSL *ssl;
    const void *buf;
    size_t buf_len, bytes_written = 0;
    uint64_t flags;

    F_POP(flags);
    F_POP2(buf, buf_len);
    REQUIRE_SSL(ssl);

    r = SSL_write_ex2(ssl, buf, buf_len, flags, &bytes_written);
    if (!TEST_true(r)
        || !check_consistent_want(ssl, r)
        || !TEST_size_t_eq(bytes_written, buf_len))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_write_fail)
{
    int ok = 0, ret;
    SSL *ssl;
    size_t bytes_written = 0;

    REQUIRE_SSL(ssl);

    ret = SSL_write_ex(ssl, "apple", 5, &bytes_written);
    if (!TEST_false(ret)
        || !TEST_true(check_consistent_want(ssl, ret))
        || !TEST_size_t_eq(bytes_written, 0))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_read_expect)
{
    int ok = 0, r;
    SSL *ssl;
    const void *buf;
    size_t buf_len, bytes_read = 0;

    F_POP2(buf, buf_len);
    REQUIRE_SSL(ssl);

    if (buf_len > 0 && RT()->tmp_buf == NULL
        && !TEST_ptr(RT()->tmp_buf = OPENSSL_malloc(buf_len)))
        goto err;

    r = SSL_read_ex(ssl, RT()->tmp_buf + RT()->tmp_buf_offset,
                    buf_len - RT()->tmp_buf_offset,
                    &bytes_read);
    if (!TEST_true(check_consistent_want(ssl, r)))
        goto err;

    if (!r)
        F_SPIN_AGAIN();

    if (bytes_read + RT()->tmp_buf_offset != buf_len) {
        RT()->tmp_buf_offset += bytes_read;
        F_SPIN_AGAIN();
    }

    if (buf_len > 0
        && !TEST_mem_eq(RT()->tmp_buf, buf_len, buf, buf_len))
        goto err;

    OPENSSL_free(RT()->tmp_buf);
    RT()->tmp_buf         = NULL;
    RT()->tmp_buf_offset  = 0;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_read_fail)
{
    int ok = 0, r;
    SSL *ssl;
    char buf[1] = {0};
    size_t bytes_read = 0;
    uint64_t do_wait;

    F_POP(do_wait);
    REQUIRE_SSL(ssl);

    r = SSL_read_ex(ssl, buf, sizeof(buf), &bytes_read);
    if (!TEST_false(r)
        || !TEST_true(check_consistent_want(ssl, r))
        || !TEST_size_t_eq(bytes_read, 0))
        goto err;

    if (do_wait && is_want(ssl, 0))
        F_SPIN_AGAIN();

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_connect_wait)
{
    int ok = 0, ret;
    SSL *ssl;

    REQUIRE_SSL(ssl);

    /* if not started */
    if (RT()->scratch0 == 0) {
        if (!TEST_true(SSL_set_blocking_mode(ssl, 0)))
            return 0;

        /* 0 is the success case for SSL_set_alpn_protos(). */
        if (!TEST_false(SSL_set_alpn_protos(ssl, alpn_ossltest,
                                            sizeof(alpn_ossltest))))
            goto err;
    }

    RT()->scratch0 = 1; /* connect started */
    ret = SSL_connect(ssl);
    radix_activate_slot(0);
    if (!TEST_true(check_consistent_want(ssl, ret)))
        goto err;

    if (ret != 1) {
        if (is_want(ssl, ret))
            F_SPIN_AGAIN();

        if (!TEST_int_eq(ret, 1))
            goto err;
    }

    ok = 1;
err:
    RT()->scratch0 = 0;
    return ok;
}

DEF_FUNC(hf_detach)
{
    int ok = 0;
    const char *conn_name, *stream_name;
    SSL *conn, *stream;

    F_POP2(conn_name, stream_name);
    if (!TEST_ptr(conn = RADIX_PROCESS_get_ssl(RP(), conn_name)))
        goto err;

    if (!TEST_ptr(stream = ossl_quic_detach_stream(conn)))
        goto err;

    if (!TEST_true(RADIX_PROCESS_set_ssl(RP(), stream_name, stream))) {
        SSL_free(stream);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_attach)
{
    int ok = 0;
    const char *conn_name, *stream_name;
    SSL *conn, *stream;

    F_POP2(conn_name, stream_name);

    if (!TEST_ptr(conn = RADIX_PROCESS_get_ssl(RP(), conn_name)))
        goto err;

    if (!TEST_ptr(stream = RADIX_PROCESS_get_ssl(RP(), stream_name)))
        goto err;

    if (!TEST_true(ossl_quic_attach_stream(conn, stream)))
        goto err;

    if (!TEST_true(RADIX_PROCESS_set_ssl(RP(), stream_name, NULL)))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_expect_fin)
{
    int ok = 0, ret;
    SSL *ssl;
    char buf[1];
    size_t bytes_read = 0;

    REQUIRE_SSL(ssl);

    ret = SSL_read_ex(ssl, buf, sizeof(buf), &bytes_read);
    if (!TEST_true(check_consistent_want(ssl, ret))
        || !TEST_false(ret)
        || !TEST_size_t_eq(bytes_read, 0))
        goto err;

    if (is_want(ssl, 0))
        F_SPIN_AGAIN();

    if (!TEST_int_eq(SSL_get_error(ssl, 0),
                     SSL_ERROR_ZERO_RETURN))
        goto err;

    if (!TEST_int_eq(SSL_want(ssl), SSL_NOTHING))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_expect_conn_close_info)
{
    int ok = 0;
    SSL *ssl;
    SSL_CONN_CLOSE_INFO cc_info = {0};
    uint64_t error_code, expect_app, expect_remote;

    F_POP(error_code);
    F_POP2(expect_app, expect_remote);
    REQUIRE_SSL(ssl);

    /* TODO BLOCKING */

    if (!SSL_get_conn_close_info(ssl, &cc_info, sizeof(cc_info)))
        F_SPIN_AGAIN();

    if (!TEST_int_eq((int)expect_app,
                     (cc_info.flags & SSL_CONN_CLOSE_FLAG_TRANSPORT) == 0)
        || !TEST_int_eq((int)expect_remote,
                        (cc_info.flags & SSL_CONN_CLOSE_FLAG_LOCAL) == 0)
        || !TEST_uint64_t_eq(error_code, cc_info.error_code)) {
        TEST_info("connection close reason: %s", cc_info.reason);
        goto err;
    }

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_wait_for_data)
{
    int ok = 0;
    SSL *ssl;
    char buf[1];
    size_t bytes_read = 0;

    REQUIRE_SSL(ssl);

    if (!SSL_peek_ex(ssl, buf, sizeof(buf), &bytes_read)
        || bytes_read == 0)
        F_SPIN_AGAIN();

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_expect_err)
{
    int ok = 0;
    uint64_t lib, reason;

    F_POP2(lib, reason);
    if (!TEST_size_t_eq((size_t)ERR_GET_LIB(ERR_peek_last_error()),
                        (size_t)lib)
        || !TEST_size_t_eq((size_t)ERR_GET_REASON(ERR_peek_last_error()),
                           (size_t)reason))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_expect_ssl_err)
{
    int ok = 0;
    uint64_t expected;
    SSL *ssl;

    F_POP(expected);
    REQUIRE_SSL(ssl);

    if (!TEST_size_t_eq((size_t)SSL_get_error(ssl, 0), (size_t)expected)
        || !TEST_int_eq(SSL_want(ssl), SSL_NOTHING))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_expect_stream_id)
{
    int ok = 0;
    SSL *ssl;
    uint64_t expected, actual;

    F_POP(expected);
    REQUIRE_SSL(ssl);

    actual = SSL_get_stream_id(ssl);
    if (!TEST_uint64_t_eq(actual, expected))
        goto err;

    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_select_ssl)
{
    int ok = 0;
    uint64_t slot;
    const char *name;
    RADIX_OBJ *obj;

    F_POP2(slot, name);
    if (!TEST_ptr(obj = RADIX_PROCESS_get_obj(RP(), name)))
        goto err;

    if (!TEST_uint64_t_lt(slot, NUM_SLOTS))
        goto err;

    RT()->slot[slot]    = obj;
    RT()->ssl[slot]     = obj->ssl;
    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_clear_slot)
{
    int ok = 0;
    uint64_t slot;

    F_POP(slot);
    if (!TEST_uint64_t_lt(slot, NUM_SLOTS))
        goto err;

    RT()->slot[slot]    = NULL;
    RT()->ssl[slot]     = NULL;
    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_skip_time)
{
    int ok = 0;
    uint64_t ms;

    F_POP(ms);

    radix_skip_time(ossl_ms2time(ms));
    ok = 1;
err:
    return ok;
}

DEF_FUNC(hf_set_peer_addr_from)
{
    int ok = 0;
    SSL *dst_ssl, *src_ssl;
    BIO *dst_bio, *src_bio;
    int src_fd = -1;
    union BIO_sock_info_u src_info;
    BIO_ADDR *src_addr = NULL;

    REQUIRE_SSL_N(0, dst_ssl);
    REQUIRE_SSL_N(1, src_ssl);
    dst_bio = SSL_get_rbio(dst_ssl);
    src_bio = SSL_get_rbio(src_ssl);
    if (!TEST_ptr(dst_bio) || !TEST_ptr(src_bio))
        goto err;

    if (!TEST_ptr(src_addr = BIO_ADDR_new()))
        goto err;

    if (!TEST_true(BIO_get_fd(src_bio, &src_fd))
        || !TEST_int_ge(src_fd, 0))
        goto err;

    src_info.addr = src_addr;
    if (!TEST_true(BIO_sock_info(src_fd, BIO_SOCK_INFO_ADDRESS, &src_info))
        || !TEST_int_ge(ntohs(BIO_ADDR_rawport(src_addr)), 0))
        goto err;

    /*
     * Could use SSL_set_initial_peer_addr here, but set it on the
     * BIO_s_datagram instead and make sure we pick it up automatically.
     */
    if (!TEST_true(BIO_dgram_set_peer(dst_bio, src_addr)))
        goto err;

    ok = 1;
err:
    BIO_ADDR_free(src_addr);
    return ok;
}

DEF_FUNC(hf_sleep)
{
    int ok = 0;
    uint64_t ms;

    F_POP(ms);

    OSSL_sleep(ms);

    ok = 1;
err:
    return ok;
}

#define OP_UNBIND(name)                                         \
    (OP_PUSH_PZ(#name),                                         \
     OP_FUNC(hf_unbind))

#define OP_SELECT_SSL(slot, name)                               \
    (OP_PUSH_U64(slot),                                         \
     OP_PUSH_PZ(#name),                                         \
     OP_FUNC(hf_select_ssl))

#define OP_CLEAR_SLOT(slot)                                     \
    (OP_PUSH_U64(slot),                                         \
     OP_FUNC(hf_clear_slot))

#define OP_CONNECT_WAIT(name)                                   \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_connect_wait))

#define OP_LISTEN(name)                                         \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_listen))

#define OP_NEW_SSL_C(name)                                      \
    (OP_PUSH_PZ(#name),                                         \
     OP_PUSH_U64(0),                                            \
     OP_FUNC(hf_new_ssl))

#define OP_NEW_SSL_L(name)                                      \
    (OP_PUSH_PZ(#name),                                         \
     OP_PUSH_U64(1),                                            \
     OP_FUNC(hf_new_ssl))

#define OP_NEW_SSL_D(name)                                      \
    (OP_PUSH_PZ(#name),                                         \
     OP_PUSH_U64(3),                                            \
     OP_FUNC(hf_new_ssl))

#define OP_NEW_SSL_L_LISTEN(name)                               \
    (OP_NEW_SSL_L(name),                                        \
     OP_LISTEN(name))

#define OP_NEW_SSL_L_FROM(domain_name, listener_name, flags)    \
    (OP_SELECT_SSL(0, domain_name),                             \
     OP_PUSH_PZ(#listener_name),                                \
     OP_PUSH_U64(flags),                                        \
     OP_FUNC(hf_new_ssl_listener_from))

#define OP_NEW_SSL_L_FROM_LISTEN(domain_name, listener_name, flags) \
    (OP_NEW_SSL_L_FROM(domain_name, listener_name, flags),      \
     OP_LISTEN(listener_name))

#define OP_SET_PEER_ADDR_FROM(dst_name, src_name)               \
    (OP_SELECT_SSL(0, dst_name),                                \
     OP_SELECT_SSL(1, src_name),                                \
     OP_FUNC(hf_set_peer_addr_from))

#define OP_SIMPLE_PAIR_CONN()                                   \
    (OP_NEW_SSL_L_LISTEN(L),                                    \
     OP_NEW_SSL_C(C),                                           \
     OP_SET_PEER_ADDR_FROM(C, L),                               \
     OP_CONNECT_WAIT(C))

#define OP_SIMPLE_PAIR_CONN_D()                                 \
    (OP_NEW_SSL_D(Ds),                                          \
     OP_NEW_SSL_L_FROM_LISTEN(Ds, L, 0),                        \
     OP_NEW_SSL_C(C),                                           \
     OP_SET_PEER_ADDR_FROM(C, L),                               \
     OP_CONNECT_WAIT(C))

#define OP_SIMPLE_PAIR_CONN_ND()                                \
    (OP_SIMPLE_PAIR_CONN(),                                     \
     OP_SET_DEFAULT_STREAM_MODE(C, SSL_DEFAULT_STREAM_MODE_NONE))

#define OP_NEW_STREAM(conn_name, stream_name, flags)            \
    (OP_SELECT_SSL(0, conn_name),                               \
     OP_PUSH_PZ(#stream_name),                                  \
     OP_PUSH_U64(flags),                                        \
     OP_PUSH_U64(0),                                            \
     OP_FUNC(hf_new_stream))

#define OP_ACCEPT_STREAM_WAIT(conn_name, stream_name, flags)    \
    (OP_SELECT_SSL(0, conn_name),                               \
     OP_PUSH_PZ(#stream_name),                                  \
     OP_PUSH_U64(flags),                                        \
     OP_PUSH_U64(1),                                            \
     OP_FUNC(hf_new_stream))

#define OP_ACCEPT_STREAM_NONE(conn_name)                        \
    (OP_SELECT_SSL(0, conn_name),                               \
     OP_FUNC(hf_accept_stream_none))

#define OP_ACCEPT_CONN_WAIT(listener_name, conn_name, flags)    \
    (OP_SELECT_SSL(0, listener_name),                           \
     OP_PUSH_PZ(#conn_name),                                    \
     OP_PUSH_U64(flags),                                        \
     OP_FUNC(hf_accept_conn))

#define OP_ACCEPT_CONN_WAIT_ND(listener_name, conn_name, flags) \
    (OP_ACCEPT_CONN_WAIT(listener_name, conn_name, flags),      \
     OP_SET_DEFAULT_STREAM_MODE(conn_name, SSL_DEFAULT_STREAM_MODE_NONE))

#define OP_ACCEPT_CONN_NONE(listener_name)                      \
    (OP_SELECT_SSL(0, listener_name),                           \
     OP_FUNC(hf_accept_conn_none))

#define OP_ACCEPT_CONN_WAIT1(listener_name, conn_name, flags)   \
    (OP_ACCEPT_CONN_WAIT(listener_name, conn_name, flags),      \
     OP_ACCEPT_CONN_NONE(listener_name))

#define OP_ACCEPT_CONN_WAIT1_ND(listener_name, conn_name, flags) \
    (OP_ACCEPT_CONN_WAIT_ND(listener_name, conn_name, flags),    \
     OP_ACCEPT_CONN_NONE(listener_name))

#define OP_WRITE(name, buf, buf_len)                            \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_BUFP(buf, buf_len),                                \
     OP_FUNC(hf_write))

#define OP_WRITE_RAND(name, buf_len)                            \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_SIZE(buf_len),                                     \
     OP_FUNC(hf_write_rand))

#define OP_WRITE_B(name, buf)                                   \
    OP_WRITE(name, (buf), sizeof(buf))

#define OP_WRITE_EX2(name, buf, buf_len, flags)                 \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_BUFP(buf, buf_len),                                \
     OP_PUSH_U64(flags),                                        \
     OP_FUNC(hf_write_ex2))

#define OP_WRITE_FAIL(name)                                     \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_write_fail))

#define OP_CONCLUDE(name)                                       \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_conclude))

#define OP_READ_EXPECT(name, buf, buf_len)                      \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_BUFP(buf, buf_len),                                \
     OP_FUNC(hf_read_expect))

#define OP_READ_EXPECT_B(name, buf)                             \
    OP_READ_EXPECT(name, (buf), sizeof(buf))

#define OP_READ_FAIL()                                          \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(0),                                            \
     OP_FUNC(hf_read_fail))

#define OP_READ_FAIL_WAIT(name)                                 \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(1),                                            \
     OP_FUNC(hf_read_fail)

#define OP_POP_ERR()                                            \
    OP_FUNC(hf_pop_err)

#define OP_SET_DEFAULT_STREAM_MODE(name, mode)                  \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(mode),                                         \
     OP_FUNC(hf_set_default_stream_mode))

#define OP_SET_INCOMING_STREAM_POLICY(name, policy, error_code) \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(policy),                                       \
     OP_PUSH_U64(error_code),                                   \
     OP_FUNC(hf_set_incoming_stream_policy))

#define OP_STREAM_RESET(name, error_code)                       \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(flags),                                        \
     OP_PUSH_U64(error_code),                                   \
     OP_FUNC(hf_stream_reset))                                  \

#define OP_SHUTDOWN_WAIT(name, flags, error_code, reason)       \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(flags),                                        \
     OP_PUSH_U64(error_code),                                   \
     OP_PUSH_PZ(reason),                                        \
     OP_FUNC(hf_shutdown_wait))

#define OP_DETACH(conn_name, stream_name)                       \
    (OP_SELECT_SSL(0, conn_name),                               \
     OP_PUSH_PZ(#stream_name),                                  \
     OP_FUNC(hf_detach))

#define OP_ATTACH(conn_name, stream_name)                       \
    (OP_SELECT_SSL(0, conn_name),                               \
     OP_PUSH_PZ(stream_name),                                   \
     OP_FUNC(hf_attach))

#define OP_EXPECT_FIN(name)                                     \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_expect_fin))

#define OP_EXPECT_CONN_CLOSE_INFO(name, error_code, expect_app, expect_remote) \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(expect_app),                                   \
     OP_PUSH_U64(expect_remote),                                \
     OP_PUSH_U64(error_code),                                   \
     OP_FUNC(hf_expect_conn_close_info))

#define OP_WAIT_FOR_DATA(name)                                  \
    (OP_SELECT_SSL(0, name),                                    \
     OP_FUNC(hf_wait_for_data))

#define OP_EXPECT_ERR(lib, reason)                              \
    (OP_PUSH_U64(lib),                                          \
     OP_PUSH_U64(reason),                                       \
     OP_FUNC(hf_expect_err))

#define OP_EXPECT_SSL_ERR(name, expected)                       \
    (OP_SELECT_SSL(0, name),                                    \
     OP_PUSH_U64(expected),                                     \
     OP_FUNC(hf_expect_ssl_err))

#define OP_EXPECT_STREAM_ID(expected)                           \
    (OP_PUSH_U64(expected),                                     \
     OP_FUNC(hf_expect_stream_id))

#define OP_SKIP_TIME(ms)                                        \
    (OP_PUSH_U64(ms),                                           \
     OP_FUNC(hf_skip_time))

#define OP_SLEEP(ms)                                            \
    (OP_PUSH_U64(ms),                                           \
     OP_FUNC(hf_sleep))
