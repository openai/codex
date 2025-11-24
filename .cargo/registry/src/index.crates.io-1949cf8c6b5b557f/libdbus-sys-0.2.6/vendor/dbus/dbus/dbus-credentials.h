/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-credentials.h Credentials provable through authentication
 *
 * Copyright (C) 2007 Red Hat Inc.
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
#ifndef DBUS_CREDENTIALS_H
#define DBUS_CREDENTIALS_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-sysdeps.h>

DBUS_BEGIN_DECLS

typedef enum {
  DBUS_CREDENTIAL_UNIX_PROCESS_ID,
  DBUS_CREDENTIAL_UNIX_USER_ID,
  DBUS_CREDENTIAL_UNIX_GROUP_IDS,
  DBUS_CREDENTIAL_ADT_AUDIT_DATA_ID,
  DBUS_CREDENTIAL_LINUX_SECURITY_LABEL,
  DBUS_CREDENTIAL_WINDOWS_SID
} DBusCredentialType;

DBUS_PRIVATE_EXPORT
DBusCredentials* _dbus_credentials_new_from_current_process (void);
DBUS_PRIVATE_EXPORT
DBusCredentials* _dbus_credentials_new                      (void);
DBUS_PRIVATE_EXPORT
void             _dbus_credentials_ref                      (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
void             _dbus_credentials_unref                    (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_add_pid                  (DBusCredentials    *credentials,
                                                             dbus_pid_t          pid);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_add_unix_uid             (DBusCredentials    *credentials,
                                                             dbus_uid_t          uid);
DBUS_PRIVATE_EXPORT
void             _dbus_credentials_take_unix_gids           (DBusCredentials    *credentials,
                                                             dbus_gid_t         *gids,
                                                             size_t              n_gids);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_add_windows_sid          (DBusCredentials    *credentials,
                                                             const char         *windows_sid);
dbus_bool_t      _dbus_credentials_add_linux_security_label (DBusCredentials    *credentials,
                                                             const char         *label);
dbus_bool_t      _dbus_credentials_add_adt_audit_data       (DBusCredentials    *credentials,
                                                             void               *audit_data,
                                                             dbus_int32_t        size);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_include                  (DBusCredentials    *credentials,
                                                             DBusCredentialType  type);
DBUS_PRIVATE_EXPORT
dbus_pid_t       _dbus_credentials_get_pid                  (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_uid_t       _dbus_credentials_get_unix_uid             (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_get_unix_gids            (DBusCredentials    *credentials,
                                                             const dbus_gid_t  **gids,
                                                             size_t             *n_gids);
DBUS_PRIVATE_EXPORT
const char*      _dbus_credentials_get_windows_sid          (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
const char *     _dbus_credentials_get_linux_security_label (DBusCredentials    *credentials);
void *           _dbus_credentials_get_adt_audit_data       (DBusCredentials    *credentials);
dbus_int32_t     _dbus_credentials_get_adt_audit_data_size  (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_are_superset             (DBusCredentials    *credentials,
                                                             DBusCredentials    *possible_subset);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_are_empty                (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_are_anonymous            (DBusCredentials    *credentials);
dbus_bool_t      _dbus_credentials_add_credentials          (DBusCredentials    *credentials,
                                                             DBusCredentials    *other_credentials);
/* must silently allow 'which' to not exist */
dbus_bool_t      _dbus_credentials_add_credential           (DBusCredentials    *credentials,
                                                             DBusCredentialType  which,
                                                             DBusCredentials    *other_credentials);
DBUS_PRIVATE_EXPORT
void             _dbus_credentials_clear                    (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
DBusCredentials* _dbus_credentials_copy                     (DBusCredentials    *credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_same_user                (DBusCredentials    *credentials,
                                                             DBusCredentials    *other_credentials);
DBUS_PRIVATE_EXPORT
dbus_bool_t      _dbus_credentials_to_string_append         (DBusCredentials    *credentials,
                                                             DBusString         *string);

static inline void
_dbus_clear_credentials (DBusCredentials **pointer_to_creds)
{
  _dbus_clear_pointer_impl (DBusCredentials, pointer_to_creds,
                            _dbus_credentials_unref);
}

DBUS_END_DECLS

#endif /* DBUS_CREDENTIALS_H */
