/*
 * Copyright 1995-2022 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_HTTP_SERVER_H
# define OSSL_HTTP_SERVER_H

# include "apps.h"
# include "log.h"

# ifndef HAVE_FORK
#  if defined(OPENSSL_SYS_VMS) || defined(OPENSSL_SYS_WINDOWS)
#   define HAVE_FORK 0
#  else
#   define HAVE_FORK 1
#  endif
# endif

# if HAVE_FORK
#  undef NO_FORK
# else
#  define NO_FORK
# endif

# if !defined(NO_FORK) && !defined(OPENSSL_NO_SOCK) \
    && !defined(OPENSSL_NO_POSIX_IO)
#  define HTTP_DAEMON
#  include <sys/types.h>
#  include <sys/wait.h>
#  include <signal.h>
#  define MAXERRLEN 1000 /* limit error text sent to syslog to 1000 bytes */
# endif

# ifndef OPENSSL_NO_SOCK
/*-
 * Initialize an HTTP server, setting up its listening BIO
 * prog: the name of the current app
 * port: the port to listen on
 * verbosity: the level of verbosity to use, or -1 for default: LOG_INFO
 * returns a BIO for accepting requests, NULL on error
 */
BIO *http_server_init(const char *prog, const char *port, int verbosity);

/*-
 * Accept an ASN.1-formatted HTTP request
 * it: the expected request ASN.1 type
 * preq: pointer to variable where to place the parsed request
 * ppath: pointer to variable where to place the request path, or NULL
 * pcbio: pointer to variable where to place the BIO for sending the response to
 * acbio: the listening bio (typically as returned by http_server_init_bio())
 * found_keep_alive: for returning flag if client requests persistent connection
 * prog: the name of the current app, for diagnostics only
 * accept_get: whether to accept GET requests (in addition to POST requests)
 * timeout: connection timeout (in seconds), or 0 for none/infinite
 * returns 0 in case caller should retry, then *preq == *ppath == *pcbio == NULL
 * returns -1 on fatal error; also then holds *preq == *ppath == *pcbio == NULL
 * returns 1 otherwise. In this case it is guaranteed that *pcbio != NULL while
 * *ppath == NULL and *preq == NULL if and only if the request is invalid,
 * On return value 1 the caller is responsible for sending an HTTP response,
 * using http_server_send_asn1_resp() or http_server_send_status().
 * The caller must free any non-NULL *preq, *ppath, and *pcbio pointers.
 */
int http_server_get_asn1_req(const ASN1_ITEM *it, ASN1_VALUE **preq,
                             char **ppath, BIO **pcbio, BIO *acbio,
                             int *found_keep_alive,
                             const char *prog, int accept_get, int timeout);

/*-
 * Send an ASN.1-formatted HTTP response
 * prog: the name of the current app, for diagnostics only
 * cbio: destination BIO (typically as returned by http_server_get_asn1_req())
 *       note: cbio should not do an encoding that changes the output length
 * keep_alive: grant persistent connection
 * content_type: string identifying the type of the response
 * it: the response ASN.1 type
 * resp: the response to send
 * returns 1 on success, 0 on failure
 */
int http_server_send_asn1_resp(const char *prog, BIO *cbio, int keep_alive,
                               const char *content_type,
                               const ASN1_ITEM *it, const ASN1_VALUE *resp);

/*-
 * Send a trivial HTTP response, typically to report an error or OK
 * prog: the name of the current app, for diagnostics only
 * cbio: destination BIO (typically as returned by http_server_get_asn1_req())
 * status: the status code to send
 * reason: the corresponding human-readable string
 * returns 1 on success, 0 on failure
 */
int http_server_send_status(const char *prog, BIO *cbio,
                            int status, const char *reason);

# endif

# ifdef HTTP_DAEMON
extern int n_responders;
extern int acfd;

void socket_timeout(int signum);
void spawn_loop(const char *prog);
# endif

#endif
