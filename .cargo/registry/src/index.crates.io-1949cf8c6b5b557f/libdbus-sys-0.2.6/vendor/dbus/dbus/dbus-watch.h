/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-watch.h DBusWatch internal interfaces
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
#ifndef DBUS_WATCH_H
#define DBUS_WATCH_H

#include <dbus/dbus-internals.h>
#include <dbus/dbus-connection.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusWatchInternals
 * @{
 */

/* Public methods on DBusWatch are in dbus-connection.h */

typedef struct DBusWatchList DBusWatchList;

#define _DBUS_WATCH_NVAL (1<<4)

/** function to run when the watch is handled */
typedef dbus_bool_t (* DBusWatchHandler) (DBusWatch    *watch,
                                          unsigned int  flags,
                                          void         *data);

DBUS_PRIVATE_EXPORT
DBusWatch* _dbus_watch_new                (DBusPollable      fd,
                                           unsigned int      flags,
                                           dbus_bool_t       enabled,
                                           DBusWatchHandler  handler,
                                           void             *data,
                                           DBusFreeFunction  free_data_function);
DBUS_PRIVATE_EXPORT
DBusWatch* _dbus_watch_ref                (DBusWatch        *watch);
DBUS_PRIVATE_EXPORT
void       _dbus_watch_unref              (DBusWatch        *watch);
DBUS_PRIVATE_EXPORT
void       _dbus_watch_invalidate         (DBusWatch        *watch);
void       _dbus_watch_sanitize_condition (DBusWatch        *watch,
                                           unsigned int     *condition);
void       _dbus_watch_set_handler        (DBusWatch        *watch,
                                           DBusWatchHandler  handler,
                                           void             *data,
                                           DBusFreeFunction  free_data_function);


DBUS_PRIVATE_EXPORT
DBusWatchList* _dbus_watch_list_new           (void);
DBUS_PRIVATE_EXPORT
void           _dbus_watch_list_free          (DBusWatchList           *watch_list);
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_watch_list_set_functions (DBusWatchList           *watch_list,
                                               DBusAddWatchFunction     add_function,
                                               DBusRemoveWatchFunction  remove_function,
                                               DBusWatchToggledFunction toggled_function,
                                               void                    *data,
                                               DBusFreeFunction         free_data_function);
DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_watch_list_add_watch     (DBusWatchList           *watch_list,
                                               DBusWatch               *watch);
DBUS_PRIVATE_EXPORT
void           _dbus_watch_list_remove_watch  (DBusWatchList           *watch_list,
                                               DBusWatch               *watch);
void           _dbus_watch_list_toggle_watch  (DBusWatchList           *watch_list,
                                               DBusWatch               *watch,
                                               dbus_bool_t              enabled);
void           _dbus_watch_list_toggle_all_watches (DBusWatchList      *watch_list,
                                               dbus_bool_t              enabled);
dbus_bool_t    _dbus_watch_get_enabled        (DBusWatch              *watch);

DBUS_PRIVATE_EXPORT
dbus_bool_t    _dbus_watch_get_oom_last_time  (DBusWatch               *watch);
DBUS_PRIVATE_EXPORT
void           _dbus_watch_set_oom_last_time  (DBusWatch               *watch,
                                               dbus_bool_t              oom);

DBusSocket     _dbus_watch_get_socket         (DBusWatch               *watch);
DBUS_PRIVATE_EXPORT
DBusPollable   _dbus_watch_get_pollable       (DBusWatch               *watch);

static inline void
_dbus_clear_watch (DBusWatch **pointer_to_watch)
{
  _dbus_clear_pointer_impl (DBusWatch, pointer_to_watch,
                            _dbus_watch_unref);
}

/** @} */

DBUS_END_DECLS

#endif /* DBUS_WATCH_H */
