/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-misc.h  A few assorted public functions that don't fit elsewhere
 *
 * Copyright (C) 2006 Red Hat, Inc.
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

#ifndef DBUS_MISC_H
#define DBUS_MISC_H

#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusMisc
 * @{
 */
DBUS_EXPORT
char*       dbus_get_local_machine_id  (void);

DBUS_EXPORT
void        dbus_get_version           (int *major_version_p,
                                        int *minor_version_p,
                                        int *micro_version_p);

DBUS_EXPORT
dbus_bool_t dbus_setenv                (const char *variable,
                                        const char *value);

DBUS_EXPORT
char *dbus_try_get_local_machine_id (DBusError *error);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_MISC_H */
