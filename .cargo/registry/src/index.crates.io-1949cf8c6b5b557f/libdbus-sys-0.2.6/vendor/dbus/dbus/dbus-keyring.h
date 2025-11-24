/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-keyring.h Store secret cookies in your homedir
 *
 * Copyright (C) 2003  Red Hat Inc.
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
#ifndef DBUS_KEYRING_H
#define DBUS_KEYRING_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-credentials.h>

DBUS_BEGIN_DECLS

typedef struct DBusKeyring DBusKeyring;

DBusKeyring* _dbus_keyring_new_for_credentials (DBusCredentials  *credentials,
                                                const DBusString *context,
                                                DBusError        *error);
DBusKeyring* _dbus_keyring_ref                 (DBusKeyring      *keyring);
void         _dbus_keyring_unref               (DBusKeyring      *keyring);
dbus_bool_t  _dbus_keyring_validate_context    (const DBusString *context);
int          _dbus_keyring_get_best_key        (DBusKeyring      *keyring,
                                                DBusError        *error);
dbus_bool_t  _dbus_keyring_is_for_credentials  (DBusKeyring      *keyring,
                                                DBusCredentials  *credentials);
dbus_bool_t  _dbus_keyring_get_hex_key         (DBusKeyring      *keyring,
                                                int               key_id,
                                                DBusString       *hex_key);


DBUS_END_DECLS

#endif /* DBUS_KEYRING_H */
