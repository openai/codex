/*
 * Copyright 2024-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_SSL_UNWRAP_H
# define OSSL_SSL_UNWRAP_H

# include <openssl/ssl.h>
# include "internal/quic_predef.h"

# define SSL_CONNECTION_FROM_SSL_ONLY_int(ssl, c) \
    ((ssl) == NULL ? NULL                         \
     : ((ssl)->type == SSL_TYPE_SSL_CONNECTION    \
       ? (c SSL_CONNECTION *)(ssl)                \
       : NULL))
# define SSL_CONNECTION_NO_CONST
# define SSL_CONNECTION_FROM_SSL_ONLY(ssl) \
    SSL_CONNECTION_FROM_SSL_ONLY_int(ssl, SSL_CONNECTION_NO_CONST)
# define SSL_CONNECTION_FROM_CONST_SSL_ONLY(ssl) \
    SSL_CONNECTION_FROM_SSL_ONLY_int(ssl, const)
# define SSL_CONNECTION_GET_CTX(sc) ((sc)->ssl.ctx)
# define SSL_CONNECTION_GET_SSL(sc) (&(sc)->ssl)
# define SSL_CONNECTION_GET_USER_SSL(sc) ((sc)->user_ssl)
# ifndef OPENSSL_NO_QUIC
struct ssl_connection_st *ossl_quic_obj_get0_handshake_layer(QUIC_OBJ *obj);
#  define SSL_CONNECTION_FROM_SSL_int(ssl, c)                                           \
    ((ssl) == NULL ? NULL                                                               \
     : ((ssl)->type == SSL_TYPE_SSL_CONNECTION                                          \
        ? (c SSL_CONNECTION *)(ssl)                                                     \
        : (SSL_TYPE_IS_QUIC((ssl)->type)                                                \
          ? (c SSL_CONNECTION *)ossl_quic_obj_get0_handshake_layer((QUIC_OBJ *)(ssl))   \
          : NULL)))
#  define SSL_CONNECTION_FROM_SSL(ssl) \
    SSL_CONNECTION_FROM_SSL_int(ssl, SSL_CONNECTION_NO_CONST)
#  define SSL_CONNECTION_FROM_CONST_SSL(ssl) \
    SSL_CONNECTION_FROM_SSL_int(ssl, const)
# else
#  define SSL_CONNECTION_FROM_SSL(ssl) \
    SSL_CONNECTION_FROM_SSL_ONLY_int(ssl, SSL_CONNECTION_NO_CONST)
#  define SSL_CONNECTION_FROM_CONST_SSL(ssl) \
    SSL_CONNECTION_FROM_SSL_ONLY_int(ssl, const)
# endif

# ifndef OPENSSL_NO_QUIC

#  define IS_QUIC_METHOD(m) \
    ((m) == OSSL_QUIC_client_method() || \
     (m) == OSSL_QUIC_client_thread_method() || \
     (m) == OSSL_QUIC_server_method())

#  define IS_QUIC_CTX(ctx)          IS_QUIC_METHOD((ctx)->method)

#  define QUIC_CONNECTION_FROM_SSL_int(ssl, c)   \
     ((ssl) == NULL ? NULL                       \
      : ((ssl)->type == SSL_TYPE_QUIC_CONNECTION \
         ? (c QUIC_CONNECTION *)(ssl)            \
         : NULL))

#  define QUIC_XSO_FROM_SSL_int(ssl, c)                             \
    ((ssl) == NULL                                                  \
     ? NULL                                                         \
     : (((ssl)->type == SSL_TYPE_QUIC_XSO                           \
        ? (c QUIC_XSO *)(ssl)                                       \
        : ((ssl)->type == SSL_TYPE_QUIC_CONNECTION                  \
           ? (c QUIC_XSO *)((QUIC_CONNECTION *)(ssl))->default_xso  \
           : NULL))))

#  define SSL_CONNECTION_FROM_QUIC_SSL_int(ssl, c)               \
     ((ssl) == NULL ? NULL                                       \
      : ((ssl)->type == SSL_TYPE_QUIC_CONNECTION                 \
         ? (c SSL_CONNECTION *)((c QUIC_CONNECTION *)(ssl))->tls \
         : NULL))

#  define QUIC_LISTENER_FROM_SSL_int(ssl, c)                            \
    ((ssl) == NULL                                                      \
     ? NULL                                                             \
     : ((ssl)->type == SSL_TYPE_QUIC_LISTENER                           \
        ? (c QUIC_LISTENER *)(ssl)                                      \
        : NULL))

#  define QUIC_DOMAIN_FROM_SSL_int(ssl, c)                              \
    ((ssl) == NULL                                                      \
     ? NULL                                                             \
     : ((ssl)->type == SSL_TYPE_QUIC_DOMAIN                             \
        ? (c QUIC_DOMAIN *)(ssl)                                        \
        : NULL))

#  define IS_QUIC_CS(ssl) ((ssl) != NULL                                \
                           && ((ssl)->type == SSL_TYPE_QUIC_CONNECTION  \
                               || (ssl)->type == SSL_TYPE_QUIC_XSO))

#  define IS_QUIC(ssl)                                                  \
    ((ssl) != NULL && SSL_TYPE_IS_QUIC((ssl)->type))

# else

#  define QUIC_CONNECTION_FROM_SSL_int(ssl, c) NULL
#  define QUIC_XSO_FROM_SSL_int(ssl, c) NULL
#  define QUIC_LISTENER_FROM_SSL_int(ssl, c) NULL
#  define SSL_CONNECTION_FROM_QUIC_SSL_int(ssl, c) NULL
#  define IS_QUIC(ssl) 0
#  define IS_QUIC_CS(ssl) 0
#  define IS_QUIC_CTX(ctx) 0
#  define IS_QUIC_METHOD(m) 0

# endif

# define QUIC_CONNECTION_FROM_SSL(ssl) \
    QUIC_CONNECTION_FROM_SSL_int(ssl, SSL_CONNECTION_NO_CONST)
# define QUIC_CONNECTION_FROM_CONST_SSL(ssl) \
    QUIC_CONNECTION_FROM_SSL_int(ssl, const)
# define QUIC_XSO_FROM_SSL(ssl) \
    QUIC_XSO_FROM_SSL_int(ssl, SSL_CONNECTION_NO_CONST)
# define QUIC_XSO_FROM_CONST_SSL(ssl) \
    QUIC_XSO_FROM_SSL_int(ssl, const)
# define QUIC_LISTENER_FROM_SSL(ssl) \
    QUIC_LISTENER_FROM_SSL_int(ssl, SSL_CONNECTION_NO_CONST)
# define QUIC_LISTENER_FROM_CONST_SSL(ssl) \
    QUIC_LISTENER_FROM_SSL_int(ssl, const)
# define SSL_CONNECTION_FROM_QUIC_SSL(ssl) \
    SSL_CONNECTION_FROM_QUIC_SSL_int(ssl, SSL_CONNECTION_NO_CONST)
# define SSL_CONNECTION_FROM_CONST_QUIC_SSL(ssl) \
    SSL_CONNECTION_FROM_CONST_QUIC_SSL_int(ssl, const)

#endif
