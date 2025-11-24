$! Quick script to check how well including individual header files works
$! on VMS, even when the VMS macro isn't defined.
$
$	sav_def = f$env("DEFAULT")
$	here = f$parse("A.;0",f$ENV("PROCEDURE")) - "A.;0"
$	set default 'here'
$	set default [-.include.openssl]
$	define openssl 'f$env("DEFAULT")'
$	set default [--]
$
$ loop:
$	f = f$search("openssl:*.h")
$	if f .eqs. "" then goto loop_end
$	write sys$output "Checking ",f
$	open/write foo foo.c
$	write foo "#undef VMS"
$	write foo "#include <stdio.h>"
$	write foo "#include <openssl/",f$parse(f,,,"NAME"),".h>"
$	write foo "main()"
$	write foo "{printf(""foo\n"");}"
$	close foo
$	cc/STANDARD=ANSI89/NOLIST/PREFIX=ALL foo.c
$	delete foo.c;
$	goto loop
$ loop_end:
$	set default 'save_def'
$	exit

