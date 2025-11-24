/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-resources.h Resource tracking/limits
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
#ifndef DBUS_RESOURCES_H
#define DBUS_RESOURCES_H

#include <dbus/dbus-macros.h>
#include <dbus/dbus-errors.h>
#include <dbus/dbus-connection.h>

DBUS_BEGIN_DECLS

typedef struct DBusCounter DBusCounter;

typedef void (* DBusCounterNotifyFunction) (DBusCounter *counter,
                                            void        *user_data);

DBusCounter* _dbus_counter_new       (void);
DBusCounter* _dbus_counter_ref       (DBusCounter *counter);
void         _dbus_counter_unref     (DBusCounter *counter);

void         _dbus_counter_adjust_size       (DBusCounter *counter,
                                              long         delta);
void         _dbus_counter_adjust_unix_fd    (DBusCounter *counter,
                                              long         delta);
void         _dbus_counter_notify            (DBusCounter *counter);
long         _dbus_counter_get_size_value    (DBusCounter *counter);
long         _dbus_counter_get_unix_fd_value (DBusCounter *counter);

void _dbus_counter_set_notify    (DBusCounter               *counter,
                                  long                       size_guard_value,
                                  long                       unix_fd_guard_value,
                                  DBusCounterNotifyFunction  function,
                                  void                      *user_data);

/* if DBUS_ENABLE_STATS */
long _dbus_counter_get_peak_size_value    (DBusCounter *counter);
long _dbus_counter_get_peak_unix_fd_value (DBusCounter *counter);

DBUS_END_DECLS

#endif /* DBUS_RESOURCES_H */
