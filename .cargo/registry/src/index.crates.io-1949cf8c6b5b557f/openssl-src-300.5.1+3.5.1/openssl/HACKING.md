MODIFYING OPENSSL SOURCE
========================

This document describes the way to add custom modifications to OpenSSL
sources.

If you are adding new C source files
------------------------------------

Please update the `build.info` files in the directories where you placed the
C source files, to include a line like this for each new C source file:

-   In `crypto/` or any of its subdirectories (intended for `libcrypto`):

        SOURCE[../libcrypto]={name-of-C-source-file}

-   In `ssl/` or any of its subdirectories (intended for `libssl`):

        SOURCE[../libssl]={name-of-C-source-file}

Do note that the path given as the `SOURCE` attribute must be adapted
appropriately for the location of the `build.info` file, as it's a relative
path to where the library itself is built, for example:

-   For `crypto/build.info`, the library path should be `../libcrypto`
-   For `crypto/evp/build.info`, the library path should be
    `../../libcrypto`
-   For `ssl/build.info`, the library path should be `../libssl`
-   For `ssl/quic/build.info`, the library path should be `../../libssl`

To know more about `build.info` files, please read [doc/internal/man7/build.info.pod].
For better viewing, consider converting it to HTML or PDF using `pod2html`
or `pod2pdf`.

Adding new public functions
---------------------------

If you are adding new public functions to the custom library build, you need to
either add a prototype in one of the existing OpenSSL header files, or
provide a new header file and edit.

Only headers in the `include/openssl` subdirectory are considered for public
functions.  If you're creating a new header file, it must be located in that
directory.

Functions declared in `include/openssl` header files are assumed to be part
of the `libcrypto` library unless specified otherwise.  *If your new
functions are meant for the `libssl` library*, you will need to edit
[Configurations/unix-Makefile.tmpl] and add the header file name in the
array `my @sslheaders_tmpl`.

Updating OpenSSL's bookkeeping files
------------------------------------

OpenSSL has a few bookkeeping files to keep track of exposed functions, most
importantly `util/libcrypto.num` and `util/libssl.num`.  Any time a new
public function - as defined above - is added, these files must be updated.

To make such an update, please do the following:

    ./Configure -Werror --strict-warnings [your-options]
    make update

If you plan to submit the changes you made to OpenSSL (see
[CONTRIBUTING.md]), it's also worth running the following after running
`make update`, to ensure that documentation has correct format.

    make doc-nits

Do note that `make update` also generates files related to OIDs (in the
`crypto/objects/`  folder) and errors messages.

If a git merge error occurs in one of these generated files, then the
generated files need to be removed and regenerated using `make update`.
To aid in this process, the generated files can be committed separately
so they can be removed easily by reverting that commit.

[doc/internal/man7/build.info.pod]: ./doc/internal/man7/build.info.pod
[Configurations/unix-Makefile.tmpl]: ./Configurations/unix-Makefile.tmpl
[CONTRIBUTING.md]: ./CONTRIBUTING.md
