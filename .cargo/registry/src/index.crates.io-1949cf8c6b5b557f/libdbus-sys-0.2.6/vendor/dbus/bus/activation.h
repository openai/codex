/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* activation.h  Activation of services
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

#ifndef BUS_ACTIVATION_H
#define BUS_ACTIVATION_H

#include <dbus/dbus.h>
#include <dbus/dbus-list.h>
#include "bus.h"

BusActivation* bus_activation_new              (BusContext        *context,
						const DBusString  *address,
						DBusList         **directories,
						DBusError         *error);
dbus_bool_t bus_activation_reload           (BusActivation     *activation,
						const DBusString  *address,
						DBusList         **directories,
						DBusError         *error);
BusActivation* bus_activation_ref              (BusActivation     *activation);
void           bus_activation_unref            (BusActivation     *activation);

dbus_bool_t   bus_activation_set_environment_variable (BusActivation     *activation,
						const char        *key,
						const char        *value,
						DBusError         *error);
dbus_bool_t    bus_activation_activate_service (BusActivation     *activation,
						DBusConnection    *connection,
						BusTransaction    *transaction,
						dbus_bool_t        auto_activation,
						DBusMessage       *activation_message,
						const char        *service_name,
						DBusError         *error);
dbus_bool_t    bus_activation_service_created  (BusActivation     *activation,
						const char        *service_name,
						BusTransaction    *transaction,
						DBusError         *error);
dbus_bool_t    bus_activation_list_services    (BusActivation     *registry,
						char            ***listp,
						int               *array_len);
dbus_bool_t    dbus_activation_systemd_failure (BusActivation     *activation,
                                                DBusMessage       *message);

dbus_bool_t    bus_activation_send_pending_auto_activation_messages (BusActivation     *activation,
								     BusService        *service,
								     BusTransaction    *transaction);

const char *bus_activation_entry_get_assumed_apparmor_label (BusActivationEntry *entry);

#endif /* BUS_ACTIVATION_H */
