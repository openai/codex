/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-syntax.h - utility functions for strings with special syntax
 *
 * Author: Simon McVittie <simon.mcvittie@collabora.co.uk>
 * Copyright © 2011 Nokia Corporation
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

#ifndef DBUS_SYNTAX_H
#define DBUS_SYNTAX_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>

DBUS_BEGIN_DECLS

DBUS_EXPORT
dbus_bool_t     dbus_validate_path                   (const char *path,
                                                      DBusError  *error);
DBUS_EXPORT
dbus_bool_t     dbus_validate_interface              (const char *name,
                                                      DBusError  *error);
DBUS_EXPORT
dbus_bool_t     dbus_validate_member                 (const char *name,
                                                      DBusError  *error);
DBUS_EXPORT
dbus_bool_t     dbus_validate_error_name             (const char *name,
                                                      DBusError  *error);
DBUS_EXPORT
dbus_bool_t     dbus_validate_bus_name               (const char *name,
                                                      DBusError  *error);
DBUS_EXPORT
dbus_bool_t     dbus_validate_utf8                   (const char *alleged_utf8,
                                                      DBusError  *error);

DBUS_END_DECLS

#endif /* multiple-inclusion guard */
