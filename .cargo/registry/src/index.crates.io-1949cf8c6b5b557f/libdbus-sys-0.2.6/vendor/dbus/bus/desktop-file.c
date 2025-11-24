/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* desktop-file.c  .desktop file parser
 *
 * Copyright (C) 2003  CodeFactory AB
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
#include <dbus/dbus-sysdeps.h>
#include <dbus/dbus-internals.h>
#include "desktop-file.h"
#include "utils.h"

typedef struct
{
  char *key;
  char *value;
} BusDesktopFileLine;

typedef struct
{
  char *section_name;
  
  int n_lines;
  BusDesktopFileLine *lines;
  int n_allocated_lines;  
} BusDesktopFileSection;

struct BusDesktopFile
{
  int n_sections;
  BusDesktopFileSection *sections;
  int n_allocated_sections;
};

/**
 * Parser for service files.
 */
typedef struct
{
  DBusString data; /**< The data from the file */

  BusDesktopFile *desktop_file; /**< The resulting object */
  int current_section;    /**< The current section being parsed */
  
  int pos;          /**< Current position */
  int len;          /**< Length */
  int line_num;     /**< Current line number */
  
} BusDesktopFileParser;

#define BUS_DESKTOP_FILE_PARSER_INIT \
  { \
    _DBUS_STRING_INIT_INVALID, \
    NULL, 0, 0, 0, 0 \
  }

#define VALID_KEY_CHAR 1
#define VALID_LOCALE_CHAR 2
static unsigned char valid[256] = { 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x3 , 0x2 , 0x0 , 
   0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 
   0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x0 , 0x0 , 0x0 , 0x0 , 0x2 , 
   0x0 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 
   0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x3 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
   0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 0x0 , 
};

static void report_error (BusDesktopFileParser *parser,
			  const char           *message,
			  const char           *error_name,
			  DBusError            *error);

static void
parser_clear (BusDesktopFileParser *parser)
{
  bus_clear_desktop_file (&parser->desktop_file);
  _dbus_string_free (&parser->data);
}

static void
bus_desktop_file_line_free (BusDesktopFileLine *line)
{
  dbus_free (line->key);
  dbus_free (line->value);
}

static void
bus_desktop_file_section_free (BusDesktopFileSection *section)
{
  int i;

  for (i = 0; i < section->n_lines; i++)
    bus_desktop_file_line_free (&section->lines[i]);

  dbus_free (section->lines);
  dbus_free (section->section_name);
}

void
bus_desktop_file_free (BusDesktopFile *desktop_file)
{
  int i;

  for (i = 0; i < desktop_file->n_sections; i++)
    bus_desktop_file_section_free (&desktop_file->sections[i]);
  dbus_free (desktop_file->sections);

  dbus_free (desktop_file);
}

static dbus_bool_t
grow_lines_in_section (BusDesktopFileSection *section)
{
  BusDesktopFileLine *lines;
  
  int new_n_lines;

  if (section->n_allocated_lines == 0)
    new_n_lines = 1;
  else
    new_n_lines = section->n_allocated_lines*2;

  lines = dbus_realloc (section->lines,
                        sizeof (BusDesktopFileLine) * new_n_lines);

  if (lines == NULL)
    return FALSE;
  
  section->lines = lines;
  section->n_allocated_lines = new_n_lines;

  return TRUE;
}

static dbus_bool_t
grow_sections (BusDesktopFile *desktop_file)
{
  int new_n_sections;
  BusDesktopFileSection *sections;
  
  if (desktop_file->n_allocated_sections == 0)
    new_n_sections = 1;
  else
    new_n_sections = desktop_file->n_allocated_sections*2;

  sections = dbus_realloc (desktop_file->sections,
                           sizeof (BusDesktopFileSection) * new_n_sections);
  if (sections == NULL)
    return FALSE;
  
  desktop_file->sections = sections;
  
  desktop_file->n_allocated_sections = new_n_sections;

  return TRUE;
}

static char *
unescape_string (BusDesktopFileParser *parser,
                 const DBusString     *str,
                 int                   pos,
                 int                   end_pos,
                 DBusError            *error)
{
  char *retval, *q;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  /* len + 1 is enough, because unescaping never makes the
   * string longer
   */
  retval = dbus_malloc (end_pos - pos + 1);
  if (retval == NULL)
    {
      BUS_SET_OOM (error);
      return NULL;
    }

  q = retval;
  
  while (pos < end_pos)
    {
      if (_dbus_string_get_byte (str, pos) == 0)
	{
	  /* Found an embedded null */
	  dbus_free (retval);
          report_error (parser, "Text to be unescaped contains embedded nul",
                        BUS_DESKTOP_PARSE_ERROR_INVALID_ESCAPES, error);
	  return NULL;
	}

      if (_dbus_string_get_byte (str, pos) == '\\')
	{
	  pos ++;

	  if (pos >= end_pos)
	    {
	      /* Escape at end of string */
	      dbus_free (retval);
              report_error (parser, "Text to be unescaped ended in \\",
                            BUS_DESKTOP_PARSE_ERROR_INVALID_ESCAPES, error);
	      return NULL;
	    }

	  switch (_dbus_string_get_byte (str, pos))
	    {
	    case 's':
              *q++ = ' ';
              break;
           case 't':
              *q++ = '\t';
              break;
           case 'n':
              *q++ = '\n';
              break;
           case 'r':
              *q++ = '\r';
              break;
           case '\\':
              *q++ = '\\';
              break;
           default:
	     /* Invalid escape code */
	     dbus_free (retval);
             report_error (parser, "Text to be unescaped had invalid escape sequence",
                           BUS_DESKTOP_PARSE_ERROR_INVALID_ESCAPES, error);
             return NULL;
	    }
	  pos++;
	}
      else
	{
	  *q++ =_dbus_string_get_byte (str, pos);

	  pos++;
	}
    }

  *q = 0;

  return retval;
}

static BusDesktopFileSection* 
new_section (BusDesktopFile *desktop_file,
             const char     *name)
{
  int n;
  char *name_copy;
  
  if (desktop_file->n_allocated_sections == desktop_file->n_sections)
    {
      if (!grow_sections (desktop_file))
        return NULL;
    }

  name_copy = _dbus_strdup (name);
  if (name_copy == NULL)
    return NULL;

  n = desktop_file->n_sections;
  desktop_file->sections[n].section_name = name_copy;

  desktop_file->sections[n].n_lines = 0;
  desktop_file->sections[n].lines = NULL;
  desktop_file->sections[n].n_allocated_lines = 0;

  if (!grow_lines_in_section (&desktop_file->sections[n]))
    {
      dbus_free (desktop_file->sections[n].section_name);
      desktop_file->sections[n].section_name = NULL;
      return NULL;
    }

  desktop_file->n_sections += 1;
  
  return &desktop_file->sections[n];  
}

static BusDesktopFileSection* 
open_section (BusDesktopFileParser *parser,
              const char           *name)
{  
  BusDesktopFileSection *section;

  section = new_section (parser->desktop_file, name);
  if (section == NULL)
    return NULL;
  
  parser->current_section = parser->desktop_file->n_sections - 1;
  _dbus_assert (&parser->desktop_file->sections[parser->current_section] == section);
  
  return section;
}

static BusDesktopFileLine *
new_line (BusDesktopFileParser *parser)
{
  BusDesktopFileSection *section;
  BusDesktopFileLine *line;
  
  section = &parser->desktop_file->sections[parser->current_section];

  if (section->n_allocated_lines == section->n_lines)
    {
      if (!grow_lines_in_section (section))
        return NULL;
    }

  line = &section->lines[section->n_lines++];

  _DBUS_ZERO(*line);
    
  return line;
}

static dbus_bool_t
is_blank_line (BusDesktopFileParser *parser)
{
  int p;
  char c;
  
  p = parser->pos;

  c = _dbus_string_get_byte (&parser->data, p);

  while (c && c != '\n')
    {
      if (!(c == ' ' || c == '\t' || c == '\n' || c == '\r' || c == '\f'))
	return FALSE;
      
      p++;
      c = _dbus_string_get_byte (&parser->data, p);
    }

  return TRUE;
}

static void
parse_comment_or_blank (BusDesktopFileParser *parser)
{
  int line_end, eol_len;
  
  if (!_dbus_string_find_eol (&parser->data, parser->pos, &line_end, &eol_len))
    line_end = parser->len;

  if (line_end == parser->len)
    parser->pos = parser->len;
  else
    parser->pos = line_end + eol_len;
  
  parser->line_num += 1;
}

static dbus_bool_t
is_valid_section_name (const DBusString *section_name)
{
  int i;
  int len;
  const unsigned char *data;

  len = _dbus_string_get_length (section_name);
  data = _dbus_string_get_const_udata_len (section_name, 0, len);

  /* 5. Group names may contain all ASCII characters except for control characters and '[' and ']'.
   *
   * We don't use isprint() here because it's locale-dependent. ASCII
   * characters <= 0x1f and 0x7f are control characters, and bytes with
   * values >= 0x80 aren't ASCII. 0x20 is a space, which we must allow,
   * not least because DBUS_SERVICE_SECTION contains one. */

  for (i = 0; i < len; i++)
    {
      unsigned char c = data[i];

      if (c <= 0x1f || c >= 0x7f || c  == '[' || c == ']')
	return FALSE;
    }

  return TRUE;
}

static dbus_bool_t
parse_section_start (BusDesktopFileParser *parser, DBusError *error)
{
  int line_end, eol_len;
  DBusString section_name;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
    
  if (!_dbus_string_find_eol (&parser->data, parser->pos, &line_end, &eol_len))
    line_end = parser->len;
  
  if (line_end - parser->pos <= 2 ||
      _dbus_string_get_byte (&parser->data, line_end - 1) != ']')
    {
      report_error (parser, "Invalid syntax for section header", BUS_DESKTOP_PARSE_ERROR_INVALID_SYNTAX, error);
      return FALSE;
    }

  if (!_dbus_string_init (&section_name))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_copy_len (&parser->data, parser->pos + 1,
                              line_end - parser->pos - 2,
                              &section_name, 0))
    {
      _dbus_string_free (&section_name);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!is_valid_section_name (&section_name))
    {
      report_error (parser, "Invalid characters in section name", BUS_DESKTOP_PARSE_ERROR_INVALID_CHARS, error);
      _dbus_string_free (&section_name);
      return FALSE;
    }

  if (open_section (parser, _dbus_string_get_const_data (&section_name)) == NULL)
    {
      _dbus_string_free (&section_name);
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (line_end == parser->len)
    parser->pos = parser->len;
  else
    parser->pos = line_end + eol_len;
  
  parser->line_num += 1;

  _dbus_string_free (&section_name);
  
  return TRUE;
}

static dbus_bool_t
parse_key_value (BusDesktopFileParser *parser, DBusError *error)
{
  int line_end, eol_len;
  int key_start, key_end;
  int value_start;
  int p;
  char *value, *tmp;
  DBusString key;
  BusDesktopFileLine *line;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  if (!_dbus_string_find_eol (&parser->data, parser->pos, &line_end, &eol_len))
    line_end = parser->len;
  
  p = parser->pos;
  key_start = p;
  while (p < line_end &&
	 (valid[_dbus_string_get_byte (&parser->data, p)] & VALID_KEY_CHAR))
    p++;
  key_end = p;
  
  if (key_start == key_end)
    {
      report_error (parser, "Empty key name", BUS_DESKTOP_PARSE_ERROR_INVALID_SYNTAX, error);
      return FALSE;
    }

  /* We ignore locales for now */
  if (p < line_end && _dbus_string_get_byte (&parser->data, p) == '[')
    {
      if (line_end == parser->len)
	parser->pos = parser->len;
      else
	parser->pos = line_end + eol_len;
	  
      parser->line_num += 1;

      return TRUE;
    }
  
  /* Skip space before '=' */
  while (p < line_end && _dbus_string_get_byte (&parser->data, p) == ' ')
    p++;

  if (p < line_end && _dbus_string_get_byte (&parser->data, p) != '=')
    {
      report_error (parser, "Invalid characters in key name", BUS_DESKTOP_PARSE_ERROR_INVALID_CHARS, error);
      return FALSE;
    }

  if (p == line_end)
    {
      report_error (parser, "No '=' in key/value pair", BUS_DESKTOP_PARSE_ERROR_INVALID_SYNTAX, error);
      return FALSE;
    }

  /* Skip the '=' */
  p++;

  /* Skip space after '=' */
  while (p < line_end && _dbus_string_get_byte (&parser->data, p) == ' ')
    p++;

  value_start = p;
  
  value = unescape_string (parser, &parser->data, value_start, line_end, error);
  if (value == NULL)
    {
      return FALSE;
    }

  line = new_line (parser);
  if (line == NULL)
    {
      dbus_free (value);
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_string_init (&key))
    {
      dbus_free (value);
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_string_copy_len (&parser->data, key_start, key_end - key_start,
                              &key, 0))
    {
      _dbus_string_free (&key);
      dbus_free (value);
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  if (!_dbus_string_steal_data (&key, &tmp))
    {
      _dbus_string_free (&key);
      dbus_free (value);
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  _dbus_string_free (&key);
  
  line->key = tmp;
  line->value = value;

  if (line_end == parser->len)
    parser->pos = parser->len;
  else
    parser->pos = line_end + eol_len;
  
  parser->line_num += 1;

  return TRUE;
}

static void
report_error (BusDesktopFileParser *parser,
	      const char           *message,
	      const char           *error_name,
	      DBusError            *error)
{
  const char *section_name = NULL;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  if (parser->current_section != -1)
    section_name = parser->desktop_file->sections[parser->current_section].section_name;

  if (section_name)
    dbus_set_error (error, error_name,
                    "Error in section %s at line %d: %s\n", section_name, parser->line_num, message);
  else
    dbus_set_error (error, error_name,
                    "Error at line %d: %s\n", parser->line_num, message);
}

BusDesktopFile*
bus_desktop_file_load (DBusString *filename,
		       DBusError  *error)
{
  BusDesktopFileParser parser = BUS_DESKTOP_FILE_PARSER_INIT;
  DBusStat sb;
  BusDesktopFile *result = NULL;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  /* Clearly there's a race here, but it's just to make it unlikely
   * that we do something silly, we still handle doing it below.
   */
  if (!_dbus_stat (filename, &sb, error))
    goto out;

  if (sb.size > _DBUS_ONE_KILOBYTE * 128)
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "Desktop file size (%ld bytes) is too large", (long) sb.size);
      goto out;
    }
  
  if (!_dbus_string_init (&parser.data))
    {
      BUS_SET_OOM (error);
      goto out;
    }

  if (!_dbus_file_get_contents (&parser.data, filename, error))
    goto out;

  if (!_dbus_string_validate_utf8 (&parser.data, 0,
                                   _dbus_string_get_length (&parser.data)))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "invalid UTF-8");   
      goto out;
    }
  
  parser.desktop_file = dbus_new0 (BusDesktopFile, 1);
  if (parser.desktop_file == NULL)
    {
      BUS_SET_OOM (error);
      goto out;
    }
  
  parser.line_num = 1;
  parser.pos = 0;
  parser.len = _dbus_string_get_length (&parser.data);
  parser.current_section = -1;

  while (parser.pos < parser.len)
    {
      if (_dbus_string_get_byte (&parser.data, parser.pos) == '[')
	{
	  if (!parse_section_start (&parser, error))
            goto out;
	}
      else if (is_blank_line (&parser) ||
	       _dbus_string_get_byte (&parser.data, parser.pos) == '#')
	parse_comment_or_blank (&parser);
      else if (parser.current_section < 0)
	{
           dbus_set_error(error, DBUS_ERROR_FAILED,
                          "invalid service file: key=value before [Section]");
           goto out;
	}
      else
	{
	  if (!parse_key_value (&parser, error))
            goto out;
	}
    }

  /* Transfer ownership on success */
  result = parser.desktop_file;
  parser.desktop_file = NULL;

out:
  _DBUS_ASSERT_ERROR_XOR_BOOL (error, result != NULL);
  parser_clear (&parser);
  return result;
}

static BusDesktopFileSection *
lookup_section (BusDesktopFile *desktop_file,
		const char     *section_name)
{
  BusDesktopFileSection *section;
  int i;
  
  if (section_name == NULL)
    return NULL;
  
  for (i = 0; i < desktop_file->n_sections; i ++)
    {
      section = &desktop_file->sections[i];

      if (strcmp (section->section_name, section_name) == 0)
	return section;
    }
  
  return NULL;
}

static BusDesktopFileLine *
lookup_line (BusDesktopFile        *desktop_file,
	     BusDesktopFileSection *section,
	     const char            *keyname)
{
  BusDesktopFileLine *line;
  int i;

  for (i = 0; i < section->n_lines; i++)
    {
      line = &section->lines[i];
      
      if (strcmp (line->key, keyname) == 0)
	return line;
    }
  
  return NULL;
}

dbus_bool_t
bus_desktop_file_get_raw (BusDesktopFile  *desktop_file,
			  const char      *section_name,
			  const char      *keyname,
			  const char     **val)
{
  BusDesktopFileSection *section;
  BusDesktopFileLine *line;

  *val = NULL;

  section = lookup_section (desktop_file, section_name);
  
  if (!section)
    return FALSE;

  line = lookup_line (desktop_file,
		      section,
		      keyname);

  if (!line)
    return FALSE;
  
  *val = line->value;
  
  return TRUE;
}

dbus_bool_t
bus_desktop_file_get_string (BusDesktopFile  *desktop_file,
			     const char      *section,
			     const char      *keyname,
			     char           **val,
			     DBusError       *error)
{
  const char *raw;
 
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  *val = NULL;
  
  if (!bus_desktop_file_get_raw (desktop_file, section, keyname, &raw))
    {
      dbus_set_error (error, DBUS_ERROR_FAILED,
                      "No \"%s\" key in .service file\n", keyname);
      return FALSE;
    }

  *val = _dbus_strdup (raw);

  if (*val == NULL)
    {
      BUS_SET_OOM (error);
      return FALSE;
    }
  
  return TRUE;
}
