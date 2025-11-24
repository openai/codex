/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-pollable-set-epoll.c - a pollable set implemented via Linux epoll(4)
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

#include <config.h>
#include "dbus-pollable-set.h"

#include <dbus/dbus-internals.h>
#include <dbus/dbus-sysdeps.h>

#ifndef __linux__
# error This file is for Linux epoll(4)
#endif

#include <errno.h>
#include <fcntl.h>
#include <sys/epoll.h>
#include <unistd.h>

#ifndef DOXYGEN_SHOULD_SKIP_THIS

typedef struct {
    DBusPollableSet parent;
    int epfd;
} DBusPollableSetEpoll;

static inline DBusPollableSetEpoll *
socket_set_epoll_cast (DBusPollableSet *set)
{
  _dbus_assert (set->cls == &_dbus_pollable_set_epoll_class);
  return (DBusPollableSetEpoll *) set;
}

/* this is safe to call on a partially-allocated socket set */
static void
socket_set_epoll_free (DBusPollableSet *set)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);

  if (self == NULL)
    return;

  if (self->epfd != -1)
    close (self->epfd);

  dbus_free (self);
}

DBusPollableSet *
_dbus_pollable_set_epoll_new (void)
{
  DBusPollableSetEpoll *self;

  self = dbus_new0 (DBusPollableSetEpoll, 1);

  if (self == NULL)
    return NULL;

  self->parent.cls = &_dbus_pollable_set_epoll_class;

  self->epfd = epoll_create1 (EPOLL_CLOEXEC);

  if (self->epfd == -1)
    {
      int flags;

      /* the size hint is ignored unless you have a rather old kernel,
       * but must be positive on some versions, so just pick something
       * arbitrary; it's a hint, not a limit */
      self->epfd = epoll_create (42);

      flags = fcntl (self->epfd, F_GETFD, 0);

      if (flags != -1)
        fcntl (self->epfd, F_SETFD, flags | FD_CLOEXEC);
    }

  if (self->epfd == -1)
    {
      socket_set_epoll_free ((DBusPollableSet *) self);
      return NULL;
    }

  return (DBusPollableSet *) self;
}

static uint32_t
watch_flags_to_epoll_events (unsigned int flags)
{
  uint32_t events = 0;

  if (flags & DBUS_WATCH_READABLE)
    events |= EPOLLIN;
  if (flags & DBUS_WATCH_WRITABLE)
    events |= EPOLLOUT;

  return events;
}

static unsigned int
epoll_events_to_watch_flags (uint32_t events)
{
  short flags = 0;

  if (events & EPOLLIN)
    flags |= DBUS_WATCH_READABLE;
  if (events & EPOLLOUT)
    flags |= DBUS_WATCH_WRITABLE;
  if (events & EPOLLHUP)
    flags |= DBUS_WATCH_HANGUP;
  if (events & EPOLLERR)
    flags |= DBUS_WATCH_ERROR;

  return flags;
}

static dbus_bool_t
socket_set_epoll_add (DBusPollableSet  *set,
                      DBusPollable      fd,
                      unsigned int      flags,
                      dbus_bool_t       enabled)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);
  struct epoll_event event;
  int err;

  _DBUS_ZERO (event);
  event.data.fd = fd;

  if (enabled)
    {
      event.events = watch_flags_to_epoll_events (flags);
    }
  else
    {
      /* We need to add *something* to reserve space in the kernel's data
       * structures: see socket_set_epoll_disable for more details */
      event.events = EPOLLET;
    }

  if (epoll_ctl (self->epfd, EPOLL_CTL_ADD, fd, &event) == 0)
    return TRUE;

  /* Anything except ENOMEM, ENOSPC means we have an internal error. */
  err = errno;
  switch (err)
    {
      case ENOMEM:
      case ENOSPC:
        /* be silent: this is basically OOM, which our callers are expected
         * to cope with */
        break;

      case EBADF:
        _dbus_warn ("Bad fd %d", fd);
        break;

      case EEXIST:
        _dbus_warn ("fd %d added and then added again", fd);
        break;

      default:
        _dbus_warn ("Misc error when trying to watch fd %d: %s", fd,
                    strerror (err));
        break;
    }

  return FALSE;
}

static void
socket_set_epoll_enable (DBusPollableSet  *set,
                         DBusPollable      fd,
                         unsigned int      flags)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);
  struct epoll_event event;
  int err;

  _DBUS_ZERO (event);
  event.data.fd = fd;
  event.events = watch_flags_to_epoll_events (flags);

  if (epoll_ctl (self->epfd, EPOLL_CTL_MOD, fd, &event) == 0)
    return;

  err = errno;

  /* Enabling a file descriptor isn't allowed to fail, even for OOM, so we
   * do our best to avoid all of these. */
  switch (err)
    {
      case EBADF:
        _dbus_warn ("Bad fd %d", fd);
        break;

      case ENOENT:
        _dbus_warn ("fd %d enabled before it was added", fd);
        break;

      case ENOMEM:
        _dbus_warn ("Insufficient memory to change watch for fd %d", fd);
        break;

      default:
        _dbus_warn ("Misc error when trying to watch fd %d: %s", fd,
                    strerror (err));
        break;
    }
}

static void
socket_set_epoll_disable (DBusPollableSet  *set,
                          DBusPollable      fd)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);
  struct epoll_event event;
  int err;

  /* The naive thing to do would be EPOLL_CTL_DEL, but that'll probably
   * free resources in the kernel. When we come to do socket_set_epoll_enable,
   * there might not be enough resources to bring it back!
   *
   * The next idea you might have is to set the flags to 0. However, events
   * always trigger on EPOLLERR and EPOLLHUP, even if libdbus isn't actually
   * delivering them to a DBusWatch. Because epoll is level-triggered by
   * default, we'll busy-loop on an unhandled error or hangup; not good.
   *
   * So, let's set it to be edge-triggered: then the worst case is that
   * we return from poll immediately on one iteration, ignore it because no
   * watch is enabled, then go back to normal. When we re-enable a watch
   * we'll switch back to level-triggered and be notified again (verified to
   * work on 2.6.32). Compile this file with -DTEST_BEHAVIOUR_OF_EPOLLET for
   * test code.
   */
  _DBUS_ZERO (event);
  event.data.fd = fd;
  event.events = EPOLLET;

  if (epoll_ctl (self->epfd, EPOLL_CTL_MOD, fd, &event) == 0)
    return;

  err = errno;
  _dbus_warn ("Error when trying to watch fd %d: %s", fd,
              strerror (err));
}

static void
socket_set_epoll_remove (DBusPollableSet  *set,
                         DBusPollable      fd)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);
  int err;
  /* Kernels < 2.6.9 require a non-NULL struct pointer, even though its
   * contents are ignored */
  struct epoll_event dummy;
  _DBUS_ZERO (dummy);

  if (epoll_ctl (self->epfd, EPOLL_CTL_DEL, fd, &dummy) == 0)
    return;

  err = errno;
  _dbus_warn ("Error when trying to remove fd %d: %s", fd, strerror (err));
}

/* Optimally, this should be the same as in DBusLoop: we use it to translate
 * between struct epoll_event and DBusSocketEvent without allocating heap
 * memory. */
#define N_STACK_DESCRIPTORS 64

static int
socket_set_epoll_poll (DBusPollableSet   *set,
                       DBusPollableEvent *revents,
                       int                max_events,
                       int                timeout_ms)
{
  DBusPollableSetEpoll *self = socket_set_epoll_cast (set);
  struct epoll_event events[N_STACK_DESCRIPTORS];
  int n_ready;
  int i;

  _dbus_assert (max_events > 0);

  n_ready = epoll_wait (self->epfd, events,
                        MIN (_DBUS_N_ELEMENTS (events), max_events),
                        timeout_ms);

  if (n_ready <= 0)
    return n_ready;

  for (i = 0; i < n_ready; i++)
    {
      revents[i].fd = events[i].data.fd;
      revents[i].flags = epoll_events_to_watch_flags (events[i].events);
    }

  return n_ready;
}

DBusPollableSetClass _dbus_pollable_set_epoll_class = {
    socket_set_epoll_free,
    socket_set_epoll_add,
    socket_set_epoll_remove,
    socket_set_epoll_enable,
    socket_set_epoll_disable,
    socket_set_epoll_poll
};

#ifdef TEST_BEHAVIOUR_OF_EPOLLET
/* usage: cat /dev/null | ./epoll
 *
 * desired output:
 * ctl ADD: 0
 * wait for HUP, edge-triggered: 1
 * wait for HUP again: 0
 * ctl MOD: 0
 * wait for HUP: 1
 */

#include <sys/epoll.h>

#include <stdio.h>

int
main (void)
{
  struct epoll_event input;
  struct epoll_event output;
  int epfd = epoll_create1 (EPOLL_CLOEXEC);
  int fd = 0; /* stdin */
  int ret;

  _DBUS_ZERO (input);

  input.events = EPOLLHUP | EPOLLET;
  ret = epoll_ctl (epfd, EPOLL_CTL_ADD, fd, &input);
  printf ("ctl ADD: %d\n", ret);

  ret = epoll_wait (epfd, &output, 1, -1);
  printf ("wait for HUP, edge-triggered: %d\n", ret);

  ret = epoll_wait (epfd, &output, 1, 1);
  printf ("wait for HUP again: %d\n", ret);

  input.events = EPOLLHUP;
  ret = epoll_ctl (epfd, EPOLL_CTL_MOD, fd, &input);
  printf ("ctl MOD: %d\n", ret);

  ret = epoll_wait (epfd, &output, 1, -1);
  printf ("wait for HUP: %d\n", ret);

  return 0;
}

#endif /* TEST_BEHAVIOUR_OF_EPOLLET */

#endif /* !DOXYGEN_SHOULD_SKIP_THIS */
