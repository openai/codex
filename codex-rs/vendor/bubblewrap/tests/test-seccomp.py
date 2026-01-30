#!/usr/bin/env python3
# Copyright 2021 Simon McVittie
# SPDX-License-Identifier: LGPL-2.0-or-later

import errno
import logging
import os
import subprocess
import sys
import tempfile
import termios
import unittest

try:
    import seccomp
except ImportError:
    print('1..0 # SKIP cannot import seccomp Python module')
    sys.exit(0)


# This is the @default set from systemd as of 2021-10-11
DEFAULT_SET = set('''
brk
cacheflush
clock_getres
clock_getres_time64
clock_gettime
clock_gettime64
clock_nanosleep
clock_nanosleep_time64
execve
exit
exit_group
futex
futex_time64
get_robust_list
get_thread_area
getegid
getegid32
geteuid
geteuid32
getgid
getgid32
getgroups
getgroups32
getpgid
getpgrp
getpid
getppid
getrandom
getresgid
getresgid32
getresuid
getresuid32
getrlimit
getsid
gettid
gettimeofday
getuid
getuid32
membarrier
mmap
mmap2
munmap
nanosleep
pause
prlimit64
restart_syscall
rseq
rt_sigreturn
sched_getaffinity
sched_yield
set_robust_list
set_thread_area
set_tid_address
set_tls
sigreturn
time
ugetrlimit
'''.split())

# This is the @basic-io set from systemd
BASIC_IO_SET = set('''
_llseek
close
close_range
dup
dup2
dup3
lseek
pread64
preadv
preadv2
pwrite64
pwritev
pwritev2
read
readv
write
writev
'''.split())

# This is the @filesystem-io set from systemd
FILESYSTEM_SET = set('''
access
chdir
chmod
close
creat
faccessat
faccessat2
fallocate
fchdir
fchmod
fchmodat
fcntl
fcntl64
fgetxattr
flistxattr
fremovexattr
fsetxattr
fstat
fstat64
fstatat64
fstatfs
fstatfs64
ftruncate
ftruncate64
futimesat
getcwd
getdents
getdents64
getxattr
inotify_add_watch
inotify_init
inotify_init1
inotify_rm_watch
lgetxattr
link
linkat
listxattr
llistxattr
lremovexattr
lsetxattr
lstat
lstat64
mkdir
mkdirat
mknod
mknodat
newfstatat
oldfstat
oldlstat
oldstat
open
openat
openat2
readlink
readlinkat
removexattr
rename
renameat
renameat2
rmdir
setxattr
stat
stat64
statfs
statfs64
statx
symlink
symlinkat
truncate
truncate64
unlink
unlinkat
utime
utimensat
utimensat_time64
utimes
'''.split())

# Miscellaneous syscalls used during process startup, at least on x86_64
ALLOWED = DEFAULT_SET | BASIC_IO_SET | FILESYSTEM_SET | set('''
arch_prctl
ioctl
madvise
mprotect
mremap
prctl
readdir
umask
'''.split())

# Syscalls we will try to use, expecting them to be either allowed or
# blocked by our allow and/or deny lists
TRY_SYSCALLS = [
    'chmod',
    'chroot',
    'clone3',
    'ioctl TIOCNOTTY',
    'ioctl TIOCSTI CVE-2019-10063',
    'ioctl TIOCSTI',
    'listen',
    'prctl',
]


class Test(unittest.TestCase):
    def setUp(self) -> None:
        here = os.path.dirname(os.path.abspath(__file__))

        if 'G_TEST_SRCDIR' in os.environ:
            self.test_srcdir = os.getenv('G_TEST_SRCDIR') + '/tests'
        else:
            self.test_srcdir = here

        if 'G_TEST_BUILDDIR' in os.environ:
            self.test_builddir = os.getenv('G_TEST_BUILDDIR') + '/tests'
        else:
            self.test_builddir = here

        self.bwrap = os.getenv('BWRAP', 'bwrap')
        self.try_syscall = os.path.join(self.test_builddir, 'try-syscall')

        completed = subprocess.run(
            [
                self.bwrap,
                '--ro-bind', '/', '/',
                'true',
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=2,
        )

        if completed.returncode != 0:
            raise unittest.SkipTest(
                'cannot run bwrap (does it need to be setuid?)'
            )

    def tearDown(self) -> None:
        pass

    def test_no_seccomp(self) -> None:
        for syscall in TRY_SYSCALLS:
            print('# {} without seccomp'.format(syscall))
            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    self.try_syscall, syscall,
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=2,
            )

            if (
                syscall == 'ioctl TIOCSTI CVE-2019-10063'
                and completed.returncode == errno.ENOENT
            ):
                print('# Cannot test 64-bit syscall parameter on 32-bit')
                continue

            if syscall == 'clone3':
                # If the kernel supports it, we didn't block it so
                # it fails with EFAULT. If the kernel doesn't support it,
                # it'll fail with ENOSYS instead.
                self.assertIn(
                    completed.returncode,
                    (errno.ENOSYS, errno.EFAULT),
                )
            elif syscall.startswith('ioctl') or syscall == 'listen':
                self.assertEqual(completed.returncode, errno.EBADF)
            else:
                self.assertEqual(completed.returncode, errno.EFAULT)

    def test_seccomp_allowlist(self) -> None:
        with tempfile.TemporaryFile() as allowlist_temp:
            allowlist = seccomp.SyscallFilter(seccomp.ERRNO(errno.ENOSYS))

            if os.uname().machine == 'x86_64':
                # Allow Python and try-syscall to be different word sizes
                allowlist.add_arch(seccomp.Arch.X86)

            for syscall in ALLOWED:
                try:
                    allowlist.add_rule(seccomp.ALLOW, syscall)
                except Exception as e:
                    print('# Cannot add {} to allowlist: {!r}'.format(syscall, e))

            allowlist.export_bpf(allowlist_temp)

            for syscall in TRY_SYSCALLS:
                print('# allowlist vs. {}'.format(syscall))
                allowlist_temp.seek(0, os.SEEK_SET)

                completed = subprocess.run(
                    [
                        self.bwrap,
                        '--ro-bind', '/', '/',
                        '--seccomp', str(allowlist_temp.fileno()),
                        self.try_syscall, syscall,
                    ],
                    pass_fds=(allowlist_temp.fileno(),),
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=2,
                )

                if (
                    syscall == 'ioctl TIOCSTI CVE-2019-10063'
                    and completed.returncode == errno.ENOENT
                ):
                    print('# Cannot test 64-bit syscall parameter on 32-bit')
                    continue

                if syscall.startswith('ioctl'):
                    # We allow this, so it is executed (and in this simple
                    # example, immediately fails)
                    self.assertEqual(completed.returncode, errno.EBADF)
                elif syscall in ('chroot', 'listen', 'clone3'):
                    # We don't allow these, so they fail with ENOSYS.
                    # clone3 might also be failing with ENOSYS because
                    # the kernel genuinely doesn't support it.
                    self.assertEqual(completed.returncode, errno.ENOSYS)
                else:
                    # We allow this, so it is executed (and in this simple
                    # example, immediately fails)
                    self.assertEqual(completed.returncode, errno.EFAULT)

    def test_seccomp_denylist(self) -> None:
        with tempfile.TemporaryFile() as denylist_temp:
            denylist = seccomp.SyscallFilter(seccomp.ALLOW)

            if os.uname().machine == 'x86_64':
                # Allow Python and try-syscall to be different word sizes
                denylist.add_arch(seccomp.Arch.X86)

            # Using ECONNREFUSED here because it's unlikely that any of
            # these syscalls will legitimately fail with that code, so
            # if they fail like this, it will be as a result of seccomp.
            denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'chmod')
            denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'chroot')
            denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'prctl')
            denylist.add_rule(
                seccomp.ERRNO(errno.ECONNREFUSED), 'ioctl',
                seccomp.Arg(1, seccomp.MASKED_EQ, 0xffffffff, termios.TIOCSTI),
            )

            denylist.export_bpf(denylist_temp)

            for syscall in TRY_SYSCALLS:
                print('# denylist vs. {}'.format(syscall))
                denylist_temp.seek(0, os.SEEK_SET)

                completed = subprocess.run(
                    [
                        self.bwrap,
                        '--ro-bind', '/', '/',
                        '--seccomp', str(denylist_temp.fileno()),
                        self.try_syscall, syscall,
                    ],
                    pass_fds=(denylist_temp.fileno(),),
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=2,
                )

                if (
                    syscall == 'ioctl TIOCSTI CVE-2019-10063'
                    and completed.returncode == errno.ENOENT
                ):
                    print('# Cannot test 64-bit syscall parameter on 32-bit')
                    continue

                if syscall == 'clone3':
                    # If the kernel supports it, we didn't block it so
                    # it fails with EFAULT. If the kernel doesn't support it,
                    # it'll fail with ENOSYS instead.
                    self.assertIn(
                        completed.returncode,
                        (errno.ENOSYS, errno.EFAULT),
                    )
                elif syscall in ('ioctl TIOCNOTTY', 'listen'):
                    # Not on the denylist
                    self.assertEqual(completed.returncode, errno.EBADF)
                else:
                    # We blocked all of these
                    self.assertEqual(completed.returncode, errno.ECONNREFUSED)

    def test_seccomp_stacked(self, allowlist_first=False) -> None:
        with tempfile.TemporaryFile(
        ) as allowlist_temp, tempfile.TemporaryFile(
        ) as denylist_temp:
            # This filter is a simplified version of what Flatpak wants

            allowlist = seccomp.SyscallFilter(seccomp.ERRNO(errno.ENOSYS))
            denylist = seccomp.SyscallFilter(seccomp.ALLOW)

            if os.uname().machine == 'x86_64':
                # Allow Python and try-syscall to be different word sizes
                allowlist.add_arch(seccomp.Arch.X86)
                denylist.add_arch(seccomp.Arch.X86)

            for syscall in ALLOWED:
                try:
                    allowlist.add_rule(seccomp.ALLOW, syscall)
                except Exception as e:
                    print('# Cannot add {} to allowlist: {!r}'.format(syscall, e))

            denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'chmod')
            denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'chroot')
            denylist.add_rule(
                seccomp.ERRNO(errno.ECONNREFUSED), 'ioctl',
                seccomp.Arg(1, seccomp.MASKED_EQ, 0xffffffff, termios.TIOCSTI),
            )

            # All seccomp programs except the last must allow prctl(),
            # because otherwise we wouldn't be able to add the remaining
            # seccomp programs. We document that the last program can
            # block prctl, so test that.
            if allowlist_first:
                denylist.add_rule(seccomp.ERRNO(errno.ECONNREFUSED), 'prctl')

            allowlist.export_bpf(allowlist_temp)
            denylist.export_bpf(denylist_temp)

            for syscall in TRY_SYSCALLS:
                print('# stacked vs. {}'.format(syscall))
                allowlist_temp.seek(0, os.SEEK_SET)
                denylist_temp.seek(0, os.SEEK_SET)

                if allowlist_first:
                    fds = [allowlist_temp.fileno(), denylist_temp.fileno()]
                else:
                    fds = [denylist_temp.fileno(), allowlist_temp.fileno()]

                completed = subprocess.run(
                    [
                        self.bwrap,
                        '--ro-bind', '/', '/',
                        '--add-seccomp-fd', str(fds[0]),
                        '--add-seccomp-fd', str(fds[1]),
                        self.try_syscall, syscall,
                    ],
                    pass_fds=fds,
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=2,
                )

                if (
                    syscall == 'ioctl TIOCSTI CVE-2019-10063'
                    and completed.returncode == errno.ENOENT
                ):
                    print('# Cannot test 64-bit syscall parameter on 32-bit')
                    continue

                if syscall == 'ioctl TIOCNOTTY':
                    # Not denied by the denylist, and allowed by the allowlist
                    self.assertEqual(completed.returncode, errno.EBADF)
                elif syscall in ('clone3', 'listen'):
                    # We didn't deny these, so the denylist has no effect
                    # and we fall back to the allowlist, which doesn't
                    # include them either.
                    # clone3 might also be failing with ENOSYS because
                    # the kernel genuinely doesn't support it.
                    self.assertEqual(completed.returncode, errno.ENOSYS)
                elif syscall == 'chroot':
                    # This is denied by the denylist *and* not allowed by
                    # the allowlist. The result depends which one we added
                    # first: the most-recently-added filter "wins".
                    if allowlist_first:
                        self.assertEqual(
                            completed.returncode,
                            errno.ECONNREFUSED,
                        )
                    else:
                        self.assertEqual(completed.returncode, errno.ENOSYS)
                elif syscall == 'prctl':
                    # We can only put this on the denylist if the denylist
                    # is the last to be added.
                    if allowlist_first:
                        self.assertEqual(
                            completed.returncode,
                            errno.ECONNREFUSED,
                        )
                    else:
                        self.assertEqual(completed.returncode, errno.EFAULT)
                else:
                    # chmod is allowed by the allowlist but blocked by the
                    # denylist. Denying takes precedence over allowing,
                    # regardless of order.
                    self.assertEqual(completed.returncode, errno.ECONNREFUSED)

    def test_seccomp_stacked_allowlist_first(self) -> None:
        self.test_seccomp_stacked(allowlist_first=True)

    def test_seccomp_invalid(self) -> None:
        with tempfile.TemporaryFile(
        ) as allowlist_temp, tempfile.TemporaryFile(
        ) as denylist_temp:
            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--add-seccomp-fd', '-1',
                    'true',
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(b'bwrap: Invalid fd: -1\n', completed.stderr)
            self.assertEqual(completed.returncode, 1)

            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--seccomp', '0a',
                    'true',
                ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(b'bwrap: Invalid fd: 0a\n', completed.stderr)
            self.assertEqual(completed.returncode, 1)

            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--add-seccomp-fd', str(denylist_temp.fileno()),
                    '--seccomp', str(allowlist_temp.fileno()),
                    'true',
                ],
                pass_fds=(allowlist_temp.fileno(), denylist_temp.fileno()),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(
                b'bwrap: --seccomp cannot be combined with --add-seccomp-fd\n',
                completed.stderr,
            )
            self.assertEqual(completed.returncode, 1)

            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--seccomp', str(allowlist_temp.fileno()),
                    '--add-seccomp-fd', str(denylist_temp.fileno()),
                    'true',
                ],
                pass_fds=(allowlist_temp.fileno(), denylist_temp.fileno()),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(
                b'--add-seccomp-fd cannot be combined with --seccomp',
                completed.stderr,
            )
            self.assertEqual(completed.returncode, 1)

            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--add-seccomp-fd', str(allowlist_temp.fileno()),
                    '--add-seccomp-fd', str(allowlist_temp.fileno()),
                    'true',
                ],
                pass_fds=(allowlist_temp.fileno(), allowlist_temp.fileno()),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(
                b"bwrap: Can't read seccomp data: ",
                completed.stderr,
            )
            self.assertEqual(completed.returncode, 1)

            allowlist_temp.write(b'\x01')
            allowlist_temp.seek(0, os.SEEK_SET)
            completed = subprocess.run(
                [
                    self.bwrap,
                    '--ro-bind', '/', '/',
                    '--add-seccomp-fd', str(denylist_temp.fileno()),
                    '--add-seccomp-fd', str(allowlist_temp.fileno()),
                    'true',
                ],
                pass_fds=(allowlist_temp.fileno(), denylist_temp.fileno()),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.PIPE,
            )
            self.assertIn(
                b'bwrap: Invalid seccomp data, must be multiple of 8\n',
                completed.stderr,
            )
            self.assertEqual(completed.returncode, 1)


def main():
    logging.basicConfig(level=logging.DEBUG)

    try:
        from tap.runner import TAPTestRunner
    except ImportError:
        TAPTestRunner = None    # type: ignore

    if TAPTestRunner is not None:
        runner = TAPTestRunner()
        runner.set_stream(True)
        unittest.main(testRunner=runner)
    else:
        print('# tap.runner not available, using simple TAP output')
        print('1..1')
        program = unittest.main(exit=False)
        if program.result.wasSuccessful():
            print('ok 1 - %r' % program.result)
        else:
            print('not ok 1 - %r' % program.result)
            sys.exit(1)

if __name__ == '__main__':
    main()
