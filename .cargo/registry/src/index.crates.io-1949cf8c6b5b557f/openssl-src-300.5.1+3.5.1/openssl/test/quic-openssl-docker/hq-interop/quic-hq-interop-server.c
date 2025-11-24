/*
 *  Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 *  Licensed under the Apache License 2.0 (the "License").  You may not use
 *  this file except in compliance with the License.  You can obtain a copy
 *  in the file LICENSE in the source distribution or at
 *  https://www.openssl.org/source/license.html
 */

/**
 * @file quic-hq-interop-server.c
 * @brief Minimal QUIC HTTP/0.9 server implementation.
 *
 * This file implements a lightweight QUIC server supporting the HTTP/0.9
 * protocol for interoperability testing. It includes functions for setting
 * up a secure QUIC connection, handling ALPN negotiation, and serving client
 * requests.  Intended for use with the quic-interop-runner
 * available at https://interop.seemann.io
 *
 * Key functionalities:
 * - Setting up SSL_CTX with QUIC support.
 * - Negotiating ALPN strings during the TLS handshake.
 * - Listening and accepting incoming QUIC connections.
 * - Handling client requests via HTTP/0.9 protocol.
 *
 * Usage:
 *   <port> <server.crt> <server.key>
 * The server binds to the specified port and serves files using the given
 * certificate and private key.
 *
 * Environment variables:
 * - FILEPREFIX: Specifies the directory containing files to serve.
 *   Defaults to "./downloads" if not set.
 * - SSLKEYLOGFILE: specifies that keylogging should be preformed on the server
 *   should be set to a file name to record keylog data to
 * - NO_ADDR_VALIDATE: Disables server address validation of clients
 *
 */

#include <string.h>

/* Include the appropriate header file for SOCK_STREAM */
#ifdef _WIN32
# include <stdarg.h>
# include <winsock2.h>
# include <ws2tcpip.h>
#else
# include <sys/socket.h>
# include <netinet/in.h>
# include <unistd.h>
#endif

#include <openssl/bio.h>
#include <openssl/ssl.h>
#include <openssl/err.h>
#include <openssl/quic.h>

#define BUF_SIZE 4096

/**
 * @brief ALPN (Application-Layer Protocol Negotiation) identifier for QUIC.
 *
 * This constant defines the ALPN string used during the TLS handshake
 * to negotiate the application-layer protocol between the client and
 * the server. It specifies "hq-interop" as the supported protocol.
 *
 * Format:
 * - The first byte represents the length of the ALPN string.
 * - Subsequent bytes represent the ASCII characters of the protocol name.
 *
 * Value:
 * - Protocol: "hq-interop"
 * - Length: 10 bytes
 *
 * Usage:
 * This is passed to the ALPN callback function to validate and
 * negotiate the desired protocol during the TLS handshake.
 */
static const unsigned char alpn_ossltest[] = {
    10, 'h', 'q', '-', 'i', 'n', 't', 'e', 'r', 'o', 'p',
};

/**
 * @brief Directory prefix for serving requested files.
 *
 * This variable specifies the directory path used as the base location
 * for serving files in response to client requests. It is used to construct
 * the full file path for requested resources.
 *
 * Default:
 * - If not set via the FILEPREFIX environment variable, it defaults to
 *   "./downloads".
 *
 * Usage:
 * - Updated at runtime based on the FILEPREFIX environment variable.
 * - Used to locate and serve files during incoming requests.
 */
static char *fileprefix = NULL;

/**
 * @brief Callback for ALPN (Application-Layer Protocol Negotiation) selection.
 *
 * This function is invoked during the TLS handshake on the server side to
 * validate and negotiate the desired ALPN (Application-Layer Protocol
 * Negotiation) protocol with the client. It ensures that the negotiated
 * protocol matches the predefined "hq-interop" string.
 *
 * @param ssl       Pointer to the SSL connection object.
 * @param[out] out  Pointer to the negotiated ALPN protocol string.
 * @param[out] out_len Length of the negotiated ALPN protocol string.
 * @param in        Pointer to the client-provided ALPN protocol list.
 * @param in_len    Length of the client-provided ALPN protocol list.
 * @param arg       Optional user-defined argument (unused in this context).
 *
 * @return SSL_TLSEXT_ERR_OK on successful ALPN negotiation,
 *         SSL_TLSEXT_ERR_ALERT_FATAL otherwise.
 *
 * Usage:
 * - This function is set as the ALPN selection callback in the SSL_CTX
 *   using `SSL_CTX_set_alpn_select_cb`.
 * - Ensures that only the predefined ALPN protocol is accepted.
 *
 * Note:
 * - The predefined protocol is specified in the `alpn_ossltest` array.
 */
static int select_alpn(SSL *ssl, const unsigned char **out,
                       unsigned char *out_len, const unsigned char *in,
                       unsigned int in_len, void *arg)
{
    /*
     * Use the next_proto helper function here.
     * This scans the list of alpns we support and matches against
     * what the client is requesting
     */
    if (SSL_select_next_proto((unsigned char **)out, out_len, alpn_ossltest,
                              sizeof(alpn_ossltest), in,
                              in_len) == OPENSSL_NPN_NEGOTIATED)
        return SSL_TLSEXT_ERR_OK;
    return SSL_TLSEXT_ERR_ALERT_FATAL;
}

/**
 * @brief Creates and configures an SSL_CTX for a QUIC server.
 *
 * This function initializes an SSL_CTX object with the QUIC server method
 * and configures it using the provided certificate and private key. The
 * context is prepared for handling secure QUIC connections and performing
 * ALPN (Application-Layer Protocol Negotiation).
 *
 * @param cert_path Path to the server's certificate chain file in PEM format.
 *                  The chain file must include the server's leaf certificate
 *                  followed by intermediate CA certificates.
 * @param key_path  Path to the server's private key file in PEM format. The
 *                  private key must correspond to the leaf certificate in
 *                  the chain file.
 *
 * @return Pointer to the initialized SSL_CTX on success, or NULL on failure.
 *
 * Configuration:
 * - Loads the certificate chain and private key into the context.
 * - Disables client certificate verification (no mutual TLS).
 * - Sets up the ALPN selection callback for protocol negotiation.
 *
 * Error Handling:
 * - If any step fails (e.g., loading the certificate or key), the function
 *   frees the SSL_CTX and returns NULL.
 *
 * Usage:
 * - Call this function to create an SSL_CTX before starting the QUIC server.
 * - Ensure valid paths for the certificate and private key are provided.
 *
 * Note:
 * - The ALPN callback only supports the predefined protocol defined in
 *   `alpn_ossltest`.
 */
static SSL_CTX *create_ctx(const char *cert_path, const char *key_path)
{
    SSL_CTX *ctx;

    /*
     * An SSL_CTX holds shared configuration information for multiple
     * subsequent per-client connections. We specifically load a QUIC
     * server method here.
     */
    ctx = SSL_CTX_new(OSSL_QUIC_server_method());
    if (ctx == NULL)
        goto err;

    /*
     * Load the server's certificate *chain* file (PEM format), which includes
     * not only the leaf (end-entity) server certificate, but also any
     * intermediate issuer-CA certificates.  The leaf certificate must be the
     * first certificate in the file.
     *
     * In advanced use-cases this can be called multiple times, once per public
     * key algorithm for which the server has a corresponding certificate.
     * However, the corresponding private key (see below) must be loaded first,
     * *before* moving on to the next chain file.
     *
     * The requisite files "chain.pem" and "pkey.pem" can be generated by running
     * "make chain" in this directory.  If the server will be executed from some
     * other directory, move or copy the files there.
     */
    if (SSL_CTX_use_certificate_chain_file(ctx, cert_path) <= 0) {
        fprintf(stderr, "couldn't load certificate file: %s\n", cert_path);
        goto err;
    }

    /*
     * Load the corresponding private key, this also checks that the private
     * key matches the just loaded end-entity certificate.  It does not check
     * whether the certificate chain is valid, the certificates could be
     * expired, or may otherwise fail to form a chain that a client can validate.
     */
    if (SSL_CTX_use_PrivateKey_file(ctx, key_path, SSL_FILETYPE_PEM) <= 0) {
        fprintf(stderr, "couldn't load key file: %s\n", key_path);
        goto err;
    }

    /*
     * Since we're not soliciting or processing client certificates, we don't
     * need to configure a trusted-certificate store, so no call to
     * SSL_CTX_set_default_verify_paths() is needed.  The server's own
     * certificate chain is assumed valid.
     */
    SSL_CTX_set_verify(ctx, SSL_VERIFY_NONE, NULL);

    /* Setup ALPN negotiation callback to decide which ALPN is accepted. */
    SSL_CTX_set_alpn_select_cb(ctx, select_alpn, NULL);

    return ctx;

err:
    SSL_CTX_free(ctx);
    return NULL;
}

/**
 * @brief Creates and binds a UDP socket to the specified port.
 *
 * This function initializes a new UDP socket, binds it to the specified
 * port on the local host, and returns the socket file descriptor for
 * further use.
 *
 * @param port The port number to which the UDP socket should be bound.
 *
 * @return On success, returns the BIO of the created socket.
 *         On failure, returns NULL.
 *
 * Steps:
 * - Creates a new UDP socket using the `socket` system call.
 * - Configures the socket address structure to bind to the specified port
 *   on the local host.
 * - Binds the socket to the port using the `bind` system call.
 *
 * Error Handling:
 * - If socket creation or binding fails, an error message is printed to
 *   `stderr`, the socket (if created) is closed, and -1 is returned.
 *
 * Usage:
 * - Call this function to set up a socket for handling incoming QUIC
 *   connections.
 *
 * Notes:
 * - This function assumes UDP (`SOCK_DGRAM`).
 * - This function accepts on both IPv4 and IPv6.
 * - The specified port is converted to network byte order using `htons`.
 */
static BIO *create_socket(uint16_t port)
{
    int fd = -1;
    BIO *sock = NULL;
    BIO_ADDR *addr = NULL;
    int opt = 0;
#ifdef _WIN32
    struct in6_addr in6addr_any;

    memset(&in6addr_any, 0, sizeof(in6addr_any));
#endif

    /* Retrieve the file descriptor for a new UDP socket */
    if ((fd = BIO_socket(AF_INET6, SOCK_DGRAM, IPPROTO_UDP, 0)) < 0) {
        fprintf(stderr, "cannot create socket");
        goto err;
    }

    /*
     * IPv6_V6ONLY is only available on some platforms. If it is defined,
     * disable it to accept both IPv4 and IPv6 connections. Otherwise, the
     * server will only accept IPv6 connections.
     */
#ifdef IPV6_V6ONLY
    if (setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &opt, sizeof(opt)) < 0) {
        fprintf(stderr, "setsockopt IPV6_V6ONLY failed");
        goto err;
    }
#endif

    /*
     * Create a new BIO_ADDR
     */
    addr = BIO_ADDR_new();
    if (addr == NULL) {
        fprintf(stderr, "Unable to create BIO_ADDR\n");
        goto err;
    }

    /*
     * Build an INADDR_ANY BIO_ADDR
     */
    if (!BIO_ADDR_rawmake(addr, AF_INET6, &in6addr_any, sizeof(in6addr_any), htons(port))) {
        fprintf(stderr, "unable to bind to port %d\n", port);
        goto err;
    }

    /* Bind to the new UDP socket */
    if (!BIO_bind(fd, addr, 0)) {
        fprintf(stderr, "cannot bind to %u\n", port);
        goto err;
    }

    /*
     * Create a new datagram socket
     */
    sock = BIO_new(BIO_s_datagram());
    if (sock == NULL) {
        fprintf(stderr, "cannot create dgram bio\n");
        goto err;
    }

    /*
     * associate the underlying socket with the dgram BIO
     */
    if (!BIO_set_fd(sock, fd, BIO_CLOSE)) {
        fprintf(stderr, "Unable to set fd of dgram sock\n");
        goto err;
    }

    /*
     * Free our allocated addr
     */
    BIO_ADDR_free(addr);
    return sock;

err:
    BIO_ADDR_free(addr);
    BIO_free(sock);
    BIO_closesocket(fd);
    return NULL;
}

/**
 * @brief Handles I/O failures on an SSL stream based on the result code.
 *
 * This function processes the result of an SSL I/O operation and handles
 * different types of errors that may occur during the operation. It takes
 * appropriate actions such as retrying the operation, reporting errors, or
 * returning specific status codes based on the error type.
 *
 * @param ssl A pointer to the SSL object representing the stream.
 * @param res The result code from the SSL I/O operation.
 * @return An integer indicating the outcome:
 *         - 0: EOF, indicating the stream has been closed.
 *         - -1: A fatal error occurred or the stream has been reset.
 *
 *
 * @note If the failure is due to an SSL verification error, additional
 * information will be logged to stderr.
 */
static int handle_io_failure(SSL *ssl, int res)
{
    switch (SSL_get_error(ssl, res)) {
    case SSL_ERROR_ZERO_RETURN:
        /* EOF */
        return 0;

    case SSL_ERROR_SYSCALL:
        return -1;

    case SSL_ERROR_SSL:
        /*
         * Some stream fatal error occurred. This could be because of a
         * stream reset - or some failure occurred on the underlying
         * connection.
         */
        switch (SSL_get_stream_read_state(ssl)) {
        case SSL_STREAM_STATE_RESET_REMOTE:
            fprintf(stderr, "Stream reset occurred\n");
            /*
             * The stream has been reset but the connection is still
             * healthy.
             */
            break;

        case SSL_STREAM_STATE_CONN_CLOSED:
            fprintf(stderr, "Connection closed\n");
            /* Connection is already closed. */
            break;

        default:
            fprintf(stderr, "Unknown stream failure\n");
            break;
        }
        /*
         * If the failure is due to a verification error we can get more
         * information about it from SSL_get_verify_result().
         */
        if (SSL_get_verify_result(ssl) != X509_V_OK)
            fprintf(stderr, "Verify error: %s\n",
                    X509_verify_cert_error_string(SSL_get_verify_result(ssl)));
        return -1;

    default:
        return -1;
    }
}

/**
 * @brief Processes a new incoming QUIC stream for an HTTP/0.9 GET request.
 *
 * This function reads an HTTP/0.9 GET request from the provided QUIC stream,
 * retrieves the requested file from the server's file system, and sends the
 * file contents back to the client over the stream.
 *
 * @param Pointer to the SSL object representing the QUIC stream.
 *
 * Operation:
 * - Reads the HTTP/0.9 GET request from the client.
 * - Parses the request to extract the requested file name.
 * - Constructs the file path using the `fileprefix` directory.
 * - Reads the requested file in chunks and sends it to the client.
 * - Concludes the QUIC stream once the file is fully sent.
 *
 * Error Handling:
 * - If the request is invalid or the file cannot be opened, appropriate
 *   error messages are logged, and the function exits without sending data.
 * - Errors during file reading or writing to the stream are handled, with
 *   retries for buffer-related issues (e.g., full send buffer).
 *
 * Notes:
 * - The request is expected to be a valid HTTP/0.9 GET request.
 * - File paths are sanitized to prevent path traversal vulnerabilities.
 * - The function uses blocking operations for reading and writing data.
 *
 * Usage:
 * - Called for each accepted QUIC stream to handle client requests.
 */
static void process_new_stream(SSL *stream)
{
    unsigned char buf[BUF_SIZE];
    char path[BUF_SIZE];
    char *req = (char *)buf;
    char *reqname;
    char *creturn;
    size_t nread;
    BIO *readbio;
    size_t bytes_read = 0;
    size_t bytes_written = 0;
    size_t offset = 0;
    int rc;
    int ret;
    size_t total_read = 0;

    memset(buf, 0, BUF_SIZE);
    for (;;) {
        nread = 0;
        ret = SSL_read_ex(stream, &buf[total_read],
                          sizeof(buf) - total_read - 1, &nread);
        total_read += nread;
        if (ret <= 0) {
            ret = handle_io_failure(stream, ret);
            if (ret == 0) {
                /* EOF condition, fin bit set, we got the whole request */
                break;
            } else {
                /* permanent failure, abort */
                fprintf(stderr, "Failure on stream\n");
                return;
            }
        }
    }

    /* We should have a valid http 0.9 GET request here */
    fprintf(stderr, "Request is %s\n", req);

    /* Look for the last '/' char in the request */
    reqname = strrchr(req, '/');
    if (reqname == NULL)
        return;
    reqname++;

    /* Requests have a trailing \r\n, eliminate them */
    creturn = strchr(reqname, '\r');
    if (creturn != NULL)
        *creturn = '\0';

    snprintf(path, BUF_SIZE, "%s/%s", fileprefix, reqname);

    fprintf(stderr, "Serving %s\n", path);
    readbio = BIO_new_file(path, "r");
    if (readbio == NULL) {
        fprintf(stderr, "Unable to open %s\n", path);
        ERR_print_errors_fp(stderr);
        goto done;
    }

    /* Read the readbio file into a buffer, and just send it to the requestor */
    while (BIO_eof(readbio) <= 0) {
        bytes_read = 0;
        if (!BIO_read_ex(readbio, buf, BUF_SIZE, &bytes_read)) {
            if (BIO_eof(readbio) <= 0) {
                fprintf(stderr, "Failed to read from %s\n", path);
                ERR_print_errors_fp(stderr);
                goto out;
            } else {
                break;
            }
        }

        offset = 0;
        for (;;) {
            bytes_written = 0;
            rc = SSL_write_ex(stream, &buf[offset], bytes_read, &bytes_written);
            if (rc <= 0) {
                rc = SSL_get_error(stream, rc);
                switch (rc) {
                case SSL_ERROR_WANT_WRITE:
                    fprintf(stderr, "Send buffer full, retrying\n");
                    continue;
                    break;
                default:
                    fprintf(stderr, "Unhandled error cause %d\n", rc);
                    goto done;
                    break;
                }
            }
            bytes_read -= bytes_written;
            offset += bytes_written;
            bytes_written = 0;
            if (bytes_read == 0)
                break;
        }
    }

done:
    if (!SSL_stream_conclude(stream, 0))
        fprintf(stderr, "Failed to conclude stream\n");

out:
    BIO_free(readbio);
    return;
}

/**
 * @brief Runs the QUIC server to accept and handle client connections.
 *
 * This function initializes a QUIC listener, binds it to the provided UDP
 * socket, and enters a loop to accept client connections and process incoming
 * QUIC streams. Each connection is handled until termination, and streams are
 * processed individually using the `process_new_stream` function.
 *
 * @param ctx Pointer to the SSL_CTX object configured for QUIC.
 * @param sock  BIO of the bound UDP socket.
 *
 * @return Returns 0 on error; otherwise, the server runs indefinitely.
 *
 * Operation:
 * - Creates a QUIC listener using the provided SSL_CTX and associates it
 *   with the specified UDP socket.
 * - Waits for incoming QUIC connections and accepts them.
 * - For each connection:
 *   - Accepts incoming streams.
 *   - Processes each stream using `process_new_stream`.
 *   - Shuts down the connection upon completion.
 *
 * Error Handling:
 * - If listener creation or connection acceptance fails, the function logs
 *   an error message and exits the loop.
 * - Cleans up allocated resources (e.g., listener, connection) on failure.
 *
 * Usage:
 * - Call this function in the main server loop after setting up the
 *   SSL_CTX and binding a UDP socket.
 *
 * Notes:
 * - Uses blocking operations for listener, connection, and stream handling.
 * - Incoming streams are processed based on the configured stream policy.
 * - The server runs in an infinite loop unless a fatal error occurs.
 */
static int run_quic_server(SSL_CTX *ctx, BIO *sock)
{
    int ok = 0;
    SSL *listener, *conn, *stream;
    unsigned long errcode;
    uint64_t flags = 0;

    /*
     * If NO_ADDR_VALIDATE exists in our environment
     * then disable address validation on our listener
     */
    if (getenv("NO_ADDR_VALIDATE") != NULL)
        flags |= SSL_LISTENER_FLAG_NO_VALIDATE;

    /*
     * Create a new QUIC listener. Listeners, and other QUIC objects, default
     * to operating in blocking mode. The configured behaviour is inherited by
     * child objects.
     */
    if ((listener = SSL_new_listener(ctx, flags)) == NULL)
        goto err;

    /* Provide the listener with our UDP socket. */
    SSL_set_bio(listener, sock, sock);

    /* Begin listening. */
    if (!SSL_listen(listener))
        goto err;

    /*
     * Begin an infinite loop of listening for connections. We will only
     * exit this loop if we encounter an error.
     */
    for (;;) {
        /* Pristine error stack for each new connection */
        ERR_clear_error();

        /* Block while waiting for a client connection */
        printf("Waiting for connection\n");
        conn = SSL_accept_connection(listener, 0);
        if (conn == NULL) {
            fprintf(stderr, "error while accepting connection\n");
            goto err;
        }
        printf("Accepted new connection\n");

        /*
         * QUIC requires that we inform the connection that
         * we always want to accept new streams, rather than reject them
         * Additionally, while we don't make an explicit call here, we
         * are using the default stream mode, as would be specified by
         * a call to SSL_set_default_stream_mode
         */
        if (!SSL_set_incoming_stream_policy(conn,
                                            SSL_INCOMING_STREAM_POLICY_ACCEPT,
                                            0)) {
            fprintf(stderr, "Failed to set incomming stream policy\n");
            goto close_conn;
        }

        /*
         * Until the connection is closed, accept incomming stream
         * requests and serve them
         */
        for (;;) {
            /*
             * Note that SSL_accept_stream is blocking here, as the
             * conn SSL object inherited the deafult blocking property
             * from its parent, the listener SSL object.  As such there
             * is no need to handle retry failures here.
             */
            stream = SSL_accept_stream(conn, 0);
            if (stream == NULL) {
                /*
                 * If we don't get a stream, either we
                 * Hit a legitimate error, and should bail out
                 * or
                 * The Client closed the connection, and there are no
                 * more incomming streams expected
                 *
                 * Filter on the shutdown error, and only print an error
                 * message if the cause is not SHUTDOWN
                 */
                ERR_print_errors_fp(stderr);
                errcode = ERR_get_error();
                if (ERR_GET_REASON(errcode) != SSL_R_PROTOCOL_IS_SHUTDOWN)
                    fprintf(stderr, "Failure in accept stream, error %s\n",
                            ERR_reason_error_string(errcode));
                break;
            }
            process_new_stream(stream);
            SSL_free(stream);
        }

        /*
         * Shut down the connection. We may need to call this multiple times
         * to ensure the connection is shutdown completely.
         */
close_conn:
        while (SSL_shutdown(conn) != 1)
            continue;

        SSL_free(conn);
    }

err:
    SSL_free(listener);
    return ok;
}

/**
 * @brief Entry point for the minimal QUIC HTTP/0.9 server.
 *
 * This function initializes the server, sets up a QUIC context, binds a UDP
 * socket to the specified port, and starts the main QUIC server loop to handle
 * client connections and requests.
 *
 * @param argc Number of command-line arguments.
 * @param argv Array of command-line arguments:
 *             - argv[0]: Program name.
 *             - argv[1]: Port number to bind the server.
 *             - argv[2]: Path to the server's certificate file (PEM format).
 *             - argv[3]: Path to the server's private key file (PEM format).
 *
 * @return Returns EXIT_SUCCESS on successful execution, or EXIT_FAILURE
 *         on error.
 *
 * Operation:
 * - Validates the command-line arguments.
 * - Reads the FILEPREFIX environment variable to set the file prefix for
 *   serving files (default is "./downloads").
 * - Creates an SSL_CTX with QUIC support using the provided certificate and
 *   key files.
 * - Parses and validates the port number.
 * - Creates and binds a UDP socket to the specified port.
 * - Starts the server loop using `run_quic_server` to accept and process
 *   client connections.
 *
 * Error Handling:
 * - If any initialization step fails (e.g., invalid arguments, socket
 *   creation, context setup), appropriate error messages are logged, and
 *   the program exits with EXIT_FAILURE.
 *
 * Usage:
 * - Run the program with the required arguments to start the server:
 *   `./server <port> <server.crt> <server.key>`
 *
 * Notes:
 * - Ensure that the certificate and key files exist and are valid.
 * - The server serves files from the directory specified by FILEPREFIX.
 */
int main(int argc, char *argv[])
{
    int res = EXIT_FAILURE;
    SSL_CTX *ctx = NULL;
    BIO *sock = NULL;
    unsigned long port;

    if (argc != 4) {
        fprintf(stderr, "usage: %s <port> <server.crt> <server.key>\n", argv[0]);
        goto out;
    }

    fileprefix = getenv("FILEPREFIX");
    if (fileprefix == NULL)
        fileprefix = "./downloads";

    fprintf(stderr, "Fileprefix is %s\n", fileprefix);

    /* Create SSL_CTX that supports QUIC. */
    if ((ctx = create_ctx(argv[2], argv[3])) == NULL) {
        ERR_print_errors_fp(stderr);
        fprintf(stderr, "Failed to create context\n");
        goto out;
    }

    /* Parse port number from command line arguments. */
    port = strtoul(argv[1], NULL, 0);
    if (port == 0 || port > UINT16_MAX) {
        fprintf(stderr, "Failed to parse port number\n");
        goto out;
    }
    fprintf(stderr, "Binding to port %lu\n", port);

    /* Create and bind a UDP socket. */
    if ((sock = create_socket((uint16_t)port)) == NULL) {
        ERR_print_errors_fp(stderr);
        fprintf(stderr, "Failed to create socket\n");
        goto out;
    }

    /* QUIC server connection acceptance loop. */
    if (!run_quic_server(ctx, sock)) {
        ERR_print_errors_fp(stderr);
        fprintf(stderr, "Failed to run quic server\n");
        goto out;
    }

    res = EXIT_SUCCESS;
out:
    /* Free resources. */
    SSL_CTX_free(ctx);
    BIO_free(sock);
    return res;
}
