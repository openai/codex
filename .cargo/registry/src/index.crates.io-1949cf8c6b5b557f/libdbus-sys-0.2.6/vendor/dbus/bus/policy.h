/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* policy.h  Bus security policy
 *
 * Copyright (C) 2003  Red Hat, Inc.
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

#ifndef BUS_POLICY_H
#define BUS_POLICY_H

#include <dbus/dbus.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-sysdeps.h>
#include "bus.h"

typedef enum
{
  BUS_POLICY_RULE_SEND,
  BUS_POLICY_RULE_RECEIVE,
  BUS_POLICY_RULE_OWN,
  BUS_POLICY_RULE_USER,
  BUS_POLICY_RULE_GROUP
} BusPolicyRuleType;

typedef enum
{
  BUS_POLICY_TRISTATE_ANY = 0,
  BUS_POLICY_TRISTATE_FALSE,
  BUS_POLICY_TRISTATE_TRUE
} BusPolicyTristate;

/** determines whether the rule affects a connection, or some global item */
#define BUS_POLICY_RULE_IS_PER_CLIENT(rule) (!((rule)->type == BUS_POLICY_RULE_USER || \
                                               (rule)->type == BUS_POLICY_RULE_GROUP))

struct BusPolicyRule
{
  int refcount;
  
  BusPolicyRuleType type;

  unsigned int allow : 1; /**< #TRUE if this allows, #FALSE if it denies */
  
  union
  {
    struct
    {
      /* message type can be DBUS_MESSAGE_TYPE_INVALID meaning "any" */
      int   message_type;
      /* any of these can be NULL meaning "any" */
      char *path;
      char *interface;
      char *member;
      char *error;
      char *destination;
      unsigned int max_fds;
      unsigned int min_fds;
      unsigned int eavesdrop : 1;
      unsigned int requested_reply : 1;
      unsigned int log : 1;
      unsigned int broadcast : 2; /**< really a BusPolicyTristate */
      unsigned int destination_is_prefix : 1;
    } send;

    struct
    {
      /* message type can be DBUS_MESSAGE_TYPE_INVALID meaning "any" */
      int   message_type;
      /* any of these can be NULL meaning "any" */
      char *path;
      char *interface;
      char *member;
      char *error;
      char *origin;
      unsigned int max_fds;
      unsigned int min_fds;
      unsigned int eavesdrop : 1;
      unsigned int requested_reply : 1;
    } receive;

    struct
    {
      /* can be NULL meaning "any" */
      char *service_name;
      /* if prefix is set, any name starting with service_name can be owned */
      unsigned int prefix : 1;
    } own;

    struct
    {
      /* can be DBUS_UID_UNSET meaning "any" */
      dbus_uid_t uid;
    } user;

    struct
    {
      /* can be DBUS_GID_UNSET meaning "any" */
      dbus_gid_t gid;
    } group;

  } d;
};

BusPolicyRule* bus_policy_rule_new   (BusPolicyRuleType type,
                                      dbus_bool_t       allow);
BusPolicyRule* bus_policy_rule_ref   (BusPolicyRule    *rule);
void           bus_policy_rule_unref (BusPolicyRule    *rule);

BusPolicy*       bus_policy_new                   (void);
BusPolicy*       bus_policy_ref                   (BusPolicy        *policy);
void             bus_policy_unref                 (BusPolicy        *policy);
BusClientPolicy* bus_policy_create_client_policy  (BusPolicy        *policy,
                                                   DBusConnection   *connection,
                                                   DBusError        *error);
dbus_bool_t      bus_policy_allow_unix_user       (BusPolicy        *policy,
                                                   unsigned long     uid);
dbus_bool_t      bus_policy_allow_windows_user    (BusPolicy        *policy,
                                                   const char       *windows_sid);
dbus_bool_t      bus_policy_append_default_rule   (BusPolicy        *policy,
                                                   BusPolicyRule    *rule);
dbus_bool_t      bus_policy_append_mandatory_rule (BusPolicy        *policy,
                                                   BusPolicyRule    *rule);
dbus_bool_t      bus_policy_append_user_rule      (BusPolicy        *policy,
                                                   dbus_uid_t        uid,
                                                   BusPolicyRule    *rule);
dbus_bool_t      bus_policy_append_group_rule     (BusPolicy        *policy,
                                                   dbus_gid_t        gid,
                                                   BusPolicyRule    *rule);
dbus_bool_t      bus_policy_append_console_rule   (BusPolicy        *policy,
                                                   dbus_bool_t        at_console,
                                                   BusPolicyRule    *rule);

dbus_bool_t      bus_policy_merge                 (BusPolicy        *policy,
                                                   BusPolicy        *to_absorb);

BusClientPolicy* bus_client_policy_new               (void);
BusClientPolicy* bus_client_policy_ref               (BusClientPolicy  *policy);
void             bus_client_policy_unref             (BusClientPolicy  *policy);
dbus_bool_t      bus_client_policy_check_can_send    (BusClientPolicy  *policy,
                                                      BusRegistry      *registry,
                                                      dbus_bool_t       requested_reply,
                                                      DBusConnection   *receiver,
                                                      DBusMessage      *message,
                                                      dbus_int32_t     *toggles,
                                                      dbus_bool_t      *log);
dbus_bool_t      bus_client_policy_check_can_receive (BusClientPolicy  *policy,
                                                      BusRegistry      *registry,
                                                      dbus_bool_t       requested_reply,
                                                      DBusConnection   *sender,
                                                      DBusConnection   *addressed_recipient,
                                                      DBusConnection   *proposed_recipient,
                                                      DBusMessage      *message,
                                                      dbus_int32_t     *toggles);
dbus_bool_t      bus_client_policy_check_can_own     (BusClientPolicy  *policy,
                                                      const DBusString *service_name);
dbus_bool_t      bus_client_policy_append_rule       (BusClientPolicy  *policy,
                                                      BusPolicyRule    *rule);
void             bus_client_policy_optimize          (BusClientPolicy  *policy);

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
dbus_bool_t      bus_policy_check_can_own     (BusPolicy  *policy,
                                               const DBusString *service_name);
#endif

#endif /* BUS_POLICY_H */
