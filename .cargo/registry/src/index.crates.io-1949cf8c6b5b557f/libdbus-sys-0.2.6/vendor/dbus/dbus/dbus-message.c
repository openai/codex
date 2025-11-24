/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-message.c  DBusMessage object
 *
 * Copyright (C) 2002, 2003, 2004, 2005  Red Hat Inc.
 * Copyright (C) 2002, 2003  CodeFactory AB
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
#include "dbus-internals.h"
#include "dbus-marshal-recursive.h"
#include "dbus-marshal-validate.h"
#include "dbus-marshal-byteswap.h"
#include "dbus-marshal-header.h"
#include "dbus-signature.h"
#include "dbus-message-private.h"
#include "dbus-object-tree.h"
#include "dbus-memory.h"
#include "dbus-list.h"
#include "dbus-threads-internal.h"
#ifdef HAVE_UNIX_FD_PASSING
#include "dbus-sysdeps.h"
#include "dbus-sysdeps-unix.h"
#endif

#include <string.h>

#define _DBUS_TYPE_IS_STRINGLIKE(type) \
  (type == DBUS_TYPE_STRING || type == DBUS_TYPE_SIGNATURE || \
   type == DBUS_TYPE_OBJECT_PATH)

static void dbus_message_finalize (DBusMessage *message);

/**
 * @defgroup DBusMessageInternals DBusMessage implementation details
 * @ingroup DBusInternals
 * @brief DBusMessage private implementation details.
 *
 * The guts of DBusMessage and its methods.
 *
 * @{
 */

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
static dbus_bool_t
_dbus_enable_message_cache (void)
{
  static int enabled = -1;

  if (enabled < 0)
    {
      const char *s = _dbus_getenv ("DBUS_MESSAGE_CACHE");

      enabled = TRUE;

      if (s && *s)
        {
          if (*s == '0')
            enabled = FALSE;
          else if (*s == '1')
            enabled = TRUE;
          else
            _dbus_warn ("DBUS_MESSAGE_CACHE should be 0 or 1 if set, not '%s'",
                s);
        }
    }

  return enabled;
}
#else
    /* constant expression, should be optimized away */
#   define _dbus_enable_message_cache() (TRUE)
#endif

#ifndef _dbus_message_trace_ref
void
_dbus_message_trace_ref (DBusMessage *message,
                         int          old_refcount,
                         int          new_refcount,
                         const char  *why)
{
  static int enabled = -1;

  _dbus_trace_ref ("DBusMessage", message, old_refcount, new_refcount, why,
      "DBUS_MESSAGE_TRACE", &enabled);
}
#endif

/* Not thread locked, but strictly const/read-only so should be OK
 */
/** An static string representing an empty signature */
_DBUS_STRING_DEFINE_STATIC(_dbus_empty_signature_str,  "");

/* these have wacky values to help trap uninitialized iterators;
 * but has to fit in 3 bits
 */
enum {
  DBUS_MESSAGE_ITER_TYPE_READER = 3,
  DBUS_MESSAGE_ITER_TYPE_WRITER = 7
};

/** typedef for internals of message iterator */
typedef struct DBusMessageRealIter DBusMessageRealIter;

/**
 * @brief Internals of DBusMessageIter
 *
 * Object representing a position in a message. All fields are internal.
 */
struct DBusMessageRealIter
{
  DBusMessage *message; /**< Message used */
  dbus_uint32_t changed_stamp : CHANGED_STAMP_BITS; /**< stamp to detect invalid iters */
  dbus_uint32_t iter_type : 3;      /**< whether this is a reader or writer iter */
  dbus_uint32_t sig_refcount : 8;   /**< depth of open_signature() */
  union
  {
    DBusTypeWriter writer; /**< writer */
    DBusTypeReader reader; /**< reader */
  } u; /**< the type writer or reader that does all the work */
};

/**
 * Layout of a DBusMessageIter on the stack in dbus 1.10.0. This is no
 * longer used, but for ABI compatibility we need to assert that the
 * new layout is the same size.
 */
typedef struct
{
  void *dummy1;
  void *dummy2;
  dbus_uint32_t dummy3;
  int dummy4;
  int dummy5;
  int dummy6;
  int dummy7;
  int dummy8;
  int dummy9;
  int dummy10;
  int dummy11;
  int pad1;
  int pad2;
  void *pad3;
} DBusMessageIter_1_10_0;

static void
get_const_signature (DBusHeader        *header,
                     const DBusString **type_str_p,
                     int               *type_pos_p)
{
  if (_dbus_header_get_field_raw (header,
                                  DBUS_HEADER_FIELD_SIGNATURE,
                                  type_str_p,
                                  type_pos_p))
    {
      *type_pos_p += 1; /* skip the signature length which is 1 byte */
    }
  else
    {
      *type_str_p = &_dbus_empty_signature_str;
      *type_pos_p = 0;
    }
}

/**
 * Swaps the message to compiler byte order if required
 *
 * @param message the message
 */
static void
_dbus_message_byteswap (DBusMessage *message)
{
  const DBusString *type_str;
  int type_pos;
  char byte_order;

  byte_order = _dbus_header_get_byte_order (&message->header);

  if (byte_order == DBUS_COMPILER_BYTE_ORDER)
    return;

  _dbus_verbose ("Swapping message into compiler byte order\n");
  
  get_const_signature (&message->header, &type_str, &type_pos);
  
  _dbus_marshal_byteswap (type_str, type_pos,
                          byte_order,
                          DBUS_COMPILER_BYTE_ORDER,
                          &message->body, 0);

  _dbus_header_byteswap (&message->header, DBUS_COMPILER_BYTE_ORDER);
  _dbus_assert (_dbus_header_get_byte_order (&message->header) ==
                DBUS_COMPILER_BYTE_ORDER);
}

/** byte-swap the message if it doesn't match our byte order.
 *  Called only when we need the message in our own byte order,
 *  normally when reading arrays of integers or doubles.
 *  Otherwise should not be called since it would do needless
 *  work.
 */
#define ensure_byte_order(message) _dbus_message_byteswap (message)

/**
 * Gets the data to be sent over the network for this message.
 * The header and then the body should be written out.
 * This function is guaranteed to always return the same
 * data once a message is locked (with dbus_message_lock()).
 *
 * @param message the message.
 * @param header return location for message header data.
 * @param body return location for message body data.
 */
void
_dbus_message_get_network_data (DBusMessage          *message,
                                const DBusString    **header,
                                const DBusString    **body)
{
  _dbus_assert (message->locked);

  *header = &message->header.data;
  *body = &message->body;
}

/**
 * Gets the unix fds to be sent over the network for this message.
 * This function is guaranteed to always return the same data once a
 * message is locked (with dbus_message_lock()).
 *
 * @param message the message.
 * @param fds return location of unix fd array
 * @param n_fds return number of entries in array
 */
void _dbus_message_get_unix_fds(DBusMessage *message,
                                const int  **fds,
                                unsigned    *n_fds)
{
  _dbus_assert (message->locked);

#ifdef HAVE_UNIX_FD_PASSING
  *fds = message->unix_fds;
  *n_fds = message->n_unix_fds;
#else
  *fds = NULL;
  *n_fds = 0;
#endif
}

/**
 * Remove every header field not known to this version of dbus.
 *
 * @param message the message
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_message_remove_unknown_fields (DBusMessage *message)
{
  return _dbus_header_remove_unknown_fields (&message->header);
}

/**
 * Sets the serial number of a message.
 * This can only be done once on a message.
 *
 * DBusConnection will automatically set the serial to an appropriate value 
 * when the message is sent; this function is only needed when encapsulating 
 * messages in another protocol, or otherwise bypassing DBusConnection.
 *
 * @param message the message
 * @param serial the serial
 */
void 
dbus_message_set_serial (DBusMessage   *message,
                         dbus_uint32_t  serial)
{
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (!message->locked);

  _dbus_header_set_serial (&message->header, serial);
}

/**
 * Adds a counter to be incremented immediately with the size/unix fds
 * of this message, and decremented by the size/unix fds of this
 * message when this message if finalized.  The link contains a
 * counter with its refcount already incremented, but the counter
 * itself not incremented.  Ownership of link and counter refcount is
 * passed to the message.
 *
 * This function may be called with locks held. As a result, the counter's
 * notify function is not called; the caller is expected to either call
 * _dbus_counter_notify() on the counter when they are no longer holding
 * locks, or take the same action that would be taken by the notify function.
 *
 * @param message the message
 * @param link link with counter as data
 */
void
_dbus_message_add_counter_link (DBusMessage  *message,
                                DBusList     *link)
{
  /* right now we don't recompute the delta when message
   * size changes, and that's OK for current purposes
   * I think, but could be important to change later.
   * Do recompute it whenever there are no outstanding counters,
   * since it's basically free.
   */
  if (message->counters == NULL)
    {
      message->size_counter_delta =
        _dbus_string_get_length (&message->header.data) +
        _dbus_string_get_length (&message->body);

#ifdef HAVE_UNIX_FD_PASSING
      message->unix_fd_counter_delta = message->n_unix_fds;
#endif

#if 0
      _dbus_verbose ("message has size %ld\n",
                     message->size_counter_delta);
#endif
    }

  _dbus_list_append_link (&message->counters, link);

  _dbus_counter_adjust_size (link->data, message->size_counter_delta);

#ifdef HAVE_UNIX_FD_PASSING
  _dbus_counter_adjust_unix_fd (link->data, message->unix_fd_counter_delta);
#endif
}

/**
 * Adds a counter to be incremented immediately with the size/unix fds
 * of this message, and decremented by the size/unix fds of this
 * message when this message if finalized.
 *
 * This function may be called with locks held. As a result, the counter's
 * notify function is not called; the caller is expected to either call
 * _dbus_counter_notify() on the counter when they are no longer holding
 * locks, or take the same action that would be taken by the notify function.
 *
 * @param message the message
 * @param counter the counter
 * @returns #FALSE if no memory
 */
dbus_bool_t
_dbus_message_add_counter (DBusMessage *message,
                           DBusCounter *counter)
{
  DBusList *link;

  link = _dbus_list_alloc_link (counter);
  if (link == NULL)
    return FALSE;

  _dbus_counter_ref (counter);
  _dbus_message_add_counter_link (message, link);

  return TRUE;
}

/**
 * Removes a counter tracking the size/unix fds of this message, and
 * decrements the counter by the size/unix fds of this message.
 *
 * @param message the message
 * @param counter the counter
 */
void
_dbus_message_remove_counter (DBusMessage  *message,
                              DBusCounter  *counter)
{
  DBusList *link;

  link = _dbus_list_find_last (&message->counters,
                               counter);
  _dbus_assert (link != NULL);

  _dbus_list_remove_link (&message->counters, link);

  _dbus_counter_adjust_size (counter, - message->size_counter_delta);

#ifdef HAVE_UNIX_FD_PASSING
  _dbus_counter_adjust_unix_fd (counter, - message->unix_fd_counter_delta);
#endif

  _dbus_counter_notify (counter);
  _dbus_counter_unref (counter);
}

/**
 * Locks a message. Allows checking that applications don't keep a
 * reference to a message in the outgoing queue and change it
 * underneath us. Messages are locked when they enter the outgoing
 * queue (dbus_connection_send_message()), and the library complains
 * if the message is modified while locked. This function may also 
 * called externally, for applications wrapping D-Bus in another protocol.
 *
 * @param message the message to lock.
 */
void
dbus_message_lock (DBusMessage  *message)
{
  if (!message->locked)
    {
      _dbus_header_update_lengths (&message->header,
                                   _dbus_string_get_length (&message->body));

      /* must have a signature if you have a body */
      _dbus_assert (_dbus_string_get_length (&message->body) == 0 ||
                    dbus_message_get_signature (message) != NULL);

      message->locked = TRUE;
    }
}

static dbus_bool_t
set_or_delete_string_field (DBusMessage *message,
                            int          field,
                            int          typecode,
                            const char  *value)
{
  if (value == NULL)
    return _dbus_header_delete_field (&message->header, field);
  else
    return _dbus_header_set_field_basic (&message->header,
                                         field,
                                         typecode,
                                         &value);
}

/* Message Cache
 *
 * We cache some DBusMessage to reduce the overhead of allocating
 * them.  In my profiling this consistently made about an 8%
 * difference.  It avoids the malloc for the message, the malloc for
 * the slot list, the malloc for the header string and body string,
 * and the associated free() calls. It does introduce another global
 * lock which could be a performance issue in certain cases.
 *
 * For the echo client/server the round trip time goes from around
 * .000077 to .000069 with the message cache on my laptop. The sysprof
 * change is as follows (numbers are cumulative percentage):
 *
 *  with message cache implemented as array as it is now (0.000069 per):
 *    new_empty_header           1.46
 *      mutex_lock               0.56    # i.e. _DBUS_LOCK(message_cache)
 *      mutex_unlock             0.25
 *      self                     0.41
 *    unref                      2.24
 *      self                     0.68
 *      list_clear               0.43
 *      mutex_lock               0.33    # i.e. _DBUS_LOCK(message_cache)
 *      mutex_unlock             0.25
 *
 *  with message cache implemented as list (0.000070 per roundtrip):
 *    new_empty_header           2.72
 *      list_pop_first           1.88
 *    unref                      3.3
 *      list_prepend             1.63
 *
 * without cache (0.000077 per roundtrip):
 *    new_empty_header           6.7
 *      string_init_preallocated 3.43
 *        dbus_malloc            2.43
 *      dbus_malloc0             2.59
 *
 *    unref                      4.02
 *      string_free              1.82
 *        dbus_free              1.63
 *      dbus_free                0.71
 *
 * If you implement the message_cache with a list, the primary reason
 * it's slower is that you add another thread lock (on the DBusList
 * mempool).
 */

/** Avoid caching huge messages */
#define MAX_MESSAGE_SIZE_TO_CACHE 10 * _DBUS_ONE_KILOBYTE

/** Avoid caching too many messages */
#define MAX_MESSAGE_CACHE_SIZE    5

/* Protected by _DBUS_LOCK (message_cache) */
static DBusMessage *message_cache[MAX_MESSAGE_CACHE_SIZE];
static int message_cache_count = 0;
static dbus_bool_t message_cache_shutdown_registered = FALSE;

static void
dbus_message_cache_shutdown (void *data)
{
  int i;

  if (!_DBUS_LOCK (message_cache))
    _dbus_assert_not_reached ("we would have initialized global locks "
        "before registering a shutdown function");

  i = 0;
  while (i < MAX_MESSAGE_CACHE_SIZE)
    {
      if (message_cache[i])
        dbus_message_finalize (message_cache[i]);

      ++i;
    }

  message_cache_count = 0;
  message_cache_shutdown_registered = FALSE;

  _DBUS_UNLOCK (message_cache);
}

/**
 * Tries to get a message from the message cache.  The retrieved
 * message will have junk in it, so it still needs to be cleared out
 * in dbus_message_new_empty_header()
 *
 * @returns the message, or #NULL if none cached
 */
static DBusMessage*
dbus_message_get_cached (void)
{
  DBusMessage *message;
  int i;

  message = NULL;

  if (!_DBUS_LOCK (message_cache))
    {
      /* we'd have initialized global locks before caching anything,
       * so there can't be anything in the cache */
      return NULL;
    }

  _dbus_assert (message_cache_count >= 0);

  if (message_cache_count == 0)
    {
      _DBUS_UNLOCK (message_cache);
      return NULL;
    }

  /* This is not necessarily true unless count > 0, and
   * message_cache is uninitialized until the shutdown is
   * registered
   */
  _dbus_assert (message_cache_shutdown_registered);

  i = 0;
  while (i < MAX_MESSAGE_CACHE_SIZE)
    {
      if (message_cache[i])
        {
          message = message_cache[i];
          message_cache[i] = NULL;
          message_cache_count -= 1;
          break;
        }
      ++i;
    }
  _dbus_assert (message_cache_count >= 0);
  _dbus_assert (i < MAX_MESSAGE_CACHE_SIZE);
  _dbus_assert (message != NULL);

  _dbus_assert (_dbus_atomic_get (&message->refcount) == 0);

  _dbus_assert (message->counters == NULL);
  
  _DBUS_UNLOCK (message_cache);

  return message;
}

#ifdef HAVE_UNIX_FD_PASSING
static void
close_unix_fds(int *fds, unsigned *n_fds)
{
  DBusError e;
  unsigned int i;

  if (*n_fds <= 0)
    return;

  dbus_error_init(&e);

  for (i = 0; i < *n_fds; i++)
    {
      if (!_dbus_close(fds[i], &e))
        {
          _dbus_warn("Failed to close file descriptor: %s", e.message);
          dbus_error_free(&e);
        }
    }

  *n_fds = 0;

  /* We don't free the array here, in case we can recycle it later */
}
#endif

static void
free_counter (void *element,
              void *data)
{
  DBusCounter *counter = element;
  DBusMessage *message = data;

  _dbus_counter_adjust_size (counter, - message->size_counter_delta);
#ifdef HAVE_UNIX_FD_PASSING
  _dbus_counter_adjust_unix_fd (counter, - message->unix_fd_counter_delta);
#endif

  _dbus_counter_notify (counter);
  _dbus_counter_unref (counter);
}

/**
 * Tries to cache a message, otherwise finalize it.
 *
 * @param message the message
 */
static void
dbus_message_cache_or_finalize (DBusMessage *message)
{
  dbus_bool_t was_cached;
  int i;

  _dbus_assert (_dbus_atomic_get (&message->refcount) == 0);

  /* This calls application code and has to be done first thing
   * without holding the lock
   */
  _dbus_data_slot_list_clear (&message->slot_list);

  _dbus_list_foreach (&message->counters,
                      free_counter, message);
  _dbus_list_clear (&message->counters);

#ifdef HAVE_UNIX_FD_PASSING
  close_unix_fds(message->unix_fds, &message->n_unix_fds);
#endif

  was_cached = FALSE;

  if (!_DBUS_LOCK (message_cache))
    {
      /* The only way to get a non-null message goes through
       * dbus_message_get_cached() which takes the lock. */
      _dbus_assert_not_reached ("we would have initialized global locks "
          "the first time we constructed a message");
    }

  if (!message_cache_shutdown_registered)
    {
      _dbus_assert (message_cache_count == 0);

      if (!_dbus_register_shutdown_func (dbus_message_cache_shutdown, NULL))
        goto out;

      i = 0;
      while (i < MAX_MESSAGE_CACHE_SIZE)
        {
          message_cache[i] = NULL;
          ++i;
        }

      message_cache_shutdown_registered = TRUE;
    }

  _dbus_assert (message_cache_count >= 0);

  if (!_dbus_enable_message_cache ())
    goto out;

  if ((_dbus_string_get_length (&message->header.data) +
       _dbus_string_get_length (&message->body)) >
      MAX_MESSAGE_SIZE_TO_CACHE)
    goto out;

  if (message_cache_count >= MAX_MESSAGE_CACHE_SIZE)
    goto out;

  /* Find empty slot */
  i = 0;
  while (message_cache[i] != NULL)
    ++i;

  _dbus_assert (i < MAX_MESSAGE_CACHE_SIZE);

  _dbus_assert (message_cache[i] == NULL);
  message_cache[i] = message;
  message_cache_count += 1;
  was_cached = TRUE;
#ifndef DBUS_DISABLE_CHECKS
  message->in_cache = TRUE;
#endif

 out:
  _dbus_assert (_dbus_atomic_get (&message->refcount) == 0);

  _DBUS_UNLOCK (message_cache);
  
  if (!was_cached)
    dbus_message_finalize (message);
}

/*
 * Arrange for iter to be something that _dbus_message_iter_check() would
 * reject as not a valid iterator.
 */
static void
_dbus_message_real_iter_zero (DBusMessageRealIter *iter)
{
  _dbus_assert (iter != NULL);
  _DBUS_ZERO (*iter);
  /* NULL is not, strictly speaking, guaranteed to be all-bits-zero */
  iter->message = NULL;
}

/**
 * Initialize iter as if with #DBUS_MESSAGE_ITER_INIT_CLOSED. The only valid
 * operation for such an iterator is
 * dbus_message_iter_abandon_container_if_open(), which does nothing.
 */
void
dbus_message_iter_init_closed (DBusMessageIter *iter)
{
  _dbus_return_if_fail (iter != NULL);
  _dbus_message_real_iter_zero ((DBusMessageRealIter *) iter);
}

static dbus_bool_t
_dbus_message_real_iter_is_zeroed (DBusMessageRealIter *iter)
{
  return (iter != NULL && iter->message == NULL && iter->changed_stamp == 0 &&
          iter->iter_type == 0 && iter->sig_refcount == 0);
}

#if defined(DBUS_ENABLE_CHECKS) || defined(DBUS_ENABLE_ASSERT)
static dbus_bool_t
_dbus_message_iter_check (DBusMessageRealIter *iter)
{
  char byte_order;

  if (iter == NULL)
    {
      _dbus_warn_check_failed ("dbus message iterator is NULL");
      return FALSE;
    }

  if (iter->message == NULL || iter->iter_type == 0)
    {
      _dbus_warn_check_failed ("dbus message iterator has already been "
                               "closed, or is uninitialized or corrupt");
      return FALSE;
    }

  byte_order = _dbus_header_get_byte_order (&iter->message->header);

  if (iter->iter_type == DBUS_MESSAGE_ITER_TYPE_READER)
    {
      if (iter->u.reader.byte_order != byte_order)
        {
          _dbus_warn_check_failed ("dbus message changed byte order since iterator was created");
          return FALSE;
        }
      /* because we swap the message into compiler order when you init an iter */
      _dbus_assert (iter->u.reader.byte_order == DBUS_COMPILER_BYTE_ORDER);
    }
  else if (iter->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER)
    {
      if (iter->u.writer.byte_order != byte_order)
        {
          _dbus_warn_check_failed ("dbus message changed byte order since append iterator was created");
          return FALSE;
        }
      /* because we swap the message into compiler order when you init an iter */
      _dbus_assert (iter->u.writer.byte_order == DBUS_COMPILER_BYTE_ORDER);
    }
  else
    {
      _dbus_warn_check_failed ("dbus message iterator looks uninitialized or corrupted");
      return FALSE;
    }

  if (iter->changed_stamp != iter->message->changed_stamp)
    {
      _dbus_warn_check_failed ("dbus message iterator invalid because the message has been modified (or perhaps the iterator is just uninitialized)");
      return FALSE;
    }

  return TRUE;
}
#endif /* DBUS_ENABLE_CHECKS || DBUS_ENABLE_ASSERT */

/**
 * Implementation of the varargs arg-getting functions.
 * dbus_message_get_args() is the place to go for complete
 * documentation.
 *
 * @see dbus_message_get_args
 * @param iter the message iter
 * @param error error to be filled in
 * @param first_arg_type type of the first argument
 * @param var_args return location for first argument, followed by list of type/location pairs
 * @returns #FALSE if error was set
 */
dbus_bool_t
_dbus_message_iter_get_args_valist (DBusMessageIter *iter,
                                    DBusError       *error,
                                    int              first_arg_type,
                                    va_list          var_args)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  int spec_type, msg_type, i, j;
  dbus_bool_t retval;
  va_list copy_args;

  _dbus_assert (_dbus_message_iter_check (real));

  retval = FALSE;

  spec_type = first_arg_type;
  i = 0;

  /* copy var_args first, then we can do another iteration over it to
   * free memory and close unix fds if parse failed at some point.
   */
  DBUS_VA_COPY (copy_args, var_args);

  while (spec_type != DBUS_TYPE_INVALID)
    {
      msg_type = dbus_message_iter_get_arg_type (iter);

      if (msg_type != spec_type)
        {
          dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                          "Argument %d is specified to be of type \"%s\", but "
                          "is actually of type \"%s\"\n", i,
                          _dbus_type_to_string (spec_type),
                          _dbus_type_to_string (msg_type));

          goto out;
        }

      if (spec_type == DBUS_TYPE_UNIX_FD)
        {
#ifdef HAVE_UNIX_FD_PASSING
          DBusBasicValue idx;
          int *pfd, nfd;

          pfd = va_arg (var_args, int*);
          _dbus_assert(pfd);

          _dbus_type_reader_read_basic(&real->u.reader, &idx);

          if (idx.u32 >= real->message->n_unix_fds)
            {
              dbus_set_error (error, DBUS_ERROR_INCONSISTENT_MESSAGE,
                              "Message refers to file descriptor at index %i,"
                              "but has only %i descriptors attached.\n",
                              idx.u32,
                              real->message->n_unix_fds);
              goto out;
            }

          if ((nfd = _dbus_dup(real->message->unix_fds[idx.u32], error)) < 0)
            goto out;

          *pfd = nfd;
#else
          dbus_set_error (error, DBUS_ERROR_NOT_SUPPORTED,
                          "Platform does not support file desciptor passing.\n");
          goto out;
#endif
        }
      else if (dbus_type_is_basic (spec_type))
        {
          void *ptr;

          ptr = va_arg (var_args, void *);

          _dbus_assert (ptr != NULL);

          _dbus_type_reader_read_basic (&real->u.reader,
                                        ptr);
        }
      else if (spec_type == DBUS_TYPE_ARRAY)
        {
          int element_type;
          int spec_element_type;
          const void **ptr;
          int *n_elements_p;
          DBusTypeReader array;

          spec_element_type = va_arg (var_args, int);
          element_type = _dbus_type_reader_get_element_type (&real->u.reader);

          if (spec_element_type != element_type)
            {
              dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                              "Argument %d is specified to be an array of \"%s\", but "
                              "is actually an array of \"%s\"\n",
                              i,
                              _dbus_type_to_string (spec_element_type),
                              _dbus_type_to_string (element_type));

              goto out;
            }

          if (dbus_type_is_fixed (spec_element_type) &&
              element_type != DBUS_TYPE_UNIX_FD)
            {
              ptr = va_arg (var_args, const void **);
              n_elements_p = va_arg (var_args, int*);

              _dbus_assert (ptr != NULL);
              _dbus_assert (n_elements_p != NULL);

              _dbus_type_reader_recurse (&real->u.reader, &array);

              _dbus_type_reader_read_fixed_multi (&array, ptr, n_elements_p);
            }
          else if (_DBUS_TYPE_IS_STRINGLIKE (spec_element_type))
            {
              char ***str_array_p;
              int n_elements;
              char **str_array;

              str_array_p = va_arg (var_args, char***);
              n_elements_p = va_arg (var_args, int*);

              _dbus_assert (str_array_p != NULL);
              _dbus_assert (n_elements_p != NULL);

              /* Count elements in the array */
              _dbus_type_reader_recurse (&real->u.reader, &array);

              n_elements = 0;
              while (_dbus_type_reader_get_current_type (&array) != DBUS_TYPE_INVALID)
                {
                  ++n_elements;
                  _dbus_type_reader_next (&array);
                }

              str_array = dbus_new0 (char*, n_elements + 1);
              if (str_array == NULL)
                {
                  _DBUS_SET_OOM (error);
                  goto out;
                }

              /* Now go through and dup each string */
              _dbus_type_reader_recurse (&real->u.reader, &array);

              j = 0;
              while (j < n_elements)
                {
                  const char *s;
                  _dbus_type_reader_read_basic (&array,
                                                (void *) &s);
                  
                  str_array[j] = _dbus_strdup (s);
                  if (str_array[j] == NULL)
                    {
                      dbus_free_string_array (str_array);
                      _DBUS_SET_OOM (error);
                      goto out;
                    }
                  
                  ++j;
                  
                  if (!_dbus_type_reader_next (&array))
                    _dbus_assert (j == n_elements);
                }

              _dbus_assert (_dbus_type_reader_get_current_type (&array) == DBUS_TYPE_INVALID);
              _dbus_assert (j == n_elements);
              _dbus_assert (str_array[j] == NULL);

              *str_array_p = str_array;
              *n_elements_p = n_elements;
            }
#ifndef DBUS_DISABLE_CHECKS
          else
            {
              _dbus_warn ("you can't read arrays of container types (struct, variant, array) with %s for now",
                          _DBUS_FUNCTION_NAME);
              goto out;
            }
#endif
        }
#ifndef DBUS_DISABLE_CHECKS
      else
        {
          _dbus_warn ("you can only read arrays and basic types with %s for now",
                      _DBUS_FUNCTION_NAME);
          goto out;
        }
#endif

      /* how many arguments already handled */
      i++;

      spec_type = va_arg (var_args, int);
      if (!_dbus_type_reader_next (&real->u.reader) && spec_type != DBUS_TYPE_INVALID)
        {
          dbus_set_error (error, DBUS_ERROR_INVALID_ARGS,
                          "Message has only %d arguments, but more were expected", i);
          goto out;
        }
    }

  retval = TRUE;

 out:
  /* there may memory or unix fd leak in the above iteration if parse failed.
   * so we have another iteration over copy_args to free memory and close
   * unix fds.
   */
  if (!retval)
    {
      spec_type = first_arg_type;
      j = 0;

      while (j < i)
        {
          if (spec_type == DBUS_TYPE_UNIX_FD)
            {
#ifdef HAVE_UNIX_FD_PASSING
              int *pfd;

              pfd = va_arg (copy_args, int *);
              _dbus_assert(pfd);
              if (*pfd >= 0)
                {
                  _dbus_close (*pfd, NULL);
                  *pfd = -1;
                }
#endif
            }
          else if (dbus_type_is_basic (spec_type))
            {
              /* move the index forward */
              va_arg (copy_args, const void *);
            }
          else if (spec_type == DBUS_TYPE_ARRAY)
            {
              int spec_element_type;

              spec_element_type = va_arg (copy_args, int);
              if (dbus_type_is_fixed (spec_element_type))
                {
                  /* move the index forward */
                  va_arg (copy_args, const void **);
                  va_arg (copy_args, int *);
                }
              else if (_DBUS_TYPE_IS_STRINGLIKE (spec_element_type))
                {
                  char ***str_array_p;

                  str_array_p = va_arg (copy_args, char ***);
                  /* move the index forward */
                  va_arg (copy_args, int *);
                  _dbus_assert (str_array_p != NULL);
                  dbus_free_string_array (*str_array_p);
                  *str_array_p = NULL;
                }
            }

          spec_type = va_arg (copy_args, int);
          j++;
        }
    }

  va_end (copy_args);
  return retval;
}

/** @} */

/**
 * @defgroup DBusMessage DBusMessage
 * @ingroup  DBus
 * @brief Message to be sent or received over a #DBusConnection.
 *
 * A DBusMessage is the most basic unit of communication over a
 * DBusConnection. A DBusConnection represents a stream of messages
 * received from a remote application, and a stream of messages
 * sent to a remote application.
 *
 * A message has a message type, returned from
 * dbus_message_get_type().  This indicates whether the message is a
 * method call, a reply to a method call, a signal, or an error reply.
 *
 * A message has header fields such as the sender, destination, method
 * or signal name, and so forth. DBusMessage has accessor functions for
 * these, such as dbus_message_get_member().
 *
 * Convenience functions dbus_message_is_method_call(), dbus_message_is_signal(),
 * and dbus_message_is_error() check several header fields at once and are
 * slightly more efficient than checking the header fields with individual
 * accessor functions.
 *
 * Finally, a message has arguments. The number and types of arguments
 * are in the message's signature header field (accessed with
 * dbus_message_get_signature()).  Simple argument values are usually
 * retrieved with dbus_message_get_args() but more complex values such
 * as structs may require the use of #DBusMessageIter.
 *
 * The D-Bus specification goes into some more detail about header fields and
 * message types.
 * 
 * @{
 */

/**
 * @typedef DBusMessage
 *
 * Opaque data type representing a message received from or to be
 * sent to another application.
 */

/**
 * Returns the serial of a message or 0 if none has been specified.
 * The message's serial number is provided by the application sending
 * the message and is used to identify replies to this message.
 *
 * All messages received on a connection will have a serial provided
 * by the remote application.
 *
 * For messages you're sending, dbus_connection_send() will assign a
 * serial and return it to you.
 *
 * @param message the message
 * @returns the serial
 */
dbus_uint32_t
dbus_message_get_serial (DBusMessage *message)
{
  _dbus_return_val_if_fail (message != NULL, 0);

  return _dbus_header_get_serial (&message->header);
}

/**
 * Sets the reply serial of a message (the serial of the message this
 * is a reply to).
 *
 * @param message the message
 * @param reply_serial the serial we're replying to
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_reply_serial (DBusMessage   *message,
                               dbus_uint32_t  reply_serial)
{
  DBusBasicValue value;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (reply_serial != 0, FALSE); /* 0 is invalid */

  value.u32 = reply_serial;

  return _dbus_header_set_field_basic (&message->header,
                                       DBUS_HEADER_FIELD_REPLY_SERIAL,
                                       DBUS_TYPE_UINT32,
                                       &value);
}

/**
 * Returns the serial that the message is a reply to or 0 if none.
 *
 * @param message the message
 * @returns the reply serial
 */
dbus_uint32_t
dbus_message_get_reply_serial  (DBusMessage *message)
{
  dbus_uint32_t v_UINT32;

  _dbus_return_val_if_fail (message != NULL, 0);

  if (_dbus_header_get_field_basic (&message->header,
                                    DBUS_HEADER_FIELD_REPLY_SERIAL,
                                    DBUS_TYPE_UINT32,
                                    &v_UINT32))
    return v_UINT32;
  else
    return 0;
}

static void
dbus_message_finalize (DBusMessage *message)
{
  _dbus_assert (_dbus_atomic_get (&message->refcount) == 0);

  /* This calls application callbacks! */
  _dbus_data_slot_list_free (&message->slot_list);

  _dbus_list_foreach (&message->counters,
                      free_counter, message);
  _dbus_list_clear (&message->counters);

  _dbus_header_free (&message->header);
  _dbus_string_free (&message->body);

#ifdef HAVE_UNIX_FD_PASSING
  close_unix_fds(message->unix_fds, &message->n_unix_fds);
  dbus_free(message->unix_fds);
#endif

  _dbus_assert (_dbus_atomic_get (&message->refcount) == 0);

  dbus_free (message);
}

static DBusMessage*
dbus_message_new_empty_header (void)
{
  DBusMessage *message;
  dbus_bool_t from_cache;

  message = dbus_message_get_cached ();

  if (message != NULL)
    {
      from_cache = TRUE;
    }
  else
    {
      from_cache = FALSE;
      message = dbus_new0 (DBusMessage, 1);
      if (message == NULL)
        return NULL;
#ifndef DBUS_DISABLE_CHECKS
      message->generation = _dbus_current_generation;
#endif

#ifdef HAVE_UNIX_FD_PASSING
      message->unix_fds = NULL;
      message->n_unix_fds_allocated = 0;
#endif
    }

  _dbus_atomic_inc (&message->refcount);

  _dbus_message_trace_ref (message, 0, 1, "new_empty_header");

  message->locked = FALSE;
#ifndef DBUS_DISABLE_CHECKS
  message->in_cache = FALSE;
#endif
  message->counters = NULL;
  message->size_counter_delta = 0;
  message->changed_stamp = 0;

#ifdef HAVE_UNIX_FD_PASSING
  message->n_unix_fds = 0;
  message->n_unix_fds_allocated = 0;
  message->unix_fd_counter_delta = 0;
#endif

  if (!from_cache)
    _dbus_data_slot_list_init (&message->slot_list);

  if (from_cache)
    {
      _dbus_header_reinit (&message->header);
      _dbus_string_set_length (&message->body, 0);
    }
  else
    {
      if (!_dbus_header_init (&message->header))
        {
          dbus_free (message);
          return NULL;
        }

      if (!_dbus_string_init_preallocated (&message->body, 32))
        {
          _dbus_header_free (&message->header);
          dbus_free (message);
          return NULL;
        }
    }

  return message;
}

/**
 * Constructs a new message of the given message type.
 * Types include #DBUS_MESSAGE_TYPE_METHOD_CALL,
 * #DBUS_MESSAGE_TYPE_SIGNAL, and so forth.
 *
 * Usually you want to use dbus_message_new_method_call(),
 * dbus_message_new_method_return(), dbus_message_new_signal(),
 * or dbus_message_new_error() instead.
 * 
 * @param message_type type of message
 * @returns new message or #NULL if no memory
 */
DBusMessage*
dbus_message_new (int message_type)
{
  DBusMessage *message;

  _dbus_return_val_if_fail (message_type != DBUS_MESSAGE_TYPE_INVALID, NULL);

  message = dbus_message_new_empty_header ();
  if (message == NULL)
    return NULL;

  if (!_dbus_header_create (&message->header,
                            DBUS_COMPILER_BYTE_ORDER,
                            message_type,
                            NULL, NULL, NULL, NULL, NULL))
    {
      dbus_message_unref (message);
      return NULL;
    }

  return message;
}

/**
 * Constructs a new message to invoke a method on a remote
 * object. Returns #NULL if memory can't be allocated for the
 * message. The destination may be #NULL in which case no destination
 * is set; this is appropriate when using D-Bus in a peer-to-peer
 * context (no message bus). The interface may be #NULL, which means
 * that if multiple methods with the given name exist it is undefined
 * which one will be invoked.
 *
 * The path and method names may not be #NULL.
 *
 * Destination, path, interface, and method name can't contain
 * any invalid characters (see the D-Bus specification).
 * 
 * @param destination name that the message should be sent to or #NULL
 * @param path object path the message should be sent to
 * @param iface interface to invoke method on, or #NULL
 * @param method method to invoke
 *
 * @returns a new DBusMessage, free with dbus_message_unref()
 */
DBusMessage*
dbus_message_new_method_call (const char *destination,
                              const char *path,
                              const char *iface,
                              const char *method)
{
  DBusMessage *message;

  _dbus_return_val_if_fail (path != NULL, NULL);
  _dbus_return_val_if_fail (method != NULL, NULL);
  _dbus_return_val_if_fail (destination == NULL ||
                            _dbus_check_is_valid_bus_name (destination), NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_path (path), NULL);
  _dbus_return_val_if_fail (iface == NULL ||
                            _dbus_check_is_valid_interface (iface), NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_member (method), NULL);

  message = dbus_message_new_empty_header ();
  if (message == NULL)
    return NULL;

  if (!_dbus_header_create (&message->header,
                            DBUS_COMPILER_BYTE_ORDER,
                            DBUS_MESSAGE_TYPE_METHOD_CALL,
                            destination, path, iface, method, NULL))
    {
      dbus_message_unref (message);
      return NULL;
    }

  return message;
}

/**
 * Constructs a message that is a reply to a method call. Returns
 * #NULL if memory can't be allocated for the message.
 *
 * @param method_call the message being replied to
 * @returns a new DBusMessage, free with dbus_message_unref()
 */
DBusMessage*
dbus_message_new_method_return (DBusMessage *method_call)
{
  DBusMessage *message;
  const char *sender;

  _dbus_return_val_if_fail (method_call != NULL, NULL);

  sender = dbus_message_get_sender (method_call);

  /* sender is allowed to be null here in peer-to-peer case */

  message = dbus_message_new_empty_header ();
  if (message == NULL)
    return NULL;

  if (!_dbus_header_create (&message->header,
                            DBUS_COMPILER_BYTE_ORDER,
                            DBUS_MESSAGE_TYPE_METHOD_RETURN,
                            sender, NULL, NULL, NULL, NULL))
    {
      dbus_message_unref (message);
      return NULL;
    }

  dbus_message_set_no_reply (message, TRUE);

  if (!dbus_message_set_reply_serial (message,
                                      dbus_message_get_serial (method_call)))
    {
      dbus_message_unref (message);
      return NULL;
    }

  return message;
}

/**
 * Constructs a new message representing a signal emission. Returns
 * #NULL if memory can't be allocated for the message.  A signal is
 * identified by its originating object path, interface, and the name
 * of the signal.
 *
 * Path, interface, and signal name must all be valid (the D-Bus
 * specification defines the syntax of these fields).
 * 
 * @param path the path to the object emitting the signal
 * @param iface the interface the signal is emitted from
 * @param name name of the signal
 * @returns a new DBusMessage, free with dbus_message_unref()
 */
DBusMessage*
dbus_message_new_signal (const char *path,
                         const char *iface,
                         const char *name)
{
  DBusMessage *message;

  _dbus_return_val_if_fail (path != NULL, NULL);
  _dbus_return_val_if_fail (iface != NULL, NULL);
  _dbus_return_val_if_fail (name != NULL, NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_path (path), NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_interface (iface), NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_member (name), NULL);

  message = dbus_message_new_empty_header ();
  if (message == NULL)
    return NULL;

  if (!_dbus_header_create (&message->header,
                            DBUS_COMPILER_BYTE_ORDER,
                            DBUS_MESSAGE_TYPE_SIGNAL,
                            NULL, path, iface, name, NULL))
    {
      dbus_message_unref (message);
      return NULL;
    }

  dbus_message_set_no_reply (message, TRUE);

  return message;
}

/**
 * Creates a new message that is an error reply to another message.
 * Error replies are most common in response to method calls, but
 * can be returned in reply to any message.
 *
 * The error name must be a valid error name according to the syntax
 * given in the D-Bus specification. If you don't want to make
 * up an error name just use #DBUS_ERROR_FAILED.
 *
 * @param reply_to the message we're replying to
 * @param error_name the error name
 * @param error_message the error message string (or #NULL for none, but please give a message)
 * @returns a new error message object, free with dbus_message_unref()
 */
DBusMessage*
dbus_message_new_error (DBusMessage *reply_to,
                        const char  *error_name,
                        const char  *error_message)
{
  DBusMessage *message;
  const char *sender;
  DBusMessageIter iter;

  _dbus_return_val_if_fail (reply_to != NULL, NULL);
  _dbus_return_val_if_fail (error_name != NULL, NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_error_name (error_name), NULL);

  sender = dbus_message_get_sender (reply_to);

  /* sender may be NULL for non-message-bus case or
   * when the message bus is dealing with an unregistered
   * connection.
   */
  message = dbus_message_new_empty_header ();
  if (message == NULL)
    return NULL;

  if (!_dbus_header_create (&message->header,
                            DBUS_COMPILER_BYTE_ORDER,
                            DBUS_MESSAGE_TYPE_ERROR,
                            sender, NULL, NULL, NULL, error_name))
    {
      dbus_message_unref (message);
      return NULL;
    }

  dbus_message_set_no_reply (message, TRUE);

  if (!dbus_message_set_reply_serial (message,
                                      dbus_message_get_serial (reply_to)))
    {
      dbus_message_unref (message);
      return NULL;
    }

  if (error_message != NULL)
    {
      dbus_message_iter_init_append (message, &iter);
      if (!dbus_message_iter_append_basic (&iter,
                                           DBUS_TYPE_STRING,
                                           &error_message))
        {
          dbus_message_unref (message);
          return NULL;
        }
    }

  return message;
}

/**
 * Creates a new message that is an error reply to another message, allowing
 * you to use printf formatting.
 *
 * See dbus_message_new_error() for details - this function is the same
 * aside from the printf formatting.
 *
 * @todo add _DBUS_GNUC_PRINTF to this (requires moving _DBUS_GNUC_PRINTF to
 * public header, see DBUS_DEPRECATED for an example)
 * 
 * @param reply_to the original message
 * @param error_name the error name
 * @param error_format the error message format as with printf
 * @param ... format string arguments
 * @returns a new error message
 */
DBusMessage*
dbus_message_new_error_printf (DBusMessage *reply_to,
			       const char  *error_name,
			       const char  *error_format,
			       ...)
{
  va_list args;
  DBusString str;
  DBusMessage *message;

  _dbus_return_val_if_fail (reply_to != NULL, NULL);
  _dbus_return_val_if_fail (error_name != NULL, NULL);
  _dbus_return_val_if_fail (_dbus_check_is_valid_error_name (error_name), NULL);

  if (!_dbus_string_init (&str))
    return NULL;

  va_start (args, error_format);

  if (_dbus_string_append_printf_valist (&str, error_format, args))
    message = dbus_message_new_error (reply_to, error_name,
				      _dbus_string_get_const_data (&str));
  else
    message = NULL;

  _dbus_string_free (&str);

  va_end (args);

  return message;
}


/**
 * Creates a new message that is an exact replica of the message
 * specified, except that its refcount is set to 1, its message serial
 * is reset to 0, and if the original message was "locked" (in the
 * outgoing message queue and thus not modifiable) the new message
 * will not be locked.
 *
 * @todo This function can't be used in programs that try to recover from OOM errors.
 *
 * @param message the message
 * @returns the new message.or #NULL if not enough memory or Unix file descriptors (in case the message to copy includes Unix file descriptors) can be allocated.
 */
DBusMessage *
dbus_message_copy (const DBusMessage *message)
{
  DBusMessage *retval;

  _dbus_return_val_if_fail (message != NULL, NULL);

  retval = dbus_new0 (DBusMessage, 1);
  if (retval == NULL)
    return NULL;

  _dbus_atomic_inc (&retval->refcount);

  retval->locked = FALSE;
#ifndef DBUS_DISABLE_CHECKS
  retval->generation = message->generation;
#endif

  if (!_dbus_header_copy (&message->header, &retval->header))
    {
      dbus_free (retval);
      return NULL;
    }

  if (!_dbus_string_init_preallocated (&retval->body,
                                       _dbus_string_get_length (&message->body)))
    {
      _dbus_header_free (&retval->header);
      dbus_free (retval);
      return NULL;
    }

  if (!_dbus_string_copy (&message->body, 0,
			  &retval->body, 0))
    goto failed_copy;

#ifdef HAVE_UNIX_FD_PASSING
  retval->unix_fds = dbus_new(int, message->n_unix_fds);
  if (retval->unix_fds == NULL && message->n_unix_fds > 0)
    goto failed_copy;

  retval->n_unix_fds_allocated = message->n_unix_fds;

  for (retval->n_unix_fds = 0;
       retval->n_unix_fds < message->n_unix_fds;
       retval->n_unix_fds++)
    {
      retval->unix_fds[retval->n_unix_fds] = _dbus_dup(message->unix_fds[retval->n_unix_fds], NULL);

      if (retval->unix_fds[retval->n_unix_fds] < 0)
        goto failed_copy;
    }

#endif

  _dbus_message_trace_ref (retval, 0, 1, "copy");
  return retval;

 failed_copy:
  _dbus_header_free (&retval->header);
  _dbus_string_free (&retval->body);

#ifdef HAVE_UNIX_FD_PASSING
  close_unix_fds(retval->unix_fds, &retval->n_unix_fds);
  dbus_free(retval->unix_fds);
#endif

  dbus_free (retval);

  return NULL;
}


/**
 * Increments the reference count of a DBusMessage.
 *
 * @param message the message
 * @returns the message
 * @see dbus_message_unref
 */
DBusMessage *
dbus_message_ref (DBusMessage *message)
{
  dbus_int32_t old_refcount;

  _dbus_return_val_if_fail (message != NULL, NULL);
  _dbus_return_val_if_fail (message->generation == _dbus_current_generation, NULL);
  _dbus_return_val_if_fail (!message->in_cache, NULL);

  old_refcount = _dbus_atomic_inc (&message->refcount);
  _dbus_assert (old_refcount >= 1);
  _dbus_message_trace_ref (message, old_refcount, old_refcount + 1, "ref");

  return message;
}

/**
 * Decrements the reference count of a DBusMessage, freeing the
 * message if the count reaches 0.
 *
 * @param message the message
 * @see dbus_message_ref
 */
void
dbus_message_unref (DBusMessage *message)
{
 dbus_int32_t old_refcount;

  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (message->generation == _dbus_current_generation);
  _dbus_return_if_fail (!message->in_cache);

  old_refcount = _dbus_atomic_dec (&message->refcount);

  _dbus_assert (old_refcount >= 1);

  _dbus_message_trace_ref (message, old_refcount, old_refcount - 1, "unref");

  if (old_refcount == 1)
    {
      /* Calls application callbacks! */
      dbus_message_cache_or_finalize (message);
    }
}

/**
 * Gets the type of a message. Types include
 * #DBUS_MESSAGE_TYPE_METHOD_CALL, #DBUS_MESSAGE_TYPE_METHOD_RETURN,
 * #DBUS_MESSAGE_TYPE_ERROR, #DBUS_MESSAGE_TYPE_SIGNAL, but other
 * types are allowed and all code must silently ignore messages of
 * unknown type. #DBUS_MESSAGE_TYPE_INVALID will never be returned.
 *
 * @param message the message
 * @returns the type of the message
 */
int
dbus_message_get_type (DBusMessage *message)
{
  _dbus_return_val_if_fail (message != NULL, DBUS_MESSAGE_TYPE_INVALID);

  return _dbus_header_get_message_type (&message->header);
}

/**
 * Appends fields to a message given a variable argument list. The
 * variable argument list should contain the type of each argument
 * followed by the value to append. Appendable types are basic types,
 * and arrays of fixed-length basic types (except arrays of Unix file
 * descriptors). To append variable-length basic types, or any more
 * complex value, you have to use an iterator rather than this
 * function.
 *
 * To append a basic type, specify its type code followed by the
 * address of the value. For example:
 *
 * @code
 *
 * dbus_int32_t v_INT32 = 42;
 * const char *v_STRING = "Hello World";
 * dbus_message_append_args (message,
 *                           DBUS_TYPE_INT32, &v_INT32,
 *                           DBUS_TYPE_STRING, &v_STRING,
 *                           DBUS_TYPE_INVALID);
 * @endcode
 *
 * To append an array of fixed-length basic types (except Unix file
 * descriptors), pass in the DBUS_TYPE_ARRAY typecode, the element
 * typecode, the address of the array pointer, and a 32-bit integer
 * giving the number of elements in the array. So for example:
 *
 * @code
 *
 * const dbus_int32_t array[] = { 1, 2, 3 };
 * const dbus_int32_t *v_ARRAY = array;
 * dbus_message_append_args (message,
 *                           DBUS_TYPE_ARRAY, DBUS_TYPE_INT32, &v_ARRAY, 3,
 *                           DBUS_TYPE_INVALID);
 *
 * @endcode
 *
 * This function does not support arrays of Unix file descriptors. If
 * you need those you need to manually recurse into the array.
 *
 * For Unix file descriptors this function will internally duplicate
 * the descriptor you passed in. Hence you may close the descriptor
 * immediately after this call.
 *
 * @warning in C, given "int array[]", "&array == array" (the
 * comp.lang.c FAQ says otherwise, but gcc and the FAQ don't agree).
 * So if you're using an array instead of a pointer you have to create
 * a pointer variable, assign the array to it, then take the address
 * of the pointer variable. For strings it works to write
 * const char *array = "Hello" and then use &array though.
 *
 * The last argument to this function must be #DBUS_TYPE_INVALID,
 * marking the end of the argument list. If you don't do this
 * then libdbus won't know to stop and will read invalid memory.
 *
 * String/signature/path arrays should be passed in as "const char***
 * address_of_array" and "int n_elements"
 *
 * @todo support DBUS_TYPE_STRUCT and DBUS_TYPE_VARIANT and complex arrays
 *
 * @todo If this fails due to lack of memory, the message is hosed and
 * you have to start over building the whole message.
 *
 * @param message the message
 * @param first_arg_type type of the first argument
 * @param ... value of first argument, list of additional type-value pairs
 * @returns #TRUE on success
 */
dbus_bool_t
dbus_message_append_args (DBusMessage *message,
			  int          first_arg_type,
			  ...)
{
  dbus_bool_t retval;
  va_list var_args;

  _dbus_return_val_if_fail (message != NULL, FALSE);

  va_start (var_args, first_arg_type);
  retval = dbus_message_append_args_valist (message,
					    first_arg_type,
					    var_args);
  va_end (var_args);

  return retval;
}

/**
 * Like dbus_message_append_args() but takes a va_list for use by language bindings.
 *
 * @todo for now, if this function fails due to OOM it will leave
 * the message half-written and you have to discard the message
 * and start over.
 *
 * @see dbus_message_append_args.
 * @param message the message
 * @param first_arg_type type of first argument
 * @param var_args value of first argument, then list of type/value pairs
 * @returns #TRUE on success
 */
dbus_bool_t
dbus_message_append_args_valist (DBusMessage *message,
				 int          first_arg_type,
				 va_list      var_args)
{
  int type;
  DBusMessageIter iter;

  _dbus_return_val_if_fail (message != NULL, FALSE);

  type = first_arg_type;

  dbus_message_iter_init_append (message, &iter);

  while (type != DBUS_TYPE_INVALID)
    {
      if (dbus_type_is_basic (type))
        {
          const void *value;
          value = va_arg (var_args, const void *);

          if (!dbus_message_iter_append_basic (&iter,
                                               type,
                                               value))
            goto failed;
        }
      else if (type == DBUS_TYPE_ARRAY)
        {
          int element_type;
          DBusMessageIter array;
          char buf[2];

          element_type = va_arg (var_args, int);
              
          buf[0] = element_type;
          buf[1] = '\0';
          if (!dbus_message_iter_open_container (&iter,
                                                 DBUS_TYPE_ARRAY,
                                                 buf,
                                                 &array))
            goto failed;

          if (dbus_type_is_fixed (element_type) &&
              element_type != DBUS_TYPE_UNIX_FD)
            {
              const void **value;
              int n_elements;

              value = va_arg (var_args, const void **);
              n_elements = va_arg (var_args, int);
              
              if (!dbus_message_iter_append_fixed_array (&array,
                                                         element_type,
                                                         value,
                                                         n_elements)) {
                dbus_message_iter_abandon_container (&iter, &array);
                goto failed;
              }
            }
          else if (_DBUS_TYPE_IS_STRINGLIKE (element_type))
            {
              const char ***value_p;
              const char **value;
              int n_elements;
              int i;
              
              value_p = va_arg (var_args, const char***);
              n_elements = va_arg (var_args, int);

              value = *value_p;
              
              i = 0;
              while (i < n_elements)
                {
                  if (!dbus_message_iter_append_basic (&array,
                                                       element_type,
                                                       &value[i])) {
                    dbus_message_iter_abandon_container (&iter, &array);
                    goto failed;
                  }
                  ++i;
                }
            }
          else
            {
              _dbus_warn ("arrays of %s can't be appended with %s for now",
                          _dbus_type_to_string (element_type),
                          _DBUS_FUNCTION_NAME);
              dbus_message_iter_abandon_container (&iter, &array);
              goto failed;
            }

          if (!dbus_message_iter_close_container (&iter, &array))
            goto failed;
        }
#ifndef DBUS_DISABLE_CHECKS
      else
        {
          _dbus_warn ("type %s isn't supported yet in %s",
                      _dbus_type_to_string (type), _DBUS_FUNCTION_NAME);
          goto failed;
        }
#endif

      type = va_arg (var_args, int);
    }

  return TRUE;

 failed:
  return FALSE;
}

/**
 * Gets arguments from a message given a variable argument list.  The
 * supported types include those supported by
 * dbus_message_append_args(); that is, basic types and arrays of
 * fixed-length basic types.  The arguments are the same as they would
 * be for dbus_message_iter_get_basic() or
 * dbus_message_iter_get_fixed_array().
 *
 * In addition to those types, arrays of string, object path, and
 * signature are supported; but these are returned as allocated memory
 * and must be freed with dbus_free_string_array(), while the other
 * types are returned as const references. To get a string array
 * pass in "char ***array_location" and "int *n_elements".
 *
 * Similar to dbus_message_get_fixed_array() this function does not
 * support arrays of type DBUS_TYPE_UNIX_FD. If you need to parse
 * messages with arrays of Unix file descriptors you need to recurse
 * into the array manually.
 *
 * Unix file descriptors that are read with this function will have
 * the FD_CLOEXEC flag set. If you need them without this flag set,
 * make sure to unset it with fcntl().
 *
 * The variable argument list should contain the type of the argument
 * followed by a pointer to where the value should be stored. The list
 * is terminated with #DBUS_TYPE_INVALID.
 *
 * Except for string arrays, the returned values are constant; do not
 * free them. They point into the #DBusMessage.
 *
 * If the requested arguments are not present, or do not have the
 * requested types, then an error will be set.
 *
 * If more arguments than requested are present, the requested
 * arguments are returned and the extra arguments are ignored.
 * 
 * @todo support DBUS_TYPE_STRUCT and DBUS_TYPE_VARIANT and complex arrays
 *
 * @param message the message
 * @param error error to be filled in on failure
 * @param first_arg_type the first argument type
 * @param ... location for first argument value, then list of type-location pairs
 * @returns #FALSE if the error was set
 */
dbus_bool_t
dbus_message_get_args (DBusMessage     *message,
                       DBusError       *error,
		       int              first_arg_type,
		       ...)
{
  dbus_bool_t retval;
  va_list var_args;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_error_is_set (error, FALSE);

  va_start (var_args, first_arg_type);
  retval = dbus_message_get_args_valist (message, error, first_arg_type, var_args);
  va_end (var_args);

  return retval;
}

/**
 * Like dbus_message_get_args but takes a va_list for use by language bindings.
 *
 * @see dbus_message_get_args
 * @param message the message
 * @param error error to be filled in
 * @param first_arg_type type of the first argument
 * @param var_args return location for first argument, followed by list of type/location pairs
 * @returns #FALSE if error was set
 */
dbus_bool_t
dbus_message_get_args_valist (DBusMessage     *message,
                              DBusError       *error,
			      int              first_arg_type,
			      va_list          var_args)
{
  DBusMessageIter iter;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_error_is_set (error, FALSE);

  dbus_message_iter_init (message, &iter);
  return _dbus_message_iter_get_args_valist (&iter, error, first_arg_type, var_args);
}

static void
_dbus_message_iter_init_common (DBusMessage         *message,
                                DBusMessageRealIter *real,
                                int                  iter_type)
{
  /* If these static assertions fail on your platform, report it as a bug. */
  _DBUS_STATIC_ASSERT (sizeof (DBusMessageRealIter) <= sizeof (DBusMessageIter));
  _DBUS_STATIC_ASSERT (_DBUS_ALIGNOF (DBusMessageRealIter) <=
      _DBUS_ALIGNOF (DBusMessageIter));
  /* A failure of these two assertions would indicate that we've broken
   * ABI on this platform since 1.10.0. */
  _DBUS_STATIC_ASSERT (sizeof (DBusMessageIter_1_10_0) ==
      sizeof (DBusMessageIter));
  _DBUS_STATIC_ASSERT (_DBUS_ALIGNOF (DBusMessageIter_1_10_0) ==
      _DBUS_ALIGNOF (DBusMessageIter));
  /* If this static assertion fails, it means the DBusMessageIter struct
   * is not "packed", which might result in "iter = other_iter" not copying
   * every byte. */
  _DBUS_STATIC_ASSERT (sizeof (DBusMessageIter) ==
      4 * sizeof (void *) + sizeof (dbus_uint32_t) + 9 * sizeof (int));

  /* Since the iterator will read or write who-knows-what from the
   * message, we need to get in the right byte order
   */
  ensure_byte_order (message);
  
  real->message = message;
  real->changed_stamp = message->changed_stamp;
  real->iter_type = iter_type;
  real->sig_refcount = 0;
}

/**
 * Initializes a #DBusMessageIter for reading the arguments of the
 * message passed in.
 *
 * When possible, dbus_message_get_args() is much more convenient.
 * Some types of argument can only be read with #DBusMessageIter
 * however.
 *
 * The easiest way to iterate is like this: 
 * @code
 * dbus_message_iter_init (message, &iter);
 * while ((current_type = dbus_message_iter_get_arg_type (&iter)) != DBUS_TYPE_INVALID)
 *   dbus_message_iter_next (&iter);
 * @endcode
 *
 * #DBusMessageIter contains no allocated memory; it need not be
 * freed, and can be copied by assignment or memcpy().
 * 
 * @param message the message
 * @param iter pointer to an iterator to initialize
 * @returns #FALSE if the message has no arguments
 */
dbus_bool_t
dbus_message_iter_init (DBusMessage     *message,
			DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  const DBusString *type_str;
  int type_pos;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (iter != NULL, FALSE);

  get_const_signature (&message->header, &type_str, &type_pos);

  _dbus_message_iter_init_common (message, real,
                                  DBUS_MESSAGE_ITER_TYPE_READER);

  _dbus_type_reader_init (&real->u.reader,
                          _dbus_header_get_byte_order (&message->header),
                          type_str, type_pos,
                          &message->body,
                          0);

  return _dbus_type_reader_get_current_type (&real->u.reader) != DBUS_TYPE_INVALID;
}

/**
 * Checks if an iterator has any more fields.
 *
 * @param iter the message iter
 * @returns #TRUE if there are more fields following
 */
dbus_bool_t
dbus_message_iter_has_next (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_READER, FALSE);

  return _dbus_type_reader_has_next (&real->u.reader);
}

/**
 * Moves the iterator to the next field, if any. If there's no next
 * field, returns #FALSE. If the iterator moves forward, returns
 * #TRUE.
 *
 * @param iter the message iter
 * @returns #TRUE if the iterator was moved to the next field
 */
dbus_bool_t
dbus_message_iter_next (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_READER, FALSE);

  return _dbus_type_reader_next (&real->u.reader);
}

/**
 * Returns the argument type of the argument that the message iterator
 * points to. If the iterator is at the end of the message, returns
 * #DBUS_TYPE_INVALID. You can thus write a loop as follows:
 *
 * @code
 * dbus_message_iter_init (message, &iter);
 * while ((current_type = dbus_message_iter_get_arg_type (&iter)) != DBUS_TYPE_INVALID)
 *   dbus_message_iter_next (&iter);
 * @endcode
 *
 * @param iter the message iter
 * @returns the argument type
 */
int
dbus_message_iter_get_arg_type (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), DBUS_TYPE_INVALID);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_READER, FALSE);

  return _dbus_type_reader_get_current_type (&real->u.reader);
}

/**
 * Returns the element type of the array that the message iterator
 * points to. Note that you need to check that the iterator points to
 * an array prior to using this function.
 *
 * @param iter the message iter
 * @returns the array element type
 */
int
dbus_message_iter_get_element_type (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), DBUS_TYPE_INVALID);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_READER, DBUS_TYPE_INVALID);
  _dbus_return_val_if_fail (dbus_message_iter_get_arg_type (iter) == DBUS_TYPE_ARRAY, DBUS_TYPE_INVALID);

  return _dbus_type_reader_get_element_type (&real->u.reader);
}

/**
 * Recurses into a container value when reading values from a message,
 * initializing a sub-iterator to use for traversing the child values
 * of the container.
 *
 * Note that this recurses into a value, not a type, so you can only
 * recurse if the value exists. The main implication of this is that
 * if you have for example an empty array of array of int32, you can
 * recurse into the outermost array, but it will have no values, so
 * you won't be able to recurse further. There's no array of int32 to
 * recurse into.
 *
 * If a container is an array of fixed-length types (except Unix file
 * descriptors), it is much more efficient to use
 * dbus_message_iter_get_fixed_array() to get the whole array in one
 * shot, rather than individually walking over the array elements.
 *
 * Be sure you have somehow checked that
 * dbus_message_iter_get_arg_type() matches the type you are expecting
 * to recurse into. Results of this function are undefined if there is
 * no container to recurse into at the current iterator position.
 *
 * @param iter the message iterator
 * @param sub the sub-iterator to initialize
 */
void
dbus_message_iter_recurse (DBusMessageIter  *iter,
                           DBusMessageIter  *sub)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusMessageRealIter *real_sub = (DBusMessageRealIter *)sub;

  _dbus_return_if_fail (_dbus_message_iter_check (real));
  _dbus_return_if_fail (sub != NULL);

  *real_sub = *real;
  _dbus_type_reader_recurse (&real->u.reader, &real_sub->u.reader);
}

/**
 * Returns the current signature of a message iterator.  This
 * is useful primarily for dealing with variants; one can
 * recurse into a variant and determine the signature of
 * the variant's value.
 *
 * The returned string must be freed with dbus_free().
 * 
 * @param iter the message iterator
 * @returns the contained signature, or NULL if out of memory
 */
char *
dbus_message_iter_get_signature (DBusMessageIter *iter)
{
  const DBusString *sig;
  DBusString retstr;
  char *ret;
  int start, len;
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), NULL);

  if (!_dbus_string_init (&retstr))
    return NULL;

  _dbus_type_reader_get_signature (&real->u.reader, &sig,
				   &start, &len);
  if (!_dbus_string_append_len (&retstr,
				_dbus_string_get_const_data (sig) + start,
				len))
    return NULL;
  if (!_dbus_string_steal_data (&retstr, &ret))
    return NULL;
  _dbus_string_free (&retstr);
  return ret;
}

/**
 * Reads a basic-typed value from the message iterator.
 * Basic types are the non-containers such as integer and string.
 *
 * The value argument should be the address of a location to store
 * the returned value. So for int32 it should be a "dbus_int32_t*"
 * and for string a "const char**". The returned value is
 * by reference and should not be freed.
 *
 * This call duplicates Unix file descriptors when reading them. It is
 * your job to close them when you don't need them anymore.
 *
 * Unix file descriptors that are read with this function will have
 * the FD_CLOEXEC flag set. If you need them without this flag set,
 * make sure to unset it with fcntl().
 *
 * Be sure you have somehow checked that
 * dbus_message_iter_get_arg_type() matches the type you are
 * expecting, or you'll crash when you try to use an integer as a
 * string or something.
 *
 * To read any container type (array, struct, dict) you will need to
 * recurse into the container with dbus_message_iter_recurse().  If
 * the container is an array of fixed-length values (except Unix file
 * descriptors), you can get all the array elements at once with
 * dbus_message_iter_get_fixed_array(). Otherwise, you have to iterate
 * over the container's contents one value at a time.
 *
 * All basic-typed values are guaranteed to fit in a #DBusBasicValue,
 * so in versions of libdbus that have that type, you can write code like this:
 *
 * @code
 * DBusBasicValue value;
 * int type;
 * dbus_message_iter_get_basic (&read_iter, &value);
 * type = dbus_message_iter_get_arg_type (&read_iter);
 * dbus_message_iter_append_basic (&write_iter, type, &value);
 * @endcode
 *
 * (All D-Bus basic types are either numeric and 8 bytes or smaller, or
 * behave like a string; so in older versions of libdbus, DBusBasicValue
 * can be replaced with union { char *string; unsigned char bytes[8]; },
 * for instance.)
 *
 * @param iter the iterator
 * @param value location to store the value
 */
void
dbus_message_iter_get_basic (DBusMessageIter  *iter,
                             void             *value)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_if_fail (_dbus_message_iter_check (real));
  _dbus_return_if_fail (value != NULL);

  if (dbus_message_iter_get_arg_type (iter) == DBUS_TYPE_UNIX_FD)
    {
#ifdef HAVE_UNIX_FD_PASSING
      DBusBasicValue idx;

      _dbus_type_reader_read_basic(&real->u.reader, &idx);

      if (idx.u32 >= real->message->n_unix_fds) {
        /* Hmm, we cannot really signal an error here, so let's make
           sure to return an invalid fd. */
        *((int*) value) = -1;
        return;
      }

      *((int*) value) = _dbus_dup(real->message->unix_fds[idx.u32], NULL);
#else
      *((int*) value) = -1;
#endif
    }
  else
    {
      _dbus_type_reader_read_basic (&real->u.reader,
                                    value);
    }
}

/**
 * Returns the number of elements in the array-typed value pointed
 * to by the iterator.
 * Note that this function is O(1) for arrays of fixed-size types
 * but O(n) for arrays of variable-length types such as strings,
 * so it may be a bad idea to use it.
 *
 * @param iter the iterator
 * @returns the number of elements in the array
 */
int
dbus_message_iter_get_element_count (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusTypeReader array;
  int element_type;
  int n_elements = 0;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), 0);
  _dbus_return_val_if_fail (_dbus_type_reader_get_current_type (&real->u.reader)
                            == DBUS_TYPE_ARRAY, 0);

  element_type = _dbus_type_reader_get_element_type (&real->u.reader);
  _dbus_type_reader_recurse (&real->u.reader, &array);
  if (dbus_type_is_fixed (element_type))
    {
      int alignment = _dbus_type_get_alignment (element_type);
      int total_len = _dbus_type_reader_get_array_length (&array);
      n_elements = total_len / alignment;
    }
  else
    {
      while (_dbus_type_reader_get_current_type (&array) != DBUS_TYPE_INVALID)
        {
          ++n_elements;
          _dbus_type_reader_next (&array);
        }
    }

   return n_elements;
}

/**
 * Returns the number of bytes in the array as marshaled in the wire
 * protocol. The iterator must currently be inside an array-typed
 * value.
 *
 * This function is deprecated on the grounds that it is stupid.  Why
 * would you want to know how many bytes are in the array as marshaled
 * in the wire protocol?  Use dbus_message_iter_get_element_count() instead.
 *
 * @param iter the iterator
 * @returns the number of bytes in the array
 */
int
dbus_message_iter_get_array_len (DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_val_if_fail (_dbus_message_iter_check (real), 0);

  return _dbus_type_reader_get_array_length (&real->u.reader);
}

/**
 * Reads a block of fixed-length values from the message iterator.
 * Fixed-length values are those basic types that are not string-like,
 * such as integers, bool, double. The returned block will be from the
 * current position in the array until the end of the array.
 *
 * There is one exception here: although DBUS_TYPE_UNIX_FD is
 * considered a 'fixed' type arrays of this type may not be read with
 * this function.
 *
 * The message iter should be "in" the array (that is, you recurse into the
 * array, and then you call dbus_message_iter_get_fixed_array() on the
 * "sub-iterator" created by dbus_message_iter_recurse()).
 *
 * The value argument should be the address of a location to store the
 * returned array. So for int32 it should be a "const dbus_int32_t**"
 * The returned value is by reference and should not be freed.
 * 
 * This function should only be used if dbus_type_is_fixed() returns
 * #TRUE for the element type.
 *
 * If an array's elements are not fixed in size, you have to recurse
 * into the array with dbus_message_iter_recurse() and read the
 * elements one by one.
 * 
 * Because the array is not copied, this function runs in constant
 * time and is fast; it's much preferred over walking the entire array
 * with an iterator. (However, you can always use
 * dbus_message_iter_recurse(), even for fixed-length types;
 * dbus_message_iter_get_fixed_array() is just an optimization.)
 * 
 * @param iter the iterator
 * @param value location to store the block
 * @param n_elements number of elements in the block
 */
void
dbus_message_iter_get_fixed_array (DBusMessageIter  *iter,
                                   void             *value,
                                   int              *n_elements)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
#ifndef DBUS_DISABLE_CHECKS
  int subtype = _dbus_type_reader_get_current_type(&real->u.reader);

  _dbus_return_if_fail (_dbus_message_iter_check (real));
  _dbus_return_if_fail (value != NULL);
  _dbus_return_if_fail ((subtype == DBUS_TYPE_INVALID) ||
                        (dbus_type_is_fixed (subtype) && subtype != DBUS_TYPE_UNIX_FD));
#endif

  _dbus_type_reader_read_fixed_multi (&real->u.reader,
                                      value, n_elements);
}

/**
 * Initializes a #DBusMessageIter for appending arguments to the end
 * of a message.
 *
 * @todo If appending any of the arguments fails due to lack of
 * memory, the message is hosed and you have to start over building
 * the whole message.
 *
 * @param message the message
 * @param iter pointer to an iterator to initialize
 */
void
dbus_message_iter_init_append (DBusMessage     *message,
			       DBusMessageIter *iter)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;

  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (iter != NULL);

  _dbus_message_iter_init_common (message, real,
                                  DBUS_MESSAGE_ITER_TYPE_WRITER);

  /* We create the signature string and point iterators at it "on demand"
   * when a value is actually appended. That means that init() never fails
   * due to OOM.
   */
  _dbus_type_writer_init_types_delayed (&real->u.writer,
                                        _dbus_header_get_byte_order (&message->header),
                                        &message->body,
                                        _dbus_string_get_length (&message->body));
}

/**
 * Creates a temporary signature string containing the current
 * signature, stores it in the iterator, and points the iterator to
 * the end of it. Used any time we write to the message.
 *
 * @param real an iterator without a type_str
 * @returns #FALSE if no memory
 */
static dbus_bool_t
_dbus_message_iter_open_signature (DBusMessageRealIter *real)
{
  DBusString *str;
  const DBusString *current_sig;
  int current_sig_pos;

  _dbus_assert (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);

  if (real->u.writer.type_str != NULL)
    {
      _dbus_assert (real->sig_refcount > 0);
      real->sig_refcount += 1;
      return TRUE;
    }

  str = dbus_new (DBusString, 1);
  if (str == NULL)
    return FALSE;

  if (!_dbus_header_get_field_raw (&real->message->header,
                                   DBUS_HEADER_FIELD_SIGNATURE,
                                   &current_sig, &current_sig_pos))
    current_sig = NULL;

  if (current_sig)
    {
      int current_len;

      current_len = _dbus_string_get_byte (current_sig, current_sig_pos);
      current_sig_pos += 1; /* move on to sig data */

      if (!_dbus_string_init_preallocated (str, current_len + 4))
        {
          dbus_free (str);
          return FALSE;
        }

      if (!_dbus_string_copy_len (current_sig, current_sig_pos, current_len,
                                  str, 0))
        {
          _dbus_string_free (str);
          dbus_free (str);
          return FALSE;
        }
    }
  else
    {
      if (!_dbus_string_init_preallocated (str, 4))
        {
          dbus_free (str);
          return FALSE;
        }
    }

  real->sig_refcount = 1;

  /* If this assertion failed, then str would be neither stored in u.writer
   * nor freed by this function, resulting in a memory leak. */
  _dbus_assert (real->u.writer.type_str == NULL);
  _dbus_type_writer_add_types (&real->u.writer,
                               str, _dbus_string_get_length (str));
  return TRUE;
}

/**
 * Sets the new signature as the message signature, frees the
 * signature string, and marks the iterator as not having a type_str
 * anymore. Frees the signature even if it fails, so you can't
 * really recover from failure. Kinda busted.
 *
 * @param real an iterator without a type_str
 * @returns #FALSE if no memory
 */
static dbus_bool_t
_dbus_message_iter_close_signature (DBusMessageRealIter *real)
{
  DBusString *str;
  const char *v_STRING;
  dbus_bool_t retval;

  _dbus_assert (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
  _dbus_assert (real->u.writer.type_str != NULL);
  _dbus_assert (real->sig_refcount > 0);

  real->sig_refcount -= 1;

  if (real->sig_refcount > 0)
    return TRUE;
  _dbus_assert (real->sig_refcount == 0);

  retval = TRUE;

  str = real->u.writer.type_str;

  v_STRING = _dbus_string_get_const_data (str);
  if (!_dbus_header_set_field_basic (&real->message->header,
                                     DBUS_HEADER_FIELD_SIGNATURE,
                                     DBUS_TYPE_SIGNATURE,
                                     &v_STRING))
    retval = FALSE;

  _dbus_type_writer_remove_types (&real->u.writer);
  _dbus_string_free (str);
  dbus_free (str);

  return retval;
}

/**
 * Frees the signature string and marks the iterator as not having a
 * type_str anymore.  Since the new signature is not set, the message
 * will generally be hosed after this is called.
 *
 * @param real an iterator without a type_str
 */
static void
_dbus_message_iter_abandon_signature (DBusMessageRealIter *real)
{
  DBusString *str;

  _dbus_assert (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
  _dbus_assert (real->u.writer.type_str != NULL);
  _dbus_assert (real->sig_refcount > 0);

  real->sig_refcount -= 1;

  if (real->sig_refcount > 0)
    return;
  _dbus_assert (real->sig_refcount == 0);

  str = real->u.writer.type_str;

  _dbus_type_writer_remove_types (&real->u.writer);
  _dbus_string_free (str);
  dbus_free (str);
}

#ifndef DBUS_DISABLE_CHECKS
static dbus_bool_t
_dbus_message_iter_append_check (DBusMessageRealIter *iter)
{
  if (!_dbus_message_iter_check (iter))
    return FALSE;

  if (iter->message->locked)
    {
      _dbus_warn_check_failed ("dbus append iterator can't be used: message is locked (has already been sent)");
      return FALSE;
    }

  return TRUE;
}
#endif /* DBUS_DISABLE_CHECKS */

#ifdef HAVE_UNIX_FD_PASSING
static int *
expand_fd_array(DBusMessage *m,
                unsigned     n)
{
  _dbus_assert(m);

  /* This makes space for adding n new fds to the array and returns a
     pointer to the place were the first fd should be put. */

  if (m->n_unix_fds + n > m->n_unix_fds_allocated)
    {
      unsigned k;
      int *p;

      /* Make twice as much space as necessary */
      k = (m->n_unix_fds + n) * 2;

      /* Allocate at least four */
      if (k < 4)
        k = 4;

      p = dbus_realloc(m->unix_fds, k * sizeof(int));
      if (p == NULL)
        return NULL;

      m->unix_fds = p;
      m->n_unix_fds_allocated = k;
    }

  return m->unix_fds + m->n_unix_fds;
}
#endif

/**
 * Appends a basic-typed value to the message. The basic types are the
 * non-container types such as integer and string.
 *
 * The "value" argument should be the address of a basic-typed value.
 * So for string, const char**. For integer, dbus_int32_t*.
 *
 * For Unix file descriptors this function will internally duplicate
 * the descriptor you passed in. Hence you may close the descriptor
 * immediately after this call.
 *
 * @todo If this fails due to lack of memory, the message is hosed and
 * you have to start over building the whole message.
 *
 * @param iter the append iterator
 * @param type the type of the value
 * @param value the address of the value
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_iter_append_basic (DBusMessageIter *iter,
                                int              type,
                                const void      *value)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  dbus_bool_t ret;

  _dbus_return_val_if_fail (_dbus_message_iter_append_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER, FALSE);
  _dbus_return_val_if_fail (dbus_type_is_basic (type), FALSE);
  _dbus_return_val_if_fail (value != NULL, FALSE);

#ifndef DBUS_DISABLE_CHECKS
  switch (type)
    {
      DBusString str;
      DBusValidity signature_validity;
      const char * const *string_p;
      const dbus_bool_t *bool_p;

      case DBUS_TYPE_STRING:
        string_p = value;
        _dbus_return_val_if_fail (_dbus_check_is_valid_utf8 (*string_p), FALSE);
        break;

      case DBUS_TYPE_OBJECT_PATH:
        string_p = value;
        _dbus_return_val_if_fail (_dbus_check_is_valid_path (*string_p), FALSE);
        break;

      case DBUS_TYPE_SIGNATURE:
        string_p = value;
        _dbus_string_init_const (&str, *string_p);
        signature_validity = _dbus_validate_signature_with_reason (&str,
                                                                   0,
                                                                   _dbus_string_get_length (&str));

        if (signature_validity == DBUS_VALIDITY_UNKNOWN_OOM_ERROR)
          return FALSE;

        _dbus_return_val_if_fail (signature_validity == DBUS_VALID, FALSE);
        break;

      case DBUS_TYPE_BOOLEAN:
        bool_p = value;
        _dbus_return_val_if_fail (*bool_p == 0 || *bool_p == 1, FALSE);
        break;

      default:
          {
            /* nothing to check, all possible values are allowed */
          }
    }
#endif

  if (!_dbus_message_iter_open_signature (real))
    return FALSE;

  if (type == DBUS_TYPE_UNIX_FD)
    {
#ifdef HAVE_UNIX_FD_PASSING
      int *fds;
      dbus_uint32_t u;

      ret = FALSE;

      /* First step, include the fd in the fd list of this message */
      if (!(fds = expand_fd_array(real->message, 1)))
        goto out;

      *fds = _dbus_dup(*(int*) value, NULL);
      if (*fds < 0)
        goto out;

      u = real->message->n_unix_fds;

      /* Second step, write the index to the fd */
      if (!(ret = _dbus_type_writer_write_basic (&real->u.writer, DBUS_TYPE_UNIX_FD, &u))) {
        _dbus_close(*fds, NULL);
        goto out;
      }

      real->message->n_unix_fds += 1;
      u += 1;

      /* Final step, update the header accordingly */
      ret = _dbus_header_set_field_basic (&real->message->header,
                                          DBUS_HEADER_FIELD_UNIX_FDS,
                                          DBUS_TYPE_UINT32,
                                          &u);

      /* If any of these operations fail the message is
         hosed. However, no memory or fds should be leaked since what
         has been added to message has been added to the message, and
         can hence be accounted for when the message is being
         freed. */
#else
      ret = FALSE;
      /* This is redundant (we could just fall through), but it avoids
       * -Wunused-label in builds that don't HAVE_UNIX_FD_PASSING */
      goto out;
#endif
    }
  else
    {
      ret = _dbus_type_writer_write_basic (&real->u.writer, type, value);
    }

out:
  if (!_dbus_message_iter_close_signature (real))
    ret = FALSE;

  return ret;
}

/**
 * Appends a block of fixed-length values to an array. The
 * fixed-length types are all basic types that are not string-like. So
 * int32, double, bool, etc. (Unix file descriptors however are not
 * supported.) You must call dbus_message_iter_open_container() to
 * open an array of values before calling this function. You may call
 * this function multiple times (and intermixed with calls to
 * dbus_message_iter_append_basic()) for the same array.
 *
 * The "value" argument should be the address of the array.  So for
 * integer, "dbus_int32_t**" is expected for example.
 *
 * @warning in C, given "int array[]", "&array == array" (the
 * comp.lang.c FAQ says otherwise, but gcc and the FAQ don't agree).
 * So if you're using an array instead of a pointer you have to create
 * a pointer variable, assign the array to it, then take the address
 * of the pointer variable.
 * @code
 * const dbus_int32_t array[] = { 1, 2, 3 };
 * const dbus_int32_t *v_ARRAY = array;
 * if (!dbus_message_iter_append_fixed_array (&iter, DBUS_TYPE_INT32, &v_ARRAY, 3))
 *   fprintf (stderr, "No memory!\n");
 * @endcode
 * For strings it works to write const char *array = "Hello" and then
 * use &array though.
 *
 * @todo If this fails due to lack of memory, the message is hosed and
 * you have to start over building the whole message.
 *
 * @param iter the append iterator
 * @param element_type the type of the array elements
 * @param value the address of the array
 * @param n_elements the number of elements to append
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_iter_append_fixed_array (DBusMessageIter *iter,
                                      int              element_type,
                                      const void      *value,
                                      int              n_elements)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  dbus_bool_t ret;

  _dbus_return_val_if_fail (_dbus_message_iter_append_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER, FALSE);
  _dbus_return_val_if_fail (dbus_type_is_fixed (element_type) && element_type != DBUS_TYPE_UNIX_FD, FALSE);
  _dbus_return_val_if_fail (real->u.writer.container_type == DBUS_TYPE_ARRAY, FALSE);
  _dbus_return_val_if_fail (value != NULL, FALSE);
  _dbus_return_val_if_fail (n_elements >= 0, FALSE);
  _dbus_return_val_if_fail (n_elements <=
                            DBUS_MAXIMUM_ARRAY_LENGTH / _dbus_type_get_alignment (element_type),
                            FALSE);

#ifndef DBUS_DISABLE_CHECKS
  if (element_type == DBUS_TYPE_BOOLEAN)
    {
      const dbus_bool_t * const *bools = value;
      int i;

      for (i = 0; i < n_elements; i++)
        {
          _dbus_return_val_if_fail ((*bools)[i] == 0 || (*bools)[i] == 1, FALSE);
        }
    }
#endif

  ret = _dbus_type_writer_write_fixed_multi (&real->u.writer, element_type, value, n_elements);

  return ret;
}

/**
 * Appends a container-typed value to the message. On success, you are
 * required to append the contents of the container using the returned
 * sub-iterator, and then call
 * dbus_message_iter_close_container(). Container types are for
 * example struct, variant, and array. For variants, the
 * contained_signature should be the type of the single value inside
 * the variant. For structs and dict entries, contained_signature
 * should be #NULL; it will be set to whatever types you write into
 * the struct.  For arrays, contained_signature should be the type of
 * the array elements.
 *
 * @todo If this fails due to lack of memory, the message is hosed and
 * you have to start over building the whole message.
 *
 * If this function fails, the sub-iterator remains invalid, and must
 * not be closed with dbus_message_iter_close_container() or abandoned
 * with dbus_message_iter_abandon_container(). However, after this
 * function has either succeeded or failed, it is valid to call
 * dbus_message_iter_abandon_container_if_open().
 *
 * @param iter the append iterator
 * @param type the type of the value
 * @param contained_signature the type of container contents
 * @param sub sub-iterator to initialize
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_iter_open_container (DBusMessageIter *iter,
                                  int              type,
                                  const char      *contained_signature,
                                  DBusMessageIter *sub)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusMessageRealIter *real_sub = (DBusMessageRealIter *)sub;
  DBusString contained_str;
  DBusValidity contained_signature_validity;
  dbus_bool_t ret;

  _dbus_return_val_if_fail (sub != NULL, FALSE);
  /* Do our best to make sure the sub-iterator doesn't contain something
   * valid-looking on failure */
  _dbus_message_real_iter_zero (real_sub);

  _dbus_return_val_if_fail (_dbus_message_iter_append_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER, FALSE);
  _dbus_return_val_if_fail (dbus_type_is_container (type), FALSE);
  _dbus_return_val_if_fail ((type == DBUS_TYPE_STRUCT &&
                             contained_signature == NULL) ||
                            (type == DBUS_TYPE_DICT_ENTRY &&
                             contained_signature == NULL) ||
                            (type == DBUS_TYPE_VARIANT &&
                             contained_signature != NULL) ||
                            (type == DBUS_TYPE_ARRAY &&
                             contained_signature != NULL), FALSE);
  
  /* this would fail if the contained_signature is a dict entry, since
   * dict entries are invalid signatures standalone (they must be in
   * an array)
   */
  if (contained_signature != NULL)
    {
      _dbus_string_init_const (&contained_str, contained_signature);
      contained_signature_validity = _dbus_validate_signature_with_reason (&contained_str,
          0,
          _dbus_string_get_length (&contained_str));

      if (contained_signature_validity == DBUS_VALIDITY_UNKNOWN_OOM_ERROR)
        return FALSE;
    }
  else
    {
      /* just some placeholder value */
      contained_signature_validity = DBUS_VALID_BUT_INCOMPLETE;
    }

  _dbus_return_val_if_fail ((type == DBUS_TYPE_ARRAY && contained_signature && *contained_signature == DBUS_DICT_ENTRY_BEGIN_CHAR) ||
                            contained_signature == NULL ||
                            contained_signature_validity == DBUS_VALID,
                            FALSE);

  if (!_dbus_message_iter_open_signature (real))
    return FALSE;

  ret = FALSE;
  *real_sub = *real;

  if (contained_signature != NULL)
    {
      _dbus_string_init_const (&contained_str, contained_signature);

      ret = _dbus_type_writer_recurse (&real->u.writer,
                                       type,
                                       &contained_str, 0,
                                       &real_sub->u.writer);
    }
  else
    {
      ret = _dbus_type_writer_recurse (&real->u.writer,
                                       type,
                                       NULL, 0,
                                       &real_sub->u.writer);
    }

  if (!ret)
    _dbus_message_iter_abandon_signature (real);

  return ret;
}


/**
 * Closes a container-typed value appended to the message; may write
 * out more information to the message known only after the entire
 * container is written, and may free resources created by
 * dbus_message_iter_open_container().
 *
 * Even if this function fails due to lack of memory, the sub-iterator sub
 * has been closed and invalidated. It must not be closed again with this
 * function, or abandoned with dbus_message_iter_abandon_container().
 * However, it remains valid to call
 * dbus_message_iter_abandon_container_if_open().
 *
 * @todo If this fails due to lack of memory, the message is hosed and
 * you have to start over building the whole message.
 *
 * @param iter the append iterator
 * @param sub sub-iterator to close
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_iter_close_container (DBusMessageIter *iter,
                                   DBusMessageIter *sub)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusMessageRealIter *real_sub = (DBusMessageRealIter *)sub;
  dbus_bool_t ret;

  _dbus_return_val_if_fail (_dbus_message_iter_append_check (real), FALSE);
  _dbus_return_val_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER, FALSE);
  _dbus_return_val_if_fail (_dbus_message_iter_append_check (real_sub), FALSE);
  _dbus_return_val_if_fail (real_sub->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER, FALSE);

  ret = _dbus_type_writer_unrecurse (&real->u.writer,
                                     &real_sub->u.writer);
  _dbus_message_real_iter_zero (real_sub);

  if (!_dbus_message_iter_close_signature (real))
    ret = FALSE;

  return ret;
}

/**
 * Abandons creation of a contained-typed value and frees resources created
 * by dbus_message_iter_open_container().  Once this returns, the message
 * is hosed and you have to start over building the whole message.
 *
 * This should only be used to abandon creation of a message when you have
 * open containers.
 *
 * @param iter the append iterator
 * @param sub sub-iterator to close
 */
void
dbus_message_iter_abandon_container (DBusMessageIter *iter,
                                     DBusMessageIter *sub)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusMessageRealIter *real_sub = (DBusMessageRealIter *)sub;

#ifndef DBUS_DISABLE_CHECKS
  _dbus_return_if_fail (_dbus_message_iter_append_check (real));
  _dbus_return_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
  _dbus_return_if_fail (_dbus_message_iter_append_check (real_sub));
  _dbus_return_if_fail (real_sub->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
#endif

  _dbus_message_iter_abandon_signature (real);
  _dbus_message_real_iter_zero (real_sub);
}

/**
 * Abandons creation of a contained-typed value and frees resources created
 * by dbus_message_iter_open_container().  Once this returns, the message
 * is hosed and you have to start over building the whole message.
 *
 * Unlike dbus_message_iter_abandon_container(), it is valid to call this
 * function on an iterator that was initialized with
 * #DBUS_MESSAGE_ITER_INIT_CLOSED, or an iterator that was already closed
 * or abandoned. However, it is not valid to call this function on
 * uninitialized memory. This is intended to be used in error cleanup
 * code paths, similar to this pattern:
 *
 *       DBusMessageIter outer = DBUS_MESSAGE_ITER_INIT_CLOSED;
 *       DBusMessageIter inner = DBUS_MESSAGE_ITER_INIT_CLOSED;
 *       dbus_bool_t result = FALSE;
 *
 *       if (!dbus_message_iter_open_container (iter, ..., &outer))
 *         goto out;
 *
 *       if (!dbus_message_iter_open_container (&outer, ..., &inner))
 *         goto out;
 *
 *       if (!dbus_message_iter_append_basic (&inner, ...))
 *         goto out;
 *
 *       if (!dbus_message_iter_close_container (&outer, ..., &inner))
 *         goto out;
 *
 *       if (!dbus_message_iter_close_container (iter, ..., &outer))
 *         goto out;
 *
 *       result = TRUE;
 *
 *     out:
 *       dbus_message_iter_abandon_container_if_open (&outer, &inner);
 *       dbus_message_iter_abandon_container_if_open (iter, &outer);
 *       return result;
 *
 * @param iter the append iterator
 * @param sub sub-iterator to close
 */
void
dbus_message_iter_abandon_container_if_open (DBusMessageIter *iter,
                                             DBusMessageIter *sub)
{
  DBusMessageRealIter *real = (DBusMessageRealIter *)iter;
  DBusMessageRealIter *real_sub = (DBusMessageRealIter *)sub;

  /* If both the parent and the child are zeroed out, then either we didn't
   * even get as far as successfully recursing into the parent, or we already
   * closed both the child and the parent. For example, in the code sample
   * in the doc-comment above, this happens for
   * abandon_container_if_open (&outer, &inner) if the first open_container
   * call failed, or if we reached result = TRUE and fell through. */
  if (_dbus_message_real_iter_is_zeroed (real) &&
      _dbus_message_real_iter_is_zeroed (real_sub))
    return;

#ifndef DBUS_DISABLE_CHECKS
  /* If the child is not zeroed out, but the parent is, then something has
   * gone horribly wrong (in practice that would probably mean both are
   * uninitialized or corrupt, and the parent happens to have ended up
   * all-bytes-zero). */
  _dbus_return_if_fail (_dbus_message_iter_append_check (real));
  _dbus_return_if_fail (real->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
#endif

  /* If the parent is not zeroed out, but the child is, then either we did
   * not successfully open the child, or we already closed the child. This
   * means we do not own a reference to the parent's signature, so it would
   * be wrong to release it; so we must not call abandon_signature() here.
   * In the code sample in the doc-comment above, this happens for
   * abandon_container_if_open (&outer, &inner) if the second open_container
   * call failed, or if the second close_container call failed. */
  if (_dbus_message_real_iter_is_zeroed (real_sub))
    return;

#ifndef DBUS_DISABLE_CHECKS
  _dbus_return_if_fail (_dbus_message_iter_append_check (real_sub));
  _dbus_return_if_fail (real_sub->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);
#endif

  /* If neither the parent nor the child is zeroed out, then we genuinely
   * have an open container; close it. In the code sample in the doc-comment,
   * this happens for abandon_container_if_open (&outer, &inner) if the
   * append_basic call failed. */
  _dbus_message_iter_abandon_signature (real);
  _dbus_message_real_iter_zero (real_sub);
}

/**
 * Sets a flag indicating that the message does not want a reply; if
 * this flag is set, the other end of the connection may (but is not
 * required to) optimize by not sending method return or error
 * replies. If this flag is set, there is no way to know whether the
 * message successfully arrived at the remote end. Normally you know a
 * message was received when you receive the reply to it.
 *
 * The flag is #FALSE by default, that is by default the other end is
 * required to reply.
 *
 * On the protocol level this toggles #DBUS_HEADER_FLAG_NO_REPLY_EXPECTED
 * 
 * @param message the message
 * @param no_reply #TRUE if no reply is desired
 */
void
dbus_message_set_no_reply (DBusMessage *message,
                           dbus_bool_t  no_reply)
{
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (!message->locked);

  _dbus_header_toggle_flag (&message->header,
                            DBUS_HEADER_FLAG_NO_REPLY_EXPECTED,
                            no_reply);
}

/**
 * Returns #TRUE if the message does not expect
 * a reply.
 *
 * @param message the message
 * @returns #TRUE if the message sender isn't waiting for a reply
 */
dbus_bool_t
dbus_message_get_no_reply (DBusMessage *message)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);

  return _dbus_header_get_flag (&message->header,
                                DBUS_HEADER_FLAG_NO_REPLY_EXPECTED);
}

/**
 * Sets a flag indicating that an owner for the destination name will
 * be automatically started before the message is delivered. When this
 * flag is set, the message is held until a name owner finishes
 * starting up, or fails to start up. In case of failure, the reply
 * will be an error.
 *
 * The flag is set to #TRUE by default, i.e. auto starting is the default.
 *
 * On the protocol level this toggles #DBUS_HEADER_FLAG_NO_AUTO_START
 * 
 * @param message the message
 * @param auto_start #TRUE if auto-starting is desired
 */
void
dbus_message_set_auto_start (DBusMessage *message,
                             dbus_bool_t  auto_start)
{
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (!message->locked);

  _dbus_header_toggle_flag (&message->header,
                            DBUS_HEADER_FLAG_NO_AUTO_START,
                            !auto_start);
}

/**
 * Returns #TRUE if the message will cause an owner for
 * destination name to be auto-started.
 *
 * @param message the message
 * @returns #TRUE if the message will use auto-start
 */
dbus_bool_t
dbus_message_get_auto_start (DBusMessage *message)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);

  return !_dbus_header_get_flag (&message->header,
                                 DBUS_HEADER_FLAG_NO_AUTO_START);
}


/**
 * Sets the object path this message is being sent to (for
 * DBUS_MESSAGE_TYPE_METHOD_CALL) or the one a signal is being
 * emitted from (for DBUS_MESSAGE_TYPE_SIGNAL).
 *
 * The path must contain only valid characters as defined
 * in the D-Bus specification.
 *
 * @param message the message
 * @param object_path the path or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_path (DBusMessage   *message,
                       const char    *object_path)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (object_path == NULL ||
                            _dbus_check_is_valid_path (object_path),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_PATH,
                                     DBUS_TYPE_OBJECT_PATH,
                                     object_path);
}

/**
 * Gets the object path this message is being sent to (for
 * DBUS_MESSAGE_TYPE_METHOD_CALL) or being emitted from (for
 * DBUS_MESSAGE_TYPE_SIGNAL). Returns #NULL if none.
 *
 * See also dbus_message_get_path_decomposed().
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 * 
 * @param message the message
 * @returns the path (should not be freed) or #NULL
 */
const char*
dbus_message_get_path (DBusMessage   *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_PATH,
                                DBUS_TYPE_OBJECT_PATH,
                                (void *) &v);
  return v;
}

/**
 * Checks if the message has a particular object path.  The object
 * path is the destination object for a method call or the emitting
 * object for a signal.
 *
 * @param message the message
 * @param path the path name
 * @returns #TRUE if there is a path field in the header
 */
dbus_bool_t
dbus_message_has_path (DBusMessage   *message,
                       const char    *path)
{
  const char *msg_path;
  msg_path = dbus_message_get_path (message);
  
  if (msg_path == NULL)
    {
      if (path == NULL)
        return TRUE;
      else
        return FALSE;
    }

  if (path == NULL)
    return FALSE;
   
  if (strcmp (msg_path, path) == 0)
    return TRUE;

  return FALSE;
}

/**
 * Gets the object path this message is being sent to
 * (for DBUS_MESSAGE_TYPE_METHOD_CALL) or being emitted
 * from (for DBUS_MESSAGE_TYPE_SIGNAL) in a decomposed
 * format (one array element per path component).
 * Free the returned array with dbus_free_string_array().
 *
 * An empty but non-NULL path array means the path "/".
 * So the path "/foo/bar" becomes { "foo", "bar", NULL }
 * and the path "/" becomes { NULL }.
 *
 * See also dbus_message_get_path().
 * 
 * @todo this could be optimized by using the len from the message
 * instead of calling strlen() again
 *
 * @param message the message
 * @param path place to store allocated array of path components; #NULL set here if no path field exists
 * @returns #FALSE if no memory to allocate the array
 */
dbus_bool_t
dbus_message_get_path_decomposed (DBusMessage   *message,
                                  char        ***path)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);

  *path = NULL;

  v = dbus_message_get_path (message);
  if (v != NULL)
    {
      if (!_dbus_decompose_path (v, strlen (v),
                                 path, NULL))
        return FALSE;
    }
  return TRUE;
}

/**
 * Sets the interface this message is being sent to
 * (for DBUS_MESSAGE_TYPE_METHOD_CALL) or
 * the interface a signal is being emitted from
 * (for DBUS_MESSAGE_TYPE_SIGNAL).
 *
 * The interface name must contain only valid characters as defined
 * in the D-Bus specification.
 * 
 * @param message the message
 * @param iface the interface or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_interface (DBusMessage  *message,
                            const char   *iface)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (iface == NULL ||
                            _dbus_check_is_valid_interface (iface),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_INTERFACE,
                                     DBUS_TYPE_STRING,
                                     iface);
}

/**
 * Gets the interface this message is being sent to
 * (for DBUS_MESSAGE_TYPE_METHOD_CALL) or being emitted
 * from (for DBUS_MESSAGE_TYPE_SIGNAL).
 * The interface name is fully-qualified (namespaced).
 * Returns #NULL if none.
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 *
 * @param message the message
 * @returns the message interface (should not be freed) or #NULL
 */
const char*
dbus_message_get_interface (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_INTERFACE,
                                DBUS_TYPE_STRING,
                                (void *) &v);
  return v;
}

/**
 * Checks if the message has an interface
 *
 * @param message the message
 * @param iface the interface name
 * @returns #TRUE if the interface field in the header matches
 */
dbus_bool_t
dbus_message_has_interface (DBusMessage   *message,
                            const char    *iface)
{
  const char *msg_interface;
  msg_interface = dbus_message_get_interface (message);
   
  if (msg_interface == NULL)
    {
      if (iface == NULL)
        return TRUE;
      else
        return FALSE;
    }

  if (iface == NULL)
    return FALSE;
     
  if (strcmp (msg_interface, iface) == 0)
    return TRUE;

  return FALSE;

}

/**
 * Sets the interface member being invoked
 * (DBUS_MESSAGE_TYPE_METHOD_CALL) or emitted
 * (DBUS_MESSAGE_TYPE_SIGNAL).
 *
 * The member name must contain only valid characters as defined
 * in the D-Bus specification.
 *
 * @param message the message
 * @param member the member or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_member (DBusMessage  *message,
                         const char   *member)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (member == NULL ||
                            _dbus_check_is_valid_member (member),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_MEMBER,
                                     DBUS_TYPE_STRING,
                                     member);
}

/**
 * Gets the interface member being invoked
 * (DBUS_MESSAGE_TYPE_METHOD_CALL) or emitted
 * (DBUS_MESSAGE_TYPE_SIGNAL). Returns #NULL if none.
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 * 
 * @param message the message
 * @returns the member name (should not be freed) or #NULL
 */
const char*
dbus_message_get_member (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_MEMBER,
                                DBUS_TYPE_STRING,
                                (void *) &v);
  return v;
}

/**
 * Checks if the message has an interface member
 *
 * @param message the message
 * @param member the member name
 * @returns #TRUE if there is a member field in the header
 */
dbus_bool_t
dbus_message_has_member (DBusMessage   *message,
                         const char    *member)
{
  const char *msg_member;
  msg_member = dbus_message_get_member (message);
 
  if (msg_member == NULL)
    {
      if (member == NULL)
        return TRUE;
      else
        return FALSE;
    }

  if (member == NULL)
    return FALSE;
    
  if (strcmp (msg_member, member) == 0)
    return TRUE;

  return FALSE;

}

/**
 * Sets the name of the error (DBUS_MESSAGE_TYPE_ERROR).
 * The name is fully-qualified (namespaced).
 *
 * The error name must contain only valid characters as defined
 * in the D-Bus specification.
 *
 * @param message the message
 * @param error_name the name or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_error_name (DBusMessage  *message,
                             const char   *error_name)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (error_name == NULL ||
                            _dbus_check_is_valid_error_name (error_name),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_ERROR_NAME,
                                     DBUS_TYPE_STRING,
                                     error_name);
}

/**
 * Gets the error name (DBUS_MESSAGE_TYPE_ERROR only)
 * or #NULL if none.
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 * 
 * @param message the message
 * @returns the error name (should not be freed) or #NULL
 */
const char*
dbus_message_get_error_name (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_ERROR_NAME,
                                DBUS_TYPE_STRING,
                                (void *) &v);
  return v;
}

/**
 * Sets the message's destination. The destination is the name of
 * another connection on the bus and may be either the unique name
 * assigned by the bus to each connection, or a well-known name
 * specified in advance.
 *
 * The destination name must contain only valid characters as defined
 * in the D-Bus specification.
 * 
 * @param message the message
 * @param destination the destination name or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_destination (DBusMessage  *message,
                              const char   *destination)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (destination == NULL ||
                            _dbus_check_is_valid_bus_name (destination),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_DESTINATION,
                                     DBUS_TYPE_STRING,
                                     destination);
}

/**
 * Gets the destination of a message or #NULL if there is none set.
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 *
 * @param message the message
 * @returns the message destination (should not be freed) or #NULL
 */
const char*
dbus_message_get_destination (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_DESTINATION,
                                DBUS_TYPE_STRING,
                                (void *) &v);
  return v;
}

/**
 * Sets the message sender.
 *
 * The sender must be a valid bus name as defined in the D-Bus
 * specification.
 *
 * Usually you don't want to call this. The message bus daemon will
 * call it to set the origin of each message. If you aren't implementing
 * a message bus daemon you shouldn't need to set the sender.
 *
 * @param message the message
 * @param sender the sender or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_sender (DBusMessage  *message,
                         const char   *sender)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (sender == NULL ||
                            _dbus_check_is_valid_bus_name (sender),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_SENDER,
                                     DBUS_TYPE_STRING,
                                     sender);
}

/**
 * Gets the unique name of the connection which originated this
 * message, or #NULL if unknown or inapplicable. The sender is filled
 * in by the message bus.
 *
 * Note, the returned sender is always the unique bus name.
 * Connections may own multiple other bus names, but those
 * are not found in the sender field.
 * 
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 *
 * @param message the message
 * @returns the unique name of the sender or #NULL
 */
const char*
dbus_message_get_sender (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_SENDER,
                                DBUS_TYPE_STRING,
                                (void *) &v);
  return v;
}

/**
 * Gets the type signature of the message, i.e. the arguments in the
 * message payload. The signature includes only "in" arguments for
 * #DBUS_MESSAGE_TYPE_METHOD_CALL and only "out" arguments for
 * #DBUS_MESSAGE_TYPE_METHOD_RETURN, so is slightly different from
 * what you might expect (that is, it does not include the signature of the
 * entire C++-style method).
 *
 * The signature is a string made up of type codes such as
 * #DBUS_TYPE_INT32. The string is terminated with nul (nul is also
 * the value of #DBUS_TYPE_INVALID).
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 *
 * @param message the message
 * @returns the type signature
 */
const char*
dbus_message_get_signature (DBusMessage *message)
{
  const DBusString *type_str;
  int type_pos;

  _dbus_return_val_if_fail (message != NULL, NULL);

  get_const_signature (&message->header, &type_str, &type_pos);

  return _dbus_string_get_const_data_len (type_str, type_pos, 0);
}

static dbus_bool_t
_dbus_message_has_type_interface_member (DBusMessage *message,
                                         int          type,
                                         const char  *iface,
                                         const char  *member)
{
  const char *n;

  _dbus_assert (message != NULL);
  _dbus_assert (iface != NULL);
  _dbus_assert (member != NULL);

  if (dbus_message_get_type (message) != type)
    return FALSE;

  /* Optimize by checking the short member name first
   * instead of the longer interface name
   */

  n = dbus_message_get_member (message);

  if (n && strcmp (n, member) == 0)
    {
      n = dbus_message_get_interface (message);

      if (n == NULL || strcmp (n, iface) == 0)
        return TRUE;
    }

  return FALSE;
}

/**
 * Checks whether the message is a method call with the given
 * interface and member fields.  If the message is not
 * #DBUS_MESSAGE_TYPE_METHOD_CALL, or has a different interface or
 * member field, returns #FALSE. If the interface field is missing,
 * then it will be assumed equal to the provided interface.  The D-Bus
 * protocol allows method callers to leave out the interface name.
 *
 * @param message the message
 * @param iface the name to check (must not be #NULL)
 * @param method the name to check (must not be #NULL)
 *
 * @returns #TRUE if the message is the specified method call
 */
dbus_bool_t
dbus_message_is_method_call (DBusMessage *message,
                             const char  *iface,
                             const char  *method)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (iface != NULL, FALSE);
  _dbus_return_val_if_fail (method != NULL, FALSE);
  /* don't check that interface/method are valid since it would be
   * expensive, and not catch many common errors
   */

  return _dbus_message_has_type_interface_member (message,
                                                  DBUS_MESSAGE_TYPE_METHOD_CALL,
                                                  iface, method);
}

/**
 * Checks whether the message is a signal with the given interface and
 * member fields.  If the message is not #DBUS_MESSAGE_TYPE_SIGNAL, or
 * has a different interface or member field, returns #FALSE.
 *
 * @param message the message
 * @param iface the name to check (must not be #NULL)
 * @param signal_name the name to check (must not be #NULL)
 *
 * @returns #TRUE if the message is the specified signal
 */
dbus_bool_t
dbus_message_is_signal (DBusMessage *message,
                        const char  *iface,
                        const char  *signal_name)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (iface != NULL, FALSE);
  _dbus_return_val_if_fail (signal_name != NULL, FALSE);
  /* don't check that interface/name are valid since it would be
   * expensive, and not catch many common errors
   */

  return _dbus_message_has_type_interface_member (message,
                                                  DBUS_MESSAGE_TYPE_SIGNAL,
                                                  iface, signal_name);
}

/**
 * Checks whether the message is an error reply with the given error
 * name.  If the message is not #DBUS_MESSAGE_TYPE_ERROR, or has a
 * different name, returns #FALSE.
 *
 * @param message the message
 * @param error_name the name to check (must not be #NULL)
 *
 * @returns #TRUE if the message is the specified error
 */
dbus_bool_t
dbus_message_is_error (DBusMessage *message,
                       const char  *error_name)
{
  const char *n;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (error_name != NULL, FALSE);
  /* don't check that error_name is valid since it would be expensive,
   * and not catch many common errors
   */

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_ERROR)
    return FALSE;

  n = dbus_message_get_error_name (message);

  if (n && strcmp (n, error_name) == 0)
    return TRUE;
  else
    return FALSE;
}

/**
 * Checks whether the message was sent to the given name.  If the
 * message has no destination specified or has a different
 * destination, returns #FALSE.
 *
 * @param message the message
 * @param name the name to check (must not be #NULL)
 *
 * @returns #TRUE if the message has the given destination name
 */
dbus_bool_t
dbus_message_has_destination (DBusMessage  *message,
                              const char   *name)
{
  const char *s;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (name != NULL, FALSE);
  /* don't check that name is valid since it would be expensive, and
   * not catch many common errors
   */

  s = dbus_message_get_destination (message);

  if (s && strcmp (s, name) == 0)
    return TRUE;
  else
    return FALSE;
}

/**
 * Checks whether the message has the given unique name as its sender.
 * If the message has no sender specified or has a different sender,
 * returns #FALSE. Note that a peer application will always have the
 * unique name of the connection as the sender. So you can't use this
 * function to see whether a sender owned a well-known name.
 *
 * Messages from the bus itself will have #DBUS_SERVICE_DBUS
 * as the sender.
 *
 * @param message the message
 * @param name the name to check (must not be #NULL)
 *
 * @returns #TRUE if the message has the given sender
 */
dbus_bool_t
dbus_message_has_sender (DBusMessage  *message,
                         const char   *name)
{
  const char *s;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (name != NULL, FALSE);
  /* don't check that name is valid since it would be expensive, and
   * not catch many common errors
   */

  s = dbus_message_get_sender (message);

  if (s && strcmp (s, name) == 0)
    return TRUE;
  else
    return FALSE;
}

/**
 * Checks whether the message has the given signature; see
 * dbus_message_get_signature() for more details on what the signature
 * looks like.
 *
 * @param message the message
 * @param signature typecode array
 * @returns #TRUE if message has the given signature
*/
dbus_bool_t
dbus_message_has_signature (DBusMessage   *message,
                            const char    *signature)
{
  const char *s;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (signature != NULL, FALSE);
  /* don't check that signature is valid since it would be expensive,
   * and not catch many common errors
   */

  s = dbus_message_get_signature (message);

  if (s && strcmp (s, signature) == 0)
    return TRUE;
  else
    return FALSE;
}

/**
 * Sets a #DBusError based on the contents of the given
 * message. The error is only set if the message
 * is an error message, as in #DBUS_MESSAGE_TYPE_ERROR.
 * The name of the error is set to the name of the message,
 * and the error message is set to the first argument
 * if the argument exists and is a string.
 *
 * The return value indicates whether the error was set (the error is
 * set if and only if the message is an error message).  So you can
 * check for an error reply and convert it to DBusError in one go:
 * @code
 *  if (dbus_set_error_from_message (error, reply))
 *    return error;
 *  else
 *    process reply;
 * @endcode
 *
 * @param error the error to set
 * @param message the message to set it from
 * @returns #TRUE if the message had type #DBUS_MESSAGE_TYPE_ERROR
 */
dbus_bool_t
dbus_set_error_from_message (DBusError   *error,
                             DBusMessage *message)
{
  const char *str;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_error_is_set (error, FALSE);

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_ERROR)
    return FALSE;

  str = NULL;
  dbus_message_get_args (message, NULL,
                         DBUS_TYPE_STRING, &str,
                         DBUS_TYPE_INVALID);

  dbus_set_error (error, dbus_message_get_error_name (message),
                  str ? "%s" : NULL, str);

  return TRUE;
}

/**
 * Checks whether a message contains unix fds
 *
 * @param message the message
 * @returns #TRUE if the message contains unix fds
 */
dbus_bool_t
dbus_message_contains_unix_fds(DBusMessage *message)
{
#ifdef HAVE_UNIX_FD_PASSING
  _dbus_assert(message);

  return message->n_unix_fds > 0;
#else
  return FALSE;
#endif
}

/**
 * Sets the container instance this message was sent from.
 *
 * The path must contain only valid characters for an object path
 * as defined in the D-Bus specification.
 *
 * @param message the message
 * @param object_path the path or #NULL to unset
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_message_set_container_instance (DBusMessage   *message,
                                     const char    *object_path)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (!message->locked, FALSE);
  _dbus_return_val_if_fail (object_path == NULL ||
                            _dbus_check_is_valid_path (object_path),
                            FALSE);

  return set_or_delete_string_field (message,
                                     DBUS_HEADER_FIELD_CONTAINER_INSTANCE,
                                     DBUS_TYPE_OBJECT_PATH,
                                     object_path);
}

/**
 * Gets the container instance this message was sent from, or #NULL
 * if none.
 *
 * The returned string becomes invalid if the message is
 * modified, since it points into the wire-marshaled message data.
 *
 * @param message the message
 * @returns the path (should not be freed) or #NULL
 */
const char *
dbus_message_get_container_instance (DBusMessage *message)
{
  const char *v;

  _dbus_return_val_if_fail (message != NULL, NULL);

  v = NULL; /* in case field doesn't exist */
  _dbus_header_get_field_basic (&message->header,
                                DBUS_HEADER_FIELD_CONTAINER_INSTANCE,
                                DBUS_TYPE_OBJECT_PATH,
                                (void *) &v);
  return v;
}

/** @} */

/**
 * @addtogroup DBusMessageInternals
 *
 * @{
 */

/**
 * The initial buffer size of the message loader.
 *
 * @todo this should be based on min header size plus some average
 * body size, or something. Or rather, the min header size only, if we
 * want to try to read only the header, store that in a DBusMessage,
 * then read only the body and store that, etc., depends on
 * how we optimize _dbus_message_loader_get_buffer() and what
 * the exact message format is.
 */
#define INITIAL_LOADER_DATA_LEN 32

/**
 * Creates a new message loader. Returns #NULL if memory can't
 * be allocated.
 *
 * @returns new loader, or #NULL.
 */
DBusMessageLoader*
_dbus_message_loader_new (void)
{
  DBusMessageLoader *loader;

  loader = dbus_new0 (DBusMessageLoader, 1);
  if (loader == NULL)
    return NULL;
  
  loader->refcount = 1;

  loader->corrupted = FALSE;
  loader->corruption_reason = DBUS_VALID;

  /* this can be configured by the app, but defaults to the protocol max */
  loader->max_message_size = DBUS_MAXIMUM_MESSAGE_LENGTH;

  /* We set a very relatively conservative default here since due to how
  SCM_RIGHTS works we need to preallocate an fd array of the maximum
  number of unix fds we want to receive in advance. A
  try-and-reallocate loop is not possible. */
  loader->max_message_unix_fds = DBUS_DEFAULT_MESSAGE_UNIX_FDS;

  if (!_dbus_string_init (&loader->data))
    {
      dbus_free (loader);
      return NULL;
    }

  /* preallocate the buffer for speed, ignore failure */
  _dbus_string_set_length (&loader->data, INITIAL_LOADER_DATA_LEN);
  _dbus_string_set_length (&loader->data, 0);

#ifdef HAVE_UNIX_FD_PASSING
  loader->unix_fds = NULL;
  loader->n_unix_fds = loader->n_unix_fds_allocated = 0;
  loader->unix_fds_outstanding = FALSE;
#endif

  return loader;
}

/**
 * Increments the reference count of the loader.
 *
 * @param loader the loader.
 * @returns the loader
 */
DBusMessageLoader *
_dbus_message_loader_ref (DBusMessageLoader *loader)
{
  loader->refcount += 1;

  return loader;
}

/**
 * Decrements the reference count of the loader and finalizes the
 * loader when the count reaches zero.
 *
 * @param loader the loader.
 */
void
_dbus_message_loader_unref (DBusMessageLoader *loader)
{
  loader->refcount -= 1;
  if (loader->refcount == 0)
    {
#ifdef HAVE_UNIX_FD_PASSING
      close_unix_fds(loader->unix_fds, &loader->n_unix_fds);
      dbus_free(loader->unix_fds);
#endif
      _dbus_list_clear_full (&loader->messages,
                             (DBusFreeFunction) dbus_message_unref);
      _dbus_string_free (&loader->data);
      dbus_free (loader);
    }
}

/**
 * Gets the buffer to use for reading data from the network.  Network
 * data is read directly into an allocated buffer, which is then used
 * in the DBusMessage, to avoid as many extra memcpy's as possible.
 * The buffer must always be returned immediately using
 * _dbus_message_loader_return_buffer(), even if no bytes are
 * successfully read.
 *
 * @todo this function can be a lot more clever. For example
 * it can probably always return a buffer size to read exactly
 * the body of the next message, thus avoiding any memory wastage
 * or reallocs.
 *
 * @todo we need to enforce a max length on strings in header fields.
 *
 * @param loader the message loader.
 * @param buffer the buffer
 */
void
_dbus_message_loader_get_buffer (DBusMessageLoader  *loader,
                                 DBusString        **buffer,
                                 int                *max_to_read,
                                 dbus_bool_t        *may_read_fds)
{
  _dbus_assert (!loader->buffer_outstanding);

  *buffer = &loader->data;

  loader->buffer_outstanding = TRUE;

  if (max_to_read != NULL)
    {
#ifdef HAVE_UNIX_FD_PASSING
      int offset = 0;
      int remain;
      int byte_order;
      int fields_array_len;
      int header_len;
      int body_len;
#endif

      *max_to_read = DBUS_MAXIMUM_MESSAGE_LENGTH;
      *may_read_fds = TRUE;

#ifdef HAVE_UNIX_FD_PASSING
      /* If we aren't holding onto any fds, we can read as much as we want
       * (fast path). */
      if (loader->n_unix_fds == 0)
        return;

      /* Slow path: we have a message with some fds in it. We don't want
       * to start on the next message until this one is out of the way;
       * otherwise a legitimate sender can keep us processing messages
       * containing fds, until we disconnect it for having had fds pending
       * for too long, a limit that is in place to stop malicious senders
       * from setting up recursive fd-passing that takes up our quota and
       * will never go away. */

      remain = _dbus_string_get_length (&loader->data);

      while (remain > 0)
        {
          DBusValidity validity = DBUS_VALIDITY_UNKNOWN;
          int needed;

          /* If 0 < remain < DBUS_MINIMUM_HEADER_SIZE, then we've had at
           * least the first byte of a message, but we don't know how
           * much more to read. Only read the rest of the
           * DBUS_MINIMUM_HEADER_SIZE for now; then we'll know. */
          if (remain < DBUS_MINIMUM_HEADER_SIZE)
            {
              *max_to_read = DBUS_MINIMUM_HEADER_SIZE - remain;
              *may_read_fds = FALSE;
              return;
            }

          if (!_dbus_header_have_message_untrusted (loader->max_message_size,
                                                    &validity,
                                                    &byte_order,
                                                    &fields_array_len,
                                                    &header_len,
                                                    &body_len,
                                                    &loader->data,
                                                    offset,
                                                    remain))
            {
              /* If a message in the buffer is invalid, we're going to
               * disconnect the sender anyway, so reading an arbitrary amount
               * is fine. */
              if (validity != DBUS_VALID)
                return;

              /* We have a partial message, with the
               * DBUS_MINIMUM_HEADER_SIZE-byte fixed part of the header (which
               * lets us work out how much more we need), but no more. Read
               * the rest of the message. */
              needed = header_len + body_len;
              _dbus_assert (needed > remain);
              *max_to_read = needed - remain;
              *may_read_fds = FALSE;
              return;
            }

          /* Skip over entire messages until we have less than a message
           * remaining. */
          needed = header_len + body_len;
          _dbus_assert (needed > DBUS_MINIMUM_HEADER_SIZE);
          _dbus_assert (remain >= needed);
          remain -= needed;
          offset += needed;
        }
#endif
    }
}

/**
 * Returns a buffer obtained from _dbus_message_loader_get_buffer(),
 * indicating to the loader how many bytes of the buffer were filled
 * in. This function must always be called, even if no bytes were
 * successfully read.
 *
 * @param loader the loader.
 * @param buffer the buffer.
 */
void
_dbus_message_loader_return_buffer (DBusMessageLoader  *loader,
                                    DBusString         *buffer)
{
  _dbus_assert (loader->buffer_outstanding);
  _dbus_assert (buffer == &loader->data);

  loader->buffer_outstanding = FALSE;
}

#ifdef HAVE_UNIX_FD_PASSING
/**
 * Gets the buffer to use for reading unix fds from the network.
 *
 * This works similar to _dbus_message_loader_get_buffer()
 *
 * @param loader the message loader.
 * @param fds the array to read fds into
 * @param max_n_fds how many fds to read at most
 * @return TRUE on success, FALSE on OOM
 */
dbus_bool_t
_dbus_message_loader_get_unix_fds(DBusMessageLoader  *loader,
                                  int               **fds,
                                  unsigned           *max_n_fds)
{
  _dbus_assert (!loader->unix_fds_outstanding);

  /* Allocate space where we can put the fds we read. We allocate
     space for max_message_unix_fds since this is an
     upper limit how many fds can be received within a single
     message. Since SCM_RIGHTS doesn't allow a reallocate+retry logic
     we are allocating the maximum possible array size right from the
     beginning. This sucks a bit, however unless SCM_RIGHTS is fixed
     there is no better way. */

  if (loader->n_unix_fds_allocated < loader->max_message_unix_fds)
    {
      int *a = dbus_realloc(loader->unix_fds,
                            loader->max_message_unix_fds * sizeof(loader->unix_fds[0]));

      if (!a)
        return FALSE;

      loader->unix_fds = a;
      loader->n_unix_fds_allocated = loader->max_message_unix_fds;
    }

  *fds = loader->unix_fds + loader->n_unix_fds;
  *max_n_fds = loader->n_unix_fds_allocated - loader->n_unix_fds;

  loader->unix_fds_outstanding = TRUE;
  return TRUE;
}

/**
 * Returns a buffer obtained from _dbus_message_loader_get_unix_fds().
 *
 * This works similar to _dbus_message_loader_return_buffer()
 *
 * @param loader the message loader.
 * @param fds the array fds were read into
 * @param n_fds how many fds were read
 */

void
_dbus_message_loader_return_unix_fds(DBusMessageLoader  *loader,
                                     int                *fds,
                                     unsigned            n_fds)
{
  _dbus_assert(loader->unix_fds_outstanding);
  _dbus_assert(loader->unix_fds + loader->n_unix_fds == fds);
  _dbus_assert(loader->n_unix_fds + n_fds <= loader->n_unix_fds_allocated);

  loader->n_unix_fds += n_fds;
  loader->unix_fds_outstanding = FALSE;

  if (n_fds && loader->unix_fds_change)
    loader->unix_fds_change (loader->unix_fds_change_data);
}
#endif

/*
 * FIXME when we move the header out of the buffer, that memmoves all
 * buffered messages. Kind of crappy.
 *
 * Also we copy the header and body, which is kind of crappy.  To
 * avoid this, we have to allow header and body to be in a single
 * memory block, which is good for messages we read and bad for
 * messages we are creating. But we could move_len() the buffer into
 * this single memory block, and move_len() will just swap the buffers
 * if you're moving the entire buffer replacing the dest string.
 *
 * We could also have the message loader tell the transport how many
 * bytes to read; so it would first ask for some arbitrary number like
 * 256, then if the message was incomplete it would use the
 * header/body len to ask for exactly the size of the message (or
 * blocks the size of a typical kernel buffer for the socket). That
 * way we don't get trailing bytes in the buffer that have to be
 * memmoved. Though I suppose we also don't have a chance of reading a
 * bunch of small messages at once, so the optimization may be stupid.
 *
 * Another approach would be to keep a "start" index into
 * loader->data and only delete it occasionally, instead of after
 * each message is loaded.
 *
 * load_message() returns FALSE if not enough memory OR the loader was corrupted
 */
static dbus_bool_t
load_message (DBusMessageLoader *loader,
              DBusMessage       *message,
              int                byte_order,
              int                fields_array_len,
              int                header_len,
              int                body_len)
{
  dbus_bool_t oom;
  DBusValidity validity;
  const DBusString *type_str;
  int type_pos;
  DBusValidationMode mode;
  dbus_uint32_t n_unix_fds = 0;

  mode = DBUS_VALIDATION_MODE_DATA_IS_UNTRUSTED;
  
  oom = FALSE;

#if 0
  _dbus_verbose_bytes_of_string (&loader->data, 0, header_len /* + body_len */);
#endif

  /* 1. VALIDATE AND COPY OVER HEADER */
  _dbus_assert (_dbus_string_get_length (&message->header.data) == 0);
  _dbus_assert ((header_len + body_len) <= _dbus_string_get_length (&loader->data));

  if (!_dbus_header_load (&message->header,
                          mode,
                          &validity,
                          byte_order,
                          fields_array_len,
                          header_len,
                          body_len,
                          &loader->data))
    {
      _dbus_verbose ("Failed to load header for new message code %d\n", validity);

      /* assert here so we can catch any code that still uses DBUS_VALID to indicate
         oom errors.  They should use DBUS_VALIDITY_UNKNOWN_OOM_ERROR instead */
      _dbus_assert (validity != DBUS_VALID);

      if (validity == DBUS_VALIDITY_UNKNOWN_OOM_ERROR)
        oom = TRUE;
      else
        {
          loader->corrupted = TRUE;
          loader->corruption_reason = validity;
        }
      goto failed;
    }

  _dbus_assert (validity == DBUS_VALID);

  /* 2. VALIDATE BODY */
  if (mode != DBUS_VALIDATION_MODE_WE_TRUST_THIS_DATA_ABSOLUTELY)
    {
      get_const_signature (&message->header, &type_str, &type_pos);
      
      /* Because the bytes_remaining arg is NULL, this validates that the
       * body is the right length
       */
      validity = _dbus_validate_body_with_reason (type_str,
                                                  type_pos,
                                                  byte_order,
                                                  NULL,
                                                  &loader->data,
                                                  header_len,
                                                  body_len);
      if (validity != DBUS_VALID)
        {
          _dbus_verbose ("Failed to validate message body code %d\n", validity);

          loader->corrupted = TRUE;
          loader->corruption_reason = validity;
          
          goto failed;
        }
    }

  /* 3. COPY OVER UNIX FDS */
  _dbus_header_get_field_basic(&message->header,
                               DBUS_HEADER_FIELD_UNIX_FDS,
                               DBUS_TYPE_UINT32,
                               &n_unix_fds);

#ifdef HAVE_UNIX_FD_PASSING

  if (n_unix_fds > loader->n_unix_fds)
    {
      _dbus_verbose("Message contains references to more unix fds than were sent %u != %u\n",
                    n_unix_fds, loader->n_unix_fds);

      loader->corrupted = TRUE;
      loader->corruption_reason = DBUS_INVALID_MISSING_UNIX_FDS;
      goto failed;
    }

  /* If this was a recycled message there might still be
     some memory allocated for the fds */
  dbus_free(message->unix_fds);

  if (n_unix_fds > 0)
    {
      message->unix_fds = _dbus_memdup(loader->unix_fds, n_unix_fds * sizeof(message->unix_fds[0]));
      if (message->unix_fds == NULL)
        {
          _dbus_verbose ("Failed to allocate file descriptor array\n");
          oom = TRUE;
          goto failed;
        }

      message->n_unix_fds_allocated = message->n_unix_fds = n_unix_fds;
      loader->n_unix_fds -= n_unix_fds;
      memmove (loader->unix_fds, loader->unix_fds + n_unix_fds, loader->n_unix_fds * sizeof (loader->unix_fds[0]));

      if (loader->unix_fds_change)
        loader->unix_fds_change (loader->unix_fds_change_data);
    }
  else
    message->unix_fds = NULL;

#else

  if (n_unix_fds > 0)
    {
      _dbus_verbose ("Hmm, message claims to come with file descriptors "
                     "but that's not supported on our platform, disconnecting.\n");

      loader->corrupted = TRUE;
      loader->corruption_reason = DBUS_INVALID_MISSING_UNIX_FDS;
      goto failed;
    }

#endif

  /* 3. COPY OVER BODY AND QUEUE MESSAGE */

  if (!_dbus_list_append (&loader->messages, message))
    {
      _dbus_verbose ("Failed to append new message to loader queue\n");
      oom = TRUE;
      goto failed;
    }

  _dbus_assert (_dbus_string_get_length (&message->body) == 0);
  _dbus_assert (_dbus_string_get_length (&loader->data) >=
                (header_len + body_len));

  if (!_dbus_string_copy_len (&loader->data, header_len, body_len, &message->body, 0))
    {
      _dbus_verbose ("Failed to move body into new message\n");
      oom = TRUE;
      goto failed;
    }

  _dbus_string_delete (&loader->data, 0, header_len + body_len);

  /* don't waste more than 2k of memory */
  _dbus_string_compact (&loader->data, 2048);

  _dbus_assert (_dbus_string_get_length (&message->header.data) == header_len);
  _dbus_assert (_dbus_string_get_length (&message->body) == body_len);

  _dbus_verbose ("Loaded message %p\n", message);

  _dbus_assert (!oom);
  _dbus_assert (!loader->corrupted);
  _dbus_assert (loader->messages != NULL);
  _dbus_assert (_dbus_list_find_last (&loader->messages, message) != NULL);

  return TRUE;

 failed:

  /* Clean up */

  /* does nothing if the message isn't in the list */
  _dbus_list_remove_last (&loader->messages, message);
  
  if (oom)
    _dbus_assert (!loader->corrupted);
  else
    _dbus_assert (loader->corrupted);

  _dbus_verbose_bytes_of_string (&loader->data, 0, _dbus_string_get_length (&loader->data));

  return FALSE;
}

/**
 * Converts buffered data into messages, if we have enough data.  If
 * we don't have enough data, does nothing.
 *
 * @todo we need to check that the proper named header fields exist
 * for each message type.
 *
 * @todo If a message has unknown type, we should probably eat it
 * right here rather than passing it out to applications.  However
 * it's not an error to see messages of unknown type.
 *
 * @param loader the loader.
 * @returns #TRUE if we had enough memory to finish.
 */
dbus_bool_t
_dbus_message_loader_queue_messages (DBusMessageLoader *loader)
{
  while (!loader->corrupted &&
         _dbus_string_get_length (&loader->data) >= DBUS_MINIMUM_HEADER_SIZE)
    {
      DBusValidity validity;
      int byte_order, fields_array_len, header_len, body_len;

      if (_dbus_header_have_message_untrusted (loader->max_message_size,
                                               &validity,
                                               &byte_order,
                                               &fields_array_len,
                                               &header_len,
                                               &body_len,
                                               &loader->data, 0,
                                               _dbus_string_get_length (&loader->data)))
        {
          DBusMessage *message;

          _dbus_assert (validity == DBUS_VALID);

          message = dbus_message_new_empty_header ();
          if (message == NULL)
            return FALSE;

          if (!load_message (loader, message,
                             byte_order, fields_array_len,
                             header_len, body_len))
            {
              dbus_message_unref (message);
              /* load_message() returns false if corrupted or OOM; if
               * corrupted then return TRUE for not OOM
               */
              return loader->corrupted;
            }

          _dbus_assert (loader->messages != NULL);
          _dbus_assert (_dbus_list_find_last (&loader->messages, message) != NULL);
	}
      else
        {
          _dbus_verbose ("Initial peek at header says we don't have a whole message yet, or data broken with invalid code %d\n",
                         validity);
          if (validity != DBUS_VALID)
            {
              loader->corrupted = TRUE;
              loader->corruption_reason = validity;
            }
          return TRUE;
        }
    }

  return TRUE;
}

/**
 * Peeks at first loaded message, returns #NULL if no messages have
 * been queued.
 *
 * @param loader the loader.
 * @returns the next message, or #NULL if none.
 */
DBusMessage*
_dbus_message_loader_peek_message (DBusMessageLoader *loader)
{
  if (loader->messages)
    return loader->messages->data;
  else
    return NULL;
}

/**
 * Pops a loaded message (passing ownership of the message
 * to the caller). Returns #NULL if no messages have been
 * queued.
 *
 * @param loader the loader.
 * @returns the next message, or #NULL if none.
 */
DBusMessage*
_dbus_message_loader_pop_message (DBusMessageLoader *loader)
{
  return _dbus_list_pop_first (&loader->messages);
}

/**
 * Pops a loaded message inside a list link (passing ownership of the
 * message and link to the caller). Returns #NULL if no messages have
 * been loaded.
 *
 * @param loader the loader.
 * @returns the next message link, or #NULL if none.
 */
DBusList*
_dbus_message_loader_pop_message_link (DBusMessageLoader *loader)
{
  return _dbus_list_pop_first_link (&loader->messages);
}

/**
 * Returns a popped message link, used to undo a pop.
 *
 * @param loader the loader
 * @param link the link with a message in it
 */
void
_dbus_message_loader_putback_message_link (DBusMessageLoader  *loader,
                                           DBusList           *link)
{
  _dbus_list_prepend_link (&loader->messages, link);
}

/**
 * Checks whether the loader is confused due to bad data.
 * If messages are received that are invalid, the
 * loader gets confused and gives up permanently.
 * This state is called "corrupted."
 *
 * @param loader the loader
 * @returns #TRUE if the loader is hosed.
 */
dbus_bool_t
_dbus_message_loader_get_is_corrupted (DBusMessageLoader *loader)
{
  _dbus_assert ((loader->corrupted && loader->corruption_reason != DBUS_VALID) ||
                (!loader->corrupted && loader->corruption_reason == DBUS_VALID));
  return loader->corrupted;
}

/**
 * Checks what kind of bad data confused the loader.
 *
 * @param loader the loader
 * @returns why the loader is hosed, or DBUS_VALID if it isn't.
 */
DBusValidity
_dbus_message_loader_get_corruption_reason (DBusMessageLoader *loader)
{
  _dbus_assert ((loader->corrupted && loader->corruption_reason != DBUS_VALID) ||
                (!loader->corrupted && loader->corruption_reason == DBUS_VALID));

  return loader->corruption_reason;
}

/**
 * Sets the maximum size message we allow.
 *
 * @param loader the loader
 * @param size the max message size in bytes
 */
void
_dbus_message_loader_set_max_message_size (DBusMessageLoader  *loader,
                                           long                size)
{
  if (size > DBUS_MAXIMUM_MESSAGE_LENGTH)
    {
      _dbus_verbose ("clamping requested max message size %ld to %d\n",
                     size, DBUS_MAXIMUM_MESSAGE_LENGTH);
      size = DBUS_MAXIMUM_MESSAGE_LENGTH;
    }
  loader->max_message_size = size;
}

/**
 * Gets the maximum allowed message size in bytes.
 *
 * @param loader the loader
 * @returns max size in bytes
 */
long
_dbus_message_loader_get_max_message_size (DBusMessageLoader  *loader)
{
  return loader->max_message_size;
}

/**
 * Sets the maximum unix fds per message we allow.
 *
 * @param loader the loader
 * @param n the max number of unix fds in a message
 */
void
_dbus_message_loader_set_max_message_unix_fds (DBusMessageLoader  *loader,
                                               long                n)
{
  if (n > DBUS_MAXIMUM_MESSAGE_UNIX_FDS)
    {
      _dbus_verbose ("clamping requested max message unix_fds %ld to %d\n",
                     n, DBUS_MAXIMUM_MESSAGE_UNIX_FDS);
      n = DBUS_MAXIMUM_MESSAGE_UNIX_FDS;
    }
  loader->max_message_unix_fds = n;
}

/**
 * Gets the maximum allowed number of unix fds per message
 *
 * @param loader the loader
 * @returns max unix fds
 */
long
_dbus_message_loader_get_max_message_unix_fds (DBusMessageLoader  *loader)
{
  return loader->max_message_unix_fds;
}

/**
 * Return how many file descriptors are pending in the loader
 *
 * @param loader the loader
 */
int
_dbus_message_loader_get_pending_fds_count (DBusMessageLoader *loader)
{
#ifdef HAVE_UNIX_FD_PASSING
  return loader->n_unix_fds;
#else
  return 0;
#endif
}

/**
 * Register a function to be called whenever the number of pending file
 * descriptors in the loader change.
 *
 * @param loader the loader
 * @param callback the callback
 * @param data the data for the callback
 */
void
_dbus_message_loader_set_pending_fds_function (DBusMessageLoader *loader,
                                               void (* callback) (void *),
                                               void *data)
{
#ifdef HAVE_UNIX_FD_PASSING
  loader->unix_fds_change = callback;
  loader->unix_fds_change_data = data;
#endif
}

static DBusDataSlotAllocator slot_allocator =
  _DBUS_DATA_SLOT_ALLOCATOR_INIT (_DBUS_LOCK_NAME (message_slots));

/**
 * Allocates an integer ID to be used for storing application-specific
 * data on any DBusMessage. The allocated ID may then be used
 * with dbus_message_set_data() and dbus_message_get_data().
 * The passed-in slot must be initialized to -1, and is filled in
 * with the slot ID. If the passed-in slot is not -1, it's assumed
 * to be already allocated, and its refcount is incremented.
 *
 * The allocated slot is global, i.e. all DBusMessage objects will
 * have a slot with the given integer ID reserved.
 *
 * @param slot_p address of a global variable storing the slot
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_message_allocate_data_slot (dbus_int32_t *slot_p)
{
  return _dbus_data_slot_allocator_alloc (&slot_allocator,
                                          slot_p);
}

/**
 * Deallocates a global ID for message data slots.
 * dbus_message_get_data() and dbus_message_set_data() may no
 * longer be used with this slot.  Existing data stored on existing
 * DBusMessage objects will be freed when the message is
 * finalized, but may not be retrieved (and may only be replaced if
 * someone else reallocates the slot).  When the refcount on the
 * passed-in slot reaches 0, it is set to -1.
 *
 * @param slot_p address storing the slot to deallocate
 */
void
dbus_message_free_data_slot (dbus_int32_t *slot_p)
{
  _dbus_return_if_fail (*slot_p >= 0);

  _dbus_data_slot_allocator_free (&slot_allocator, slot_p);
}

/**
 * Stores a pointer on a DBusMessage, along
 * with an optional function to be used for freeing
 * the data when the data is set again, or when
 * the message is finalized. The slot number
 * must have been allocated with dbus_message_allocate_data_slot().
 *
 * @param message the message
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
dbus_message_set_data (DBusMessage     *message,
                       dbus_int32_t     slot,
                       void            *data,
                       DBusFreeFunction free_data_func)
{
  DBusFreeFunction old_free_func;
  void *old_data;
  dbus_bool_t retval;

  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (slot >= 0, FALSE);

  retval = _dbus_data_slot_list_set (&slot_allocator,
                                     &message->slot_list,
                                     slot, data, free_data_func,
                                     &old_free_func, &old_data);

  if (retval)
    {
      /* Do the actual free outside the message lock */
      if (old_free_func)
        (* old_free_func) (old_data);
    }

  return retval;
}

/**
 * Retrieves data previously set with dbus_message_set_data().
 * The slot must still be allocated (must not have been freed).
 *
 * @param message the message
 * @param slot the slot to get data from
 * @returns the data, or #NULL if not found
 */
void*
dbus_message_get_data (DBusMessage   *message,
                       dbus_int32_t   slot)
{
  void *res;

  _dbus_return_val_if_fail (message != NULL, NULL);

  res = _dbus_data_slot_list_get (&slot_allocator,
                                  &message->slot_list,
                                  slot);

  return res;
}

/**
 * Utility function to convert a machine-readable (not translated)
 * string into a D-Bus message type.
 *
 * @code
 *   "method_call"    -> DBUS_MESSAGE_TYPE_METHOD_CALL
 *   "method_return"  -> DBUS_MESSAGE_TYPE_METHOD_RETURN
 *   "signal"         -> DBUS_MESSAGE_TYPE_SIGNAL
 *   "error"          -> DBUS_MESSAGE_TYPE_ERROR
 *   anything else    -> DBUS_MESSAGE_TYPE_INVALID
 * @endcode
 *
 */
int
dbus_message_type_from_string (const char *type_str)
{
  if (strcmp (type_str, "method_call") == 0)
    return DBUS_MESSAGE_TYPE_METHOD_CALL;
  if (strcmp (type_str, "method_return") == 0)
    return DBUS_MESSAGE_TYPE_METHOD_RETURN;
  else if (strcmp (type_str, "signal") == 0)
    return DBUS_MESSAGE_TYPE_SIGNAL;
  else if (strcmp (type_str, "error") == 0)
    return DBUS_MESSAGE_TYPE_ERROR;
  else
    return DBUS_MESSAGE_TYPE_INVALID;
}

/**
 * Utility function to convert a D-Bus message type into a
 * machine-readable string (not translated).
 *
 * @code
 *   DBUS_MESSAGE_TYPE_METHOD_CALL    -> "method_call"
 *   DBUS_MESSAGE_TYPE_METHOD_RETURN  -> "method_return"
 *   DBUS_MESSAGE_TYPE_SIGNAL         -> "signal"
 *   DBUS_MESSAGE_TYPE_ERROR          -> "error"
 *   DBUS_MESSAGE_TYPE_INVALID        -> "invalid"
 * @endcode
 *
 */
const char *
dbus_message_type_to_string (int type)
{
  switch (type)
    {
    case DBUS_MESSAGE_TYPE_METHOD_CALL:
      return "method_call";
    case DBUS_MESSAGE_TYPE_METHOD_RETURN:
      return "method_return";
    case DBUS_MESSAGE_TYPE_SIGNAL:
      return "signal";
    case DBUS_MESSAGE_TYPE_ERROR:
      return "error";
    default:
      return "invalid";
    }
}

/**
 * Turn a DBusMessage into the marshalled form as described in the D-Bus
 * specification.
 *
 * Generally, this function is only useful for encapsulating D-Bus messages in
 * a different protocol.
 *
 * @param msg the DBusMessage
 * @param marshalled_data_p the location to save the marshalled form to
 * @param len_p the location to save the length of the marshalled form to
 * @returns #FALSE if there was not enough memory
 */
dbus_bool_t
dbus_message_marshal (DBusMessage  *msg,
                      char        **marshalled_data_p,
                      int          *len_p)
{
  DBusString tmp;
  dbus_bool_t was_locked;

  _dbus_return_val_if_fail (msg != NULL, FALSE);
  _dbus_return_val_if_fail (marshalled_data_p != NULL, FALSE);
  _dbus_return_val_if_fail (len_p != NULL, FALSE);
  
  if (!_dbus_string_init (&tmp))
    return FALSE;

  /* Ensure the message is locked, to ensure the length header is filled in. */
  was_locked = msg->locked;

  if (!was_locked)
    dbus_message_lock (msg);

  if (!_dbus_string_copy (&(msg->header.data), 0, &tmp, 0))
    goto fail;

  *len_p = _dbus_string_get_length (&tmp);

  if (!_dbus_string_copy (&(msg->body), 0, &tmp, *len_p))
    goto fail;

  *len_p = _dbus_string_get_length (&tmp);

  if (!_dbus_string_steal_data (&tmp, marshalled_data_p))
    goto fail;

  _dbus_string_free (&tmp);

  if (!was_locked)
    msg->locked = FALSE;

  return TRUE;

 fail:
  _dbus_string_free (&tmp);

  if (!was_locked)
    msg->locked = FALSE;

  return FALSE;
}

/**
 * Demarshal a D-Bus message from the format described in the D-Bus
 * specification.
 *
 * Generally, this function is only useful for encapsulating D-Bus messages in
 * a different protocol.
 *
 * @param str the marshalled DBusMessage
 * @param len the length of str
 * @param error the location to save errors to
 * @returns #NULL if there was an error
 */
DBusMessage *
dbus_message_demarshal (const char *str,
                        int         len,
                        DBusError  *error)
{
  DBusMessageLoader *loader = NULL;
  DBusString *buffer;
  DBusMessage *msg;

  _dbus_return_val_if_fail (str != NULL, NULL);

  loader = _dbus_message_loader_new ();

  if (loader == NULL)
    goto fail_oom;

  _dbus_message_loader_get_buffer (loader, &buffer, NULL, NULL);

  if (!_dbus_string_append_len (buffer, str, len))
    goto fail_oom;

  _dbus_message_loader_return_buffer (loader, buffer);

  if (!_dbus_message_loader_queue_messages (loader))
    goto fail_oom;

  if (_dbus_message_loader_get_is_corrupted (loader))
    goto fail_corrupt;

  msg = _dbus_message_loader_pop_message (loader);

  if (!msg)
    goto fail_oom;

  _dbus_message_loader_unref (loader);
  return msg;

 fail_corrupt:
  dbus_set_error (error, DBUS_ERROR_INVALID_ARGS, "Message is corrupted (%s)",
                  _dbus_validity_to_error_message (loader->corruption_reason));
  _dbus_message_loader_unref (loader);
  return NULL;

 fail_oom:
  _DBUS_SET_OOM (error);

  if (loader != NULL)
    _dbus_message_loader_unref (loader);

  return NULL;
}

/**
 * Returns the number of bytes required to be in the buffer to demarshal a
 * D-Bus message.
 *
 * Generally, this function is only useful for encapsulating D-Bus messages in
 * a different protocol.
 *
 * @param buf data to be marshalled
 * @param len the length of @p buf
 * @returns -1 if there was no valid data to be demarshalled, 0 if there wasn't enough data to determine how much should be demarshalled. Otherwise returns the number of bytes to be demarshalled
 * 
 */
int 
dbus_message_demarshal_bytes_needed(const char *buf, 
                                    int         len)
{
  DBusString str;
  int byte_order, fields_array_len, header_len, body_len;
  DBusValidity validity = DBUS_VALID;
  int have_message;

  if (!buf || len < DBUS_MINIMUM_HEADER_SIZE)
    return 0;

  if (len > DBUS_MAXIMUM_MESSAGE_LENGTH)
    len = DBUS_MAXIMUM_MESSAGE_LENGTH;
  _dbus_string_init_const_len (&str, buf, len);
  
  validity = DBUS_VALID;
  have_message
    = _dbus_header_have_message_untrusted(DBUS_MAXIMUM_MESSAGE_LENGTH,
                                          &validity, &byte_order,
                                          &fields_array_len,
                                          &header_len,
                                          &body_len,
                                          &str, 0,
                                          len);
  _dbus_string_free (&str);

  if (validity == DBUS_VALID)
    {
      _dbus_assert (have_message || (header_len + body_len) > len);
      (void) have_message; /* unused unless asserting */
      return header_len + body_len;
    }
  else
    {
      return -1; /* broken! */
    }
}

/**
 * Sets a flag indicating that the caller of the method is prepared
 * to wait for interactive authorization to take place (for instance
 * via Polkit) before the actual method is processed.
 *
 * The flag is #FALSE by default; that is, by default the other end is
 * expected to make any authorization decisions non-interactively
 * and promptly. It may use the error
 * #DBUS_ERROR_INTERACTIVE_AUTHORIZATION_REQUIRED to signal that
 * authorization failed, but could have succeeded if this flag had
 * been used.
 *
 * For messages whose type is not #DBUS_MESSAGE_TYPE_METHOD_CALL,
 * this flag is meaningless and should not be set.
 *
 * On the protocol level this toggles
 * #DBUS_HEADER_FLAG_ALLOW_INTERACTIVE_AUTHORIZATION.
 *
 * @param message the message
 * @param allow #TRUE if interactive authorization is acceptable
 */
void
dbus_message_set_allow_interactive_authorization (DBusMessage *message,
                                                  dbus_bool_t  allow)
{
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (!message->locked);

  _dbus_header_toggle_flag (&message->header,
                            DBUS_HEADER_FLAG_ALLOW_INTERACTIVE_AUTHORIZATION,
                            allow);
}

/**
 * Returns whether the flag controlled by
 * dbus_message_set_allow_interactive_authorization() has been set.
 *
 * @param message the message
 */
dbus_bool_t
dbus_message_get_allow_interactive_authorization (DBusMessage *message)
{
  _dbus_return_val_if_fail (message != NULL, FALSE);

  return _dbus_header_get_flag (&message->header,
                                DBUS_HEADER_FLAG_ALLOW_INTERACTIVE_AUTHORIZATION);
}

/**
 * An opaque data structure containing the serialized form of any single
 * D-Bus message item, whose signature is a single complete type.
 *
 * (Implementation detail: It's serialized as a single variant.)
 */
struct DBusVariant
{
  DBusString data;
};

/**
 * Copy a single D-Bus message item from reader into a
 * newly-allocated #DBusVariant.
 *
 * For example, if a message contains three string arguments, and reader points
 * to the second string, the resulting DBusVariant will have signature
 * #DBUS_TYPE_STRING_AS_STRING and contain only that second string.
 *
 * @param reader An iterator over message items, pointing to one item to copy
 * @returns The variant, or #NULL if out of memory
 */
DBusVariant *
_dbus_variant_read (DBusMessageIter *reader)
{
  DBusVariant *self = NULL;
  /* Points to the single item we will read from the reader */
  DBusMessageRealIter *real_reader = (DBusMessageRealIter *) reader;
  /* The position in self at which we will write a single variant
   * (it is position 0) */
  DBusTypeWriter items_writer;
  /* The position in self at which we will write a copy of reader
   * (it is inside the variant) */
  DBusTypeWriter variant_writer;
  /* 'v' */
  DBusString variant_signature;
  /* Whatever is the signature of the item we will copy from the reader */
  DBusString contained_signature;
  /* TRUE if self->data needs to be freed */
  dbus_bool_t data_inited = FALSE;
  /* The type of the item we will read from the reader */
  int type;
  /* The string, start position within that string, and length of the signature
   * of the single complete type of the item reader points to */
  const DBusString *sig;
  int start, len;

  _dbus_assert (_dbus_message_iter_check (real_reader));
  _dbus_assert (real_reader->iter_type == DBUS_MESSAGE_ITER_TYPE_READER);
  _dbus_string_init_const (&variant_signature, DBUS_TYPE_VARIANT_AS_STRING);
  type = dbus_message_iter_get_arg_type (reader);
  _dbus_type_reader_get_signature (&real_reader->u.reader, &sig, &start, &len);

  if (!_dbus_string_init (&contained_signature))
    return NULL;

  if (!_dbus_string_copy_len (sig, start, len, &contained_signature, 0))
    goto oom;

  self = dbus_new0 (DBusVariant, 1);

  if (self == NULL)
    goto oom;

  if (!_dbus_string_init (&self->data))
    goto oom;

  data_inited = TRUE;

  _dbus_type_writer_init_values_only (&items_writer, DBUS_COMPILER_BYTE_ORDER,
                                      &variant_signature, 0, &self->data, 0);

  if (!_dbus_type_writer_recurse (&items_writer, DBUS_TYPE_VARIANT,
                                  &contained_signature, 0, &variant_writer))
    goto oom;

  if (type == DBUS_TYPE_ARRAY)
    {
      /* Points to each item in turn inside the array we are copying */
      DBusMessageIter array_reader;
      /* Same as array_reader */
      DBusMessageRealIter *real_array_reader = (DBusMessageRealIter *) &array_reader;
      /* The position inside the copied array at which we will write
       * the copy of array_reader */
      DBusTypeWriter array_writer;

      dbus_message_iter_recurse (reader, &array_reader);

      if (!_dbus_type_writer_recurse (&variant_writer, type,
                                      &contained_signature, 1, &array_writer))
        goto oom;

      if (!_dbus_type_writer_write_reader (&array_writer,
                                           &real_array_reader->u.reader))
        goto oom;

      if (!_dbus_type_writer_unrecurse (&variant_writer, &array_writer))
        goto oom;
    }
  else if (type == DBUS_TYPE_DICT_ENTRY || type == DBUS_TYPE_VARIANT ||
           type == DBUS_TYPE_STRUCT)
    {
      /* Points to each item in turn inside the container we are copying */
      DBusMessageIter inner_reader;
      /* Same as inner_reader */
      DBusMessageRealIter *real_inner_reader = (DBusMessageRealIter *) &inner_reader;
      /* The position inside the copied container at which we will write the
       * copy of inner_reader */
      DBusTypeWriter inner_writer;

      dbus_message_iter_recurse (reader, &inner_reader);

      if (!_dbus_type_writer_recurse (&variant_writer, type, NULL, 0,
                                      &inner_writer))
        goto oom;

      if (!_dbus_type_writer_write_reader (&inner_writer,
                                           &real_inner_reader->u.reader))
        goto oom;

      if (!_dbus_type_writer_unrecurse (&variant_writer, &inner_writer))
        goto oom;
    }
  else
    {
      DBusBasicValue value;

      /* We eliminated all the container types above */
      _dbus_assert (dbus_type_is_basic (type));

      dbus_message_iter_get_basic (reader, &value);

      if (!_dbus_type_writer_write_basic (&variant_writer, type, &value))
        goto oom;
    }

  _dbus_string_free (&contained_signature);
  return self;

oom:
  if (self != NULL)
    {
      if (data_inited)
        _dbus_string_free (&self->data);

      dbus_free (self);
    }

  _dbus_string_free (&contained_signature);
  return NULL;
}

/**
 * Return the signature of the item stored in self. It is a single complete
 * type.
 *
 * @param self the variant
 */
const char *
_dbus_variant_get_signature (DBusVariant *self)
{
  unsigned char len;
  const char *ret;

  _dbus_assert (self != NULL);

  /* Here we make use of the fact that the serialization of a variant starts
   * with the 1-byte length, then that many bytes of signature, then \0. */
  len = _dbus_string_get_byte (&self->data, 0);
  ret = _dbus_string_get_const_data_len (&self->data, 1, len);
  _dbus_assert (strlen (ret) == len);
  return ret;
}

/**
 * Copy the single D-Bus message item from self into writer.
 *
 * For example, if writer points into the body of an empty message and self has
 * signature #DBUS_TYPE_STRING_AS_STRING, then the message will
 * have signature #DBUS_TYPE_STRING_AS_STRING after this function returns
 *
 * @param self the variant
 * @param writer the place to write the contents of the variant
 * @returns #TRUE on success, #FALSE if out of memory
 */
dbus_bool_t
_dbus_variant_write (DBusVariant *self,
                     DBusMessageIter *writer)
{
  /* 'v' */
  DBusString variant_signature;
  /* Points to the single item in self */
  DBusTypeReader variant_reader;
  /* Points to the single item (of whatever type) inside the variant */
  DBusTypeReader reader;
  /* The position at which we will copy reader */
  DBusMessageRealIter *real_writer = (DBusMessageRealIter *) writer;
  dbus_bool_t ret;

  _dbus_assert (self != NULL);
  _dbus_assert (_dbus_message_iter_append_check (real_writer));
  _dbus_assert (real_writer->iter_type == DBUS_MESSAGE_ITER_TYPE_WRITER);

  _dbus_string_init_const (&variant_signature, DBUS_TYPE_VARIANT_AS_STRING);
  _dbus_type_reader_init (&reader, DBUS_COMPILER_BYTE_ORDER,
                          &variant_signature, 0, &self->data, 0);
  _dbus_type_reader_recurse (&reader, &variant_reader);

  if (!_dbus_message_iter_open_signature (real_writer))
    return FALSE;

  ret = _dbus_type_writer_write_reader (&real_writer->u.writer,
                                        &variant_reader);

  if (!_dbus_message_iter_close_signature (real_writer))
    return FALSE;

  return ret;
}

int
_dbus_variant_get_length (DBusVariant *self)
{
  _dbus_assert (self != NULL);
  return _dbus_string_get_length (&self->data);
}

const DBusString *
_dbus_variant_peek (DBusVariant *self)
{
  _dbus_assert (self != NULL);
  return &self->data;
}

void
_dbus_variant_free (DBusVariant *self)
{
  _dbus_assert (self != NULL);
  _dbus_string_free (&self->data);
  dbus_free (self);
}

/** @} */

/* tests in dbus-message-util.c */
