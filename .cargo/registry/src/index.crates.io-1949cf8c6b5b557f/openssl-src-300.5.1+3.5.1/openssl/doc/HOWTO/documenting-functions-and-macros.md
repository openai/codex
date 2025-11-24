Documenting public Functions and Macros
=======================================

In the last few years, the OpenSSL project has strived to improve the quality
and coverage of the API documentation. A while ago, this goal has been
turned into an official [documentation-policy]. This policy is actively
enforced by the `make doc-nits` target resp. `check-docs` GitHub action.

[documentation-policy]: https://www.openssl.org/policies/technical/documentation-policy.html

If you add a new public function or macro to a header file without documenting
it, it will give you an error message like this:

```text
include/openssl/bio.h: macro BIO_set_dgram_origin(3) undocumented
include/openssl/bio.h: macro BIO_get_dgram_origin(3) undocumented
include/openssl/bio.h: macro BIO_set_dgram_dest(3) undocumented
include/openssl/bio.h: macro BIO_get_dgram_dest(3) undocumented
```

and you'll want to document this.

So, create a new `.pod` file named `doc/man3/FUNCTION.pod`.

If you are asked to document several related functions in that file,
you can create a single pod file in which you document them together.
In this case, use the name of the first function as the file name,
like for the above example:

```text
doc/man3/BIO_set_dgram_origin.pod
```

If you do use an unrelated name (like `BIO_dgram.pod`) then you'll get
a warning about that.

Next, you need to add your new file to the `doc/build.info` file.
This command does it automatically for you:

```console
$ make generate_doc_buildinfo
```

this will update `doc/build.info`.
You should git add the result as `generate_doc_buildinfo` is not run on every build.

With these two changes, running `make doc-nits` locally should
now agree with you that you have documented all your new defines,
but it might then complain:

```text
BIO_get_dgram_dest(3) is supposedly internal
(maybe missing from other.syms) but is documented as public
```

If it is the case that your interface is meant to be public, then you need
to edit the file `util/other.syms` to add the names of your `#define`
functions.
This file gets sorted alphabetically prior to each major release,
but new additions should be placed at the end of the file.

Example
-------

For demonstration purposes, two new public symbols have been added
by "implementing" a public function `BIO_set_dgram_foo()`
and a public function-like macro `BIO_set_dgram_bar()`:

```diff
diff --git a/crypto/bio/bss_dgram.c b/crypto/bio/bss_dgram.c
index 82d382cf4e..30382f0abe 100644
--- a/crypto/bio/bss_dgram.c
+++ b/crypto/bio/bss_dgram.c
@@ -192,6 +192,13 @@ BIO *BIO_new_dgram(int fd, int close_flag)
     return ret;
 }

+
+int BIO_set_dgram_foo(BIO* b, int foo)
+{
+    return foo;
+}
+
+
 static int dgram_new(BIO *bi)
 {
     bio_dgram_data *data = OPENSSL_zalloc(sizeof(*data));
diff --git a/include/openssl/bio.h.in b/include/openssl/bio.h.in
index c70185db34..4ddea2f96b 100644
--- a/include/openssl/bio.h.in
+++ b/include/openssl/bio.h.in
@@ -485,6 +485,9 @@ struct bio_dgram_sctp_prinfo {
 #define BIO_set_dgram_dest(b, addr)   BIO_ctrl(b, BIO_CTRL_DGRAM_SET_PEER, 0, addr)
 #define BIO_get_dgram_dest(b, addr)   BIO_ctrl(b, BIO_CTRL_DGRAM_GET_PEER, 0, addr)

+int BIO_set_dgram_foo(BIO* b, int foo);
+#define BIO_set_dgram_bar(b, bar) BIO_ctrl(b, BIO_CTRL_DGRAM_SET_ADDR, 0, bar)
+
 /*
  * name is cast to lose const, but might be better to route through a
  * function so we can do it safely
```

If you run `make doc-nits`, you might be surprised that it only
complains about the undocumented macro, not the function:

```console
$ make doc-nits

/usr/bin/perl ./util/find-doc-nits -c -n -l -e
include/openssl/bio.h: macro BIO_set_dgram_bar(3) undocumented
# 1 macros undocumented (count is approximate)
make: *** [Makefile:3833: doc-nits] Error 1
```

The explanation for this is that one important step is still missing,
it needs to be done first: you need to run

```console
$ make update
```

which triggers a scan of the public headers for new API functions.

All new functions will be added to either `util/libcrypto.num`
or `util/libssl.num`.
Those files store the information about the symbols which need
to be exported from the shared library resp. DLL.
Among other stuff, they contain the ordinal numbers for the
[module definition file] of the Windows DLL, which is the
reason for the `.num` extension.

[module definition file]: https://docs.microsoft.com/en-us/cpp/build/exporting-from-a-dll-using-def-files

After running `make update`, you can use `git diff` to check the outcome:

```diff
diff --git a/util/libcrypto.num b/util/libcrypto.num
index 394f454732..fc3c67313a 100644
--- a/util/libcrypto.num
+++ b/util/libcrypto.num
@@ -5437,3 +5437,4 @@ BN_signed_bn2native                     ? 3_1_0   EXIST::FUNCTION:
 ASYNC_set_mem_functions                 ?  3_1_0   EXIST::FUNCTION:
 ASYNC_get_mem_functions                 ?  3_1_0   EXIST::FUNCTION:
 BIO_ADDR_dup                            ?  3_1_0   EXIST::FUNCTION:SOCK
+BIO_set_dgram_foo                       ?  3_1_0   EXIST::FUNCTION:
```

The changes need to be committed, ideally as a separate commit:

```console
$ git commit -a -m "make update"
```

which has the advantage that it can easily be discarded when it
becomes necessary to rerun `make update`.

Finally, we reached the point where `make doc-nits` complains about
both symbols:

```console
$ make doc-nits
/usr/bin/perl ./util/find-doc-nits -c -n -l -e
crypto: function BIO_set_dgram_foo(3) undocumented
# 1 libcrypto names are not documented
include/openssl/bio.h: macro BIO_set_dgram_bar(3) undocumented
# 1 macros undocumented (count is approximate)
make: *** [Makefile:3833: doc-nits] Error 1
```

Additionally, public symbols added should contain an entry in the HISTORY
section of their documentation explaining the exact OpenSSL version in which
they have appeared for the first time. The option -i for "find-doc-nits"
can be utilized to check for this. A completely new documentation file
should also contain a HISTORY section with wording along this line, e.g.
"These functions have been added in OpenSSL version xxx.".

Summary
-------

The bottom line is that only the way how the public symbols
are recorded is different between functions and macros,
the rest of the documentation procedure is analogous.
