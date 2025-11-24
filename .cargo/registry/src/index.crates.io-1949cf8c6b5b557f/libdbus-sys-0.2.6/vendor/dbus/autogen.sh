#!/bin/sh
# Run this to generate all the initial makefiles, etc.

srcdir=`dirname $0`
test -z "$srcdir" && srcdir=.

ORIGDIR=`pwd`
cd $srcdir

PROJECT=dbus
TEST_TYPE=-f
FILE=dbus-1.pc.in

DIE=0

if [ -f .git/hooks/pre-commit.sample -a ! -f .git/hooks/pre-commit ] ; then
    echo "Activating pre-commit hook."
    cp .git/hooks/pre-commit.sample .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
fi

(autoconf --version) < /dev/null > /dev/null 2>&1 || {
	echo
	echo "You must have autoconf installed to compile $PROJECT."
	echo "Download the appropriate package for your distribution,"
	echo "or get the source tarball at ftp://ftp.gnu.org/pub/gnu/"
	DIE=1
}

# If the user hasn't explicitly chosen an Automake version, use 1.11. This is
# the earliest version that gives us silent rules.
if test -z "$AUTOMAKE"; then
    AUTOMAKE=automake-1.11
    ACLOCAL=aclocal-1.11
fi

($AUTOMAKE --version) < /dev/null > /dev/null 2>&1 || {
        AUTOMAKE=automake
        ACLOCAL=aclocal
}

($AUTOMAKE --version) < /dev/null > /dev/null 2>&1 || {
	echo
	echo "You must have automake installed to compile $PROJECT."
	echo "Get ftp://ftp.cygnus.com/pub/home/tromey/automake-1.2d.tar.gz"
	echo "(or a newer version if it is available)"
	DIE=1
}

LIBTOOLIZE=`which libtoolize`
if ! test -f "$LIBTOOLIZE"; then
	LIBTOOLIZE=`which glibtoolize`
fi

($LIBTOOLIZE --version) < /dev/null > /dev/null 2>&1 || {
	echo
	echo "You must have libtoolize installed to compile $PROJECT."
	echo "Install the libtool package from ftp.gnu.org or a mirror."
	DIE=1
}

if test "$DIE" -eq 1; then
	exit 1
fi

test $TEST_TYPE $FILE || {
	echo "You must run this script in the top-level $PROJECT directory"
	exit 1
}

if test -z "$*"; then
	echo "I am going to run ./configure with no arguments - if you wish "
        echo "to pass any to it, please specify them on the $0 command line."
fi

$LIBTOOLIZE --copy --force

$ACLOCAL -I m4 $ACLOCAL_FLAGS

## optionally feature autoheader
(autoheader --version)  < /dev/null > /dev/null 2>&1 && autoheader

$AUTOMAKE -a $am_opt
if ! autoconf; then
  echo "autoconf failed - version 2.5x is probably required" >&2
  exit 1
fi

cd $ORIGDIR

if test x"$NOCONFIGURE" = x; then
  run_configure=true
  for arg in $*; do
    case $arg in 
        --no-configure)
            run_configure=false
            ;;
        *)
            ;;
    esac
  done
else
  run_configure=false
fi

if $run_configure; then
    $srcdir/configure --enable-developer --config-cache "$@"
fi
