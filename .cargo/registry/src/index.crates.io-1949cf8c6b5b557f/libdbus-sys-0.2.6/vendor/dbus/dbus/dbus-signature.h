/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-signatures.h utility functions for D-Bus types
 *
 * Copyright (C) 2005 Red Hat Inc.
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

#ifndef DBUS_SIGNATURES_H
#define DBUS_SIGNATURES_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-types.h>
#include <dbus/dbus-errors.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusSignature
 * @{
 */

/**
 * DBusSignatureIter struct; contains no public fields 
 */
typedef struct
{ 
  void *dummy1;         /**< Don't use this */
  void *dummy2;         /**< Don't use this */
  dbus_uint32_t dummy8; /**< Don't use this */
  int dummy12;           /**< Don't use this */
  int dummy17;           /**< Don't use this */
} DBusSignatureIter;

DBUS_EXPORT
void            dbus_signature_iter_init             (DBusSignatureIter       *iter,
						      const char              *signature);

DBUS_EXPORT
int             dbus_signature_iter_get_current_type (const DBusSignatureIter *iter);

DBUS_EXPORT
char *          dbus_signature_iter_get_signature    (const DBusSignatureIter *iter);

DBUS_EXPORT
int             dbus_signature_iter_get_element_type (const DBusSignatureIter *iter);

DBUS_EXPORT
dbus_bool_t     dbus_signature_iter_next             (DBusSignatureIter       *iter);

DBUS_EXPORT
void            dbus_signature_iter_recurse          (const DBusSignatureIter *iter,
						      DBusSignatureIter       *subiter);

DBUS_EXPORT
dbus_bool_t     dbus_signature_validate              (const char       *signature,
						      DBusError        *error);

DBUS_EXPORT
dbus_bool_t     dbus_signature_validate_single       (const char       *signature,
						      DBusError        *error);

DBUS_EXPORT
dbus_bool_t     dbus_type_is_valid                   (int            typecode);

DBUS_EXPORT
dbus_bool_t     dbus_type_is_basic                   (int            typecode);
DBUS_EXPORT
dbus_bool_t     dbus_type_is_container               (int            typecode);
DBUS_EXPORT
dbus_bool_t     dbus_type_is_fixed                   (int            typecode);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_SIGNATURE_H */
