/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-uuidgen.h  The guts of the dbus-uuidgen binary live in libdbus, in this file.
 *
 * Copyright (C) 2006  Red Hat, Inc.
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
#ifdef DBUS_INSIDE_DBUS_H
#error "You can't include dbus-uuidgen.h in the public header dbus.h"
#endif

#ifndef DBUS_UUIDGEN_H
#define DBUS_UUIDGEN_H

#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>

DBUS_BEGIN_DECLS

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_get_uuid    (const char   *filename,
                               char        **uuid_p,
                               dbus_bool_t   create_if_not_found,
                               DBusError    *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_create_uuid                    (char      **uuid_p,
                                                  DBusError  *error);

DBUS_END_DECLS

#endif /* DBUS_UUIDGEN_H */
