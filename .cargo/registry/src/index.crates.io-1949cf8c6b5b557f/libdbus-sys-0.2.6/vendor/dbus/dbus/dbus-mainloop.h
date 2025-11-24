/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-mainloop.h  Main loop utility
 *
 * Copyright (C) 2003  Red Hat, Inc.
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

#ifndef DBUS_MAINLOOP_H
#define DBUS_MAINLOOP_H

#ifndef DOXYGEN_SHOULD_SKIP_THIS

#include <dbus/dbus.h>

typedef struct DBusLoop DBusLoop;

typedef dbus_bool_t (* DBusWatchFunction)   (DBusWatch     *watch,
                                             unsigned int   condition,
                                             void          *data);

DBusLoop*   _dbus_loop_new            (void);
DBusLoop*   _dbus_loop_ref            (DBusLoop            *loop);
void        _dbus_loop_unref          (DBusLoop            *loop);
dbus_bool_t _dbus_loop_add_watch      (DBusLoop            *loop,
                                       DBusWatch           *watch);
void        _dbus_loop_remove_watch   (DBusLoop            *loop,
                                       DBusWatch           *watch);
void        _dbus_loop_toggle_watch   (DBusLoop            *loop,
                                       DBusWatch           *watch);
dbus_bool_t _dbus_loop_add_timeout    (DBusLoop            *loop,
                                       DBusTimeout         *timeout);
void        _dbus_loop_remove_timeout (DBusLoop            *loop,
                                       DBusTimeout         *timeout);

dbus_bool_t _dbus_loop_queue_dispatch (DBusLoop            *loop,
                                       DBusConnection      *connection);

void        _dbus_loop_run            (DBusLoop            *loop);
void        _dbus_loop_quit           (DBusLoop            *loop);
dbus_bool_t _dbus_loop_iterate        (DBusLoop            *loop,
                                       dbus_bool_t          block);
dbus_bool_t _dbus_loop_dispatch       (DBusLoop            *loop);

int  _dbus_get_oom_wait    (void);
void _dbus_wait_for_memory (void);

static inline void
_dbus_clear_loop (DBusLoop **pointer_to_loop)
{
  _dbus_clear_pointer_impl (DBusLoop, pointer_to_loop,
                            _dbus_loop_unref);
}

#endif /* !DOXYGEN_SHOULD_SKIP_THIS */

#endif /* DBUS_MAINLOOP_H */
