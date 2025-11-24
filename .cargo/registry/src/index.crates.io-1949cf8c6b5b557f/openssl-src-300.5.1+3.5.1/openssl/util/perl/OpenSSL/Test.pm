# Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::Test;

use strict;
use warnings;

use Carp;
use Test::More 0.96;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
$VERSION = "1.0";
@ISA = qw(Exporter);
@EXPORT = (@Test::More::EXPORT, qw(setup run indir cmd app fuzz test
                                   perlapp perltest subtest));
@EXPORT_OK = (@Test::More::EXPORT_OK, qw(bldtop_dir bldtop_file
                                         srctop_dir srctop_file
                                         data_file data_dir
                                         result_file result_dir
                                         pipe with cmdstr
                                         openssl_versions
                                         ok_nofips is_nofips isnt_nofips));

=head1 NAME

OpenSSL::Test - a private extension of Test::More

=head1 SYNOPSIS

  use OpenSSL::Test;

  setup("my_test_name");

  plan tests => 2;

  ok(run(app(["openssl", "version"])), "check for openssl presence");

  indir "subdir" => sub {
    ok(run(test(["sometest", "arg1"], stdout => "foo.txt")),
       "run sometest with output to foo.txt");
  };

=head1 DESCRIPTION

This module is a private extension of L<Test::More> for testing OpenSSL.
In addition to the Test::More functions, it also provides functions that
easily find the diverse programs within a OpenSSL build tree, as well as
some other useful functions.

This module I<depends> on the environment variables C<$TOP> or C<$SRCTOP>
and C<$BLDTOP>.  Without one of the combinations it refuses to work.
See L</ENVIRONMENT> below.

With each test recipe, a parallel data directory with (almost) the same name
as the recipe is possible in the source directory tree.  For example, for a
recipe C<$SRCTOP/test/recipes/99-foo.t>, there could be a directory
C<$SRCTOP/test/recipes/99-foo_data/>.

=cut

use File::Copy;
use File::Spec::Functions qw/file_name_is_absolute curdir canonpath splitdir
                             catdir catfile splitpath catpath devnull abs2rel/;
use File::Path 2.00 qw/rmtree mkpath/;
use File::Basename;
use Cwd qw/getcwd abs_path/;
use OpenSSL::Util;

my $level = 0;

# The name of the test.  This is set by setup() and is used in the other
# functions to verify that setup() has been used.
my $test_name = undef;

# Directories we want to keep track of TOP, APPS, TEST and RESULTS are the
# ones we're interested in, corresponding to the environment variables TOP
# (mandatory), BIN_D, TEST_D, UTIL_D and RESULT_D.
my %directories = ();

# The environment variables that gave us the contents in %directories.  These
# get modified whenever we change directories, so that subprocesses can use
# the values of those environment variables as well
my @direnv = ();

# A bool saying if we shall stop all testing if the current recipe has failing
# tests or not.  This is set by setup() if the environment variable STOPTEST
# is defined with a non-empty value.
my $end_with_bailout = 0;

# A set of hooks that is affected by with() and may be used in diverse places.
# All hooks are expected to be CODE references.
my %hooks = (

    # exit_checker is used by run() directly after completion of a command.
    # it receives the exit code from that command and is expected to return
    # 1 (for success) or 0 (for failure).  This is the status value that run()
    # will give back (through the |statusvar| reference and as returned value
    # when capture => 1 doesn't apply).
    exit_checker => sub { return shift == 0 ? 1 : 0 },

    );

# Debug flag, to be set manually when needed
my $debug = 0;

=head2 Main functions

The following functions are exported by default when using C<OpenSSL::Test>.

=cut

=over 4

=item B<setup "NAME">

C<setup> is used for initial setup, and it is mandatory that it's used.
If it's not used in a OpenSSL test recipe, the rest of the recipe will
most likely refuse to run.

C<setup> checks for environment variables (see L</ENVIRONMENT> below),
checks that C<$TOP/Configure> or C<$SRCTOP/Configure> exists, C<chdir>
into the results directory (defined by the C<$RESULT_D> environment
variable if defined, otherwise C<$BLDTOP/test-runs> or C<$TOP/test-runs>,
whichever is defined).

=back

=cut

sub setup {
    my $old_test_name = $test_name;
    $test_name = shift;
    my %opts = @_;

    BAIL_OUT("setup() must receive a name") unless $test_name;
    warn "setup() detected test name change.  Innocuous, so we continue...\n"
        if $old_test_name && $old_test_name ne $test_name;

    return if $old_test_name;

    BAIL_OUT("setup() needs \$TOP or \$SRCTOP and \$BLDTOP to be defined")
        unless $ENV{TOP} || ($ENV{SRCTOP} && $ENV{BLDTOP});
    BAIL_OUT("setup() found both \$TOP and \$SRCTOP or \$BLDTOP...")
        if $ENV{TOP} && ($ENV{SRCTOP} || $ENV{BLDTOP});

    __env();

    BAIL_OUT("setup() expects the file Configure in the source top directory")
        unless -f srctop_file("Configure");

    note "The results of this test will end up in $directories{RESULTS}"
        unless $opts{quiet};

    __cwd($directories{RESULTS});
}

=over 4

=item B<indir "SUBDIR" =E<gt> sub BLOCK, OPTS>

C<indir> is used to run a part of the recipe in a different directory than
the one C<setup> moved into, usually a subdirectory, given by SUBDIR.
The part of the recipe that's run there is given by the codeblock BLOCK.

C<indir> takes some additional options OPTS that affect the subdirectory:

=over 4

=item B<create =E<gt> 0|1>

When set to 1 (or any value that perl perceives as true), the subdirectory
will be created if it doesn't already exist.  This happens before BLOCK
is executed.

=back

An example:

  indir "foo" => sub {
      ok(run(app(["openssl", "version"]), stdout => "foo.txt"));
      if (ok(open(RESULT, "foo.txt"), "reading foo.txt")) {
          my $line = <RESULT>;
          close RESULT;
          is($line, qr/^OpenSSL 1\./,
             "check that we're using OpenSSL 1.x.x");
      }
  }, create => 1;

=back

=cut

sub indir {
    my $subdir = shift;
    my $codeblock = shift;
    my %opts = @_;

    my $reverse = __cwd($subdir,%opts);
    BAIL_OUT("FAILURE: indir, \"$subdir\" wasn't possible to move into")
	unless $reverse;

    $codeblock->();

    __cwd($reverse);
}

=over 4

=item B<cmd ARRAYREF, OPTS>

This functions build up a platform dependent command based on the
input.  It takes a reference to a list that is the executable or
script and its arguments, and some additional options (described
further on).  Where necessary, the command will be wrapped in a
suitable environment to make sure the correct shared libraries are
used (currently only on Unix).

It returns a CODEREF to be used by C<run>, C<pipe> or C<cmdstr>.

The options that C<cmd> (as well as its derivatives described below) can take
are in the form of hash values:

=over 4

=item B<stdin =E<gt> PATH>

=item B<stdout =E<gt> PATH>

=item B<stderr =E<gt> PATH>

In all three cases, the corresponding standard input, output or error is
redirected from (for stdin) or to (for the others) a file given by the
string PATH, I<or>, if the value is C<undef>, C</dev/null> or similar.

=back

=item B<app ARRAYREF, OPTS>

=item B<test ARRAYREF, OPTS>

Both of these are specific applications of C<cmd>, with just a couple
of small difference:

C<app> expects to find the given command (the first item in the given list
reference) as an executable in C<$BIN_D> (if defined, otherwise C<$TOP/apps>
or C<$BLDTOP/apps>).

C<test> expects to find the given command (the first item in the given list
reference) as an executable in C<$TEST_D> (if defined, otherwise C<$TOP/test>
or C<$BLDTOP/test>).

Also, for both C<app> and C<test>, the command may be prefixed with
the content of the environment variable C<$EXE_SHELL>, which is useful
in case OpenSSL has been cross compiled.

=item B<perlapp ARRAYREF, OPTS>

=item B<perltest ARRAYREF, OPTS>

These are also specific applications of C<cmd>, where the interpreter
is predefined to be C<perl>, and they expect the script to be
interpreted to reside in the same location as C<app> and C<test>.

C<perlapp> and C<perltest> will also take the following option:

=over 4

=item B<interpreter_args =E<gt> ARRAYref>

The array reference is a set of arguments for the interpreter rather
than the script.  Take care so that none of them can be seen as a
script!  Flags and their eventual arguments only!

=back

An example:

  ok(run(perlapp(["foo.pl", "arg1"],
                 interpreter_args => [ "-I", srctop_dir("test") ])));

=back

=begin comment

One might wonder over the complexity of C<apps>, C<fuzz>, C<test>, ...
with all the lazy evaluations and all that.  The reason for this is that
we want to make sure the directory in which those programs are found are
correct at the time these commands are used.  Consider the following code
snippet:

  my $cmd = app(["openssl", ...]);

  indir "foo", sub {
      ok(run($cmd), "Testing foo")
  };

If there wasn't this lazy evaluation, the directory where C<openssl> is
found would be incorrect at the time C<run> is called, because it was
calculated before we moved into the directory "foo".

=end comment

=cut

sub cmd {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my $num = shift;
        # Make a copy to not destroy the caller's array
        my @cmdargs = ( @$cmd );
        my @prog = __wrap_cmd(shift @cmdargs, $opts{exe_shell} // ());

        return __decorate_cmd($num, [ @prog, fixup_cmd_elements(@cmdargs) ],
                              %opts);
    }
}

sub app {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my @cmdargs = ( @{$cmd} );
        my @prog = __fixup_prg(__apps_file(shift @cmdargs, __exeext()));
        return cmd([ @prog, @cmdargs ],
                   exe_shell => $ENV{EXE_SHELL}, %opts) -> (shift);
    }
}

sub fuzz {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my @cmdargs = ( @{$cmd} );
        my @prog = __fixup_prg(__fuzz_file(shift @cmdargs, __exeext()));
        return cmd([ @prog, @cmdargs ],
                   exe_shell => $ENV{EXE_SHELL}, %opts) -> (shift);
    }
}

sub test {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my @cmdargs = ( @{$cmd} );
        my @prog = __fixup_prg(__test_file(shift @cmdargs, __exeext()));
        return cmd([ @prog, @cmdargs ],
                   exe_shell => $ENV{EXE_SHELL}, %opts) -> (shift);
    }
}

sub perlapp {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my @interpreter_args = defined $opts{interpreter_args} ?
            @{$opts{interpreter_args}} : ();
        my @interpreter = __fixup_prg($^X);
        my @cmdargs = ( @{$cmd} );
        my @prog = __apps_file(shift @cmdargs, undef);
        return cmd([ @interpreter, @interpreter_args,
                     @prog, @cmdargs ], %opts) -> (shift);
    }
}

sub perltest {
    my $cmd = shift;
    my %opts = @_;
    return sub {
        my @interpreter_args = defined $opts{interpreter_args} ?
            @{$opts{interpreter_args}} : ();
        my @interpreter = __fixup_prg($^X);
        my @cmdargs = ( @{$cmd} );
        my @prog = __test_file(shift @cmdargs, undef);
        return cmd([ @interpreter, @interpreter_args,
                     @prog, @cmdargs ], %opts) -> (shift);
    }
}

=over 4

=item B<run CODEREF, OPTS>

CODEREF is expected to be the value return by C<cmd> or any of its
derivatives, anything else will most likely cause an error unless you
know what you're doing.

C<run> executes the command returned by CODEREF and return either the
resulting standard output (if the option C<capture> is set true) or a boolean
indicating if the command succeeded or not.

The options that C<run> can take are in the form of hash values:

=over 4

=item B<capture =E<gt> 0|1>

If true, the command will be executed with a perl backtick,
and C<run> will return the resulting standard output as an array of lines.
If false or not given, the command will be executed with C<system()>,
and C<run> will return 1 if the command was successful or 0 if it wasn't.

=item B<prefix =E<gt> EXPR>

If specified, EXPR will be used as a string to prefix the output from the
command.  This is useful if the output contains lines starting with C<ok >
or C<not ok > that can disturb Test::Harness.

=item B<statusvar =E<gt> VARREF>

If used, B<VARREF> must be a reference to a scalar variable.  It will be
assigned a boolean indicating if the command succeeded or not.  This is
particularly useful together with B<capture>.

=back

Usually 1 indicates that the command was successful and 0 indicates failure.
For further discussion on what is considered a successful command or not, see
the function C<with> further down.

=back

=cut

sub run {
    my ($cmd, $display_cmd) = shift->(0);
    my %opts = @_;

    return () if !$cmd;

    my $prefix = "";
    if ( $^O eq "VMS" ) {	# VMS
	$prefix = "pipe ";
    }

    my @r = ();
    my $r = 0;
    my $e = 0;

    die "OpenSSL::Test::run(): statusvar value not a scalar reference"
        if $opts{statusvar} && ref($opts{statusvar}) ne "SCALAR";

    # For some reason, program output, or even output from this function
    # somehow isn't caught by TAP::Harness (TAP::Parser?) on VMS, so we're
    # silencing it specifically there until further notice.
    my $save_STDOUT;
    my $save_STDERR;
    if ($^O eq 'VMS') {
        # In non-verbose, we want to shut up the command interpreter, in case
        # it has something to complain about.  On VMS, it might complain both
        # on stdout and stderr
        if ($ENV{HARNESS_ACTIVE} && !$ENV{HARNESS_VERBOSE}) {
            open $save_STDOUT, '>&', \*STDOUT or die "Can't dup STDOUT: $!";
            open $save_STDERR, '>&', \*STDERR or die "Can't dup STDERR: $!";
            open STDOUT, ">", devnull();
            open STDERR, ">", devnull();
        }
    }

    $ENV{HARNESS_OSSL_LEVEL} = $level + 1;

    # The dance we do with $? is the same dance the Unix shells appear to
    # do.  For example, a program that gets aborted (and therefore signals
    # SIGABRT = 6) will appear to exit with the code 134.  We mimic this
    # to make it easier to compare with a manual run of the command.
    if ($opts{capture} || defined($opts{prefix})) {
	my $pipe;
	local $_;

	open($pipe, '-|', "$prefix$cmd") or die "Can't start command: $!";
	while(<$pipe>) {
	    my $l = ($opts{prefix} // "") . $_;
	    if ($opts{capture}) {
		push @r, $l;
	    } else {
		print STDOUT $l;
	    }
	}
	close $pipe;
    } else {
	$ENV{HARNESS_OSSL_PREFIX} = "# ";
	system("$prefix$cmd");
	delete $ENV{HARNESS_OSSL_PREFIX};
    }
    $e = ($? & 0x7f) ? ($? & 0x7f)|0x80 : ($? >> 8);
    $r = $hooks{exit_checker}->($e);
    if ($opts{statusvar}) {
        ${$opts{statusvar}} = $r;
    }

    # Restore STDOUT / STDERR on VMS
    if ($^O eq 'VMS') {
        if ($ENV{HARNESS_ACTIVE} && !$ENV{HARNESS_VERBOSE}) {
            close STDOUT;
            close STDERR;
            open STDOUT, '>&', $save_STDOUT or die "Can't restore STDOUT: $!";
            open STDERR, '>&', $save_STDERR or die "Can't restore STDERR: $!";
        }

        print STDERR "$prefix$display_cmd => $e\n"
            if !$ENV{HARNESS_ACTIVE} || $ENV{HARNESS_VERBOSE};
    } else {
        print STDERR "$prefix$display_cmd => $e\n";
    }

    # At this point, $? stops being interesting, and unfortunately,
    # there are Test::More versions that get picky if we leave it
    # non-zero.
    $? = 0;

    if ($opts{capture}) {
	return @r;
    } else {
	return $r;
    }
}

END {
    my $tb = Test::More->builder;
    my $failure = scalar(grep { $_ == 0; } $tb->summary);
    if ($failure && $end_with_bailout) {
	BAIL_OUT("Stoptest!");
    }
}

=head2 Utility functions

The following functions are exported on request when using C<OpenSSL::Test>.

  # To only get the bldtop_file and srctop_file functions.
  use OpenSSL::Test qw/bldtop_file srctop_file/;

  # To only get the bldtop_file function in addition to the default ones.
  use OpenSSL::Test qw/:DEFAULT bldtop_file/;

=cut

# Utility functions, exported on request

=over 4

=item B<bldtop_dir LIST>

LIST is a list of directories that make up a path from the top of the OpenSSL
build directory (as indicated by the environment variable C<$TOP> or
C<$BLDTOP>).
C<bldtop_dir> returns the resulting directory as a string, adapted to the local
operating system.

=back

=cut

sub bldtop_dir {
    my $d = __bldtop_dir(@_);	# This caters for operating systems that have
				# a very distinct syntax for directories.

    croak "$d isn't a directory" if -e $d && ! -d $d;
    return $d;
}

=over 4

=item B<bldtop_file LIST, FILENAME>

LIST is a list of directories that make up a path from the top of the OpenSSL
build directory (as indicated by the environment variable C<$TOP> or
C<$BLDTOP>) and FILENAME is the name of a file located in that directory path.
C<bldtop_file> returns the resulting file path as a string, adapted to the local
operating system.

=back

=cut

sub bldtop_file {
    my $f = __bldtop_file(@_);

    croak "$f isn't a file" if -e $f && ! -f $f;
    return $f;
}

=over 4

=item B<srctop_dir LIST>

LIST is a list of directories that make up a path from the top of the OpenSSL
source directory (as indicated by the environment variable C<$TOP> or
C<$SRCTOP>).
C<srctop_dir> returns the resulting directory as a string, adapted to the local
operating system.

=back

=cut

sub srctop_dir {
    my $d = __srctop_dir(@_);	# This caters for operating systems that have
				# a very distinct syntax for directories.

    croak "$d isn't a directory" if -e $d && ! -d $d;
    return $d;
}

=over 4

=item B<srctop_file LIST, FILENAME>

LIST is a list of directories that make up a path from the top of the OpenSSL
source directory (as indicated by the environment variable C<$TOP> or
C<$SRCTOP>) and FILENAME is the name of a file located in that directory path.
C<srctop_file> returns the resulting file path as a string, adapted to the local
operating system.

=back

=cut

sub srctop_file {
    my $f = __srctop_file(@_);

    croak "$f isn't a file" if -e $f && ! -f $f;
    return $f;
}

=over 4

=item B<data_dir LIST>

LIST is a list of directories that make up a path from the data directory
associated with the test (see L</DESCRIPTION> above).
C<data_dir> returns the resulting directory as a string, adapted to the local
operating system.

=back

=cut

sub data_dir {
    my $d = __data_dir(@_);

    croak "$d isn't a directory" if -e $d && ! -d $d;
    return $d;
}

=over 4

=item B<data_file LIST, FILENAME>

LIST is a list of directories that make up a path from the data directory
associated with the test (see L</DESCRIPTION> above) and FILENAME is the name
of a file located in that directory path.  C<data_file> returns the resulting
file path as a string, adapted to the local operating system.

=back

=cut

sub data_file {
    my $f = __data_file(@_);

    croak "$f isn't a file" if -e $f && ! -f $f;
    return $f;
}

=over 4

=item B<result_dir LIST>

LIST is a list of directories that make up a path from the result directory
associated with the test (see L</DESCRIPTION> above).
C<result_dir> returns the resulting directory as a string, adapted to the local
operating system.

=back

=cut

sub result_dir {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $d = catdir($directories{RESULTS},@_);

    croak "$d isn't a directory" if -e $d && ! -d $d;
    return $d;
}

=over 4

=item B<result_file LIST, FILENAME>

LIST is a list of directories that make up a path from the data directory
associated with the test (see L</DESCRIPTION> above) and FILENAME is the name
of a file located in that directory path.  C<result_file> returns the resulting
file path as a string, adapted to the local operating system.

=back

=cut

sub result_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $f = catfile(result_dir(),@_);

    croak "$f isn't a file" if -e $f && ! -f $f;
    return $f;
}

=over 4

=item B<pipe LIST>

LIST is a list of CODEREFs returned by C<app> or C<test>, from which C<pipe>
creates a new command composed of all the given commands put together in a
pipe.  C<pipe> returns a new CODEREF in the same manner as C<app> or C<test>,
to be passed to C<run> for execution.

=back

=cut

sub pipe {
    my @cmds = @_;
    return
	sub {
	    my @cs  = ();
	    my @dcs = ();
	    my @els = ();
	    my $counter = 0;
	    foreach (@cmds) {
		my ($c, $dc, @el) = $_->(++$counter);

		return () if !$c;

		push @cs, $c;
		push @dcs, $dc;
		push @els, @el;
	    }
	    return (
		join(" | ", @cs),
		join(" | ", @dcs),
		@els
		);
    };
}

=over 4

=item B<with HASHREF, CODEREF>

C<with> will temporarily install hooks given by the HASHREF and then execute
the given CODEREF.  Hooks are usually expected to have a coderef as value.

The currently available hoosk are:

=over 4

=item B<exit_checker =E<gt> CODEREF>

This hook is executed after C<run> has performed its given command.  The
CODEREF receives the exit code as only argument and is expected to return
1 (if the exit code indicated success) or 0 (if the exit code indicated
failure).

=back

=back

=cut

sub with {
    my $opts = shift;
    my %opts = %{$opts};
    my $codeblock = shift;

    my %saved_hooks = ();

    foreach (keys %opts) {
	$saved_hooks{$_} = $hooks{$_}	if exists($hooks{$_});
	$hooks{$_} = $opts{$_};
    }

    $codeblock->();

    foreach (keys %saved_hooks) {
	$hooks{$_} = $saved_hooks{$_};
    }
}

=over 4

=item B<cmdstr CODEREF, OPTS>

C<cmdstr> takes a CODEREF from C<app> or C<test> and simply returns the
command as a string.

C<cmdstr> takes some additional options OPTS that affect the string returned:

=over 4

=item B<display =E<gt> 0|1>

When set to 0, the returned string will be with all decorations, such as a
possible redirect of stderr to the null device.  This is suitable if the
string is to be used directly in a recipe.

When set to 1, the returned string will be without extra decorations.  This
is suitable for display if that is desired (doesn't confuse people with all
internal stuff), or if it's used to pass a command down to a subprocess.

Default: 0

=back

=back

=cut

sub cmdstr {
    my ($cmd, $display_cmd) = shift->(0);
    my %opts = @_;

    if ($opts{display}) {
        return $display_cmd;
    } else {
        return $cmd;
    }
}

=over 4

=over 4

=item B<openssl_versions>

Returns a list of two version numbers, the first representing the build
version, the second representing the library version.  See opensslv.h for
more information on those numbers.

=back

=cut

my @versions = ();
sub openssl_versions {
    unless (@versions) {
        my %lines =
            map { s/\R$//;
                  /^(.*): (.*)$/;
                  $1 => $2 }
            run(test(['versions']), capture => 1);
        @versions = ( $lines{'Build version'}, $lines{'Library version'} );
    }
    return @versions;
}

=over 4

=item B<ok_nofips EXPR, TEST_NAME>

C<ok_nofips> is equivalent to using C<ok> when the environment variable
C<FIPS_MODE> is undefined, otherwise it is equivalent to C<not ok>. This can be
used for C<ok> tests that must fail when testing a FIPS provider. The parameters
are the same as used by C<ok> which is an expression EXPR followed by the test
description TEST_NAME.

An example:

  ok_nofips(run(app(["md5.pl"])), "md5 should fail in fips mode");

=item B<is_nofips EXPR1, EXPR2, TEST_NAME>

C<is_nofips> is equivalent to using C<is> when the environment variable
C<FIPS_MODE> is undefined, otherwise it is equivalent to C<isnt>. This can be
used for C<is> tests that must fail when testing a FIPS provider. The parameters
are the same as used by C<is> which has 2 arguments EXPR1 and EXPR2 that can be
compared using eq or ne, followed by a test description TEST_NAME.

An example:

  is_nofips(ultimate_answer(), 42,  "Meaning of Life");

=item B<isnt_nofips EXPR1, EXPR2, TEST_NAME>

C<isnt_nofips> is equivalent to using C<isnt> when the environment variable
C<FIPS_MODE> is undefined, otherwise it is equivalent to C<is>. This can be
used for C<isnt> tests that must fail when testing a FIPS provider. The
parameters are the same as used by C<isnt> which has 2 arguments EXPR1 and EXPR2
that can be compared using ne or eq, followed by a test description TEST_NAME.

An example:

  isnt_nofips($foo, '',  "Got some foo");

=back

=cut

sub ok_nofips {
    return ok(!$_[0], @_[1..$#_]) if defined $ENV{FIPS_MODE};
    return ok($_[0], @_[1..$#_]);
}

sub is_nofips {
    return isnt($_[0], $_[1], @_[2..$#_]) if defined $ENV{FIPS_MODE};
    return is($_[0], $_[1], @_[2..$#_]);
}

sub isnt_nofips {
    return is($_[0], $_[1], @_[2..$#_]) if defined $ENV{FIPS_MODE};
    return isnt($_[0], $_[1], @_[2..$#_]);
}

######################################################################
# private functions.  These are never exported.

=head1 ENVIRONMENT

OpenSSL::Test depends on some environment variables.

=over 4

=item B<TOP>

This environment variable is mandatory.  C<setup> will check that it's
defined and that it's a directory that contains the file C<Configure>.
If this isn't so, C<setup> will C<BAIL_OUT>.

=item B<BIN_D>

If defined, its value should be the directory where the openssl application
is located.  Defaults to C<$TOP/apps> (adapted to the operating system).

=item B<TEST_D>

If defined, its value should be the directory where the test applications
are located.  Defaults to C<$TOP/test> (adapted to the operating system).

=item B<STOPTEST>

If defined, it puts testing in a different mode, where a recipe with
failures will result in a C<BAIL_OUT> at the end of its run.

=item B<FIPS_MODE>

If defined it indicates that the FIPS provider is being tested. Tests may use
B<ok_nofips>, B<is_nofips> and B<isnt_nofips> to invert test results
i.e. Some tests may only work in non FIPS mode.

=back

=cut

sub __env {
    (my $recipe_datadir = basename($0)) =~ s/\.t$/_data/i;

    $directories{SRCTOP}    = abs_path($ENV{SRCTOP} || $ENV{TOP});
    $directories{BLDTOP}    = abs_path($ENV{BLDTOP} || $ENV{TOP});
    $directories{BLDAPPS}   = $ENV{BIN_D}  || __bldtop_dir("apps");
    $directories{SRCAPPS}   =                 __srctop_dir("apps");
    $directories{BLDFUZZ}   =                 __bldtop_dir("fuzz");
    $directories{SRCFUZZ}   =                 __srctop_dir("fuzz");
    $directories{BLDTEST}   = $ENV{TEST_D} || __bldtop_dir("test");
    $directories{SRCTEST}   =                 __srctop_dir("test");
    $directories{SRCDATA}   =                 __srctop_dir("test", "recipes",
                                                           $recipe_datadir);
    $directories{RESULTTOP} = $ENV{RESULT_D} || __bldtop_dir("test-runs");
    $directories{RESULTS}   = catdir($directories{RESULTTOP}, $test_name);

    # Create result directory dynamically
    rmtree($directories{RESULTS}, { safe => 0, keep_root => 1 });
    mkpath($directories{RESULTS});

    # All directories are assumed to exist, except for SRCDATA.  If that one
    # doesn't exist, just drop it.
    delete $directories{SRCDATA} unless -d $directories{SRCDATA};

    push @direnv, "TOP"       if $ENV{TOP};
    push @direnv, "SRCTOP"    if $ENV{SRCTOP};
    push @direnv, "BLDTOP"    if $ENV{BLDTOP};
    push @direnv, "BIN_D"     if $ENV{BIN_D};
    push @direnv, "TEST_D"    if $ENV{TEST_D};
    push @direnv, "RESULT_D"  if $ENV{RESULT_D};

    $end_with_bailout = $ENV{STOPTEST} ? 1 : 0;
};

# __srctop_file and __srctop_dir are helpers to build file and directory
# names on top of the source directory.  They depend on $SRCTOP, and
# therefore on the proper use of setup() and when needed, indir().
# __bldtop_file and __bldtop_dir do the same thing but relative to $BLDTOP.
# __srctop_file and __bldtop_file take the same kind of argument as
# File::Spec::Functions::catfile.
# Similarly, __srctop_dir and __bldtop_dir take the same kind of argument
# as File::Spec::Functions::catdir
sub __srctop_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $f = pop;
    return abs2rel(catfile($directories{SRCTOP},@_,$f),getcwd);
}

sub __srctop_dir {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    return abs2rel(catdir($directories{SRCTOP},@_), getcwd);
}

sub __bldtop_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $f = pop;
    return abs2rel(catfile($directories{BLDTOP},@_,$f), getcwd);
}

sub __bldtop_dir {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    return abs2rel(catdir($directories{BLDTOP},@_), getcwd);
}

# __exeext is a function that returns the platform dependent file extension
# for executable binaries, or the value of the environment variable $EXE_EXT
# if that one is defined.
sub __exeext {
    my $ext = "";
    if ($^O eq "VMS" ) {	# VMS
	$ext = ".exe";
    } elsif ($^O eq "MSWin32") { # Windows
	$ext = ".exe";
    }
    return $ENV{"EXE_EXT"} || $ext;
}

# __test_file, __apps_file and __fuzz_file return the full path to a file
# relative to the test/, apps/ or fuzz/ directory in the build tree or the
# source tree, depending on where the file is found.  Note that when looking
# in the build tree, the file name with an added extension is looked for, if
# an extension is given.  The intent is to look for executable binaries (in
# the build tree) or possibly scripts (in the source tree).
# These functions all take the same arguments as File::Spec::Functions::catfile,
# *plus* a mandatory extension argument.  This extension argument can be undef,
# and is ignored in such a case.
sub __test_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $e = pop || "";
    my $f = pop;
    my $out = catfile($directories{BLDTEST},@_,$f . $e);
    $out = catfile($directories{SRCTEST},@_,$f) unless -f $out;
    return $out;
}

sub __apps_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $e = pop || "";
    my $f = pop;
    my $out = catfile($directories{BLDAPPS},@_,$f . $e);
    $out = catfile($directories{SRCAPPS},@_,$f) unless -f $out;
    return $out;
}

sub __fuzz_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $e = pop || "";
    my $f = pop;
    my $out = catfile($directories{BLDFUZZ},@_,$f . $e);
    $out = catfile($directories{SRCFUZZ},@_,$f) unless -f $out;
    return $out;
}

sub __data_file {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    return undef unless exists $directories{SRCDATA};

    my $f = pop;
    return catfile($directories{SRCDATA},@_,$f);
}

sub __data_dir {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    return undef unless exists $directories{SRCDATA};

    return catdir($directories{SRCDATA},@_);
}

# __cwd DIR
# __cwd DIR, OPTS
#
# __cwd changes directory to DIR (string) and changes all the relative
# entries in %directories accordingly.  OPTS is an optional series of
# hash style arguments to alter __cwd's behavior:
#
#    create = 0|1       The directory we move to is created if 1, not if 0.

sub __cwd {
    my $dir = catdir(shift);
    my %opts = @_;

    # If the directory is to be created, we must do that before using
    # abs_path().
    $dir = canonpath($dir);
    if ($opts{create}) {
	mkpath($dir);
    }

    my $abscurdir = abs_path(curdir());
    my $absdir = abs_path($dir);
    my $reverse = abs2rel($abscurdir, $absdir);

    # PARANOIA: if we're not moving anywhere, we do nothing more
    if ($abscurdir eq $absdir) {
	return $reverse;
    }

    # Do not support a move to a different volume for now.  Maybe later.
    BAIL_OUT("FAILURE: \"$dir\" moves to a different volume, not supported")
	if $reverse eq $abscurdir;

    # If someone happened to give a directory that leads back to the current,
    # it's extremely silly to do anything more, so just simulate that we did
    # move.
    # In this case, we won't even clean it out, for safety's sake.
    return "." if $reverse eq "";

    # We are recalculating the directories we keep track of, but need to save
    # away the result for after having moved into the new directory.
    my %tmp_directories = ();
    my %tmp_ENV = ();

    # For each of these directory variables, figure out where they are relative
    # to the directory we want to move to if they aren't absolute (if they are,
    # they don't change!)
    my @dirtags = sort keys %directories;
    foreach (@dirtags) {
	if (!file_name_is_absolute($directories{$_})) {
	    my $oldpath = abs_path($directories{$_});
	    my $newpath = abs2rel($oldpath, $absdir);
	    if ($debug) {
		print STDERR "DEBUG: [dir $_] old path: $oldpath\n";
		print STDERR "DEBUG: [dir $_] new base: $absdir\n";
		print STDERR "DEBUG: [dir $_] resulting new path: $newpath\n";
	    }
	    $tmp_directories{$_} = $newpath;
	}
    }

    # Treat each environment variable that was used to get us the values in
    # %directories the same was as the paths in %directories, so any sub
    # process can use their values properly as well
    foreach (@direnv) {
	if (!file_name_is_absolute($ENV{$_})) {
	    my $oldpath = abs_path($ENV{$_});
	    my $newpath = abs2rel($oldpath, $absdir);
	    if ($debug) {
		print STDERR "DEBUG: [env $_] old path: $oldpath\n";
		print STDERR "DEBUG: [env $_] new base: $absdir\n";
		print STDERR "DEBUG: [env $_] resulting new path: $newpath\n";
	    }
	    $tmp_ENV{$_} = $newpath;
	}
    }

    # Should we just bail out here as well?  I'm unsure.
    return undef unless chdir($dir);

    # We put back new values carefully.  Doing the obvious
    # %directories = ( %tmp_directories )
    # will clear out any value that happens to be an absolute path
    foreach (keys %tmp_directories) {
        $directories{$_} = $tmp_directories{$_};
    }
    foreach (keys %tmp_ENV) {
        $ENV{$_} = $tmp_ENV{$_};
    }

    if ($debug) {
	print STDERR "DEBUG: __cwd(), directories and files:\n";
	print STDERR "	Moving from $abscurdir\n";
	print STDERR "	Moving to $absdir\n";
	print STDERR "\n";
	print STDERR "	\$directories{BLDTEST} = \"$directories{BLDTEST}\"\n";
	print STDERR "	\$directories{SRCTEST} = \"$directories{SRCTEST}\"\n";
	print STDERR "	\$directories{SRCDATA} = \"$directories{SRCDATA}\"\n"
            if exists $directories{SRCDATA};
	print STDERR "	\$directories{RESULTS} = \"$directories{RESULTS}\"\n";
	print STDERR "	\$directories{BLDAPPS} = \"$directories{BLDAPPS}\"\n";
	print STDERR "	\$directories{SRCAPPS} = \"$directories{SRCAPPS}\"\n";
	print STDERR "	\$directories{SRCTOP}  = \"$directories{SRCTOP}\"\n";
	print STDERR "	\$directories{BLDTOP}  = \"$directories{BLDTOP}\"\n";
	print STDERR "\n";
	print STDERR "  the way back is \"$reverse\"\n";
    }

    return $reverse;
}

# __wrap_cmd CMD
# __wrap_cmd CMD, EXE_SHELL
#
# __wrap_cmd "wraps" CMD (string) with a beginning command that makes sure
# the command gets executed with an appropriate environment.  If EXE_SHELL
# is given, it is used as the beginning command.
#
# __wrap_cmd returns a list that should be used to build up a larger list
# of command tokens, or be joined together like this:
#
#    join(" ", __wrap_cmd($cmd))
sub __wrap_cmd {
    my $cmd = shift;
    my $exe_shell = shift;

    my @prefix = ();

    if (defined($exe_shell)) {
        # If $exe_shell is defined, trust it
        @prefix = ( $exe_shell );
    } else {
        # Otherwise, use the standard wrapper
        my $std_wrapper = __bldtop_file("util", "wrap.pl");

        if ($^O eq "VMS" || $^O eq "MSWin32") {
            # On VMS and Windows, we run the perl executable explicitly,
            # with necessary fixups.  We might not need that for Windows,
            # but that depends on if the user has associated the '.pl'
            # extension with a perl interpreter, so better be safe.
            @prefix = ( __fixup_prg($^X), $std_wrapper );
        } else {
            # Otherwise, we assume Unix semantics, and trust that the #!
            # line activates perl for us.
            @prefix = ( $std_wrapper );
        }
    }

    return (@prefix, $cmd);
}

# __fixup_prg PROG
#
# __fixup_prg does whatever fixup is needed to execute an executable binary
# given by PROG (string).
#
# __fixup_prg returns a string with the possibly prefixed program path spec.
sub __fixup_prg {
    my $prog = shift;

    return join(' ', fixup_cmd($prog));
}

# __decorate_cmd NUM, CMDARRAYREF
#
# __decorate_cmd takes a command number NUM and a command token array
# CMDARRAYREF, builds up a command string from them and decorates it
# with necessary redirections.
# __decorate_cmd returns a list of two strings, one with the command
# string to actually be used, the other to be displayed for the user.
# The reason these strings might differ is that we redirect stderr to
# the null device unless we're verbose and unless the user has
# explicitly specified a stderr redirection.
sub __decorate_cmd {
    BAIL_OUT("Must run setup() first") if (! $test_name);

    my $num = shift;
    my $cmd = shift;
    my %opts = @_;

    my $cmdstr = join(" ", @$cmd);
    my $null = devnull();
    my $fileornull = sub { $_[0] ? $_[0] : $null; };
    my $stdin = "";
    my $stdout = "";
    my $stderr = "";
    my $saved_stderr = undef;
    $stdin = " < ".$fileornull->($opts{stdin})  if exists($opts{stdin});
    $stdout= " > ".$fileornull->($opts{stdout}) if exists($opts{stdout});
    $stderr=" 2> ".$fileornull->($opts{stderr}) if exists($opts{stderr});

    my $display_cmd = "$cmdstr$stdin$stdout$stderr";

    # VMS program output escapes TAP::Parser
    if ($^O eq 'VMS') {
        $stderr=" 2> ".$null
            unless $stderr || !$ENV{HARNESS_ACTIVE} || $ENV{HARNESS_VERBOSE};
    }

    $cmdstr .= "$stdin$stdout$stderr";

    if ($debug) {
	print STDERR "DEBUG[__decorate_cmd]: \$cmdstr = \"$cmdstr\"\n";
	print STDERR "DEBUG[__decorate_cmd]: \$display_cmd = \"$display_cmd\"\n";
    }

    return ($cmdstr, $display_cmd);
}

=head1 SEE ALSO

L<Test::More>, L<Test::Harness>

=head1 AUTHORS

Richard Levitte E<lt>levitte@openssl.orgE<gt> with assistance and
inspiration from Andy Polyakov E<lt>appro@openssl.org<gt>.

=cut

no warnings 'redefine';
sub subtest {
    $level++;

    Test::More::subtest @_;

    $level--;
};

1;
