/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-pipe-win.c windows related pipe implementation
 *
 * Copyright (C) 2002, 2003, 2006  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
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

#include <config.h>
#include "dbus-protocol.h"
#include "dbus-string.h"
#include "dbus-internals.h"
#include "dbus-pipe.h"

#include <windows.h>
#include <io.h>

/**
 * write data to a pipe.
 *
 * @param pipe the pipe instance
 * @param buffer the buffer to write data from
 * @param start the first byte in the buffer to write
 * @param len the number of bytes to try to write
 * @param error error return
 * @returns the number of bytes written or -1 on error
 */
int
_dbus_pipe_write (DBusPipe         *pipe,
                  const DBusString *buffer,
                  int               start,
                  int               len,
                  DBusError        *error)
{
  const char *buffer_c = _dbus_string_get_const_data (buffer);
  int written;

  written = _write (pipe->fd, buffer_c + start, len);

  if (written >= 0)
    return written;

  dbus_set_error (error, _dbus_error_from_system_errno (),
                  "Writing to pipe: %s",
                  _dbus_strerror_from_errno ());
  return -1;
}

/**
 * close a pipe.
 *
 * @param pipe the pipe instance
 * @param error return location for an error
 * @returns #FALSE if error is set
 */
int
_dbus_pipe_close  (DBusPipe         *pipe,
                   DBusError        *error)
{
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (_close (pipe->fd) != 0)
    {
      dbus_set_error (error, _dbus_error_from_system_errno (),
                      "Could not close pipe fd %d: %s", pipe->fd,
                      _dbus_strerror_from_errno ());
      return -1;
    }
  else
    {
      _dbus_pipe_invalidate (pipe);
      return 0;
    }
}
