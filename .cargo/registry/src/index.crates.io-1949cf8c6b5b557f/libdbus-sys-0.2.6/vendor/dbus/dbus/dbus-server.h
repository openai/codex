/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server.h DBusServer object
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
#if !defined (DBUS_INSIDE_DBUS_H) && !defined (DBUS_COMPILATION)
#error "Only <dbus/dbus.h> can be included directly, this file may disappear or change contents."
#endif

#ifndef DBUS_SERVER_H
#define DBUS_SERVER_H

#include <dbus/dbus-errors.h>
#include <dbus/dbus-macros.h>
#include <dbus/dbus-message.h>
#include <dbus/dbus-connection.h>
#include <dbus/dbus-protocol.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusServer
 * @{
 */

typedef struct DBusServer DBusServer;

/** Called when a new connection to the server is available. Must reference and save the new
 * connection, or close the new connection. Set with dbus_server_set_new_connection_function().
 */
typedef void (* DBusNewConnectionFunction) (DBusServer     *server,
                                            DBusConnection *new_connection,
                                            void           *data);

DBUS_EXPORT
DBusServer* dbus_server_listen           (const char     *address,
                                          DBusError      *error);
DBUS_EXPORT
DBusServer* dbus_server_ref              (DBusServer     *server);
DBUS_EXPORT
void        dbus_server_unref            (DBusServer     *server);
DBUS_EXPORT
void        dbus_server_disconnect       (DBusServer     *server);
DBUS_EXPORT
dbus_bool_t dbus_server_get_is_connected (DBusServer     *server);
DBUS_EXPORT
char*       dbus_server_get_address      (DBusServer     *server);
DBUS_EXPORT
char*       dbus_server_get_id           (DBusServer     *server);
DBUS_EXPORT
void        dbus_server_set_new_connection_function (DBusServer                *server,
                                                     DBusNewConnectionFunction  function,
                                                     void                      *data,
                                                     DBusFreeFunction           free_data_function);
DBUS_EXPORT
dbus_bool_t dbus_server_set_watch_functions         (DBusServer                *server,
                                                     DBusAddWatchFunction       add_function,
                                                     DBusRemoveWatchFunction    remove_function,
                                                     DBusWatchToggledFunction   toggled_function,
                                                     void                      *data,
                                                     DBusFreeFunction           free_data_function);
DBUS_EXPORT
dbus_bool_t dbus_server_set_timeout_functions       (DBusServer                *server,
                                                     DBusAddTimeoutFunction     add_function,
                                                     DBusRemoveTimeoutFunction  remove_function,
                                                     DBusTimeoutToggledFunction toggled_function,
                                                     void                      *data,
                                                     DBusFreeFunction           free_data_function);
DBUS_EXPORT
dbus_bool_t dbus_server_set_auth_mechanisms         (DBusServer                *server,
                                                     const char               **mechanisms);

DBUS_EXPORT
dbus_bool_t dbus_server_allocate_data_slot (dbus_int32_t     *slot_p);
DBUS_EXPORT
void        dbus_server_free_data_slot     (dbus_int32_t     *slot_p);
DBUS_EXPORT
dbus_bool_t dbus_server_set_data           (DBusServer       *server,
                                            int               slot,
                                            void             *data,
                                            DBusFreeFunction  free_data_func);
DBUS_EXPORT
void*       dbus_server_get_data           (DBusServer       *server,
                                            int               slot);

/**
 * Clear a variable or struct member that contains a #DBusServer.
 * If it does not contain #NULL, the server that was previously
 * there is unreferenced with dbus_server_unref().
 *
 * This is very similar to dbus_clear_connection(): see that function
 * for more details.
 *
 * @param pointer_to_server A pointer to a variable or struct member.
 * pointer_to_server must not be #NULL, but *pointer_to_server
 * may be #NULL.
 */
static inline void
dbus_clear_server (DBusServer **pointer_to_server)
{
  _dbus_clear_pointer_impl (DBusServer, pointer_to_server, dbus_server_unref);
}

/** @} */

DBUS_END_DECLS

#endif /* DBUS_SERVER_H */
