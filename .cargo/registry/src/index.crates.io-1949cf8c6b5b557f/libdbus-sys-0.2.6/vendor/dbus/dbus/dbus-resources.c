/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-resources.c Resource tracking/limits
 *
 * Copyright (C) 2003  Red Hat Inc.
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
#include <dbus/dbus-resources.h>
#include <dbus/dbus-internals.h>

/**
 * @defgroup DBusResources Resource limits related code
 * @ingroup  DBusInternals
 * @brief DBusCounter and other stuff related to resource limits
 *
 * Types and functions related to tracking resource limits,
 * such as the maximum amount of memory/unix fds a connection can use
 * for messages, etc.
 */

/**
 * @defgroup DBusResourcesInternals Resource limits implementation details
 * @ingroup  DBusInternals
 * @brief Resource limits implementation details
 *
 * Implementation details of resource limits code.
 *
 * @{
 */

/**
 * @brief Internals of DBusCounter.
 * 
 * DBusCounter internals. DBusCounter is an opaque object, it must be
 * used via accessor functions.
 */
struct DBusCounter
{
  int refcount;  /**< reference count */

  long size_value;       /**< current size counter value */
  long unix_fd_value;    /**< current unix fd counter value */

#ifdef DBUS_ENABLE_STATS
  long peak_size_value;     /**< largest ever size counter value */
  long peak_unix_fd_value;  /**< largest ever unix fd counter value */
#endif

  long notify_size_guard_value;    /**< call notify function when crossing this size value */
  long notify_unix_fd_guard_value; /**< call notify function when crossing this unix fd value */

  DBusCounterNotifyFunction notify_function; /**< notify function */
  void *notify_data; /**< data for notify function */
  dbus_bool_t notify_pending : 1; /**< TRUE if the guard value has been crossed */
  DBusRMutex *mutex;    /**< Lock on the entire DBusCounter */
};

/** @} */  /* end of resource limits internals docs */

/**
 * @addtogroup DBusResources
 * @{
 */

/**
 * Creates a new DBusCounter. DBusCounter is used
 * to count usage of some resource such as memory.
 *
 * @returns new counter or #NULL on failure
 */
DBusCounter*
_dbus_counter_new (void)
{
  DBusCounter *counter;

  counter = dbus_new0 (DBusCounter, 1);
  if (counter == NULL)
    return NULL;

  counter->refcount = 1;

  _dbus_rmutex_new_at_location (&counter->mutex);
  if (counter->mutex == NULL)
  {
    dbus_free (counter);
    counter = NULL;
  }

  return counter;
}

/**
 * Increments refcount of the counter
 *
 * @param counter the counter
 * @returns the counter
 */
DBusCounter *
_dbus_counter_ref (DBusCounter *counter)
{
  _dbus_rmutex_lock (counter->mutex);

  _dbus_assert (counter->refcount > 0);
  
  counter->refcount += 1;

  _dbus_rmutex_unlock (counter->mutex);

  return counter;
}

/**
 * Decrements refcount of the counter and possibly
 * finalizes the counter.
 *
 * @param counter the counter
 */
void
_dbus_counter_unref (DBusCounter *counter)
{
  dbus_bool_t last_ref = FALSE;

  _dbus_rmutex_lock (counter->mutex);

  _dbus_assert (counter->refcount > 0);

  counter->refcount -= 1;
  last_ref = (counter->refcount == 0);

  _dbus_rmutex_unlock (counter->mutex);

  if (last_ref)
    {
      _dbus_rmutex_free_at_location (&counter->mutex);
      dbus_free (counter);
    }
}

/**
 * Adjusts the value of the size counter by the given
 * delta which may be positive or negative.
 *
 * This function may be called with locks held. After calling it, when
 * any relevant locks are no longer held you must call _dbus_counter_notify().
 *
 * @param counter the counter
 * @param delta value to add to the size counter's current value
 */
void
_dbus_counter_adjust_size (DBusCounter *counter,
                           long         delta)
{
  long old = 0;

  _dbus_rmutex_lock (counter->mutex);

  old = counter->size_value;

  counter->size_value += delta;

#ifdef DBUS_ENABLE_STATS
  if (counter->peak_size_value < counter->size_value)
    counter->peak_size_value = counter->size_value;
#endif

#if 0
  _dbus_verbose ("Adjusting counter %ld by %ld = %ld\n",
                 old, delta, counter->size_value);
#endif

  if (counter->notify_function != NULL &&
      ((old < counter->notify_size_guard_value &&
        counter->size_value >= counter->notify_size_guard_value) ||
       (old >= counter->notify_size_guard_value &&
        counter->size_value < counter->notify_size_guard_value)))
    counter->notify_pending = TRUE;

  _dbus_rmutex_unlock (counter->mutex);
}

/**
 * Calls the notify function from _dbus_counter_set_notify(),
 * if that function has been specified and the counter has crossed the
 * guard value (in either direction) since the last call to this function.
 *
 * This function must not be called with locks held, since it can call out
 * to user code.
 */
void
_dbus_counter_notify (DBusCounter *counter)
{
  DBusCounterNotifyFunction notify_function = NULL;
  void *notify_data = NULL;

  _dbus_rmutex_lock (counter->mutex);
  if (counter->notify_pending)
    {
      counter->notify_pending = FALSE;
      notify_function = counter->notify_function;
      notify_data = counter->notify_data;
    }
  _dbus_rmutex_unlock (counter->mutex);

  if (notify_function != NULL)
    (* notify_function) (counter, notify_data);
}

/**
 * Adjusts the value of the unix fd counter by the given
 * delta which may be positive or negative.
 *
 * This function may be called with locks held. After calling it, when
 * any relevant locks are no longer held you must call _dbus_counter_notify().
 *
 * @param counter the counter
 * @param delta value to add to the unix fds counter's current value
 */
void
_dbus_counter_adjust_unix_fd (DBusCounter *counter,
                              long         delta)
{
  long old = 0;

  _dbus_rmutex_lock (counter->mutex);

  old = counter->unix_fd_value;
  
  counter->unix_fd_value += delta;

#ifdef DBUS_ENABLE_STATS
  if (counter->peak_unix_fd_value < counter->unix_fd_value)
    counter->peak_unix_fd_value = counter->unix_fd_value;
#endif

#if 0
  _dbus_verbose ("Adjusting counter %ld by %ld = %ld\n",
                 old, delta, counter->unix_fd_value);
#endif
  
  if (counter->notify_function != NULL &&
      ((old < counter->notify_unix_fd_guard_value &&
        counter->unix_fd_value >= counter->notify_unix_fd_guard_value) ||
       (old >= counter->notify_unix_fd_guard_value &&
        counter->unix_fd_value < counter->notify_unix_fd_guard_value)))
    counter->notify_pending = TRUE;

  _dbus_rmutex_unlock (counter->mutex);
}

/**
 * Gets the current value of the size counter.
 *
 * @param counter the counter
 * @returns its current size value
 */
long
_dbus_counter_get_size_value (DBusCounter *counter)
{
  return counter->size_value;
}

/**
 * Gets the current value of the unix fd counter.
 *
 * @param counter the counter
 * @returns its current unix fd value
 */
long
_dbus_counter_get_unix_fd_value (DBusCounter *counter)
{
  return counter->unix_fd_value;
}

/**
 * Sets the notify function for this counter; the notify function is
 * called whenever the counter's values cross the guard values in
 * either direction (moving up, or moving down).
 *
 * @param counter the counter
 * @param size_guard_value the value we're notified if the size counter crosses
 * @param unix_fd_guard_value the value we're notified if the unix fd counter crosses
 * @param function function to call in order to notify
 * @param user_data data to pass to the function
 */
void
_dbus_counter_set_notify (DBusCounter               *counter,
                          long                       size_guard_value,
                          long                       unix_fd_guard_value,
                          DBusCounterNotifyFunction  function,
                          void                      *user_data)
{
  _dbus_rmutex_lock (counter->mutex);
  counter->notify_size_guard_value = size_guard_value;
  counter->notify_unix_fd_guard_value = unix_fd_guard_value;
  counter->notify_function = function;
  counter->notify_data = user_data;
  counter->notify_pending = FALSE;
  _dbus_rmutex_unlock (counter->mutex);
}

#ifdef DBUS_ENABLE_STATS
long
_dbus_counter_get_peak_size_value (DBusCounter *counter)
{
  return counter->peak_size_value;
}

long
_dbus_counter_get_peak_unix_fd_value (DBusCounter *counter)
{
  return counter->peak_unix_fd_value;
}
#endif

/** @} */  /* end of resource limits exported API */
