/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-socket.h Server implementation for sockets
 *
 * Copyright (C) 2002, 2006  Red Hat Inc.
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
#ifndef DBUS_SERVER_SOCKET_H
#define DBUS_SERVER_SOCKET_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-server-protected.h>
#include <dbus/dbus-nonce.h>

DBUS_BEGIN_DECLS

DBusServer* _dbus_server_new_for_socket           (DBusSocket       *fds,
                                                   int               n_fds,
                                                   const DBusString *address,
                                                   DBusNonceFile    *noncefile,
                                                   DBusError        *error);
DBusServer* _dbus_server_new_for_autolaunch       (const DBusString *address,
                                                   DBusError        *error);
DBUS_PRIVATE_EXPORT
DBusServer* _dbus_server_new_for_tcp_socket       (const char       *host,
                                                   const char       *bind,
                                                   const char       *port,
                                                   const char       *family,
                                                   DBusError        *error,
                                                   dbus_bool_t      use_nonce);
DBusServerListenResult _dbus_server_listen_socket (DBusAddressEntry  *entry,
                                                   DBusServer       **server_p,
                                                   DBusError         *error);


void _dbus_server_socket_own_filename (DBusServer *server,
                                       char       *filename);

DBUS_END_DECLS

#endif /* DBUS_SERVER_SOCKET_H */
