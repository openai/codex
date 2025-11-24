/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* config-loader-expat.c  expat XML loader
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

#include <config.h>
#include "config-parser.h"
#include <dbus/dbus-internals.h>
#include <expat.h>

static XML_Memory_Handling_Suite memsuite;

typedef struct
{
  BusConfigParser *parser;
  const char *filename;
  DBusString content;
  DBusError *error;
  dbus_bool_t failed;
} ExpatParseContext;

static dbus_bool_t
process_content (ExpatParseContext *context)
{
  if (context->failed)
    return FALSE;

  if (_dbus_string_get_length (&context->content) > 0)
    {
      if (!bus_config_parser_content (context->parser,
                                      &context->content,
                                      context->error))
        {
          context->failed = TRUE;
          return FALSE;
        }
      _dbus_string_set_length (&context->content, 0);
    }

  return TRUE;
}

static void
expat_StartElementHandler (void            *userData,
                           const XML_Char  *name,
                           const XML_Char **atts)
{
  ExpatParseContext *context = userData;
  int i;
  char **names;
  char **values;

  /* Expat seems to suck and can't abort the parse if we
   * throw an error. Expat 2.0 is supposed to fix this.
   */
  if (context->failed)
    return;

  if (!process_content (context))
    return;

  /* "atts" is key, value, key, value, NULL */
  for (i = 0; atts[i] != NULL; ++i)
    ; /* nothing */

  _dbus_assert (i % 2 == 0);
  names = dbus_new0 (char *, i / 2 + 1);
  values = dbus_new0 (char *, i / 2 + 1);

  if (names == NULL || values == NULL)
    {
      dbus_set_error (context->error, DBUS_ERROR_NO_MEMORY, NULL);
      context->failed = TRUE;
      dbus_free (names);
      dbus_free (values);
      return;
    }

  i = 0;
  while (atts[i] != NULL)
    {
      _dbus_assert (i % 2 == 0);
      names [i / 2] = (char*) atts[i];
      values[i / 2] = (char*) atts[i+1];

      i += 2;
    }

  if (!bus_config_parser_start_element (context->parser,
                                        name,
                                        (const char **) names,
                                        (const char **) values,
                                        context->error))
    {
      dbus_free (names);
      dbus_free (values);
      context->failed = TRUE;
      return;
    }

  dbus_free (names);
  dbus_free (values);
}

static void
expat_EndElementHandler (void           *userData,
                         const XML_Char *name)
{
  ExpatParseContext *context = userData;

  if (!process_content (context))
    return;

  if (!bus_config_parser_end_element (context->parser,
                                      name,
                                      context->error))
    {
      context->failed = TRUE;
      return;
    }
}

/* s is not 0 terminated. */
static void
expat_CharacterDataHandler (void           *userData,
                            const XML_Char *s,
                            int             len)
{
  ExpatParseContext *context = userData;
  if (context->failed)
    return;

  if (!_dbus_string_append_len (&context->content,
                                s, len))
    {
      dbus_set_error (context->error, DBUS_ERROR_NO_MEMORY, NULL);
      context->failed = TRUE;
      return;
    }
}


BusConfigParser*
bus_config_load (const DBusString      *file,
                 dbus_bool_t            is_toplevel,
                 const BusConfigParser *parent,
                 DBusError             *error)
{
  XML_Parser expat;
  const char *filename;
  BusConfigParser *parser;
  ExpatParseContext context;
  DBusString dirname;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  parser = NULL;
  expat = NULL;
  context.error = error;
  context.failed = FALSE;

  filename = _dbus_string_get_const_data (file);

  if (!_dbus_string_init (&context.content))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      return NULL;
    }

  if (!_dbus_string_init (&dirname))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      _dbus_string_free (&context.content);
      return NULL;
    }

  memsuite.malloc_fcn = dbus_malloc;
  memsuite.realloc_fcn = dbus_realloc;
  memsuite.free_fcn = dbus_free;

  expat = XML_ParserCreate_MM ("UTF-8", &memsuite, NULL);
  if (expat == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }

  /* We do not need protection against hash collisions (CVE-2012-0876)
   * because we are only parsing trusted XML; and if we let Expat block
   * waiting for the CSPRNG to be initialized, as it does by default to
   * defeat CVE-2012-0876, it can cause timeouts during early boot on
   * entropy-starved embedded devices.
   *
   * TODO: When Expat gets a more explicit API for this than
   * XML_SetHashSalt, check for that too, and use it preferentially.
   * https://github.com/libexpat/libexpat/issues/91 */
#if defined(HAVE_XML_SETHASHSALT)
  /* Any nonzero number will do. https://xkcd.com/221/ */
  XML_SetHashSalt (expat, 4);
#endif

  if (!_dbus_string_get_dirname (file, &dirname))
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }
  
  parser = bus_config_parser_new (&dirname, is_toplevel, parent);
  if (parser == NULL)
    {
      dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
      goto failed;
    }
  context.parser = parser;

  XML_SetUserData (expat, &context);
  XML_SetElementHandler (expat,
                         expat_StartElementHandler,
                         expat_EndElementHandler);
  XML_SetCharacterDataHandler (expat,
                               expat_CharacterDataHandler);

  {
    DBusString data;
    const char *data_str;

    if (!_dbus_string_init (&data))
      {
        dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
        goto failed;
      }

    if (!_dbus_file_get_contents (&data, file, error))
      {
        _dbus_string_free (&data);
        goto failed;
      }

    data_str = _dbus_string_get_const_data (&data);

    if (XML_Parse (expat, data_str, _dbus_string_get_length (&data), TRUE) == XML_STATUS_ERROR)
      {
        if (context.error != NULL &&
            !dbus_error_is_set (context.error))
          {
            enum XML_Error e;

            e = XML_GetErrorCode (expat);
            if (e == XML_ERROR_NO_MEMORY)
              dbus_set_error (error, DBUS_ERROR_NO_MEMORY, NULL);
            else
              dbus_set_error (error, DBUS_ERROR_FAILED,
                              "Error in file %s, line %lu, column %lu: %s\n",
                              filename,
                              /* The XML_Size type varies according to
                               * build options, so cast to something we can
                               * cope with. */
                              (unsigned long) XML_GetCurrentLineNumber (expat),
                              (unsigned long) XML_GetCurrentColumnNumber (expat),
                              XML_ErrorString (e));
          }

        _dbus_string_free (&data);
        goto failed;
      }

    _dbus_string_free (&data);

    if (context.failed)
      goto failed;
  }

  if (!bus_config_parser_finished (parser, error))
    goto failed;

  _dbus_string_free (&dirname);
  _dbus_string_free (&context.content);
  XML_ParserFree (expat);

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  return parser;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);

  _dbus_string_free (&dirname);
  _dbus_string_free (&context.content);
  if (expat)
    XML_ParserFree (expat);
  if (parser)
    bus_config_parser_unref (parser);
  return NULL;
}
