#include <openssl/ssl.h>

/*
 * Demo 1: Client — Managed Connection — Blocking
 * ==============================================
 *
 * This is an example of (part of) an application which uses libssl in a simple,
 * synchronous, blocking fashion. The functions show all interactions with
 * libssl the application makes, and would hypothetically be linked into a
 * larger application.
 */

/*
 * The application is initializing and wants an SSL_CTX which it will use for
 * some number of outgoing connections, which it creates in subsequent calls to
 * new_conn. The application may also call this function multiple times to
 * create multiple SSL_CTX.
 */
SSL_CTX *create_ssl_ctx(void)
{
    SSL_CTX *ctx;

#ifdef USE_QUIC
    ctx = SSL_CTX_new(OSSL_QUIC_client_method());
#else
    ctx = SSL_CTX_new(TLS_client_method());
#endif
    if (ctx == NULL)
        return NULL;

    /* Enable trust chain verification. */
    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);

    /* Load default root CA store. */
    if (SSL_CTX_set_default_verify_paths(ctx) == 0) {
        SSL_CTX_free(ctx);
        return NULL;
    }

    return ctx;
}

/*
 * The application wants to create a new outgoing connection using a given
 * SSL_CTX.
 *
 * hostname is a string like "openssl.org:443" or "[::1]:443".
 */
BIO *new_conn(SSL_CTX *ctx, const char *hostname)
{
    BIO *out;
    SSL *ssl = NULL;
    const char *bare_hostname;
#ifdef USE_QUIC
    static const unsigned char alpn[] = {5, 'd', 'u', 'm', 'm', 'y'};
#endif

    out = BIO_new_ssl_connect(ctx);
    if (out == NULL)
        return NULL;

    if (BIO_get_ssl(out, &ssl) == 0) {
        BIO_free_all(out);
        return NULL;
    }

    if (BIO_set_conn_hostname(out, hostname) == 0) {
        BIO_free_all(out);
        return NULL;
    }

    /* Returns the parsed hostname extracted from the hostname:port string. */
    bare_hostname = BIO_get_conn_hostname(out);
    if (bare_hostname == NULL) {
        BIO_free_all(out);
        return NULL;
    }

    /* Tell the SSL object the hostname to check certificates against. */
    if (SSL_set1_host(ssl, bare_hostname) <= 0) {
        BIO_free_all(out);
        return NULL;
    }

#ifdef USE_QUIC
    /* Configure ALPN, which is required for QUIC. */
    if (SSL_set_alpn_protos(ssl, alpn, sizeof(alpn))) {
        /* Note: SSL_set_alpn_protos returns 1 for failure. */
        BIO_free_all(out);
        return NULL;
    }
#endif

    return out;
}

/*
 * The application wants to send some block of data to the peer.
 * This is a blocking call.
 */
int tx(BIO *bio, const void *buf, int buf_len)
{
    return BIO_write(bio, buf, buf_len);
}

/*
 * The application wants to receive some block of data from
 * the peer. This is a blocking call.
 */
int rx(BIO *bio, void *buf, int buf_len)
{
    return BIO_read(bio, buf, buf_len);
}

/*
 * The application wants to close the connection and free bookkeeping
 * structures.
 */
void teardown(BIO *bio)
{
    BIO_free_all(bio);
}

/*
 * The application is shutting down and wants to free a previously
 * created SSL_CTX.
 */
void teardown_ctx(SSL_CTX *ctx)
{
    SSL_CTX_free(ctx);
}

/*
 * ============================================================================
 * Example driver for the above code. This is just to demonstrate that the code
 * works and is not intended to be representative of a real application.
 */
int main(int argc, char **argv)
{
    static char msg[384], host_port[300];
    SSL_CTX *ctx = NULL;
    BIO *b = NULL;
    char buf[2048];
    int l, mlen, res = 1;

    if (argc < 3) {
        fprintf(stderr, "usage: %s host port\n", argv[0]);
        goto fail;
    }

    snprintf(host_port, sizeof(host_port), "%s:%s", argv[1], argv[2]);
    mlen = snprintf(msg, sizeof(msg),
                    "GET / HTTP/1.0\r\nHost: %s\r\n\r\n", argv[1]);

    ctx = create_ssl_ctx();
    if (ctx == NULL) {
        fprintf(stderr, "could not create context\n");
        goto fail;
    }

    b = new_conn(ctx, host_port);
    if (b == NULL) {
        fprintf(stderr, "could not create connection\n");
        goto fail;
    }

    l = tx(b, msg, mlen);
    if (l < mlen) {
        fprintf(stderr, "tx error\n");
        goto fail;
    }

    for (;;) {
        l = rx(b, buf, sizeof(buf));
        if (l <= 0)
            break;
        fwrite(buf, 1, l, stdout);
    }

    res = 0;
fail:
    if (b != NULL)
        teardown(b);
    if (ctx != NULL)
        teardown_ctx(ctx);
    return res;
}
