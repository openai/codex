/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps-unix.c Wrappers around UNIX system/libc features (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003, 2006  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
 *
 * Licensed under the Academic Free License version 2.1
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 *
 */

#include <config.h>

#include "dbus-internals.h"
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-unix.h"
#include "dbus-threads.h"
#include "dbus-protocol.h"
#include "dbus-file.h"
#include "dbus-transport.h"
#include "dbus-string.h"
#include "dbus-userdb.h"
#include "dbus-list.h"
#include "dbus-credentials.h"
#include "dbus-nonce.h"

#include <sys/types.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <unistd.h>
#include <stdio.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <dirent.h>
#include <sys/un.h>
#include <pwd.h>
#include <time.h>
#include <locale.h>
#include <sys/time.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <netdb.h>
#include <grp.h>
#include <arpa/inet.h>

#ifdef HAVE_ERRNO_H
#include <errno.h>
#endif
#ifdef HAVE_SYSLOG_H
#include <syslog.h>
#endif
#ifdef HAVE_WRITEV
#include <sys/uio.h>
#endif
#ifdef HAVE_BACKTRACE
#include <execinfo.h>
#endif
#ifdef HAVE_GETPEERUCRED
#include <ucred.h>
#endif
#ifdef HAVE_ALLOCA_H
#include <alloca.h>
#endif
#ifdef HAVE_SYS_RANDOM_H
#include <sys/random.h>
#endif

#ifdef HAVE_ADT
#include <bsm/adt.h>
#endif

#ifdef HAVE_SYSTEMD
#include <systemd/sd-daemon.h>
#endif

#if !DBUS_USE_SYNC
#include <pthread.h>
#endif

#ifndef O_BINARY
#define O_BINARY 0
#endif

#ifndef AI_ADDRCONFIG
#define AI_ADDRCONFIG 0
#endif

#ifndef HAVE_SOCKLEN_T
#define socklen_t int
#endif

#if defined (__sun) || defined (__sun__)
/*
 * CMS_SPACE etc. definitions for Solaris < 10, based on
 *   http://mailman.videolan.org/pipermail/vlc-devel/2006-May/024402.html
 * via
 *   http://wiki.opencsw.org/porting-faq#toc10
 *
 * These are only redefined for Solaris, for now: if your OS needs these too,
 * please file a bug. (Or preferably, improve your OS so they're not needed.)
 */

# ifndef CMSG_ALIGN
#   ifdef __sun__
#     define CMSG_ALIGN(len) _CMSG_DATA_ALIGN (len)
#   else
      /* aligning to sizeof (long) is assumed to be portable (fd.o#40235) */
#     define CMSG_ALIGN(len) (((len) + sizeof (long) - 1) & \
                              ~(sizeof (long) - 1))
#   endif
# endif

# ifndef CMSG_SPACE
#   define CMSG_SPACE(len) (CMSG_ALIGN (sizeof (struct cmsghdr)) + \
                            CMSG_ALIGN (len))
# endif

# ifndef CMSG_LEN
#   define CMSG_LEN(len) (CMSG_ALIGN (sizeof (struct cmsghdr)) + (len))
# endif

#endif /* Solaris */

/**
 * Ensure that the standard file descriptors stdin, stdout and stderr
 * are open, by opening /dev/null if necessary.
 *
 * This function does not use DBusError, to avoid calling malloc(), so
 * that it can be used in contexts where an async-signal-safe function
 * is required (for example after fork()). Instead, on failure it sets
 * errno and returns something like "Failed to open /dev/null" in
 * *error_str_p. Callers are expected to combine *error_str_p
 * with _dbus_strerror (errno) to get a full error report.
 *
 * This function can only be called while single-threaded: either during
 * startup of an executable, or after fork().
 */
dbus_bool_t
_dbus_ensure_standard_fds (DBusEnsureStandardFdsFlags   flags,
                           const char                 **error_str_p)
{
  static int const relevant_flag[] = { DBUS_FORCE_STDIN_NULL,
      DBUS_FORCE_STDOUT_NULL,
      DBUS_FORCE_STDERR_NULL };
  /* Should always get replaced with the real error before use */
  const char *error_str = "Failed mysteriously";
  int devnull = -1;
  int saved_errno;
  /* This function relies on the standard fds having their POSIX values. */
  _DBUS_STATIC_ASSERT (STDIN_FILENO == 0);
  _DBUS_STATIC_ASSERT (STDOUT_FILENO == 1);
  _DBUS_STATIC_ASSERT (STDERR_FILENO == 2);
  int i;

  for (i = STDIN_FILENO; i <= STDERR_FILENO; i++)
    {
      /* Because we rely on being single-threaded, and we want the
       * standard fds to not be close-on-exec, we don't set it
       * close-on-exec. */
      if (devnull < i)
        devnull = open ("/dev/null", O_RDWR);

      if (devnull < 0)
        {
          error_str = "Failed to open /dev/null";
          goto out;
        }

      /* We already opened all fds < i, so the only way this assertion
       * could fail is if another thread closed one, and we document
       * this function as not safe for multi-threading. */
      _dbus_assert (devnull >= i);

      if (devnull != i && (flags & relevant_flag[i]) != 0)
        {
          if (dup2 (devnull, i) < 0)
            {
              error_str = "Failed to dup2 /dev/null onto a standard fd";
              goto out;
            }
        }
    }

  error_str = NULL;

out:
  saved_errno = errno;

  if (devnull > STDERR_FILENO)
    close (devnull);

  if (error_str_p != NULL)
    *error_str_p = error_str;

  errno = saved_errno;
  return (error_str == NULL);
}

static dbus_bool_t _dbus_set_fd_nonblocking (int             fd,
                                             DBusError      *error);

static dbus_bool_t
_dbus_open_socket (int              *fd_p,
                   int               domain,
                   int               type,
                   int               protocol,
                   DBusError        *error)
{
#ifdef SOCK_CLOEXEC
  dbus_bool_t cloexec_done;

  *fd_p = socket (domain, type | SOCK_CLOEXEC, protocol);
  cloexec_done = *fd_p >= 0;

  /* Check if kernel seems to be too old to know SOCK_CLOEXEC */
  if (*fd_p < 0 && (errno == EINVAL || errno == EPROTOTYPE))
#endif
    {
      *fd_p = socket (domain, type, protocol);
    }

  if (*fd_p >= 0)
    {
#ifdef SOCK_CLOEXEC
      if (!cloexec_done)
#endif
        {
          _dbus_fd_set_close_on_exec(*fd_p);
        }

      _dbus_verbose ("socket fd %d opened\n", *fd_p);
      return TRUE;
    }
  else
    {
      dbus_set_error(error,
                     _dbus_error_from_errno (errno),
                     "Failed to open socket: %s",
                     _dbus_strerror (errno));
      return FALSE;
    }
}

/**
 * Opens a UNIX domain socket (as in the socket() call).
 * Does not bind the socket.
 *
 * This will set FD_CLOEXEC for the socket returned
 *
 * @param fd return location for socket descriptor
 * @param error return location for an error
 * @returns #FALSE if error is set
 */
static dbus_bool_t
_dbus_open_unix_socket (int              *fd,
                        DBusError        *error)
{
  return _dbus_open_socket(fd, PF_UNIX, SOCK_STREAM, 0, error);
}

/**
 * Closes a socket. Should not be used on non-socket
 * file descriptors or handles.
 *
 * @param fd the socket
 * @param error return location for an error
 * @returns #FALSE if error is set
 */
dbus_bool_t
_dbus_close_socket (DBusSocket        fd,
                    DBusError        *error)
{
  return _dbus_close (fd.fd, error);
}

/**
 * Like _dbus_read(), but only works on sockets so is
 * available on Windows.
 *
 * @param fd the socket
 * @param buffer string to append data to
 * @param count max amount of data to read
 * @returns number of bytes appended to the string
 */
int
_dbus_read_socket (DBusSocket        fd,
                   DBusString       *buffer,
                   int               count)
{
  return _dbus_read (fd.fd, buffer, count);
}

/**
 * Like _dbus_write(), but only supports sockets
 * and is thus available on Windows.
 *
 * @param fd the file descriptor to write
 * @param buffer the buffer to write data from
 * @param start the first byte in the buffer to write
 * @param len the number of bytes to try to write
 * @returns the number of bytes written or -1 on error
 */
int
_dbus_write_socket (DBusSocket        fd,
                    const DBusString *buffer,
                    int               start,
                    int               len)
{
#if HAVE_DECL_MSG_NOSIGNAL
  const char *data;
  int bytes_written;

  data = _dbus_string_get_const_data_len (buffer, start, len);

 again:

  bytes_written = send (fd.fd, data, len, MSG_NOSIGNAL);

  if (bytes_written < 0 && errno == EINTR)
    goto again;

  return bytes_written;

#else
  return _dbus_write (fd.fd, buffer, start, len);
#endif
}

/**
 * Like _dbus_read_socket() but also tries to read unix fds from the
 * socket. When there are more fds to read than space in the array
 * passed this function will fail with ENOSPC.
 *
 * @param fd the socket
 * @param buffer string to append data to
 * @param count max amount of data to read
 * @param fds array to place read file descriptors in
 * @param n_fds on input space in fds array, on output how many fds actually got read
 * @returns number of bytes appended to string
 */
int
_dbus_read_socket_with_unix_fds (DBusSocket        fd,
                                 DBusString       *buffer,
                                 int               count,
                                 int              *fds,
                                 unsigned int     *n_fds) {
#ifndef HAVE_UNIX_FD_PASSING
  int r;

  if ((r = _dbus_read_socket(fd, buffer, count)) < 0)
    return r;

  *n_fds = 0;
  return r;

#else
  int bytes_read;
  int start;
  struct msghdr m;
  struct iovec iov;

  _dbus_assert (count >= 0);
  _dbus_assert (*n_fds <= DBUS_MAXIMUM_MESSAGE_UNIX_FDS);

  start = _dbus_string_get_length (buffer);

  if (!_dbus_string_lengthen (buffer, count))
    {
      errno = ENOMEM;
      return -1;
    }

  _DBUS_ZERO(iov);
  iov.iov_base = _dbus_string_get_data_len (buffer, start, count);
  iov.iov_len = count;

  _DBUS_ZERO(m);
  m.msg_iov = &iov;
  m.msg_iovlen = 1;

  /* Hmm, we have no clue how long the control data will actually be
     that is queued for us. The least we can do is assume that the
     caller knows. Hence let's make space for the number of fds that
     we shall read at max plus the cmsg header. */
  m.msg_controllen = CMSG_SPACE(*n_fds * sizeof(int));

  /* It's probably safe to assume that systems with SCM_RIGHTS also
     know alloca() */
  m.msg_control = alloca(m.msg_controllen);
  memset(m.msg_control, 0, m.msg_controllen);

  /* Do not include the padding at the end when we tell the kernel
   * how much we're willing to receive. This avoids getting
   * the padding filled with additional fds that we weren't expecting,
   * if a (potentially malicious) sender included them. (fd.o #83622) */
  m.msg_controllen = CMSG_LEN (*n_fds * sizeof(int));

 again:

  bytes_read = recvmsg (fd.fd, &m, 0
#ifdef MSG_CMSG_CLOEXEC
                       |MSG_CMSG_CLOEXEC
#endif
                       );

  if (bytes_read < 0)
    {
      if (errno == EINTR)
        goto again;
      else
        {
          /* put length back (note that this doesn't actually realloc anything) */
          _dbus_string_set_length (buffer, start);
          return -1;
        }
    }
  else
    {
      struct cmsghdr *cm;
      dbus_bool_t found = FALSE;

      for (cm = CMSG_FIRSTHDR(&m); cm; cm = CMSG_NXTHDR(&m, cm))
        if (cm->cmsg_level == SOL_SOCKET && cm->cmsg_type == SCM_RIGHTS)
          {
            size_t i;
            int *payload = (int *) CMSG_DATA (cm);
            size_t payload_len_bytes = (cm->cmsg_len - CMSG_LEN (0));
            size_t payload_len_fds;
            size_t fds_to_use;

            /* Every unsigned int fits in a size_t without truncation, so
             * casting (size_t) *n_fds is OK */
            _DBUS_STATIC_ASSERT (sizeof (size_t) >= sizeof (unsigned int));

            if ((m.msg_flags & MSG_CTRUNC) && CMSG_NXTHDR(&m, cm) == NULL &&
              (char *) payload + payload_len_bytes >
              (char *) m.msg_control + m.msg_controllen)
              {
                /* This is the last cmsg in a truncated message and using
                 * cmsg_len would apparently overrun the allocated buffer.
                 * Some operating systems (illumos and Solaris are known) do
                 * not adjust cmsg_len in the last cmsg when truncation occurs.
                 * Adjust the payload length here. The calculation for
                 * payload_len_fds below will discard any trailing bytes that
                 * belong to an incomplete file descriptor - the kernel will
                 * have already closed that (at least for illumos and Solaris)
                 */
                 payload_len_bytes = m.msg_controllen -
                   ((char *) payload - (char *) m.msg_control);
              }

            payload_len_fds = payload_len_bytes / sizeof (int);

            if (_DBUS_LIKELY (payload_len_fds <= (size_t) *n_fds))
              {
                /* The fds in the payload will fit in our buffer */
                fds_to_use = payload_len_fds;
              }
            else
              {
                /* Too many fds in the payload. This shouldn't happen
                 * any more because we're setting m.msg_controllen to
                 * the exact number we can accept, but be safe and
                 * truncate. */
                fds_to_use = (size_t) *n_fds;

                /* Close the excess fds to avoid DoS: if they stayed open,
                 * someone could send us an extra fd per message
                 * and we'd eventually run out. */
                for (i = fds_to_use; i < payload_len_fds; i++)
                  {
                    close (payload[i]);
                  }
              }

            memcpy (fds, payload, fds_to_use * sizeof (int));
            found = TRUE;
            /* This narrowing cast from size_t to unsigned int cannot
             * overflow because we have chosen fds_to_use
             * to be <= *n_fds */
            *n_fds = (unsigned int) fds_to_use;

            /* Linux doesn't tell us whether MSG_CMSG_CLOEXEC actually
               worked, hence we need to go through this list and set
               CLOEXEC everywhere in any case */
            for (i = 0; i < fds_to_use; i++)
              _dbus_fd_set_close_on_exec(fds[i]);

            break;
          }

      if (!found)
        *n_fds = 0;

      if (m.msg_flags & MSG_CTRUNC)
        {
          unsigned int i;

          /* Hmm, apparently the control data was truncated. The bad
             thing is that we might have completely lost a couple of fds
             without chance to recover them. Hence let's treat this as a
             serious error. */

          /* We still need to close whatever fds we *did* receive,
           * otherwise they'll never get closed. (CVE-2020-12049) */
          for (i = 0; i < *n_fds; i++)
            close (fds[i]);

          *n_fds = 0;
          errno = ENOSPC;
          _dbus_string_set_length (buffer, start);
          return -1;
        }

      /* put length back (doesn't actually realloc) */
      _dbus_string_set_length (buffer, start + bytes_read);

#if 0
      if (bytes_read > 0)
        _dbus_verbose_bytes_of_string (buffer, start, bytes_read);
#endif

      return bytes_read;
    }
#endif
}

int
_dbus_write_socket_with_unix_fds(DBusSocket        fd,
                                 const DBusString *buffer,
                                 int               start,
                                 int               len,
                                 const int        *fds,
                                 int               n_fds) {

#ifndef HAVE_UNIX_FD_PASSING

  if (n_fds > 0) {
    errno = ENOTSUP;
    return -1;
  }

  return _dbus_write_socket(fd, buffer, start, len);
#else
  return _dbus_write_socket_with_unix_fds_two(fd, buffer, start, len, NULL, 0, 0, fds, n_fds);
#endif
}

int
_dbus_write_socket_with_unix_fds_two(DBusSocket        fd,
                                     const DBusString *buffer1,
                                     int               start1,
                                     int               len1,
                                     const DBusString *buffer2,
                                     int               start2,
                                     int               len2,
                                     const int        *fds,
                                     int               n_fds) {

#ifndef HAVE_UNIX_FD_PASSING

  if (n_fds > 0) {
    errno = ENOTSUP;
    return -1;
  }

  return _dbus_write_socket_two(fd,
                                buffer1, start1, len1,
                                buffer2, start2, len2);
#else

  struct msghdr m;
  struct cmsghdr *cm;
  struct iovec iov[2];
  int bytes_written;

  _dbus_assert (len1 >= 0);
  _dbus_assert (len2 >= 0);
  _dbus_assert (n_fds >= 0);

  _DBUS_ZERO(iov);
  iov[0].iov_base = (char*) _dbus_string_get_const_data_len (buffer1, start1, len1);
  iov[0].iov_len = len1;

  if (buffer2)
    {
      iov[1].iov_base = (char*) _dbus_string_get_const_data_len (buffer2, start2, len2);
      iov[1].iov_len = len2;
    }

  _DBUS_ZERO(m);
  m.msg_iov = iov;
  m.msg_iovlen = buffer2 ? 2 : 1;

  if (n_fds > 0)
    {
      m.msg_controllen = CMSG_SPACE(n_fds * sizeof(int));
      m.msg_control = alloca(m.msg_controllen);
      memset(m.msg_control, 0, m.msg_controllen);

      cm = CMSG_FIRSTHDR(&m);
      cm->cmsg_level = SOL_SOCKET;
      cm->cmsg_type = SCM_RIGHTS;
      cm->cmsg_len = CMSG_LEN(n_fds * sizeof(int));
      memcpy(CMSG_DATA(cm), fds, n_fds * sizeof(int));
    }

 again:

  bytes_written = sendmsg (fd.fd, &m, 0
#if HAVE_DECL_MSG_NOSIGNAL
                           |MSG_NOSIGNAL
#endif
                           );

  if (bytes_written < 0 && errno == EINTR)
    goto again;

#if 0
  if (bytes_written > 0)
    _dbus_verbose_bytes_of_string (buffer, start, bytes_written);
#endif

  return bytes_written;
#endif
}

/**
 * Like _dbus_write_two() but only works on sockets and is thus
 * available on Windows.
 *
 * @param fd the file descriptor
 * @param buffer1 first buffer
 * @param start1 first byte to write in first buffer
 * @param len1 number of bytes to write from first buffer
 * @param buffer2 second buffer, or #NULL
 * @param start2 first byte to write in second buffer
 * @param len2 number of bytes to write in second buffer
 * @returns total bytes written from both buffers, or -1 on error
 */
int
_dbus_write_socket_two (DBusSocket        fd,
                        const DBusString *buffer1,
                        int               start1,
                        int               len1,
                        const DBusString *buffer2,
                        int               start2,
                        int               len2)
{
#if HAVE_DECL_MSG_NOSIGNAL
  struct iovec vectors[2];
  const char *data1;
  const char *data2;
  int bytes_written;
  struct msghdr m;

  _dbus_assert (buffer1 != NULL);
  _dbus_assert (start1 >= 0);
  _dbus_assert (start2 >= 0);
  _dbus_assert (len1 >= 0);
  _dbus_assert (len2 >= 0);

  data1 = _dbus_string_get_const_data_len (buffer1, start1, len1);

  if (buffer2 != NULL)
    data2 = _dbus_string_get_const_data_len (buffer2, start2, len2);
  else
    {
      data2 = NULL;
      start2 = 0;
      len2 = 0;
    }

  vectors[0].iov_base = (char*) data1;
  vectors[0].iov_len = len1;
  vectors[1].iov_base = (char*) data2;
  vectors[1].iov_len = len2;

  _DBUS_ZERO(m);
  m.msg_iov = vectors;
  m.msg_iovlen = data2 ? 2 : 1;

 again:

  bytes_written = sendmsg (fd.fd, &m, MSG_NOSIGNAL);

  if (bytes_written < 0 && errno == EINTR)
    goto again;

  return bytes_written;

#else
  return _dbus_write_two (fd.fd, buffer1, start1, len1,
                          buffer2, start2, len2);
#endif
}

/**
 * Thin wrapper around the read() system call that appends
 * the data it reads to the DBusString buffer. It appends
 * up to the given count, and returns the same value
 * and same errno as read(). The only exception is that
 * _dbus_read() handles EINTR for you. Also, _dbus_read() can
 * return ENOMEM, even though regular UNIX read doesn't.
 *
 * Unlike _dbus_read_socket(), _dbus_read() is not available
 * on Windows.
 *
 * @param fd the file descriptor to read from
 * @param buffer the buffer to append data to
 * @param count the amount of data to read
 * @returns the number of bytes read or -1
 */
int
_dbus_read (int               fd,
            DBusString       *buffer,
            int               count)
{
  int bytes_read;
  int start;
  char *data;

  _dbus_assert (count >= 0);

  start = _dbus_string_get_length (buffer);

  if (!_dbus_string_lengthen (buffer, count))
    {
      errno = ENOMEM;
      return -1;
    }

  data = _dbus_string_get_data_len (buffer, start, count);

 again:

  bytes_read = read (fd, data, count);

  if (bytes_read < 0)
    {
      if (errno == EINTR)
        goto again;
      else
        {
          /* put length back (note that this doesn't actually realloc anything) */
          _dbus_string_set_length (buffer, start);
          return -1;
        }
    }
  else
    {
      /* put length back (doesn't actually realloc) */
      _dbus_string_set_length (buffer, start + bytes_read);

#if 0
      if (bytes_read > 0)
        _dbus_verbose_bytes_of_string (buffer, start, bytes_read);
#endif

      return bytes_read;
    }
}

/**
 * Thin wrapper around the write() system call that writes a part of a
 * DBusString and handles EINTR for you.
 *
 * @param fd the file descriptor to write
 * @param buffer the buffer to write data from
 * @param start the first byte in the buffer to write
 * @param len the number of bytes to try to write
 * @returns the number of bytes written or -1 on error
 */
int
_dbus_write (int               fd,
             const DBusString *buffer,
             int               start,
             int               len)
{
  const char *data;
  int bytes_written;

  data = _dbus_string_get_const_data_len (buffer, start, len);

 again:

  bytes_written = write (fd, data, len);

  if (bytes_written < 0 && errno == EINTR)
    goto again;

#if 0
  if (bytes_written > 0)
    _dbus_verbose_bytes_of_string (buffer, start, bytes_written);
#endif

  return bytes_written;
}

/**
 * Like _dbus_write() but will use writev() if possible
 * to write both buffers in sequence. The return value
 * is the number of bytes written in the first buffer,
 * plus the number written in the second. If the first
 * buffer is written successfully and an error occurs
 * writing the second, the number of bytes in the first
 * is returned (i.e. the error is ignored), on systems that
 * don't have writev. Handles EINTR for you.
 * The second buffer may be #NULL.
 *
 * @param fd the file descriptor
 * @param buffer1 first buffer
 * @param start1 first byte to write in first buffer
 * @param len1 number of bytes to write from first buffer
 * @param buffer2 second buffer, or #NULL
 * @param start2 first byte to write in second buffer
 * @param len2 number of bytes to write in second buffer
 * @returns total bytes written from both buffers, or -1 on error
 */
int
_dbus_write_two (int               fd,
                 const DBusString *buffer1,
                 int               start1,
                 int               len1,
                 const DBusString *buffer2,
                 int               start2,
                 int               len2)
{
  _dbus_assert (buffer1 != NULL);
  _dbus_assert (start1 >= 0);
  _dbus_assert (start2 >= 0);
  _dbus_assert (len1 >= 0);
  _dbus_assert (len2 >= 0);

#ifdef HAVE_WRITEV
  {
    struct iovec vectors[2];
    const char *data1;
    const char *data2;
    int bytes_written;

    data1 = _dbus_string_get_const_data_len (buffer1, start1, len1);

    if (buffer2 != NULL)
      data2 = _dbus_string_get_const_data_len (buffer2, start2, len2);
    else
      {
        data2 = NULL;
        start2 = 0;
        len2 = 0;
      }

    vectors[0].iov_base = (char*) data1;
    vectors[0].iov_len = len1;
    vectors[1].iov_base = (char*) data2;
    vectors[1].iov_len = len2;

  again:

    bytes_written = writev (fd,
                            vectors,
                            data2 ? 2 : 1);

    if (bytes_written < 0 && errno == EINTR)
      goto again;

    return bytes_written;
  }
#else /* HAVE_WRITEV */
  {
    int ret1, ret2;

    ret1 = _dbus_write (fd, buffer1, start1, len1);
    if (ret1 == len1 && buffer2 != NULL)
      {
        ret2 = _dbus_write (fd, buffer2, start2, len2);
        if (ret2 < 0)
          ret2 = 0; /* we can't report an error as the first write was OK */

        return ret1 + ret2;
      }
    else
      return ret1;
  }
#endif /* !HAVE_WRITEV */
}

#define _DBUS_MAX_SUN_PATH_LENGTH 99

/**
 * @def _DBUS_MAX_SUN_PATH_LENGTH
 *
 * Maximum length of the path to a UNIX domain socket,
 * sockaddr_un::sun_path member. POSIX requires that all systems
 * support at least 100 bytes here, including the nul termination.
 * We use 99 for the max value to allow for the nul.
 *
 * We could probably also do sizeof (addr.sun_path)
 * but this way we are the same on all platforms
 * which is probably a good idea.
 */

/**
 * Creates a socket and connects it to the UNIX domain socket at the
 * given path.  The connection fd is returned, and is set up as
 * nonblocking.
 *
 * Uses abstract sockets instead of filesystem-linked sockets if
 * requested (it's possible only on Linux; see "man 7 unix" on Linux).
 * On non-Linux abstract socket usage always fails.
 *
 * This will set FD_CLOEXEC for the socket returned.
 *
 * @param path the path to UNIX domain socket
 * @param abstract #TRUE to use abstract namespace
 * @param error return location for error code
 * @returns connection file descriptor or -1 on error
 */
int
_dbus_connect_unix_socket (const char     *path,
                           dbus_bool_t     abstract,
                           DBusError      *error)
{
  int fd;
  size_t path_len;
  struct sockaddr_un addr;
  _DBUS_STATIC_ASSERT (sizeof (addr.sun_path) > _DBUS_MAX_SUN_PATH_LENGTH);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_verbose ("connecting to unix socket %s abstract=%d\n",
                 path, abstract);


  if (!_dbus_open_unix_socket (&fd, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET(error);
      return -1;
    }
  _DBUS_ASSERT_ERROR_IS_CLEAR(error);

  _DBUS_ZERO (addr);
  addr.sun_family = AF_UNIX;
  path_len = strlen (path);

  if (abstract)
    {
#ifdef __linux__
      addr.sun_path[0] = '\0'; /* this is what says "use abstract" */
      path_len++; /* Account for the extra nul byte added to the start of sun_path */

      if (path_len > _DBUS_MAX_SUN_PATH_LENGTH)
        {
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                      "Abstract socket name too long\n");
          _dbus_close (fd, NULL);
          return -1;
	}

      strncpy (&addr.sun_path[1], path, sizeof (addr.sun_path) - 2);
      /* _dbus_verbose_bytes (addr.sun_path, sizeof (addr.sun_path)); */
#else /* !__linux__ */
      dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                      "Operating system does not support abstract socket namespace\n");
      _dbus_close (fd, NULL);
      return -1;
#endif /* !__linux__ */
    }
  else
    {
      if (path_len > _DBUS_MAX_SUN_PATH_LENGTH)
        {
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                      "Socket name too long\n");
          _dbus_close (fd, NULL);
          return -1;
	}

      strncpy (addr.sun_path, path, sizeof (addr.sun_path) - 1);
    }

  if (connect (fd, (struct sockaddr*) &addr, _DBUS_STRUCT_OFFSET (struct sockaddr_un, sun_path) + path_len) < 0)
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (errno),
                      "Failed to connect to socket %s: %s",
                      path, _dbus_strerror (errno));

      _dbus_close (fd, NULL);
      return -1;
    }

  if (!_dbus_set_fd_nonblocking (fd, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);

      _dbus_close (fd, NULL);
      return -1;
    }

  return fd;
}

/**
 * Creates a UNIX domain socket and connects it to the specified
 * process to execute.
 *
 * This will set FD_CLOEXEC for the socket returned.
 *
 * @param path the path to the executable
 * @param argv the argument list for the process to execute.
 * argv[0] typically is identical to the path of the executable
 * @param error return location for error code
 * @returns connection file descriptor or -1 on error
 */
int
_dbus_connect_exec (const char     *path,
                    char *const    argv[],
                    DBusError      *error)
{
  int fds[2];
  pid_t pid;
  int retval;
  dbus_bool_t cloexec_done = 0;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_verbose ("connecting to process %s\n", path);

#ifdef SOCK_CLOEXEC
  retval = socketpair (AF_UNIX, SOCK_STREAM|SOCK_CLOEXEC, 0, fds);
  cloexec_done = (retval >= 0);

  if (retval < 0 && (errno == EINVAL || errno == EPROTOTYPE))
#endif
    {
      retval = socketpair (AF_UNIX, SOCK_STREAM, 0, fds);
    }

  if (retval < 0)
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (errno),
                      "Failed to create socket pair: %s",
                      _dbus_strerror (errno));
      return -1;
    }

  if (!cloexec_done)
    {
      _dbus_fd_set_close_on_exec (fds[0]);
      _dbus_fd_set_close_on_exec (fds[1]);
    }

  /* Make sure our output buffers aren't redundantly printed by both the
   * parent and the child */
  fflush (stdout);
  fflush (stderr);

  pid = fork ();
  if (pid < 0)
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (errno),
                      "Failed to fork() to call %s: %s",
                      path, _dbus_strerror (errno));
      close (fds[0]);
      close (fds[1]);
      return -1;
    }

  if (pid == 0)
    {
      /* child */
      close (fds[0]);

      dup2 (fds[1], STDIN_FILENO);
      dup2 (fds[1], STDOUT_FILENO);

      if (fds[1] != STDIN_FILENO &&
          fds[1] != STDOUT_FILENO)
        close (fds[1]);

      /* Inherit STDERR and the controlling terminal from the
         parent */

      _dbus_close_all ();

      execvp (path, (char * const *) argv);

      fprintf (stderr, "Failed to execute process %s: %s\n", path, _dbus_strerror (errno));

      _exit(1);
    }

  /* parent */
  close (fds[1]);

  if (!_dbus_set_fd_nonblocking (fds[0], error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);

      close (fds[0]);
      return -1;
    }

  return fds[0];
}

/**
 * Creates a socket and binds it to the given path,
 * then listens on the socket. The socket is
 * set to be nonblocking.
 *
 * Uses abstract sockets instead of filesystem-linked
 * sockets if requested (it's possible only on Linux;
 * see "man 7 unix" on Linux).
 * On non-Linux abstract socket usage always fails.
 *
 * This will set FD_CLOEXEC for the socket returned
 *
 * @param path the socket name
 * @param abstract #TRUE to use abstract namespace
 * @param error return location for errors
 * @returns the listening file descriptor or -1 on error
 */
int
_dbus_listen_unix_socket (const char     *path,
                          dbus_bool_t     abstract,
                          DBusError      *error)
{
  int listen_fd;
  struct sockaddr_un addr;
  size_t path_len;
  _DBUS_STATIC_ASSERT (sizeof (addr.sun_path) > _DBUS_MAX_SUN_PATH_LENGTH);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_verbose ("listening on unix socket %s abstract=%d\n",
                 path, abstract);

  if (!_dbus_open_unix_socket (&listen_fd, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET(error);
      return -1;
    }
  _DBUS_ASSERT_ERROR_IS_CLEAR(error);

  _DBUS_ZERO (addr);
  addr.sun_family = AF_UNIX;
  path_len = strlen (path);

  if (abstract)
    {
#ifdef __linux__
      /* remember that abstract names aren't nul-terminated so we rely
       * on sun_path being filled in with zeroes above.
       */
      addr.sun_path[0] = '\0'; /* this is what says "use abstract" */
      path_len++; /* Account for the extra nul byte added to the start of sun_path */

      if (path_len > _DBUS_MAX_SUN_PATH_LENGTH)
        {
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                      "Abstract socket name too long\n");
          _dbus_close (listen_fd, NULL);
          return -1;
	}

      strncpy (&addr.sun_path[1], path, sizeof (addr.sun_path) - 2);
      /* _dbus_verbose_bytes (addr.sun_path, sizeof (addr.sun_path)); */
#else /* !__linux__ */
      dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                      "Operating system does not support abstract socket namespace\n");
      _dbus_close (listen_fd, NULL);
      return -1;
#endif /* !__linux__ */
    }
  else
    {
      /* Discussed security implications of this with Nalin,
       * and we couldn't think of where it would kick our ass, but
       * it still seems a bit sucky. It also has non-security suckage;
       * really we'd prefer to exit if the socket is already in use.
       * But there doesn't seem to be a good way to do this.
       *
       * Just to be extra careful, I threw in the stat() - clearly
       * the stat() can't *fix* any security issue, but it at least
       * avoids inadvertent/accidental data loss.
       */
      {
        struct stat sb;

        if (stat (path, &sb) == 0 &&
            S_ISSOCK (sb.st_mode))
          unlink (path);
      }

      if (path_len > _DBUS_MAX_SUN_PATH_LENGTH)
        {
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                      "Socket name too long\n");
          _dbus_close (listen_fd, NULL);
          return -1;
	}

      strncpy (addr.sun_path, path, sizeof (addr.sun_path) - 1);
    }

  if (bind (listen_fd, (struct sockaddr*) &addr, _DBUS_STRUCT_OFFSET (struct sockaddr_un, sun_path) + path_len) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to bind socket \"%s\": %s",
                      path, _dbus_strerror (errno));
      _dbus_close (listen_fd, NULL);
      return -1;
    }

  if (listen (listen_fd, SOMAXCONN /* backlog */) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to listen on socket \"%s\": %s",
                      path, _dbus_strerror (errno));
      _dbus_close (listen_fd, NULL);
      return -1;
    }

  if (!_dbus_set_fd_nonblocking (listen_fd, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      _dbus_close (listen_fd, NULL);
      return -1;
    }

  /* Try opening up the permissions, but if we can't, just go ahead
   * and continue, maybe it will be good enough.
   */
  if (!abstract && chmod (path, 0777) < 0)
    _dbus_warn ("Could not set mode 0777 on socket %s", path);

  return listen_fd;
}

/**
 * Acquires one or more sockets passed in from systemd. The sockets
 * are set to be nonblocking.
 *
 * This will set FD_CLOEXEC for the sockets returned.
 *
 * @param fds the file descriptors
 * @param error return location for errors
 * @returns the number of file descriptors
 */
int
_dbus_listen_systemd_sockets (DBusSocket **fds,
                              DBusError   *error)
{
#ifdef HAVE_SYSTEMD
  int r, n;
  int fd;
  DBusSocket *new_fds;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  n = sd_listen_fds (TRUE);
  if (n < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (-n),
                      "Failed to acquire systemd socket: %s",
                      _dbus_strerror (-n));
      return -1;
    }

  if (n <= 0)
    {
      dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                      "No socket received.");
      return -1;
    }

  for (fd = SD_LISTEN_FDS_START; fd < SD_LISTEN_FDS_START + n; fd ++)
    {
      r = sd_is_socket (fd, AF_UNSPEC, SOCK_STREAM, 1);
      if (r < 0)
        {
          dbus_set_error (error, _dbus_error_from_errno (-r),
                          "Failed to verify systemd socket type: %s",
                          _dbus_strerror (-r));
          return -1;
        }

      if (!r)
        {
          dbus_set_error (error, DBUS_ERROR_BAD_ADDRESS,
                          "Passed socket has wrong type.");
          return -1;
        }
    }

  /* OK, the file descriptors are all good, so let's take posession of
     them then. */

  new_fds = dbus_new (DBusSocket, n);
  if (!new_fds)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                      "Failed to allocate file handle array.");
      goto fail;
    }

  for (fd = SD_LISTEN_FDS_START; fd < SD_LISTEN_FDS_START + n; fd ++)
    {
      if (!_dbus_set_fd_nonblocking (fd, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          goto fail;
        }

      new_fds[fd - SD_LISTEN_FDS_START].fd = fd;
    }

  *fds = new_fds;
  return n;

 fail:

  for (fd = SD_LISTEN_FDS_START; fd < SD_LISTEN_FDS_START + n; fd ++)
    {
      _dbus_close (fd, NULL);
    }

  dbus_free (new_fds);
  return -1;
#else
  dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
                        "dbus was compiled without systemd support");
  return -1;
#endif
}

/* Convert an error code from getaddrinfo() or getnameinfo() into
 * a D-Bus error name. */
static const char *
_dbus_error_from_gai (int gai_res,
                      int saved_errno)
{
  switch (gai_res)
    {
#ifdef EAI_FAMILY
      case EAI_FAMILY:
        /* ai_family not supported (at all) */
        return DBUS_ERROR_NOT_SUPPORTED;
#endif

#ifdef EAI_SOCKTYPE
      case EAI_SOCKTYPE:
        /* ai_socktype not supported (at all) */
        return DBUS_ERROR_NOT_SUPPORTED;
#endif

#ifdef EAI_MEMORY
      case EAI_MEMORY:
        /* Out of memory */
        return DBUS_ERROR_NO_MEMORY;
#endif

#ifdef EAI_SYSTEM
      case EAI_SYSTEM:
        /* Unspecified system error, details in errno */
        return _dbus_error_from_errno (saved_errno);
#endif

      case 0:
        /* It succeeded, but we didn't get any addresses? */
        return DBUS_ERROR_FAILED;

      /* EAI_AGAIN: Transient failure */
      /* EAI_BADFLAGS: invalid ai_flags (programming error) */
      /* EAI_FAIL: Non-recoverable failure */
      /* EAI_NODATA: host exists but has no addresses */
      /* EAI_NONAME: host does not exist */
      /* EAI_OVERFLOW: argument buffer overflow */
      /* EAI_SERVICE: service not available for specified socket
       * type (we should never see this because we use numeric
       * ports) */
      default:
        return DBUS_ERROR_FAILED;
    }
}

/**
 * Creates a socket and connects to a socket at the given host
 * and port. The connection fd is returned, and is set up as
 * nonblocking.
 *
 * This will set FD_CLOEXEC for the socket returned
 *
 * @param host the host name to connect to
 * @param port the port to connect to
 * @param family the address family to listen on, NULL for all
 * @param error return location for error code
 * @returns connection file descriptor or -1 on error
 */
DBusSocket
_dbus_connect_tcp_socket (const char     *host,
                          const char     *port,
                          const char     *family,
                          DBusError      *error)
{
    return _dbus_connect_tcp_socket_with_nonce (host, port, family, (const char*)NULL, error);
}

DBusSocket
_dbus_connect_tcp_socket_with_nonce (const char     *host,
                                     const char     *port,
                                     const char     *family,
                                     const char     *noncefile,
                                     DBusError      *error)
{
  int saved_errno = 0;
  DBusList *connect_errors = NULL;
  DBusSocket fd = DBUS_SOCKET_INIT;
  int res;
  struct addrinfo hints;
  struct addrinfo *ai = NULL;
  const struct addrinfo *tmp;
  DBusError *connect_error;

  _DBUS_ASSERT_ERROR_IS_CLEAR(error);

  _DBUS_ZERO (hints);

  if (!family)
    hints.ai_family = AF_UNSPEC;
  else if (!strcmp(family, "ipv4"))
    hints.ai_family = AF_INET;
  else if (!strcmp(family, "ipv6"))
    hints.ai_family = AF_INET6;
  else
    {
      dbus_set_error (error,
                      DBUS_ERROR_BAD_ADDRESS,
                      "Unknown address family %s", family);
      return _dbus_socket_get_invalid ();
    }
  hints.ai_protocol = IPPROTO_TCP;
  hints.ai_socktype = SOCK_STREAM;
  hints.ai_flags = AI_ADDRCONFIG;

  if ((res = getaddrinfo(host, port, &hints, &ai)) != 0)
    {
      dbus_set_error (error,
                      _dbus_error_from_gai (res, errno),
                      "Failed to lookup host/port: \"%s:%s\": %s (%d)",
                      host, port, gai_strerror(res), res);
      _dbus_socket_invalidate (&fd);
      goto out;
    }

  tmp = ai;
  while (tmp)
    {
      if (!_dbus_open_socket (&fd.fd, tmp->ai_family, SOCK_STREAM, 0, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET(error);
          _dbus_socket_invalidate (&fd);
          goto out;
        }
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);

      if (connect (fd.fd, (struct sockaddr*) tmp->ai_addr, tmp->ai_addrlen) < 0)
        {
          saved_errno = errno;
          _dbus_close (fd.fd, NULL);
          _dbus_socket_invalidate (&fd);

          connect_error = dbus_new0 (DBusError, 1);

          if (connect_error == NULL)
            {
              _DBUS_SET_OOM (error);
              goto out;
            }

          dbus_error_init (connect_error);
          _dbus_set_error_with_inet_sockaddr (connect_error,
                                              tmp->ai_addr, tmp->ai_addrlen,
                                              "Failed to connect to socket",
                                              saved_errno);

          if (!_dbus_list_append (&connect_errors, connect_error))
            {
              dbus_error_free (connect_error);
              dbus_free (connect_error);
              _DBUS_SET_OOM (error);
              goto out;
            }

          tmp = tmp->ai_next;
          continue;
        }

      break;
    }

  if (!_dbus_socket_is_valid (fd))
    {
      _dbus_combine_tcp_errors (&connect_errors, "Failed to connect",
                                host, port, error);
      goto out;
    }

  if (noncefile != NULL)
    {
      DBusString noncefileStr;
      dbus_bool_t ret;
      _dbus_string_init_const (&noncefileStr, noncefile);
      ret = _dbus_send_nonce (fd, &noncefileStr, error);

      if (!ret)
        {
          _dbus_close (fd.fd, NULL);
          _dbus_socket_invalidate (&fd);
          goto out;
        }
    }

  if (!_dbus_set_fd_nonblocking (fd.fd, error))
    {
      _dbus_close (fd.fd, NULL);
      _dbus_socket_invalidate (&fd);
      goto out;
    }

out:
  if (ai != NULL)
    freeaddrinfo (ai);

  while ((connect_error = _dbus_list_pop_first (&connect_errors)))
    {
      dbus_error_free (connect_error);
      dbus_free (connect_error);
    }

  return fd;
}

/**
 * Creates a socket and binds it to the given path, then listens on
 * the socket. The socket is set to be nonblocking.  In case of port=0
 * a random free port is used and returned in the port parameter.
 * If inaddr_any is specified, the hostname is ignored.
 *
 * This will set FD_CLOEXEC for the socket returned
 *
 * @param host the host name to listen on
 * @param port the port to listen on, if zero a free port will be used
 * @param family the address family to listen on, NULL for all
 * @param retport string to return the actual port listened on
 * @param retfamily string to return the actual family listened on
 * @param fds_p location to store returned file descriptors
 * @param error return location for errors
 * @returns the number of listening file descriptors or -1 on error
 */
int
_dbus_listen_tcp_socket (const char     *host,
                         const char     *port,
                         const char     *family,
                         DBusString     *retport,
                         const char    **retfamily,
                         DBusSocket    **fds_p,
                         DBusError      *error)
{
  int saved_errno;
  int nlisten_fd = 0, res, i;
  DBusList *bind_errors = NULL;
  DBusError *bind_error = NULL;
  DBusSocket *listen_fd = NULL;
  struct addrinfo hints;
  struct addrinfo *ai, *tmp;
  unsigned int reuseaddr;
  dbus_bool_t have_ipv4 = FALSE;
  dbus_bool_t have_ipv6 = FALSE;

  *fds_p = NULL;
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _DBUS_ZERO (hints);

  if (!family)
    hints.ai_family = AF_UNSPEC;
  else if (!strcmp(family, "ipv4"))
    hints.ai_family = AF_INET;
  else if (!strcmp(family, "ipv6"))
    hints.ai_family = AF_INET6;
  else
    {
      dbus_set_error (error,
                      DBUS_ERROR_BAD_ADDRESS,
                      "Unknown address family %s", family);
      return -1;
    }

  hints.ai_protocol = IPPROTO_TCP;
  hints.ai_socktype = SOCK_STREAM;
  hints.ai_flags = AI_ADDRCONFIG | AI_PASSIVE;

 redo_lookup_with_port:
  ai = NULL;
  if ((res = getaddrinfo(host, port, &hints, &ai)) != 0 || !ai)
    {
      dbus_set_error (error,
                      _dbus_error_from_gai (res, errno),
                      "Failed to lookup host/port: \"%s:%s\": %s (%d)",
                      host ? host : "*", port, gai_strerror(res), res);
      goto failed;
    }

  tmp = ai;
  while (tmp)
    {
      int fd = -1, tcp_nodelay_on;
      DBusSocket *newlisten_fd;

      if (!_dbus_open_socket (&fd, tmp->ai_family, SOCK_STREAM, 0, error))
        {
          _DBUS_ASSERT_ERROR_IS_SET(error);
          goto failed;
        }
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);

      reuseaddr = 1;
      if (setsockopt (fd, SOL_SOCKET, SO_REUSEADDR, &reuseaddr, sizeof(reuseaddr))==-1)
        {
          _dbus_warn ("Failed to set socket option \"%s:%s\": %s",
                      host ? host : "*", port, _dbus_strerror (errno));
        }

      /* Nagle's algorithm imposes a huge delay on the initial messages
         going over TCP. */
      tcp_nodelay_on = 1;
      if (setsockopt (fd, IPPROTO_TCP, TCP_NODELAY, &tcp_nodelay_on, sizeof (tcp_nodelay_on)) == -1)
        {
          _dbus_warn ("Failed to set TCP_NODELAY socket option \"%s:%s\": %s",
                      host ? host : "*", port, _dbus_strerror (errno));
        }

      if (bind (fd, (struct sockaddr*) tmp->ai_addr, tmp->ai_addrlen) < 0)
        {
          saved_errno = errno;
          _dbus_close(fd, NULL);

          /*
           * We don't treat this as a fatal error, because there might be
           * other addresses that we can listen on. In particular:
           *
           * - If saved_errno is EADDRINUSE after we
           *   "goto redo_lookup_with_port" after binding a port on one of the
           *   possible addresses, we will try to bind that same port on
           *   every address, including the same address again for a second
           *   time, which will fail with EADDRINUSE.
           *
           * - If saved_errno is EADDRINUSE, it might be because binding to
           *   an IPv6 address implicitly binds to a corresponding IPv4
           *   address or vice versa (e.g. Linux with bindv6only=0).
           *
           * - If saved_errno is EADDRNOTAVAIL when we asked for family
           *   AF_UNSPEC, it might be because IPv6 is disabled for this
           *   particular interface (e.g. Linux with
           *   /proc/sys/net/ipv6/conf/lo/disable_ipv6).
           */
          bind_error = dbus_new0 (DBusError, 1);

          if (bind_error == NULL)
            {
              _DBUS_SET_OOM (error);
              goto failed;
            }

          dbus_error_init (bind_error);
          _dbus_set_error_with_inet_sockaddr (bind_error, tmp->ai_addr, tmp->ai_addrlen,
                                              "Failed to bind socket",
                                              saved_errno);

          if (!_dbus_list_append (&bind_errors, bind_error))
            {
              dbus_error_free (bind_error);
              dbus_free (bind_error);
              _DBUS_SET_OOM (error);
              goto failed;
            }

          /* Try the next address, maybe it will work better */
          tmp = tmp->ai_next;
          continue;
        }

      if (listen (fd, 30 /* backlog */) < 0)
        {
          saved_errno = errno;
          _dbus_close (fd, NULL);
          _dbus_set_error_with_inet_sockaddr (error, tmp->ai_addr, tmp->ai_addrlen,
                                              "Failed to listen on socket",
                                              saved_errno);
          goto failed;
        }

      newlisten_fd = dbus_realloc(listen_fd, sizeof(DBusSocket)*(nlisten_fd+1));
      if (!newlisten_fd)
        {
          _dbus_close (fd, NULL);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                          "Failed to allocate file handle array");
          goto failed;
        }
      listen_fd = newlisten_fd;
      listen_fd[nlisten_fd].fd = fd;
      nlisten_fd++;

      if (tmp->ai_addr->sa_family == AF_INET)
        have_ipv4 = TRUE;
      else if (tmp->ai_addr->sa_family == AF_INET6)
        have_ipv6 = TRUE;

      if (!_dbus_string_get_length(retport))
        {
          /* If the user didn't specify a port, or used 0, then
             the kernel chooses a port. After the first address
             is bound to, we need to force all remaining addresses
             to use the same port */
          if (!port || !strcmp(port, "0"))
            {
              int result;
              struct sockaddr_storage addr;
              socklen_t addrlen;
              char portbuf[50];

              addrlen = sizeof(addr);
              result = getsockname(fd, (struct sockaddr*) &addr, &addrlen);

              if (result == -1)
                {
                  saved_errno = errno;
                  dbus_set_error (error, _dbus_error_from_errno (saved_errno),
                                  "Failed to retrieve socket name for \"%s:%s\": %s",
                                  host ? host : "*", port, _dbus_strerror (saved_errno));
                  goto failed;
                }

              if ((res = getnameinfo ((struct sockaddr*)&addr, addrlen, NULL, 0,
                                      portbuf, sizeof(portbuf),
                                      NI_NUMERICHOST | NI_NUMERICSERV)) != 0)
                {
                  saved_errno = errno;
                  dbus_set_error (error, _dbus_error_from_gai (res, saved_errno),
                                  "Failed to resolve port \"%s:%s\": %s (%d)",
                                  host ? host : "*", port, gai_strerror(res), res);
                  goto failed;
                }

              if (!_dbus_string_append(retport, portbuf))
                {
                  dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
                  goto failed;
                }

              /* Release current address list & redo lookup */
              port = _dbus_string_get_const_data(retport);
              freeaddrinfo(ai);
              goto redo_lookup_with_port;
            }
          else
            {
              if (!_dbus_string_append(retport, port))
                {
                    dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
                    goto failed;
                }
            }
        }

      tmp = tmp->ai_next;
    }
  freeaddrinfo(ai);
  ai = NULL;

  if (!nlisten_fd)
    {
      _dbus_combine_tcp_errors (&bind_errors, "Failed to bind", host,
                                port, error);
      goto failed;
    }

  if (have_ipv4 && !have_ipv6)
    *retfamily = "ipv4";
  else if (!have_ipv4 && have_ipv6)
    *retfamily = "ipv6";

  for (i = 0 ; i < nlisten_fd ; i++)
    {
      if (!_dbus_set_fd_nonblocking (listen_fd[i].fd, error))
        {
          goto failed;
        }
    }

  *fds_p = listen_fd;

  /* This list might be non-empty even on success, because we might be
   * ignoring EADDRINUSE or EADDRNOTAVAIL */
  while ((bind_error = _dbus_list_pop_first (&bind_errors)))
    {
      dbus_error_free (bind_error);
      dbus_free (bind_error);
    }

  return nlisten_fd;

 failed:
  if (ai)
    freeaddrinfo(ai);
  for (i = 0 ; i < nlisten_fd ; i++)
    _dbus_close(listen_fd[i].fd, NULL);

  while ((bind_error = _dbus_list_pop_first (&bind_errors)))
    {
      dbus_error_free (bind_error);
      dbus_free (bind_error);
    }

  dbus_free(listen_fd);
  return -1;
}

static dbus_bool_t
write_credentials_byte (int             server_fd,
                        DBusError      *error)
{
  int bytes_written;
  char buf[1] = { '\0' };
#if defined(HAVE_CMSGCRED)
  union {
	  struct cmsghdr hdr;
	  char cred[CMSG_SPACE (sizeof (struct cmsgcred))];
  } cmsg;
  struct iovec iov;
  struct msghdr msg;
  iov.iov_base = buf;
  iov.iov_len = 1;

  _DBUS_ZERO(msg);
  msg.msg_iov = &iov;
  msg.msg_iovlen = 1;

  msg.msg_control = (caddr_t) &cmsg;
  msg.msg_controllen = CMSG_SPACE (sizeof (struct cmsgcred));
  _DBUS_ZERO(cmsg);
  cmsg.hdr.cmsg_len = CMSG_LEN (sizeof (struct cmsgcred));
  cmsg.hdr.cmsg_level = SOL_SOCKET;
  cmsg.hdr.cmsg_type = SCM_CREDS;
#endif

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

 again:

#if defined(HAVE_CMSGCRED)
  bytes_written = sendmsg (server_fd, &msg, 0
#if HAVE_DECL_MSG_NOSIGNAL
                           |MSG_NOSIGNAL
#endif
                           );

  /* If we HAVE_CMSGCRED, the OS still might not let us sendmsg()
   * with a SOL_SOCKET/SCM_CREDS message - for instance, FreeBSD
   * only allows that on AF_UNIX. Try just doing a send() instead. */
  if (bytes_written < 0 && errno == EINVAL)
#endif
    {
      bytes_written = send (server_fd, buf, 1, 0
#if HAVE_DECL_MSG_NOSIGNAL
                            |MSG_NOSIGNAL
#endif
                            );
    }

  if (bytes_written < 0 && errno == EINTR)
    goto again;

  if (bytes_written < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to write credentials byte: %s",
                     _dbus_strerror (errno));
      return FALSE;
    }
  else if (bytes_written == 0)
    {
      dbus_set_error (error, DBUS_ERROR_IO_ERROR,
                      "wrote zero bytes writing credentials byte");
      return FALSE;
    }
  else
    {
      _dbus_assert (bytes_written == 1);
      _dbus_verbose ("wrote credentials byte\n");
      return TRUE;
    }
}

/* return FALSE on OOM, TRUE otherwise, even if no groups were found */
static dbus_bool_t
add_groups_to_credentials (int              client_fd,
                           DBusCredentials *credentials,
                           dbus_gid_t       primary)
{
#if defined(__linux__) && defined(SO_PEERGROUPS)
  _DBUS_STATIC_ASSERT (sizeof (gid_t) <= sizeof (dbus_gid_t));
  gid_t *buf = NULL;
  socklen_t len = 1024;
  dbus_bool_t oom = FALSE;
  /* libdbus has a different representation of group IDs just to annoy you */
  dbus_gid_t *converted_gids = NULL;
  dbus_bool_t need_primary = TRUE;
  size_t n_gids;
  size_t i;

  n_gids = ((size_t) len) / sizeof (gid_t);
  buf = dbus_new (gid_t, n_gids);

  if (buf == NULL)
    return FALSE;

  while (getsockopt (client_fd, SOL_SOCKET, SO_PEERGROUPS, buf, &len) < 0)
    {
      int e = errno;
      gid_t *replacement;

      _dbus_verbose ("getsockopt failed with %s, len now %lu\n",
                     _dbus_strerror (e), (unsigned long) len);

      if (e != ERANGE || (size_t) len <= n_gids * sizeof (gid_t))
        {
          _dbus_verbose ("Failed to getsockopt(SO_PEERGROUPS): %s\n",
                         _dbus_strerror (e));
          goto out;
        }

      /* If not enough space, len is updated to be enough.
       * Try again with a large enough buffer. */
      n_gids = ((size_t) len) / sizeof (gid_t);
      replacement = dbus_realloc (buf, len);

      if (replacement == NULL)
        {
          oom = TRUE;
          goto out;
        }

      buf = replacement;
      _dbus_verbose ("will try again with %lu\n", (unsigned long) len);
    }

  if (len <= 0)
    {
      _dbus_verbose ("getsockopt(SO_PEERGROUPS) yielded <= 0 bytes: %ld\n",
                     (long) len);
      goto out;
    }

  if (len > n_gids * sizeof (gid_t))
    {
      _dbus_verbose ("%lu > %zu", (unsigned long) len, n_gids * sizeof (gid_t));
      _dbus_assert_not_reached ("getsockopt(SO_PEERGROUPS) overflowed");
    }

  if (len % sizeof (gid_t) != 0)
    {
      _dbus_verbose ("getsockopt(SO_PEERGROUPS) did not return an "
                     "integer multiple of sizeof(gid_t): %lu should be "
                     "divisible by %zu",
                     (unsigned long) len, sizeof (gid_t));
      goto out;
    }

  /* Allocate an extra space for the primary group ID */
  n_gids = ((size_t) len) / sizeof (gid_t);

  /* If n_gids is less than this, then (n_gids + 1) certainly doesn't
   * overflow, and neither does multiplying that by sizeof(dbus_gid_t).
   * This is using _DBUS_INT32_MAX as a conservative lower bound for
   * the maximum size_t. */
  if (n_gids >= (_DBUS_INT32_MAX / sizeof (dbus_gid_t)) - 1)
    {
      _dbus_verbose ("getsockopt(SO_PEERGROUPS) returned a huge number "
                     "of groups (%lu bytes), ignoring",
                     (unsigned long) len);
      goto out;
    }

  converted_gids = dbus_new (dbus_gid_t, n_gids + 1);

  if (converted_gids == NULL)
    {
      oom = TRUE;
      goto out;
    }

  for (i = 0; i < n_gids; i++)
    {
      converted_gids[i] = (dbus_gid_t) buf[i];

      if (converted_gids[i] == primary)
        need_primary = FALSE;
    }

  if (need_primary && primary != DBUS_GID_UNSET)
    {
      converted_gids[n_gids] = primary;
      n_gids++;
    }

  _dbus_credentials_take_unix_gids (credentials, converted_gids, n_gids);

out:
  dbus_free (buf);
  return !oom;
#else
  /* no error */
  return TRUE;
#endif
}

/* return FALSE on OOM, TRUE otherwise, even if no credentials were found */
static dbus_bool_t
add_linux_security_label_to_credentials (int              client_fd,
                                         DBusCredentials *credentials)
{
#if defined(__linux__) && defined(SO_PEERSEC)
  DBusString buf;
  socklen_t len = 1024;
  dbus_bool_t oom = FALSE;

  if (!_dbus_string_init_preallocated (&buf, len) ||
      !_dbus_string_set_length (&buf, len))
    return FALSE;

  while (getsockopt (client_fd, SOL_SOCKET, SO_PEERSEC,
         _dbus_string_get_data (&buf), &len) < 0)
    {
      int e = errno;

      _dbus_verbose ("getsockopt failed with %s, len now %lu\n",
                     _dbus_strerror (e), (unsigned long) len);

      if (e != ERANGE || len <= _dbus_string_get_length_uint (&buf))
        {
          _dbus_verbose ("Failed to getsockopt(SO_PEERSEC): %s\n",
                         _dbus_strerror (e));
          goto out;
        }

      /* If not enough space, len is updated to be enough.
       * Try again with a large enough buffer. */
      if (!_dbus_string_set_length (&buf, len))
        {
          oom = TRUE;
          goto out;
        }

      _dbus_verbose ("will try again with %lu\n", (unsigned long) len);
    }

  if (len <= 0)
    {
      _dbus_verbose ("getsockopt(SO_PEERSEC) yielded <= 0 bytes: %lu\n",
                     (unsigned long) len);
      goto out;
    }

  if (len > _dbus_string_get_length_uint (&buf))
    {
      _dbus_verbose ("%lu > %u", (unsigned long) len,
                     _dbus_string_get_length_uint (&buf));
      _dbus_assert_not_reached ("getsockopt(SO_PEERSEC) overflowed");
    }

  if (_dbus_string_get_byte (&buf, len - 1) == 0)
    {
      /* the kernel included the trailing \0 in its count,
       * but DBusString always has an extra \0 after the data anyway */
      _dbus_verbose ("subtracting trailing \\0\n");
      len--;
    }

  if (!_dbus_string_set_length (&buf, len))
    {
      _dbus_assert_not_reached ("shortening string should not lead to OOM");
      oom = TRUE;
      goto out;
    }

  if (strlen (_dbus_string_get_const_data (&buf)) != len)
    {
      /* LSM people on the linux-security-module@ mailing list say this
       * should never happen: the label should be a bytestring with
       * an optional trailing \0 */
      _dbus_verbose ("security label from kernel had an embedded \\0, "
                     "ignoring it\n");
      goto out;
    }

  _dbus_verbose ("getsockopt(SO_PEERSEC): %lu bytes excluding \\0: %s\n",
                 (unsigned long) len,
                 _dbus_string_get_const_data (&buf));

  if (!_dbus_credentials_add_linux_security_label (credentials,
        _dbus_string_get_const_data (&buf)))
    {
      oom = TRUE;
      goto out;
    }

out:
  _dbus_string_free (&buf);
  return !oom;
#else
  /* no error */
  return TRUE;
#endif
}

/**
 * Reads a single byte which must be nul (an error occurs otherwise),
 * and reads unix credentials if available. Clears the credentials
 * object, then adds pid/uid if available, so any previous credentials
 * stored in the object are lost.
 *
 * DBusServer makes the security assumption that the credentials
 * returned by this method are the credentials that were active
 * at the time the socket was opened. Do not add APIs to this
 * method that would break that assumption.
 *
 * In particular, it is incorrect to use any API of the form
 * "get the process ID at the other end of the connection, then
 * determine its uid, gid, or other credentials from the pid"
 * (e.g. looking in /proc on Linux). If we did that, we would
 * be vulnerable to several attacks. A malicious process could
 * queue up the rest of the authentication handshake and a malicious
 * message that it should not be allowed to send, then race with
 * the DBusServer to exec() a more privileged (e.g. setuid) binary that
 * would have been allowed to send that message; or it could exit,
 * and arrange for enough setuid processes to be started that its
 * pid would be recycled for one of those processes with high
 * probability; or it could fd-pass the connection to a more
 * privileged process.
 *
 * Return value indicates whether a byte was read, not whether
 * we got valid credentials. On some systems, such as Linux,
 * reading/writing the byte isn't actually required, but we do it
 * anyway just to avoid multiple codepaths.
 *
 * Fails if no byte is available, so you must select() first.
 *
 * The point of the byte is that on some systems we have to
 * use sendmsg()/recvmsg() to transmit credentials.
 *
 * @param client_fd the client file descriptor
 * @param credentials object to add client credentials to
 * @param error location to store error code
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_read_credentials_socket  (DBusSocket       client_fd,
                                DBusCredentials *credentials,
                                DBusError       *error)
{
  struct msghdr msg;
  struct iovec iov;
  char buf;
  dbus_uid_t uid_read;
  dbus_gid_t primary_gid_read;
  dbus_pid_t pid_read;
  int bytes_read;

#ifdef HAVE_CMSGCRED
  union {
    struct cmsghdr hdr;
    char cred[CMSG_SPACE (sizeof (struct cmsgcred))];
  } cmsg;
#endif

  /* The POSIX spec certainly doesn't promise this, but
   * we need these assertions to fail as soon as we're wrong about
   * it so we can do the porting fixups
   */
  _DBUS_STATIC_ASSERT (sizeof (pid_t) <= sizeof (dbus_pid_t));
  _DBUS_STATIC_ASSERT (sizeof (uid_t) <= sizeof (dbus_uid_t));
  _DBUS_STATIC_ASSERT (sizeof (gid_t) <= sizeof (dbus_gid_t));

  uid_read = DBUS_UID_UNSET;
  primary_gid_read = DBUS_GID_UNSET;
  pid_read = DBUS_PID_UNSET;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_credentials_clear (credentials);

  iov.iov_base = &buf;
  iov.iov_len = 1;

  _DBUS_ZERO(msg);
  msg.msg_iov = &iov;
  msg.msg_iovlen = 1;

#if defined(HAVE_CMSGCRED)
  _DBUS_ZERO(cmsg);
  msg.msg_control = (caddr_t) &cmsg;
  msg.msg_controllen = CMSG_SPACE (sizeof (struct cmsgcred));
#endif

 again:
  bytes_read = recvmsg (client_fd.fd, &msg, 0);

  if (bytes_read < 0)
    {
      if (errno == EINTR)
	goto again;

      /* EAGAIN or EWOULDBLOCK would be unexpected here since we would
       * normally only call read_credentials if the socket was ready
       * for reading
       */

      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to read credentials byte: %s",
                      _dbus_strerror (errno));
      return FALSE;
    }
  else if (bytes_read == 0)
    {
      /* this should not happen unless we are using recvmsg wrong,
       * so is essentially here for paranoia
       */
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to read credentials byte (zero-length read)");
      return FALSE;
    }
  else if (buf != '\0')
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Credentials byte was not nul");
      return FALSE;
    }

  _dbus_verbose ("read credentials byte\n");

  {
#ifdef SO_PEERCRED
    /* Supported by at least Linux and OpenBSD, with minor differences.
     *
     * This mechanism passes the process ID through and does not require
     * the peer's cooperation, so we prefer it over all others. Notably,
     * Linux also supports SCM_CREDENTIALS, which is similar to FreeBSD
     * SCM_CREDS; it's implemented in GIO, but we don't use it in dbus at all,
     * because this is much less fragile.
     */
#ifdef __OpenBSD__
    struct sockpeercred cr;
#else
    struct ucred cr;
#endif
    socklen_t cr_len = sizeof (cr);

    if (getsockopt (client_fd.fd, SOL_SOCKET, SO_PEERCRED, &cr, &cr_len) != 0)
      {
        _dbus_verbose ("Failed to getsockopt(SO_PEERCRED): %s\n",
                       _dbus_strerror (errno));
      }
    else if (cr_len != sizeof (cr))
      {
        _dbus_verbose ("Failed to getsockopt(SO_PEERCRED), returned %d bytes, expected %d\n",
                       cr_len, (int) sizeof (cr));
      }
    else
      {
        pid_read = cr.pid;
        uid_read = cr.uid;
#ifdef __linux__
        /* Do other platforms have cr.gid? (Not that it really matters,
         * because the gid is useless to us unless we know the complete
         * group vector, which we only know on Linux.) */
        primary_gid_read = cr.gid;
#endif
      }
#elif defined(HAVE_UNPCBID) && defined(LOCAL_PEEREID)
    /* Another variant of the above - used on NetBSD
     */
    struct unpcbid cr;
    socklen_t cr_len = sizeof (cr);

    if (getsockopt (client_fd.fd, 0, LOCAL_PEEREID, &cr, &cr_len) != 0)
      {
        _dbus_verbose ("Failed to getsockopt(LOCAL_PEEREID): %s\n",
                       _dbus_strerror (errno));
      }
    else if (cr_len != sizeof (cr))
      {
        _dbus_verbose ("Failed to getsockopt(LOCAL_PEEREID), returned %d bytes, expected %d\n",
                       cr_len, (int) sizeof (cr));
      }
    else
      {
        pid_read = cr.unp_pid;
        uid_read = cr.unp_euid;
      }
#elif defined(HAVE_CMSGCRED)
    /* We only check for HAVE_CMSGCRED, but we're really assuming that the
     * presence of that struct implies SCM_CREDS. Supported by at least
     * FreeBSD and DragonflyBSD.
     *
     * This mechanism requires the peer to help us (it has to send us a
     * SCM_CREDS message) but it does pass the process ID through,
     * which makes it better than getpeereid().
     */
    struct cmsgcred *cred;
    struct cmsghdr *cmsgp;

    for (cmsgp = CMSG_FIRSTHDR (&msg);
         cmsgp != NULL;
         cmsgp = CMSG_NXTHDR (&msg, cmsgp))
      {
        if (cmsgp->cmsg_type == SCM_CREDS &&
            cmsgp->cmsg_level == SOL_SOCKET &&
            cmsgp->cmsg_len >= CMSG_LEN (sizeof (struct cmsgcred)))
          {
            cred = (struct cmsgcred *) CMSG_DATA (cmsgp);
            pid_read = cred->cmcred_pid;
            uid_read = cred->cmcred_euid;
            break;
          }
      }

#elif defined(HAVE_GETPEERUCRED)
    /* Supported in at least Solaris >= 10. It should probably be higher
     * up this list, because it carries the pid and we use this code path
     * for audit data. */
    ucred_t * ucred = NULL;
    if (getpeerucred (client_fd.fd, &ucred) == 0)
      {
#ifdef HAVE_ADT
        adt_session_data_t *adth = NULL;
#endif
        pid_read = ucred_getpid (ucred);
        uid_read = ucred_geteuid (ucred);
#ifdef HAVE_ADT
        /* generate audit session data based on socket ucred */
        if (adt_start_session (&adth, NULL, 0) || (adth == NULL))
          {
            _dbus_verbose ("Failed to adt_start_session(): %s\n", _dbus_strerror (errno));
          }
        else
          {
            if (adt_set_from_ucred (adth, ucred, ADT_NEW))
              {
                _dbus_verbose ("Failed to adt_set_from_ucred(): %s\n", _dbus_strerror (errno));
              }
            else
              {
                adt_export_data_t *data = NULL;
                size_t size = adt_export_session_data (adth, &data);
                if (size <= 0)
                  {
                    _dbus_verbose ("Failed to adt_export_session_data(): %s\n", _dbus_strerror (errno));
                  }
                else
                  {
                    _dbus_credentials_add_adt_audit_data (credentials, data, size);
                    free (data);
                  }
              }
            (void) adt_end_session (adth);
          }
#endif /* HAVE_ADT */
      }
    else
      {
        _dbus_verbose ("Failed to getpeerucred() credentials: %s\n", _dbus_strerror (errno));
      }
    if (ucred != NULL)
      ucred_free (ucred);

    /* ----------------------------------------------------------------
     * When adding new mechanisms, please add them above this point
     * if they support passing the process ID through, or below if not.
     * ---------------------------------------------------------------- */

#elif defined(HAVE_GETPEEREID)
    /* getpeereid() originates from D.J. Bernstein and is fairly
     * widely-supported. According to a web search, it might be present in
     * any/all of:
     *
     * - AIX?
     * - Blackberry?
     * - Cygwin
     * - FreeBSD 4.6+ (but we prefer SCM_CREDS: it carries the pid)
     * - Mac OS X
     * - Minix 3.1.8+
     * - MirBSD?
     * - NetBSD 5.0+ (but LOCAL_PEEREID would be better: it carries the pid)
     * - OpenBSD 3.0+ (but we prefer SO_PEERCRED: it carries the pid)
     * - QNX?
     */
    uid_t euid;
    gid_t egid;
    if (getpeereid (client_fd.fd, &euid, &egid) == 0)
      {
        uid_read = euid;
      }
    else
      {
        _dbus_verbose ("Failed to getpeereid() credentials: %s\n", _dbus_strerror (errno));
      }
#else /* no supported mechanism */

#warning Socket credentials not supported on this Unix OS
#warning Please tell https://bugs.freedesktop.org/enter_bug.cgi?product=DBus

    /* Please add other operating systems known to support at least one of
     * the mechanisms above to this list, keeping alphabetical order.
     * Everything not in this list  is best-effort.
     */
#if defined(__FreeBSD__) || defined(__FreeBSD_kernel__) || \
    defined(__linux__) || \
    defined(__OpenBSD__) || \
    defined(__NetBSD__)
# error Credentials passing not working on this OS is a regression!
#endif

    _dbus_verbose ("Socket credentials not supported on this OS\n");
#endif
  }

  _dbus_verbose ("Credentials:"
                 "  pid "DBUS_PID_FORMAT
                 "  uid "DBUS_UID_FORMAT
                 "\n",
		 pid_read,
		 uid_read);

  if (pid_read != DBUS_PID_UNSET)
    {
      if (!_dbus_credentials_add_pid (credentials, pid_read))
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }
    }

  if (uid_read != DBUS_UID_UNSET)
    {
      if (!_dbus_credentials_add_unix_uid (credentials, uid_read))
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }
    }

  if (!add_linux_security_label_to_credentials (client_fd.fd, credentials))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  /* We don't put any groups in the credentials unless we can put them
   * all there. */
  if (!add_groups_to_credentials (client_fd.fd, credentials, primary_gid_read))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

/**
 * Sends a single nul byte with our UNIX credentials as ancillary
 * data.  Returns #TRUE if the data was successfully written.  On
 * systems that don't support sending credentials, just writes a byte,
 * doesn't send any credentials.  On some systems, such as Linux,
 * reading/writing the byte isn't actually required, but we do it
 * anyway just to avoid multiple codepaths.
 *
 * Fails if no byte can be written, so you must select() first.
 *
 * The point of the byte is that on some systems we have to
 * use sendmsg()/recvmsg() to transmit credentials.
 *
 * @param server_fd file descriptor for connection to server
 * @param error return location for error code
 * @returns #TRUE if the byte was sent
 */
dbus_bool_t
_dbus_send_credentials_socket  (DBusSocket       server_fd,
                                DBusError       *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (write_credentials_byte (server_fd.fd, error))
    return TRUE;
  else
    return FALSE;
}

/**
 * Accepts a connection on a listening socket.
 * Handles EINTR for you.
 *
 * This will enable FD_CLOEXEC for the returned socket.
 *
 * @param listen_fd the listen file descriptor
 * @returns the connection fd of the client, or -1 on error
 */
DBusSocket
_dbus_accept  (DBusSocket listen_fd)
{
  DBusSocket client_fd;
  struct sockaddr addr;
  socklen_t addrlen;
#ifdef HAVE_ACCEPT4
  dbus_bool_t cloexec_done;
#endif

  addrlen = sizeof (addr);

 retry:

#ifdef HAVE_ACCEPT4
  /*
   * At compile-time, we assume that if accept4() is available in
   * libc headers, SOCK_CLOEXEC is too. At runtime, it is still
   * not necessarily true that either is supported by the running kernel.
   */
  client_fd.fd = accept4 (listen_fd.fd, &addr, &addrlen, SOCK_CLOEXEC);
  cloexec_done = client_fd.fd >= 0;

  if (client_fd.fd < 0 && (errno == ENOSYS || errno == EINVAL))
#endif
    {
      client_fd.fd = accept (listen_fd.fd, &addr, &addrlen);
    }

  if (client_fd.fd < 0)
    {
      if (errno == EINTR)
        goto retry;
    }

  _dbus_verbose ("client fd %d accepted\n", client_fd.fd);

#ifdef HAVE_ACCEPT4
  if (!cloexec_done)
#endif
    {
      _dbus_fd_set_close_on_exec(client_fd.fd);
    }

  return client_fd;
}

/**
 * Checks to make sure the given directory is
 * private to the user
 *
 * @param dir the name of the directory
 * @param error error return
 * @returns #FALSE on failure
 **/
dbus_bool_t
_dbus_check_dir_is_private_to_user (DBusString *dir, DBusError *error)
{
  const char *directory;
  struct stat sb;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  directory = _dbus_string_get_const_data (dir);

  if (stat (directory, &sb) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "%s", _dbus_strerror (errno));

      return FALSE;
    }

  if (sb.st_uid != geteuid ())
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                     "%s directory is owned by user %lu, not %lu",
                     directory,
                     (unsigned long) sb.st_uid,
                     (unsigned long) geteuid ());
      return FALSE;
    }

  if ((S_IROTH & sb.st_mode) || (S_IWOTH & sb.st_mode) ||
      (S_IRGRP & sb.st_mode) || (S_IWGRP & sb.st_mode))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                     "%s directory is not private to the user", directory);
      return FALSE;
    }

  return TRUE;
}

static dbus_bool_t
fill_user_info_from_passwd (struct passwd *p,
                            DBusUserInfo  *info,
                            DBusError     *error)
{
  _dbus_assert (p->pw_name != NULL);
  _dbus_assert (p->pw_dir != NULL);

  info->uid = p->pw_uid;
  info->primary_gid = p->pw_gid;
  info->username = _dbus_strdup (p->pw_name);
  info->homedir = _dbus_strdup (p->pw_dir);

  if (info->username == NULL ||
      info->homedir == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return FALSE;
    }

  return TRUE;
}

static dbus_bool_t
fill_user_info (DBusUserInfo       *info,
                dbus_uid_t          uid,
                const DBusString   *username,
                DBusError          *error)
{
  const char *username_c;

  /* exactly one of username/uid provided */
  _dbus_assert (username != NULL || uid != DBUS_UID_UNSET);
  _dbus_assert (username == NULL || uid == DBUS_UID_UNSET);

  info->uid = DBUS_UID_UNSET;
  info->primary_gid = DBUS_GID_UNSET;
  info->group_ids = NULL;
  info->n_group_ids = 0;
  info->username = NULL;
  info->homedir = NULL;

  if (username != NULL)
    username_c = _dbus_string_get_const_data (username);
  else
    username_c = NULL;

  /* For now assuming that the getpwnam() and getpwuid() flavors
   * are always symmetrical, if not we have to add more configure
   * checks
   */

#ifdef HAVE_GETPWNAM_R
  {
    struct passwd *p;
    int result;
    size_t buflen;
    char *buf;
    struct passwd p_str;

    /* retrieve maximum needed size for buf */
    buflen = sysconf (_SC_GETPW_R_SIZE_MAX);

    /* sysconf actually returns a long, but everything else expects size_t,
     * so just recast here.
     * https://bugs.freedesktop.org/show_bug.cgi?id=17061
     */
    if ((long) buflen <= 0)
      buflen = 1024;

    result = -1;
    while (1)
      {
        buf = dbus_malloc (buflen);
        if (buf == NULL)
          {
            dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
            return FALSE;
          }

        p = NULL;
        if (uid != DBUS_UID_UNSET)
          result = getpwuid_r (uid, &p_str, buf, buflen,
                               &p);
        else
          result = getpwnam_r (username_c, &p_str, buf, buflen,
                               &p);
        //Try a bigger buffer if ERANGE was returned
        if (result == ERANGE && buflen < 512 * 1024)
          {
            dbus_free (buf);
            buflen *= 2;
          }
        else
          {
            break;
          }
      }
    if (result == 0 && p == &p_str)
      {
        if (!fill_user_info_from_passwd (p, info, error))
          {
            dbus_free (buf);
            return FALSE;
          }
        dbus_free (buf);
      }
    else
      {
        dbus_set_error (error, _dbus_error_from_errno (errno),
                        "User \"%s\" unknown or no memory to allocate password entry\n",
                        username_c ? username_c : "???");
        _dbus_verbose ("User %s unknown\n", username_c ? username_c : "???");
        dbus_free (buf);
        return FALSE;
      }
  }
#else /* ! HAVE_GETPWNAM_R */
  {
    /* I guess we're screwed on thread safety here */
    struct passwd *p;

#warning getpwnam_r() not available, please report this to the dbus maintainers with details of your OS

    if (uid != DBUS_UID_UNSET)
      p = getpwuid (uid);
    else
      p = getpwnam (username_c);

    if (p != NULL)
      {
        if (!fill_user_info_from_passwd (p, info, error))
          {
            return FALSE;
          }
      }
    else
      {
        dbus_set_error (error, _dbus_error_from_errno (errno),
                        "User \"%s\" unknown or no memory to allocate password entry\n",
                        username_c ? username_c : "???");
        _dbus_verbose ("User %s unknown\n", username_c ? username_c : "???");
        return FALSE;
      }
  }
#endif  /* ! HAVE_GETPWNAM_R */

  /* Fill this in so we can use it to get groups */
  username_c = info->username;

#ifdef HAVE_GETGROUPLIST
  {
    gid_t *buf;
    int buf_count;
    int i;
    int initial_buf_count;

    initial_buf_count = 17;
    buf_count = initial_buf_count;
    buf = dbus_new (gid_t, buf_count);
    if (buf == NULL)
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        goto failed;
      }

    if (getgrouplist (username_c,
                      info->primary_gid,
                      buf, &buf_count) < 0)
      {
        gid_t *new;
        /* Presumed cause of negative return code: buf has insufficient
           entries to hold the entire group list. The Linux behavior in this
           case is to pass back the actual number of groups in buf_count, but
           on Mac OS X 10.5, buf_count is unhelpfully left alone.
           So as a hack, try to help out a bit by guessing a larger
           number of groups, within reason.. might still fail, of course,
           but we can at least print a more informative message.  I looked up
           the "right way" to do this by downloading Apple's own source code
           for the "id" command, and it turns out that they use an
           undocumented library function getgrouplist_2 (!) which is not
           declared in any header in /usr/include (!!). That did not seem
           like the way to go here.
        */
        if (buf_count == initial_buf_count)
          {
            buf_count *= 16; /* Retry with an arbitrarily scaled-up array */
          }
        new = dbus_realloc (buf, buf_count * sizeof (buf[0]));
        if (new == NULL)
          {
            dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
            dbus_free (buf);
            goto failed;
          }

        buf = new;

        errno = 0;
        if (getgrouplist (username_c, info->primary_gid, buf, &buf_count) < 0)
          {
            if (errno == 0)
              {
                _dbus_warn ("It appears that username \"%s\" is in more than %d groups.\nProceeding with just the first %d groups.",
                            username_c, buf_count, buf_count);
              }
            else
              {
                dbus_set_error (error,
                                _dbus_error_from_errno (errno),
                                "Failed to get groups for username \"%s\" primary GID "
                                DBUS_GID_FORMAT ": %s\n",
                                username_c, info->primary_gid,
                                _dbus_strerror (errno));
                dbus_free (buf);
                goto failed;
              }
          }
      }

    info->group_ids = dbus_new (dbus_gid_t, buf_count);
    if (info->group_ids == NULL)
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        dbus_free (buf);
        goto failed;
      }

    for (i = 0; i < buf_count; ++i)
      info->group_ids[i] = buf[i];

    info->n_group_ids = buf_count;

    dbus_free (buf);
  }
#else  /* HAVE_GETGROUPLIST */
  {
    /* We just get the one group ID */
    info->group_ids = dbus_new (dbus_gid_t, 1);
    if (info->group_ids == NULL)
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        goto failed;
      }

    info->n_group_ids = 1;

    (info->group_ids)[0] = info->primary_gid;
  }
#endif /* HAVE_GETGROUPLIST */

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  return TRUE;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  return FALSE;
}

/**
 * Gets user info for the given username.
 *
 * @param info user info object to initialize
 * @param username the username
 * @param error error return
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_user_info_fill (DBusUserInfo     *info,
                      const DBusString *username,
                      DBusError        *error)
{
  return fill_user_info (info, DBUS_UID_UNSET,
                         username, error);
}

/**
 * Gets user info for the given user ID.
 *
 * @param info user info object to initialize
 * @param uid the user ID
 * @param error error return
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_user_info_fill_uid (DBusUserInfo *info,
                          dbus_uid_t    uid,
                          DBusError    *error)
{
  return fill_user_info (info, uid,
                         NULL, error);
}

/**
 * Adds the most important credentials of the current process
 * (the uid and pid) to the passed-in credentials object.
 *
 * The group vector is not included because it is rarely needed.
 * The Linux security label is not included because it is rarely
 * needed, it requires reading /proc, and the LSM API doesn't actually
 * guarantee that the string seen in /proc is comparable to the strings
 * found in SO_PEERSEC results.
 *
 * @param credentials credentials to add to
 * @returns #FALSE if no memory; does not properly roll back on failure, so only some credentials may have been added
 */
dbus_bool_t
_dbus_credentials_add_from_current_process (DBusCredentials *credentials)
{
  /* The POSIX spec certainly doesn't promise this, but
   * we need these assertions to fail as soon as we're wrong about
   * it so we can do the porting fixups
   */
  _DBUS_STATIC_ASSERT (sizeof (pid_t) <= sizeof (dbus_pid_t));
  _DBUS_STATIC_ASSERT (sizeof (uid_t) <= sizeof (dbus_uid_t));
  _DBUS_STATIC_ASSERT (sizeof (gid_t) <= sizeof (dbus_gid_t));

  if (!_dbus_credentials_add_pid(credentials, _dbus_getpid()))
    return FALSE;
  if (!_dbus_credentials_add_unix_uid(credentials, _dbus_geteuid()))
    return FALSE;

  return TRUE;
}

/**
 * Append to the string the identity we would like to have when we
 * authenticate, on UNIX this is the current process UID and on
 * Windows something else, probably a Windows SID string.  No escaping
 * is required, that is done in dbus-auth.c. The username here
 * need not be anything human-readable, it can be the machine-readable
 * form i.e. a user id.
 *
 * @param str the string to append to
 * @returns #FALSE on no memory
 */
dbus_bool_t
_dbus_append_user_from_current_process (DBusString *str)
{
  return _dbus_string_append_uint (str,
                                   _dbus_geteuid ());
}

/**
 * Gets our process ID
 * @returns process ID
 */
dbus_pid_t
_dbus_getpid (void)
{
  return getpid ();
}

/** Gets our UID
 * @returns process UID
 */
dbus_uid_t
_dbus_getuid (void)
{
  return getuid ();
}

/** Gets our effective UID
 * @returns process effective UID
 */
dbus_uid_t
_dbus_geteuid (void)
{
  return geteuid ();
}

/**
 * The only reason this is separate from _dbus_getpid() is to allow it
 * on Windows for logging but not for other purposes.
 *
 * @returns process ID to put in log messages
 */
unsigned long
_dbus_pid_for_log (void)
{
  return getpid ();
}

#if !DBUS_USE_SYNC
/* To be thread-safe by default on platforms that don't necessarily have
 * atomic operations (notably Debian armel, which is armv4t), we must
 * use a mutex that can be initialized statically, like this.
 * GLib >= 2.32 uses a similar system.
 */
static pthread_mutex_t atomic_mutex = PTHREAD_MUTEX_INITIALIZER;
#endif

/**
 * Atomically increments an integer
 *
 * @param atomic pointer to the integer to increment
 * @returns the value before incrementing
 */
dbus_int32_t
_dbus_atomic_inc (DBusAtomic *atomic)
{
#if DBUS_USE_SYNC
  return __sync_add_and_fetch(&atomic->value, 1)-1;
#else
  dbus_int32_t res;

  pthread_mutex_lock (&atomic_mutex);
  res = atomic->value;
  atomic->value += 1;
  pthread_mutex_unlock (&atomic_mutex);

  return res;
#endif
}

/**
 * Atomically decrement an integer
 *
 * @param atomic pointer to the integer to decrement
 * @returns the value before decrementing
 */
dbus_int32_t
_dbus_atomic_dec (DBusAtomic *atomic)
{
#if DBUS_USE_SYNC
  return __sync_sub_and_fetch(&atomic->value, 1)+1;
#else
  dbus_int32_t res;

  pthread_mutex_lock (&atomic_mutex);
  res = atomic->value;
  atomic->value -= 1;
  pthread_mutex_unlock (&atomic_mutex);

  return res;
#endif
}

/**
 * Atomically get the value of an integer. It may change at any time
 * thereafter, so this is mostly only useful for assertions.
 *
 * @param atomic pointer to the integer to get
 * @returns the value at this moment
 */
dbus_int32_t
_dbus_atomic_get (DBusAtomic *atomic)
{
#if DBUS_USE_SYNC
  __sync_synchronize ();
  return atomic->value;
#else
  dbus_int32_t res;

  pthread_mutex_lock (&atomic_mutex);
  res = atomic->value;
  pthread_mutex_unlock (&atomic_mutex);

  return res;
#endif
}

/**
 * Atomically set the value of an integer to 0.
 *
 * @param atomic pointer to the integer to set
 */
void
_dbus_atomic_set_zero (DBusAtomic *atomic)
{
#if DBUS_USE_SYNC
  /* Atomic version of "*atomic &= 0; return *atomic" */
  __sync_and_and_fetch (&atomic->value, 0);
#else
  pthread_mutex_lock (&atomic_mutex);
  atomic->value = 0;
  pthread_mutex_unlock (&atomic_mutex);
#endif
}

/**
 * Atomically set the value of an integer to something nonzero.
 *
 * @param atomic pointer to the integer to set
 */
void
_dbus_atomic_set_nonzero (DBusAtomic *atomic)
{
#if DBUS_USE_SYNC
  /* Atomic version of "*atomic |= 1; return *atomic" */
  __sync_or_and_fetch (&atomic->value, 1);
#else
  pthread_mutex_lock (&atomic_mutex);
  atomic->value = 1;
  pthread_mutex_unlock (&atomic_mutex);
#endif
}

/**
 * Wrapper for poll().
 *
 * @param fds the file descriptors to poll
 * @param n_fds number of descriptors in the array
 * @param timeout_milliseconds timeout or -1 for infinite
 * @returns numbers of fds with revents, or <0 on error
 */
int
_dbus_poll (DBusPollFD *fds,
            int         n_fds,
            int         timeout_milliseconds)
{
#if defined(HAVE_POLL) && !defined(BROKEN_POLL)
  /* DBusPollFD is a struct pollfd in this code path, so we can just poll() */
  if (timeout_milliseconds < -1)
    {
      timeout_milliseconds = -1;
    }

  return poll (fds,
               n_fds,
               timeout_milliseconds);
#else /* ! HAVE_POLL */
  /* Emulate poll() in terms of select() */
  fd_set read_set, write_set, err_set;
  int max_fd = 0;
  int i;
  struct timeval tv;
  int ready;

  FD_ZERO (&read_set);
  FD_ZERO (&write_set);
  FD_ZERO (&err_set);

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i];

      if (fdp->events & _DBUS_POLLIN)
	FD_SET (fdp->fd, &read_set);

      if (fdp->events & _DBUS_POLLOUT)
	FD_SET (fdp->fd, &write_set);

      FD_SET (fdp->fd, &err_set);

      max_fd = MAX (max_fd, fdp->fd);
    }

  tv.tv_sec = timeout_milliseconds / 1000;
  tv.tv_usec = (timeout_milliseconds % 1000) * 1000;

  ready = select (max_fd + 1, &read_set, &write_set, &err_set,
                  timeout_milliseconds < 0 ? NULL : &tv);

  if (ready > 0)
    {
      for (i = 0; i < n_fds; i++)
	{
	  DBusPollFD *fdp = &fds[i];

	  fdp->revents = 0;

	  if (FD_ISSET (fdp->fd, &read_set))
	    fdp->revents |= _DBUS_POLLIN;

	  if (FD_ISSET (fdp->fd, &write_set))
	    fdp->revents |= _DBUS_POLLOUT;

	  if (FD_ISSET (fdp->fd, &err_set))
	    fdp->revents |= _DBUS_POLLERR;
	}
    }

  return ready;
#endif
}

/**
 * Get current time, as in gettimeofday(). Use the monotonic clock if
 * available, to avoid problems when the system time changes.
 *
 * @param tv_sec return location for number of seconds
 * @param tv_usec return location for number of microseconds
 */
void
_dbus_get_monotonic_time (long *tv_sec,
                          long *tv_usec)
{
#ifdef HAVE_MONOTONIC_CLOCK
  struct timespec ts;
  clock_gettime (CLOCK_MONOTONIC, &ts);

  if (tv_sec)
    *tv_sec = ts.tv_sec;
  if (tv_usec)
    *tv_usec = ts.tv_nsec / 1000;
#else
  struct timeval t;

  gettimeofday (&t, NULL);

  if (tv_sec)
    *tv_sec = t.tv_sec;
  if (tv_usec)
    *tv_usec = t.tv_usec;
#endif
}

/**
 * Get current time, as in gettimeofday(). Never uses the monotonic
 * clock.
 *
 * @param tv_sec return location for number of seconds
 * @param tv_usec return location for number of microseconds
 */
void
_dbus_get_real_time (long *tv_sec,
                     long *tv_usec)
{
  struct timeval t;

  gettimeofday (&t, NULL);

  if (tv_sec)
    *tv_sec = t.tv_sec;
  if (tv_usec)
    *tv_usec = t.tv_usec;
}

/**
 * Creates a directory; succeeds if the directory
 * is created or already existed.
 *
 * @param filename directory filename
 * @param error initialized error object
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_ensure_directory (const DBusString *filename,
                        DBusError        *error)
{
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  if (mkdir (filename_c, 0700) < 0)
    {
      if (errno == EEXIST)
        return TRUE;

      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to create directory %s: %s\n",
                      filename_c, _dbus_strerror (errno));
      return FALSE;
    }
  else
    return TRUE;
}

/**
 * Creates a directory. Unlike _dbus_ensure_directory(), this only succeeds
 * if the directory is genuinely newly-created.
 *
 * @param filename directory filename
 * @param error initialized error object
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_create_directory (const DBusString *filename,
                        DBusError        *error)
{
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  if (mkdir (filename_c, 0700) < 0)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to create directory %s: %s\n",
                      filename_c, _dbus_strerror (errno));
      return FALSE;
    }
  else
    return TRUE;
}

/**
 * Appends the given filename to the given directory.
 *
 * @todo it might be cute to collapse multiple '/' such as "foo//"
 * concat "//bar"
 *
 * @param dir the directory name
 * @param next_component the filename
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_concat_dir_and_file (DBusString       *dir,
                           const DBusString *next_component)
{
  dbus_bool_t dir_ends_in_slash;
  dbus_bool_t file_starts_with_slash;

  if (_dbus_string_get_length (dir) == 0 ||
      _dbus_string_get_length (next_component) == 0)
    return TRUE;

  dir_ends_in_slash = '/' == _dbus_string_get_byte (dir,
                                                    _dbus_string_get_length (dir) - 1);

  file_starts_with_slash = '/' == _dbus_string_get_byte (next_component, 0);

  if (dir_ends_in_slash && file_starts_with_slash)
    {
      _dbus_string_shorten (dir, 1);
    }
  else if (!(dir_ends_in_slash || file_starts_with_slash))
    {
      if (!_dbus_string_append_byte (dir, '/'))
        return FALSE;
    }

  return _dbus_string_copy (next_component, 0, dir,
                            _dbus_string_get_length (dir));
}

/** nanoseconds in a second */
#define NANOSECONDS_PER_SECOND       1000000000
/** microseconds in a second */
#define MICROSECONDS_PER_SECOND      1000000
/** milliseconds in a second */
#define MILLISECONDS_PER_SECOND      1000
/** nanoseconds in a millisecond */
#define NANOSECONDS_PER_MILLISECOND  1000000
/** microseconds in a millisecond */
#define MICROSECONDS_PER_MILLISECOND 1000

/**
 * Sleeps the given number of milliseconds.
 * @param milliseconds number of milliseconds
 */
void
_dbus_sleep_milliseconds (int milliseconds)
{
#ifdef HAVE_NANOSLEEP
  struct timespec req;
  struct timespec rem;

  req.tv_sec = milliseconds / MILLISECONDS_PER_SECOND;
  req.tv_nsec = (milliseconds % MILLISECONDS_PER_SECOND) * NANOSECONDS_PER_MILLISECOND;
  rem.tv_sec = 0;
  rem.tv_nsec = 0;

  while (nanosleep (&req, &rem) < 0 && errno == EINTR)
    req = rem;
#elif defined (HAVE_USLEEP)
  usleep (milliseconds * MICROSECONDS_PER_MILLISECOND);
#else /* ! HAVE_USLEEP */
  sleep (MAX (milliseconds / 1000, 1));
#endif
}

/**
 * Generates the given number of securely random bytes,
 * using the best mechanism we can come up with.
 *
 * @param str the string
 * @param n_bytes the number of random bytes to append to string
 * @param error location to store reason for failure
 * @returns #TRUE on success, #FALSE on error
 */
dbus_bool_t
_dbus_generate_random_bytes (DBusString *str,
                             int         n_bytes,
                             DBusError  *error)
{
  int old_len = _dbus_string_get_length (str);
  int fd;
  int result;
#ifdef HAVE_GETRANDOM
  char *buffer;

  if (!_dbus_string_lengthen (str, n_bytes))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  buffer = _dbus_string_get_data_len (str, old_len, n_bytes);
  result = getrandom (buffer, n_bytes, GRND_NONBLOCK);

  if (result == n_bytes)
    return TRUE;

  _dbus_string_set_length (str, old_len);
#endif

  /* note, urandom on linux will fall back to pseudorandom */
  fd = open ("/dev/urandom", O_RDONLY);

  if (fd < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Could not open /dev/urandom: %s",
                      _dbus_strerror (errno));
      return FALSE;
    }

  _dbus_verbose ("/dev/urandom fd %d opened\n", fd);

  result = _dbus_read (fd, str, n_bytes);

  if (result != n_bytes)
    {
      if (result < 0)
        dbus_set_error (error, _dbus_error_from_errno (errno),
                        "Could not read /dev/urandom: %s",
                        _dbus_strerror (errno));
      else
        dbus_set_error (error, DBUS_ERROR_IO_ERROR,
                        "Short read from /dev/urandom");

      _dbus_close (fd, NULL);
      _dbus_string_set_length (str, old_len);
      return FALSE;
    }

  _dbus_verbose ("Read %d bytes from /dev/urandom\n",
                 n_bytes);

  _dbus_close (fd, NULL);

  return TRUE;
}

/**
 * Exit the process, returning the given value.
 *
 * @param code the exit code
 */
void
_dbus_exit (int code)
{
  _exit (code);
}

/**
 * A wrapper around strerror() because some platforms
 * may be lame and not have strerror(). Also, never
 * returns NULL.
 *
 * @param error_number errno.
 * @returns error description.
 */
const char*
_dbus_strerror (int error_number)
{
  const char *msg;

  msg = strerror (error_number);
  if (msg == NULL)
    msg = "unknown";

  return msg;
}

/**
 * signal (SIGPIPE, SIG_IGN);
 */
void
_dbus_disable_sigpipe (void)
{
  signal (SIGPIPE, SIG_IGN);
}

/**
 * Sets the file descriptor to be close
 * on exec. Should be called for all file
 * descriptors in D-Bus code.
 *
 * @param fd the file descriptor
 */
void
_dbus_fd_set_close_on_exec (int fd)
{
  int val;

  val = fcntl (fd, F_GETFD, 0);

  if (val < 0)
    return;

  val |= FD_CLOEXEC;

  fcntl (fd, F_SETFD, val);
}

/**
 * Sets the file descriptor to *not* be close-on-exec. This can be called
 * after _dbus_fd_set_all_close_on_exec() to make exceptions for pipes
 * used to communicate with child processes.
 *
 * @param fd the file descriptor
 */
void
_dbus_fd_clear_close_on_exec (int fd)
{
  int val;

  val = fcntl (fd, F_GETFD, 0);

  if (val < 0)
    return;

  val &= ~FD_CLOEXEC;

  fcntl (fd, F_SETFD, val);
}

/**
 * Closes a file descriptor.
 *
 * @param fd the file descriptor
 * @param error error object
 * @returns #FALSE if error set
 */
dbus_bool_t
_dbus_close (int        fd,
             DBusError *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

 again:
  if (close (fd) < 0)
    {
      if (errno == EINTR)
        goto again;

      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Could not close fd %d", fd);
      return FALSE;
    }

  return TRUE;
}

/**
 * Duplicates a file descriptor. Makes sure the fd returned is >= 3
 * (i.e. avoids stdin/stdout/stderr). Sets O_CLOEXEC.
 *
 * @param fd the file descriptor to duplicate
 * @param error address of error location.
 * @returns duplicated file descriptor
 * */
int
_dbus_dup(int        fd,
          DBusError *error)
{
  int new_fd;

#ifdef F_DUPFD_CLOEXEC
  dbus_bool_t cloexec_done;

  new_fd = fcntl(fd, F_DUPFD_CLOEXEC, 3);
  cloexec_done = new_fd >= 0;

  if (new_fd < 0 && errno == EINVAL)
#endif
    {
      new_fd = fcntl(fd, F_DUPFD, 3);
    }

  if (new_fd < 0) {

    dbus_set_error (error, _dbus_error_from_errno (errno),
                    "Could not duplicate fd %d", fd);
    return -1;
  }

#ifdef F_DUPFD_CLOEXEC
  if (!cloexec_done)
#endif
    {
      _dbus_fd_set_close_on_exec(new_fd);
    }

  return new_fd;
}

/**
 * Sets a file descriptor to be nonblocking.
 *
 * @param fd the file descriptor.
 * @param error address of error location.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_set_socket_nonblocking (DBusSocket      fd,
                              DBusError      *error)
{
  return _dbus_set_fd_nonblocking (fd.fd, error);
}

static dbus_bool_t
_dbus_set_fd_nonblocking (int             fd,
                          DBusError      *error)
{
  int val;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  val = fcntl (fd, F_GETFL, 0);
  if (val < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to get flags from file descriptor %d: %s",
                      fd, _dbus_strerror (errno));
      _dbus_verbose ("Failed to get flags for fd %d: %s\n", fd,
                     _dbus_strerror (errno));
      return FALSE;
    }

  if (fcntl (fd, F_SETFL, val | O_NONBLOCK) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to set nonblocking flag of file descriptor %d: %s",
                      fd, _dbus_strerror (errno));
      _dbus_verbose ("Failed to set fd %d nonblocking: %s\n",
                     fd, _dbus_strerror (errno));

      return FALSE;
    }

  return TRUE;
}

/**
 * On GNU libc systems, print a crude backtrace to stderr.  On other
 * systems, print "no backtrace support" and block for possible gdb
 * attachment if an appropriate environment variable is set.
 */
void
_dbus_print_backtrace (void)
{
#if defined (HAVE_BACKTRACE) && defined (DBUS_BUILT_R_DYNAMIC)
  void *bt[500];
  int bt_size;
  int i;
  char **syms;

  bt_size = backtrace (bt, 500);

  syms = backtrace_symbols (bt, bt_size);

  i = 0;
  while (i < bt_size)
    {
      /* don't use dbus_warn since it can _dbus_abort() */
      fprintf (stderr, "  %s\n", syms[i]);
      ++i;
    }
  fflush (stderr);

  free (syms);
#elif defined (HAVE_BACKTRACE) && ! defined (DBUS_BUILT_R_DYNAMIC)
  fprintf (stderr, "  D-Bus not built with -rdynamic so unable to print a backtrace\n");
#else
  fprintf (stderr, "  D-Bus not compiled with backtrace support so unable to print a backtrace\n");
#endif
}

/**
 * Creates pair of connect sockets (as in socketpair()).
 * Sets both ends of the pair nonblocking.
 *
 * Marks both file descriptors as close-on-exec
 *
 * @param fd1 return location for one end
 * @param fd2 return location for the other end
 * @param blocking #TRUE if pair should be blocking
 * @param error error return
 * @returns #FALSE on failure (if error is set)
 */
dbus_bool_t
_dbus_socketpair (DBusSocket *fd1,
                  DBusSocket *fd2,
                  dbus_bool_t blocking,
                  DBusError  *error)
{
#ifdef HAVE_SOCKETPAIR
  int fds[2];
  int retval;

#ifdef SOCK_CLOEXEC
  dbus_bool_t cloexec_done;

  retval = socketpair(AF_UNIX, SOCK_STREAM|SOCK_CLOEXEC, 0, fds);
  cloexec_done = retval >= 0;

  if (retval < 0 && (errno == EINVAL || errno == EPROTOTYPE))
#endif
    {
      retval = socketpair(AF_UNIX, SOCK_STREAM, 0, fds);
    }

  if (retval < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Could not create full-duplex pipe");
      return FALSE;
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

#ifdef SOCK_CLOEXEC
  if (!cloexec_done)
#endif
    {
      _dbus_fd_set_close_on_exec (fds[0]);
      _dbus_fd_set_close_on_exec (fds[1]);
    }

  if (!blocking &&
      (!_dbus_set_fd_nonblocking (fds[0], NULL) ||
       !_dbus_set_fd_nonblocking (fds[1], NULL)))
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Could not set full-duplex pipe nonblocking");

      _dbus_close (fds[0], NULL);
      _dbus_close (fds[1], NULL);

      return FALSE;
    }

  fd1->fd = fds[0];
  fd2->fd = fds[1];

  _dbus_verbose ("full-duplex pipe %d <-> %d\n",
                 fd1->fd, fd2->fd);

  return TRUE;
#else
  _dbus_warn ("_dbus_socketpair() not implemented on this OS");
  dbus_set_error (error, DBUS_ERROR_FAILED,
                  "_dbus_socketpair() not implemented on this OS");
  return FALSE;
#endif
}

/**
 * Measure the length of the given format string and arguments,
 * not including the terminating nul.
 *
 * @param format a printf-style format string
 * @param args arguments for the format string
 * @returns length of the given format string and args, or -1 if no memory
 */
int
_dbus_printf_string_upper_bound (const char *format,
                                 va_list     args)
{
  char static_buf[1024];
  int bufsize = sizeof (static_buf);
  int len;
  va_list args_copy;

  DBUS_VA_COPY (args_copy, args);
  len = vsnprintf (static_buf, bufsize, format, args_copy);
  va_end (args_copy);

  /* If vsnprintf() returned non-negative, then either the string fits in
   * static_buf, or this OS has the POSIX and C99 behaviour where vsnprintf
   * returns the number of characters that were needed, or this OS returns the
   * truncated length.
   *
   * We ignore the possibility that snprintf might just ignore the length and
   * overrun the buffer (64-bit Solaris 7), because that's pathological.
   * If your libc is really that bad, come back when you have a better one. */
  if (len == bufsize)
    {
      /* This could be the truncated length (Tru64 and IRIX have this bug),
       * or the real length could be coincidentally the same. Which is it?
       * If vsnprintf returns the truncated length, we'll go to the slow
       * path. */
      DBUS_VA_COPY (args_copy, args);

      if (vsnprintf (static_buf, 1, format, args_copy) == 1)
        len = -1;

      va_end (args_copy);
    }

  /* If vsnprintf() returned negative, we have to do more work.
   * HP-UX returns negative. */
  while (len < 0)
    {
      char *buf;

      bufsize *= 2;

      buf = dbus_malloc (bufsize);

      if (buf == NULL)
        return -1;

      DBUS_VA_COPY (args_copy, args);
      len = vsnprintf (buf, bufsize, format, args_copy);
      va_end (args_copy);

      dbus_free (buf);

      /* If the reported length is exactly the buffer size, round up to the
       * next size, in case vsnprintf has been returning the truncated
       * length */
      if (len == bufsize)
        len = -1;
    }

  return len;
}

/**
 * Gets the temporary files directory by inspecting the environment variables
 * TMPDIR, TMP, and TEMP in that order. If none of those are set "/tmp" is returned
 *
 * @returns location of temp directory, or #NULL if no memory for locking
 */
const char*
_dbus_get_tmpdir(void)
{
  /* Protected by _DBUS_LOCK_sysdeps */
  static const char* tmpdir = NULL;

  if (!_DBUS_LOCK (sysdeps))
    return NULL;

  if (tmpdir == NULL)
    {
      /* TMPDIR is what glibc uses, then
       * glibc falls back to the P_tmpdir macro which
       * just expands to "/tmp"
       */
      if (tmpdir == NULL)
        tmpdir = getenv("TMPDIR");

      /* These two env variables are probably
       * broken, but maybe some OS uses them?
       */
      if (tmpdir == NULL)
        tmpdir = getenv("TMP");
      if (tmpdir == NULL)
        tmpdir = getenv("TEMP");

      /* And this is the sane fallback. */
      if (tmpdir == NULL)
        tmpdir = "/tmp";
    }

  _DBUS_UNLOCK (sysdeps);

  _dbus_assert(tmpdir != NULL);

  return tmpdir;
}

#if defined(DBUS_ENABLE_X11_AUTOLAUNCH) || defined(DBUS_ENABLE_LAUNCHD)
/**
 * Execute a subprocess, returning up to 1024 bytes of output
 * into @p result.
 *
 * If successful, returns #TRUE and appends the output to @p
 * result. If a failure happens, returns #FALSE and
 * sets an error in @p error.
 *
 * @note It's not an error if the subprocess terminates normally
 * without writing any data to stdout. Verify the @p result length
 * before and after this function call to cover this case.
 *
 * @param progname initial path to exec (may or may not be absolute)
 * @param path_fallback if %TRUE, search PATH for executable
 * @param argv NULL-terminated list of arguments
 * @param result a DBusString where the output can be append
 * @param error a DBusError to store the error in case of failure
 * @returns #TRUE on success, #FALSE if an error happened
 */
static dbus_bool_t
_read_subprocess_line_argv (const char *progpath,
                            dbus_bool_t path_fallback,
                            const char * const *argv,
                            DBusString *result,
                            DBusError  *error)
{
  int result_pipe[2] = { -1, -1 };
  int errors_pipe[2] = { -1, -1 };
  pid_t pid;
  int ret;
  int status;
  int orig_len;

  dbus_bool_t retval;
  sigset_t new_set, old_set;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  retval = FALSE;

  /* We need to block any existing handlers for SIGCHLD temporarily; they
   * will cause waitpid() below to fail.
   * https://bugs.freedesktop.org/show_bug.cgi?id=21347
   */
  sigemptyset (&new_set);
  sigaddset (&new_set, SIGCHLD);
  sigprocmask (SIG_BLOCK, &new_set, &old_set);

  orig_len = _dbus_string_get_length (result);

#define READ_END        0
#define WRITE_END       1
  if (pipe (result_pipe) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to create a pipe to call %s: %s",
                      progpath, _dbus_strerror (errno));
      _dbus_verbose ("Failed to create a pipe to call %s: %s\n",
                     progpath, _dbus_strerror (errno));
      goto out;
    }
  if (pipe (errors_pipe) < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to create a pipe to call %s: %s",
                      progpath, _dbus_strerror (errno));
      _dbus_verbose ("Failed to create a pipe to call %s: %s\n",
                     progpath, _dbus_strerror (errno));
      goto out;
    }

  /* Make sure our output buffers aren't redundantly printed by both the
   * parent and the child */
  fflush (stdout);
  fflush (stderr);

  pid = fork ();
  if (pid < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to fork() to call %s: %s",
                      progpath, _dbus_strerror (errno));
      _dbus_verbose ("Failed to fork() to call %s: %s\n",
                     progpath, _dbus_strerror (errno));
      goto out;
    }

  if (pid == 0)
    {
      /* child process */
      const char *error_str;

      if (!_dbus_ensure_standard_fds (DBUS_FORCE_STDIN_NULL, &error_str))
        {
          int saved_errno = errno;

          /* Try to write details into the pipe, but don't bother
           * trying too hard (no retry loop). */

          if (write (errors_pipe[WRITE_END], error_str, strlen (error_str)) < 0 ||
              write (errors_pipe[WRITE_END], ": ", 2) < 0)
            {
              /* ignore, not much we can do */
            }

          error_str = _dbus_strerror (saved_errno);

          if (write (errors_pipe[WRITE_END], error_str, strlen (error_str)) < 0)
            {
              /* ignore, not much we can do */
            }

          _exit (1);
        }

      /* set-up stdXXX */
      close (result_pipe[READ_END]);
      close (errors_pipe[READ_END]);

      if (dup2 (result_pipe[WRITE_END], 1) == -1) /* setup stdout */
        _exit (1);
      if (dup2 (errors_pipe[WRITE_END], 2) == -1) /* setup stderr */
        _exit (1);

      _dbus_close_all ();

      sigprocmask (SIG_SETMASK, &old_set, NULL);

      /* If it looks fully-qualified, try execv first */
      if (progpath[0] == '/')
        {
          execv (progpath, (char * const *) argv);
          /* Ok, that failed.  Now if path_fallback is given, let's
           * try unqualified.  This is mostly a hack to work
           * around systems which ship dbus-launch in /usr/bin
           * but everything else in /bin (because dbus-launch
           * depends on X11).
           */
          if (path_fallback)
            /* We must have a slash, because we checked above */
            execvp (strrchr (progpath, '/')+1, (char * const *) argv);
        }
      else
        execvp (progpath, (char * const *) argv);

      /* still nothing, we failed */
      _exit (1);
    }

  /* parent process */
  close (result_pipe[WRITE_END]);
  close (errors_pipe[WRITE_END]);
  result_pipe[WRITE_END] = -1;
  errors_pipe[WRITE_END] = -1;

  ret = 0;
  do
    {
      ret = _dbus_read (result_pipe[READ_END], result, 1024);
    }
  while (ret > 0);

  /* reap the child process to avoid it lingering as zombie */
  do
    {
      ret = waitpid (pid, &status, 0);
    }
  while (ret == -1 && errno == EINTR);

  /* We succeeded if the process exited with status 0 and
     anything was read */
  if (!WIFEXITED (status) || WEXITSTATUS (status) != 0 )
    {
      /* The process ended with error */
      DBusString error_message;
      if (!_dbus_string_init (&error_message))
        {
          _DBUS_SET_OOM (error);
          goto out;
        }

      ret = 0;
      do
        {
          ret = _dbus_read (errors_pipe[READ_END], &error_message, 1024);
        }
      while (ret > 0);

      _dbus_string_set_length (result, orig_len);
      if (_dbus_string_get_length (&error_message) > 0)
        dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                        "%s terminated abnormally with the following error: %s",
                        progpath, _dbus_string_get_data (&error_message));
      else
        dbus_set_error (error, DBUS_ERROR_SPAWN_EXEC_FAILED,
                        "%s terminated abnormally without any error message",
                        progpath);
      goto out;
    }

  retval = TRUE;

 out:
  sigprocmask (SIG_SETMASK, &old_set, NULL);

  _DBUS_ASSERT_ERROR_XOR_BOOL (error, retval);

  if (result_pipe[0] != -1)
    close (result_pipe[0]);
  if (result_pipe[1] != -1)
    close (result_pipe[1]);
  if (errors_pipe[0] != -1)
    close (errors_pipe[0]);
  if (errors_pipe[1] != -1)
    close (errors_pipe[1]);

  return retval;
}
#endif

/**
 * Returns the address of a new session bus.
 *
 * If successful, returns #TRUE and appends the address to @p
 * address. If a failure happens, returns #FALSE and
 * sets an error in @p error.
 *
 * @param scope scope of autolaunch (Windows only)
 * @param address a DBusString where the address can be stored
 * @param error a DBusError to store the error in case of failure
 * @returns #TRUE on success, #FALSE if an error happened
 */
dbus_bool_t
_dbus_get_autolaunch_address (const char *scope,
                              DBusString *address,
                              DBusError  *error)
{
#ifdef DBUS_ENABLE_X11_AUTOLAUNCH
  static const char arg_dbus_launch[] = "dbus-launch";
  static const char arg_autolaunch[] = "--autolaunch";
  static const char arg_binary_syntax[] = "--binary-syntax";
  static const char arg_close_stderr[] = "--close-stderr";

  /* Perform X11-based autolaunch. (We also support launchd-based autolaunch,
   * but that's done elsewhere, and if it worked, this function wouldn't
   * be called.) */
  const char *display;
  const char *progpath;
  const char *argv[6];
  int i;
  DBusString uuid;
  dbus_bool_t retval;

  if (_dbus_check_setuid ())
    {
      dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
                            "Unable to autolaunch when setuid");
      return FALSE;
    }

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  retval = FALSE;

  /* fd.o #19997: if $DISPLAY isn't set to something useful, then
   * dbus-launch-x11 is just going to fail. Rather than trying to
   * run it, we might as well bail out early with a nice error.
   *
   * This is not strictly true in a world where the user bus exists,
   * because dbus-launch --autolaunch knows how to connect to that -
   * but if we were going to connect to the user bus, we'd have done
   * so before trying autolaunch: in any case. */
  display = _dbus_getenv ("DISPLAY");

  if (display == NULL || display[0] == '\0')
    {
      dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
          "Unable to autolaunch a dbus-daemon without a $DISPLAY for X11");
      return FALSE;
    }

  if (!_dbus_string_init (&uuid))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_get_local_machine_uuid_encoded (&uuid, error))
    {
      goto out;
    }

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  progpath = _dbus_getenv ("DBUS_TEST_DBUS_LAUNCH");

  if (progpath == NULL)
#endif
    progpath = DBUS_BINDIR "/dbus-launch";
  /*
   * argv[0] is always dbus-launch, that's the name what we'll
   * get from /proc, or ps(1), regardless what the progpath is,
   * see fd.o#69716
   */
  i = 0;
  argv[i] = arg_dbus_launch;
  ++i;
  argv[i] = arg_autolaunch;
  ++i;
  argv[i] = _dbus_string_get_data (&uuid);
  ++i;
  argv[i] = arg_binary_syntax;
  ++i;
  argv[i] = arg_close_stderr;
  ++i;
  argv[i] = NULL;
  ++i;

  _dbus_assert (i == _DBUS_N_ELEMENTS (argv));

  retval = _read_subprocess_line_argv (progpath,
                                       TRUE,
                                       argv, address, error);

 out:
  _dbus_string_free (&uuid);
  return retval;
#else
  dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
      "Using X11 for dbus-daemon autolaunch was disabled at compile time, "
      "set your DBUS_SESSION_BUS_ADDRESS instead");
  return FALSE;
#endif
}

/**
 * Reads the uuid of the machine we're running on from
 * the dbus configuration. Optionally try to create it
 * (only root can do this usually).
 *
 * On UNIX, reads a file that gets created by dbus-uuidgen
 * in a post-install script. On Windows, if there's a standard
 * machine uuid we could just use that, but I can't find one
 * with the right properties (the hardware profile guid can change
 * without rebooting I believe). If there's no standard one
 * we might want to use the registry instead of a file for
 * this, and I'm not sure how we'd ensure the uuid gets created.
 *
 * @param machine_id guid to init with the machine's uuid
 * @param create_if_not_found try to create the uuid if it doesn't exist
 * @param error the error return
 * @returns #FALSE if the error is set
 */
dbus_bool_t
_dbus_read_local_machine_uuid (DBusGUID   *machine_id,
                               dbus_bool_t create_if_not_found,
                               DBusError  *error)
{
  DBusError our_error = DBUS_ERROR_INIT;
  DBusError etc_error = DBUS_ERROR_INIT;
  DBusString filename;
  dbus_bool_t b;

  _dbus_string_init_const (&filename, DBUS_MACHINE_UUID_FILE);

  b = _dbus_read_uuid_file (&filename, machine_id, FALSE, &our_error);
  if (b)
    return TRUE;

  /* Fallback to the system machine ID */
  _dbus_string_init_const (&filename, "/etc/machine-id");
  b = _dbus_read_uuid_file (&filename, machine_id, FALSE, &etc_error);

  if (b)
    {
      if (create_if_not_found)
        {
          /* try to copy it to the DBUS_MACHINE_UUID_FILE, but do not
           * complain if that isn't possible for whatever reason */
          _dbus_string_init_const (&filename, DBUS_MACHINE_UUID_FILE);
          _dbus_write_uuid_file (&filename, machine_id, NULL);
        }

      dbus_error_free (&our_error);
      return TRUE;
    }

  if (!create_if_not_found)
    {
      dbus_set_error (error, etc_error.name,
                      "D-Bus library appears to be incorrectly set up: "
                      "see the manual page for dbus-uuidgen to correct "
                      "this issue. (%s; %s)",
                      our_error.message, etc_error.message);
      dbus_error_free (&our_error);
      dbus_error_free (&etc_error);
      return FALSE;
    }

  dbus_error_free (&our_error);
  dbus_error_free (&etc_error);

  /* if none found, try to make a new one */
  _dbus_string_init_const (&filename, DBUS_MACHINE_UUID_FILE);

  if (!_dbus_generate_uuid (machine_id, error))
    return FALSE;

  return _dbus_write_uuid_file (&filename, machine_id, error);
}

/**
 * quries launchd for a specific env var which holds the socket path.
 * @param socket_path append the socket path to this DBusString
 * @param launchd_env_var the env var to look up
 * @param error a DBusError to store the error in case of failure
 * @return the value of the env var
 */
dbus_bool_t
_dbus_lookup_launchd_socket (DBusString *socket_path,
                             const char *launchd_env_var,
                             DBusError  *error)
{
#ifdef DBUS_ENABLE_LAUNCHD
  char *argv[4];
  int i;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (_dbus_check_setuid ())
    {
      dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
                            "Unable to find launchd socket when setuid");
      return FALSE;
    }

  i = 0;
  argv[i] = "launchctl";
  ++i;
  argv[i] = "getenv";
  ++i;
  argv[i] = (char*)launchd_env_var;
  ++i;
  argv[i] = NULL;
  ++i;

  _dbus_assert (i == _DBUS_N_ELEMENTS (argv));

  if (!_read_subprocess_line_argv(argv[0], TRUE, argv, socket_path, error))
    {
      return FALSE;
    }

  /* no error, but no result either */
  if (_dbus_string_get_length(socket_path) == 0)
    {
      return FALSE;
    }

  /* strip the carriage-return */
  _dbus_string_shorten(socket_path, 1);
  return TRUE;
#else /* DBUS_ENABLE_LAUNCHD */
  dbus_set_error(error, DBUS_ERROR_NOT_SUPPORTED,
                "can't lookup socket from launchd; launchd support not compiled in");
  return FALSE;
#endif
}

#ifdef DBUS_ENABLE_LAUNCHD
static dbus_bool_t
_dbus_lookup_session_address_launchd (DBusString *address, DBusError  *error)
{
  dbus_bool_t valid_socket;
  DBusString socket_path;

  if (_dbus_check_setuid ())
    {
      dbus_set_error_const (error, DBUS_ERROR_NOT_SUPPORTED,
                            "Unable to find launchd socket when setuid");
      return FALSE;
    }

  if (!_dbus_string_init (&socket_path))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  valid_socket = _dbus_lookup_launchd_socket (&socket_path, "DBUS_LAUNCHD_SESSION_BUS_SOCKET", error);

  if (dbus_error_is_set(error))
    {
      _dbus_string_free(&socket_path);
      return FALSE;
    }

  if (!valid_socket)
    {
      dbus_set_error(error, "no socket path",
                "launchd did not provide a socket path, "
                "verify that org.freedesktop.dbus-session.plist is loaded!");
      _dbus_string_free(&socket_path);
      return FALSE;
    }
  if (!_dbus_string_append (address, "unix:path="))
    {
      _DBUS_SET_OOM (error);
      _dbus_string_free(&socket_path);
      return FALSE;
    }
  if (!_dbus_string_copy (&socket_path, 0, address,
                          _dbus_string_get_length (address)))
    {
      _DBUS_SET_OOM (error);
      _dbus_string_free(&socket_path);
      return FALSE;
    }

  _dbus_string_free(&socket_path);
  return TRUE;
}
#endif

dbus_bool_t
_dbus_lookup_user_bus (dbus_bool_t *supported,
                       DBusString  *address,
                       DBusError   *error)
{
  const char *runtime_dir = _dbus_getenv ("XDG_RUNTIME_DIR");
  dbus_bool_t ret = FALSE;
  struct stat stbuf;
  DBusString user_bus_path;

  if (runtime_dir == NULL)
    {
      _dbus_verbose ("XDG_RUNTIME_DIR not found in environment");
      *supported = FALSE;
      return TRUE;        /* Cannot use it, but not an error */
    }

  if (!_dbus_string_init (&user_bus_path))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_append_printf (&user_bus_path, "%s/bus", runtime_dir))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  if (lstat (_dbus_string_get_const_data (&user_bus_path), &stbuf) == -1)
    {
      _dbus_verbose ("XDG_RUNTIME_DIR/bus not available: %s",
                     _dbus_strerror (errno));
      *supported = FALSE;
      ret = TRUE;       /* Cannot use it, but not an error */
      goto out;
    }

  if (stbuf.st_uid != getuid ())
    {
      _dbus_verbose ("XDG_RUNTIME_DIR/bus owned by uid %ld, not our uid %ld",
                     (long) stbuf.st_uid, (long) getuid ());
      *supported = FALSE;
      ret = TRUE;       /* Cannot use it, but not an error */
      goto out;
    }

  if ((stbuf.st_mode & S_IFMT) != S_IFSOCK)
    {
      _dbus_verbose ("XDG_RUNTIME_DIR/bus is not a socket: st_mode = 0o%lo",
                     (long) stbuf.st_mode);
      *supported = FALSE;
      ret = TRUE;       /* Cannot use it, but not an error */
      goto out;
    }

  if (!_dbus_string_append (address, "unix:path=") ||
      !_dbus_address_append_escaped (address, &user_bus_path))
    {
      _DBUS_SET_OOM (error);
      goto out;
    }

  *supported = TRUE;
  ret = TRUE;

out:
  _dbus_string_free (&user_bus_path);
  return ret;
}

/**
 * Determines the address of the session bus by querying a
 * platform-specific method.
 *
 * The first parameter will be a boolean specifying whether
 * or not a dynamic session lookup is supported on this platform.
 *
 * If supported is TRUE and the return value is #TRUE, the
 * address will be  appended to @p address.
 * If a failure happens, returns #FALSE and sets an error in
 * @p error.
 *
 * If supported is FALSE, ignore the return value.
 *
 * @param supported returns whether this method is supported
 * @param address a DBusString where the address can be stored
 * @param error a DBusError to store the error in case of failure
 * @returns #TRUE on success, #FALSE if an error happened
 */
dbus_bool_t
_dbus_lookup_session_address (dbus_bool_t *supported,
                              DBusString  *address,
                              DBusError   *error)
{
#ifdef DBUS_ENABLE_LAUNCHD
  *supported = TRUE;
  return _dbus_lookup_session_address_launchd (address, error);
#else
  *supported = FALSE;

  if (!_dbus_lookup_user_bus (supported, address, error))
    return FALSE;
  else if (*supported)
    return TRUE;

  /* On non-Mac Unix platforms, if the session address isn't already
   * set in DBUS_SESSION_BUS_ADDRESS environment variable and the
   * $XDG_RUNTIME_DIR/bus can't be used, we punt and fall back to the
   * autolaunch: global default; see init_session_address in
   * dbus/dbus-bus.c. */
  return TRUE;
#endif
}

/**
 * Called when the bus daemon is signaled to reload its configuration; any
 * caches should be nuked. Of course any caches that need explicit reload
 * are probably broken, but c'est la vie.
 *
 *
 */
void
_dbus_flush_caches (void)
{
  _dbus_user_database_flush_system ();
}

/**
 * Appends the directory in which a keyring for the given credentials
 * should be stored.  The credentials should have either a Windows or
 * UNIX user in them.  The directory should be an absolute path.
 *
 * On UNIX the directory is ~/.dbus-keyrings while on Windows it should probably
 * be something else, since the dotfile convention is not normal on Windows.
 *
 * @param directory string to append directory to
 * @param credentials credentials the directory should be for
 *
 * @returns #FALSE on no memory
 */
dbus_bool_t
_dbus_append_keyring_directory_for_credentials (DBusString      *directory,
                                                DBusCredentials *credentials)
{
  DBusString homedir;
  DBusString dotdir;
  dbus_uid_t uid;

  _dbus_assert (credentials != NULL);
  _dbus_assert (!_dbus_credentials_are_anonymous (credentials));

  if (!_dbus_string_init (&homedir))
    return FALSE;

  uid = _dbus_credentials_get_unix_uid (credentials);
  _dbus_assert (uid != DBUS_UID_UNSET);

  if (!_dbus_homedir_from_uid (uid, &homedir))
    goto failed;

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  {
    const char *override;

    override = _dbus_getenv ("DBUS_TEST_HOMEDIR");
    if (override != NULL && *override != '\0')
      {
        _dbus_string_set_length (&homedir, 0);
        if (!_dbus_string_append (&homedir, override))
          goto failed;

        _dbus_verbose ("Using fake homedir for testing: %s\n",
                       _dbus_string_get_const_data (&homedir));
      }
    else
      {
        /* Not strictly thread-safe, but if we fail at thread-safety here,
         * the worst that will happen is some extra warnings. */
        static dbus_bool_t already_warned = FALSE;
        if (!already_warned)
          {
            _dbus_warn ("Using %s for testing, set DBUS_TEST_HOMEDIR to avoid",
                _dbus_string_get_const_data (&homedir));
            already_warned = TRUE;
          }
      }
  }
#endif

  _dbus_string_init_const (&dotdir, ".dbus-keyrings");
  if (!_dbus_concat_dir_and_file (&homedir,
                                  &dotdir))
    goto failed;

  if (!_dbus_string_copy (&homedir, 0,
                          directory, _dbus_string_get_length (directory))) {
    goto failed;
  }

  _dbus_string_free (&homedir);
  return TRUE;

 failed:
  _dbus_string_free (&homedir);
  return FALSE;
}

/* Documented in dbus-sysdeps-win.c, does nothing on Unix */
void
_dbus_daemon_unpublish_session_bus_address (void)
{
}

/**
 * See if errno is EAGAIN or EWOULDBLOCK (this has to be done differently
 * for Winsock so is abstracted)
 *
 * @returns #TRUE if e == EAGAIN or e == EWOULDBLOCK
 */
dbus_bool_t
_dbus_get_is_errno_eagain_or_ewouldblock (int e)
{
  /* Avoid the -Wlogical-op GCC warning, which can be triggered when EAGAIN and
   * EWOULDBLOCK are numerically equal, which is permitted as described by
   * errno(3).
   */
#if EAGAIN == EWOULDBLOCK
  return e == EAGAIN;
#else
  return e == EAGAIN || e == EWOULDBLOCK;
#endif
}

/**
 * Removes a directory; Directory must be empty
 *
 * @param filename directory filename
 * @param error initialized error object
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_delete_directory (const DBusString *filename,
                        DBusError        *error)
{
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  if (rmdir (filename_c) != 0)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to remove directory %s: %s\n",
                      filename_c, _dbus_strerror (errno));
      return FALSE;
    }

  return TRUE;
}

/**
 *  Checks whether file descriptors may be passed via the socket
 *
 *  @param fd the socket
 *  @return TRUE when fd passing over this socket is supported
 *
 */
dbus_bool_t
_dbus_socket_can_pass_unix_fd (DBusSocket fd)
{
#ifdef SCM_RIGHTS
  union {
    struct sockaddr sa;
    struct sockaddr_storage storage;
    struct sockaddr_un un;
  } sa_buf;

  socklen_t sa_len = sizeof(sa_buf);

  _DBUS_ZERO(sa_buf);

  if (getsockname(fd.fd, &sa_buf.sa, &sa_len) < 0)
    return FALSE;

  return sa_buf.sa.sa_family == AF_UNIX;

#else
  return FALSE;

#endif
}

static void
close_ignore_error (int fd)
{
  close (fd);
}

/*
 * Similar to Solaris fdwalk(3), but without the ability to stop iteration,
 * and may call func for integers that are not actually valid fds.
 */
static void
act_on_fds_3_and_up (void (*func) (int fd))
{
  int maxfds, i;

#if defined(__linux__) && defined(__GLIBC__)
  DIR *d;

  /* On Linux we can optimize this a bit if /proc is available. If it
     isn't available, fall back to the brute force way. */

  d = opendir ("/proc/self/fd");
  if (d)
    {
      for (;;)
        {
          struct dirent *de;
          int fd;
          long l;
          char *e = NULL;

          de = readdir (d);
          if (!de)
            break;

          if (de->d_name[0] == '.')
            continue;

          errno = 0;
          l = strtol (de->d_name, &e, 10);
          if (errno != 0 || e == NULL || *e != '\0')
            continue;

          fd = (int) l;
          if (fd < 3)
            continue;

          if (fd == dirfd (d))
            continue;

          func (fd);
        }

      closedir (d);
      return;
    }
#endif

  maxfds = sysconf (_SC_OPEN_MAX);

  /* Pick something reasonable if for some reason sysconf says
   * unlimited.
   */
  if (maxfds < 0)
    maxfds = 1024;

  /* close all inherited fds */
  for (i = 3; i < maxfds; i++)
    func (i);
}

/**
 * Closes all file descriptors except the first three (i.e. stdin,
 * stdout, stderr).
 */
void
_dbus_close_all (void)
{
  act_on_fds_3_and_up (close_ignore_error);
}

/**
 * Sets all file descriptors except the first three (i.e. stdin,
 * stdout, stderr) to be close-on-execute.
 */
void
_dbus_fd_set_all_close_on_exec (void)
{
  act_on_fds_3_and_up (_dbus_fd_set_close_on_exec);
}

/**
 * **NOTE**: If you modify this function, please also consider making
 * the corresponding change in GLib.  See
 * glib/gutils.c:g_check_setuid().
 *
 * Returns TRUE if the current process was executed as setuid (or an
 * equivalent __libc_enable_secure is available).  See:
 * http://osdir.com/ml/linux.lfs.hardened/2007-04/msg00032.html
 */
dbus_bool_t
_dbus_check_setuid (void)
{
  /* TODO: get __libc_enable_secure exported from glibc.
   * See http://www.openwall.com/lists/owl-dev/2012/08/14/1
   */
#if 0 && defined(HAVE_LIBC_ENABLE_SECURE)
  {
    /* See glibc/include/unistd.h */
    extern int __libc_enable_secure;
    return __libc_enable_secure;
  }
#elif defined(HAVE_ISSETUGID)
  /* BSD: http://www.freebsd.org/cgi/man.cgi?query=issetugid&sektion=2 */
  return issetugid ();
#else
  uid_t ruid, euid, suid; /* Real, effective and saved user ID's */
  gid_t rgid, egid, sgid; /* Real, effective and saved group ID's */

  /* We call into this function from _dbus_threads_init_platform_specific()
   * to make sure these are initialized before we start threading. */
  static dbus_bool_t check_setuid_initialised;
  static dbus_bool_t is_setuid;

  if (_DBUS_UNLIKELY (!check_setuid_initialised))
    {
#ifdef HAVE_GETRESUID
      if (getresuid (&ruid, &euid, &suid) != 0 ||
          getresgid (&rgid, &egid, &sgid) != 0)
#endif /* HAVE_GETRESUID */
        {
          suid = ruid = getuid ();
          sgid = rgid = getgid ();
          euid = geteuid ();
          egid = getegid ();
        }

      check_setuid_initialised = TRUE;
      is_setuid = (ruid != euid || ruid != suid ||
                   rgid != egid || rgid != sgid);

    }
  return is_setuid;
#endif
}

/**
 * Read the address from the socket and append it to the string
 *
 * @param fd the socket
 * @param address
 * @param error return location for error code
 */
dbus_bool_t
_dbus_append_address_from_socket (DBusSocket  fd,
                                  DBusString *address,
                                  DBusError  *error)
{
  union {
      struct sockaddr sa;
      struct sockaddr_storage storage;
      struct sockaddr_un un;
      struct sockaddr_in ipv4;
      struct sockaddr_in6 ipv6;
  } socket;
  char hostip[INET6_ADDRSTRLEN];
  socklen_t size = sizeof (socket);
  DBusString path_str;
  const char *family_name = NULL;
  dbus_uint16_t port;

  if (getsockname (fd.fd, &socket.sa, &size))
    goto err;

  switch (socket.sa.sa_family)
    {
    case AF_UNIX:
      if (socket.un.sun_path[0]=='\0')
        {
          _dbus_string_init_const (&path_str, &(socket.un.sun_path[1]));
          if (_dbus_string_append (address, "unix:abstract=") &&
              _dbus_address_append_escaped (address, &path_str))
            {
              return TRUE;
            }
          else
            {
              _DBUS_SET_OOM (error);
              return FALSE;
            }
        }
      else
        {
          _dbus_string_init_const (&path_str, socket.un.sun_path);
          if (_dbus_string_append (address, "unix:path=") &&
              _dbus_address_append_escaped (address, &path_str))
            {
              return TRUE;
            }
          else
            {
              _DBUS_SET_OOM (error);
              return FALSE;
            }
        }
      /* not reached */
      break;

    case AF_INET:
#ifdef AF_INET6
    case AF_INET6:
#endif
       _dbus_string_init_const (&path_str, hostip);

      if (_dbus_inet_sockaddr_to_string (&socket, size, hostip, sizeof (hostip),
                                         &family_name, &port, error))
        {
          if (_dbus_string_append_printf (address, "tcp:family=%s,port=%u,host=",
                                          family_name, port) &&
              _dbus_address_append_escaped (address, &path_str))
            {
              return TRUE;
            }
          else
            {
              _DBUS_SET_OOM (error);
              return FALSE;
            }
        }
      else
        {
          return FALSE;
        }
      /* not reached */
      break;

    default:
      dbus_set_error (error,
                      _dbus_error_from_errno (EINVAL),
                      "Failed to read address from socket: Unknown socket type.");
      return FALSE;
    }
 err:
  dbus_set_error (error,
                  _dbus_error_from_errno (errno),
                  "Failed to read address from socket: %s",
                  _dbus_strerror (errno));
  return FALSE;
}

int
_dbus_save_socket_errno (void)
{
  return errno;
}

void
_dbus_restore_socket_errno (int saved_errno)
{
  errno = saved_errno;
}

static const char *syslog_tag = "dbus";
#ifdef HAVE_SYSLOG_H
static DBusLogFlags log_flags = DBUS_LOG_FLAGS_STDERR;
#endif

/**
 * Initialize the system log.
 *
 * The "tag" is not copied, and must remain valid for the entire lifetime of
 * the process or until _dbus_init_system_log() is called again. In practice
 * it will normally be a constant.
 *
 * On platforms that do not support a system log, the
 * #DBUS_LOG_FLAGS_SYSTEM_LOG flag is treated as equivalent to
 * #DBUS_LOG_FLAGS_STDERR.
 *
 * @param tag the name of the executable (syslog tag)
 * @param mode whether to log to stderr, the system log or both
 */
void
_dbus_init_system_log (const char   *tag,
                       DBusLogFlags  flags)
{
  /* We never want to turn off logging completely */
  _dbus_assert (
      (flags & (DBUS_LOG_FLAGS_STDERR | DBUS_LOG_FLAGS_SYSTEM_LOG)) != 0);

  syslog_tag = tag;

#ifdef HAVE_SYSLOG_H
  log_flags = flags;

  if (log_flags & DBUS_LOG_FLAGS_SYSTEM_LOG)
    openlog (tag, LOG_PID, LOG_DAEMON);
#endif
}

/**
 * Log a message to the system log file (e.g. syslog on Unix) and/or stderr.
 *
 * @param severity a severity value
 * @param msg a printf-style format string
 * @param args arguments for the format string
 */
void
_dbus_logv (DBusSystemLogSeverity  severity,
            const char            *msg,
            va_list                args)
{
  va_list tmp;
#ifdef HAVE_SYSLOG_H
  if (log_flags & DBUS_LOG_FLAGS_SYSTEM_LOG)
    {
      int flags;
      switch (severity)
        {
          case DBUS_SYSTEM_LOG_INFO:
            flags =  LOG_DAEMON | LOG_INFO;
            break;
          case DBUS_SYSTEM_LOG_WARNING:
            flags =  LOG_DAEMON | LOG_WARNING;
            break;
          case DBUS_SYSTEM_LOG_SECURITY:
            flags = LOG_AUTH | LOG_NOTICE;
            break;
          case DBUS_SYSTEM_LOG_ERROR:
            flags = LOG_DAEMON|LOG_CRIT;
            break;
          default:
            _dbus_assert_not_reached ("invalid log severity");
        }

      DBUS_VA_COPY (tmp, args);
      vsyslog (flags, msg, tmp);
      va_end (tmp);
    }

  /* If we don't have syslog.h, we always behave as though stderr was in
   * the flags */
  if (log_flags & DBUS_LOG_FLAGS_STDERR)
#endif
    {
      DBUS_VA_COPY (tmp, args);
      fprintf (stderr, "%s[" DBUS_PID_FORMAT "]: ", syslog_tag, _dbus_getpid ());
      vfprintf (stderr, msg, tmp);
      fputc ('\n', stderr);
      va_end (tmp);
    }
}

/*
 * Return the low-level representation of a socket error, as used by
 * cross-platform socket APIs like inet_ntop(), send() and recv(). This
 * is the standard errno on Unix, but is WSAGetLastError() on Windows.
 *
 * Some libdbus internal functions copy this into errno, but with
 * hindsight that was probably a design flaw.
 */
int
_dbus_get_low_level_socket_errno (void)
{
  return errno;
}

/* tests in dbus-sysdeps-util.c */
