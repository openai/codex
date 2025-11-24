/* stats.h - statistics from the bus driver
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

#ifndef BUS_STATS_H
#define BUS_STATS_H

#include "bus.h"

#define BUS_INTERFACE_STATS "org.freedesktop.DBus.Debug.Stats"

dbus_bool_t bus_stats_handle_get_stats (DBusConnection *connection,
                                        BusTransaction *transaction,
                                        DBusMessage    *message,
                                        DBusError      *error);

dbus_bool_t bus_stats_handle_get_connection_stats (DBusConnection *connection,
                                                   BusTransaction *transaction,
                                                   DBusMessage    *message,
                                                   DBusError      *error);

dbus_bool_t bus_stats_handle_get_all_match_rules (DBusConnection *caller_connection,
                                                  BusTransaction *transaction,
                                                  DBusMessage    *message,
                                                  DBusError      *error);

#endif /* multiple-inclusion guard */
