/* -*- mode: C; c-file-style: "gnu"; indent-tabs-mode: nil; -*- */
/* dbus-shell.c Shell command line utility functions.
 *
 * Copyright (C) 2002, 2003  Red Hat, Inc.
 * Copyright (C) 2003 CodeFactory AB
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
#include "dbus-internals.h"
#include "dbus-list.h"
#include "dbus-memory.h"
#include "dbus-protocol.h"
#include "dbus-shell.h"
#include "dbus-string.h"

/* Single quotes preserve the literal string exactly. escape
 * sequences are not allowed; not even \' - if you want a '
 * in the quoted text, you have to do something like 'foo'\''bar'
 *
 * Double quotes allow $ ` " \ and newline to be escaped with backslash.
 * Otherwise double quotes preserve things literally.
 */

static dbus_bool_t
unquote_string_inplace (char* str, char** end)
{
  char* dest;
  char* s;
  char quote_char;
  
  dest = s = str;

  quote_char = *s;
  
  if (!(*s == '"' || *s == '\''))
    {
      *end = str;
      return FALSE;
    }

  /* Skip the initial quote mark */
  ++s;

  if (quote_char == '"')
    {
      while (*s)
        {
          _dbus_assert(s > dest); /* loop invariant */
      
          switch (*s)
            {
            case '"':
              /* End of the string, return now */
              *dest = '\0';
              ++s;
              *end = s;
              return TRUE;

            case '\\':
              /* Possible escaped quote or \ */
              ++s;
              switch (*s)
                {
                case '"':
                case '\\':
                case '`':
                case '$':
                case '\n':
                  *dest = *s;
                  ++s;
                  ++dest;
                  break;

                default:
                  /* not an escaped char */
                  *dest = '\\';
                  ++dest;
                  /* ++s already done. */
                  break;
                }
              break;

            default:
              *dest = *s;
              ++dest;
              ++s;
              break;
            }

          _dbus_assert(s > dest); /* loop invariant */
        }
    }
  else
    {
      while (*s)
        {
          _dbus_assert(s > dest); /* loop invariant */
          
          if (*s == '\'')
            {
              /* End of the string, return now */
              *dest = '\0';
              ++s;
              *end = s;
              return TRUE;
            }
          else
            {
              *dest = *s;
              ++dest;
              ++s;
            }

          _dbus_assert(s > dest); /* loop invariant */
        }
    }
  
  /* If we reach here this means the close quote was never encountered */

  *dest = '\0';
  
  *end = s;
  return FALSE;
}

/** 
 * Unquotes a string as the shell (/bin/sh) would. Only handles
 * quotes; if a string contains file globs, arithmetic operators,
 * variables, backticks, redirections, or other special-to-the-shell
 * features, the result will be different from the result a real shell
 * would produce (the variables, backticks, etc. will be passed
 * through literally instead of being expanded). This function is
 * guaranteed to succeed if applied to the result of
 * _dbus_shell_quote(). If it fails, it returns %NULL.
 * The @p quoted_string need not actually contain quoted or
 * escaped text; _dbus_shell_unquote() simply goes through the string and
 * unquotes/unescapes anything that the shell would. Both single and
 * double quotes are handled, as are escapes including escaped
 * newlines. The return value must be freed with dbus_free().
 * 
 * Shell quoting rules are a bit strange. Single quotes preserve the
 * literal string exactly. escape sequences are not allowed; not even
 * \' - if you want a ' in the quoted text, you have to do something
 * like 'foo'\''bar'.  Double quotes allow $, `, ", \, and newline to
 * be escaped with backslash. Otherwise double quotes preserve things
 * literally.
 *
 * @param quoted_string shell-quoted string
 **/
char*
_dbus_shell_unquote (const char *quoted_string)
{
  char *unquoted;
  char *end;
  char *start;
  char *ret;
  DBusString retval;

  unquoted = _dbus_strdup (quoted_string);
  if (unquoted == NULL)
    return NULL;

  start = unquoted;
  end = unquoted;
  if (!_dbus_string_init (&retval))
    {
      dbus_free (unquoted);
      return NULL;
    }

  /* The loop allows cases such as
   * "foo"blah blah'bar'woo foo"baz"la la la\'\''foo'
   */
  while (*start)
    {
      /* Append all non-quoted chars, honoring backslash escape
       */
      
      while (*start && !(*start == '"' || *start == '\''))
        {
          if (*start == '\\')
            {
              /* all characters can get escaped by backslash,
               * except newline, which is removed if it follows
               * a backslash outside of quotes
               */
              
              ++start;
              if (*start)
                {
                  if (*start != '\n')
		    {
		      if (!_dbus_string_append_byte (&retval, *start))
			goto error;
		    }
                  ++start;
                }
            }
          else
            {
              if (!_dbus_string_append_byte (&retval, *start))
		goto error;
              ++start;
            }
        }

      if (*start)
        {
          if (!unquote_string_inplace (start, &end))
	    goto error;
          else
            {
              if (!_dbus_string_append (&retval, start))
		goto error;
              start = end;
            }
        }
    }

  ret = _dbus_strdup (_dbus_string_get_data (&retval));
  if (!ret)
    goto error;

  dbus_free (unquoted);
  _dbus_string_free (&retval);
  
  return ret;
  
 error:
  dbus_free (unquoted);
  _dbus_string_free (&retval);
  return NULL;
}

/* _dbus_shell_parse_argv() does a semi-arbitrary weird subset of the way
 * the shell parses a command line. We don't do variable expansion,
 * don't understand that operators are tokens, don't do tilde expansion,
 * don't do command substitution, no arithmetic expansion, IFS gets ignored,
 * don't do filename globs, don't remove redirection stuff, etc.
 *
 * READ THE UNIX98 SPEC on "Shell Command Language" before changing
 * the behavior of this code.
 *
 * Steps to parsing the argv string:
 *
 *  - tokenize the string (but since we ignore operators,
 *    our tokenization may diverge from what the shell would do)
 *    note that tokenization ignores the internals of a quoted
 *    word and it always splits on spaces, not on IFS even
 *    if we used IFS. We also ignore "end of input indicator"
 *    (I guess this is control-D?)
 *
 *    Tokenization steps, from UNIX98 with operator stuff removed,
 *    are:
 * 
 *    1) "If the current character is backslash, single-quote or
 *        double-quote (\, ' or ") and it is not quoted, it will affect
 *        quoting for subsequent characters up to the end of the quoted
 *        text. The rules for quoting are as described in Quoting
 *        . During token recognition no substitutions will be actually
 *        performed, and the result token will contain exactly the
 *        characters that appear in the input (except for newline
 *        character joining), unmodified, including any embedded or
 *        enclosing quotes or substitution operators, between the quote
 *        mark and the end of the quoted text. The token will not be
 *        delimited by the end of the quoted field."
 *
 *    2) "If the current character is an unquoted newline character,
 *        the current token will be delimited."
 *
 *    3) "If the current character is an unquoted blank character, any
 *        token containing the previous character is delimited and the
 *        current character will be discarded."
 *
 *    4) "If the previous character was part of a word, the current
 *        character will be appended to that word."
 *
 *    5) "If the current character is a "#", it and all subsequent
 *        characters up to, but excluding, the next newline character
 *        will be discarded as a comment. The newline character that
 *        ends the line is not considered part of the comment. The
 *        "#" starts a comment only when it is at the beginning of a
 *        token. Since the search for the end-of-comment does not
 *        consider an escaped newline character specially, a comment
 *        cannot be continued to the next line."
 *
 *    6) "The current character will be used as the start of a new word."
 *
 *
 *  - for each token (word), perform portions of word expansion, namely
 *    field splitting (using default whitespace IFS) and quote
 *    removal.  Field splitting may increase the number of words.
 *    Quote removal does not increase the number of words.
 *
 *   "If the complete expansion appropriate for a word results in an
 *   empty field, that empty field will be deleted from the list of
 *   fields that form the completely expanded command, unless the
 *   original word contained single-quote or double-quote characters."
 *    - UNIX98 spec
 *
 *
 */

static dbus_bool_t
delimit_token (DBusString *token,
               DBusList **retval,
	       DBusError *error)
{
  char *str;

  str = _dbus_strdup (_dbus_string_get_data (token));
  if (!str)
    {
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  if (!_dbus_list_append (retval, str))
    {
      dbus_free (str);
      _DBUS_SET_OOM (error);
      return FALSE;
    }

  return TRUE;
}

static DBusList*
tokenize_command_line (const char *command_line, DBusError *error)
{
  char current_quote;
  const char *p;
  DBusString current_token;
  DBusList *retval = NULL;
  dbus_bool_t quoted;;

  current_quote = '\0';
  quoted = FALSE;
  p = command_line;

  if (!_dbus_string_init (&current_token))
    {
      _DBUS_SET_OOM (error);
      return NULL;
    }

  while (*p)
    {
      if (current_quote == '\\')
        {
          if (*p == '\n')
            {
              /* we append nothing; backslash-newline become nothing */
            }
          else
            {
	      if (!_dbus_string_append_byte (&current_token, '\\') || 
	          !_dbus_string_append_byte (&current_token, *p))
		{
		  _DBUS_SET_OOM (error);
		  goto error;
		}
            }

          current_quote = '\0';
        }
      else if (current_quote == '#')
        {
          /* Discard up to and including next newline */
          while (*p && *p != '\n')
            ++p;

          current_quote = '\0';
          
          if (*p == '\0')
            break;
        }
      else if (current_quote)
        {
          if (*p == current_quote &&
              /* check that it isn't an escaped double quote */
              !(current_quote == '"' && quoted))
            {
              /* close the quote */
              current_quote = '\0';
            }

          /* Everything inside quotes, and the close quote,
           * gets appended literally.
           */

          if (!_dbus_string_append_byte (&current_token, *p))
	    {
	      _DBUS_SET_OOM (error);
	      goto error;
	    }
        }
      else
        {
          switch (*p)
            {
            case '\n':
              if (!delimit_token (&current_token, &retval, error))
                goto error;

              _dbus_string_free (&current_token);

              if (!_dbus_string_init (&current_token))
                {
                  _DBUS_SET_OOM (error);
                  goto init_error;
                }

              break;

            case ' ':
            case '\t':
              /* If the current token contains the previous char, delimit
               * the current token. A nonzero length
               * token should always contain the previous char.
               */
              if (_dbus_string_get_length (&current_token) > 0)
                {
                  if (!delimit_token (&current_token, &retval, error))
		    goto error;

		  _dbus_string_free (&current_token);

		  if (!_dbus_string_init (&current_token))
		    {
		      _DBUS_SET_OOM (error);
		      goto init_error;
		    }

                }
              
              /* discard all unquoted blanks (don't add them to a token) */
              break;


              /* single/double quotes are appended to the token,
               * escapes are maybe appended next time through the loop,
               * comment chars are never appended.
               */
              
            case '\'':
            case '"':
              if (!_dbus_string_append_byte (&current_token, *p))
		{
		  _DBUS_SET_OOM (error);
		  goto error;
		}

              /* FALL THRU */
              
            case '#':
            case '\\':
              current_quote = *p;
              break;

            default:
              /* Combines rules 4) and 6) - if we have a token, append to it,
               * otherwise create a new token.
               */
              if (!_dbus_string_append_byte (&current_token, *p))
		{
		  _DBUS_SET_OOM (error);
		  goto error;
		}
              break;
            }
        }

      /* We need to count consecutive backslashes mod 2, 
       * to detect escaped doublequotes.
       */
      if (*p != '\\')
	quoted = FALSE;
      else
	quoted = !quoted;

      ++p;
    }

  if (!delimit_token (&current_token, &retval, error))
    goto error;

  if (current_quote)
    {
      dbus_set_error_const (error, DBUS_ERROR_INVALID_ARGS, "Unclosed quotes in command line");
      goto error;
    }

  if (retval == NULL)
    {
      dbus_set_error_const (error, DBUS_ERROR_INVALID_ARGS, "No tokens found in command line");
      goto error;
    }
 
  _dbus_string_free (&current_token);
 
  return retval;

 error:
  _dbus_string_free (&current_token);

 init_error:
  _dbus_list_clear_full (&retval, dbus_free);
  return NULL;
}

/**
 * _dbus_shell_parse_argv:
 * 
 * Parses a command line into an argument vector, in much the same way
 * the shell would, but without many of the expansions the shell would
 * perform (variable expansion, globs, operators, filename expansion,
 * etc. are not supported). The results are defined to be the same as
 * those you would get from a UNIX98 /bin/sh, as long as the input
 * contains none of the unsupported shell expansions. If the input
 * does contain such expansions, they are passed through
 * literally. Free the returned vector with dbus_free_string_array().
 * 
 * @param command_line command line to parse
 * @param argcp return location for number of args
 * @param argvp return location for array of args
 * @param error error information
 **/
dbus_bool_t
_dbus_shell_parse_argv (const char *command_line,
			int        *argcp,
			char     ***argvp,
			DBusError  *error)
{
  /* Code based on poptParseArgvString() from libpopt */
  int argc = 0;
  char **argv = NULL;
  DBusList *tokens = NULL;
  int i;
  DBusList *tmp_list;
  
  if (!command_line)
    {
      _dbus_verbose ("Command line is NULL\n");
      return FALSE;
    }

  tokens = tokenize_command_line (command_line, error);
  if (tokens == NULL)
    {
      _dbus_verbose ("No tokens for command line '%s'\n", command_line);
      return FALSE;
    }

  /* Because we can't have introduced any new blank space into the
   * tokens (we didn't do any new expansions), we don't need to
   * perform field splitting. If we were going to honor IFS or do any
   * expansions, we would have to do field splitting on each word
   * here. Also, if we were going to do any expansion we would need to
   * remove any zero-length words that didn't contain quotes
   * originally; but since there's no expansion we know all words have
   * nonzero length, unless they contain quotes.
   * 
   * So, we simply remove quotes, and don't do any field splitting or
   * empty word removal, since we know there was no way to introduce
   * such things.
   */

  argc = _dbus_list_get_length (&tokens);
  argv = dbus_new (char *, argc + 1);
  if (!argv)
    {
      _DBUS_SET_OOM (error);
      goto error;
    }

  i = 0;
  tmp_list = tokens;
  while (tmp_list)
    {
      argv[i] = _dbus_shell_unquote (tmp_list->data);

      if (!argv[i])
        {
          int j;
	  for (j = 0; j < i; j++)
	    dbus_free(argv[j]);

          dbus_free (argv);
	  _DBUS_SET_OOM (error);
	  goto error;
        }

      tmp_list = _dbus_list_get_next_link (&tokens, tmp_list);
      ++i;
    }
  argv[argc] = NULL;

  _dbus_list_clear_full (&tokens, dbus_free);
  
  if (argcp)
    *argcp = argc;

  if (argvp)
    *argvp = argv;
  else
    dbus_free_string_array (argv);

  return TRUE;

 error:
  _dbus_list_clear_full (&tokens, dbus_free);

  return FALSE;

}
