/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-debug-pipe.c In-proc debug server implementation
 *
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2003, 2004  Red Hat, Inc.
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
#include "dbus-server-debug-pipe.h"
#include "dbus-transport-socket.h"
#include "dbus-connection-internal.h"
#include "dbus-hash.h"
#include "dbus-string.h"
#include "dbus-protocol.h"

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

/**
 * @defgroup DBusServerDebugPipe DBusServerDebugPipe
 * @ingroup  DBusInternals
 * @brief In-process pipe debug server used in unit tests.
 *
 * Types and functions related to DBusServerDebugPipe.
 * This is used for unit testing.
 *
 * @{
 */

/**
 * Opaque object representing a debug server implementation.
 */
typedef struct DBusServerDebugPipe DBusServerDebugPipe;

/**
 * Implementation details of DBusServerDebugPipe. All members
 * are private.
 */
struct DBusServerDebugPipe
{
  DBusServer base;  /**< Parent class members. */

  char *name; /**< Server name. */

  dbus_bool_t disconnected; /**< TRUE if disconnect has been called */
};

/* FIXME not threadsafe (right now the test suite doesn't use threads anyhow ) */
static DBusHashTable *server_pipe_hash;
static int server_pipe_hash_refcount = 0;

static dbus_bool_t
pipe_hash_ref (void)
{
  if (!server_pipe_hash)
    {
      _dbus_assert (server_pipe_hash_refcount == 0);
      
      server_pipe_hash = _dbus_hash_table_new (DBUS_HASH_STRING, NULL, NULL);

      if (!server_pipe_hash)
        return FALSE;
    }

  server_pipe_hash_refcount = 1;

  return TRUE;
}

static void
pipe_hash_unref (void)
{
  _dbus_assert (server_pipe_hash != NULL);
  _dbus_assert (server_pipe_hash_refcount > 0);

  server_pipe_hash_refcount -= 1;
  if (server_pipe_hash_refcount == 0)
    {
      _dbus_hash_table_unref (server_pipe_hash);
      server_pipe_hash = NULL;
    }
}

static void
debug_finalize (DBusServer *server)
{
  DBusServerDebugPipe *debug_server = (DBusServerDebugPipe*) server;

  pipe_hash_unref ();
  
  _dbus_server_finalize_base (server);

  dbus_free (debug_server->name);
  dbus_free (server);
}

static void
debug_disconnect (DBusServer *server)
{
  ((DBusServerDebugPipe*)server)->disconnected = TRUE;
}

static DBusServerVTable debug_vtable = {
  debug_finalize,
  debug_disconnect
};

/**
 * Creates a new debug server using an in-process pipe
 *
 * @param server_name the name of the server.
 * @param error address where an error can be returned.
 * @returns a new server, or #NULL on failure.
 */
DBusServer*
_dbus_server_debug_pipe_new (const char     *server_name,
                             DBusError      *error)
{
  DBusServerDebugPipe *debug_server;
  DBusString address;
  DBusString name_str;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  if (!pipe_hash_ref ())
    return NULL;
  
  if (_dbus_hash_table_lookup_string (server_pipe_hash, server_name) != NULL)
    {
      dbus_set_error (error, DBUS_ERROR_ADDRESS_IN_USE, NULL);
      pipe_hash_unref ();
      return NULL;
    }
  
  debug_server = dbus_new0 (DBusServerDebugPipe, 1);
  if (debug_server == NULL)
    goto nomem_0;

  if (!_dbus_string_init (&address))
    goto nomem_1;

  _dbus_string_init_const (&name_str, server_name);
  if (!_dbus_string_append (&address, "debug-pipe:name=") ||
      !_dbus_address_append_escaped (&address, &name_str))
    goto nomem_2;
  
  debug_server->name = _dbus_strdup (server_name);
  if (debug_server->name == NULL)
    goto nomem_2;
  
  if (!_dbus_server_init_base (&debug_server->base,
			       &debug_vtable, &address,
                               error))
    goto fail_3;

  if (!_dbus_hash_table_insert_string (server_pipe_hash,
				       debug_server->name,
				       debug_server))
    goto nomem_4;

  _dbus_string_free (&address);

  /* server keeps the pipe hash ref */

  _dbus_server_trace_ref (&debug_server->base, 0, 1, "debug_pipe_new");
  return (DBusServer *)debug_server;

 nomem_4:
  _dbus_server_finalize_base (&debug_server->base);
 fail_3:
  dbus_free (debug_server->name);
 nomem_2:
  _dbus_string_free (&address);
 nomem_1:
  dbus_free (debug_server);
 nomem_0:
  pipe_hash_unref ();
  if (error != NULL && !dbus_error_is_set (error))
    _DBUS_SET_OOM (error);
  return NULL;
}

/**
 * Creates the client-side transport for
 * a debug-pipe connection connected to the
 * given debug-pipe server name.
 * 
 * @param server_name name of server to connect to
 * @param error address where an error can be returned.
 * @returns #NULL on no memory or transport
 */
DBusTransport*
_dbus_transport_debug_pipe_new (const char     *server_name,
                                DBusError      *error)
{
  DBusTransport *client_transport;
  DBusTransport *server_transport;
  DBusConnection *connection;
  DBusSocket client_fd, server_fd;
  DBusServer *server;
  DBusString address;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (server_pipe_hash == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_SERVER, NULL);
      return NULL;
    }
  
  server = _dbus_hash_table_lookup_string (server_pipe_hash,
                                           server_name);
  if (server == NULL ||
      ((DBusServerDebugPipe*)server)->disconnected)
    {
      dbus_set_error (error, DBUS_ERROR_NO_SERVER, NULL);
      return NULL;
    }

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if (!_dbus_string_append (&address, "debug-pipe:name=") ||
      !_dbus_string_append (&address, server_name))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&address);
      return NULL;
    }
  
  if (!_dbus_socketpair (&client_fd, &server_fd, FALSE, NULL))
    {
      _dbus_verbose ("failed to create full duplex pipe\n");
      dbus_set_error (error, DBUS_ERROR_FAILED, "Could not create full-duplex pipe");
      _dbus_string_free (&address);
      return NULL;
    }

  client_transport = _dbus_transport_new_for_socket (client_fd,
                                                     NULL, &address);
  if (client_transport == NULL)
    {
      _dbus_close_socket (client_fd, NULL);
      _dbus_close_socket (server_fd, NULL);
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&address);
      return NULL;
    }

  _dbus_string_free (&address);
  
  _dbus_socket_invalidate (&client_fd);

  server_transport = _dbus_transport_new_for_socket (server_fd,
                                                     &server->guid_hex, NULL);
  if (server_transport == NULL)
    {
      _dbus_transport_unref (client_transport);
      _dbus_close_socket (server_fd, NULL);
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  _dbus_socket_invalidate (&server_fd);

  if (!_dbus_transport_set_auth_mechanisms (server_transport,
                                            (const char**) server->auth_mechanisms))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_transport_unref (server_transport);
      _dbus_transport_unref (client_transport);
      return NULL;
    }
  
  connection = _dbus_connection_new_for_transport (server_transport);
  _dbus_transport_unref (server_transport);
  server_transport = NULL;
  
  if (connection == NULL)
    {
      _dbus_transport_unref (client_transport);
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  /* See if someone wants to handle this new connection,
   * self-referencing for paranoia
   */
  if (server->new_connection_function)
    {
      dbus_server_ref (server);
      (* server->new_connection_function) (server, connection,
                                           server->new_connection_data);
      dbus_server_unref (server);
    }
  
  /* If no one grabbed a reference, the connection will die,
   * and the client transport will get an immediate disconnect
   */
  _dbus_connection_close_if_only_one_ref (connection);
  dbus_connection_unref (connection);

  return client_transport;
}

/**
 * Tries to interpret the address entry as a debug pipe entry.
 * 
 * Sets error if the result is not OK.
 * 
 * @param entry an address entry
 * @param server_p location to store a new DBusServer, or #NULL on failure.
 * @param error location to store rationale for failure on bad address
 * @returns the outcome
 * 
 */
DBusServerListenResult
_dbus_server_listen_debug_pipe (DBusAddressEntry *entry,
                                DBusServer      **server_p,
                                DBusError        *error)
{
  const char *method;

  *server_p = NULL;
  
  method = dbus_address_entry_get_method (entry);
  
  if (strcmp (method, "debug-pipe") == 0)
    {
      const char *name = dbus_address_entry_get_value (entry, "name");
      
      if (name == NULL)
        {
          _dbus_set_bad_address(error, "debug-pipe", "name",
                                NULL);
          return DBUS_SERVER_LISTEN_BAD_ADDRESS;
        }

      *server_p = _dbus_server_debug_pipe_new (name, error);
      
      if (*server_p)
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
  else
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR(error);
      return DBUS_SERVER_LISTEN_NOT_HANDLED;
    }
}

/**
 * Opens a debug pipe transport, used in the test suite.
 * 
 * @param entry the address entry to try opening as debug-pipe
 * @param transport_p return location for the opened transport
 * @param error error to be set
 * @returns result of the attempt
 */
DBusTransportOpenResult
_dbus_transport_open_debug_pipe (DBusAddressEntry  *entry,
                                 DBusTransport    **transport_p,
                                 DBusError         *error)
{
  const char *method;
  
  method = dbus_address_entry_get_method (entry);
  _dbus_assert (method != NULL);

  if (strcmp (method, "debug-pipe") == 0)
    {
      const char *name = dbus_address_entry_get_value (entry, "name");

      if (name == NULL)
        {
          _dbus_set_bad_address (error, "debug-pipe", "name",
                                 NULL);
          return DBUS_TRANSPORT_OPEN_BAD_ADDRESS;
        }
          
      *transport_p = _dbus_transport_debug_pipe_new (name, error);

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
  else
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR (error);
      return DBUS_TRANSPORT_OPEN_NOT_HANDLED;
    }
}


/** @} */

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
