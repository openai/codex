/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server.c DBusServer object
 *
 * Copyright (C) 2002, 2003, 2004, 2005 Red Hat Inc.
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
#include "dbus-server.h"
#include "dbus-server-unix.h"
#include "dbus-server-socket.h"
#include "dbus-string.h"
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "dbus-server-debug-pipe.h"
#endif
#include "dbus-address.h"
#include "dbus-protocol.h"

/**
 * @defgroup DBusServer DBusServer
 * @ingroup  DBus
 * @brief Server that listens for new connections.
 *
 * A DBusServer represents a server that other applications
 * can connect to. Each connection from another application
 * is represented by a #DBusConnection.
 *
 * @todo Thread safety hasn't been tested much for #DBusServer
 * @todo Need notification to apps of disconnection, may matter for some transports
 */

/**
 * @defgroup DBusServerInternals DBusServer implementation details
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusServer
 *
 * @{
 */

#ifndef _dbus_server_trace_ref
void
_dbus_server_trace_ref (DBusServer *server,
    int old_refcount,
    int new_refcount,
    const char *why)
{
  static int enabled = -1;

  _dbus_trace_ref ("DBusServer", server, old_refcount, new_refcount, why,
      "DBUS_SERVER_TRACE", &enabled);
}
#endif

/* this is a little fragile since it assumes the address doesn't
 * already have a guid, but it shouldn't
 */
static char*
copy_address_with_guid_appended (const DBusString *address,
                                 const DBusString *guid_hex)
{
  DBusString with_guid;
  char *retval;
  
  if (!_dbus_string_init (&with_guid))
    return NULL;

  if (!_dbus_string_copy (address, 0, &with_guid,
                          _dbus_string_get_length (&with_guid)) ||
      !_dbus_string_append (&with_guid, ",guid=") ||
      !_dbus_string_copy (guid_hex, 0,
                          &with_guid, _dbus_string_get_length (&with_guid)))
    {
      _dbus_string_free (&with_guid);
      return NULL;
    }

  retval = NULL;
  _dbus_string_steal_data (&with_guid, &retval);

  _dbus_string_free (&with_guid);
      
  return retval; /* may be NULL if steal_data failed */
}

/**
 * Initializes the members of the DBusServer base class.
 * Chained up to by subclass constructors.
 *
 * @param server the server.
 * @param vtable the vtable for the subclass.
 * @param address the server's address
 * @param error location to store reason for failure
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_server_init_base (DBusServer             *server,
                        const DBusServerVTable *vtable,
                        const DBusString       *address,
                        DBusError              *error)
{
  server->vtable = vtable;

#ifdef DBUS_DISABLE_ASSERT
  _dbus_atomic_inc (&server->refcount);
#else
    {
      dbus_int32_t old_refcount = _dbus_atomic_inc (&server->refcount);

      _dbus_assert (old_refcount == 0);
    }
#endif

  server->address = NULL;
  server->watches = NULL;
  server->timeouts = NULL;
  server->published_address = FALSE;

  if (!_dbus_string_init (&server->guid_hex))
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_generate_uuid (&server->guid, error))
    goto failed;

  if (!_dbus_uuid_encode (&server->guid, &server->guid_hex))
    goto oom;
  
  server->address = copy_address_with_guid_appended (address,
                                                     &server->guid_hex);
  if (server->address == NULL)
    goto oom;
  
  _dbus_rmutex_new_at_location (&server->mutex);
  if (server->mutex == NULL)
    goto oom;
  
  server->watches = _dbus_watch_list_new ();
  if (server->watches == NULL)
    goto oom;

  server->timeouts = _dbus_timeout_list_new ();
  if (server->timeouts == NULL)
    goto oom;

  _dbus_data_slot_list_init (&server->slot_list);

  _dbus_verbose ("Initialized server on address %s\n", server->address);
  
  return TRUE;

 oom:
  _DBUS_SET_OOM (error);
 failed:
  _dbus_rmutex_free_at_location (&server->mutex);
  server->mutex = NULL;
  if (server->watches)
    {
      _dbus_watch_list_free (server->watches);
      server->watches = NULL;
    }
  if (server->timeouts)
    {
      _dbus_timeout_list_free (server->timeouts);
      server->timeouts = NULL;
    }
  if (server->address)
    {
      dbus_free (server->address);
      server->address = NULL;
    }
  _dbus_string_free (&server->guid_hex);
  
  return FALSE;
}

/**
 * Finalizes the members of the DBusServer base class.
 * Chained up to by subclass finalizers.
 *
 * @param server the server.
 */
void
_dbus_server_finalize_base (DBusServer *server)
{
  /* We don't have the lock, but nobody should be accessing
   * concurrently since they don't have a ref
   */
#ifndef DBUS_DISABLE_CHECKS
  _dbus_assert (!server->have_server_lock);
#endif
  _dbus_assert (server->disconnected);
  
  /* calls out to application code... */
  _dbus_data_slot_list_free (&server->slot_list);

  dbus_server_set_new_connection_function (server, NULL, NULL, NULL);

  _dbus_watch_list_free (server->watches);
  _dbus_timeout_list_free (server->timeouts);

  _dbus_rmutex_free_at_location (&server->mutex);
  
  dbus_free (server->address);

  dbus_free_string_array (server->auth_mechanisms);

  _dbus_string_free (&server->guid_hex);
}


/** Function to be called in protected_change_watch() with refcount held */
typedef dbus_bool_t (* DBusWatchAddFunction)     (DBusWatchList *list,
                                                  DBusWatch     *watch);
/** Function to be called in protected_change_watch() with refcount held */
typedef void        (* DBusWatchRemoveFunction)  (DBusWatchList *list,
                                                  DBusWatch     *watch);
/** Function to be called in protected_change_watch() with refcount held */
typedef void        (* DBusWatchToggleFunction)  (DBusWatchList *list,
                                                  DBusWatch     *watch,
                                                  dbus_bool_t    enabled);

static dbus_bool_t
protected_change_watch (DBusServer             *server,
                        DBusWatch              *watch,
                        DBusWatchAddFunction    add_function,
                        DBusWatchRemoveFunction remove_function,
                        DBusWatchToggleFunction toggle_function,
                        dbus_bool_t             enabled)
{
  DBusWatchList *watches;
  dbus_bool_t retval;
  
  HAVE_LOCK_CHECK (server);

  /* This isn't really safe or reasonable; a better pattern is the "do
   * everything, then drop lock and call out" one; but it has to be
   * propagated up through all callers
   */
  
  watches = server->watches;
  if (watches)
    {
      server->watches = NULL;
      _dbus_server_ref_unlocked (server);
      SERVER_UNLOCK (server);

      if (add_function)
        retval = (* add_function) (watches, watch);
      else if (remove_function)
        {
          retval = TRUE;
          (* remove_function) (watches, watch);
        }
      else
        {
          retval = TRUE;
          (* toggle_function) (watches, watch, enabled);
        }
      
      SERVER_LOCK (server);
      server->watches = watches;
      _dbus_server_unref_unlocked (server);

      return retval;
    }
  else
    return FALSE;
}

/**
 * Adds a watch for this server, chaining out to application-provided
 * watch handlers.
 *
 * @param server the server.
 * @param watch the watch to add.
 */
dbus_bool_t
_dbus_server_add_watch (DBusServer *server,
                        DBusWatch  *watch)
{
  HAVE_LOCK_CHECK (server);
  return protected_change_watch (server, watch,
                                 _dbus_watch_list_add_watch,
                                 NULL, NULL, FALSE);
}

/**
 * Removes a watch previously added with _dbus_server_remove_watch().
 *
 * @param server the server.
 * @param watch the watch to remove.
 */
void
_dbus_server_remove_watch  (DBusServer *server,
                            DBusWatch  *watch)
{
  HAVE_LOCK_CHECK (server);
  protected_change_watch (server, watch,
                          NULL,
                          _dbus_watch_list_remove_watch,
                          NULL, FALSE);
}

/**
 * Toggles all watch and notifies app via server's
 * DBusWatchToggledFunction if available.
 *
 * @param server the server.
 * @param enabled whether to enable or disable
 */
void
_dbus_server_toggle_all_watches (DBusServer  *server,
                                dbus_bool_t  enabled)
{
  _dbus_watch_list_toggle_all_watches (server->watches, enabled);
}

/** Function to be called in protected_change_timeout() with refcount held */
typedef dbus_bool_t (* DBusTimeoutAddFunction)    (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout);
/** Function to be called in protected_change_timeout() with refcount held */
typedef void        (* DBusTimeoutRemoveFunction) (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout);
/** Function to be called in protected_change_timeout() with refcount held */
typedef void        (* DBusTimeoutToggleFunction) (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout,
                                                   dbus_bool_t      enabled);


static dbus_bool_t
protected_change_timeout (DBusServer               *server,
                          DBusTimeout              *timeout,
                          DBusTimeoutAddFunction    add_function,
                          DBusTimeoutRemoveFunction remove_function,
                          DBusTimeoutToggleFunction toggle_function,
                          dbus_bool_t               enabled)
{
  DBusTimeoutList *timeouts;
  dbus_bool_t retval;
  
  HAVE_LOCK_CHECK (server);

  /* This isn't really safe or reasonable; a better pattern is the "do everything, then
   * drop lock and call out" one; but it has to be propagated up through all callers
   */
  
  timeouts = server->timeouts;
  if (timeouts)
    {
      server->timeouts = NULL;
      _dbus_server_ref_unlocked (server);
      SERVER_UNLOCK (server);

      if (add_function)
        retval = (* add_function) (timeouts, timeout);
      else if (remove_function)
        {
          retval = TRUE;
          (* remove_function) (timeouts, timeout);
        }
      else
        {
          retval = TRUE;
          (* toggle_function) (timeouts, timeout, enabled);
        }
      
      SERVER_LOCK (server);
      server->timeouts = timeouts;
      _dbus_server_unref_unlocked (server);

      return retval;
    }
  else
    return FALSE;
}

/**
 * Adds a timeout for this server, chaining out to
 * application-provided timeout handlers. The timeout should be
 * repeatedly handled with dbus_timeout_handle() at its given interval
 * until it is removed.
 *
 * @param server the server.
 * @param timeout the timeout to add.
 */
dbus_bool_t
_dbus_server_add_timeout (DBusServer  *server,
			  DBusTimeout *timeout)
{
  return protected_change_timeout (server, timeout,
                                   _dbus_timeout_list_add_timeout,
                                   NULL, NULL, FALSE);
}

/**
 * Removes a timeout previously added with _dbus_server_add_timeout().
 *
 * @param server the server.
 * @param timeout the timeout to remove.
 */
void
_dbus_server_remove_timeout (DBusServer  *server,
			     DBusTimeout *timeout)
{
  protected_change_timeout (server, timeout,
                            NULL,
                            _dbus_timeout_list_remove_timeout,
                            NULL, FALSE);
}

/**
 * Toggles a timeout and notifies app via server's
 * DBusTimeoutToggledFunction if available. It's an error to call this
 * function on a timeout that was not previously added.
 *
 * @param server the server.
 * @param timeout the timeout to toggle.
 * @param enabled whether to enable or disable
 */
void
_dbus_server_toggle_timeout (DBusServer  *server,
                             DBusTimeout *timeout,
                             dbus_bool_t  enabled)
{
  protected_change_timeout (server, timeout,
                            NULL, NULL,
                            _dbus_timeout_list_toggle_timeout,
                            enabled);
}


/**
 * Like dbus_server_ref() but does not acquire the lock (must already be held)
 *
 * @param server the server.
 */
void
_dbus_server_ref_unlocked (DBusServer *server)
{
  dbus_int32_t old_refcount;

  _dbus_assert (server != NULL);
  HAVE_LOCK_CHECK (server);

  old_refcount = _dbus_atomic_inc (&server->refcount);
  _dbus_assert (old_refcount > 0);
  _dbus_server_trace_ref (server, old_refcount, old_refcount + 1,
      "ref_unlocked");
}

/**
 * Like dbus_server_unref() but does not acquire the lock (must already be held)
 *
 * @param server the server.
 */
void
_dbus_server_unref_unlocked (DBusServer *server)
{
  dbus_int32_t old_refcount;

  /* Keep this in sync with dbus_server_unref */

  _dbus_assert (server != NULL);

  HAVE_LOCK_CHECK (server);

  old_refcount = _dbus_atomic_dec (&server->refcount);
  _dbus_assert (old_refcount > 0);

  _dbus_server_trace_ref (server, old_refcount, old_refcount - 1,
      "unref_unlocked");

  if (old_refcount == 1)
    {
      _dbus_assert (server->disconnected);
      
      SERVER_UNLOCK (server);
      
      _dbus_assert (server->vtable->finalize != NULL);
      
      (* server->vtable->finalize) (server);
    }
}

/** @} */

/**
 * @addtogroup DBusServer
 *
 * @{
 */


/**
 * @typedef DBusServer
 *
 * An opaque object representing a server that listens for
 * connections from other applications. Each time a connection
 * is made, a new DBusConnection is created and made available
 * via an application-provided DBusNewConnectionFunction.
 * The DBusNewConnectionFunction is provided with
 * dbus_server_set_new_connection_function().
 * 
 */

static const struct {
  DBusServerListenResult (* func) (DBusAddressEntry *entry,
                                   DBusServer      **server_p,
                                   DBusError        *error);
} listen_funcs[] = {
  { _dbus_server_listen_socket }
  , { _dbus_server_listen_platform_specific }
#ifdef DBUS_ENABLE_EMBEDDED_TESTS
  , { _dbus_server_listen_debug_pipe }
#endif
};

/**
 * Listens for new connections on the given address.  If there are
 * multiple semicolon-separated address entries in the address, tries
 * each one and listens on the first one that works.
 * 
 * Returns #NULL and sets error if listening fails for any reason.
 * Otherwise returns a new #DBusServer.
 * dbus_server_set_new_connection_function(),
 * dbus_server_set_watch_functions(), and
 * dbus_server_set_timeout_functions() should be called immediately to
 * render the server fully functional.
 *
 * To free the server, applications must call first
 * dbus_server_disconnect() and then dbus_server_unref().
 * 
 * @param address the address of this server.
 * @param error location to store reason for failure.
 * @returns a new #DBusServer, or #NULL on failure.
 * 
 */
DBusServer*
dbus_server_listen (const char     *address,
                    DBusError      *error)
{
  DBusServer *server;
  DBusAddressEntry **entries;
  int len, i;
  DBusError first_connect_error = DBUS_ERROR_INIT;
  dbus_bool_t handled_once;
  
  _dbus_return_val_if_fail (address != NULL, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);
  
  if (!dbus_parse_address (address, &entries, &len, error))
    return NULL;

  server = NULL;
  handled_once = FALSE;

  for (i = 0; i < len; i++)
    {
      int j;

      for (j = 0; j < (int) _DBUS_N_ELEMENTS (listen_funcs); ++j)
        {
          DBusServerListenResult result;
          DBusError tmp_error = DBUS_ERROR_INIT;

          result = (* listen_funcs[j].func) (entries[i],
                                             &server,
                                             &tmp_error);

          if (result == DBUS_SERVER_LISTEN_OK)
            {
              _dbus_assert (server != NULL);
              _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
              handled_once = TRUE;
              goto out;
            }
          else if (result == DBUS_SERVER_LISTEN_ADDRESS_ALREADY_USED)
            {
              _dbus_assert (server == NULL);
              _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
              dbus_set_error (error,
                       DBUS_ERROR_ADDRESS_IN_USE,
                       "Address '%s' already used",
                       dbus_address_entry_get_method (entries[0]));
              handled_once = TRUE;
              goto out;
            }
          else if (result == DBUS_SERVER_LISTEN_BAD_ADDRESS)
            {
              _dbus_assert (server == NULL);
              _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
              dbus_move_error (&tmp_error, error);
              handled_once = TRUE;
              goto out;
            }
          else if (result == DBUS_SERVER_LISTEN_NOT_HANDLED)
            {
              _dbus_assert (server == NULL);
              _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);

              /* keep trying addresses */
            }
          else if (result == DBUS_SERVER_LISTEN_DID_NOT_CONNECT)
            {
              _dbus_assert (server == NULL);
              _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
              if (!dbus_error_is_set (&first_connect_error))
                dbus_move_error (&tmp_error, &first_connect_error);
              else
                dbus_error_free (&tmp_error);

              handled_once = TRUE;
              
              /* keep trying addresses */
            }
          else
            {
              _dbus_assert_not_reached ("Unknown result in dbus_server_listen");
            }
        }

      _dbus_assert (server == NULL);
      _DBUS_ASSERT_ERROR_IS_CLEAR (error);
    }

 out:

  if (!handled_once)
    {
      _DBUS_ASSERT_ERROR_IS_CLEAR (error);
      if (len > 0)
        dbus_set_error (error,
                       DBUS_ERROR_BAD_ADDRESS,
                       "Unknown address type '%s'",
                       dbus_address_entry_get_method (entries[0]));
      else
        dbus_set_error (error,
                        DBUS_ERROR_BAD_ADDRESS,
                        "Empty address '%s'",
                        address);
    }
  
  dbus_address_entries_free (entries);

  if (server == NULL)
    {
      _dbus_assert (error == NULL || dbus_error_is_set (&first_connect_error) ||
                   dbus_error_is_set (error));
      
      if (error && dbus_error_is_set (error))
        {
          /* already set the error */
        }
      else
        {
          /* didn't set the error but either error should be
           * NULL or first_connect_error should be set.
           */
          _dbus_assert (error == NULL || dbus_error_is_set (&first_connect_error));
          dbus_move_error (&first_connect_error, error);
        }

      _DBUS_ASSERT_ERROR_IS_CLEAR (&first_connect_error); /* be sure we freed it */
      _DBUS_ASSERT_ERROR_IS_SET (error);

      return NULL;
    }
  else
    {
      dbus_error_free (&first_connect_error);
      _DBUS_ASSERT_ERROR_IS_CLEAR (error);
      return server;
    }
}

/**
 * Increments the reference count of a DBusServer.
 *
 * @param server the server.
 * @returns the server
 */
DBusServer *
dbus_server_ref (DBusServer *server)
{
  dbus_int32_t old_refcount;

  _dbus_return_val_if_fail (server != NULL, NULL);

  old_refcount = _dbus_atomic_inc (&server->refcount);

#ifndef DBUS_DISABLE_CHECKS
  if (_DBUS_UNLIKELY (old_refcount <= 0))
    {
      _dbus_atomic_dec (&server->refcount);
      _dbus_warn_return_if_fail (_DBUS_FUNCTION_NAME, "old_refcount > 0",
                                 __FILE__, __LINE__);
      return NULL;
    }
#endif

  _dbus_server_trace_ref (server, old_refcount, old_refcount + 1, "ref");

  return server;
}

/**
 * Decrements the reference count of a DBusServer.  Finalizes the
 * server if the reference count reaches zero.
 *
 * The server must be disconnected before the refcount reaches zero.
 *
 * @param server the server.
 */
void
dbus_server_unref (DBusServer *server)
{
  dbus_int32_t old_refcount;

  /* keep this in sync with unref_unlocked */

  _dbus_return_if_fail (server != NULL);

  old_refcount = _dbus_atomic_dec (&server->refcount);

#ifndef DBUS_DISABLE_CHECKS
  if (_DBUS_UNLIKELY (old_refcount <= 0))
    {
      /* undo side-effect first
       * please do not try to simplify the code here by using
       * _dbus_atomic_get(), why we don't use it is
       * because it issues another atomic operation even though
       * DBUS_DISABLE_CHECKS defined.
       * Bug: https://bugs.freedesktop.org/show_bug.cgi?id=68303
       */
      _dbus_atomic_inc (&server->refcount);
      _dbus_warn_return_if_fail (_DBUS_FUNCTION_NAME, "old_refcount > 0",
                                 __FILE__, __LINE__);
      return;
    }
#endif

  _dbus_server_trace_ref (server, old_refcount, old_refcount - 1, "unref");

  if (old_refcount == 1)
    {
      /* lock not held! */
      _dbus_assert (server->disconnected);
      
      _dbus_assert (server->vtable->finalize != NULL);
      
      (* server->vtable->finalize) (server);
    }
}

void
_dbus_server_disconnect_unlocked (DBusServer *server)
{
  _dbus_assert (server->vtable->disconnect != NULL);

  if (!server->disconnected)
    {
      /* this has to be first so recursive calls to disconnect don't happen */
      server->disconnected = TRUE;

      (* server->vtable->disconnect) (server);
    }
}

/**
 * Releases the server's address and stops listening for
 * new clients. If called more than once, only the first
 * call has an effect. Does not modify the server's
 * reference count.
 * 
 * @param server the server.
 */
void
dbus_server_disconnect (DBusServer *server)
{
  _dbus_return_if_fail (server != NULL);

  dbus_server_ref (server);
  SERVER_LOCK (server);

  _dbus_server_disconnect_unlocked (server);

  SERVER_UNLOCK (server);
  dbus_server_unref (server);
}

/**
 * Returns #TRUE if the server is still listening for new connections.
 *
 * @param server the server.
 */
dbus_bool_t
dbus_server_get_is_connected (DBusServer *server)
{
  dbus_bool_t retval;
  
  _dbus_return_val_if_fail (server != NULL, FALSE);

  SERVER_LOCK (server);
  retval = !server->disconnected;
  SERVER_UNLOCK (server);

  return retval;
}

/**
 * Returns the address of the server, as a newly-allocated
 * string which must be freed by the caller.
 *
 * @param server the server
 * @returns the address or #NULL if no memory
 */
char*
dbus_server_get_address (DBusServer *server)
{
  char *retval;
  
  _dbus_return_val_if_fail (server != NULL, NULL);

  SERVER_LOCK (server);
  retval = _dbus_strdup (server->address);
  SERVER_UNLOCK (server);

  return retval;
}

/**
 * Returns the unique ID of the server, as a newly-allocated
 * string which must be freed by the caller. This ID is
 * normally used by clients to tell when two #DBusConnection
 * would be equivalent (because the server address passed
 * to dbus_connection_open() will have the same guid in the
 * two cases). dbus_connection_open() can re-use an existing
 * connection with the same ID instead of opening a new
 * connection.
 *
 * This is an ID unique to each #DBusServer. Remember that
 * a #DBusServer represents only one mode of connecting,
 * so e.g. a bus daemon can listen on multiple addresses
 * which will mean it has multiple #DBusServer each with
 * their own ID.
 *
 * The ID is not a UUID in the sense of RFC4122; the details
 * are explained in the D-Bus specification.
 *
 * @param server the server
 * @returns the id of the server or #NULL if no memory
 */
char*
dbus_server_get_id (DBusServer *server)
{
  char *retval;
  
  _dbus_return_val_if_fail (server != NULL, NULL);

  SERVER_LOCK (server);
  retval = NULL;
  _dbus_string_copy_data (&server->guid_hex, &retval);
  SERVER_UNLOCK (server);

  return retval;
}

/**
 * Sets a function to be used for handling new connections.  The given
 * function is passed each new connection as the connection is
 * created. If the new connection function increments the connection's
 * reference count, the connection will stay alive. Otherwise, the
 * connection will be unreferenced and closed. The new connection
 * function may also close the connection itself, which is considered
 * good form if the connection is not wanted.
 *
 * The connection here is private in the sense of
 * dbus_connection_open_private(), so if the new connection function
 * keeps a reference it must arrange for the connection to be closed.
 * i.e. libdbus does not own this connection once the new connection
 * function takes a reference.
 *
 * @param server the server.
 * @param function a function to handle new connections.
 * @param data data to pass to the new connection handler.
 * @param free_data_function function to free the data.
 */
void
dbus_server_set_new_connection_function (DBusServer                *server,
                                         DBusNewConnectionFunction  function,
                                         void                      *data,
                                         DBusFreeFunction           free_data_function)
{
  DBusFreeFunction old_free_function;
  void *old_data;
  
  _dbus_return_if_fail (server != NULL);

  SERVER_LOCK (server);
  old_free_function = server->new_connection_free_data_function;
  old_data = server->new_connection_data;
  
  server->new_connection_function = function;
  server->new_connection_data = data;
  server->new_connection_free_data_function = free_data_function;
  SERVER_UNLOCK (server);
    
  if (old_free_function != NULL)
    (* old_free_function) (old_data);
}

/**
 * Sets the watch functions for the server. These functions are
 * responsible for making the application's main loop aware of file
 * descriptors that need to be monitored for events.
 *
 * This function behaves exactly like dbus_connection_set_watch_functions();
 * see the documentation for that routine.
 *
 * @param server the server.
 * @param add_function function to begin monitoring a new descriptor.
 * @param remove_function function to stop monitoring a descriptor.
 * @param toggled_function function to notify when the watch is enabled/disabled
 * @param data data to pass to add_function and remove_function.
 * @param free_data_function function to be called to free the data.
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_server_set_watch_functions (DBusServer              *server,
                                 DBusAddWatchFunction     add_function,
                                 DBusRemoveWatchFunction  remove_function,
                                 DBusWatchToggledFunction toggled_function,
                                 void                    *data,
                                 DBusFreeFunction         free_data_function)
{
  dbus_bool_t result;
  DBusWatchList *watches;
  
  _dbus_return_val_if_fail (server != NULL, FALSE);

  SERVER_LOCK (server);
  watches = server->watches;
  server->watches = NULL;
  if (watches)
    {
      SERVER_UNLOCK (server);
      result = _dbus_watch_list_set_functions (watches,
                                               add_function,
                                               remove_function,
                                               toggled_function,
                                               data,
                                               free_data_function);
      SERVER_LOCK (server);
    }
  else
    {
      _dbus_warn_check_failed ("Re-entrant call to %s", _DBUS_FUNCTION_NAME);
      result = FALSE;
    }
  server->watches = watches;
  SERVER_UNLOCK (server);
  
  return result;
}

/**
 * Sets the timeout functions for the server. These functions are
 * responsible for making the application's main loop aware of timeouts.
 *
 * This function behaves exactly like dbus_connection_set_timeout_functions();
 * see the documentation for that routine.
 *
 * @param server the server.
 * @param add_function function to add a timeout.
 * @param remove_function function to remove a timeout.
 * @param toggled_function function to notify when the timeout is enabled/disabled
 * @param data data to pass to add_function and remove_function.
 * @param free_data_function function to be called to free the data.
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_server_set_timeout_functions (DBusServer                *server,
				   DBusAddTimeoutFunction     add_function,
				   DBusRemoveTimeoutFunction  remove_function,
                                   DBusTimeoutToggledFunction toggled_function,
				   void                      *data,
				   DBusFreeFunction           free_data_function)
{
  dbus_bool_t result;
  DBusTimeoutList *timeouts;
  
  _dbus_return_val_if_fail (server != NULL, FALSE);

  SERVER_LOCK (server);
  timeouts = server->timeouts;
  server->timeouts = NULL;
  if (timeouts)
    {
      SERVER_UNLOCK (server);
      result = _dbus_timeout_list_set_functions (timeouts,
                                                 add_function,
                                                 remove_function,
                                                 toggled_function,
                                                 data,
                                                 free_data_function);
      SERVER_LOCK (server);
    }
  else
    {
      _dbus_warn_check_failed ("Re-entrant call to %s", _DBUS_FUNCTION_NAME);
      result = FALSE;
    }
  server->timeouts = timeouts;
  SERVER_UNLOCK (server);
  
  return result;
}

/**
 * Sets the authentication mechanisms that this server offers to
 * clients, as a #NULL-terminated array of mechanism names. This
 * function only affects connections created <em>after</em> it is
 * called.  Pass #NULL instead of an array to use all available
 * mechanisms (this is the default behavior).
 *
 * The D-Bus specification describes some of the supported mechanisms.
 *
 * @param server the server
 * @param mechanisms #NULL-terminated array of mechanisms
 * @returns #FALSE if no memory
 */
dbus_bool_t
dbus_server_set_auth_mechanisms (DBusServer  *server,
                                 const char **mechanisms)
{
  char **copy;

  _dbus_return_val_if_fail (server != NULL, FALSE);

  SERVER_LOCK (server);
  
  if (mechanisms != NULL)
    {
      copy = _dbus_dup_string_array (mechanisms);
      if (copy == NULL)
        {
          SERVER_UNLOCK (server);
          return FALSE;
        }
    }
  else
    copy = NULL;

  dbus_free_string_array (server->auth_mechanisms);
  server->auth_mechanisms = copy;

  SERVER_UNLOCK (server);
  
  return TRUE;
}

static DBusDataSlotAllocator slot_allocator =
  _DBUS_DATA_SLOT_ALLOCATOR_INIT (_DBUS_LOCK_NAME (server_slots));

/**
 * Allocates an integer ID to be used for storing application-specific
 * data on any DBusServer. The allocated ID may then be used
 * with dbus_server_set_data() and dbus_server_get_data().
 * The slot must be initialized with -1. If a nonnegative
 * slot is passed in, the refcount is incremented on that
 * slot, rather than creating a new slot.
 *  
 * The allocated slot is global, i.e. all DBusServer objects will have
 * a slot with the given integer ID reserved.
 *
 * @param slot_p address of global variable storing the slot ID
 * @returns #FALSE on no memory
 */
dbus_bool_t
dbus_server_allocate_data_slot (dbus_int32_t *slot_p)
{
  return _dbus_data_slot_allocator_alloc (&slot_allocator,
                                          slot_p);
}

/**
 * Deallocates a global ID for server data slots.
 * dbus_server_get_data() and dbus_server_set_data()
 * may no longer be used with this slot.
 * Existing data stored on existing DBusServer objects
 * will be freed when the server is finalized,
 * but may not be retrieved (and may only be replaced
 * if someone else reallocates the slot).
 *
 * @param slot_p address of the slot to deallocate
 */
void
dbus_server_free_data_slot (dbus_int32_t *slot_p)
{
  _dbus_return_if_fail (*slot_p >= 0);
  
  _dbus_data_slot_allocator_free (&slot_allocator, slot_p);
}

/**
 * Stores a pointer on a DBusServer, along
 * with an optional function to be used for freeing
 * the data when the data is set again, or when
 * the server is finalized. The slot number
 * must have been allocated with dbus_server_allocate_data_slot().
 *
 * @param server the server
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
dbus_server_set_data (DBusServer       *server,
                      int               slot,
                      void             *data,
                      DBusFreeFunction  free_data_func)
{
  DBusFreeFunction old_free_func;
  void *old_data;
  dbus_bool_t retval;

  _dbus_return_val_if_fail (server != NULL, FALSE);

  SERVER_LOCK (server);
  
  retval = _dbus_data_slot_list_set (&slot_allocator,
                                     &server->slot_list,
                                     slot, data, free_data_func,
                                     &old_free_func, &old_data);


  SERVER_UNLOCK (server);
  
  if (retval)
    {
      /* Do the actual free outside the server lock */
      if (old_free_func)
        (* old_free_func) (old_data);
    }

  return retval;
}

/**
 * Retrieves data previously set with dbus_server_set_data().
 * The slot must still be allocated (must not have been freed).
 *
 * @param server the server
 * @param slot the slot to get data from
 * @returns the data, or #NULL if not found
 */
void*
dbus_server_get_data (DBusServer   *server,
                      int           slot)
{
  void *res;

  _dbus_return_val_if_fail (server != NULL, NULL);
  
  SERVER_LOCK (server);
  
  res = _dbus_data_slot_list_get (&slot_allocator,
                                  &server->slot_list,
                                  slot);

  SERVER_UNLOCK (server);
  
  return res;
}

/** @} */
