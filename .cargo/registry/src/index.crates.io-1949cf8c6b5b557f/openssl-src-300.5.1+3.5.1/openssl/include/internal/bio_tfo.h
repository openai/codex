/*
 * Copyright 2022-2023 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * Contains definitions for simplifying the use of TCP Fast Open
 * (RFC7413) in OpenSSL socket BIOs.
 */

/* If a supported OS is added here, update test/bio_tfo_test.c */
#if defined(TCP_FASTOPEN) && !defined(OPENSSL_NO_TFO)

# if defined(OPENSSL_SYS_MACOSX) || defined(__FreeBSD__)
#  include <sys/sysctl.h>
# endif

/*
 * OSSL_TFO_SYSCTL is used to determine if TFO is supported by
 * this kernel, and if supported, if it is enabled. This is more of
 * a problem on FreeBSD 10.3 ~ 11.4, where TCP_FASTOPEN was defined,
 * but not enabled by default in the kernel, and only for the server.
 * Linux does not have sysctlbyname(), and the closest equivalent
 * is to go into the /proc filesystem, but I'm not sure it's
 * worthwhile.
 *
 * On MacOS and Linux:
 * These operating systems use a single parameter to control TFO.
 * The OSSL_TFO_CLIENT_FLAG and OSSL_TFO_SERVER_FLAGS are used to
 * determine if TFO is enabled for the client and server respectively.
 *
 * OSSL_TFO_CLIENT_FLAG = 1 = client TFO enabled
 * OSSL_TFO_SERVER_FLAG = 2 = server TFO enabled
 *
 * Such that:
 * 0 = TFO disabled
 * 3 = server and client TFO enabled
 *
 * macOS 10.14 and later support TFO.
 * Linux kernel 3.6 added support for client TFO.
 * Linux kernel 3.7 added support for server TFO.
 * Linux kernel 3.13 enabled TFO by default.
 * Linux kernel 4.11 added the TCP_FASTOPEN_CONNECT option.
 *
 * On FreeBSD:
 * FreeBSD 10.3 ~ 11.4 uses a single sysctl for server enable.
 * FreeBSD 12.0 and later uses separate sysctls for server and
 * client enable.
 *
 * Some options are purposely NOT defined per-platform
 *
 * OSSL_TFO_SYSCTL
 *     Defined as a sysctlbyname() option to determine if
 *     TFO is enabled in the kernel (macOS, FreeBSD)
 *
 * OSSL_TFO_SERVER_SOCKOPT
 *     Defined to indicate the socket option used to enable
 *     TFO on a server socket (all)
 *
 * OSSL_TFO_SERVER_SOCKOPT_VALUE
 *     Value to be used with OSSL_TFO_SERVER_SOCKOPT
 *
 * OSSL_TFO_CONNECTX
 *     Use the connectx() function to make a client connection
 *     (macOS)
 *
 * OSSL_TFO_CLIENT_SOCKOPT
 *     Defined to indicate the socket option used to enable
 *     TFO on a client socket (FreeBSD, Linux 4.14 and later)
 *
 * OSSL_TFO_SENDTO
 *     Defined to indicate the sendto() message type to
 *     be used to initiate a TFO connection (FreeBSD,
 *     Linux pre-4.14)
 *
 * OSSL_TFO_DO_NOT_CONNECT
 *     Defined to skip calling connect() when creating a
 *     client socket (macOS, FreeBSD, Linux pre-4.14)
 */

# if defined(OPENSSL_SYS_WINDOWS)
/*
 * NO WINDOWS SUPPORT
 *
 * But this is what would be used on the server:
 *
 * define OSSL_TFO_SERVER_SOCKOPT       TCP_FASTOPEN
 * define OSSL_TFO_SERVER_SOCKOPT_VALUE 1
 *
 * Still have to figure out client support
 */
#  undef TCP_FASTOPEN
# endif

/* NO VMS SUPPORT */
# if defined(OPENSSL_SYS_VMS)
#  undef TCP_FASTOPEN
# endif

# if defined(OPENSSL_SYS_MACOSX)
#  define OSSL_TFO_SYSCTL               "net.inet.tcp.fastopen"
#  define OSSL_TFO_SERVER_SOCKOPT       TCP_FASTOPEN
#  define OSSL_TFO_SERVER_SOCKOPT_VALUE 1
#  define OSSL_TFO_CONNECTX             1
#  define OSSL_TFO_DO_NOT_CONNECT       1
#  define OSSL_TFO_CLIENT_FLAG          1
#  define OSSL_TFO_SERVER_FLAG          2
# endif

# if defined(__FreeBSD__)
#  if defined(TCP_FASTOPEN_PSK_LEN)
/* As of 12.0 these are the SYSCTLs */
#   define OSSL_TFO_SYSCTL_SERVER        "net.inet.tcp.fastopen.server_enable"
#   define OSSL_TFO_SYSCTL_CLIENT        "net.inet.tcp.fastopen.client_enable"
#   define OSSL_TFO_SERVER_SOCKOPT       TCP_FASTOPEN
#   define OSSL_TFO_SERVER_SOCKOPT_VALUE MAX_LISTEN
#   define OSSL_TFO_CLIENT_SOCKOPT       TCP_FASTOPEN
#   define OSSL_TFO_DO_NOT_CONNECT       1
#   define OSSL_TFO_SENDTO               0
/* These are the same because the sysctl are client/server-specific */
#   define OSSL_TFO_CLIENT_FLAG          1
#   define OSSL_TFO_SERVER_FLAG          1
#  else
/* 10.3 through 11.4 SYSCTL - ONLY SERVER SUPPORT */
#   define OSSL_TFO_SYSCTL               "net.inet.tcp.fastopen.enabled"
#   define OSSL_TFO_SERVER_SOCKOPT       TCP_FASTOPEN
#   define OSSL_TFO_SERVER_SOCKOPT_VALUE MAX_LISTEN
#   define OSSL_TFO_SERVER_FLAG          1
#  endif
# endif

# if defined(OPENSSL_SYS_LINUX)
/* OSSL_TFO_PROC not used, but of interest */
#  define OSSL_TFO_PROC                 "/proc/sys/net/ipv4/tcp_fastopen"
#  define OSSL_TFO_SERVER_SOCKOPT       TCP_FASTOPEN
#  define OSSL_TFO_SERVER_SOCKOPT_VALUE MAX_LISTEN
#  if defined(TCP_FASTOPEN_CONNECT)
#   define OSSL_TFO_CLIENT_SOCKOPT      TCP_FASTOPEN_CONNECT
#  else
#   define OSSL_TFO_SENDTO              MSG_FASTOPEN
#   define OSSL_TFO_DO_NOT_CONNECT      1
#  endif
#  define OSSL_TFO_CLIENT_FLAG          1
#  define OSSL_TFO_SERVER_FLAG          2
# endif

#endif
