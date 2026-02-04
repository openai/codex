/*
 * Copyright 2021 Simon McVittie
 * SPDX-License-Identifier: LGPL-2.0-or-later
 *
 * Try one or more system calls that might have been blocked by a
 * seccomp filter. Return the last value of errno seen.
 *
 * In general, we pass a bad fd or pointer to each syscall that will
 * accept one, so that it will fail with EBADF or EFAULT without side-effects.
 *
 * This helper is used for regression tests in both bubblewrap and flatpak.
 * Please keep both copies in sync.
 */

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/stat.h>
#include <sys/types.h>

#if defined(_MIPS_SIM)
# if _MIPS_SIM == _ABIO32
#   define MISSING_SYSCALL_BASE 4000
# elif _MIPS_SIM == _ABI64
#   define MISSING_SYSCALL_BASE 5000
# elif _MIPS_SIM == _ABIN32
#   define MISSING_SYSCALL_BASE 6000
# else
#   error "Unknown MIPS ABI"
# endif
#endif

#if defined(__ia64__)
# define MISSING_SYSCALL_BASE 1024
#endif

#if defined(__alpha__)
# define MISSING_SYSCALL_BASE 110
#endif

#if defined(__x86_64__) && defined(__ILP32__)
# define MISSING_SYSCALL_BASE 0x40000000
#endif

/*
 * MISSING_SYSCALL_BASE:
 *
 * Number to add to the syscall numbers of recently-added syscalls
 * to get the appropriate syscall for the current ABI.
 */
#ifndef MISSING_SYSCALL_BASE
# define MISSING_SYSCALL_BASE 0
#endif

#ifndef __NR_clone3
# define __NR_clone3 (MISSING_SYSCALL_BASE + 435)
#endif

/*
 * The size of clone3's parameter (as of 2021)
 */
#define SIZEOF_STRUCT_CLONE_ARGS ((size_t) 88)

/*
 * An invalid pointer that will cause syscalls to fail with EFAULT
 */
#define WRONG_POINTER ((char *) 1)

#ifndef PR_GET_CHILD_SUBREAPER
#define PR_GET_CHILD_SUBREAPER 37
#endif

int
main (int argc, char **argv)
{
  int errsv = 0;
  int i;

  for (i = 1; i < argc; i++)
    {
      const char *arg = argv[i];

      if (strcmp (arg, "print-errno-values") == 0)
        {
          printf ("EBADF=%d\n", EBADF);
          printf ("EFAULT=%d\n", EFAULT);
          printf ("ENOENT=%d\n", ENOENT);
          printf ("ENOSYS=%d\n", ENOSYS);
          printf ("EPERM=%d\n", EPERM);
        }
      else if (strcmp (arg, "chmod") == 0)
        {
          /* If not blocked by seccomp, this will fail with EFAULT */
          if (chmod (WRONG_POINTER, 0700) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
      else if (strcmp (arg, "chroot") == 0)
        {
          /* If not blocked by seccomp, this will fail with EFAULT */
          if (chroot (WRONG_POINTER) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
      else if (strcmp (arg, "clone3") == 0)
        {
          /* If not blocked by seccomp, this will fail with EFAULT */
          if (syscall (__NR_clone3, WRONG_POINTER, SIZEOF_STRUCT_CLONE_ARGS) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
      else if (strcmp (arg, "ioctl TIOCNOTTY") == 0)
        {
          /* If not blocked by seccomp, this will fail with EBADF */
          if (ioctl (-1, TIOCNOTTY) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
      else if (strcmp (arg, "ioctl TIOCSTI") == 0)
        {
          /* If not blocked by seccomp, this will fail with EBADF */
          if (ioctl (-1, TIOCSTI, WRONG_POINTER) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
#ifdef __LP64__
      else if (strcmp (arg, "ioctl TIOCSTI CVE-2019-10063") == 0)
        {
          unsigned long not_TIOCSTI = (0x123UL << 32) | (unsigned long) TIOCSTI;

          /* If not blocked by seccomp, this will fail with EBADF */
          if (syscall (__NR_ioctl, -1, not_TIOCSTI, WRONG_POINTER) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
#endif
     else if (strcmp (arg, "listen") == 0)
        {
          /* If not blocked by seccomp, this will fail with EBADF */
          if (listen (-1, 42) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
     else if (strcmp (arg, "prctl") == 0)
        {
          /* If not blocked by seccomp, this will fail with EFAULT */
          if (prctl (PR_GET_CHILD_SUBREAPER, WRONG_POINTER, 0, 0, 0) != 0)
            {
              errsv = errno;
              perror (arg);
            }
        }
      else
        {
          fprintf (stderr, "Unsupported syscall \"%s\"\n", arg);
          errsv = ENOENT;
        }
   }

  return errsv;
}
