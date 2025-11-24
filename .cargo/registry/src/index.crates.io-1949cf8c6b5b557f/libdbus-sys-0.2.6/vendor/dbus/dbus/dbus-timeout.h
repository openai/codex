/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-timeout.h DBusTimeout internal interfaces
 *
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
#ifndef DBUS_TIMEOUT_H
#define DBUS_TIMEOUT_H

#include <dbus/dbus-connection.h>
#include <dbus/dbus-internals.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusTimeoutInternals
 * @{
 */

/* Public methods on DBusTimeout are in dbus-connection.h */

typedef struct DBusTimeoutList DBusTimeoutList;

/** function to run when the timeout is handled */
typedef dbus_bool_t (* DBusTimeoutHandler) (void *data);

DBUS_PRIVATE_EXPORT
DBusTimeout* _dbus_timeout_new          (int                 interval,
                                         DBusTimeoutHandler  handler,
                                         void               *data,
                                         DBusFreeFunction    free_data_function);
DBusTimeout* _dbus_timeout_ref          (DBusTimeout        *timeout);
DBUS_PRIVATE_EXPORT
void         _dbus_timeout_unref        (DBusTimeout        *timeout);
DBUS_PRIVATE_EXPORT
void         _dbus_timeout_restart      (DBusTimeout        *timeout,
                                         int                 interval);
DBUS_PRIVATE_EXPORT
void         _dbus_timeout_disable      (DBusTimeout        *timeout);

DBusTimeoutList *_dbus_timeout_list_new            (void);
void             _dbus_timeout_list_free           (DBusTimeoutList           *timeout_list);
dbus_bool_t      _dbus_timeout_list_set_functions  (DBusTimeoutList           *timeout_list,
						    DBusAddTimeoutFunction     add_function,
						    DBusRemoveTimeoutFunction  remove_function,
                                                    DBusTimeoutToggledFunction toggled_function,
						    void                      *data,
						    DBusFreeFunction           free_data_function);
dbus_bool_t      _dbus_timeout_list_add_timeout    (DBusTimeoutList           *timeout_list,
						    DBusTimeout               *timeout);
void             _dbus_timeout_list_remove_timeout (DBusTimeoutList           *timeout_list,
						    DBusTimeout               *timeout);
void             _dbus_timeout_list_toggle_timeout (DBusTimeoutList           *timeout_list,
                                                    DBusTimeout               *timeout,
                                                    dbus_bool_t                enabled);

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_timeout_needs_restart (DBusTimeout *timeout);
DBUS_PRIVATE_EXPORT
void        _dbus_timeout_restarted     (DBusTimeout *timeout);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_TIMEOUT_H */
