$	! OpenSSL config: determine the architecture and run Configure
$	! Copyright 2016-2020 The OpenSSL Project Authors. All Rights Reserved.
$	!
$	! Licensed under the Apache License 2.0 (the "License").  You may not use
$	! this file except in compliance with the License.  You can obtain a
$	! copy in the file LICENSE in the source distribution or at
$	! https://www.openssl.org/source/license.html
$	!
$	! Very simple for the moment, it will take the following arguments:
$	!
$	! -32 or 32	sets /POINTER_SIZE=32
$	! -64 or 64	sets /POINTER_SIZE=64
$	! -d		sets debugging
$	! -h		prints a usage and exits
$	! -t		test mode, doesn't run Configure
$
$	arch = f$edit( f$getsyi( "arch_name"), "lowercase")
$	pointer_size = ""
$	dryrun = 0
$	verbose = 0
$	here = F$PARSE("A.;",F$ENVIRONMENT("PROCEDURE"),,,"SYNTAX_ONLY") - "A.;"
$
$	collected_args = ""
$	P_index = 0
$	LOOP1:
$	    P_index = P_index + 1
$	    IF P_index .GT. 8 THEN GOTO ENDLOOP1
$	    P = F$EDIT(P1,"TRIM,LOWERCASE")
$	    IF P .EQS. "-h"
$           THEN
$               dryrun = 1
$               P = ""
$               TYPE SYS$INPUT
$               DECK
Usage: @config [options]

  -32 or 32	Build with 32-bit pointer size.
  -64 or 64	Build with 64-bit pointer size.
  -d		Build with debugging.
  -t            Test mode, do not run the Configure perl script.
  -v            Verbose mode, show the exact Configure call that is being made.
  -h		This help.

Any other text will be passed to the Configure perl script.
See INSTALL.md for instructions.

$               EOD
$           ENDIF
$	    IF P .EQS. "-t"
$	    THEN
$		dryrun = 1
$		verbose = 1
$		P = ""
$	    ENDIF
$	    IF P .EQS. "-v"
$	    THEN
$		verbose = 1
$		P = ""
$	    ENDIF
$	    IF P .EQS. "-32" .OR. P .EQS. "32"
$	    THEN
$		pointer_size = "-P32"
$		P = ""
$	    ENDIF
$	    IF P .EQS. "-64" .OR. P .EQS. "64"
$	    THEN
$		pointer_size = "-P64"
$		P = ""
$	    ENDIF
$	    IF P .EQS. "-d"
$	    THEN
$               collected_args = collected_args + " --debug"
$		P = ""
$	    ENDIF
$	    IF P .NES. "" THEN -
	       collected_args = collected_args + " """ + P1 + """"
$	    P1 = P2
$	    P2 = P3
$	    P3 = P4
$	    P4 = P5
$	    P5 = P6
$	    P6 = P7
$	    P7 = P8
$	    P8 = ""
$	    GOTO LOOP1
$	ENDLOOP1:
$
$	target = "vms-''arch'''pointer_size'"
$       IF verbose THEN -
           WRITE SYS$OUTPUT "PERL ''here'Configure ""''target'""",collected_args
$       IF .not. dryrun THEN -
           PERL 'here'Configure "''target'"'collected_args'
$       EXIT $STATUS
