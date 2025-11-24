/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* test.h  unit test routines
 *
 * Copyright 2003-2007 Red Hat, Inc.
 * Copyright 2003-2004 Imendio
 * Copyright 2009 Lennart Poettering
 * Copyright 2018 Collabora Ltd.
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

#ifndef BUS_TEST_H
#define BUS_TEST_H

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

#include <dbus/dbus.h>
#include <dbus/dbus-string.h>
#include "connection.h"

typedef dbus_bool_t (* BusConnectionForeachFunction) (DBusConnection *connection,
                                                      void           *data);

dbus_bool_t bus_dispatch_test         (const char                   *test_data_dir_cstr);
dbus_bool_t bus_dispatch_sha1_test    (const char                   *test_data_dir_cstr);
dbus_bool_t bus_config_parser_test    (const char                   *test_data_dir_cstr);
dbus_bool_t bus_config_parser_trivial_test (const char              *test_data_dir_cstr);
dbus_bool_t bus_signals_test          (const char                   *test_data_dir);
dbus_bool_t bus_expire_list_test      (const char                   *test_data_dir);
dbus_bool_t bus_activation_service_reload_test (const char          *test_data_dir_cstr);
dbus_bool_t bus_setup_debug_client    (DBusConnection               *connection);
void        bus_test_clients_foreach  (BusConnectionForeachFunction  function,
                                       void                         *data);
dbus_bool_t bus_test_client_listed    (DBusConnection               *connection);
void        bus_test_run_bus_loop     (BusContext                   *context,
                                       dbus_bool_t                   block);
void        bus_test_run_clients_loop (dbus_bool_t                   block);
void        bus_test_run_everything   (BusContext                   *context);
BusContext* bus_context_new_test      (const DBusString             *test_data_dir,
                                       const char                   *filename);
dbus_bool_t bus_unix_fds_passing_test (const char                   *test_data_dir_cstr);

#endif

#endif /* BUS_TEST_H */
