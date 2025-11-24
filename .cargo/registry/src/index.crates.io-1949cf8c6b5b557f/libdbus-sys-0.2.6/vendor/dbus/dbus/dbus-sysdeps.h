/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps.h Wrappers around system/libc features (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
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

#ifndef DBUS_SYSDEPS_H
#define DBUS_SYSDEPS_H

#ifndef VERSION
#warning Please include config.h before dbus-sysdeps.h
#include "config.h"
#endif

#ifdef HAVE_STDINT_H
#include <stdint.h>
#endif

#ifdef HAVE_INTTYPES_H
#include <inttypes.h>
#endif

#include <dbus/dbus-errors.h>
#include <dbus/dbus-file.h>
#include <dbus/dbus-string.h>

/* this is perhaps bogus, but strcmp() etc. are faster if we use the
 * stuff straight out of string.h, so have this here for now.
 */
#include <string.h>
#include <stdarg.h>

#if !defined(BROKEN_POLL) && (defined(__APPLE__) || defined(__INTERIX))
  /* Following libcurl's example, we blacklist poll() on Darwin
   * (macOS, iOS, etc.) and Interix due to a history of implementation
   * issues.
   * https://github.com/curl/curl/blob/master/m4/curl-functions.m4
   *
   * On unspecified older macOS versions, poll() failed if given a
   * device node to poll.
   *
   * On macOS < 10.9, poll() with nfds=0 failed instead of waiting for
   * the timeout and then succeeding.
   *
   * On macOS >= 10.12, poll() with nfds=0 succeeded immediately
   * instead of waiting for the timeout, resulting in busy-looping.
   *
   * On Interix, poll() apparently only works for files in /proc.
   *
   * The "legacy" build flavour in our CI machinery defines BROKEN_POLL
   * on whatever platform is in use (normally Linux) to force use of the
   * same select()-based poll() emulation that we use for macOS, Interix,
   * and any platform that lacks a real poll(), so that we can test it
   * more regularly.
   */
# define BROKEN_POLL
#endif

/* Normally we'd only include this in dbus-sysdeps-unix.c.
 * However, the member names in DBusPollFD are (deliberately) the same as
 * in POSIX struct pollfd, and AIX's poll() implementation is known to
 * do things like "#define events reqevents", which would break that approach.
 * Defend against that by ensuring that if it's renamed anywhere, it's renamed
 * everywhere.
 */
#ifdef HAVE_POLL
#include <poll.h>
#endif

#ifdef DBUS_WINCE
/* Windows CE lacks some system functions (such as errno and clock).
   We bring them in here.  */
#include "dbus-sysdeps-wince-glue.h"
#endif

#ifdef DBUS_WIN
#include <ws2tcpip.h>
#endif

DBUS_BEGIN_DECLS

#ifdef DBUS_WIN
#define _DBUS_PATH_SEPARATOR ";"
#else
#define _DBUS_PATH_SEPARATOR ":"
#endif

/* Forward declarations */


/** An opaque list type */
typedef struct DBusList DBusList;

/** Object that contains a list of credentials such as UNIX or Windows user ID */
typedef struct DBusCredentials DBusCredentials;

/** A wrapper around a pipe descriptor or handle */
typedef struct DBusPipe DBusPipe;

/**
 * @addtogroup DBusSysdeps
 *
 * @{
 */

DBUS_PRIVATE_EXPORT
void _dbus_abort (void) _DBUS_GNUC_NORETURN;

dbus_bool_t _dbus_check_setuid (void);
DBUS_PRIVATE_EXPORT
const char* _dbus_getenv (const char *varname);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_clearenv (void);
char **     _dbus_get_environment (void);

/** A process ID */
typedef unsigned long dbus_pid_t;
/** A user ID */
typedef unsigned long dbus_uid_t;
/** A group ID */
typedef unsigned long dbus_gid_t;

/** an invalid PID used to represent an uninitialized dbus_pid_t field */
#define DBUS_PID_UNSET ((dbus_pid_t) -1)
/** an invalid UID used to represent an uninitialized dbus_uid_t field */
#define DBUS_UID_UNSET ((dbus_uid_t) -1)
/** an invalid GID used to represent an uninitialized dbus_gid_t field */
#define DBUS_GID_UNSET ((dbus_gid_t) -1)

/** an appropriate printf format for dbus_pid_t */
#define DBUS_PID_FORMAT "%lu"
/** an appropriate printf format for dbus_uid_t */
#define DBUS_UID_FORMAT "%lu"
/** an appropriate printf format for dbus_gid_t */
#define DBUS_GID_FORMAT "%lu"

/**
 * Socket interface
 */
#ifdef DBUS_WIN

typedef struct { SOCKET sock; } DBusSocket;
# define DBUS_SOCKET_FORMAT "Iu"
# define DBUS_SOCKET_INIT { INVALID_SOCKET }

_DBUS_WARN_UNUSED_RESULT
static inline SOCKET
_dbus_socket_printable (DBusSocket s) { return s.sock; }

_DBUS_WARN_UNUSED_RESULT
static inline dbus_bool_t
_dbus_socket_is_valid (DBusSocket s) { return s.sock != INVALID_SOCKET; }

static inline void
_dbus_socket_invalidate (DBusSocket *s) { s->sock = INVALID_SOCKET; }

_DBUS_WARN_UNUSED_RESULT
static inline int
_dbus_socket_get_int (DBusSocket s) { return (int)s.sock; }

#else /* not DBUS_WIN */

typedef struct { int fd; } DBusSocket;
# define DBUS_SOCKET_FORMAT "d"
# define DBUS_SOCKET_INIT { -1 }

_DBUS_WARN_UNUSED_RESULT
static inline int
_dbus_socket_printable (DBusSocket s) { return s.fd; }

_DBUS_WARN_UNUSED_RESULT
static inline dbus_bool_t
_dbus_socket_is_valid (DBusSocket s) { return s.fd >= 0; }

static inline void
_dbus_socket_invalidate (DBusSocket *s) { s->fd = -1; }

_DBUS_WARN_UNUSED_RESULT
static inline int
_dbus_socket_get_int (DBusSocket s) { return s.fd; }

#endif /* not DBUS_WIN */

_DBUS_WARN_UNUSED_RESULT
static inline DBusSocket
_dbus_socket_get_invalid (void)
{
  DBusSocket s = DBUS_SOCKET_INIT;

  return s;
}

dbus_bool_t _dbus_set_socket_nonblocking (DBusSocket      fd,
                                          DBusError      *error);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_close_socket     (DBusSocket        fd,
                                    DBusError        *error);
DBUS_PRIVATE_EXPORT
int         _dbus_read_socket      (DBusSocket        fd,
                                    DBusString       *buffer,
                                    int               count);
DBUS_PRIVATE_EXPORT
int         _dbus_write_socket     (DBusSocket        fd,
                                    const DBusString *buffer,
                                    int               start,
                                    int               len);
int         _dbus_write_socket_two (DBusSocket        fd,
                                    const DBusString *buffer1,
                                    int               start1,
                                    int               len1,
                                    const DBusString *buffer2,
                                    int               start2,
                                    int               len2);

int _dbus_read_socket_with_unix_fds      (DBusSocket        fd,
                                          DBusString       *buffer,
                                          int               count,
                                          int              *fds,
                                          unsigned int     *n_fds);
DBUS_PRIVATE_EXPORT
int _dbus_write_socket_with_unix_fds     (DBusSocket        fd,
                                          const DBusString *buffer,
                                          int               start,
                                          int               len,
                                          const int        *fds,
                                          int               n_fds);
int _dbus_write_socket_with_unix_fds_two (DBusSocket        fd,
                                          const DBusString *buffer1,
                                          int               start1,
                                          int               len1,
                                          const DBusString *buffer2,
                                          int               start2,
                                          int               len2,
                                          const int        *fds,
                                          int               n_fds);

DBusSocket _dbus_connect_tcp_socket  (const char     *host,
                                      const char     *port,
                                      const char     *family,
                                      DBusError      *error);
DBusSocket _dbus_connect_tcp_socket_with_nonce  (const char     *host,
                                                 const char     *port,
                                                 const char     *family,
                                                 const char     *noncefile,
                                                 DBusError      *error);
int _dbus_listen_tcp_socket   (const char     *host,
                               const char     *port,
                               const char     *family,
                               DBusString     *retport,
                               const char    **retfamily,
                               DBusSocket    **fds_p,
                               DBusError      *error);
DBusSocket _dbus_accept       (DBusSocket      listen_fd);

dbus_bool_t _dbus_read_credentials_socket (DBusSocket        client_fd,
                                           DBusCredentials  *credentials,
                                           DBusError        *error);
dbus_bool_t _dbus_send_credentials_socket (DBusSocket       server_fd,
                                           DBusError       *error);

typedef enum
{
  DBUS_CREDENTIALS_ADD_FLAGS_USER_DATABASE = (1 << 0),
  DBUS_CREDENTIALS_ADD_FLAGS_NONE = 0
} DBusCredentialsAddFlags;

dbus_bool_t _dbus_credentials_add_from_user            (DBusCredentials         *credentials,
                                                        const DBusString        *username,
                                                        DBusCredentialsAddFlags  flags,
                                                        DBusError               *error);

dbus_bool_t _dbus_credentials_add_from_current_process (DBusCredentials  *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_append_user_from_current_process     (DBusString        *str);

dbus_bool_t _dbus_parse_unix_user_from_config   (const DBusString  *username,
                                                 dbus_uid_t        *uid_p);
dbus_bool_t _dbus_parse_unix_group_from_config  (const DBusString  *groupname,
                                                 dbus_gid_t        *gid_p);
dbus_bool_t _dbus_unix_groups_from_uid          (dbus_uid_t         uid,
                                                 dbus_gid_t       **group_ids,
                                                 int               *n_group_ids);
dbus_bool_t _dbus_unix_user_is_at_console       (dbus_uid_t         uid,
                                                 DBusError         *error);
dbus_bool_t _dbus_unix_user_is_process_owner    (dbus_uid_t         uid);
dbus_bool_t _dbus_windows_user_is_process_owner (const char        *windows_sid);

dbus_bool_t _dbus_append_keyring_directory_for_credentials (DBusString      *directory,
                                                            DBusCredentials *credentials);

void _dbus_daemon_unpublish_session_bus_address (void);

dbus_bool_t _dbus_socket_can_pass_unix_fd(DBusSocket fd);

/** Opaque type representing an atomically-modifiable integer
 * that can be used from multiple threads.
 */
typedef struct DBusAtomic DBusAtomic;

/**
 * An atomic integer safe to increment or decrement from multiple threads.
 */
struct DBusAtomic
{
#ifdef DBUS_WIN
  volatile long value; /**< Value of the atomic integer. */
#else
  volatile dbus_int32_t value; /**< Value of the atomic integer. */
#endif
};

DBUS_PRIVATE_EXPORT
dbus_int32_t _dbus_atomic_inc (DBusAtomic *atomic);
DBUS_PRIVATE_EXPORT
dbus_int32_t _dbus_atomic_dec (DBusAtomic *atomic);
DBUS_PRIVATE_EXPORT
dbus_int32_t _dbus_atomic_get (DBusAtomic *atomic);
DBUS_PRIVATE_EXPORT
void         _dbus_atomic_set_zero    (DBusAtomic *atomic);
DBUS_PRIVATE_EXPORT
void         _dbus_atomic_set_nonzero (DBusAtomic *atomic);

#ifdef DBUS_WIN

/* On Windows, you can only poll sockets. We emulate Unix poll() using
 * select(), so it doesn't matter what precise type we put in DBusPollFD;
 * use DBusSocket so that the compiler can check we are doing it right.
 */
typedef DBusSocket DBusPollable;
# define DBUS_POLLABLE_FORMAT "Iu"

static inline DBusPollable
_dbus_socket_get_pollable (DBusSocket s) { return s; }

static inline SOCKET
_dbus_pollable_printable (DBusPollable p) { return p.sock; }

static inline dbus_bool_t
_dbus_pollable_is_valid (DBusPollable p) { return _dbus_socket_is_valid (p); }

static inline void
_dbus_pollable_invalidate (DBusPollable *p) { _dbus_socket_invalidate (p); }

static inline dbus_bool_t
_dbus_pollable_equals (DBusPollable a, DBusPollable b) { return a.sock == b.sock; }

#else /* !DBUS_WIN */

/* On Unix, you can poll sockets, pipes, etc., and we must put exactly
 * "int" in DBusPollFD because we're relying on its layout exactly matching
 * struct pollfd. (This is silly, and one day we should use a better
 * abstraction.)
 */
typedef int DBusPollable;
# define DBUS_POLLABLE_FORMAT "d"

static inline DBusPollable
_dbus_socket_get_pollable (DBusSocket s) { return s.fd; }

static inline int
_dbus_pollable_printable (DBusPollable p) { return p; }

static inline dbus_bool_t
_dbus_pollable_is_valid (DBusPollable p) { return p >= 0; }

static inline void
_dbus_pollable_invalidate (DBusPollable *p) { *p = -1; }

static inline dbus_bool_t
_dbus_pollable_equals (DBusPollable a, DBusPollable b) { return a == b; }

#endif /* !DBUS_WIN */

#if defined(HAVE_POLL) && !defined(BROKEN_POLL)
/**
 * A portable struct pollfd wrapper, or an emulation of struct pollfd
 * on platforms where poll() is missing or broken.
 */
typedef struct pollfd DBusPollFD;

/** There is data to read */
#define _DBUS_POLLIN      POLLIN
/** There is urgent data to read */
#define _DBUS_POLLPRI     POLLPRI
/** Writing now will not block */
#define _DBUS_POLLOUT     POLLOUT
/** Error condition */
#define _DBUS_POLLERR     POLLERR
/** Hung up */
#define _DBUS_POLLHUP     POLLHUP
/** Invalid request: fd not open */
#define _DBUS_POLLNVAL    POLLNVAL
#else
/* Emulate poll() via select(). Because we aren't really going to call
 * poll(), any similarly-shaped struct is acceptable, and any power of 2
 * will do for the events/revents; these values happen to match Linux
 * and *BSD. */
typedef struct
{
  DBusPollable fd;   /**< File descriptor */
  short events;      /**< Events to poll for */
  short revents;     /**< Events that occurred */
} DBusPollFD;

/** There is data to read */
#define _DBUS_POLLIN      0x0001
/** There is urgent data to read */
#define _DBUS_POLLPRI     0x0002
/** Writing now will not block */
#define _DBUS_POLLOUT     0x0004
/** Error condition */
#define _DBUS_POLLERR     0x0008
/** Hung up */
#define _DBUS_POLLHUP     0x0010
/** Invalid request: fd not open */
#define _DBUS_POLLNVAL    0x0020
#endif

DBUS_PRIVATE_EXPORT
int _dbus_poll (DBusPollFD *fds,
                int         n_fds,
                int         timeout_milliseconds);

DBUS_PRIVATE_EXPORT
void _dbus_sleep_milliseconds (int milliseconds);

DBUS_PRIVATE_EXPORT
void _dbus_get_monotonic_time (long *tv_sec,
                               long *tv_usec);

DBUS_PRIVATE_EXPORT
void _dbus_get_real_time (long *tv_sec,
                          long *tv_usec);

/**
 * directory interface
 */
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_create_directory        (const DBusString *filename,
                                              DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_ensure_directory        (const DBusString *filename,
                                              DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_delete_directory        (const DBusString *filename,
					      DBusError        *error);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_concat_dir_and_file (DBusString       *dir,
                                       const DBusString *next_component);
dbus_bool_t _dbus_string_get_dirname  (const DBusString *filename,
                                       DBusString       *dirname);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_path_is_absolute    (const DBusString *filename);

dbus_bool_t _dbus_get_standard_session_servicedirs (DBusList **dirs);
dbus_bool_t _dbus_get_standard_system_servicedirs (DBusList **dirs);
dbus_bool_t _dbus_set_up_transient_session_servicedirs (DBusList  **dirs,
                                                        DBusError  *error);

dbus_bool_t _dbus_get_system_config_file  (DBusString *str);
dbus_bool_t _dbus_get_session_config_file (DBusString *str);

/** Opaque type for reading a directory listing */
typedef struct DBusDirIter DBusDirIter;

DBusDirIter* _dbus_directory_open          (const DBusString *filename,
                                            DBusError        *error);
dbus_bool_t  _dbus_directory_get_next_file (DBusDirIter      *iter,
                                            DBusString       *filename,
                                            DBusError        *error);
void         _dbus_directory_close         (DBusDirIter      *iter);

dbus_bool_t  _dbus_check_dir_is_private_to_user    (DBusString *dir,
                                                    DBusError *error);

DBUS_PRIVATE_EXPORT
const char* _dbus_get_tmpdir      (void);

/**
 * Random numbers 
 */
_DBUS_WARN_UNUSED_RESULT
dbus_bool_t _dbus_generate_random_bytes_buffer (char       *buffer,
                                                int         n_bytes,
                                                DBusError  *error);
dbus_bool_t _dbus_generate_random_bytes        (DBusString *str,
                                                int         n_bytes,
                                                DBusError  *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_generate_random_ascii        (DBusString *str,
                                                int         n_bytes,
                                                DBusError  *error);

DBUS_PRIVATE_EXPORT
const char* _dbus_error_from_errno (int error_number);
DBUS_PRIVATE_EXPORT
const char* _dbus_error_from_system_errno (void);

int         _dbus_get_low_level_socket_errno         (void);

int         _dbus_save_socket_errno                  (void);
void        _dbus_restore_socket_errno               (int saved_errno);
void        _dbus_set_errno_to_zero                  (void);
dbus_bool_t _dbus_get_is_errno_eagain_or_ewouldblock (int e);
dbus_bool_t _dbus_get_is_errno_enomem                (int e);
dbus_bool_t _dbus_get_is_errno_eintr                 (int e);
dbus_bool_t _dbus_get_is_errno_epipe                 (int e);
dbus_bool_t _dbus_get_is_errno_etoomanyrefs          (int e);
DBUS_PRIVATE_EXPORT
const char* _dbus_strerror_from_errno                (void);

void _dbus_disable_sigpipe (void);

DBUS_PRIVATE_EXPORT
void _dbus_exit (int code) _DBUS_GNUC_NORETURN;

DBUS_PRIVATE_EXPORT
int _dbus_printf_string_upper_bound (const char *format,
                                     va_list args) _DBUS_GNUC_PRINTF (1, 0);

#ifdef DBUS_ENABLE_VERBOSE_MODE
DBUS_PRIVATE_EXPORT
void _dbus_print_thread (void);
#endif

/**
 * Portable struct with stat() results
 */
typedef struct
{
  unsigned long mode;  /**< File mode */
  unsigned long nlink; /**< Number of hard links */
  dbus_uid_t    uid;   /**< User owning file */
  dbus_gid_t    gid;   /**< Group owning file */
  unsigned long size;  /**< Size of file */
  unsigned long atime; /**< Access time */
  unsigned long mtime; /**< Modify time */
  unsigned long ctime; /**< Creation time */
} DBusStat;

dbus_bool_t _dbus_stat             (const DBusString *filename,
                                    DBusStat         *statbuf,
                                    DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_socketpair (DBusSocket       *fd1,
                              DBusSocket       *fd2,
                              dbus_bool_t       blocking,
                              DBusError        *error);

DBUS_PRIVATE_EXPORT
void        _dbus_print_backtrace  (void);

dbus_bool_t _dbus_become_daemon   (const DBusString *pidfile,
                                   DBusPipe         *print_pid_pipe,
                                   DBusError        *error,
                                   dbus_bool_t       keep_umask);

dbus_bool_t _dbus_verify_daemon_user    (const char *user);
dbus_bool_t _dbus_change_to_daemon_user (const char *user,
                                         DBusError  *error);

dbus_bool_t _dbus_write_pid_to_file_and_pipe (const DBusString *pidfile,
                                              DBusPipe         *print_pid_pipe,
                                              dbus_pid_t        pid_to_write,
                                              DBusError        *error);

dbus_bool_t _dbus_command_for_pid (unsigned long  pid,
                                   DBusString    *str,
                                   int            max_len,
                                   DBusError     *error);

dbus_bool_t _dbus_user_at_console (const char *username,
                                   DBusError  *error);

typedef enum {
  DBUS_LOG_FLAGS_STDERR = (1 << 0),
  DBUS_LOG_FLAGS_SYSTEM_LOG = (1 << 1)
} DBusLogFlags;

DBUS_PRIVATE_EXPORT
void _dbus_init_system_log (const char   *tag,
                            DBusLogFlags  flags);

typedef enum {
  DBUS_SYSTEM_LOG_INFO,
  DBUS_SYSTEM_LOG_WARNING,
  DBUS_SYSTEM_LOG_SECURITY,
  DBUS_SYSTEM_LOG_ERROR
} DBusSystemLogSeverity;

DBUS_PRIVATE_EXPORT
void _dbus_log  (DBusSystemLogSeverity  severity,
                 const char            *msg,
                 ...) _DBUS_GNUC_PRINTF (2, 3);
DBUS_PRIVATE_EXPORT
void _dbus_logv (DBusSystemLogSeverity  severity,
                 const char            *msg,
                 va_list args) _DBUS_GNUC_PRINTF (2, 0);

/**
 * Casts a primitive C type to a byte array and then indexes
 * a particular byte of the array.
 */
#define _DBUS_BYTE_OF_PRIMITIVE(p, i) \
    (((const char*)&(p))[(i)])
/** On x86 there is an 80-bit FPU, and if you do "a == b" it may have a
 * or b in an 80-bit register, thus failing to compare the two 64-bit
 * doubles for bitwise equality. So this macro compares the two doubles
 * bitwise.
 */
#define _DBUS_DOUBLES_BITWISE_EQUAL(a, b)                                       \
     (_DBUS_BYTE_OF_PRIMITIVE (a, 0) == _DBUS_BYTE_OF_PRIMITIVE (b, 0) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 1) == _DBUS_BYTE_OF_PRIMITIVE (b, 1) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 2) == _DBUS_BYTE_OF_PRIMITIVE (b, 2) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 3) == _DBUS_BYTE_OF_PRIMITIVE (b, 3) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 4) == _DBUS_BYTE_OF_PRIMITIVE (b, 4) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 5) == _DBUS_BYTE_OF_PRIMITIVE (b, 5) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 6) == _DBUS_BYTE_OF_PRIMITIVE (b, 6) &&       \
      _DBUS_BYTE_OF_PRIMITIVE (a, 7) == _DBUS_BYTE_OF_PRIMITIVE (b, 7))

dbus_bool_t _dbus_get_autolaunch_address (const char *scope,
                                          DBusString *address,
					                      DBusError  *error);

dbus_bool_t _dbus_lookup_session_address (dbus_bool_t *supported,
                                          DBusString  *address,
                                          DBusError   *error);

/** Type representing a universally unique ID
 * @todo rename to UUID instead of GUID
 */
typedef union DBusGUID DBusGUID;

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_read_local_machine_uuid   (DBusGUID         *machine_id,
                                             dbus_bool_t       create_if_not_found,
                                             DBusError        *error);

/**
 * Initialize threads as in dbus_threads_init_default(), appropriately
 * for the platform.
 * @returns #FALSE if no memory
 */
dbus_bool_t _dbus_threads_init_platform_specific (void);

/**
 * Lock a static mutex used to protect _dbus_threads_init_platform_specific().
 */
void _dbus_threads_lock_platform_specific (void);

/**
 * Undo _dbus_threads_lock_platform_specific().
 */
void _dbus_threads_unlock_platform_specific (void);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_split_paths_and_append (DBusString *dirs, 
                                          const char *suffix, 
                                          DBusList **dir_list);

unsigned long _dbus_pid_for_log (void);

/* FIXME move back to dbus-sysdeps-unix.h probably -
 * the PID file handling just needs a little more abstraction
 * in the bus daemon first.
 */
DBUS_PRIVATE_EXPORT
dbus_pid_t    _dbus_getpid (void);

DBUS_PRIVATE_EXPORT
dbus_uid_t    _dbus_getuid (void);

DBUS_PRIVATE_EXPORT
void _dbus_flush_caches (void);

dbus_bool_t _dbus_replace_install_prefix (DBusString *path);

/* Do not set this too high: it is a denial-of-service risk.
 * See <https://bugs.freedesktop.org/show_bug.cgi?id=82820>
 *
 * (This needs to be in the non-Unix-specific header so that
 * the config-parser can use it.)
 */
#define DBUS_DEFAULT_MESSAGE_UNIX_FDS 16

typedef struct DBusRLimit DBusRLimit;

DBusRLimit     *_dbus_rlimit_save_fd_limit                 (DBusError    *error);
dbus_bool_t     _dbus_rlimit_raise_fd_limit                (DBusError    *error);
dbus_bool_t     _dbus_rlimit_restore_fd_limit              (DBusRLimit   *saved,
                                                            DBusError    *error);
void            _dbus_rlimit_free                          (DBusRLimit   *lim);

void            _dbus_daemon_report_ready                  (void);
void            _dbus_daemon_report_reloading              (void);
void            _dbus_daemon_report_reloaded               (void);
void            _dbus_daemon_report_stopping               (void);

dbus_bool_t _dbus_inet_sockaddr_to_string (const void *sockaddr_pointer,
                                           size_t len,
                                           char *string,
                                           size_t string_len,
                                           const char **family_name,
                                           dbus_uint16_t *port,
                                           DBusError *error);
void _dbus_set_error_with_inet_sockaddr (DBusError *error,
                                         const void *sockaddr_pointer,
                                         size_t len,
                                         const char *description,
                                         int saved_errno);
void _dbus_combine_tcp_errors (DBusList **sources,
                               const char *summary,
                               const char *host,
                               const char *port,
                               DBusError *dest);

/** @} */

DBUS_END_DECLS


#ifdef DBUS_WIN
#include "dbus-sysdeps-win.h"
#endif

#endif /* DBUS_SYSDEPS_H */
