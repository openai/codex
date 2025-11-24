/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-bus.h  Convenience functions for communicating with the bus.
 *
 * Copyright (C) 2003  CodeFactory AB
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

#ifndef DBUS_BUS_H
#define DBUS_BUS_H

#include <dbus/dbus-connection.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusBus
 * @{
 */

DBUS_EXPORT
DBusConnection *dbus_bus_get              (DBusBusType     type,
					   DBusError      *error);
DBUS_EXPORT
DBusConnection *dbus_bus_get_private      (DBusBusType     type,
					   DBusError      *error);

DBUS_EXPORT
dbus_bool_t     dbus_bus_register         (DBusConnection *connection,
					   DBusError      *error);
DBUS_EXPORT
dbus_bool_t     dbus_bus_set_unique_name  (DBusConnection *connection,
					   const char     *unique_name);
DBUS_EXPORT
const char*     dbus_bus_get_unique_name  (DBusConnection *connection);
DBUS_EXPORT
unsigned long   dbus_bus_get_unix_user    (DBusConnection *connection,
			                   const char     *name,
                                           DBusError      *error);
DBUS_EXPORT
char*           dbus_bus_get_id           (DBusConnection *connection,
                                           DBusError      *error);
DBUS_EXPORT
int             dbus_bus_request_name     (DBusConnection *connection,
					   const char     *name,
					   unsigned int    flags,
					   DBusError      *error);
DBUS_EXPORT
int             dbus_bus_release_name     (DBusConnection *connection,
					   const char     *name,
					   DBusError      *error);
DBUS_EXPORT
dbus_bool_t     dbus_bus_name_has_owner   (DBusConnection *connection,
					   const char     *name,
					   DBusError      *error);

DBUS_EXPORT
dbus_bool_t     dbus_bus_start_service_by_name (DBusConnection *connection,
                                                const char     *name,
                                                dbus_uint32_t   flags,
                                                dbus_uint32_t  *reply,
                                                DBusError      *error);

DBUS_EXPORT
void            dbus_bus_add_match        (DBusConnection *connection,
                                           const char     *rule,
                                           DBusError      *error);
DBUS_EXPORT
void            dbus_bus_remove_match     (DBusConnection *connection,
                                           const char     *rule,
                                           DBusError      *error);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_BUS_H */
