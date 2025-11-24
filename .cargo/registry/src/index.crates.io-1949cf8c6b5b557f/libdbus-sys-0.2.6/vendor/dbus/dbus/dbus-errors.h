/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-errors.h Error reporting
 *
 * Copyright (C) 2002  Red Hat Inc.
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
#if !defined (DBUS_INSIDE_DBUS_H) && !defined (DBUS_COMPILATION)
#error "Only <dbus/dbus.h> can be included directly, this file may disappear or change contents."
#endif

#ifndef DBUS_ERROR_H
#define DBUS_ERROR_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-protocol.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusErrors
 * @{
 */

/** Mostly-opaque type representing an error that occurred */
typedef struct DBusError DBusError;

/**
 * Object representing an exception.
 */
struct DBusError
{
  const char *name;    /**< public error name field */
  const char *message; /**< public error message field */

  unsigned int dummy1 : 1; /**< placeholder */
  unsigned int dummy2 : 1; /**< placeholder */
  unsigned int dummy3 : 1; /**< placeholder */
  unsigned int dummy4 : 1; /**< placeholder */
  unsigned int dummy5 : 1; /**< placeholder */

  void *padding1; /**< placeholder */
};

#define DBUS_ERROR_INIT { NULL, NULL, TRUE, 0, 0, 0, 0, NULL }

DBUS_EXPORT
void        dbus_error_init      (DBusError       *error);
DBUS_EXPORT
void        dbus_error_free      (DBusError       *error);
DBUS_EXPORT
void        dbus_set_error       (DBusError       *error,
                                  const char      *name,
                                  const char      *message,
                                  ...) _DBUS_GNUC_PRINTF (3, 4);
DBUS_EXPORT
void        dbus_set_error_const (DBusError       *error,
                                  const char      *name,
                                  const char      *message);
DBUS_EXPORT
void        dbus_move_error      (DBusError       *src,
                                  DBusError       *dest);
DBUS_EXPORT
dbus_bool_t dbus_error_has_name  (const DBusError *error,
                                  const char      *name);
DBUS_EXPORT
dbus_bool_t dbus_error_is_set    (const DBusError *error);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_ERROR_H */
