/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-unix.c Server implementation for Unix network protocols.
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
#include "dbus-internals.h"
#include "dbus-server-unix.h"
#include "dbus-server-socket.h"
#include "dbus-server-launchd.h"
#include "dbus-transport-unix.h"
#include "dbus-connection-internal.h"
#include "dbus-sysdeps-unix.h"
#include "dbus-string.h"

/**
 * @defgroup DBusServerUnix DBusServer implementations for UNIX
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusServer on UNIX
 *
 * @{
 */

/**
 * Tries to interpret the address entry in a platform-specific
 * way, creating a platform-specific server type if appropriate.
 * Sets error if the result is not OK.
 *
 * @param entry an address entry
 * @param server_p location to store a new DBusServer, or #NULL on failure.
 * @param error location to store rationale for failure on bad address
 * @returns the outcome
 *
 */
DBusServerListenResult
_dbus_server_listen_platform_specific (DBusAddressEntry *entry,
                                       DBusServer      **server_p,
                                       DBusError        *error)
{
  const char *method;

  *server_p = NULL;

  method = dbus_address_entry_get_method (entry);

  if (strcmp (method, "unix") == 0)
    {
      const char *path = dbus_address_entry_get_value (entry, "path");
      const char *dir = dbus_address_entry_get_value (entry, "dir");
      const char *tmpdir = dbus_address_entry_get_value (entry, "tmpdir");
      const char *abstract = dbus_address_entry_get_value (entry, "abstract");
      const char *runtime = dbus_address_entry_get_value (entry, "runtime");
      int mutually_exclusive_modes = 0;

      mutually_exclusive_modes = (path != NULL) + (tmpdir != NULL) +
        (abstract != NULL) + (runtime != NULL) + (dir != NULL);

      if (mutually_exclusive_modes < 1)
        {
          _dbus_set_bad_address(error, "unix",
                                "path or tmpdir or abstract or runtime or dir",
                                NULL);
          return DBUS_SERVER_LISTEN_BAD_ADDRESS;
        }

      if (mutually_exclusive_modes > 1)
        {
          _dbus_set_bad_address(error, NULL, NULL,
                                "cannot specify two of \"path\", \"tmpdir\", \"abstract\", \"runtime\" and \"dir\" at the same time");
          return DBUS_SERVER_LISTEN_BAD_ADDRESS;
        }

      if (runtime != NULL)
        {
          DBusString full_path;
          DBusString filename;
          const char *runtimedir;

          if (strcmp (runtime, "yes") != 0)
            {
              _dbus_set_bad_address(error, NULL, NULL,
                  "if given, the only value allowed for \"runtime\" is \"yes\"");
              return DBUS_SERVER_LISTEN_BAD_ADDRESS;
            }

          runtimedir = _dbus_getenv ("XDG_RUNTIME_DIR");

          if (runtimedir == NULL)
            {
              dbus_set_error (error,
                  DBUS_ERROR_NOT_SUPPORTED, "\"XDG_RUNTIME_DIR\" is not set");
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          _dbus_string_init_const (&filename, "bus");

          if (!_dbus_string_init (&full_path))
            {
              _DBUS_SET_OOM (error);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          if (!_dbus_string_append (&full_path, runtimedir) ||
              !_dbus_concat_dir_and_file (&full_path, &filename))
            {
              _dbus_string_free (&full_path);
              _DBUS_SET_OOM (error);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          /* We can safely use filesystem sockets in the runtime directory,
           * and they are preferred because they can be bind-mounted between
           * Linux containers. */
          *server_p = _dbus_server_new_for_domain_socket (
              _dbus_string_get_const_data (&full_path),
              FALSE, error);

          _dbus_string_free (&full_path);
        }
      else if (tmpdir != NULL || dir != NULL)
        {
          DBusString full_path;
          DBusString filename;

          /* tmpdir is now equivalent to dir. Previously it would try to
           * use an abstract socket. */
          if (tmpdir != NULL)
            dir = tmpdir;

          if (!_dbus_string_init (&full_path))
            {
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          if (!_dbus_string_init (&filename))
            {
              _dbus_string_free (&full_path);
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          if (!_dbus_string_append (&filename, "dbus-"))
            {
              _dbus_string_free (&full_path);
              _dbus_string_free (&filename);
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          if (!_dbus_generate_random_ascii (&filename, 10, error))
            {
              _dbus_string_free (&full_path);
              _dbus_string_free (&filename);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          if (!_dbus_string_append (&full_path, dir) ||
              !_dbus_concat_dir_and_file (&full_path, &filename))
            {
              _dbus_string_free (&full_path);
              _dbus_string_free (&filename);
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
              return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
            }

          *server_p =
            _dbus_server_new_for_domain_socket (_dbus_string_get_const_data (&full_path),
                                                FALSE,
                                                error);

          _dbus_string_free (&full_path);
          _dbus_string_free (&filename);
        }
      else
        {
          if (path)
            *server_p = _dbus_server_new_for_domain_socket (path, FALSE, error);
          else
            *server_p = _dbus_server_new_for_domain_socket (abstract, TRUE, error);
        }

      if (*server_p != NULL)
        {
          _DBUS_ASSERT_ERROR_IS_CLEAR(error);
          return DBUS_SERVER_LISTEN_OK;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_SET(error);
          return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
        }
    }
  else if (strcmp (method, "systemd") == 0)
    {
      int i, n;
      DBusSocket *fds;
      DBusString address;

      n = _dbus_listen_systemd_sockets (&fds, error);
      if (n < 0)
        {
          _DBUS_ASSERT_ERROR_IS_SET (error);
          return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
        }

      if (!_dbus_string_init (&address))
          goto systemd_oom;

      for (i = 0; i < n; i++)
        {
          if (i > 0)
            {
              if (!_dbus_string_append (&address, ";"))
                goto systemd_oom;
            }
          if (!_dbus_append_address_from_socket (fds[i], &address, error))
            goto systemd_err;
        }

      *server_p = _dbus_server_new_for_socket (fds, n, &address, NULL, error);
      if (*server_p == NULL)
        goto systemd_err;

      dbus_free (fds);
      _dbus_string_free (&address);

      return DBUS_SERVER_LISTEN_OK;

  systemd_oom:
      _DBUS_SET_OOM (error);
  systemd_err:
      for (i = 0; i < n; i++)
        {
          _dbus_close_socket (fds[i], NULL);
        }
      dbus_free (fds);
      _dbus_string_free (&address);

      return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
    }
#ifdef DBUS_ENABLE_LAUNCHD
  else if (strcmp (method, "launchd") == 0)
    {
      const char *launchd_env_var = dbus_address_entry_get_value (entry, "env");
      if (launchd_env_var == NULL)
        {
          _dbus_set_bad_address (error, "launchd", "env", NULL);
          return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
        }
      *server_p = _dbus_server_new_for_launchd (launchd_env_var, error);

      if (*server_p != NULL)
        {
          _DBUS_ASSERT_ERROR_IS_CLEAR(error);
          return DBUS_SERVER_LISTEN_OK;
        }
      else
        {
          _DBUS_ASSERT_ERROR_IS_SET(error);
          return DBUS_SERVER_LISTEN_DID_NOT_CONNECT;
        }
    }
#endif
  else
    {
      /* If we don't handle the method, we return NULL with the
       * error unset
       */
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);
      return DBUS_SERVER_LISTEN_NOT_HANDLED;
    }
}

/**
 * Creates a new server listening on the given Unix domain socket.
 *
 * @param path the path for the domain socket.
 * @param abstract #TRUE to use abstract socket namespace
 * @param error location to store reason for failure.
 * @returns the new server, or #NULL on failure.
 */
DBusServer*
_dbus_server_new_for_domain_socket (const char     *path,
                                    dbus_bool_t     abstract,
                                    DBusError      *error)
{
  DBusServer *server;
  DBusSocket listen_fd;
  DBusString address;
  char *path_copy;
  DBusString path_str;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  _dbus_string_init_const (&path_str, path);
  if ((abstract &&
       !_dbus_string_append (&address, "unix:abstract=")) ||
      (!abstract &&
       !_dbus_string_append (&address, "unix:path=")) ||
      !_dbus_address_append_escaped (&address, &path_str))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed_0;
    }

  if (abstract)
    {
      path_copy = NULL;
    }
  else
    {
      path_copy = _dbus_strdup (path);
      if (path_copy == NULL)
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          goto failed_0;
        }
    }

  listen_fd.fd = _dbus_listen_unix_socket (path, abstract, error);

  if (listen_fd.fd < 0)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto failed_1;
    }

  server = _dbus_server_new_for_socket (&listen_fd, 1, &address, 0, error);
  if (server == NULL)
    {
      goto failed_2;
    }

  if (path_copy != NULL)
    _dbus_server_socket_own_filename(server, path_copy);

  _dbus_string_free (&address);

  return server;

 failed_2:
  _dbus_close_socket (listen_fd, NULL);
 failed_1:
  dbus_free (path_copy);
 failed_0:
  _dbus_string_free (&address);

  return NULL;
}

/** @} */
