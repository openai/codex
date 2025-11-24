/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-protected.h Used by subclasses of DBusServer object (internal to D-Bus implementation)
 *
 * Copyright (C) 2002  Red Hat Inc.
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
#ifndef DBUS_SERVER_PROTECTED_H
#define DBUS_SERVER_PROTECTED_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-threads-internal.h>
#include <dbus/dbus-server.h>
#include <dbus/dbus-address.h>
#include <dbus/dbus-timeout.h>
#include <dbus/dbus-watch.h>
#include <dbus/dbus-resources.h>
#include <dbus/dbus-dataslot.h>
#include <dbus/dbus-string.h>

DBUS_BEGIN_DECLS

typedef struct DBusServerVTable DBusServerVTable;

/**
 * Virtual table to be implemented by all server "subclasses"
 */
struct DBusServerVTable
{
  void        (* finalize)      (DBusServer *server);
  /**< The finalize method must free the server. */
  
  void        (* disconnect)    (DBusServer *server);
  /**< Disconnect this server. */
};

/**
 * @ingroup DBusServerInternals
 * Internals of DBusServer object
 */
struct DBusServer
{
  DBusAtomic refcount;                        /**< Reference count. */
  const DBusServerVTable *vtable;             /**< Virtual methods for this instance. */
  DBusRMutex *mutex;                          /**< Lock on the server object */

  DBusGUID guid;                              /**< Globally unique ID of server */

  DBusString guid_hex;                        /**< Hex-encoded version of GUID */
  
  DBusWatchList *watches;                     /**< Our watches */
  DBusTimeoutList *timeouts;                  /**< Our timeouts */  

  char *address;                              /**< Address this server is listening on. */
  dbus_bool_t published_address;              /**< flag which indicates that server has published its bus address. */

  int max_connections;                        /**< Max number of connections allowed at once. */

  DBusDataSlotList slot_list;   /**< Data stored by allocated integer ID */
  
  DBusNewConnectionFunction  new_connection_function;
  /**< Callback to invoke when a new connection is created. */
  void *new_connection_data;
  /**< Data for new connection callback */
  DBusFreeFunction new_connection_free_data_function;
  /**< Callback to invoke to free new_connection_data
   * when server is finalized or data is replaced.
   */

  char **auth_mechanisms; /**< Array of allowed authentication mechanisms */
  
  unsigned int disconnected : 1;              /**< TRUE if we are disconnected. */

#ifndef DBUS_DISABLE_CHECKS
  unsigned int have_server_lock : 1; /**< Does someone have the server mutex locked */
#endif
};

dbus_bool_t _dbus_server_init_base      (DBusServer             *server,
                                         const DBusServerVTable *vtable,
                                         const DBusString       *address,
                                         DBusError              *error);
void        _dbus_server_finalize_base  (DBusServer             *server);
void        _dbus_server_disconnect_unlocked (DBusServer        *server);
dbus_bool_t _dbus_server_add_watch      (DBusServer             *server,
                                         DBusWatch              *watch);
void        _dbus_server_remove_watch   (DBusServer             *server,
                                         DBusWatch              *watch);
DBUS_PRIVATE_EXPORT
void        _dbus_server_toggle_all_watches (DBusServer         *server,
                                             dbus_bool_t         enabled);
dbus_bool_t _dbus_server_add_timeout    (DBusServer             *server,
                                         DBusTimeout            *timeout);
void        _dbus_server_remove_timeout (DBusServer             *server,
                                         DBusTimeout            *timeout);
void        _dbus_server_toggle_timeout (DBusServer             *server,
                                         DBusTimeout            *timeout,
                                         dbus_bool_t             enabled);

DBUS_PRIVATE_EXPORT
void        _dbus_server_ref_unlocked   (DBusServer             *server);
DBUS_PRIVATE_EXPORT
void        _dbus_server_unref_unlocked (DBusServer             *server);

typedef enum
{
  DBUS_SERVER_LISTEN_NOT_HANDLED, /**< we aren't in charge of this address type */
  DBUS_SERVER_LISTEN_OK,          /**< we set up the listen */
  DBUS_SERVER_LISTEN_BAD_ADDRESS, /**< malformed address */
  DBUS_SERVER_LISTEN_DID_NOT_CONNECT, /**< well-formed address but failed to set it up */
  DBUS_SERVER_LISTEN_ADDRESS_ALREADY_USED /**< address is already used */
} DBusServerListenResult;

DBusServerListenResult _dbus_server_listen_platform_specific (DBusAddressEntry  *entry,
                                                              DBusServer       **server_p,
                                                              DBusError         *error);

#ifdef DBUS_ENABLE_VERBOSE_MODE
void _dbus_server_trace_ref (DBusServer *server,
    int old_refcount,
    int new_refcount,
    const char *why);
#else
#define _dbus_server_trace_ref(s,o,n,w) \
  do \
  {\
    (void) (o); \
    (void) (n); \
  } while (0)
#endif

#ifdef DBUS_DISABLE_CHECKS
#define TOOK_LOCK_CHECK(server)
#define RELEASING_LOCK_CHECK(server)
#define HAVE_LOCK_CHECK(server)
#else
#define TOOK_LOCK_CHECK(server) do {                \
    _dbus_assert (!(server)->have_server_lock); \
    (server)->have_server_lock = TRUE;          \
  } while (0)
#define RELEASING_LOCK_CHECK(server) do {            \
    _dbus_assert ((server)->have_server_lock);   \
    (server)->have_server_lock = FALSE;          \
  } while (0)
#define HAVE_LOCK_CHECK(server)        _dbus_assert ((server)->have_server_lock)
/* A "DO_NOT_HAVE_LOCK_CHECK" is impossible since we need the lock to check the flag */
#endif

#define TRACE_LOCKS 0

#define SERVER_LOCK(server)   do {                                              \
    if (TRACE_LOCKS) { _dbus_verbose ("LOCK\n"); }   \
    _dbus_rmutex_lock ((server)->mutex);                                        \
    TOOK_LOCK_CHECK (server);                                                   \
  } while (0)

#define SERVER_UNLOCK(server) do {                                                      \
    if (TRACE_LOCKS) { _dbus_verbose ("UNLOCK\n");  }        \
    RELEASING_LOCK_CHECK (server);                                                      \
    _dbus_rmutex_unlock ((server)->mutex);                                              \
  } while (0)

DBUS_END_DECLS

#endif /* DBUS_SERVER_PROTECTED_H */
