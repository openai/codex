/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-transport-unix.c UNIX socket subclasses of DBusTransport
 *
 * Copyright (C) 2002, 2003, 2004  Red Hat Inc.
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

#include <stdio.h>

#include "dbus-internals.h"
#include "dbus-connection-internal.h"
#include "dbus-transport-unix.h"
#include "dbus-transport-socket.h"
#include "dbus-transport-protected.h"
#include "dbus-watch.h"
#include "dbus-sysdeps-unix.h"
#include "dbus-test.h"

/**
 * @defgroup DBusTransportUnix DBusTransport implementations for UNIX
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusTransport on UNIX
 *
 * @{
 */

/**
 * Creates a new transport for the given Unix domain socket
 * path. This creates a client-side of a transport.
 *
 * @todo once we add a way to escape paths in a dbus
 * address, this function needs to do escaping.
 *
 * @param path the path to the domain socket.
 * @param abstract #TRUE to use abstract socket namespace
 * @param error address where an error can be returned.
 * @returns a new transport, or #NULL on failure.
 */
DBusTransport*
_dbus_transport_new_for_domain_socket (const char     *path,
                                       dbus_bool_t     abstract,
                                       DBusError      *error)
{
  DBusSocket fd = DBUS_SOCKET_INIT;
  DBusTransport *transport;
  DBusString address;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if ((abstract &&
       !_dbus_string_append (&address, "unix:abstract=")) ||
      (!abstract &&
       !_dbus_string_append (&address, "unix:path=")) ||
      !_dbus_string_append (&address, path))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed_0;
    }
  
  fd.fd = _dbus_connect_unix_socket (path, abstract, error);
  if (fd.fd < 0)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed_0;
    }

  _dbus_verbose ("Successfully connected to unix socket %s\n",
                 path);

  transport = _dbus_transport_new_for_socket (fd, NULL, &address);
  if (transport == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed_1;
    }
  
  _dbus_string_free (&address);
  
  return transport;

 failed_1:
  _dbus_close_socket (fd, NULL);
 failed_0:
  _dbus_string_free (&address);
  return NULL;
}

/**
 * Creates a new transport for the given binary and arguments. This
 * creates a client-side of a transport. The process will be forked
 * off and executed with stdin/stdout connected to a local AF_UNIX
 * socket.
 *
 * @param path the path to the domain socket.
 * @param argv Parameters list
 * @param error address where an error can be returned.
 * @returns a new transport, or #NULL on failure.
 */
static DBusTransport*
_dbus_transport_new_for_exec (const char     *path,
                              char *const     argv[],
                              DBusError      *error)
{
  DBusSocket fd = DBUS_SOCKET_INIT;
  DBusTransport *transport;
  DBusString address;
  unsigned i;
  char *escaped;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  escaped = dbus_address_escape_value (path);
  if (!escaped)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  if (!_dbus_string_append (&address, "unixexec:path=") ||
      !_dbus_string_append (&address, escaped))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      dbus_free (escaped);
      goto failed;
    }

  dbus_free (escaped);

  if (argv)
    {
      for (i = 0; argv[i]; i++)
        {
          dbus_bool_t success;

          escaped = dbus_address_escape_value (argv[i]);
          if (!escaped)
            {
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              goto failed;
            }

          success = _dbus_string_append_printf (&address, ",argv%u=%s", i, escaped);
          dbus_free (escaped);

          if (!success)
            {
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              goto failed;
            }
        }
    }

  fd.fd = _dbus_connect_exec (path, argv, error);
  if (fd.fd < 0)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed;
    }

  _dbus_verbose ("Successfully connected to process %s\n",
                 path);

  transport = _dbus_transport_new_for_socket (fd, NULL, &address);
  if (transport == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  _dbus_string_free (&address);

  return transport;

 failed:
  if (fd.fd >= 0)
    _dbus_close_socket (fd, NULL);

  _dbus_string_free (&address);
  return NULL;
}

/**
 * Opens platform specific transport types.
 * 
 * @param entry the address entry to try opening
 * @param transport_p return location for the opened transport
 * @param error error to be set
 * @returns result of the attempt
 */
DBusTransportOpenResult
_dbus_transport_open_platform_specific (DBusAddressEntry  *entry,
                                        DBusTransport    **transport_p,
                                        DBusError         *error)
{
  const char *method;
  
  method = dbus_address_entry_get_method (entry);
  _dbus_assert (method != NULL);

  if (strcmp (method, "unix") == 0)
    {
      const char *path = dbus_address_entry_get_value (entry, "path");
      const char *tmpdir = dbus_address_entry_get_value (entry, "tmpdir");
      const char *abstract = dbus_address_entry_get_value (entry, "abstract");
          
      if (tmpdir != NULL)
        {
          _dbus_set_bad_address (error, NULL, NULL,
                                 "cannot use the \"tmpdir\" option for an address to connect to, only in an address to listen on");
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }
          
      if (path == NULL && abstract == NULL)
        {
          _dbus_set_bad_address (error, "unix",
                                 "path or abstract",
                                 NULL);
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }

      if (path != NULL && abstract != NULL)
        {
          _dbus_set_bad_address (error, NULL, NULL,
                                 "can't specify both \"path\" and \"abstract\" options in an address");
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }

      if (path)
        *transport_p = _dbus_transport_new_for_domain_socket (path, FALSE,
                                                           error);
      else
        *transport_p = _dbus_transport_new_for_domain_socket (abstract, TRUE,
                                                           error);
      if (*transport_p == NULL)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_CLEAR (error);
          return DBUS_TRANSPORT_OPEN_OK;
        }
    }
  else if (strcmp (method, "unixexec") == 0)
    {
      const char *path;
      unsigned i;
      char **argv;

      path = dbus_address_entry_get_value (entry, "path");
      if (path == NULL)
        {
          _dbus_set_bad_address (error, NULL, NULL,
                                 "No process path specified");
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }

      /* First count argv arguments */
      for (i = 1; ; i++)
        {
          char t[4+20+1]; /* "argv" plus space for a formatted base 10 64bit integer, plus NUL */

          snprintf (t, sizeof(t), "argv%u", i);

          if (!dbus_address_entry_get_value (entry, t))
            break;
        }

      /* Allocate string array */
      argv = dbus_new0 (char*, i+1);
      if (!argv)
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }

      /* Fill in string array */
      for (i = 0; ; i++)
        {
          char t[4+20+1];
          const char *p;

          snprintf (t, sizeof(t), "argv%u", i);

          p = dbus_address_entry_get_value (entry, t);
          if (!p)
            {
              if (i == 0)
                /* If argv0 isn't specified, fill in the path instead */
                p = path;
              else
                break;
            }

          argv[i] = _dbus_strdup (p);
          if (!argv[i])
            {
              dbus_free_string_array (argv);
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
            }
        }

      *transport_p = _dbus_transport_new_for_exec (path, argv, error);
      dbus_free_string_array (argv);

      if (*transport_p == NULL)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_CLEAR (error);
          return DBUS_TRANSPORT_OPEN_OK;
        }
    }
#ifdef DBUS_ENABLE_LAUNCHD
  else if (strcmp (method, "launchd") == 0)
    {
      DBusError tmp_error = DBUS_ERROR_INIT;
      const char *launchd_env_var = dbus_address_entry_get_value (entry, "env");
      const char *launchd_socket;
      DBusString socket_path;
      dbus_bool_t valid_socket;

      if (!_dbus_string_init (&socket_path))
        {
          _DBUS_SET_OOM (error);
          return FALSE;
        }

      if (launchd_env_var == NULL)
        {
          _dbus_set_bad_address (error, "launchd", "env", NULL);
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }

      valid_socket = _dbus_lookup_launchd_socket (&socket_path, launchd_env_var, error);

      if (dbus_error_is_set(error))
        {
          _dbus_string_free(&socket_path);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }

      if (!valid_socket)
        {
          dbus_set_error(&tmp_error, DBUS_ERROR_BAD_ADDRESS,
                         "launchd's env var %s does not exist", launchd_env_var);
          dbus_error_free(error);
          dbus_move_error(&tmp_error, error);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }

      launchd_socket = _dbus_string_get_const_data(&socket_path);
      *transport_p = _dbus_transport_new_for_domain_socket (launchd_socket, FALSE, error);

      if (*transport_p == NULL)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          return DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_CLEAR (error);
          return DBUS_TRANSPORT_OPEN_OK;
        }
    }
#endif
  else
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR (error);
      return DBUS_TRANSPORT_OPEN_NOT_HANDLED;
    }
}

/** @} */
