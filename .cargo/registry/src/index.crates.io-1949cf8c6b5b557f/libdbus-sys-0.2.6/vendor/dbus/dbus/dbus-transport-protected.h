/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-transport-protected.h Used by subclasses of DBusTransport object (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2004  Red Hat Inc.
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
#ifndef DBUS_TRANSPORT_PROTECTED_H
#define DBUS_TRANSPORT_PROTECTED_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-transport.h>
#include <dbus/dbus-message-internal.h>
#include <dbus/dbus-auth.h>
#include <dbus/dbus-resources.h>

DBUS_BEGIN_DECLS

typedef struct DBusTransportVTable DBusTransportVTable;

/**
 * The virtual table that must be implemented to
 * create a new kind of transport.
 */
struct DBusTransportVTable
{
  void        (* finalize)              (DBusTransport *transport);
  /**< The finalize method must free the transport. */

  dbus_bool_t (* handle_watch)          (DBusTransport *transport,
                                         DBusWatch     *watch,
                                         unsigned int   flags);
  /**< The handle_watch method handles reading/writing
   * data as indicated by the flags.
   */

  void        (* disconnect)            (DBusTransport *transport);
  /**< Disconnect this transport. */

  dbus_bool_t (* connection_set)        (DBusTransport *transport);
  /**< Called when transport->connection has been filled in */

  void        (* do_iteration)          (DBusTransport *transport,
                                         unsigned int   flags,
                                         int            timeout_milliseconds);
  /**< Called to do a single "iteration" (block on select/poll
   * followed by reading or writing data).
   */

  void        (* live_messages_changed) (DBusTransport *transport);
  /**< Outstanding messages counter changed */

  dbus_bool_t (* get_socket_fd) (DBusTransport *transport,
                                 DBusSocket    *fd_p);
  /**< Get socket file descriptor */
};

/**
 * Object representing a transport such as a socket.
 * A transport can shuttle messages from point A to point B,
 * and is the backend for a #DBusConnection.
 *
 */
struct DBusTransport
{
  int refcount;                               /**< Reference count. */

  const DBusTransportVTable *vtable;          /**< Virtual methods for this instance. */

  DBusConnection *connection;                 /**< Connection owning this transport. */

  DBusMessageLoader *loader;                  /**< Message-loading buffer. */

  DBusAuth *auth;                             /**< Authentication conversation */

  DBusCredentials *credentials;               /**< Credentials of other end read from the socket */  

  long max_live_messages_size;                /**< Max total size of received messages. */
  long max_live_messages_unix_fds;            /**< Max total unix fds of received messages. */

  DBusCounter *live_messages;                 /**< Counter for size/unix fds of all live messages. */

  char *address;                              /**< Address of the server we are connecting to (#NULL for the server side of a transport) */

  char *expected_guid;                        /**< GUID we expect the server to have, #NULL on server side or if we don't have an expectation */
  
  DBusAllowUnixUserFunction unix_user_function; /**< Function for checking whether a user is authorized. */
  void *unix_user_data;                         /**< Data for unix_user_function */
  
  DBusFreeFunction free_unix_user_data;         /**< Function to free unix_user_data */

  DBusAllowWindowsUserFunction windows_user_function; /**< Function for checking whether a user is authorized. */
  void *windows_user_data;                            /**< Data for windows_user_function */
  
  DBusFreeFunction free_windows_user_data;            /**< Function to free windows_user_data */
  
  unsigned int disconnected : 1;              /**< #TRUE if we are disconnected. */
  unsigned int authenticated : 1;             /**< Cache of auth state; use _dbus_transport_peek_is_authenticated() to query value */
  unsigned int send_credentials_pending : 1;  /**< #TRUE if we need to send credentials */
  unsigned int receive_credentials_pending : 1; /**< #TRUE if we need to receive credentials */
  unsigned int is_server : 1;                 /**< #TRUE if on the server side */
  unsigned int unused_bytes_recovered : 1;    /**< #TRUE if we've recovered unused bytes from auth */
  unsigned int allow_anonymous : 1;           /**< #TRUE if an anonymous client can connect */
};

dbus_bool_t _dbus_transport_init_base     (DBusTransport             *transport,
                                           const DBusTransportVTable *vtable,
                                           const DBusString          *server_guid,
                                           const DBusString          *address);
void        _dbus_transport_finalize_base (DBusTransport             *transport);


typedef enum
{
  DBUS_TRANSPORT_OPEN_NOT_HANDLED,    /**< we aren't in charge of this address type */
  DBUS_TRANSPORT_OPEN_OK,             /**< we set up the listen */
  DBUS_TRANSPORT_OPEN_BAD_ADDRESS,    /**< malformed address */
  DBUS_TRANSPORT_OPEN_DID_NOT_CONNECT /**< well-formed address but failed to set it up */
} DBusTransportOpenResult;

DBusTransportOpenResult _dbus_transport_open_platform_specific (DBusAddressEntry  *entry,
                                                                DBusTransport    **transport_p,
                                                                DBusError         *error);

#define DBUS_TRANSPORT_CAN_SEND_UNIX_FD(x)      \
  _dbus_auth_get_unix_fd_negotiated((x)->auth)

DBUS_END_DECLS

#endif /* DBUS_TRANSPORT_PROTECTED_H */
