/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-connection.c DBusConnection object
 *
 * Copyright (C) 2002-2006  Red Hat Inc.
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
#include "dbus-shared.h"
#include "dbus-connection.h"
#include "dbus-list.h"
#include "dbus-timeout.h"
#include "dbus-transport.h"
#include "dbus-watch.h"
#include "dbus-connection-internal.h"
#include "dbus-pending-call-internal.h"
#include "dbus-list.h"
#include "dbus-hash.h"
#include "dbus-message-internal.h"
#include "dbus-message-private.h"
#include "dbus-threads.h"
#include "dbus-protocol.h"
#include "dbus-dataslot.h"
#include "dbus-string.h"
#include "dbus-signature.h"
#include "dbus-pending-call.h"
#include "dbus-object-tree.h"
#include "dbus-threads-internal.h"
#include "dbus-bus.h"
#include "dbus-marshal-basic.h"

#ifdef DBUS_DISABLE_CHECKS
#define TOOK_LOCK_CHECK(connection)
#define RELEASING_LOCK_CHECK(connection)
#define HAVE_LOCK_CHECK(connection)
#else
#define TOOK_LOCK_CHECK(connection) do {                \
    _dbus_assert (!(connection)->have_connection_lock); \
    (connection)->have_connection_lock = TRUE;          \
  } while (0)
#define RELEASING_LOCK_CHECK(connection) do {            \
    _dbus_assert ((connection)->have_connection_lock);   \
    (connection)->have_connection_lock = FALSE;          \
  } while (0)
#define HAVE_LOCK_CHECK(connection)        _dbus_assert ((connection)->have_connection_lock)
/* A "DO_NOT_HAVE_LOCK_CHECK" is impossible since we need the lock to check the flag */
#endif

#define TRACE_LOCKS 1

#define CONNECTION_LOCK(connection)   do {                                      \
    if (TRACE_LOCKS) { _dbus_verbose ("LOCK\n"); }   \
    _dbus_rmutex_lock ((connection)->mutex);                                    \
    TOOK_LOCK_CHECK (connection);                                               \
  } while (0)

#define CONNECTION_UNLOCK(connection) _dbus_connection_unlock (connection)

#define SLOTS_LOCK(connection) do {                     \
    _dbus_rmutex_lock ((connection)->slot_mutex);       \
  } while (0)

#define SLOTS_UNLOCK(connection) do {                   \
    _dbus_rmutex_unlock ((connection)->slot_mutex);     \
  } while (0)

#define DISPATCH_STATUS_NAME(s)                                            \
                     ((s) == DBUS_DISPATCH_COMPLETE ? "complete" :         \
                      (s) == DBUS_DISPATCH_DATA_REMAINS ? "data remains" : \
                      (s) == DBUS_DISPATCH_NEED_MEMORY ? "need memory" :   \
                      "???")

/**
 * @defgroup DBusConnection DBusConnection
 * @ingroup  DBus
 * @brief Connection to another application
 *
 * A DBusConnection represents a connection to another
 * application. Messages can be sent and received via this connection.
 * The other application may be a message bus; for convenience, the
 * function dbus_bus_get() is provided to automatically open a
 * connection to the well-known message buses.
 * 
 * In brief a DBusConnection is a message queue associated with some
 * message transport mechanism such as a socket.  The connection
 * maintains a queue of incoming messages and a queue of outgoing
 * messages.
 *
 * Several functions use the following terms:
 * <ul>
 * <li><b>read</b> means to fill the incoming message queue by reading from the socket</li>
 * <li><b>write</b> means to drain the outgoing queue by writing to the socket</li>
 * <li><b>dispatch</b> means to drain the incoming queue by invoking application-provided message handlers</li>
 * </ul>
 *
 * The function dbus_connection_read_write_dispatch() for example does all
 * three of these things, offering a simple alternative to a main loop.
 *
 * In an application with a main loop, the read/write/dispatch
 * operations are usually separate.
 *
 * The connection provides #DBusWatch and #DBusTimeout objects to
 * the main loop. These are used to know when reading, writing, or
 * dispatching should be performed.
 * 
 * Incoming messages are processed
 * by calling dbus_connection_dispatch(). dbus_connection_dispatch()
 * runs any handlers registered for the topmost message in the message
 * queue, then discards the message, then returns.
 * 
 * dbus_connection_get_dispatch_status() indicates whether
 * messages are currently in the queue that need dispatching.
 * dbus_connection_set_dispatch_status_function() allows
 * you to set a function to be used to monitor the dispatch status.
 * 
 * If you're using GLib or Qt add-on libraries for D-Bus, there are
 * special convenience APIs in those libraries that hide
 * all the details of dispatch and watch/timeout monitoring.
 * For example, dbus_connection_setup_with_g_main().
 *
 * If you aren't using these add-on libraries, but want to process
 * messages asynchronously, you must manually call
 * dbus_connection_set_dispatch_status_function(),
 * dbus_connection_set_watch_functions(),
 * dbus_connection_set_timeout_functions() providing appropriate
 * functions to integrate the connection with your application's main
 * loop. This can be tricky to get right; main loops are not simple.
 *
 * If you don't need to be asynchronous, you can ignore #DBusWatch,
 * #DBusTimeout, and dbus_connection_dispatch().  Instead,
 * dbus_connection_read_write_dispatch() can be used.
 *
 * Or, in <em>very</em> simple applications,
 * dbus_connection_pop_message() may be all you need, allowing you to
 * avoid setting up any handler functions (see
 * dbus_connection_add_filter(),
 * dbus_connection_register_object_path() for more on handlers).
 * 
 * When you use dbus_connection_send() or one of its variants to send
 * a message, the message is added to the outgoing queue.  It's
 * actually written to the network later; either in
 * dbus_watch_handle() invoked by your main loop, or in
 * dbus_connection_flush() which blocks until it can write out the
 * entire outgoing queue. The GLib/Qt add-on libraries again
 * handle the details here for you by setting up watch functions.
 *
 * When a connection is disconnected, you are guaranteed to get a
 * signal "Disconnected" from the interface
 * #DBUS_INTERFACE_LOCAL, path
 * #DBUS_PATH_LOCAL.
 *
 * You may not drop the last reference to a #DBusConnection
 * until that connection has been disconnected.
 *
 * You may dispatch the unprocessed incoming message queue even if the
 * connection is disconnected. However, "Disconnected" will always be
 * the last message in the queue (obviously no messages are received
 * after disconnection).
 *
 * After calling dbus_threads_init(), #DBusConnection has thread
 * locks and drops them when invoking user callbacks, so in general is
 * transparently threadsafe. However, #DBusMessage does NOT have
 * thread locks; you must not send the same message to multiple
 * #DBusConnection if those connections will be used from different threads,
 * for example.
 *
 * Also, if you dispatch or pop messages from multiple threads, it
 * may work in the sense that it won't crash, but it's tough to imagine
 * sane results; it will be completely unpredictable which messages
 * go to which threads.
 *
 * It's recommended to dispatch from a single thread.
 *
 * The most useful function to call from multiple threads at once
 * is dbus_connection_send_with_reply_and_block(). That is,
 * multiple threads can make method calls at the same time.
 *
 * If you aren't using threads, you can use a main loop and
 * dbus_pending_call_set_notify() to achieve a similar result.
 */

/**
 * @defgroup DBusConnectionInternals DBusConnection implementation details
 * @ingroup  DBusInternals
 * @brief Implementation details of DBusConnection
 *
 * @{
 */

static void
_dbus_connection_trace_ref (DBusConnection *connection,
    int old_refcount,
    int new_refcount,
    const char *why)
{
#ifdef DBUS_ENABLE_VERBOSE_MODE
  static int enabled = -1;

  _dbus_trace_ref ("DBusConnection", connection, old_refcount, new_refcount,
      why, "DBUS_CONNECTION_TRACE", &enabled);
#endif
}

/**
 * Internal struct representing a message filter function 
 */
typedef struct DBusMessageFilter DBusMessageFilter;

/**
 * Internal struct representing a message filter function 
 */
struct DBusMessageFilter
{
  DBusAtomic refcount; /**< Reference count */
  DBusHandleMessageFunction function; /**< Function to call to filter */
  void *user_data; /**< User data for the function */
  DBusFreeFunction free_user_data_function; /**< Function to free the user data */
};


/**
 * Internals of DBusPreallocatedSend
 */
struct DBusPreallocatedSend
{
  DBusConnection *connection; /**< Connection we'd send the message to */
  DBusList *queue_link;       /**< Preallocated link in the queue */
  DBusList *counter_link;     /**< Preallocated link in the resource counter */
};

#if HAVE_DECL_MSG_NOSIGNAL
static DBusAtomic _dbus_modify_sigpipe = { FALSE };
#else
static DBusAtomic _dbus_modify_sigpipe = { TRUE };
#endif

/**
 * Implementation details of DBusConnection. All fields are private.
 */
struct DBusConnection
{
  DBusAtomic refcount; /**< Reference count. */

  DBusRMutex *mutex; /**< Lock on the entire DBusConnection */

  DBusCMutex *dispatch_mutex;     /**< Protects dispatch_acquired */
  DBusCondVar *dispatch_cond;    /**< Notify when dispatch_acquired is available */
  DBusCMutex *io_path_mutex;      /**< Protects io_path_acquired */
  DBusCondVar *io_path_cond;     /**< Notify when io_path_acquired is available */
  
  DBusList *outgoing_messages; /**< Queue of messages we need to send, send the end of the list first. */
  DBusList *incoming_messages; /**< Queue of messages we have received, end of the list received most recently. */
  DBusList *expired_messages;  /**< Messages that will be released when we next unlock. */

  DBusMessage *message_borrowed; /**< Filled in if the first incoming message has been borrowed;
                                  *   dispatch_acquired will be set by the borrower
                                  */
  
  int n_outgoing;              /**< Length of outgoing queue. */
  int n_incoming;              /**< Length of incoming queue. */

  DBusCounter *outgoing_counter; /**< Counts size of outgoing messages. */
  
  DBusTransport *transport;    /**< Object that sends/receives messages over network. */
  DBusWatchList *watches;      /**< Stores active watches. */
  DBusTimeoutList *timeouts;   /**< Stores active timeouts. */
  
  DBusList *filter_list;        /**< List of filters. */

  DBusRMutex *slot_mutex;        /**< Lock on slot_list so overall connection lock need not be taken */
  DBusDataSlotList slot_list;   /**< Data stored by allocated integer ID */

  DBusHashTable *pending_replies;  /**< Hash of message serials to #DBusPendingCall. */  
  
  dbus_uint32_t client_serial;       /**< Client serial. Increments each time a message is sent  */
  DBusList *disconnect_message_link; /**< Preallocated list node for queueing the disconnection message */

  DBusWakeupMainFunction wakeup_main_function; /**< Function to wake up the mainloop  */
  void *wakeup_main_data; /**< Application data for wakeup_main_function */
  DBusFreeFunction free_wakeup_main_data; /**< free wakeup_main_data */

  DBusDispatchStatusFunction dispatch_status_function; /**< Function on dispatch status changes  */
  void *dispatch_status_data; /**< Application data for dispatch_status_function */
  DBusFreeFunction free_dispatch_status_data; /**< free dispatch_status_data */

  DBusDispatchStatus last_dispatch_status; /**< The last dispatch status we reported to the application. */

  DBusObjectTree *objects; /**< Object path handlers registered with this connection */

  char *server_guid; /**< GUID of server if we are in shared_connections, #NULL if server GUID is unknown or connection is private */

  /* These two MUST be bools and not bitfields, because they are protected by a separate lock
   * from connection->mutex and all bitfields in a word have to be read/written together.
   * So you can't have a different lock for different bitfields in the same word.
   */
  dbus_bool_t dispatch_acquired; /**< Someone has dispatch path (can drain incoming queue) */
  dbus_bool_t io_path_acquired;  /**< Someone has transport io path (can use the transport to read/write messages) */
  
  unsigned int shareable : 1; /**< #TRUE if libdbus owns a reference to the connection and can return it from dbus_connection_open() more than once */
  
  unsigned int exit_on_disconnect : 1; /**< If #TRUE, exit after handling disconnect signal */

  unsigned int route_peer_messages : 1; /**< If #TRUE, if org.freedesktop.DBus.Peer messages have a bus name, don't handle them automatically */

  unsigned int disconnected_message_arrived : 1;   /**< We popped or are dispatching the disconnected message.
                                                    * if the disconnect_message_link is NULL then we queued it, but
                                                    * this flag is whether it got to the head of the queue.
                                                    */
  unsigned int disconnected_message_processed : 1; /**< We did our default handling of the disconnected message,
                                                    * such as closing the connection.
                                                    */
  
#ifndef DBUS_DISABLE_CHECKS
  unsigned int have_connection_lock : 1; /**< Used to check locking */
#endif

#if defined(DBUS_ENABLE_CHECKS) || defined(DBUS_ENABLE_ASSERT)
  int generation; /**< _dbus_current_generation that should correspond to this connection */
#endif 
};

static DBusDispatchStatus _dbus_connection_get_dispatch_status_unlocked      (DBusConnection     *connection);
static void               _dbus_connection_update_dispatch_status_and_unlock (DBusConnection     *connection,
                                                                              DBusDispatchStatus  new_status);
static void               _dbus_connection_last_unref                        (DBusConnection     *connection);
static void               _dbus_connection_acquire_dispatch                  (DBusConnection     *connection);
static void               _dbus_connection_release_dispatch                  (DBusConnection     *connection);
static DBusDispatchStatus _dbus_connection_flush_unlocked                    (DBusConnection     *connection);
static void               _dbus_connection_close_possibly_shared_and_unlock  (DBusConnection     *connection);
static dbus_bool_t        _dbus_connection_get_is_connected_unlocked         (DBusConnection     *connection);
static dbus_bool_t        _dbus_connection_peek_for_reply_unlocked           (DBusConnection     *connection,
                                                                              dbus_uint32_t       client_serial);

static DBusMessageFilter *
_dbus_message_filter_ref (DBusMessageFilter *filter)
{
#ifdef DBUS_DISABLE_ASSERT
  _dbus_atomic_inc (&filter->refcount);
#else
  dbus_int32_t old_value;

  old_value = _dbus_atomic_inc (&filter->refcount);
  _dbus_assert (old_value > 0);
#endif

  return filter;
}

static void
_dbus_message_filter_unref (DBusMessageFilter *filter)
{
  dbus_int32_t old_value;

  old_value = _dbus_atomic_dec (&filter->refcount);
  _dbus_assert (old_value > 0);

  if (old_value == 1)
    {
      if (filter->free_user_data_function)
        (* filter->free_user_data_function) (filter->user_data);
      
      dbus_free (filter);
    }
}

/**
 * Acquires the connection lock.
 *
 * @param connection the connection.
 */
void
_dbus_connection_lock (DBusConnection *connection)
{
  CONNECTION_LOCK (connection);
}

/**
 * Releases the connection lock.
 *
 * @param connection the connection.
 */
void
_dbus_connection_unlock (DBusConnection *connection)
{
  DBusList *expired_messages;
  DBusList *iter;

  if (TRACE_LOCKS)
    {
      _dbus_verbose ("UNLOCK\n");
    }

  /* If we had messages that expired (fell off the incoming or outgoing
   * queues) while we were locked, actually release them now */
  expired_messages = connection->expired_messages;
  connection->expired_messages = NULL;

  RELEASING_LOCK_CHECK (connection);
  _dbus_rmutex_unlock (connection->mutex);

  for (iter = _dbus_list_pop_first_link (&expired_messages);
      iter != NULL;
      iter = _dbus_list_pop_first_link (&expired_messages))
    {
      DBusMessage *message = iter->data;

      dbus_message_unref (message);
      _dbus_list_free_link (iter);
    }
}

/**
 * Wakes up the main loop if it is sleeping
 * Needed if we're e.g. queueing outgoing messages
 * on a thread while the mainloop sleeps.
 *
 * @param connection the connection.
 */
static void
_dbus_connection_wakeup_mainloop (DBusConnection *connection)
{
  if (connection->wakeup_main_function)
    (*connection->wakeup_main_function) (connection->wakeup_main_data);
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Gets the locks so we can examine them
 *
 * @param connection the connection.
 * @param mutex_loc return for the location of the main mutex pointer
 * @param dispatch_mutex_loc return location of the dispatch mutex pointer
 * @param io_path_mutex_loc return location of the io_path mutex pointer
 * @param dispatch_cond_loc return location of the dispatch conditional 
 *        variable pointer
 * @param io_path_cond_loc return location of the io_path conditional 
 *        variable pointer
 */ 
void 
_dbus_connection_test_get_locks (DBusConnection *connection,
                                 DBusMutex     **mutex_loc,
                                 DBusMutex     **dispatch_mutex_loc,
                                 DBusMutex     **io_path_mutex_loc,
                                 DBusCondVar   **dispatch_cond_loc,
                                 DBusCondVar   **io_path_cond_loc)
{
  *mutex_loc = (DBusMutex *) connection->mutex;
  *dispatch_mutex_loc = (DBusMutex *) connection->dispatch_mutex;
  *io_path_mutex_loc = (DBusMutex *) connection->io_path_mutex;
  *dispatch_cond_loc = connection->dispatch_cond;
  *io_path_cond_loc = connection->io_path_cond;
}
#endif

/**
 * Adds a message-containing list link to the incoming message queue,
 * taking ownership of the link and the message's current refcount.
 * Cannot fail due to lack of memory.
 *
 * @param connection the connection.
 * @param link the message link to queue.
 */
void
_dbus_connection_queue_received_message_link (DBusConnection  *connection,
                                              DBusList        *link)
{
  DBusPendingCall *pending;
  dbus_uint32_t reply_serial;
  DBusMessage *message;

  _dbus_assert (_dbus_transport_peek_is_authenticated (connection->transport));

  _dbus_list_append_link (&connection->incoming_messages,
                          link);
  message = link->data;

  /* If this is a reply we're waiting on, remove timeout for it */
  reply_serial = dbus_message_get_reply_serial (message);
  if (reply_serial != 0)
    {
      pending = _dbus_hash_table_lookup_int (connection->pending_replies,
                                             reply_serial);
      if (pending != NULL)
	{
	  if (_dbus_pending_call_is_timeout_added_unlocked (pending))
            _dbus_connection_remove_timeout_unlocked (connection,
                                                      _dbus_pending_call_get_timeout_unlocked (pending));

	  _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);
	}
    }
  
  

  connection->n_incoming += 1;

  _dbus_connection_wakeup_mainloop (connection);
  
  _dbus_verbose ("Message %p (%s %s %s %s '%s' reply to %u) added to incoming queue %p, %d incoming\n",
                 message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_path (message) ?
                 dbus_message_get_path (message) :
                 "no path",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message),
                 dbus_message_get_reply_serial (message),
                 connection,
                 connection->n_incoming);

  _dbus_message_trace_ref (message, -1, -1,
      "_dbus_conection_queue_received_message_link");
}

/**
 * Adds a link + message to the incoming message queue.
 * Can't fail. Takes ownership of both link and message.
 *
 * @param connection the connection.
 * @param link the list node and message to queue.
 *
 */
void
_dbus_connection_queue_synthesized_message_link (DBusConnection *connection,
						 DBusList *link)
{
  HAVE_LOCK_CHECK (connection);
  
  _dbus_list_append_link (&connection->incoming_messages, link);

  connection->n_incoming += 1;

  _dbus_connection_wakeup_mainloop (connection);

  _dbus_message_trace_ref (link->data, -1, -1,
      "_dbus_connection_queue_synthesized_message_link");

  _dbus_verbose ("Synthesized message %p added to incoming queue %p, %d incoming\n",
                 link->data, connection, connection->n_incoming);
}


/**
 * Checks whether there are messages in the outgoing message queue.
 * Called with connection lock held.
 *
 * @param connection the connection.
 * @returns #TRUE if the outgoing queue is non-empty.
 */
dbus_bool_t
_dbus_connection_has_messages_to_send_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  return connection->outgoing_messages != NULL;
}

/**
 * Checks whether there are messages in the outgoing message queue.
 * Use dbus_connection_flush() to block until all outgoing
 * messages have been written to the underlying transport
 * (such as a socket).
 * 
 * @param connection the connection.
 * @returns #TRUE if the outgoing queue is non-empty.
 */
dbus_bool_t
dbus_connection_has_messages_to_send (DBusConnection *connection)
{
  dbus_bool_t v;
  
  _dbus_return_val_if_fail (connection != NULL, FALSE);

  CONNECTION_LOCK (connection);
  v = _dbus_connection_has_messages_to_send_unlocked (connection);
  CONNECTION_UNLOCK (connection);

  return v;
}

/**
 * Gets the next outgoing message. The message remains in the
 * queue, and the caller does not own a reference to it.
 *
 * @param connection the connection.
 * @returns the message to be sent.
 */ 
DBusMessage*
_dbus_connection_get_message_to_send (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  return _dbus_list_get_last (&connection->outgoing_messages);
}

/**
 * Notifies the connection that a message has been sent, so the
 * message can be removed from the outgoing queue.
 * Called with the connection lock held.
 *
 * @param connection the connection.
 * @param message the message that was sent.
 */
void
_dbus_connection_message_sent_unlocked (DBusConnection *connection,
                                        DBusMessage    *message)
{
  DBusList *link;

  HAVE_LOCK_CHECK (connection);
  
  /* This can be called before we even complete authentication, since
   * it's called on disconnect to clean up the outgoing queue.
   * It's also called as we successfully send each message.
   */
  
  link = _dbus_list_get_last_link (&connection->outgoing_messages);
  _dbus_assert (link != NULL);
  _dbus_assert (link->data == message);

  _dbus_list_unlink (&connection->outgoing_messages,
                     link);
  _dbus_list_prepend_link (&connection->expired_messages, link);

  connection->n_outgoing -= 1;

  _dbus_verbose ("Message %p (%s %s %s %s '%s') removed from outgoing queue %p, %d left to send\n",
                 message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_path (message) ?
                 dbus_message_get_path (message) :
                 "no path",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message),
                 connection, connection->n_outgoing);

  /* It's OK that in principle we call the notify function, because for the
   * outgoing limit, there isn't one */
  _dbus_message_remove_counter (message, connection->outgoing_counter);

  /* The message will actually be unreffed when we unlock */
}

/** Function to be called in protected_change_watch() with refcount held */
typedef dbus_bool_t (* DBusWatchAddFunction)     (DBusWatchList *list,
                                                  DBusWatch     *watch);
/** Function to be called in protected_change_watch() with refcount held */
typedef void        (* DBusWatchRemoveFunction)  (DBusWatchList *list,
                                                  DBusWatch     *watch);
/** Function to be called in protected_change_watch() with refcount held */
typedef void        (* DBusWatchToggleFunction)  (DBusWatchList *list,
                                                  DBusWatch     *watch,
                                                  dbus_bool_t    enabled);

static dbus_bool_t
protected_change_watch (DBusConnection         *connection,
                        DBusWatch              *watch,
                        DBusWatchAddFunction    add_function,
                        DBusWatchRemoveFunction remove_function,
                        DBusWatchToggleFunction toggle_function,
                        dbus_bool_t             enabled)
{
  dbus_bool_t retval;

  HAVE_LOCK_CHECK (connection);

  /* The original purpose of protected_change_watch() was to hold a
   * ref on the connection while dropping the connection lock, then
   * calling out to the app.  This was a broken hack that did not
   * work, since the connection was in a hosed state (no WatchList
   * field) while calling out.
   *
   * So for now we'll just keep the lock while calling out. This means
   * apps are not allowed to call DBusConnection methods inside a
   * watch function or they will deadlock.
   *
   * The "real fix" is to use the _and_unlock() pattern found
   * elsewhere in the code, to defer calling out to the app until
   * we're about to drop locks and return flow of control to the app
   * anyway.
   *
   * See http://lists.freedesktop.org/archives/dbus/2007-July/thread.html#8144
   */

  if (connection->watches)
    {
      if (add_function)
        retval = (* add_function) (connection->watches, watch);
      else if (remove_function)
        {
          retval = TRUE;
          (* remove_function) (connection->watches, watch);
        }
      else
        {
          retval = TRUE;
          (* toggle_function) (connection->watches, watch, enabled);
        }
      return retval;
    }
  else
    return FALSE;
}
     

/**
 * Adds a watch using the connection's DBusAddWatchFunction if
 * available. Otherwise records the watch to be added when said
 * function is available. Also re-adds the watch if the
 * DBusAddWatchFunction changes. May fail due to lack of memory.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param watch the watch to add.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_connection_add_watch_unlocked (DBusConnection *connection,
                                     DBusWatch      *watch)
{
  return protected_change_watch (connection, watch,
                                 _dbus_watch_list_add_watch,
                                 NULL, NULL, FALSE);
}

/**
 * Removes a watch using the connection's DBusRemoveWatchFunction
 * if available. It's an error to call this function on a watch
 * that was not previously added.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param watch the watch to remove.
 */
void
_dbus_connection_remove_watch_unlocked (DBusConnection *connection,
                                        DBusWatch      *watch)
{
  protected_change_watch (connection, watch,
                          NULL,
                          _dbus_watch_list_remove_watch,
                          NULL, FALSE);
}

/**
 * Toggles a watch and notifies app via connection's
 * DBusWatchToggledFunction if available. It's an error to call this
 * function on a watch that was not previously added.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param watch the watch to toggle.
 * @param enabled whether to enable or disable
 */
void
_dbus_connection_toggle_watch_unlocked (DBusConnection *connection,
                                        DBusWatch      *watch,
                                        dbus_bool_t     enabled)
{
  _dbus_assert (watch != NULL);

  protected_change_watch (connection, watch,
                          NULL, NULL,
                          _dbus_watch_list_toggle_watch,
                          enabled);
}

/** Function to be called in protected_change_timeout() with refcount held */
typedef dbus_bool_t (* DBusTimeoutAddFunction)    (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout);
/** Function to be called in protected_change_timeout() with refcount held */
typedef void        (* DBusTimeoutRemoveFunction) (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout);
/** Function to be called in protected_change_timeout() with refcount held */
typedef void        (* DBusTimeoutToggleFunction) (DBusTimeoutList *list,
                                                   DBusTimeout     *timeout,
                                                   dbus_bool_t      enabled);

static dbus_bool_t
protected_change_timeout (DBusConnection           *connection,
                          DBusTimeout              *timeout,
                          DBusTimeoutAddFunction    add_function,
                          DBusTimeoutRemoveFunction remove_function,
                          DBusTimeoutToggleFunction toggle_function,
                          dbus_bool_t               enabled)
{
  dbus_bool_t retval;

  HAVE_LOCK_CHECK (connection);

  /* The original purpose of protected_change_timeout() was to hold a
   * ref on the connection while dropping the connection lock, then
   * calling out to the app.  This was a broken hack that did not
   * work, since the connection was in a hosed state (no TimeoutList
   * field) while calling out.
   *
   * So for now we'll just keep the lock while calling out. This means
   * apps are not allowed to call DBusConnection methods inside a
   * timeout function or they will deadlock.
   *
   * The "real fix" is to use the _and_unlock() pattern found
   * elsewhere in the code, to defer calling out to the app until
   * we're about to drop locks and return flow of control to the app
   * anyway.
   *
   * See http://lists.freedesktop.org/archives/dbus/2007-July/thread.html#8144
   */

  if (connection->timeouts)
    {
      if (add_function)
        retval = (* add_function) (connection->timeouts, timeout);
      else if (remove_function)
        {
          retval = TRUE;
          (* remove_function) (connection->timeouts, timeout);
        }
      else
        {
          retval = TRUE;
          (* toggle_function) (connection->timeouts, timeout, enabled);
        }
      return retval;
    }
  else
    return FALSE;
}

/**
 * Adds a timeout using the connection's DBusAddTimeoutFunction if
 * available. Otherwise records the timeout to be added when said
 * function is available. Also re-adds the timeout if the
 * DBusAddTimeoutFunction changes. May fail due to lack of memory.
 * The timeout will fire repeatedly until removed.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param timeout the timeout to add.
 * @returns #TRUE on success.
 */
dbus_bool_t
_dbus_connection_add_timeout_unlocked (DBusConnection *connection,
                                       DBusTimeout    *timeout)
{
  return protected_change_timeout (connection, timeout,
                                   _dbus_timeout_list_add_timeout,
                                   NULL, NULL, FALSE);
}

/**
 * Removes a timeout using the connection's DBusRemoveTimeoutFunction
 * if available. It's an error to call this function on a timeout
 * that was not previously added.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param timeout the timeout to remove.
 */
void
_dbus_connection_remove_timeout_unlocked (DBusConnection *connection,
                                          DBusTimeout    *timeout)
{
  protected_change_timeout (connection, timeout,
                            NULL,
                            _dbus_timeout_list_remove_timeout,
                            NULL, FALSE);
}

/**
 * Toggles a timeout and notifies app via connection's
 * DBusTimeoutToggledFunction if available. It's an error to call this
 * function on a timeout that was not previously added.
 * Connection lock should be held when calling this.
 *
 * @param connection the connection.
 * @param timeout the timeout to toggle.
 * @param enabled whether to enable or disable
 */
void
_dbus_connection_toggle_timeout_unlocked (DBusConnection   *connection,
                                          DBusTimeout      *timeout,
                                          dbus_bool_t       enabled)
{
  protected_change_timeout (connection, timeout,
                            NULL, NULL,
                            _dbus_timeout_list_toggle_timeout,
                            enabled);
}

static dbus_bool_t
_dbus_connection_attach_pending_call_unlocked (DBusConnection  *connection,
                                               DBusPendingCall *pending)
{
  dbus_uint32_t reply_serial;
  DBusTimeout *timeout;

  HAVE_LOCK_CHECK (connection);

  reply_serial = _dbus_pending_call_get_reply_serial_unlocked (pending);

  _dbus_assert (reply_serial != 0);

  timeout = _dbus_pending_call_get_timeout_unlocked (pending);

  if (timeout)
    {
      if (!_dbus_connection_add_timeout_unlocked (connection, timeout))
        return FALSE;
      
      if (!_dbus_hash_table_insert_int (connection->pending_replies,
                                        reply_serial,
                                        pending))
        {
          _dbus_connection_remove_timeout_unlocked (connection, timeout);

          _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);
          HAVE_LOCK_CHECK (connection);
          return FALSE;
        }
      
      _dbus_pending_call_set_timeout_added_unlocked (pending, TRUE);
    }
  else
    {
      if (!_dbus_hash_table_insert_int (connection->pending_replies,
                                        reply_serial,
                                        pending))
        {
          HAVE_LOCK_CHECK (connection);
          return FALSE;
        }
    }

  _dbus_pending_call_ref_unlocked (pending);

  HAVE_LOCK_CHECK (connection);
  
  return TRUE;
}

static void
free_pending_call_on_hash_removal (void *data)
{
  DBusPendingCall *pending;
  DBusConnection  *connection;
  
  if (data == NULL)
    return;

  pending = data;

  connection = _dbus_pending_call_get_connection_unlocked (pending);

  HAVE_LOCK_CHECK (connection);
  
  if (_dbus_pending_call_is_timeout_added_unlocked (pending))
    {
      _dbus_connection_remove_timeout_unlocked (connection,
                                                _dbus_pending_call_get_timeout_unlocked (pending));
      
      _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);
    }

  /* FIXME 1.0? this is sort of dangerous and undesirable to drop the lock 
   * here, but the pending call finalizer could in principle call out to 
   * application code so we pretty much have to... some larger code reorg 
   * might be needed.
   */
  _dbus_connection_ref_unlocked (connection);
  _dbus_pending_call_unref_and_unlock (pending);
  CONNECTION_LOCK (connection);
  _dbus_connection_unref_unlocked (connection);
}

static void
_dbus_connection_detach_pending_call_unlocked (DBusConnection  *connection,
                                               DBusPendingCall *pending)
{
  /* This ends up unlocking to call the pending call finalizer, which is unexpected to
   * say the least.
   */
  _dbus_hash_table_remove_int (connection->pending_replies,
                               _dbus_pending_call_get_reply_serial_unlocked (pending));
}

static void
_dbus_connection_detach_pending_call_and_unlock (DBusConnection  *connection,
                                                 DBusPendingCall *pending)
{
  /* The idea here is to avoid finalizing the pending call
   * with the lock held, since there's a destroy notifier
   * in pending call that goes out to application code.
   *
   * There's an extra unlock inside the hash table
   * "free pending call" function FIXME...
   */
  _dbus_pending_call_ref_unlocked (pending);
  _dbus_hash_table_remove_int (connection->pending_replies,
                               _dbus_pending_call_get_reply_serial_unlocked (pending));

  if (_dbus_pending_call_is_timeout_added_unlocked (pending))
      _dbus_connection_remove_timeout_unlocked (connection,
              _dbus_pending_call_get_timeout_unlocked (pending));

  _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);

  _dbus_pending_call_unref_and_unlock (pending);
}

/**
 * Removes a pending call from the connection, such that
 * the pending reply will be ignored. May drop the last
 * reference to the pending call.
 *
 * @param connection the connection
 * @param pending the pending call
 */
void
_dbus_connection_remove_pending_call (DBusConnection  *connection,
                                      DBusPendingCall *pending)
{
  CONNECTION_LOCK (connection);
  _dbus_connection_detach_pending_call_and_unlock (connection, pending);
}

/**
 * Acquire the transporter I/O path. This must be done before
 * doing any I/O in the transporter. May sleep and drop the
 * IO path mutex while waiting for the I/O path.
 *
 * @param connection the connection.
 * @param timeout_milliseconds maximum blocking time, or -1 for no limit.
 * @returns TRUE if the I/O path was acquired.
 */
static dbus_bool_t
_dbus_connection_acquire_io_path (DBusConnection *connection,
				  int             timeout_milliseconds)
{
  dbus_bool_t we_acquired;
  
  HAVE_LOCK_CHECK (connection);

  /* We don't want the connection to vanish */
  _dbus_connection_ref_unlocked (connection);

  /* We will only touch io_path_acquired which is protected by our mutex */
  CONNECTION_UNLOCK (connection);
  
  _dbus_verbose ("locking io_path_mutex\n");
  _dbus_cmutex_lock (connection->io_path_mutex);

  _dbus_verbose ("start connection->io_path_acquired = %d timeout = %d\n",
                 connection->io_path_acquired, timeout_milliseconds);

  we_acquired = FALSE;
  
  if (connection->io_path_acquired)
    {
      if (timeout_milliseconds != -1)
        {
          _dbus_verbose ("waiting %d for IO path to be acquirable\n",
                         timeout_milliseconds);

          if (!_dbus_condvar_wait_timeout (connection->io_path_cond,
                                           connection->io_path_mutex,
                                           timeout_milliseconds))
            {
              /* We timed out before anyone signaled. */
              /* (writing the loop to handle the !timedout case by
               * waiting longer if needed is a pain since dbus
               * wraps pthread_cond_timedwait to take a relative
               * time instead of absolute, something kind of stupid
               * on our part. for now it doesn't matter, we will just
               * end up back here eventually.)
               */
            }
        }
      else
        {
          while (connection->io_path_acquired)
            {
              _dbus_verbose ("waiting for IO path to be acquirable\n");
              _dbus_condvar_wait (connection->io_path_cond, 
                                  connection->io_path_mutex);
            }
        }
    }
  
  if (!connection->io_path_acquired)
    {
      we_acquired = TRUE;
      connection->io_path_acquired = TRUE;
    }
  
  _dbus_verbose ("end connection->io_path_acquired = %d we_acquired = %d\n",
                 connection->io_path_acquired, we_acquired);

  _dbus_verbose ("unlocking io_path_mutex\n");
  _dbus_cmutex_unlock (connection->io_path_mutex);

  CONNECTION_LOCK (connection);
  
  HAVE_LOCK_CHECK (connection);

  _dbus_connection_unref_unlocked (connection);
  
  return we_acquired;
}

/**
 * Release the I/O path when you're done with it. Only call
 * after you've acquired the I/O. Wakes up at most one thread
 * currently waiting to acquire the I/O path.
 *
 * @param connection the connection.
 */
static void
_dbus_connection_release_io_path (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  _dbus_verbose ("locking io_path_mutex\n");
  _dbus_cmutex_lock (connection->io_path_mutex);
  
  _dbus_assert (connection->io_path_acquired);

  _dbus_verbose ("start connection->io_path_acquired = %d\n",
                 connection->io_path_acquired);
  
  connection->io_path_acquired = FALSE;
  _dbus_condvar_wake_one (connection->io_path_cond);

  _dbus_verbose ("unlocking io_path_mutex\n");
  _dbus_cmutex_unlock (connection->io_path_mutex);
}

/**
 * Queues incoming messages and sends outgoing messages for this
 * connection, optionally blocking in the process. Each call to
 * _dbus_connection_do_iteration_unlocked() will call select() or poll() one
 * time and then read or write data if possible.
 *
 * The purpose of this function is to be able to flush outgoing
 * messages or queue up incoming messages without returning
 * control to the application and causing reentrancy weirdness.
 *
 * The flags parameter allows you to specify whether to
 * read incoming messages, write outgoing messages, or both,
 * and whether to block if no immediate action is possible.
 *
 * The timeout_milliseconds parameter does nothing unless the
 * iteration is blocking.
 *
 * If there are no outgoing messages and DBUS_ITERATION_DO_READING
 * wasn't specified, then it's impossible to block, even if
 * you specify DBUS_ITERATION_BLOCK; in that case the function
 * returns immediately.
 *
 * If pending is not NULL then a check is made if the pending call
 * is completed after the io path has been required. If the call
 * has been completed nothing is done. This must be done since
 * the _dbus_connection_acquire_io_path releases the connection
 * lock for a while.
 *
 * Called with connection lock held.
 * 
 * @param connection the connection.
 * @param pending the pending call that should be checked or NULL
 * @param flags iteration flags.
 * @param timeout_milliseconds maximum blocking time, or -1 for no limit.
 */
void
_dbus_connection_do_iteration_unlocked (DBusConnection *connection,
                                        DBusPendingCall *pending,
                                        unsigned int    flags,
                                        int             timeout_milliseconds)
{
  _dbus_verbose ("start\n");
  
  HAVE_LOCK_CHECK (connection);
  
  if (connection->n_outgoing == 0)
    flags &= ~DBUS_ITERATION_DO_WRITING;

  if (_dbus_connection_acquire_io_path (connection,
					(flags & DBUS_ITERATION_BLOCK) ? timeout_milliseconds : 0))
    {
      HAVE_LOCK_CHECK (connection);
      
      if ( (pending != NULL) && _dbus_pending_call_get_completed_unlocked(pending))
        {
          _dbus_verbose ("pending call completed while acquiring I/O path");
        }
      else if ( (pending != NULL) &&
                _dbus_connection_peek_for_reply_unlocked (connection,
                                                          _dbus_pending_call_get_reply_serial_unlocked (pending)))
        {
          _dbus_verbose ("pending call completed while acquiring I/O path (reply found in queue)");
        }
      else
        {
          _dbus_transport_do_iteration (connection->transport,
                                        flags, timeout_milliseconds);
        }

      _dbus_connection_release_io_path (connection);
    }

  HAVE_LOCK_CHECK (connection);

  _dbus_verbose ("end\n");
}

/**
 * Creates a new connection for the given transport.  A transport
 * represents a message stream that uses some concrete mechanism, such
 * as UNIX domain sockets. May return #NULL if insufficient
 * memory exists to create the connection.
 *
 * @param transport the transport.
 * @returns the new connection, or #NULL on failure.
 */
DBusConnection*
_dbus_connection_new_for_transport (DBusTransport *transport)
{
  DBusConnection *connection;
  DBusWatchList *watch_list;
  DBusTimeoutList *timeout_list;
  DBusHashTable *pending_replies;
  DBusList *disconnect_link;
  DBusMessage *disconnect_message;
  DBusCounter *outgoing_counter;
  DBusObjectTree *objects;
  
  watch_list = NULL;
  connection = NULL;
  pending_replies = NULL;
  timeout_list = NULL;
  disconnect_link = NULL;
  disconnect_message = NULL;
  outgoing_counter = NULL;
  objects = NULL;
  
  watch_list = _dbus_watch_list_new ();
  if (watch_list == NULL)
    goto error;

  timeout_list = _dbus_timeout_list_new ();
  if (timeout_list == NULL)
    goto error;  

  pending_replies =
    _dbus_hash_table_new (DBUS_HASH_INT,
			  NULL,
                          (DBusFreeFunction)free_pending_call_on_hash_removal);
  if (pending_replies == NULL)
    goto error;
  
  connection = dbus_new0 (DBusConnection, 1);
  if (connection == NULL)
    goto error;

  _dbus_rmutex_new_at_location (&connection->mutex);
  if (connection->mutex == NULL)
    goto error;

  _dbus_cmutex_new_at_location (&connection->io_path_mutex);
  if (connection->io_path_mutex == NULL)
    goto error;

  _dbus_cmutex_new_at_location (&connection->dispatch_mutex);
  if (connection->dispatch_mutex == NULL)
    goto error;
  
  _dbus_condvar_new_at_location (&connection->dispatch_cond);
  if (connection->dispatch_cond == NULL)
    goto error;
  
  _dbus_condvar_new_at_location (&connection->io_path_cond);
  if (connection->io_path_cond == NULL)
    goto error;

  _dbus_rmutex_new_at_location (&connection->slot_mutex);
  if (connection->slot_mutex == NULL)
    goto error;

  disconnect_message = dbus_message_new_signal (DBUS_PATH_LOCAL,
                                                DBUS_INTERFACE_LOCAL,
                                                "Disconnected");
  
  if (disconnect_message == NULL)
    goto error;

  disconnect_link = _dbus_list_alloc_link (disconnect_message);
  if (disconnect_link == NULL)
    goto error;

  outgoing_counter = _dbus_counter_new ();
  if (outgoing_counter == NULL)
    goto error;

  objects = _dbus_object_tree_new (connection);
  if (objects == NULL)
    goto error;
  
  if (_dbus_atomic_get (&_dbus_modify_sigpipe) != 0)
    _dbus_disable_sigpipe ();

  /* initialized to 0: use atomic op to avoid mixing atomic and non-atomic */
  _dbus_atomic_inc (&connection->refcount);
  connection->transport = transport;
  connection->watches = watch_list;
  connection->timeouts = timeout_list;
  connection->pending_replies = pending_replies;
  connection->outgoing_counter = outgoing_counter;
  connection->filter_list = NULL;
  connection->last_dispatch_status = DBUS_DISPATCH_COMPLETE; /* so we're notified first time there's data */
  connection->objects = objects;
  connection->exit_on_disconnect = FALSE;
  connection->shareable = FALSE;
  connection->route_peer_messages = FALSE;
  connection->disconnected_message_arrived = FALSE;
  connection->disconnected_message_processed = FALSE;
  
#if defined(DBUS_ENABLE_CHECKS) || defined(DBUS_ENABLE_ASSERT)
  connection->generation = _dbus_current_generation;
#endif
  
  _dbus_data_slot_list_init (&connection->slot_list);

  connection->client_serial = 1;

  connection->disconnect_message_link = disconnect_link;

  CONNECTION_LOCK (connection);
  
  if (!_dbus_transport_set_connection (transport, connection))
    {
      CONNECTION_UNLOCK (connection);

      goto error;
    }

  _dbus_transport_ref (transport);

  CONNECTION_UNLOCK (connection);

  _dbus_connection_trace_ref (connection, 0, 1, "new_for_transport");
  return connection;
  
 error:
  if (disconnect_message != NULL)
    dbus_message_unref (disconnect_message);
  
  if (disconnect_link != NULL)
    _dbus_list_free_link (disconnect_link);
  
  if (connection != NULL)
    {
      _dbus_condvar_free_at_location (&connection->io_path_cond);
      _dbus_condvar_free_at_location (&connection->dispatch_cond);
      _dbus_rmutex_free_at_location (&connection->mutex);
      _dbus_cmutex_free_at_location (&connection->io_path_mutex);
      _dbus_cmutex_free_at_location (&connection->dispatch_mutex);
      _dbus_rmutex_free_at_location (&connection->slot_mutex);
      dbus_free (connection);
    }
  if (pending_replies)
    _dbus_hash_table_unref (pending_replies);
  
  if (watch_list)
    _dbus_watch_list_free (watch_list);

  if (timeout_list)
    _dbus_timeout_list_free (timeout_list);

  if (outgoing_counter)
    _dbus_counter_unref (outgoing_counter);

  if (objects)
    _dbus_object_tree_unref (objects);
  
  return NULL;
}

/**
 * Increments the reference count of a DBusConnection.
 * Requires that the caller already holds the connection lock.
 *
 * @param connection the connection.
 * @returns the connection.
 */
DBusConnection *
_dbus_connection_ref_unlocked (DBusConnection *connection)
{
  dbus_int32_t old_refcount;

  _dbus_assert (connection != NULL);
  _dbus_assert (connection->generation == _dbus_current_generation);

  HAVE_LOCK_CHECK (connection);

  old_refcount = _dbus_atomic_inc (&connection->refcount);
  _dbus_connection_trace_ref (connection, old_refcount, old_refcount + 1,
      "ref_unlocked");

  return connection;
}

/**
 * Decrements the reference count of a DBusConnection.
 * Requires that the caller already holds the connection lock.
 *
 * @param connection the connection.
 */
void
_dbus_connection_unref_unlocked (DBusConnection *connection)
{
  dbus_int32_t old_refcount;

  HAVE_LOCK_CHECK (connection);

  _dbus_assert (connection != NULL);

  old_refcount = _dbus_atomic_dec (&connection->refcount);

  _dbus_connection_trace_ref (connection, old_refcount, old_refcount - 1,
      "unref_unlocked");

  if (old_refcount == 1)
    _dbus_connection_last_unref (connection);
}

static dbus_uint32_t
_dbus_connection_get_next_client_serial (DBusConnection *connection)
{
  dbus_uint32_t serial;

  serial = connection->client_serial++;

  if (connection->client_serial == 0)
    connection->client_serial = 1;

  return serial;
}

/**
 * A callback for use with dbus_watch_new() to create a DBusWatch.
 * 
 * @todo This is basically a hack - we could delete _dbus_transport_handle_watch()
 * and the virtual handle_watch in DBusTransport if we got rid of it.
 * The reason this is some work is threading, see the _dbus_connection_handle_watch()
 * implementation.
 *
 * @param watch the watch.
 * @param condition the current condition of the file descriptors being watched.
 * @param data must be a pointer to a #DBusConnection
 * @returns #FALSE if the IO condition may not have been fully handled due to lack of memory
 */
dbus_bool_t
_dbus_connection_handle_watch (DBusWatch                   *watch,
                               unsigned int                 condition,
                               void                        *data)
{
  DBusConnection *connection;
  dbus_bool_t retval;
  DBusDispatchStatus status;

  connection = data;

  _dbus_verbose ("start\n");
  
  CONNECTION_LOCK (connection);

  if (!_dbus_connection_acquire_io_path (connection, 1))
    {
      /* another thread is handling the message */
      CONNECTION_UNLOCK (connection);
      return TRUE;
    }

  HAVE_LOCK_CHECK (connection);
  retval = _dbus_transport_handle_watch (connection->transport,
                                         watch, condition);

  _dbus_connection_release_io_path (connection);

  HAVE_LOCK_CHECK (connection);

  _dbus_verbose ("middle\n");
  
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* this calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);

  _dbus_verbose ("end\n");
  
  return retval;
}

/* Protected by _DBUS_LOCK (shared_connections) */
static DBusHashTable *shared_connections = NULL;
static DBusList *shared_connections_no_guid = NULL;

static void
close_connection_on_shutdown (DBusConnection *connection)
{
  DBusMessage *message;

  dbus_connection_ref (connection);
  _dbus_connection_close_possibly_shared (connection);

  /* Churn through to the Disconnected message */
  while ((message = dbus_connection_pop_message (connection)))
    {
      dbus_message_unref (message);
    }
  dbus_connection_unref (connection);
}

static void
shared_connections_shutdown (void *data)
{
  int n_entries;

  if (!_DBUS_LOCK (shared_connections))
    {
      /* We'd have initialized locks before adding anything, so there
       * can't be anything there. */
      return;
    }

  /* This is a little bit unpleasant... better ideas? */
  while ((n_entries = _dbus_hash_table_get_n_entries (shared_connections)) > 0)
    {
      DBusConnection *connection;
      DBusHashIter iter;
      
      _dbus_hash_iter_init (shared_connections, &iter);
      _dbus_hash_iter_next (&iter);
       
      connection = _dbus_hash_iter_get_value (&iter);

      _DBUS_UNLOCK (shared_connections);
      close_connection_on_shutdown (connection);
      if (!_DBUS_LOCK (shared_connections))
        _dbus_assert_not_reached ("global locks were already initialized");

      /* The connection should now be dead and not in our hash ... */
      _dbus_assert (_dbus_hash_table_get_n_entries (shared_connections) < n_entries);
    }

  _dbus_assert (_dbus_hash_table_get_n_entries (shared_connections) == 0);
  
  _dbus_hash_table_unref (shared_connections);
  shared_connections = NULL;

  if (shared_connections_no_guid != NULL)
    {
      DBusConnection *connection;
      connection = _dbus_list_pop_first (&shared_connections_no_guid);
      while (connection != NULL)
        {
          _DBUS_UNLOCK (shared_connections);
          close_connection_on_shutdown (connection);
          if (!_DBUS_LOCK (shared_connections))
            _dbus_assert_not_reached ("global locks were already initialized");
          connection = _dbus_list_pop_first (&shared_connections_no_guid);
        }
    }

  shared_connections_no_guid = NULL;
  
  _DBUS_UNLOCK (shared_connections);
}

static dbus_bool_t
connection_lookup_shared (DBusAddressEntry  *entry,
                          DBusConnection   **result)
{
  _dbus_verbose ("checking for existing connection\n");
  
  *result = NULL;

  if (!_DBUS_LOCK (shared_connections))
    {
      /* If it was shared, we'd have initialized global locks when we put
       * it in shared_connections. */
      return FALSE;
    }

  if (shared_connections == NULL)
    {
      _dbus_verbose ("creating shared_connections hash table\n");
      
      shared_connections = _dbus_hash_table_new (DBUS_HASH_STRING,
                                                 dbus_free,
                                                 NULL);
      if (shared_connections == NULL)
        {
          _DBUS_UNLOCK (shared_connections);
          return FALSE;
        }

      if (!_dbus_register_shutdown_func (shared_connections_shutdown, NULL))
        {
          _dbus_hash_table_unref (shared_connections);
          shared_connections = NULL;
          _DBUS_UNLOCK (shared_connections);
          return FALSE;
        }

      _dbus_verbose ("  successfully created shared_connections\n");
      
      _DBUS_UNLOCK (shared_connections);
      return TRUE; /* no point looking up in the hash we just made */
    }
  else
    {
      const char *guid;

      guid = dbus_address_entry_get_value (entry, "guid");
      
      if (guid != NULL)
        {
          DBusConnection *connection;
          
          connection = _dbus_hash_table_lookup_string (shared_connections,
                                                       guid);

          if (connection)
            {
              /* The DBusConnection can't be finalized without taking
               * the shared_connections lock to remove it from the
               * hash.  So it's safe to ref the connection here.
               * However, it may be disconnected if the Disconnected
               * message hasn't been processed yet, in which case we
               * want to pretend it isn't in the hash and avoid
               * returning it.
               *
               * The idea is to avoid ever returning a disconnected connection
               * from dbus_connection_open(). We could just synchronously
               * drop our shared ref to the connection on connection disconnect,
               * and then assert here that the connection is connected, but
               * that causes reentrancy headaches.
               */
              CONNECTION_LOCK (connection);
              if (_dbus_connection_get_is_connected_unlocked (connection))
                {
                  _dbus_connection_ref_unlocked (connection);
                  *result = connection;
                  _dbus_verbose ("looked up existing connection to server guid %s\n",
                                 guid);
                }
              else
                {
                  _dbus_verbose ("looked up existing connection to server guid %s but it was disconnected so ignoring it\n",
                                 guid);
                }
              CONNECTION_UNLOCK (connection);
            }
        }
      
      _DBUS_UNLOCK (shared_connections);
      return TRUE;
    }
}

static dbus_bool_t
connection_record_shared_unlocked (DBusConnection *connection,
                                   const char     *guid)
{
  char *guid_key;
  char *guid_in_connection;

  HAVE_LOCK_CHECK (connection);
  _dbus_assert (connection->server_guid == NULL);
  _dbus_assert (connection->shareable);

  /* get a hard ref on this connection, even if
   * we won't in fact store it in the hash, we still
   * need to hold a ref on it until it's disconnected.
   */
  _dbus_connection_ref_unlocked (connection);

  if (guid == NULL)
    {
      if (!_DBUS_LOCK (shared_connections))
        return FALSE;

      if (!_dbus_list_prepend (&shared_connections_no_guid, connection))
        {
          _DBUS_UNLOCK (shared_connections);
          return FALSE;
        }

      _DBUS_UNLOCK (shared_connections);
      return TRUE; /* don't store in the hash */
    }
  
  /* A separate copy of the key is required in the hash table, because
   * we don't have a lock on the connection when we are doing a hash
   * lookup.
   */
  
  guid_key = _dbus_strdup (guid);
  if (guid_key == NULL)
    return FALSE;

  guid_in_connection = _dbus_strdup (guid);
  if (guid_in_connection == NULL)
    {
      dbus_free (guid_key);
      return FALSE;
    }

  if (!_DBUS_LOCK (shared_connections))
    {
      dbus_free (guid_in_connection);
      dbus_free (guid_key);
      return FALSE;
    }

  _dbus_assert (shared_connections != NULL);
  
  if (!_dbus_hash_table_insert_string (shared_connections,
                                       guid_key, connection))
    {
      dbus_free (guid_key);
      dbus_free (guid_in_connection);
      _DBUS_UNLOCK (shared_connections);
      return FALSE;
    }

  connection->server_guid = guid_in_connection;

  _dbus_verbose ("stored connection to %s to be shared\n",
                 connection->server_guid);
  
  _DBUS_UNLOCK (shared_connections);

  _dbus_assert (connection->server_guid != NULL);
  
  return TRUE;
}

static void
connection_forget_shared_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);

  if (!connection->shareable)
    return;

  if (!_DBUS_LOCK (shared_connections))
    {
      /* If it was shared, we'd have initialized global locks when we put
       * it in the table; so it can't be there. */
      return;
    }

  if (connection->server_guid != NULL)
    {
      _dbus_verbose ("dropping connection to %s out of the shared table\n",
                     connection->server_guid);
      
      if (!_dbus_hash_table_remove_string (shared_connections,
                                           connection->server_guid))
        _dbus_assert_not_reached ("connection was not in the shared table");
      
      dbus_free (connection->server_guid);
      connection->server_guid = NULL;
    }
  else
    {
      _dbus_list_remove (&shared_connections_no_guid, connection);
    }

  _DBUS_UNLOCK (shared_connections);
  
  /* remove our reference held on all shareable connections */
  _dbus_connection_unref_unlocked (connection);
}

static DBusConnection*
connection_try_from_address_entry (DBusAddressEntry *entry,
                                   DBusError        *error)
{
  DBusTransport *transport;
  DBusConnection *connection;

  transport = _dbus_transport_open (entry, error);

  if (transport == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (error);
      return NULL;
    }

  connection = _dbus_connection_new_for_transport (transport);

  _dbus_transport_unref (transport);
  
  if (connection == NULL)
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

#ifndef DBUS_DISABLE_CHECKS
  _dbus_assert (!connection->have_connection_lock);
#endif
  return connection;
}

/*
 * If the shared parameter is true, then any existing connection will
 * be used (and if a new connection is created, it will be available
 * for use by others). If the shared parameter is false, a new
 * connection will always be created, and the new connection will
 * never be returned to other callers.
 *
 * @param address the address
 * @param shared whether the connection is shared or private
 * @param error error return
 * @returns the connection or #NULL on error
 */
static DBusConnection*
_dbus_connection_open_internal (const char     *address,
                                dbus_bool_t     shared,
                                DBusError      *error)
{
  DBusConnection *connection;
  DBusAddressEntry **entries;
  DBusError tmp_error = DBUS_ERROR_INIT;
  DBusError first_error = DBUS_ERROR_INIT;
  int len, i;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  _dbus_verbose ("opening %s connection to: %s\n",
                 shared ? "shared" : "private", address);
  
  if (!dbus_parse_address (address, &entries, &len, error))
    return NULL;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  connection = NULL;

  for (i = 0; i < len; i++)
    {
      if (shared)
        {
          if (!connection_lookup_shared (entries[i], &connection))
            _DBUS_SET_OOM (&tmp_error);
        }

      if (connection == NULL)
        {
          connection = connection_try_from_address_entry (entries[i],
                                                          &tmp_error);

          if (connection != NULL && shared)
            {
              const char *guid;
                  
              connection->shareable = TRUE;
                  
              /* guid may be NULL */
              guid = dbus_address_entry_get_value (entries[i], "guid");
                  
              CONNECTION_LOCK (connection);
          
              if (!connection_record_shared_unlocked (connection, guid))
                {
                  _DBUS_SET_OOM (&tmp_error);
                  _dbus_connection_close_possibly_shared_and_unlock (connection);
                  dbus_connection_unref (connection);
                  connection = NULL;
                }
              else
                CONNECTION_UNLOCK (connection);
            }
        }
      
      if (connection)
        break;

      _DBUS_ASSERT_ERROR_IS_SET (&tmp_error);
      
      if (i == 0)
        dbus_move_error (&tmp_error, &first_error);
      else
        dbus_error_free (&tmp_error);
    }
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  _DBUS_ASSERT_ERROR_IS_CLEAR (&tmp_error);
  
  if (connection == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (&first_error);
      dbus_move_error (&first_error, error);
    }
  else
    dbus_error_free (&first_error);
  
  dbus_address_entries_free (entries);
  return connection;
}

/**
 * Closes a shared OR private connection, while dbus_connection_close() can
 * only be used on private connections. Should only be called by the
 * dbus code that owns the connection - an owner must be known,
 * the open/close state is like malloc/free, not like ref/unref.
 * 
 * @param connection the connection
 */
void
_dbus_connection_close_possibly_shared (DBusConnection *connection)
{
  _dbus_assert (connection != NULL);
  _dbus_assert (connection->generation == _dbus_current_generation);

  CONNECTION_LOCK (connection);
  _dbus_connection_close_possibly_shared_and_unlock (connection);
}

static DBusPreallocatedSend*
_dbus_connection_preallocate_send_unlocked (DBusConnection *connection)
{
  DBusPreallocatedSend *preallocated;

  HAVE_LOCK_CHECK (connection);
  
  _dbus_assert (connection != NULL);
  
  preallocated = dbus_new (DBusPreallocatedSend, 1);
  if (preallocated == NULL)
    return NULL;

  preallocated->queue_link = _dbus_list_alloc_link (NULL);
  if (preallocated->queue_link == NULL)
    goto failed_0;

  preallocated->counter_link = _dbus_list_alloc_link (connection->outgoing_counter);
  if (preallocated->counter_link == NULL)
    goto failed_1;

  _dbus_counter_ref (preallocated->counter_link->data);

  preallocated->connection = connection;
  
  return preallocated;
  
 failed_1:
  _dbus_list_free_link (preallocated->queue_link);
 failed_0:
  dbus_free (preallocated);
  
  return NULL;
}

/* Called with lock held, does not update dispatch status */
static void
_dbus_connection_send_preallocated_unlocked_no_update (DBusConnection       *connection,
                                                       DBusPreallocatedSend *preallocated,
                                                       DBusMessage          *message,
                                                       dbus_uint32_t        *client_serial)
{
  dbus_uint32_t serial;

  preallocated->queue_link->data = message;
  _dbus_list_prepend_link (&connection->outgoing_messages,
                           preallocated->queue_link);

  /* It's OK that we'll never call the notify function, because for the
   * outgoing limit, there isn't one */
  _dbus_message_add_counter_link (message,
                                  preallocated->counter_link);

  dbus_free (preallocated);
  preallocated = NULL;
  
  dbus_message_ref (message);
  
  connection->n_outgoing += 1;

  _dbus_verbose ("Message %p (%s %s %s %s '%s') for %s added to outgoing queue %p, %d pending to send\n",
                 message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_path (message) ?
                 dbus_message_get_path (message) :
                 "no path",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message),
                 dbus_message_get_destination (message) ?
                 dbus_message_get_destination (message) :
                 "null",
                 connection,
                 connection->n_outgoing);

  if (dbus_message_get_serial (message) == 0)
    {
      serial = _dbus_connection_get_next_client_serial (connection);
      dbus_message_set_serial (message, serial);
      if (client_serial)
        *client_serial = serial;
    }
  else
    {
      if (client_serial)
        *client_serial = dbus_message_get_serial (message);
    }

  _dbus_verbose ("Message %p serial is %u\n",
                 message, dbus_message_get_serial (message));
  
  dbus_message_lock (message);

  /* Now we need to run an iteration to hopefully just write the messages
   * out immediately, and otherwise get them queued up
   */
  _dbus_connection_do_iteration_unlocked (connection,
                                          NULL,
                                          DBUS_ITERATION_DO_WRITING,
                                          -1);

  /* If stuff is still queued up, be sure we wake up the main loop */
  if (connection->n_outgoing > 0)
    _dbus_connection_wakeup_mainloop (connection);
}

static void
_dbus_connection_send_preallocated_and_unlock (DBusConnection       *connection,
					       DBusPreallocatedSend *preallocated,
					       DBusMessage          *message,
					       dbus_uint32_t        *client_serial)
{
  DBusDispatchStatus status;

  HAVE_LOCK_CHECK (connection);
  
  _dbus_connection_send_preallocated_unlocked_no_update (connection,
                                                         preallocated,
                                                         message, client_serial);

  _dbus_verbose ("middle\n");
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* this calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
}

/**
 * Like dbus_connection_send(), but assumes the connection
 * is already locked on function entry, and unlocks before returning.
 *
 * @param connection the connection
 * @param message the message to send
 * @param client_serial return location for client serial of sent message
 * @returns #FALSE on out-of-memory
 */
dbus_bool_t
_dbus_connection_send_and_unlock (DBusConnection *connection,
				  DBusMessage    *message,
				  dbus_uint32_t  *client_serial)
{
  DBusPreallocatedSend *preallocated;

  _dbus_assert (connection != NULL);
  _dbus_assert (message != NULL);
  
  preallocated = _dbus_connection_preallocate_send_unlocked (connection);
  if (preallocated == NULL)
    {
      CONNECTION_UNLOCK (connection);
      return FALSE;
    }

  _dbus_connection_send_preallocated_and_unlock (connection,
						 preallocated,
						 message,
						 client_serial);
  return TRUE;
}

/**
 * Used internally to handle the semantics of dbus_server_set_new_connection_function().
 * If the new connection function does not ref the connection, we want to close it.
 *
 * A bit of a hack, probably the new connection function should have returned a value
 * for whether to close, or should have had to close the connection itself if it
 * didn't want it.
 *
 * But, this works OK as long as the new connection function doesn't do anything
 * crazy like keep the connection around without ref'ing it.
 *
 * We have to lock the connection across refcount check and close in case
 * the new connection function spawns a thread that closes and unrefs.
 * In that case, if the app thread
 * closes and unrefs first, we'll harmlessly close again; if the app thread
 * still has the ref, we'll close and then the app will close harmlessly.
 * If the app unrefs without closing, the app is broken since if the
 * app refs from the new connection function it is supposed to also close.
 *
 * If we didn't atomically check the refcount and close with the lock held
 * though, we could screw this up.
 * 
 * @param connection the connection
 */
void
_dbus_connection_close_if_only_one_ref (DBusConnection *connection)
{
  dbus_int32_t refcount;

  CONNECTION_LOCK (connection);

  refcount = _dbus_atomic_get (&connection->refcount);
  /* The caller should have at least one ref */
  _dbus_assert (refcount >= 1);

  if (refcount == 1)
    _dbus_connection_close_possibly_shared_and_unlock (connection);
  else
    CONNECTION_UNLOCK (connection);
}


/**
 * When a function that blocks has been called with a timeout, and we
 * run out of memory, the time to wait for memory is based on the
 * timeout. If the caller was willing to block a long time we wait a
 * relatively long time for memory, if they were only willing to block
 * briefly then we retry for memory at a rapid rate.
 *
 * @param timeout_milliseconds the timeout requested for blocking
 */
static void
_dbus_memory_pause_based_on_timeout (int timeout_milliseconds)
{
  if (timeout_milliseconds == -1)
    _dbus_sleep_milliseconds (1000);
  else if (timeout_milliseconds < 100)
    ; /* just busy loop */
  else if (timeout_milliseconds <= 1000)
    _dbus_sleep_milliseconds (timeout_milliseconds / 3);
  else
    _dbus_sleep_milliseconds (1000);
}

static DBusMessage *
generate_local_error_message (dbus_uint32_t serial,
                              const char *error_name,
                              const char *error_msg)
{
  DBusMessage *message;
  message = dbus_message_new (DBUS_MESSAGE_TYPE_ERROR);
  if (!message)
    goto out;

  if (!dbus_message_set_error_name (message, error_name))
    {
      dbus_message_unref (message);
      message = NULL;
      goto out; 
    }

  dbus_message_set_no_reply (message, TRUE); 

  if (!dbus_message_set_reply_serial (message,
                                      serial))
    {
      dbus_message_unref (message);
      message = NULL;
      goto out;
    }

  if (error_msg != NULL)
    {
      DBusMessageIter iter;

      dbus_message_iter_init_append (message, &iter);
      if (!dbus_message_iter_append_basic (&iter,
                                           DBUS_TYPE_STRING,
                                           &error_msg))
        {
          dbus_message_unref (message);
          message = NULL;
	  goto out;
        }
    }

 out:
  return message;
}

/*
 * Peek the incoming queue to see if we got reply for a specific serial
 */
static dbus_bool_t
_dbus_connection_peek_for_reply_unlocked (DBusConnection *connection,
                                          dbus_uint32_t   client_serial)
{
  DBusList *link;
  HAVE_LOCK_CHECK (connection);

  link = _dbus_list_get_first_link (&connection->incoming_messages);

  while (link != NULL)
    {
      DBusMessage *reply = link->data;

      if (dbus_message_get_reply_serial (reply) == client_serial)
        {
          _dbus_verbose ("%s reply to %d found in queue\n", _DBUS_FUNCTION_NAME, client_serial);
          return TRUE;
        }
      link = _dbus_list_get_next_link (&connection->incoming_messages, link);
    }

  return FALSE;
}

/* This is slightly strange since we can pop a message here without
 * the dispatch lock.
 */
static DBusMessage*
check_for_reply_unlocked (DBusConnection *connection,
                          dbus_uint32_t   client_serial)
{
  DBusList *link;

  HAVE_LOCK_CHECK (connection);
  
  link = _dbus_list_get_first_link (&connection->incoming_messages);

  while (link != NULL)
    {
      DBusMessage *reply = link->data;

      if (dbus_message_get_reply_serial (reply) == client_serial)
	{
	  _dbus_list_remove_link (&connection->incoming_messages, link);
	  connection->n_incoming  -= 1;
	  return reply;
	}
      link = _dbus_list_get_next_link (&connection->incoming_messages, link);
    }

  return NULL;
}

static void
connection_timeout_and_complete_all_pending_calls_unlocked (DBusConnection *connection)
{
   /* We can't iterate over the hash in the normal way since we'll be
    * dropping the lock for each item. So we restart the
    * iter each time as we drain the hash table.
    */
   
   while (_dbus_hash_table_get_n_entries (connection->pending_replies) > 0)
    {
      DBusPendingCall *pending;
      DBusHashIter iter;
      
      _dbus_hash_iter_init (connection->pending_replies, &iter);
      _dbus_hash_iter_next (&iter);
       
      pending = _dbus_hash_iter_get_value (&iter);
      _dbus_pending_call_ref_unlocked (pending);
       
      _dbus_pending_call_queue_timeout_error_unlocked (pending, 
                                                       connection);

      if (_dbus_pending_call_is_timeout_added_unlocked (pending))
          _dbus_connection_remove_timeout_unlocked (connection,
                                                    _dbus_pending_call_get_timeout_unlocked (pending));
      _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);       
      _dbus_hash_iter_remove_entry (&iter);

      _dbus_pending_call_unref_and_unlock (pending);
      CONNECTION_LOCK (connection);
    }
  HAVE_LOCK_CHECK (connection);
}

static void
complete_pending_call_and_unlock (DBusConnection  *connection,
                                  DBusPendingCall *pending,
                                  DBusMessage     *message)
{
  _dbus_pending_call_set_reply_unlocked (pending, message);
  _dbus_pending_call_ref_unlocked (pending); /* in case there's no app with a ref held */
  _dbus_pending_call_start_completion_unlocked(pending);
  _dbus_connection_detach_pending_call_and_unlock (connection, pending);

  /* Must be called unlocked since it invokes app callback */
  _dbus_pending_call_finish_completion (pending);
  dbus_pending_call_unref (pending);
}

static dbus_bool_t
check_for_reply_and_update_dispatch_unlocked (DBusConnection  *connection,
                                              DBusPendingCall *pending)
{
  DBusMessage *reply;
  DBusDispatchStatus status;

  reply = check_for_reply_unlocked (connection, 
                                    _dbus_pending_call_get_reply_serial_unlocked (pending));
  if (reply != NULL)
    {
      _dbus_verbose ("checked for reply\n");

      _dbus_verbose ("dbus_connection_send_with_reply_and_block(): got reply\n");

      complete_pending_call_and_unlock (connection, pending, reply);
      dbus_message_unref (reply);

      CONNECTION_LOCK (connection);
      status = _dbus_connection_get_dispatch_status_unlocked (connection);
      _dbus_connection_update_dispatch_status_and_unlock (connection, status);
      dbus_pending_call_unref (pending);

      return TRUE;
    }

  return FALSE;
}

/**
 * Blocks until a pending call times out or gets a reply.
 *
 * Does not re-enter the main loop or run filter/path-registered
 * callbacks. The reply to the message will not be seen by
 * filter callbacks.
 *
 * Returns immediately if pending call already got a reply.
 * 
 * @todo could use performance improvements (it keeps scanning
 * the whole message queue for example)
 *
 * @param pending the pending call we block for a reply on
 */
void
_dbus_connection_block_pending_call (DBusPendingCall *pending)
{
  long start_tv_sec, start_tv_usec;
  long tv_sec, tv_usec;
  DBusDispatchStatus status;
  DBusConnection *connection;
  dbus_uint32_t client_serial;
  DBusTimeout *timeout;
  int timeout_milliseconds, elapsed_milliseconds;

  _dbus_assert (pending != NULL);

  if (dbus_pending_call_get_completed (pending))
    return;

  dbus_pending_call_ref (pending); /* necessary because the call could be canceled */

  connection = _dbus_pending_call_get_connection_and_lock (pending);
  
  /* Flush message queue - note, can affect dispatch status */
  _dbus_connection_flush_unlocked (connection);

  client_serial = _dbus_pending_call_get_reply_serial_unlocked (pending);

  /* note that timeout_milliseconds is limited to a smallish value
   * in _dbus_pending_call_new() so overflows aren't possible
   * below
   */
  timeout = _dbus_pending_call_get_timeout_unlocked (pending);
  _dbus_get_monotonic_time (&start_tv_sec, &start_tv_usec);
  if (timeout)
    {
      timeout_milliseconds = dbus_timeout_get_interval (timeout);

      _dbus_verbose ("dbus_connection_send_with_reply_and_block(): will block %d milliseconds for reply serial %u from %ld sec %ld usec\n",
                     timeout_milliseconds,
                     client_serial,
                     start_tv_sec, start_tv_usec);
    }
  else
    {
      timeout_milliseconds = -1;

      _dbus_verbose ("dbus_connection_send_with_reply_and_block(): will block for reply serial %u\n", client_serial);
    }

  /* check to see if we already got the data off the socket */
  /* from another blocked pending call */
  if (check_for_reply_and_update_dispatch_unlocked (connection, pending))
    return;

  /* Now we wait... */
  /* always block at least once as we know we don't have the reply yet */
  _dbus_connection_do_iteration_unlocked (connection,
                                          pending,
                                          DBUS_ITERATION_DO_READING |
                                          DBUS_ITERATION_BLOCK,
                                          timeout_milliseconds);

 recheck_status:

  _dbus_verbose ("top of recheck\n");
  
  HAVE_LOCK_CHECK (connection);
  
  /* queue messages and get status */

  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* the get_completed() is in case a dispatch() while we were blocking
   * got the reply instead of us.
   */
  if (_dbus_pending_call_get_completed_unlocked (pending))
    {
      _dbus_verbose ("Pending call completed by dispatch\n");
      _dbus_connection_update_dispatch_status_and_unlock (connection, status);
      dbus_pending_call_unref (pending);
      return;
    }
  
  if (status == DBUS_DISPATCH_DATA_REMAINS)
    {
      if (check_for_reply_and_update_dispatch_unlocked (connection, pending))
        return;
    }
  
  _dbus_get_monotonic_time (&tv_sec, &tv_usec);
  elapsed_milliseconds = (tv_sec - start_tv_sec) * 1000 +
	  (tv_usec - start_tv_usec) / 1000;
  
  if (!_dbus_connection_get_is_connected_unlocked (connection))
    {
      DBusMessage *error_msg;

      error_msg = generate_local_error_message (client_serial,
                                                DBUS_ERROR_DISCONNECTED, 
                                                "Connection was disconnected before a reply was received"); 

      /* on OOM error_msg is set to NULL */
      complete_pending_call_and_unlock (connection, pending, error_msg);
      if (error_msg != NULL)
        dbus_message_unref (error_msg);
      dbus_pending_call_unref (pending);
      return;
    }
  else if (connection->disconnect_message_link == NULL)
    _dbus_verbose ("dbus_connection_send_with_reply_and_block(): disconnected\n");
  else if (timeout == NULL)
    {
       if (status == DBUS_DISPATCH_NEED_MEMORY)
        {
          /* Try sleeping a bit, as we aren't sure we need to block for reading,
           * we may already have a reply in the buffer and just can't process
           * it.
           */
          _dbus_verbose ("dbus_connection_send_with_reply_and_block() waiting for more memory\n");

          _dbus_memory_pause_based_on_timeout (timeout_milliseconds - elapsed_milliseconds);
        }
      else
        {          
          /* block again, we don't have the reply buffered yet. */
          _dbus_connection_do_iteration_unlocked (connection,
                                                  pending,
                                                  DBUS_ITERATION_DO_READING |
                                                  DBUS_ITERATION_BLOCK,
                                                  timeout_milliseconds - elapsed_milliseconds);
        }

      goto recheck_status;
    }
  else if (tv_sec < start_tv_sec)
    _dbus_verbose ("dbus_connection_send_with_reply_and_block(): clock set backward\n");
  else if (elapsed_milliseconds < timeout_milliseconds)
    {
      _dbus_verbose ("dbus_connection_send_with_reply_and_block(): %d milliseconds remain\n", timeout_milliseconds - elapsed_milliseconds);
      
      if (status == DBUS_DISPATCH_NEED_MEMORY)
        {
          /* Try sleeping a bit, as we aren't sure we need to block for reading,
           * we may already have a reply in the buffer and just can't process
           * it.
           */
          _dbus_verbose ("dbus_connection_send_with_reply_and_block() waiting for more memory\n");

          _dbus_memory_pause_based_on_timeout (timeout_milliseconds - elapsed_milliseconds);
        }
      else
        {          
          /* block again, we don't have the reply buffered yet. */
          _dbus_connection_do_iteration_unlocked (connection,
                                                  pending,
                                                  DBUS_ITERATION_DO_READING |
                                                  DBUS_ITERATION_BLOCK,
                                                  timeout_milliseconds - elapsed_milliseconds);
        }

      goto recheck_status;
    }

  _dbus_verbose ("dbus_connection_send_with_reply_and_block(): Waited %d milliseconds and got no reply\n",
                 elapsed_milliseconds);

  _dbus_assert (!_dbus_pending_call_get_completed_unlocked (pending));
  
  /* unlock and call user code */
  complete_pending_call_and_unlock (connection, pending, NULL);

  /* update user code on dispatch status */
  CONNECTION_LOCK (connection);
  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
  dbus_pending_call_unref (pending);
}

/**
 * Return how many file descriptors are pending in the loader
 *
 * @param connection the connection
 */
int
_dbus_connection_get_pending_fds_count (DBusConnection *connection)
{
  return _dbus_transport_get_pending_fds_count (connection->transport);
}

/**
 * Register a function to be called whenever the number of pending file
 * descriptors in the loader change.
 *
 * @param connection the connection
 * @param callback the callback
 */
void
_dbus_connection_set_pending_fds_function (DBusConnection *connection,
                                           DBusPendingFdsChangeFunction callback,
                                           void *data)
{
  _dbus_transport_set_pending_fds_function (connection->transport,
                                            callback, data);
}

/** @} */

/**
 * @addtogroup DBusConnection
 *
 * @{
 */

/**
 * Gets a connection to a remote address. If a connection to the given
 * address already exists, returns the existing connection with its
 * reference count incremented.  Otherwise, returns a new connection
 * and saves the new connection for possible re-use if a future call
 * to dbus_connection_open() asks to connect to the same server.
 *
 * Use dbus_connection_open_private() to get a dedicated connection
 * not shared with other callers of dbus_connection_open().
 *
 * If the open fails, the function returns #NULL, and provides a
 * reason for the failure in the error parameter. Pass #NULL for the
 * error parameter if you aren't interested in the reason for
 * failure.
 *
 * Because this connection is shared, no user of the connection
 * may call dbus_connection_close(). However, when you are done with the
 * connection you should call dbus_connection_unref().
 *
 * @note Prefer dbus_connection_open() to dbus_connection_open_private()
 * unless you have good reason; connections are expensive enough
 * that it's wasteful to create lots of connections to the same
 * server.
 * 
 * @param address the address.
 * @param error address where an error can be returned.
 * @returns new connection, or #NULL on failure.
 */
DBusConnection*
dbus_connection_open (const char     *address,
                      DBusError      *error)
{
  DBusConnection *connection;

  _dbus_return_val_if_fail (address != NULL, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);

  connection = _dbus_connection_open_internal (address,
                                               TRUE,
                                               error);

  return connection;
}

/**
 * Opens a new, dedicated connection to a remote address. Unlike
 * dbus_connection_open(), always creates a new connection.
 * This connection will not be saved or recycled by libdbus.
 *
 * If the open fails, the function returns #NULL, and provides a
 * reason for the failure in the error parameter. Pass #NULL for the
 * error parameter if you aren't interested in the reason for
 * failure.
 *
 * When you are done with this connection, you must
 * dbus_connection_close() to disconnect it,
 * and dbus_connection_unref() to free the connection object.
 * 
 * (The dbus_connection_close() can be skipped if the
 * connection is already known to be disconnected, for example
 * if you are inside a handler for the Disconnected signal.)
 *
 * @note Prefer dbus_connection_open() to dbus_connection_open_private()
 * unless you have good reason; connections are expensive enough
 * that it's wasteful to create lots of connections to the same
 * server.
 *
 * @param address the address.
 * @param error address where an error can be returned.
 * @returns new connection, or #NULL on failure.
 */
DBusConnection*
dbus_connection_open_private (const char     *address,
                              DBusError      *error)
{
  DBusConnection *connection;

  _dbus_return_val_if_fail (address != NULL, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);

  connection = _dbus_connection_open_internal (address,
                                               FALSE,
                                               error);

  return connection;
}

/**
 * Increments the reference count of a DBusConnection.
 *
 * @param connection the connection.
 * @returns the connection.
 */
DBusConnection *
dbus_connection_ref (DBusConnection *connection)
{
  dbus_int32_t old_refcount;

  _dbus_return_val_if_fail (connection != NULL, NULL);
  _dbus_return_val_if_fail (connection->generation == _dbus_current_generation, NULL);
  old_refcount = _dbus_atomic_inc (&connection->refcount);
  _dbus_connection_trace_ref (connection, old_refcount, old_refcount + 1,
      "ref");

  return connection;
}

static void
free_outgoing_message (void *element,
                       void *data)
{
  DBusMessage *message = element;
  DBusConnection *connection = data;

  _dbus_message_remove_counter (message, connection->outgoing_counter);
  dbus_message_unref (message);
}

/* This is run without the mutex held, but after the last reference
 * to the connection has been dropped we should have no thread-related
 * problems
 */
static void
_dbus_connection_last_unref (DBusConnection *connection)
{
  DBusList *link;

  _dbus_verbose ("Finalizing connection %p\n", connection);

  _dbus_assert (_dbus_atomic_get (&connection->refcount) == 0);

  /* You have to disconnect the connection before unref:ing it. Otherwise
   * you won't get the disconnected message.
   */
  _dbus_assert (!_dbus_transport_get_is_connected (connection->transport));
  _dbus_assert (connection->server_guid == NULL);
  
  /* ---- We're going to call various application callbacks here, hope it doesn't break anything... */
  _dbus_object_tree_free_all_unlocked (connection->objects);
  
  dbus_connection_set_dispatch_status_function (connection, NULL, NULL, NULL);
  dbus_connection_set_wakeup_main_function (connection, NULL, NULL, NULL);
  dbus_connection_set_unix_user_function (connection, NULL, NULL, NULL);
  dbus_connection_set_windows_user_function (connection, NULL, NULL, NULL);
  
  _dbus_watch_list_free (connection->watches);
  connection->watches = NULL;
  
  _dbus_timeout_list_free (connection->timeouts);
  connection->timeouts = NULL;

  _dbus_data_slot_list_free (&connection->slot_list);
  
  link = _dbus_list_get_first_link (&connection->filter_list);
  while (link != NULL)
    {
      DBusMessageFilter *filter = link->data;
      DBusList *next = _dbus_list_get_next_link (&connection->filter_list, link);

      filter->function = NULL;
      _dbus_message_filter_unref (filter); /* calls app callback */
      link->data = NULL;
      
      link = next;
    }
  _dbus_list_clear (&connection->filter_list);
  
  /* ---- Done with stuff that invokes application callbacks */

  _dbus_object_tree_unref (connection->objects);  

  _dbus_hash_table_unref (connection->pending_replies);
  connection->pending_replies = NULL;

  _dbus_list_foreach (&connection->outgoing_messages,
                      free_outgoing_message,
		      connection);
  _dbus_list_clear (&connection->outgoing_messages);

  _dbus_list_clear_full (&connection->incoming_messages,
                         (DBusFreeFunction) dbus_message_unref);

  _dbus_counter_unref (connection->outgoing_counter);

  _dbus_transport_unref (connection->transport);

  if (connection->disconnect_message_link)
    {
      DBusMessage *message = connection->disconnect_message_link->data;
      dbus_message_unref (message);
      _dbus_list_free_link (connection->disconnect_message_link);
    }

  _dbus_condvar_free_at_location (&connection->dispatch_cond);
  _dbus_condvar_free_at_location (&connection->io_path_cond);

  _dbus_cmutex_free_at_location (&connection->io_path_mutex);
  _dbus_cmutex_free_at_location (&connection->dispatch_mutex);

  _dbus_rmutex_free_at_location (&connection->slot_mutex);

  _dbus_rmutex_free_at_location (&connection->mutex);
  
  dbus_free (connection);
}

/**
 * Decrements the reference count of a DBusConnection, and finalizes
 * it if the count reaches zero.
 *
 * Note: it is a bug to drop the last reference to a connection that
 * is still connected.
 *
 * For shared connections, libdbus will own a reference
 * as long as the connection is connected, so you can know that either
 * you don't have the last reference, or it's OK to drop the last reference.
 * Most connections are shared. dbus_connection_open() and dbus_bus_get()
 * return shared connections.
 *
 * For private connections, the creator of the connection must arrange for
 * dbus_connection_close() to be called prior to dropping the last reference.
 * Private connections come from dbus_connection_open_private() or dbus_bus_get_private().
 *
 * @param connection the connection.
 */
void
dbus_connection_unref (DBusConnection *connection)
{
  dbus_int32_t old_refcount;

  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (connection->generation == _dbus_current_generation);

  old_refcount = _dbus_atomic_dec (&connection->refcount);

  _dbus_connection_trace_ref (connection, old_refcount, old_refcount - 1,
      "unref");

  if (old_refcount == 1)
    {
#ifndef DBUS_DISABLE_CHECKS
      if (_dbus_transport_get_is_connected (connection->transport))
        {
          _dbus_warn_check_failed ("The last reference on a connection was dropped without closing the connection. This is a bug in an application. See dbus_connection_unref() documentation for details.\n%s",
                                   connection->shareable ?
                                   "Most likely, the application called unref() too many times and removed a reference belonging to libdbus, since this is a shared connection." :
                                    "Most likely, the application was supposed to call dbus_connection_close(), since this is a private connection.");
          return;
        }
#endif
      _dbus_connection_last_unref (connection);
    }
}

/*
 * Note that the transport can disconnect itself (other end drops us)
 * and in that case this function never runs. So this function must
 * not do anything more than disconnect the transport and update the
 * dispatch status.
 * 
 * If the transport self-disconnects, then we assume someone will
 * dispatch the connection to cause the dispatch status update.
 */
static void
_dbus_connection_close_possibly_shared_and_unlock (DBusConnection *connection)
{
  DBusDispatchStatus status;

  HAVE_LOCK_CHECK (connection);
  
  _dbus_verbose ("Disconnecting %p\n", connection);

  /* We need to ref because update_dispatch_status_and_unlock will unref
   * the connection if it was shared and libdbus was the only remaining
   * refcount holder.
   */
  _dbus_connection_ref_unlocked (connection);
  
  _dbus_transport_disconnect (connection->transport);

  /* This has the side effect of queuing the disconnect message link
   * (unless we don't have enough memory, possibly, so don't assert it).
   * After the disconnect message link is queued, dbus_bus_get/dbus_connection_open
   * should never again return the newly-disconnected connection.
   *
   * However, we only unref the shared connection and exit_on_disconnect when
   * the disconnect message reaches the head of the message queue,
   * NOT when it's first queued.
   */
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* This calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);

  /* Could also call out to user code */
  dbus_connection_unref (connection);
}

/**
 * Closes a private connection, so no further data can be sent or received.
 * This disconnects the transport (such as a socket) underlying the
 * connection.
 *
 * Attempts to send messages after closing a connection are safe, but will result in
 * error replies generated locally in libdbus.
 * 
 * This function does not affect the connection's reference count.  It's
 * safe to close a connection more than once; all calls after the
 * first do nothing. It's impossible to "reopen" a connection, a
 * new connection must be created. This function may result in a call
 * to the DBusDispatchStatusFunction set with
 * dbus_connection_set_dispatch_status_function(), as the disconnect
 * message it generates needs to be dispatched.
 *
 * If a connection is dropped by the remote application, it will
 * close itself. 
 * 
 * You must close a connection prior to releasing the last reference to
 * the connection. If you dbus_connection_unref() for the last time
 * without closing the connection, the results are undefined; it
 * is a bug in your program and libdbus will try to print a warning.
 *
 * You may not close a shared connection. Connections created with
 * dbus_connection_open() or dbus_bus_get() are shared.
 * These connections are owned by libdbus, and applications should
 * only unref them, never close them. Applications can know it is
 * safe to unref these connections because libdbus will be holding a
 * reference as long as the connection is open. Thus, either the
 * connection is closed and it is OK to drop the last reference,
 * or the connection is open and the app knows it does not have the
 * last reference.
 *
 * Connections created with dbus_connection_open_private() or
 * dbus_bus_get_private() are not kept track of or referenced by
 * libdbus. The creator of these connections is responsible for
 * calling dbus_connection_close() prior to releasing the last
 * reference, if the connection is not already disconnected.
 *
 * @param connection the private (unshared) connection to close
 */
void
dbus_connection_close (DBusConnection *connection)
{
  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (connection->generation == _dbus_current_generation);

  CONNECTION_LOCK (connection);

#ifndef DBUS_DISABLE_CHECKS
  if (connection->shareable)
    {
      CONNECTION_UNLOCK (connection);

      _dbus_warn_check_failed ("Applications must not close shared connections - see dbus_connection_close() docs. This is a bug in the application.");
      return;
    }
#endif
  
  _dbus_connection_close_possibly_shared_and_unlock (connection);
}

static dbus_bool_t
_dbus_connection_get_is_connected_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  return _dbus_transport_get_is_connected (connection->transport);
}

/**
 * Gets whether the connection is currently open.  A connection may
 * become disconnected when the remote application closes its end, or
 * exits; a connection may also be disconnected with
 * dbus_connection_close().
 * 
 * There are not separate states for "closed" and "disconnected," the two
 * terms are synonymous. This function should really be called
 * get_is_open() but for historical reasons is not.
 *
 * @param connection the connection.
 * @returns #TRUE if the connection is still alive.
 */
dbus_bool_t
dbus_connection_get_is_connected (DBusConnection *connection)
{
  dbus_bool_t res;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  
  CONNECTION_LOCK (connection);
  res = _dbus_connection_get_is_connected_unlocked (connection);
  CONNECTION_UNLOCK (connection);
  
  return res;
}

/**
 * Gets whether the connection was authenticated. (Note that
 * if the connection was authenticated then disconnected,
 * this function still returns #TRUE)
 *
 * @param connection the connection
 * @returns #TRUE if the connection was ever authenticated
 */
dbus_bool_t
dbus_connection_get_is_authenticated (DBusConnection *connection)
{
  dbus_bool_t res;

  _dbus_return_val_if_fail (connection != NULL, FALSE);

  CONNECTION_LOCK (connection);
  res = _dbus_transport_try_to_authenticate (connection->transport);
  CONNECTION_UNLOCK (connection);
  
  return res;
}

/**
 * Gets whether the connection is not authenticated as a specific
 * user.  If the connection is not authenticated, this function
 * returns #TRUE, and if it is authenticated but as an anonymous user,
 * it returns #TRUE.  If it is authenticated as a specific user, then
 * this returns #FALSE. (Note that if the connection was authenticated
 * as anonymous then disconnected, this function still returns #TRUE.)
 *
 * If the connection is not anonymous, you can use
 * dbus_connection_get_unix_user() and
 * dbus_connection_get_windows_user() to see who it's authorized as.
 *
 * If you want to prevent non-anonymous authorization, use
 * dbus_server_set_auth_mechanisms() to remove the mechanisms that
 * allow proving user identity (i.e. only allow the ANONYMOUS
 * mechanism).
 * 
 * @param connection the connection
 * @returns #TRUE if not authenticated or authenticated as anonymous 
 */
dbus_bool_t
dbus_connection_get_is_anonymous (DBusConnection *connection)
{
  dbus_bool_t res;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  
  CONNECTION_LOCK (connection);
  res = _dbus_transport_get_is_anonymous (connection->transport);
  CONNECTION_UNLOCK (connection);
  
  return res;
}

/**
 * Gets the ID of the server address we are authenticated to, if this
 * connection is on the client side. If the connection is on the
 * server side, this will always return #NULL - use dbus_server_get_id()
 * to get the ID of your own server, if you are the server side.
 * 
 * If a client-side connection is not authenticated yet, the ID may be
 * available if it was included in the server address, but may not be
 * available. The only way to be sure the server ID is available
 * is to wait for authentication to complete.
 *
 * In general, each mode of connecting to a given server will have
 * its own ID. So for example, if the session bus daemon is listening
 * on UNIX domain sockets and on TCP, then each of those modalities
 * will have its own server ID.
 *
 * If you want an ID that identifies an entire session bus, look at
 * dbus_bus_get_id() instead (which is just a convenience wrapper
 * around the org.freedesktop.DBus.GetId method invoked on the bus).
 *
 * You can also get a machine ID; see dbus_try_get_local_machine_id() to
 * get the machine you are on.  There isn't a convenience wrapper, but
 * you can invoke org.freedesktop.DBus.Peer.GetMachineId on any peer
 * to get the machine ID on the other end.
 * 
 * The D-Bus specification describes the server ID and other IDs in a
 * bit more detail.
 *
 * @param connection the connection
 * @returns the server ID or #NULL if no memory or the connection is server-side
 */
char*
dbus_connection_get_server_id (DBusConnection *connection)
{
  char *id;

  _dbus_return_val_if_fail (connection != NULL, NULL);

  CONNECTION_LOCK (connection);
  id = _dbus_strdup (_dbus_transport_get_server_id (connection->transport));
  CONNECTION_UNLOCK (connection);

  return id;
}

/**
 * Tests whether a certain type can be send via the connection. This
 * will always return TRUE for all types, with the exception of
 * DBUS_TYPE_UNIX_FD. The function will return TRUE for
 * DBUS_TYPE_UNIX_FD only on systems that know Unix file descriptors
 * and can send them via the chosen transport and when the remote side
 * supports this.
 *
 * This function can be used to do runtime checking for types that
 * might be unknown to the specific D-Bus client implementation
 * version, i.e. it will return FALSE for all types this
 * implementation does not know, including invalid or reserved types.
 *
 * @param connection the connection
 * @param type the type to check
 * @returns TRUE if the type may be send via the connection
 */
dbus_bool_t
dbus_connection_can_send_type(DBusConnection *connection,
                                  int type)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);

  if (!dbus_type_is_valid (type))
    return FALSE;

  if (type != DBUS_TYPE_UNIX_FD)
    return TRUE;

#ifdef HAVE_UNIX_FD_PASSING
  {
    dbus_bool_t b;

    CONNECTION_LOCK(connection);
    b = _dbus_transport_can_pass_unix_fd(connection->transport);
    CONNECTION_UNLOCK(connection);

    return b;
  }
#endif

  return FALSE;
}

/**
 * Set whether _exit() should be called when the connection receives a
 * disconnect signal. The call to _exit() comes after any handlers for
 * the disconnect signal run; handlers can cancel the exit by calling
 * this function.
 *
 * By default, exit_on_disconnect is #FALSE; but for message bus
 * connections returned from dbus_bus_get() it will be toggled on
 * by default.
 *
 * @param connection the connection
 * @param exit_on_disconnect #TRUE if _exit() should be called after a disconnect signal
 */
void
dbus_connection_set_exit_on_disconnect (DBusConnection *connection,
                                        dbus_bool_t     exit_on_disconnect)
{
  _dbus_return_if_fail (connection != NULL);

  CONNECTION_LOCK (connection);
  connection->exit_on_disconnect = exit_on_disconnect != FALSE;
  CONNECTION_UNLOCK (connection);
}

/**
 * Preallocates resources needed to send a message, allowing the message 
 * to be sent without the possibility of memory allocation failure.
 * Allows apps to create a future guarantee that they can send
 * a message regardless of memory shortages.
 *
 * @param connection the connection we're preallocating for.
 * @returns the preallocated resources, or #NULL
 */
DBusPreallocatedSend*
dbus_connection_preallocate_send (DBusConnection *connection)
{
  DBusPreallocatedSend *preallocated;

  _dbus_return_val_if_fail (connection != NULL, NULL);

  CONNECTION_LOCK (connection);
  
  preallocated =
    _dbus_connection_preallocate_send_unlocked (connection);

  CONNECTION_UNLOCK (connection);

  return preallocated;
}

/**
 * Frees preallocated message-sending resources from
 * dbus_connection_preallocate_send(). Should only
 * be called if the preallocated resources are not used
 * to send a message.
 *
 * @param connection the connection
 * @param preallocated the resources
 */
void
dbus_connection_free_preallocated_send (DBusConnection       *connection,
                                        DBusPreallocatedSend *preallocated)
{
  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (preallocated != NULL);  
  _dbus_return_if_fail (connection == preallocated->connection);

  _dbus_list_free_link (preallocated->queue_link);
  _dbus_counter_unref (preallocated->counter_link->data);
  _dbus_list_free_link (preallocated->counter_link);
  dbus_free (preallocated);
}

/**
 * Sends a message using preallocated resources. This function cannot fail.
 * It works identically to dbus_connection_send() in other respects.
 * Preallocated resources comes from dbus_connection_preallocate_send().
 * This function "consumes" the preallocated resources, they need not
 * be freed separately.
 *
 * @param connection the connection
 * @param preallocated the preallocated resources
 * @param message the message to send
 * @param client_serial return location for client serial assigned to the message
 */
void
dbus_connection_send_preallocated (DBusConnection       *connection,
                                   DBusPreallocatedSend *preallocated,
                                   DBusMessage          *message,
                                   dbus_uint32_t        *client_serial)
{
  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (preallocated != NULL);
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (preallocated->connection == connection);
  _dbus_return_if_fail (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_METHOD_CALL ||
                        dbus_message_get_member (message) != NULL);
  _dbus_return_if_fail (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_SIGNAL ||
                        (dbus_message_get_interface (message) != NULL &&
                         dbus_message_get_member (message) != NULL));

  CONNECTION_LOCK (connection);

#ifdef HAVE_UNIX_FD_PASSING

  if (!_dbus_transport_can_pass_unix_fd(connection->transport) &&
      message->n_unix_fds > 0)
    {
      /* Refuse to send fds on a connection that cannot handle
         them. Unfortunately we cannot return a proper error here, so
         the best we can is just return. */
      CONNECTION_UNLOCK (connection);
      return;
    }

#endif

  _dbus_connection_send_preallocated_and_unlock (connection,
						 preallocated,
						 message, client_serial);
}

static dbus_bool_t
_dbus_connection_send_unlocked_no_update (DBusConnection *connection,
                                          DBusMessage    *message,
                                          dbus_uint32_t  *client_serial)
{
  DBusPreallocatedSend *preallocated;

  _dbus_assert (connection != NULL);
  _dbus_assert (message != NULL);
  
  preallocated = _dbus_connection_preallocate_send_unlocked (connection);
  if (preallocated == NULL)
    return FALSE;

  _dbus_connection_send_preallocated_unlocked_no_update (connection,
                                                         preallocated,
                                                         message,
                                                         client_serial);
  return TRUE;
}

/**
 * Adds a message to the outgoing message queue. Does not block to
 * write the message to the network; that happens asynchronously. To
 * force the message to be written, call dbus_connection_flush() however
 * it is not necessary to call dbus_connection_flush() by hand; the 
 * message will be sent the next time the main loop is run. 
 * dbus_connection_flush() should only be used, for example, if
 * the application was expected to exit before running the main loop.
 *
 * Because this only queues the message, the only reason it can
 * fail is lack of memory. Even if the connection is disconnected,
 * no error will be returned. If the function fails due to lack of memory, 
 * it returns #FALSE. The function will never fail for other reasons; even 
 * if the connection is disconnected, you can queue an outgoing message,
 * though obviously it won't be sent.
 *
 * The message serial is used by the remote application to send a
 * reply; see dbus_message_get_serial() or the D-Bus specification.
 *
 * dbus_message_unref() can be called as soon as this method returns
 * as the message queue will hold its own ref until the message is sent.
 * 
 * @param connection the connection.
 * @param message the message to write.
 * @param serial return location for message serial, or #NULL if you don't care
 * @returns #TRUE on success.
 */
dbus_bool_t
dbus_connection_send (DBusConnection *connection,
                      DBusMessage    *message,
                      dbus_uint32_t  *serial)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (message != NULL, FALSE);

  CONNECTION_LOCK (connection);

#ifdef HAVE_UNIX_FD_PASSING

  if (!_dbus_transport_can_pass_unix_fd(connection->transport) &&
      message->n_unix_fds > 0)
    {
      /* Refuse to send fds on a connection that cannot handle
         them. Unfortunately we cannot return a proper error here, so
         the best we can is just return. */
      CONNECTION_UNLOCK (connection);
      return FALSE;
    }

#endif

  return _dbus_connection_send_and_unlock (connection,
					   message,
					   serial);
}

static dbus_bool_t
reply_handler_timeout (void *data)
{
  DBusConnection *connection;
  DBusDispatchStatus status;
  DBusPendingCall *pending = data;

  connection = _dbus_pending_call_get_connection_and_lock (pending);
  _dbus_connection_ref_unlocked (connection);

  _dbus_pending_call_queue_timeout_error_unlocked (pending, 
                                                   connection);
  _dbus_connection_remove_timeout_unlocked (connection,
				            _dbus_pending_call_get_timeout_unlocked (pending));
  _dbus_pending_call_set_timeout_added_unlocked (pending, FALSE);

  _dbus_verbose ("middle\n");
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* Unlocks, and calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
  dbus_connection_unref (connection);
  
  return TRUE;
}

/**
 * Queues a message to send, as with dbus_connection_send(),
 * but also returns a #DBusPendingCall used to receive a reply to the
 * message. If no reply is received in the given timeout_milliseconds,
 * this function expires the pending reply and generates a synthetic
 * error reply (generated in-process, not by the remote application)
 * indicating that a timeout occurred.
 *
 * A #DBusPendingCall will see a reply message before any filters or
 * registered object path handlers. See dbus_connection_dispatch() for
 * details on when handlers are run.
 *
 * A #DBusPendingCall will always see exactly one reply message,
 * unless it's cancelled with dbus_pending_call_cancel().
 * 
 * If #NULL is passed for the pending_return, the #DBusPendingCall
 * will still be generated internally, and used to track
 * the message reply timeout. This means a timeout error will
 * occur if no reply arrives, unlike with dbus_connection_send().
 *
 * If -1 is passed for the timeout, a sane default timeout is used. -1
 * is typically the best value for the timeout for this reason, unless
 * you want a very short or very long timeout.  If #DBUS_TIMEOUT_INFINITE is
 * passed for the timeout, no timeout will be set and the call will block
 * forever.
 *
 * @warning if the connection is disconnected or you try to send Unix
 * file descriptors on a connection that does not support them, the
 * #DBusPendingCall will be set to #NULL, so be careful with this.
 *
 * @param connection the connection
 * @param message the message to send
 * @param pending_return return location for a #DBusPendingCall
 * object, or #NULL if connection is disconnected or when you try to
 * send Unix file descriptors on a connection that does not support
 * them.
 * @param timeout_milliseconds timeout in milliseconds, -1 (or
 *  #DBUS_TIMEOUT_USE_DEFAULT) for default or #DBUS_TIMEOUT_INFINITE for no
 *  timeout
 * @returns #FALSE if no memory, #TRUE otherwise.
 *
 */
dbus_bool_t
dbus_connection_send_with_reply (DBusConnection     *connection,
                                 DBusMessage        *message,
                                 DBusPendingCall   **pending_return,
                                 int                 timeout_milliseconds)
{
  DBusPendingCall *pending;
  dbus_int32_t serial = -1;
  DBusDispatchStatus status;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (message != NULL, FALSE);
  _dbus_return_val_if_fail (timeout_milliseconds >= 0 || timeout_milliseconds == -1, FALSE);

  if (pending_return)
    *pending_return = NULL;

  CONNECTION_LOCK (connection);

#ifdef HAVE_UNIX_FD_PASSING

  if (!_dbus_transport_can_pass_unix_fd(connection->transport) &&
      message->n_unix_fds > 0)
    {
      /* Refuse to send fds on a connection that cannot handle
         them. Unfortunately we cannot return a proper error here, so
         the best we can do is return TRUE but leave *pending_return
         as NULL. */
      CONNECTION_UNLOCK (connection);
      return TRUE;
    }

#endif

   if (!_dbus_connection_get_is_connected_unlocked (connection))
    {
      CONNECTION_UNLOCK (connection);

      return TRUE;
    }

  pending = _dbus_pending_call_new_unlocked (connection,
                                             timeout_milliseconds,
                                             reply_handler_timeout);

  if (pending == NULL)
    {
      CONNECTION_UNLOCK (connection);
      return FALSE;
    }

  /* Assign a serial to the message */
  serial = dbus_message_get_serial (message);
  if (serial == 0)
    {
      serial = _dbus_connection_get_next_client_serial (connection);
      dbus_message_set_serial (message, serial);
    }

  if (!_dbus_pending_call_set_timeout_error_unlocked (pending, message, serial))
    goto error;
    
  /* Insert the serial in the pending replies hash;
   * hash takes a refcount on DBusPendingCall.
   * Also, add the timeout.
   */
  if (!_dbus_connection_attach_pending_call_unlocked (connection,
						      pending))
    goto error;
 
  if (!_dbus_connection_send_unlocked_no_update (connection, message, NULL))
    {
      _dbus_connection_detach_pending_call_and_unlock (connection,
						       pending);
      goto error_unlocked;
    }

  if (pending_return)
    *pending_return = pending; /* hand off refcount */
  else
    {
      _dbus_connection_detach_pending_call_unlocked (connection, pending);
      /* we still have a ref to the pending call in this case, we unref
       * after unlocking, below
       */
    }

  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* this calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);

  if (pending_return == NULL)
    dbus_pending_call_unref (pending);
  
  return TRUE;

 error:
  CONNECTION_UNLOCK (connection);
 error_unlocked:
  dbus_pending_call_unref (pending);
  return FALSE;
}

/**
 * Sends a message and blocks a certain time period while waiting for
 * a reply.  This function does not reenter the main loop,
 * i.e. messages other than the reply are queued up but not
 * processed. This function is used to invoke method calls on a
 * remote object.
 * 
 * If a normal reply is received, it is returned, and removed from the
 * incoming message queue. If it is not received, #NULL is returned
 * and the error is set to #DBUS_ERROR_NO_REPLY.  If an error reply is
 * received, it is converted to a #DBusError and returned as an error,
 * then the reply message is deleted and #NULL is returned. If
 * something else goes wrong, result is set to whatever is
 * appropriate, such as #DBUS_ERROR_NO_MEMORY or
 * #DBUS_ERROR_DISCONNECTED.
 *
 * @warning While this function blocks the calling thread will not be
 * processing the incoming message queue. This means you can end up
 * deadlocked if the application you're talking to needs you to reply
 * to a method. To solve this, either avoid the situation, block in a
 * separate thread from the main connection-dispatching thread, or use
 * dbus_pending_call_set_notify() to avoid blocking.
 *
 * @param connection the connection
 * @param message the message to send
 * @param timeout_milliseconds timeout in milliseconds, -1 (or
 *  #DBUS_TIMEOUT_USE_DEFAULT) for default or #DBUS_TIMEOUT_INFINITE for no
 *  timeout
 * @param error return location for error message
 * @returns the message that is the reply or #NULL with an error code if the
 * function fails.
 */
DBusMessage*
dbus_connection_send_with_reply_and_block (DBusConnection     *connection,
                                           DBusMessage        *message,
                                           int                 timeout_milliseconds,
                                           DBusError          *error)
{
  DBusMessage *reply;
  DBusPendingCall *pending;

  _dbus_return_val_if_fail (connection != NULL, NULL);
  _dbus_return_val_if_fail (message != NULL, NULL);
  _dbus_return_val_if_fail (timeout_milliseconds >= 0 || timeout_milliseconds == -1, NULL);
  _dbus_return_val_if_error_is_set (error, NULL);

#ifdef HAVE_UNIX_FD_PASSING

  CONNECTION_LOCK (connection);
  if (!_dbus_transport_can_pass_unix_fd(connection->transport) &&
      message->n_unix_fds > 0)
    {
      CONNECTION_UNLOCK (connection);
      dbus_set_error(error, DBUS_ERROR_FAILED, "Cannot send file descriptors on this connection.");
      return NULL;
    }
  CONNECTION_UNLOCK (connection);

#endif

  if (!dbus_connection_send_with_reply (connection, message,
                                        &pending, timeout_milliseconds))
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  if (pending == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_DISCONNECTED, "Connection is closed");
      return NULL;
    }
  
  dbus_pending_call_block (pending);

  reply = dbus_pending_call_steal_reply (pending);
  dbus_pending_call_unref (pending);

  /* call_complete_and_unlock() called from pending_call_block() should
   * always fill this in.
   */
  _dbus_assert (reply != NULL);
  
   if (dbus_set_error_from_message (error, reply))
    {
      dbus_message_unref (reply);
      return NULL;
    }
  else
    return reply;
}

/**
 * Blocks until the outgoing message queue is empty.
 * Assumes connection lock already held.
 *
 * If you call this, you MUST call update_dispatch_status afterword...
 * 
 * @param connection the connection.
 */
static DBusDispatchStatus
_dbus_connection_flush_unlocked (DBusConnection *connection)
{
  /* We have to specify DBUS_ITERATION_DO_READING here because
   * otherwise we could have two apps deadlock if they are both doing
   * a flush(), and the kernel buffers fill up. This could change the
   * dispatch status.
   */
  DBusDispatchStatus status;

  HAVE_LOCK_CHECK (connection);
  
  while (connection->n_outgoing > 0 &&
         _dbus_connection_get_is_connected_unlocked (connection))
    {
      _dbus_verbose ("doing iteration in\n");
      HAVE_LOCK_CHECK (connection);
      _dbus_connection_do_iteration_unlocked (connection,
                                              NULL,
                                              DBUS_ITERATION_DO_READING |
                                              DBUS_ITERATION_DO_WRITING |
                                              DBUS_ITERATION_BLOCK,
                                              -1);
    }

  HAVE_LOCK_CHECK (connection);
  _dbus_verbose ("middle\n");
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  HAVE_LOCK_CHECK (connection);
  return status;
}

/**
 * Blocks until the outgoing message queue is empty.
 *
 * @param connection the connection.
 */
void
dbus_connection_flush (DBusConnection *connection)
{
  /* We have to specify DBUS_ITERATION_DO_READING here because
   * otherwise we could have two apps deadlock if they are both doing
   * a flush(), and the kernel buffers fill up. This could change the
   * dispatch status.
   */
  DBusDispatchStatus status;

  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);

  status = _dbus_connection_flush_unlocked (connection);
  
  HAVE_LOCK_CHECK (connection);
  /* Unlocks and calls out to user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);

  _dbus_verbose ("end\n");
}

/**
 * This function implements dbus_connection_read_write_dispatch() and
 * dbus_connection_read_write() (they pass a different value for the
 * dispatch parameter).
 * 
 * @param connection the connection
 * @param timeout_milliseconds max time to block or -1 for infinite
 * @param dispatch dispatch new messages or leave them on the incoming queue
 * @returns #TRUE if the disconnect message has not been processed
 */
static dbus_bool_t
_dbus_connection_read_write_dispatch (DBusConnection *connection,
                                     int             timeout_milliseconds, 
                                     dbus_bool_t     dispatch)
{
  DBusDispatchStatus dstatus;
  dbus_bool_t progress_possible;

  /* Need to grab a ref here in case we're a private connection and
   * the user drops the last ref in a handler we call; see bug 
   * https://bugs.freedesktop.org/show_bug.cgi?id=15635
   */
  dbus_connection_ref (connection);
  dstatus = dbus_connection_get_dispatch_status (connection);

  if (dispatch && dstatus == DBUS_DISPATCH_DATA_REMAINS)
    {
      _dbus_verbose ("doing dispatch\n");
      dbus_connection_dispatch (connection);
      CONNECTION_LOCK (connection);
    }
  else if (dstatus == DBUS_DISPATCH_NEED_MEMORY)
    {
      _dbus_verbose ("pausing for memory\n");
      _dbus_memory_pause_based_on_timeout (timeout_milliseconds);
      CONNECTION_LOCK (connection);
    }
  else
    {
      CONNECTION_LOCK (connection);
      if (_dbus_connection_get_is_connected_unlocked (connection))
        {
          _dbus_verbose ("doing iteration\n");
          _dbus_connection_do_iteration_unlocked (connection,
                                                  NULL,
                                                  DBUS_ITERATION_DO_READING |
                                                  DBUS_ITERATION_DO_WRITING |
                                                  DBUS_ITERATION_BLOCK,
                                                  timeout_milliseconds);
        }
    }
  
  HAVE_LOCK_CHECK (connection);
  /* If we can dispatch, we can make progress until the Disconnected message
   * has been processed; if we can only read/write, we can make progress
   * as long as the transport is open.
   */
  if (dispatch)
    progress_possible = connection->n_incoming != 0 ||
      connection->disconnect_message_link != NULL;
  else
    progress_possible = _dbus_connection_get_is_connected_unlocked (connection);

  CONNECTION_UNLOCK (connection);

  dbus_connection_unref (connection);

  return progress_possible; /* TRUE if we can make more progress */
}


/**
 * This function is intended for use with applications that don't want
 * to write a main loop and deal with #DBusWatch and #DBusTimeout. An
 * example usage would be:
 * 
 * @code
 *   while (dbus_connection_read_write_dispatch (connection, -1))
 *     ; // empty loop body
 * @endcode
 * 
 * In this usage you would normally have set up a filter function to look
 * at each message as it is dispatched. The loop terminates when the last
 * message from the connection (the disconnected signal) is processed.
 * 
 * If there are messages to dispatch, this function will
 * dbus_connection_dispatch() once, and return. If there are no
 * messages to dispatch, this function will block until it can read or
 * write, then read or write, then return.
 *
 * The way to think of this function is that it either makes some sort
 * of progress, or it blocks. Note that, while it is blocked on I/O, it
 * cannot be interrupted (even by other threads), which makes this function
 * unsuitable for applications that do more than just react to received
 * messages.
 *
 * The return value indicates whether the disconnect message has been
 * processed, NOT whether the connection is connected. This is
 * important because even after disconnecting, you want to process any
 * messages you received prior to the disconnect.
 *
 * @param connection the connection
 * @param timeout_milliseconds max time to block or -1 for infinite
 * @returns #TRUE if the disconnect message has not been processed
 */
dbus_bool_t
dbus_connection_read_write_dispatch (DBusConnection *connection,
                                     int             timeout_milliseconds)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (timeout_milliseconds >= 0 || timeout_milliseconds == -1, FALSE);
   return _dbus_connection_read_write_dispatch(connection, timeout_milliseconds, TRUE);
}

/** 
 * This function is intended for use with applications that don't want to
 * write a main loop and deal with #DBusWatch and #DBusTimeout. See also
 * dbus_connection_read_write_dispatch().
 * 
 * As long as the connection is open, this function will block until it can
 * read or write, then read or write, then return #TRUE.
 *
 * If the connection is closed, the function returns #FALSE.
 *
 * The return value indicates whether reading or writing is still
 * possible, i.e. whether the connection is connected.
 *
 * Note that even after disconnection, messages may remain in the
 * incoming queue that need to be
 * processed. dbus_connection_read_write_dispatch() dispatches
 * incoming messages for you; with dbus_connection_read_write() you
 * have to arrange to drain the incoming queue yourself.
 * 
 * @param connection the connection 
 * @param timeout_milliseconds max time to block or -1 for infinite 
 * @returns #TRUE if still connected
 */
dbus_bool_t 
dbus_connection_read_write (DBusConnection *connection, 
                            int             timeout_milliseconds) 
{ 
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (timeout_milliseconds >= 0 || timeout_milliseconds == -1, FALSE);
   return _dbus_connection_read_write_dispatch(connection, timeout_milliseconds, FALSE);
}

/* We need to call this anytime we pop the head of the queue, and then
 * update_dispatch_status_and_unlock needs to be called afterward
 * which will "process" the disconnected message and set
 * disconnected_message_processed.
 */
static void
check_disconnected_message_arrived_unlocked (DBusConnection *connection,
                                             DBusMessage    *head_of_queue)
{
  HAVE_LOCK_CHECK (connection);

  /* checking that the link is NULL is an optimization to avoid the is_signal call */
  if (connection->disconnect_message_link == NULL &&
      dbus_message_is_signal (head_of_queue,
                              DBUS_INTERFACE_LOCAL,
                              "Disconnected"))
    {
      connection->disconnected_message_arrived = TRUE;
    }
}

/**
 * Returns the first-received message from the incoming message queue,
 * leaving it in the queue. If the queue is empty, returns #NULL.
 * 
 * The caller does not own a reference to the returned message, and
 * must either return it using dbus_connection_return_message() or
 * keep it after calling dbus_connection_steal_borrowed_message(). No
 * one can get at the message while its borrowed, so return it as
 * quickly as possible and don't keep a reference to it after
 * returning it. If you need to keep the message, make a copy of it.
 *
 * dbus_connection_dispatch() will block if called while a borrowed
 * message is outstanding; only one piece of code can be playing with
 * the incoming queue at a time. This function will block if called
 * during a dbus_connection_dispatch().
 *
 * @param connection the connection.
 * @returns next message in the incoming queue.
 */
DBusMessage*
dbus_connection_borrow_message (DBusConnection *connection)
{
  DBusDispatchStatus status;
  DBusMessage *message;

  _dbus_return_val_if_fail (connection != NULL, NULL);

  _dbus_verbose ("start\n");
  
  /* this is called for the side effect that it queues
   * up any messages from the transport
   */
  status = dbus_connection_get_dispatch_status (connection);
  if (status != DBUS_DISPATCH_DATA_REMAINS)
    return NULL;
  
  CONNECTION_LOCK (connection);

  _dbus_connection_acquire_dispatch (connection);

  /* While a message is outstanding, the dispatch lock is held */
  _dbus_assert (connection->message_borrowed == NULL);

  connection->message_borrowed = _dbus_list_get_first (&connection->incoming_messages);
  
  message = connection->message_borrowed;

  check_disconnected_message_arrived_unlocked (connection, message);
  
  /* Note that we KEEP the dispatch lock until the message is returned */
  if (message == NULL)
    _dbus_connection_release_dispatch (connection);

  CONNECTION_UNLOCK (connection);

  _dbus_message_trace_ref (message, -1, -1, "dbus_connection_borrow_message");

  /* We don't update dispatch status until it's returned or stolen */
  
  return message;
}

/**
 * Used to return a message after peeking at it using
 * dbus_connection_borrow_message(). Only called if
 * message from dbus_connection_borrow_message() was non-#NULL.
 *
 * @param connection the connection
 * @param message the message from dbus_connection_borrow_message()
 */
void
dbus_connection_return_message (DBusConnection *connection,
				DBusMessage    *message)
{
  DBusDispatchStatus status;
  
  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (message == connection->message_borrowed);
  _dbus_return_if_fail (connection->dispatch_acquired);
  
  CONNECTION_LOCK (connection);
  
  _dbus_assert (message == connection->message_borrowed);
  
  connection->message_borrowed = NULL;

  _dbus_connection_release_dispatch (connection); 

  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);

  _dbus_message_trace_ref (message, -1, -1, "dbus_connection_return_message");
}

/**
 * Used to keep a message after peeking at it using
 * dbus_connection_borrow_message(). Before using this function, see
 * the caveats/warnings in the documentation for
 * dbus_connection_pop_message().
 *
 * @param connection the connection
 * @param message the message from dbus_connection_borrow_message()
 */
void
dbus_connection_steal_borrowed_message (DBusConnection *connection,
					DBusMessage    *message)
{
  DBusMessage *pop_message;
  DBusDispatchStatus status;

  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (message != NULL);
  _dbus_return_if_fail (message == connection->message_borrowed);
  _dbus_return_if_fail (connection->dispatch_acquired);
  
  CONNECTION_LOCK (connection);
 
  _dbus_assert (message == connection->message_borrowed);

  pop_message = _dbus_list_pop_first (&connection->incoming_messages);
  _dbus_assert (message == pop_message);
  (void) pop_message; /* unused unless asserting */

  connection->n_incoming -= 1;
 
  _dbus_verbose ("Incoming message %p stolen from queue, %d incoming\n",
		 message, connection->n_incoming);
 
  connection->message_borrowed = NULL;

  _dbus_connection_release_dispatch (connection);

  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
  _dbus_message_trace_ref (message, -1, -1,
      "dbus_connection_steal_borrowed_message");
}

/* See dbus_connection_pop_message, but requires the caller to own
 * the lock before calling. May drop the lock while running.
 */
static DBusList*
_dbus_connection_pop_message_link_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  _dbus_assert (connection->message_borrowed == NULL);
  
  if (connection->n_incoming > 0)
    {
      DBusList *link;

      link = _dbus_list_pop_first_link (&connection->incoming_messages);
      connection->n_incoming -= 1;

      _dbus_verbose ("Message %p (%s %s %s %s sig:'%s' serial:%u) removed from incoming queue %p, %d incoming\n",
                     link->data,
                     dbus_message_type_to_string (dbus_message_get_type (link->data)),
                     dbus_message_get_path (link->data) ?
                     dbus_message_get_path (link->data) :
                     "no path",
                     dbus_message_get_interface (link->data) ?
                     dbus_message_get_interface (link->data) :
                     "no interface",
                     dbus_message_get_member (link->data) ?
                     dbus_message_get_member (link->data) :
                     "no member",
                     dbus_message_get_signature (link->data),
                     dbus_message_get_serial (link->data),
                     connection, connection->n_incoming);

      _dbus_message_trace_ref (link->data, -1, -1,
          "_dbus_connection_pop_message_link_unlocked");

      check_disconnected_message_arrived_unlocked (connection, link->data);
      
      return link;
    }
  else
    return NULL;
}

/* See dbus_connection_pop_message, but requires the caller to own
 * the lock before calling. May drop the lock while running.
 */
static DBusMessage*
_dbus_connection_pop_message_unlocked (DBusConnection *connection)
{
  DBusList *link;

  HAVE_LOCK_CHECK (connection);
  
  link = _dbus_connection_pop_message_link_unlocked (connection);

  if (link != NULL)
    {
      DBusMessage *message;
      
      message = link->data;
      
      _dbus_list_free_link (link);
      
      return message;
    }
  else
    return NULL;
}

static void
_dbus_connection_putback_message_link_unlocked (DBusConnection *connection,
                                                DBusList       *message_link)
{
  HAVE_LOCK_CHECK (connection);
  
  _dbus_assert (message_link != NULL);
  /* You can't borrow a message while a link is outstanding */
  _dbus_assert (connection->message_borrowed == NULL);
  /* We had to have the dispatch lock across the pop/putback */
  _dbus_assert (connection->dispatch_acquired);

  _dbus_list_prepend_link (&connection->incoming_messages,
                           message_link);
  connection->n_incoming += 1;

  _dbus_verbose ("Message %p (%s %s %s '%s') put back into queue %p, %d incoming\n",
                 message_link->data,
                 dbus_message_type_to_string (dbus_message_get_type (message_link->data)),
                 dbus_message_get_interface (message_link->data) ?
                 dbus_message_get_interface (message_link->data) :
                 "no interface",
                 dbus_message_get_member (message_link->data) ?
                 dbus_message_get_member (message_link->data) :
                 "no member",
                 dbus_message_get_signature (message_link->data),
                 connection, connection->n_incoming);

  _dbus_message_trace_ref (message_link->data, -1, -1,
      "_dbus_connection_putback_message_link_unlocked");
}

/**
 * Returns the first-received message from the incoming message queue,
 * removing it from the queue. The caller owns a reference to the
 * returned message. If the queue is empty, returns #NULL.
 *
 * This function bypasses any message handlers that are registered,
 * and so using it is usually wrong. Instead, let the main loop invoke
 * dbus_connection_dispatch(). Popping messages manually is only
 * useful in very simple programs that don't share a #DBusConnection
 * with any libraries or other modules.
 *
 * There is a lock that covers all ways of accessing the incoming message
 * queue, so dbus_connection_dispatch(), dbus_connection_pop_message(),
 * dbus_connection_borrow_message(), etc. will all block while one of the others
 * in the group is running.
 * 
 * @param connection the connection.
 * @returns next message in the incoming queue.
 */
DBusMessage*
dbus_connection_pop_message (DBusConnection *connection)
{
  DBusMessage *message;
  DBusDispatchStatus status;

  _dbus_verbose ("start\n");
  
  /* this is called for the side effect that it queues
   * up any messages from the transport
   */
  status = dbus_connection_get_dispatch_status (connection);
  if (status != DBUS_DISPATCH_DATA_REMAINS)
    return NULL;
  
  CONNECTION_LOCK (connection);
  _dbus_connection_acquire_dispatch (connection);
  HAVE_LOCK_CHECK (connection);
  
  message = _dbus_connection_pop_message_unlocked (connection);

  _dbus_verbose ("Returning popped message %p\n", message);    

  _dbus_connection_release_dispatch (connection);

  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
  
  return message;
}

/**
 * Acquire the dispatcher. This is a separate lock so the main
 * connection lock can be dropped to call out to application dispatch
 * handlers.
 *
 * @param connection the connection.
 */
static void
_dbus_connection_acquire_dispatch (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);

  _dbus_connection_ref_unlocked (connection);
  CONNECTION_UNLOCK (connection);
  
  _dbus_verbose ("locking dispatch_mutex\n");
  _dbus_cmutex_lock (connection->dispatch_mutex);

  while (connection->dispatch_acquired)
    {
      _dbus_verbose ("waiting for dispatch to be acquirable\n");
      _dbus_condvar_wait (connection->dispatch_cond, 
                          connection->dispatch_mutex);
    }
  
  _dbus_assert (!connection->dispatch_acquired);

  connection->dispatch_acquired = TRUE;

  _dbus_verbose ("unlocking dispatch_mutex\n");
  _dbus_cmutex_unlock (connection->dispatch_mutex);
  
  CONNECTION_LOCK (connection);
  _dbus_connection_unref_unlocked (connection);
}

/**
 * Release the dispatcher when you're done with it. Only call
 * after you've acquired the dispatcher. Wakes up at most one
 * thread currently waiting to acquire the dispatcher.
 *
 * @param connection the connection.
 */
static void
_dbus_connection_release_dispatch (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  _dbus_verbose ("locking dispatch_mutex\n");
  _dbus_cmutex_lock (connection->dispatch_mutex);
  
  _dbus_assert (connection->dispatch_acquired);

  connection->dispatch_acquired = FALSE;
  _dbus_condvar_wake_one (connection->dispatch_cond);

  _dbus_verbose ("unlocking dispatch_mutex\n");
  _dbus_cmutex_unlock (connection->dispatch_mutex);
}

static void
_dbus_connection_failed_pop (DBusConnection *connection,
			     DBusList       *message_link)
{
  _dbus_list_prepend_link (&connection->incoming_messages,
			   message_link);
  connection->n_incoming += 1;
}

/* Note this may be called multiple times since we don't track whether we already did it */
static void
notify_disconnected_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);

  /* Set the weakref in dbus-bus.c to NULL, so nobody will get a disconnected
   * connection from dbus_bus_get(). We make the same guarantee for
   * dbus_connection_open() but in a different way since we don't want to
   * unref right here; we instead check for connectedness before returning
   * the connection from the hash.
   */
  _dbus_bus_notify_shared_connection_disconnected_unlocked (connection);

  /* Dump the outgoing queue, we aren't going to be able to
   * send it now, and we'd like accessors like
   * dbus_connection_get_outgoing_size() to be accurate.
   */
  if (connection->n_outgoing > 0)
    {
      DBusList *link;
      
      _dbus_verbose ("Dropping %d outgoing messages since we're disconnected\n",
                     connection->n_outgoing);
      
      while ((link = _dbus_list_get_last_link (&connection->outgoing_messages)))
        {
          _dbus_connection_message_sent_unlocked (connection, link->data);
        }
    } 
}

/* Note this may be called multiple times since we don't track whether we already did it */
static DBusDispatchStatus
notify_disconnected_and_dispatch_complete_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  if (connection->disconnect_message_link != NULL)
    {
      _dbus_verbose ("Sending disconnect message\n");
      
      /* If we have pending calls, queue their timeouts - we want the Disconnected
       * to be the last message, after these timeouts.
       */
      connection_timeout_and_complete_all_pending_calls_unlocked (connection);
      
      /* We haven't sent the disconnect message already,
       * and all real messages have been queued up.
       */
      _dbus_connection_queue_synthesized_message_link (connection,
                                                       connection->disconnect_message_link);
      connection->disconnect_message_link = NULL;

      return DBUS_DISPATCH_DATA_REMAINS;
    }

  return DBUS_DISPATCH_COMPLETE;
}

static DBusDispatchStatus
_dbus_connection_get_dispatch_status_unlocked (DBusConnection *connection)
{
  HAVE_LOCK_CHECK (connection);
  
  if (connection->n_incoming > 0)
    return DBUS_DISPATCH_DATA_REMAINS;
  else if (!_dbus_transport_queue_messages (connection->transport))
    return DBUS_DISPATCH_NEED_MEMORY;
  else
    {
      DBusDispatchStatus status;
      dbus_bool_t is_connected;
      
      status = _dbus_transport_get_dispatch_status (connection->transport);
      is_connected = _dbus_transport_get_is_connected (connection->transport);

      _dbus_verbose ("dispatch status = %s is_connected = %d\n",
                     DISPATCH_STATUS_NAME (status), is_connected);
      
      if (!is_connected)
        {
          /* It's possible this would be better done by having an explicit
           * notification from _dbus_transport_disconnect() that would
           * synchronously do this, instead of waiting for the next dispatch
           * status check. However, probably not good to change until it causes
           * a problem.
           */
          notify_disconnected_unlocked (connection);

          /* I'm not sure this is needed; the idea is that we want to
           * queue the Disconnected only after we've read all the
           * messages, but if we're disconnected maybe we are guaranteed
           * to have read them all ?
           */
          if (status == DBUS_DISPATCH_COMPLETE)
            status = notify_disconnected_and_dispatch_complete_unlocked (connection);
        }
      
      if (status != DBUS_DISPATCH_COMPLETE)
        return status;
      else if (connection->n_incoming > 0)
        return DBUS_DISPATCH_DATA_REMAINS;
      else
        return DBUS_DISPATCH_COMPLETE;
    }
}

static void
_dbus_connection_update_dispatch_status_and_unlock (DBusConnection    *connection,
                                                    DBusDispatchStatus new_status)
{
  dbus_bool_t changed;
  DBusDispatchStatusFunction function;
  void *data;

  HAVE_LOCK_CHECK (connection);

  _dbus_connection_ref_unlocked (connection);

  changed = new_status != connection->last_dispatch_status;

  connection->last_dispatch_status = new_status;

  function = connection->dispatch_status_function;
  data = connection->dispatch_status_data;

  if (connection->disconnected_message_arrived &&
      !connection->disconnected_message_processed)
    {
      connection->disconnected_message_processed = TRUE;
      
      /* this does an unref, but we have a ref
       * so we should not run the finalizer here
       * inside the lock.
       */
      connection_forget_shared_unlocked (connection);

      if (connection->exit_on_disconnect)
        {
          CONNECTION_UNLOCK (connection);            
          
          _dbus_verbose ("Exiting on Disconnected signal\n");
          _dbus_exit (1);
          _dbus_assert_not_reached ("Call to exit() returned");
        }
    }
  
  /* We drop the lock */
  CONNECTION_UNLOCK (connection);
  
  if (changed && function)
    {
      _dbus_verbose ("Notifying of change to dispatch status of %p now %d (%s)\n",
                     connection, new_status,
                     DISPATCH_STATUS_NAME (new_status));
      (* function) (connection, new_status, data);      
    }
  
  dbus_connection_unref (connection);
}

/**
 * Gets the current state of the incoming message queue.
 * #DBUS_DISPATCH_DATA_REMAINS indicates that the message queue
 * may contain messages. #DBUS_DISPATCH_COMPLETE indicates that the
 * incoming queue is empty. #DBUS_DISPATCH_NEED_MEMORY indicates that
 * there could be data, but we can't know for sure without more
 * memory.
 *
 * To process the incoming message queue, use dbus_connection_dispatch()
 * or (in rare cases) dbus_connection_pop_message().
 *
 * Note, #DBUS_DISPATCH_DATA_REMAINS really means that either we
 * have messages in the queue, or we have raw bytes buffered up
 * that need to be parsed. When these bytes are parsed, they
 * may not add up to an entire message. Thus, it's possible
 * to see a status of #DBUS_DISPATCH_DATA_REMAINS but not
 * have a message yet.
 *
 * In particular this happens on initial connection, because all sorts
 * of authentication protocol stuff has to be parsed before the
 * first message arrives.
 * 
 * @param connection the connection.
 * @returns current dispatch status
 */
DBusDispatchStatus
dbus_connection_get_dispatch_status (DBusConnection *connection)
{
  DBusDispatchStatus status;

  _dbus_return_val_if_fail (connection != NULL, DBUS_DISPATCH_COMPLETE);

  _dbus_verbose ("start\n");
  
  CONNECTION_LOCK (connection);

  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  
  CONNECTION_UNLOCK (connection);

  return status;
}

/**
 * Filter funtion for handling the Peer standard interface.
 */
static DBusHandlerResult
_dbus_connection_peer_filter_unlocked_no_update (DBusConnection *connection,
                                                 DBusMessage    *message)
{
  dbus_bool_t sent = FALSE;
  DBusMessage *ret = NULL;
  DBusList *expire_link;

  if (connection->route_peer_messages && dbus_message_get_destination (message) != NULL)
    {
      /* This means we're letting the bus route this message */
      return DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
    }

  if (!dbus_message_has_interface (message, DBUS_INTERFACE_PEER))
    {
      return DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
    }

  /* Preallocate a linked-list link, so that if we need to dispose of a
   * message, we can attach it to the expired list */
  expire_link = _dbus_list_alloc_link (NULL);

  if (!expire_link)
    return DBUS_HANDLER_RESULT_NEED_MEMORY;

  if (dbus_message_is_method_call (message,
                                   DBUS_INTERFACE_PEER,
                                   "Ping"))
    {
      ret = dbus_message_new_method_return (message);
      if (ret == NULL)
        goto out;

      sent = _dbus_connection_send_unlocked_no_update (connection, ret, NULL);
    }
  else if (dbus_message_is_method_call (message,
                                        DBUS_INTERFACE_PEER,
                                        "GetMachineId"))
    {
      DBusString uuid;
      DBusError error = DBUS_ERROR_INIT;

      if (!_dbus_string_init (&uuid))
        goto out;

      if (_dbus_get_local_machine_uuid_encoded (&uuid, &error))
        {
          const char *v_STRING;

          ret = dbus_message_new_method_return (message);

          if (ret == NULL)
            {
              _dbus_string_free (&uuid);
              goto out;
            }

          v_STRING = _dbus_string_get_const_data (&uuid);
          if (dbus_message_append_args (ret,
                                        DBUS_TYPE_STRING, &v_STRING,
                                        DBUS_TYPE_INVALID))
            {
              sent = _dbus_connection_send_unlocked_no_update (connection, ret, NULL);
            }
        }
      else if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          dbus_error_free (&error);
          goto out;
        }
      else
        {
          ret = dbus_message_new_error (message, error.name, error.message);
          dbus_error_free (&error);

          if (ret == NULL)
            goto out;

          sent = _dbus_connection_send_unlocked_no_update (connection, ret,
                                                           NULL);
        }

      _dbus_string_free (&uuid);
    }
  else
    {
      /* We need to bounce anything else with this interface, otherwise apps
       * could start extending the interface and when we added extensions
       * here to DBusConnection we'd break those apps.
       */
      ret = dbus_message_new_error (message,
                                    DBUS_ERROR_UNKNOWN_METHOD,
                                    "Unknown method invoked on org.freedesktop.DBus.Peer interface");
      if (ret == NULL)
        goto out;

      sent = _dbus_connection_send_unlocked_no_update (connection, ret, NULL);
    }

out:
  if (ret == NULL)
    {
      _dbus_list_free_link (expire_link);
    }
  else
    {
      /* It'll be safe to unref the reply when we unlock */
      expire_link->data = ret;
      _dbus_list_prepend_link (&connection->expired_messages, expire_link);
    }

  if (!sent)
    return DBUS_HANDLER_RESULT_NEED_MEMORY;

  return DBUS_HANDLER_RESULT_HANDLED;
}

/**
* Processes all builtin filter functions
*
* If the spec specifies a standard interface
* they should be processed from this method
**/
static DBusHandlerResult
_dbus_connection_run_builtin_filters_unlocked_no_update (DBusConnection *connection,
                                                           DBusMessage    *message)
{
  /* We just run one filter for now but have the option to run more
     if the spec calls for it in the future */

  return _dbus_connection_peer_filter_unlocked_no_update (connection, message);
}

/**
 * Processes any incoming data.
 *
 * If there's incoming raw data that has not yet been parsed, it is
 * parsed, which may or may not result in adding messages to the
 * incoming queue.
 *
 * The incoming data buffer is filled when the connection reads from
 * its underlying transport (such as a socket).  Reading usually
 * happens in dbus_watch_handle() or dbus_connection_read_write().
 * 
 * If there are complete messages in the incoming queue,
 * dbus_connection_dispatch() removes one message from the queue and
 * processes it. Processing has three steps.
 *
 * First, any method replies are passed to #DBusPendingCall or
 * dbus_connection_send_with_reply_and_block() in order to
 * complete the pending method call.
 * 
 * Second, any filters registered with dbus_connection_add_filter()
 * are run. If any filter returns #DBUS_HANDLER_RESULT_HANDLED
 * then processing stops after that filter.
 *
 * Third, if the message is a method call it is forwarded to
 * any registered object path handlers added with
 * dbus_connection_register_object_path() or
 * dbus_connection_register_fallback().
 *
 * A single call to dbus_connection_dispatch() will process at most
 * one message; it will not clear the entire message queue.
 *
 * Be careful about calling dbus_connection_dispatch() from inside a
 * message handler, i.e. calling dbus_connection_dispatch()
 * recursively.  If threads have been initialized with a recursive
 * mutex function, then this will not deadlock; however, it can
 * certainly confuse your application.
 * 
 * @todo some FIXME in here about handling DBUS_HANDLER_RESULT_NEED_MEMORY
 * 
 * @param connection the connection
 * @returns dispatch status, see dbus_connection_get_dispatch_status()
 */
DBusDispatchStatus
dbus_connection_dispatch (DBusConnection *connection)
{
  DBusMessage *message;
  DBusList *link, *filter_list_copy, *message_link;
  DBusHandlerResult result;
  DBusPendingCall *pending;
  dbus_int32_t reply_serial;
  DBusDispatchStatus status;
  dbus_bool_t found_object;

  _dbus_return_val_if_fail (connection != NULL, DBUS_DISPATCH_COMPLETE);

  _dbus_verbose ("\n");
  
  CONNECTION_LOCK (connection);
  status = _dbus_connection_get_dispatch_status_unlocked (connection);
  if (status != DBUS_DISPATCH_DATA_REMAINS)
    {
      /* unlocks and calls out to user code */
      _dbus_connection_update_dispatch_status_and_unlock (connection, status);
      return status;
    }
  
  /* We need to ref the connection since the callback could potentially
   * drop the last ref to it
   */
  _dbus_connection_ref_unlocked (connection);

  _dbus_connection_acquire_dispatch (connection);
  HAVE_LOCK_CHECK (connection);

  message_link = _dbus_connection_pop_message_link_unlocked (connection);
  if (message_link == NULL)
    {
      /* another thread dispatched our stuff */

      _dbus_verbose ("another thread dispatched message (during acquire_dispatch above)\n");
      
      _dbus_connection_release_dispatch (connection);

      status = _dbus_connection_get_dispatch_status_unlocked (connection);

      _dbus_connection_update_dispatch_status_and_unlock (connection, status);
      
      dbus_connection_unref (connection);
      
      return status;
    }

  message = message_link->data;

  _dbus_verbose (" dispatching message %p (%s %s %s '%s')\n",
                 message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message));

  result = DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
  
  /* Pending call handling must be first, because if you do
   * dbus_connection_send_with_reply_and_block() or
   * dbus_pending_call_block() then no handlers/filters will be run on
   * the reply. We want consistent semantics in the case where we
   * dbus_connection_dispatch() the reply.
   */
  
  reply_serial = dbus_message_get_reply_serial (message);
  pending = _dbus_hash_table_lookup_int (connection->pending_replies,
                                         reply_serial);
  if (pending)
    {
      _dbus_verbose ("Dispatching a pending reply\n");
      complete_pending_call_and_unlock (connection, pending, message);
      pending = NULL; /* it's probably unref'd */
      
      CONNECTION_LOCK (connection);
      _dbus_verbose ("pending call completed in dispatch\n");
      result = DBUS_HANDLER_RESULT_HANDLED;
      goto out;
    }

  result = _dbus_connection_run_builtin_filters_unlocked_no_update (connection, message);
  if (result != DBUS_HANDLER_RESULT_NOT_YET_HANDLED)
    goto out;
 
  if (!_dbus_list_copy (&connection->filter_list, &filter_list_copy))
    {
      _dbus_connection_release_dispatch (connection);
      HAVE_LOCK_CHECK (connection);
      
      _dbus_connection_failed_pop (connection, message_link);

      /* unlocks and calls user code */
      _dbus_connection_update_dispatch_status_and_unlock (connection,
                                                          DBUS_DISPATCH_NEED_MEMORY);
      dbus_connection_unref (connection);
      
      return DBUS_DISPATCH_NEED_MEMORY;
    }

  for (link = _dbus_list_get_first_link (&filter_list_copy);
       link != NULL;
       link = _dbus_list_get_next_link (&filter_list_copy, link))
    _dbus_message_filter_ref (link->data);

  /* We're still protected from dispatch() reentrancy here
   * since we acquired the dispatcher
   */
  CONNECTION_UNLOCK (connection);
  
  link = _dbus_list_get_first_link (&filter_list_copy);
  while (link != NULL)
    {
      DBusMessageFilter *filter = link->data;
      DBusList *next = _dbus_list_get_next_link (&filter_list_copy, link);

      if (filter->function == NULL)
        {
          _dbus_verbose ("  filter was removed in a callback function\n");
          link = next;
          continue;
        }

      _dbus_verbose ("  running filter on message %p\n", message);
      result = (* filter->function) (connection, message, filter->user_data);

      if (result != DBUS_HANDLER_RESULT_NOT_YET_HANDLED)
	break;

      link = next;
    }

  _dbus_list_clear_full (&filter_list_copy,
                         (DBusFreeFunction) _dbus_message_filter_unref);

  CONNECTION_LOCK (connection);

  if (result == DBUS_HANDLER_RESULT_NEED_MEMORY)
    {
      _dbus_verbose ("No memory\n");
      goto out;
    }
  else if (result == DBUS_HANDLER_RESULT_HANDLED)
    {
      _dbus_verbose ("filter handled message in dispatch\n");
      goto out;
    }

  /* We're still protected from dispatch() reentrancy here
   * since we acquired the dispatcher
   */
  _dbus_verbose ("  running object path dispatch on message %p (%s %s %s '%s')\n",
                 message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message));

  HAVE_LOCK_CHECK (connection);
  result = _dbus_object_tree_dispatch_and_unlock (connection->objects,
                                                  message,
                                                  &found_object);
  
  CONNECTION_LOCK (connection);

  if (result != DBUS_HANDLER_RESULT_NOT_YET_HANDLED)
    {
      _dbus_verbose ("object tree handled message in dispatch\n");
      goto out;
    }

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_CALL)
    {
      DBusMessage *reply;
      DBusString str;
      DBusPreallocatedSend *preallocated;
      DBusList *expire_link;

      _dbus_verbose ("  sending error %s\n",
                     DBUS_ERROR_UNKNOWN_METHOD);

      if (!_dbus_string_init (&str))
        {
          result = DBUS_HANDLER_RESULT_NEED_MEMORY;
          _dbus_verbose ("no memory for error string in dispatch\n");
          goto out;
        }
              
      if (!_dbus_string_append_printf (&str,
                                       "Method \"%s\" with signature \"%s\" on interface \"%s\" doesn't exist\n",
                                       dbus_message_get_member (message),
                                       dbus_message_get_signature (message),
                                       dbus_message_get_interface (message)))
        {
          _dbus_string_free (&str);
          result = DBUS_HANDLER_RESULT_NEED_MEMORY;
          _dbus_verbose ("no memory for error string in dispatch\n");
          goto out;
        }
      
      reply = dbus_message_new_error (message,
                                      found_object ? DBUS_ERROR_UNKNOWN_METHOD : DBUS_ERROR_UNKNOWN_OBJECT,
                                      _dbus_string_get_const_data (&str));
      _dbus_string_free (&str);

      if (reply == NULL)
        {
          result = DBUS_HANDLER_RESULT_NEED_MEMORY;
          _dbus_verbose ("no memory for error reply in dispatch\n");
          goto out;
        }

      expire_link = _dbus_list_alloc_link (reply);

      if (expire_link == NULL)
        {
          dbus_message_unref (reply);
          result = DBUS_HANDLER_RESULT_NEED_MEMORY;
          _dbus_verbose ("no memory for error send in dispatch\n");
          goto out;
        }

      preallocated = _dbus_connection_preallocate_send_unlocked (connection);

      if (preallocated == NULL)
        {
          _dbus_list_free_link (expire_link);
          /* It's OK that this is finalized, because it hasn't been seen by
           * anything that could attach user callbacks */
          dbus_message_unref (reply);
          result = DBUS_HANDLER_RESULT_NEED_MEMORY;
          _dbus_verbose ("no memory for error send in dispatch\n");
          goto out;
        }

      _dbus_connection_send_preallocated_unlocked_no_update (connection, preallocated,
                                                             reply, NULL);
      /* reply will be freed when we release the lock */
      _dbus_list_prepend_link (&connection->expired_messages, expire_link);

      result = DBUS_HANDLER_RESULT_HANDLED;
    }
  
  _dbus_verbose ("  done dispatching %p (%s %s %s '%s') on connection %p\n", message,
                 dbus_message_type_to_string (dbus_message_get_type (message)),
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) :
                 "no interface",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) :
                 "no member",
                 dbus_message_get_signature (message),
                 connection);
  
 out:
  if (result == DBUS_HANDLER_RESULT_NEED_MEMORY)
    {
      _dbus_verbose ("out of memory\n");
      
      /* Put message back, and we'll start over.
       * Yes this means handlers must be idempotent if they
       * don't return HANDLED; c'est la vie.
       */
      _dbus_connection_putback_message_link_unlocked (connection,
                                                      message_link);
      /* now we don't want to free them */
      message_link = NULL;
      message = NULL;
    }
  else
    {
      _dbus_verbose (" ... done dispatching\n");
    }

  _dbus_connection_release_dispatch (connection);
  HAVE_LOCK_CHECK (connection);

  if (message != NULL)
    {
      /* We don't want this message to count in maximum message limits when
       * computing the dispatch status, below. We have to drop the lock
       * temporarily, because finalizing a message can trigger callbacks.
       *
       * We have a reference to the connection, and we don't use any cached
       * pointers to the connection's internals below this point, so it should
       * be safe to drop the lock and take it back. */
      CONNECTION_UNLOCK (connection);
      dbus_message_unref (message);
      CONNECTION_LOCK (connection);
    }

  if (message_link != NULL)
    _dbus_list_free_link (message_link);

  _dbus_verbose ("before final status update\n");
  status = _dbus_connection_get_dispatch_status_unlocked (connection);

  /* unlocks and calls user code */
  _dbus_connection_update_dispatch_status_and_unlock (connection, status);
  
  dbus_connection_unref (connection);
  
  return status;
}

/**
 * Sets the watch functions for the connection. These functions are
 * responsible for making the application's main loop aware of file
 * descriptors that need to be monitored for events, using select() or
 * poll(). When using Qt, typically the DBusAddWatchFunction would
 * create a QSocketNotifier. When using GLib, the DBusAddWatchFunction
 * could call g_io_add_watch(), or could be used as part of a more
 * elaborate GSource. Note that when a watch is added, it may
 * not be enabled.
 *
 * The DBusWatchToggledFunction notifies the application that the
 * watch has been enabled or disabled. Call dbus_watch_get_enabled()
 * to check this. A disabled watch should have no effect, and enabled
 * watch should be added to the main loop. This feature is used
 * instead of simply adding/removing the watch because
 * enabling/disabling can be done without memory allocation.  The
 * toggled function may be NULL if a main loop re-queries
 * dbus_watch_get_enabled() every time anyway.
 * 
 * The DBusWatch can be queried for the file descriptor to watch using
 * dbus_watch_get_unix_fd() or dbus_watch_get_socket(), and for the
 * events to watch for using dbus_watch_get_flags(). The flags
 * returned by dbus_watch_get_flags() will only contain
 * DBUS_WATCH_READABLE and DBUS_WATCH_WRITABLE, never
 * DBUS_WATCH_HANGUP or DBUS_WATCH_ERROR; all watches implicitly
 * include a watch for hangups, errors, and other exceptional
 * conditions.
 *
 * Once a file descriptor becomes readable or writable, or an exception
 * occurs, dbus_watch_handle() should be called to
 * notify the connection of the file descriptor's condition.
 *
 * dbus_watch_handle() cannot be called during the
 * DBusAddWatchFunction, as the connection will not be ready to handle
 * that watch yet.
 * 
 * It is not allowed to reference a DBusWatch after it has been passed
 * to remove_function.
 *
 * If #FALSE is returned due to lack of memory, the failure may be due
 * to a #FALSE return from the new add_function. If so, the
 * add_function may have been called successfully one or more times,
 * but the remove_function will also have been called to remove any
 * successful adds. i.e. if #FALSE is returned the net result
 * should be that dbus_connection_set_watch_functions() has no effect,
 * but the add_function and remove_function may have been called.
 *
 * @note The thread lock on DBusConnection is held while
 * watch functions are invoked, so inside these functions you
 * may not invoke any methods on DBusConnection or it will deadlock.
 * See the comments in the code or http://lists.freedesktop.org/archives/dbus/2007-July/tread.html#8144
 * if you encounter this issue and want to attempt writing a patch.
 * 
 * @param connection the connection.
 * @param add_function function to begin monitoring a new descriptor.
 * @param remove_function function to stop monitoring a descriptor.
 * @param toggled_function function to notify of enable/disable
 * @param data data to pass to add_function and remove_function.
 * @param free_data_function function to be called to free the data.
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_connection_set_watch_functions (DBusConnection              *connection,
                                     DBusAddWatchFunction         add_function,
                                     DBusRemoveWatchFunction      remove_function,
                                     DBusWatchToggledFunction     toggled_function,
                                     void                        *data,
                                     DBusFreeFunction             free_data_function)
{
  dbus_bool_t retval;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  
  CONNECTION_LOCK (connection);

  retval = _dbus_watch_list_set_functions (connection->watches,
                                           add_function, remove_function,
                                           toggled_function,
                                           data, free_data_function);

  CONNECTION_UNLOCK (connection);

  return retval;
}

/**
 * Sets the timeout functions for the connection. These functions are
 * responsible for making the application's main loop aware of timeouts.
 * When using Qt, typically the DBusAddTimeoutFunction would create a
 * QTimer. When using GLib, the DBusAddTimeoutFunction would call
 * g_timeout_add.
 * 
 * The DBusTimeoutToggledFunction notifies the application that the
 * timeout has been enabled or disabled. Call
 * dbus_timeout_get_enabled() to check this. A disabled timeout should
 * have no effect, and enabled timeout should be added to the main
 * loop. This feature is used instead of simply adding/removing the
 * timeout because enabling/disabling can be done without memory
 * allocation. With Qt, QTimer::start() and QTimer::stop() can be used
 * to enable and disable. The toggled function may be NULL if a main
 * loop re-queries dbus_timeout_get_enabled() every time anyway.
 * Whenever a timeout is toggled, its interval may change.
 *
 * The DBusTimeout can be queried for the timer interval using
 * dbus_timeout_get_interval(). dbus_timeout_handle() should be called
 * repeatedly, each time the interval elapses, starting after it has
 * elapsed once. The timeout stops firing when it is removed with the
 * given remove_function.  The timer interval may change whenever the
 * timeout is added, removed, or toggled.
 *
 * @note The thread lock on DBusConnection is held while
 * timeout functions are invoked, so inside these functions you
 * may not invoke any methods on DBusConnection or it will deadlock.
 * See the comments in the code or http://lists.freedesktop.org/archives/dbus/2007-July/thread.html#8144
 * if you encounter this issue and want to attempt writing a patch.
 *
 * @param connection the connection.
 * @param add_function function to add a timeout.
 * @param remove_function function to remove a timeout.
 * @param toggled_function function to notify of enable/disable
 * @param data data to pass to add_function and remove_function.
 * @param free_data_function function to be called to free the data.
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_connection_set_timeout_functions   (DBusConnection            *connection,
					 DBusAddTimeoutFunction     add_function,
					 DBusRemoveTimeoutFunction  remove_function,
                                         DBusTimeoutToggledFunction toggled_function,
					 void                      *data,
					 DBusFreeFunction           free_data_function)
{
  dbus_bool_t retval;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  
  CONNECTION_LOCK (connection);

  retval = _dbus_timeout_list_set_functions (connection->timeouts,
                                             add_function, remove_function,
                                             toggled_function,
                                             data, free_data_function);

  CONNECTION_UNLOCK (connection);

  return retval;
}

/**
 * Sets the mainloop wakeup function for the connection. This function
 * is responsible for waking up the main loop (if its sleeping in
 * another thread) when some some change has happened to the
 * connection that the mainloop needs to reconsider (e.g. a message
 * has been queued for writing).  When using Qt, this typically
 * results in a call to QEventLoop::wakeUp().  When using GLib, it
 * would call g_main_context_wakeup().
 *
 * @param connection the connection.
 * @param wakeup_main_function function to wake up the mainloop
 * @param data data to pass wakeup_main_function
 * @param free_data_function function to be called to free the data.
 */
void
dbus_connection_set_wakeup_main_function (DBusConnection            *connection,
					  DBusWakeupMainFunction     wakeup_main_function,
					  void                      *data,
					  DBusFreeFunction           free_data_function)
{
  void *old_data;
  DBusFreeFunction old_free_data;

  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  old_data = connection->wakeup_main_data;
  old_free_data = connection->free_wakeup_main_data;

  connection->wakeup_main_function = wakeup_main_function;
  connection->wakeup_main_data = data;
  connection->free_wakeup_main_data = free_data_function;
  
  CONNECTION_UNLOCK (connection);

  /* Callback outside the lock */
  if (old_free_data)
    (*old_free_data) (old_data);
}

/**
 * Set a function to be invoked when the dispatch status changes.
 * If the dispatch status is #DBUS_DISPATCH_DATA_REMAINS, then
 * dbus_connection_dispatch() needs to be called to process incoming
 * messages. However, dbus_connection_dispatch() MUST NOT BE CALLED
 * from inside the DBusDispatchStatusFunction. Indeed, almost
 * any reentrancy in this function is a bad idea. Instead,
 * the DBusDispatchStatusFunction should simply save an indication
 * that messages should be dispatched later, when the main loop
 * is re-entered.
 *
 * If you don't set a dispatch status function, you have to be sure to
 * dispatch on every iteration of your main loop, especially if
 * dbus_watch_handle() or dbus_timeout_handle() were called.
 *
 * @param connection the connection
 * @param function function to call on dispatch status changes
 * @param data data for function
 * @param free_data_function free the function data
 */
void
dbus_connection_set_dispatch_status_function (DBusConnection             *connection,
                                              DBusDispatchStatusFunction  function,
                                              void                       *data,
                                              DBusFreeFunction            free_data_function)
{
  void *old_data;
  DBusFreeFunction old_free_data;

  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  old_data = connection->dispatch_status_data;
  old_free_data = connection->free_dispatch_status_data;

  connection->dispatch_status_function = function;
  connection->dispatch_status_data = data;
  connection->free_dispatch_status_data = free_data_function;
  
  CONNECTION_UNLOCK (connection);

  /* Callback outside the lock */
  if (old_free_data)
    (*old_free_data) (old_data);
}

/**
 * Get the UNIX file descriptor of the connection, if any.  This can
 * be used for SELinux access control checks with getpeercon() for
 * example. DO NOT read or write to the file descriptor, or try to
 * select() on it; use DBusWatch for main loop integration. Not all
 * connections will have a file descriptor. So for adding descriptors
 * to the main loop, use dbus_watch_get_unix_fd() and so forth.
 *
 * If the connection is socket-based, you can also use
 * dbus_connection_get_socket(), which will work on Windows too.
 * This function always fails on Windows.
 *
 * Right now the returned descriptor is always a socket, but
 * that is not guaranteed.
 * 
 * @param connection the connection
 * @param fd return location for the file descriptor.
 * @returns #TRUE if fd is successfully obtained.
 */
dbus_bool_t
dbus_connection_get_unix_fd (DBusConnection *connection,
                             int            *fd)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (connection->transport != NULL, FALSE);

#ifdef DBUS_WIN
  /* FIXME do this on a lower level */
  return FALSE;
#endif
  
  return dbus_connection_get_socket(connection, fd);
}

/**
 * Gets the underlying Windows or UNIX socket file descriptor
 * of the connection, if any. DO NOT read or write to the file descriptor, or try to
 * select() on it; use DBusWatch for main loop integration. Not all
 * connections will have a socket. So for adding descriptors
 * to the main loop, use dbus_watch_get_socket() and so forth.
 *
 * If the connection is not socket-based, this function will return FALSE,
 * even if the connection does have a file descriptor of some kind.
 * i.e. this function always returns specifically a socket file descriptor.
 * 
 * @param connection the connection
 * @param fd return location for the file descriptor.
 * @returns #TRUE if fd is successfully obtained.
 */
dbus_bool_t
dbus_connection_get_socket(DBusConnection              *connection,
                           int                         *fd)
{
  dbus_bool_t retval;
  DBusSocket s = DBUS_SOCKET_INIT;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (connection->transport != NULL, FALSE);
  
  CONNECTION_LOCK (connection);
  
  retval = _dbus_transport_get_socket_fd (connection->transport, &s);

  if (retval)
    {
      *fd = _dbus_socket_get_int (s);
    }

  CONNECTION_UNLOCK (connection);

  return retval;
}


/**
 * Gets the UNIX user ID of the connection if known.  Returns #TRUE if
 * the uid is filled in.  Always returns #FALSE on non-UNIX platforms
 * for now, though in theory someone could hook Windows to NIS or
 * something.  Always returns #FALSE prior to authenticating the
 * connection.
 *
 * The UID is only read by servers from clients; clients can't usually
 * get the UID of servers, because servers do not authenticate to
 * clients.  The returned UID is the UID the connection authenticated
 * as.
 *
 * The message bus is a server and the apps connecting to the bus
 * are clients.
 *
 * You can ask the bus to tell you the UID of another connection though
 * if you like; this is done with dbus_bus_get_unix_user().
 *
 * @param connection the connection
 * @param uid return location for the user ID
 * @returns #TRUE if uid is filled in with a valid user ID
 */
dbus_bool_t
dbus_connection_get_unix_user (DBusConnection *connection,
                               unsigned long  *uid)
{
  dbus_bool_t result;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (uid != NULL, FALSE);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = FALSE;
  else
    result = _dbus_transport_get_unix_user (connection->transport,
                                            uid);

#ifdef DBUS_WIN
  _dbus_assert (!result);
#endif
  
  CONNECTION_UNLOCK (connection);

  return result;
}

/**
 * Gets the process ID of the connection if any.
 * Returns #TRUE if the pid is filled in.
 * Always returns #FALSE prior to authenticating the
 * connection.
 *
 * @param connection the connection
 * @param pid return location for the process ID
 * @returns #TRUE if uid is filled in with a valid process ID
 */
dbus_bool_t
dbus_connection_get_unix_process_id (DBusConnection *connection,
				     unsigned long  *pid)
{
  dbus_bool_t result;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (pid != NULL, FALSE);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = FALSE;
  else
    result = _dbus_transport_get_unix_process_id (connection->transport,
						  pid);

  CONNECTION_UNLOCK (connection);

  return result;
}

/**
 * Gets the ADT audit data of the connection if any.
 * Returns #TRUE if the structure pointer is returned.
 * Always returns #FALSE prior to authenticating the
 * connection.
 *
 * @param connection the connection
 * @param data return location for audit data
 * @param data_size return location for length of audit data
 * @returns #TRUE if audit data is filled in with a valid ucred pointer
 */
dbus_bool_t
dbus_connection_get_adt_audit_session_data (DBusConnection *connection,
					    void          **data,
					    dbus_int32_t   *data_size)
{
  dbus_bool_t result;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (data != NULL, FALSE);
  _dbus_return_val_if_fail (data_size != NULL, FALSE);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = FALSE;
  else
    result = _dbus_transport_get_adt_audit_session_data (connection->transport,
					    	         data,
			  			         data_size);
  CONNECTION_UNLOCK (connection);

  return result;
}

/**
 * Sets a predicate function used to determine whether a given user ID
 * is allowed to connect. When an incoming connection has
 * authenticated with a particular user ID, this function is called;
 * if it returns #TRUE, the connection is allowed to proceed,
 * otherwise the connection is disconnected.
 *
 * If the function is set to #NULL (as it is by default), then
 * only the same UID as the server process will be allowed to
 * connect. Also, root is always allowed to connect.
 *
 * On Windows, the function will be set and its free_data_function will
 * be invoked when the connection is freed or a new function is set.
 * However, the function will never be called, because there are
 * no UNIX user ids to pass to it, or at least none of the existing
 * auth protocols would allow authenticating as a UNIX user on Windows.
 * 
 * @param connection the connection
 * @param function the predicate
 * @param data data to pass to the predicate
 * @param free_data_function function to free the data
 */
void
dbus_connection_set_unix_user_function (DBusConnection             *connection,
                                        DBusAllowUnixUserFunction   function,
                                        void                       *data,
                                        DBusFreeFunction            free_data_function)
{
  void *old_data = NULL;
  DBusFreeFunction old_free_function = NULL;

  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  _dbus_transport_set_unix_user_function (connection->transport,
                                          function, data, free_data_function,
                                          &old_data, &old_free_function);
  CONNECTION_UNLOCK (connection);

  if (old_free_function != NULL)
    (* old_free_function) (old_data);
}

/* Same calling convention as dbus_connection_get_windows_user */
dbus_bool_t
_dbus_connection_get_linux_security_label (DBusConnection  *connection,
                                           char           **label_p)
{
  dbus_bool_t result;

  _dbus_assert (connection != NULL);
  _dbus_assert (label_p != NULL);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = FALSE;
  else
    result = _dbus_transport_get_linux_security_label (connection->transport,
                                                       label_p);
#ifndef __linux__
  _dbus_assert (!result);
#endif

  CONNECTION_UNLOCK (connection);

  return result;
}

DBusCredentials *
_dbus_connection_get_credentials (DBusConnection *connection)
{
  DBusCredentials *result;

  _dbus_assert (connection != NULL);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = NULL;
  else
    result = _dbus_transport_get_credentials (connection->transport);

  CONNECTION_UNLOCK (connection);

  return result;
}

/**
 * Gets the Windows user SID of the connection if known.  Returns
 * #TRUE if the ID is filled in.  Always returns #FALSE on non-Windows
 * platforms for now, though in theory someone could hook UNIX to
 * Active Directory or something.  Always returns #FALSE prior to
 * authenticating the connection.
 *
 * The user is only read by servers from clients; clients can't usually
 * get the user of servers, because servers do not authenticate to
 * clients. The returned user is the user the connection authenticated
 * as.
 *
 * The message bus is a server and the apps connecting to the bus
 * are clients.
 *
 * The returned user string has to be freed with dbus_free().
 *
 * The return value indicates whether the user SID is available;
 * if it's available but we don't have the memory to copy it,
 * then the return value is #TRUE and #NULL is given as the SID.
 * 
 * @todo We would like to be able to say "You can ask the bus to tell
 * you the user of another connection though if you like; this is done
 * with dbus_bus_get_windows_user()." But this has to be implemented
 * in bus/driver.c and dbus/dbus-bus.c, and is pointless anyway
 * since on Windows we only use the session bus for now.
 *
 * @param connection the connection
 * @param windows_sid_p return location for an allocated copy of the user ID, or #NULL if no memory
 * @returns #TRUE if user is available (returned value may be #NULL anyway if no memory)
 */
dbus_bool_t
dbus_connection_get_windows_user (DBusConnection             *connection,
                                  char                      **windows_sid_p)
{
  dbus_bool_t result;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (windows_sid_p != NULL, FALSE);

  CONNECTION_LOCK (connection);

  if (!_dbus_transport_try_to_authenticate (connection->transport))
    result = FALSE;
  else
    result = _dbus_transport_get_windows_user (connection->transport,
                                               windows_sid_p);

#ifdef DBUS_UNIX
  _dbus_assert (!result);
#endif
  
  CONNECTION_UNLOCK (connection);

  return result;
}

/**
 * Sets a predicate function used to determine whether a given user ID
 * is allowed to connect. When an incoming connection has
 * authenticated with a particular user ID, this function is called;
 * if it returns #TRUE, the connection is allowed to proceed,
 * otherwise the connection is disconnected.
 *
 * If the function is set to #NULL (as it is by default), then
 * only the same user owning the server process will be allowed to
 * connect.
 *
 * On UNIX, the function will be set and its free_data_function will
 * be invoked when the connection is freed or a new function is set.
 * However, the function will never be called, because there is no
 * way right now to authenticate as a Windows user on UNIX.
 * 
 * @param connection the connection
 * @param function the predicate
 * @param data data to pass to the predicate
 * @param free_data_function function to free the data
 */
void
dbus_connection_set_windows_user_function (DBusConnection              *connection,
                                           DBusAllowWindowsUserFunction function,
                                           void                        *data,
                                           DBusFreeFunction             free_data_function)
{
  void *old_data = NULL;
  DBusFreeFunction old_free_function = NULL;

  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  _dbus_transport_set_windows_user_function (connection->transport,
                                             function, data, free_data_function,
                                             &old_data, &old_free_function);
  CONNECTION_UNLOCK (connection);

  if (old_free_function != NULL)
    (* old_free_function) (old_data);
}

/**
 * This function must be called on the server side of a connection when the
 * connection is first seen in the #DBusNewConnectionFunction. If set to
 * #TRUE (the default is #FALSE), then the connection can proceed even if
 * the client does not authenticate as some user identity, i.e. clients
 * can connect anonymously.
 * 
 * This setting interacts with the available authorization mechanisms
 * (see dbus_server_set_auth_mechanisms()). Namely, an auth mechanism
 * such as ANONYMOUS that supports anonymous auth must be included in
 * the list of available mechanisms for anonymous login to work.
 *
 * This setting also changes the default rule for connections
 * authorized as a user; normally, if a connection authorizes as
 * a user identity, it is permitted if the user identity is
 * root or the user identity matches the user identity of the server
 * process. If anonymous connections are allowed, however,
 * then any user identity is allowed.
 *
 * You can override the rules for connections authorized as a
 * user identity with dbus_connection_set_unix_user_function()
 * and dbus_connection_set_windows_user_function().
 * 
 * @param connection the connection
 * @param value whether to allow authentication as an anonymous user
 */
void
dbus_connection_set_allow_anonymous (DBusConnection             *connection,
                                     dbus_bool_t                 value)
{
  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  _dbus_transport_set_allow_anonymous (connection->transport, value);
  CONNECTION_UNLOCK (connection);
}

/**
 *
 * Normally #DBusConnection automatically handles all messages to the
 * org.freedesktop.DBus.Peer interface. However, the message bus wants
 * to be able to route methods on that interface through the bus and
 * to other applications. If routing peer messages is enabled, then
 * messages with the org.freedesktop.DBus.Peer interface that also
 * have a bus destination name set will not be automatically
 * handled by the #DBusConnection and instead will be dispatched
 * normally to the application.
 *
 * If a normal application sets this flag, it can break things badly.
 * So don't set this unless you are the message bus.
 *
 * @param connection the connection
 * @param value #TRUE to pass through org.freedesktop.DBus.Peer messages with a bus name set
 */
void
dbus_connection_set_route_peer_messages (DBusConnection             *connection,
                                         dbus_bool_t                 value)
{
  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  connection->route_peer_messages = value;
  CONNECTION_UNLOCK (connection);
}

/**
 * Adds a message filter. Filters are handlers that are run on all
 * incoming messages, prior to the objects registered with
 * dbus_connection_register_object_path().  Filters are run in the
 * order that they were added.  The same handler can be added as a
 * filter more than once, in which case it will be run more than once.
 * Filters added during a filter callback won't be run on the message
 * being processed.
 *
 * @todo we don't run filters on messages while blocking without
 * entering the main loop, since filters are run as part of
 * dbus_connection_dispatch(). This is probably a feature, as filters
 * could create arbitrary reentrancy. But kind of sucks if you're
 * trying to filter METHOD_RETURN for some reason.
 *
 * @param connection the connection
 * @param function function to handle messages
 * @param user_data user data to pass to the function
 * @param free_data_function function to use for freeing user data
 * @returns #TRUE on success, #FALSE if not enough memory.
 */
dbus_bool_t
dbus_connection_add_filter (DBusConnection            *connection,
                            DBusHandleMessageFunction  function,
                            void                      *user_data,
                            DBusFreeFunction           free_data_function)
{
  DBusMessageFilter *filter;
  
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (function != NULL, FALSE);

  filter = dbus_new0 (DBusMessageFilter, 1);
  if (filter == NULL)
    return FALSE;

  _dbus_atomic_inc (&filter->refcount);

  CONNECTION_LOCK (connection);

  if (!_dbus_list_append (&connection->filter_list,
                          filter))
    {
      _dbus_message_filter_unref (filter);
      CONNECTION_UNLOCK (connection);
      return FALSE;
    }

  /* Fill in filter after all memory allocated,
   * so we don't run the free_user_data_function
   * if the add_filter() fails
   */
  
  filter->function = function;
  filter->user_data = user_data;
  filter->free_user_data_function = free_data_function;
        
  CONNECTION_UNLOCK (connection);
  return TRUE;
}

/**
 * Removes a previously-added message filter. It is a programming
 * error to call this function for a handler that has not been added
 * as a filter. If the given handler was added more than once, only
 * one instance of it will be removed (the most recently-added
 * instance).
 *
 * @param connection the connection
 * @param function the handler to remove
 * @param user_data user data for the handler to remove
 *
 */
void
dbus_connection_remove_filter (DBusConnection            *connection,
                               DBusHandleMessageFunction  function,
                               void                      *user_data)
{
  DBusList *link;
  DBusMessageFilter *filter;
  
  _dbus_return_if_fail (connection != NULL);
  _dbus_return_if_fail (function != NULL);
  
  CONNECTION_LOCK (connection);

  filter = NULL;
  
  link = _dbus_list_get_last_link (&connection->filter_list);
  while (link != NULL)
    {
      filter = link->data;

      if (filter->function == function &&
          filter->user_data == user_data)
        {
          _dbus_list_remove_link (&connection->filter_list, link);
          filter->function = NULL;
          
          break;
        }
        
      link = _dbus_list_get_prev_link (&connection->filter_list, link);
      filter = NULL;
    }
  
  CONNECTION_UNLOCK (connection);

#ifndef DBUS_DISABLE_CHECKS
  if (filter == NULL)
    {
      _dbus_warn_check_failed ("Attempt to remove filter function %p user data %p, but no such filter has been added",
                               function, user_data);
      return;
    }
#endif
  
  /* Call application code */
  if (filter->free_user_data_function)
    (* filter->free_user_data_function) (filter->user_data);

  filter->free_user_data_function = NULL;
  filter->user_data = NULL;
  
  _dbus_message_filter_unref (filter);
}

/**
 * Registers a handler for a given path or subsection in the object
 * hierarchy. The given vtable handles messages sent to exactly the
 * given path or also for paths bellow that, depending on fallback
 * parameter.
 *
 * @param connection the connection
 * @param fallback whether to handle messages also for "subdirectory"
 * @param path a '/' delimited string of path elements
 * @param vtable the virtual table
 * @param user_data data to pass to functions in the vtable
 * @param error address where an error can be returned
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) is reported
 */
static dbus_bool_t
_dbus_connection_register_object_path (DBusConnection              *connection,
                                       dbus_bool_t                  fallback,
                                       const char                  *path,
                                       const DBusObjectPathVTable  *vtable,
                                       void                        *user_data,
                                       DBusError                   *error)
{
  char **decomposed_path;
  dbus_bool_t retval;

  if (!_dbus_decompose_path (path, strlen (path), &decomposed_path, NULL))
    return FALSE;

  CONNECTION_LOCK (connection);

  retval = _dbus_object_tree_register (connection->objects,
                                       fallback,
                                       (const char **) decomposed_path, vtable,
                                       user_data, error);

  CONNECTION_UNLOCK (connection);

  dbus_free_string_array (decomposed_path);

  return retval;
}

/**
 * Registers a handler for a given path in the object hierarchy.
 * The given vtable handles messages sent to exactly the given path.
 *
 * @param connection the connection
 * @param path a '/' delimited string of path elements
 * @param vtable the virtual table
 * @param user_data data to pass to functions in the vtable
 * @param error address where an error can be returned
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) is reported
 */
dbus_bool_t
dbus_connection_try_register_object_path (DBusConnection              *connection,
                                          const char                  *path,
                                          const DBusObjectPathVTable  *vtable,
                                          void                        *user_data,
                                          DBusError                   *error)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (path[0] == '/', FALSE);
  _dbus_return_val_if_fail (vtable != NULL, FALSE);

  return _dbus_connection_register_object_path (connection, FALSE, path, vtable, user_data, error);
}

/**
 * Registers a handler for a given path in the object hierarchy.
 * The given vtable handles messages sent to exactly the given path.
 *
 * It is a bug to call this function for object paths which already
 * have a handler. Use dbus_connection_try_register_object_path() if this
 * might be the case.
 *
 * @param connection the connection
 * @param path a '/' delimited string of path elements
 * @param vtable the virtual table
 * @param user_data data to pass to functions in the vtable
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) ocurred
 */
dbus_bool_t
dbus_connection_register_object_path (DBusConnection              *connection,
                                      const char                  *path,
                                      const DBusObjectPathVTable  *vtable,
                                      void                        *user_data)
{
  dbus_bool_t retval;
  DBusError error = DBUS_ERROR_INIT;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (path[0] == '/', FALSE);
  _dbus_return_val_if_fail (vtable != NULL, FALSE);

  retval = _dbus_connection_register_object_path (connection, FALSE, path, vtable, user_data, &error);

  if (dbus_error_has_name (&error, DBUS_ERROR_OBJECT_PATH_IN_USE))
    {
      _dbus_warn ("%s", error.message);
      dbus_error_free (&error);
      return FALSE;
    }

  return retval;
}

/**
 * Registers a fallback handler for a given subsection of the object
 * hierarchy.  The given vtable handles messages at or below the given
 * path. You can use this to establish a default message handling
 * policy for a whole "subdirectory."
 *
 * @param connection the connection
 * @param path a '/' delimited string of path elements
 * @param vtable the virtual table
 * @param user_data data to pass to functions in the vtable
 * @param error address where an error can be returned
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) is reported
 */
dbus_bool_t
dbus_connection_try_register_fallback (DBusConnection              *connection,
                                       const char                  *path,
                                       const DBusObjectPathVTable  *vtable,
                                       void                        *user_data,
                                       DBusError                   *error)
{
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (path[0] == '/', FALSE);
  _dbus_return_val_if_fail (vtable != NULL, FALSE);

  return _dbus_connection_register_object_path (connection, TRUE, path, vtable, user_data, error);
}

/**
 * Registers a fallback handler for a given subsection of the object
 * hierarchy.  The given vtable handles messages at or below the given
 * path. You can use this to establish a default message handling
 * policy for a whole "subdirectory."
 *
 * It is a bug to call this function for object paths which already
 * have a handler. Use dbus_connection_try_register_fallback() if this
 * might be the case.
 *
 * @param connection the connection
 * @param path a '/' delimited string of path elements
 * @param vtable the virtual table
 * @param user_data data to pass to functions in the vtable
 * @returns #FALSE if an error (#DBUS_ERROR_NO_MEMORY or
 *    #DBUS_ERROR_OBJECT_PATH_IN_USE) occured
 */
dbus_bool_t
dbus_connection_register_fallback (DBusConnection              *connection,
                                   const char                  *path,
                                   const DBusObjectPathVTable  *vtable,
                                   void                        *user_data)
{
  dbus_bool_t retval;
  DBusError error = DBUS_ERROR_INIT;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (path[0] == '/', FALSE);
  _dbus_return_val_if_fail (vtable != NULL, FALSE);

  retval = _dbus_connection_register_object_path (connection, TRUE, path, vtable, user_data, &error);

  if (dbus_error_has_name (&error, DBUS_ERROR_OBJECT_PATH_IN_USE))
    {
      _dbus_warn ("%s", error.message);
      dbus_error_free (&error);
      return FALSE;
    }

  return retval;
}

/**
 * Unregisters the handler registered with exactly the given path.
 * It's a bug to call this function for a path that isn't registered.
 * Can unregister both fallback paths and object paths.
 *
 * @param connection the connection
 * @param path a '/' delimited string of path elements
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_connection_unregister_object_path (DBusConnection              *connection,
                                        const char                  *path)
{
  char **decomposed_path;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (path[0] == '/', FALSE);

  if (!_dbus_decompose_path (path, strlen (path), &decomposed_path, NULL))
      return FALSE;

  CONNECTION_LOCK (connection);

  _dbus_object_tree_unregister_and_unlock (connection->objects, (const char **) decomposed_path);

  dbus_free_string_array (decomposed_path);

  return TRUE;
}

/**
 * Gets the user data passed to dbus_connection_register_object_path()
 * or dbus_connection_register_fallback(). If nothing was registered
 * at this path, the data is filled in with #NULL.
 *
 * @param connection the connection
 * @param path the path you registered with
 * @param data_p location to store the user data, or #NULL
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_connection_get_object_path_data (DBusConnection *connection,
                                      const char     *path,
                                      void          **data_p)
{
  char **decomposed_path;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (path != NULL, FALSE);
  _dbus_return_val_if_fail (data_p != NULL, FALSE);

  *data_p = NULL;
  
  if (!_dbus_decompose_path (path, strlen (path), &decomposed_path, NULL))
    return FALSE;
  
  CONNECTION_LOCK (connection);

  *data_p = _dbus_object_tree_get_user_data_unlocked (connection->objects, (const char**) decomposed_path);

  CONNECTION_UNLOCK (connection);

  dbus_free_string_array (decomposed_path);

  return TRUE;
}

/**
 * Lists the registered fallback handlers and object path handlers at
 * the given parent_path. The returned array should be freed with
 * dbus_free_string_array().
 *
 * @param connection the connection
 * @param parent_path the path to list the child handlers of
 * @param child_entries returns #NULL-terminated array of children
 * @returns #FALSE if no memory to allocate the child entries
 */
dbus_bool_t
dbus_connection_list_registered (DBusConnection              *connection,
                                 const char                  *parent_path,
                                 char                      ***child_entries)
{
  char **decomposed_path;
  dbus_bool_t retval;
  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (parent_path != NULL, FALSE);
  _dbus_return_val_if_fail (parent_path[0] == '/', FALSE);
  _dbus_return_val_if_fail (child_entries != NULL, FALSE);

  if (!_dbus_decompose_path (parent_path, strlen (parent_path), &decomposed_path, NULL))
    return FALSE;

  CONNECTION_LOCK (connection);

  retval = _dbus_object_tree_list_registered_and_unlock (connection->objects,
							 (const char **) decomposed_path,
							 child_entries);
  dbus_free_string_array (decomposed_path);

  return retval;
}

static DBusDataSlotAllocator slot_allocator =
  _DBUS_DATA_SLOT_ALLOCATOR_INIT (_DBUS_LOCK_NAME (connection_slots));

/**
 * Allocates an integer ID to be used for storing application-specific
 * data on any DBusConnection. The allocated ID may then be used
 * with dbus_connection_set_data() and dbus_connection_get_data().
 * The passed-in slot must be initialized to -1, and is filled in
 * with the slot ID. If the passed-in slot is not -1, it's assumed
 * to be already allocated, and its refcount is incremented.
 * 
 * The allocated slot is global, i.e. all DBusConnection objects will
 * have a slot with the given integer ID reserved.
 *
 * @param slot_p address of a global variable storing the slot
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_connection_allocate_data_slot (dbus_int32_t *slot_p)
{
  return _dbus_data_slot_allocator_alloc (&slot_allocator,
                                          slot_p);
}

/**
 * Deallocates a global ID for connection data slots.
 * dbus_connection_get_data() and dbus_connection_set_data() may no
 * longer be used with this slot.  Existing data stored on existing
 * DBusConnection objects will be freed when the connection is
 * finalized, but may not be retrieved (and may only be replaced if
 * someone else reallocates the slot).  When the refcount on the
 * passed-in slot reaches 0, it is set to -1.
 *
 * @param slot_p address storing the slot to deallocate
 */
void
dbus_connection_free_data_slot (dbus_int32_t *slot_p)
{
  _dbus_return_if_fail (*slot_p >= 0);
  
  _dbus_data_slot_allocator_free (&slot_allocator, slot_p);
}

/**
 * Stores a pointer on a DBusConnection, along
 * with an optional function to be used for freeing
 * the data when the data is set again, or when
 * the connection is finalized. The slot number
 * must have been allocated with dbus_connection_allocate_data_slot().
 *
 * @note This function does not take the
 * main thread lock on DBusConnection, which allows it to be
 * used from inside watch and timeout functions. (See the
 * note in docs for dbus_connection_set_watch_functions().)
 * A side effect of this is that you need to know there's
 * a reference held on the connection while invoking
 * dbus_connection_set_data(), or the connection could be
 * finalized during dbus_connection_set_data().
 *
 * @param connection the connection
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
dbus_connection_set_data (DBusConnection   *connection,
                          dbus_int32_t      slot,
                          void             *data,
                          DBusFreeFunction  free_data_func)
{
  DBusFreeFunction old_free_func;
  void *old_data;
  dbus_bool_t retval;

  _dbus_return_val_if_fail (connection != NULL, FALSE);
  _dbus_return_val_if_fail (slot >= 0, FALSE);
  
  SLOTS_LOCK (connection);

  retval = _dbus_data_slot_list_set (&slot_allocator,
                                     &connection->slot_list,
                                     slot, data, free_data_func,
                                     &old_free_func, &old_data);
  
  SLOTS_UNLOCK (connection);

  if (retval)
    {
      /* Do the actual free outside the connection lock */
      if (old_free_func)
        (* old_free_func) (old_data);
    }

  return retval;
}

/**
 * Retrieves data previously set with dbus_connection_set_data().
 * The slot must still be allocated (must not have been freed).
 *
 * @note This function does not take the
 * main thread lock on DBusConnection, which allows it to be
 * used from inside watch and timeout functions. (See the
 * note in docs for dbus_connection_set_watch_functions().)
 * A side effect of this is that you need to know there's
 * a reference held on the connection while invoking
 * dbus_connection_get_data(), or the connection could be
 * finalized during dbus_connection_get_data().
 *
 * @param connection the connection
 * @param slot the slot to get data from
 * @returns the data, or #NULL if not found
 */
void*
dbus_connection_get_data (DBusConnection   *connection,
                          dbus_int32_t      slot)
{
  void *res;

  _dbus_return_val_if_fail (connection != NULL, NULL);
  _dbus_return_val_if_fail (slot >= 0, NULL);

  SLOTS_LOCK (connection);

  res = _dbus_data_slot_list_get (&slot_allocator,
                                  &connection->slot_list,
                                  slot);
  
  SLOTS_UNLOCK (connection);

  return res;
}

/**
 * This function sets a global flag for whether dbus_connection_new()
 * will set SIGPIPE behavior to SIG_IGN.
 *
 * @param will_modify_sigpipe #TRUE to allow sigpipe to be set to SIG_IGN
 */
void
dbus_connection_set_change_sigpipe (dbus_bool_t will_modify_sigpipe)
{
  if (will_modify_sigpipe)
    _dbus_atomic_set_nonzero (&_dbus_modify_sigpipe);
  else
    _dbus_atomic_set_zero (&_dbus_modify_sigpipe);
}

/**
 * Specifies the maximum size message this connection is allowed to
 * receive. Larger messages will result in disconnecting the
 * connection.
 * 
 * @param connection a #DBusConnection
 * @param size maximum message size the connection can receive, in bytes
 */
void
dbus_connection_set_max_message_size (DBusConnection *connection,
                                      long            size)
{
  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  _dbus_transport_set_max_message_size (connection->transport,
                                        size);
  CONNECTION_UNLOCK (connection);
}

/**
 * Gets the value set by dbus_connection_set_max_message_size().
 *
 * @param connection the connection
 * @returns the max size of a single message
 */
long
dbus_connection_get_max_message_size (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);
  
  CONNECTION_LOCK (connection);
  res = _dbus_transport_get_max_message_size (connection->transport);
  CONNECTION_UNLOCK (connection);
  return res;
}

/**
 * Specifies the maximum number of unix fds a message on this
 * connection is allowed to receive. Messages with more unix fds will
 * result in disconnecting the connection.
 *
 * @param connection a #DBusConnection
 * @param n maximum message unix fds the connection can receive
 */
void
dbus_connection_set_max_message_unix_fds (DBusConnection *connection,
                                          long            n)
{
  _dbus_return_if_fail (connection != NULL);

  CONNECTION_LOCK (connection);
  _dbus_transport_set_max_message_unix_fds (connection->transport,
                                            n);
  CONNECTION_UNLOCK (connection);
}

/**
 * Gets the value set by dbus_connection_set_max_message_unix_fds().
 *
 * @param connection the connection
 * @returns the max numer of unix fds of a single message
 */
long
dbus_connection_get_max_message_unix_fds (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);

  CONNECTION_LOCK (connection);
  res = _dbus_transport_get_max_message_unix_fds (connection->transport);
  CONNECTION_UNLOCK (connection);
  return res;
}

/**
 * Sets the maximum total number of bytes that can be used for all messages
 * received on this connection. Messages count toward the maximum until
 * they are finalized. When the maximum is reached, the connection will
 * not read more data until some messages are finalized.
 *
 * The semantics of the maximum are: if outstanding messages are
 * already above the maximum, additional messages will not be read.
 * The semantics are not: if the next message would cause us to exceed
 * the maximum, we don't read it. The reason is that we don't know the
 * size of a message until after we read it.
 *
 * Thus, the max live messages size can actually be exceeded
 * by up to the maximum size of a single message.
 * 
 * Also, if we read say 1024 bytes off the wire in a single read(),
 * and that contains a half-dozen small messages, we may exceed the
 * size max by that amount. But this should be inconsequential.
 *
 * This does imply that we can't call read() with a buffer larger
 * than we're willing to exceed this limit by.
 *
 * @param connection the connection
 * @param size the maximum size in bytes of all outstanding messages
 */
void
dbus_connection_set_max_received_size (DBusConnection *connection,
                                       long            size)
{
  _dbus_return_if_fail (connection != NULL);
  
  CONNECTION_LOCK (connection);
  _dbus_transport_set_max_received_size (connection->transport,
                                         size);
  CONNECTION_UNLOCK (connection);
}

/**
 * Gets the value set by dbus_connection_set_max_received_size().
 *
 * @param connection the connection
 * @returns the max size of all live messages
 */
long
dbus_connection_get_max_received_size (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);
  
  CONNECTION_LOCK (connection);
  res = _dbus_transport_get_max_received_size (connection->transport);
  CONNECTION_UNLOCK (connection);
  return res;
}

/**
 * Sets the maximum total number of unix fds that can be used for all messages
 * received on this connection. Messages count toward the maximum until
 * they are finalized. When the maximum is reached, the connection will
 * not read more data until some messages are finalized.
 *
 * The semantics are analogous to those of dbus_connection_set_max_received_size().
 *
 * @param connection the connection
 * @param n the maximum size in bytes of all outstanding messages
 */
void
dbus_connection_set_max_received_unix_fds (DBusConnection *connection,
                                           long            n)
{
  _dbus_return_if_fail (connection != NULL);

  CONNECTION_LOCK (connection);
  _dbus_transport_set_max_received_unix_fds (connection->transport,
                                             n);
  CONNECTION_UNLOCK (connection);
}

/**
 * Gets the value set by dbus_connection_set_max_received_unix_fds().
 *
 * @param connection the connection
 * @returns the max unix fds of all live messages
 */
long
dbus_connection_get_max_received_unix_fds (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);

  CONNECTION_LOCK (connection);
  res = _dbus_transport_get_max_received_unix_fds (connection->transport);
  CONNECTION_UNLOCK (connection);
  return res;
}

/**
 * Gets the approximate size in bytes of all messages in the outgoing
 * message queue. The size is approximate in that you shouldn't use
 * it to decide how many bytes to read off the network or anything
 * of that nature, as optimizations may choose to tell small white lies
 * to avoid performance overhead.
 *
 * @param connection the connection
 * @returns the number of bytes that have been queued up but not sent
 */
long
dbus_connection_get_outgoing_size (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);

  CONNECTION_LOCK (connection);
  res = _dbus_counter_get_size_value (connection->outgoing_counter);
  CONNECTION_UNLOCK (connection);
  return res;
}

#ifdef DBUS_ENABLE_STATS
void
_dbus_connection_get_stats (DBusConnection *connection,
                            dbus_uint32_t  *in_messages,
                            dbus_uint32_t  *in_bytes,
                            dbus_uint32_t  *in_fds,
                            dbus_uint32_t  *in_peak_bytes,
                            dbus_uint32_t  *in_peak_fds,
                            dbus_uint32_t  *out_messages,
                            dbus_uint32_t  *out_bytes,
                            dbus_uint32_t  *out_fds,
                            dbus_uint32_t  *out_peak_bytes,
                            dbus_uint32_t  *out_peak_fds)
{
  CONNECTION_LOCK (connection);

  if (in_messages != NULL)
    *in_messages = connection->n_incoming;

  _dbus_transport_get_stats (connection->transport,
                             in_bytes, in_fds, in_peak_bytes, in_peak_fds);

  if (out_messages != NULL)
    *out_messages = connection->n_outgoing;

  if (out_bytes != NULL)
    *out_bytes = _dbus_counter_get_size_value (connection->outgoing_counter);

  if (out_fds != NULL)
    *out_fds = _dbus_counter_get_unix_fd_value (connection->outgoing_counter);

  if (out_peak_bytes != NULL)
    *out_peak_bytes = _dbus_counter_get_peak_size_value (connection->outgoing_counter);

  if (out_peak_fds != NULL)
    *out_peak_fds = _dbus_counter_get_peak_unix_fd_value (connection->outgoing_counter);

  CONNECTION_UNLOCK (connection);
}
#endif /* DBUS_ENABLE_STATS */

/**
 * Gets the approximate number of uni fds of all messages in the
 * outgoing message queue.
 *
 * @param connection the connection
 * @returns the number of unix fds that have been queued up but not sent
 */
long
dbus_connection_get_outgoing_unix_fds (DBusConnection *connection)
{
  long res;

  _dbus_return_val_if_fail (connection != NULL, 0);

  CONNECTION_LOCK (connection);
  res = _dbus_counter_get_unix_fd_value (connection->outgoing_counter);
  CONNECTION_UNLOCK (connection);
  return res;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
/**
 * Returns the address of the transport object of this connection
 *
 * @param connection the connection
 * @returns the address string
 */
const char*
_dbus_connection_get_address (DBusConnection *connection)
{
  return _dbus_transport_get_address (connection->transport);
}
#endif

/** @} */
