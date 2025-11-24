/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-message-util.c Would be in dbus-message.c, but only used by bus/tests
 *
 * Copyright 2017 Collabora Ltd.
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

#include "dbus-message-private.h"

/**
 * @addtogroup DBusMessageInternals
 * @{
 */

/**
 * Gets the number of unix fds attached to this message.
 *
 * @param message the message
 * @returns the number of file descriptors
 */
unsigned int
_dbus_message_get_n_unix_fds (DBusMessage *message)
{
#ifdef HAVE_UNIX_FD_PASSING
  return message->n_unix_fds;
#else
  return 0;
#endif
}

/** @} */
