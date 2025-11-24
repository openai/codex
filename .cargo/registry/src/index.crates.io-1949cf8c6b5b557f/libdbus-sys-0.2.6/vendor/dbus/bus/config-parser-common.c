/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* config-parser-common.c  Common defines and routines for config file parsing
 *
 * Copyright (C) 2007 Red Hat, Inc.
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
#include <dbus/dbus-internals.h>
#include <string.h>

#include "config-parser-common.h"
#include "utils.h"

ElementType
bus_config_parser_element_name_to_type (const char *name)
{
  if (strcmp (name, "none") == 0)
    {
      return ELEMENT_NONE;
    }
  else if (strcmp (name, "busconfig") == 0)
    {
      return ELEMENT_BUSCONFIG;
    }
  else if (strcmp (name, "user") == 0)
    {
      return ELEMENT_USER;
    }
  else if (strcmp (name, "auth") == 0)
    {
      return ELEMENT_AUTH;
    }
  else if (strcmp (name, "type") == 0)
    {
      return ELEMENT_CONFIGTYPE;
    }
  else if (strcmp (name, "fork") == 0)
    {
      return ELEMENT_FORK;
    }
  else if (strcmp (name, "pidfile") == 0)
    {
      return ELEMENT_PIDFILE;
    }
  else if (strcmp (name, "listen") == 0)
    {
      return ELEMENT_LISTEN;
    }
  else if (strcmp (name, "auth") == 0)
    {
      return ELEMENT_AUTH;
    }
  else if (strcmp (name, "allow") == 0)
    {
      return ELEMENT_ALLOW;
    }
  else if (strcmp (name, "deny") == 0)
    {
      return ELEMENT_DENY;
    }
  else if (strcmp (name, "servicehelper") == 0)
    {
      return ELEMENT_SERVICEHELPER;
    }
  else if (strcmp (name, "includedir") == 0)
    {
      return ELEMENT_INCLUDEDIR;
    }
  else if (strcmp (name, "standard_session_servicedirs") == 0)
    {
      return ELEMENT_STANDARD_SESSION_SERVICEDIRS;
    }
  else if (strcmp (name, "standard_system_servicedirs") == 0)
    {
      return ELEMENT_STANDARD_SYSTEM_SERVICEDIRS;
    }
  else if (strcmp (name, "servicedir") == 0)
    {
      return ELEMENT_SERVICEDIR;
    }
  else if (strcmp (name, "include") == 0)
    {
      return ELEMENT_INCLUDE;
    }
  else if (strcmp (name, "policy") == 0)
    {
      return ELEMENT_POLICY;
    }
  else if (strcmp (name, "limit") == 0)
    {
      return ELEMENT_LIMIT;
    }
  else if (strcmp (name, "selinux") == 0)
    {
      return ELEMENT_SELINUX;
    }
  else if (strcmp (name, "associate") == 0)
    {
      return ELEMENT_ASSOCIATE;
    }
  else if (strcmp (name, "syslog") == 0)
    {
      return ELEMENT_SYSLOG;
    }
  else if (strcmp (name, "keep_umask") == 0)
    {
      return ELEMENT_KEEP_UMASK;
    }
  else if (strcmp (name, "allow_anonymous") == 0)
    {
      return ELEMENT_ALLOW_ANONYMOUS;
    }
  else if (strcmp (name, "apparmor") == 0)
    {
      return ELEMENT_APPARMOR;
    }
  return ELEMENT_NONE;
}

const char*
bus_config_parser_element_type_to_name (ElementType type)
{
  switch (type)
    {
    case ELEMENT_NONE:
      return NULL;
    case ELEMENT_BUSCONFIG:
      return "busconfig";
    case ELEMENT_INCLUDE:
      return "include";
    case ELEMENT_USER:
      return "user";
    case ELEMENT_LISTEN:
      return "listen";
    case ELEMENT_AUTH:
      return "auth";
    case ELEMENT_POLICY:
      return "policy";
    case ELEMENT_LIMIT:
      return "limit";
    case ELEMENT_ALLOW:
      return "allow";
    case ELEMENT_DENY:
      return "deny";
    case ELEMENT_FORK:
      return "fork";
    case ELEMENT_PIDFILE:
      return "pidfile";
    case ELEMENT_STANDARD_SESSION_SERVICEDIRS:
      return "standard_session_servicedirs";
    case ELEMENT_STANDARD_SYSTEM_SERVICEDIRS:
      return "standard_system_servicedirs";
    case ELEMENT_SERVICEDIR:
      return "servicedir";
    case ELEMENT_SERVICEHELPER:
      return "servicehelper";
    case ELEMENT_INCLUDEDIR:
      return "includedir";
    case ELEMENT_CONFIGTYPE:
      return "type";
    case ELEMENT_SELINUX:
      return "selinux";
    case ELEMENT_ASSOCIATE:
      return "associate";
    case ELEMENT_SYSLOG:
      return "syslog";
    case ELEMENT_KEEP_UMASK:
      return "keep_umask";
    case ELEMENT_ALLOW_ANONYMOUS:
      return "allow_anonymous";
    case ELEMENT_APPARMOR:
      return "apparmor";
    default:
      _dbus_assert_not_reached ("bad element type");
      return NULL;
    }
}
