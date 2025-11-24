/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-auth.h Authentication
 *
 * Copyright (C) 2002  Red Hat Inc.
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
#ifndef DBUS_AUTH_H
#define DBUS_AUTH_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-sysdeps.h>

DBUS_BEGIN_DECLS

typedef struct DBusAuth DBusAuth;

typedef enum
{
  DBUS_AUTH_STATE_WAITING_FOR_INPUT,
  DBUS_AUTH_STATE_WAITING_FOR_MEMORY,
  DBUS_AUTH_STATE_HAVE_BYTES_TO_SEND,
  DBUS_AUTH_STATE_NEED_DISCONNECT,
  DBUS_AUTH_STATE_AUTHENTICATED,
  DBUS_AUTH_STATE_INVALID = -1
} DBusAuthState;

DBUS_PRIVATE_EXPORT
DBusAuth*     _dbus_auth_server_new          (const DBusString       *guid);
DBUS_PRIVATE_EXPORT
DBusAuth*     _dbus_auth_client_new          (void);
DBUS_PRIVATE_EXPORT
DBusAuth*     _dbus_auth_ref                 (DBusAuth               *auth);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_unref               (DBusAuth               *auth);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_set_mechanisms      (DBusAuth               *auth,
                                              const char            **mechanisms);
DBUS_PRIVATE_EXPORT
DBusAuthState _dbus_auth_do_work             (DBusAuth               *auth);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_get_bytes_to_send   (DBusAuth               *auth,
                                              const DBusString      **str);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_bytes_sent          (DBusAuth               *auth,
                                              int                     bytes_sent);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_get_buffer          (DBusAuth               *auth,
                                              DBusString            **buffer);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_return_buffer       (DBusAuth               *auth,
                                              DBusString             *buffer);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_get_unused_bytes    (DBusAuth               *auth,
                                              const DBusString      **str);
DBUS_PRIVATE_EXPORT
void          _dbus_auth_delete_unused_bytes (DBusAuth               *auth);
dbus_bool_t   _dbus_auth_needs_encoding      (DBusAuth               *auth);
dbus_bool_t   _dbus_auth_encode_data         (DBusAuth               *auth,
                                              const DBusString       *plaintext,
                                              DBusString             *encoded);
dbus_bool_t   _dbus_auth_needs_decoding      (DBusAuth               *auth);
dbus_bool_t   _dbus_auth_decode_data         (DBusAuth               *auth,
                                              const DBusString       *encoded,
                                              DBusString             *plaintext);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_set_credentials     (DBusAuth               *auth,
                                              DBusCredentials        *credentials);
DBUS_PRIVATE_EXPORT
DBusCredentials* _dbus_auth_get_identity     (DBusAuth               *auth);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_set_context         (DBusAuth               *auth,
                                              const DBusString       *context);
const char*   _dbus_auth_get_guid_from_server(DBusAuth               *auth);

void          _dbus_auth_set_unix_fd_possible(DBusAuth               *auth, dbus_bool_t b);
dbus_bool_t   _dbus_auth_get_unix_fd_negotiated(DBusAuth             *auth);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_is_supported_mechanism(DBusString           *name);
DBUS_PRIVATE_EXPORT
dbus_bool_t   _dbus_auth_dump_supported_mechanisms(DBusString        *buffer);

DBUS_END_DECLS

#endif /* DBUS_AUTH_H */
