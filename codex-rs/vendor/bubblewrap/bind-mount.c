/* bubblewrap
 * Copyright (C) 2016 Alexander Larsson
 * SPDX-License-Identifier: LGPL-2.0-or-later
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library. If not, see <http://www.gnu.org/licenses/>.
 *
 */

#include "config.h"

#include <sys/mount.h>

#include "utils.h"
#include "bind-mount.h"

static char *
skip_token (char *line, bool eat_whitespace)
{
  while (*line != ' ' && *line != '\n')
    line++;

  if (eat_whitespace && *line == ' ')
    line++;

  return line;
}

static char *
unescape_inline (char *escaped)
{
  char *unescaped, *res;
  const char *end;

  res = escaped;
  end = escaped + strlen (escaped);

  unescaped = escaped;
  while (escaped < end)
    {
      if (*escaped == '\\')
        {
          *unescaped++ =
            ((escaped[1] - '0') << 6) |
            ((escaped[2] - '0') << 3) |
            ((escaped[3] - '0') << 0);
          escaped += 4;
        }
      else
        {
          *unescaped++ = *escaped++;
        }
    }
  *unescaped = 0;
  return res;
}

static bool
match_token (const char *token, const char *token_end, const char *str)
{
  while (token != token_end && *token == *str)
    {
      token++;
      str++;
    }
  if (token == token_end)
    return *str == 0;

  return false;
}

static unsigned long
decode_mountoptions (const char *options)
{
  const char *token, *end_token;
  int i;
  unsigned long flags = 0;
  static const struct  { int   flag;
                         const char *name;
  } flags_data[] = {
    { 0, "rw" },
    { MS_RDONLY, "ro" },
    { MS_NOSUID, "nosuid" },
    { MS_NODEV, "nodev" },
    { MS_NOEXEC, "noexec" },
    { MS_NOATIME, "noatime" },
    { MS_NODIRATIME, "nodiratime" },
    { MS_RELATIME, "relatime" },
    { 0, NULL }
  };

  token = options;
  do
    {
      end_token = strchr (token, ',');
      if (end_token == NULL)
        end_token = token + strlen (token);

      for (i = 0; flags_data[i].name != NULL; i++)
        {
          if (match_token (token, end_token, flags_data[i].name))
            {
              flags |= flags_data[i].flag;
              break;
            }
        }

      if (*end_token != 0)
        token = end_token + 1;
      else
        token = NULL;
    }
  while (token != NULL);

  return flags;
}

typedef struct MountInfo MountInfo;
struct MountInfo {
  char *mountpoint;
  unsigned long options;
};

typedef MountInfo *MountTab;

static void
mount_tab_free (MountTab tab)
{
  int i;

  for (i = 0; tab[i].mountpoint != NULL; i++)
    free (tab[i].mountpoint);
  free (tab);
}

static inline void
cleanup_mount_tabp (void *p)
{
  void **pp = (void **) p;

  if (*pp)
    mount_tab_free ((MountTab)*pp);
}

#define cleanup_mount_tab __attribute__((cleanup (cleanup_mount_tabp)))

typedef struct MountInfoLine MountInfoLine;
struct MountInfoLine {
  const char *mountpoint;
  const char *options;
  bool covered;
  int id;
  int parent_id;
  MountInfoLine *first_child;
  MountInfoLine *next_sibling;
};

static unsigned int
count_lines (const char *data)
{
  unsigned int count = 0;
  const char *p = data;

  while (*p != 0)
    {
      if (*p == '\n')
        count++;
      p++;
    }

  /* If missing final newline, add one */
  if (p > data && *(p-1) != '\n')
    count++;

  return count;
}

static int
count_mounts (MountInfoLine *line)
{
  MountInfoLine *child;
  int res = 0;

  if (!line->covered)
    res += 1;

  child = line->first_child;
  while (child != NULL)
    {
      res += count_mounts (child);
      child = child->next_sibling;
    }

  return res;
}

static MountInfo *
collect_mounts (MountInfo *info, MountInfoLine *line)
{
  MountInfoLine *child;

  if (!line->covered)
    {
      info->mountpoint = xstrdup (line->mountpoint);
      info->options = decode_mountoptions (line->options);
      info ++;
    }

  child = line->first_child;
  while (child != NULL)
    {
      info = collect_mounts (info, child);
      child = child->next_sibling;
    }

  return info;
}

static MountTab
parse_mountinfo (int  proc_fd,
                 const char *root_mount)
{
  cleanup_free char *mountinfo = NULL;
  cleanup_free MountInfoLine *lines = NULL;
  cleanup_free MountInfoLine **by_id = NULL;
  cleanup_mount_tab MountTab mount_tab = NULL;
  MountInfo *end_tab;
  int n_mounts;
  char *line;
  unsigned int i;
  int max_id;
  unsigned int n_lines;
  int root;

  mountinfo = load_file_at (proc_fd, "self/mountinfo");
  if (mountinfo == NULL)
    die_with_error ("Can't open /proc/self/mountinfo");

  n_lines = count_lines (mountinfo);
  lines = xcalloc (n_lines, sizeof (MountInfoLine));

  max_id = 0;
  line = mountinfo;
  i = 0;
  root = -1;
  while (*line != 0)
    {
      int rc, consumed = 0;
      unsigned int maj, min;
      char *end;
      char *rest;
      char *mountpoint;
      char *mountpoint_end;
      char *options;
      char *options_end;
      char *next_line;

      assert (i < n_lines);

      end = strchr (line, '\n');
      if (end != NULL)
        {
          *end = 0;
          next_line = end + 1;
        }
      else
        next_line = line + strlen (line);

      rc = sscanf (line, "%d %d %u:%u %n", &lines[i].id, &lines[i].parent_id, &maj, &min, &consumed);
      if (rc != 4)
        die ("Can't parse mountinfo line");
      rest = line + consumed;

      rest = skip_token (rest, true); /* mountroot */
      mountpoint = rest;
      rest = skip_token (rest, false); /* mountpoint */
      mountpoint_end = rest++;
      options = rest;
      rest = skip_token (rest, false); /* vfs options */
      options_end = rest;

      *mountpoint_end = 0;
      lines[i].mountpoint = unescape_inline (mountpoint);

      *options_end = 0;
      lines[i].options = options;

      if (lines[i].id > max_id)
        max_id = lines[i].id;
      if (lines[i].parent_id > max_id)
        max_id = lines[i].parent_id;

      if (path_equal (lines[i].mountpoint, root_mount))
        root = i;

      i++;
      line = next_line;
    }
  assert (i == n_lines);

  if (root == -1)
    {
      mount_tab = xcalloc (1, sizeof (MountInfo));
      return steal_pointer (&mount_tab);
    }

  by_id = xcalloc (max_id + 1, sizeof (MountInfoLine*));
  for (i = 0; i < n_lines; i++)
    by_id[lines[i].id] = &lines[i];

  for (i = 0; i < n_lines; i++)
    {
      MountInfoLine *this = &lines[i];
      MountInfoLine *parent = by_id[this->parent_id];
      MountInfoLine **to_sibling;
      MountInfoLine *sibling;
      bool covered = false;

      if (!has_path_prefix (this->mountpoint, root_mount))
        continue;

      if (parent == NULL)
        continue;

      if (strcmp (parent->mountpoint, this->mountpoint) == 0)
        parent->covered = true;

      to_sibling = &parent->first_child;
      sibling = parent->first_child;
      while (sibling != NULL)
        {
          /* If this mountpoint is a path prefix of the sibling,
           * say this->mp=/foo/bar and sibling->mp=/foo, then it is
           * covered by the sibling, and we drop it. */
          if (has_path_prefix (this->mountpoint, sibling->mountpoint))
            {
              covered = true;
              break;
            }

          /* If the sibling is a path prefix of this mount point,
           * say this->mp=/foo and sibling->mp=/foo/bar, then the sibling
           * is covered, and we drop it.
            */
          if (has_path_prefix (sibling->mountpoint, this->mountpoint))
            *to_sibling = sibling->next_sibling;
          else
            to_sibling = &sibling->next_sibling;
          sibling = sibling->next_sibling;
        }

      if (covered)
          continue;

      *to_sibling = this;
    }

  n_mounts = count_mounts (&lines[root]);
  mount_tab = xcalloc (n_mounts + 1, sizeof (MountInfo));

  end_tab = collect_mounts (&mount_tab[0], &lines[root]);
  assert (end_tab == &mount_tab[n_mounts]);

  return steal_pointer (&mount_tab);
}

bind_mount_result
bind_mount (int           proc_fd,
            const char   *src,
            const char   *dest,
            bind_option_t options,
            char        **failing_path)
{
  bool readonly = (options & BIND_READONLY) != 0;
  bool devices = (options & BIND_DEVICES) != 0;
  bool recursive = (options & BIND_RECURSIVE) != 0;
  unsigned long current_flags, new_flags;
  cleanup_mount_tab MountTab mount_tab = NULL;
  cleanup_free char *resolved_dest = NULL;
  cleanup_free char *dest_proc = NULL;
  cleanup_free char *oldroot_dest_proc = NULL;
  cleanup_free char *kernel_case_combination = NULL;
  cleanup_fd int dest_fd = -1;
  int i;

  if (src)
    {
      if (mount (src, dest, NULL, MS_SILENT | MS_BIND | (recursive ? MS_REC : 0), NULL) != 0)
        return BIND_MOUNT_ERROR_MOUNT;
    }

  /* The mount operation will resolve any symlinks in the destination
     path, so to find it in the mount table we need to do that too. */
  resolved_dest = realpath (dest, NULL);
  if (resolved_dest == NULL)
    return BIND_MOUNT_ERROR_REALPATH_DEST;

  dest_fd = TEMP_FAILURE_RETRY (open (resolved_dest, O_PATH | O_CLOEXEC));
  if (dest_fd < 0)
    {
      if (failing_path != NULL)
        *failing_path = steal_pointer (&resolved_dest);

      return BIND_MOUNT_ERROR_REOPEN_DEST;
    }

  /* If we are in a case-insensitive filesystem, mountinfo might contain a
   * different case combination of the path we requested to mount.
   * This is due to the fact that the kernel, as of the beginning of 2021,
   * populates mountinfo with whatever case combination first appeared in the
   * dcache; kernel developers plan to change this in future so that it
   * reflects the on-disk encoding instead.
   * To avoid throwing an error when this happens, we use readlink() result
   * instead of the provided @root_mount, so that we can compare the mountinfo
   * entries with the same case combination that the kernel is expected to
   * use. */
  dest_proc = xasprintf ("/proc/self/fd/%d", dest_fd);
  oldroot_dest_proc = get_oldroot_path (dest_proc);
  kernel_case_combination = readlink_malloc (oldroot_dest_proc);
  if (kernel_case_combination == NULL)
    {
      if (failing_path != NULL)
        *failing_path = steal_pointer (&resolved_dest);

      return BIND_MOUNT_ERROR_READLINK_DEST_PROC_FD;
    }

  mount_tab = parse_mountinfo (proc_fd, kernel_case_combination);
  if (mount_tab[0].mountpoint == NULL)
    {
      if (failing_path != NULL)
        *failing_path = steal_pointer (&kernel_case_combination);

      errno = EINVAL;
      return BIND_MOUNT_ERROR_FIND_DEST_MOUNT;
    }

  assert (path_equal (mount_tab[0].mountpoint, kernel_case_combination));
  current_flags = mount_tab[0].options;
  new_flags = current_flags | (devices ? 0 : MS_NODEV) | MS_NOSUID | (readonly ? MS_RDONLY : 0);
  if (new_flags != current_flags &&
      mount ("none", resolved_dest,
             NULL, MS_SILENT | MS_BIND | MS_REMOUNT | new_flags, NULL) != 0)
    {
      if (failing_path != NULL)
        *failing_path = steal_pointer (&resolved_dest);

      return BIND_MOUNT_ERROR_REMOUNT_DEST;
    }

  /* We need to work around the fact that a bind mount does not apply the flags, so we need to manually
   * apply the flags to all submounts in the recursive case.
   * Note: This does not apply the flags to mounts which are later propagated into this namespace.
   */
  if (recursive)
    {
      for (i = 1; mount_tab[i].mountpoint != NULL; i++)
        {
          current_flags = mount_tab[i].options;
          new_flags = current_flags | (devices ? 0 : MS_NODEV) | MS_NOSUID | (readonly ? MS_RDONLY : 0);
          if (new_flags != current_flags &&
              mount ("none", mount_tab[i].mountpoint,
                     NULL, MS_SILENT | MS_BIND | MS_REMOUNT | new_flags, NULL) != 0)
            {
              /* If we can't read the mountpoint we can't remount it, but that should
                 be safe to ignore because its not something the user can access. */
              if (errno != EACCES)
                {
                  if (failing_path != NULL)
                    *failing_path = xstrdup (mount_tab[i].mountpoint);

                  return BIND_MOUNT_ERROR_REMOUNT_SUBMOUNT;
                }
            }
        }
    }

  return BIND_MOUNT_SUCCESS;
}

/**
 * Return a string representing bind_mount_result, like strerror().
 * If want_errno_p is non-NULL, *want_errno_p is used to indicate whether
 * it would make sense to print strerror(saved_errno).
 */
static char *
bind_mount_result_to_string (bind_mount_result res,
                             const char *failing_path,
                             bool *want_errno_p)
{
  char *string = NULL;
  bool want_errno = true;

  switch (res)
    {
      case BIND_MOUNT_ERROR_MOUNT:
        string = xstrdup ("Unable to mount source on destination");
        break;

      case BIND_MOUNT_ERROR_REALPATH_DEST:
        string = xstrdup ("realpath(destination)");
        break;

      case BIND_MOUNT_ERROR_REOPEN_DEST:
        string = xasprintf ("open(\"%s\", O_PATH)", failing_path);
        break;

      case BIND_MOUNT_ERROR_READLINK_DEST_PROC_FD:
        string = xasprintf ("readlink(/proc/self/fd/N) for \"%s\"", failing_path);
        break;

      case BIND_MOUNT_ERROR_FIND_DEST_MOUNT:
        string = xasprintf ("Unable to find \"%s\" in mount table", failing_path);
        want_errno = false;
        break;

      case BIND_MOUNT_ERROR_REMOUNT_DEST:
        string = xasprintf ("Unable to remount destination \"%s\" with correct flags",
                            failing_path);
        break;

      case BIND_MOUNT_ERROR_REMOUNT_SUBMOUNT:
        string = xasprintf ("Unable to apply mount flags: remount \"%s\"",
                            failing_path);
        break;

      case BIND_MOUNT_SUCCESS:
        string = xstrdup ("Success");
        break;

      default:
        string = xstrdup ("(unknown/invalid bind_mount_result)");
        break;
    }

  if (want_errno_p != NULL)
    *want_errno_p = want_errno;

  return string;
}

void
die_with_bind_result (bind_mount_result res,
                      int               saved_errno,
                      const char       *failing_path,
                      const char       *format,
                      ...)
{
  va_list args;
  bool want_errno = true;
  char *message;

  if (bwrap_level_prefix)
    fprintf (stderr, "<%d>", LOG_ERR);

  fprintf (stderr, "bwrap: ");

  va_start (args, format);
  vfprintf (stderr, format, args);
  va_end (args);

  message = bind_mount_result_to_string (res, failing_path, &want_errno);
  fprintf (stderr, ": %s", message);
  /* message is leaked, but we're exiting unsuccessfully anyway, so ignore */

  if (want_errno)
    {
      switch (res)
        {
          case BIND_MOUNT_ERROR_MOUNT:
          case BIND_MOUNT_ERROR_REMOUNT_DEST:
          case BIND_MOUNT_ERROR_REMOUNT_SUBMOUNT:
            fprintf (stderr, ": %s", mount_strerror (saved_errno));
            break;

          case BIND_MOUNT_ERROR_REALPATH_DEST:
          case BIND_MOUNT_ERROR_REOPEN_DEST:
          case BIND_MOUNT_ERROR_READLINK_DEST_PROC_FD:
          case BIND_MOUNT_ERROR_FIND_DEST_MOUNT:
          case BIND_MOUNT_SUCCESS:
          default:
            fprintf (stderr, ": %s", strerror (saved_errno));
        }
    }

  fprintf (stderr, "\n");
  exit (1);
}
