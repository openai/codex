/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps.c Wrappers around system/libc features (internal to D-BUS implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
 * Copyright (C) 2005 Novell, Inc.
 * Copyright (C) 2006 Peter Kümmel  <syntheticpp@gmx.net>
 * Copyright (C) 2006 Christian Ehrlicher <ch.ehrlicher@gmx.de>
 * Copyright (C) 2006-2021 Ralf Habacker <ralf.habacker@freenet.de>
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

#define STRSAFE_NO_DEPRECATE

#include "dbus-internals.h"
#include "dbus-sha.h"
#include "dbus-sysdeps.h"
#include "dbus-threads.h"
#include "dbus-protocol.h"
#include "dbus-string.h"
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-win.h"
#include "dbus-protocol.h"
#include "dbus-hash.h"
#include "dbus-sockets-win.h"
#include "dbus-list.h"
#include "dbus-nonce.h"
#include "dbus-credentials.h"

#include <windows.h>
#include <wincrypt.h>
#include <iphlpapi.h>

/* Declarations missing in mingw's and windows sdk 7.0 headers */
extern BOOL WINAPI ConvertStringSidToSidA (LPCSTR  StringSid, PSID *Sid);
extern BOOL WINAPI ConvertSidToStringSidA (PSID Sid, LPSTR *StringSid);

#include <stdio.h>
#include <stdlib.h>

#include <string.h>
#if HAVE_ERRNO_H
#include <errno.h>
#endif
#ifndef DBUS_WINCE
#include <mbstring.h>
#include <sys/stat.h>
#include <sys/types.h>
#endif

#ifdef HAVE_WS2TCPIP_H
/* getaddrinfo for Windows CE (and Windows).  */
#include <ws2tcpip.h>
#endif

#ifndef O_BINARY
#define O_BINARY 0
#endif

#ifndef PROCESS_QUERY_LIMITED_INFORMATION
/* MinGW32 < 4 does not define this value in its headers */
#define PROCESS_QUERY_LIMITED_INFORMATION (0x1000)
#endif

typedef int socklen_t;

/* uncomment to enable windows event based poll implementation */
//#define USE_CHRIS_IMPL

void
_dbus_win_set_errno (int err)
{
#ifdef DBUS_WINCE
  SetLastError (err);
#else
  errno = err;
#endif
}

static BOOL is_winxp_sp3_or_lower (void);

/*
 * _MIB_TCPROW_EX and friends are not available in system headers
 *  and are mapped to attribute identical ...OWNER_PID typedefs.
 */
typedef MIB_TCPROW_OWNER_PID _MIB_TCPROW_EX;
typedef MIB_TCPTABLE_OWNER_PID MIB_TCPTABLE_EX;
typedef PMIB_TCPTABLE_OWNER_PID PMIB_TCPTABLE_EX;
typedef DWORD (WINAPI *ProcAllocateAndGetTcpExtTableFromStack)(PMIB_TCPTABLE_EX*,BOOL,HANDLE,DWORD,DWORD);

/* Not protected by a lock, but if we miss a write, all that
 * happens is that the lazy initialization will happen in two threads
 * concurrently - it results in the same value either way so that's OK */
static ProcAllocateAndGetTcpExtTableFromStack lpfnAllocateAndGetTcpExTableFromStack = NULL;

/**
 * AllocateAndGetTcpExTableFromStack() is undocumented and not exported,
 * but is the only way to do this in older XP versions.
 * @return true if the procedures could be loaded
 */
static BOOL
load_ex_ip_helper_procedures(void)
{
  HMODULE hModule = LoadLibrary ("iphlpapi.dll");
  if (hModule == NULL)
    {
      _dbus_verbose ("could not load iphlpapi.dll\n");
      return FALSE;
    }

  lpfnAllocateAndGetTcpExTableFromStack = (ProcAllocateAndGetTcpExtTableFromStack) (void (*)(void))GetProcAddress (hModule, "AllocateAndGetTcpExTableFromStack");
  if (lpfnAllocateAndGetTcpExTableFromStack == NULL)
    {
      _dbus_verbose ("could not find function AllocateAndGetTcpExTableFromStack in iphlpapi.dll\n");
      return FALSE;
    }
  return TRUE;
}

/**
 * get pid from localhost tcp connection using peer_port
 * This function is available on WinXP >= SP3
 * @param peer_port peers tcp port
 * @return process id or 0 if connection has not been found
 */
static dbus_pid_t
get_pid_from_extended_tcp_table(int peer_port)
{
  dbus_pid_t result;
  DWORD errorCode, size = 0, i;
  MIB_TCPTABLE_OWNER_PID *tcp_table;

  if ((errorCode =
       GetExtendedTcpTable (NULL, &size, TRUE, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0)) == ERROR_INSUFFICIENT_BUFFER)
    {
      tcp_table = (MIB_TCPTABLE_OWNER_PID *) dbus_malloc (size);
      if (tcp_table == NULL)
        {
          _dbus_verbose ("Error allocating memory\n");
          return 0;
        }
    }
  else
    {
      _dbus_win_warn_win_error ("unexpected error returned from GetExtendedTcpTable", errorCode);
      return 0;
    }

  if ((errorCode = GetExtendedTcpTable (tcp_table, &size, TRUE, AF_INET, TCP_TABLE_OWNER_PID_ALL, 0)) != NO_ERROR)
    {
      _dbus_verbose ("Error fetching tcp table %d\n", (int)errorCode);
      dbus_free (tcp_table);
      return 0;
    }

  result = 0;
  for (i = 0; i < tcp_table->dwNumEntries; i++)
    {
      MIB_TCPROW_OWNER_PID *p = &tcp_table->table[i];
      int local_address = ntohl (p->dwLocalAddr);
      int local_port = ntohs (p->dwLocalPort);
      if (p->dwState == MIB_TCP_STATE_ESTAB
          && local_address == INADDR_LOOPBACK && local_port == peer_port)
         result = p->dwOwningPid;
    }

  dbus_free (tcp_table);
  _dbus_verbose ("got pid %lu\n", result);
  return result;
}

/**
 * get pid from localhost tcp connection using peer_port
 * This function is available on all WinXP versions, but
 * not in wine (at least version <= 1.6.0)
 * @param peer_port peers tcp port
 * @return process id or 0 if connection has not been found
 */
static dbus_pid_t
get_pid_from_tcp_ex_table(int peer_port)
{
  dbus_pid_t result;
  DWORD errorCode, i;
  PMIB_TCPTABLE_EX tcp_table = NULL;

  if (!load_ex_ip_helper_procedures ())
    {
      _dbus_verbose
        ("Error not been able to load iphelper procedures\n");
      return 0;
    }

  errorCode = lpfnAllocateAndGetTcpExTableFromStack (&tcp_table, TRUE, GetProcessHeap(), 0, 2);

  if (errorCode != NO_ERROR)
    {
      _dbus_verbose
        ("Error not been able to call AllocateAndGetTcpExTableFromStack()\n");
      return 0;
    }

  result = 0;
  for (i = 0; i < tcp_table->dwNumEntries; i++)
    {
      _MIB_TCPROW_EX *p = &tcp_table->table[i];
      int local_port = ntohs (p->dwLocalPort);
      int local_address = ntohl (p->dwLocalAddr);
      if (local_address == INADDR_LOOPBACK && local_port == peer_port)
        {
          result = p->dwOwningPid;
          break;
        }
    }

  HeapFree (GetProcessHeap(), 0, tcp_table);
  _dbus_verbose ("got pid %lu\n", result);
  return result;
}

/**
 * @brief return peer process id from tcp handle for localhost connections
 * @param handle tcp socket descriptor
 * @return process id or 0 in case the process id could not be fetched
 */
static dbus_pid_t
_dbus_get_peer_pid_from_tcp_handle (int handle)
{
  struct sockaddr_storage addr;
  socklen_t len = sizeof (addr);
  int peer_port;

  dbus_pid_t result;
  dbus_bool_t is_localhost = FALSE;

  getpeername (handle, (struct sockaddr *) &addr, &len);

  if (addr.ss_family == AF_INET)
    {
      struct sockaddr_in *s = (struct sockaddr_in *) &addr;
      peer_port = ntohs (s->sin_port);
      is_localhost = (ntohl (s->sin_addr.s_addr) == INADDR_LOOPBACK);
    }
  else if (addr.ss_family == AF_INET6)
    {
      _dbus_verbose ("FIXME [61922]: IPV6 support not working on windows\n");
      return 0;
      /*
         struct sockaddr_in6 *s = (struct sockaddr_in6 * )&addr;
         peer_port = ntohs (s->sin6_port);
         is_localhost = (memcmp(s->sin6_addr.s6_addr, in6addr_loopback.s6_addr, 16) == 0);
         _dbus_verbose ("IPV6 %08x %08x\n", s->sin6_addr.s6_addr, in6addr_loopback.s6_addr);
       */
    }
  else
    {
      _dbus_verbose ("no idea what address family %d is\n", addr.ss_family);
      return 0;
    }

  if (!is_localhost)
    {
      _dbus_verbose ("could not fetch process id from remote process\n");
      return 0;
    }

  if (peer_port == 0)
    {
      _dbus_verbose
        ("Error not been able to fetch tcp peer port from connection\n");
      return 0;
    }

  _dbus_verbose ("trying to get peer's pid\n");

  result = get_pid_from_extended_tcp_table (peer_port);
  if (result > 0)
      return result;
  result = get_pid_from_tcp_ex_table (peer_port);
  return result;
}

/* Convert GetLastError() to a dbus error.  */
const char*
_dbus_win_error_from_last_error (void)
{
  switch (GetLastError())
    {
    case 0:
      return DBUS_ERROR_FAILED;
    
    case ERROR_NO_MORE_FILES:
    case ERROR_TOO_MANY_OPEN_FILES:
      return DBUS_ERROR_LIMITS_EXCEEDED; /* kernel out of memory */

    case ERROR_ACCESS_DENIED:
    case ERROR_CANNOT_MAKE:
      return DBUS_ERROR_ACCESS_DENIED;

    case ERROR_NOT_ENOUGH_MEMORY:
      return DBUS_ERROR_NO_MEMORY;

    case ERROR_FILE_EXISTS:
      return DBUS_ERROR_FILE_EXISTS;

    case ERROR_FILE_NOT_FOUND:
    case ERROR_PATH_NOT_FOUND:
      return DBUS_ERROR_FILE_NOT_FOUND;

    default:
      return DBUS_ERROR_FAILED;
    }
}


char*
_dbus_win_error_string (int error_number)
{
  char *msg;

  FormatMessageA (FORMAT_MESSAGE_ALLOCATE_BUFFER |
                  FORMAT_MESSAGE_IGNORE_INSERTS |
                  FORMAT_MESSAGE_FROM_SYSTEM,
                  NULL, error_number, 0,
                  (LPSTR) &msg, 0, NULL);

  if (msg[strlen (msg) - 1] == '\n')
    msg[strlen (msg) - 1] = '\0';
  if (msg[strlen (msg) - 1] == '\r')
    msg[strlen (msg) - 1] = '\0';

  return msg;
}

void
_dbus_win_free_error_string (char *string)
{
  LocalFree (string);
}

/**
 * Socket interface
 *
 */

/**
 * Thin wrapper around the read() system call that appends
 * the data it reads to the DBusString buffer. It appends
 * up to the given count, and returns the same value
 * and same errno as read(). The only exception is that
 * _dbus_read_socket() handles EINTR for you. 
 * _dbus_read_socket() can return ENOMEM, even though 
 * regular UNIX read doesn't.
 *
 * @param fd the file descriptor to read from
 * @param buffer the buffer to append data to
 * @param count the amount of data to read
 * @returns the number of bytes read or -1
 */

int
_dbus_read_socket (DBusSocket        fd,
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
      _dbus_win_set_errno (ENOMEM);
      return -1;
    }

  data = _dbus_string_get_data_len (buffer, start, count);

 again:
 
  _dbus_verbose ("recv: count=%d fd=%Iu\n", count, fd.sock);
  bytes_read = recv (fd.sock, data, count, 0);
  
  if (bytes_read == SOCKET_ERROR)
	{
	  DBUS_SOCKET_SET_ERRNO();
	  _dbus_verbose ("recv: failed: %s (%d)\n", _dbus_strerror (errno), errno);
	  bytes_read = -1;
	}
	else
	  _dbus_verbose ("recv: = %d\n", bytes_read);

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
_dbus_write_socket (DBusSocket        fd,
                    const DBusString *buffer,
                    int               start,
                    int               len)
{
  const char *data;
  int bytes_written;

  data = _dbus_string_get_const_data_len (buffer, start, len);

 again:

  _dbus_verbose ("send: len=%d fd=%Iu\n", len, fd.sock);
  bytes_written = send (fd.sock, data, len, 0);

  if (bytes_written == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO();
      _dbus_verbose ("send: failed: %s\n", _dbus_strerror_from_errno ());
      bytes_written = -1;
    }
    else
      _dbus_verbose ("send: = %d\n", bytes_written);

  if (bytes_written < 0 && errno == EINTR)
    goto again;
    
#if 0
  if (bytes_written > 0)
    _dbus_verbose_bytes_of_string (buffer, start, bytes_written);
#endif

  return bytes_written;
}


/**
 * Closes a file descriptor.
 *
 * @param fd the file descriptor
 * @param error error object
 * @returns #FALSE if error set
 */
dbus_bool_t
_dbus_close_socket (DBusSocket fd,
                    DBusError *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

 again:
  if (closesocket (fd.sock) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      
      if (errno == EINTR)
        goto again;
        
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Could not close socket: socket=%Iu, , %s",
                      fd.sock, _dbus_strerror_from_errno ());
      return FALSE;
    }
  _dbus_verbose ("socket=%Iu, \n", fd.sock);

  return TRUE;
}

/**
 * Sets the file descriptor to be close
 * on exec. Should be called for all file
 * descriptors in D-Bus code.
 *
 * @param handle the Windows HANDLE (a SOCKET is also OK)
 */
static void
_dbus_win_handle_set_close_on_exec (HANDLE handle)
{
  if ( !SetHandleInformation( (HANDLE) handle,
                        HANDLE_FLAG_INHERIT | HANDLE_FLAG_PROTECT_FROM_CLOSE,
                        0 /*disable both flags*/ ) )
    {
      _dbus_win_warn_win_error ("Disabling socket handle inheritance failed:", GetLastError());
    }
}

/**
 * Sets a file descriptor to be nonblocking.
 *
 * @param handle the file descriptor.
 * @param error address of error location.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_set_socket_nonblocking (DBusSocket      handle,
                              DBusError      *error)
{
  u_long one = 1;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (ioctlsocket (handle.sock, FIONBIO, &one) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to set socket %Iu to nonblocking: %s",
                      handle.sock, _dbus_strerror_from_errno ());
      return FALSE;
    }

  return TRUE;
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
_dbus_write_socket_two (DBusSocket        fd,
                        const DBusString *buffer1,
                        int               start1,
                        int               len1,
                        const DBusString *buffer2,
                        int               start2,
                        int               len2)
{
  WSABUF vectors[2];
  const char *data1;
  const char *data2;
  int rc;
  DWORD bytes_written;

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

  vectors[0].buf = (char*) data1;
  vectors[0].len = len1;
  vectors[1].buf = (char*) data2;
  vectors[1].len = len2;

 again:
 
  _dbus_verbose ("WSASend: len1+2=%d+%d fd=%Iu\n", len1, len2, fd.sock);
  rc = WSASend (fd.sock,
                vectors,
                data2 ? 2 : 1, 
                &bytes_written,
                0, 
                NULL, 
                NULL);
                
  if (rc == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      _dbus_verbose ("WSASend: failed: %s\n", _dbus_strerror_from_errno ());
      bytes_written = (DWORD) -1;
    }
  else
    _dbus_verbose ("WSASend: = %ld\n", bytes_written);
    
  if (bytes_written == (DWORD) -1 && errno == EINTR)
    goto again;
      
  return bytes_written;
}

#if 0

/**
 * Opens the client side of a Windows named pipe. The connection D-BUS
 * file descriptor index is returned. It is set up as nonblocking.
 * 
 * @param path the path to named pipe socket
 * @param error return location for error code
 * @returns connection D-BUS file descriptor or -1 on error
 */
int
_dbus_connect_named_pipe (const char     *path,
                          DBusError      *error)
{
  _dbus_assert_not_reached ("not implemented");
}

#endif

/**
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_win_startup_winsock (void)
{
  /* Straight from MSDN, deuglified */

  /* Protected by _DBUS_LOCK_sysdeps */
  static dbus_bool_t beenhere = FALSE;

  WORD wVersionRequested;
  WSADATA wsaData;
  int err;

  if (!_DBUS_LOCK (sysdeps))
    return FALSE;

  if (beenhere)
    goto out;

  wVersionRequested = MAKEWORD (2, 0);

  err = WSAStartup (wVersionRequested, &wsaData);
  if (err != 0)
    {
      _dbus_assert_not_reached ("Could not initialize WinSock");
      _dbus_abort ();
    }

  /* Confirm that the WinSock DLL supports 2.0.  Note that if the DLL
   * supports versions greater than 2.0 in addition to 2.0, it will
   * still return 2.0 in wVersion since that is the version we
   * requested.
   */
  if (LOBYTE (wsaData.wVersion) != 2 ||
      HIBYTE (wsaData.wVersion) != 0)
    {
      _dbus_assert_not_reached ("No usable WinSock found");
      _dbus_abort ();
    }

  beenhere = TRUE;

out:
  _DBUS_UNLOCK (sysdeps);
  return TRUE;
}









/************************************************************************
 
 UTF / string code
 
 ************************************************************************/

/**
 * Measure the message length without terminating nul 
 */
int _dbus_printf_string_upper_bound (const char *format,
                                     va_list args)
{
  /* MSVCRT's vsnprintf semantics are a bit different */
  char buf[1024];
  int bufsize;
  int len;
  va_list args_copy;

  bufsize = sizeof (buf);
  DBUS_VA_COPY (args_copy, args);
  len = _vsnprintf (buf, bufsize - 1, format, args_copy);
  va_end (args_copy);

  while (len == -1) /* try again */
    {
      char *p;

      bufsize *= 2;

      p = malloc (bufsize);

      if (p == NULL)
        return -1;

      DBUS_VA_COPY (args_copy, args);
      len = _vsnprintf (p, bufsize - 1, format, args_copy);
      va_end (args_copy);
      free (p);
    }

  return len;
}


/**
 * Returns the UTF-16 form of a UTF-8 string. The result should be
 * freed with dbus_free() when no longer needed.
 *
 * @param str the UTF-8 string
 * @param error return location for error code
 */
wchar_t *
_dbus_win_utf8_to_utf16 (const char *str,
                         DBusError  *error)
{
  DBusString s;
  int n;
  wchar_t *retval;

  _dbus_string_init_const (&s, str);

  if (!_dbus_string_validate_utf8 (&s, 0, _dbus_string_get_length (&s)))
    {
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "Invalid UTF-8");
      return NULL;
    }

  n = MultiByteToWideChar (CP_UTF8, 0, str, -1, NULL, 0);

  if (n == 0)
    {
      _dbus_win_set_error_from_win_error (error, GetLastError ());
      return NULL;
    }

  retval = dbus_new (wchar_t, n);

  if (!retval)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  if (MultiByteToWideChar (CP_UTF8, 0, str, -1, retval, n) != n)
    {
      dbus_free (retval);
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "MultiByteToWideChar inconsistency");
      return NULL;
    }

  return retval;
}

/**
 * Returns the UTF-8 form of a UTF-16 string. The result should be
 * freed with dbus_free() when no longer needed.
 *
 * @param str the UTF-16 string
 * @param error return location for error code
 */
char *
_dbus_win_utf16_to_utf8 (const wchar_t *str,
                         DBusError     *error)
{
  int n;
  char *retval;

  n = WideCharToMultiByte (CP_UTF8, 0, str, -1, NULL, 0, NULL, NULL);

  if (n == 0)
    {
      _dbus_win_set_error_from_win_error (error, GetLastError ());
      return NULL;
    }

  retval = dbus_malloc (n);

  if (!retval)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  if (WideCharToMultiByte (CP_UTF8, 0, str, -1, retval, n, NULL, NULL) != n)
    {
      dbus_free (retval);
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "WideCharToMultiByte inconsistency");
      return NULL;
    }

  return retval;
}






/************************************************************************
 
 
 ************************************************************************/

dbus_bool_t
_dbus_win_account_to_sid (const wchar_t *waccount,
                          void      	 **ppsid,
                          DBusError 	  *error)
{
  dbus_bool_t retval = FALSE;
  DWORD sid_length, wdomain_length;
  SID_NAME_USE use;
  wchar_t *wdomain;

  *ppsid = NULL;

  sid_length = 0;
  wdomain_length = 0;
  if (!LookupAccountNameW (NULL, waccount, NULL, &sid_length,
                           NULL, &wdomain_length, &use) &&
      GetLastError () != ERROR_INSUFFICIENT_BUFFER)
    {
      _dbus_win_set_error_from_win_error (error, GetLastError ());
      return FALSE;
    }

  *ppsid = dbus_malloc (sid_length);
  if (!*ppsid)
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  wdomain = dbus_new (wchar_t, wdomain_length);
  if (!wdomain)
    {
      _DBUS_SET_OOM (error);
      goto out1;
    }

  if (!LookupAccountNameW (NULL, waccount, (PSID) *ppsid, &sid_length,
                           wdomain, &wdomain_length, &use))
    {
      _dbus_win_set_error_from_win_error (error, GetLastError ());
      goto out2;
    }

  if (!IsValidSid ((PSID) *ppsid))
    {
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "Invalid SID");
      goto out2;
    }

  retval = TRUE;

out2:
  dbus_free (wdomain);
out1:
  if (!retval)
    {
      dbus_free (*ppsid);
      *ppsid = NULL;
    }

  return retval;
}

/** @} end of sysdeps-win */


/**
 * The only reason this is separate from _dbus_getpid() is to allow it
 * on Windows for logging but not for other purposes.
 * 
 * @returns process ID to put in log messages
 */
unsigned long
_dbus_pid_for_log (void)
{
  return _dbus_getpid ();
}

#ifndef DBUS_WINCE

static BOOL
is_winxp_sp3_or_lower (void)
{
   OSVERSIONINFOEX osvi;
   DWORDLONG dwlConditionMask = 0;
   int op=VER_LESS_EQUAL;

   // Initialize the OSVERSIONINFOEX structure.

   ZeroMemory(&osvi, sizeof(OSVERSIONINFOEX));
   osvi.dwOSVersionInfoSize = sizeof(OSVERSIONINFOEX);
   osvi.dwMajorVersion = 5;
   osvi.dwMinorVersion = 1;
   osvi.wServicePackMajor = 3;
   osvi.wServicePackMinor = 0;

   // Initialize the condition mask.

   VER_SET_CONDITION (dwlConditionMask, VER_MAJORVERSION, op);
   VER_SET_CONDITION (dwlConditionMask, VER_MINORVERSION, op);
   VER_SET_CONDITION (dwlConditionMask, VER_SERVICEPACKMAJOR, op);
   VER_SET_CONDITION (dwlConditionMask, VER_SERVICEPACKMINOR, op);

   // Perform the test.

   return VerifyVersionInfo(
      &osvi,
      VER_MAJORVERSION | VER_MINORVERSION |
      VER_SERVICEPACKMAJOR | VER_SERVICEPACKMINOR,
      dwlConditionMask);
}

/** Gets our SID
 * @param sid points to sid buffer, need to be freed with LocalFree()
 * @param process_id the process id for which the sid should be returned (use 0 for current process)
 * @returns process sid
 */
dbus_bool_t
_dbus_getsid(char **sid, dbus_pid_t process_id)
{
  HANDLE process_token = INVALID_HANDLE_VALUE;
  TOKEN_USER *token_user = NULL;
  DWORD n;
  PSID psid;
  int retval = FALSE;

  HANDLE process_handle;
  if (process_id == 0)
    process_handle = GetCurrentProcess();
  else if (is_winxp_sp3_or_lower())
    process_handle = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, process_id);
  else
    process_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, process_id);

  if (!OpenProcessToken (process_handle, TOKEN_QUERY, &process_token))
    {
      _dbus_win_warn_win_error ("OpenProcessToken failed", GetLastError ());
      goto failed;
    }
  if ((!GetTokenInformation (process_token, TokenUser, NULL, 0, &n)
            && GetLastError () != ERROR_INSUFFICIENT_BUFFER)
           || (token_user = alloca (n)) == NULL
           || !GetTokenInformation (process_token, TokenUser, token_user, n, &n))
    {
      _dbus_win_warn_win_error ("GetTokenInformation failed", GetLastError ());
      goto failed;
    }
  psid = token_user->User.Sid;
  if (!IsValidSid (psid))
    {
      _dbus_verbose("%s invalid sid\n",__FUNCTION__);
      goto failed;
    }
  if (!ConvertSidToStringSidA (psid, sid))
    {
      _dbus_verbose("%s invalid sid\n",__FUNCTION__);
      goto failed;
    }
//okay:
  retval = TRUE;

failed:
  CloseHandle (process_handle);
  if (process_token != INVALID_HANDLE_VALUE)
    CloseHandle (process_token);

  _dbus_verbose("_dbus_getsid() got '%s' and returns %d\n", *sid, retval);
  return retval;
}
#endif

/************************************************************************
 
 pipes
 
 ************************************************************************/

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
  SOCKET temp, socket1 = -1, socket2 = -1;
  struct sockaddr_in saddr;
  int len;
  u_long arg;

  if (!_dbus_win_startup_winsock ())
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  temp = socket (AF_INET, SOCK_STREAM, 0);
  if (temp == INVALID_SOCKET)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out0;
    }

  _DBUS_ZERO (saddr);
  saddr.sin_family = AF_INET;
  saddr.sin_port = 0;
  saddr.sin_addr.s_addr = htonl (INADDR_LOOPBACK);

  if (bind (temp, (struct sockaddr *)&saddr, sizeof (saddr)) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out0;
    }

  if (listen (temp, 1) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out0;
    }

  len = sizeof (saddr);
  if (getsockname (temp, (struct sockaddr *)&saddr, &len) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out0;
    }

  socket1 = socket (AF_INET, SOCK_STREAM, 0);
  if (socket1 == INVALID_SOCKET)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out0;
    }

  if (connect (socket1, (struct sockaddr  *)&saddr, len) == SOCKET_ERROR)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out1;
    }

  socket2 = accept (temp, (struct sockaddr *) &saddr, &len);
  if (socket2 == INVALID_SOCKET)
    {
      DBUS_SOCKET_SET_ERRNO ();
      goto out1;
    }

  if (!blocking)
    {
      arg = 1;
      if (ioctlsocket (socket1, FIONBIO, &arg) == SOCKET_ERROR)
        {
          DBUS_SOCKET_SET_ERRNO ();
          goto out2;
        }

      arg = 1;
      if (ioctlsocket (socket2, FIONBIO, &arg) == SOCKET_ERROR)
        {
          DBUS_SOCKET_SET_ERRNO ();
          goto out2;
        }
    }

  fd1->sock = socket1;
  fd2->sock = socket2;

  _dbus_verbose ("full-duplex pipe %Iu:%Iu <-> %Iu:%Iu\n",
                 fd1->sock, socket1, fd2->sock, socket2);

  closesocket (temp);

  return TRUE;

out2:
  closesocket (socket2);
out1:
  closesocket (socket1);
out0:
  closesocket (temp);

  dbus_set_error (error, _dbus_error_from_errno (errno),
                  "Could not setup socket pair: %s",
                  _dbus_strerror_from_errno ());

  return FALSE;
}

#ifdef DBUS_ENABLE_VERBOSE_MODE
static dbus_bool_t
_dbus_dump_fd_events (DBusPollFD *fds, int n_fds)
{
  DBusString msg = _DBUS_STRING_INIT_INVALID;
  dbus_bool_t result = FALSE;
  int i;

  if (!_dbus_string_init (&msg))
    goto oom;

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i];
      if (!_dbus_string_append (&msg, i > 0 ? "\n\t" : "\t"))
        goto oom;

      if ((fdp->events & _DBUS_POLLIN) &&
          !_dbus_string_append_printf (&msg, "R:%Iu ", fdp->fd.sock))
        goto oom;

      if ((fdp->events & _DBUS_POLLOUT) &&
          !_dbus_string_append_printf (&msg, "W:%Iu ", fdp->fd.sock))
        goto oom;

      if (!_dbus_string_append_printf (&msg, "E:%Iu", fdp->fd.sock))
        goto oom;
    }

  _dbus_verbose ("%s\n", _dbus_string_get_const_data (&msg));
  result = TRUE;
oom:
  _dbus_string_free (&msg);
  return result;
}

#ifdef USE_CHRIS_IMPL
static dbus_bool_t
_dbus_dump_fd_revents (DBusPollFD *fds, int n_fds)
{
  DBusString msg = _DBUS_STRING_INIT_INVALID;
  dbus_bool_t result = FALSE;
  int i;

  if (!_dbus_string_init (&msg))
    goto oom;

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i];
      if (!_dbus_string_append (&msg, i > 0 ? "\n\t" : "\t"))
        goto oom;

      if ((fdp->revents & _DBUS_POLLIN) &&
          !_dbus_string_append_printf (&msg, "R:%Iu ", fdp->fd.sock))
        goto oom;

      if ((fdp->revents & _DBUS_POLLOUT) &&
          !_dbus_string_append_printf (&msg, "W:%Iu ", fdp->fd.sock))
        goto oom;

      if ((fdp->revents & _DBUS_POLLERR) &&
          !_dbus_string_append_printf (&msg, "E:%Iu", fdp->fd.sock))
        goto oom;
    }

  _dbus_verbose ("%s\n", _dbus_string_get_const_data (&msg));
  result = TRUE;
oom:
  _dbus_string_free (&msg);
  return result;
}
#else
static dbus_bool_t
_dbus_dump_fdset (DBusPollFD *fds, int n_fds, fd_set *read_set, fd_set *write_set, fd_set *err_set)
{
  DBusString msg = _DBUS_STRING_INIT_INVALID;
  dbus_bool_t result = FALSE;
  int i;

  if (!_dbus_string_init (&msg))
    goto oom;

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i];

      if (!_dbus_string_append (&msg, i > 0 ? "\n\t" : "\t"))
        goto oom;

      if (FD_ISSET (fdp->fd.sock, read_set) &&
          !_dbus_string_append_printf (&msg, "R:%Iu ", fdp->fd.sock))
        goto oom;

      if (FD_ISSET (fdp->fd.sock, write_set) &&
          !_dbus_string_append_printf (&msg, "W:%Iu ", fdp->fd.sock))
        goto oom;

      if (FD_ISSET (fdp->fd.sock, err_set) &&
          !_dbus_string_append_printf (&msg, "E:%Iu", fdp->fd.sock))
        goto oom;
    }
  _dbus_verbose ("%s\n", _dbus_string_get_const_data (&msg));
  result = TRUE;
oom:
  _dbus_string_free (&msg);
  return result;
}
#endif
#endif

#ifdef USE_CHRIS_IMPL
/**
 * Windows event based implementation for _dbus_poll().
 *
 * @param fds the file descriptors to poll
 * @param n_fds number of descriptors in the array
 * @param timeout_milliseconds timeout or -1 for infinite
 * @returns numbers of fds with revents, or <0 on error
 */
static int
_dbus_poll_events (DBusPollFD *fds,
                   int         n_fds,
                   int         timeout_milliseconds)
{
  int ret = 0;
  int i;
  DWORD ready;

#define DBUS_STACK_WSAEVENTS 256
  WSAEVENT eventsOnStack[DBUS_STACK_WSAEVENTS];
  WSAEVENT *pEvents = NULL;
  if (n_fds > DBUS_STACK_WSAEVENTS)
    pEvents = calloc(sizeof(WSAEVENT), n_fds);
  else
    pEvents = eventsOnStack;

  if (pEvents == NULL)
   {
     _dbus_win_set_errno (ENOMEM);
     ret = -1;
     goto oom;
   }

#ifdef DBUS_ENABLE_VERBOSE_MODE
  _dbus_verbose ("_dbus_poll: to=%d", timeout_milliseconds);
  if (!_dbus_dump_fd_events (fds, n_fds))
    {
      _dbus_win_set_errno (ENOMEM);
      ret = -1;
      goto oom;
    }
#endif

  for (i = 0; i < n_fds; i++)
    pEvents[i] = WSA_INVALID_EVENT;

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i];
      WSAEVENT ev;
      long lNetworkEvents = FD_OOB;

      ev = WSACreateEvent();

      if (fdp->events & _DBUS_POLLIN)
        lNetworkEvents |= FD_READ | FD_ACCEPT | FD_CLOSE;

      if (fdp->events & _DBUS_POLLOUT)
        lNetworkEvents |= FD_WRITE | FD_CONNECT;

      WSAEventSelect (fdp->fd.sock, ev, lNetworkEvents);

      pEvents[i] = ev;
    }

  ready = WSAWaitForMultipleEvents (n_fds, pEvents, FALSE, timeout_milliseconds, FALSE);

  if (ready == WSA_WAIT_FAILED)
    {
      DBUS_SOCKET_SET_ERRNO ();
      if (errno != WSAEWOULDBLOCK)
        _dbus_verbose ("WSAWaitForMultipleEvents: failed: %s\n", _dbus_strerror_from_errno ());
      ret = -1;
    }
  else if (ready == WSA_WAIT_TIMEOUT)
    {
      _dbus_verbose ("WSAWaitForMultipleEvents: WSA_WAIT_TIMEOUT\n");
      ret = 0;
    }
  else if (ready < (WSA_WAIT_EVENT_0 + n_fds))
    {
      for (i = 0; i < n_fds; i++)
        {
          DBusPollFD *fdp = &fds[i];
          WSANETWORKEVENTS ne;

          fdp->revents = 0;

          WSAEnumNetworkEvents (fdp->fd.sock, pEvents[i], &ne);

          if (ne.lNetworkEvents & (FD_READ | FD_ACCEPT | FD_CLOSE))
            fdp->revents |= _DBUS_POLLIN;

          if (ne.lNetworkEvents & (FD_WRITE | FD_CONNECT))
            fdp->revents |= _DBUS_POLLOUT;

          if (ne.lNetworkEvents & (FD_OOB))
            fdp->revents |= _DBUS_POLLERR;

          if(ne.lNetworkEvents)
            ret++;

          WSAEventSelect (fdp->fd.sock, pEvents[i], 0);
        }
#ifdef DBUS_ENABLE_VERBOSE_MODE
      _dbus_verbose ("_dbus_poll: to=%d", timeout_milliseconds);
      if (!_dbus_dump_fd_revents (fds, n_fds))
        {
          _dbus_win_set_errno (ENOMEM);
          ret = -1;
          goto oom;
        }
#endif
    }
  else
    {
      _dbus_verbose ("WSAWaitForMultipleEvents: failed for unknown reason!");
      ret = -1;
    }

oom:
  if (pEvents != NULL)
    {
      for (i = 0; i < n_fds; i++)
        {
          if (pEvents[i] != WSA_INVALID_EVENT)
            WSACloseEvent (pEvents[i]);
        }
      if (n_fds > DBUS_STACK_WSAEVENTS)
        free (pEvents);
    }

  return ret;
}
#else
/**
 * Select based implementation for _dbus_poll().
 *
 * @param fds the file descriptors to poll
 * @param n_fds number of descriptors in the array
 * @param timeout_milliseconds timeout or -1 for infinite
 * @returns numbers of fds with revents, or <0 on error
 */
static int
_dbus_poll_select (DBusPollFD *fds,
                   int         n_fds,
                   int         timeout_milliseconds)
{
  fd_set read_set, write_set, err_set;
  SOCKET max_fd = 0;
  int i;
  struct timeval tv;
  int ready;

  FD_ZERO (&read_set);
  FD_ZERO (&write_set);
  FD_ZERO (&err_set);
#ifdef DBUS_ENABLE_VERBOSE_MODE
  _dbus_verbose("_dbus_poll: to=%d\n", timeout_milliseconds);
  if (!_dbus_dump_fd_events (fds, n_fds))
    {
      ready = -1;
      goto oom;
    }
#endif

  for (i = 0; i < n_fds; i++)
    {
      DBusPollFD *fdp = &fds[i]; 

      if (fdp->events & _DBUS_POLLIN)
        FD_SET (fdp->fd.sock, &read_set);

      if (fdp->events & _DBUS_POLLOUT)
        FD_SET (fdp->fd.sock, &write_set);

      FD_SET (fdp->fd.sock, &err_set);

      max_fd = MAX (max_fd, fdp->fd.sock);
    }

  // Avoid random lockups with send(), for lack of a better solution so far
  tv.tv_sec = timeout_milliseconds < 0 ? 1 : timeout_milliseconds / 1000;
  tv.tv_usec = timeout_milliseconds < 0 ? 0 : (timeout_milliseconds % 1000) * 1000;

  ready = select (max_fd + 1, &read_set, &write_set, &err_set, &tv);

  if (DBUS_SOCKET_API_RETURNS_ERROR (ready))
    {
      DBUS_SOCKET_SET_ERRNO ();
      if (errno != WSAEWOULDBLOCK)
        _dbus_verbose ("select: failed: %s\n", _dbus_strerror_from_errno ());
    }
  else if (ready == 0)
    _dbus_verbose ("select: = 0\n");
  else
    if (ready > 0)
      {
#ifdef DBUS_ENABLE_VERBOSE_MODE
        _dbus_verbose ("select: to=%d\n", ready);
        if (!_dbus_dump_fdset (fds, n_fds, &read_set, &write_set, &err_set))
          {
            _dbus_win_set_errno (ENOMEM);
            ready = -1;
            goto oom;
          }
#endif
        for (i = 0; i < n_fds; i++)
          {
            DBusPollFD *fdp = &fds[i];

            fdp->revents = 0;

            if (FD_ISSET (fdp->fd.sock, &read_set))
              fdp->revents |= _DBUS_POLLIN;

            if (FD_ISSET (fdp->fd.sock, &write_set))
              fdp->revents |= _DBUS_POLLOUT;

            if (FD_ISSET (fdp->fd.sock, &err_set))
              fdp->revents |= _DBUS_POLLERR;
          }
      }
oom:
  return ready;
}
#endif

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
#ifdef USE_CHRIS_IMPL
  return _dbus_poll_events (fds, n_fds, timeout_milliseconds);
#else
  return _dbus_poll_select (fds, n_fds, timeout_milliseconds);
#endif
}

/******************************************************************************
 
Original CVS version of dbus-sysdeps.c
 
******************************************************************************/
/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps.c Wrappers around system/libc features (internal to D-Bus implementation)
 * 
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
 * Copyright (C) 2005 Novell, Inc.
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
 * Creates a socket and connects to a socket at the given host 
 * and port. The connection fd is returned, and is set up as
 * nonblocking.
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

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_win_startup_winsock ())
    {
      _DBUS_SET_OOM (error);
      return _dbus_socket_get_invalid ();
    }

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
#ifdef AI_ADDRCONFIG
  hints.ai_flags = AI_ADDRCONFIG;
#else
  hints.ai_flags = 0;
#endif

  if ((res = getaddrinfo(host, port, &hints, &ai)) != 0 || !ai)
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (res),
                      "Failed to lookup host/port: \"%s:%s\": %s (%d)",
                      host, port, _dbus_strerror (res), res);
      goto out;
    }

  tmp = ai;
  while (tmp)
    {
      if ((fd.sock = socket (tmp->ai_family, SOCK_STREAM, 0)) == INVALID_SOCKET)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          dbus_set_error (error,
                          _dbus_error_from_errno (saved_errno),
                          "Failed to open socket: %s",
                          _dbus_strerror (saved_errno));
          _dbus_socket_invalidate (&fd);
          goto out;
        }
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);

      if (connect (fd.sock, (struct sockaddr*) tmp->ai_addr, tmp->ai_addrlen) == SOCKET_ERROR)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          closesocket(fd.sock);
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
          closesocket (fd.sock);
          _dbus_socket_invalidate (&fd);
          goto out;
        }
    }

  /* Every SOCKET is also a HANDLE. */
  _dbus_win_handle_set_close_on_exec ((HANDLE) fd.sock);

  if (!_dbus_set_socket_nonblocking (fd, error))
    {
      closesocket (fd.sock);
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
  int nlisten_fd = 0, res, i, port_num = -1;
  DBusList *bind_errors = NULL;
  DBusError *bind_error = NULL;
  DBusSocket *listen_fd = NULL;
  struct addrinfo hints;
  struct addrinfo *ai, *tmp;
  dbus_bool_t have_ipv4 = FALSE;
  dbus_bool_t have_ipv6 = FALSE;

  // On Vista, sockaddr_gen must be a sockaddr_in6, and not a sockaddr_in6_old
  //That's required for family == IPv6(which is the default on Vista if family is not given)
  //So we use our own union instead of sockaddr_gen:

  typedef union {
	struct sockaddr Address;
	struct sockaddr_in AddressIn;
	struct sockaddr_in6 AddressIn6;
  } mysockaddr_gen;

  *fds_p = NULL;
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_win_startup_winsock ())
    {
      _DBUS_SET_OOM (error);
      return -1;
    }

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
                      DBUS_ERROR_INVALID_ARGS,
                      "Unknown address family %s", family);
      return -1;
    }

  hints.ai_protocol = IPPROTO_TCP;
  hints.ai_socktype = SOCK_STREAM;
#ifdef AI_ADDRCONFIG
  hints.ai_flags = AI_ADDRCONFIG | AI_PASSIVE;
#else
  hints.ai_flags = AI_PASSIVE;
#endif

 redo_lookup_with_port:
  ai = NULL;
  if ((res = getaddrinfo(host, port, &hints, &ai)) != 0 || !ai)
    {
      dbus_set_error (error,
                      _dbus_error_from_errno (res),
                      "Failed to lookup host/port: \"%s:%s\": %s (%d)",
                      host ? host : "*", port, _dbus_strerror(res), res);
      return -1;
    }

  tmp = ai;
  while (tmp)
    {
      const int reuseaddr = 1, tcp_nodelay_on = 1;
      DBusSocket fd = DBUS_SOCKET_INIT, *newlisten_fd;

      if ((fd.sock = socket (tmp->ai_family, SOCK_STREAM, 0)) == INVALID_SOCKET)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          dbus_set_error (error,
                          _dbus_error_from_errno (saved_errno),
                         "Failed to open socket: %s",
                         _dbus_strerror (saved_errno));
          goto failed;
        }
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);

      if (setsockopt (fd.sock, SOL_SOCKET, SO_REUSEADDR, (const char *)&reuseaddr, sizeof(reuseaddr)) == SOCKET_ERROR)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          _dbus_warn ("Failed to set socket option \"%s:%s\": %s",
                      host ? host : "*", port, _dbus_strerror (saved_errno));
        }

      /* Nagle's algorithm imposes a huge delay on the initial messages
         going over TCP. */
      if (setsockopt (fd.sock, IPPROTO_TCP, TCP_NODELAY, (const char *)&tcp_nodelay_on, sizeof (tcp_nodelay_on)) == SOCKET_ERROR)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          _dbus_warn ("Failed to set TCP_NODELAY socket option \"%s:%s\": %s",
                      host ? host : "*", port, _dbus_strerror (saved_errno));
        }

      if (bind (fd.sock, (struct sockaddr*) tmp->ai_addr, tmp->ai_addrlen) == SOCKET_ERROR)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          closesocket (fd.sock);

          /*
           * We don't treat this as a fatal error, because there might be
           * other addresses that we can listen on. In particular:
           *
           * - If saved_errno is WSAEADDRINUSE after we
           *   "goto redo_lookup_with_port" after binding a port on one of the
           *   possible addresses, we will try to bind that same port on
           *   every address, including the same address again for a second
           *   time, which will fail with WSAEADDRINUSE .
           *
           * - If saved_errno is WSAEADDRINUSE, it might be because binding to
           *   an IPv6 address implicitly binds to a corresponding IPv4
           *   address or vice versa.
           *
           * - If saved_errno is WSAEADDRNOTAVAIL when we asked for family
           *   AF_UNSPEC, it might be because IPv6 is disabled for this
           *   particular interface.
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

      if (listen (fd.sock, 30 /* backlog */) == SOCKET_ERROR)
        {
          saved_errno = _dbus_get_low_level_socket_errno ();
          closesocket (fd.sock);
          _dbus_set_error_with_inet_sockaddr (error, tmp->ai_addr, tmp->ai_addrlen,
                                              "Failed to listen on socket",
                                              saved_errno);
          goto failed;
        }

      newlisten_fd = dbus_realloc(listen_fd, sizeof(DBusSocket)*(nlisten_fd+1));
      if (!newlisten_fd)
        {
          closesocket (fd.sock);
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY,
                          "Failed to allocate file handle array");
          goto failed;
        }
      listen_fd = newlisten_fd;
      listen_fd[nlisten_fd] = fd;
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
              mysockaddr_gen addr;
              socklen_t addrlen = sizeof(addr);
              char portbuf[NI_MAXSERV];

              if (getsockname (fd.sock, &addr.Address, &addrlen) == SOCKET_ERROR ||
                (res = getnameinfo (&addr.Address, addrlen, NULL, 0,
                                    portbuf, sizeof(portbuf),
                                    NI_NUMERICSERV)) != 0)
                {
                  saved_errno = _dbus_get_low_level_socket_errno ();
                  dbus_set_error (error, _dbus_error_from_errno (saved_errno),
                                  "Failed to resolve port \"%s:%s\": %s",
                                  host ? host : "*", port, _dbus_strerror (saved_errno));
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
      _dbus_combine_tcp_errors (&bind_errors, "Failed to bind", host, port, error);
      goto failed;
    }

  if (have_ipv4 && !have_ipv6)
    *retfamily = "ipv4";
  else if (!have_ipv4 && have_ipv6)
    *retfamily = "ipv6";

  sscanf(_dbus_string_get_const_data(retport), "%d", &port_num);

  for (i = 0 ; i < nlisten_fd ; i++)
    {
      _dbus_win_handle_set_close_on_exec ((HANDLE) listen_fd[i].sock);
      if (!_dbus_set_socket_nonblocking (listen_fd[i], error))
        {
          goto failed;
        }
    }

  *fds_p = listen_fd;

  /* This list might be non-empty even on success, because we might be
   * ignoring WSAEADDRINUSE or WSAEADDRNOTAVAIL */
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
    closesocket (listen_fd[i].sock);

  while ((bind_error = _dbus_list_pop_first (&bind_errors)))
    {
      dbus_error_free (bind_error);
      dbus_free (bind_error);
    }

  dbus_free(listen_fd);
  return -1;
}


/**
 * Accepts a connection on a listening socket.
 * Handles EINTR for you.
 *
 * @param listen_fd the listen file descriptor
 * @returns the connection fd of the client, or -1 on error
 */
DBusSocket
_dbus_accept  (DBusSocket listen_fd)
{
  DBusSocket client_fd;

 retry:
  client_fd.sock = accept (listen_fd.sock, NULL, NULL);

  if (!_dbus_socket_is_valid (client_fd))
    {
      DBUS_SOCKET_SET_ERRNO ();
      if (errno == EINTR)
        goto retry;
    }

  _dbus_verbose ("client fd %Iu accepted\n", client_fd.sock);
  
  return client_fd;
}




dbus_bool_t
_dbus_send_credentials_socket (DBusSocket      handle,
                               DBusError      *error)
{
/* FIXME: for the session bus credentials shouldn't matter (?), but
 * for the system bus they are presumably essential. A rough outline
 * of a way to implement the credential transfer would be this:
 *
 * client waits to *read* a byte.
 *
 * server creates a named pipe with a random name, sends a byte
 * contining its length, and its name.
 *
 * client reads the name, connects to it (using Win32 API).
 *
 * server waits for connection to the named pipe, then calls
 * ImpersonateNamedPipeClient(), notes its now-current credentials,
 * calls RevertToSelf(), closes its handles to the named pipe, and
 * is done. (Maybe there is some other way to get the SID of a named
 * pipe client without having to use impersonation?)
 *
 * client closes its handles and is done.
 * 
 * Ralf: Why not sending credentials over the given this connection ?
 * Using named pipes makes it impossible to be connected from a unix client.  
 *
 */
  int bytes_written;
  DBusString buf; 

  _dbus_string_init_const_len (&buf, "\0", 1);
again:
  bytes_written = _dbus_write_socket (handle, &buf, 0, 1 );

  if (bytes_written < 0 && errno == EINTR)
    goto again;

  if (bytes_written < 0)
    {
      dbus_set_error (error, _dbus_error_from_errno (errno),
                      "Failed to write credentials byte: %s",
                     _dbus_strerror_from_errno ());
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
      _dbus_verbose ("wrote 1 zero byte, credential sending isn't implemented yet\n");
      return TRUE;
    }
  return TRUE;
}

/**
 * Reads a single byte which must be nul (an error occurs otherwise),
 * and reads unix credentials if available. Fills in pid/uid/gid with
 * -1 if no credentials are available. Return value indicates whether
 * a byte was read, not whether we got valid credentials. On some
 * systems, such as Linux, reading/writing the byte isn't actually
 * required, but we do it anyway just to avoid multiple codepaths.
 *
 * Fails if no byte is available, so you must select() first.
 *
 * The point of the byte is that on some systems we have to
 * use sendmsg()/recvmsg() to transmit credentials.
 *
 * @param handle the client file descriptor
 * @param credentials struct to fill with credentials of client
 * @param error location to store error code
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_read_credentials_socket  (DBusSocket       handle,
                                DBusCredentials *credentials,
                                DBusError       *error)
{
  int bytes_read = 0;
  DBusString buf;

  char *sid = NULL;
  dbus_pid_t pid;
  int retval = FALSE;

  // could fail due too OOM
  if (_dbus_string_init (&buf))
    {
      bytes_read = _dbus_read_socket (handle, &buf, 1 );

      if (bytes_read > 0) 
        _dbus_verbose ("got one zero byte from server\n");

      _dbus_string_free (&buf);
    }

  pid = _dbus_get_peer_pid_from_tcp_handle (handle.sock);
  if (pid == 0)
    return TRUE;

  _dbus_credentials_add_pid (credentials, pid);

  if (_dbus_getsid (&sid, pid))
    {
      if (!_dbus_credentials_add_windows_sid (credentials, sid))
        goto out;
    }

  retval = TRUE;

out:
  if (sid)
    LocalFree (sid);

  return retval;
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
  /* TODO */
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
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

  dir_ends_in_slash =
    ('/' == _dbus_string_get_byte (dir, _dbus_string_get_length (dir) - 1) ||
     '\\' == _dbus_string_get_byte (dir, _dbus_string_get_length (dir) - 1));

  file_starts_with_slash =
    ('/' == _dbus_string_get_byte (next_component, 0) ||
     '\\' == _dbus_string_get_byte (next_component, 0));

  if (dir_ends_in_slash && file_starts_with_slash)
    {
      _dbus_string_shorten (dir, 1);
    }
  else if (!(dir_ends_in_slash || file_starts_with_slash))
    {
      if (!_dbus_string_append_byte (dir, '\\'))
        return FALSE;
    }

  return _dbus_string_copy (next_component, 0, dir,
                            _dbus_string_get_length (dir));
}

/*---------------- DBusCredentials ----------------------------------*/

/**
 * Adds the credentials corresponding to the given username.
 *
 * @param credentials credentials to fill in 
 * @param username the username
 * @returns #TRUE if the username existed and we got some credentials
 */
dbus_bool_t
_dbus_credentials_add_from_user (DBusCredentials         *credentials,
                                 const DBusString        *username,
                                 DBusCredentialsAddFlags  flags,
                                 DBusError               *error)
{
  if (!_dbus_credentials_add_windows_sid (credentials,
                                          _dbus_string_get_const_data (username)))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

/**
 * Adds the credentials of the current process to the
 * passed-in credentials object.
 *
 * @param credentials credentials to add to
 * @returns #FALSE if no memory; does not properly roll back on failure, so only some credentials may have been added
 */

dbus_bool_t
_dbus_credentials_add_from_current_process (DBusCredentials *credentials)
{
  dbus_bool_t retval = FALSE;
  char *sid = NULL;

  if (!_dbus_getsid(&sid, _dbus_getpid()))
    goto failed;

  if (!_dbus_credentials_add_pid (credentials, _dbus_getpid()))
    goto failed;

  if (!_dbus_credentials_add_windows_sid (credentials,sid))
    goto failed;

  retval = TRUE;
  goto end;
failed:
  retval = FALSE;
end:
  if (sid)
    LocalFree(sid);

  return retval;
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
 * @todo to which class belongs this 
 */
dbus_bool_t
_dbus_append_user_from_current_process (DBusString *str)
{
  dbus_bool_t retval = FALSE;
  char *sid = NULL;

  if (!_dbus_getsid(&sid, _dbus_getpid()))
    return FALSE;

  retval = _dbus_string_append (str,sid);

  LocalFree(sid);
  return retval;
}

/**
 * Gets our process ID
 * @returns process ID
 */
dbus_pid_t
_dbus_getpid (void)
{
  return GetCurrentProcessId ();
}

/** Gets our Unix UID
 * @returns on Windows, just DBUS_UID_UNSET
 */
dbus_uid_t
_dbus_getuid (void)
{
  return DBUS_UID_UNSET;
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
  Sleep (milliseconds);
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
  FILETIME ft;
  dbus_uint64_t time64;

  GetSystemTimeAsFileTime (&ft);

  memcpy (&time64, &ft, sizeof (time64));

  /* Convert from 100s of nanoseconds since 1601-01-01
  * to Unix epoch. Yes, this is Y2038 unsafe.
  */
  time64 -= DBUS_INT64_CONSTANT (116444736000000000);
  time64 /= 10;

  if (tv_sec)
    *tv_sec = time64 / 1000000;

  if (tv_usec)
    *tv_usec = time64 % 1000000;
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
  /* no implementation yet, fall back to wall-clock time */
  _dbus_get_real_time (tv_sec, tv_usec);
}

/**
 * signal (SIGPIPE, SIG_IGN);
 */
void
_dbus_disable_sigpipe (void)
{
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

  if (!CreateDirectoryA (filename_c, NULL))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to create directory %s: %s\n",
                      filename_c, _dbus_strerror_from_errno ());
      return FALSE;
    }
  else
    return TRUE;
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

  if (!CreateDirectoryA (filename_c, NULL))
    {
      if (GetLastError () == ERROR_ALREADY_EXISTS)
        return TRUE;

      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to create directory %s: %s\n",
                      filename_c, _dbus_strerror_from_errno ());
      return FALSE;
    }
  else
    return TRUE;
}


/**
 * Generates the given number of random bytes,
 * using the best mechanism we can come up with.
 *
 * @param str the string
 * @param n_bytes the number of random bytes to append to string
 * @param error location to store reason for failure
 * @returns #TRUE on success
 */
dbus_bool_t
_dbus_generate_random_bytes (DBusString *str,
                             int         n_bytes,
                             DBusError  *error)
{
  int old_len;
  unsigned char *p;
  HCRYPTPROV hprov;

  old_len = _dbus_string_get_length (str);

  if (!_dbus_string_lengthen (str, n_bytes))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  p = _dbus_string_get_udata_len (str, old_len, n_bytes);

  if (!CryptAcquireContext (&hprov, NULL, NULL, PROV_RSA_FULL, CRYPT_VERIFYCONTEXT))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!CryptGenRandom (hprov, n_bytes, p))
    {
      _DBUS_SET_OOM (error);
      CryptReleaseContext (hprov, 0);
      return FALSE;
    }

  CryptReleaseContext (hprov, 0);

  return TRUE;
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
  static char buf[1000];

  if (!_DBUS_LOCK (sysdeps))
    return NULL;

  if (tmpdir == NULL)
    {
      unsigned char *last_slash;
      unsigned char *p = (unsigned char *)buf;

      if (!GetTempPathA (sizeof (buf), buf))
        {
          _dbus_warn ("GetTempPath failed");
          _dbus_abort ();
        }

      /* Drop terminating backslash or slash */
      last_slash = _mbsrchr (p, '\\');
      if (last_slash > p && last_slash[1] == '\0')
        last_slash[0] = '\0';
      last_slash = _mbsrchr (p, '/');
      if (last_slash > p && last_slash[1] == '\0')
        last_slash[0] = '\0';

      tmpdir = buf;
    }

  _DBUS_UNLOCK (sysdeps);

  _dbus_assert(tmpdir != NULL);

  return tmpdir;
}


/**
 * Deletes the given file.
 *
 * @param filename the filename
 * @param error error location
 * 
 * @returns #TRUE if unlink() succeeded
 */
dbus_bool_t
_dbus_delete_file (const DBusString *filename,
                   DBusError        *error)
{
  const char *filename_c;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  filename_c = _dbus_string_get_const_data (filename);

  if (DeleteFileA (filename_c) == 0)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Failed to delete file %s: %s\n",
                      filename_c, _dbus_strerror_from_errno ());
      return FALSE;
    }
  else
    return TRUE;
}

#if !defined (DBUS_DISABLE_ASSERT) || defined(DBUS_ENABLE_EMBEDDED_TESTS)

#if defined(_MSC_VER) || defined(DBUS_WINCE)
# ifdef BACKTRACES
#  undef BACKTRACES
# endif
#else
# define BACKTRACES
#endif

#ifdef BACKTRACES
/*
 * Backtrace Generator
 *
 * Copyright 2004 Eric Poech
 * Copyright 2004 Robert Shearman
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 */

#include <winver.h>
#include <imagehlp.h>
#include <stdio.h>

#define DPRINTF(fmt, ...) fprintf (stderr, fmt, ##__VA_ARGS__)

#ifdef _MSC_VER
#define BOOL int

#define __i386__
#endif

static void dump_backtrace_for_thread (HANDLE hThread)
{
  ADDRESS old_address;
  STACKFRAME sf;
  CONTEXT context;
  DWORD dwImageType;
  int i = 0;

  SymSetOptions (SYMOPT_UNDNAME | SYMOPT_LOAD_LINES);
  SymInitialize (GetCurrentProcess (), NULL, TRUE);


  /* can't use this function for current thread as GetThreadContext
   * doesn't support getting context from current thread */
  if (hThread == GetCurrentThread())
    return;

  DPRINTF ("Backtrace:\n");

  _DBUS_ZERO (old_address);
  _DBUS_ZERO (context);
  context.ContextFlags = CONTEXT_FULL;

  SuspendThread (hThread);

  if (!GetThreadContext (hThread, &context))
    {
      DPRINTF ("Couldn't get thread context (error %ld)\n", GetLastError ());
      ResumeThread (hThread);
      return;
    }

  _DBUS_ZERO (sf);

#ifdef __i386__
  dwImageType         = IMAGE_FILE_MACHINE_I386;
  sf.AddrFrame.Offset = context.Ebp;
  sf.AddrFrame.Mode   = AddrModeFlat;
  sf.AddrPC.Offset    = context.Eip;
  sf.AddrPC.Mode      = AddrModeFlat;
#elif defined(_M_X64)
  dwImageType         = IMAGE_FILE_MACHINE_AMD64;
  sf.AddrPC.Offset    = context.Rip;
  sf.AddrPC.Mode      = AddrModeFlat;
  sf.AddrFrame.Offset = context.Rsp;
  sf.AddrFrame.Mode   = AddrModeFlat;
  sf.AddrStack.Offset = context.Rsp;
  sf.AddrStack.Mode   = AddrModeFlat;
#elif defined(_M_IA64)
  dwImageType         = IMAGE_FILE_MACHINE_IA64;
  sf.AddrPC.Offset    = context.StIIP;
  sf.AddrPC.Mode      = AddrModeFlat;
  sf.AddrFrame.Offset = context.IntSp;
  sf.AddrFrame.Mode   = AddrModeFlat;
  sf.AddrBStore.Offset= context.RsBSP;
  sf.AddrBStore.Mode  = AddrModeFlat;
  sf.AddrStack.Offset = context.IntSp;
  sf.AddrStack.Mode   = AddrModeFlat;
#else
# error You need to fill in the STACKFRAME structure for your architecture
#endif

  /*
    backtrace format
    <level> <address> <symbol>[+offset] [ '[' <file> ':' <line> ']' ] [ 'in' <module> ]
    example:
      6 0xf75ade6b wine_switch_to_stack+0x2a [/usr/src/debug/wine-snapshot/libs/wine/port.c:59] in libwine.so.1
  */
  while (StackWalk (dwImageType, GetCurrentProcess (),
                    hThread, &sf, &context, NULL, SymFunctionTableAccess,
                    SymGetModuleBase, NULL))
    {
      char buffer[sizeof(SYMBOL_INFO) + MAX_SYM_NAME * sizeof(char)];
      PSYMBOL_INFO pSymbol = (PSYMBOL_INFO)buffer;
      DWORD64 displacement;
      IMAGEHLP_LINE line;
      DWORD dwDisplacement;
      IMAGEHLP_MODULE moduleInfo;

      /*
         on Wine64 version 1.7.54, we get an infinite number of stack entries
         pointing to the same stack frame  (_start+0x29 in <wine-loader>)
         see bug https://bugs.winehq.org/show_bug.cgi?id=39606
      */
#ifndef __i386__
      if (old_address.Offset == sf.AddrPC.Offset)
        {
          break;
        }
#endif

      pSymbol->SizeOfStruct = sizeof(SYMBOL_INFO);
      pSymbol->MaxNameLen = MAX_SYM_NAME;

      if (SymFromAddr (GetCurrentProcess (), sf.AddrPC.Offset, &displacement, pSymbol))
        {
          if (displacement)
            DPRINTF ("%3d %s+0x%I64x", i++, pSymbol->Name, displacement);
          else
            DPRINTF ("%3d %s", i++, pSymbol->Name);
        }
      else
        DPRINTF ("%3d 0x%Ix", i++, sf.AddrPC.Offset);

      line.SizeOfStruct = sizeof(IMAGEHLP_LINE);
      if (SymGetLineFromAddr (GetCurrentProcess (), sf.AddrPC.Offset, &dwDisplacement, &line))
        {
          DPRINTF (" [%s:%ld]", line.FileName, line.LineNumber);
        }

      moduleInfo.SizeOfStruct = sizeof(moduleInfo);
      if (SymGetModuleInfo (GetCurrentProcess (), sf.AddrPC.Offset, &moduleInfo))
        {
          DPRINTF (" in %s", moduleInfo.ModuleName);
        }
      DPRINTF ("\n");
      old_address = sf.AddrPC;
    }
  ResumeThread (hThread);
}

static DWORD WINAPI dump_thread_proc (LPVOID lpParameter)
{
  dump_backtrace_for_thread ((HANDLE) lpParameter);
  return 0;
}

/* cannot get valid context from current thread, so we have to execute
 * backtrace from another thread */
static void
dump_backtrace (void)
{
  HANDLE hCurrentThread;
  HANDLE hThread;
  DWORD dwThreadId;
  DuplicateHandle (GetCurrentProcess (), GetCurrentThread (),
                   GetCurrentProcess (), &hCurrentThread,
                   0, FALSE, DUPLICATE_SAME_ACCESS);
  hThread = CreateThread (NULL, 0, dump_thread_proc, (LPVOID)hCurrentThread,
                          0, &dwThreadId);
  WaitForSingleObject (hThread, INFINITE);
  CloseHandle (hThread);
  CloseHandle (hCurrentThread);
}
#endif
#endif /* asserts or tests enabled */

#ifdef BACKTRACES
void _dbus_print_backtrace (void)
{
  dump_backtrace ();
}
#else
void _dbus_print_backtrace (void)
{
  _dbus_verbose ("  D-Bus not compiled with backtrace support\n");
}
#endif

static dbus_uint32_t fromAscii(char ascii)
{
    if(ascii >= '0' && ascii <= '9')
        return ascii - '0';
    if(ascii >= 'A' && ascii <= 'F')
        return ascii - 'A' + 10;
    if(ascii >= 'a' && ascii <= 'f')
        return ascii - 'a' + 10;
    return 0;    
}

dbus_bool_t _dbus_read_local_machine_uuid   (DBusGUID         *machine_id,
                                             dbus_bool_t       create_if_not_found,
                                             DBusError        *error)
{
#ifdef DBUS_WINCE
	return TRUE;
  // TODO
#else
    HW_PROFILE_INFOA info;
    char *lpc = &info.szHwProfileGuid[0];
    dbus_uint32_t u;

    //  the hw-profile guid lives long enough
    if(!GetCurrentHwProfileA(&info))
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL); // FIXME
        return FALSE;  
      }

    // Form: {12340001-4980-1920-6788-123456789012}
    lpc++;
    // 12340001
    u = ((fromAscii(lpc[0]) <<  0) |
         (fromAscii(lpc[1]) <<  4) |
         (fromAscii(lpc[2]) <<  8) |
         (fromAscii(lpc[3]) << 12) |
         (fromAscii(lpc[4]) << 16) |
         (fromAscii(lpc[5]) << 20) |
         (fromAscii(lpc[6]) << 24) |
         (fromAscii(lpc[7]) << 28));
    machine_id->as_uint32s[0] = u;

    lpc += 9;
    // 4980-1920
    u = ((fromAscii(lpc[0]) <<  0) |
         (fromAscii(lpc[1]) <<  4) |
         (fromAscii(lpc[2]) <<  8) |
         (fromAscii(lpc[3]) << 12) |
         (fromAscii(lpc[5]) << 16) |
         (fromAscii(lpc[6]) << 20) |
         (fromAscii(lpc[7]) << 24) |
         (fromAscii(lpc[8]) << 28));
    machine_id->as_uint32s[1] = u;
    
    lpc += 10;
    // 6788-1234
    u = ((fromAscii(lpc[0]) <<  0) |
         (fromAscii(lpc[1]) <<  4) |
         (fromAscii(lpc[2]) <<  8) |
         (fromAscii(lpc[3]) << 12) |
         (fromAscii(lpc[5]) << 16) |
         (fromAscii(lpc[6]) << 20) |
         (fromAscii(lpc[7]) << 24) |
         (fromAscii(lpc[8]) << 28));
    machine_id->as_uint32s[2] = u;
    
    lpc += 9;
    // 56789012
    u = ((fromAscii(lpc[0]) <<  0) |
         (fromAscii(lpc[1]) <<  4) |
         (fromAscii(lpc[2]) <<  8) |
         (fromAscii(lpc[3]) << 12) |
         (fromAscii(lpc[4]) << 16) |
         (fromAscii(lpc[5]) << 20) |
         (fromAscii(lpc[6]) << 24) |
         (fromAscii(lpc[7]) << 28));
    machine_id->as_uint32s[3] = u;
#endif
    return TRUE;
}

static
HANDLE _dbus_global_lock (const char *mutexname)
{
  HANDLE mutex;
  DWORD gotMutex;

  mutex = CreateMutexA (NULL, FALSE, mutexname);
  if (!mutex)
    {
      return FALSE;
    }

   gotMutex = WaitForSingleObject (mutex, INFINITE);
   switch (gotMutex)
     {
       case WAIT_ABANDONED:
               ReleaseMutex (mutex);
               CloseHandle (mutex);
               return 0;
       case WAIT_FAILED:
       case WAIT_TIMEOUT:
               return 0;
       default:
               return mutex;
     }
}

static
void _dbus_global_unlock (HANDLE mutex)
{
  ReleaseMutex (mutex);
  CloseHandle (mutex); 
}

// for proper cleanup in dbus-daemon
static HANDLE hDBusDaemonMutex = NULL;
static HANDLE hDBusSharedMem = NULL;
// sync _dbus_daemon_publish_session_bus_address, _dbus_daemon_unpublish_session_bus_address and _dbus_daemon_already_runs
static const char *cUniqueDBusInitMutex = "UniqueDBusInitMutex";
// sync _dbus_get_autolaunch_address
static const char *cDBusAutolaunchMutex = "DBusAutolaunchMutex";
// mutex to determine if dbus-daemon is already started (per user)
static const char *cDBusDaemonMutex = "DBusDaemonMutex";
// named shm for dbus adress info (per user)
static const char *cDBusDaemonAddressInfo = "DBusDaemonAddressInfo";

/**
 * Return the hash of the installation root directory, which can be
 * used to construct a per-installation-root scope for autolaunching
 *
 * If the installation root directory could not be
 * determined, the returned length is set to zero.
 *
 * @param out initialized DBusString instance to return hash string
 * @returns #FALSE on OOM, #TRUE if not OOM
 */
static dbus_bool_t
_dbus_get_install_root_as_hash (DBusString *out)
{
  DBusString install_path;
  dbus_bool_t retval = FALSE;
  _dbus_assert (out != NULL);

  if (!_dbus_string_init (&install_path))
    return FALSE;

  if (!_dbus_get_install_root (&install_path))
    goto out;

  /* the install path can't be determined */
  if (_dbus_string_get_length (&install_path) == 0)
    {
      _dbus_string_set_length (out, 0);
      retval = TRUE;
      goto out;
    }

  _dbus_string_tolower_ascii (&install_path, 0, _dbus_string_get_length (&install_path));

  if (!_dbus_sha_compute (&install_path, out))
    goto out;

  retval = TRUE;

out:
  _dbus_string_free (&install_path);
  return retval;
}

/**
 * Build a name from \p basestring and \p scope, and append it to \p out
 *
 * The name will be suitable for naming Windows objects such as mutexes
 * and shared memory segments that need to be unique for each distinct
 * \p scope, but shared between clients with the same \p scope.
 *
 * If \p scope has one of the special values recognised in autolaunch:
 * addresses on Windows, substitute a unique string based on the scope
 * (the username or the hash of the installation path) instead of the
 * literal scope itself.
 *
 * With the '*install-path' \p scope the returned length can be zero,
 * indicating that the name could not be determined.
 *
 * @param out initialized DBusString instance to return bus address
 * @returns #FALSE on OOM, #TRUE if not OOM
 */
static dbus_bool_t
_dbus_get_address_string (DBusString *out, const char *basestring, const char *scope)
{
  _dbus_assert (out != NULL);

  if (!scope || strlen (scope) == 0)
    {
      return _dbus_string_append (out, basestring);
    }
  else if (strcmp (scope, "*install-path") == 0
        // for 1.3 compatibility
        || strcmp (scope, "install-path") == 0)
    {
      DBusString temp;
      dbus_bool_t retval = FALSE;

      if (!_dbus_string_init (&temp))
        return FALSE;

      if (!_dbus_get_install_root_as_hash (&temp))
        goto out;

      if (_dbus_string_get_length (&temp) == 0)
        {
          _dbus_string_set_length (out, 0);
          retval = TRUE;
          goto out;
        }

      if (!_dbus_string_append_printf (out, "%s-%s", basestring, _dbus_string_get_const_data (&temp)))
        goto out;

      retval = TRUE;
out:
      _dbus_string_free (&temp);
      return retval;
    }
  else if (strcmp (scope, "*user") == 0)
    {
      char *sid = NULL;
      dbus_bool_t retval;

      if (!_dbus_getsid (&sid, _dbus_getpid()))
        return FALSE;

      retval = _dbus_string_append_printf (out, "%s-%s", basestring, sid);

      LocalFree(sid);

      return retval;
    }
  else /* strlen(scope) > 0 */
    {
      return _dbus_string_append_printf (out, "%s-%s", basestring, scope);
    }
}

/**
 * Return name of shared memory segment constructed from the autolaunch scope \p scope
 *
 * See @ref _dbus_get_address_string for further usage information.
 *
 * @param out initialized DBusString instance to return shared memory segment name
 * @returns #FALSE on OOM, #TRUE if not OOM
 */
static dbus_bool_t
_dbus_get_shm_name (DBusString *out,const char *scope)
{
  return _dbus_get_address_string (out, cDBusDaemonAddressInfo, scope);
}

/**
 * Return mutex name for scope \p scope in \p out
 *
 * See @ref _dbus_get_address_string for further usage information.
 *
 * @param out initialized DBusString instance to return mutex name
 * @param scope scope for the requested mutex name
 * @returns #FALSE on OOM, #TRUE if not OOM
 */
static dbus_bool_t
_dbus_get_mutex_name (DBusString *out, const char *scope)
{
  return _dbus_get_address_string (out, cDBusDaemonMutex, scope);
}

dbus_bool_t
_dbus_daemon_is_session_bus_address_published (const char *scope)
{
  HANDLE lock;
  DBusString mutex_name;

  if (!_dbus_string_init (&mutex_name))
    return FALSE;

  _dbus_verbose ("scope:%s\n", scope);
  if (!_dbus_get_mutex_name (&mutex_name, scope) ||
      /* not determinable */
      _dbus_string_get_length (&mutex_name) == 0)
    {
      _dbus_string_free (&mutex_name);
      return FALSE;
    }

  if (hDBusDaemonMutex)
    {
      _dbus_verbose ("(scope:%s) -> yes\n", scope);
      return TRUE;
    }

  // sync _dbus_daemon_publish_session_bus_address, _dbus_daemon_unpublish_session_bus_address and _dbus_daemon_already_runs
  lock = _dbus_global_lock (cUniqueDBusInitMutex);

  // we use CreateMutex instead of OpenMutex because of possible race conditions,
  // see http://msdn.microsoft.com/en-us/library/ms684315%28VS.85%29.aspx
  hDBusDaemonMutex = CreateMutexA (NULL, FALSE, _dbus_string_get_const_data(&mutex_name));

  /* The client uses mutex ownership to detect a running server, so the server should do so too.
     Fortunally the client deletes the mutex in the lock protected area, so checking presence 
     will work too.  */

  _dbus_global_unlock (lock);

  _dbus_string_free (&mutex_name);

  if (hDBusDaemonMutex  == NULL)
    {
      _dbus_verbose ("(scope:%s) -> no\n", scope);
      return FALSE;
    }
  if (GetLastError() == ERROR_ALREADY_EXISTS)
    {
      CloseHandle(hDBusDaemonMutex);
      hDBusDaemonMutex = NULL;
      _dbus_verbose ("(scope:%s) -> yes\n", scope);
      return TRUE;
    }
  // mutex wasn't created before, so return false.
  // We leave the mutex name allocated for later reusage
  // in _dbus_daemon_publish_session_bus_address.
  _dbus_verbose ("(scope:%s) -> no\n", scope);
  return FALSE;
}

dbus_bool_t
_dbus_daemon_publish_session_bus_address (const char* address, const char *scope)
{
  HANDLE lock;
  char *shared_addr = NULL;
  DBusString shm_name;
  DBusString mutex_name;
  dbus_uint64_t len;

  _dbus_assert (address);

  if (!_dbus_string_init (&mutex_name))
    return FALSE;

  _dbus_verbose ("address:%s scope:%s\n", address, scope);
  if (!_dbus_get_mutex_name (&mutex_name, scope) ||
      /* not determinable */
      _dbus_string_get_length (&mutex_name) == 0)
    {
      _dbus_string_free (&mutex_name);
      return FALSE;
    }

  // sync _dbus_daemon_publish_session_bus_address, _dbus_daemon_unpublish_session_bus_address and _dbus_daemon_already_runs
  lock = _dbus_global_lock (cUniqueDBusInitMutex);

  if (!hDBusDaemonMutex)
    {
      hDBusDaemonMutex = CreateMutexA (NULL, FALSE, _dbus_string_get_const_data(&mutex_name));
    }
  _dbus_string_free (&mutex_name);

  // acquire the mutex
  if (WaitForSingleObject (hDBusDaemonMutex, 10) != WAIT_OBJECT_0)
    {
      _dbus_global_unlock (lock);
      CloseHandle (hDBusDaemonMutex);
      return FALSE;
    }

  if (!_dbus_string_init (&shm_name))
    {
      _dbus_global_unlock (lock);
      return FALSE;
    }

  if (!_dbus_get_shm_name (&shm_name, scope) ||
      /* not determinable */
      _dbus_string_get_length (&shm_name) == 0)
    {
      _dbus_string_free (&shm_name);
      _dbus_global_unlock (lock);
      return FALSE;
    }

  // create shm
  len = strlen (address) + 1;

  hDBusSharedMem = CreateFileMappingA ( INVALID_HANDLE_VALUE, NULL, PAGE_READWRITE,
                                       len >> 32, len & 0xffffffffu,
                                       _dbus_string_get_const_data (&shm_name) );
  _dbus_assert (hDBusSharedMem);

  shared_addr = MapViewOfFile (hDBusSharedMem, FILE_MAP_WRITE, 0, 0, 0);

  _dbus_assert (shared_addr);

  strcpy(shared_addr, address);

  // cleanup
  UnmapViewOfFile (shared_addr);

  _dbus_global_unlock (lock);
  _dbus_verbose ("published session bus address at %s\n",_dbus_string_get_const_data (&shm_name));

  _dbus_string_free (&shm_name);
  return TRUE;
}

/**
 * Clear the platform-specific centralized location where the session
 * bus address is published.
 *
 * This must only be called if \ref DBusServer.published_address is #TRUE,
 * which is be the case if and only if platform-specific code has published
 * the address centrally.
 *
 * On Windows, this is implemented by closing a global shared memory segment.
 *
 * On Unix, the session bus address is not published in a centralized
 * location by libdbus, so this function does nothing. The closest
 * equivalent on Unix is that the session bus address is published by the
 * dbus-launch tool, and unpublished automatically when the dbus-launch
 * tool exits.
 */
void
_dbus_daemon_unpublish_session_bus_address (void)
{
  HANDLE lock;

  _dbus_verbose ("\n");
  // sync _dbus_daemon_publish_session_bus_address, _dbus_daemon_unpublish_session_bus_address and _dbus_daemon_already_runs
  lock = _dbus_global_lock (cUniqueDBusInitMutex);

  CloseHandle (hDBusSharedMem);

  hDBusSharedMem = NULL;

  ReleaseMutex (hDBusDaemonMutex);

  CloseHandle (hDBusDaemonMutex);

  hDBusDaemonMutex = NULL;

  _dbus_global_unlock (lock);
}

static dbus_bool_t
_dbus_get_autolaunch_shm (DBusString *address, DBusString *shm_name)
{
  HANDLE sharedMem;
  char *shared_addr;
  int i;

  // read shm
  for(i=0;i<20;++i) {
      // we know that dbus-daemon is available, so we wait until shm is available
      sharedMem = OpenFileMappingA (FILE_MAP_READ, FALSE, _dbus_string_get_const_data (shm_name));
      if (sharedMem == 0)
          Sleep (100);
      if ( sharedMem != 0)
          break;
  }

  if (sharedMem == 0)
      return FALSE;

  shared_addr = MapViewOfFile (sharedMem, FILE_MAP_READ, 0, 0, 0);

  if (!shared_addr)
      return FALSE;

  _dbus_string_init (address);

  _dbus_string_append (address, shared_addr);

  // cleanup
  UnmapViewOfFile (shared_addr);

  CloseHandle (sharedMem);

  return TRUE;
}

static dbus_bool_t
_dbus_daemon_already_runs (DBusString *address, DBusString *shm_name, const char *scope)
{
  HANDLE lock;
  HANDLE daemon;
  DBusString mutex_name;
  dbus_bool_t bRet = TRUE;

  if (!_dbus_string_init (&mutex_name))
    return FALSE;

  if (!_dbus_get_mutex_name (&mutex_name,scope) ||
      /* not determinable */
      _dbus_string_get_length (&mutex_name) == 0)
    {
      _dbus_string_free (&mutex_name);
      return FALSE;
    }

  // sync _dbus_daemon_publish_session_bus_address, _dbus_daemon_unpublish_session_bus_address and _dbus_daemon_already_runs
  lock = _dbus_global_lock (cUniqueDBusInitMutex);

  // do checks
  daemon = CreateMutexA (NULL, FALSE, _dbus_string_get_const_data (&mutex_name));
  if(WaitForSingleObject (daemon, 10) != WAIT_TIMEOUT)
    {
      ReleaseMutex (daemon);
      CloseHandle (daemon);

      _dbus_global_unlock (lock);
      _dbus_string_free (&mutex_name);
      return FALSE;
    }

  // read shm
  bRet = _dbus_get_autolaunch_shm (address, shm_name);

  // cleanup
  CloseHandle (daemon);

  _dbus_global_unlock (lock);
  _dbus_string_free (&mutex_name);

  return bRet;
}

dbus_bool_t
_dbus_get_autolaunch_address (const char *scope,
                              DBusString *address,
                              DBusError *error)
{
  HANDLE mutex = NULL;
  STARTUPINFOA si;
  PROCESS_INFORMATION pi;
  dbus_bool_t retval = FALSE;
  LPSTR lpFile;
  char dbus_exe_path[MAX_PATH];
  DBusString dbus_args = _DBUS_STRING_INIT_INVALID;
  const char * daemon_name = DBUS_DAEMON_NAME ".exe";
  DBusString shm_name;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&shm_name))
    {
      _DBUS_SET_OOM(error);
      return FALSE;
    }

  if (!_dbus_get_shm_name (&shm_name, scope) ||
      /* not determinable */
      _dbus_string_get_length (&shm_name) == 0)
    {
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "could not determine shm name");
      goto out;
    }

  mutex = _dbus_global_lock (cDBusAutolaunchMutex);

  if (_dbus_daemon_already_runs (address, &shm_name, scope))
    {
      _dbus_verbose ("found running dbus daemon for scope '%s' at %s\n",
                     scope ? scope : "", _dbus_string_get_const_data (&shm_name) );
      retval = TRUE;
      goto out;
    }

  if (!SearchPathA (NULL, daemon_name, NULL, sizeof(dbus_exe_path), dbus_exe_path, &lpFile))
    {
      // Look in directory containing dbus shared library
      HMODULE hmod;
      char dbus_module_path[MAX_PATH];
      DWORD rc;

      _dbus_verbose ("did not found dbus daemon executable on default search path, "
                     "trying path where dbus shared library is located");

      hmod = _dbus_win_get_dll_hmodule ();
      rc = GetModuleFileNameA (hmod, dbus_module_path, sizeof(dbus_module_path));
      if (rc <= 0)
        {
          dbus_set_error_const (error, DBUS_ERROR_FAILED, "could not retrieve dbus shared library file name");
          retval = FALSE;
          goto out;
        }
      else
        {
          char *ext_idx = strrchr (dbus_module_path, '\\');
          if (ext_idx)
            *ext_idx = '\0';
          if (!SearchPathA (dbus_module_path, daemon_name, NULL, sizeof(dbus_exe_path), dbus_exe_path, &lpFile))
            {
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Could not find dbus-daemon executable. "
                              "Please add the path to %s to your PATH "
                              "environment variable or start the daemon manually",
                              daemon_name);
              retval = FALSE;
              goto out;
            }
          _dbus_verbose ("found dbus daemon executable at %s", dbus_module_path);
        }
    }


  // Create process
  ZeroMemory (&si, sizeof(si));
  si.cb = sizeof (si);
  ZeroMemory (&pi, sizeof(pi));

  if (!_dbus_string_init (&dbus_args))
    {
      dbus_set_error_const (error, DBUS_ERROR_NO_MEMORY, "Failed to initialize argument buffer");
      retval = FALSE;
      goto out;
    }

  if (!_dbus_string_append_printf (&dbus_args, "\"%s\" --session", dbus_exe_path))
    {
      dbus_set_error_const (error, DBUS_ERROR_NO_MEMORY, "Failed to append string to argument buffer");
      retval = FALSE;
      goto out;
    }

//  argv[i] = "--config-file=bus\\session.conf";
  if(CreateProcessA (dbus_exe_path, _dbus_string_get_data (&dbus_args), NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, NULL, &si, &pi))
    {
      CloseHandle (pi.hThread);
      CloseHandle (pi.hProcess);
      retval = _dbus_get_autolaunch_shm (address, &shm_name);
      if (retval == FALSE)
        dbus_set_error_const (error, DBUS_ERROR_FAILED, "Failed to get autolaunch address from launched dbus-daemon");
    }
  else
    {
      dbus_set_error_const (error, DBUS_ERROR_FAILED, "Failed to launch dbus-daemon");
      retval = FALSE;
    }

out:
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, retval);
  if (mutex != NULL)
    _dbus_global_unlock (mutex);
  _dbus_string_free (&shm_name);
  _dbus_string_free (&dbus_args);

  return retval;
 }


/** Makes the file readable by every user in the system.
 *
 * @param filename the filename
 * @param error error location
 * @returns #TRUE if the file's permissions could be changed.
 */
dbus_bool_t
_dbus_make_file_world_readable(const DBusString *filename,
                               DBusError *error)
{
  // TODO
  return TRUE;
}

/**
 * Atomically increments an integer
 *
 * @param atomic pointer to the integer to increment
 * @returns the value before incrementing
 *
 */
dbus_int32_t
_dbus_atomic_inc (DBusAtomic *atomic)
{
  // +/- 1 is needed here!
  // no volatile argument with mingw
  return InterlockedIncrement (&atomic->value) - 1;
}

/**
 * Atomically decrement an integer
 *
 * @param atomic pointer to the integer to decrement
 * @returns the value before decrementing
 *
 */
dbus_int32_t
_dbus_atomic_dec (DBusAtomic *atomic)
{
  // +/- 1 is needed here!
  // no volatile argument with mingw
  return InterlockedDecrement (&atomic->value) + 1;
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
  /* In this situation, GLib issues a MemoryBarrier() and then returns
   * atomic->value. However, mingw from mingw.org (not to be confused with
   * mingw-w64 from mingw-w64.sf.net) does not have MemoryBarrier in its
   * headers, so we have to get a memory barrier some other way.
   *
   * InterlockedIncrement is older, and is documented on MSDN to be a full
   * memory barrier, so let's use that.
   */
  long dummy = 0;

  InterlockedExchange (&dummy, 1);

  return atomic->value;
}

/**
 * Atomically set the value of an integer to 0.
 *
 * @param atomic pointer to the integer to set
 */
void
_dbus_atomic_set_zero (DBusAtomic *atomic)
{
  InterlockedExchange (&atomic->value, 0);
}

/**
 * Atomically set the value of an integer to something nonzero.
 *
 * @param atomic pointer to the integer to set
 */
void
_dbus_atomic_set_nonzero (DBusAtomic *atomic)
{
  InterlockedExchange (&atomic->value, 1);
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
  return e == WSAEWOULDBLOCK;
}

/**
 * Fill str with the absolute path of the D-Bus installation, or truncate str
 * to zero length if we cannot determine it.
 *
 * @param str buffer for installation path
 * @returns #FALSE on OOM, #TRUE if not OOM
 */
dbus_bool_t
_dbus_get_install_root (DBusString *str)
{
    /* this is just an initial guess */
    DWORD pathLength = MAX_PATH;
    unsigned char *lastSlash;
    unsigned char *prefix;

    do
      {
        /* allocate enough space for our best guess at the length */
        if (!_dbus_string_set_length (str, pathLength))
          {
            _dbus_string_set_length (str, 0);
            return FALSE;
          }

        SetLastError (0);
        pathLength = GetModuleFileNameA (_dbus_win_get_dll_hmodule (),
            _dbus_string_get_data (str), _dbus_string_get_length (str));

        if (pathLength == 0 || GetLastError () != 0)
          {
            /* failed, but not OOM */
            _dbus_string_set_length (str, 0);
            return TRUE;
          }

        /* if the return is strictly less than the buffer size, it has
         * not been truncated, so we can continue */
        if (pathLength < (DWORD) _dbus_string_get_length (str))
          {
            /* reduce the length to match what Windows filled in */
            if (!_dbus_string_set_length (str, pathLength))
              {
                _dbus_string_set_length (str, 0);
                return FALSE;
              }

            break;
          }

        /* else it may have been truncated; try with a larger buffer */
        pathLength *= 2;
      }
    while (TRUE);

    /* the rest of this function works by direct byte manipulation of the
     * underlying buffer */
    prefix = _dbus_string_get_udata (str);

    lastSlash = _mbsrchr (prefix, '\\');
    if (lastSlash == NULL) {
        /* failed, but not OOM */
        _dbus_string_set_length (str, 0);
        return TRUE;
    }
    //cut off binary name
    lastSlash[1] = 0;

    //cut possible "\\bin"
    //this fails if we are in a double-byte system codepage and the
    //folder's name happens to end with the *bytes*
    //"\\bin"... (I.e. the second byte of some Han character and then
    //the Latin "bin", but that is not likely I think...
    if (lastSlash - prefix >= 4 && _mbsnicmp (lastSlash - 4, (const unsigned char *)"\\bin", 4) == 0)
        lastSlash[-3] = 0;
    else if (lastSlash - prefix >= 10 && _mbsnicmp (lastSlash - 10, (const unsigned char *)"\\bin\\debug", 10) == 0)
        lastSlash[-9] = 0;
    else if (lastSlash - prefix >= 12 && _mbsnicmp (lastSlash - 12, (const unsigned char *)"\\bin\\release", 12) == 0)
        lastSlash[-11] = 0;

    /* fix up the length to match the byte-manipulation */
    _dbus_string_set_length (str, strlen ((char *) prefix));

    return TRUE;
}

/* See comment in dbus-sysdeps-unix.c */
dbus_bool_t
_dbus_lookup_session_address (dbus_bool_t *supported,
                              DBusString  *address,
                              DBusError   *error)
{
  /* Probably fill this in with something based on COM? */
  *supported = FALSE;
  return TRUE;
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
  const char *homepath;
  const char *homedrive;

  _dbus_assert (credentials != NULL);
  _dbus_assert (!_dbus_credentials_are_anonymous (credentials));
  
  if (!_dbus_string_init (&homedir))
    return FALSE;

  homedrive = _dbus_getenv("HOMEDRIVE");
  if (homedrive != NULL && *homedrive != '\0')
    {
      _dbus_string_append(&homedir,homedrive);
    }

  homepath = _dbus_getenv("HOMEPATH");
  if (homepath != NULL && *homepath != '\0')
    {
      _dbus_string_append(&homedir,homepath);
    }
  
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
            _dbus_warn ("Using your real home directory for testing, set DBUS_TEST_HOMEDIR to avoid");
            already_warned = TRUE;
          }
      }
  }
#endif

#ifdef DBUS_WINCE
  /* It's not possible to create a .something directory in Windows CE
     using the file explorer.  */
#define KEYRING_DIR "dbus-keyrings"
#else
#define KEYRING_DIR ".dbus-keyrings"
#endif

  _dbus_string_init_const (&dotdir, KEYRING_DIR);
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

/** Checks if a file exists
*
* @param file full path to the file
* @returns #TRUE if file exists
*/
dbus_bool_t 
_dbus_file_exists (const char *file)
{
  DWORD attributes = GetFileAttributesA (file);

  if (attributes != INVALID_FILE_ATTRIBUTES && GetLastError() != ERROR_PATH_NOT_FOUND)
    return TRUE;
  else
    return FALSE;  
}

/**
 * A wrapper around strerror() because some platforms
 * may be lame and not have strerror().
 *
 * @param error_number errno.
 * @returns error description.
 */
const char*
_dbus_strerror (int error_number)
{
#ifdef DBUS_WINCE
  // TODO
  return "unknown";
#else
  const char *msg;

  switch (error_number)
    {
    case WSAEINTR:
      return "Interrupted function call";
    case WSAEACCES:
      return "Permission denied";
    case WSAEFAULT:
      return "Bad address";
    case WSAEINVAL:
      return "Invalid argument";
    case WSAEMFILE:
      return "Too many open files";
    case WSAEWOULDBLOCK:
      return "Resource temporarily unavailable";
    case WSAEINPROGRESS:
      return "Operation now in progress";
    case WSAEALREADY:
      return "Operation already in progress";
    case WSAENOTSOCK:
      return "Socket operation on nonsocket";
    case WSAEDESTADDRREQ:
      return "Destination address required";
    case WSAEMSGSIZE:
      return "Message too long";
    case WSAEPROTOTYPE:
      return "Protocol wrong type for socket";
    case WSAENOPROTOOPT:
      return "Bad protocol option";
    case WSAEPROTONOSUPPORT:
      return "Protocol not supported";
    case WSAESOCKTNOSUPPORT:
      return "Socket type not supported";
    case WSAEOPNOTSUPP:
      return "Operation not supported";
    case WSAEPFNOSUPPORT:
      return "Protocol family not supported";
    case WSAEAFNOSUPPORT:
      return "Address family not supported by protocol family";
    case WSAEADDRINUSE:
      return "Address already in use";
    case WSAEADDRNOTAVAIL:
      return "Cannot assign requested address";
    case WSAENETDOWN:
      return "Network is down";
    case WSAENETUNREACH:
      return "Network is unreachable";
    case WSAENETRESET:
      return "Network dropped connection on reset";
    case WSAECONNABORTED:
      return "Software caused connection abort";
    case WSAECONNRESET:
      return "Connection reset by peer";
    case WSAENOBUFS:
      return "No buffer space available";
    case WSAEISCONN:
      return "Socket is already connected";
    case WSAENOTCONN:
      return "Socket is not connected";
    case WSAESHUTDOWN:
      return "Cannot send after socket shutdown";
    case WSAETIMEDOUT:
      return "Connection timed out";
    case WSAECONNREFUSED:
      return "Connection refused";
    case WSAEHOSTDOWN:
      return "Host is down";
    case WSAEHOSTUNREACH:
      return "No route to host";
    case WSAEPROCLIM:
      return "Too many processes";
    case WSAEDISCON:
      return "Graceful shutdown in progress";
    case WSATYPE_NOT_FOUND:
      return "Class type not found";
    case WSAHOST_NOT_FOUND:
      return "Host not found";
    case WSATRY_AGAIN:
      return "Nonauthoritative host not found";
    case WSANO_RECOVERY:
      return "This is a nonrecoverable error";
    case WSANO_DATA:
      return "Valid name, no data record of requested type";
    case WSA_INVALID_HANDLE:
      return "Specified event object handle is invalid";
    case WSA_INVALID_PARAMETER:
      return "One or more parameters are invalid";
    case WSA_IO_INCOMPLETE:
      return "Overlapped I/O event object not in signaled state";
    case WSA_IO_PENDING:
      return "Overlapped operations will complete later";
    case WSA_NOT_ENOUGH_MEMORY:
      return "Insufficient memory available";
    case WSA_OPERATION_ABORTED:
      return "Overlapped operation aborted";
#ifdef WSAINVALIDPROCTABLE

    case WSAINVALIDPROCTABLE:
      return "Invalid procedure table from service provider";
#endif
#ifdef WSAINVALIDPROVIDER

    case WSAINVALIDPROVIDER:
      return "Invalid service provider version number";
#endif
#ifdef WSAPROVIDERFAILEDINIT

    case WSAPROVIDERFAILEDINIT:
      return "Unable to initialize a service provider";
#endif

    case WSASYSCALLFAILURE:
      return "System call failure";

    default:
      msg = strerror (error_number);

      if (msg == NULL)
        msg = "unknown";

      return msg;
    }
#endif //DBUS_WINCE
}

/**
 * Assigns an error name and message corresponding to a Win32 error
 * code to a DBusError. Does nothing if error is #NULL.
 *
 * @param error the error.
 * @param code the Win32 error code
 */
void
_dbus_win_set_error_from_win_error (DBusError *error,
                                    int        code)
{
  char *msg;

  /* As we want the English message, use the A API */
  FormatMessageA (FORMAT_MESSAGE_ALLOCATE_BUFFER |
                  FORMAT_MESSAGE_IGNORE_INSERTS |
                  FORMAT_MESSAGE_FROM_SYSTEM,
                  NULL, code, MAKELANGID (LANG_ENGLISH, SUBLANG_ENGLISH_US),
                  (LPSTR) &msg, 0, NULL);
  if (msg)
    {
      dbus_set_error (error, "win32.error", "%s", msg);
      LocalFree (msg);
    }
  else
    dbus_set_error (error, "win32.error", "Unknown error code %d or FormatMessage failed", code);
}

void
_dbus_win_warn_win_error (const char *message,
                          unsigned long code)
{
  DBusError error;

  dbus_error_init (&error);
  _dbus_win_set_error_from_win_error (&error, code);
  _dbus_warn ("%s: %s", message, error.message);
  dbus_error_free (&error);
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

  if (RemoveDirectoryA (filename_c) == 0)
    {
      char *emsg = _dbus_win_error_string (GetLastError ());
      dbus_set_error (error, _dbus_win_error_from_last_error (),
                      "Failed to remove directory %s: %s",
                      filename_c, emsg);
      _dbus_win_free_error_string (emsg);
      return FALSE;
    }

  return TRUE;
}

/**
 * Checks whether the filename is an absolute path
 *
 * @param filename the filename
 * @returns #TRUE if an absolute path
 */
dbus_bool_t
_dbus_path_is_absolute (const DBusString *filename)
{
  if (_dbus_string_get_length (filename) > 0)
    return _dbus_string_get_byte (filename, 1) == ':'
           || _dbus_string_get_byte (filename, 0) == '\\'
           || _dbus_string_get_byte (filename, 0) == '/';
  else
    return FALSE;
}

dbus_bool_t
_dbus_check_setuid (void)
{
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
  _dbus_win_set_errno (saved_errno);
}

static const char *log_tag = "dbus";
static DBusLogFlags log_flags = DBUS_LOG_FLAGS_STDERR;

/**
 * Initialize the system log.
 *
 * The "tag" is not copied, and must remain valid for the entire lifetime of
 * the process or until _dbus_init_system_log() is called again. In practice
 * it will normally be a constant.
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

  log_tag = tag;
  log_flags = flags;
}

/**
 * Log a message to the system log file (e.g. syslog on Unix).
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
  const char *s = "";
  va_list tmp;

  switch(severity)
   {
     case DBUS_SYSTEM_LOG_INFO: s = "info"; break;
     case DBUS_SYSTEM_LOG_WARNING: s = "warning"; break;
     case DBUS_SYSTEM_LOG_SECURITY: s = "security"; break;
     case DBUS_SYSTEM_LOG_ERROR: s = "error"; break;
     default: _dbus_assert_not_reached ("invalid log severity");
   }

  if (log_flags & DBUS_LOG_FLAGS_SYSTEM_LOG)
    {
      DBusString out = _DBUS_STRING_INIT_INVALID;
      const char *message = NULL;
      DBUS_VA_COPY (tmp, args);

      if (!_dbus_string_init (&out))
        goto out;
      if (!_dbus_string_append_printf (&out, "%s: ", s))
        goto out;
      if (!_dbus_string_append_printf_valist (&out, msg, tmp))
        goto out;
      message = _dbus_string_get_const_data (&out);
out:
      if (message != NULL)
        {
          OutputDebugStringA (message);
        }
      else
        {
          OutputDebugStringA ("Out of memory while formatting message: '''");
          OutputDebugStringA (msg);
          OutputDebugStringA ("'''");
        }

      va_end (tmp);
      _dbus_string_free (&out);
    }

  if (log_flags & DBUS_LOG_FLAGS_STDERR)
    {
      DBUS_VA_COPY (tmp, args);
      fprintf (stderr, "%s[%lu]: %s: ", log_tag, _dbus_pid_for_log (), s);
      vfprintf (stderr, msg, tmp);
      fprintf (stderr, "\n");
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
  return WSAGetLastError ();
}

void
_dbus_win_set_error_from_last_error (DBusError *error,
                                     const char *format,
                                     ...)
{
  const char *name;
  char *message = NULL;

  if (error == NULL)
    return;

  /* make sure to do this first, in case subsequent library calls overwrite GetLastError() */
  name = _dbus_win_error_from_last_error ();
  message = _dbus_win_error_string (GetLastError ());

  if (format != NULL)
    {
      DBusString str;
      va_list args;
      dbus_bool_t retval;

      if (!_dbus_string_init (&str))
        {
          _DBUS_SET_OOM (error);
          goto out;
        }

      va_start (args, format);
      retval = _dbus_string_append_printf_valist (&str, format, args);
      va_end (args);
      if (!retval)
        {
          _DBUS_SET_OOM (error);
          _dbus_string_free (&str);
          goto out;
        }

      dbus_set_error (error, name, "%s: %s", _dbus_string_get_const_data (&str), message);
      _dbus_string_free (&str);
    }
  else
    {
      dbus_set_error (error, name, "%s", message);
    }

out:
  if (message != NULL)
    _dbus_win_free_error_string (message);

  _DBUS_ASSERT_ERROR_IS_SET (error);
}

/**
 * Creates a Windows event object and returns the corresponding handle
 *
 * The returned object is unnamed, is a manual-reset event object,
 * is initially in the non-signalled state, and is inheritable by child
 * processes.
 *
 * @param error the error to set
 * @return handle for the created event
 * @return #NULL if an error has occurred, the reason is returned in \p error
 */
HANDLE
_dbus_win_event_create_inheritable (DBusError *error)
{
  HANDLE handle;

  handle = CreateEvent (NULL, TRUE, FALSE, NULL);
  if (handle == NULL)
    {
      _dbus_win_set_error_from_last_error (error, "Could not create event");
      return NULL;
    }
  else if (GetLastError () == ERROR_ALREADY_EXISTS)
   {
      _dbus_win_set_error_from_last_error (error, "Event already exists");
      return NULL;
   }

  if (!SetHandleInformation (handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT))
    {
      _dbus_win_set_error_from_last_error (error, "Could not set inheritance for event %p", handle);
      CloseHandle (handle);
      return NULL;
    }
  return handle;
}

/**
 * Set a Windows event to the signalled state
 *
 * @param handle the handle for the event to be set
 * @return TRUE the event was set successfully
 * @return FALSE an error has occurred, the reason is returned in \p error
 */
dbus_bool_t
_dbus_win_event_set (HANDLE handle, DBusError *error)
{
  _dbus_assert (handle != NULL);

  if (!SetEvent (handle))
    {
      _dbus_win_set_error_from_last_error (error, "Could not trigger event (handle %p)", handle);
      return FALSE;
    }
  return TRUE;
}

/**
 * Wait for a Windows event to enter the signalled state
 *
 * @param handle the handle for the event to wait for
 * @param timeout the waiting time in milliseconds, or INFINITE to wait forever,
 *                or 0 to check immediately and not wait (polling)
 * @param error the error to set
 * @return TRUE the event was set successfully
 * @return FALSE an error has occurred, the reason is returned in \p error
 */
dbus_bool_t
_dbus_win_event_wait (HANDLE handle, int timeout, DBusError *error)
{
  DWORD status;

  _dbus_assert (handle != NULL);

  status = WaitForSingleObject (handle, timeout);
  switch (status)
    {
      case WAIT_OBJECT_0:
        return TRUE;

      case WAIT_FAILED:
        {
          _dbus_win_set_error_from_last_error (error, "Unable to wait for event (handle %p)", handle);
          return FALSE;
        }

      case WAIT_TIMEOUT:
        /* GetLastError() is not set */
        dbus_set_error (error, DBUS_ERROR_TIMEOUT, "Timed out waiting for event (handle %p)", handle);
        return FALSE;

      default:
        /* GetLastError() is probably not set? */
        dbus_set_error (error, DBUS_ERROR_FAILED, "Unknown result '%lu' while waiting for event (handle %p)", status, handle);
        return FALSE;
    }
}

/**
 * Delete a Windows event
 *
 * @param handle handle for the event to delete
 * @param error the error to set (optional)
 * @return TRUE the event has been deleted successfully or the handle is one of the special sentinel values #NULL or #INVALID_HANDLE_VALUE
 * @return FALSE an error has occurred, the reason is returned in \p error if specified
 */
dbus_bool_t
_dbus_win_event_free (HANDLE handle, DBusError *error)
{
  if (handle == NULL || handle == INVALID_HANDLE_VALUE)
    return TRUE;

  if (CloseHandle (handle))
    return TRUE;

  /* the handle may already be closed */
  if (GetLastError () == ERROR_INVALID_HANDLE)
    return TRUE;

  _dbus_win_set_error_from_last_error (error, "Could not close event (handle %p)", handle);
  return FALSE;
}

/** @} end of sysdeps-win */
/* tests in dbus-sysdeps-util.c */
