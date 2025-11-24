/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* connection.h  Client connections
 *
 * Copyright (C) 2003, 2004  Red Hat, Inc.
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

#ifndef BUS_CONNECTION_H
#define BUS_CONNECTION_H

#include <dbus/dbus.h>
#include <dbus/dbus-list.h>
#include "bus.h"

typedef enum
{
  BUS_EXTRA_HEADERS_CONTAINER_INSTANCE = (1 << 0),
  BUS_EXTRA_HEADERS_NONE = 0
} BusExtraHeaders;

BusConnections* bus_connections_new               (BusContext                   *context);
BusConnections* bus_connections_ref               (BusConnections               *connections);
void            bus_connections_unref             (BusConnections               *connections);
dbus_bool_t     bus_connections_setup_connection  (BusConnections               *connections,
                                                   DBusConnection               *connection);
void            bus_connections_increment_stamp   (BusConnections               *connections);
dbus_bool_t     bus_connections_reload_policy     (BusConnections               *connections,
                                                   DBusError                    *error);
BusContext*     bus_connection_get_context        (DBusConnection               *connection);
BusConnections* bus_connection_get_connections    (DBusConnection               *connection);
BusRegistry*    bus_connection_get_registry       (DBusConnection               *connection);
BusActivation*  bus_connection_get_activation     (DBusConnection               *connection);
BusMatchmaker*  bus_connection_get_matchmaker     (DBusConnection               *connection);
const char *    bus_connection_get_loginfo        (DBusConnection        *connection);
BusSELinuxID*   bus_connection_get_selinux_id     (DBusConnection               *connection);
BusAppArmorConfinement* bus_connection_dup_apparmor_confinement (DBusConnection *connection);
dbus_bool_t     bus_connections_check_limits      (BusConnections               *connections,
                                                   DBusConnection               *requesting_completion,
                                                   const char                  **limit_name_out,
                                                   int                          *limit_out,
                                                   DBusError                    *error);
void            bus_connections_expire_incomplete (BusConnections               *connections);

dbus_bool_t     bus_connections_expect_reply      (BusConnections               *connections,
                                                   BusTransaction               *transaction,
                                                   DBusConnection               *will_get_reply,
                                                   DBusConnection               *will_send_reply,
                                                   DBusMessage                  *reply_to_this,
                                                   DBusError                    *error);
dbus_bool_t     bus_connections_check_reply       (BusConnections               *connections,
                                                   BusTransaction               *transaction,
                                                   DBusConnection               *sending_reply,
                                                   DBusConnection               *receiving_reply,
                                                   DBusMessage                  *reply,
                                                   DBusError                    *error);

dbus_bool_t     bus_connection_mark_stamp         (DBusConnection               *connection);

dbus_bool_t bus_connection_is_active (DBusConnection *connection);
const char *bus_connection_get_name  (DBusConnection *connection);

dbus_bool_t bus_connection_preallocate_oom_error (DBusConnection *connection);
void        bus_connection_send_oom_error        (DBusConnection *connection,
                                                  DBusMessage    *in_reply_to);

void        bus_connection_request_headers       (DBusConnection  *connection,
                                                  BusExtraHeaders  headers);

/* called by policy.c */
dbus_bool_t bus_connection_is_queued_owner_by_prefix (DBusConnection *connection,
                                                      const char *name_prefix);

/* called by signals.c */
dbus_bool_t bus_connection_add_match_rule      (DBusConnection *connection,
                                                BusMatchRule   *rule);
void        bus_connection_add_match_rule_link (DBusConnection *connection,
                                                DBusList       *link);
void        bus_connection_remove_match_rule   (DBusConnection *connection,
                                                BusMatchRule   *rule);
int         bus_connection_get_n_match_rules   (DBusConnection *connection);


/* called by services.c */
dbus_bool_t bus_connection_add_owned_service      (DBusConnection *connection,
                                                   BusService     *service);
void        bus_connection_remove_owned_service   (DBusConnection *connection,
                                                   BusService     *service);
void        bus_connection_add_owned_service_link (DBusConnection *connection,
                                                   DBusList       *link);
int         bus_connection_get_n_services_owned   (DBusConnection *connection);

/* called by driver.c */
dbus_bool_t bus_connection_complete (DBusConnection               *connection,
				     const DBusString             *name,
                                     DBusError                    *error);

/* called by dispatch.c when the connection is dropped */
void        bus_connection_disconnected (DBusConnection *connection);

dbus_bool_t      bus_connection_is_in_unix_group (DBusConnection       *connection,
                                                  unsigned long         gid);
dbus_bool_t      bus_connection_get_unix_groups  (DBusConnection       *connection,
                                                  unsigned long       **groups,
                                                  int                  *n_groups,
                                                  DBusError            *error);
BusClientPolicy* bus_connection_get_policy  (DBusConnection       *connection);

dbus_bool_t bus_connection_is_monitor (DBusConnection  *connection);
dbus_bool_t bus_connection_be_monitor (DBusConnection  *connection,
                                       BusTransaction  *transaction,
                                       DBusList       **rules,
                                       DBusError       *error);

/* transaction API so we can send or not send a block of messages as a whole */

typedef void (* BusTransactionCancelFunction) (void *data);

BusTransaction* bus_transaction_new              (BusContext                   *context);
BusContext*     bus_transaction_get_context      (BusTransaction               *transaction);
dbus_bool_t     bus_transaction_send             (BusTransaction               *transaction,
                                                  DBusConnection               *sender,
                                                  DBusConnection               *destination,
                                                  DBusMessage                  *message);
dbus_bool_t     bus_transaction_capture          (BusTransaction               *transaction,
                                                  DBusConnection               *connection,
                                                  DBusConnection               *addressed_recipient,
                                                  DBusMessage                  *message);
dbus_bool_t     bus_transaction_capture_error_reply (BusTransaction            *transaction,
                                                  DBusConnection               *addressed_recipient,
                                                  const DBusError              *error,
                                                  DBusMessage                  *in_reply_to);
dbus_bool_t     bus_transaction_send_from_driver (BusTransaction               *transaction,
                                                  DBusConnection               *connection,
                                                  DBusMessage                  *message);
dbus_bool_t     bus_transaction_send_error_reply (BusTransaction               *transaction,
                                                  DBusConnection               *connection,
                                                  const DBusError              *error,
                                                  DBusMessage                  *in_reply_to);
void            bus_transaction_cancel_and_free  (BusTransaction               *transaction);
void            bus_transaction_execute_and_free (BusTransaction               *transaction);
dbus_bool_t     bus_transaction_add_cancel_hook  (BusTransaction               *transaction,
                                                  BusTransactionCancelFunction  cancel_function,
                                                  void                         *data,
                                                  DBusFreeFunction              free_data_function);

int bus_connections_get_n_active                  (BusConnections *connections);
int bus_connections_get_n_incomplete              (BusConnections *connections);

/* called by stats.c, only present if DBUS_ENABLE_STATS */
int bus_connections_get_total_match_rules         (BusConnections *connections);
int bus_connections_get_peak_match_rules          (BusConnections *connections);
int bus_connections_get_peak_match_rules_per_conn (BusConnections *connections);
int bus_connections_get_total_bus_names           (BusConnections *connections);
int bus_connections_get_peak_bus_names            (BusConnections *connections);
int bus_connections_get_peak_bus_names_per_conn   (BusConnections *connections);

int bus_connection_get_peak_match_rules           (DBusConnection *connection);
int bus_connection_get_peak_bus_names             (DBusConnection *connection);

#endif /* BUS_CONNECTION_H */
