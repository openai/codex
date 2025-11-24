/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-transport-socket.h Socket subclasses of DBusTransport
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
#ifndef DBUS_TRANSPORT_SOCKET_H
#define DBUS_TRANSPORT_SOCKET_H

#include <dbus/dbus-transport-protected.h>

DBUS_BEGIN_DECLS

DBusTransport*          _dbus_transport_new_for_socket     (DBusSocket         fd,
                                                            const DBusString  *server_guid,
                                                            const DBusString  *address);
DBusTransport*          _dbus_transport_new_for_tcp_socket (const char        *host,
                                                            const char        *port,
                                                            const char        *family,
                                                            const char        *noncefile,
                                                            DBusError         *error);
DBusTransportOpenResult _dbus_transport_open_socket        (DBusAddressEntry  *entry,
                                                            DBusTransport    **transport_p,
                                                            DBusError         *error);



DBUS_END_DECLS

#endif /* DBUS_TRANSPORT_SOCKET_H */
