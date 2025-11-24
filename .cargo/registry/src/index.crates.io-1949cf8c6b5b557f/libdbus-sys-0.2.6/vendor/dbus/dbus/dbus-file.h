/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-file.h dbus file related stuff (internal to D-Bus implementation)
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
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

#ifndef DBUS_FILE_H
#define DBUS_FILE_H

//#include <dbus/dbus-types.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-errors.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusFile
 * @{
 */

/**
 * File interface
 */
dbus_bool_t _dbus_file_exists         (const char       *file);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_file_get_contents   (DBusString       *str,
                                       const DBusString *filename,
                                       DBusError        *error);
dbus_bool_t _dbus_string_save_to_file (const DBusString *str,
                                       const DBusString *filename,
                                       dbus_bool_t       world_readable,
                                       DBusError        *error);

dbus_bool_t _dbus_make_file_world_readable   (const DBusString *filename,
                                              DBusError *error);

dbus_bool_t    _dbus_create_file_exclusively (const DBusString *filename,
                                              DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_delete_file             (const DBusString *filename,
                                              DBusError        *error);
                                              
/** @} */

DBUS_END_DECLS

#endif
