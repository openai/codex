/* containers.h - restricted bus servers for containers
 *
 * Copyright © 2017 Collabora Ltd.
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
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA
 * 02110-1301 USA
 */

#ifndef BUS_CONTAINERS_H
#define BUS_CONTAINERS_H

#include "bus.h"

#include <dbus/dbus-macros.h>

BusContainers        *bus_containers_new           (void);
BusContainers        *bus_containers_ref           (BusContainers *self);
void                  bus_containers_unref         (BusContainers *self);
void                  bus_containers_stop_listening (BusContainers *self);

dbus_bool_t bus_containers_handle_add_server          (DBusConnection  *connection,
                                                       BusTransaction  *transaction,
                                                       DBusMessage     *message,
                                                       DBusError       *error);
dbus_bool_t bus_containers_handle_stop_instance       (DBusConnection  *connection,
                                                       BusTransaction  *transaction,
                                                       DBusMessage     *message,
                                                       DBusError       *error);
dbus_bool_t bus_containers_handle_stop_listening      (DBusConnection  *connection,
                                                       BusTransaction  *transaction,
                                                       DBusMessage     *message,
                                                       DBusError       *error);
dbus_bool_t bus_containers_handle_get_instance_info   (DBusConnection  *connection,
                                                       BusTransaction  *transaction,
                                                       DBusMessage     *message,
                                                       DBusError       *error);
dbus_bool_t bus_containers_handle_get_connection_instance (DBusConnection *connection,
                                                           BusTransaction *transaction,
                                                           DBusMessage    *message,
                                                           DBusError      *error);
dbus_bool_t bus_containers_handle_request_header      (DBusConnection  *connection,
                                                       BusTransaction  *transaction,
                                                       DBusMessage     *message,
                                                       DBusError       *error);
dbus_bool_t bus_containers_supported_arguments_getter (BusContext      *context,
                                                       DBusMessageIter *var_iter);

void        bus_containers_remove_connection          (BusContainers *self,
                                                       DBusConnection *connection);
dbus_bool_t bus_containers_connection_is_contained    (DBusConnection *connection,
                                                       const char **path,
                                                       const char **type,
                                                       const char **name);

static inline void
bus_clear_containers (BusContainers **containers_p)
{
  _dbus_clear_pointer_impl (BusContainers, containers_p, bus_containers_unref);
}

#endif /* multiple-inclusion guard */
