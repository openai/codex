/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-message-private.h header shared between dbus-message.c and dbus-message-util.c
 *
 * Copyright (C) 2005  Red Hat Inc.
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
#ifndef DBUS_MESSAGE_PRIVATE_H
#define DBUS_MESSAGE_PRIVATE_H

#include <dbus/dbus-message.h>
#include <dbus/dbus-message-internal.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-dataslot.h>
#include <dbus/dbus-marshal-header.h>

DBUS_BEGIN_DECLS

/**
 * @addtogroup DBusMessageInternals
 * @{
 */

/**
 * @typedef DBusMessageLoader
 *
 * The DBusMessageLoader object encapsulates the process of converting
 * a byte stream into a series of DBusMessage. It buffers the incoming
 * bytes as efficiently as possible, and generates a queue of
 * messages. DBusMessageLoader is typically used as part of a
 * DBusTransport implementation. The DBusTransport then hands off
 * the loaded messages to a DBusConnection, making the messages
 * visible to the application.
 *
 * @todo write tests for break-loader that a) randomly delete header
 * fields and b) set string fields to zero-length and other funky
 * values.
 *
 */

/**
 * Implementation details of DBusMessageLoader.
 * All members are private.
 */
struct DBusMessageLoader
{
  int refcount;        /**< Reference count. */

  DBusString data;     /**< Buffered data */

  DBusList *messages;  /**< Complete messages. */

  long max_message_size; /**< Maximum size of a message */
  long max_message_unix_fds; /**< Maximum unix fds in a message */

  DBusValidity corruption_reason; /**< why we were corrupted */

  unsigned int corrupted : 1; /**< We got broken data, and are no longer working */

  unsigned int buffer_outstanding : 1; /**< Someone is using the buffer to read */

#ifdef HAVE_UNIX_FD_PASSING
  unsigned int unix_fds_outstanding : 1; /**< Someone is using the unix fd array to read */

  int *unix_fds; /**< File descriptors that have been read from the transport but not yet been handed to any message. Array will be allocated at first use. */
  unsigned n_unix_fds_allocated; /**< Number of file descriptors this array has space for */
  unsigned n_unix_fds; /**< Number of valid file descriptors in array */
  void (* unix_fds_change) (void *); /**< Notify when the pending fds change */
  void *unix_fds_change_data;
#endif
};


/** How many bits are in the changed_stamp used to validate iterators */
#define CHANGED_STAMP_BITS 21

/**
 * @brief Internals of DBusMessage
 *
 * Object representing a message received from or to be sent to
 * another application. This is an opaque object, all members
 * are private.
 */
struct DBusMessage
{
  DBusAtomic refcount; /**< Reference count */

  DBusHeader header; /**< Header network data and associated cache */

  DBusString body;   /**< Body network data. */

  unsigned int locked : 1; /**< Message being sent, no modifications allowed. */

#ifndef DBUS_DISABLE_CHECKS
  unsigned int in_cache : 1; /**< Has been "freed" since it's in the cache (this is a debug feature) */
#endif

  DBusList *counters;   /**< 0-N DBusCounter used to track message size/unix fds. */
  long size_counter_delta;   /**< Size we incremented the size counters by.   */

  dbus_uint32_t changed_stamp : CHANGED_STAMP_BITS; /**< Incremented when iterators are invalidated. */

  DBusDataSlotList slot_list;   /**< Data stored by allocated integer ID */

#ifndef DBUS_DISABLE_CHECKS
  int generation; /**< _dbus_current_generation when message was created */
#endif

#ifdef HAVE_UNIX_FD_PASSING
  int *unix_fds;
  /**< Unix file descriptors associated with this message. These are
     closed when the message is destroyed, hence make sure to dup()
     them when adding or removing them here. */
  unsigned n_unix_fds; /**< Number of valid fds in the array */
  unsigned n_unix_fds_allocated; /**< Allocated size of the array */

  long unix_fd_counter_delta; /**< Size we incremented the unix fd counter by */
#endif
};

DBUS_PRIVATE_EXPORT
dbus_bool_t _dbus_message_iter_get_args_valist (DBusMessageIter *iter,
                                                DBusError       *error,
                                                int              first_arg_type,
                                                va_list          var_args);

/** @} */

DBUS_END_DECLS

#endif /* DBUS_MESSAGE_H */
