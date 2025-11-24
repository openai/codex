/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* config-parser-trivial.h  XML-library-agnostic configuration file parser
 *
 * Copyright (C) 2003 Red Hat, Inc.
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

#ifndef BUS_CONFIG_PARSER_TRIVIAL_H
#define BUS_CONFIG_PARSER_TRIVIAL_H

#include <dbus/dbus.h>
#include <dbus/dbus-string.h>
#include <dbus/dbus-list.h>
#include <dbus/dbus-hash.h>

/* Whatever XML library we're using just pushes data into this API */

typedef struct BusConfigParser BusConfigParser;

BusConfigParser* bus_config_parser_new (const DBusString      *basedir,
                                        dbus_bool_t            is_toplevel,
                                        const BusConfigParser *parent);

BusConfigParser* bus_config_parser_ref           (BusConfigParser   *parser);
void             bus_config_parser_unref         (BusConfigParser   *parser);
dbus_bool_t      bus_config_parser_start_element (BusConfigParser   *parser,
                                                  const char        *element_name,
                                                  const char       **attribute_names,
                                                  const char       **attribute_values,
                                                  DBusError         *error);
dbus_bool_t      bus_config_parser_end_element   (BusConfigParser   *parser,
                                                  const char        *element_name,
                                                  DBusError         *error);
dbus_bool_t      bus_config_parser_content       (BusConfigParser   *parser,
                                                  const DBusString  *content,
                                                  DBusError         *error);
dbus_bool_t      bus_config_parser_finished      (BusConfigParser   *parser,
                                                  DBusError         *error);

/* Functions for extracting the parse results */
const char* bus_config_parser_get_user         (BusConfigParser *parser);
const char* bus_config_parser_get_type         (BusConfigParser *parser);
DBusList**  bus_config_parser_get_service_paths (BusConfigParser *parser);

/* Loader functions (backended off one of the XML parsers).  Returns a
 * finished ConfigParser.
 */
BusConfigParser* bus_config_load (const DBusString      *file,
                                  dbus_bool_t            is_toplevel,
                                  const BusConfigParser *parent,
                                  DBusError             *error);

#endif /* BUS_CONFIG_PARSER_TRIVIAL_H */
