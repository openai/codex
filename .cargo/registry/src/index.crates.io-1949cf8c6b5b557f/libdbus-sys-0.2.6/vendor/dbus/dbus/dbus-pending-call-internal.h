/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-pending-call-internal.h DBusPendingCall internal interfaces
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
#ifndef DBUS_PENDING_CALL_INTERNAL_H
#define DBUS_PENDING_CALL_INTERNAL_H


#include <dbus/dbus-internals.h>
#include <dbus/dbus-message.h>
#include <dbus/dbus-connection.h>
#include <dbus/dbus-list.h>

DBUS_BEGIN_DECLS

dbus_bool_t      _dbus_pending_call_is_timeout_added_unlocked    (DBusPendingCall    *pending);
void             _dbus_pending_call_set_timeout_added_unlocked   (DBusPendingCall    *pending,
                                                                  dbus_bool_t         is_added);
DBusTimeout    * _dbus_pending_call_get_timeout_unlocked         (DBusPendingCall    *pending);
dbus_uint32_t    _dbus_pending_call_get_reply_serial_unlocked    (DBusPendingCall    *pending);
void             _dbus_pending_call_set_reply_serial_unlocked    (DBusPendingCall    *pending,
                                                                  dbus_uint32_t       serial);
DBusConnection * _dbus_pending_call_get_connection_and_lock      (DBusPendingCall    *pending);
DBusConnection * _dbus_pending_call_get_connection_unlocked      (DBusPendingCall    *pending);
dbus_bool_t      _dbus_pending_call_get_completed_unlocked       (DBusPendingCall    *pending);

void             _dbus_pending_call_start_completion_unlocked    (DBusPendingCall    *pending);
void             _dbus_pending_call_finish_completion            (DBusPendingCall    *pending);

void             _dbus_pending_call_set_reply_unlocked           (DBusPendingCall    *pending,
                                                                  DBusMessage        *message);
void             _dbus_pending_call_queue_timeout_error_unlocked (DBusPendingCall    *pending,
                                                                  DBusConnection     *connection);
dbus_bool_t      _dbus_pending_call_set_timeout_error_unlocked   (DBusPendingCall    *pending,
                                                                  DBusMessage        *message,
                                                                  dbus_uint32_t       serial);
DBUS_PRIVATE_EXPORT
DBusPendingCall* _dbus_pending_call_new_unlocked                 (DBusConnection     *connection,
                                                                  int                 timeout_milliseconds,
                                                                  DBusTimeoutHandler  timeout_handler);
DBUS_PRIVATE_EXPORT
DBusPendingCall* _dbus_pending_call_ref_unlocked                 (DBusPendingCall    *pending);
DBUS_PRIVATE_EXPORT
void             _dbus_pending_call_unref_and_unlock             (DBusPendingCall    *pending);
dbus_bool_t      _dbus_pending_call_set_data_unlocked            (DBusPendingCall    *pending,
                                                                  dbus_int32_t        slot,
                                                                  void               *data,
                                                                  DBusFreeFunction    free_data_func);


DBUS_END_DECLS

#endif /* DBUS_PENDING_CALL_INTERNAL_H */
