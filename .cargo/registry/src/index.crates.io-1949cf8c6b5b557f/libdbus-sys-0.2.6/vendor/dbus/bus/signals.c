/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* signals.c  Bus signal connection implementation
 *
 * Copyright (C) 2003, 2005  Red Hat, Inc.
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

#include <string.h>

#include "signals.h"
#include "services.h"
#include "utils.h"
#include <dbus/dbus-marshal-validate.h>
#include <dbus/dbus-test-tap.h>

struct BusMatchRule
{
  int refcount;       /**< reference count */

  DBusConnection *matches_go_to; /**< Owner of the rule */

  unsigned int flags; /**< BusMatchFlags */

  int   message_type;
  char *interface;
  char *member;
  char *sender;
  char *destination;
  char *path;

  unsigned int *arg_lens;
  char **args;
  int args_len;
};

#define BUS_MATCH_ARG_NAMESPACE   0x4000000u
#define BUS_MATCH_ARG_IS_PATH  0x8000000u

#define BUS_MATCH_ARG_FLAGS (BUS_MATCH_ARG_NAMESPACE | BUS_MATCH_ARG_IS_PATH)

BusMatchRule*
bus_match_rule_new (DBusConnection *matches_go_to)
{
  BusMatchRule *rule;

  rule = dbus_new0 (BusMatchRule, 1);
  if (rule == NULL)
    return NULL;

  rule->refcount = 1;
  rule->matches_go_to = matches_go_to;

#ifndef DBUS_ENABLE_EMBEDDED_TESTS
  _dbus_assert (rule->matches_go_to != NULL);
#endif
  
  return rule;
}

BusMatchRule *
bus_match_rule_ref (BusMatchRule *rule)
{
  _dbus_assert (rule->refcount > 0);

  rule->refcount += 1;

  return rule;
}

void
bus_match_rule_unref (BusMatchRule *rule)
{
  _dbus_assert (rule->refcount > 0);

  rule->refcount -= 1;
  if (rule->refcount == 0)
    {
      dbus_free (rule->interface);
      dbus_free (rule->member);
      dbus_free (rule->sender);
      dbus_free (rule->destination);
      dbus_free (rule->path);
      dbus_free (rule->arg_lens);

      /* can't use dbus_free_string_array() since there
       * are embedded NULL
       */
      if (rule->args)
        {
          int i;

          i = 0;
          while (i < rule->args_len)
            {
              if (rule->args[i])
                dbus_free (rule->args[i]);
              ++i;
            }

          dbus_free (rule->args);
        }
      
      dbus_free (rule);
    }
}

#if defined(DBUS_ENABLE_VERBOSE_MODE) || defined(DBUS_ENABLE_STATS) || defined(DBUS_ENABLE_EMBEDDED_TESTS)
static dbus_bool_t
append_key_and_escaped_value (DBusString *str, const char *token, const char *value)
{
  const char *p = value;

  if (!_dbus_string_append_printf (str, "%s='", token))
    return FALSE;

  while (*p != '\0')
    {
      const char *next = strchr (p, '\'');

      if (next)
        {
          if (!_dbus_string_append_printf (str, "%.*s", (int) (next - p), p))
            return FALSE;
          /* Horrible escape sequence: single quote cannot be escaped inside
           * a single quoted string. So we close the single quote, escape the
           * single quote, and reopen a single quote.
           */
          if (!_dbus_string_append_printf (str, "'\\''"))
            return FALSE;
          p = next + 1;
        }
      else
        {
          if (!_dbus_string_append_printf (str, "%s", p))
            return FALSE;
          break;
        }
    }

  if (!_dbus_string_append_byte (str, '\''))
    return FALSE;

  return TRUE;
}

/* returns NULL if no memory */
static char*
match_rule_to_string (BusMatchRule *rule)
{
  DBusString str;
  char *ret;
  
  if (!_dbus_string_init (&str))
    {
      return NULL;
    }
  
  if (rule->flags & BUS_MATCH_MESSAGE_TYPE)
    {
      if (!append_key_and_escaped_value (&str, "type",
            dbus_message_type_to_string (rule->message_type)))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_INTERFACE)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }
      
      if (!append_key_and_escaped_value (&str, "interface", rule->interface))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_MEMBER)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }
      
      if (!append_key_and_escaped_value (&str, "member", rule->member))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_PATH)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }
      
      if (!append_key_and_escaped_value (&str, "path", rule->path))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_PATH_NAMESPACE)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }

      if (!append_key_and_escaped_value (&str, "path_namespace", rule->path))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_SENDER)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }
      
      if (!append_key_and_escaped_value (&str, "sender", rule->sender))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_DESTINATION)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }
      
      if (!append_key_and_escaped_value (&str, "destination", rule->destination))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_CLIENT_IS_EAVESDROPPING)
    {
      if (_dbus_string_get_length (&str) > 0)
        {
          if (!_dbus_string_append (&str, ","))
            goto nomem;
        }

      if (!append_key_and_escaped_value (&str, "eavesdrop",
            (rule->flags & BUS_MATCH_CLIENT_IS_EAVESDROPPING) ?
            "true" : "false"))
        goto nomem;
    }

  if (rule->flags & BUS_MATCH_ARGS)
    {
      int i;
      
      _dbus_assert (rule->args != NULL);

      i = 0;
      while (i < rule->args_len)
        {
          if (rule->args[i] != NULL)
            {
              dbus_bool_t is_path, is_namespace;

              if (_dbus_string_get_length (&str) > 0)
                {
                  if (!_dbus_string_append (&str, ","))
                    goto nomem;
                }

              is_path = (rule->arg_lens[i] & BUS_MATCH_ARG_IS_PATH) != 0;
              is_namespace = (rule->arg_lens[i] & BUS_MATCH_ARG_NAMESPACE) != 0;
              
              if (!_dbus_string_append_printf (&str,
                                               "arg%d%s",
                                               i,
                                               is_path ? "path" :
                                               is_namespace ? "namespace" : ""))
                goto nomem;
              if (!append_key_and_escaped_value (&str, "", rule->args[i]))
                goto nomem;
            }
          
          ++i;
        }
    }
  
  if (!_dbus_string_steal_data (&str, &ret))
    goto nomem;

  _dbus_string_free (&str);
  return ret;
  
 nomem:
  _dbus_string_free (&str);
  return NULL;
}
#endif /* defined(DBUS_ENABLE_VERBOSE_MODE) || defined(DBUS_ENABLE_STATS) || defined(DBUS_ENABLE_EMBEDDED_TESTS) */

dbus_bool_t
bus_match_rule_set_message_type (BusMatchRule *rule,
                                 int           type)
{
  rule->flags |= BUS_MATCH_MESSAGE_TYPE;

  rule->message_type = type;

  return TRUE;
}

dbus_bool_t
bus_match_rule_set_interface (BusMatchRule *rule,
                              const char   *interface)
{
  char *new;

  _dbus_assert (interface != NULL);

  new = _dbus_strdup (interface);
  if (new == NULL)
    return FALSE;

  rule->flags |= BUS_MATCH_INTERFACE;
  dbus_free (rule->interface);
  rule->interface = new;

  return TRUE;
}

dbus_bool_t
bus_match_rule_set_member (BusMatchRule *rule,
                           const char   *member)
{
  char *new;

  _dbus_assert (member != NULL);

  new = _dbus_strdup (member);
  if (new == NULL)
    return FALSE;

  rule->flags |= BUS_MATCH_MEMBER;
  dbus_free (rule->member);
  rule->member = new;

  return TRUE;
}

dbus_bool_t
bus_match_rule_set_sender (BusMatchRule *rule,
                           const char   *sender)
{
  char *new;

  _dbus_assert (sender != NULL);

  new = _dbus_strdup (sender);
  if (new == NULL)
    return FALSE;

  rule->flags |= BUS_MATCH_SENDER;
  dbus_free (rule->sender);
  rule->sender = new;

  return TRUE;
}

dbus_bool_t
bus_match_rule_set_destination (BusMatchRule *rule,
                                const char   *destination)
{
  char *new;

  _dbus_assert (destination != NULL);

  new = _dbus_strdup (destination);
  if (new == NULL)
    return FALSE;

  rule->flags |= BUS_MATCH_DESTINATION;
  dbus_free (rule->destination);
  rule->destination = new;

  return TRUE;
}

void
bus_match_rule_set_client_is_eavesdropping (BusMatchRule *rule,
                                            dbus_bool_t is_eavesdropping)
{
  if (is_eavesdropping)
    rule->flags |= BUS_MATCH_CLIENT_IS_EAVESDROPPING;
  else
    rule->flags &= ~(BUS_MATCH_CLIENT_IS_EAVESDROPPING);
}

dbus_bool_t
bus_match_rule_get_client_is_eavesdropping (BusMatchRule *rule)
{
  if (rule->flags & BUS_MATCH_CLIENT_IS_EAVESDROPPING)
    return TRUE;
  else
    return FALSE;
}

dbus_bool_t
bus_match_rule_set_path (BusMatchRule *rule,
                         const char   *path,
                         dbus_bool_t   is_namespace)
{
  char *new;

  _dbus_assert (path != NULL);

  new = _dbus_strdup (path);
  if (new == NULL)
    return FALSE;

  rule->flags &= ~(BUS_MATCH_PATH|BUS_MATCH_PATH_NAMESPACE);

  if (is_namespace)
    rule->flags |= BUS_MATCH_PATH_NAMESPACE;
  else
    rule->flags |= BUS_MATCH_PATH;

  dbus_free (rule->path);
  rule->path = new;

  return TRUE;
}

dbus_bool_t
bus_match_rule_set_arg (BusMatchRule     *rule,
                        int                arg,
                        const DBusString *value,
                        dbus_bool_t       is_path,
                        dbus_bool_t       is_namespace)
{
  int length;
  char *new;

  _dbus_assert (value != NULL);

  /* args_len is the number of args not including null termination
   * in the char**
   */
  if (arg >= rule->args_len)
    {
      unsigned int *new_arg_lens;
      char **new_args;
      int new_args_len;
      int i;

      new_args_len = arg + 1;

      /* add another + 1 here for null termination */
      new_args = dbus_realloc (rule->args,
                               sizeof (char *) * (new_args_len + 1));
      if (new_args == NULL)
        return FALSE;

      /* NULL the new slots */
      i = rule->args_len;
      while (i <= new_args_len) /* <= for null termination */
        {
          new_args[i] = NULL;
          ++i;
        }
      
      rule->args = new_args;

      /* and now add to the lengths */
      new_arg_lens = dbus_realloc (rule->arg_lens,
                                   sizeof (int) * (new_args_len + 1));

      if (new_arg_lens == NULL)
        return FALSE;

      /* zero the new slots */
      i = rule->args_len;
      while (i <= new_args_len) /* <= for null termination */
        {
          new_arg_lens[i] = 0;
          ++i;
        }

      rule->arg_lens = new_arg_lens;
      rule->args_len = new_args_len;
    }

  length = _dbus_string_get_length (value);
  if (!_dbus_string_copy_data (value, &new))
    return FALSE;

  rule->flags |= BUS_MATCH_ARGS;

  dbus_free (rule->args[arg]);
  rule->arg_lens[arg] = length;
  rule->args[arg] = new;

  if (is_path)
    rule->arg_lens[arg] |= BUS_MATCH_ARG_IS_PATH;

  if (is_namespace)
    rule->arg_lens[arg] |= BUS_MATCH_ARG_NAMESPACE;

  /* NULL termination didn't get busted */
  _dbus_assert (rule->args[rule->args_len] == NULL);
  _dbus_assert (rule->arg_lens[rule->args_len] == 0);

  return TRUE;
}

#define ISWHITE(c) (((c) == ' ') || ((c) == '\t') || ((c) == '\n') || ((c) == '\r'))

static dbus_bool_t
find_key (const DBusString *str,
          int               start,
          DBusString       *key,
          int              *value_pos,
          DBusError        *error)
{
  const char *p;
  const char *s;
  const char *key_start;
  const char *key_end;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  s = _dbus_string_get_const_data (str);

  p = s + start;

  while (*p && ISWHITE (*p))
    ++p;

  key_start = p;

  while (*p && *p != '=' && !ISWHITE (*p))
    ++p;

  key_end = p;

  while (*p && ISWHITE (*p))
    ++p;
  
  if (key_start == key_end)
    {
      /* Empty match rules or trailing whitespace are OK */
      *value_pos = p - s;
      return TRUE;
    }

  if (*p != '=')
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Match rule has a key with no subsequent '=' character");
      return FALSE;
    }
  ++p;
  
  if (!_dbus_string_append_len (key, key_start, key_end - key_start))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  *value_pos = p - s;
  
  return TRUE;
}

static dbus_bool_t
find_value (const DBusString *str,
            int               start,
            const char       *key,
            DBusString       *value,
            int              *value_end,
            DBusError        *error)
{
  const char *p;
  const char *s;
  char quote_char;
  int orig_len;

  _DBUS_ASSERT_ERROR_IS_CLEAR (error);
  
  orig_len = _dbus_string_get_length (value);
  
  s = _dbus_string_get_const_data (str);

  p = s + start;

  quote_char = '\0';

  while (*p)
    {
      if (quote_char == '\0')
        {
          switch (*p)
            {
            case '\0':
              goto done;

            case '\'':
              quote_char = '\'';
              goto next;
              
            case ',':
              ++p;
              goto done;

            case '\\':
              quote_char = '\\';
              goto next;
              
            default:
              if (!_dbus_string_append_byte (value, *p))
                {
                  BUS_SET_OOM (error);
                  goto failed;
                }
            }
        }
      else if (quote_char == '\\')
        {
          /* \ only counts as an escape if escaping a quote mark */
          if (*p != '\'')
            {
              if (!_dbus_string_append_byte (value, '\\'))
                {
                  BUS_SET_OOM (error);
                  goto failed;
                }
            }

          if (!_dbus_string_append_byte (value, *p))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
          
          quote_char = '\0';
        }
      else
        {
          _dbus_assert (quote_char == '\'');

          if (*p == '\'')
            {
              quote_char = '\0';
            }
          else
            {
              if (!_dbus_string_append_byte (value, *p))
                {
                  BUS_SET_OOM (error);
                  goto failed;
                }
            }
        }

    next:
      ++p;
    }

 done:

  if (quote_char == '\\')
    {
      if (!_dbus_string_append_byte (value, '\\'))
        {
          BUS_SET_OOM (error);
          goto failed;
        }
    }
  else if (quote_char == '\'')
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Unbalanced quotation marks in match rule");
      goto failed;
    }
  else
    _dbus_assert (quote_char == '\0');

  /* Zero-length values are allowed */
  
  *value_end = p - s;
  
  return TRUE;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  _dbus_string_set_length (value, orig_len);
  return FALSE;
}

/* duplicates aren't allowed so the real legitimate max is only 6 or
 * so. Leaving extra so we don't have to bother to update it.
 * FIXME this is sort of busted now with arg matching, but we let
 * you match on up to 10 args for now
 */
#define MAX_RULE_TOKENS 16

/* this is slightly too high level to be termed a "token"
 * but let's not be pedantic.
 */
typedef struct
{
  char *key;
  char *value;
} RuleToken;

static dbus_bool_t
tokenize_rule (const DBusString *rule_text,
               RuleToken         tokens[MAX_RULE_TOKENS],
               DBusError        *error) 
{
  int i;
  int pos;
  DBusString key;
  DBusString value;
  dbus_bool_t retval;

  retval = FALSE;
  
  if (!_dbus_string_init (&key))
    {
      BUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_string_init (&value))
    {
      _dbus_string_free (&key);
      BUS_SET_OOM (error);
      return FALSE;
    }

  i = 0;
  pos = 0;
  while (i < MAX_RULE_TOKENS &&
         pos < _dbus_string_get_length (rule_text))
    {
      _dbus_assert (tokens[i].key == NULL);
      _dbus_assert (tokens[i].value == NULL);

      if (!find_key (rule_text, pos, &key, &pos, error))
        goto out;

      if (_dbus_string_get_length (&key) == 0)
        goto next;
      
      if (!_dbus_string_steal_data (&key, &tokens[i].key))
        {
          BUS_SET_OOM (error);
          goto out;
        }

      if (!find_value (rule_text, pos, tokens[i].key, &value, &pos, error))
        goto out;

      if (!_dbus_string_steal_data (&value, &tokens[i].value))
        {
          BUS_SET_OOM (error);
          goto out;
        }

    next:
      ++i;
    }

  retval = TRUE;
  
 out:
  if (!retval)
    {
      i = 0;
      while (tokens[i].key || tokens[i].value)
        {
          dbus_free (tokens[i].key);
          dbus_free (tokens[i].value);
          tokens[i].key = NULL;
          tokens[i].value = NULL;
          ++i;
        }
    }
  
  _dbus_string_free (&key);
  _dbus_string_free (&value);
  
  return retval;
}

static dbus_bool_t
bus_match_rule_parse_arg_match (BusMatchRule     *rule,
                                const char       *key,
                                const DBusString *value,
                                DBusError        *error)
{
  dbus_bool_t is_path = FALSE;
  dbus_bool_t is_namespace = FALSE;
  DBusString key_str;
  unsigned long arg;
  int length;
  int end;

  /* For now, arg0='foo' always implies that 'foo' is a
   * DBUS_TYPE_STRING. Someday we could add an arg0type='int32' thing
   * if we wanted, which would specify another type, in which case
   * arg0='5' would have the 5 parsed as an int rather than string.
   */
  
  /* First we need to parse arg0 = 0, arg27 = 27 */

  _dbus_string_init_const (&key_str, key);
  length = _dbus_string_get_length (&key_str);

  if (_dbus_string_get_length (&key_str) < 4)
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Key '%s' in match rule starts with 'arg' but lacks an arg number. Should be 'arg0' or 'arg7' for example.\n", key);
      goto failed;
    }

  if (!_dbus_string_parse_uint (&key_str, 3, &arg, &end))
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Key '%s' in match rule starts with 'arg' but could not parse arg number. Should be 'arg0' or 'arg7' for example.\n", key);
      goto failed;
    }

  if (end != length)
    {
      int len1 = strlen ("path");
      if ((end + len1) == length &&
          _dbus_string_ends_with_c_str (&key_str, "path"))
        {
          is_path = TRUE;
        }
      else if (_dbus_string_equal_c_str (&key_str, "arg0namespace"))
        {
          int value_len = _dbus_string_get_length (value);

          is_namespace = TRUE;

          if (!_dbus_validate_bus_namespace (value, 0, value_len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                  "arg0namespace='%s' is not a valid prefix of a bus name",
                  _dbus_string_get_const_data (value));
              goto failed;
            }
        }
      else
        {
          dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
              "Key '%s' in match rule contains junk after argument number (%lu). Only 'arg%lupath' (for example) or 'arg0namespace' are valid", key, arg, arg);
          goto failed;
        }
    }

  /* If we didn't check this we could allocate a huge amount of RAM */
  if (arg > DBUS_MAXIMUM_MATCH_RULE_ARG_NUMBER)
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Key '%s' in match rule has arg number %lu but the maximum is %d.\n", key, (unsigned long) arg, DBUS_MAXIMUM_MATCH_RULE_ARG_NUMBER);
      goto failed;
    }
  
  if ((rule->flags & BUS_MATCH_ARGS) &&
      rule->args_len > (int) arg &&
      rule->args[arg] != NULL)
    {
      dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                      "Argument %s matched more than once in match rule\n", key);
      goto failed;
    }
  
  if (!bus_match_rule_set_arg (rule, arg, value, is_path, is_namespace))
    {
      BUS_SET_OOM (error);
      goto failed;
    }

  return TRUE;

 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  return FALSE;
}

/*
 * The format is comma-separated with strings quoted with single quotes
 * as for the shell (to escape a literal single quote, use '\'').
 *
 * type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='Foo',
 * path='/bar/foo',destination=':452345.34'
 *
 */
BusMatchRule*
bus_match_rule_parse (DBusConnection   *matches_go_to,
                      const DBusString *rule_text,
                      DBusError        *error)
{
  BusMatchRule *rule;
  RuleToken tokens[MAX_RULE_TOKENS+1]; /* NULL termination + 1 */
  int i;
  
  _DBUS_ASSERT_ERROR_IS_CLEAR (error);

  if (_dbus_string_get_length (rule_text) > DBUS_MAXIMUM_MATCH_RULE_LENGTH)
    {
      dbus_set_error (error, DBUS_ERROR_LIMITS_EXCEEDED,
                      "Match rule text is %d bytes, maximum is %d",
                      _dbus_string_get_length (rule_text),
                      DBUS_MAXIMUM_MATCH_RULE_LENGTH);
      return NULL;
    }
  
  memset (tokens, '\0', sizeof (tokens));
  
  rule = bus_match_rule_new (matches_go_to);
  if (rule == NULL)
    {
      BUS_SET_OOM (error);
      goto failed;
    }
  
  if (!tokenize_rule (rule_text, tokens, error))
    goto failed;
  
  i = 0;
  while (tokens[i].key != NULL)
    {
      DBusString tmp_str;
      int len;
      const char *key = tokens[i].key;
      const char *value = tokens[i].value;
      
      _dbus_string_init_const (&tmp_str, value);
      len = _dbus_string_get_length (&tmp_str);

      if (strcmp (key, "type") == 0)
        {
          int t;

          if (rule->flags & BUS_MATCH_MESSAGE_TYPE)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Key %s specified twice in match rule\n", key);
              goto failed;
            }
          
          t = dbus_message_type_from_string (value);
          
          if (t == DBUS_MESSAGE_TYPE_INVALID)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Invalid message type (%s) in match rule\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_message_type (rule, t))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "sender") == 0)
        {
          if (rule->flags & BUS_MATCH_SENDER)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Key %s specified twice in match rule\n", key);
              goto failed;
            }

          if (!_dbus_validate_bus_name (&tmp_str, 0, len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Sender name '%s' is invalid\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_sender (rule, value))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "interface") == 0)
        {
          if (rule->flags & BUS_MATCH_INTERFACE)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Key %s specified twice in match rule\n", key);
              goto failed;
            }

          if (!_dbus_validate_interface (&tmp_str, 0, len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Interface name '%s' is invalid\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_interface (rule, value))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "member") == 0)
        {
          if (rule->flags & BUS_MATCH_MEMBER)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Key %s specified twice in match rule\n", key);
              goto failed;
            }

          if (!_dbus_validate_member (&tmp_str, 0, len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Member name '%s' is invalid\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_member (rule, value))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "path") == 0 ||
          strcmp (key, "path_namespace") == 0)
        {
          dbus_bool_t is_namespace = (strcmp (key, "path_namespace") == 0);

          if (rule->flags & (BUS_MATCH_PATH | BUS_MATCH_PATH_NAMESPACE))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "path or path_namespace specified twice in match rule\n");
              goto failed;
            }

          if (!_dbus_validate_path (&tmp_str, 0, len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Path '%s' is invalid\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_path (rule, value, is_namespace))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "destination") == 0)
        {
          if (rule->flags & BUS_MATCH_DESTINATION)
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Key %s specified twice in match rule\n", key);
              goto failed;
            }

          if (!_dbus_validate_bus_name (&tmp_str, 0, len))
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "Destination name '%s' is invalid\n", value);
              goto failed;
            }

          if (!bus_match_rule_set_destination (rule, value))
            {
              BUS_SET_OOM (error);
              goto failed;
            }
        }
      else if (strcmp (key, "eavesdrop") == 0)
        {
          /* do not detect "eavesdrop" being used more than once in rule:
           * 1) it's not possible, it's only in the flags
           * 2) it might be used twice to disable eavesdropping when it's
           * automatically added (eg dbus-monitor/bustle) */

          /* we accept only "true|false" as possible values */
          if ((strcmp (value, "true") == 0))
            {
              bus_match_rule_set_client_is_eavesdropping (rule, TRUE);
            }
          else if (strcmp (value, "false") == 0)
            {
              bus_match_rule_set_client_is_eavesdropping (rule, FALSE);
            }
          else
            {
              dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                              "eavesdrop='%s' is invalid, "
                              "it should be 'true' or 'false'\n",
                              value);
              goto failed;
            }
        }
      else if (strncmp (key, "arg", 3) == 0)
        {
          if (!bus_match_rule_parse_arg_match (rule, key, &tmp_str, error))
            goto failed;
        }
      else
        {
          dbus_set_error (error, DBUS_ERROR_MATCH_RULE_INVALID,
                          "Unknown key \"%s\" in match rule",
                          key);
          goto failed;
        }

      ++i;
    }
  

  goto out;
  
 failed:
  _DBUS_ASSERT_ERROR_IS_SET (error);
  if (rule)
    {
      bus_match_rule_unref (rule);
      rule = NULL;
    }

 out:
  
  i = 0;
  while (tokens[i].key || tokens[i].value)
    {
      _dbus_assert (i < MAX_RULE_TOKENS);
      dbus_free (tokens[i].key);
      dbus_free (tokens[i].value);
      ++i;
    }
  
  return rule;
}

typedef struct RulePool RulePool;
struct RulePool
{
  /* Maps non-NULL interface names to non-NULL (DBusList **)s */
  DBusHashTable *rules_by_iface;

  /* List of BusMatchRules which don't specify an interface */
  DBusList *rules_without_iface;
};

struct BusMatchmaker
{
  int refcount;

  /* Pools of rules, grouped by the type of message they match. 0
   * (DBUS_MESSAGE_TYPE_INVALID) represents rules that do not specify a message
   * type.
   */
  RulePool rules_by_type[DBUS_NUM_MESSAGE_TYPES];
};

#ifdef DBUS_ENABLE_STATS
dbus_bool_t
bus_match_rule_dump (BusMatchmaker *matchmaker,
                     DBusConnection *conn_filter,
                     DBusMessageIter *arr_iter)
{
  int i;

  for (i = 0 ; i < DBUS_NUM_MESSAGE_TYPES ; i++)
    {
      DBusHashIter iter;
      DBusList **list;
      DBusList *link;

      _dbus_hash_iter_init (matchmaker->rules_by_type[i].rules_by_iface, &iter);
      while (_dbus_hash_iter_next (&iter))
        {
          list =  _dbus_hash_iter_get_value (&iter);
          for (link = _dbus_list_get_first_link (list);
               link != NULL;
               link = _dbus_list_get_next_link (list, link))
            {
              BusMatchRule *rule = link->data;

              if (rule->matches_go_to == conn_filter)
                {
                  char *s = match_rule_to_string (rule);

                  if (s == NULL)
                    return FALSE;

                  if (!dbus_message_iter_append_basic (arr_iter, DBUS_TYPE_STRING, &s))
                    {
                      dbus_free (s);
                      return FALSE;
                    }
                  dbus_free (s);
                }
            }
        }
      list = &matchmaker->rules_by_type[i].rules_without_iface;
      for (link = _dbus_list_get_first_link (list);
           link != NULL;
           link = _dbus_list_get_next_link (list, link))
        {
          BusMatchRule *rule = link->data;

          if (rule->matches_go_to == conn_filter)
            {
              char *s = match_rule_to_string (rule);

              if (s == NULL)
                return FALSE;

              if (!dbus_message_iter_append_basic (arr_iter, DBUS_TYPE_STRING, &s))
                {
                  dbus_free (s);
                  return FALSE;
                }
              dbus_free (s);
            }
        }
    }

  return TRUE;
}
#endif

static void
rule_list_free (DBusList **rules)
{
  while (*rules != NULL)
    {
      BusMatchRule *rule;

      rule = (*rules)->data;
      bus_match_rule_unref (rule);
      _dbus_list_remove_link (rules, *rules);
    }
}

static void
rule_list_ptr_free (DBusList **list)
{
  /* We have to cope with NULL because the hash table frees the "existing"
   * value (which is NULL) when creating a new table entry...
   */
  if (list != NULL)
    {
      rule_list_free (list);
      dbus_free (list);
    }
}

BusMatchmaker*
bus_matchmaker_new (void)
{
  BusMatchmaker *matchmaker;
  int i;

  matchmaker = dbus_new0 (BusMatchmaker, 1);
  if (matchmaker == NULL)
    return NULL;

  matchmaker->refcount = 1;

  for (i = DBUS_MESSAGE_TYPE_INVALID; i < DBUS_NUM_MESSAGE_TYPES; i++)
    {
      RulePool *p = matchmaker->rules_by_type + i;

      p->rules_by_iface = _dbus_hash_table_new (DBUS_HASH_STRING,
          dbus_free, (DBusFreeFunction) rule_list_ptr_free);

      if (p->rules_by_iface == NULL)
        goto nomem;
    }

  return matchmaker;

 nomem:
  for (i = DBUS_MESSAGE_TYPE_INVALID; i < DBUS_NUM_MESSAGE_TYPES; i++)
    {
      RulePool *p = matchmaker->rules_by_type + i;

      if (p->rules_by_iface == NULL)
        break;
      else
        _dbus_hash_table_unref (p->rules_by_iface);
    }
  dbus_free (matchmaker);

  return NULL;
}

static DBusList **
bus_matchmaker_get_rules (BusMatchmaker *matchmaker,
                          int            message_type,
                          const char    *interface,
                          dbus_bool_t    create)
{
  RulePool *p;

  _dbus_assert (message_type >= 0);
  _dbus_assert (message_type < DBUS_NUM_MESSAGE_TYPES);

  _dbus_verbose ("Looking up rules for message_type %d, interface %s\n",
                 message_type,
                 interface != NULL ? interface : "<null>");

  p = matchmaker->rules_by_type + message_type;

  if (interface == NULL)
    {
      return &p->rules_without_iface;
    }
  else
    {
      DBusList **list;

      list = _dbus_hash_table_lookup_string (p->rules_by_iface, interface);

      if (list == NULL && create)
        {
          char *dupped_interface;

          list = dbus_new0 (DBusList *, 1);
          if (list == NULL)
            return NULL;

          dupped_interface = _dbus_strdup (interface);
          if (dupped_interface == NULL)
            {
              dbus_free (list);
              return NULL;
            }

          _dbus_verbose ("Adding list for type %d, iface %s\n", message_type,
                         interface);

          if (!_dbus_hash_table_insert_string (p->rules_by_iface,
                                               dupped_interface, list))
            {
              dbus_free (list);
              dbus_free (dupped_interface);
              return NULL;
            }
        }

      return list;
    }
}

static void
bus_matchmaker_gc_rules (BusMatchmaker *matchmaker,
                         int            message_type,
                         const char    *interface,
                         DBusList     **rules)
{
  RulePool *p;

  if (interface == NULL)
    return;

  if (*rules != NULL)
    return;

  _dbus_verbose ("GCing HT entry for message_type %u, interface %s\n",
                 message_type, interface);

  p = matchmaker->rules_by_type + message_type;

  _dbus_assert (_dbus_hash_table_lookup_string (p->rules_by_iface, interface)
      == rules);

  _dbus_hash_table_remove_string (p->rules_by_iface, interface);
}

BusMatchmaker *
bus_matchmaker_ref (BusMatchmaker *matchmaker)
{
  _dbus_assert (matchmaker->refcount > 0);

  matchmaker->refcount += 1;

  return matchmaker;
}

void
bus_matchmaker_unref (BusMatchmaker *matchmaker)
{
  _dbus_assert (matchmaker->refcount > 0);

  matchmaker->refcount -= 1;
  if (matchmaker->refcount == 0)
    {
      int i;

      for (i = DBUS_MESSAGE_TYPE_INVALID; i < DBUS_NUM_MESSAGE_TYPES; i++)
        {
          RulePool *p = matchmaker->rules_by_type + i;

          _dbus_hash_table_unref (p->rules_by_iface);
          rule_list_free (&p->rules_without_iface);
        }

      dbus_free (matchmaker);
    }
}

/* The rule can't be modified after it's added. */
dbus_bool_t
bus_matchmaker_add_rule (BusMatchmaker   *matchmaker,
                         BusMatchRule    *rule)
{
  DBusList **rules;

  _dbus_assert (bus_connection_is_active (rule->matches_go_to));

  _dbus_verbose ("Adding rule with message_type %d, interface %s\n",
                 rule->message_type,
                 rule->interface != NULL ? rule->interface : "<null>");

  rules = bus_matchmaker_get_rules (matchmaker, rule->message_type,
                                    rule->interface, TRUE);

  if (rules == NULL)
    return FALSE;

  if (!_dbus_list_append (rules, rule))
    return FALSE;

  if (!bus_connection_add_match_rule (rule->matches_go_to, rule))
    {
      _dbus_list_remove_last (rules, rule);
      bus_matchmaker_gc_rules (matchmaker, rule->message_type,
                               rule->interface, rules);
      return FALSE;
    }

  bus_match_rule_ref (rule);

#ifdef DBUS_ENABLE_VERBOSE_MODE
  {
    char *s = match_rule_to_string (rule);

    _dbus_verbose ("Added match rule %s to connection %p\n",
                   s ? s : "nomem", rule->matches_go_to);
    dbus_free (s);
  }
#endif
  
  return TRUE;
}

static dbus_bool_t
match_rule_equal (BusMatchRule *a,
                  BusMatchRule *b)
{
  if (a->flags != b->flags)
    return FALSE;

  if (a->matches_go_to != b->matches_go_to)
    return FALSE;

  if ((a->flags & BUS_MATCH_MESSAGE_TYPE) &&
      a->message_type != b->message_type)
    return FALSE;

  if ((a->flags & BUS_MATCH_MEMBER) &&
      strcmp (a->member, b->member) != 0)
    return FALSE;

  if ((a->flags & BUS_MATCH_PATH) &&
      strcmp (a->path, b->path) != 0)
    return FALSE;

  if ((a->flags & BUS_MATCH_INTERFACE) &&
      strcmp (a->interface, b->interface) != 0)
    return FALSE;

  if ((a->flags & BUS_MATCH_SENDER) &&
      strcmp (a->sender, b->sender) != 0)
    return FALSE;

  if ((a->flags & BUS_MATCH_DESTINATION) &&
      strcmp (a->destination, b->destination) != 0)
    return FALSE;

  /* we already compared the value of flags, and
   * BUS_MATCH_CLIENT_IS_EAVESDROPPING does not have another struct member */

  if (a->flags & BUS_MATCH_ARGS)
    {
      int i;
      
      if (a->args_len != b->args_len)
        return FALSE;
      
      i = 0;
      while (i < a->args_len)
        {
          int length;

          if ((a->args[i] != NULL) != (b->args[i] != NULL))
            return FALSE;

          if (a->arg_lens[i] != b->arg_lens[i])
            return FALSE;

          length = a->arg_lens[i] & ~BUS_MATCH_ARG_FLAGS;

          if (a->args[i] != NULL)
            {
              _dbus_assert (b->args[i] != NULL);
              if (memcmp (a->args[i], b->args[i], length) != 0)
                return FALSE;
            }
          
          ++i;
        }
    }
  
  return TRUE;
}

static void
bus_matchmaker_remove_rule_link (DBusList       **rules,
                                 DBusList        *link)
{
  BusMatchRule *rule = link->data;
  
  bus_connection_remove_match_rule (rule->matches_go_to, rule);
  _dbus_list_remove_link (rules, link);

#ifdef DBUS_ENABLE_VERBOSE_MODE
  {
    char *s = match_rule_to_string (rule);

    _dbus_verbose ("Removed match rule %s for connection %p\n",
                   s ? s : "nomem", rule->matches_go_to);
    dbus_free (s);
  }
#endif
  
  bus_match_rule_unref (rule);  
}

void
bus_matchmaker_remove_rule (BusMatchmaker   *matchmaker,
                            BusMatchRule    *rule)
{
  DBusList **rules;

  _dbus_verbose ("Removing rule with message_type %d, interface %s\n",
                 rule->message_type,
                 rule->interface != NULL ? rule->interface : "<null>");

  bus_connection_remove_match_rule (rule->matches_go_to, rule);

  rules = bus_matchmaker_get_rules (matchmaker, rule->message_type,
                                    rule->interface, FALSE);

  /* We should only be asked to remove a rule by identity right after it was
   * added, so there should be a list for it.
   */
  _dbus_assert (rules != NULL);

  _dbus_list_remove (rules, rule);
  bus_matchmaker_gc_rules (matchmaker, rule->message_type, rule->interface,
      rules);

#ifdef DBUS_ENABLE_VERBOSE_MODE
  {
    char *s = match_rule_to_string (rule);

    _dbus_verbose ("Removed match rule %s for connection %p\n",
                   s ? s : "nomem", rule->matches_go_to);
    dbus_free (s);
  }
#endif
  
  bus_match_rule_unref (rule);
}

/*
 * Prepare to remove the the most-recently-added rule which is equal to
 * the given rule by value, but do not actually do it yet.
 *
 * Return a linked-list link which must be treated as opaque by the caller:
 * the only valid thing to do with it is to pass it to
 * bus_matchmaker_commit_remove_rule_by_value().
 *
 * The returned linked-list link becomes invalid when control returns to
 * the main loop. If the caller decides not to remove the rule after all,
 * there is currently no need to cancel explicitly.
 */
DBusList *
bus_matchmaker_prepare_remove_rule_by_value (BusMatchmaker   *matchmaker,
                                             BusMatchRule    *value,
                                             DBusError       *error)
{
  DBusList **rules;
  DBusList *link = NULL;

  _dbus_verbose ("Finding rule by value with message_type %d, interface %s\n",
                 value->message_type,
                 value->interface != NULL ? value->interface : "<null>");

  rules = bus_matchmaker_get_rules (matchmaker, value->message_type,
                                    value->interface, FALSE);

  if (rules != NULL)
    {
      /* we traverse backward because bus_connection_remove_match_rule()
       * removes the most-recently-added rule
       */
      link = _dbus_list_get_last_link (rules);
      while (link != NULL)
        {
          BusMatchRule *rule;
          DBusList *prev;

          rule = link->data;
          prev = _dbus_list_get_prev_link (rules, link);

          if (match_rule_equal (rule, value))
            return link;

          link = prev;
        }
    }

  dbus_set_error (error, DBUS_ERROR_MATCH_RULE_NOT_FOUND,
                  "The given match rule wasn't found and can't be removed");
  return NULL;
}

/*
 * Commit a previous call to bus_matchmaker_prepare_remove_rule_by_value(),
 * which must have been done during the same main-loop iteration.
 */
void
bus_matchmaker_commit_remove_rule_by_value (BusMatchmaker *matchmaker,
                                            BusMatchRule  *value,
                                            DBusList      *link)
{
  DBusList **rules;

  _dbus_assert (match_rule_equal (link->data, value));
  rules = bus_matchmaker_get_rules (matchmaker, value->message_type,
                                    value->interface, FALSE);
  /* Should only be called if a rule matching value was successfully
   * added, which means rules must contain at least link */
  _dbus_assert (rules != NULL);
  bus_matchmaker_remove_rule_link (rules, link);
  bus_matchmaker_gc_rules (matchmaker, value->message_type, value->interface,
      rules);
}

static void
rule_list_remove_by_connection (DBusList       **rules,
                                DBusConnection  *connection)
{
  DBusList *link;

  link = _dbus_list_get_first_link (rules);
  while (link != NULL)
    {
      BusMatchRule *rule;
      DBusList *next;

      rule = link->data;
      next = _dbus_list_get_next_link (rules, link);

      if (rule->matches_go_to == connection)
        {
          bus_matchmaker_remove_rule_link (rules, link);
        }
      else if (((rule->flags & BUS_MATCH_SENDER) && *rule->sender == ':') ||
               ((rule->flags & BUS_MATCH_DESTINATION) && *rule->destination == ':'))
        {
          /* The rule matches to/from a base service, see if it's the
           * one being disconnected, since we know this service name
           * will never be recycled.
           */
          const char *name;

          name = bus_connection_get_name (connection);
          _dbus_assert (name != NULL); /* because we're an active connection */

          if (((rule->flags & BUS_MATCH_SENDER) &&
               strcmp (rule->sender, name) == 0) ||
              ((rule->flags & BUS_MATCH_DESTINATION) &&
               strcmp (rule->destination, name) == 0))
            {
              bus_matchmaker_remove_rule_link (rules, link);
            }
        }

      link = next;
    }
}

void
bus_matchmaker_disconnected (BusMatchmaker   *matchmaker,
                             DBusConnection  *connection)
{
  int i;

  /* FIXME
   *
   * This scans all match rules on the bus. We could avoid that
   * for the rules belonging to the connection, since we keep
   * a list of those; but for the rules that just refer to
   * the connection we'd need to do something more elaborate.
   */

  _dbus_assert (bus_connection_is_active (connection));

  _dbus_verbose ("Removing all rules for connection %p\n", connection);

  for (i = DBUS_MESSAGE_TYPE_INVALID; i < DBUS_NUM_MESSAGE_TYPES; i++)
    {
      RulePool *p = matchmaker->rules_by_type + i;
      DBusHashIter iter;

      rule_list_remove_by_connection (&p->rules_without_iface, connection);

      _dbus_hash_iter_init (p->rules_by_iface, &iter);
      while (_dbus_hash_iter_next (&iter))
        {
          DBusList **items = _dbus_hash_iter_get_value (&iter);

          rule_list_remove_by_connection (items, connection);

          if (*items == NULL)
            _dbus_hash_iter_remove_entry (&iter);
        }
    }
}

static dbus_bool_t
connection_is_primary_owner (DBusConnection *connection,
                             const char     *service_name)
{
  BusService *service;
  DBusString str;
  BusRegistry *registry;

  _dbus_assert (connection != NULL);
  
  registry = bus_connection_get_registry (connection);

  _dbus_string_init_const (&str, service_name);
  service = bus_registry_lookup (registry, &str);

  if (service == NULL)
    return FALSE; /* Service doesn't exist so connection can't own it. */

  return bus_service_get_primary_owners_connection (service) == connection;
}

static dbus_bool_t
str_has_prefix (const char *str, const char *prefix)
{
  size_t prefix_len;
  prefix_len = strlen (prefix);
  if (strncmp (str, prefix, prefix_len) == 0)
    return TRUE;
  else
    return FALSE;
}

static dbus_bool_t
match_rule_matches (BusMatchRule    *rule,
                    DBusConnection  *sender,
                    DBusConnection  *addressed_recipient,
                    DBusMessage     *message,
                    BusMatchFlags    already_matched)
{
  dbus_bool_t wants_to_eavesdrop = FALSE;
  int flags;

  /* All features of the match rule are AND'd together,
   * so FALSE if any of them don't match.
   */

  /* sender/addressed_recipient of #NULL may mean bus driver,
   * or for addressed_recipient may mean a message with no
   * specific recipient (i.e. a signal)
   */

  /* Don't bother re-matching features we've already checked implicitly. */
  flags = rule->flags & (~already_matched);

  if (flags & BUS_MATCH_CLIENT_IS_EAVESDROPPING)
    wants_to_eavesdrop = TRUE;

  if (flags & BUS_MATCH_MESSAGE_TYPE)
    {
      _dbus_assert (rule->message_type != DBUS_MESSAGE_TYPE_INVALID);

      if (rule->message_type != dbus_message_get_type (message))
        return FALSE;
    }

  if (flags & BUS_MATCH_INTERFACE)
    {
      const char *iface;

      _dbus_assert (rule->interface != NULL);

      iface = dbus_message_get_interface (message);
      if (iface == NULL)
        return FALSE;

      if (strcmp (iface, rule->interface) != 0)
        return FALSE;
    }

  if (flags & BUS_MATCH_MEMBER)
    {
      const char *member;

      _dbus_assert (rule->member != NULL);

      member = dbus_message_get_member (message);
      if (member == NULL)
        return FALSE;

      if (strcmp (member, rule->member) != 0)
        return FALSE;
    }

  if (flags & BUS_MATCH_SENDER)
    {
      _dbus_assert (rule->sender != NULL);

      if (sender == NULL)
        {
          if (strcmp (rule->sender,
                      DBUS_SERVICE_DBUS) != 0)
            return FALSE;
        }
      else
        {
          if (!connection_is_primary_owner (sender, rule->sender))
            return FALSE;
        }
    }

  /* Note: this part is relevant for eavesdropper rules:
   * Two cases:
   * 1) rule has a destination to be matched
   *   (flag BUS_MATCH_DESTINATION present). Rule will match if:
   *   - rule->destination matches the addressed_recipient
   *   AND
   *   - wants_to_eavesdrop=TRUE
   *
   *   Note: (the case in which addressed_recipient is the actual rule owner
   *   is handled elsewere in dispatch.c:bus_dispatch_matches().
   *
   * 2) rule has no destination. Rule will match if:
   *    - message has no specified destination (ie broadcasts)
   *      (Note: this will rule out unicast method calls and unicast signals,
   *      fixing FDO#269748)
   *    OR
   *    - wants_to_eavesdrop=TRUE (destination-catch-all situation)
   */
  if (flags & BUS_MATCH_DESTINATION)
    {
      const char *destination;

      _dbus_assert (rule->destination != NULL);

      destination = dbus_message_get_destination (message);
      if (destination == NULL)
        /* broadcast, but this rule specified a destination: no match */
        return FALSE;

      /* rule owner does not intend to eavesdrop: we'll deliver only msgs
       * directed to it, NOT MATCHING */
      if (!wants_to_eavesdrop)
        return FALSE;

      if (addressed_recipient == NULL)
        {
          /* If the message is going to be delivered to the dbus-daemon
           * itself, its destination will be "org.freedesktop.DBus",
           * which we again match against the rule (see bus_dispatch()
           * in bus/dispatch.c, which checks for o.fd.DBus first).
           *
           * If we are monitoring and we don't know who is going to receive
           * the message (for instance because they haven't been activated yet),
           * assume they will own the requested destination name and no other,
           * and match the rule's destination against that.
           */
          if (strcmp (rule->destination, destination) != 0)
            return FALSE;
        }
      else
        {
          if (!connection_is_primary_owner (addressed_recipient, rule->destination))
            return FALSE;
        }
    } else { /* no destination in rule */
        dbus_bool_t msg_is_broadcast;

        _dbus_assert (rule->destination == NULL);

        msg_is_broadcast = (dbus_message_get_destination (message) == NULL);

        if (!wants_to_eavesdrop && !msg_is_broadcast)
          return FALSE;

        /* if we are here rule owner intends to eavesdrop
         * OR
         * message is being broadcasted */
    }

  if (flags & BUS_MATCH_PATH)
    {
      const char *path;

      _dbus_assert (rule->path != NULL);

      path = dbus_message_get_path (message);
      if (path == NULL)
        return FALSE;

      if (strcmp (path, rule->path) != 0)
        return FALSE;
    }

  if (flags & BUS_MATCH_PATH_NAMESPACE)
    {
      const char *path;
      int len;

      _dbus_assert (rule->path != NULL);

      path = dbus_message_get_path (message);
      if (path == NULL)
        return FALSE;

      if (!str_has_prefix (path, rule->path))
        return FALSE;

      len = strlen (rule->path);

      /* Check that the actual argument is within the expected
       * namespace, rather than just starting with that string,
       * by checking that the matched prefix is followed by a '/'
       * or the end of the path.
       *
       * Special case: the only valid path of length 1, "/",
       * matches everything.
       */
      if (len > 1 && path[len] != '\0' && path[len] != '/')
        return FALSE;
    }

  if (flags & BUS_MATCH_ARGS)
    {
      int i;
      DBusMessageIter iter;
      
      _dbus_assert (rule->args != NULL);

      dbus_message_iter_init (message, &iter);
      
      i = 0;
      while (i < rule->args_len)
        {
          int current_type;
          const char *expected_arg;
          int expected_length;
          dbus_bool_t is_path, is_namespace;

          expected_arg = rule->args[i];
          expected_length = rule->arg_lens[i] & ~BUS_MATCH_ARG_FLAGS;
          is_path = (rule->arg_lens[i] & BUS_MATCH_ARG_IS_PATH) != 0;
          is_namespace = (rule->arg_lens[i] & BUS_MATCH_ARG_NAMESPACE) != 0;
          
          current_type = dbus_message_iter_get_arg_type (&iter);

          if (expected_arg != NULL)
            {
              const char *actual_arg;
              int actual_length;

              if (current_type != DBUS_TYPE_STRING &&
                  (!is_path || current_type != DBUS_TYPE_OBJECT_PATH))
                return FALSE;

              actual_arg = NULL;
              dbus_message_iter_get_basic (&iter, &actual_arg);
              _dbus_assert (actual_arg != NULL);

              actual_length = strlen (actual_arg);

              if (is_path)
                {
                  if (actual_length < expected_length &&
                      actual_arg[actual_length - 1] != '/')
                    return FALSE;

                  if (expected_length < actual_length &&
                      expected_arg[expected_length - 1] != '/')
                    return FALSE;

                  if (memcmp (actual_arg, expected_arg,
                              MIN (actual_length, expected_length)) != 0)
                    return FALSE;
                }
              else if (is_namespace)
                {
                  if (expected_length > actual_length)
                    return FALSE;

                  /* If the actual argument doesn't start with the expected
                   * namespace, then we don't match.
                   */
                  if (memcmp (expected_arg, actual_arg, expected_length) != 0)
                    return FALSE;

                  if (expected_length < actual_length)
                    {
                      /* Check that the actual argument is within the expected
                       * namespace, rather than just starting with that string,
                       * by checking that the matched prefix ends in a '.'.
                       *
                       * This doesn't stop "foo.bar." matching "foo.bar..baz"
                       * which is an invalid namespace, but at some point the
                       * daemon can't cover up for broken services.
                       */
                      if (actual_arg[expected_length] != '.')
                        return FALSE;
                    }
                  /* otherwise we had an exact match. */
                }
              else
                {
                  if (expected_length != actual_length ||
                      memcmp (expected_arg, actual_arg, expected_length) != 0)
                    return FALSE;
                }

            }
          
          if (current_type != DBUS_TYPE_INVALID)
            dbus_message_iter_next (&iter);

          ++i;
        }
    }
  
  return TRUE;
}

static dbus_bool_t
get_recipients_from_list (DBusList       **rules,
                          DBusConnection  *sender,
                          DBusConnection  *addressed_recipient,
                          DBusMessage     *message,
                          DBusList       **recipients_p)
{
  DBusList *link;

  if (rules == NULL)
    return TRUE;

  link = _dbus_list_get_first_link (rules);
  while (link != NULL)
    {
      BusMatchRule *rule;

      rule = link->data;

#ifdef DBUS_ENABLE_VERBOSE_MODE
      {
        char *s = match_rule_to_string (rule);

        _dbus_verbose ("Checking whether message matches rule %s for connection %p\n",
                       s ? s : "nomem", rule->matches_go_to);
        dbus_free (s);
      }
#endif

      if (match_rule_matches (rule,
                              sender, addressed_recipient, message,
                              BUS_MATCH_MESSAGE_TYPE | BUS_MATCH_INTERFACE))
        {
          _dbus_verbose ("Rule matched\n");

          /* Append to the list if we haven't already */
          if (bus_connection_mark_stamp (rule->matches_go_to))
            {
              if (!_dbus_list_append (recipients_p, rule->matches_go_to))
                return FALSE;
            }
          else
            {
              _dbus_verbose ("Connection already receiving this message, so not adding again\n");
            }
        }

      link = _dbus_list_get_next_link (rules, link);
    }

  return TRUE;
}

dbus_bool_t
bus_matchmaker_get_recipients (BusMatchmaker   *matchmaker,
                               BusConnections  *connections,
                               DBusConnection  *sender,
                               DBusConnection  *addressed_recipient,
                               DBusMessage     *message,
                               DBusList       **recipients_p)
{
  int type;
  const char *interface;
  DBusList **neither, **just_type, **just_iface, **both;

  _dbus_assert (*recipients_p == NULL);

  /* This avoids sending same message to the same connection twice.
   * Purpose of the stamp instead of a bool is to avoid iterating over
   * all connections resetting the bool each time.
   */
  bus_connections_increment_stamp (connections);

  /* addressed_recipient is already receiving the message, don't add to list.
   * NULL addressed_recipient means either bus driver, or this is a signal
   * and thus lacks a specific addressed_recipient.
   */
  if (addressed_recipient != NULL)
    bus_connection_mark_stamp (addressed_recipient);

  type = dbus_message_get_type (message);
  interface = dbus_message_get_interface (message);

  neither = bus_matchmaker_get_rules (matchmaker, DBUS_MESSAGE_TYPE_INVALID,
      NULL, FALSE);
  just_type = just_iface = both = NULL;

  if (interface != NULL)
    just_iface = bus_matchmaker_get_rules (matchmaker,
        DBUS_MESSAGE_TYPE_INVALID, interface, FALSE);

  if (type > DBUS_MESSAGE_TYPE_INVALID && type < DBUS_NUM_MESSAGE_TYPES)
    {
      just_type = bus_matchmaker_get_rules (matchmaker, type, NULL, FALSE);

      if (interface != NULL)
        both = bus_matchmaker_get_rules (matchmaker, type, interface, FALSE);
    }

  if (!(get_recipients_from_list (neither, sender, addressed_recipient,
                                  message, recipients_p) &&
        get_recipients_from_list (just_iface, sender, addressed_recipient,
                                  message, recipients_p) &&
        get_recipients_from_list (just_type, sender, addressed_recipient,
                                  message, recipients_p) &&
        get_recipients_from_list (both, sender, addressed_recipient,
                                  message, recipients_p)))
    {
      _dbus_list_clear (recipients_p);
      return FALSE;
    }

  return TRUE;
}

#ifdef DBUS_ENABLE_EMBEDDED_TESTS
#include "test.h"
#include <stdlib.h>

static BusMatchRule*
check_parse (dbus_bool_t should_succeed,
             const char *text)
{
  BusMatchRule *rule;
  DBusString str;
  DBusError error;

  dbus_error_init (&error);

  _dbus_string_init_const (&str, text);
  
  rule = bus_match_rule_parse (NULL, &str, &error);
  if (dbus_error_has_name (&error, DBUS_ERROR_NO_MEMORY))
    {
      dbus_error_free (&error);
      return NULL;
    }

  if (should_succeed && rule == NULL)
    {
      _dbus_warn ("Failed to parse: %s: %s: \"%s\"",
                  error.name, error.message,
                  _dbus_string_get_const_data (&str));
      exit (1);
    }

  if (!should_succeed && rule != NULL)
    {
      _dbus_warn ("Failed to fail to parse: \"%s\"",
                  _dbus_string_get_const_data (&str));
      exit (1);
    }

  dbus_error_free (&error);

  return rule;
}

static void
assert_large_rule (BusMatchRule *rule)
{
  _dbus_assert (rule->flags & BUS_MATCH_MESSAGE_TYPE);
  _dbus_assert (rule->flags & BUS_MATCH_SENDER);
  _dbus_assert (rule->flags & BUS_MATCH_INTERFACE);
  _dbus_assert (rule->flags & BUS_MATCH_MEMBER);
  _dbus_assert (rule->flags & BUS_MATCH_DESTINATION);
  _dbus_assert (rule->flags & BUS_MATCH_PATH);

  _dbus_assert (rule->message_type == DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (rule->interface != NULL);
  _dbus_assert (rule->member != NULL);
  _dbus_assert (rule->sender != NULL);
  _dbus_assert (rule->destination != NULL);
  _dbus_assert (rule->path != NULL);

  _dbus_assert (strcmp (rule->interface, "org.freedesktop.DBusInterface") == 0);
  _dbus_assert (strcmp (rule->sender, "org.freedesktop.DBusSender") == 0);
  _dbus_assert (strcmp (rule->member, "Foo") == 0);
  _dbus_assert (strcmp (rule->path, "/bar/foo") == 0);
  _dbus_assert (strcmp (rule->destination, ":452345.34") == 0);
}

static dbus_bool_t
test_parsing (void        *data,
              dbus_bool_t  have_memory)
{
  BusMatchRule *rule;

  rule = check_parse (TRUE, "type='signal',sender='org.freedesktop.DBusSender',interface='org.freedesktop.DBusInterface',member='Foo',path='/bar/foo',destination=':452345.34'");
  if (rule != NULL)
    {
      assert_large_rule (rule);
      bus_match_rule_unref (rule);
    }

  /* With extra whitespace and useless quotes */
  rule = check_parse (TRUE, "    type='signal',  \tsender='org.freedes''ktop.DBusSender',   interface='org.freedesktop.DBusInterface''''', \tmember='Foo',path='/bar/foo',destination=':452345.34'''''");
  if (rule != NULL)
    {
      assert_large_rule (rule);
      bus_match_rule_unref (rule);
    }


  /* A simple signal connection */
  rule = check_parse (TRUE, "type='signal',path='/foo',interface='org.Bar'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags & BUS_MATCH_MESSAGE_TYPE);
      _dbus_assert (rule->flags & BUS_MATCH_INTERFACE);
      _dbus_assert (rule->flags & BUS_MATCH_PATH);

      _dbus_assert (rule->message_type == DBUS_MESSAGE_TYPE_SIGNAL);
      _dbus_assert (rule->interface != NULL);
      _dbus_assert (rule->path != NULL);

      _dbus_assert (strcmp (rule->interface, "org.Bar") == 0);
      _dbus_assert (strcmp (rule->path, "/foo") == 0);
  
      bus_match_rule_unref (rule);
    }

  /* argN */
  rule = check_parse (TRUE, "arg0='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 1);
      _dbus_assert (rule->args[0] != NULL);
      _dbus_assert (rule->args[1] == NULL);
      _dbus_assert (strcmp (rule->args[0], "foo") == 0);

      bus_match_rule_unref (rule);
    }
  
  rule = check_parse (TRUE, "arg1='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 2);
      _dbus_assert (rule->args[0] == NULL);
      _dbus_assert (rule->args[1] != NULL);
      _dbus_assert (rule->args[2] == NULL);
      _dbus_assert (strcmp (rule->args[1], "foo") == 0);

      bus_match_rule_unref (rule);
    }

  rule = check_parse (TRUE, "arg2='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 3);
      _dbus_assert (rule->args[0] == NULL);
      _dbus_assert (rule->args[1] == NULL);
      _dbus_assert (rule->args[2] != NULL);
      _dbus_assert (rule->args[3] == NULL);
      _dbus_assert (strcmp (rule->args[2], "foo") == 0);

      bus_match_rule_unref (rule);
    }
  
  rule = check_parse (TRUE, "arg40='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 41);
      _dbus_assert (rule->args[0] == NULL);
      _dbus_assert (rule->args[1] == NULL);
      _dbus_assert (rule->args[40] != NULL);
      _dbus_assert (rule->args[41] == NULL);
      _dbus_assert (strcmp (rule->args[40], "foo") == 0);

      bus_match_rule_unref (rule);
    }
  
  rule = check_parse (TRUE, "arg63='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 64);
      _dbus_assert (rule->args[0] == NULL);
      _dbus_assert (rule->args[1] == NULL);
      _dbus_assert (rule->args[63] != NULL);
      _dbus_assert (rule->args[64] == NULL);
      _dbus_assert (strcmp (rule->args[63], "foo") == 0);

      bus_match_rule_unref (rule);
    }

  rule = check_parse (TRUE, "arg7path='/foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 8);
      _dbus_assert (rule->args[7] != NULL);
      _dbus_assert (rule->args[8] == NULL);
      _dbus_assert (strcmp (rule->args[7], "/foo") == 0);
      _dbus_assert ((rule->arg_lens[7] & BUS_MATCH_ARG_IS_PATH)
          == BUS_MATCH_ARG_IS_PATH);

      bus_match_rule_unref (rule);
    }

  /* Arg 0 namespace matches */
  rule = check_parse (TRUE, "arg0namespace='foo'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 1);
      _dbus_assert (strcmp (rule->args[0], "foo") == 0);
      _dbus_assert ((rule->arg_lens[0] & BUS_MATCH_ARG_NAMESPACE)
          == BUS_MATCH_ARG_NAMESPACE);

      bus_match_rule_unref (rule);
    }

  rule = check_parse (TRUE, "arg0namespace='foo.bar'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_ARGS);
      _dbus_assert (rule->args != NULL);
      _dbus_assert (rule->args_len == 1);
      _dbus_assert (strcmp (rule->args[0], "foo.bar") == 0);
      _dbus_assert ((rule->arg_lens[0] & BUS_MATCH_ARG_NAMESPACE)
          == BUS_MATCH_ARG_NAMESPACE);

      bus_match_rule_unref (rule);
    }

  /* Only arg0namespace is supported. */
  rule = check_parse (FALSE, "arg1namespace='foo'");
  _dbus_assert (rule == NULL);

  /* An empty string isn't a valid namespace prefix (you should just not
   * specify this key at all).
   */
  rule = check_parse (FALSE, "arg0namespace=''");
  _dbus_assert (rule == NULL);

  /* Trailing periods aren't allowed (earlier versions of the arg0namespace
   * spec allowed a single trailing period, which altered the semantics) */
  rule = check_parse (FALSE, "arg0namespace='foo.'");
  _dbus_assert (rule == NULL);

  rule = check_parse (FALSE, "arg0namespace='foo.bar.'");
  _dbus_assert (rule == NULL);

  rule = check_parse (FALSE, "arg0namespace='foo..'");
  _dbus_assert (rule == NULL);

  rule = check_parse (FALSE, "arg0namespace='foo.bar..'");
  _dbus_assert (rule == NULL);

  /* Too-large argN */
  rule = check_parse (FALSE, "arg300='foo'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "arg64='foo'");
  _dbus_assert (rule == NULL);

  /* No N in argN */
  rule = check_parse (FALSE, "arg='foo'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "argv='foo'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "arg3junk='foo'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "argument='foo'");
  _dbus_assert (rule == NULL);
  
  /* Reject duplicates */
  rule = check_parse (FALSE, "type='signal',type='method_call'");
  _dbus_assert (rule == NULL);

  rule = check_parse (TRUE, "path_namespace='/foo/bar'");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == BUS_MATCH_PATH_NAMESPACE);
      _dbus_assert (rule->path != NULL);
      _dbus_assert (strcmp (rule->path, "/foo/bar") == 0);

      bus_match_rule_unref (rule);
    }

  /* Almost a duplicate */
  rule = check_parse (FALSE, "path='/foo',path_namespace='/foo'");
  _dbus_assert (rule == NULL);

  /* Trailing / was supported in the initial proposal, but now isn't */
  rule = check_parse (FALSE, "path_namespace='/foo/'");
  _dbus_assert (rule == NULL);

  /* Duplicates with the argN code */
  rule = check_parse (FALSE, "arg0='foo',arg0='bar'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "arg3='foo',arg3='bar'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "arg30='foo',arg30='bar'");
  _dbus_assert (rule == NULL);
  
  /* Reject broken keys */
  rule = check_parse (FALSE, "blah='signal'");
  _dbus_assert (rule == NULL);

  /* Reject broken values */
  rule = check_parse (FALSE, "type='chouin'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "interface='abc@def++'");
  _dbus_assert (rule == NULL);
  rule = check_parse (FALSE, "service='youpi'");
  _dbus_assert (rule == NULL);

  /* Allow empty rule */
  rule = check_parse (TRUE, "");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == 0);
      
      bus_match_rule_unref (rule);
    }

  /* All-whitespace rule is the same as empty */
  rule = check_parse (TRUE, "    \t");
  if (rule != NULL)
    {
      _dbus_assert (rule->flags == 0);
      
      bus_match_rule_unref (rule);
    }

  /* But with non-whitespace chars and no =value, it's not OK */
  rule = check_parse (FALSE, "type");
  _dbus_assert (rule == NULL);
  
  return TRUE;
}

static struct {
  const char *first;
  const char *second;
} equality_tests[] = {
  { "type='signal'", "type='signal'" },
  { "type='signal',interface='foo.bar'", "interface='foo.bar',type='signal'" },
  { "type='signal',member='bar'", "member='bar',type='signal'" },
  { "type='method_call',sender=':1.0'", "sender=':1.0',type='method_call'" },
  { "type='method_call',destination=':1.0'", "destination=':1.0',type='method_call'" },
  { "type='method_call',path='/foo/bar'", "path='/foo/bar',type='method_call'" },
  { "type='method_call',arg0='blah'", "arg0='blah',type='method_call'" },
  { "type='method_call',arg0='boo'", "arg0='boo',type='method_call'" },
  { "type='method_call',arg0='blah',arg1='baz'", "arg0='blah',arg1='baz',type='method_call'" },
  { "type='method_call',arg3='foosh'", "arg3='foosh',type='method_call'" },
  { "arg3='fool'", "arg3='fool'" },
  { "arg0namespace='fool'", "arg0namespace='fool'" },
  { "member='food'", "member='food'" },
  { "member=escape", "member='escape'" },
  { "member=icecream", "member=ice'cream'" },
  { "arg0='comma,type=comma',type=signal", "type=signal,arg0='comma,type=comma'" },
  { "arg0=escap\\e", "arg0='escap\\e'" },
  { "arg0=Time: 8 o\\'clock", "arg0='Time: 8 o'\\''clock'" },
};

static void
test_equality (void)
{
  int i;
  
  i = 0;
  while (i < _DBUS_N_ELEMENTS (equality_tests))
    {
      BusMatchRule *first;
      BusMatchRule *second;
      char *first_str, *second_str;
      BusMatchRule *first_reparsed, *second_reparsed;
      int j;
      
      first = check_parse (TRUE, equality_tests[i].first);
      _dbus_assert (first != NULL);
      second = check_parse (TRUE, equality_tests[i].second);
      _dbus_assert (second != NULL);

      if (!match_rule_equal (first, second))
        {
          _dbus_warn ("rule %s and %s should have been equal",
                      equality_tests[i].first,
                      equality_tests[i].second);
          exit (1);
        }

      /* Check match_rule_to_string */
      first_str = match_rule_to_string (first);
      _dbus_assert (first_str != NULL);
      second_str = match_rule_to_string (second);
      _dbus_assert (second_str != NULL);
      _dbus_assert (strcmp (first_str, second_str) == 0);
      first_reparsed = check_parse (TRUE, first_str);
      _dbus_assert (first_reparsed != NULL);
      second_reparsed = check_parse (TRUE, second_str);
      _dbus_assert (second_reparsed != NULL);
      _dbus_assert (match_rule_equal (first, first_reparsed));
      _dbus_assert (match_rule_equal (second, second_reparsed));
      bus_match_rule_unref (first_reparsed);
      bus_match_rule_unref (second_reparsed);
      dbus_free (first_str);
      dbus_free (second_str);

      bus_match_rule_unref (second);

      /* Check that the rule is not equal to any of the
       * others besides its pair match
       */
      j = 0;
      while (j < _DBUS_N_ELEMENTS (equality_tests))
        {
          if (i != j)
            {
              second = check_parse (TRUE, equality_tests[j].second);
              _dbus_assert (second != NULL);

              if (match_rule_equal (first, second))
                {
                  _dbus_warn ("rule %s and %s should not have been equal",
                              equality_tests[i].first,
                              equality_tests[j].second);
                  exit (1);
                }
              
              bus_match_rule_unref (second);
            }
          
          ++j;
        }

      bus_match_rule_unref (first);

      ++i;
    }
}

static const char*
should_match_message_1[] = {
  "type='signal'",
  "member='Frobated'",
  "arg0='foobar'",
  "type='signal',member='Frobated'",
  "type='signal',member='Frobated',arg0='foobar'",
  "member='Frobated',arg0='foobar'",
  "type='signal',arg0='foobar'",
  /* The definition of argXpath matches says: "As with normal argument matches,
   * if the argument is exactly equal to the string given in the match rule
   * then the rule is satisfied." So this should match (even though the
   * argument is not a valid path)!
   */
  "arg0path='foobar'",
  "arg0namespace='foobar'",
  NULL
};

static const char*
should_not_match_message_1[] = {
  "type='method_call'",
  "type='error'",
  "type='method_return'",
  "type='signal',member='Oopsed'",
  "arg0='blah'",
  "arg1='foobar'",
  "arg2='foobar'",
  "arg3='foobar'",
  "arg0='3'",
  "arg1='3'",
  "arg0='foobar',arg1='abcdef'",
  "arg0='foobar',arg1='abcdef',arg2='abcdefghi',arg3='abcdefghi',arg4='abcdefghi'",
  "arg0='foobar',arg1='abcdef',arg4='abcdefghi',arg3='abcdefghi',arg2='abcdefghi'",
  "arg0path='foo'",
  "arg0path='foobar/'",
  "arg1path='3'",
  "arg0namespace='foo'",
  "arg0namespace='foo',arg1='abcdef'",
  "arg0namespace='moo'",
  NULL
};

#define EXAMPLE_NAME "com.example.backend.foo"

static const char *
should_match_message_2[] = {
  /* EXAMPLE_NAME is in all of these namespaces */
  "arg0namespace='com.example.backend'",
  "arg0namespace='com.example'",
  "arg0namespace='com'",

  /* If the client specifies the name exactly, with no trailing period, then
   * it should match.
   */
  "arg0namespace='com.example.backend.foo'",

  NULL
};

static const char *
should_not_match_message_2[] = {
  /* These are not even prefixes */
  "arg0namespace='com.example.backend.foo.bar'",
  "arg0namespace='com.example.backend.foobar'",

  /* These are prefixes, but they're not parent namespaces. */
  "arg0namespace='com.example.backend.fo'",
  "arg0namespace='com.example.backen'",
  "arg0namespace='com.exampl'",
  "arg0namespace='co'",

  NULL
};

static void
check_matches (dbus_bool_t  expected_to_match,
               int          number,
               DBusMessage *message,
               const char  *rule_text)
{
  BusMatchRule *rule;
  dbus_bool_t matched;

  rule = check_parse (TRUE, rule_text);
  _dbus_assert (rule != NULL);

  /* We can't test sender/destination rules since we pass NULL here */
  matched = match_rule_matches (rule, NULL, NULL, message, 0);

  if (matched != expected_to_match)
    {
      _dbus_warn ("Expected rule %s to %s message %d, failed",
                  rule_text, expected_to_match ?
                  "match" : "not match", number);
      exit (1);
    }

  bus_match_rule_unref (rule);
}

static void
check_matching (DBusMessage *message,
                int          number,
                const char **should_match,
                const char **should_not_match)
{
  int i;

  i = 0;
  while (should_match[i] != NULL)
    {
      check_matches (TRUE, number, message, should_match[i]);
      ++i;
    }

  i = 0;
  while (should_not_match[i] != NULL)
    {
      check_matches (FALSE, number, message, should_not_match[i]);
      ++i;
    }
}

static void
test_matching (void)
{
  DBusMessage *message1, *message2;
  const char *v_STRING;
  dbus_int32_t v_INT32;

  message1 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message1 != NULL);
  if (!dbus_message_set_member (message1, "Frobated"))
    _dbus_test_fatal ("oom");

  v_STRING = "foobar";
  v_INT32 = 3;
  if (!dbus_message_append_args (message1,
                                 DBUS_TYPE_STRING, &v_STRING,
                                 DBUS_TYPE_INT32, &v_INT32,
                                 NULL))
    _dbus_test_fatal ("oom");

  check_matching (message1, 1,
                  should_match_message_1,
                  should_not_match_message_1);
  
  dbus_message_unref (message1);

  message2 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message2 != NULL);
  if (!dbus_message_set_member (message2, "NameOwnerChanged"))
    _dbus_test_fatal ("oom");

  /* Obviously this isn't really a NameOwnerChanged signal. */
  v_STRING = EXAMPLE_NAME;
  if (!dbus_message_append_args (message2,
                                 DBUS_TYPE_STRING, &v_STRING,
                                 NULL))
    _dbus_test_fatal ("oom");

  check_matching (message2, 2,
                  should_match_message_2,
                  should_not_match_message_2);

  dbus_message_unref (message2);
}

#define PATH_MATCH_RULE "arg0path='/aa/bb/'"

/* This is a list of paths that should be matched by PATH_MATCH_RULE, taken
 * from the specification. Notice that not all of them are actually legal D-Bus
 * paths.
 *
 * The author of this test takes no responsibility for the semantics of
 * this match rule key.
 */
static const char *paths_that_should_be_matched[] = {
    "/aa/",
    "/aa/bb/",
    "/aa/bb/cc/",
#define FIRST_VALID_PATH_WHICH_SHOULD_MATCH 3
    "/",
    "/aa/bb/cc",
    NULL
};

/* These paths should not be matched by PATH_MATCH_RULE. */
static const char *paths_that_should_not_be_matched[] = {
    "/aa/b",
    "/aa",
    /* or even... */
    "/aa/bb",
    NULL
};

static void
test_path_match (int type,
                 const char   *path,
                 const char   *rule_text,
                 BusMatchRule *rule,
                 dbus_bool_t   should_match)
{
  DBusMessage *message = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  dbus_bool_t matched;

  _dbus_assert (message != NULL);
  if (!dbus_message_set_member (message, "Foo"))
    _dbus_test_fatal ("oom");

  if (!dbus_message_append_args (message,
                                 type, &path,
                                 NULL))
    _dbus_test_fatal ("oom");

  matched = match_rule_matches (rule, NULL, NULL, message, 0);

  if (matched != should_match)
    {
      _dbus_warn ("Expected rule %s to %s message "
                  "with first arg %s of type '%c', failed",
                  rule_text,
                  should_match ? "match" : "not match",
                  path,
                  (char) type);
      exit (1);
    }

  dbus_message_unref (message);
}

static void
test_path_matching (void)
{
  BusMatchRule *rule;
  const char **s;

  rule = check_parse (TRUE, PATH_MATCH_RULE);
  _dbus_assert (rule != NULL);

  for (s = paths_that_should_be_matched; *s != NULL; s++)
    test_path_match (DBUS_TYPE_STRING, *s, PATH_MATCH_RULE, rule, TRUE);

  for (s = paths_that_should_be_matched + FIRST_VALID_PATH_WHICH_SHOULD_MATCH;
       *s != NULL; s++)
    test_path_match (DBUS_TYPE_OBJECT_PATH, *s, PATH_MATCH_RULE, rule, TRUE);

  for (s = paths_that_should_not_be_matched; *s != NULL; s++)
    {
      test_path_match (DBUS_TYPE_STRING, *s, PATH_MATCH_RULE, rule, FALSE);
      test_path_match (DBUS_TYPE_OBJECT_PATH, *s, PATH_MATCH_RULE, rule, FALSE);
    }

  bus_match_rule_unref (rule);
}

static const char*
path_namespace_should_match_message_1[] = {
  "type='signal',path_namespace='/'",
  "type='signal',path_namespace='/foo'",
  "type='signal',path_namespace='/foo/TheObjectManager'",
  NULL
};

static const char*
path_namespace_should_not_match_message_1[] = {
  "type='signal',path_namespace='/bar'",
  "type='signal',path_namespace='/bar/TheObjectManager'",
  NULL
};

static const char*
path_namespace_should_match_message_2[] = {
  "type='signal',path_namespace='/'",
  "type='signal',path_namespace='/foo/TheObjectManager'",
  NULL
};

static const char*
path_namespace_should_not_match_message_2[] = {
  NULL
};

static const char*
path_namespace_should_match_message_3[] = {
  "type='signal',path_namespace='/'",
  NULL
};

static const char*
path_namespace_should_not_match_message_3[] = {
  "type='signal',path_namespace='/foo/TheObjectManager'",
  NULL
};

static const char*
path_namespace_should_match_message_4[] = {
  "type='signal',path_namespace='/'",
  NULL
};

static const char*
path_namespace_should_not_match_message_4[] = {
  "type='signal',path_namespace='/foo/TheObjectManager'",
  NULL
};

static void
test_matching_path_namespace (void)
{
  DBusMessage *message1;
  DBusMessage *message2;
  DBusMessage *message3;
  DBusMessage *message4;

  message1 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message1 != NULL);
  if (!dbus_message_set_path (message1, "/foo/TheObjectManager"))
    _dbus_test_fatal ("oom");

  message2 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message2 != NULL);
  if (!dbus_message_set_path (message2, "/foo/TheObjectManager/child_object"))
    _dbus_test_fatal ("oom");

  message3 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message3 != NULL);
  if (!dbus_message_set_path (message3, "/foo/TheObjectManagerOther"))
    _dbus_test_fatal ("oom");

  message4 = dbus_message_new (DBUS_MESSAGE_TYPE_SIGNAL);
  _dbus_assert (message4 != NULL);
  if (!dbus_message_set_path (message4, "/"))
    _dbus_test_fatal ("oom");

  check_matching (message1, 1,
                  path_namespace_should_match_message_1,
                  path_namespace_should_not_match_message_1);
  check_matching (message2, 2,
                  path_namespace_should_match_message_2,
                  path_namespace_should_not_match_message_2);
  check_matching (message3, 3,
                  path_namespace_should_match_message_3,
                  path_namespace_should_not_match_message_3);
  check_matching (message4, 4,
                  path_namespace_should_match_message_4,
                  path_namespace_should_not_match_message_4);

  dbus_message_unref (message4);
  dbus_message_unref (message3);
  dbus_message_unref (message2);
  dbus_message_unref (message1);
}

dbus_bool_t
bus_signals_test (const char *test_data_dir _DBUS_GNUC_UNUSED)
{
  BusMatchmaker *matchmaker;

  matchmaker = bus_matchmaker_new ();
  bus_matchmaker_ref (matchmaker);
  bus_matchmaker_unref (matchmaker);
  bus_matchmaker_unref (matchmaker);

  if (!_dbus_test_oom_handling ("parsing match rules", test_parsing, NULL))
    _dbus_test_fatal ("Parsing match rules test failed");

  test_equality ();
  test_matching ();
  test_path_matching ();
  test_matching_path_namespace ();

  return TRUE;
}

#endif /* DBUS_ENABLE_EMBEDDED_TESTS */
