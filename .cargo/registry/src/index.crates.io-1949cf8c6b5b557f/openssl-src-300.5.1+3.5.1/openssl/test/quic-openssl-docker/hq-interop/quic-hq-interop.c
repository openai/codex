/*
 *  Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 *  Licensed under the Apache License 2.0 (the "License").  You may not use
 *  this file except in compliance with the License.  You can obtain a copy
 *  in the file LICENSE in the source distribution or at
 *  https://www.openssl.org/source/license.html
 */

/**
 * @file quic-hq-interop.c
 * @brief QUIC client interoperability demo using OpenSSL.
 *
 * This file contains the implementation of a QUIC client that demonstrates
 * interoperability with hq-interop servers. It handles connection setup,
 * session caching, key logging, and sending HTTP GET requests over QUIC.
 *
 * The file includes functions for setting up SSL/TLS contexts, managing session
 * caches, and handling I/O failures. It supports both IPv4 and IPv6 connections
 * and uses non-blocking mode for QUIC operations.
 *
 * The client sends HTTP/1.0 GET requests for specified files and saves the
 * responses to disk. It demonstrates OpenSSL's QUIC API, including ALPN protocol
 * settings, peer address management, and SSL handshake and shutdown processes.
 *
 * @note This client is intended for demonstration purposes and may require
 * additional error handling and robustness improvements for production use.
 *
 * USAGE
 * quic-hq-interop <host> <port> <reqfile>
 * host - The hostname of the server to contact
 * port - The port that the server is listening on
 * reqfile - a text file containing a space separated list of paths to fetch
 *           via http 1.0 GET requests
 *
 * ENVIRONMENT VARIABLES
 * SSLKEYLOGFILE - set to a file path to record keylog exchange with server
 * SSL_SESSION_FILE - set to a file path to record ssl sessions and restore
 *                    said sessions on next invocation
 * SSL_CERT_FILE - The ca file to use when validating a server
 * SSL_CIPHER_SUITES - The list of cipher suites to use (see openssl-ciphers)
 */
#include <string.h>

/* Include the appropriate header file for SOCK_DGRAM */
#ifdef _WIN32 /* Windows */
# include <winsock2.h>
#else /* Linux/Unix */
# include <sys/socket.h>
# include <sys/select.h>
#endif

#include <openssl/bio.h>
#include <openssl/ssl.h>
#include <openssl/err.h>

static int handle_io_failure(SSL *ssl, int res);

#define REQ_STRING_SZ 1024

/**
 * @brief A static pointer to a BIO object representing the session's BIO.
 *
 * This variable holds a reference to a BIO object used for network
 * communication during the session. It is initialized to NULL and should
 * be assigned a valid BIO object before use. The BIO object pointed to by
 * this variable may be used throughout the session for reading and writing
 * data.
 *
 * @note This variable is static, meaning it is only accessible within the
 * file in which it is declared.
 */
static BIO *session_bio = NULL;

/**
 * @brief Creates a BIO object for a UDP socket connection to a server.
 *
 * This function attempts to create a UDP socket and connect it to the server
 * specified by the given hostname and port. The socket is wrapped in a BIO
 * object, which allows for network communication via OpenSSL's BIO API.
 * The function also returns the address of the connected peer.
 *
 * @param hostname The hostname of the server to connect to.
 * @param port The port number of the server to connect to.
 * @param peer_addr A pointer to a BIO_ADDR pointer that will hold the address
 *                  of the connected peer on success. The caller is responsible
 *                  for freeing this memory using BIO_ADDR_free().
 * @return A pointer to a BIO object on success, or NULL on failure.
 *         The returned BIO object will be associated with the connected socket.
 *         If the BIO object is successfully created, it will take ownership of
 *         the socket and automatically close it when the BIO is freed.
 *
 * @note The function uses OpenSSL's socket-related functions (e.g., BIO_socket,
 *       BIO_connect) or portability and to integrate with OpenSSL's error
 *       handling mechanisms.
 * @note If no valid connection is established, the function returns NULL and
 *       ensures that any resources allocated during the process are properly
 *       freed.
 */
static BIO *create_socket_bio(const char *hostname, const char *port,
                              BIO_ADDR **peer_addr)
{
    int sock = -1;
    BIO_ADDRINFO *res;
    const BIO_ADDRINFO *ai = NULL;
    BIO *bio;

    /*
     * Lookup IP address info for the server.
     */
    if (!BIO_lookup_ex(hostname, port, BIO_LOOKUP_CLIENT, AF_UNSPEC, SOCK_DGRAM,
                       0, &res))
        return NULL;

    /*
     * Loop through all the possible addresses for the server and find one
     * we can connect to.
     */
    for (ai = res; ai != NULL; ai = BIO_ADDRINFO_next(ai)) {
        /*
         * Create a UDP socket. We could equally use non-OpenSSL calls such
         * as "socket" here for this and the subsequent connect and close
         * functions. But for portability reasons and also so that we get
         * errors on the OpenSSL stack in the event of a failure we use
         * OpenSSL's versions of these functions.
         */
        sock = BIO_socket(BIO_ADDRINFO_family(ai), SOCK_DGRAM, 0, 0);
        if (sock == -1)
            continue;

        /* Connect the socket to the server's address */
        if (!BIO_connect(sock, BIO_ADDRINFO_address(ai), 0)) {
            BIO_closesocket(sock);
            sock = -1;
            continue;
        }

        /*
         * Set to nonblocking mode
         * Note: This function returns a range of errors
         * <= 0 if something goes wrong, so catch them all here
         */
        if (BIO_socket_nbio(sock, 1) <= 0) {
            BIO_closesocket(sock);
            sock = -1;
            continue;
        }

        break;
    }

    if (sock != -1) {
        *peer_addr = BIO_ADDR_dup(BIO_ADDRINFO_address(ai));
        if (*peer_addr == NULL) {
            BIO_closesocket(sock);
            return NULL;
        }
    }

    /* Free the address information resources we allocated earlier */
    BIO_ADDRINFO_free(res);

    /* If sock is -1 then we've been unable to connect to the server */
    if (sock == -1)
        return NULL;

    /* Create a BIO to wrap the socket */
    bio = BIO_new(BIO_s_datagram());
    if (bio == NULL) {
        BIO_closesocket(sock);
        return NULL;
    }

    /*
     * Associate the newly created BIO with the underlying socket. By
     * passing BIO_CLOSE here the socket will be automatically closed when
     * the BIO is freed. Alternatively you can use BIO_NOCLOSE, in which
     * case you must close the socket explicitly when it is no longer
     * needed.
     */
    if (BIO_set_fd(bio, sock, BIO_CLOSE) <= 0) {
        BIO_closesocket(sock);
        BIO_free(bio);
        return NULL;
    }

    return bio;
}

/**
 * @brief Waits for activity on the SSL socket, either for reading or writing.
 *
 * This function monitors the underlying file descriptor of the given SSL
 * connection to determine when it is ready for reading or writing, or both.
 * It uses the select function to wait until the socket is either readable
 * or writable, depending on what the SSL connection requires.
 *
 * @param ssl A pointer to the SSL object representing the connection.
 *
 * @note This function blocks until there is activity on the socket or
 * until the timeout specified by OpenSSL is reached. In a real application,
 * you might want to perform other tasks while waiting, such as updating a
 * GUI or handling other connections.
 *
 * @note This function uses select for simplicity and portability. Depending
 * on your application's requirements, you might consider using other
 * mechanisms like poll or epoll for handling multiple file descriptors.
 */
static void wait_for_activity(SSL *ssl)
{
    fd_set wfds, rfds;
    int width, sock, isinfinite;
    struct timeval tv;
    struct timeval *tvp = NULL;

    /* Get hold of the underlying file descriptor for the socket */
    sock = SSL_get_fd(ssl);

    FD_ZERO(&wfds);
    FD_ZERO(&rfds);

    /*
     * Find out if we would like to write to the socket, or read from it (or
     * both)
     */
    if (SSL_net_write_desired(ssl))
        FD_SET(sock, &wfds);
    if (SSL_net_read_desired(ssl))
        FD_SET(sock, &rfds);
    width = sock + 1;

    /*
     * Find out when OpenSSL would next like to be called, regardless of
     * whether the state of the underlying socket has changed or not.
     */
    if (SSL_get_event_timeout(ssl, &tv, &isinfinite) && !isinfinite)
        tvp = &tv;

    /*
     * Wait until the socket is writeable or readable. We use select here
     * for the sake of simplicity and portability, but you could equally use
     * poll/epoll or similar functions
     *
     * NOTE: For the purposes of this demonstration code this effectively
     * makes this demo block until it has something more useful to do. In a
     * real application you probably want to go and do other work here (e.g.
     * update a GUI, or service other connections).
     *
     * Let's say for example that you want to update the progress counter on
     * a GUI every 100ms. One way to do that would be to use the timeout in
     * the last parameter to "select" below. If the tvp value is greater
     * than 100ms then use 100ms instead. Then, when select returns, you
     * check if it did so because of activity on the file descriptors or
     * because of the timeout. If the 100ms GUI timeout has expired but the
     * tvp timeout has not then go and update the GUI and then restart the
     * "select" (with updated timeouts).
     */

    select(width, &rfds, &wfds, NULL, tvp);
}

/**
 * @brief Handles I/O failures on an SSL connection based on the result code.
 *
 * This function processes the result of an SSL I/O operation and handles
 * different types of errors that may occur during the operation. It takes
 * appropriate actions such as retrying the operation, reporting errors, or
 * returning specific status codes based on the error type.
 *
 * @param ssl A pointer to the SSL object representing the connection.
 * @param res The result code from the SSL I/O operation.
 * @return An integer indicating the outcome:
 *         - 1: Temporary failure, the operation should be retried.
 *         - 0: EOF, indicating the connection has been closed.
 *         - -1: A fatal error occurred or the connection has been reset.
 *
 * @note This function may block if a temporary failure occurs and
 * wait_for_activity() is called.
 *
 * @note If the failure is due to an SSL verification error, additional
 * information will be logged to stderr.
 */
static int handle_io_failure(SSL *ssl, int res)
{
    switch (SSL_get_error(ssl, res)) {
    case SSL_ERROR_WANT_READ:
    case SSL_ERROR_WANT_WRITE:
        /* Temporary failure. Wait until we can read/write and try again */
        wait_for_activity(ssl);
        return 1;

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
 * @brief A static integer indicating whether the session is cached.
 *
 * This variable is used to track the state of session caching. It is
 * initialized to 0, meaning no session is cached. The value may be updated
 * to indicate that a session has been successfully cached.
 *
 * @note This variable is static, meaning it is only accessible within the
 * file in which it is declared.
 */
static int session_cached = 0;

/**
 * @brief Caches a new SSL session if one is not already cached.
 *
 * This function writes a new SSL session to the session BIO and caches it.
 * It ensures that only one session is cached at a time by checking the
 * `session_cached` flag. If a session is already cached, the function
 * returns without caching the new session.
 *
 * @param ssl A pointer to the SSL object associated with the session.
 * @param sess A pointer to the SSL_SESSION object to be cached.
 * @return 1 if the session is successfully cached, 0 otherwise.
 *
 * @note This function only allows one session to be cached. Subsequent
 * sessions will not be cached unless `session_cached` is reset.
 */
static int cache_new_session(struct ssl_st *ssl, SSL_SESSION *sess)
{

    if (session_cached == 1)
        return 0;

    /* Just write the new session to our bio */
    if (!PEM_write_bio_SSL_SESSION(session_bio, sess))
        return 0;

    (void)BIO_flush(session_bio);
    /* only cache one session */
    session_cached = 1;
    return 1;
}

/**
 * @brief Sets up the session cache for the SSL connection.
 *
 * This function configures session caching for the given SSL connection
 * and context. It attempts to load a session from the specified cache file
 * or creates a new one if the file does not exist. It also configures the
 * session cache mode and disables stateless session tickets.
 *
 * @param ssl A pointer to the SSL object for the connection.
 * @param ctx A pointer to the SSL_CTX object representing the context.
 * @param filename The name of the file used to store the session cache.
 * @return 1 on success, 0 on failure.
 *
 * @note If the cache file does not exist, a new file is created and the
 * session cache is initialized. If a session is successfully loaded from
 * the file, it is added to the context and set for the SSL connection.
 * If an error occurs during setup, the session BIO is freed.
 */
static int setup_session_cache(SSL *ssl, SSL_CTX *ctx, const char *filename)
{
    SSL_SESSION *sess = NULL;
    int rc = 0;
    int new_cache = 0;

    /* 
     * Because we cache sessions to a file in this client, we don't 
     * actualy need to internally store sessions, because we restore them
     * from the file with SSL_set_session below, but we want to ensure
     * that caching is enabled so that the session cache callbacks get called
     * properly.  The documentation is a bit unclear under what conditions
     * the callback is made, so play it safe here, by enforcing enablement
     */
    if (!SSL_CTX_set_session_cache_mode(ctx, SSL_SESS_CACHE_CLIENT |
                                             SSL_SESS_CACHE_NO_INTERNAL_STORE |
                                             SSL_SESS_CACHE_NO_AUTO_CLEAR))
        return rc;

    /* open our cache file */
    session_bio = BIO_new_file(filename, "r+");
    if (session_bio == NULL) {
        /* file might need to be created */
        session_bio = BIO_new_file(filename, "w+");
        if (session_bio == NULL)
            return rc;
        new_cache = 1;
    }

    if (new_cache == 0) {
        /* read in our cached session */
        if (PEM_read_bio_SSL_SESSION(session_bio, &sess, NULL, NULL)) {
            /* set our session */
            if (!SSL_set_session(ssl, sess))
                goto err;
        }
    } else {
        /* Set the callback to store new sessions */
        SSL_CTX_sess_set_new_cb(ctx, cache_new_session);
    }

    rc = 1;

err:
    if (rc == 0)
        BIO_free(session_bio);
    return rc;
}

/**
 * @brief Pointer to a list of SSL polling items.
 *
 * This static variable holds the reference to a dynamically allocated list
 * of SSL_POLL_ITEM structures used for SSL polling operations. It is
 * initialized to NULL and will be populated as needed.
 */
static SSL_POLL_ITEM *poll_list = NULL;

/**
 * @brief Pointer to an array of BIO objects for output.
 *
 * This static variable holds the reference to a dynamically allocated array
 * of BIO structures, which are used for handling output in SSL operations.
 * It is initialized to NULL and will be set when needed.  This array holds
 * the out bio's for all received data from GET requests
 */
static BIO **outbiolist = NULL;

/**
 * @brief Pointer to an array of output names.
 *
 * This static variable holds the reference to a dynamically allocated array
 * of strings, representing output names. It is initialized to NULL and
 * populated as required during operation.  This array holds the names of the
 * output files from http GET requests.  Indicies are correlated with the
 * corresponding outbiolist and poll_list arrays
 */
static char **outnames = NULL;

/**
 * @brief Counter for the number of poll items.
 *
 * This static variable holds the count of items in the poll_list. It is
 * initialized to 0 and updated as items are added or removed from the list.
 */
static size_t poll_count = 0;

/**
 * @brief Pointer to an array of request strings.
 *
 * This static variable holds the reference to a dynamically allocated array
 * of strings, representing requests. It is initialized to NULL and populated
 * as requests are added during execution.
 */
static char **req_array = NULL;

/**
 * @brief Counter for the total number of requests.
 *
 * This static variable tracks the total number of parsed from reqfile. It is
 * initialized to 0 and incremented as new requests are processed.
 */
static size_t total_requests = 0;

/**
 * @brief Index for the current request in the request array.
 *
 * This static variable keeps track of the index of the current request being
 * processed in the request array. It is initialized to 0 and updated as
 * requests are handled.
 */
static size_t req_idx = 0;

/**
 * @brief Builds and processes a set of SSL poll requests.
 *
 * This function creates a new set of SSL poll requests based on the current
 * request array. It allocates and manages memory for poll lists, BIO output
 * files, and associated request names. Each request sends an HTTP GET to the
 * corresponding peer. The function processes the requests until a batch limit
 * or error is encountered.
 *
 * @param ssl A pointer to the SSL object to use for creating new streams.
 *
 * @return The number of poll requests successfully built, or 0 on error.
 */
static size_t build_request_set(SSL *ssl)
{
    size_t poll_idx;
    char *req;
    char outfilename[REQ_STRING_SZ];
    char req_string[REQ_STRING_SZ];
    SSL *new_stream;
    size_t written;
    unsigned long error;
    size_t retry_count;

    /*
     * Free any previous poll list
     */
    for (poll_idx = 0; poll_idx < poll_count; poll_idx++) {
        (void)BIO_flush(outbiolist[poll_idx]);
        BIO_free(outbiolist[poll_idx]);
        SSL_free(poll_list[poll_idx].desc.value.ssl);
    }

    /*
     * Reset out lists and poll_count
     */
    OPENSSL_free(outbiolist);
    OPENSSL_free(outnames);
    OPENSSL_free(poll_list);
    outnames = NULL;
    poll_list = NULL;
    outbiolist = NULL;

    poll_count = 0;

    /*
     * Iterate through our parsed lists of requests
     * note req_idx may start at a non-zero value if
     * multiple calls to build_request_list are made
     */
    while (req_idx < total_requests) {
        req = req_array[req_idx];
        /* Up our poll count and set our poll_list index */
        poll_count++;
        poll_idx = poll_count - 1;

        /*
         * Expand our poll_list, outbiolist, and outnames arrays
         */
        poll_list = OPENSSL_realloc(poll_list,
                                    sizeof(SSL_POLL_ITEM) * poll_count);
        if (poll_list == NULL) {
            fprintf(stderr, "Unable to realloc poll_list\n");
            goto err;
        }

        outbiolist = OPENSSL_realloc(outbiolist,
                                     sizeof(BIO *) * poll_count);
        if (outbiolist == NULL) {
            fprintf(stderr, "Unable to realloc outbiolist\n");
            goto err;
        }

        outnames = OPENSSL_realloc(outnames, sizeof(char *) * poll_count);
        if (outnames == NULL) {
            fprintf(stderr, "Unable to realloc outnames\n");
            goto err;
        }

        /* set the output file name for this index */
        outnames[poll_idx] = req;

        /* Format the http request */
        BIO_snprintf(req_string, REQ_STRING_SZ, "GET /%s\r\n", req);

        /* build the outfile request path */
        memset(outfilename, 0, REQ_STRING_SZ);
        BIO_snprintf(outfilename, REQ_STRING_SZ, "/downloads/%s", req);

        /* open a bio to write the file */
        outbiolist[poll_idx] = BIO_new_file(outfilename, "w+");
        if (outbiolist[poll_idx] == NULL) {
            fprintf(stderr, "Failed to open outfile %s\n", outfilename);
            goto err;
        }

        /* create a request stream */
        new_stream = NULL;

        /*
         * NOTE: We are doing groups of 25 because thats 1/4 of the initial max
         * stream count that most servers advertise.  This gives the server an
         * opportunity to send us updated MAX_STREAM frames to extend our stream
         * allotment before we run out, which many servers defer doing.
         */
        if (poll_count <= 25) {
            for (retry_count = 0; retry_count < 10; retry_count++) {
                ERR_clear_error();
                new_stream = SSL_new_stream(ssl, 0);
                if (new_stream == NULL
                    && (error = ERR_get_error()) != 0
                    && ERR_GET_REASON(error) == SSL_R_STREAM_COUNT_LIMITED) {
                    /*
                     * Kick the SSL state machine in the hopes that
                     * the server has a MAX_STREAM frame for us to process
                     */
                    fprintf(stderr, "Stream limit reached, retrying\n");
                    SSL_handle_events(ssl);
                    continue;
                }
                break;
            }
        }

        if (new_stream == NULL) {
            /*
             * We ran out of new streams to allocate
             * return and process this batch before getting more
             */
            poll_count--;
            return poll_count;
        }

        /*
         * Create a poll descriptor for this stream
         */
        poll_list[poll_idx].desc = SSL_as_poll_descriptor(new_stream);
        poll_list[poll_idx].revents = 0;
        poll_list[poll_idx].events = SSL_POLL_EVENT_R;

        /* Write an HTTP GET request to the peer */
        while (!SSL_write_ex2(poll_list[poll_idx].desc.value.ssl,
                              req_string, strlen(req_string),
                              SSL_WRITE_FLAG_CONCLUDE, &written)) {
            if (handle_io_failure(poll_list[poll_idx].desc.value.ssl, 0) == 1)
                continue; /* Retry */
            fprintf(stderr, "Failed to write start of HTTP request\n");
            goto err; /* Cannot retry: error */
        }

        req_idx++;
    }
    return poll_count;

err:
    for (poll_idx = 0; poll_idx < poll_count; poll_idx++) {
        BIO_free(outbiolist[poll_idx]);
        SSL_free(poll_list[poll_idx].desc.value.ssl);
    }
    OPENSSL_free(poll_list);
    OPENSSL_free(outbiolist);
    poll_list = NULL;
    outbiolist = NULL;
    poll_count = 0;
    return poll_count;
}

/**
 * @brief Static pointer to a BIO_ADDR structure representing the peer's address.
 *
 * This variable is used to store the address of a peer for network communication.
 * It is statically allocated and should be initialized appropriately.
 */
static BIO_ADDR *peer_addr = NULL;

/**
 * @brief Set up a TLS/QUIC connection to the specified hostname and port.
 *
 * This function creates and configures an SSL context for a client connection
 * using the QUIC client method. It sets up the necessary certificates,
 * performs host verification, configures ALPN, and establishes a non-blocking
 * connection.
 *
 * @param hostname Hostname to connect to.
 * @param port Port to connect to.
 * @param ctx Pointer to an SSL_CTX object, which will be created.
 * @param ssl Pointer to an SSL object, which will be created.
 *
 * @return Returns 0 on success, 1 on error.
 */
static int setup_connection(char *hostname, char *port,
                            SSL_CTX **ctx, SSL **ssl)
{
    unsigned char alpn[] = {10, 'h', 'q', '-', 'i', 'n', 't', 'e', 'r', 'o', 'p'};
    int ret = 0;
    BIO *bio = NULL;

    /*
     * Create an SSL_CTX which we can use to create SSL objects from. We
     * want an SSL_CTX for creating clients so we use
     * OSSL_QUIC_client_method() here.
     */
    *ctx = SSL_CTX_new(OSSL_QUIC_client_method());
    if (*ctx == NULL) {
        fprintf(stderr, "Failed to create the SSL_CTX\n");
        goto end;
    }

    /*
     * Configure the client to abort the handshake if certificate
     * verification fails. Virtually all clients should do this unless you
     * really know what you are doing.
     */
    SSL_CTX_set_verify(*ctx, SSL_VERIFY_PEER, NULL);

    /*
     * Use the default trusted certificate store
     * Note: The store is read from SSL_CERT_DIR and SSL_CERT_FILE
     * environment variables in the default case, so users can set those
     * When running this application to direct where the store is loaded from
     */
    if (!SSL_CTX_set_default_verify_paths(*ctx)) {
        fprintf(stderr, "Failed to set the default trusted certificate store\n");
        goto end;
    }

    /*
     * If the SSL_CIPHER_SUITES env variable is set, assign those
     * ciphers to the context
     */
    if (getenv("SSL_CIPHER_SUITES") != NULL) {
        if (!SSL_CTX_set_ciphersuites(*ctx, getenv("SSL_CIPHER_SUITES"))) {
            fprintf(stderr, "Failed to set cipher suites for connection\n");
            goto end;
        }
    }

    /* Create an SSL object to represent the TLS connection */
    *ssl = SSL_new(*ctx);
    if (*ssl == NULL) {
        fprintf(stderr, "Failed to create the SSL object\n");
        goto end;
    }

    if (getenv("SSL_SESSION_FILE") != NULL) {
        if (!setup_session_cache(*ssl, *ctx, getenv("SSL_SESSION_FILE"))) {
            fprintf(stderr, "Unable to setup session cache\n");
            goto end;
        }
    }

    /*
     * Create the underlying transport socket/BIO and associate it with the
     * connection.
     */
    bio = create_socket_bio(hostname, port, &peer_addr);
    if (bio == NULL) {
        fprintf(stderr, "Failed to crete the BIO\n");
        goto end;
    }
    SSL_set_bio(*ssl, bio, bio);

    /*
     * Tell the server during the handshake which hostname we are attempting
     * to connect to in case the server supports multiple hosts.
     */
    if (!SSL_set_tlsext_host_name(*ssl, hostname)) {
        fprintf(stderr, "Failed to set the SNI hostname\n");
        goto end;
    }

    /*
     * Ensure we check during certificate verification that the server has
     * supplied a certificate for the hostname that we were expecting.
     * Virtually all clients should do this unless you really know what you
     * are doing.
     */
    if (!SSL_set1_host(*ssl, hostname)) {
        fprintf(stderr, "Failed to set the certificate verification hostname");
        goto end;
    }

    /* SSL_set_alpn_protos returns 0 for success! */
    if (SSL_set_alpn_protos(*ssl, alpn, sizeof(alpn)) != 0) {
        fprintf(stderr, "Failed to set the ALPN for the connection\n");
        goto end;
    }

    /* Set the IP address of the remote peer */
    if (!SSL_set1_initial_peer_addr(*ssl, peer_addr)) {
        fprintf(stderr, "Failed to set the initial peer address\n");
        goto end;
    }

    /*
     * The underlying socket is always nonblocking with QUIC, but the default
     * behaviour of the SSL object is still to block. We set it for nonblocking
     * mode in this demo.
     */
    if (!SSL_set_blocking_mode(*ssl, 0)) {
        fprintf(stderr, "Failed to turn off blocking mode\n");
        goto end;
    }

    /* Do the handshake with the server */
    while ((ret = SSL_connect(*ssl)) != 1) {
        if (handle_io_failure(*ssl, ret) == 1)
            continue; /* Retry */
        fprintf(stderr, "Failed to connect to server\n");
        goto end; /* Cannot retry: error */
    }

    return 1;
end:
    SSL_CTX_free(*ctx);
    SSL_free(*ssl);
    BIO_ADDR_free(peer_addr);
    *ctx = NULL;
    *ssl = NULL;
    peer_addr = NULL;
    return 0;
}

/**
 * @brief Entry point for the QUIC hq-interop client demo application.
 *
 * This function sets up an SSL/TLS connection using QUIC, sends HTTP GET
 * requests for files specified in the command-line arguments, and saves
 * the responses to disk. It handles various configurations such as session
 * caching, and key logging.
 *
 * @param argc The number of command-line arguments.
 * @param argv The array of command-line arguments. The expected format is
 *             "hostname port file".
 * @return EXIT_SUCCESS on success, or EXIT_FAILURE on error.
 *
 * @note The function performs the following main tasks:
 *       - Parses command-line arguments.
 *       - Reads the list of requests from the specified file.
 *       - Sets up the SSL context and configures certificate verification.
 *       - Optionally enables key logging and session caching.
 *       - Establishes a non-blocking QUIC connection to the server.
 *       - Sends an HTTP GET request for each file and writes the response
 *         to the corresponding output file.
 *       - Gracefully shuts down the SSL connection and frees resources.
 *       - Prints any OpenSSL error stack information on failure.
 */
int main(int argc, char *argv[])
{
    SSL_CTX *ctx = NULL;
    SSL *ssl = NULL;
    BIO *req_bio = NULL;
    int res = EXIT_FAILURE;
    int ret;
    size_t readbytes = 0;
    char buf[160];
    int eof = 0;
    int argnext = 1;
    char *reqfile = NULL;
    char *reqnames = OPENSSL_zalloc(1025);
    size_t read_offset = 0;
    size_t bytes_read = 0;
    size_t poll_idx = 0;
    size_t poll_done = 0;
    size_t result_count = 0;
    struct timeval poll_timeout;
    size_t this_poll_count = 0;
    char *req = NULL;
    char *hostname, *port;

    if (argc < 4) {
        fprintf(stderr, "Usage: quic-hq-interop hostname port reqfile\n");
        goto end;
    }

    hostname = argv[argnext++];
    port = argv[argnext++];
    reqfile = argv[argnext];

    req_bio = BIO_new_file(reqfile, "r");
    if (req_bio == NULL) {
        fprintf(stderr, "Failed to open request file %s\n", reqfile);
        goto end;
    }

    /* Get the list of requests */
    while (!BIO_eof(req_bio)) {
        if (!BIO_read_ex(req_bio, &reqnames[read_offset], REQ_STRING_SZ, &bytes_read)) {
            fprintf(stderr, "Failed to read some data from request file\n");
            goto end;
        }
        read_offset += bytes_read;
        reqnames = OPENSSL_realloc(reqnames, read_offset + REQ_STRING_SZ);
        if (reqnames == NULL) {
            fprintf(stderr, "Realloc failure\n");
            goto end;
        }
    }
    BIO_free(req_bio);
    req_bio = NULL;
    reqnames[read_offset + 1] = '\0';

    if (!setup_connection(hostname, port, &ctx, &ssl)) {
        fprintf(stderr, "Unable to establish connection\n");
        goto end;
    }

    req = strtok(reqnames, " ");

    while (req != NULL) {
        total_requests++;
        req_array = OPENSSL_realloc(req_array, sizeof(char *) * total_requests);
        if (req_array == NULL)
            goto end;
        req_array[total_requests - 1] = req;
        req = strtok(NULL, " ");
    }

    /* get a list of requests to poll */
    this_poll_count = build_request_set(ssl);

    /*
     * Now poll all our descriptors for events
     */
    while (this_poll_count != 0 && poll_done < this_poll_count) {
        result_count = 0;
        poll_timeout.tv_sec = 0;
        poll_timeout.tv_usec = 0;
        if (!SSL_poll(poll_list, this_poll_count, sizeof(SSL_POLL_ITEM),
                      &poll_timeout, 0, &result_count)) {
            fprintf(stderr, "Failed to poll\n");
            goto end;
        }

        /* Iterate over our poll array looking for ready SSL's */
        for (poll_idx = 0; poll_idx < this_poll_count; poll_idx++) {
            /*
             * If we have visited the number of SSL's that SSL_poll
             * indicated were ready, we can go poll again
             */
            if (result_count == 0)
                break;

            if (poll_list[poll_idx].revents == SSL_POLL_EVENT_R) {
                /*
                 * We found an SSL that we can read, drop our result count
                 */
                result_count--;

                /* And clear the revents for the next poll */
                poll_list[poll_idx].revents = 0;

                /*
                 * Get up to sizeof(buf) bytes of the response. We keep reading until
                 * the server closes the connection.
                 */
                eof = 0;

                /* Read our data, and handle any errors/eof conditions */
                if (!SSL_read_ex(poll_list[poll_idx].desc.value.ssl, buf,
                                 sizeof(buf), &readbytes)) {
                    switch (handle_io_failure(poll_list[poll_idx].desc.value.ssl,
                                              0)) {
                    case 1:
                        eof = 0;
                        break; /* Retry on next poll */
                    case 0:
                        eof = 1;
                        break;
                    case -1:
                    default:
                        fprintf(stderr, "Failed reading remaining data\n");
                        goto end; /* Cannot retry: error */
                    }
                }

                /*
                 * If error handling indicated that this SSL is in an EOF state
                 * we mark the SSL as not needing any more polling, and up our
                 * poll_done count.  Otherwise, just write to the outbio
                 */
                if (!eof) {
                    BIO_write(outbiolist[poll_idx], buf, readbytes);
                } else {
                    fprintf(stderr, "completed %s\n", outnames[poll_idx]);
                    /* This file is done, take it out of polling contention */
                    poll_list[poll_idx].events = 0;
                    poll_done++;
                }
            }
        }

        /*
         * If we've completed this poll set, try get another one
         */
        if (poll_done == this_poll_count) {
            this_poll_count = build_request_set(ssl);
            poll_done = 0;
        }
    }

    /*
     * Repeatedly call SSL_shutdown() until the connection is fully
     * closed.
     */
    fprintf(stderr, "Shutting down\n");
    while ((ret = SSL_shutdown(ssl)) != 1) {
        if (ret < 0 && handle_io_failure(ssl, ret) == 1)
            continue; /* Retry */
    }

    /* Success! */
    res = EXIT_SUCCESS;
 end:
    /*
     * If something bad happened then we will dump the contents of the
     * OpenSSL error stack to stderr. There might be some useful diagnostic
     * information there.
     */
    if (res == EXIT_FAILURE)
        ERR_print_errors_fp(stderr);

    /*
     * Free the resources we allocated. We do not free the BIO object here
     * because ownership of it was immediately transferred to the SSL object
     * via SSL_set_bio(). The BIO will be freed when we free the SSL object.
     */
    BIO_ADDR_free(peer_addr);
    OPENSSL_free(reqnames);
    BIO_free(session_bio);
    for (poll_idx = 0; poll_idx < poll_count; poll_idx++) {
        BIO_free(outbiolist[poll_idx]);
        SSL_free(poll_list[poll_idx].desc.value.ssl);
    }
    SSL_free(ssl);
    SSL_CTX_free(ctx);
    OPENSSL_free(outbiolist);
    OPENSSL_free(poll_list);
    return res;
}
