/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-socket.c Server implementation for sockets
 *
 * Copyright (C) 2002, 2003, 2004, 2006  Red Hat Inc.
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
#include "dbus-server-socket.h"
#include "dbus-transport-socket.h"
#include "dbus-connection-internal.h"
#include "dbus-memory.h"
#include "dbus-nonce.h"
#include "dbus-string.h"

/**
 * @defgroup DBusServerSocket DBusServer implementations for SOCKET
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusServer on SOCKET
 *
 * @{
 */
/**
 *
 * Opaque object representing a Socket server implementation.
 */
typedef struct DBusServerSocket DBusServerSocket;

/**
 * Implementation details of DBusServerSocket. All members
 * are private.
 */
struct DBusServerSocket
{
  DBusServer base;   /**< Parent class members. */
  int n_fds;         /**< Number of active file handles */
  DBusSocket *fds;   /**< File descriptor or DBUS_SOCKET_INVALID if disconnected. */
  DBusWatch **watch; /**< File descriptor watch. */
  char *socket_name; /**< Name of domain socket, to unlink if appropriate */
  DBusNonceFile *noncefile; /**< Nonce file used to authenticate clients */
};

static void
socket_finalize (DBusServer *server)
{
  DBusServerSocket *socket_server = (DBusServerSocket*) server;
  int i;

  _dbus_server_finalize_base (server);

  for (i = 0 ; i < socket_server->n_fds ; i++)
    if (socket_server->watch[i])
      {
        _dbus_watch_unref (socket_server->watch[i]);
        socket_server->watch[i] = NULL;
      }

  dbus_free (socket_server->fds);
  dbus_free (socket_server->watch);
  dbus_free (socket_server->socket_name);
  _dbus_noncefile_delete (&socket_server->noncefile, NULL);
  dbus_free (server);
}

/* Return value is just for memory, not other failures. */
static dbus_bool_t
handle_new_client_fd_and_unlock (DBusServer *server,
                                 DBusSocket  client_fd)
{
  DBusConnection *connection;
  DBusTransport *transport;
  DBusNewConnectionFunction new_connection_function;
  void *new_connection_data;

  _dbus_verbose ("Creating new client connection with fd %" DBUS_SOCKET_FORMAT "\n",
                 _dbus_socket_printable (client_fd));

  HAVE_LOCK_CHECK (server);

  if (!_dbus_set_socket_nonblocking (client_fd, NULL))
    {
      SERVER_UNLOCK (server);
      return TRUE;
    }

  transport = _dbus_transport_new_for_socket (client_fd, &server->guid_hex, NULL);
  if (transport == NULL)
    {
      _dbus_close_socket (client_fd, NULL);
      SERVER_UNLOCK (server);
      return FALSE;
    }

  if (!_dbus_transport_set_auth_mechanisms (transport,
                                            (const char **) server->auth_mechanisms))
    {
      _dbus_transport_unref (transport);
      SERVER_UNLOCK (server);
      return FALSE;
    }

  /* note that client_fd is now owned by the transport, and will be
   * closed on transport disconnection/finalization
   */

  connection = _dbus_connection_new_for_transport (transport);
  _dbus_transport_unref (transport);
  transport = NULL; /* now under the connection lock */

  if (connection == NULL)
    {
      SERVER_UNLOCK (server);
      return FALSE;
    }

  /* See if someone wants to handle this new connection, self-referencing
   * for paranoia.
   */
  new_connection_function = server->new_connection_function;
  new_connection_data = server->new_connection_data;

  _dbus_server_ref_unlocked (server);
  SERVER_UNLOCK (server);

  if (new_connection_function)
    {
      (* new_connection_function) (server, connection,
                                   new_connection_data);
    }
  dbus_server_unref (server);

  /* If no one grabbed a reference, the connection will die. */
  _dbus_connection_close_if_only_one_ref (connection);
  dbus_connection_unref (connection);

  return TRUE;
}

static dbus_bool_t
socket_handle_watch (DBusWatch    *watch,
                   unsigned int  flags,
                   void         *data)
{
  DBusServer *server = data;
  DBusServerSocket *socket_server = data;

#ifndef DBUS_DISABLE_ASSERT
  int i;
  dbus_bool_t found = FALSE;
#endif

  SERVER_LOCK (server);

#ifndef DBUS_DISABLE_ASSERT
  for (i = 0 ; i < socket_server->n_fds ; i++)
    {
      if (socket_server->watch[i] == watch)
        found = TRUE;
    }
  _dbus_assert (found);
#endif

  _dbus_verbose ("Handling client connection, flags 0x%x\n", flags);

  if (flags & DBUS_WATCH_READABLE)
    {
      DBusSocket client_fd;
      DBusSocket listen_fd;
      int saved_errno;

      listen_fd = _dbus_watch_get_socket (watch);

      if (socket_server->noncefile)
          client_fd = _dbus_accept_with_noncefile (listen_fd, socket_server->noncefile);
      else 
          client_fd = _dbus_accept (listen_fd);

      saved_errno = _dbus_save_socket_errno ();

      if (!_dbus_socket_is_valid (client_fd))
        {
          /* EINTR handled for us */

          if (_dbus_get_is_errno_eagain_or_ewouldblock (saved_errno))
            _dbus_verbose ("No client available to accept after all\n");
          else
            _dbus_verbose ("Failed to accept a client connection: %s\n",
                           _dbus_strerror (saved_errno));

          SERVER_UNLOCK (server);
        }
      else
        {
          if (!handle_new_client_fd_and_unlock (server, client_fd))
            _dbus_verbose ("Rejected client connection due to lack of memory\n");
        }
    }

  if (flags & DBUS_WATCH_ERROR)
    _dbus_verbose ("Error on server listening socket\n");

  if (flags & DBUS_WATCH_HANGUP)
    _dbus_verbose ("Hangup on server listening socket\n");

  return TRUE;
}

static void
socket_disconnect (DBusServer *server)
{
  DBusServerSocket *socket_server = (DBusServerSocket*) server;
  int i;

  HAVE_LOCK_CHECK (server);

  for (i = 0 ; i < socket_server->n_fds ; i++)
    {
      if (socket_server->watch[i])
        {
          _dbus_server_remove_watch (server,
                                     socket_server->watch[i]);
          _dbus_watch_invalidate (socket_server->watch[i]);
          _dbus_watch_unref (socket_server->watch[i]);
          socket_server->watch[i] = NULL;
        }

      if (_dbus_socket_is_valid (socket_server->fds[i]))
        {
          _dbus_close_socket (socket_server->fds[i], NULL);
          _dbus_socket_invalidate (&socket_server->fds[i]);
        }
    }

  if (socket_server->socket_name != NULL)
    {
      DBusString tmp;
      _dbus_string_init_const (&tmp, socket_server->socket_name);
      _dbus_delete_file (&tmp, NULL);
    }

  if (server->published_address)
      _dbus_daemon_unpublish_session_bus_address();

  HAVE_LOCK_CHECK (server);
}

static const DBusServerVTable socket_vtable = {
  socket_finalize,
  socket_disconnect
};

/**
 * Creates a new server listening on the given file descriptor.  The
 * file descriptor should be nonblocking (use
 * _dbus_set_fd_nonblocking() to make it so). The file descriptor
 * should be listening for connections, that is, listen() should have
 * been successfully invoked on it. The server will use accept() to
 * accept new client connections.
 *
 * @param fds list of file descriptors.
 * @param n_fds number of file descriptors
 * @param address the server's address
 * @param noncefile to be used for authentication (NULL if not needed)
 * @param error location to store reason for failure
 * @returns the new server, or #NULL on OOM or other error.
 *
 */
DBusServer*
_dbus_server_new_for_socket (DBusSocket       *fds,
                             int               n_fds,
                             const DBusString *address,
                             DBusNonceFile    *noncefile,
                             DBusError        *error)
{
  DBusServerSocket *socket_server;
  DBusServer *server;
  int i;

  socket_server = dbus_new0 (DBusServerSocket, 1);
  if (socket_server == NULL)
    goto failed;

  socket_server->noncefile = noncefile;

  socket_server->fds = dbus_new (DBusSocket, n_fds);
  if (!socket_server->fds)
    goto failed;

  socket_server->watch = dbus_new0 (DBusWatch *, n_fds);
  if (!socket_server->watch)
    goto failed;

  for (i = 0 ; i < n_fds ; i++)
    {
      DBusWatch *watch;

      watch = _dbus_watch_new (_dbus_socket_get_pollable (fds[i]),
                               DBUS_WATCH_READABLE,
                               TRUE,
                               socket_handle_watch, socket_server,
                               NULL);
      if (watch == NULL)
        goto failed;

      socket_server->n_fds++;
      socket_server->fds[i] = fds[i];
      socket_server->watch[i] = watch;
    }

  if (!_dbus_server_init_base (&socket_server->base,
                               &socket_vtable, address,
                               error))
    goto failed;

  server = (DBusServer*)socket_server;

  SERVER_LOCK (server);

  for (i = 0 ; i < n_fds ; i++)
    {
      if (!_dbus_server_add_watch (&socket_server->base,
                                   socket_server->watch[i]))
        {
          int j;

          /* The caller is still responsible for closing the fds until
           * we return successfully, so don't let socket_disconnect()
           * close them */
          for (j = 0; j < n_fds; j++)
            _dbus_socket_invalidate (&socket_server->fds[j]);

          /* socket_disconnect() will try to remove all the watches;
           * make sure it doesn't see the ones that weren't even added
           * yet */
          for (j = i; j < n_fds; j++)
            {
              _dbus_watch_invalidate (socket_server->watch[j]);
              _dbus_watch_unref (socket_server->watch[j]);
              socket_server->watch[j] = NULL;
            }

          _dbus_server_disconnect_unlocked (server);
          SERVER_UNLOCK (server);
          _dbus_server_finalize_base (&socket_server->base);
          goto failed;
        }
    }

  SERVER_UNLOCK (server);

  _dbus_server_trace_ref (&socket_server->base, 0, 1, "new_for_socket");
  return (DBusServer*) socket_server;

failed:
  if (socket_server != NULL)
    {
      if (socket_server->watch != NULL)
        {
          for (i = 0; i < n_fds; i++)
            {
              if (socket_server->watch[i] != NULL)
                {
                  _dbus_watch_invalidate (socket_server->watch[i]);
                  _dbus_watch_unref (socket_server->watch[i]);
                  socket_server->watch[i] = NULL;
                }
            }
        }

      dbus_free (socket_server->watch);
      dbus_free (socket_server->fds);
      dbus_free (socket_server);
    }

  if (error != NULL && !dbus_error_is_set (error))
    _DBUS_SET_OOM (error);

  return NULL;
}

/**
 * Creates a new server listening on TCP.
 * If host is NULL, it will default to localhost.
 * If bind is NULL, it will default to the value for the host
 * parameter, and if that is NULL, then localhost
 * If bind is a hostname, it will be resolved and will listen
 * on all returned addresses.
 * If family is NULL, hostname resolution will try all address
 * families, otherwise it can be ipv4 or ipv6 to restrict the
 * addresses considered.
 *
 * @param host the hostname to report for the listen address
 * @param bind the hostname to listen on
 * @param port the port to listen on or 0 to let the OS choose
 * @param family
 * @param error location to store reason for failure.
 * @param use_nonce whether to use a nonce for low-level authentication (nonce-tcp transport) or not (tcp transport)
 * @returns the new server, or #NULL on failure.
 */
DBusServer*
_dbus_server_new_for_tcp_socket (const char     *host,
                                 const char     *bind,
                                 const char     *port,
                                 const char     *family,
                                 DBusError      *error,
                                 dbus_bool_t    use_nonce)
{
  DBusServer *server = NULL;
  DBusSocket *listen_fds = NULL;
  int nlisten_fds = 0, i;
  DBusString address = _DBUS_STRING_INIT_INVALID;
  DBusString host_str;    /* Initialized as const later, not freed */
  DBusString port_str = _DBUS_STRING_INIT_INVALID;
  DBusNonceFile *noncefile = NULL;
  const char *family_used = NULL;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  if (!_dbus_string_init (&port_str))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  if (host == NULL)
    host = "localhost";

  if (port == NULL)
    port = "0";

  if (bind == NULL)
    bind = host;
  else if (strcmp (bind, "*") == 0)
    bind = NULL;

  nlisten_fds =_dbus_listen_tcp_socket (bind, port, family,
                                        &port_str,
                                        &family_used,
                                        &listen_fds, error);
  if (nlisten_fds <= 0)
    {
      _DBUS_ASSERT_ERROR_IS_SET(error);
      goto failed;
    }

  _dbus_string_init_const (&host_str, host);
  if (!_dbus_string_append (&address, use_nonce ? "nonce-tcp:host=" : "tcp:host=") ||
      !_dbus_address_append_escaped (&address, &host_str) ||
      !_dbus_string_append (&address, ",port=") ||
      !_dbus_string_append (&address, _dbus_string_get_const_data(&port_str)))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }
  if (family_used &&
      (!_dbus_string_append (&address, ",family=") ||
       !_dbus_string_append (&address, family_used)))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  if (use_nonce)
    {
      if (!_dbus_noncefile_create (&noncefile, error))
        goto failed;

      if (!_dbus_string_append (&address, ",noncefile=") ||
          !_dbus_address_append_escaped (&address, _dbus_noncefile_get_path (noncefile)))
        {
          dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
          goto failed;
        }
    }

  server = _dbus_server_new_for_socket (listen_fds, nlisten_fds, &address, noncefile, error);
  if (server == NULL)
    goto failed;

  /* server has taken ownership of noncefile and the fds in listen_fds */
  _dbus_string_free (&port_str);
  _dbus_string_free (&address);
  dbus_free(listen_fds);

  return server;

failed:
  _dbus_noncefile_delete (&noncefile, NULL);

  if (listen_fds != NULL)
    {
      for (i = 0; i < nlisten_fds; i++)
        _dbus_close_socket (listen_fds[i], NULL);
      dbus_free (listen_fds);
    }

  _dbus_string_free (&port_str);
  _dbus_string_free (&address);
  return NULL;
}

/**
 * Tries to interpret the address entry for various socket-related
 * addresses (well, currently only tcp and nonce-tcp).
 *
 * Sets error if the result is not OK.
 *
 * @param entry an address entry
 * @param server_p a new DBusServer, or #NULL on failure.
 * @param error location to store rationale for failure on bad address
 * @returns the outcome
 *
 */
DBusServerListenResult
_dbus_server_listen_socket (DBusAddressEntry *entry,
                            DBusServer      **server_p,
                            DBusError        *error)
{
  const char *method;

  *server_p = NULL;

  method = dbus_address_entry_get_method (entry);

  if (strcmp (method, "tcp") == 0 || strcmp (method, "nonce-tcp") == 0)
    {
      const char *host;
      const char *port;
      const char *bind;
      const char *family;

      host = dbus_address_entry_get_value (entry, "host");
      bind = dbus_address_entry_get_value (entry, "bind");
      port = dbus_address_entry_get_value (entry, "port");
      family = dbus_address_entry_get_value (entry, "family");

      *server_p = _dbus_server_new_for_tcp_socket (host, bind, port,
                                                   family, error, strcmp (method, "nonce-tcp") == 0 ? TRUE : FALSE);

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
 * This is a bad hack since it's really unix domain socket
 * specific. Also, the function weirdly adopts ownership
 * of the passed-in string.
 *
 * @param server a socket server
 * @param filename socket filename to report/delete
 *
 */
void
_dbus_server_socket_own_filename (DBusServer *server,
                                  char       *filename)
{
  DBusServerSocket *socket_server = (DBusServerSocket*) server;

  socket_server->socket_name = filename;
}


/** @} */
