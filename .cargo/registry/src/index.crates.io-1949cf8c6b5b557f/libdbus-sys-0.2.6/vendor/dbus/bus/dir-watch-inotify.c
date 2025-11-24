/* -*- mode: C; c-file-style: "gnu" -*- */
/* dir-watch-inotify.c  OS specific directory change notification for message bus
 *
 * Copyright (C) 2003 Red Hat, Inc.
 *           (c) 2006 Mandriva
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

/* Be careful, this file is not Linux-only: QNX also uses it */

#include <config.h>

#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
/* QNX's inotify is broken, and requires stdint.h to be manually included first */
#include <stdint.h>
#include <sys/inotify.h>
#include <sys/types.h>
#include <signal.h>
#include <errno.h>

#include <dbus/dbus-internals.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-sysdeps-unix.h>
#include <dbus/dbus-watch.h>
#include "dir-watch.h"

#define MAX_DIRS_TO_WATCH 128
#define INOTIFY_EVENT_SIZE (sizeof(struct inotify_event))
#define INOTIFY_BUF_LEN (1024 * (INOTIFY_EVENT_SIZE + 16))

/* use a static array to avoid handling OOM */
static int wds[MAX_DIRS_TO_WATCH];
static char *dirs[MAX_DIRS_TO_WATCH];
static int num_wds = 0;
static int inotify_fd = -1;
static DBusWatch *watch = NULL;
static DBusLoop *loop = NULL;

static dbus_bool_t
_handle_inotify_watch (DBusWatch *passed_watch, unsigned int flags, void *data)
{
  char buffer[INOTIFY_BUF_LEN];
  ssize_t ret = 0;
#ifdef DBUS_ENABLE_VERBOSE_MODE
  int i = 0;
#endif

  ret = read (inotify_fd, buffer, INOTIFY_BUF_LEN);
  if (ret < 0)
    _dbus_verbose ("Error reading inotify event: '%s'\n", _dbus_strerror(errno));
  else if (!ret)
    _dbus_verbose ("Error reading inotify event: buffer too small\n");
  else
    {
      _dbus_verbose ("Sending SIGHUP signal on reception of %ld inotify event(s)\n", (long) ret);
      (void) kill (_dbus_getpid (), SIGHUP);
    }

#ifdef DBUS_ENABLE_VERBOSE_MODE
  while (i < ret)
    {
      struct inotify_event *ev;

      ev = (struct inotify_event *) &buffer[i];
      i += INOTIFY_EVENT_SIZE + ev->len;
      if (ev->len)
        _dbus_verbose ("event name: '%s'\n", ev->name);
      _dbus_verbose ("inotify event: wd=%d mask=%u cookie=%u len=%u\n", ev->wd, ev->mask, ev->cookie, ev->len);
    }
#endif

  return TRUE;
}

#include <stdio.h>

static void
_set_watched_dirs_internal (DBusList **directories)
{
  int new_wds[MAX_DIRS_TO_WATCH];
  char *new_dirs[MAX_DIRS_TO_WATCH];
  DBusList *link;
  int i, j, wd;

  for (i = 0; i < MAX_DIRS_TO_WATCH; i++)
    {
      new_wds[i] = -1;
      new_dirs[i] = NULL;
    }

  i = 0;
  link = _dbus_list_get_first_link (directories);
  while (link != NULL && i < MAX_DIRS_TO_WATCH)
    {
      new_dirs[i++] = (char *)link->data;
      link = _dbus_list_get_next_link (directories, link);
    }

  if (link != NULL)
    {
      _dbus_warn ("Too many directories to watch them all, only watching first %d.", MAX_DIRS_TO_WATCH);
    }

  /* Look for directories in both the old and new sets, if
   * we find one, move its data into the new set.
   */
  for (i = 0; new_dirs[i]; i++)
    {
      for (j = 0; j < num_wds; j++)
        {
          if (dirs[j] && strcmp (new_dirs[i], dirs[j]) == 0)
            {
              new_wds[i] = wds[j];
              new_dirs[i] = dirs[j];
              wds[j] = -1;
              dirs[j] = NULL;
              break;
            }
        }
    }

  /* Any directories we find in "wds" with a nonzero fd must
   * not be in the new set, so perform cleanup now.
   */
  for (j = 0; j < num_wds; j++)
    {
      if (wds[j] != -1)
        {
          inotify_rm_watch (inotify_fd, wds[j]);
          dbus_free (dirs[j]);
          wds[j] = -1;
          dirs[j] = NULL;
        }
    }

  for (i = 0; new_dirs[i]; i++)
    {
      if (new_wds[i] == -1)
        {
          /* FIXME - less lame error handling for failing to add a watch; we may need to sleep. */
          wd = inotify_add_watch (inotify_fd, new_dirs[i], IN_CLOSE_WRITE | IN_DELETE | IN_MOVED_TO | IN_MOVED_FROM);
          if (wd < 0)
            {
              /* Not all service directories need to exist. */
              if (errno != ENOENT)
                {
                  _dbus_warn ("Cannot setup inotify for '%s'; error '%s'", new_dirs[i], _dbus_strerror (errno));
                  goto out;
                }
              else
                {
                  new_wds[i] = -1;
                  new_dirs[i] = NULL;
                  continue;
                }
            }
          new_wds[i] = wd;
          new_dirs[i] = _dbus_strdup (new_dirs[i]);
          if (!new_dirs[i])
            {
              /* FIXME have less lame handling for OOM, we just silently fail to
               * watch.  (In reality though, the whole OOM handling in dbus is stupid
               * but we won't go into that in this comment =) )
               */
              inotify_rm_watch (inotify_fd, wd);
              new_wds[i] = -1;
            }
        }
    }

  num_wds = i;

  for (i = 0; i < MAX_DIRS_TO_WATCH; i++)
    {
      wds[i] = new_wds[i];
      dirs[i] = new_dirs[i];
    }

 out:;
}

#include <stdio.h>
static void
_shutdown_inotify (void *data)
{
  DBusList *empty = NULL;

  if (inotify_fd == -1)
    return;

  _set_watched_dirs_internal (&empty);

  if (watch != NULL)
    {
      _dbus_loop_remove_watch (loop, watch);
      _dbus_watch_invalidate (watch);
      _dbus_watch_unref (watch);
      _dbus_loop_unref (loop);
    }
  watch = NULL;
  loop = NULL;

  close (inotify_fd);
  inotify_fd = -1;
}

static int
_init_inotify (BusContext *context)
{
  int ret = 0;

  if (inotify_fd == -1)
    {
#ifdef HAVE_INOTIFY_INIT1
      inotify_fd = inotify_init1 (IN_CLOEXEC);
      /* This ensures we still run on older Linux kernels.
       * https://bugs.freedesktop.org/show_bug.cgi?id=23957
       */
      if (inotify_fd < 0)
        inotify_fd = inotify_init ();
#else
      inotify_fd = inotify_init ();
#endif
      if (inotify_fd < 0)
        {
          _dbus_warn ("Cannot initialize inotify: %s", _dbus_strerror (errno));
          goto out;
        }

      /* In the inotify_init1 case this is unnecessary but harmless,
       * in the other cases it's necessary */
      _dbus_fd_set_close_on_exec (inotify_fd);

      loop = bus_context_get_loop (context);
      _dbus_loop_ref (loop);

      watch = _dbus_watch_new (inotify_fd, DBUS_WATCH_READABLE, TRUE,
                               _handle_inotify_watch, NULL, NULL);

      if (watch == NULL)
        {
          _dbus_warn ("Unable to create inotify watch");
          goto out;
        }

      if (!_dbus_loop_add_watch (loop, watch))
        {
          _dbus_warn ("Unable to add reload watch to main loop");
          _dbus_watch_unref (watch);
          watch = NULL;
          goto out;
        }

      if (!_dbus_register_shutdown_func (_shutdown_inotify, NULL))
      {
          _dbus_warn ("Unable to register shutdown func");
          _dbus_watch_unref (watch);
          watch = NULL;
          goto out;
      }
    }

  ret = 1;

out:
  return ret;
}

void
bus_set_watched_dirs (BusContext *context, DBusList **directories)
{
  if (!_init_inotify (context))
    return;

  _set_watched_dirs_internal (directories);
}
