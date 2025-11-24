/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-transport.c DBusTransport object (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat Inc.
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
#include "dbus-transport-protected.h"
#include "dbus-transport-unix.h"
#include "dbus-transport-socket.h"
#include "dbus-connection-internal.h"
#include "dbus-watch.h"
#include "dbus-auth.h"
#include "dbus-address.h"
#include "dbus-credentials.h"
#include "dbus-mainloop.h"
#include "dbus-message.h"
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "dbus-server-debug-pipe.h"
#endif

/**
 * @defgroup DBusTransport DBusTransport object
 * @ingroup  DBusInternals
 * @brief "Backend" for a DBusConnection.
 *
 * Types and functions related to DBusTransport.  A transport is an
 * abstraction that can send and receive data via various kinds of
 * network connections or other IPC mechanisms.
 * 
 * @{
 */

/**
 * @typedef DBusTransport
 *
 * Opaque object representing a way message stream.
 * DBusTransport abstracts various kinds of actual
 * transport mechanism, such as different network protocols,
 * or encryption schemes.
 */

static void
live_messages_notify (DBusCounter *counter,
                           void        *user_data)
{
  DBusTransport *transport = user_data;

  _dbus_connection_lock (transport->connection);
  _dbus_transport_ref (transport);

#if 0
  _dbus_verbose ("Size counter value is now %d\n",
                 (int) _dbus_counter_get_size_value (counter));
  _dbus_verbose ("Unix FD counter value is now %d\n",
                 (int) _dbus_counter_get_unix_fd_value (counter));
#endif

  /* disable or re-enable the read watch for the transport if
   * required.
   */
  if (transport->vtable->live_messages_changed)
    {
      (* transport->vtable->live_messages_changed) (transport);
    }

  _dbus_transport_unref (transport);
  _dbus_connection_unlock (transport->connection);
}

/**
 * Initializes the base class members of DBusTransport.  Chained up to
 * by subclasses in their constructor.  The server GUID is the
 * globally unique ID for the server creating this connection
 * and will be #NULL for the client side of a connection. The GUID
 * is in hex format.
 *
 * @param transport the transport being created.
 * @param vtable the subclass vtable.
 * @param server_guid non-#NULL if this transport is on the server side of a connection
 * @param address the address of the transport
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_transport_init_base (DBusTransport             *transport,
                           const DBusTransportVTable *vtable,
                           const DBusString          *server_guid,
                           const DBusString          *address)
{
  DBusMessageLoader *loader;
  DBusAuth *auth;
  DBusCounter *counter;
  char *address_copy;
  DBusCredentials *creds;
  
  loader = _dbus_message_loader_new ();
  if (loader == NULL)
    return FALSE;
  
  if (server_guid)
    auth = _dbus_auth_server_new (server_guid);
  else
    auth = _dbus_auth_client_new ();
  if (auth == NULL)
    {
      _dbus_message_loader_unref (loader);
      return FALSE;
    }

  counter = _dbus_counter_new ();
  if (counter == NULL)
    {
      _dbus_auth_unref (auth);
      _dbus_message_loader_unref (loader);
      return FALSE;
    }  

  creds = _dbus_credentials_new ();
  if (creds == NULL)
    {
      _dbus_counter_unref (counter);
      _dbus_auth_unref (auth);
      _dbus_message_loader_unref (loader);
      return FALSE;
    }
  
  if (server_guid)
    {
      _dbus_assert (address == NULL);
      address_copy = NULL;
    }
  else
    {
      _dbus_assert (address != NULL);

      if (!_dbus_string_copy_data (address, &address_copy))
        {
          _dbus_credentials_unref (creds);
          _dbus_counter_unref (counter);
          _dbus_auth_unref (auth);
          _dbus_message_loader_unref (loader);
          return FALSE;
        }
    }
  
  transport->refcount = 1;
  transport->vtable = vtable;
  transport->loader = loader;
  transport->auth = auth;
  transport->live_messages = counter;
  transport->authenticated = FALSE;
  transport->disconnected = FALSE;
  transport->is_server = (server_guid != NULL);
  transport->send_credentials_pending = !transport->is_server;
  transport->receive_credentials_pending = transport->is_server;
  transport->address = address_copy;
  
  transport->unix_user_function = NULL;
  transport->unix_user_data = NULL;
  transport->free_unix_user_data = NULL;

  transport->windows_user_function = NULL;
  transport->windows_user_data = NULL;
  transport->free_windows_user_data = NULL;
  
  transport->expected_guid = NULL;
  
  /* Try to default to something that won't totally hose the system,
   * but doesn't impose too much of a limitation.
   */
  transport->max_live_messages_size = _DBUS_ONE_MEGABYTE * 63;

  /* On Linux RLIMIT_NOFILE defaults to 1024, so allowing 4096 fds live
     should be more than enough */
  transport->max_live_messages_unix_fds = 4096;

  /* credentials read from socket if any */
  transport->credentials = creds;

  _dbus_counter_set_notify (transport->live_messages,
                            transport->max_live_messages_size,
                            transport->max_live_messages_unix_fds,
                            live_messages_notify,
                            transport);

  if (transport->address)
    _dbus_verbose ("Initialized transport on address %s\n", transport->address);

  return TRUE;
}

/**
 * Finalizes base class members of DBusTransport.
 * Chained up to from subclass finalizers.
 *
 * @param transport the transport.
 */
void
_dbus_transport_finalize_base (DBusTransport *transport)
{
  if (!transport->disconnected)
    _dbus_transport_disconnect (transport);

  if (transport->free_unix_user_data != NULL)
    (* transport->free_unix_user_data) (transport->unix_user_data);

  if (transport->free_windows_user_data != NULL)
    (* transport->free_windows_user_data) (transport->windows_user_data);
  
  _dbus_message_loader_unref (transport->loader);
  _dbus_auth_unref (transport->auth);
  _dbus_counter_set_notify (transport->live_messages,
                            0, 0, NULL, NULL);
  _dbus_counter_unref (transport->live_messages);
  dbus_free (transport->address);
  dbus_free (transport->expected_guid);
  if (transport->credentials)
    _dbus_credentials_unref (transport->credentials);
}


/**
 * Verifies if a given D-Bus address is a valid address
 * by attempting to connect to it. If it is, returns the
 * opened DBusTransport object. If it isn't, returns #NULL
 * and sets @p error.
 *
 * @param address the address to be checked.
 * @param error address where an error can be returned.
 * @returns a new transport, or #NULL on failure.
 */
static DBusTransport*
check_address (const char *address, DBusError *error)
{
  DBusAddressEntry **entries;
  DBusTransport *transport = NULL;
  int len, i;

  _dbus_assert (address != NULL);
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!dbus_parse_address (address, &entries, &len, error))
    return NULL;              /* not a valid address */

  for (i = 0; i < len; i++)
    {
      dbus_error_free (error);
      transport = _dbus_transport_open (entries[i], error);

      if (transport != NULL)
        break;
    }

  dbus_address_entries_free (entries);
  return transport;
}

/**
 * Creates a new transport for the "autostart" method.
 * This creates a client-side of a transport.
 *
 * @param scope scope of autolaunch (Windows only)
 * @param error address where an error can be returned.
 * @returns a new transport, or #NULL on failure.
 */
static DBusTransport*
_dbus_transport_new_for_autolaunch (const char *scope, DBusError *error)
{
  DBusString address;
  DBusTransport *result = NULL;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (!_dbus_string_init (&address))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if (!_dbus_get_autolaunch_address (scope, &address, error))
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      goto out;
    }

  result = check_address (_dbus_string_get_const_data (&address), error);
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, result != NULL);

 out:
  _dbus_string_free (&address);
  return result;
}

static DBusTransportOpenResult
_dbus_transport_open_autolaunch (DBusAddressEntry  *entry,
                                 DBusTransport    **transport_p,
                                 DBusError         *error)
{
  const char *method;
  
  method = dbus_address_entry_get_method (entry);
  _dbus_assert (method != NULL);

  if (strcmp (method, "autolaunch") == 0)
    {
      const char *scope = dbus_address_entry_get_value (entry, "scope");

      *transport_p = _dbus_transport_new_for_autolaunch (scope, error);

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

static const struct {
  DBusTransportOpenResult (* func) (DBusAddressEntry *entry,
                                    DBusTransport   **transport_p,
                                    DBusError        *error);
} open_funcs[] = {
  { _dbus_transport_open_socket },
  { _dbus_transport_open_platform_specific },
  { _dbus_transport_open_autolaunch }
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  , { _dbus_transport_open_debug_pipe }
#endif
};

/**
 * Try to open a new transport for the given address entry.  (This
 * opens a client-side-of-the-connection transport.)
 * 
 * @param entry the address entry
 * @param error location to store reason for failure.
 * @returns new transport of #NULL on failure.
 */
DBusTransport*
_dbus_transport_open (DBusAddressEntry *entry,
                      DBusError        *error)
{
  DBusTransport *transport;
  const char *expected_guid_orig;
  char *expected_guid;
  int i;
  DBusError tmp_error = DBUS_ERROR_INIT;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  transport = NULL;
  expected_guid_orig = dbus_address_entry_get_value (entry, "guid");
  expected_guid = _dbus_strdup (expected_guid_orig);

  if (expected_guid_orig != NULL && expected_guid == NULL)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  for (i = 0; i < (int) _DBUS_N_ELEMENTS (open_funcs); ++i)
    {
      DBusTransportOpenResult result;

      _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
      result = (* open_funcs[i].func) (entry, &transport, &tmp_error);

      switch (result)
        {
        case DBUS_TRANSPORT_OPEN_OK:
          _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
          goto out;
          break;
        case DBUS_TRANSPORT_OPEN_NOT_HANDLED:
          _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
          /* keep going through the loop of open funcs */
          break;
        case DBUS_TRANSPORT_OPEN_BAD_ADDRESS:
          _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
          goto out;
          break;
        case DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT:
          _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
          goto out;
          break;
        default:
          _dbus_assert_not_reached ("invalid transport open result");
          break;
        }
    }

 out:
  
  if (transport == NULL)
    {
      if (!dbus_error_is_set (&tmp_error))
        _dbus_set_bad_address (&tmp_error,
                               NULL, NULL,
                               "Unknown address type (examples of valid types are \"tcp\" and on UNIX \"unix\")");
      
      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      dbus_move_error(&tmp_error, error);
      dbus_free (expected_guid);
    }
  else
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);

      /* In the case of autostart the initial guid is NULL
       * and the autostart transport recursively calls
       * _dbus_open_transport wich returns a transport
       * with a guid.  That guid is the definitive one.
       *
       * FIXME: if more transports are added they may have
       * an effect on the expected_guid semantics (i.e. 
       * expected_guid and transport->expected_guid may
       * both have values).  This is very unlikely though
       * we should either throw asserts here for those 
       * corner cases or refactor the code so it is 
       * clearer on what is expected and what is not
       */
      if(expected_guid)
        transport->expected_guid = expected_guid;
    }

  return transport;
}

/**
 * Increments the reference count for the transport.
 *
 * @param transport the transport.
 * @returns the transport.
 */
DBusTransport *
_dbus_transport_ref (DBusTransport *transport)
{
  _dbus_assert (transport->refcount > 0);
  
  transport->refcount += 1;

  return transport;
}

/**
 * Decrements the reference count for the transport.
 * Disconnects and finalizes the transport if
 * the reference count reaches zero.
 *
 * @param transport the transport.
 */
void
_dbus_transport_unref (DBusTransport *transport)
{
  _dbus_assert (transport != NULL);
  _dbus_assert (transport->refcount > 0);
  
  transport->refcount -= 1;
  if (transport->refcount == 0)
    {
      _dbus_verbose ("finalizing\n");
      
      _dbus_assert (transport->vtable->finalize != NULL);
      
      (* transport->vtable->finalize) (transport);
    }
}

/**
 * Closes our end of the connection to a remote application. Further
 * attempts to use this transport will fail. Only the first call to
 * _dbus_transport_disconnect() will have an effect.
 *
 * @param transport the transport.
 * 
 */
void
_dbus_transport_disconnect (DBusTransport *transport)
{
  _dbus_verbose ("start\n");
  
  _dbus_assert (transport->vtable->disconnect != NULL);
  
  if (transport->disconnected)
    return;

  (* transport->vtable->disconnect) (transport);
  
  transport->disconnected = TRUE;

  _dbus_verbose ("end\n");
}

/**
 * Returns #TRUE if the transport has not been disconnected.
 * Disconnection can result from _dbus_transport_disconnect()
 * or because the server drops its end of the connection.
 *
 * @param transport the transport.
 * @returns whether we're connected
 */
dbus_bool_t
_dbus_transport_get_is_connected (DBusTransport *transport)
{
  return !transport->disconnected;
}

static dbus_bool_t
auth_via_unix_user_function (DBusTransport *transport)
{
  DBusCredentials *auth_identity;
  dbus_bool_t allow;
  DBusConnection *connection;
  DBusAllowUnixUserFunction unix_user_function;
  void *unix_user_data;
  dbus_uid_t uid;

  /* Dropping the lock here probably isn't that safe. */
  
  auth_identity = _dbus_auth_get_identity (transport->auth);
  _dbus_assert (auth_identity != NULL);

  connection = transport->connection;
  unix_user_function = transport->unix_user_function;
  unix_user_data = transport->unix_user_data;
  uid = _dbus_credentials_get_unix_uid (auth_identity);
              
  _dbus_verbose ("unlock\n");
  _dbus_connection_unlock (connection);

  allow = (* unix_user_function) (connection,
                                  uid,
                                  unix_user_data);
              
  _dbus_verbose ("lock post unix user function\n");
  _dbus_connection_lock (connection);

  if (allow)
    {
      _dbus_verbose ("Client UID "DBUS_UID_FORMAT" authorized\n", uid);
    }
  else
    {
      _dbus_verbose ("Client UID "DBUS_UID_FORMAT
                     " was rejected, disconnecting\n",
                     _dbus_credentials_get_unix_uid (auth_identity));
      _dbus_transport_disconnect (transport);
    }

  return allow;
}

static dbus_bool_t
auth_via_windows_user_function (DBusTransport *transport)
{
  DBusCredentials *auth_identity;  
  dbus_bool_t allow;
  DBusConnection *connection;
  DBusAllowWindowsUserFunction windows_user_function;
  void *windows_user_data;
  char *windows_sid;

  /* Dropping the lock here probably isn't that safe. */
  
  auth_identity = _dbus_auth_get_identity (transport->auth);
  _dbus_assert (auth_identity != NULL);

  connection = transport->connection;
  windows_user_function = transport->windows_user_function;
  windows_user_data = transport->unix_user_data;
  windows_sid = _dbus_strdup (_dbus_credentials_get_windows_sid (auth_identity));

  if (windows_sid == NULL)
    {
      /* OOM */
      return FALSE;
    }
                
  _dbus_verbose ("unlock\n");
  _dbus_connection_unlock (connection);

  allow = (* windows_user_function) (connection,
                                     windows_sid,
                                     windows_user_data);
              
  _dbus_verbose ("lock post windows user function\n");
  _dbus_connection_lock (connection);

  if (allow)
    {
      _dbus_verbose ("Client SID '%s' authorized\n", windows_sid);
    }
  else
    {
      _dbus_verbose ("Client SID '%s' was rejected, disconnecting\n",
                     _dbus_credentials_get_windows_sid (auth_identity));
      _dbus_transport_disconnect (transport);
    }

  return allow;
}

static dbus_bool_t
auth_via_default_rules (DBusTransport *transport)
{
  DBusCredentials *auth_identity;
  DBusCredentials *our_identity;
  dbus_bool_t allow;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);
  _dbus_assert (auth_identity != NULL);

  /* By default, connection is allowed if the client is 1) root or 2)
   * has the same UID as us or 3) anonymous is allowed.
   */
  
  our_identity = _dbus_credentials_new_from_current_process ();
  if (our_identity == NULL)
    {
      /* OOM */
      return FALSE;
    }
              
  if (transport->allow_anonymous ||
      _dbus_credentials_get_unix_uid (auth_identity) == 0 ||
      _dbus_credentials_same_user (our_identity,
                                   auth_identity))
    {
      if (_dbus_credentials_include(our_identity,DBUS_CREDENTIAL_WINDOWS_SID))
          _dbus_verbose ("Client authorized as SID '%s'"
                         "matching our SID '%s'\n",
                         _dbus_credentials_get_windows_sid(auth_identity),
                         _dbus_credentials_get_windows_sid(our_identity));
      else
          _dbus_verbose ("Client authorized as UID "DBUS_UID_FORMAT
                         " matching our UID "DBUS_UID_FORMAT"\n",
                         _dbus_credentials_get_unix_uid(auth_identity),
                         _dbus_credentials_get_unix_uid(our_identity));
      /* We have authenticated! */
      allow = TRUE;
    }
  else
    {
      if (_dbus_credentials_include(our_identity,DBUS_CREDENTIAL_WINDOWS_SID))
          _dbus_verbose ("Client authorized as SID '%s'"
                         " but our SID is '%s', disconnecting\n",
                         (_dbus_credentials_get_windows_sid(auth_identity) ?
                          _dbus_credentials_get_windows_sid(auth_identity) : "<null>"),
                         (_dbus_credentials_get_windows_sid(our_identity) ?
                          _dbus_credentials_get_windows_sid(our_identity) : "<null>"));
      else
          _dbus_verbose ("Client authorized as UID "DBUS_UID_FORMAT
                         " but our UID is "DBUS_UID_FORMAT", disconnecting\n",
                         _dbus_credentials_get_unix_uid(auth_identity),
                         _dbus_credentials_get_unix_uid(our_identity));
      _dbus_transport_disconnect (transport);
      allow = FALSE;
    }  

  _dbus_credentials_unref (our_identity);
  
  return allow;
}

/**
 * Returns #TRUE if we have been authenticated. It will return #TRUE even if
 * the transport is now disconnected, but was ever authenticated before
 * disconnecting.
 *
 * This replaces the older _dbus_transport_get_is_authenticated() which
 * had side-effects.
 *
 * @param transport the transport
 * @returns whether we're authenticated
 */
dbus_bool_t
_dbus_transport_peek_is_authenticated (DBusTransport *transport)
{
  return transport->authenticated;
}

/**
 * Returns #TRUE if we have been authenticated. It will return #TRUE even if
 * the transport is now disconnected, but was ever authenticated before
 * disconnecting.
 *
 * If we have not finished authenticating, but we have enough buffered input
 * to finish the job, then this function will do so before it returns.
 *
 * This used to be called _dbus_transport_get_is_authenticated(), but that
 * name seems inappropriate for a function with side-effects.
 *
 * @todo we drop connection->mutex when calling the unix_user_function,
 * and windows_user_function, which may not be safe really.
 *
 * @param transport the transport
 * @returns whether we're authenticated
 */
dbus_bool_t
_dbus_transport_try_to_authenticate (DBusTransport *transport)
{  
  if (transport->authenticated)
    return TRUE;
  else
    {
      dbus_bool_t maybe_authenticated;
      
      if (transport->disconnected)
        return FALSE;

      /* paranoia ref since we call user callbacks sometimes */
      _dbus_connection_ref_unlocked (transport->connection);
      
      maybe_authenticated =
        (!(transport->send_credentials_pending ||
           transport->receive_credentials_pending));

      if (maybe_authenticated)
        {
          switch (_dbus_auth_do_work (transport->auth))
            {
            case DBUS_AUTH_STATE_AUTHENTICATED:
              /* leave as maybe_authenticated */
              break;

            case DBUS_AUTH_STATE_WAITING_FOR_INPUT:
            case DBUS_AUTH_STATE_WAITING_FOR_MEMORY:
            case DBUS_AUTH_STATE_HAVE_BYTES_TO_SEND:
            case DBUS_AUTH_STATE_NEED_DISCONNECT:
              maybe_authenticated = FALSE;
              break;

            case DBUS_AUTH_STATE_INVALID:
            default:
              _dbus_assert_not_reached ("invalid authentication state");
            }
        }

      /* If we're the client, verify the GUID
       */
      if (maybe_authenticated && !transport->is_server)
        {
          const char *server_guid;

          server_guid = _dbus_auth_get_guid_from_server (transport->auth);
          _dbus_assert (server_guid != NULL);

          if (transport->expected_guid &&
              strcmp (transport->expected_guid, server_guid) != 0)
            {
              _dbus_verbose ("Client expected GUID '%s' and we got '%s' from the server\n",
                             transport->expected_guid, server_guid);
              _dbus_transport_disconnect (transport);
              _dbus_connection_unref_unlocked (transport->connection);
              return FALSE;
            }
        }

      /* If we're the server, see if we want to allow this identity to proceed.
       */
      if (maybe_authenticated && transport->is_server)
        {
          dbus_bool_t allow;
          DBusCredentials *auth_identity;
          
          auth_identity = _dbus_auth_get_identity (transport->auth);
          _dbus_assert (auth_identity != NULL);
          
          /* If we have an auth'd user and a user function, delegate
           * deciding whether auth credentials are good enough to the
           * app; otherwise, use our default decision process.
           */
          if (transport->unix_user_function != NULL &&
              _dbus_credentials_include (auth_identity, DBUS_CREDENTIAL_UNIX_USER_ID))
            {
              allow = auth_via_unix_user_function (transport);
            }
          else if (transport->windows_user_function != NULL &&
                   _dbus_credentials_include (auth_identity, DBUS_CREDENTIAL_WINDOWS_SID))
            {
              allow = auth_via_windows_user_function (transport);
            }      
          else
            {
              allow = auth_via_default_rules (transport);
            }
          
          if (!allow)
            maybe_authenticated = FALSE;
        }

      transport->authenticated = maybe_authenticated;

      _dbus_connection_unref_unlocked (transport->connection);
      return maybe_authenticated;
    }
}

/**
 * See dbus_connection_get_is_anonymous().
 *
 * @param transport the transport
 * @returns #TRUE if not authenticated or authenticated as anonymous
 */
dbus_bool_t
_dbus_transport_get_is_anonymous (DBusTransport *transport)
{
  DBusCredentials *auth_identity;
  
  if (!transport->authenticated)
    return TRUE;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_are_anonymous (auth_identity))
    return TRUE;
  else
    return FALSE;
}

/**
 * Returns TRUE if the transport supports sending unix fds.
 *
 * @param transport the transport
 * @returns #TRUE if TRUE it is possible to send unix fds across the transport.
 */
dbus_bool_t
_dbus_transport_can_pass_unix_fd(DBusTransport *transport)
{
  return DBUS_TRANSPORT_CAN_SEND_UNIX_FD(transport);
}

/**
 * Gets the address of a transport. It will be
 * #NULL for a server-side transport.
 *
 * @param transport the transport
 * @returns transport's address
 */
const char*
_dbus_transport_get_address (DBusTransport *transport)
{
  return transport->address;
}

/**
 * Gets the id of the server we are connected to (see
 * dbus_server_get_id()). Only works on client side.
 *
 * @param transport the transport
 * @returns transport's server's id or #NULL if we are the server side
 */
const char*
_dbus_transport_get_server_id (DBusTransport *transport)
{
  if (transport->is_server)
    return NULL;
  else if (transport->authenticated)
    return _dbus_auth_get_guid_from_server (transport->auth);
  else
    return transport->expected_guid;
}

/**
 * Handles a watch by reading data, writing data, or disconnecting
 * the transport, as appropriate for the given condition.
 *
 * @param transport the transport.
 * @param watch the watch.
 * @param condition the current state of the watched file descriptor.
 * @returns #FALSE if not enough memory to fully handle the watch
 */
dbus_bool_t
_dbus_transport_handle_watch (DBusTransport           *transport,
                              DBusWatch               *watch,
                              unsigned int             condition)
{
  dbus_bool_t retval;
  
  _dbus_assert (transport->vtable->handle_watch != NULL);

  if (transport->disconnected)
    return TRUE;

  if (dbus_watch_get_socket (watch) < 0)
    {
      _dbus_warn_check_failed ("Tried to handle an invalidated watch; this watch should have been removed");
      return TRUE;
    }
  
  _dbus_watch_sanitize_condition (watch, &condition);

  _dbus_transport_ref (transport);
  _dbus_watch_ref (watch);
  retval = (* transport->vtable->handle_watch) (transport, watch, condition);
  _dbus_watch_unref (watch);
  _dbus_transport_unref (transport);

  return retval;
}

/**
 * Sets the connection using this transport. Allows the transport
 * to add watches to the connection, queue incoming messages,
 * and pull outgoing messages.
 *
 * @param transport the transport.
 * @param connection the connection.
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
_dbus_transport_set_connection (DBusTransport  *transport,
                                DBusConnection *connection)
{
  _dbus_assert (transport->vtable->connection_set != NULL);
  _dbus_assert (transport->connection == NULL);
  
  transport->connection = connection;

  _dbus_transport_ref (transport);
  if (!(* transport->vtable->connection_set) (transport))
    transport->connection = NULL;
  _dbus_transport_unref (transport);

  return transport->connection != NULL;
}

/**
 * Get the socket file descriptor, if any.
 *
 * @param transport the transport
 * @param fd_p pointer to fill in with the descriptor
 * @returns #TRUE if a descriptor was available
 */
dbus_bool_t
_dbus_transport_get_socket_fd (DBusTransport *transport,
                               DBusSocket    *fd_p)
{
  dbus_bool_t retval;
  
  if (transport->vtable->get_socket_fd == NULL)
    return FALSE;

  if (transport->disconnected)
    return FALSE;

  _dbus_transport_ref (transport);

  retval = (* transport->vtable->get_socket_fd) (transport,
                                                 fd_p);
  
  _dbus_transport_unref (transport);

  return retval;
}

/**
 * Performs a single poll()/select() on the transport's file
 * descriptors and then reads/writes data as appropriate,
 * queueing incoming messages and sending outgoing messages.
 * This is the backend for _dbus_connection_do_iteration().
 * See _dbus_connection_do_iteration() for full details.
 *
 * @param transport the transport.
 * @param flags indicates whether to read or write, and whether to block.
 * @param timeout_milliseconds if blocking, timeout or -1 for no timeout.
 */
void
_dbus_transport_do_iteration (DBusTransport  *transport,
                              unsigned int    flags,
                              int             timeout_milliseconds)
{
  _dbus_assert (transport->vtable->do_iteration != NULL);

  _dbus_verbose ("Transport iteration flags 0x%x timeout %d connected = %d\n",
                 flags, timeout_milliseconds, !transport->disconnected);
  
  if ((flags & (DBUS_ITERATION_DO_WRITING |
                DBUS_ITERATION_DO_READING)) == 0)
    return; /* Nothing to do */

  if (transport->disconnected)
    return;

  _dbus_transport_ref (transport);
  (* transport->vtable->do_iteration) (transport, flags,
                                       timeout_milliseconds);
  _dbus_transport_unref (transport);

  _dbus_verbose ("end\n");
}

static dbus_bool_t
recover_unused_bytes (DBusTransport *transport)
{
  if (_dbus_auth_needs_decoding (transport->auth))
    {
      DBusString plaintext;
      const DBusString *encoded;
      DBusString *buffer;
      int orig_len;
      
      if (!_dbus_string_init (&plaintext))
        goto nomem;
      
      _dbus_auth_get_unused_bytes (transport->auth,
                                   &encoded);

      if (!_dbus_auth_decode_data (transport->auth,
                                   encoded, &plaintext))
        {
          _dbus_string_free (&plaintext);
          goto nomem;
        }
      
      _dbus_message_loader_get_buffer (transport->loader,
                                       &buffer,
                                       NULL,
                                       NULL);
      
      orig_len = _dbus_string_get_length (buffer);
      
      if (!_dbus_string_move (&plaintext, 0, buffer,
                              orig_len))
        {
          _dbus_string_free (&plaintext);
          goto nomem;
        }
      
      _dbus_verbose (" %d unused bytes sent to message loader\n", 
                     _dbus_string_get_length (buffer) -
                     orig_len);
      
      _dbus_message_loader_return_buffer (transport->loader,
                                          buffer);

      _dbus_auth_delete_unused_bytes (transport->auth);
      
      _dbus_string_free (&plaintext);
    }
  else
    {
      const DBusString *bytes;
      DBusString *buffer;
#ifdef DBUS_ENABLE_VERBOSE_MODE
      int orig_len;
#endif
      dbus_bool_t succeeded;

      _dbus_message_loader_get_buffer (transport->loader,
                                       &buffer,
                                       NULL,
                                       NULL);

#ifdef DBUS_ENABLE_VERBOSE_MODE
      orig_len = _dbus_string_get_length (buffer);
#endif

      _dbus_auth_get_unused_bytes (transport->auth,
                                   &bytes);

      succeeded = TRUE;
      if (!_dbus_string_copy (bytes, 0, buffer, _dbus_string_get_length (buffer)))
        succeeded = FALSE;
      
      _dbus_verbose (" %d unused bytes sent to message loader\n", 
                     _dbus_string_get_length (buffer) -
                     orig_len);
      
      _dbus_message_loader_return_buffer (transport->loader,
                                          buffer);

      if (succeeded)
        _dbus_auth_delete_unused_bytes (transport->auth);
      else
        goto nomem;
    }

  return TRUE;

 nomem:
  _dbus_verbose ("Not enough memory to transfer unused bytes from auth conversation\n");
  return FALSE;
}

/**
 * Reports our current dispatch status (whether there's buffered
 * data to be queued as messages, or not, or we need memory).
 *
 * @param transport the transport
 * @returns current status
 */
DBusDispatchStatus
_dbus_transport_get_dispatch_status (DBusTransport *transport)
{
  if (_dbus_counter_get_size_value (transport->live_messages) >= transport->max_live_messages_size ||
      _dbus_counter_get_unix_fd_value (transport->live_messages) >= transport->max_live_messages_unix_fds)
    return DBUS_DISPATCH_COMPLETE; /* complete for now */

  if (!_dbus_transport_try_to_authenticate (transport))
    {
      if (_dbus_auth_do_work (transport->auth) ==
          DBUS_AUTH_STATE_WAITING_FOR_MEMORY)
        return DBUS_DISPATCH_NEED_MEMORY;
      else if (!_dbus_transport_try_to_authenticate (transport))
        return DBUS_DISPATCH_COMPLETE;
    }

  if (!transport->unused_bytes_recovered &&
      !recover_unused_bytes (transport))
    return DBUS_DISPATCH_NEED_MEMORY;

  transport->unused_bytes_recovered = TRUE;
  
  if (!_dbus_message_loader_queue_messages (transport->loader))
    return DBUS_DISPATCH_NEED_MEMORY;

  if (_dbus_message_loader_peek_message (transport->loader) != NULL)
    return DBUS_DISPATCH_DATA_REMAINS;
  else
    return DBUS_DISPATCH_COMPLETE;
}

/**
 * Processes data we've read while handling a watch, potentially
 * converting some of it to messages and queueing those messages on
 * the connection.
 *
 * @param transport the transport
 * @returns #TRUE if we had enough memory to queue all messages
 */
dbus_bool_t
_dbus_transport_queue_messages (DBusTransport *transport)
{
  DBusDispatchStatus status;

#if 0
  _dbus_verbose ("enter\n");
#endif

  /* Queue any messages */
  while ((status = _dbus_transport_get_dispatch_status (transport)) == DBUS_DISPATCH_DATA_REMAINS)
    {
      DBusMessage *message;
      DBusList *link;

      link = _dbus_message_loader_pop_message_link (transport->loader);
      _dbus_assert (link != NULL);
      
      message = link->data;
      
      _dbus_verbose ("queueing received message %p\n", message);

      if (!_dbus_message_add_counter (message, transport->live_messages))
        {
          _dbus_message_loader_putback_message_link (transport->loader,
                                                     link);
          status = DBUS_DISPATCH_NEED_MEMORY;
          break;
        }
      else
        {
          /* We didn't call the notify function when we added the counter, so
           * catch up now. Since we have the connection's lock, it's desirable
           * that we bypass the notify function and call this virtual method
           * directly. */
          if (transport->vtable->live_messages_changed)
            (* transport->vtable->live_messages_changed) (transport);

          /* pass ownership of link and message ref to connection */
          _dbus_connection_queue_received_message_link (transport->connection,
                                                        link);
        }
    }

  if (_dbus_message_loader_get_is_corrupted (transport->loader))
    {
      _dbus_verbose ("Corrupted message stream, disconnecting\n");
      _dbus_transport_disconnect (transport);
    }

  return status != DBUS_DISPATCH_NEED_MEMORY;
}

/**
 * See dbus_connection_set_max_message_size().
 *
 * @param transport the transport
 * @param size the max size of a single message
 */
void
_dbus_transport_set_max_message_size (DBusTransport  *transport,
                                      long            size)
{
  _dbus_message_loader_set_max_message_size (transport->loader, size);
}

/**
 * See dbus_connection_set_max_message_unix_fds().
 *
 * @param transport the transport
 * @param n the max number of unix fds of a single message
 */
void
_dbus_transport_set_max_message_unix_fds (DBusTransport  *transport,
                                          long            n)
{
  _dbus_message_loader_set_max_message_unix_fds (transport->loader, n);
}

/**
 * See dbus_connection_get_max_message_size().
 *
 * @param transport the transport
 * @returns max message size
 */
long
_dbus_transport_get_max_message_size (DBusTransport  *transport)
{
  return _dbus_message_loader_get_max_message_size (transport->loader);
}

/**
 * See dbus_connection_get_max_message_unix_fds().
 *
 * @param transport the transport
 * @returns max message unix fds
 */
long
_dbus_transport_get_max_message_unix_fds (DBusTransport  *transport)
{
  return _dbus_message_loader_get_max_message_unix_fds (transport->loader);
}

/**
 * See dbus_connection_set_max_received_size().
 *
 * @param transport the transport
 * @param size the max size of all incoming messages
 */
void
_dbus_transport_set_max_received_size (DBusTransport  *transport,
                                       long            size)
{
  transport->max_live_messages_size = size;
  _dbus_counter_set_notify (transport->live_messages,
                            transport->max_live_messages_size,
                            transport->max_live_messages_unix_fds,
                            live_messages_notify,
                            transport);
}

/**
 * See dbus_connection_set_max_received_unix_fds().
 *
 * @param transport the transport
 * @param n the max unix fds of all incoming messages
 */
void
_dbus_transport_set_max_received_unix_fds (DBusTransport  *transport,
                                           long            n)
{
  transport->max_live_messages_unix_fds = n;
  _dbus_counter_set_notify (transport->live_messages,
                            transport->max_live_messages_size,
                            transport->max_live_messages_unix_fds,
                            live_messages_notify,
                            transport);
}

/**
 * See dbus_connection_get_max_received_size().
 *
 * @param transport the transport
 * @returns max bytes for all live messages
 */
long
_dbus_transport_get_max_received_size (DBusTransport  *transport)
{
  return transport->max_live_messages_size;
}

/**
 * See dbus_connection_set_max_received_unix_fds().
 *
 * @param transport the transport
 * @returns max unix fds for all live messages
 */
long
_dbus_transport_get_max_received_unix_fds (DBusTransport  *transport)
{
  return transport->max_live_messages_unix_fds;
}

/**
 * See dbus_connection_get_unix_user().
 *
 * @param transport the transport
 * @param uid return location for the user ID
 * @returns #TRUE if uid is filled in with a valid user ID
 */
dbus_bool_t
_dbus_transport_get_unix_user (DBusTransport *transport,
                               unsigned long *uid)
{
  DBusCredentials *auth_identity;

  *uid = _DBUS_INT32_MAX; /* better than some root or system user in
                           * case of bugs in the caller. Caller should
                           * never use this value on purpose, however.
                           */
  
  if (!transport->authenticated)
    return FALSE;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_include (auth_identity,
                                 DBUS_CREDENTIAL_UNIX_USER_ID))
    {
      *uid = _dbus_credentials_get_unix_uid (auth_identity);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * See dbus_connection_get_unix_process_id().
 *
 * @param transport the transport
 * @param pid return location for the process ID
 * @returns #TRUE if uid is filled in with a valid process ID
 */
dbus_bool_t
_dbus_transport_get_unix_process_id (DBusTransport *transport,
				     unsigned long *pid)
{
  DBusCredentials *auth_identity;

  *pid = DBUS_PID_UNSET; /* Caller should never use this value on purpose,
			  * but we set it to a safe number, INT_MAX,
			  * just to root out possible bugs in bad callers.
			  */
  
  if (!transport->authenticated)
    return FALSE;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_include (auth_identity,
                                 DBUS_CREDENTIAL_UNIX_PROCESS_ID))
    {
      *pid = _dbus_credentials_get_pid (auth_identity);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * See dbus_connection_get_adt_audit_session_data().
 *
 * @param transport the transport
 * @param data return location for the ADT audit data 
 * @param data_size return length of audit data
 * @returns #TRUE if audit data is filled in with a valid ucred
 */
dbus_bool_t
_dbus_transport_get_adt_audit_session_data (DBusTransport      *transport,
                                            void              **data,
                                            int                *data_size)
{
  DBusCredentials *auth_identity;

  *data = NULL;
  *data_size = 0;
  
  if (!transport->authenticated)
    return FALSE;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_include (auth_identity,
                                 DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID))
    {
      *data = (void *) _dbus_credentials_get_adt_audit_data (auth_identity);
      *data_size = _dbus_credentials_get_adt_audit_data_size (auth_identity);
      return TRUE;
    }
  else
    return FALSE;
}

/**
 * See dbus_connection_set_unix_user_function().
 *
 * @param transport the transport
 * @param function the predicate
 * @param data data to pass to the predicate
 * @param free_data_function function to free the data
 * @param old_data the old user data to be freed
 * @param old_free_data_function old free data function to free it with
 */
void
_dbus_transport_set_unix_user_function (DBusTransport             *transport,
                                        DBusAllowUnixUserFunction  function,
                                        void                      *data,
                                        DBusFreeFunction           free_data_function,
                                        void                     **old_data,
                                        DBusFreeFunction          *old_free_data_function)
{  
  *old_data = transport->unix_user_data;
  *old_free_data_function = transport->free_unix_user_data;

  transport->unix_user_function = function;
  transport->unix_user_data = data;
  transport->free_unix_user_data = free_data_function;
}

dbus_bool_t
_dbus_transport_get_linux_security_label (DBusTransport  *transport,
                                          char          **label_p)
{
  DBusCredentials *auth_identity;

  *label_p = NULL;

  if (!transport->authenticated)
    return FALSE;

  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_include (auth_identity,
                                 DBUS_CREDENTIAL_LINUX_SECURITY_LABEL))
    {
      /* If no memory, we are supposed to return TRUE and set NULL */
      *label_p = _dbus_strdup (_dbus_credentials_get_linux_security_label (auth_identity));

      return TRUE;
    }
  else
    {
      return FALSE;
    }
}

/**
 * If the transport has already been authenticated, return its
 * credentials. If not, return #NULL.
 *
 * The caller must ref the returned credentials object if it wants to
 * keep it.
 */
DBusCredentials *
_dbus_transport_get_credentials (DBusTransport  *transport)
{
  if (!transport->authenticated)
    return FALSE;

  return _dbus_auth_get_identity (transport->auth);
}

/**
 * See dbus_connection_get_windows_user().
 *
 * @param transport the transport
 * @param windows_sid_p return location for the user ID
 * @returns #TRUE if user is available; the returned value may still be #NULL if no memory to copy it
 */
dbus_bool_t
_dbus_transport_get_windows_user (DBusTransport              *transport,
                                  char                      **windows_sid_p)
{
  DBusCredentials *auth_identity;

  *windows_sid_p = NULL;
  
  if (!transport->authenticated)
    return FALSE;
  
  auth_identity = _dbus_auth_get_identity (transport->auth);

  if (_dbus_credentials_include (auth_identity,
                                 DBUS_CREDENTIAL_WINDOWS_SID))
    {
      /* If no memory, we are supposed to return TRUE and set NULL */
      *windows_sid_p = _dbus_strdup (_dbus_credentials_get_windows_sid (auth_identity));

      return TRUE;
    }
  else
    return FALSE;
}

/**
 * See dbus_connection_set_windows_user_function().
 *
 * @param transport the transport
 * @param function the predicate
 * @param data data to pass to the predicate
 * @param free_data_function function to free the data
 * @param old_data the old user data to be freed
 * @param old_free_data_function old free data function to free it with
 */

void
_dbus_transport_set_windows_user_function (DBusTransport              *transport,
                                           DBusAllowWindowsUserFunction   function,
                                           void                       *data,
                                           DBusFreeFunction            free_data_function,
                                           void                      **old_data,
                                           DBusFreeFunction           *old_free_data_function)
{
  *old_data = transport->windows_user_data;
  *old_free_data_function = transport->free_windows_user_data;

  transport->windows_user_function = function;
  transport->windows_user_data = data;
  transport->free_windows_user_data = free_data_function;
}

/**
 * Sets the SASL authentication mechanisms supported by this transport.
 *
 * @param transport the transport
 * @param mechanisms the #NULL-terminated array of mechanisms
 *
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_transport_set_auth_mechanisms (DBusTransport  *transport,
                                     const char    **mechanisms)
{
  return _dbus_auth_set_mechanisms (transport->auth, mechanisms);
}

/**
 * See dbus_connection_set_allow_anonymous()
 *
 * @param transport the transport
 * @param value #TRUE to allow anonymous connection
 */
void
_dbus_transport_set_allow_anonymous (DBusTransport              *transport,
                                     dbus_bool_t                 value)
{
  transport->allow_anonymous = value != FALSE;
}

/**
 * Return how many file descriptors are pending in the loader
 *
 * @param transport the transport
 */
int
_dbus_transport_get_pending_fds_count (DBusTransport *transport)
{
  return _dbus_message_loader_get_pending_fds_count (transport->loader);
}

/**
 * Register a function to be called whenever the number of pending file
 * descriptors in the loader change.
 *
 * @param transport the transport
 * @param callback the callback
 */
void
_dbus_transport_set_pending_fds_function (DBusTransport *transport,
                                           void (* callback) (void *),
                                           void *data)
{
  _dbus_message_loader_set_pending_fds_function (transport->loader,
                                                 callback, data);
}

#ifdef DBUS_ENABLE_STATS
void
_dbus_transport_get_stats (DBusTransport  *transport,
                           dbus_uint32_t  *queue_bytes,
                           dbus_uint32_t  *queue_fds,
                           dbus_uint32_t  *peak_queue_bytes,
                           dbus_uint32_t  *peak_queue_fds)
{
  if (queue_bytes != NULL)
    *queue_bytes = _dbus_counter_get_size_value (transport->live_messages);

  if (queue_fds != NULL)
    *queue_fds = _dbus_counter_get_unix_fd_value (transport->live_messages);

  if (peak_queue_bytes != NULL)
    *peak_queue_bytes = _dbus_counter_get_peak_size_value (transport->live_messages);

  if (peak_queue_fds != NULL)
    *peak_queue_fds = _dbus_counter_get_peak_unix_fd_value (transport->live_messages);
}
#endif /* DBUS_ENABLE_STATS */

/** @} */
