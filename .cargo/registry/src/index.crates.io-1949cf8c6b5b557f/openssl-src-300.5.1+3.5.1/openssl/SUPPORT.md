OpenSSL User Support resources
==============================

See the <https://www.openssl.org/support/contracts.html> for details on how to
obtain commercial technical support.

If you have general questions about using OpenSSL
-------------------------------------------------

In this case the [openssl-users] mailing list is the right place for you.
The list is not only watched by the OpenSSL team members, but also by many
other OpenSSL users. Here you will most likely get the answer to your questions.
An overview over the [mailing lists](#mailing-lists) can be found below.

If you think you found a Bug
----------------------------

*NOTE: this section assumes that you want to report it or figure it out and
fix it.  What's written here is not to be taken as a recipe for how to get a
working production installation*

If you have any problems with OpenSSL then please take the following steps
first:

- Search the mailing lists and/or the GitHub issues to find out whether
  the problem has already been reported.
- Download the latest version from the repository to see if the problem
  has already been addressed.
- Configure without assembler support (`no-asm`) and check whether the
  problem persists.
- Remove compiler optimization flags.

Please keep in mind: Just because something doesn't work the way you expect
does not mean it is necessarily a bug in OpenSSL. If you are not sure,
consider searching the mail archives and posting a question to the
[openssl-users] mailing list first.

### Open an Issue

If you wish to report a bug, please open an [issue][github-issues] on GitHub
and include the following information:

- OpenSSL version: output of `openssl version -a`
- Configuration data: output of `perl configdata.pm --dump`
- OS Name, Version, Hardware platform
- Compiler Details (name, version)
- Application Details (name, version)
- Problem Description (steps that will reproduce the problem, if known)
- Stack Traceback (if the application dumps core)

Not only errors in the software, also errors in the documentation, in
particular the manual pages, can be reported as issues.

### Submit a Pull Request

The fastest way to get a bug fixed is to fix it yourself ;-). If you are
experienced in programming and know how to fix the bug, you can open a
pull request. The details are covered in the [Contributing][contributing] section.

Don't hesitate to open a pull request, even if it's only a small change
like a grammatical or typographical error in the documentation.

Mailing Lists
=============

The OpenSSL maintains a number of [mailing lists] for various purposes.
The most important lists are:

- [openssl-users]    for general questions about using the OpenSSL software
                     and discussions between OpenSSL users.

- [openssl-announce] for official announcements to the OpenSSL community.

- [openssl-project]  for discussion about the development roadmap
                     and governance.

Only subscribers can post to [openssl-users] or [openssl-project].  The
archives are made public, however.  For more information, see the [mailing
lists] page.

There was an [openssl-dev] list that has been discontinued since development
is now taking place in the form of GitHub pull requests. Although not active
anymore, the searchable archive may still contain useful information.

<!-- Links -->

[mailing lists]:     https://www.openssl.org/community/mailinglists.html
[openssl-users]:     https://mta.openssl.org/mailman/listinfo/openssl-users
[openssl-announce]:  https://mta.openssl.org/mailman/listinfo/openssl-announce
[openssl-project]:   https://mta.openssl.org/mailman/listinfo/openssl-project
[openssl-dev]:       https://mta.openssl.org/mailman/listinfo/openssl-dev
[github-issues]:     https://github.com/openssl/openssl/issues/new/choose
[contributing]:      https://github.com/openssl/openssl/blob/master/CONTRIBUTING.md
