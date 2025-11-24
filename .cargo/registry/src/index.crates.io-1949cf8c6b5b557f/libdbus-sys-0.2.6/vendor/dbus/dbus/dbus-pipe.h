/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sysdeps.h Wrappers around system/libc features (internal to D-Bus implementation)
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

#ifndef DBUS_PIPE_H
#define DBUS_PIPE_H

#ifdef HAVE_STDINT_H
#include <stdint.h>
#endif

#ifdef HAVE_INTTYPES_H
#include <inttypes.h>
#endif

#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-sysdeps.h>

struct DBusPipe {
  int fd;
};

DBUS_PRIVATE_EXPORT
void        _dbus_pipe_init                (DBusPipe         *pipe,
                                            int               fd);
DBUS_PRIVATE_EXPORT
void        _dbus_pipe_init_stdout         (DBusPipe         *pipe);
DBUS_PRIVATE_EXPORT
int         _dbus_pipe_write               (DBusPipe         *pipe,
                                            const DBusString *buffer,
                                            int               start,
                                            int               len,
                                            DBusError        *error);
DBUS_PRIVATE_EXPORT
int         _dbus_pipe_close               (DBusPipe         *pipe,
                                            DBusError        *error);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_pipe_is_valid            (DBusPipe         *pipe);
DBUS_PRIVATE_EXPORT
void        _dbus_pipe_invalidate          (DBusPipe         *pipe);
DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_pipe_is_stdout_or_stderr (DBusPipe         *pipe);

#endif
