/* selinux.h  SELinux security check headers for D-BUS
 *
 * Author: Matthew Rickard <mjricka@epoch.ncsc.mil>
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

#ifndef BUS_SELINUX_H
#define BUS_SELINUX_H

#include <dbus/dbus-hash.h>
#include <dbus/dbus-connection.h>
#include "services.h"

dbus_bool_t bus_selinux_pre_init (void);
dbus_bool_t bus_selinux_full_init(BusContext *context, DBusError *error);
void        bus_selinux_shutdown (void);

dbus_bool_t bus_selinux_enabled  (void);

BusSELinuxID *bus_selinux_get_self (void);

DBusHashTable* bus_selinux_id_table_new    (void);
BusSELinuxID*  bus_selinux_id_table_lookup (DBusHashTable    *service_table,
                                            const DBusString *service_name);
dbus_bool_t    bus_selinux_id_table_insert (DBusHashTable    *service_table,
                                            const char       *service_name,
                                            const char       *service_context);
void           bus_selinux_id_table_print  (DBusHashTable    *service_table);
const char*    bus_selinux_get_policy_root (void);

dbus_bool_t    bus_selinux_append_context      (DBusMessage    *message,
						BusSELinuxID   *context,
						DBusError      *error);

dbus_bool_t bus_selinux_allows_acquire_service (DBusConnection *connection,
                                                BusSELinuxID   *service_sid,
						const char     *service_name,
						DBusError      *error);

dbus_bool_t bus_selinux_allows_send            (DBusConnection *sender,
                                                DBusConnection *proposed_recipient,
						const char     *msgtype, /* Supplementary audit data */
						const char     *interface,
						const char     *member,
						const char     *error_name,
						const char     *destination,
						BusActivationEntry *activation_entry,
						DBusError      *error);

BusSELinuxID* bus_selinux_init_connection_id (DBusConnection *connection,
                                              DBusError      *error);

#endif /* BUS_SELINUX_H */
