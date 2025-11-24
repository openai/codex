/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-server-debug-pipe.h In-proc debug server implementation
 *
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2003  Red Hat, Inc.
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
#ifndef DBUS_SERVER_DEBUG_PIPE_H
#define DBUS_SERVER_DEBUG_PIPE_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-server-protected.h>
#include <dbus/dbus-transport-protected.h>

DBUS_BEGIN_DECLS

DBusServer*             _dbus_server_debug_pipe_new     (const char        *server_name,
                                                         DBusError         *error);
DBusTransport*          _dbus_transport_debug_pipe_new  (const char        *server_name,
                                                         DBusError         *error);
DBusServerListenResult  _dbus_server_listen_debug_pipe  (DBusAddressEntry  *entry,
                                                         DBusServer       **server_p,
                                                         DBusError         *error);
DBusTransportOpenResult _dbus_transport_open_debug_pipe (DBusAddressEntry  *entry,
                                                         DBusTransport    **transport_p,
                                                         DBusError         *error);


DBUS_END_DECLS

#endif /* DBUS_SERVER_DEBUG_PIPE_H */
