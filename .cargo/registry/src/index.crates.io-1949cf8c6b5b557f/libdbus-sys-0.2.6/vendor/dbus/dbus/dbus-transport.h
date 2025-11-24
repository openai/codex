/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-transport.h DBusTransport object (internal to D-BUS implementation)
 *
 * Copyright (C) 2002, 2004  Red Hat Inc.
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
#ifndef DBUS_TRANSPORT_H
#define DBUS_TRANSPORT_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-connection.h>
#include <dbus/dbus-credentials.h>
#include <dbus/dbus-protocol.h>
#include <dbus/dbus-address.h>

DBUS_BEGIN_DECLS

typedef struct DBusTransport DBusTransport;

DBusTransport*     _dbus_transport_open                   (DBusAddressEntry           *entry,
                                                           DBusError                  *error);
DBusTransport*     _dbus_transport_ref                    (DBusTransport              *transport);
void               _dbus_transport_unref                  (DBusTransport              *transport);
void               _dbus_transport_disconnect             (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_get_is_connected       (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_peek_is_authenticated  (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_try_to_authenticate    (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_get_is_anonymous       (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_can_pass_unix_fd       (DBusTransport              *transport);

const char*        _dbus_transport_get_address            (DBusTransport              *transport);
const char*        _dbus_transport_get_server_id          (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_handle_watch           (DBusTransport              *transport,
                                                           DBusWatch                  *watch,
                                                           unsigned int                condition);
dbus_bool_t        _dbus_transport_set_connection         (DBusTransport              *transport,
                                                           DBusConnection             *connection);
void               _dbus_transport_do_iteration           (DBusTransport              *transport,
                                                           unsigned int                flags,
                                                           int                         timeout_milliseconds);
DBusDispatchStatus _dbus_transport_get_dispatch_status    (DBusTransport              *transport);
dbus_bool_t        _dbus_transport_queue_messages         (DBusTransport              *transport);

void               _dbus_transport_set_max_message_size   (DBusTransport              *transport,
                                                           long                        size);
long               _dbus_transport_get_max_message_size   (DBusTransport              *transport);
void               _dbus_transport_set_max_received_size  (DBusTransport              *transport,
                                                           long                        size);
long               _dbus_transport_get_max_received_size  (DBusTransport              *transport);

void               _dbus_transport_set_max_message_unix_fds (DBusTransport              *transport,
                                                             long                        n);
long               _dbus_transport_get_max_message_unix_fds (DBusTransport              *transport);
void               _dbus_transport_set_max_received_unix_fds(DBusTransport              *transport,
                                                             long                        n);
long               _dbus_transport_get_max_received_unix_fds(DBusTransport              *transport);

dbus_bool_t        _dbus_transport_get_socket_fd          (DBusTransport              *transport,
                                                           DBusSocket                 *fd_p);
dbus_bool_t        _dbus_transport_get_unix_user          (DBusTransport              *transport,
                                                           unsigned long              *uid);
dbus_bool_t        _dbus_transport_get_unix_process_id     (DBusTransport              *transport,
                                                           unsigned long              *pid);
dbus_bool_t        _dbus_transport_get_adt_audit_session_data (DBusTransport              *transport,
                                                               void                      **data,
                                                               int                        *data_size);
void               _dbus_transport_set_unix_user_function (DBusTransport              *transport,
                                                           DBusAllowUnixUserFunction   function,
                                                           void                       *data,
                                                           DBusFreeFunction            free_data_function,
                                                           void                      **old_data,
                                                           DBusFreeFunction           *old_free_data_function);
dbus_bool_t        _dbus_transport_get_windows_user       (DBusTransport              *transport,
                                                           char                      **windows_sid_p);
dbus_bool_t        _dbus_transport_get_linux_security_label (DBusTransport            *transport,
                                                           char                      **label_p);
DBusCredentials   *_dbus_transport_get_credentials        (DBusTransport  *transport);

void               _dbus_transport_set_windows_user_function (DBusTransport              *transport,
                                                              DBusAllowWindowsUserFunction   function,
                                                              void                       *data,
                                                              DBusFreeFunction            free_data_function,
                                                              void                      **old_data,
                                                              DBusFreeFunction           *old_free_data_function);
dbus_bool_t        _dbus_transport_set_auth_mechanisms    (DBusTransport              *transport,
                                                           const char                **mechanisms);
void               _dbus_transport_set_allow_anonymous    (DBusTransport              *transport,
                                                           dbus_bool_t                 value);
int                _dbus_transport_get_pending_fds_count  (DBusTransport              *transport);
void               _dbus_transport_set_pending_fds_function (DBusTransport *transport,
                                                             void (* callback) (void *),
                                                             void *data);

/* if DBUS_ENABLE_STATS */
void _dbus_transport_get_stats (DBusTransport  *transport,
                                dbus_uint32_t  *queue_bytes,
                                dbus_uint32_t  *queue_fds,
                                dbus_uint32_t  *peak_queue_bytes,
                                dbus_uint32_t  *peak_queue_fds);

DBUS_END_DECLS

#endif /* DBUS_TRANSPORT_H */
