/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-marshal-byteswap.h  Swap a block of marshaled data
 *
 * Copyright (C) 2005 Red Hat, Inc.
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

#ifndef DBUS_MARSHAL_BYTESWAP_H
#define DBUS_MARSHAL_BYTESWAP_H

#include <dbus/dbus-protocol.h>
#include <dbus/dbus-marshal-recursive.h>

DBUS_PRIVATE_EXPORT
void _dbus_marshal_byteswap (const DBusString *signature,
                             int               signature_start,
                             int               old_byte_order,
                             int               new_byte_order,
                             DBusString       *value_str,
                             int               value_pos);

#endif /* DBUS_MARSHAL_BYTESWAP_H */
