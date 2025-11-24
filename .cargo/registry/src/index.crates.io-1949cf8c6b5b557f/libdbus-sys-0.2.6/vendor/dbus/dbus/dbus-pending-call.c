/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-pending-call.c Object representing a call in progress.
 *
 * Copyright (C) 2002, 2003 Red Hat Inc.
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
#include "dbus-connection-internal.h"
#include "dbus-message-internal.h"
#include "dbus-pending-call-internal.h"
#include "dbus-pending-call.h"
#include "dbus-list.h"
#include "dbus-threads.h"
#include "dbus-test.h"

/**
 * @defgroup DBusPendingCallInternals DBusPendingCall implementation details
 * @ingroup DBusInternals
 * @brief DBusPendingCall private implementation details.
 *
 * The guts of DBusPendingCall and its methods.
 *
 * @{
 */

/**
 * @brief Internals of DBusPendingCall
 *
 * Opaque object representing a reply message that we're waiting for.
 */

/**
 * shorter and more visible way to write _dbus_connection_lock()
 */
#define CONNECTION_LOCK(connection)   _dbus_connection_lock(connection)
/**
 * shorter and more visible way to write _dbus_connection_unlock()
 */
#define CONNECTION_UNLOCK(connection) _dbus_connection_unlock(connection)

/**
 * Implementation details of #DBusPendingCall - all fields are private.
 */
struct DBusPendingCall
{
  DBusAtomic refcount;                            /**< reference count */

  DBusDataSlotList slot_list;                     /**< Data stored by allocated integer ID */
  
  DBusPendingCallNotifyFunction function;         /**< Notifier when reply arrives. */

  DBusConnection *connection;                     /**< Connections we're associated with */
  DBusMessage *reply;                             /**< Reply (after we've received it) */
  DBusTimeout *timeout;                           /**< Timeout */

  DBusList *timeout_link;                         /**< Preallocated timeout response */
  
  dbus_uint32_t reply_serial;                     /**< Expected serial of reply */

  /**
   * TRUE if some thread has taken responsibility for completing this
   * pending call: either the pending call has completed, or it is about
   * to be completed. Protected by the connection lock.
   */
  unsigned int completed : 1;
  /**
   * TRUE if we have added the timeout. Protected by the connection lock.
   */
  unsigned int timeout_added : 1;
};

static void
_dbus_pending_call_trace_ref (DBusPendingCall *pending_call,
    int old_refcount,
    int new_refcount,
    const char *why)
{
#ifdef DBUS_ENABLE_VERBOSE_MODE
  static int enabled = -1;

  _dbus_trace_ref ("DBusPendingCall", pending_call, old_refcount,
      new_refcount, why, "DBUS_PENDING_CALL_TRACE", &enabled);
#endif
}

/* protected by _DBUS_LOCK_pending_call_slots */
static dbus_int32_t notify_user_data_slot = -1;

/**
 * Creates a new pending reply object.
 *
 * @param connection connection where reply will arrive
 * @param timeout_milliseconds length of timeout, -1 (or
 *  #DBUS_TIMEOUT_USE_DEFAULT) for default,
 *  #DBUS_TIMEOUT_INFINITE for no timeout
 * @param timeout_handler timeout handler, takes pending call as data
 * @returns a new #DBusPendingCall or #NULL if no memory.
 */
DBusPendingCall*
_dbus_pending_call_new_unlocked (DBusConnection    *connection,
                                 int                timeout_milliseconds,
                                 DBusTimeoutHandler timeout_handler)
{
  DBusPendingCall *pending;
  DBusTimeout *timeout;

  _dbus_assert (timeout_milliseconds >= 0 || timeout_milliseconds == -1);
 
  if (timeout_milliseconds == -1)
    timeout_milliseconds = _DBUS_DEFAULT_TIMEOUT_VALUE;

  if (!dbus_pending_call_allocate_data_slot (&notify_user_data_slot))
    return NULL;
  
  pending = dbus_new0 (DBusPendingCall, 1);
  
  if (pending == NULL)
    {
      dbus_pending_call_free_data_slot (&notify_user_data_slot);
      return NULL;
    }

  if (timeout_milliseconds != DBUS_TIMEOUT_INFINITE)
    {
      timeout = _dbus_timeout_new (timeout_milliseconds,
                                   timeout_handler,
                                   pending, NULL);  

      if (timeout == NULL)
        {
          dbus_pending_call_free_data_slot (&notify_user_data_slot);
          dbus_free (pending);
          return NULL;
        }

      pending->timeout = timeout;
    }
  else
    {
      pending->timeout = NULL;
    }

  _dbus_atomic_inc (&pending->refcount);
  pending->connection = connection;
  _dbus_connection_ref_unlocked (pending->connection);

  _dbus_data_slot_list_init (&pending->slot_list);

  _dbus_pending_call_trace_ref (pending, 0, 1, "new_unlocked");

  return pending;
}

/**
 * Sets the reply of a pending call with the given message,
 * or if the message is #NULL, by timing out the pending call.
 * 
 * @param pending the pending call
 * @param message the message to complete the call with, or #NULL
 *  to time out the call
 */
void
_dbus_pending_call_set_reply_unlocked (DBusPendingCall *pending,
                                       DBusMessage     *message)
{
  if (message == NULL)
    {
      message = pending->timeout_link->data;
      _dbus_list_clear (&pending->timeout_link);
    }
  else
    dbus_message_ref (message);

  _dbus_verbose ("  handing message %p (%s) to pending call serial %u\n",
                 message,
                 dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN ?
                 "method return" :
                 dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR ?
                 "error" : "other type",
                 pending->reply_serial);
  
  _dbus_assert (pending->reply == NULL);
  _dbus_assert (pending->reply_serial == dbus_message_get_reply_serial (message));
  pending->reply = message;
}

/**
 * Sets the pending call to completed
 *
 * This method is called with the connection lock held, to protect
 * pending->completed. It must be paired with a call to
 * _dbus_pending_call_finish_completion() after the connection lock has
 * been released.
 *
 * @param pending the pending call
 */
void
_dbus_pending_call_start_completion_unlocked (DBusPendingCall *pending)
{
  _dbus_assert (!pending->completed);
  
  pending->completed = TRUE;
}

/**
 * Call the notifier function for the pending call.
 *
 * This method must be called after the connection lock has been
 * released, and must be paired with a call to
 * _dbus_pending_call_start_completion_unlocked().
 *
 * @param pending the pending call
 */
void
_dbus_pending_call_finish_completion (DBusPendingCall *pending)
{
  _dbus_assert (pending->completed);

  if (pending->function)
    {
      void *user_data;
      user_data = dbus_pending_call_get_data (pending,
                                              notify_user_data_slot);
      
      (* pending->function) (pending, user_data);
    }
}

/**
 * If the pending call hasn't been timed out, add its timeout
 * error reply to the connection's incoming message queue.
 *
 * @param pending the pending call
 * @param connection the connection the call was sent to
 */
void
_dbus_pending_call_queue_timeout_error_unlocked (DBusPendingCall *pending, 
                                                 DBusConnection  *connection)
{
  _dbus_assert (connection == pending->connection);
  
  if (pending->timeout_link)
    {
      _dbus_connection_queue_synthesized_message_link (connection,
						       pending->timeout_link);
      pending->timeout_link = NULL;
    }
}

/**
 * Checks to see if a timeout has been added
 *
 * @param pending the pending_call
 * @returns #TRUE if there is a timeout or #FALSE if not
 */
dbus_bool_t 
_dbus_pending_call_is_timeout_added_unlocked (DBusPendingCall  *pending)
{
  _dbus_assert (pending != NULL);

  return pending->timeout_added;
}


/**
 * Sets wether the timeout has been added
 *
 * @param pending the pending_call
 * @param is_added whether or not a timeout is added
 */
void
_dbus_pending_call_set_timeout_added_unlocked (DBusPendingCall  *pending,
                                               dbus_bool_t       is_added)
{
  _dbus_assert (pending != NULL);

  pending->timeout_added = is_added;
}


/**
 * Retrives the timeout
 *
 * @param pending the pending_call
 * @returns a timeout object or NULL if call has no timeout
 */
DBusTimeout *
_dbus_pending_call_get_timeout_unlocked (DBusPendingCall  *pending)
{
  _dbus_assert (pending != NULL);

  return pending->timeout;
}

/**
 * Gets the reply's serial number
 *
 * @param pending the pending_call
 * @returns a serial number for the reply or 0 
 */
dbus_uint32_t 
_dbus_pending_call_get_reply_serial_unlocked (DBusPendingCall  *pending)
{
  _dbus_assert (pending != NULL);

  return pending->reply_serial;
}

/**
 * Sets the reply's serial number
 *
 * @param pending the pending_call
 * @param serial the serial number 
 */
void
_dbus_pending_call_set_reply_serial_unlocked  (DBusPendingCall *pending,
                                               dbus_uint32_t serial)
{
  _dbus_assert (pending != NULL);
  _dbus_assert (pending->reply_serial == 0);

  pending->reply_serial = serial;
}

/**
 * Gets the connection associated with this pending call.
 *
 * @param pending the pending_call
 * @returns the connection associated with the pending call
 */
DBusConnection *
_dbus_pending_call_get_connection_and_lock (DBusPendingCall *pending)
{
  _dbus_assert (pending != NULL);
 
  CONNECTION_LOCK (pending->connection);
  return pending->connection;
}

/**
 * Gets the connection associated with this pending call.
 *
 * @param pending the pending_call
 * @returns the connection associated with the pending call
 */
DBusConnection *
_dbus_pending_call_get_connection_unlocked (DBusPendingCall *pending)
{
  _dbus_assert (pending != NULL);
 
  return pending->connection;
}

/**
 * Sets the reply message associated with the pending call to a timeout error
 *
 * @param pending the pending_call
 * @param message the message we are sending the error reply to 
 * @param serial serial number for the reply
 * @return #FALSE on OOM
 */
dbus_bool_t
_dbus_pending_call_set_timeout_error_unlocked (DBusPendingCall *pending,
                                               DBusMessage     *message,
                                               dbus_uint32_t    serial)
{ 
  DBusList *reply_link;
  DBusMessage *reply;

  reply = dbus_message_new_error (message, DBUS_ERROR_NO_REPLY,
                                  "Did not receive a reply. Possible causes include: "
                                  "the remote application did not send a reply, "
                                  "the message bus security policy blocked the reply, "
                                  "the reply timeout expired, or "
                                  "the network connection was broken.");
  if (reply == NULL)
    return FALSE;

  reply_link = _dbus_list_alloc_link (reply);
  if (reply_link == NULL)
    {
      /* it's OK to unref this, nothing that could have attached a callback
       * has ever seen it */
      dbus_message_unref (reply);
      return FALSE;
    }

  pending->timeout_link = reply_link;

  _dbus_pending_call_set_reply_serial_unlocked (pending, serial);
  
  return TRUE;
}

/**
 * Increments the reference count on a pending call,
 * while the lock on its connection is already held.
 *
 * @param pending the pending call object
 * @returns the pending call object
 */
DBusPendingCall *
_dbus_pending_call_ref_unlocked (DBusPendingCall *pending)
{
  dbus_int32_t old_refcount;

  old_refcount = _dbus_atomic_inc (&pending->refcount);
  _dbus_pending_call_trace_ref (pending, old_refcount, old_refcount + 1,
      "ref_unlocked");

  return pending;
}


static void
_dbus_pending_call_last_unref (DBusPendingCall *pending)
{
  DBusConnection *connection;
  
  /* If we get here, we should be already detached
   * from the connection, or never attached.
   */
  _dbus_assert (!pending->timeout_added);  

  connection = pending->connection;

  /* this assumes we aren't holding connection lock... */
  _dbus_data_slot_list_free (&pending->slot_list);

  if (pending->timeout != NULL)
    _dbus_timeout_unref (pending->timeout);
      
  if (pending->timeout_link)
    {
      dbus_message_unref ((DBusMessage *)pending->timeout_link->data);
      _dbus_list_free_link (pending->timeout_link);
      pending->timeout_link = NULL;
    }

  if (pending->reply)
    {
      dbus_message_unref (pending->reply);
      pending->reply = NULL;
    }
      
  dbus_free (pending);

  dbus_pending_call_free_data_slot (&notify_user_data_slot);

  /* connection lock should not be held. */
  /* Free the connection last to avoid a weird state while
   * calling out to application code where the pending exists
   * but not the connection.
   */
  dbus_connection_unref (connection);
}

/**
 * Decrements the reference count on a pending call,
 * freeing it if the count reaches 0. Assumes
 * connection lock is already held.
 *
 * @param pending the pending call object
 */
void
_dbus_pending_call_unref_and_unlock (DBusPendingCall *pending)
{
  dbus_int32_t old_refcount;

  old_refcount = _dbus_atomic_dec (&pending->refcount);
  _dbus_assert (old_refcount > 0);
  _dbus_pending_call_trace_ref (pending, old_refcount,
      old_refcount - 1, "unref_and_unlock");

  CONNECTION_UNLOCK (pending->connection);

  if (old_refcount == 1)
    _dbus_pending_call_last_unref (pending);
}

/**
 * Checks whether the pending call has received a reply
 * yet, or not. Assumes connection lock is held.
 *
 * @param pending the pending call
 * @returns #TRUE if a reply has been received
 */
dbus_bool_t
_dbus_pending_call_get_completed_unlocked (DBusPendingCall    *pending)
{
  return pending->completed;
}

static DBusDataSlotAllocator slot_allocator =
  _DBUS_DATA_SLOT_ALLOCATOR_INIT (_DBUS_LOCK_NAME (pending_call_slots));

/**
 * Stores a pointer on a #DBusPendingCall, along
 * with an optional function to be used for freeing
 * the data when the data is set again, or when
 * the pending call is finalized. The slot number
 * must have been allocated with dbus_pending_call_allocate_data_slot().
 *
 * @param pending the pending_call
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
_dbus_pending_call_set_data_unlocked (DBusPendingCall  *pending,
                                     dbus_int32_t      slot,
                                     void             *data,
                                     DBusFreeFunction  free_data_func)
{
  DBusFreeFunction old_free_func;
  void *old_data;
  dbus_bool_t retval;

  retval = _dbus_data_slot_list_set (&slot_allocator,
                                     &pending->slot_list,
                                     slot, data, free_data_func,
                                     &old_free_func, &old_data);

  /* Drop locks to call out to app code */
  CONNECTION_UNLOCK (pending->connection);
  
  if (retval)
    {
      if (old_free_func)
        (* old_free_func) (old_data);
    }

  CONNECTION_LOCK (pending->connection);
  
  return retval;
}

/** @} */

/**
 * @defgroup DBusPendingCall DBusPendingCall
 * @ingroup  DBus
 * @brief Pending reply to a method call message
 *
 * A DBusPendingCall is an object representing an
 * expected reply. A #DBusPendingCall can be created
 * when you send a message that should have a reply.
 *
 * @{
 */

/**
 * @def DBUS_TIMEOUT_INFINITE
 *
 * An integer constant representing an infinite timeout. This has the
 * numeric value 0x7fffffff (the largest 32-bit signed integer).
 *
 * For source compatibility with D-Bus versions earlier than 1.4.12, use
 * 0x7fffffff, or INT32_MAX (assuming your platform has it).
 */

/**
 * @def DBUS_TIMEOUT_USE_DEFAULT
 *
 * An integer constant representing a request to use the default timeout.
 * This has numeric value -1.
 *
 * For source compatibility with D-Bus versions earlier than 1.4.12, use a
 * literal -1.
 */

/**
 * @typedef DBusPendingCall
 *
 * Opaque data type representing a message pending.
 */

/**
 * Increments the reference count on a pending call.
 *
 * @param pending the pending call object
 * @returns the pending call object
 */
DBusPendingCall *
dbus_pending_call_ref (DBusPendingCall *pending)
{
  dbus_int32_t old_refcount;

  _dbus_return_val_if_fail (pending != NULL, NULL);

  old_refcount = _dbus_atomic_inc (&pending->refcount);
  _dbus_pending_call_trace_ref (pending, old_refcount, old_refcount + 1,
      "ref");

  return pending;
}

/**
 * Decrements the reference count on a pending call,
 * freeing it if the count reaches 0.
 *
 * @param pending the pending call object
 */
void
dbus_pending_call_unref (DBusPendingCall *pending)
{
  dbus_int32_t old_refcount;

  _dbus_return_if_fail (pending != NULL);

  old_refcount = _dbus_atomic_dec (&pending->refcount);
  _dbus_pending_call_trace_ref (pending, old_refcount, old_refcount - 1,
      "unref");

  if (old_refcount == 1)
    _dbus_pending_call_last_unref(pending);
}

/**
 * Sets a notification function to be called when the reply is
 * received or the pending call times out.
 *
 * @param pending the pending call
 * @param function notifier function
 * @param user_data data to pass to notifier function
 * @param free_user_data function to free the user data
 * @returns #FALSE if not enough memory
 */
dbus_bool_t
dbus_pending_call_set_notify (DBusPendingCall              *pending,
                              DBusPendingCallNotifyFunction function,
                              void                         *user_data,
                              DBusFreeFunction              free_user_data)
{
  dbus_bool_t ret = FALSE;

  _dbus_return_val_if_fail (pending != NULL, FALSE);

  CONNECTION_LOCK (pending->connection);
  
  /* could invoke application code! */
  if (!_dbus_pending_call_set_data_unlocked (pending, notify_user_data_slot,
                                             user_data, free_user_data))
    goto out;
  
  pending->function = function;
  ret = TRUE;

out:
  CONNECTION_UNLOCK (pending->connection);
  
  return ret;
}

/**
 * Cancels the pending call, such that any reply or error received
 * will just be ignored.  Drops the dbus library's internal reference
 * to the #DBusPendingCall so will free the call if nobody else is
 * holding a reference. However you usually get a reference from
 * dbus_connection_send_with_reply() so probably your app owns a ref
 * also.
 *
 * Note that canceling a pending call will <em>not</em> simulate a
 * timed-out call; if a call times out, then a timeout error reply is
 * received. If you cancel the call, no reply is received unless the
 * the reply was already received before you canceled.
 * 
 * @param pending the pending call
 */
void
dbus_pending_call_cancel (DBusPendingCall *pending)
{
  _dbus_return_if_fail (pending != NULL);

  _dbus_connection_remove_pending_call (pending->connection,
                                        pending);
}

/**
 * Checks whether the pending call has received a reply
 * yet, or not.
 *
 * @param pending the pending call
 * @returns #TRUE if a reply has been received
 */
dbus_bool_t
dbus_pending_call_get_completed (DBusPendingCall *pending)
{
  dbus_bool_t completed;
  
  _dbus_return_val_if_fail (pending != NULL, FALSE);

  CONNECTION_LOCK (pending->connection);
  completed = pending->completed;
  CONNECTION_UNLOCK (pending->connection);

  return completed;
}

/**
 * Gets the reply, or returns #NULL if none has been received
 * yet. Ownership of the reply message passes to the caller. This
 * function can only be called once per pending call, since the reply
 * message is tranferred to the caller.
 * 
 * @param pending the pending call
 * @returns the reply message or #NULL.
 */
DBusMessage*
dbus_pending_call_steal_reply (DBusPendingCall *pending)
{
  DBusMessage *message;
  
  _dbus_return_val_if_fail (pending != NULL, NULL);
  _dbus_return_val_if_fail (pending->completed, NULL);
  _dbus_return_val_if_fail (pending->reply != NULL, NULL);

  CONNECTION_LOCK (pending->connection);
  
  message = pending->reply;
  pending->reply = NULL;

  CONNECTION_UNLOCK (pending->connection);

  _dbus_message_trace_ref (message, -1, -1, "dbus_pending_call_steal_reply");
  return message;
}

/**
 * Block until the pending call is completed.  The blocking is as with
 * dbus_connection_send_with_reply_and_block(); it does not enter the
 * main loop or process other messages, it simply waits for the reply
 * in question.
 *
 * If the pending call is already completed, this function returns
 * immediately.
 *
 * @todo when you start blocking, the timeout is reset, but it should
 * really only use time remaining since the pending call was created.
 * This requires storing timestamps instead of intervals in the timeout
 *
 * @param pending the pending call
 */
void
dbus_pending_call_block (DBusPendingCall *pending)
{
  _dbus_return_if_fail (pending != NULL);

  _dbus_connection_block_pending_call (pending);
}

/**
 * Allocates an integer ID to be used for storing application-specific
 * data on any DBusPendingCall. The allocated ID may then be used
 * with dbus_pending_call_set_data() and dbus_pending_call_get_data().
 * The passed-in slot must be initialized to -1, and is filled in
 * with the slot ID. If the passed-in slot is not -1, it's assumed
 * to be already allocated, and its refcount is incremented.
 * 
 * The allocated slot is global, i.e. all DBusPendingCall objects will
 * have a slot with the given integer ID reserved.
 *
 * @param slot_p address of a global variable storing the slot
 * @returns #FALSE on failure (no memory)
 */
dbus_bool_t
dbus_pending_call_allocate_data_slot (dbus_int32_t *slot_p)
{
  _dbus_return_val_if_fail (slot_p != NULL, FALSE);

  return _dbus_data_slot_allocator_alloc (&slot_allocator,
                                          slot_p);
}

/**
 * Deallocates a global ID for #DBusPendingCall data slots.
 * dbus_pending_call_get_data() and dbus_pending_call_set_data() may
 * no longer be used with this slot.  Existing data stored on existing
 * DBusPendingCall objects will be freed when the #DBusPendingCall is
 * finalized, but may not be retrieved (and may only be replaced if
 * someone else reallocates the slot).  When the refcount on the
 * passed-in slot reaches 0, it is set to -1.
 *
 * @param slot_p address storing the slot to deallocate
 */
void
dbus_pending_call_free_data_slot (dbus_int32_t *slot_p)
{
  _dbus_return_if_fail (slot_p != NULL);
  _dbus_return_if_fail (*slot_p >= 0);

  _dbus_data_slot_allocator_free (&slot_allocator, slot_p);
}

/**
 * Stores a pointer on a #DBusPendingCall, along
 * with an optional function to be used for freeing
 * the data when the data is set again, or when
 * the pending call is finalized. The slot number
 * must have been allocated with dbus_pending_call_allocate_data_slot().
 *
 * @param pending the pending_call
 * @param slot the slot number
 * @param data the data to store
 * @param free_data_func finalizer function for the data
 * @returns #TRUE if there was enough memory to store the data
 */
dbus_bool_t
dbus_pending_call_set_data (DBusPendingCall  *pending,
                            dbus_int32_t      slot,
                            void             *data,
                            DBusFreeFunction  free_data_func)
{
  dbus_bool_t retval;
  
  _dbus_return_val_if_fail (pending != NULL, FALSE);
  _dbus_return_val_if_fail (slot >= 0, FALSE);

  
  CONNECTION_LOCK (pending->connection);
  retval = _dbus_pending_call_set_data_unlocked (pending, slot, data, free_data_func);
  CONNECTION_UNLOCK (pending->connection);
  return retval;
}

/**
 * Retrieves data previously set with dbus_pending_call_set_data().
 * The slot must still be allocated (must not have been freed).
 *
 * @param pending the pending_call
 * @param slot the slot to get data from
 * @returns the data, or #NULL if not found
 */
void*
dbus_pending_call_get_data (DBusPendingCall   *pending,
                            dbus_int32_t       slot)
{
  void *res;

  _dbus_return_val_if_fail (pending != NULL, NULL);

  CONNECTION_LOCK (pending->connection);
  res = _dbus_data_slot_list_get (&slot_allocator,
                                  &pending->slot_list,
                                  slot);
  CONNECTION_UNLOCK (pending->connection);

  return res;
}

/** @} */
