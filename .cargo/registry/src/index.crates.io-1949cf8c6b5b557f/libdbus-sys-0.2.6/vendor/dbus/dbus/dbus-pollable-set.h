/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/*
 * dbus-pollable-set.h - a set of pollable objects (file descriptors, sockets or handles)
 *
 * Copyright © 2011 Nokia Corporation
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
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston,
 * MA  02110-1301  USA
 *
 */

#ifndef DBUS_POLLABLE_SET_H
#define DBUS_POLLABLE_SET_H

#ifndef DOXYGEN_SHOULD_SKIP_THIS

#include <dbus/dbus.h>
#include <dbus/dbus-sysdeps.h>

typedef struct {
    DBusPollable fd;
    unsigned int flags;
} DBusPollableEvent;

typedef struct DBusPollableSet DBusPollableSet;

typedef struct DBusPollableSetClass DBusPollableSetClass;
struct DBusPollableSetClass {
    void            (*free)     (DBusPollableSet   *self);
    dbus_bool_t     (*add)      (DBusPollableSet   *self,
                                 DBusPollable       fd,
                                 unsigned int       flags,
                                 dbus_bool_t        enabled);
    void            (*remove)   (DBusPollableSet   *self,
                                 DBusPollable       fd);
    void            (*enable)   (DBusPollableSet   *self,
                                 DBusPollable       fd,
                                 unsigned int       flags);
    void            (*disable)  (DBusPollableSet   *self,
                                 DBusPollable       fd);
    int             (*poll)     (DBusPollableSet   *self,
                                 DBusPollableEvent *revents,
                                 int                max_events,
                                 int                timeout_ms);
};

struct DBusPollableSet {
    DBusPollableSetClass *cls;
};

DBusPollableSet *_dbus_pollable_set_new (int size_hint);

static inline void
_dbus_pollable_set_free (DBusPollableSet *self)
{
  (self->cls->free) (self);
}

static inline dbus_bool_t
_dbus_pollable_set_add (DBusPollableSet *self,
                        DBusPollable     fd,
                        unsigned int     flags,
                        dbus_bool_t      enabled)
{
  return (self->cls->add) (self, fd, flags, enabled);
}

static inline void
_dbus_pollable_set_remove (DBusPollableSet *self,
                           DBusPollable     fd)
{
  (self->cls->remove) (self, fd);
}

static inline void
_dbus_pollable_set_enable (DBusPollableSet *self,
                           DBusPollable     fd,
                           unsigned int     flags)
{
  (self->cls->enable) (self, fd, flags);
}

static inline void
_dbus_pollable_set_disable (DBusPollableSet *self,
                            DBusPollable     fd)
{
  (self->cls->disable) (self, fd);
}


static inline int
_dbus_pollable_set_poll (DBusPollableSet    *self,
                         DBusPollableEvent  *revents,
                         int                 max_events,
                         int                 timeout_ms)
{
  return (self->cls->poll) (self, revents, max_events, timeout_ms);
}

/* concrete implementations, not necessarily built on all platforms */

extern DBusPollableSetClass _dbus_pollable_set_poll_class;
extern DBusPollableSetClass _dbus_pollable_set_epoll_class;

DBusPollableSet *_dbus_pollable_set_poll_new  (int  size_hint);
DBusPollableSet *_dbus_pollable_set_epoll_new (void);

#endif /* !DOXYGEN_SHOULD_SKIP_THIS */
#endif /* multiple-inclusion guard */
