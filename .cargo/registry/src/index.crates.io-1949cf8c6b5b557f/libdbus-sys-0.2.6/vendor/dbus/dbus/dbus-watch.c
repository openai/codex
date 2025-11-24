/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-watch.c DBusWatch implementation
 *
 * Copyright (C) 2002, 2003  Red Hat Inc.
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
#include "dbus-watch.h"
#include "dbus-list.h"

/**
 * @defgroup DBusWatchInternals DBusWatch implementation details
 * @ingroup  DBusInternals
 * @brief implementation details for DBusWatch
 * 
 * @{
 */

/**
 * Implementation of DBusWatch
 */
struct DBusWatch
{
  int refcount;                        /**< Reference count */
  DBusPollable fd;                     /**< File descriptor. */
  unsigned int flags;                  /**< Conditions to watch. */

  DBusWatchHandler handler;                    /**< Watch handler. */
  void *handler_data;                          /**< Watch handler data. */
  DBusFreeFunction free_handler_data_function; /**< Free the watch handler data. */
  
  void *data;                          /**< Application data. */
  DBusFreeFunction free_data_function; /**< Free the application data. */
  unsigned int enabled : 1;            /**< Whether it's enabled. */
  unsigned int oom_last_time : 1;      /**< Whether it was OOM last time. */
};

dbus_bool_t
_dbus_watch_get_enabled (DBusWatch *watch)
{
  return watch->enabled;
}

dbus_bool_t
_dbus_watch_get_oom_last_time (DBusWatch *watch)
{
  return watch->oom_last_time;
}

void
_dbus_watch_set_oom_last_time (DBusWatch   *watch,
                               dbus_bool_t  oom)
{
  watch->oom_last_time = oom;
}

/**
 * Creates a new DBusWatch. Used to add a file descriptor to be polled
 * by a main loop.
 * 
 * @param fd the file descriptor to be watched.
 * @param flags the conditions to watch for on the descriptor.
 * @param enabled the initial enabled state
 * @param handler the handler function
 * @param data data for handler function
 * @param free_data_function function to free the data
 * @returns the new DBusWatch object.
 */
DBusWatch*
_dbus_watch_new (DBusPollable      fd,
                 unsigned int      flags,
                 dbus_bool_t       enabled,
                 DBusWatchHandler  handler,
                 void             *data,
                 DBusFreeFunction  free_data_function)
{
  DBusWatch *watch;

#define VALID_WATCH_FLAGS (DBUS_WATCH_WRITABLE | DBUS_WATCH_READABLE)
  
  _dbus_assert ((flags & VALID_WATCH_FLAGS) == flags);
  
  watch = dbus_new0 (DBusWatch, 1);
  if (watch == NULL)
    return NULL;
  
  watch->refcount = 1;
  watch->fd = fd;
  watch->flags = flags;
  watch->enabled = enabled;

  watch->handler = handler;
  watch->handler_data = data;
  watch->free_handler_data_function = free_data_function;
  
  return watch;
}

/**
 * Increments the reference count of a DBusWatch object.
 *
 * @param watch the watch object.
 * @returns the watch object.
 */
DBusWatch *
_dbus_watch_ref (DBusWatch *watch)
{
  watch->refcount += 1;

  return watch;
}

/**
 * Decrements the reference count of a DBusWatch object
 * and finalizes the object if the count reaches zero.
 *
 * @param watch the watch object.
 */
void
_dbus_watch_unref (DBusWatch *watch)
{
  _dbus_assert (watch != NULL);
  _dbus_assert (watch->refcount > 0);

  watch->refcount -= 1;
  if (watch->refcount == 0)
    {
      if (_dbus_pollable_is_valid (watch->fd))
        _dbus_warn ("this watch should have been invalidated");

      dbus_watch_set_data (watch, NULL, NULL); /* call free_data_function */

      if (watch->free_handler_data_function)
	(* watch->free_handler_data_function) (watch->handler_data);
      
      dbus_free (watch);
    }
}

/**
 * Clears the file descriptor from a now-invalid watch object so that
 * no one tries to use it.  This is because a watch may stay alive due
 * to reference counts after the file descriptor is closed.
 * Invalidation makes it easier to catch bugs. It also
 * keeps people from doing dorky things like assuming file descriptors
 * are unique (never recycled).
 *
 * @param watch the watch object.
 */
void
_dbus_watch_invalidate (DBusWatch *watch)
{
  _dbus_pollable_invalidate (&watch->fd);
  watch->flags = 0;
}

/**
 * Sanitizes the given condition so that it only contains
 * flags that the DBusWatch requested. e.g. if the
 * watch is a DBUS_WATCH_READABLE watch then
 * DBUS_WATCH_WRITABLE will be stripped from the condition.
 *
 * @param watch the watch object.
 * @param condition address of the condition to sanitize.
 */
void
_dbus_watch_sanitize_condition (DBusWatch    *watch,
                                unsigned int *condition)
{
  if (!(watch->flags & DBUS_WATCH_READABLE))
    *condition &= ~DBUS_WATCH_READABLE;
  if (!(watch->flags & DBUS_WATCH_WRITABLE))
    *condition &= ~DBUS_WATCH_WRITABLE;
}


/**
 * @typedef DBusWatchList
 *
 * Opaque data type representing a list of watches
 * and a set of DBusAddWatchFunction/DBusRemoveWatchFunction.
 * Automatically handles removing/re-adding watches
 * when the DBusAddWatchFunction is updated or changed.
 * Holds a reference count to each watch.
 *
 * Used in the implementation of both DBusServer and
 * DBusClient.
 *
 */

/**
 * DBusWatchList implementation details. All fields
 * are private.
 *
 */
struct DBusWatchList
{
  DBusList *watches;           /**< Watch objects. */

  DBusAddWatchFunction add_watch_function;    /**< Callback for adding a watch. */
  DBusRemoveWatchFunction remove_watch_function; /**< Callback for removing a watch. */
  DBusWatchToggledFunction watch_toggled_function; /**< Callback on toggling enablement */
  void *watch_data;                           /**< Data for watch callbacks */
  DBusFreeFunction watch_free_data_function;  /**< Free function for watch callback data */
};

/**
 * Creates a new watch list. Returns #NULL if insufficient
 * memory exists.
 *
 * @returns the new watch list, or #NULL on failure.
 */
DBusWatchList*
_dbus_watch_list_new (void)
{
  DBusWatchList *watch_list;

  watch_list = dbus_new0 (DBusWatchList, 1);
  if (watch_list == NULL)
    return NULL;

  return watch_list;
}

/**
 * Frees a DBusWatchList.
 *
 * @param watch_list the watch list.
 */
void
_dbus_watch_list_free (DBusWatchList *watch_list)
{
  /* free watch_data and removes watches as a side effect */
  _dbus_watch_list_set_functions (watch_list,
                                  NULL, NULL, NULL, NULL, NULL);

  _dbus_list_clear_full (&watch_list->watches,
                         (DBusFreeFunction) _dbus_watch_unref);

  dbus_free (watch_list);
}

#ifdef DBUS_ENABLE_VERBOSE_MODE
static const char*
watch_flags_to_string (int flags)
{
  const char *watch_type;

  if ((flags & DBUS_WATCH_READABLE) &&
      (flags & DBUS_WATCH_WRITABLE))
    watch_type = "readwrite";
  else if (flags & DBUS_WATCH_READABLE)
    watch_type = "read";
  else if (flags & DBUS_WATCH_WRITABLE)
    watch_type = "write";
  else
    watch_type = "not read or write";
  return watch_type;
}
#endif /* DBUS_ENABLE_VERBOSE_MODE */

/**
 * Sets the watch functions. This function is the "backend"
 * for dbus_connection_set_watch_functions() and
 * dbus_server_set_watch_functions().
 *
 * @param watch_list the watch list.
 * @param add_function the add watch function.
 * @param remove_function the remove watch function.
 * @param toggled_function function on toggling enabled flag, or #NULL
 * @param data the data for those functions.
 * @param free_data_function the function to free the data.
 * @returns #FALSE if not enough memory
 *
 */
dbus_bool_t
_dbus_watch_list_set_functions (DBusWatchList           *watch_list,
                                DBusAddWatchFunction     add_function,
                                DBusRemoveWatchFunction  remove_function,
                                DBusWatchToggledFunction toggled_function,
                                void                    *data,
                                DBusFreeFunction         free_data_function)
{
  /* Add watches with the new watch function, failing on OOM */
  if (add_function != NULL)
    {
      DBusList *link;
      
      link = _dbus_list_get_first_link (&watch_list->watches);
      while (link != NULL)
        {
          DBusList *next = _dbus_list_get_next_link (&watch_list->watches,
                                                     link);
#ifdef DBUS_ENABLE_VERBOSE_MODE
          DBusWatch *watch = link->data;

          _dbus_verbose ("Adding a %s watch on fd %" DBUS_POLLABLE_FORMAT " using newly-set add watch function\n",
                         watch_flags_to_string (dbus_watch_get_flags (link->data)),
                         _dbus_pollable_printable (watch->fd));
#endif
          
          if (!(* add_function) (link->data, data))
            {
              /* remove it all again and return FALSE */
              DBusList *link2;
              
              link2 = _dbus_list_get_first_link (&watch_list->watches);
              while (link2 != link)
                {
                  DBusList *next2 = _dbus_list_get_next_link (&watch_list->watches,
                                                              link2);
#ifdef DBUS_ENABLE_VERBOSE_MODE
                  DBusWatch *watch2 = link2->data;
                  
                  _dbus_verbose ("Removing watch on fd %" DBUS_POLLABLE_FORMAT " using newly-set remove function because initial add failed\n",
                                 _dbus_pollable_printable (watch2->fd));
#endif
                  
                  (* remove_function) (link2->data, data);
                  
                  link2 = next2;
                }

              return FALSE;
            }
      
          link = next;
        }
    }
  
  /* Remove all current watches from previous watch handlers */

  if (watch_list->remove_watch_function != NULL)
    {
      _dbus_verbose ("Removing all pre-existing watches\n");
      
      _dbus_list_foreach (&watch_list->watches,
                          (DBusForeachFunction) watch_list->remove_watch_function,
                          watch_list->watch_data);
    }

  if (watch_list->watch_free_data_function != NULL)
    (* watch_list->watch_free_data_function) (watch_list->watch_data);
  
  watch_list->add_watch_function = add_function;
  watch_list->remove_watch_function = remove_function;
  watch_list->watch_toggled_function = toggled_function;
  watch_list->watch_data = data;
  watch_list->watch_free_data_function = free_data_function;

  return TRUE;
}

/**
 * Adds a new watch to the watch list, invoking the
 * application DBusAddWatchFunction if appropriate.
 *
 * @param watch_list the watch list.
 * @param watch the watch to add.
 * @returns #TRUE on success, #FALSE if no memory.
 */
dbus_bool_t
_dbus_watch_list_add_watch (DBusWatchList *watch_list,
                            DBusWatch     *watch)
{
  if (!_dbus_list_append (&watch_list->watches, watch))
    return FALSE;
  
  _dbus_watch_ref (watch);

  if (watch_list->add_watch_function != NULL)
    {
      _dbus_verbose ("Adding watch on fd %" DBUS_POLLABLE_FORMAT "\n",
                     _dbus_pollable_printable (watch->fd));
      
      if (!(* watch_list->add_watch_function) (watch,
                                               watch_list->watch_data))
        {
          _dbus_list_remove_last (&watch_list->watches, watch);
          _dbus_watch_unref (watch);
          return FALSE;
        }
    }
  
  return TRUE;
}

/**
 * Removes a watch from the watch list, invoking the
 * application's DBusRemoveWatchFunction if appropriate.
 *
 * @param watch_list the watch list.
 * @param watch the watch to remove.
 */
void
_dbus_watch_list_remove_watch  (DBusWatchList *watch_list,
                                DBusWatch     *watch)
{
  if (!_dbus_list_remove (&watch_list->watches, watch))
    _dbus_assert_not_reached ("Nonexistent watch was removed");
  
  if (watch_list->remove_watch_function != NULL)
    {
      _dbus_verbose ("Removing watch on fd %" DBUS_POLLABLE_FORMAT "\n",
                     _dbus_pollable_printable (watch->fd));
      
      (* watch_list->remove_watch_function) (watch,
                                             watch_list->watch_data);
    }
  
  _dbus_watch_unref (watch);
}

/**
 * Sets a watch to the given enabled state, invoking the
 * application's DBusWatchToggledFunction if appropriate.
 *
 * @param watch_list the watch list.
 * @param watch the watch to toggle.
 * @param enabled #TRUE to enable
 */
void
_dbus_watch_list_toggle_watch (DBusWatchList           *watch_list,
                               DBusWatch               *watch,
                               dbus_bool_t              enabled)
{
  enabled = !!enabled;
  
  if (enabled == watch->enabled)
    return;

  watch->enabled = enabled;
  
  if (watch_list->watch_toggled_function != NULL)
    {
      _dbus_verbose ("Toggling watch %p on fd %" DBUS_POLLABLE_FORMAT " to %d\n",
                     watch,
                     _dbus_pollable_printable (watch->fd),
                     watch->enabled);
      
      (* watch_list->watch_toggled_function) (watch,
                                              watch_list->watch_data);
    }
}

/**
 * Sets all watches to the given enabled state, invoking the
 * application's DBusWatchToggledFunction if appropriate.
 *
 * @param watch_list the watch list.
 * @param enabled #TRUE to enable
 */
void
_dbus_watch_list_toggle_all_watches (DBusWatchList           *watch_list,
                                     dbus_bool_t              enabled)
{
  DBusList *link;

  for (link = _dbus_list_get_first_link (&watch_list->watches);
       link != NULL;
       link = _dbus_list_get_next_link (&watch_list->watches, link))
    {
      _dbus_watch_list_toggle_watch (watch_list, link->data, enabled);
    }
}

/**
 * Sets the handler for the watch.
 *
 * @todo this function only exists because of the weird
 * way connection watches are done, see the note
 * in docs for _dbus_connection_handle_watch().
 *
 * @param watch the watch
 * @param handler the new handler
 * @param data the data
 * @param free_data_function free data with this
 */
void
_dbus_watch_set_handler (DBusWatch        *watch,
                         DBusWatchHandler  handler,
                         void             *data,
                         DBusFreeFunction  free_data_function)
{
  if (watch->free_handler_data_function)
    (* watch->free_handler_data_function) (watch->handler_data);

  watch->handler = handler;
  watch->handler_data = data;
  watch->free_handler_data_function = free_data_function;
}

/** @} */

/**
 * @defgroup DBusWatch DBusWatch
 * @ingroup  DBus
 * @brief Object representing a file descriptor to be watched.
 *
 * Types and functions related to DBusWatch. A watch represents
 * a file descriptor that the main loop needs to monitor,
 * as in Qt's QSocketNotifier or GLib's g_io_add_watch().
 *
 * Use dbus_connection_set_watch_functions() or dbus_server_set_watch_functions()
 * to be notified when libdbus needs to add or remove watches.
 * 
 * @{
 */

/**
 * @typedef DBusWatch
 *
 * Opaque object representing a file descriptor
 * to be watched for changes in readability,
 * writability, or hangup.
 */

/**
 * Deprecated former name of dbus_watch_get_unix_fd().
 * 
 * @param watch the DBusWatch object.
 * @returns the file descriptor to watch.
 */
int
dbus_watch_get_fd (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, -1);

  return dbus_watch_get_unix_fd(watch);
}

/**
 * Returns a UNIX file descriptor to be watched,
 * which may be a pipe, socket, or other type of
 * descriptor. On UNIX this is preferred to
 * dbus_watch_get_socket() since it works with
 * more kinds of #DBusWatch.
 *
 * Always returns -1 on Windows. On Windows you use
 * dbus_watch_get_socket() to get a Winsock socket to watch.
 * 
 * @param watch the DBusWatch object.
 * @returns the file descriptor to watch.
 */
int
dbus_watch_get_unix_fd (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, -1);

  /* FIXME remove #ifdef and do this on a lower level
   * (watch should have set_socket and set_unix_fd and track
   * which it has, and the transport should provide the
   * appropriate watch type)
   */
#ifdef DBUS_UNIX
  return watch->fd;
#else
  return dbus_watch_get_socket( watch );
#endif
}

/**
 * Returns a socket to be watched, on UNIX this will return -1 if our
 * transport is not socket-based so dbus_watch_get_unix_fd() is
 * preferred.
 *
 * On Windows, dbus_watch_get_unix_fd() returns -1 but this function
 * returns a Winsock socket (assuming the transport is socket-based,
 * as it always is for now).
 * 
 * @param watch the DBusWatch object.
 * @returns the socket to watch.
 */
int
dbus_watch_get_socket (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, -1);

#ifdef DBUS_UNIX
  return watch->fd;
#else
  return _dbus_socket_get_int (watch->fd);
#endif
}

DBusSocket
_dbus_watch_get_socket (DBusWatch *watch)
{
  DBusSocket s;

  _dbus_assert (watch != NULL);

#ifdef DBUS_UNIX
  s.fd = watch->fd;
#else
  s = watch->fd;
#endif

  return s;
}

DBusPollable
_dbus_watch_get_pollable (DBusWatch *watch)
{
  _dbus_assert (watch != NULL);

  return watch->fd;
}

/**
 * Gets flags from DBusWatchFlags indicating
 * what conditions should be monitored on the
 * file descriptor.
 * 
 * The flags returned will only contain DBUS_WATCH_READABLE
 * and DBUS_WATCH_WRITABLE, never DBUS_WATCH_HANGUP or
 * DBUS_WATCH_ERROR; all watches implicitly include a watch
 * for hangups, errors, and other exceptional conditions.
 *
 * @param watch the DBusWatch object.
 * @returns the conditions to watch.
 */
unsigned int
dbus_watch_get_flags (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, 0);
  _dbus_assert ((watch->flags & VALID_WATCH_FLAGS) == watch->flags);

  return watch->flags;
}

/**
 * Gets data previously set with dbus_watch_set_data()
 * or #NULL if none.
 *
 * @param watch the DBusWatch object.
 * @returns previously-set data.
 */
void*
dbus_watch_get_data (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, NULL);

  return watch->data;
}

/**
 * Sets data which can be retrieved with dbus_watch_get_data().
 * Intended for use by the DBusAddWatchFunction and
 * DBusRemoveWatchFunction to store their own data.  For example with
 * Qt you might store the QSocketNotifier for this watch and with GLib
 * you might store a GSource.
 *
 * @param watch the DBusWatch object.
 * @param data the data.
 * @param free_data_function function to be called to free the data.
 */
void
dbus_watch_set_data (DBusWatch        *watch,
                     void             *data,
                     DBusFreeFunction  free_data_function)
{
  _dbus_return_if_fail (watch != NULL);

  _dbus_verbose ("Setting watch fd %" DBUS_POLLABLE_FORMAT " data to data = %p function = %p from data = %p function = %p\n",
                 _dbus_pollable_printable (watch->fd),
                 data, free_data_function, watch->data, watch->free_data_function);
  
  if (watch->free_data_function != NULL)
    (* watch->free_data_function) (watch->data);
  
  watch->data = data;
  watch->free_data_function = free_data_function;
}

/**
 * Returns whether a watch is enabled or not. If not
 * enabled, it should not be polled by the main loop.
 *
 * @param watch the DBusWatch object
 * @returns #TRUE if the watch is enabled
 */
dbus_bool_t
dbus_watch_get_enabled (DBusWatch *watch)
{
  _dbus_return_val_if_fail (watch != NULL, FALSE);

  return watch->enabled;
}


/**
 * Called to notify the D-Bus library when a previously-added watch is
 * ready for reading or writing, or has an exception such as a hangup.
 * 
 * If this function returns #FALSE, then the file descriptor may still
 * be ready for reading or writing, but more memory is needed in order
 * to do the reading or writing. If you ignore the #FALSE return, your
 * application may spin in a busy loop on the file descriptor until
 * memory becomes available, but nothing more catastrophic should
 * happen.
 *
 * dbus_watch_handle() cannot be called during the
 * DBusAddWatchFunction, as the connection will not be ready to handle
 * that watch yet.
 * 
 * It is not allowed to reference a DBusWatch after it has been passed
 * to remove_function.
 *
 * @param watch the DBusWatch object.
 * @param flags the poll condition using #DBusWatchFlags values
 * @returns #FALSE if there wasn't enough memory 
 */
dbus_bool_t
dbus_watch_handle (DBusWatch    *watch,
                   unsigned int  flags)
{
  _dbus_return_val_if_fail (watch != NULL, FALSE);

#ifndef DBUS_DISABLE_CHECKS
  if (!_dbus_pollable_is_valid (watch->fd) || watch->flags == 0)
    {
      _dbus_warn_check_failed ("Watch is invalid, it should have been removed");
      return TRUE;
    }
#endif
    
  _dbus_return_val_if_fail (_dbus_pollable_is_valid (watch->fd) /* fails if watch was removed */, TRUE);
  
  _dbus_watch_sanitize_condition (watch, &flags);

  if (flags == 0)
    {
      _dbus_verbose ("After sanitization, watch flags on fd %" DBUS_POLLABLE_FORMAT " were 0\n",
                     _dbus_pollable_printable (watch->fd));
      return TRUE;
    }
  else
    return (* watch->handler) (watch, flags,
                               watch->handler_data);
}


/** @} */
