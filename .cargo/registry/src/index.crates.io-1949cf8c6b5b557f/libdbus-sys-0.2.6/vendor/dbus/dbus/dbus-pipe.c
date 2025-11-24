/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-pipe.c pipe implementation (internal to D-Bus implementation)
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
#include "dbus-pipe.h"

/*
 * init a pipe instance.
 *
 * @param pipe the pipe
 * @param fd the file descriptor to init from 
 */
void
_dbus_pipe_init (DBusPipe *pipe,
                 int       fd)
{
  pipe->fd = fd;
}

/**
 * init a pipe with stdout
 *
 * @param pipe the pipe
 */
void
_dbus_pipe_init_stdout (DBusPipe *pipe)
{
  _dbus_pipe_init (pipe, 1);
}

/**
 * check if a pipe is valid; pipes can be set invalid, similar to
 * a -1 file descriptor.
 *
 * @param pipe the pipe instance
 * @returns #FALSE if pipe is not valid
 */
dbus_bool_t
_dbus_pipe_is_valid(DBusPipe *pipe)
{
  return pipe->fd >= 0;
}

/**
 * Check if a pipe is stdout or stderr.
 *
 * @param pipe the pipe instance
 * @returns #TRUE if pipe is one of the standard out/err channels
 */
dbus_bool_t
_dbus_pipe_is_stdout_or_stderr (DBusPipe *pipe)
{
  return pipe->fd == 1 || pipe->fd == 2;
}

/**
 * Initializes a pipe to an invalid value.
 * @param pipe the pipe
 */
void
_dbus_pipe_invalidate (DBusPipe *pipe)
{
  pipe->fd = -1;
}
