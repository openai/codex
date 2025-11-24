/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-sha.h SHA-1 implementation
 *
 * Copyright (C) 2003 Red Hat Inc.
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
#ifndef DBUS_SHA_H
#define DBUS_SHA_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>

DBUS_BEGIN_DECLS

typedef struct DBusSHAContext DBusSHAContext;

/**
 * Struct storing state of the SHA algorithm
 */
struct DBusSHAContext
{
  dbus_uint32_t  digest[5];         /**< Message digest */
  dbus_uint32_t  count_lo;          /**< 64-bit bit count */
  dbus_uint32_t  count_hi;          /**< No clue */
  dbus_uint32_t  data[16];          /**< SHA data buffer */
};

void        _dbus_sha_init    (DBusSHAContext   *context);
void        _dbus_sha_update  (DBusSHAContext   *context,
                               const DBusString *data);
dbus_bool_t _dbus_sha_final   (DBusSHAContext   *context,
                               DBusString       *results);
DBUS_EMBEDDED_TESTS_EXPORT
dbus_bool_t _dbus_sha_compute (const DBusString *data,
                               DBusString       *ascii_output);

DBUS_END_DECLS

#endif /* DBUS_SHA_H */
