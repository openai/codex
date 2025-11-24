/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* utils.h  General utility functions
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

#ifndef BUS_UTILS_H
#define BUS_UTILS_H

#include <dbus/dbus.h>

extern const char bus_no_memory_message[];
#define BUS_SET_OOM(error) dbus_set_error_const ((error), DBUS_ERROR_NO_MEMORY, bus_no_memory_message)

void        bus_connection_dispatch_all_messages (DBusConnection *connection);
dbus_bool_t bus_connection_dispatch_one_message  (DBusConnection *connection);

#endif /* BUS_UTILS_H */
