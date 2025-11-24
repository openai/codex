/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-timeout.c DBusTimeout implementation
 *
 * Copyright (C) 2003  CodeFactory AB
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
#include "dbus-timeout.h"
#include "dbus-list.h"

/**
 * @defgroup DBusTimeoutInternals DBusTimeout implementation details
 * @ingroup  DBusInternals
 * @brief implementation details for DBusTimeout
 * 
 * @{
 */

/**
 * Internals of DBusTimeout
 */
struct DBusTimeout
{
  int refcount;                                /**< Reference count */
  int interval;                                /**< Timeout interval in milliseconds. */

  DBusTimeoutHandler handler;                  /**< Timeout handler. */
  void *handler_data;                          /**< Timeout handler data. */
  DBusFreeFunction free_handler_data_function; /**< Free the timeout handler data. */
  
  void *data;		   	               /**< Application data. */
  DBusFreeFunction free_data_function;         /**< Free the application data. */
  unsigned int enabled : 1;                    /**< True if timeout is active. */
  unsigned int needs_restart : 1;              /**< Flag that timeout should be restarted after re-enabling. */
};

/**
 * Creates a new DBusTimeout, enabled by default.
 * @param interval the timeout interval in milliseconds.
 * @param handler function to call when the timeout occurs.
 * @param data data to pass to the handler
 * @param free_data_function function to be called to free the data.
 * @returns the new DBusTimeout object,
 */
DBusTimeout*
_dbus_timeout_new (int                 interval,
		   DBusTimeoutHandler  handler,
		   void               *data,
		   DBusFreeFunction    free_data_function)
{
  DBusTimeout *timeout;

  timeout = dbus_new0 (DBusTimeout, 1);
  if (timeout == NULL)
    return NULL;
  
  timeout->refcount = 1;
  timeout->interval = interval;

  timeout->handler = handler;
  timeout->handler_data = data;
  timeout->free_handler_data_function = free_data_function;

  timeout->enabled = TRUE;
  timeout->needs_restart = FALSE;
  
  return timeout;
}

/**
 * Increments the reference count of a DBusTimeout object.
 *
 * @param timeout the timeout object.
 * @returns the timeout object.
 */
DBusTimeout *
_dbus_timeout_ref (DBusTimeout *timeout)
{
  timeout->refcount += 1;

  return timeout;
}

/**
 * Decrements the reference count of a DBusTimeout object
 * and finalizes the object if the count reaches zero.
 *
 * @param timeout the timeout object.
 */
void
_dbus_timeout_unref (DBusTimeout *timeout)
{
  _dbus_assert (timeout != NULL);
  _dbus_assert (timeout->refcount > 0);
  
  timeout->refcount -= 1;
  if (timeout->refcount == 0)
    {
      dbus_timeout_set_data (timeout, NULL, NULL); /* call free_data_function */

      if (timeout->free_handler_data_function)
	(* timeout->free_handler_data_function) (timeout->handler_data);
      
      dbus_free (timeout);
    }
}

/**
 * Change the timeout interval to be interval milliseconds from now
 * (forgetting when the timeout was initially started), and enable it.
 *
 * This function is only valid when used in conjunction with DBusLoop:
 * it can be used in the message bus daemon implementation or in unit tests,
 * but it cannot be used in conjunction with an application main loop.
 *
 * @param timeout the timeout
 * @param interval the new interval
 */
void
_dbus_timeout_restart (DBusTimeout *timeout,
                       int          interval)
{
  _dbus_assert (interval >= 0);
  
  timeout->interval = interval;
  timeout->enabled = TRUE;
  timeout->needs_restart = TRUE;
}

/**
 * Disable the timeout. Note that you should use
 * _dbus_connection_toggle_timeout_unlocked() etc. instead, if
 * the timeout is passed out to an application main loop.
 * i.e. you can't use this function in the D-Bus library, it's
 * only used in the message bus daemon implementation.
 *
 * @param timeout the timeout
 * @param enabled #TRUE if timeout should be enabled.
 */
void
_dbus_timeout_disable (DBusTimeout  *timeout)
{
  timeout->enabled = FALSE;
}

/**
 * @typedef DBusTimeoutList
 *
 * Opaque data type representing a list of timeouts
 * and a set of DBusAddTimeoutFunction/DBusRemoveTimeoutFunction.
 * Automatically handles removing/re-adding timeouts
 * when the DBusAddTimeoutFunction is updated or changed.
 * Holds a reference count to each timeout.
 *
 */

/**
 * DBusTimeoutList implementation details. All fields
 * are private.
 *
 */
struct DBusTimeoutList
{
  DBusList *timeouts; /**< Timeout objects. */

  DBusAddTimeoutFunction add_timeout_function;       /**< Callback for adding a timeout. */
  DBusRemoveTimeoutFunction remove_timeout_function; /**< Callback for removing a timeout. */
  DBusTimeoutToggledFunction timeout_toggled_function; /**< Callback when timeout is enabled/disabled or changes interval */
  void *timeout_data;                                /**< Data for timeout callbacks */
  DBusFreeFunction timeout_free_data_function;       /**< Free function for timeout callback data */
};

/**
 * Creates a new timeout list. Returns #NULL if insufficient
 * memory exists.
 *
 * @returns the new timeout list, or #NULL on failure.
 */
DBusTimeoutList*
_dbus_timeout_list_new (void)
{
  DBusTimeoutList *timeout_list;

  timeout_list = dbus_new0 (DBusTimeoutList, 1);
  if (timeout_list == NULL)
    return NULL;

  return timeout_list;
}

/**
 * Frees a DBusTimeoutList.
 *
 * @param timeout_list the timeout list.
 */
void
_dbus_timeout_list_free (DBusTimeoutList *timeout_list)
{
  /* free timeout_data and remove timeouts as a side effect */
  _dbus_timeout_list_set_functions (timeout_list,
				    NULL, NULL, NULL, NULL, NULL);

  _dbus_list_clear_full (&timeout_list->timeouts,
                         (DBusFreeFunction) _dbus_timeout_unref);

  dbus_free (timeout_list);
}

/**
 * Sets the timeout functions. This function is the "backend"
 * for dbus_connection_set_timeout_functions().
 *
 * @param timeout_list the timeout list
 * @param add_function the add timeout function.
 * @param remove_function the remove timeout function.
 * @param toggled_function toggle notify function, or #NULL
 * @param data the data for those functions.
 * @param free_data_function the function to free the data.
 * @returns #FALSE if no memory
 *
 */
dbus_bool_t
_dbus_timeout_list_set_functions (DBusTimeoutList           *timeout_list,
				  DBusAddTimeoutFunction     add_function,
				  DBusRemoveTimeoutFunction  remove_function,
                                  DBusTimeoutToggledFunction toggled_function,
				  void                      *data,
				  DBusFreeFunction           free_data_function)
{
  /* Add timeouts with the new function, failing on OOM */
  if (add_function != NULL)
    {
      DBusList *link;
      
      link = _dbus_list_get_first_link (&timeout_list->timeouts);
      while (link != NULL)
        {
          DBusList *next = _dbus_list_get_next_link (&timeout_list->timeouts,
                                                     link);
      
          if (!(* add_function) (link->data, data))
            {
              /* remove it all again and return FALSE */
              DBusList *link2;
              
              link2 = _dbus_list_get_first_link (&timeout_list->timeouts);
              while (link2 != link)
                {
                  DBusList *next2 = _dbus_list_get_next_link (&timeout_list->timeouts,
                                                              link2);

                  (* remove_function) (link2->data, data);
                  
                  link2 = next2;
                }

              return FALSE;
            }
      
          link = next;
        }
    }
  
  /* Remove all current timeouts from previous timeout handlers */

  if (timeout_list->remove_timeout_function != NULL)
    {
      _dbus_list_foreach (&timeout_list->timeouts,
			  (DBusForeachFunction) timeout_list->remove_timeout_function,
			  timeout_list->timeout_data);
    }

  if (timeout_list->timeout_free_data_function != NULL)
    (* timeout_list->timeout_free_data_function) (timeout_list->timeout_data);

  timeout_list->add_timeout_function = add_function;
  timeout_list->remove_timeout_function = remove_function;
  timeout_list->timeout_toggled_function = toggled_function;
  timeout_list->timeout_data = data;
  timeout_list->timeout_free_data_function = free_data_function;

  return TRUE;
}

/**
 * Adds a new timeout to the timeout list, invoking the
 * application DBusAddTimeoutFunction if appropriate.
 *
 * @param timeout_list the timeout list.
 * @param timeout the timeout to add.
 * @returns #TRUE on success, #FALSE If no memory.
 */
dbus_bool_t
_dbus_timeout_list_add_timeout (DBusTimeoutList *timeout_list,
				DBusTimeout     *timeout)
{
  if (!_dbus_list_append (&timeout_list->timeouts, timeout))
    return FALSE;

  _dbus_timeout_ref (timeout);

  if (timeout_list->add_timeout_function != NULL)
    {
      if (!(* timeout_list->add_timeout_function) (timeout,
                                                   timeout_list->timeout_data))
        {
          _dbus_list_remove_last (&timeout_list->timeouts, timeout);
          _dbus_timeout_unref (timeout);
          return FALSE;
        }
    }

  return TRUE;
}

/**
 * Removes a timeout from the timeout list, invoking the
 * application's DBusRemoveTimeoutFunction if appropriate.
 *
 * @param timeout_list the timeout list.
 * @param timeout the timeout to remove.
 */
void
_dbus_timeout_list_remove_timeout (DBusTimeoutList *timeout_list,
				   DBusTimeout     *timeout)
{
  if (!_dbus_list_remove (&timeout_list->timeouts, timeout))
    _dbus_assert_not_reached ("Nonexistent timeout was removed");

  if (timeout_list->remove_timeout_function != NULL)
    (* timeout_list->remove_timeout_function) (timeout,
					       timeout_list->timeout_data);

  _dbus_timeout_unref (timeout);
}

/**
 * Sets a timeout to the given enabled state, invoking the
 * application's DBusTimeoutToggledFunction if appropriate.
 *
 * @param timeout_list the timeout list.
 * @param timeout the timeout to toggle.
 * @param enabled #TRUE to enable
 */
void
_dbus_timeout_list_toggle_timeout (DBusTimeoutList           *timeout_list,
                                   DBusTimeout               *timeout,
                                   dbus_bool_t                enabled)
{
  enabled = !!enabled;
  
  if (enabled == timeout->enabled)
    return;

  timeout->enabled = enabled;
  
  if (timeout_list->timeout_toggled_function != NULL)
    (* timeout_list->timeout_toggled_function) (timeout,
                                                timeout_list->timeout_data);
}

/**
 * Returns whether a timeout needs restart time counting in the event loop.
 *
 * @param timeout the DBusTimeout object
 * @returns #TRUE if restart is needed
 */
dbus_bool_t
_dbus_timeout_needs_restart (DBusTimeout *timeout)
{
  return timeout->needs_restart;
}

/**
 * Mark timeout as restarted (setting timestamps is responsibility of the event
 * loop).
 *
 * @param timeout the DBusTimeout object
 */
void
_dbus_timeout_restarted (DBusTimeout *timeout)
{
  timeout->needs_restart = FALSE;
}

/** @} */

/**
 * @defgroup DBusTimeout DBusTimeout
 * @ingroup  DBus
 * @brief Object representing a timeout
 *
 * Types and functions related to DBusTimeout. A timeout
 * represents a timeout that the main loop needs to monitor,
 * as in Qt's QTimer or GLib's g_timeout_add().
 *
 * Use dbus_connection_set_timeout_functions() or dbus_server_set_timeout_functions()
 * to be notified when libdbus needs to add or remove timeouts.
 * 
 * @{
 */


/**
 * @typedef DBusTimeout
 *
 * Opaque object representing a timeout.
 */

/**
 * Gets the timeout interval. The dbus_timeout_handle()
 * should be called each time this interval elapses,
 * starting after it elapses once.
 *
 * The interval may change during the life of the
 * timeout; if so, the timeout will be disabled and
 * re-enabled (calling the "timeout toggled function")
 * to notify you of the change.
 *
 * @param timeout the DBusTimeout object.
 * @returns the interval in milliseconds.
 */
int
dbus_timeout_get_interval (DBusTimeout *timeout)
{
  return timeout->interval;
}

/**
 * Gets data previously set with dbus_timeout_set_data()
 * or #NULL if none.
 *
 * @param timeout the DBusTimeout object.
 * @returns previously-set data.
 */
void*
dbus_timeout_get_data (DBusTimeout *timeout)
{
  return timeout->data;
}

/**
 * Sets data which can be retrieved with dbus_timeout_get_data().
 * Intended for use by the DBusAddTimeoutFunction and
 * DBusRemoveTimeoutFunction to store their own data.  For example with
 * Qt you might store the QTimer for this timeout and with GLib
 * you might store a g_timeout_add result id.
 *
 * @param timeout the DBusTimeout object.
 * @param data the data.
 * @param free_data_function function to be called to free the data.
 */
void
dbus_timeout_set_data (DBusTimeout      *timeout,
		       void             *data,
		       DBusFreeFunction  free_data_function)
{
  if (timeout->free_data_function != NULL)
    (* timeout->free_data_function) (timeout->data);

  timeout->data = data;
  timeout->free_data_function = free_data_function;
}

/**
 * Calls the timeout handler for this timeout.
 * This function should be called when the timeout
 * occurs.
 *
 * If this function returns #FALSE, then there wasn't
 * enough memory to handle the timeout. Typically just
 * letting the timeout fire again next time it naturally
 * times out is an adequate response to that problem,
 * but you could try to do more if you wanted.
 *
 * @param timeout the DBusTimeout object.
 * @returns #FALSE if there wasn't enough memory 
 */
dbus_bool_t
dbus_timeout_handle (DBusTimeout *timeout)
{
  return (* timeout->handler) (timeout->handler_data);
}


/**
 * Returns whether a timeout is enabled or not. If not
 * enabled, it should not be polled by the main loop.
 *
 * @param timeout the DBusTimeout object
 * @returns #TRUE if the timeout is enabled
 */
dbus_bool_t
dbus_timeout_get_enabled (DBusTimeout *timeout)
{
  return timeout->enabled;
}

/** @} end public API docs */
