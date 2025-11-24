/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-nonce.h  Nonce handling functions used by nonce-tcp (internal to D-Bus implementation)
 *
 * Copyright (C) 2009 Klaralvdalens Datakonsult AB, a KDAB Group company, info@kdab.net
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
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA
 *
 */
#ifndef DBUS_NONCE_H
#define DBUS_NONCE_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-sysdeps.h>

DBUS_BEGIN_DECLS

typedef struct DBusNonceFile DBusNonceFile;

// server

dbus_bool_t _dbus_noncefile_create (DBusNonceFile **noncefile_out,
                                    DBusError *error);

dbus_bool_t _dbus_noncefile_delete (DBusNonceFile **noncefile_location,
                                    DBusError *error);

dbus_bool_t _dbus_noncefile_check_nonce (DBusSocket fd,
                                         const DBusNonceFile *noncefile,
                                         DBusError *error);

const DBusString* _dbus_noncefile_get_path (const DBusNonceFile *noncefile);

DBusSocket _dbus_accept_with_noncefile(DBusSocket listen_fd,
                                 const DBusNonceFile *noncefile);

// shared

dbus_bool_t _dbus_read_nonce (const DBusString *fname,
                              DBusString *nonce,
                              DBusError *error);

// client

dbus_bool_t _dbus_send_nonce (DBusSocket fd,
                              const DBusString *noncefile,
                              DBusError *error);

DBUS_END_DECLS

#endif /* DBUS_NONCE_H */
