/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-test.h  Declarations of test functions.
 *
 * Copyright (C) 2002  Red Hat Inc.
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

#ifndef DBUS_TEST_H
#define DBUS_TEST_H

#include <dbus/dbus-types.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-marshal-validate.h>

/* Only things that are in libdbus-1.la and used from libdbus-internal.la
 * need to have DBUS_PRIVATE_EXPORT. If you get
 *
 * warning: 'foo' redeclared without dllimport attribute: previous
 * dllimport ignored [-Wattributes]
 *
 * then you have added too many.
 */

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_marshal_test           (const char *test_data_dir);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_keyring_test           (const char *test_data_dir);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_data_slot_test         (const char *test_data_dir);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_memory_test            (const char *test_data_dir);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_object_tree_test       (const char *test_data_dir);

#endif /* DBUS_TEST_H */
