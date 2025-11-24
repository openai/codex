/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dispatch.c  Message dispatcher
 *
 * Copyright (C) 2003  CodeFactory AB
 * Copyright (C) 2003, 2004, 2005  Red Hat, Inc.
 * Copyright (C) 2004  Imendio HB
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
#include "dispatch.h"
#include "connection.h"
#include "driver.h"
#include "services.h"
#include "activation.h"
#include "utils.h"
#include "bus.h"
#include "signals.h"
#include "test.h"
#include <dbus/dbus-internals.h>
#include <dbus/dbus-message-internal.h>
#include <dbus/dbus-misc.h>
#include <dbus/dbus-test-tap.h>
#include <string.h>

#ifdef HAVE_UNIX_FD_PASSING
#include <dbus/dbus-sysdeps-unix.h>
#include <unistd.h>
#endif

/* This is hard-coded in the files in valid-config-files-*. We have to use
 * the debug-pipe transport because the tests in this file require that
 * dbus_connection_open_private() does not block. */
#define TEST_DEBUG_PIPE "debug-pipe:name=test-server"

static inline const char *
nonnull (const char *maybe_null,
         const char *if_null)
{
  return (maybe_null ? maybe_null : if_null);
}

static dbus_bool_t
send_one_message (DBusConnection *connection,
                  BusContext     *context,
                  DBusConnection *sender,
                  DBusConnection *addressed_recipient,
                  DBusMessage    *message,
                  BusTransaction *transaction,
                  DBusError      *error)
{
  DBusError stack_error = DBUS_ERROR_INIT;

  if (!bus_context_check_security_policy (context, transaction,
                                          sender,
                                          addressed_recipient,
                                          connection,
                                          message,
                                          NULL,
                                          &stack_error))
    {
      if (!bus_transaction_capture_error_reply (transaction, sender,
                                                &stack_error, message))
        {
          bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                           "broadcast rejected, but not enough "
                           "memory to tell monitors");
        }

      dbus_error_free (&stack_error);
      return TRUE; /* don't send it but don't return an error either */
    }

  if (dbus_message_contains_unix_fds(message) &&
      !dbus_connection_can_send_type(connection, DBUS_TYPE_UNIX_FD))
    {
      dbus_set_error (&stack_error, DBUS_ERROR_NOT_SUPPORTED,
                      "broadcast cannot be delivered to %s (%s) because "
                      "it does not support receiving Unix fds",
                      bus_connection_get_name (connection),
                      bus_connection_get_loginfo (connection));

      if (!bus_transaction_capture_error_reply (transaction, sender,
                                                &stack_error, message))
        {
          bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                           "broadcast with Unix fd not delivered, but not "
                           "enough memory to tell monitors");
        }

      dbus_error_free (&stack_error);
      return TRUE; /* don't send it but don't return an error either */
    }

  if (!bus_transaction_send (transaction, sender, connection, message))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

dbus_bool_t
bus_dispatch_matches (BusTransaction *transaction,
                      DBusConnection *sender,
                      DBusConnection *addressed_recipient,
                      DBusMessage    *message,
                      DBusError      *error)
{
  DBusError tmp_error;
  BusConnections *connections;
  DBusList *recipients;
  BusMatchmaker *matchmaker;
  DBusList *link;
  BusContext *context;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  /* sender and recipient can both be NULL for the bus driver,
   * or for signals with no particular recipient
   */

  _dbus_assert (sender == NULL || bus_connection_is_active (sender));
  _dbus_assert (dbus_message_get_sender (message) != NULL);

  context = bus_transaction_get_context (transaction);

  /* First, send the message to the addressed_recipient, if there is one. */
  if (addressed_recipient != NULL)
    {
      if (!bus_context_check_security_policy (context, transaction,
                                              sender, addressed_recipient,
                                              addressed_recipient,
                                              message, NULL, error))
        return FALSE;

      if (dbus_message_contains_unix_fds (message) &&
          !dbus_connection_can_send_type (addressed_recipient,
                                          DBUS_TYPE_UNIX_FD))
        {
          dbus_set_error (error,
                          DBUS_ERROR_NOT_SUPPORTED,
                          "Tried to send message with Unix file descriptors"
                          "to a client that doesn't support that.");
          return FALSE;
      }

      /* Dispatch the message */
      if (!bus_transaction_send (transaction, sender, addressed_recipient,
                                 message))
        {
          BUS_SET_OOM (error);
          return FALSE;
        }
    }

  /* Now dispatch to others who look interested in this message */
  connections = bus_context_get_connections (context);
  dbus_error_init (&tmp_error);
  matchmaker = bus_context_get_matchmaker (context);

  recipients = NULL;
  if (!bus_matchmaker_get_recipients (matchmaker, connections,
                                      sender, addressed_recipient, message,
                                      &recipients))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  link = _dbus_list_get_first_link (&recipients);
  while (link != NULL)
    {
      DBusConnection *dest;

      dest = link->data;

      if (!send_one_message (dest, context, sender, addressed_recipient,
                             message, transaction, &tmp_error))
        break;

      link = _dbus_list_get_next_link (&recipients, link);
    }

  _dbus_list_clear (&recipients);

  if (dbus_error_is_set (&tmp_error))
    {
      dbus_move_error (&tmp_error, error);
      return FALSE;
    }
  else
    return TRUE;
}

static DBusHandlerResult
bus_dispatch (DBusConnection *connection,
              DBusMessage    *message)
{
  const char *sender, *service_name;
  DBusError error;
  BusTransaction *transaction;
  BusContext *context;
  DBusHandlerResult result;
  DBusConnection *addressed_recipient;

  result = DBUS_HANDLER_RESULT_HANDLED;

  transaction = NULL;
  addressed_recipient = NULL;
  dbus_error_init (&error);

  context = bus_connection_get_context (connection);
  _dbus_assert (context != NULL);

  /* If we can't even allocate an OOM error, we just go to sleep
   * until we can.
   */
  while (!bus_connection_preallocate_oom_error (connection))
    _dbus_wait_for_memory ();

  /* Ref connection in case we disconnect it at some point in here */
  dbus_connection_ref (connection);

  /* Monitors aren't meant to send messages to us. */
  if (bus_connection_is_monitor (connection))
    {
      sender = bus_connection_get_name (connection);

      /* should never happen */
      if (sender == NULL)
        sender = "(unknown)";

      if (dbus_message_is_signal (message,
                                  DBUS_INTERFACE_LOCAL,
                                  "Disconnected"))
        {
          bus_context_log (context, DBUS_SYSTEM_LOG_INFO,
                           "Monitoring connection %s closed.", sender);
          bus_connection_disconnected (connection);
          goto out;
        }
      else
        {
          /* Monitors are not allowed to send messages, because that
           * probably indicates that the monitor is incorrectly replying
           * to its eavesdropped messages, and we want the authors of
           * such monitors to fix them.
           */
          bus_context_log (context, DBUS_SYSTEM_LOG_WARNING,
                           "Monitoring connection %s (%s) is not allowed "
                           "to send messages; closing it. Please fix the "
                           "monitor to not do that. "
                           "(message type=\"%s\" interface=\"%s\" "
                           "member=\"%s\" error name=\"%s\" "
                           "destination=\"%s\")",
                           sender, bus_connection_get_loginfo (connection),
                           dbus_message_type_to_string (
                             dbus_message_get_type (message)),
                           nonnull (dbus_message_get_interface (message),
                                    "(unset)"),
                           nonnull (dbus_message_get_member (message),
                                    "(unset)"),
                           nonnull (dbus_message_get_error_name (message),
                                    "(unset)"),
                           nonnull (dbus_message_get_destination (message),
                                    DBUS_SERVICE_DBUS));
          dbus_connection_close (connection);
          goto out;
        }
    }

  /* Make sure the message does not have any header fields that we
   * don't understand (or validate), so that we can add header fields
   * in future and clients can assume that we have checked them. */
  if (!_dbus_message_remove_unknown_fields (message) ||
      !dbus_message_set_container_instance (message, NULL))
    {
      BUS_SET_OOM (&error);
      goto out;
    }

  service_name = dbus_message_get_destination (message);

#ifdef DBUS_ENABLE_VERBOSE_MODE
  {
    const char *interface_name, *member_name, *error_name;

    interface_name = dbus_message_get_interface (message);
    member_name = dbus_message_get_member (message);
    error_name = dbus_message_get_error_name (message);

    _dbus_verbose ("DISPATCH: %s %s %s to %s\n",
                   interface_name ? interface_name : "(no interface)",
                   member_name ? member_name : "(no member)",
                   error_name ? error_name : "(no error name)",
                   service_name ? service_name : "peer");
  }
#endif /* DBUS_ENABLE_VERBOSE_MODE */

  /* If service_name is NULL, if it's a signal we send it to all
   * connections with a match rule. If it's not a signal, there
   * are some special cases here but mostly we just bail out.
   */
  if (service_name == NULL)
    {
      if (dbus_message_is_signal (message,
                                  DBUS_INTERFACE_LOCAL,
                                  "Disconnected"))
        {
          bus_connection_disconnected (connection);
          goto out;
        }

      if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_SIGNAL)
        {
          /* DBusConnection also handles some of these automatically, we leave
           * it to do so.
           *
           * FIXME: this means monitors won't get the opportunity to see
           * non-signals with NULL destination, or their replies (which in
           * practice are UnknownMethod errors)
           */
          result = DBUS_HANDLER_RESULT_NOT_YET_HANDLED;
          goto out;
        }
    }

  /* Create our transaction */
  transaction = bus_transaction_new (context);
  if (transaction == NULL)
    {
      BUS_SET_OOM (&error);
      goto out;
    }

  /* Assign a sender to the message */
  if (bus_connection_is_active (connection))
    {
      sender = bus_connection_get_name (connection);
      _dbus_assert (sender != NULL);

      if (!dbus_message_set_sender (message, sender))
        {
          BUS_SET_OOM (&error);
          goto out;
        }
    }
  else
    {
      /* For monitors' benefit: we don't want the sender to be able to
       * trick the monitor by supplying a forged sender, and we also
       * don't want the message to have no sender at all. */
      if (!dbus_message_set_sender (message, ":not.active.yet"))
        {
          BUS_SET_OOM (&error);
          goto out;
        }
    }

  /* We need to refetch the service name here, because
   * dbus_message_set_sender can cause the header to be
   * reallocated, and thus the service_name pointer will become
   * invalid.
   */
  service_name = dbus_message_get_destination (message);

  if (service_name &&
      strcmp (service_name, DBUS_SERVICE_DBUS) == 0) /* to bus driver */
    {
      if (!bus_transaction_capture (transaction, connection, NULL, message))
        {
          BUS_SET_OOM (&error);
          goto out;
        }

      if (!bus_context_check_security_policy (context, transaction,
                                              connection, NULL, NULL, message,
                                              NULL, &error))
        {
          _dbus_verbose ("Security policy rejected message\n");
          goto out;
        }

      _dbus_verbose ("Giving message to %s\n", DBUS_SERVICE_DBUS);
      if (!bus_driver_handle_message (connection, transaction, message, &error))
        goto out;
    }
  else if (!bus_connection_is_active (connection)) /* clients must talk to bus driver first */
    {
      if (!bus_transaction_capture (transaction, connection, NULL, message))
        {
          BUS_SET_OOM (&error);
          goto out;
        }

      _dbus_verbose ("Received message from non-registered client. Disconnecting.\n");
      dbus_connection_close (connection);
      goto out;
    }
  else if (service_name != NULL) /* route to named service */
    {
      DBusString service_string;
      BusService *service;
      BusRegistry *registry;

      _dbus_assert (service_name != NULL);

      registry = bus_connection_get_registry (connection);

      _dbus_string_init_const (&service_string, service_name);
      service = bus_registry_lookup (registry, &service_string);

      if (service == NULL && dbus_message_get_auto_start (message))
        {
          BusActivation *activation;

          if (!bus_transaction_capture (transaction, connection, NULL,
                                        message))
            {
              BUS_SET_OOM (&error);
              goto out;
            }

          activation = bus_connection_get_activation (connection);

          /* This will do as much of a security policy check as it can.
           * We can't do the full security policy check here, since the
           * addressed recipient service doesn't exist yet. We do it before
           * sending the message after the service has been created.
           */
          if (!bus_activation_activate_service (activation, connection, transaction, TRUE,
                                                message, service_name, &error))
            {
              _DBUS_ASSERT_ERROR_IS_SET (&error);
              _dbus_verbose ("bus_activation_activate_service() failed: %s\n", error.name);
              goto out;
            }

          goto out;
        }
      else if (service == NULL)
        {
          if (!bus_transaction_capture (transaction, connection,
                                        NULL, message))
            {
              BUS_SET_OOM (&error);
              goto out;
            }

          dbus_set_error (&error,
                          DBUS_ERROR_NAME_HAS_NO_OWNER,
                          "Name \"%s\" does not exist",
                          service_name);
          goto out;
        }
      else
        {
          addressed_recipient = bus_service_get_primary_owners_connection (service);
          _dbus_assert (addressed_recipient != NULL);

          if (!bus_transaction_capture (transaction, connection,
                                        addressed_recipient, message))
            {
              BUS_SET_OOM (&error);
              goto out;
            }
        }
    }
  else /* service_name == NULL */
    {
      if (!bus_transaction_capture (transaction, connection, NULL, message))
        {
          BUS_SET_OOM (&error);
          goto out;
        }
    }

  /* Now send the message to its destination (or not, if
   * addressed_recipient == NULL), and match it against other connections'
   * match rules.
   */
  if (!bus_dispatch_matches (transaction, connection, addressed_recipient, message, &error))
    goto out;

 out:
  if (dbus_error_is_set (&error))
    {
      /* Even if we disconnected it, pretend to send it any pending error
       * messages so that monitors can observe them.
       */
      if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          bus_connection_send_oom_error (connection, message);

          /* cancel transaction due to OOM */
          if (transaction != NULL)
            {
              bus_transaction_cancel_and_free (transaction);
              transaction = NULL;
            }
        }
      else
        {
          /* Try to send the real error, if no mem to do that, send
           * the OOM error
           */
          _dbus_assert (transaction != NULL);
          if (!bus_transaction_send_error_reply (transaction, connection,
                                                 &error, message))
            {
              bus_connection_send_oom_error (connection, message);

              /* cancel transaction due to OOM */
              if (transaction != NULL)
                {
                  bus_transaction_cancel_and_free (transaction);
                  transaction = NULL;
                }
            }
        }


      dbus_error_free (&error);
    }

  if (transaction != NULL)
    {
      bus_transaction_execute_and_free (transaction);
    }

  dbus_connection_unref (connection);

  return result;
}

static DBusHandlerResult
bus_dispatch_message_filter (DBusConnection     *connection,
                             DBusMessage        *message,
                             void               *user_data)
{
  return bus_dispatch (connection, message);
}

dbus_bool_t
bus_dispatch_add_connection (DBusConnection *connection)
{
  if (!dbus_connection_add_filter (connection,
                                   bus_dispatch_message_filter,
                                   NULL, NULL))
    return FALSE;

  return TRUE;
}

void
bus_dispatch_remove_connection (DBusConnection *connection)
{
  /* Here we tell the bus driver that we want to get off. */
  bus_driver_remove_connection (connection);

  dbus_connection_remove_filter (connection,
                                 bus_dispatch_message_filter,
                                 NULL);
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS

#include <stdio.h>

#include "stats.h"

/* This is used to know whether we need to block in order to finish
 * sending a message, or whether the initial dbus_connection_send()
 * already flushed the queue.
 */
#define SEND_PENDING(connection) (dbus_connection_has_messages_to_send (connection))

typedef dbus_bool_t (* Check2Func) (BusContext     *context,
                                    DBusConnection *connection);

static dbus_bool_t check_no_leftovers (BusContext *context);

static void
block_connection_until_message_from_bus (BusContext     *context,
                                         DBusConnection *connection,
                                         const char     *what_is_expected)
{
  _dbus_verbose ("expecting: %s\n", what_is_expected);

  while (dbus_connection_get_dispatch_status (connection) ==
         DBUS_DISPATCH_COMPLETE &&
         dbus_connection_get_is_connected (connection))
    {
      bus_test_run_bus_loop (context, TRUE);
      bus_test_run_clients_loop (FALSE);
    }
}

static void
spin_connection_until_authenticated (BusContext     *context,
                                     DBusConnection *connection)
{
  _dbus_verbose ("Spinning to auth connection %p\n", connection);
  while (!dbus_connection_get_is_authenticated (connection) &&
         dbus_connection_get_is_connected (connection))
    {
      bus_test_run_bus_loop (context, FALSE);
      bus_test_run_clients_loop (FALSE);
    }
  _dbus_verbose (" ... done spinning to auth connection %p\n", connection);
}

/* compensate for fact that pop_message() can return #NULL due to OOM */
static DBusMessage*
pop_message_waiting_for_memory (DBusConnection *connection)
{
  while (dbus_connection_get_dispatch_status (connection) ==
         DBUS_DISPATCH_NEED_MEMORY)
    _dbus_wait_for_memory ();

  return dbus_connection_pop_message (connection);
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
static DBusMessage*
borrow_message_waiting_for_memory (DBusConnection *connection)
{
  while (dbus_connection_get_dispatch_status (connection) ==
         DBUS_DISPATCH_NEED_MEMORY)
    _dbus_wait_for_memory ();

  return dbus_connection_borrow_message (connection);
}
#endif

static void
warn_unexpected_real (DBusConnection *connection,
                      DBusMessage    *message,
                      const char     *expected,
                      const char     *function,
                      int             line)
{
  if (message)
    _dbus_warn ("%s:%d received message interface \"%s\" member \"%s\" error name \"%s\" on %p, expecting %s",
                function, line,
                dbus_message_get_interface (message) ?
                dbus_message_get_interface (message) : "(unset)",
                dbus_message_get_member (message) ?
                dbus_message_get_member (message) : "(unset)",
                dbus_message_get_error_name (message) ?
                dbus_message_get_error_name (message) : "(unset)",
                connection,
                expected);
  else
    _dbus_warn ("%s:%d received no message on %p, expecting %s",
                function, line, connection, expected);
}

#define warn_unexpected(connection, message, expected) \
  warn_unexpected_real (connection, message, expected, _DBUS_FUNCTION_NAME, __LINE__)

static void
verbose_message_received (DBusConnection *connection,
                          DBusMessage    *message)
{
  _dbus_verbose ("Received message interface \"%s\" member \"%s\" error name \"%s\" on %p\n",
                 dbus_message_get_interface (message) ?
                 dbus_message_get_interface (message) : "(unset)",
                 dbus_message_get_member (message) ?
                 dbus_message_get_member (message) : "(unset)",
                 dbus_message_get_error_name (message) ?
                 dbus_message_get_error_name (message) : "(unset)",
                 connection);
}

typedef enum
{
  SERVICE_CREATED,
  OWNER_CHANGED,
  SERVICE_DELETED
} ServiceInfoKind;

typedef struct
{
  ServiceInfoKind expected_kind;
  const char *expected_service_name;
  dbus_bool_t failed;
  DBusConnection *skip_connection;
  BusContext *context;
} CheckServiceOwnerChangedData;

static dbus_bool_t
check_service_owner_changed_foreach (DBusConnection *connection,
                                     void           *data)
{
  CheckServiceOwnerChangedData *d = data;
  DBusMessage *message;
  DBusError error;
  const char *service_name, *old_owner, *new_owner;

  if (d->expected_kind == SERVICE_CREATED
      && connection == d->skip_connection)
    return TRUE;

  dbus_error_init (&error);
  d->failed = TRUE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      block_connection_until_message_from_bus (d->context, connection, "NameOwnerChanged");
      message = pop_message_waiting_for_memory (connection);
      if (message == NULL)
        {
          _dbus_warn ("Did not receive a message on %p, expecting %s",
                      connection, "NameOwnerChanged");
          goto out;
        }
    }
  else if (!dbus_message_is_signal (message,
                                    DBUS_INTERFACE_DBUS,
                                    "NameOwnerChanged"))
    {
      warn_unexpected (connection, message, "NameOwnerChanged");

      goto out;
    }
  else
    {
    reget_service_info_data:
      service_name = NULL;
      old_owner = NULL;
      new_owner = NULL;

      dbus_message_get_args (message, &error,
                             DBUS_TYPE_STRING, &service_name,
                             DBUS_TYPE_STRING, &old_owner,
                             DBUS_TYPE_STRING, &new_owner,
                             DBUS_TYPE_INVALID);

      if (dbus_error_is_set (&error))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto reget_service_info_data;
            }
          else
            {
              _dbus_warn ("Did not get the expected arguments");
              goto out;
            }
        }

      if ((d->expected_kind == SERVICE_CREATED    && ( old_owner[0] || !new_owner[0]))
          || (d->expected_kind == OWNER_CHANGED   && (!old_owner[0] || !new_owner[0]))
          || (d->expected_kind == SERVICE_DELETED && (!old_owner[0] ||  new_owner[0])))
        {
          _dbus_warn ("inconsistent NameOwnerChanged arguments");
          goto out;
        }

      if (strcmp (service_name, d->expected_service_name) != 0)
        {
          _dbus_warn ("expected info on service %s, got info on %s",
                      d->expected_service_name,
                      service_name);
          goto out;
        }

      if (*service_name == ':' && new_owner[0]
          && strcmp (service_name, new_owner) != 0)
        {
          _dbus_warn ("inconsistent ServiceOwnedChanged message (\"%s\" [ %s -> %s ])",
                      service_name, old_owner, new_owner);
          goto out;
        }
    }

  d->failed = FALSE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return !d->failed;
}


static void
kill_client_connection (BusContext     *context,
                        DBusConnection *connection)
{
  char *base_service;
  const char *s;
  CheckServiceOwnerChangedData socd;

  _dbus_verbose ("killing connection %p\n", connection);

  s = dbus_bus_get_unique_name (connection);
  _dbus_assert (s != NULL);

  while ((base_service = _dbus_strdup (s)) == NULL)
    _dbus_wait_for_memory ();

  dbus_connection_ref (connection);

  /* kick in the disconnect handler that unrefs the connection */
  dbus_connection_close (connection);

  bus_test_run_everything (context);

  _dbus_assert (bus_test_client_listed (connection));

  /* Run disconnect handler in test.c */
  if (bus_connection_dispatch_one_message (connection))
    _dbus_test_fatal ("something received on connection being killed other than the disconnect");

  _dbus_assert (!dbus_connection_get_is_connected (connection));
  dbus_connection_unref (connection);
  connection = NULL;
  _dbus_assert (!bus_test_client_listed (connection));

  socd.expected_kind = SERVICE_DELETED;
  socd.expected_service_name = base_service;
  socd.failed = FALSE;
  socd.skip_connection = NULL;
  socd.context = context;

  bus_test_clients_foreach (check_service_owner_changed_foreach,
                            &socd);

  dbus_free (base_service);

  if (socd.failed)
    _dbus_test_fatal ("didn't get the expected NameOwnerChanged (deletion) messages");

  if (!check_no_leftovers (context))
    _dbus_test_fatal ("stuff left in message queues after disconnecting a client");
}

static void
kill_client_connection_unchecked (DBusConnection *connection)
{
  /* This kills the connection without expecting it to affect
   * the rest of the bus.
   */
  _dbus_verbose ("Unchecked kill of connection %p\n", connection);

  dbus_connection_ref (connection);
  dbus_connection_close (connection);
  /* dispatching disconnect handler will unref once */
  if (bus_connection_dispatch_one_message (connection))
    _dbus_test_fatal ("message other than disconnect dispatched after failure to register");

  _dbus_assert (!bus_test_client_listed (connection));
  dbus_connection_unref (connection);
}

typedef struct
{
  dbus_bool_t failed;
} CheckNoMessagesData;

static dbus_bool_t
check_no_messages_foreach (DBusConnection *connection,
                           void           *data)
{
  CheckNoMessagesData *d = data;
  DBusMessage *message;

  message = pop_message_waiting_for_memory (connection);
  if (message != NULL)
    {
      warn_unexpected (connection, message, "no messages");

      d->failed = TRUE;
    }

  if (message)
    dbus_message_unref (message);
  return !d->failed;
}

static dbus_bool_t
check_no_leftovers (BusContext *context)
{
  CheckNoMessagesData nmd;

  nmd.failed = FALSE;
  bus_test_clients_foreach (check_no_messages_foreach,
                            &nmd);

  if (nmd.failed)
    {
      _dbus_verbose ("leftover message found\n");
      return FALSE;
    }
  else
    return TRUE;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_hello_message (BusContext     *context,
                     DBusConnection *connection)
{
  DBusMessage *message;
  DBusMessage *name_message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  DBusError error;
  const char *name;
  const char *acquired;

  retval = FALSE;
  dbus_error_init (&error);
  name = NULL;
  acquired = NULL;
  message = NULL;
  name_message = NULL;

  _dbus_verbose ("check_hello_message for %p\n", connection);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "Hello");

  if (message == NULL)
    return TRUE;

  dbus_connection_ref (connection); /* because we may get disconnected */

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      dbus_connection_unref (connection);
      return TRUE;
    }

  _dbus_assert (dbus_message_has_signature (message, ""));

  dbus_message_unref (message);
  message = NULL;

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected (presumably auth failed)\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected (presumably auth failed)\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  block_connection_until_message_from_bus (context, connection, "reply to Hello");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected (presumably auth failed)\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Hello", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
    {
      _dbus_warn ("Message has wrong sender %s",
                  dbus_message_get_sender (message) ?
                  dbus_message_get_sender (message) : "(none)");
      goto out;
    }

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      CheckServiceOwnerChangedData socd;

      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
        {
          ; /* good, expected */
        }
      else
        {
          warn_unexpected (connection, message, "method return for Hello");

          goto out;
        }

    retry_get_hello_name:
      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_STRING, &name,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              _dbus_verbose ("no memory to get service name arg from hello\n");
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto retry_get_hello_name;
            }
          else
            {
              _dbus_assert (dbus_error_is_set (&error));
              _dbus_warn ("Did not get the expected single string argument to hello");
              goto out;
            }
        }

      _dbus_verbose ("Got hello name: %s\n", name);

      while (!dbus_bus_set_unique_name (connection, name))
        _dbus_wait_for_memory ();

      socd.expected_kind = SERVICE_CREATED;
      socd.expected_service_name = name;
      socd.failed = FALSE;
      socd.skip_connection = connection; /* we haven't done AddMatch so won't get it ourselves */
      socd.context = context;

      bus_test_clients_foreach (check_service_owner_changed_foreach,
                                &socd);

      if (socd.failed)
        goto out;

      name_message = message;
      /* Client should also have gotten ServiceAcquired */

      message = pop_message_waiting_for_memory (connection);
      if (message == NULL)
        {
          block_connection_until_message_from_bus (context, connection, "signal NameAcquired");
          message = pop_message_waiting_for_memory (connection);
          if (message == NULL)
            {
              _dbus_warn ("Expecting %s, got nothing",
                      "NameAcquired");
              goto out;
            }
        }
      if (! dbus_message_is_signal (message, DBUS_INTERFACE_DBUS,
                                    "NameAcquired"))
        {
          _dbus_warn ("Expecting %s, got smthg else",
                      "NameAcquired");
          goto out;
        }

    retry_get_acquired_name:
      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_STRING, &acquired,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              _dbus_verbose ("no memory to get service name arg from acquired\n");
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto retry_get_acquired_name;
            }
          else
            {
              _dbus_assert (dbus_error_is_set (&error));
              _dbus_warn ("Did not get the expected single string argument to ServiceAcquired");
              goto out;
            }
        }

      _dbus_verbose ("Got acquired name: %s\n", acquired);

      if (strcmp (acquired, name) != 0)
        {
          _dbus_warn ("Acquired name is %s but expected %s",
                      acquired, name);
          goto out;
        }
      acquired = NULL;
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  _dbus_verbose ("ending - retval = %d\n", retval);

  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  if (name_message)
    dbus_message_unref (name_message);

  dbus_connection_unref (connection);

  return retval;
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_double_hello_message (BusContext     *context,
                            DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  DBusError error;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  _dbus_verbose ("check_double_hello_message for %p\n", connection);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "Hello");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  dbus_connection_ref (connection); /* because we may get disconnected */
  block_connection_until_message_from_bus (context, connection, "reply to Hello");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Hello", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
    {
      _dbus_warn ("Message has wrong sender %s",
                  dbus_message_get_sender (message) ?
                  dbus_message_get_sender (message) : "(none)");
      goto out;
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_ERROR)
    {
      warn_unexpected (connection, message, "method return for Hello");
      goto out;
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_get_connection_unix_user (BusContext     *context,
                                DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  DBusError error;
  const char *base_service_name;
  dbus_uint32_t uid;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  _dbus_verbose ("check_get_connection_unix_user for %p\n", connection);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "GetConnectionUnixUser");

  if (message == NULL)
    return TRUE;

  base_service_name = dbus_bus_get_unique_name (connection);

  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &base_service_name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  dbus_message_unref (message);
  message = NULL;

  dbus_connection_ref (connection); /* because we may get disconnected */
  block_connection_until_message_from_bus (context, connection, "reply to GetConnectionUnixUser");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "GetConnectionUnixUser", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message, DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
#ifdef DBUS_WIN
      else if (dbus_message_is_error (message, DBUS_ERROR_FAILED))
        {
          /* this is OK, Unix uids aren't meaningful on Windows */
        }
#endif
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
        {
          ; /* good, expected */
        }
      else
        {
          warn_unexpected (connection, message,
                           "method_return for GetConnectionUnixUser");

          goto out;
        }

    retry_get_property:

      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_UINT32, &uid,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              _dbus_verbose ("no memory to get uid by GetConnectionUnixUser\n");
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto retry_get_property;
            }
          else
            {
              _dbus_assert (dbus_error_is_set (&error));
              _dbus_warn ("Did not get the expected DBUS_TYPE_UINT32 from GetConnectionUnixUser");
              goto out;
            }
        }
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_get_connection_unix_process_id (BusContext     *context,
                                      DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  DBusError error;
  const char *base_service_name;
  dbus_uint32_t pid;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  _dbus_verbose ("check_get_connection_unix_process_id for %p\n", connection);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "GetConnectionUnixProcessID");

  if (message == NULL)
    return TRUE;

  base_service_name = dbus_bus_get_unique_name (connection);

  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &base_service_name,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  dbus_message_unref (message);
  message = NULL;

  dbus_connection_ref (connection); /* because we may get disconnected */
  block_connection_until_message_from_bus (context, connection, "reply to GetConnectionUnixProcessID");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "GetConnectionUnixProcessID", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message, DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else
        {
#if defined(__FreeBSD__) || defined(__FreeBSD_kernel__) || \
          defined(__linux__) || \
          defined(__OpenBSD__)
          /* In principle NetBSD should also be in that list, but
           * its implementation of PID-passing doesn't work
           * over a socketpair() as used in the debug-pipe transport.
           * We test this functionality in a more realistic situation
           * in test/dbus-daemon.c. */
          warn_unexpected (connection, message, "not this error");

          goto out;
#else
          _dbus_verbose ("does not support GetConnectionUnixProcessID but perhaps that's OK?\n");
#endif
        }
    }
  else
    {
      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
        {
          ; /* good, expected */
        }
      else
        {
          warn_unexpected (connection, message,
                           "method_return for GetConnectionUnixProcessID");

          goto out;
        }

    retry_get_property:

      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_UINT32, &pid,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              _dbus_verbose ("no memory to get pid by GetConnectionUnixProcessID\n");
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto retry_get_property;
            }
          else
            {
              _dbus_assert (dbus_error_is_set (&error));
              _dbus_warn ("Did not get the expected DBUS_TYPE_UINT32 from GetConnectionUnixProcessID");
              goto out;
            }
        }
      else
        {
          /* test if returned pid is the same as our own pid
           *
           * @todo It would probably be good to restructure the tests
           *       in a way so our parent is the bus that we're testing
           *       cause then we can test that the pid returned matches
           *       getppid()
           */
          if (pid != (dbus_uint32_t) _dbus_getpid ())
            {
              _dbus_assert (dbus_error_is_set (&error));
              _dbus_warn ("Result from GetConnectionUnixProcessID is not our own pid");
              goto out;
            }
        }
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}
#endif

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_add_match (BusContext     *context,
                 DBusConnection *connection,
                 const char     *rule)
{
  DBusMessage *message;
  dbus_bool_t retval;
  dbus_uint32_t serial;
  DBusError error;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  _dbus_verbose ("check_add_match for connection %p, rule %s\n",
                 connection, rule);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "AddMatch");

  if (message == NULL)
    return TRUE;

  if (!dbus_message_append_args (message, DBUS_TYPE_STRING, &rule,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  dbus_connection_ref (connection); /* because we may get disconnected */

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  block_connection_until_message_from_bus (context, connection, "reply to AddMatch");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "AddMatch", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
    {
      _dbus_warn ("Message has wrong sender %s",
                  dbus_message_get_sender (message) ?
                  dbus_message_get_sender (message) : "(none)");
      goto out;
    }

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
        {
          ; /* good, expected */
          _dbus_assert (dbus_message_get_reply_serial (message) == serial);
        }
      else
        {
          warn_unexpected (connection, message, "method return for AddMatch");

          goto out;
        }
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}

#if defined(ENABLE_TRADITIONAL_ACTIVATION) && defined(DBUS_ENABLE_STATS)
/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_get_all_match_rules (BusContext     *context,
                           DBusConnection *connection)
{
  DBusMessage *message;
  dbus_bool_t retval;
  dbus_uint32_t serial;
  DBusError error;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  _dbus_verbose ("check_get_all_match_rules for connection %p\n",
                 connection);

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          BUS_INTERFACE_STATS,
                                          "GetAllMatchRules");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  dbus_connection_ref (connection); /* because we may get disconnected */

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  block_connection_until_message_from_bus (context, connection, "reply to AddMatch");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "AddMatch", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
    {
      _dbus_warn ("Message has wrong sender %s",
                  dbus_message_get_sender (message) ?
                  dbus_message_get_sender (message) : "(none)");
      goto out;
    }

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
        {
          ; /* good, expected */
          _dbus_assert (dbus_message_get_reply_serial (message) == serial);
        }
      else
        {
          warn_unexpected (connection, message, "method return for AddMatch");

          goto out;
        }
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}
#endif

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_hello_connection (BusContext     *context,
                        DBusConnection *nil _DBUS_GNUC_UNUSED)
{
  DBusConnection *connection;
  DBusError error;

  dbus_error_init (&error);

  connection = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (connection == NULL)
    {
      _DBUS_ASSERT_ERROR_IS_SET (&error);
      dbus_error_free (&error);
      return TRUE;
    }

  if (!bus_setup_debug_client (connection))
    {
      dbus_connection_close (connection);
      dbus_connection_unref (connection);
      return TRUE;
    }

  spin_connection_until_authenticated (context, connection);

  if (!check_hello_message (context, connection))
    return FALSE;

  if (dbus_bus_get_unique_name (connection) == NULL)
    {
      /* We didn't successfully register, so we can't
       * do the usual kill_client_connection() checks
       */
      kill_client_connection_unchecked (connection);
    }
  else
    {
      if (!check_add_match (context, connection, ""))
        return FALSE;

      kill_client_connection (context, connection);
    }

  return TRUE;
}

#define NONEXISTENT_SERVICE_NAME "test.this.service.does.not.exist.ewuoiurjdfxcvn"

#ifdef ENABLE_TRADITIONAL_ACTIVATION
/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_nonexistent_service_no_auto_start (BusContext     *context,
                                         DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  const char *nonexistent = NONEXISTENT_SERVICE_NAME;
  dbus_uint32_t flags;

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "StartServiceByName");

  if (message == NULL)
    return TRUE;

  dbus_message_set_auto_start (message, FALSE);

  flags = 0;
  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &nonexistent,
                                 DBUS_TYPE_UINT32, &flags,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to ActivateService on nonexistent");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "StartServiceByName", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SERVICE_UNKNOWN))
        {
          ; /* good, this is expected also */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");
          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully activate %s",
                  NONEXISTENT_SERVICE_NAME);
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_nonexistent_service_auto_start (BusContext     *context,
                                      DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (NONEXISTENT_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to Echo");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);

  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SERVICE_UNKNOWN))
        {
          ; /* good, this is expected also */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");
          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully activate %s",
                  NONEXISTENT_SERVICE_NAME);
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

static dbus_bool_t
check_base_service_activated (BusContext     *context,
                              DBusConnection *connection,
                              DBusMessage    *initial_message,
                              const char    **base_service_p)
{
  DBusMessage *message;
  dbus_bool_t retval;
  DBusError error;
  const char *base_service, *base_service_from_bus, *old_owner;

  retval = FALSE;

  dbus_error_init (&error);
  base_service = NULL;
  old_owner = NULL;
  base_service_from_bus = NULL;

  message = initial_message;
  dbus_message_ref (message);

  if (dbus_message_is_signal (message,
                              DBUS_INTERFACE_DBUS,
                              "NameOwnerChanged"))
    {
      CheckServiceOwnerChangedData socd;

    reget_service_name_arg:
      base_service = NULL;
      old_owner = NULL;
      base_service_from_bus = NULL;

      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_STRING, &base_service,
                                  DBUS_TYPE_STRING, &old_owner,
                                  DBUS_TYPE_STRING, &base_service_from_bus,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto reget_service_name_arg;
            }
          else
            {
              _dbus_warn ("Message %s doesn't have a service name: %s",
                          "NameOwnerChanged (creation)",
                          error.message);
              goto out;
            }
        }

      if (*base_service != ':')
        {
          _dbus_warn ("Expected base service activation, got \"%s\" instead",
                      base_service);
          goto out;
        }

      if (strcmp (base_service, base_service_from_bus) != 0)
        {
          _dbus_warn ("Expected base service activation, got \"%s\" instead with owner \"%s\"",
                      base_service, base_service_from_bus);
          goto out;
        }

      if (old_owner[0])
        {
          _dbus_warn ("Received an old_owner argument during base service activation, \"%s\"",
                      old_owner);
          goto out;
        }

      socd.expected_kind = SERVICE_CREATED;
      socd.expected_service_name = base_service;
      socd.failed = FALSE;
      socd.skip_connection = connection;
      socd.context = context;

      bus_test_clients_foreach (check_service_owner_changed_foreach,
                                &socd);

      if (socd.failed)
        goto out;
    }
  else
    {
      warn_unexpected (connection, message, "NameOwnerChanged (creation) for base service");

      goto out;
    }

  if (base_service_p)
    *base_service_p = base_service;

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);
  dbus_error_free (&error);

  return retval;
}

static dbus_bool_t
check_service_activated (BusContext     *context,
                         DBusConnection *connection,
                         const char     *activated_name,
                         const char     *base_service_name,
                         DBusMessage    *initial_message)
{
  DBusMessage *message;
  dbus_bool_t retval;
  DBusError error;
  dbus_uint32_t activation_result;

  retval = FALSE;

  dbus_error_init (&error);

  message = initial_message;
  dbus_message_ref (message);

  if (dbus_message_is_signal (message,
                              DBUS_INTERFACE_DBUS,
                              "NameOwnerChanged"))
    {
      CheckServiceOwnerChangedData socd;
      const char *service_name, *base_service_from_bus, *old_owner;

    reget_service_name_arg:
      service_name = NULL;
      old_owner = NULL;
      base_service_from_bus = NULL;

      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_STRING, &service_name,
                                   DBUS_TYPE_STRING, &old_owner,
                                  DBUS_TYPE_STRING, &base_service_from_bus,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto reget_service_name_arg;
            }
          else
            {
              _dbus_warn ("Message %s doesn't have a service name: %s",
                          "NameOwnerChanged (creation)",
                          error.message);
              goto out;
            }
        }

      if (strcmp (service_name, activated_name) != 0)
        {
          _dbus_warn ("Expected to see service %s created, saw %s instead",
                      activated_name, service_name);
          goto out;
        }

      if (strcmp (base_service_name, base_service_from_bus) != 0)
        {
          _dbus_warn ("NameOwnerChanged reports wrong base service: %s owner, expected %s instead",
                      base_service_from_bus, base_service_name);
          goto out;
        }

      if (old_owner[0])
        {
          _dbus_warn ("expected a %s, got a %s",
                      "NameOwnerChanged (creation)",
                      "NameOwnerChanged (change)");
          goto out;
        }

      socd.expected_kind = SERVICE_CREATED;
      socd.skip_connection = connection;
      socd.failed = FALSE;
      socd.expected_service_name = service_name;
      socd.context = context;

      bus_test_clients_foreach (check_service_owner_changed_foreach,
                                &socd);

      if (socd.failed)
        goto out;

      dbus_message_unref (message);
      service_name = NULL;
      old_owner = NULL;
      base_service_from_bus = NULL;

      message = pop_message_waiting_for_memory (connection);
      if (message == NULL)
        {
          _dbus_warn ("Expected a reply to %s, got nothing",
                      "StartServiceByName");
          goto out;
        }
    }
  else
    {
      warn_unexpected (connection, message, "NameOwnerChanged for the activated name");

      goto out;
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_METHOD_RETURN)
    {
      warn_unexpected (connection, message, "reply to StartServiceByName");

      goto out;
    }

  activation_result = 0;
  if (!dbus_message_get_args (message, &error,
                              DBUS_TYPE_UINT32, &activation_result,
                              DBUS_TYPE_INVALID))
    {
      if (!dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
        {
          _dbus_warn ("Did not have activation result first argument to %s: %s",
                      "StartServiceByName", error.message);
          goto out;
        }

      dbus_error_free (&error);
    }
  else
    {
      if (activation_result == DBUS_START_REPLY_SUCCESS)
        ; /* Good */
      else if (activation_result == DBUS_START_REPLY_ALREADY_RUNNING)
        ; /* Good also */
      else
        {
          _dbus_warn ("Activation result was %u, no good.",
                      activation_result);
          goto out;
        }
    }

  dbus_message_unref (message);
  message = NULL;

  if (!check_no_leftovers (context))
    {
      _dbus_warn ("Messages were left over after verifying existent activation results");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);
  dbus_error_free (&error);

  return retval;
}

static dbus_bool_t
check_service_auto_activated (BusContext     *context,
                              DBusConnection *connection,
                              const char     *activated_name,
                              const char     *base_service_name,
                              DBusMessage    *initial_message)
{
  DBusMessage *message;
  dbus_bool_t retval;
  DBusError error;

  retval = FALSE;

  dbus_error_init (&error);

  message = initial_message;
  dbus_message_ref (message);

  if (dbus_message_is_signal (message,
                              DBUS_INTERFACE_DBUS,
                              "NameOwnerChanged"))
    {
      const char *service_name;
      CheckServiceOwnerChangedData socd;

    reget_service_name_arg:
      if (!dbus_message_get_args (message, &error,
                                  DBUS_TYPE_STRING, &service_name,
                                  DBUS_TYPE_INVALID))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              _dbus_wait_for_memory ();
              goto reget_service_name_arg;
            }
          else
            {
              _dbus_warn ("Message %s doesn't have a service name: %s",
                          "NameOwnerChanged",
                          error.message);
              dbus_error_free (&error);
              goto out;
            }
        }

      if (strcmp (service_name, activated_name) != 0)
        {
          _dbus_warn ("Expected to see service %s created, saw %s instead",
                      activated_name, service_name);
          goto out;
        }

      socd.expected_kind = SERVICE_CREATED;
      socd.expected_service_name = service_name;
      socd.failed = FALSE;
      socd.skip_connection = connection;
      socd.context = context;

      bus_test_clients_foreach (check_service_owner_changed_foreach,
                                &socd);

      if (socd.failed)
        goto out;

      /* Note that this differs from regular activation in that we don't get a
       * reply to ActivateService here.
       */

      dbus_message_unref (message);
      message = NULL;
      service_name = NULL;
    }
  else
    {
      warn_unexpected (connection, message, "NameOwnerChanged for the activated name");

      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

static dbus_bool_t
check_service_deactivated (BusContext     *context,
                           DBusConnection *connection,
                           const char     *activated_name,
                           const char     *base_service)
{
  dbus_bool_t retval;
  CheckServiceOwnerChangedData socd;

  retval = FALSE;

  /* Now we are expecting ServiceOwnerChanged (deletion) messages for the base
   * service and the activated_name.  The base service
   * notification is required to come last.
   */
  socd.expected_kind = SERVICE_DELETED;
  socd.expected_service_name = activated_name;
  socd.failed = FALSE;
  socd.skip_connection = NULL;
  socd.context = context;

  bus_test_clients_foreach (check_service_owner_changed_foreach,
                            &socd);

  if (socd.failed)
    goto out;

  socd.expected_kind = SERVICE_DELETED;
  socd.expected_service_name = base_service;
  socd.failed = FALSE;
  socd.skip_connection = NULL;
  socd.context = context;

  bus_test_clients_foreach (check_service_owner_changed_foreach,
                            &socd);

  if (socd.failed)
    goto out;

  retval = TRUE;

 out:
  return retval;
}

static dbus_bool_t
check_send_exit_to_service (BusContext     *context,
                            DBusConnection *connection,
                            const char     *service_name,
                            const char     *base_service)
{
  dbus_bool_t got_error;
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  _dbus_verbose ("Sending exit message to the test service\n");

  retval = FALSE;

  /* Kill off the test service by sending it a quit message */
  message = dbus_message_new_method_call (service_name,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Exit");

  if (message == NULL)
    {
      /* Do this again; we still need the service to exit... */
      if (!check_send_exit_to_service (context, connection,
                                       service_name, base_service))
        goto out;

      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);

      /* Do this again; we still need the service to exit... */
      if (!check_send_exit_to_service (context, connection,
                                       service_name, base_service))
        goto out;

      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  /* send message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  /* read it in and write it out to test service */
  bus_test_run_bus_loop (context, FALSE);

  /* see if we got an error during message bus dispatching */
  bus_test_run_clients_loop (FALSE);
  message = borrow_message_waiting_for_memory (connection);
  got_error = message != NULL && dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR;
  if (message)
    {
      dbus_connection_return_message (connection, message);
      message = NULL;
    }

  if (!got_error)
    {
      /* If no error, wait for the test service to exit */
      block_connection_until_message_from_bus (context, connection, "test service to exit");

      bus_test_run_everything (context);
    }

  if (got_error)
    {
      message = pop_message_waiting_for_memory (connection);
      _dbus_assert (message != NULL);

      if (dbus_message_get_reply_serial (message) != serial)
        {
          warn_unexpected (connection, message,
                           "error with the correct reply serial");
          goto out;
        }

      if (!dbus_message_is_error (message,
                                  DBUS_ERROR_NO_MEMORY))
        {
          warn_unexpected (connection, message,
                           "a no memory error from asking test service to exit");
          goto out;
        }

      _dbus_verbose ("Got error %s when asking test service to exit\n",
                     dbus_message_get_error_name (message));

      /* Do this again; we still need the service to exit... */
      if (!check_send_exit_to_service (context, connection,
                                       service_name, base_service))
        goto out;
    }
  else
    {
      if (!check_service_deactivated (context, connection,
                                      service_name, base_service))
        goto out;

      /* Should now have a NoReply error from the Exit() method
       * call; it should have come after all the deactivation
       * stuff.
       */
      message = pop_message_waiting_for_memory (connection);

      if (message == NULL)
        {
          warn_unexpected (connection, NULL,
                           "reply to Exit() method call");
          goto out;
        }
      if (!dbus_message_is_error (message,
                                  DBUS_ERROR_NO_REPLY))
        {
          warn_unexpected (connection, message,
                           "NoReply error from Exit() method call");
          goto out;
        }

      if (dbus_message_get_reply_serial (message) != serial)
        {
          warn_unexpected (connection, message,
                           "error with the correct reply serial");
          goto out;
        }

      _dbus_verbose ("Got error %s after test service exited\n",
                     dbus_message_get_error_name (message));

      if (!check_no_leftovers (context))
        {
          _dbus_warn ("Messages were left over after %s",
                      _DBUS_FUNCTION_NAME);
          goto out;
        }
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

static dbus_bool_t
check_got_error (BusContext     *context,
                 DBusConnection *connection,
                 const char     *first_error_name,
                 ...)
{
  DBusMessage *message;
  dbus_bool_t retval;
  va_list ap;
  dbus_bool_t error_found;
  const char *error_name;

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not get an expected error");
      goto out;
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_ERROR)
    {
      warn_unexpected (connection, message, "an error");

      goto out;
    }

  error_found = FALSE;

  va_start (ap, first_error_name);
  error_name = first_error_name;
  while (error_name != NULL)
    {
      if (dbus_message_is_error (message, error_name))
        {
          error_found = TRUE;
          break;
        }
      error_name = va_arg (ap, char*);
    }
  va_end (ap);

  if (!error_found)
    {
      _dbus_warn ("Expected error %s or other, got %s instead",
                  first_error_name,
                  dbus_message_get_error_name (message));
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

typedef enum
{
  GOT_SERVICE_CREATED,
  GOT_SERVICE_DELETED,
  GOT_ERROR,
  GOT_SOMETHING_ELSE
} GotServiceInfo;

static GotServiceInfo
check_got_service_info (DBusMessage *message)
{
  GotServiceInfo message_kind;

  if (dbus_message_is_signal (message,
                              DBUS_INTERFACE_DBUS,
                              "NameOwnerChanged"))
    {
      DBusError error;
      const char *service_name, *old_owner, *new_owner;
      dbus_error_init (&error);

    reget_service_info_data:
      service_name = NULL;
      old_owner = NULL;
      new_owner = NULL;

      dbus_message_get_args (message, &error,
                             DBUS_TYPE_STRING, &service_name,
                             DBUS_TYPE_STRING, &old_owner,
                             DBUS_TYPE_STRING, &new_owner,
                             DBUS_TYPE_INVALID);
      if (dbus_error_is_set (&error))
        {
          if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
            {
              dbus_error_free (&error);
              goto reget_service_info_data;
            }
          else
            {
              _dbus_warn ("unexpected arguments for NameOwnerChanged message");
              message_kind = GOT_SOMETHING_ELSE;
            }
        }
      else if (!old_owner[0])
        message_kind = GOT_SERVICE_CREATED;
      else if (!new_owner[0])
        message_kind = GOT_SERVICE_DELETED;
      else
        message_kind = GOT_SOMETHING_ELSE;

      dbus_error_free (&error);
    }
  else if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    message_kind = GOT_ERROR;
  else
    message_kind = GOT_SOMETHING_ELSE;

  return message_kind;
}

#define EXISTENT_SERVICE_NAME "org.freedesktop.DBus.TestSuiteEchoService"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_existent_service_no_auto_start (BusContext     *context,
                                      DBusConnection *connection)
{
  DBusMessage *message;
  DBusMessage *base_service_message;
  const char *base_service;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  const char *existent = EXISTENT_SERVICE_NAME;
  dbus_uint32_t flags;

  base_service_message = NULL;

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "StartServiceByName");

  if (message == NULL)
    return TRUE;

  dbus_message_set_auto_start (message, FALSE);

  flags = 0;
  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &existent,
                                 DBUS_TYPE_UINT32, &flags,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* now wait for the message bus to hear back from the activated
   * service.
   */
  block_connection_until_message_from_bus (context, connection, "activated service to connect");

  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive any messages after %s %d on %p",
                  "StartServiceByName", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);
  _dbus_verbose ("  (after sending %s)\n", "StartServiceByName");

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_EXITED) ||
               dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_SIGNALED) ||
               dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_EXEC_FAILED))
        {
          ; /* good, this is expected also */
        }
      else
        {
          _dbus_warn ("Did not expect error %s",
                      dbus_message_get_error_name (message));
          goto out;
        }
    }
  else
    {
      GotServiceInfo message_kind;

      if (!check_base_service_activated (context, connection,
                                         message, &base_service))
        goto out;

      base_service_message = message;
      message = NULL;

      /* We may need to block here for the test service to exit or finish up */
      block_connection_until_message_from_bus (context, connection, "test service to exit or finish up");

      message = dbus_connection_borrow_message (connection);
      if (message == NULL)
        {
          _dbus_warn ("Did not receive any messages after base service creation notification");
          goto out;
        }

      message_kind = check_got_service_info (message);

      dbus_connection_return_message (connection, message);
      message = NULL;

      switch (message_kind)
        {
        case GOT_SOMETHING_ELSE:
        default:
          _dbus_warn ("Unexpected message after ActivateService "
                      "(should be an error or a service announcement");
          goto out;

        case GOT_ERROR:
          if (!check_got_error (context, connection,
                                DBUS_ERROR_SPAWN_CHILD_EXITED,
                                DBUS_ERROR_NO_MEMORY,
                                NULL))
            goto out;
          /* A service deleted should be coming along now after this error.
           * We can also get the error *after* the service deleted.
           */

          /* fall through */

        case GOT_SERVICE_DELETED:
          {
            /* The service started up and got a base address, but then
             * failed to register under EXISTENT_SERVICE_NAME
             */
            CheckServiceOwnerChangedData socd;

            socd.expected_kind = SERVICE_DELETED;
            socd.expected_service_name = base_service;
            socd.failed = FALSE;
            socd.skip_connection = NULL;
            socd.context = context;

            bus_test_clients_foreach (check_service_owner_changed_foreach,
                                      &socd);

            if (socd.failed)
              goto out;

            /* Now we should get an error about the service exiting
             * if we didn't get it before.
             */
            if (message_kind != GOT_ERROR)
              {
                block_connection_until_message_from_bus (context, connection, "error about service exiting");

                /* and process everything again */
                bus_test_run_everything (context);

                if (!check_got_error (context, connection,
                                      DBUS_ERROR_SPAWN_CHILD_EXITED,
				      DBUS_ERROR_NO_MEMORY,
                                      NULL))
                  goto out;
              }
            break;
          }

        case GOT_SERVICE_CREATED:
          message = pop_message_waiting_for_memory (connection);
          if (message == NULL)
            {
              _dbus_warn ("Failed to pop message we just put back! "
                          "should have been a NameOwnerChanged (creation)");
              goto out;
            }

          if (!check_service_activated (context, connection, EXISTENT_SERVICE_NAME,
                                        base_service, message))
            goto out;

          dbus_message_unref (message);
          message = NULL;

          if (!check_no_leftovers (context))
            {
              _dbus_warn ("Messages were left over after successful activation");
              goto out;
            }

	  if (!check_send_exit_to_service (context, connection,
                                           EXISTENT_SERVICE_NAME, base_service))
	    goto out;

          break;
        }
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  if (base_service_message)
    dbus_message_unref (base_service_message);

  return retval;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_segfault_service_no_auto_start (BusContext     *context,
                                      DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  const char *segv_service;
  dbus_uint32_t flags;

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
                                          DBUS_PATH_DBUS,
                                          DBUS_INTERFACE_DBUS,
                                          "StartServiceByName");

  if (message == NULL)
    return TRUE;

  dbus_message_set_auto_start (message, FALSE);

  segv_service = "org.freedesktop.DBus.TestSuiteSegfaultService";
  flags = 0;
  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &segv_service,
                                 DBUS_TYPE_UINT32, &flags,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to activating segfault service");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "StartServiceByName", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_FAILED))
        {
          const char *servicehelper;
          servicehelper = bus_context_get_servicehelper (context);
          /* make sure this only happens with the launch helper */
          _dbus_assert (servicehelper != NULL);
        }
#ifdef DBUS_WIN
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_EXITED))
        {
          /* unhandled exceptions are normal exit codes */
        }
#else
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_SIGNALED))
        {
          ; /* good, this is expected also */
        }
#endif
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully activate segfault service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}


/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_segfault_service_auto_start (BusContext     *context,
                                   DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call ("org.freedesktop.DBus.TestSuiteSegfaultService",
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to Echo on segfault service");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
#ifdef DBUS_WIN
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_EXITED))
        {
          /* unhandled exceptions are normal exit codes */
        }
#else
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_CHILD_SIGNALED))
        {
          ; /* good, this is expected also */
        }
#endif
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully activate segfault service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}
#endif

#define TEST_ECHO_MESSAGE "Test echo message"
#define TEST_RUN_HELLO_FROM_SELF_MESSAGE "Test sending message to self"

#ifdef ENABLE_TRADITIONAL_ACTIVATION
/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_existent_hello_from_self (BusContext     *context,
                                DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  const char *text;

  message = dbus_message_new_method_call (EXISTENT_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "RunHelloFromSelf");

  if (message == NULL)
    return TRUE;

  text = TEST_RUN_HELLO_FROM_SELF_MESSAGE;
  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &text,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* Note: if this test is run in OOM mode, it will block when the bus
   * doesn't send a reply due to OOM.
   */
  block_connection_until_message_from_bus (context, connection, "reply from running hello from self");

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Failed to pop message! Should have been reply from RunHelloFromSelf message");
      return FALSE;
    }

  if (dbus_message_get_reply_serial (message) != serial)
    {
      _dbus_warn ("Wrong reply serial");
      dbus_message_unref (message);
      return FALSE;
    }

  dbus_message_unref (message);
  message = NULL;

  return TRUE;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_existent_ping (BusContext     *context,
                     DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  message = dbus_message_new_method_call (EXISTENT_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.DBus.Peer",
                                          "Ping");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* Note: if this test is run in OOM mode, it will block when the bus
   * doesn't send a reply due to OOM.
   */
  block_connection_until_message_from_bus (context, connection, "reply from running Ping");

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Failed to pop message! Should have been reply from Ping message");
      return FALSE;
    }

  if (dbus_message_get_reply_serial (message) != serial)
    {
      _dbus_warn ("Wrong reply serial");
      dbus_message_unref (message);
      return FALSE;
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_METHOD_RETURN)
    {
      _dbus_warn ("Unexpected message return during Ping");
      dbus_message_unref (message);
      return FALSE;
    }

  dbus_message_unref (message);
  message = NULL;

  return TRUE;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_existent_get_machine_id (BusContext     *context,
                               DBusConnection *connection)
{
  DBusError error = DBUS_ERROR_INIT;
  DBusMessage *message;
  dbus_uint32_t serial;
  DBusGUID uuid;
  const char *machine_id;

  if (!_dbus_read_local_machine_uuid (&uuid, FALSE, &error))
    {
      /* Unable to test further: either we ran out of memory, or neither
       * dbus nor systemd was ever correctly installed on this machine */
      _dbus_verbose ("Machine UUID not available: %s", error.message);
      dbus_error_free (&error);
      return TRUE;
    }

  message = dbus_message_new_method_call (EXISTENT_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.DBus.Peer",
                                          "GetMachineId");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* Note: if this test is run in OOM mode, it will block when the bus
   * doesn't send a reply due to OOM.
   */
  block_connection_until_message_from_bus (context, connection, "reply from running GetMachineId");

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Failed to pop message! Should have been reply from GetMachineId message");
      return FALSE;
    }

  if (dbus_message_get_reply_serial (message) != serial)
    {
      _dbus_warn ("Wrong reply serial");
      dbus_message_unref (message);
      return FALSE;
    }

  if (dbus_message_get_type (message) != DBUS_MESSAGE_TYPE_METHOD_RETURN)
    {
      _dbus_warn ("Unexpected message return during GetMachineId");
      dbus_message_unref (message);
      return FALSE;
    }

  machine_id = NULL;
  if (!dbus_message_get_args (message, NULL, DBUS_TYPE_STRING, &machine_id, DBUS_TYPE_INVALID))
    {
      _dbus_warn ("Did not get a machine ID in reply to GetMachineId");
      dbus_message_unref (message);
      return FALSE;
    }

  if (machine_id == NULL || strlen (machine_id) != 32)
    {
      _dbus_warn ("Machine id looks bogus: '%s'", machine_id ? machine_id : "null");
      dbus_message_unref (message);
      return FALSE;
    }

  /* We can't check that the machine id is correct because during make check it is
   * just made up for each process separately
   */

  dbus_message_unref (message);
  message = NULL;

  return TRUE;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_existent_service_auto_start (BusContext     *context,
                                   DBusConnection *connection)
{
  DBusMessage *message;
  DBusMessage *base_service_message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  const char *base_service;
  const char *text;

  base_service_message = NULL;

  message = dbus_message_new_method_call (EXISTENT_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  text = TEST_ECHO_MESSAGE;
  if (!dbus_message_append_args (message,
                                 DBUS_TYPE_STRING, &text,
                                 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* now wait for the message bus to hear back from the activated
   * service.
   */
  block_connection_until_message_from_bus (context, connection, "reply to Echo on existent service");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive any messages after auto start %d on %p",
                  serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);
  _dbus_verbose ("  (after sending %s)\n", "auto start");

  /* we should get zero or two ServiceOwnerChanged signals */
  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_SIGNAL)
    {
      GotServiceInfo message_kind;

      if (!check_base_service_activated (context, connection,
                                         message, &base_service))
        goto out;

      base_service_message = message;
      message = NULL;

      /* We may need to block here for the test service to exit or finish up */
      block_connection_until_message_from_bus (context, connection, "service to exit");

      /* Should get a service creation notification for the activated
       * service name, or a service deletion on the base service name
       */
      message = dbus_connection_borrow_message (connection);
      if (message == NULL)
        {
          _dbus_warn ("No message after auto activation "
                      "(should be a service announcement)");
          dbus_connection_return_message (connection, message);
          message = NULL;
          goto out;
        }

      message_kind = check_got_service_info (message);

      dbus_connection_return_message (connection, message);
      message = NULL;

      switch (message_kind)
        {
        case GOT_SERVICE_CREATED:
          message = pop_message_waiting_for_memory (connection);
          if (message == NULL)
            {
              _dbus_warn ("Failed to pop message we just put back! "
                          "should have been a NameOwnerChanged (creation)");
              goto out;
            }

          /* Check that ServiceOwnerChanged (creation) was correctly received */
          if (!check_service_auto_activated (context, connection, EXISTENT_SERVICE_NAME,
                                             base_service, message))
            goto out;

          dbus_message_unref (message);
          message = NULL;

          break;

        case GOT_SERVICE_DELETED:
          {
            /* The service started up and got a base address, but then
             * failed to register under EXISTENT_SERVICE_NAME
             */
            CheckServiceOwnerChangedData socd;

            socd.expected_kind = SERVICE_DELETED;
            socd.expected_service_name = base_service;
            socd.failed = FALSE;
            socd.skip_connection = NULL;
            socd.context = context;

            bus_test_clients_foreach (check_service_owner_changed_foreach,
                                      &socd);

            if (socd.failed)
              goto out;

            break;
          }

        case GOT_ERROR:
        case GOT_SOMETHING_ELSE:
        default:
          _dbus_warn ("Unexpected message after auto activation");
          goto out;
        }
    }

  /* OK, now we've dealt with ServiceOwnerChanged signals, now should
   * come the method reply (or error) from the initial method call
   */

  /* Note: if this test is run in OOM mode, it will block when the bus
   * doesn't send a reply due to OOM.
   */
  block_connection_until_message_from_bus (context, connection, "reply from echo message after auto-activation");

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Failed to pop message! Should have been reply from echo message");
      goto out;
    }

  if (dbus_message_get_reply_serial (message) != serial)
    {
      _dbus_warn ("Wrong reply serial");
      goto out;
    }

  dbus_message_unref (message);
  message = NULL;

  if (!check_existent_ping (context, connection))
    goto out;

  if (!check_existent_get_machine_id (context, connection))
    goto out;

  if (!check_existent_hello_from_self (context, connection))
    goto out;

  if (!check_send_exit_to_service (context, connection,
                                   EXISTENT_SERVICE_NAME,
                                   base_service))
    goto out;

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  if (base_service_message)
    dbus_message_unref (base_service_message);

  return retval;
}

#define SERVICE_FILE_MISSING_NAME "org.freedesktop.DBus.TestSuiteEchoServiceDotServiceFileDoesNotExist"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_launch_service_file_missing (BusContext     *context,
                                   DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (SERVICE_FILE_MISSING_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to service file missing should fail to auto-start");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SERVICE_UNKNOWN))
        {
          _dbus_verbose("got service unknown\n");
          ; /* good, this is expected (only valid when using launch helper) */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully auto-start missing service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}
#endif

#if defined(ENABLE_TRADITIONAL_ACTIVATION) && !defined(DBUS_WIN)

#define SERVICE_USER_MISSING_NAME "org.freedesktop.DBus.TestSuiteNoUser"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_launch_service_user_missing (BusContext     *context,
                                   DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (SERVICE_USER_MISSING_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection,
  					   "reply to service which should fail to auto-start (missing User)");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_warn ("connection was disconnected");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_FILE_INVALID))
        {
          _dbus_verbose("got service file invalid\n");
          ; /* good, this is expected (only valid when using launch helper) */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully auto-start missing service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

#define SERVICE_EXEC_MISSING_NAME "org.freedesktop.DBus.TestSuiteNoExec"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_launch_service_exec_missing (BusContext     *context,
                                   DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (SERVICE_EXEC_MISSING_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection,
  					   "reply to service which should fail to auto-start (missing Exec)");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_warn ("connection was disconnected");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SERVICE_UNKNOWN))
        {
          _dbus_verbose("could not activate as invalid service file was not added\n");
          ; /* good, this is expected as we shouldn't have been added to
             * the activation list with a missing Exec key */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_FILE_INVALID))
        {
          _dbus_verbose("got service file invalid\n");
          ; /* good, this is allowed, and is the message passed back from the
             * launch helper */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully auto-start missing service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

#define SERVICE_SERVICE_MISSING_NAME "org.freedesktop.DBus.TestSuiteNoService"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_launch_service_service_missing (BusContext     *context,
                                      DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (SERVICE_SERVICE_MISSING_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection,
  					   "reply to service which should fail to auto-start (missing Service)");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_warn ("connection was disconnected");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SERVICE_UNKNOWN))
        {
          _dbus_verbose("could not activate as invalid service file was not added\n");
          ; /* good, this is expected as we shouldn't have been added to
             * the activation list with a missing Exec key */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_SPAWN_FILE_INVALID))
        {
          _dbus_verbose("got service file invalid\n");
          ; /* good, this is allowed, and is the message passed back from the
             * launch helper */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully auto-start missing service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}
#endif

#define SHELL_FAIL_SERVICE_NAME "org.freedesktop.DBus.TestSuiteShellEchoServiceFail"

#ifdef ENABLE_TRADITIONAL_ACTIVATION
/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_shell_fail_service_auto_start (BusContext     *context,
                                     DBusConnection *connection)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;

  message = dbus_message_new_method_call (SHELL_FAIL_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);
  block_connection_until_message_from_bus (context, connection, "reply to shell Echo on service which should fail to auto-start");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
                  "Echo message (auto activation)", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
        {
          _dbus_warn ("Message has wrong sender %s",
                      dbus_message_get_sender (message) ?
                      dbus_message_get_sender (message) : "(none)");
          goto out;
        }

      if (dbus_message_is_error (message,
                                 DBUS_ERROR_NO_MEMORY))
        {
          ; /* good, this is a valid response */
        }
      else if (dbus_message_is_error (message,
                                      DBUS_ERROR_INVALID_ARGS))
        {
          _dbus_verbose("got invalid args\n");
          ; /* good, this is expected also */
        }
      else
        {
          warn_unexpected (connection, message, "not this error");

          goto out;
        }
    }
  else
    {
      _dbus_warn ("Did not expect to successfully auto-start shell fail service");
      goto out;
    }

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  return retval;
}

#define SHELL_SUCCESS_SERVICE_NAME "org.freedesktop.DBus.TestSuiteShellEchoServiceSuccess"

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_shell_service_success_auto_start (BusContext     *context,
                                        DBusConnection *connection)
{
  DBusMessage *message;
  DBusMessage *base_service_message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  const char *base_service;
  const char *argv[7] = {NULL, NULL, NULL, NULL, NULL, NULL, NULL};

  base_service_message = NULL;

  message = dbus_message_new_method_call (SHELL_SUCCESS_SERVICE_NAME,
                                          "/org/freedesktop/TestSuite",
                                          "org.freedesktop.TestSuite",
                                          "Echo");

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* now wait for the message bus to hear back from the activated
   * service.
   */
  block_connection_until_message_from_bus (context, connection, "reply to Echo on shell success service");
  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive any messages after auto start %d on %p",
                  serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);
  _dbus_verbose ("  (after sending %s)\n", "auto start");

  /* we should get zero or two ServiceOwnerChanged signals */
  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_SIGNAL)
    {
      GotServiceInfo message_kind;

      if (!check_base_service_activated (context, connection,
                                         message, &base_service))
        goto out;

      base_service_message = message;
      message = NULL;

      /* We may need to block here for the test service to exit or finish up */
      block_connection_until_message_from_bus (context, connection, "service to exit");

      /* Should get a service creation notification for the activated
       * service name, or a service deletion on the base service name
       */
      message = dbus_connection_borrow_message (connection);
      if (message == NULL)
        {
          _dbus_warn ("No message after auto activation "
                      "(should be a service announcement)");
          dbus_connection_return_message (connection, message);
          message = NULL;
          goto out;
        }

      message_kind = check_got_service_info (message);

      dbus_connection_return_message (connection, message);
      message = NULL;

      switch (message_kind)
        {
        case GOT_SERVICE_CREATED:
          message = pop_message_waiting_for_memory (connection);
          if (message == NULL)
            {
              _dbus_warn ("Failed to pop message we just put back! "
                          "should have been a NameOwnerChanged (creation)");
              goto out;
            }

          /* Check that ServiceOwnerChanged (creation) was correctly received */
          if (!check_service_auto_activated (context, connection, SHELL_SUCCESS_SERVICE_NAME,
                                             base_service, message))
            goto out;

          dbus_message_unref (message);
          message = NULL;

          break;

        case GOT_SERVICE_DELETED:
          {
            /* The service started up and got a base address, but then
             * failed to register under SHELL_SUCCESS_SERVICE_NAME
             */
            CheckServiceOwnerChangedData socd;

            socd.expected_kind = SERVICE_DELETED;
            socd.expected_service_name = base_service;
            socd.failed = FALSE;
            socd.skip_connection = NULL;
            socd.context = context;

            bus_test_clients_foreach (check_service_owner_changed_foreach,
                                      &socd);

            if (socd.failed)
              goto out;

            break;
          }

        case GOT_ERROR:
        case GOT_SOMETHING_ELSE:
        default:
          _dbus_warn ("Unexpected message after auto activation");
          goto out;
        }
    }

  /* OK, now we've dealt with ServiceOwnerChanged signals, now should
   * come the method reply (or error) from the initial method call
   */

  /* Note: if this test is run in OOM mode, it will block when the bus
   * doesn't send a reply due to OOM.
   */
  block_connection_until_message_from_bus (context, connection, "reply from echo message after auto-activation");

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Failed to pop message! Should have been reply from echo message");
      goto out;
    }

  if (dbus_message_get_reply_serial (message) != serial)
    {
      _dbus_warn ("Wrong reply serial");
      goto out;
    }

  if (!dbus_message_get_args (message, NULL,
                                       DBUS_TYPE_STRING, &argv[0],
                                       DBUS_TYPE_STRING, &argv[1],
                                       DBUS_TYPE_STRING, &argv[2],
                                       DBUS_TYPE_STRING, &argv[3],
                                       DBUS_TYPE_STRING, &argv[4],
                                       DBUS_TYPE_STRING, &argv[5],
                                       DBUS_TYPE_STRING, &argv[6],
                                       DBUS_TYPE_INVALID))
    {
      _dbus_warn ("Error getting arguments from return");
      goto out;
    }

   /* don't worry about arg[0] as it may be different
      depending on the path to the tests
   */
  if (strcmp("-test", argv[1]) != 0)
    {
      _dbus_warn ("Unexpected argv[1] in shell success service test (expected: %s, got: %s)",
                  "-test", argv[1]);
      goto out;
    }

  if (strcmp("that", argv[2]) != 0)
    {
      _dbus_warn ("Unexpected argv[2] in shell success service test (expected: %s, got: %s)",
                   "that", argv[2]);
      goto out;
    }

  if (strcmp("we get", argv[3]) != 0)
    {
      _dbus_warn ("Unexpected argv[3] in shell success service test (expected: %s, got: %s)",
                   "we get", argv[3]);
      goto out;
    }

  if (strcmp("back", argv[4]) != 0)
    {
      _dbus_warn ("Unexpected argv[4] in shell success service test (expected: %s, got: %s)",
                   "back", argv[4]);
      goto out;
    }

  if (strcmp("--what", argv[5]) != 0)
    {
      _dbus_warn ("Unexpected argv[5] in shell success service test (expected: %s, got: %s)",
                   "--what", argv[5]);
      goto out;
    }

  if (strcmp("we put in", argv[6]) != 0)
    {
      _dbus_warn ("Unexpected argv[6] in shell success service test (expected: %s, got: %s)",
                   "we put in", argv[6]);
      goto out;
    }

  dbus_message_unref (message);
  message = NULL;

  if (!check_send_exit_to_service (context, connection,
                                   SHELL_SUCCESS_SERVICE_NAME,
                                   base_service))
    goto out;

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  if (base_service_message)
    dbus_message_unref (base_service_message);

  return retval;
}

static dbus_bool_t
check_get_services (BusContext     *context,
		    DBusConnection *connection,
		    const char     *method,
		    char         ***services,
		    int            *len)
{
  DBusMessage *message;
  dbus_uint32_t serial;
  dbus_bool_t retval;
  DBusError error;
  char **srvs;
  int l;

  retval = FALSE;
  dbus_error_init (&error);
  message = NULL;

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
					  DBUS_PATH_DBUS,
					  DBUS_INTERFACE_DBUS,
					  method);

  if (message == NULL)
    return TRUE;

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  /* send our message */
  bus_test_run_clients_loop (SEND_PENDING (connection));

  dbus_message_unref (message);
  message = NULL;

  dbus_connection_ref (connection); /* because we may get disconnected */
  block_connection_until_message_from_bus (context, connection, "reply to ListActivatableNames/ListNames");

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");

      dbus_connection_unref (connection);

      return TRUE;
    }

  dbus_connection_unref (connection);

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive a reply to %s %d on %p",
		  method, serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (dbus_message_is_error (message, DBUS_ERROR_NO_MEMORY))
	{
	  ; /* good, this is a valid response */
	}
      else
	{
	  warn_unexpected (connection, message, "not this error");

	  goto out;
	}
    }
  else
    {
      if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_METHOD_RETURN)
	{
	  ; /* good, expected */
	}
      else
	{
	  warn_unexpected (connection, message,
			   "method_return for ListActivatableNames/ListNames");

	  goto out;
	}

    retry_get_property:

      if (!dbus_message_get_args (message, &error,
				  DBUS_TYPE_ARRAY,
				  DBUS_TYPE_STRING,
				  &srvs, &l,
				  DBUS_TYPE_INVALID))
	{
	  if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
	    {
	      _dbus_verbose ("no memory to list services by %s\n", method);
	      dbus_error_free (&error);
	      _dbus_wait_for_memory ();
	      goto retry_get_property;
	    }
	  else
	    {
	      _dbus_assert (dbus_error_is_set (&error));
	      _dbus_warn ("Did not get the expected DBUS_TYPE_ARRAY from %s", method);
	      goto out;
	    }
	} else {
	  *services = srvs;
	  *len = l;
	}
    }

  if (!check_no_leftovers (context))
    goto out;

  retval = TRUE;

 out:
  dbus_error_free (&error);

  if (message)
    dbus_message_unref (message);

  return retval;
}

/* returns TRUE if the correct thing happens,
 * but the correct thing may include OOM errors.
 */
static dbus_bool_t
check_list_services (BusContext     *context,
		     DBusConnection *connection)
{
  DBusMessage  *message;
  DBusMessage  *base_service_message;
  const char   *base_service;
  dbus_uint32_t serial;
  dbus_bool_t   retval;
  const char   *existent = EXISTENT_SERVICE_NAME;
  dbus_uint32_t flags;
  char        **services;
  int           len;

  _dbus_verbose ("check_list_services for %p\n", connection);

  if (!check_get_services (context, connection, "ListActivatableNames", &services, &len))
    {
      return TRUE;
    }

  if (!_dbus_string_array_contains ((const char **)services, existent))
    {
      _dbus_warn ("Did not get the expected %s from ListActivatableNames", existent);
      dbus_free_string_array (services);
      return FALSE;
    }

  dbus_free_string_array (services);

  base_service_message = NULL;

  message = dbus_message_new_method_call (DBUS_SERVICE_DBUS,
					  DBUS_PATH_DBUS,
					  DBUS_INTERFACE_DBUS,
					  "StartServiceByName");

  if (message == NULL)
    return TRUE;

  dbus_message_set_auto_start (message, FALSE);

  flags = 0;
  if (!dbus_message_append_args (message,
				 DBUS_TYPE_STRING, &existent,
				 DBUS_TYPE_UINT32, &flags,
				 DBUS_TYPE_INVALID))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  if (!dbus_connection_send (connection, message, &serial))
    {
      dbus_message_unref (message);
      return TRUE;
    }

  dbus_message_unref (message);
  message = NULL;

  bus_test_run_everything (context);

  /* now wait for the message bus to hear back from the activated
   * service.
   */
  block_connection_until_message_from_bus (context, connection, "activated service to connect");

  bus_test_run_everything (context);

  if (!dbus_connection_get_is_connected (connection))
    {
      _dbus_verbose ("connection was disconnected\n");
      return TRUE;
    }

  retval = FALSE;

  message = pop_message_waiting_for_memory (connection);
  if (message == NULL)
    {
      _dbus_warn ("Did not receive any messages after %s %d on %p",
		  "StartServiceByName", serial, connection);
      goto out;
    }

  verbose_message_received (connection, message);
  _dbus_verbose ("  (after sending %s)\n", "StartServiceByName");

  if (dbus_message_get_type (message) == DBUS_MESSAGE_TYPE_ERROR)
    {
      if (!dbus_message_has_sender (message, DBUS_SERVICE_DBUS))
	{
	  _dbus_warn ("Message has wrong sender %s",
		      dbus_message_get_sender (message) ?
		      dbus_message_get_sender (message) : "(none)");
	  goto out;
	}

      if (dbus_message_is_error (message,
				 DBUS_ERROR_NO_MEMORY))
	{
	  ; /* good, this is a valid response */
	}
      else if (dbus_message_is_error (message,
				      DBUS_ERROR_SPAWN_CHILD_EXITED) ||
	       dbus_message_is_error (message,
				      DBUS_ERROR_SPAWN_CHILD_SIGNALED) ||
	       dbus_message_is_error (message,
				      DBUS_ERROR_SPAWN_EXEC_FAILED))
	{
	  ; /* good, this is expected also */
	}
      else
	{
	  _dbus_warn ("Did not expect error %s",
		      dbus_message_get_error_name (message));
	  goto out;
	}
    }
  else
    {
      GotServiceInfo message_kind;

      if (!check_base_service_activated (context, connection,
					 message, &base_service))
	goto out;

      base_service_message = message;
      message = NULL;

      /* We may need to block here for the test service to exit or finish up */
      block_connection_until_message_from_bus (context, connection, "test service to exit or finish up");

      message = dbus_connection_borrow_message (connection);
      if (message == NULL)
	{
	  _dbus_warn ("Did not receive any messages after base service creation notification");
	  goto out;
	}

      message_kind = check_got_service_info (message);

      dbus_connection_return_message (connection, message);
      message = NULL;

      switch (message_kind)
	{
	case GOT_SOMETHING_ELSE:
	case GOT_ERROR:
	case GOT_SERVICE_DELETED:
	default:
	  _dbus_warn ("Unexpected message after ActivateService "
		      "(should be an error or a service announcement)");
	  goto out;

	case GOT_SERVICE_CREATED:
	  message = pop_message_waiting_for_memory (connection);
	  if (message == NULL)
	    {
	      _dbus_warn ("Failed to pop message we just put back! "
			  "should have been a NameOwnerChanged (creation)");
	      goto out;
	    }

	  if (!check_service_activated (context, connection, EXISTENT_SERVICE_NAME,
					base_service, message))
	    goto out;

	  dbus_message_unref (message);
	  message = NULL;

	  if (!check_no_leftovers (context))
	    {
	      _dbus_warn ("Messages were left over after successful activation");
	      goto out;
	    }

	  break;
	}
    }

  if (!check_get_services (context, connection, "ListNames", &services, &len))
    {
      return TRUE;
    }

  if (!_dbus_string_array_contains ((const char **)services, existent))
    {
      _dbus_warn ("Did not get the expected %s from ListNames", existent);
      goto out;
    }

  dbus_free_string_array (services);

  if (!check_send_exit_to_service (context, connection,
				   EXISTENT_SERVICE_NAME, base_service))
    goto out;

  retval = TRUE;

 out:
  if (message)
    dbus_message_unref (message);

  if (base_service_message)
    dbus_message_unref (base_service_message);

  return retval;
}
#endif

typedef struct
{
  Check2Func func;
  BusContext *context;
  DBusConnection *connection;
} Check2Data;

static dbus_bool_t
check_oom_check2_func (void        *data,
                       dbus_bool_t  have_memory)
{
  dbus_bool_t ret = TRUE;
  Check2Data *d = data;

  if (!have_memory)
    bus_context_quiet_log_begin (d->context);

  if (! (* d->func) (d->context, d->connection))
    ret = FALSE;

  if (!have_memory)
    bus_context_quiet_log_end (d->context);

  if (ret && !check_no_leftovers (d->context))
    {
      _dbus_warn ("Messages were left over, should be covered by test suite");
      ret = FALSE;
    }

  return ret;
}

static void
check2_try_iterations (BusContext     *context,
                       DBusConnection *connection,
                       const char     *description,
                       Check2Func      func)
{
  Check2Data d;

  d.func = func;
  d.context = context;
  d.connection = connection;

  if (!_dbus_test_oom_handling (description, check_oom_check2_func,
                                &d))
    _dbus_test_fatal ("%s failed during oom", description);
}

#ifdef ENABLE_TRADITIONAL_ACTIVATION
static dbus_bool_t
setenv_TEST_LAUNCH_HELPER_CONFIG(const DBusString *test_data_dir,
                                 const char       *filename)
{
  DBusString full;
  DBusString file;

  if (!_dbus_string_init (&full))
    return FALSE;

  if (!_dbus_string_copy (test_data_dir, 0, &full, 0))
    {
      _dbus_string_free (&full);
      return FALSE;
    }

  _dbus_string_init_const (&file, filename);

  if (!_dbus_concat_dir_and_file (&full, &file))
    {
      _dbus_string_free (&full);
      return FALSE;
    }

  _dbus_verbose ("Setting TEST_LAUNCH_HELPER_CONFIG to '%s'\n",
                 _dbus_string_get_const_data (&full));

  dbus_setenv ("TEST_LAUNCH_HELPER_CONFIG", _dbus_string_get_const_data (&full));

  _dbus_string_free (&full);

  return TRUE;
}

static dbus_bool_t
bus_dispatch_test_conf (const DBusString *test_data_dir,
		        const char       *filename,
		        dbus_bool_t       use_launcher)
{
  BusContext *context;
  DBusConnection *foo;
  DBusConnection *bar;
  DBusConnection *baz;
  DBusError error;

  _dbus_test_diag ("%s:%s...", _DBUS_FUNCTION_NAME, filename);

  /* save the config name for the activation helper */
  if (!setenv_TEST_LAUNCH_HELPER_CONFIG (test_data_dir, filename))
    _dbus_test_fatal ("no memory setting TEST_LAUNCH_HELPER_CONFIG");

  dbus_error_init (&error);

  context = bus_context_new_test (test_data_dir, filename);
  if (context == NULL)
    {
      _dbus_test_not_ok ("%s:%s - bus_context_new_test() failed",
          _DBUS_FUNCTION_NAME, filename);
      return FALSE;
    }

  foo = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (foo == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (foo))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, foo);

  if (!check_hello_message (context, foo))
    _dbus_test_fatal ("hello message failed");

  if (!check_double_hello_message (context, foo))
    _dbus_test_fatal ("double hello message failed");

  if (!check_add_match (context, foo, ""))
    _dbus_test_fatal ("AddMatch message failed");

  bar = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (bar == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (bar))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, bar);

  if (!check_hello_message (context, bar))
    _dbus_test_fatal ("hello message failed");

  if (!check_add_match (context, bar, ""))
    _dbus_test_fatal ("AddMatch message failed");

  baz = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (baz == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (baz))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, baz);

  if (!check_hello_message (context, baz))
    _dbus_test_fatal ("hello message failed");

  if (!check_add_match (context, baz, ""))
    _dbus_test_fatal ("AddMatch message failed");

  if (!check_add_match (context, baz, "interface='com.example'"))
    _dbus_test_fatal ("AddMatch message failed");

#ifdef DBUS_ENABLE_STATS
  if (!check_get_all_match_rules (context, baz))
    _dbus_test_fatal ("GetAllMatchRules message failed");
#endif

  if (!check_get_connection_unix_user (context, baz))
    _dbus_test_fatal ("GetConnectionUnixUser message failed");

  if (!check_get_connection_unix_process_id (context, baz))
    _dbus_test_fatal ("GetConnectionUnixProcessID message failed");

  if (!check_list_services (context, baz))
    _dbus_test_fatal ("ListActivatableNames message failed");

  if (!check_no_leftovers (context))
    _dbus_test_fatal ("Messages were left over after setting up initial connections");

  _dbus_test_ok ("%s:%s - connection setup", _DBUS_FUNCTION_NAME, filename);

  check2_try_iterations (context, NULL, "create_and_hello",
                         check_hello_connection);
  _dbus_test_ok ("%s:%s - check_hello_connection", _DBUS_FUNCTION_NAME, filename);

  check2_try_iterations (context, foo, "nonexistent_service_no_auto_start",
                         check_nonexistent_service_no_auto_start);
  _dbus_test_ok ("%s:%s - check_nonexistent_service_no_auto_start", _DBUS_FUNCTION_NAME, filename);

  check2_try_iterations (context, foo, "segfault_service_no_auto_start",
                         check_segfault_service_no_auto_start);
  _dbus_test_ok ("%s:%s - check_segfault_service_no_auto_start", _DBUS_FUNCTION_NAME, filename);

  check2_try_iterations (context, foo, "existent_service_no_auto_start",
                         check_existent_service_no_auto_start);
  _dbus_test_ok ("%s:%s - check_existent_service_no_auto_start", _DBUS_FUNCTION_NAME, filename);

  check2_try_iterations (context, foo, "nonexistent_service_auto_start",
                         check_nonexistent_service_auto_start);
  _dbus_test_ok ("%s:%s - check_nonexistent_service_auto_start", _DBUS_FUNCTION_NAME, filename);

  /* only do the segfault test if we are not using the launcher */
  check2_try_iterations (context, foo, "segfault_service_auto_start",
                         check_segfault_service_auto_start);
  _dbus_test_ok ("%s:%s - check_segfault_service_auto_start", _DBUS_FUNCTION_NAME, filename);

  /* only do the shell fail test if we are not using the launcher */
  check2_try_iterations (context, foo, "shell_fail_service_auto_start",
                         check_shell_fail_service_auto_start);
  _dbus_test_ok ("%s:%s - check_shell_fail_service_auto_start", _DBUS_FUNCTION_NAME, filename);

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  /* specific to launcher */
  if (use_launcher)
    {
      if (!check_launch_service_file_missing (context, foo))
        _dbus_test_fatal ("did not get service file not found error");

      _dbus_test_ok ("%s:%s - check_launch_service_file_missing", _DBUS_FUNCTION_NAME, filename);
    }
#endif

#if 0
  /* Note: need to resolve some issues with the testing code in order to run
   * this in oom (handle that we sometimes don't get replies back from the bus
   * when oom happens, without blocking the test).
   */
  check2_try_iterations (context, foo, "existent_service_auto_auto_start",
                         check_existent_service_auto_start);
  _dbus_test_ok ("check_existent_service_auto_start");
#endif

  if (!check_existent_service_auto_start (context, foo))
    _dbus_test_fatal ("existent service auto start failed");
  _dbus_test_ok ("%s:%s - check_existent_service_auto_start", _DBUS_FUNCTION_NAME, filename);

  if (!check_shell_service_success_auto_start (context, foo))
    _dbus_test_fatal ("shell success service auto start failed");
  _dbus_test_ok ("%s:%s - check_shell_service_success_auto_start", _DBUS_FUNCTION_NAME, filename);

#ifdef DBUS_WIN
  _dbus_verbose("TODO: Fix memory leaks after running check_shell_service_success_auto_start\n");
  _dbus_sleep_milliseconds (500);
#endif

  _dbus_verbose ("Disconnecting foo, bar, and baz\n");

  kill_client_connection_unchecked (foo);
  kill_client_connection_unchecked (bar);
  kill_client_connection_unchecked (baz);

  bus_context_unref (context);

  _dbus_test_ok ("%s:%s", _DBUS_FUNCTION_NAME, filename);
  return TRUE;
}
#endif

#if defined(ENABLE_TRADITIONAL_ACTIVATION) && !defined(DBUS_WIN)
static dbus_bool_t
bus_dispatch_test_conf_fail (const DBusString *test_data_dir,
		             const char       *filename)
{
  BusContext *context;
  DBusConnection *foo;
  DBusError error;

  _dbus_test_diag ("%s:%s...", _DBUS_FUNCTION_NAME, filename);

  /* save the config name for the activation helper */
  if (!setenv_TEST_LAUNCH_HELPER_CONFIG (test_data_dir, filename))
    _dbus_test_fatal ("no memory setting TEST_LAUNCH_HELPER_CONFIG");

  dbus_error_init (&error);

  context = bus_context_new_test (test_data_dir, filename);
  if (context == NULL)
    {
      _dbus_test_not_ok ("%s:%s - bus_context_new_test() failed",
          _DBUS_FUNCTION_NAME, filename);
      return FALSE;
    }

  foo = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (foo == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (foo))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, foo);

  if (!check_hello_message (context, foo))
    _dbus_test_fatal ("hello message failed");

  if (!check_double_hello_message (context, foo))
    _dbus_test_fatal ("double hello message failed");

  if (!check_add_match (context, foo, ""))
    _dbus_test_fatal ("AddMatch message failed");

  /* this only tests the activation.c user check */
  if (!check_launch_service_user_missing (context, foo))
    _dbus_test_fatal ("user missing did not trigger error");

  /* this only tests the desktop.c exec check */
  if (!check_launch_service_exec_missing (context, foo))
    _dbus_test_fatal ("exec missing did not trigger error");

  /* this only tests the desktop.c service check */
  if (!check_launch_service_service_missing (context, foo))
    _dbus_test_fatal ("service missing did not trigger error");

  _dbus_verbose ("Disconnecting foo\n");

  kill_client_connection_unchecked (foo);

  bus_context_unref (context);

  _dbus_test_ok ("%s:%s", _DBUS_FUNCTION_NAME, filename);
  return TRUE;
}
#endif

dbus_bool_t
bus_dispatch_test (const char *test_data_dir_cstr)
{
  DBusString test_data_dir;

  _dbus_string_init_const (&test_data_dir, test_data_dir_cstr);

#ifdef ENABLE_TRADITIONAL_ACTIVATION
  /* run normal activation tests */
  _dbus_verbose ("Normal activation tests\n");
  if (!bus_dispatch_test_conf (&test_data_dir,
  			       "valid-config-files/debug-allow-all.conf", FALSE))
    return FALSE;

#ifndef DBUS_WIN
  /* run launch-helper activation tests */
  _dbus_verbose ("Launch helper activation tests\n");
  if (!bus_dispatch_test_conf (&test_data_dir,
  			       "valid-config-files-system/debug-allow-all-pass.conf", TRUE))
    return FALSE;

  /* run select launch-helper activation tests on broken service files */
  if (!bus_dispatch_test_conf_fail (&test_data_dir,
  			            "valid-config-files-system/debug-allow-all-fail.conf"))
    return FALSE;
#endif
#endif

  return TRUE;
}

dbus_bool_t
bus_dispatch_sha1_test (const char *test_data_dir_cstr)
{
  DBusString test_data_dir;
  BusContext *context;
  DBusConnection *foo;
  DBusError error;

  _dbus_string_init_const (&test_data_dir, test_data_dir_cstr);
  dbus_error_init (&error);

  /* Test SHA1 authentication */
  _dbus_verbose ("Testing SHA1 context\n");

  context = bus_context_new_test (&test_data_dir,
                                  "valid-config-files/debug-allow-all-sha1.conf");
  if (context == NULL)
    return FALSE;

  foo = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (foo == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (foo))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, foo);

  if (!check_hello_message (context, foo))
    _dbus_test_fatal ("hello message failed");

  if (!check_add_match (context, foo, ""))
    _dbus_test_fatal ("addmatch message failed");

  if (!check_no_leftovers (context))
    _dbus_test_fatal ("Messages were left over after setting up initial SHA-1 connection");

  check2_try_iterations (context, NULL, "create_and_hello_sha1",
                         check_hello_connection);

  kill_client_connection_unchecked (foo);

  bus_context_unref (context);

  return TRUE;
}

dbus_bool_t
bus_unix_fds_passing_test (const char *test_data_dir_cstr)
{
#ifdef HAVE_UNIX_FD_PASSING
  DBusString test_data_dir;
  BusContext *context;
  DBusConnection *foo, *bar;
  DBusError error;
  DBusMessage *m;
  DBusSocket one[2], two[2];
  int x, y, z;
  char r;

  _dbus_string_init_const (&test_data_dir, test_data_dir_cstr);
  dbus_error_init (&error);

  context = bus_context_new_test (&test_data_dir,
                                  "valid-config-files/debug-allow-all.conf");
  if (context == NULL)
    _dbus_test_fatal ("could not alloc context");

  foo = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (foo == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (foo))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, foo);

  if (!check_hello_message (context, foo))
    _dbus_test_fatal ("hello message failed");

  if (!check_add_match (context, foo, ""))
    _dbus_test_fatal ("AddMatch message failed");

  bar = dbus_connection_open_private (TEST_DEBUG_PIPE, &error);
  if (bar == NULL)
    _dbus_test_fatal ("could not alloc connection");

  if (!bus_setup_debug_client (bar))
    _dbus_test_fatal ("could not set up connection");

  spin_connection_until_authenticated (context, bar);

  if (!check_hello_message (context, bar))
    _dbus_test_fatal ("hello message failed");

  if (!check_add_match (context, bar, ""))
    _dbus_test_fatal ("AddMatch message failed");

  if (!(m = dbus_message_new_signal("/", "a.b.c", "d")))
    _dbus_test_fatal ("could not alloc message");

  if (!(_dbus_socketpair (one, one+1, TRUE, &error)))
    _dbus_test_fatal ("Failed to allocate pipe #1");

  if (!(_dbus_socketpair (two, two+1, TRUE, &error)))
    _dbus_test_fatal ("Failed to allocate pipe #2");

  if (!dbus_message_append_args(m,
                                DBUS_TYPE_UNIX_FD, one,
                                DBUS_TYPE_UNIX_FD, two,
                                DBUS_TYPE_UNIX_FD, two,
                                DBUS_TYPE_INVALID))
    _dbus_test_fatal ("Failed to attach fds.");

  if (!_dbus_close_socket (one[0], &error))
    _dbus_test_fatal ("Failed to close pipe #1 ");
  if (!_dbus_close_socket (two[0], &error))
    _dbus_test_fatal ("Failed to close pipe #2 ");

  if (!(dbus_connection_can_send_type(foo, DBUS_TYPE_UNIX_FD)))
    _dbus_test_fatal ("Connection cannot do fd passing");

  if (!(dbus_connection_can_send_type(bar, DBUS_TYPE_UNIX_FD)))
    _dbus_test_fatal ("Connection cannot do fd passing");

  if (!dbus_connection_send (foo, m, NULL))
    _dbus_test_fatal ("Failed to send fds");

  dbus_message_unref(m);

  bus_test_run_clients_loop (SEND_PENDING (foo));

  bus_test_run_everything (context);

  block_connection_until_message_from_bus (context, foo, "unix fd reception on foo");

  if (!(m = pop_message_waiting_for_memory (foo)))
    _dbus_test_fatal ("Failed to receive msg");

  if (!dbus_message_is_signal(m, "a.b.c", "d"))
    _dbus_test_fatal ("bogus message received");

  dbus_message_unref(m);

  block_connection_until_message_from_bus (context, bar, "unix fd reception on bar");

  if (!(m = pop_message_waiting_for_memory (bar)))
    _dbus_test_fatal ("Failed to receive msg");

  if (!dbus_message_is_signal(m, "a.b.c", "d"))
    _dbus_test_fatal ("bogus message received");

  if (!dbus_message_get_args(m,
                             &error,
                             DBUS_TYPE_UNIX_FD, &x,
                             DBUS_TYPE_UNIX_FD, &y,
                             DBUS_TYPE_UNIX_FD, &z,
                             DBUS_TYPE_INVALID))
    _dbus_test_fatal ("Failed to parse fds.");

  dbus_message_unref(m);

  if (write(x, "X", 1) != 1)
    _dbus_test_fatal ("Failed to write to pipe #1");
  if (write(y, "Y", 1) != 1)
    _dbus_test_fatal ("Failed to write to pipe #2");
  if (write(z, "Z", 1) != 1)
    _dbus_test_fatal ("Failed to write to pipe #2/2nd fd");

  if (!_dbus_close(x, &error))
    _dbus_test_fatal ("Failed to close pipe #1/other side ");
  if (!_dbus_close(y, &error))
    _dbus_test_fatal ("Failed to close pipe #2/other side ");
  if (!_dbus_close(z, &error))
    _dbus_test_fatal ("Failed to close pipe #2/other size 2nd fd ");

  if (read(one[1].fd, &r, 1) != 1 || r != 'X')
    _dbus_test_fatal ("Failed to read value from pipe.");
  if (read(two[1].fd, &r, 1) != 1 || r != 'Y')
    _dbus_test_fatal ("Failed to read value from pipe.");
  if (read(two[1].fd, &r, 1) != 1 || r != 'Z')
    _dbus_test_fatal ("Failed to read value from pipe.");

  if (!_dbus_close_socket (one[1], &error))
    _dbus_test_fatal ("Failed to close pipe #1 ");
  if (!_dbus_close_socket (two[1], &error))
    _dbus_test_fatal ("Failed to close pipe #2 ");

  _dbus_verbose ("Disconnecting foo\n");
  kill_client_connection_unchecked (foo);

  _dbus_verbose ("Disconnecting bar\n");
  kill_client_connection_unchecked (bar);

  bus_context_unref (context);

#else
  _dbus_test_skip ("fd-passing not supported on this platform");
#endif
  return TRUE;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
