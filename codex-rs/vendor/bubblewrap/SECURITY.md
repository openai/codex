## Security and Disclosure Information Policy for the bubblewrap Project

The bubblewrap Project follows the [Security and Disclosure Information Policy](https://github.com/containers/common/blob/HEAD/SECURITY.md) for the Containers Projects.

### System security

If bubblewrap is setuid root, then the goal is that it does not allow
a malicious local user to do anything that would not have been possible
on a kernel that allows unprivileged users to create new user namespaces.
For example, [CVE-2020-5291](https://github.com/containers/bubblewrap/security/advisories/GHSA-j2qp-rvxj-43vj)
was treated as a security vulnerability in bubblewrap.

If bubblewrap is not setuid root, then it is not a security boundary
between the user and the OS, because anything bubblewrap could do, a
malicious user could equally well do by writing their own tool equivalent
to bubblewrap.

### Sandbox security

bubblewrap is a toolkit for constructing sandbox environments.
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

For example,
[CVE-2017-5226](https://github.com/flatpak/flatpak/security/advisories/GHSA-7gfv-rvfx-h87x)
(in which a Flatpak app could send input to a parent terminal using the
`TIOCSTI` ioctl) is considered to be a Flatpak vulnerability, not a
bubblewrap vulnerability.
