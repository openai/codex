Bubblewrap
==========

Many container runtime tools like `systemd-nspawn`, `docker`,
etc. focus on providing infrastructure for system administrators and
orchestration tools (e.g. Kubernetes) to run containers.

These tools are not suitable to give to unprivileged users, because it
is trivial to turn such access into a fully privileged root shell
on the host.

User namespaces
---------------

There is an effort in the Linux kernel called
[user namespaces](https://www.google.com/search?q=user+namespaces+site%3Ahttps%3A%2F%2Flwn.net)
which attempts to allow unprivileged users to use container features.
While significant progress has been made, there are
[still concerns](https://lwn.net/Articles/673597/) about it, and
it is not available to unprivileged users in several production distributions
such as CentOS/Red Hat Enterprise Linux 7, Debian Jessie, etc.

See for example
[CVE-2016-3135](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2016-3135)
which is a local root vulnerability introduced by userns.
[This March 2016 post](https://lkml.org/lkml/2016/3/9/555) has some
more discussion.

Bubblewrap could be viewed as setuid implementation of a *subset* of
user namespaces.  Emphasis on subset - specifically relevant to the
above CVE, bubblewrap does not allow control over iptables.

The original bubblewrap code existed before user namespaces - it inherits code from
[xdg-app helper](https://cgit.freedesktop.org/xdg-app/xdg-app/tree/common/xdg-app-helper.c?id=4c3bf179e2e4a2a298cd1db1d045adaf3f564532)
which in turn distantly derives from
[linux-user-chroot](https://git.gnome.org/browse/linux-user-chroot).

System security
---------------

The maintainers of this tool believe that it does not, even when used
in combination with typical software installed on that distribution,
allow privilege escalation.  It may increase the ability of a logged
in user to perform denial of service attacks, however.

In particular, bubblewrap uses `PR_SET_NO_NEW_PRIVS` to turn off
setuid binaries, which is the [traditional way](https://en.wikipedia.org/wiki/Chroot#Limitations) to get out of things
like chroots.

Sandbox security
----------------

bubblewrap is a tool for constructing sandbox environments.
bubblewrap is not a complete, ready-made sandbox with a specific security
policy.

Some of bubblewrap's use-cases want a security boundary between the sandbox
and the real system; other use-cases want the ability to change the layout of
the filesystem for processes inside the sandbox, but do not aim to be a
security boundary.
As a result, the level of protection between the sandboxed processes and
the host system is entirely determined by the arguments passed to
bubblewrap.

Whatever program constructs the command-line arguments for bubblewrap
(often a larger framework like Flatpak, libgnome-desktop, sandwine
or an ad-hoc script) is responsible for defining its own security model,
and choosing appropriate bubblewrap command-line arguments to implement
that security model.

Some aspects of sandbox security that require particular care are described
in the [Limitations](#limitations) section below.

Users
-----

This program can be shared by all container tools which perform
non-root operation, such as:

 - [Flatpak](https://www.flatpak.org)
 - [rpm-ostree unprivileged](https://github.com/projectatomic/rpm-ostree/pull/209)
 - [bwrap-oci](https://github.com/projectatomic/bwrap-oci)

We would also like to see this be available in Kubernetes/OpenShift
clusters.  Having the ability for unprivileged users to use container
features would make it significantly easier to do interactive
debugging scenarios and the like.

Installation
------------

bubblewrap is available in the package repositories of the most Linux distributions
and can be installed from there.

If you need to build bubblewrap from source, you can do this with meson:

```sh
meson _builddir
meson compile -C _builddir
meson test -C _builddir
meson install -C _builddir
```

Usage
-----

bubblewrap works by creating a new, completely empty, mount
namespace where the root is on a tmpfs that is invisible from the
host, and will be automatically cleaned up when the last process
exits. You can then use commandline options to construct the root
filesystem and process environment and command to run in the
namespace.

There's a larger [demo script](./demos/bubblewrap-shell.sh) in the
source code, but here's a trimmed down version which runs
a new shell reusing the host's `/usr`.

```
bwrap \
    --ro-bind /usr /usr \
    --symlink usr/lib64 /lib64 \
    --proc /proc \
    --dev /dev \
    --unshare-pid \
    --new-session \
    bash
```

This is an incomplete example, but useful for purposes of
illustration.  More often, rather than creating a container using the
host's filesystem tree, you want to target a chroot.  There, rather
than creating the symlink `lib64 -> usr/lib64` in the tmpfs, you might
have already created it in the target rootfs.

Sandboxing
----------

The goal of bubblewrap is to run an application in a sandbox, where it
has restricted access to parts of the operating system or user data
such as the home directory.

bubblewrap always creates a new mount namespace, and the user can specify
exactly what parts of the filesystem should be visible in the sandbox.
Any such directories you specify mounted `nodev` by default, and can be made readonly.

Additionally you can use these kernel features:

User namespaces ([CLONE_NEWUSER](https://linux.die.net/man/2/clone)): This hides all but the current uid and gid from the
sandbox. You can also change what the value of uid/gid should be in the sandbox.

IPC namespaces ([CLONE_NEWIPC](https://linux.die.net/man/2/clone)): The sandbox will get its own copy of all the
different forms of IPCs, like SysV shared memory and semaphores.

PID namespaces ([CLONE_NEWPID](https://linux.die.net/man/2/clone)): The sandbox will not see any processes outside the sandbox. Additionally, bubblewrap will run a trivial pid1 inside your container to handle the requirements of reaping children in the sandbox. This avoids what is known now as the [Docker pid 1 problem](https://blog.phusion.nl/2015/01/20/docker-and-the-pid-1-zombie-reaping-problem/).


Network namespaces ([CLONE_NEWNET](https://linux.die.net/man/2/clone)): The sandbox will not see the network. Instead it will have its own network namespace with only a loopback device.

UTS namespace ([CLONE_NEWUTS](https://linux.die.net/man/2/clone)): The sandbox will have its own hostname.

Seccomp filters: You can pass in seccomp filters that limit which syscalls can be done in the sandbox. For more information, see [Seccomp](https://en.wikipedia.org/wiki/Seccomp).

Limitations
-----------

As noted in the [Sandbox security](#sandbox-security) section above,
the level of protection between the sandboxed processes and the host system
is entirely determined by the arguments passed to bubblewrap.
Some aspects that require special care are noted here.

- If you are not filtering out `TIOCSTI` commands using seccomp filters,
argument `--new-session` is needed to protect against out-of-sandbox
command execution
(see [CVE-2017-5226](https://github.com/containers/bubblewrap/issues/142)).

- Everything mounted into the sandbox can potentially be used to escalate
privileges.
For example, if you bind a D-Bus socket into the sandbox, it can be used to
execute commands via systemd. You can use
[xdg-dbus-proxy](https://github.com/flatpak/xdg-dbus-proxy) to filter
D-Bus communication.

- Some applications deploy their own sandboxing mechanisms, and these can be
restricted by the constraints imposed by bubblewrap's sandboxing.
For example, some web browsers which configure their child proccesses via
seccomp to not have access to the filesystem. If you limit the syscalls and
don't allow the seccomp syscall, a browser cannot apply these restrictions.
Similarly, if these rules were compiled into a file that is not available in
the sandbox, the browser cannot load these rules from this file and cannot
apply these restrictions.

Related project comparison: Firejail
------------------------------------

[Firejail](https://github.com/netblue30/firejail/tree/HEAD/src/firejail)
is similar to Flatpak before bubblewrap was split out in that it combines
a setuid tool with a lot of desktop-specific sandboxing features.  For
example, Firejail knows about Pulseaudio, whereas bubblewrap does not.

The bubblewrap authors believe it's much easier to audit a small
setuid program, and keep features such as Pulseaudio filtering as an
unprivileged process, as now occurs in Flatpak.

Also, @cgwalters thinks trying to
[whitelist file paths](https://github.com/netblue30/firejail/blob/37a5a3545ef6d8d03dad8bbd888f53e13274c9e5/src/firejail/fs_whitelist.c#L176)
is a bad idea given the myriad ways users have to manipulate paths,
and the myriad ways in which system administrators may configure a
system.  The bubblewrap approach is to only retain a few specific
Linux capabilities such as `CAP_SYS_ADMIN`, but to always access the
filesystem as the invoking uid.  This entirely closes
[TOCTTOU attacks](https://cwe.mitre.org/data/definitions/367.html) and
such.

Related project comparison: Sandstorm.io
----------------------------------------

[Sandstorm.io](https://sandstorm.io/) requires unprivileged user
namespaces to set up its sandbox, though it could easily be adapted
to operate in a setuid mode as well. @cgwalters believes their code is
fairly good, but it could still make sense to unify on bubblewrap.
However, @kentonv (of Sandstorm) feels that while this makes sense
in principle, the switching cost outweighs the practical benefits for
now. This decision could be re-evaluated in the future, but it is not
being actively pursued today.

Related project comparison: runc/binctr
----------------------------------------

[runC](https://github.com/opencontainers/runc) is currently working on
supporting [rootless containers](https://github.com/opencontainers/runc/pull/774),
without needing `setuid` or any other privileges during installation of
runC (using unprivileged user namespaces rather than `setuid`),
creation, and management of containers. However, the standard mode of
using runC is similar to [systemd nspawn](https://www.freedesktop.org/software/systemd/man/systemd-nspawn.html)
in that it is tooling intended to be invoked by root.

The bubblewrap authors believe that runc and systemd-nspawn are not
designed to be made setuid, and are distant from supporting such a mode.
However with rootless containers, runC will be able to fulfill certain usecases
that bubblewrap supports (with the added benefit of being a standardised and
complete OCI runtime).

[binctr](https://github.com/jfrazelle/binctr) is just a wrapper for
runC, so inherits all of its design tradeoffs.

What's with the name?!
----------------------

The name bubblewrap was chosen to convey that this
tool runs as the parent of the application (so wraps it in some sense) and creates
a protective layer (the sandbox) around it.

![](bubblewrap.jpg)

(Bubblewrap cat by [dancing_stupidity](https://www.flickr.com/photos/27549668@N03/))
