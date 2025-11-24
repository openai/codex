/*
 * Copyright 1995-2025 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifndef OSSL_INTERNAL_SOCKETS_H
# define OSSL_INTERNAL_SOCKETS_H
# pragma once

# include <openssl/opensslconf.h>
# include "internal/common.h"

# if defined(OPENSSL_SYS_VXWORKS) || defined(OPENSSL_SYS_UEFI)
#  define NO_SYS_PARAM_H
# endif
# ifdef WIN32
#  define NO_SYS_UN_H
# endif
# ifdef OPENSSL_SYS_VMS
#  define NO_SYS_PARAM_H
#  define NO_SYS_UN_H
# endif

# ifdef OPENSSL_NO_SOCK

# elif defined(OPENSSL_SYS_WINDOWS) || defined(OPENSSL_SYS_MSDOS)
#  if defined(__DJGPP__)
#   define WATT32
#   define WATT32_NO_OLDIES
#   include <sys/socket.h>
#   include <sys/un.h>
#   include <tcp.h>
#   include <netdb.h>
#   include <arpa/inet.h>
#   include <netinet/tcp.h>
#  elif defined(_WIN32_WCE) && _WIN32_WCE<410
#   define getservbyname _masked_declaration_getservbyname
#  endif
#  if !defined(IPPROTO_IP)
    /* winsock[2].h was included already? */
#   include "internal/e_winsock.h"
#  endif
#  ifdef getservbyname
     /* this is used to be wcecompat/include/winsock_extras.h */
#   undef getservbyname
struct servent *PASCAL getservbyname(const char *, const char *);
#  endif

#  ifdef _WIN64
/*
 * Even though sizeof(SOCKET) is 8, it's safe to cast it to int, because
 * the value constitutes an index in per-process table of limited size
 * and not a real pointer. And we also depend on fact that all processors
 * Windows run on happen to be two's complement, which allows to
 * interchange INVALID_SOCKET and -1.
 */
#   define socket(d,t,p)   ((int)socket(d,t,p))
#   define accept(s,f,l)   ((int)accept(s,f,l))
#  endif

/* Windows have other names for shutdown() reasons */
#  ifndef SHUT_RD
#   define SHUT_RD SD_RECEIVE
#  endif
#  ifndef SHUT_WR
#   define SHUT_WR SD_SEND
#  endif
#  ifndef SHUT_RDWR
#   define SHUT_RDWR SD_BOTH
#  endif

# else
#  if defined(__APPLE__)
    /*
     * This must be defined before including <netinet/in6.h> to get
     * IPV6_RECVPKTINFO
     */
#   define __APPLE_USE_RFC_3542
#  endif

#  ifndef NO_SYS_PARAM_H
#   include <sys/param.h>
#  endif
#  ifdef OPENSSL_SYS_VXWORKS
#   include <time.h>
#  endif

#  include <netdb.h>
#  if defined(OPENSSL_SYS_VMS)
typedef size_t socklen_t;        /* Currently appears to be missing on VMS */
#  endif
#  if defined(OPENSSL_SYS_VMS_NODECC)
#   include <socket.h>
#   include <in.h>
#   include <inet.h>
#  else
#   include <sys/socket.h>
#   if !defined(NO_SYS_UN_H) && defined(AF_UNIX) && !defined(OPENSSL_NO_UNIX_SOCK)
#    include <sys/un.h>
#    ifndef UNIX_PATH_MAX
#     define UNIX_PATH_MAX sizeof(((struct sockaddr_un *)NULL)->sun_path)
#    endif
#   endif
#   ifdef FILIO_H
#    include <sys/filio.h> /* FIONBIO in some SVR4, e.g. unixware, solaris */
#   endif
#   include <netinet/in.h>
#   include <arpa/inet.h>
#   include <netinet/tcp.h>
#  endif

#  ifdef OPENSSL_SYS_AIX
#   include <sys/select.h>
#  endif

#  ifdef OPENSSL_SYS_UNIX
#    ifndef OPENSSL_SYS_TANDEM
#     include <poll.h>
#    endif
#    include <errno.h>
#  endif

#  ifndef VMS
#   include <sys/ioctl.h>
#  else
#   if !defined(TCPIP_TYPE_SOCKETSHR) && defined(__VMS_VER) && (__VMS_VER > 70000000)
     /* ioctl is only in VMS > 7.0 and when socketshr is not used */
#    include <sys/ioctl.h>
#   endif
#   include <unixio.h>
#   if defined(TCPIP_TYPE_SOCKETSHR)
#    include <socketshr.h>
#   endif
#  endif

#  ifndef INVALID_SOCKET
#   define INVALID_SOCKET      (-1)
#  endif
# endif

/*
 * Some IPv6 implementations are broken, you can disable them in known
 * bad versions.
 */
# if !defined(OPENSSL_USE_IPV6)
#  if defined(AF_INET6)
#   define OPENSSL_USE_IPV6 1
#  else
#   define OPENSSL_USE_IPV6 0
#  endif
# endif

/*
 * Some platforms define AF_UNIX, but don't support it
 */
# if !defined(OPENSSL_NO_UNIX_SOCK)
#  if !defined(AF_UNIX) || defined(NO_SYS_UN_H)
#   define OPENSSL_NO_UNIX_SOCK
#  endif
# endif

# define get_last_socket_error() errno
# define clear_socket_error()    errno=0
# define get_last_socket_error_is_eintr() (get_last_socket_error() == EINTR)

# if defined(OPENSSL_SYS_WINDOWS)
#  undef get_last_socket_error
#  undef clear_socket_error
#  undef get_last_socket_error_is_eintr
#  define get_last_socket_error() WSAGetLastError()
#  define clear_socket_error()    WSASetLastError(0)
#  define get_last_socket_error_is_eintr() (get_last_socket_error() == WSAEINTR)
#  define readsocket(s,b,n)       recv((s),(b),(n),0)
#  define writesocket(s,b,n)      send((s),(b),(n),0)
# elif defined(__DJGPP__)
#  define closesocket(s)          close_s(s)
#  define readsocket(s,b,n)       read_s(s,b,n)
#  define writesocket(s,b,n)      send(s,b,n,0)
# elif defined(OPENSSL_SYS_VMS)
#  define ioctlsocket(a,b,c)      ioctl(a,b,c)
#  define closesocket(s)          close(s)
#  define readsocket(s,b,n)       recv((s),(b),(n),0)
#  define writesocket(s,b,n)      send((s),(b),(n),0)
# elif defined(OPENSSL_SYS_VXWORKS)
#  define ioctlsocket(a,b,c)          ioctl((a),(b),(int)(c))
#  define closesocket(s)              close(s)
#  define readsocket(s,b,n)           read((s),(b),(n))
#  define writesocket(s,b,n)          write((s),(char *)(b),(n))
# elif defined(OPENSSL_SYS_TANDEM)
#  define readsocket(s,b,n)       read((s),(b),(n))
#  define writesocket(s,b,n)      write((s),(b),(n))
#  define ioctlsocket(a,b,c)      ioctl(a,b,c)
#  define closesocket(s)          close(s)
# else
#  define ioctlsocket(a,b,c)      ioctl(a,b,c)
#  define closesocket(s)          close(s)
#  define readsocket(s,b,n)       read((s),(b),(n))
#  define writesocket(s,b,n)      write((s),(b),(n))
# endif

/* also in apps/include/apps.h */
# if defined(OPENSSL_SYS_WIN32) || defined(OPENSSL_SYS_WINCE)
#  define openssl_fdset(a, b) FD_SET((unsigned int)(a), b)
# else
#  define openssl_fdset(a, b) FD_SET(a, b)
# endif

#endif
