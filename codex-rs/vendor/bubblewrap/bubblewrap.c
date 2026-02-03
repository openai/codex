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

#include <poll.h>
#include <sched.h>
#include <pwd.h>
#include <grp.h>
#include <ctype.h>
#include <sys/mount.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <sys/eventfd.h>
#include <sys/fsuid.h>
#include <sys/signalfd.h>
#include <sys/capability.h>
#include <sys/prctl.h>
#include <linux/sched.h>
#include <linux/seccomp.h>
#include <linux/filter.h>

#include "utils.h"
#include "network.h"
#include "bind-mount.h"

#ifndef CLONE_NEWCGROUP
#define CLONE_NEWCGROUP 0x02000000 /* New cgroup namespace */
#endif

/* We limit the size of a tmpfs to half the architecture's address space,
 * to avoid hitting arbitrary limits in the kernel.
 * For example, on at least one x86_64 machine, the actual limit seems to be
 * 2^64 - 2^12. */
#define MAX_TMPFS_BYTES ((size_t) (SIZE_MAX >> 1))

/* Globals to avoid having to use getuid(), since the uid/gid changes during runtime */
static uid_t real_uid;
static gid_t real_gid;
static uid_t overflow_uid;
static gid_t overflow_gid;
static bool is_privileged; /* See acquire_privs() */
static const char *argv0;
static const char *host_tty_dev;
static int proc_fd = -1;
static const char *opt_exec_label = NULL;
static const char *opt_file_label = NULL;
static bool opt_as_pid_1;

static const char *opt_argv0 = NULL;
static const char *opt_chdir_path = NULL;
static bool opt_assert_userns_disabled = false;
static bool opt_disable_userns = false;
static bool opt_unshare_user = false;
static bool opt_unshare_user_try = false;
static bool opt_unshare_pid = false;
static bool opt_unshare_ipc = false;
static bool opt_unshare_net = false;
static bool opt_unshare_uts = false;
static bool opt_unshare_cgroup = false;
static bool opt_unshare_cgroup_try = false;
static bool opt_needs_devpts = false;
static bool opt_new_session = false;
static bool opt_die_with_parent = false;
static uid_t opt_sandbox_uid = -1;
static gid_t opt_sandbox_gid = -1;
static int opt_sync_fd = -1;
static int opt_block_fd = -1;
static int opt_userns_block_fd = -1;
static int opt_info_fd = -1;
static int opt_json_status_fd = -1;
static int opt_seccomp_fd = -1;
static const char *opt_sandbox_hostname = NULL;
static char *opt_args_data = NULL;  /* owned */
static int opt_userns_fd = -1;
static int opt_userns2_fd = -1;
static int opt_pidns_fd = -1;
static int opt_tmp_overlay_count = 0;
static int next_perms = -1;
static size_t next_size_arg = 0;
static int next_overlay_src_count = 0;

#define CAP_TO_MASK_0(x) (1L << ((x) & 31))
#define CAP_TO_MASK_1(x) CAP_TO_MASK_0(x - 32)

typedef struct _NsInfo NsInfo;

struct _NsInfo {
  const char *name;
  bool       *do_unshare;
  ino_t       id;
};

static NsInfo ns_infos[] = {
  {"cgroup", &opt_unshare_cgroup, 0},
  {"ipc",    &opt_unshare_ipc,    0},
  {"mnt",    NULL,                0},
  {"net",    &opt_unshare_net,    0},
  {"pid",    &opt_unshare_pid,    0},
  /* user namespace info omitted because it
   * is not (yet) valid when we obtain the
   * namespace info (get un-shared later) */
  {"uts",    &opt_unshare_uts,    0},
  {NULL,     NULL,                0}
};

typedef enum {
  SETUP_BIND_MOUNT,
  SETUP_RO_BIND_MOUNT,
  SETUP_DEV_BIND_MOUNT,
  SETUP_OVERLAY_MOUNT,
  SETUP_TMP_OVERLAY_MOUNT,
  SETUP_RO_OVERLAY_MOUNT,
  SETUP_OVERLAY_SRC,
  SETUP_MOUNT_PROC,
  SETUP_MOUNT_DEV,
  SETUP_MOUNT_TMPFS,
  SETUP_MOUNT_MQUEUE,
  SETUP_MAKE_DIR,
  SETUP_MAKE_FILE,
  SETUP_MAKE_BIND_FILE,
  SETUP_MAKE_RO_BIND_FILE,
  SETUP_MAKE_SYMLINK,
  SETUP_REMOUNT_RO_NO_RECURSIVE,
  SETUP_SET_HOSTNAME,
  SETUP_CHMOD,
} SetupOpType;

typedef enum {
  NO_CREATE_DEST = (1 << 0),
  ALLOW_NOTEXIST = (1 << 1),
} SetupOpFlag;

typedef struct _SetupOp SetupOp;

struct _SetupOp
{
  SetupOpType type;
  const char *source;
  const char *dest;
  int         fd;
  SetupOpFlag flags;
  int         perms;
  size_t      size;  /* number of bytes, zero means unset/default */
  SetupOp    *next;
};

typedef struct _LockFile LockFile;

struct _LockFile
{
  const char *path;
  int         fd;
  LockFile   *next;
};

enum {
  PRIV_SEP_OP_DONE,
  PRIV_SEP_OP_BIND_MOUNT,
  PRIV_SEP_OP_OVERLAY_MOUNT,
  PRIV_SEP_OP_PROC_MOUNT,
  PRIV_SEP_OP_TMPFS_MOUNT,
  PRIV_SEP_OP_DEVPTS_MOUNT,
  PRIV_SEP_OP_MQUEUE_MOUNT,
  PRIV_SEP_OP_REMOUNT_RO_NO_RECURSIVE,
  PRIV_SEP_OP_SET_HOSTNAME,
};

typedef struct
{
  uint32_t op;
  uint32_t flags;
  uint32_t perms;
  size_t   size_arg;
  uint32_t arg1_offset;
  uint32_t arg2_offset;
} PrivSepOp;

/*
 * DEFINE_LINKED_LIST:
 * @Type: A struct with a `Type *next` member
 * @name: Used to form the names of variables and functions
 *
 * Define a global linked list of @Type structures, with pointers
 * `NAMEs` to the head of the list and `last_NAME` to the tail of the
 * list.
 *
 * A new zero-filled item can be allocated and appended to the list
 * by calling `_NAME_append_new()`, which returns the new item.
 */
#define DEFINE_LINKED_LIST(Type, name) \
static Type *name ## s = NULL; \
static Type *last_ ## name = NULL; \
\
static inline Type * \
_ ## name ## _append_new (void) \
{ \
  Type *self = xcalloc (1, sizeof (Type)); \
\
  if (last_ ## name != NULL) \
    last_ ## name ->next = self; \
  else \
    name ## s = self; \
\
  last_ ## name = self; \
  return self; \
}

DEFINE_LINKED_LIST (SetupOp, op)

static SetupOp *
setup_op_new (SetupOpType type)
{
  SetupOp *op = _op_append_new ();

  op->type = type;
  op->fd = -1;
  op->flags = 0;
  return op;
}

DEFINE_LINKED_LIST (LockFile, lock_file)

static LockFile *
lock_file_new (const char *path)
{
  LockFile *lock = _lock_file_append_new ();

  lock->path = path;
  return lock;
}

typedef struct _SeccompProgram SeccompProgram;

struct _SeccompProgram
{
  struct sock_fprog  program;
  SeccompProgram    *next;
};

DEFINE_LINKED_LIST (SeccompProgram, seccomp_program)

static SeccompProgram *
seccomp_program_new (int *fd)
{
  SeccompProgram *self = _seccomp_program_append_new ();
  cleanup_free char *data = NULL;
  size_t len;

  data = load_file_data (*fd, &len);

  if (data == NULL)
    die_with_error ("Can't read seccomp data");

  close (*fd);
  *fd = -1;

  if (len % 8 != 0)
    die ("Invalid seccomp data, must be multiple of 8");

  self->program.len = len / 8;
  self->program.filter = (struct sock_filter *) steal_pointer (&data);
  return self;
}

static void
seccomp_programs_apply (void)
{
  SeccompProgram *program;

  for (program = seccomp_programs; program != NULL; program = program->next)
    {
      if (prctl (PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program->program) != 0)
        {
          if (errno == EINVAL)
            die ("Unable to set up system call filtering as requested: "
                 "prctl(PR_SET_SECCOMP) reported EINVAL. "
                 "(Hint: this requires a kernel configured with "
                 "CONFIG_SECCOMP and CONFIG_SECCOMP_FILTER.)");

          die_with_error ("prctl(PR_SET_SECCOMP)");
        }
    }
}

static void
usage (int ecode, FILE *out)
{
  fprintf (out, "usage: %s [OPTIONS...] [--] COMMAND [ARGS...]\n\n", argv0 ? argv0 : "bwrap");

  fprintf (out,
           "    --help                       Print this help\n"
           "    --version                    Print version\n"
           "    --args FD                    Parse NUL-separated args from FD\n"
           "    --argv0 VALUE                Set argv[0] to the value VALUE before running the program\n"
           "    --level-prefix               Prepend e.g. <3> to diagnostic messages\n"
           "    --unshare-all                Unshare every namespace we support by default\n"
           "    --share-net                  Retain the network namespace (can only combine with --unshare-all)\n"
           "    --unshare-user               Create new user namespace (may be automatically implied if not setuid)\n"
           "    --unshare-user-try           Create new user namespace if possible else continue by skipping it\n"
           "    --unshare-ipc                Create new ipc namespace\n"
           "    --unshare-pid                Create new pid namespace\n"
           "    --unshare-net                Create new network namespace\n"
           "    --unshare-uts                Create new uts namespace\n"
           "    --unshare-cgroup             Create new cgroup namespace\n"
           "    --unshare-cgroup-try         Create new cgroup namespace if possible else continue by skipping it\n"
           "    --userns FD                  Use this user namespace (cannot combine with --unshare-user)\n"
           "    --userns2 FD                 After setup switch to this user namespace, only useful with --userns\n"
           "    --disable-userns             Disable further use of user namespaces inside sandbox\n"
           "    --assert-userns-disabled     Fail unless further use of user namespace inside sandbox is disabled\n"
           "    --pidns FD                   Use this pid namespace (as parent namespace if using --unshare-pid)\n"
           "    --uid UID                    Custom uid in the sandbox (requires --unshare-user or --userns)\n"
           "    --gid GID                    Custom gid in the sandbox (requires --unshare-user or --userns)\n"
           "    --hostname NAME              Custom hostname in the sandbox (requires --unshare-uts)\n"
           "    --chdir DIR                  Change directory to DIR\n"
           "    --clearenv                   Unset all environment variables\n"
           "    --setenv VAR VALUE           Set an environment variable\n"
           "    --unsetenv VAR               Unset an environment variable\n"
           "    --lock-file DEST             Take a lock on DEST while sandbox is running\n"
           "    --sync-fd FD                 Keep this fd open while sandbox is running\n"
           "    --bind SRC DEST              Bind mount the host path SRC on DEST\n"
           "    --bind-try SRC DEST          Equal to --bind but ignores non-existent SRC\n"
           "    --dev-bind SRC DEST          Bind mount the host path SRC on DEST, allowing device access\n"
           "    --dev-bind-try SRC DEST      Equal to --dev-bind but ignores non-existent SRC\n"
           "    --ro-bind SRC DEST           Bind mount the host path SRC readonly on DEST\n"
           "    --ro-bind-try SRC DEST       Equal to --ro-bind but ignores non-existent SRC\n"
           "    --bind-fd FD DEST            Bind open directory or path fd on DEST\n"
           "    --ro-bind-fd FD DEST         Bind open directory or path fd read-only on DEST\n"
           "    --remount-ro DEST            Remount DEST as readonly; does not recursively remount\n"
           "    --overlay-src SRC            Read files from SRC in the following overlay\n"
           "    --overlay RWSRC WORKDIR DEST Mount overlayfs on DEST, with RWSRC as the host path for writes and\n"
           "                                 WORKDIR an empty directory on the same filesystem as RWSRC\n"
           "    --tmp-overlay DEST           Mount overlayfs on DEST, with writes going to an invisible tmpfs\n"
           "    --ro-overlay DEST            Mount overlayfs read-only on DEST\n"
           "    --exec-label LABEL           Exec label for the sandbox\n"
           "    --file-label LABEL           File label for temporary sandbox content\n"
           "    --proc DEST                  Mount new procfs on DEST\n"
           "    --dev DEST                   Mount new dev on DEST\n"
           "    --tmpfs DEST                 Mount new tmpfs on DEST\n"
           "    --mqueue DEST                Mount new mqueue on DEST\n"
           "    --dir DEST                   Create dir at DEST\n"
           "    --file FD DEST               Copy from FD to destination DEST\n"
           "    --bind-data FD DEST          Copy from FD to file which is bind-mounted on DEST\n"
           "    --ro-bind-data FD DEST       Copy from FD to file which is readonly bind-mounted on DEST\n"
           "    --symlink SRC DEST           Create symlink at DEST with target SRC\n"
           "    --seccomp FD                 Load and use seccomp rules from FD (not repeatable)\n"
           "    --add-seccomp-fd FD          Load and use seccomp rules from FD (repeatable)\n"
           "    --block-fd FD                Block on FD until some data to read is available\n"
           "    --userns-block-fd FD         Block on FD until the user namespace is ready\n"
           "    --info-fd FD                 Write information about the running container to FD\n"
           "    --json-status-fd FD          Write container status to FD as multiple JSON documents\n"
           "    --new-session                Create a new terminal session\n"
           "    --die-with-parent            Kills with SIGKILL child process (COMMAND) when bwrap or bwrap's parent dies.\n"
           "    --as-pid-1                   Do not install a reaper process with PID=1\n"
           "    --cap-add CAP                Add cap CAP when running as privileged user\n"
           "    --cap-drop CAP               Drop cap CAP when running as privileged user\n"
           "    --perms OCTAL                Set permissions of next argument (--bind-data, --file, etc.)\n"
           "    --size BYTES                 Set size of next argument (only for --tmpfs)\n"
           "    --chmod OCTAL PATH           Change permissions of PATH (must already exist)\n"
          );
  exit (ecode);
}

/* If --die-with-parent was specified, use PDEATHSIG to ensure SIGKILL
 * is sent to the current process when our parent dies.
 */
static void
handle_die_with_parent (void)
{
  if (opt_die_with_parent && prctl (PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0)
    die_with_error ("prctl");
}

static void
block_sigchild (void)
{
  sigset_t mask;
  int status;

  sigemptyset (&mask);
  sigaddset (&mask, SIGCHLD);

  if (sigprocmask (SIG_BLOCK, &mask, NULL) == -1)
    die_with_error ("sigprocmask");

  /* Reap any outstanding zombies that we may have inherited */
  while (waitpid (-1, &status, WNOHANG) > 0)
    ;
}

static void
unblock_sigchild (void)
{
  sigset_t mask;

  sigemptyset (&mask);
  sigaddset (&mask, SIGCHLD);

  if (sigprocmask (SIG_UNBLOCK, &mask, NULL) == -1)
    die_with_error ("sigprocmask");
}

/* Closes all fd:s except 0,1,2 and the passed in array of extra fds */
static int
close_extra_fds (void *data, int fd)
{
  int *extra_fds = (int *) data;
  int i;

  for (i = 0; extra_fds[i] != -1; i++)
    if (fd == extra_fds[i])
      return 0;

  if (fd <= 2)
    return 0;

  close (fd);
  return 0;
}

static int
propagate_exit_status (int status)
{
  if (WIFEXITED (status))
    return WEXITSTATUS (status);

  /* The process died of a signal, we can't really report that, but we
   * can at least be bash-compatible. The bash manpage says:
   *   The return value of a simple command is its
   *   exit status, or 128+n if the command is
   *   terminated by signal n.
   */
  if (WIFSIGNALED (status))
    return 128 + WTERMSIG (status);

  /* Weird? */
  return 255;
}

static void
dump_info (int fd, const char *output, bool exit_on_error)
{
  size_t len = strlen (output);
  if (write_to_fd (fd, output, len))
    {
      if (exit_on_error)
        die_with_error ("Write to info_fd");
    }
}

static void
report_child_exit_status (int exitc, int setup_finished_fd)
{
  ssize_t s;
  char data[2];
  cleanup_free char *output = NULL;
  if (opt_json_status_fd == -1 || setup_finished_fd == -1)
    return;

  s = TEMP_FAILURE_RETRY (read (setup_finished_fd, data, sizeof data));
  if (s == -1 && errno != EAGAIN)
    die_with_error ("read eventfd");
  if (s != 1) // Is 0 if pipe closed before exec, is 2 if closed after exec.
    return;

  output = xasprintf ("{ \"exit-code\": %i }\n", exitc);
  dump_info (opt_json_status_fd, output, false);
  close (opt_json_status_fd);
  opt_json_status_fd = -1;
  close (setup_finished_fd);
}

/* This stays around for as long as the initial process in the app does
 * and when that exits it exits, propagating the exit status. We do this
 * by having pid 1 in the sandbox detect this exit and tell the monitor
 * the exit status via a eventfd. We also track the exit of the sandbox
 * pid 1 via a signalfd for SIGCHLD, and exit with an error in this case.
 * This is to catch e.g. problems during setup. */
static int
monitor_child (int event_fd, pid_t child_pid, int setup_finished_fd)
{
  int res;
  uint64_t val;
  ssize_t s;
  int signal_fd;
  sigset_t mask;
  struct pollfd fds[2];
  int num_fds;
  struct signalfd_siginfo fdsi;
  int dont_close[] = {-1, -1, -1, -1};
  unsigned int j = 0;
  int exitc;
  pid_t died_pid;
  int died_status;

  /* Close all extra fds in the monitoring process.
     Any passed in fds have been passed on to the child anyway. */
  if (event_fd != -1)
    dont_close[j++] = event_fd;
  if (opt_json_status_fd != -1)
    dont_close[j++] = opt_json_status_fd;
  if (setup_finished_fd != -1)
    dont_close[j++] = setup_finished_fd;
  assert (j < sizeof(dont_close)/sizeof(*dont_close));
  fdwalk (proc_fd, close_extra_fds, dont_close);

  sigemptyset (&mask);
  sigaddset (&mask, SIGCHLD);

  signal_fd = signalfd (-1, &mask, SFD_CLOEXEC | SFD_NONBLOCK);
  if (signal_fd == -1)
    die_with_error ("Can't create signalfd");

  num_fds = 1;
  fds[0].fd = signal_fd;
  fds[0].events = POLLIN;
  if (event_fd != -1)
    {
      fds[1].fd = event_fd;
      fds[1].events = POLLIN;
      num_fds++;
    }

  while (1)
    {
      fds[0].revents = fds[1].revents = 0;
      res = poll (fds, num_fds, -1);
      if (res == -1 && errno != EINTR)
        die_with_error ("poll");

      /* Always read from the eventfd first, if pid 2 died then pid 1 often
       * dies too, and we could race, reporting that first and we'd lose
       * the real exit status. */
      if (event_fd != -1)
        {
          s = read (event_fd, &val, 8);
          if (s == -1 && errno != EINTR && errno != EAGAIN)
            die_with_error ("read eventfd");
          else if (s == 8)
            {
              exitc = (int) val - 1;
              report_child_exit_status (exitc, setup_finished_fd);
              return exitc;
            }
        }

      /* We need to read the signal_fd, or it will keep polling as read,
       * however we ignore the details as we get them from waitpid
       * below anyway */
      s = read (signal_fd, &fdsi, sizeof (struct signalfd_siginfo));
      if (s == -1 && errno != EINTR && errno != EAGAIN)
        die_with_error ("read signalfd");

      /* We may actually get several sigchld compressed into one
         SIGCHLD, so we have to handle all of them. */
      while ((died_pid = waitpid (-1, &died_status, WNOHANG)) > 0)
        {
          /* We may be getting sigchild from other children too. For instance if
             someone created a child process, and then exec:ed bubblewrap. Ignore them */
          if (died_pid == child_pid)
            {
              exitc = propagate_exit_status (died_status);
              report_child_exit_status (exitc, setup_finished_fd);
              return exitc;
            }
        }
    }

  die ("Should not be reached");

  return 0;
}

/* This is pid 1 in the app sandbox. It is needed because we're using
 * pid namespaces, and someone has to reap zombies in it. We also detect
 * when the initial process (pid 2) dies and report its exit status to
 * the monitor so that it can return it to the original spawner.
 *
 * When there are no other processes in the sandbox the wait will return
 * ECHILD, and we then exit pid 1 to clean up the sandbox. */
static int
do_init (int event_fd, pid_t initial_pid)
{
  int initial_exit_status = 1;
  LockFile *lock;

  for (lock = lock_files; lock != NULL; lock = lock->next)
    {
      int fd = TEMP_FAILURE_RETRY (open (lock->path, O_RDONLY | O_CLOEXEC));
      if (fd == -1)
        die_with_error ("Unable to open lock file %s", lock->path);

      struct flock l = {
        .l_type = F_RDLCK,
        .l_whence = SEEK_SET,
        .l_start = 0,
        .l_len = 0
      };

      if (TEMP_FAILURE_RETRY (fcntl (fd, F_SETLK, &l)) < 0)
        die_with_error ("Unable to lock file %s", lock->path);

      /* Keep fd open to hang on to lock */
      lock->fd = fd;
    }

  /* Optionally bind our lifecycle to that of the caller */
  handle_die_with_parent ();

  seccomp_programs_apply ();

  while (true)
    {
      pid_t child;
      int status;

      child = TEMP_FAILURE_RETRY (wait (&status));
      if (child == initial_pid)
        {
          initial_exit_status = propagate_exit_status (status);

          if(event_fd != -1)
            {
              uint64_t val;
              int res UNUSED;

              val = initial_exit_status + 1;
              res = TEMP_FAILURE_RETRY (write (event_fd, &val, 8));
              /* Ignore res, if e.g. the parent died and closed event_fd
                 we don't want to error out here */
            }
        }

      if (child == -1 && errno != EINTR)
        {
          if (errno != ECHILD)
            die_with_error ("init wait()");
          break;
        }
    }

  /* Close FDs. */
  for (lock = lock_files; lock != NULL; lock = lock->next)
    {
      if (lock->fd >= 0)
        {
          close (lock->fd);
          lock->fd = -1;
        }
    }

  return initial_exit_status;
}

#define CAP_TO_MASK_0(x) (1L << ((x) & 31))
#define CAP_TO_MASK_1(x) CAP_TO_MASK_0(x - 32)

/* Set if --cap-add or --cap-drop were used */
static bool opt_cap_add_or_drop_used;
/* The capability set we'll target, used if above is true */
static uint32_t requested_caps[2] = {0, 0};

/* low 32bit caps needed */
/* CAP_SYS_PTRACE is needed to dereference the symlinks in /proc/<pid>/ns/, see namespaces(7) */
#define REQUIRED_CAPS_0 (CAP_TO_MASK_0 (CAP_SYS_ADMIN) | CAP_TO_MASK_0 (CAP_SYS_CHROOT) | CAP_TO_MASK_0 (CAP_NET_ADMIN) | CAP_TO_MASK_0 (CAP_SETUID) | CAP_TO_MASK_0 (CAP_SETGID) | CAP_TO_MASK_0 (CAP_SYS_PTRACE))
/* high 32bit caps needed */
#define REQUIRED_CAPS_1 0

static void
set_required_caps (void)
{
  struct __user_cap_header_struct hdr = { _LINUX_CAPABILITY_VERSION_3, 0 };
  struct __user_cap_data_struct data[2] = { { 0 } };

  /* Drop all non-require capabilities */
  data[0].effective = REQUIRED_CAPS_0;
  data[0].permitted = REQUIRED_CAPS_0;
  data[0].inheritable = 0;
  data[1].effective = REQUIRED_CAPS_1;
  data[1].permitted = REQUIRED_CAPS_1;
  data[1].inheritable = 0;
  if (capset (&hdr, data) < 0)
    die_with_error ("capset failed");
}

static void
drop_all_caps (bool keep_requested_caps)
{
  struct __user_cap_header_struct hdr = { _LINUX_CAPABILITY_VERSION_3, 0 };
  struct __user_cap_data_struct data[2] = { { 0 } };

  if (keep_requested_caps)
    {
      /* Avoid calling capset() unless we need to; currently
       * systemd-nspawn at least is known to install a seccomp
       * policy denying capset() for dubious reasons.
       * <https://github.com/projectatomic/bubblewrap/pull/122>
       */
      if (!opt_cap_add_or_drop_used && real_uid == 0)
        {
          assert (!is_privileged);
          return;
        }
      data[0].effective = requested_caps[0];
      data[0].permitted = requested_caps[0];
      data[0].inheritable = requested_caps[0];
      data[1].effective = requested_caps[1];
      data[1].permitted = requested_caps[1];
      data[1].inheritable = requested_caps[1];
    }

  if (capset (&hdr, data) < 0)
    {
      /* While the above logic ensures we don't call capset() for the primary
       * process unless configured to do so, we still try to drop privileges for
       * the init process unconditionally. Since due to the systemd seccomp
       * filter that will fail, let's just ignore it.
       */
      if (errno == EPERM && real_uid == 0 && !is_privileged)
        return;
      else
        die_with_error ("capset failed");
    }
}

static bool
has_caps (void)
{
  struct __user_cap_header_struct hdr = { _LINUX_CAPABILITY_VERSION_3, 0 };
  struct __user_cap_data_struct data[2] = { { 0 } };

  if (capget (&hdr, data)  < 0)
    die_with_error ("capget failed");

  return data[0].permitted != 0 || data[1].permitted != 0;
}

/* Most of the code here is used both to add caps to the ambient capabilities
 * and drop caps from the bounding set.  Handle both cases here and add
 * drop_cap_bounding_set/set_ambient_capabilities wrappers to facilitate its usage.
 */
static void
prctl_caps (uint32_t *caps, bool do_cap_bounding, bool do_set_ambient)
{
  unsigned long cap;

  /* We ignore both EINVAL and EPERM, as we are actually relying
   * on PR_SET_NO_NEW_PRIVS to ensure the right capabilities are
   * available.  EPERM in particular can happen with old, buggy
   * kernels.  See:
   *  https://github.com/projectatomic/bubblewrap/pull/175#issuecomment-278051373
   *  https://git.kernel.org/cgit/linux/kernel/git/torvalds/linux.git/commit/security/commoncap.c?id=160da84dbb39443fdade7151bc63a88f8e953077
   */
  for (cap = 0; cap <= CAP_LAST_CAP; cap++)
    {
      bool keep = false;
      if (cap < 32)
        {
          if (CAP_TO_MASK_0 (cap) & caps[0])
            keep = true;
        }
      else
        {
          if (CAP_TO_MASK_1 (cap) & caps[1])
            keep = true;
        }

      if (keep && do_set_ambient)
        {
#ifdef PR_CAP_AMBIENT
          int res = prctl (PR_CAP_AMBIENT, PR_CAP_AMBIENT_RAISE, cap, 0, 0);
          if (res == -1 && !(errno == EINVAL || errno == EPERM))
            die_with_error ("Adding ambient capability %ld", cap);
#else
          /* We ignore the EINVAL that results from not having PR_CAP_AMBIENT
           * in the current kernel at runtime, so also ignore not having it
           * in the current kernel headers at compile-time */
#endif
        }

      if (!keep && do_cap_bounding)
        {
          int res = prctl (PR_CAPBSET_DROP, cap, 0, 0, 0);
          if (res == -1 && !(errno == EINVAL || errno == EPERM))
            die_with_error ("Dropping capability %ld from bounds", cap);
        }
    }
}

static void
drop_cap_bounding_set (bool drop_all)
{
  if (!drop_all)
    prctl_caps (requested_caps, true, false);
  else
    {
      uint32_t no_caps[2] = {0, 0};
      prctl_caps (no_caps, true, false);
    }
}

static void
set_ambient_capabilities (void)
{
  if (is_privileged)
    return;
  prctl_caps (requested_caps, false, true);
}

/* This acquires the privileges that the bwrap will need it to work.
 * If bwrap is not setuid, then this does nothing, and it relies on
 * unprivileged user namespaces to be used. This case is
 * "is_privileged = false".
 *
 * If bwrap is setuid, then we do things in phases.
 * The first part is run as euid 0, but with fsuid as the real user.
 * The second part, inside the child, is run as the real user but with
 * capabilities.
 * And finally we drop all capabilities.
 * The reason for the above dance is to avoid having the setup phase
 * being able to read files the user can't, while at the same time
 * working around various kernel issues. See below for details.
 */
static void
acquire_privs (void)
{
  uid_t euid, new_fsuid;

  euid = geteuid ();

  /* Are we setuid ? */
  if (real_uid != euid)
    {
      if (euid != 0)
        die ("Unexpected setuid user %d, should be 0", euid);

      is_privileged = true;
      /* We want to keep running as euid=0 until at the clone()
       * operation because doing so will make the user namespace be
       * owned by root, which makes it not ptrace:able by the user as
       * it otherwise would be. After that we will run fully as the
       * user, which is necessary e.g. to be able to read from a fuse
       * mount from the user.
       *
       * However, we don't want to accidentally mis-use euid=0 for
       * escalated filesystem access before the clone(), so we set
       * fsuid to the uid.
       */
      if (setfsuid (real_uid) < 0)
        die_with_error ("Unable to set fsuid");

      /* setfsuid can't properly report errors, check that it worked (as per manpage) */
      new_fsuid = setfsuid (-1);
      if (new_fsuid != real_uid)
        die ("Unable to set fsuid (was %d)", (int)new_fsuid);

      /* We never need capabilities after execve(), so lets drop everything from the bounding set */
      drop_cap_bounding_set (true);

      /* Keep only the required capabilities for setup */
      set_required_caps ();
    }
  else if (real_uid != 0 && has_caps ())
    {
      /* We have some capabilities in the non-setuid case, which should not happen.
         Probably caused by the binary being setcap instead of setuid which we
         don't support anymore */
      die ("Unexpected capabilities but not setuid, old file caps config?");
    }
  else if (real_uid == 0)
    {
      /* If our uid is 0, default to inheriting all caps; the caller
       * can drop them via --cap-drop.  This is used by at least rpm-ostree.
       * Note this needs to happen before the argument parsing of --cap-drop.
       */
      struct __user_cap_header_struct hdr = { _LINUX_CAPABILITY_VERSION_3, 0 };
      struct __user_cap_data_struct data[2] = { { 0 } };

      if (capget (&hdr, data) < 0)
        die_with_error ("capget (for uid == 0) failed");

      requested_caps[0] = data[0].effective;
      requested_caps[1] = data[1].effective;
    }

  /* Else, we try unprivileged user namespaces */
}

/* This is called once we're inside the namespace */
static void
switch_to_user_with_privs (void)
{
  /* If we're in a new user namespace, we got back the bounding set, clear it again */
  if (opt_unshare_user || opt_userns_fd != -1)
    drop_cap_bounding_set (false);

  /* If we switched to a new user namespace it may allow other uids/gids, so switch to the target one */
  if (opt_userns_fd != -1)
    {
      if (opt_sandbox_uid != real_uid && setuid (opt_sandbox_uid) < 0)
        die_with_error ("unable to switch to uid %d", opt_sandbox_uid);

      if (opt_sandbox_gid != real_gid && setgid (opt_sandbox_gid) < 0)
        die_with_error ("unable to switch to gid %d", opt_sandbox_gid);
    }

  if (!is_privileged)
    return;

  /* Tell kernel not clear capabilities when later dropping root uid */
  if (prctl (PR_SET_KEEPCAPS, 1, 0, 0, 0) < 0)
    die_with_error ("prctl(PR_SET_KEEPCAPS) failed");

  if (setuid (opt_sandbox_uid) < 0)
    die_with_error ("unable to drop root uid");

  /* Regain effective required capabilities from permitted */
  set_required_caps ();
}

/* Call setuid() and use capset() to adjust capabilities */
static void
drop_privs (bool keep_requested_caps,
            bool already_changed_uid)
{
  assert (!keep_requested_caps || !is_privileged);
  /* Drop root uid */
  if (is_privileged && !already_changed_uid &&
      setuid (opt_sandbox_uid) < 0)
    die_with_error ("unable to drop root uid");

  drop_all_caps (keep_requested_caps);

  /* We don't have any privs now, so mark us dumpable which makes /proc/self be owned by the user instead of root */
  if (prctl (PR_SET_DUMPABLE, 1, 0, 0, 0) != 0)
    die_with_error ("can't set dumpable");
}

static void
write_uid_gid_map (uid_t sandbox_uid,
                   uid_t parent_uid,
                   uid_t sandbox_gid,
                   uid_t parent_gid,
                   pid_t pid,
                   bool  deny_groups,
                   bool  map_root)
{
  cleanup_free char *uid_map = NULL;
  cleanup_free char *gid_map = NULL;
  cleanup_free char *dir = NULL;
  cleanup_fd int dir_fd = -1;
  uid_t old_fsuid = (uid_t)-1;

  if (pid == -1)
    dir = xstrdup ("self");
  else
    dir = xasprintf ("%d", pid);

  dir_fd = openat (proc_fd, dir, O_PATH);
  if (dir_fd < 0)
    die_with_error ("open /proc/%s failed", dir);

  if (map_root && parent_uid != 0 && sandbox_uid != 0)
    uid_map = xasprintf ("0 %d 1\n"
                         "%d %d 1\n", overflow_uid, sandbox_uid, parent_uid);
  else
    uid_map = xasprintf ("%d %d 1\n", sandbox_uid, parent_uid);

  if (map_root && parent_gid != 0 && sandbox_gid != 0)
    gid_map = xasprintf ("0 %d 1\n"
                         "%d %d 1\n", overflow_gid, sandbox_gid, parent_gid);
  else
    gid_map = xasprintf ("%d %d 1\n", sandbox_gid, parent_gid);

  /* We have to be root to be allowed to write to the uid map
   * for setuid apps, so temporary set fsuid to 0 */
  if (is_privileged)
    old_fsuid = setfsuid (0);

  if (write_file_at (dir_fd, "uid_map", uid_map) != 0)
    die_with_error ("setting up uid map");

  if (deny_groups &&
      write_file_at (dir_fd, "setgroups", "deny\n") != 0)
    {
      /* If /proc/[pid]/setgroups does not exist, assume we are
       * running a linux kernel < 3.19, i.e. we live with the
       * vulnerability known as CVE-2014-8989 in older kernels
       * where setgroups does not exist.
       */
      if (errno != ENOENT)
        die_with_error ("error writing to setgroups");
    }

  if (write_file_at (dir_fd, "gid_map", gid_map) != 0)
    die_with_error ("setting up gid map");

  if (is_privileged)
    {
      setfsuid (old_fsuid);
      if ((uid_t) setfsuid (-1) != real_uid)
        die ("Unable to re-set fsuid");
    }
}

static void
privileged_op (int         privileged_op_socket,
               uint32_t    op,
               uint32_t    flags,
               uint32_t    perms,
               size_t      size_arg,
               const char *arg1,
               const char *arg2)
{
  bind_mount_result bind_result;
  char *failing_path = NULL;

  if (privileged_op_socket != -1)
    {
      uint32_t buffer[2048];  /* 8k, but is int32 to guarantee nice alignment */
      PrivSepOp *op_buffer = (PrivSepOp *) buffer;
      size_t buffer_size = sizeof (PrivSepOp);
      uint32_t arg1_offset = 0, arg2_offset = 0;

      /* We're unprivileged, send this request to the privileged part */

      if (arg1 != NULL)
        {
          arg1_offset = buffer_size;
          buffer_size += strlen (arg1) + 1;
        }
      if (arg2 != NULL)
        {
          arg2_offset = buffer_size;
          buffer_size += strlen (arg2) + 1;
        }

      if (buffer_size >= sizeof (buffer))
        die ("privilege separation operation to large");

      op_buffer->op = op;
      op_buffer->flags = flags;
      op_buffer->perms = perms;
      op_buffer->size_arg = size_arg;
      op_buffer->arg1_offset = arg1_offset;
      op_buffer->arg2_offset = arg2_offset;
      if (arg1 != NULL)
        strcpy ((char *) buffer + arg1_offset, arg1);
      if (arg2 != NULL)
        strcpy ((char *) buffer + arg2_offset, arg2);

      if (TEMP_FAILURE_RETRY (write (privileged_op_socket, buffer, buffer_size)) != (ssize_t)buffer_size)
        die ("Can't write to privileged_op_socket");

      if (TEMP_FAILURE_RETRY (read (privileged_op_socket, buffer, 1)) != 1)
        die ("Can't read from privileged_op_socket");

      return;
    }

  /*
   * This runs a privileged request for the unprivileged setup
   * code. Note that since the setup code is unprivileged it is not as
   * trusted, so we need to verify that all requests only affect the
   * child namespace as set up by the privileged parts of the setup,
   * and that all the code is very careful about handling input.
   *
   * This means:
   *  * Bind mounts are safe, since we always use filesystem namespace. They
   *     must be recursive though, as otherwise you can use a non-recursive bind
   *     mount to access an otherwise over-mounted mountpoint.
   *  * Mounting proc, tmpfs, mqueue, devpts in the child namespace is assumed to
   *    be safe.
   *  * Remounting RO (even non-recursive) is safe because it decreases privileges.
   *  * sethostname() is safe only if we set up a UTS namespace
   */
  switch (op)
    {
    case PRIV_SEP_OP_DONE:
      break;

    case PRIV_SEP_OP_REMOUNT_RO_NO_RECURSIVE:
      bind_result = bind_mount (proc_fd, NULL, arg2, BIND_READONLY, &failing_path);

      if (bind_result != BIND_MOUNT_SUCCESS)
        die_with_bind_result (bind_result, errno, failing_path,
                              "Can't remount readonly on %s", arg2);

      assert (failing_path == NULL);    /* otherwise we would have died */
      break;

    case PRIV_SEP_OP_BIND_MOUNT:
      /* We always bind directories recursively, otherwise this would let us
         access files that are otherwise covered on the host */
      bind_result = bind_mount (proc_fd, arg1, arg2, BIND_RECURSIVE | flags, &failing_path);

      if (bind_result != BIND_MOUNT_SUCCESS)
        die_with_bind_result (bind_result, errno, failing_path,
                              "Can't bind mount %s on %s", arg1, arg2);

      assert (failing_path == NULL);    /* otherwise we would have died */
      break;

    case PRIV_SEP_OP_PROC_MOUNT:
      if (mount ("proc", arg1, "proc", MS_NOSUID | MS_NOEXEC | MS_NODEV, NULL) != 0)
        die_with_mount_error ("Can't mount proc on %s", arg1);
      break;

    case PRIV_SEP_OP_TMPFS_MOUNT:
      {
        cleanup_free char *mode = NULL;

        /* This check should be unnecessary since we checked this when parsing
         * the --size option as well. However, better be safe than sorry. */
        if (size_arg > MAX_TMPFS_BYTES)
          die_with_error ("Specified tmpfs size too large (%zu > %zu)", size_arg, MAX_TMPFS_BYTES);

        if (size_arg != 0)
          mode = xasprintf ("mode=%#o,size=%zu", perms, size_arg);
        else
          mode = xasprintf ("mode=%#o", perms);

        cleanup_free char *opt = label_mount (mode, opt_file_label);
        if (mount ("tmpfs", arg1, "tmpfs", MS_NOSUID | MS_NODEV, opt) != 0)
          die_with_mount_error ("Can't mount tmpfs on %s", arg1);
        break;
      }

    case PRIV_SEP_OP_DEVPTS_MOUNT:
      if (mount ("devpts", arg1, "devpts", MS_NOSUID | MS_NOEXEC,
                 "newinstance,ptmxmode=0666,mode=620") != 0)
        die_with_mount_error ("Can't mount devpts on %s", arg1);
      break;

    case PRIV_SEP_OP_MQUEUE_MOUNT:
      if (mount ("mqueue", arg1, "mqueue", 0, NULL) != 0)
        die_with_mount_error ("Can't mount mqueue on %s", arg1);
      break;

    case PRIV_SEP_OP_OVERLAY_MOUNT:
      if (mount ("overlay", arg2, "overlay", MS_MGC_VAL, arg1) != 0)
        {
          /* The standard message for ELOOP, "Too many levels of symbolic
           * links", is not helpful here. */
          if (errno == ELOOP)
            die ("Can't make overlay mount on %s with options %s: "
                "Overlay directories may not overlap",
                arg2, arg1);
          die_with_mount_error ("Can't make overlay mount on %s with options %s",
                                arg2, arg1);
        }
      break;

    case PRIV_SEP_OP_SET_HOSTNAME:
      /* This is checked at the start, but lets verify it here in case
         something manages to send hacked priv-sep operation requests. */
      if (!opt_unshare_uts)
        die ("Refusing to set hostname in original namespace");
      if (sethostname (arg1, strlen(arg1)) != 0)
        die_with_error ("Can't set hostname to %s", arg1);
      break;

    default:
      die ("Unexpected privileged op %d", op);
    }
}

/* This is run unprivileged in the child namespace but can request
 * some privileged operations (also in the child namespace) via the
 * privileged_op_socket.
 */
static void
setup_newroot (bool unshare_pid,
               int  privileged_op_socket)
{
  SetupOp *op;
  int tmp_overlay_idx = 0;

  for (op = ops; op != NULL; op = op->next)
    {
      cleanup_free char *source = NULL;
      cleanup_free char *dest = NULL;
      int source_mode = 0;
      unsigned int i;

      if (op->source &&
          op->type != SETUP_MAKE_SYMLINK)
        {
          source = get_oldroot_path (op->source);
          source_mode = get_file_mode (source);
          if (source_mode < 0)
            {
              if (op->flags & ALLOW_NOTEXIST && errno == ENOENT)
                continue; /* Ignore and move on */
              die_with_error("Can't get type of source %s", op->source);
            }
        }

      if (op->dest &&
          (op->flags & NO_CREATE_DEST) == 0)
        {
          unsigned parent_mode = 0755;

          /* If we're creating a file that is inaccessible by the owning group,
           * try to achieve least-astonishment by creating parents also
           * inaccessible by that group. */
          if (op->perms >= 0 &&
              (op->perms & 0070) == 0)
            parent_mode &= ~0050U;

          /* The same, but for users other than the owner and group. */
          if (op->perms >= 0 &&
              (op->perms & 0007) == 0)
            parent_mode &= ~0005U;

          dest = get_newroot_path (op->dest);
          if (mkdir_with_parents (dest, parent_mode, false) != 0)
            die_with_error ("Can't mkdir parents for %s", op->dest);
        }

      switch (op->type)
        {
        case SETUP_RO_BIND_MOUNT:
        case SETUP_DEV_BIND_MOUNT:
        case SETUP_BIND_MOUNT:
          if (source_mode == S_IFDIR)
            {
              if (ensure_dir (dest, 0755) != 0)
                die_with_error ("Can't mkdir %s", op->dest);
            }
          else if (ensure_file (dest, 0444) != 0)
            die_with_error ("Can't create file at %s", op->dest);

          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_BIND_MOUNT,
                         (op->type == SETUP_RO_BIND_MOUNT ? BIND_READONLY : 0) |
                         (op->type == SETUP_DEV_BIND_MOUNT ? BIND_DEVICES : 0),
                         0, 0, source, dest);

          if (op->fd >= 0)
            {
              struct stat fd_st, mount_st;

              /* When using bind-fd, there is a race condition between resolving the fd as a magic symlink
               * and mounting it, where someone could replace what is at the symlink target. Ideally
               * we would not even resolve the symlink and directly bind-mount from the fd, but unfortunately
               * we can't do that, because its not permitted to bind mount a fd from another user namespace.
               * So, we resolve, mount and then compare fstat+stat to detect the race. */

              if (fstat(op->fd, &fd_st) != 0)
                die_with_error("Can't stat fd %d", op->fd);
              if (lstat(dest, &mount_st) != 0)
                die_with_error("Can't stat mount at %s", dest);

              if (fd_st.st_ino != mount_st.st_ino ||
                  fd_st.st_dev != mount_st.st_dev)
                die_with_error("Race condition binding dirfd");

              close(op->fd);
              op->fd = -1;
            }

          break;

        case SETUP_OVERLAY_MOUNT:
        case SETUP_RO_OVERLAY_MOUNT:
        case SETUP_TMP_OVERLAY_MOUNT:
          {
            StringBuilder sb = {0};
            bool multi_src = false;

            if (ensure_dir (dest, 0755) != 0)
              die_with_error ("Can't mkdir %s", op->dest);

            if (op->source != NULL)
              {
                strappend (&sb, "upperdir=/oldroot");
                strappend_escape_for_mount_options (&sb, op->source);
                strappend (&sb, ",workdir=/oldroot");
                op = op->next;
                strappend_escape_for_mount_options (&sb, op->source);
                strappend (&sb, ",");
              }
            else if (op->type == SETUP_TMP_OVERLAY_MOUNT)
              strappendf (&sb, "upperdir=/tmp-overlay-upper-%1$d,workdir=/tmp-overlay-work-%1$d,",
                          tmp_overlay_idx++);

            strappend (&sb, "lowerdir=/oldroot");
            while (op->next != NULL && op->next->type == SETUP_OVERLAY_SRC)
              {
                op = op->next;
                if (multi_src)
                  strappend (&sb, ":/oldroot");
                strappend_escape_for_mount_options (&sb, op->source);
                multi_src = true;
              }

            strappend (&sb, ",userxattr");

            privileged_op (privileged_op_socket,
                           PRIV_SEP_OP_OVERLAY_MOUNT, 0, 0, 0, sb.str, dest);
            free (sb.str);
          }
          break;

        case SETUP_REMOUNT_RO_NO_RECURSIVE:
          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_REMOUNT_RO_NO_RECURSIVE, 0, 0, 0, NULL, dest);
          break;

        case SETUP_MOUNT_PROC:
          if (ensure_dir (dest, 0755) != 0)
            die_with_error ("Can't mkdir %s", op->dest);

          if (unshare_pid || opt_pidns_fd != -1)
            {
              /* Our own procfs */
              privileged_op (privileged_op_socket,
                             PRIV_SEP_OP_PROC_MOUNT, 0, 0, 0,
                             dest, NULL);
            }
          else
            {
              /* Use system procfs, as we share pid namespace anyway */
              privileged_op (privileged_op_socket,
                             PRIV_SEP_OP_BIND_MOUNT, 0, 0, 0,
                             "oldroot/proc", dest);
            }

          /* There are a bunch of weird old subdirs of /proc that could potentially be
             problematic (for instance /proc/sysrq-trigger lets you shut down the machine
             if you have write access). We should not have access to these as a non-privileged
             user, but lets cover them anyway just to make sure */
          static const char * const cover_proc_dirs[] = { "sys", "sysrq-trigger", "irq", "bus" };
          for (i = 0; i < N_ELEMENTS (cover_proc_dirs); i++)
            {
              cleanup_free char *subdir = strconcat3 (dest, "/", cover_proc_dirs[i]);
              if (access (subdir, W_OK) < 0)
                {
                  /* The file is already read-only or doesn't exist.  */
                  if (errno == EACCES || errno == ENOENT || errno == EROFS)
                    continue;

                  die_with_error ("Can't access %s", subdir);
                }

              privileged_op (privileged_op_socket,
                             PRIV_SEP_OP_BIND_MOUNT, BIND_READONLY, 0, 0,
                             subdir, subdir);
            }

          break;

        case SETUP_MOUNT_DEV:
          if (ensure_dir (dest, 0755) != 0)
            die_with_error ("Can't mkdir %s", op->dest);

          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_TMPFS_MOUNT, 0, 0755, 0,
                         dest, NULL);

          static const char *const devnodes[] = { "null", "zero", "full", "random", "urandom", "tty" };
          for (i = 0; i < N_ELEMENTS (devnodes); i++)
            {
              cleanup_free char *node_dest = strconcat3 (dest, "/", devnodes[i]);
              cleanup_free char *node_src = strconcat ("/oldroot/dev/", devnodes[i]);
              if (create_file (node_dest, 0444, NULL) != 0)
                die_with_error ("Can't create file %s/%s", op->dest, devnodes[i]);
              privileged_op (privileged_op_socket,
                             PRIV_SEP_OP_BIND_MOUNT, BIND_DEVICES, 0, 0,
                             node_src, node_dest);
            }

          static const char *const stdionodes[] = { "stdin", "stdout", "stderr" };
          for (i = 0; i < N_ELEMENTS (stdionodes); i++)
            {
              cleanup_free char *target = xasprintf ("/proc/self/fd/%d", i);
              cleanup_free char *node_dest = strconcat3 (dest, "/", stdionodes[i]);
              if (symlink (target, node_dest) < 0)
                die_with_error ("Can't create symlink %s/%s", op->dest, stdionodes[i]);
            }

          /* /dev/fd and /dev/core - legacy, but both nspawn and docker do these */
          { cleanup_free char *dev_fd = strconcat (dest, "/fd");
            if (symlink ("/proc/self/fd", dev_fd) < 0)
              die_with_error ("Can't create symlink %s", dev_fd);
          }
          { cleanup_free char *dev_core = strconcat (dest, "/core");
            if (symlink ("/proc/kcore", dev_core) < 0)
              die_with_error ("Can't create symlink %s", dev_core);
          }

          {
            cleanup_free char *pts = strconcat (dest, "/pts");
            cleanup_free char *ptmx = strconcat (dest, "/ptmx");
            cleanup_free char *shm = strconcat (dest, "/shm");

            if (mkdir (shm, 0755) == -1)
              die_with_error ("Can't create %s/shm", op->dest);

            if (mkdir (pts, 0755) == -1)
              die_with_error ("Can't create %s/devpts", op->dest);
            privileged_op (privileged_op_socket,
                           PRIV_SEP_OP_DEVPTS_MOUNT, 0, 0, 0, pts, NULL);

            if (symlink ("pts/ptmx", ptmx) != 0)
              die_with_error ("Can't make symlink at %s/ptmx", op->dest);
          }

          /* If stdout is a tty, that means the sandbox can write to the
             outside-sandbox tty. In that case we also create a /dev/console
             that points to this tty device. This should not cause any more
             access than we already have, and it makes ttyname() work in the
             sandbox. */
          if (host_tty_dev != NULL && *host_tty_dev != 0)
            {
              cleanup_free char *src_tty_dev = strconcat ("/oldroot", host_tty_dev);
              cleanup_free char *dest_console = strconcat (dest, "/console");

              if (create_file (dest_console, 0444, NULL) != 0)
                die_with_error ("creating %s/console", op->dest);

              privileged_op (privileged_op_socket,
                             PRIV_SEP_OP_BIND_MOUNT, BIND_DEVICES, 0, 0,
                             src_tty_dev, dest_console);
            }

          break;

        case SETUP_MOUNT_TMPFS:
          assert (dest != NULL);
          assert (op->perms >= 0);
          assert (op->perms <= 07777);

          if (ensure_dir (dest, 0755) != 0)
            die_with_error ("Can't mkdir %s", op->dest);

          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_TMPFS_MOUNT, 0, op->perms, op->size,
                         dest, NULL);
          break;

        case SETUP_MOUNT_MQUEUE:
          if (ensure_dir (dest, 0755) != 0)
            die_with_error ("Can't mkdir %s", op->dest);

          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_MQUEUE_MOUNT, 0, 0, 0,
                         dest, NULL);
          break;

        case SETUP_MAKE_DIR:
          assert (dest != NULL);
          assert (op->perms >= 0);
          assert (op->perms <= 07777);

          if (ensure_dir (dest, op->perms) != 0)
            die_with_error ("Can't mkdir %s", op->dest);

          break;

        case SETUP_CHMOD:
          assert (op->dest != NULL);
          /* We used NO_CREATE_DEST so we have to use get_newroot_path()
           * explicitly */
          assert (dest == NULL);
          dest = get_newroot_path (op->dest);
          assert (dest != NULL);
          assert (op->perms >= 0);
          assert (op->perms <= 07777);

          if (chmod (dest, op->perms) != 0)
            die_with_error ("Can't chmod %#o %s", op->perms, op->dest);

          break;

        case SETUP_MAKE_FILE:
          {
            cleanup_fd int dest_fd = -1;

            assert (dest != NULL);
            assert (op->perms >= 0);
            assert (op->perms <= 07777);

            dest_fd = creat (dest, op->perms);
            if (dest_fd == -1)
              die_with_error ("Can't create file %s", op->dest);

            if (copy_file_data (op->fd, dest_fd) != 0)
              die_with_error ("Can't write data to file %s", op->dest);

            close (op->fd);
            op->fd = -1;
          }
          break;

        case SETUP_MAKE_BIND_FILE:
        case SETUP_MAKE_RO_BIND_FILE:
          {
            cleanup_fd int dest_fd = -1;
            char tempfile[] = "/bindfileXXXXXX";

            assert (dest != NULL);
            assert (op->perms >= 0);
            assert (op->perms <= 07777);

            dest_fd = mkstemp (tempfile);
            if (dest_fd == -1)
              die_with_error ("Can't create tmpfile for %s", op->dest);

            if (fchmod (dest_fd, op->perms) != 0)
              die_with_error ("Can't set mode %#o on file to be used for %s",
                              op->perms, op->dest);

            if (copy_file_data (op->fd, dest_fd) != 0)
              die_with_error ("Can't write data to file %s", op->dest);

            close (op->fd);
            op->fd = -1;

            assert (dest != NULL);

            if (ensure_file (dest, 0444) != 0)
              die_with_error ("Can't create file at %s", op->dest);

            privileged_op (privileged_op_socket,
                           PRIV_SEP_OP_BIND_MOUNT,
                           (op->type == SETUP_MAKE_RO_BIND_FILE ? BIND_READONLY : 0),
                           0, 0, tempfile, dest);

            /* Remove the file so we're sure the app can't get to it in any other way.
               Its outside the container chroot, so it shouldn't be possible, but lets
               make it really sure. */
            unlink (tempfile);
          }
          break;

        case SETUP_MAKE_SYMLINK:
          assert (op->source != NULL);  /* guaranteed by the constructor */
          if (symlink (op->source, dest) != 0)
            {
              if (errno == EEXIST)
                {
                  cleanup_free char *existing = readlink_malloc (dest);
                  if (existing == NULL)
                    {
                      if (errno == EINVAL)
                        die ("Can't make symlink at %s: destination exists and is not a symlink", op->dest);
                      else
                        die_with_error ("Can't make symlink at %s: destination exists, and cannot read symlink target", op->dest);
                    }

                  if (strcmp (existing, op->source) == 0)
                    break;

                  die ("Can't make symlink at %s: existing destination is %s", op->dest, existing);
                }
              die_with_error ("Can't make symlink at %s", op->dest);
            }
          break;

        case SETUP_SET_HOSTNAME:
          assert (op->dest != NULL);  /* guaranteed by the constructor */
          privileged_op (privileged_op_socket,
                         PRIV_SEP_OP_SET_HOSTNAME, 0, 0, 0,
                         op->dest, NULL);
          break;

        case SETUP_OVERLAY_SRC:  /* handled by SETUP_OVERLAY_MOUNT */
        default:
          die ("Unexpected type %d", op->type);
        }
    }
  privileged_op (privileged_op_socket,
                 PRIV_SEP_OP_DONE, 0, 0, 0, NULL, NULL);
}

/* Do not leak file descriptors already used by setup_newroot () */
static void
close_ops_fd (void)
{
  SetupOp *op;

  for (op = ops; op != NULL; op = op->next)
    {
      if (op->fd != -1)
        {
          (void) close (op->fd);
          op->fd = -1;
        }
    }
}

/* We need to resolve relative symlinks in the sandbox before we
   chroot so that absolute symlinks are handled correctly. We also
   need to do this after we've switched to the real uid so that
   e.g. paths on fuse mounts work */
static void
resolve_symlinks_in_ops (void)
{
  SetupOp *op;

  for (op = ops; op != NULL; op = op->next)
    {
      const char *old_source;

      switch (op->type)
        {
        case SETUP_RO_BIND_MOUNT:
        case SETUP_DEV_BIND_MOUNT:
        case SETUP_BIND_MOUNT:
        case SETUP_OVERLAY_SRC:
        case SETUP_OVERLAY_MOUNT:
          old_source = op->source;
          op->source = realpath (old_source, NULL);
          if (op->source == NULL)
            {
              if (op->flags & ALLOW_NOTEXIST && errno == ENOENT)
                op->source = old_source;
              else
                die_with_error("Can't find source path %s", old_source);
            }
          break;

        case SETUP_RO_OVERLAY_MOUNT:
        case SETUP_TMP_OVERLAY_MOUNT:
        case SETUP_MOUNT_PROC:
        case SETUP_MOUNT_DEV:
        case SETUP_MOUNT_TMPFS:
        case SETUP_MOUNT_MQUEUE:
        case SETUP_MAKE_DIR:
        case SETUP_MAKE_FILE:
        case SETUP_MAKE_BIND_FILE:
        case SETUP_MAKE_RO_BIND_FILE:
        case SETUP_MAKE_SYMLINK:
        case SETUP_REMOUNT_RO_NO_RECURSIVE:
        case SETUP_SET_HOSTNAME:
        case SETUP_CHMOD:
        default:
          break;
        }
    }
}


static const char *
resolve_string_offset (void    *buffer,
                       size_t   buffer_size,
                       uint32_t offset)
{
  if (offset == 0)
    return NULL;

  if (offset > buffer_size)
    die ("Invalid string offset %d (buffer size %zd)", offset, buffer_size);

  return (const char *) buffer + offset;
}

static uint32_t
read_priv_sec_op (int          read_socket,
                  void        *buffer,
                  size_t       buffer_size,
                  uint32_t    *flags,
                  uint32_t    *perms,
                  size_t      *size_arg,
                  const char **arg1,
                  const char **arg2)
{
  const PrivSepOp *op = (const PrivSepOp *) buffer;
  ssize_t rec_len;

  do
    rec_len = read (read_socket, buffer, buffer_size - 1);
  while (rec_len == -1 && errno == EINTR);

  if (rec_len < 0)
    die_with_error ("Can't read from unprivileged helper");

  if (rec_len == 0)
    exit (1); /* Privileged helper died and printed error, so exit silently */

  if ((size_t)rec_len < sizeof (PrivSepOp))
    die ("Invalid size %zd from unprivileged helper", rec_len);

  /* Guarantee zero termination of any strings */
  ((char *) buffer)[rec_len] = 0;

  *flags = op->flags;
  *perms = op->perms;
  *size_arg = op->size_arg;
  *arg1 = resolve_string_offset (buffer, rec_len, op->arg1_offset);
  *arg2 = resolve_string_offset (buffer, rec_len, op->arg2_offset);

  return op->op;
}

static void __attribute__ ((noreturn))
print_version_and_exit (void)
{
  printf ("%s\n", PACKAGE_STRING);
  exit (0);
}

static int
is_modifier_option (const char *option)
{
  return strcmp (option, "--perms") == 0
         || strcmp(option, "--size") == 0;
}

static void
warn_only_last_option (const char *name)
{
  warn ("Only the last %s option will take effect", name);
}

static void
make_setup_overlay_src_ops (const char *const *const argv)
{
  /* SETUP_OVERLAY_SRC is unlike other SETUP_* ops in that it exists to hold
   * data for SETUP_{,TMP_,RO_}OVERLAY_MOUNT ops, not to be its own operation.
   * This lets us reuse existing code paths to handle resolving the realpaths
   * of each source, as no other operations involve multiple sources the way
   * the *_OVERLAY_MOUNT ops do.
   *
   * While the --overlay-src arguments are expected to (directly) precede the
   * --overlay argument, in bottom-to-top order, the SETUP_OVERLAY_SRC ops
   * follow their corresponding *_OVERLAY_MOUNT op, in top-to-bottom order
   * (the order in which overlayfs will want them). They are handled specially
   * in setup_new_root () during the processing of *_OVERLAY_MOUNT.
   */
  int i;
  SetupOp *op;

  for (i = 1; i <= next_overlay_src_count; i++)
    {
      op = setup_op_new (SETUP_OVERLAY_SRC);
      op->source = argv[1 - 2 * i];
    }
  next_overlay_src_count = 0;
}

static void
parse_args_recurse (int          *argcp,
                    const char ***argvp,
                    bool          in_file,
                    int          *total_parsed_argc_p)
{
  SetupOp *op;
  int argc = *argcp;
  const char **argv = *argvp;
  /* I can't imagine a case where someone wants more than this.
   * If you do...you should be able to pass multiple files
   * via a single tmpfs and linking them there, etc.
   *
   * We're adding this hardening due to precedent from
   * http://googleprojectzero.blogspot.com/2014/08/the-poisoned-nul-byte-2014-edition.html
   *
   * I picked 9000 because the Internet told me to and it was hard to
   * resist.
   */
  static const int32_t MAX_ARGS = 9000;

  if (*total_parsed_argc_p > MAX_ARGS)
    die ("Exceeded maximum number of arguments %u", MAX_ARGS);

  while (argc > 0)
    {
      const char *arg = argv[0];

      if (strcmp (arg, "--help") == 0)
        {
          usage (EXIT_SUCCESS, stdout);
        }
      else if (strcmp (arg, "--version") == 0)
        {
          print_version_and_exit ();
        }
      else if (strcmp (arg, "--args") == 0)
        {
          int the_fd;
          char *endptr;
          const char *p, *data_end;
          size_t data_len;
          cleanup_free const char **data_argv = NULL;
          const char **data_argv_copy;
          int data_argc;
          int i;

          if (in_file)
            die ("--args not supported in arguments file");

          if (argc < 2)
            die ("--args takes an argument");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          /* opt_args_data is essentially a recursive argv array, which we must
           * keep allocated until exit time, since its argv entries get used
           * by the other cases in parse_args_recurse() when we recurse. */
          opt_args_data = load_file_data (the_fd, &data_len);
          if (opt_args_data == NULL)
            die_with_error ("Can't read --args data");
          (void) close (the_fd);

          data_end = opt_args_data + data_len;
          data_argc = 0;

          p = opt_args_data;
          while (p != NULL && p < data_end)
            {
              data_argc++;
              (*total_parsed_argc_p)++;
              if (*total_parsed_argc_p > MAX_ARGS)
                die ("Exceeded maximum number of arguments %u", MAX_ARGS);
              p = memchr (p, 0, data_end - p);
              if (p != NULL)
                p++;
            }

          data_argv = xcalloc (data_argc + 1, sizeof (char *));

          i = 0;
          p = opt_args_data;
          while (p != NULL && p < data_end)
            {
              /* Note: load_file_data always adds a nul terminator, so this is safe
               * even for the last string. */
              data_argv[i++] = p;
              p = memchr (p, 0, data_end - p);
              if (p != NULL)
                p++;
            }

          data_argv_copy = data_argv; /* Don't change data_argv, we need to free it */
          parse_args_recurse (&data_argc, &data_argv_copy, true, total_parsed_argc_p);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--argv0") == 0)
        {
          if (argc < 2)
            die ("--argv0 takes one argument");

          if (opt_argv0 != NULL)
            die ("--argv0 used multiple times");

          opt_argv0 = argv[1];
          argv++;
          argc--;
        }
      else if (strcmp (arg, "--level-prefix") == 0)
        {
          bwrap_level_prefix = true;
        }
      else if (strcmp (arg, "--unshare-all") == 0)
        {
          /* Keep this in order with the older (legacy) --unshare arguments,
           * we use the --try variants of user and cgroup, since we want
           * to support systems/kernels without support for those.
           */
          opt_unshare_user_try = opt_unshare_ipc = opt_unshare_pid =
            opt_unshare_uts = opt_unshare_cgroup_try =
            opt_unshare_net = true;
        }
      /* Begin here the older individual --unshare variants */
      else if (strcmp (arg, "--unshare-user") == 0)
        {
          opt_unshare_user = true;
        }
      else if (strcmp (arg, "--unshare-user-try") == 0)
        {
          opt_unshare_user_try = true;
        }
      else if (strcmp (arg, "--unshare-ipc") == 0)
        {
          opt_unshare_ipc = true;
        }
      else if (strcmp (arg, "--unshare-pid") == 0)
        {
          opt_unshare_pid = true;
        }
      else if (strcmp (arg, "--unshare-net") == 0)
        {
          opt_unshare_net = true;
        }
      else if (strcmp (arg, "--unshare-uts") == 0)
        {
          opt_unshare_uts = true;
        }
      else if (strcmp (arg, "--unshare-cgroup") == 0)
        {
          opt_unshare_cgroup = true;
        }
      else if (strcmp (arg, "--unshare-cgroup-try") == 0)
        {
          opt_unshare_cgroup_try = true;
        }
      /* Begin here the newer --share variants */
      else if (strcmp (arg, "--share-net") == 0)
        {
          opt_unshare_net = false;
        }
      /* End --share variants, other arguments begin */
      else if (strcmp (arg, "--chdir") == 0)
        {
          if (argc < 2)
            die ("--chdir takes one argument");

          if (opt_chdir_path != NULL)
            warn_only_last_option ("--chdir");

          opt_chdir_path = argv[1];
          argv++;
          argc--;
        }
      else if (strcmp (arg, "--disable-userns") == 0)
        {
          opt_disable_userns = true;
        }
      else if (strcmp (arg, "--assert-userns-disabled") == 0)
        {
          opt_assert_userns_disabled = true;
        }
      else if (strcmp (arg, "--remount-ro") == 0)
        {
          if (argc < 2)
            die ("--remount-ro takes one argument");

          op = setup_op_new (SETUP_REMOUNT_RO_NO_RECURSIVE);
          op->dest = argv[1];

          argv++;
          argc--;
        }
      else if (strcmp(arg, "--bind") == 0 ||
               strcmp(arg, "--bind-try") == 0)
        {
          if (argc < 3)
            die ("%s takes two arguments", arg);

          op = setup_op_new (SETUP_BIND_MOUNT);
          op->source = argv[1];
          op->dest = argv[2];
          if (strcmp(arg, "--bind-try") == 0)
            op->flags = ALLOW_NOTEXIST;

          argv += 2;
          argc -= 2;
        }
      else if (strcmp(arg, "--ro-bind") == 0 ||
               strcmp(arg, "--ro-bind-try") == 0)
        {
          if (argc < 3)
            die ("%s takes two arguments", arg);

          op = setup_op_new (SETUP_RO_BIND_MOUNT);
          op->source = argv[1];
          op->dest = argv[2];
          if (strcmp(arg, "--ro-bind-try") == 0)
            op->flags = ALLOW_NOTEXIST;

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--dev-bind") == 0 ||
               strcmp (arg, "--dev-bind-try") == 0)
        {
          if (argc < 3)
            die ("%s takes two arguments", arg);

          op = setup_op_new (SETUP_DEV_BIND_MOUNT);
          op->source = argv[1];
          op->dest = argv[2];
          if (strcmp(arg, "--dev-bind-try") == 0)
            op->flags = ALLOW_NOTEXIST;

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--bind-fd") == 0 ||
               strcmp (arg, "--ro-bind-fd") == 0)
        {
          int src_fd;
          char *endptr;

          if (argc < 3)
            die ("--bind-fd takes two arguments");

          src_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || src_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          if (strcmp(arg, "--ro-bind-fd") == 0)
            op = setup_op_new (SETUP_RO_BIND_MOUNT);
          else
            op = setup_op_new (SETUP_BIND_MOUNT);
          op->source = xasprintf ("/proc/self/fd/%d", src_fd);
          op->fd = src_fd;
          op->dest = argv[2];

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--overlay-src") == 0)
        {
          if (is_privileged)
            die ("The --overlay-src option is not permitted in setuid mode");

          next_overlay_src_count++;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--overlay") == 0)
        {
          SetupOp *workdir_op;

          if (is_privileged)
            die ("The --overlay option is not permitted in setuid mode");

          if (argc < 4)
            die ("--overlay takes three arguments");

          if (next_overlay_src_count < 1)
            die ("--overlay requires at least one --overlay-src");

          op = setup_op_new (SETUP_OVERLAY_MOUNT);
          op->source = argv[1];
          workdir_op = setup_op_new (SETUP_OVERLAY_SRC);
          workdir_op->source = argv[2];
          op->dest = argv[3];
          make_setup_overlay_src_ops (argv);

          argv += 3;
          argc -= 3;
        }
      else if (strcmp (arg, "--tmp-overlay") == 0)
        {
          if (is_privileged)
            die ("The --tmp-overlay option is not permitted in setuid mode");

          if (argc < 2)
            die ("--tmp-overlay takes an argument");

          if (next_overlay_src_count < 1)
            die ("--tmp-overlay requires at least one --overlay-src");

          op = setup_op_new (SETUP_TMP_OVERLAY_MOUNT);
          op->dest = argv[1];
          make_setup_overlay_src_ops (argv);
          opt_tmp_overlay_count++;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--ro-overlay") == 0)
        {
          if (is_privileged)
            die ("The --ro-overlay option is not permitted in setuid mode");

          if (argc < 2)
            die ("--ro-overlay takes an argument");

          if (next_overlay_src_count < 2)
            die ("--ro-overlay requires at least two --overlay-src");

          op = setup_op_new (SETUP_RO_OVERLAY_MOUNT);
          op->dest = argv[1];
          make_setup_overlay_src_ops (argv);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--proc") == 0)
        {
          if (argc < 2)
            die ("--proc takes an argument");

          op = setup_op_new (SETUP_MOUNT_PROC);
          op->dest = argv[1];

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--exec-label") == 0)
        {
          if (argc < 2)
            die ("--exec-label takes an argument");

          if (opt_exec_label != NULL)
            warn_only_last_option ("--exec-label");

          opt_exec_label = argv[1];
          die_unless_label_valid (opt_exec_label);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--file-label") == 0)
        {
          if (argc < 2)
            die ("--file-label takes an argument");

          if (opt_file_label != NULL)
            warn_only_last_option ("--file-label");

          opt_file_label = argv[1];
          die_unless_label_valid (opt_file_label);
          if (label_create_file (opt_file_label))
            die_with_error ("--file-label setup failed");

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--dev") == 0)
        {
          if (argc < 2)
            die ("--dev takes an argument");

          op = setup_op_new (SETUP_MOUNT_DEV);
          op->dest = argv[1];
          opt_needs_devpts = true;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--tmpfs") == 0)
        {
          if (argc < 2)
            die ("--tmpfs takes an argument");

          op = setup_op_new (SETUP_MOUNT_TMPFS);
          op->dest = argv[1];

          /* We historically hard-coded the mode of a tmpfs as 0755. */
          if (next_perms >= 0)
            op->perms = next_perms;
          else
            op->perms = 0755;

          next_perms = -1;

          /* If the option is unset, next_size_arg is zero, which results in
           * the default tmpfs size. This is exactly what we want. */
          op->size = next_size_arg;

          next_size_arg = 0;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--mqueue") == 0)
        {
          if (argc < 2)
            die ("--mqueue takes an argument");

          op = setup_op_new (SETUP_MOUNT_MQUEUE);
          op->dest = argv[1];

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--dir") == 0)
        {
          if (argc < 2)
            die ("--dir takes an argument");

          op = setup_op_new (SETUP_MAKE_DIR);
          op->dest = argv[1];

          /* We historically hard-coded the mode of a --dir as 0755. */
          if (next_perms >= 0)
            op->perms = next_perms;
          else
            op->perms = 0755;

          next_perms = -1;
          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--file") == 0)
        {
          int file_fd;
          char *endptr;

          if (argc < 3)
            die ("--file takes two arguments");

          file_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || file_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          op = setup_op_new (SETUP_MAKE_FILE);
          op->fd = file_fd;
          op->dest = argv[2];

          /* We historically hard-coded the mode of a --file as 0666. */
          if (next_perms >= 0)
            op->perms = next_perms;
          else
            op->perms = 0666;

          next_perms = -1;
          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--bind-data") == 0)
        {
          int file_fd;
          char *endptr;

          if (argc < 3)
            die ("--bind-data takes two arguments");

          file_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || file_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          op = setup_op_new (SETUP_MAKE_BIND_FILE);
          op->fd = file_fd;
          op->dest = argv[2];

          /* This is consistent with previous bubblewrap behaviour:
           * before implementing --perms, we took the permissions
           * given to us by mkstemp(), which are documented to be 0600. */
          if (next_perms >= 0)
            op->perms = next_perms;
          else
            op->perms = 0600;

          next_perms = -1;
          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--ro-bind-data") == 0)
        {
          int file_fd;
          char *endptr;

          if (argc < 3)
            die ("--ro-bind-data takes two arguments");

          file_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || file_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          op = setup_op_new (SETUP_MAKE_RO_BIND_FILE);
          op->fd = file_fd;
          op->dest = argv[2];

          /* This is consistent with previous bubblewrap behaviour:
           * before implementing --perms, we took the permissions
           * given to us by mkstemp(), which are documented to be 0600. */
          if (next_perms >= 0)
            op->perms = next_perms;
          else
            op->perms = 0600;

          next_perms = -1;
          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--symlink") == 0)
        {
          if (argc < 3)
            die ("--symlink takes two arguments");

          op = setup_op_new (SETUP_MAKE_SYMLINK);
          op->source = argv[1];
          op->dest = argv[2];

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--lock-file") == 0)
        {
          if (argc < 2)
            die ("--lock-file takes an argument");

          (void) lock_file_new (argv[1]);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--sync-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--sync-fd takes an argument");

          if (opt_sync_fd != -1)
            warn_only_last_option ("--sync-fd");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_sync_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--block-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--block-fd takes an argument");

          if (opt_block_fd != -1)
            warn_only_last_option ("--block-fd");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_block_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--userns-block-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--userns-block-fd takes an argument");

          if (opt_userns_block_fd != -1)
            warn_only_last_option ("--userns-block-fd");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_userns_block_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--info-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--info-fd takes an argument");

          if (opt_info_fd != -1)
            warn_only_last_option ("--info-fd");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_info_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--json-status-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--json-status-fd takes an argument");

          if (opt_json_status_fd != -1)
            warn_only_last_option ("--json-status-fd");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_json_status_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--seccomp") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--seccomp takes an argument");

          if (seccomp_programs != NULL)
            die ("--seccomp cannot be combined with --add-seccomp-fd");

          if (opt_seccomp_fd != -1)
            warn_only_last_option ("--seccomp");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_seccomp_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--add-seccomp-fd") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--add-seccomp-fd takes an argument");

          if (opt_seccomp_fd != -1)
            die ("--add-seccomp-fd cannot be combined with --seccomp");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          /* takes ownership of fd */
          seccomp_program_new (&the_fd);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--userns") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--userns takes an argument");

          if (opt_userns_fd != -1)
            warn_only_last_option ("--userns");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_userns_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--userns2") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--userns2 takes an argument");

          if (opt_userns2_fd != -1)
            warn_only_last_option ("--userns2");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_userns2_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--pidns") == 0)
        {
          int the_fd;
          char *endptr;

          if (argc < 2)
            die ("--pidns takes an argument");

          if (opt_pidns_fd != -1)
            warn_only_last_option ("--pidns");

          the_fd = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_fd < 0)
            die ("Invalid fd: %s", argv[1]);

          opt_pidns_fd = the_fd;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--clearenv") == 0)
        {
          xclearenv ();
        }
      else if (strcmp (arg, "--setenv") == 0)
        {
          if (argc < 3)
            die ("--setenv takes two arguments");

          xsetenv (argv[1], argv[2], 1);

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--unsetenv") == 0)
        {
          if (argc < 2)
            die ("--unsetenv takes an argument");

          xunsetenv (argv[1]);

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--uid") == 0)
        {
          int the_uid;
          char *endptr;

          if (argc < 2)
            die ("--uid takes an argument");

          if (opt_sandbox_uid != (uid_t)-1)
            warn_only_last_option ("--uid");

          the_uid = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_uid < 0)
            die ("Invalid uid: %s", argv[1]);

          opt_sandbox_uid = the_uid;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--gid") == 0)
        {
          int the_gid;
          char *endptr;

          if (argc < 2)
            die ("--gid takes an argument");

          if (opt_sandbox_gid != (gid_t)-1)
            warn_only_last_option ("--gid");

          the_gid = strtol (argv[1], &endptr, 10);
          if (argv[1][0] == 0 || endptr[0] != 0 || the_gid < 0)
            die ("Invalid gid: %s", argv[1]);

          opt_sandbox_gid = the_gid;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--hostname") == 0)
        {
          if (argc < 2)
            die ("--hostname takes an argument");

          if (opt_sandbox_hostname != NULL)
            warn_only_last_option ("--hostname");

          op = setup_op_new (SETUP_SET_HOSTNAME);
          op->dest = argv[1];
          op->flags = NO_CREATE_DEST;

          opt_sandbox_hostname = argv[1];

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--new-session") == 0)
        {
          opt_new_session = true;
        }
      else if (strcmp (arg, "--die-with-parent") == 0)
        {
          opt_die_with_parent = true;
        }
      else if (strcmp (arg, "--as-pid-1") == 0)
        {
          opt_as_pid_1 = true;
        }
      else if (strcmp (arg, "--cap-add") == 0)
        {
          cap_value_t cap;
          if (argc < 2)
            die ("--cap-add takes an argument");

          opt_cap_add_or_drop_used = true;

          if (strcasecmp (argv[1], "ALL") == 0)
            {
              requested_caps[0] = requested_caps[1] = 0xFFFFFFFF;
            }
          else
            {
              if (cap_from_name (argv[1], &cap) < 0)
                die ("unknown cap: %s", argv[1]);

              if (cap < 32)
                requested_caps[0] |= CAP_TO_MASK_0 (cap);
              else
                requested_caps[1] |= CAP_TO_MASK_1 (cap - 32);
            }

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--cap-drop") == 0)
        {
          cap_value_t cap;
          if (argc < 2)
            die ("--cap-drop takes an argument");

          opt_cap_add_or_drop_used = true;

          if (strcasecmp (argv[1], "ALL") == 0)
            {
              requested_caps[0] = requested_caps[1] = 0;
            }
          else
            {
              if (cap_from_name (argv[1], &cap) < 0)
                die ("unknown cap: %s", argv[1]);

              if (cap < 32)
                requested_caps[0] &= ~CAP_TO_MASK_0 (cap);
              else
                requested_caps[1] &= ~CAP_TO_MASK_1 (cap - 32);
            }

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--perms") == 0)
        {
          unsigned long perms;
          char *endptr = NULL;

          if (argc < 2)
            die ("--perms takes an argument");

          if (next_perms != -1)
            die ("--perms given twice for the same action");

          perms = strtoul (argv[1], &endptr, 8);

          if (argv[1][0] == '\0'
              || endptr == NULL
              || *endptr != '\0'
              || perms > 07777)
            die ("--perms takes an octal argument <= 07777");

          next_perms = (int) perms;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--size") == 0)
        {
          unsigned long long size;
          char *endptr = NULL;

          if (is_privileged)
            die ("The --size option is not permitted in setuid mode");

          if (argc < 2)
            die ("--size takes an argument");

          if (next_size_arg != 0)
            die ("--size given twice for the same action");

          errno = 0;  /* reset errno so we can detect ERANGE from strtoull */

          size = strtoull (argv[1], &endptr, 0);

          /* isdigit: Not only check that the first digit is not '\0', but
           * simultaneously guard against negative numbers or preceding
           * spaces. */
          if (errno != 0  /* from strtoull */
              || !isdigit(argv[1][0])
              || endptr == NULL
              || *endptr != '\0'
              || size == 0)
            die ("--size takes a non-zero number of bytes");

          if (size > MAX_TMPFS_BYTES)
            die ("--size (for tmpfs) is limited to %zu", MAX_TMPFS_BYTES);

          next_size_arg = (size_t) size;

          argv += 1;
          argc -= 1;
        }
      else if (strcmp (arg, "--chmod") == 0)
        {
          unsigned long perms;
          char *endptr = NULL;

          if (argc < 3)
            die ("--chmod takes two arguments");

          perms = strtoul (argv[1], &endptr, 8);

          if (argv[1][0] == '\0'
              || endptr == NULL
              || *endptr != '\0'
              || perms > 07777)
            die ("--chmod takes an octal argument <= 07777");

          op = setup_op_new (SETUP_CHMOD);
          op->flags = NO_CREATE_DEST;
          op->perms = (int) perms;
          op->dest = argv[2];

          argv += 2;
          argc -= 2;
        }
      else if (strcmp (arg, "--") == 0)
        {
          argv += 1;
          argc -= 1;
          break;
        }
      else if (*arg == '-')
        {
          die ("Unknown option %s", arg);
        }
      else
        {
          break;
        }

      /* If --perms was set for the current action but the current action
       * didn't consume the setting, apparently --perms wasn't suitable for
       * this action. */
      if (!is_modifier_option(arg) && next_perms >= 0)
        die ("--perms must be followed by an option that creates a file");

      /* Similarly for --size. */
      if (!is_modifier_option(arg) && next_size_arg != 0)
        die ("--size must be followed by --tmpfs");

      /* Similarly for --overlay-src. */
      if (strcmp (arg, "--overlay-src") != 0 && next_overlay_src_count > 0)
        die ("--overlay-src must be followed by another --overlay-src or one of --overlay, --tmp-overlay, or --ro-overlay");

      argv++;
      argc--;
    }

  *argcp = argc;
  *argvp = argv;
}

static void
parse_args (int          *argcp,
            const char ***argvp)
{
  int total_parsed_argc = *argcp;

  parse_args_recurse (argcp, argvp, false, &total_parsed_argc);

  if (next_overlay_src_count > 0)
    die ("--overlay-src must be followed by another --overlay-src or one of --overlay, --tmp-overlay, or --ro-overlay");
}

static void
read_overflowids (void)
{
  cleanup_free char *uid_data = NULL;
  cleanup_free char *gid_data = NULL;

  uid_data = load_file_at (AT_FDCWD, "/proc/sys/kernel/overflowuid");
  if (uid_data == NULL)
    die_with_error ("Can't read /proc/sys/kernel/overflowuid");

  overflow_uid = strtol (uid_data, NULL, 10);
  if (overflow_uid == 0)
    die ("Can't parse /proc/sys/kernel/overflowuid");

  gid_data = load_file_at (AT_FDCWD, "/proc/sys/kernel/overflowgid");
  if (gid_data == NULL)
    die_with_error ("Can't read /proc/sys/kernel/overflowgid");

  overflow_gid = strtol (gid_data, NULL, 10);
  if (overflow_gid == 0)
    die ("Can't parse /proc/sys/kernel/overflowgid");
}

static void
namespace_ids_read (pid_t  pid)
{
  cleanup_free char *dir = NULL;
  cleanup_fd int ns_fd = -1;
  NsInfo *info;

  dir = xasprintf ("%d/ns", pid);
  ns_fd = TEMP_FAILURE_RETRY (openat (proc_fd, dir, O_PATH));

  if (ns_fd < 0)
    die_with_error ("open /proc/%s/ns failed", dir);

  for (info = ns_infos; info->name; info++)
    {
      bool *do_unshare = info->do_unshare;
      struct stat st;
      int r;

      /* if we don't unshare this ns, ignore it */
      if (do_unshare && *do_unshare == false)
        continue;

      r = fstatat (ns_fd, info->name, &st, 0);

      /* if we can't get the information, ignore it */
      if (r != 0)
        continue;

      info->id = st.st_ino;
    }
}

static void
namespace_ids_write (int    fd,
                     bool   in_json)
{
  NsInfo *info;

  for (info = ns_infos; info->name; info++)
    {
      cleanup_free char *output = NULL;
      const char *indent;
      uintmax_t nsid;

      nsid = (uintmax_t) info->id;

      /* if we don't have the information, we don't write it */
      if (nsid == 0)
        continue;

      indent = in_json ? " " : "\n    ";
      output = xasprintf (",%s\"%s-namespace\": %ju",
                          indent, info->name, nsid);

      dump_info (fd, output, true);
    }
}

int
main (int    argc,
      char **argv)
{
  mode_t old_umask;
  const char *base_path = NULL;
  int clone_flags;
  char *old_cwd = NULL;
  pid_t pid;
  int event_fd = -1;
  int child_wait_fd = -1;
  int setup_finished_pipe[] = {-1, -1};
  const char *new_cwd;
  uid_t ns_uid;
  gid_t ns_gid;
  struct stat sbuf;
  uint64_t val;
  int res UNUSED;
  cleanup_free char *args_data UNUSED = NULL;
  int intermediate_pids_sockets[2] = {-1, -1};
  const char *exec_path = NULL;
  int i;
  struct sigaction sa = {};

  /* Handle --version early on before we try to acquire/drop
   * any capabilities so it works in a build environment;
   * right now flatpak's build runs bubblewrap --version.
   * https://github.com/projectatomic/bubblewrap/issues/185
   */
  if (argc == 2 && (strcmp (argv[1], "--version") == 0))
    print_version_and_exit ();

  /* Reset SIGCHILD to SIG_DFL allowing signalfd working propertly
   * if the parent process had set SIGCHLD to SIG_IGN. */
  sigemptyset (&sa.sa_mask);
  sa.sa_handler = SIG_DFL;
  sigaction (SIGCHLD, &sa, NULL);

  real_uid = getuid ();
  real_gid = getgid ();

  /* Get the (optional) privileges we need */
  acquire_privs ();

  /* Never gain any more privs during exec */
  if (prctl (PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0)
    die_with_error ("prctl(PR_SET_NO_NEW_PRIVS) failed");

  /* The initial code is run with high permissions
     (i.e. CAP_SYS_ADMIN), so take lots of care. */

  read_overflowids ();

  argv0 = argv[0];

  if (isatty (1))
    host_tty_dev = ttyname (1);

  argv++;
  argc--;

  if (argc <= 0)
    usage (EXIT_FAILURE, stderr);

  parse_args (&argc, (const char ***) &argv);

  /* suck the args into a cleanup_free variable to control their lifecycle */
  args_data = opt_args_data;
  opt_args_data = NULL;

  if ((requested_caps[0] || requested_caps[1]) && is_privileged)
    die ("--cap-add in setuid mode can be used only by root");

  if (opt_userns_block_fd != -1 && !opt_unshare_user)
    die ("--userns-block-fd requires --unshare-user");

  if (opt_userns_block_fd != -1 && opt_info_fd == -1)
    die ("--userns-block-fd requires --info-fd");

  if (opt_userns_fd != -1 && opt_unshare_user)
    die ("--userns not compatible --unshare-user");

  if (opt_userns_fd != -1 && opt_unshare_user_try)
    die ("--userns not compatible --unshare-user-try");

  if (opt_disable_userns && !opt_unshare_user)
    die ("--disable-userns requires --unshare-user");

  if (opt_disable_userns && opt_userns_block_fd != -1)
    die ("--disable-userns is not compatible with  --userns-block-fd");

  /* Technically using setns() is probably safe even in the privileged
   * case, because we got passed in a file descriptor to the
   * namespace, and that can only be gotten if you have ptrace
   * permissions against the target, and then you could do whatever to
   * the namespace anyway.
   *
   * However, for practical reasons this isn't possible to use,
   * because (as described in acquire_privs()) setuid bwrap causes
   * root to own the namespaces that it creates, so you will not be
   * able to access these namespaces anyway. So, best just not support
   * it anyway.
   */
  if (opt_userns_fd != -1 && is_privileged)
    die ("--userns doesn't work in setuid mode");

  if (opt_userns2_fd != -1 && is_privileged)
    die ("--userns2 doesn't work in setuid mode");

  /* We have to do this if we weren't installed setuid (and we're not
   * root), so let's just DWIM */
  if (!is_privileged && getuid () != 0 && opt_userns_fd == -1)
    opt_unshare_user = true;

#ifdef ENABLE_REQUIRE_USERNS
  /* In this build option, we require userns. */
  if (is_privileged && getuid () != 0 && opt_userns_fd == -1)
    opt_unshare_user = true;
#endif

  if (opt_unshare_user_try &&
      stat ("/proc/self/ns/user", &sbuf) == 0)
    {
      bool disabled = false;

      /* RHEL7 has a kernel module parameter that lets you enable user namespaces */
      if (stat ("/sys/module/user_namespace/parameters/enable", &sbuf) == 0)
        {
          cleanup_free char *enable = NULL;
          enable = load_file_at (AT_FDCWD, "/sys/module/user_namespace/parameters/enable");
          if (enable != NULL && enable[0] == 'N')
            disabled = true;
        }

      /* Check for max_user_namespaces */
      if (stat ("/proc/sys/user/max_user_namespaces", &sbuf) == 0)
        {
          cleanup_free char *max_user_ns = NULL;
          max_user_ns = load_file_at (AT_FDCWD, "/proc/sys/user/max_user_namespaces");
          if (max_user_ns != NULL && strcmp(max_user_ns, "0\n") == 0)
            disabled = true;
        }

      /* Debian lets you disable *unprivileged* user namespaces. However this is not
         a problem if we're privileged, and if we're not opt_unshare_user is true
         already, and there is not much we can do, its just a non-working setup. */

      if (!disabled)
        opt_unshare_user = true;
    }

  if (argc <= 0)
    usage (EXIT_FAILURE, stderr);

  debug ("Creating root mount point");

  if (opt_sandbox_uid == (uid_t)-1)
    opt_sandbox_uid = real_uid;
  if (opt_sandbox_gid == (gid_t)-1)
    opt_sandbox_gid = real_gid;

  if (!opt_unshare_user && opt_userns_fd == -1 && opt_sandbox_uid != real_uid)
    die ("Specifying --uid requires --unshare-user or --userns");

  if (!opt_unshare_user && opt_userns_fd == -1 && opt_sandbox_gid != real_gid)
    die ("Specifying --gid requires --unshare-user or --userns");

  if (!opt_unshare_uts && opt_sandbox_hostname != NULL)
    die ("Specifying --hostname requires --unshare-uts");

  if (opt_as_pid_1 && !opt_unshare_pid)
    die ("Specifying --as-pid-1 requires --unshare-pid");

  if (opt_as_pid_1 && lock_files != NULL)
    die ("Specifying --as-pid-1 and --lock-file is not permitted");

  /* We need to read stuff from proc during the pivot_root dance, etc.
     Lets keep a fd to it open */
  proc_fd = TEMP_FAILURE_RETRY (open ("/proc", O_PATH));
  if (proc_fd == -1)
    die_with_error ("Can't open /proc");

  /* We need *some* mountpoint where we can mount the root tmpfs.
   * Because we use pivot_root, it won't appear to be mounted from
   * the perspective of the sandboxed process, so we can use anywhere
   * that is sure to exist, that is sure to not be a symlink controlled
   * by someone malicious, and that we won't immediately need to
   * access ourselves. */
  base_path = "/tmp";

  debug ("creating new namespace");

  if (opt_unshare_pid && !opt_as_pid_1)
    {
      event_fd = eventfd (0, EFD_CLOEXEC | EFD_NONBLOCK);
      if (event_fd == -1)
        die_with_error ("eventfd()");
    }

  /* We block sigchild here so that we can use signalfd in the monitor. */
  block_sigchild ();

  clone_flags = SIGCHLD | CLONE_NEWNS;
  if (opt_unshare_user)
    clone_flags |= CLONE_NEWUSER;
  if (opt_unshare_pid && opt_pidns_fd == -1)
    clone_flags |= CLONE_NEWPID;
  if (opt_unshare_net)
    clone_flags |= CLONE_NEWNET;
  if (opt_unshare_ipc)
    clone_flags |= CLONE_NEWIPC;
  if (opt_unshare_uts)
    clone_flags |= CLONE_NEWUTS;
  if (opt_unshare_cgroup)
    {
      if (stat ("/proc/self/ns/cgroup", &sbuf))
        {
          if (errno == ENOENT)
            die ("Cannot create new cgroup namespace because the kernel does not support it");
          else
            die_with_error ("stat on /proc/self/ns/cgroup failed");
        }
      clone_flags |= CLONE_NEWCGROUP;
    }
  if (opt_unshare_cgroup_try)
    {
      opt_unshare_cgroup = !stat ("/proc/self/ns/cgroup", &sbuf);
      if (opt_unshare_cgroup)
        clone_flags |= CLONE_NEWCGROUP;
    }

  child_wait_fd = eventfd (0, EFD_CLOEXEC);
  if (child_wait_fd == -1)
    die_with_error ("eventfd()");

  /* Track whether pre-exec setup finished if we're reporting process exit */
  if (opt_json_status_fd != -1)
    {
      int ret;
      ret = pipe2 (setup_finished_pipe, O_CLOEXEC);
      if (ret == -1)
        die_with_error ("pipe2()");
    }

  /* Switch to the custom user ns before the clone, gets us privs in that ns (assuming its a child of the current and thus allowed) */
  if (opt_userns_fd > 0 && setns (opt_userns_fd, CLONE_NEWUSER) != 0)
    {
      if (errno == EINVAL)
        die ("Joining the specified user namespace failed, it might not be a descendant of the current user namespace.");
      die_with_error ("Joining specified user namespace failed");
    }

  /* Sometimes we have uninteresting intermediate pids during the setup, set up code to pass the real pid down */
  if (opt_pidns_fd != -1)
    {
      /* Mark us as a subreaper, this way we can get exit status from grandchildren */
      prctl (PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0);
      create_pid_socketpair (intermediate_pids_sockets);
    }

  pid = raw_clone (clone_flags, NULL);
  if (pid == -1)
    {
      if (opt_unshare_user)
        {
          if (errno == EINVAL)
            die ("Creating new namespace failed, likely because the kernel does not support user namespaces.  bwrap must be installed setuid on such systems.");
          else if (errno == EPERM && !is_privileged)
            die ("No permissions to create a new namespace, likely because the kernel does not allow non-privileged user namespaces. On e.g. debian this can be enabled with 'sysctl kernel.unprivileged_userns_clone=1'.");
        }

      if (errno == ENOSPC)
        die ("Creating new namespace failed: nesting depth or /proc/sys/user/max_*_namespaces exceeded (ENOSPC)");

      die_with_error ("Creating new namespace failed");
    }

  ns_uid = opt_sandbox_uid;
  ns_gid = opt_sandbox_gid;

  if (pid != 0)
    {
      /* Parent, outside sandbox, privileged (initially) */

      if (intermediate_pids_sockets[0] != -1)
        {
          close (intermediate_pids_sockets[1]);
          pid = read_pid_from_socket (intermediate_pids_sockets[0]);
          close (intermediate_pids_sockets[0]);
        }

      /* Discover namespace ids before we drop privileges */
      namespace_ids_read (pid);

      if (is_privileged && opt_unshare_user && opt_userns_block_fd == -1)
        {
          /* We're running as euid 0, but the uid we want to map is
           * not 0. This means we're not allowed to write this from
           * the child user namespace, so we do it from the parent.
           *
           * Also, we map uid/gid 0 in the namespace (to overflowuid)
           * if opt_needs_devpts is true, because otherwise the mount
           * of devpts fails due to root not being mapped.
           */
          write_uid_gid_map (ns_uid, real_uid,
                             ns_gid, real_gid,
                             pid, true, opt_needs_devpts);
        }

      /* Initial launched process, wait for pid 1 or exec:ed command to exit */

      if (opt_userns2_fd > 0 && setns (opt_userns2_fd, CLONE_NEWUSER) != 0)
        die_with_error ("Setting userns2 failed");

      /* We don't need any privileges in the launcher, drop them immediately. */
      drop_privs (false, false);

      /* Optionally bind our lifecycle to that of the parent */
      handle_die_with_parent ();

      if (opt_info_fd != -1)
        {
          cleanup_free char *output = xasprintf ("{\n    \"child-pid\": %i", pid);
          dump_info (opt_info_fd, output, true);
          namespace_ids_write (opt_info_fd, false);
          dump_info (opt_info_fd, "\n}\n", true);
          close (opt_info_fd);
        }
      if (opt_json_status_fd != -1)
        {
          cleanup_free char *output = xasprintf ("{ \"child-pid\": %i", pid);
          dump_info (opt_json_status_fd, output, true);
          namespace_ids_write (opt_json_status_fd, true);
          dump_info (opt_json_status_fd, " }\n", true);
        }

      if (opt_userns_block_fd != -1)
        {
          char b[1];
          (void) TEMP_FAILURE_RETRY (read (opt_userns_block_fd, b, 1));
          close (opt_userns_block_fd);
        }

      /* Let child run now that the uid maps are set up */
      val = 1;
      res = TEMP_FAILURE_RETRY (write (child_wait_fd, &val, 8));
      /* Ignore res, if e.g. the child died and closed child_wait_fd we don't want to error out here */
      close (child_wait_fd);

      return monitor_child (event_fd, pid, setup_finished_pipe[0]);
    }

  if (opt_pidns_fd > 0)
    {
      if (setns (opt_pidns_fd, CLONE_NEWPID) != 0)
        die_with_error ("Setting pidns failed");

      /* fork to get the passed in pid ns */
      fork_intermediate_child ();

      /* We might both have specified an --pidns *and* --unshare-pid, so set up a new child pid namespace under the specified one */
      if (opt_unshare_pid)
        {
          if (unshare (CLONE_NEWPID))
            die_with_error ("unshare pid ns");

          /* fork to get the new pid ns */
          fork_intermediate_child ();
        }

      /* We're back, either in a child or grandchild, so message the actual pid to the monitor */

      close (intermediate_pids_sockets[0]);
      send_pid_on_socket (intermediate_pids_sockets[1]);
      close (intermediate_pids_sockets[1]);
    }

  /* Child, in sandbox, privileged in the parent or in the user namespace (if --unshare-user).
   *
   * Note that for user namespaces we run as euid 0 during clone(), so
   * the child user namespace is owned by euid 0., This means that the
   * regular user namespace parent (with uid != 0) doesn't have any
   * capabilities in it, which is nice as we can't exploit those. In
   * particular the parent user namespace doesn't have CAP_PTRACE
   * which would otherwise allow the parent to hijack of the child
   * after this point.
   *
   * Unfortunately this also means you can't ptrace the final
   * sandboxed process from outside the sandbox either.
   */

  if (opt_info_fd != -1)
    close (opt_info_fd);

  if (opt_json_status_fd != -1)
    close (opt_json_status_fd);

  /* Wait for the parent to init uid/gid maps and drop caps */
  res = read (child_wait_fd, &val, 8);
  close (child_wait_fd);

  /* At this point we can completely drop root uid, but retain the
   * required permitted caps. This allow us to do full setup as
   * the user uid, which makes e.g. fuse access work.
   */
  switch_to_user_with_privs ();

  if (opt_unshare_net)
    loopback_setup (); /* Will exit if unsuccessful */

  ns_uid = opt_sandbox_uid;
  ns_gid = opt_sandbox_gid;
  if (!is_privileged && opt_unshare_user && opt_userns_block_fd == -1)
    {
      /* In the unprivileged case we have to write the uid/gid maps in
       * the child, because we have no caps in the parent */

      if (opt_needs_devpts)
        {
          /* This is a bit hacky, but we need to first map the real uid/gid to
             0, otherwise we can't mount the devpts filesystem because root is
             not mapped. Later we will create another child user namespace and
             map back to the real uid */
          ns_uid = 0;
          ns_gid = 0;
        }

      write_uid_gid_map (ns_uid, real_uid,
                         ns_gid, real_gid,
                         -1, true, false);
    }

  old_umask = umask (0);

  /* Need to do this before the chroot, but after we're the real uid */
  resolve_symlinks_in_ops ();

  /* Mark everything as slave, so that we still
   * receive mounts from the real root, but don't
   * propagate mounts to the real root. */
  if (mount (NULL, "/", NULL, MS_SILENT | MS_SLAVE | MS_REC, NULL) < 0)
    die_with_mount_error ("Failed to make / slave");

  /* Create a tmpfs which we will use as / in the namespace */
  if (mount ("tmpfs", base_path, "tmpfs", MS_NODEV | MS_NOSUID, NULL) != 0)
    die_with_mount_error ("Failed to mount tmpfs");

  old_cwd = get_current_dir_name ();

  /* Chdir to the new root tmpfs mount. This will be the CWD during
     the entire setup. Access old or new root via "oldroot" and "newroot". */
  if (chdir (base_path) != 0)
    die_with_error ("chdir base_path");

  /* We create a subdir "$base_path/newroot" for the new root, that
   * way we can pivot_root to base_path, and put the old root at
   * "$base_path/oldroot". This avoids problems accessing the oldroot
   * dir if the user requested to bind mount something over / (or
   * over /tmp, now that we use that for base_path). */

  if (mkdir ("newroot", 0755))
    die_with_error ("Creating newroot failed");

  if (mount ("newroot", "newroot", NULL, MS_SILENT | MS_MGC_VAL | MS_BIND | MS_REC, NULL) < 0)
    die_with_mount_error ("setting up newroot bind");

  if (mkdir ("oldroot", 0755))
    die_with_error ("Creating oldroot failed");

  for (i = 0; i < opt_tmp_overlay_count; i++)
    {
      char *dirname;
      dirname = xasprintf ("tmp-overlay-upper-%d", i);
      if (mkdir (dirname, 0755))
        die_with_error ("Creating --tmp-overlay upperdir failed");
      free (dirname);
      dirname = xasprintf ("tmp-overlay-work-%d", i);
      if (mkdir (dirname, 0755))
        die_with_error ("Creating --tmp-overlay workdir failed");
      free (dirname);
    }

  if (pivot_root (base_path, "oldroot"))
    die_with_error ("pivot_root");

  if (chdir ("/") != 0)
    die_with_error ("chdir / (base path)");

  if (is_privileged)
    {
      pid_t child;
      int privsep_sockets[2];

      if (socketpair (AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0, privsep_sockets) != 0)
        die_with_error ("Can't create privsep socket");

      child = fork ();
      if (child == -1)
        die_with_error ("Can't fork unprivileged helper");

      if (child == 0)
        {
          /* Unprivileged setup process */
          drop_privs (false, true);
          close (privsep_sockets[0]);
          setup_newroot (opt_unshare_pid, privsep_sockets[1]);
          exit (0);
        }
      else
        {
          int status;
          uint32_t buffer[2048];  /* 8k, but is int32 to guarantee nice alignment */
          uint32_t op, flags, perms;
          size_t size_arg;
          const char *arg1, *arg2;
          cleanup_fd int unpriv_socket = -1;

          unpriv_socket = privsep_sockets[0];
          close (privsep_sockets[1]);

          do
            {
              op = read_priv_sec_op (unpriv_socket, buffer, sizeof (buffer),
                                     &flags, &perms, &size_arg, &arg1, &arg2);
              privileged_op (-1, op, flags, perms, size_arg, arg1, arg2);
              if (TEMP_FAILURE_RETRY (write (unpriv_socket, buffer, 1)) != 1)
                die ("Can't write to op_socket");
            }
          while (op != PRIV_SEP_OP_DONE);

          TEMP_FAILURE_RETRY (waitpid (child, &status, 0));
          /* Continue post setup */
        }
    }
  else
    {
      setup_newroot (opt_unshare_pid, -1);
    }

  close_ops_fd ();

  /* The old root better be rprivate or we will send unmount events to the parent namespace */
  if (mount ("oldroot", "oldroot", NULL, MS_SILENT | MS_REC | MS_PRIVATE, NULL) != 0)
    die_with_mount_error ("Failed to make old root rprivate");

  if (umount2 ("oldroot", MNT_DETACH))
    die_with_error ("unmount old root");

  /* This is our second pivot. It's like we're a Silicon Valley startup flush
   * with cash but short on ideas!
   *
   * We're aiming to make /newroot the real root, and get rid of /oldroot. To do
   * that we need a temporary place to store it before we can unmount it.
   */
  { cleanup_fd int oldrootfd = TEMP_FAILURE_RETRY (open ("/", O_DIRECTORY | O_RDONLY));
    if (oldrootfd < 0)
      die_with_error ("can't open /");
    if (chdir ("/newroot") != 0)
      die_with_error ("chdir /newroot");
    /* While the documentation claims that put_old must be underneath
     * new_root, it is perfectly fine to use the same directory as the
     * kernel checks only if old_root is accessible from new_root.
     *
     * Both runc and LXC are using this "alternative" method for
     * setting up the root of the container:
     *
     * https://github.com/opencontainers/runc/blob/HEAD/libcontainer/rootfs_linux.go#L671
     * https://github.com/lxc/lxc/blob/HEAD/src/lxc/conf.c#L1121
     */
    if (pivot_root (".", ".") != 0)
      die_with_error ("pivot_root(/newroot)");
    if (fchdir (oldrootfd) < 0)
      die_with_error ("fchdir to oldroot");
    if (umount2 (".", MNT_DETACH) < 0)
      die_with_error ("umount old root");
    if (chdir ("/") != 0)
      die_with_error ("chdir /");
  }

  if (opt_userns2_fd > 0 && setns (opt_userns2_fd, CLONE_NEWUSER) != 0)
    die_with_error ("Setting userns2 failed");

  if (opt_unshare_user && opt_userns_block_fd == -1 &&
      (ns_uid != opt_sandbox_uid || ns_gid != opt_sandbox_gid ||
       opt_disable_userns))
    {
      /* Here we create a second level userns inside the first one. This is
         used for one or more of these reasons:

         * The 1st level namespace has a different uid/gid than the
           requested due to requirements of beeing root in the first
           level due for mounting devpts (opt_needs_devpts).

         * To disable user namespaces we set max_user_namespaces and then
           create the second namespace so that the sandbox cannot undo this
           change.
      */

      if (opt_disable_userns)
        {
          cleanup_fd int sysctl_fd = -1;

          sysctl_fd = TEMP_FAILURE_RETRY (openat (proc_fd, "sys/user/max_user_namespaces", O_WRONLY));

          if (sysctl_fd < 0)
            die_with_error ("cannot open /proc/sys/user/max_user_namespaces");

          if (write_to_fd (sysctl_fd, "1", 1) < 0)
            die_with_error ("sysctl user.max_user_namespaces = 1");
        }

      if (unshare (CLONE_NEWUSER))
        die_with_error ("unshare user ns");

      /* We're in a new user namespace, we got back the bounding set, clear it again */
      drop_cap_bounding_set (false);

      write_uid_gid_map (opt_sandbox_uid, ns_uid,
                         opt_sandbox_gid, ns_gid,
                         -1, false, false);
    }

  if (opt_disable_userns || opt_assert_userns_disabled)
    {
      /* Verify that we can't make a new userns again */
      res = unshare (CLONE_NEWUSER);

      if (res == 0)
        die ("creation of new user namespaces was not disabled as requested");
    }

  /* All privileged ops are done now, so drop caps we don't need */
  drop_privs (!is_privileged, true);

  if (opt_block_fd != -1)
    {
      char b[1];
      (void) TEMP_FAILURE_RETRY (read (opt_block_fd, b, 1));
      close (opt_block_fd);
    }

  if (opt_seccomp_fd != -1)
    {
      assert (seccomp_programs == NULL);
      /* takes ownership of fd */
      seccomp_program_new (&opt_seccomp_fd);
    }

  umask (old_umask);

  new_cwd = "/";
  if (opt_chdir_path)
    {
      if (chdir (opt_chdir_path))
        die_with_error ("Can't chdir to %s", opt_chdir_path);
      new_cwd = opt_chdir_path;
    }
  else if (chdir (old_cwd) == 0)
    {
      /* If the old cwd is mapped in the sandbox, go there */
      new_cwd = old_cwd;
    }
  else
    {
      /* If the old cwd is not mapped, go to home */
      const char *home = getenv ("HOME");
      if (home != NULL &&
          chdir (home) == 0)
        new_cwd = home;
    }
  xsetenv ("PWD", new_cwd, 1);
  free (old_cwd);

  if (opt_new_session &&
      setsid () == (pid_t) -1)
    die_with_error ("setsid");

  if (label_exec (opt_exec_label) == -1)
    die_with_error ("label_exec %s", argv[0]);

  debug ("forking for child");

  if (!opt_as_pid_1 && (opt_unshare_pid || lock_files != NULL || opt_sync_fd != -1))
    {
      /* We have to have a pid 1 in the pid namespace, because
       * otherwise we'll get a bunch of zombies as nothing reaps
       * them. Alternatively if we're using sync_fd or lock_files we
       * need some process to own these.
       */

      pid = fork ();
      if (pid == -1)
        die_with_error ("Can't fork for pid 1");

      if (pid != 0)
        {
          drop_all_caps (false);

          /* Close fds in pid 1, except stdio and optionally event_fd
             (for syncing pid 2 lifetime with monitor_child) and
             opt_sync_fd (for syncing sandbox lifetime with outside
             process).
             Any other fds will been passed on to the child though. */
          {
            int dont_close[3];
            int j = 0;
            if (event_fd != -1)
              dont_close[j++] = event_fd;
            if (opt_sync_fd != -1)
              dont_close[j++] = opt_sync_fd;
            dont_close[j++] = -1;
            fdwalk (proc_fd, close_extra_fds, dont_close);
          }

          return do_init (event_fd, pid);
        }
    }

  debug ("launch executable %s", argv[0]);

  if (proc_fd != -1)
    close (proc_fd);

  /* If we are using --as-pid-1 leak the sync fd into the sandbox.
     --sync-fd will still work unless the container process doesn't close this file.  */
  if (!opt_as_pid_1)
    {
      if (opt_sync_fd != -1)
        close (opt_sync_fd);
    }

  /* We want sigchild in the child */
  unblock_sigchild ();

  /* Optionally bind our lifecycle */
  handle_die_with_parent ();

  if (!is_privileged)
    set_ambient_capabilities ();

  /* Should be the last thing before execve() so that filters don't
   * need to handle anything above */
  seccomp_programs_apply ();

  if (setup_finished_pipe[1] != -1)
    {
      char data = 0;
      res = write_to_fd (setup_finished_pipe[1], &data, 1);
      /* Ignore res, if e.g. the parent died and closed setup_finished_pipe[0]
         we don't want to error out here */
    }

  exec_path = argv[0];
  if (opt_argv0 != NULL)
    argv[0] = (char *) opt_argv0;

  if (execvp (exec_path, argv) == -1)
    {
      if (setup_finished_pipe[1] != -1)
        {
          int saved_errno = errno;
          char data = 0;
          res = write_to_fd (setup_finished_pipe[1], &data, 1);
          errno = saved_errno;
          /* Ignore res, if e.g. the parent died and closed setup_finished_pipe[0]
             we don't want to error out here */
        }
      die_with_error ("execvp %s", exec_path);
    }

  return 0;
}
