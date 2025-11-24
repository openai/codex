#! /usr/bin/env perl
# Copyright 1998-2024 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# Determine the operating system and run ./Configure.  Far descendant from
# Apache's minarch and GuessOS.

package OpenSSL::config;

use strict;
use warnings;
use Getopt::Std;
use File::Basename;
use File::Spec;
use IPC::Cmd;
use POSIX;
use Config;
use Carp;

# These control our behavior.
my $DRYRUN;
my $VERBOSE;
my $WHERE = dirname($0);
my $WAIT = 1;

# Machine type, etc., used to determine the platform
my $MACHINE;
my $RELEASE;
my $SYSTEM;
my $VERSION;
my $CCVENDOR;
my $CCVER;
my $CL_ARCH;
my $GCC_BITS;
my $GCC_ARCH;

# Some environment variables; they will affect Configure
my $CONFIG_OPTIONS = $ENV{CONFIG_OPTIONS} // '';
my $CC;
my $CROSS_COMPILE;

# For determine_compiler_settings, the list of known compilers
my @c_compilers = qw(clang gcc cc);
# Methods to determine compiler version.  The expected output is one of
# MAJOR or MAJOR.MINOR or MAJOR.MINOR.PATCH...  or false if the compiler
# isn't of the given brand.
# This is a list to ensure that gnu comes last, as we've made it a fallback
my @cc_version =
    (
     clang => sub {
         return undef unless IPC::Cmd::can_run("$CROSS_COMPILE$CC");
         my $v = `$CROSS_COMPILE$CC -v 2>&1`;
         $v =~ m/(?:(?:clang|LLVM) version|.*based on LLVM)\s+([0-9]+\.[0-9]+)/;
         return $1;
     },
     gnu => sub {
         return undef unless IPC::Cmd::can_run("$CROSS_COMPILE$CC");
         my $nul = File::Spec->devnull();
         my $v = `$CROSS_COMPILE$CC -dumpversion 2> $nul`;
         # Strip off whatever prefix egcs prepends the number with.
         # Hopefully, this will work for any future prefixes as well.
         $v =~ s/^[a-zA-Z]*\-//;
         return $v;
     },
    );

# This is what we will set as the target for calling Configure.
my $options = '';

# Pattern matches against "${SYSTEM}:${RELEASE}:${VERSION}:${MACHINE}"
# The patterns are assumed to be wrapped like this: /^(${pattern})$/
my $guess_patterns = [
    [ 'A\/UX:.*',                   'm68k-apple-aux3' ],
    [ 'AIX:[3-9]:4:.*',             '${MACHINE}-ibm-aix' ],
    [ 'AIX:.*?:[5-9]:.*',           '${MACHINE}-ibm-aix' ],
    [ 'AIX:.*',                     '${MACHINE}-ibm-aix3' ],
    [ 'HI-UX:.*',                   '${MACHINE}-hi-hiux' ],
    [ 'HP-UX:.*',
      sub {
          my $HPUXVER = $RELEASE;
          $HPUXVER =~ s/[^.]*.[0B]*//;
          # HPUX 10 and 11 targets are unified
          return "${MACHINE}-hp-hpux1x" if $HPUXVER =~ m@1[0-9]@;
          return "${MACHINE}-hp-hpux";
      }
    ],
    [ 'IRIX:6\..*',                 'mips3-sgi-irix' ],
    [ 'IRIX64:.*',                  'mips4-sgi-irix64' ],
    [ 'Linux:[2-9]\..*',            '${MACHINE}-whatever-linux2' ],
    [ 'Linux:1\..*',                '${MACHINE}-whatever-linux1' ],
    [ 'GNU:.*86-AT386',             'hurd-x86' ],
    [ 'GNU:.*86_64-AT386',          'hurd-x86_64' ],
    [ 'LynxOS:.*',                  '${MACHINE}-lynx-lynxos' ],
    # BSD/OS always says 386
    [ 'BSD\/OS:4\..*',              'i486-whatever-bsdi4' ],
    # Order is important, this has to appear before 'BSD\/386:'
    [ 'BSD/386:.*?:.*?:.*486.*|BSD/OS:.*?:.*?:.*?:.*486.*',
      sub {
          my $BSDVAR = `/sbin/sysctl -n hw.model`;
          return "i586-whatever-bsdi" if $BSDVAR =~ m@Pentium@;
          return "i386-whatever-bsdi";
      }
    ],
    [ 'BSD\/386:.*|BSD\/OS:.*',     '${MACHINE}-whatever-bsdi' ],
    # Order is important, this has to appear before 'FreeBSD:'
    [ 'FreeBSD:.*?:.*?:.*386.*',
      sub {
          my $VERS = $RELEASE;
          $VERS =~ s/[-(].*//;
          my $MACH = `sysctl -n hw.model`;
          $MACH = "i386" if $MACH =~ m@386@;
          $MACH = "i486" if $MACH =~ m@486@;
          $MACH = "i686" if $MACH =~ m@Pentium II@;
          $MACH = "i586" if $MACH =~ m@Pentium@;
          $MACH = "$MACHINE" if $MACH !~ /i.86/;
          my $ARCH = 'whatever';
          $ARCH = "pc" if $MACH =~ m@i[0-9]86@;
          return "${MACH}-${ARCH}-freebsd${VERS}";
      }
    ],
    [ 'DragonFly:.*',               '${MACHINE}-whatever-dragonfly' ],
    [ 'FreeBSD:.*',                 '${MACHINE}-whatever-freebsd' ],
    [ 'Haiku:.*',                   '${MACHINE}-whatever-haiku' ],
    # Order is important, this has to appear before 'NetBSD:.*'
    [ 'NetBSD:.*?:.*?:.*386.*',
      sub {
          my $hw = `/usr/sbin/sysctl -n hw.model || /sbin/sysctl -n hw.model`;
          $hw =~  s@.*(.)86-class.*@i${1}86@;
          return "${hw}-whatever-netbsd";
      }
    ],
    [ 'NetBSD:.*',                  '${MACHINE}-whatever-netbsd' ],
    [ 'OpenBSD:.*',                 '${MACHINE}-whatever-openbsd' ],
    [ 'OpenUNIX:.*',                '${MACHINE}-unknown-OpenUNIX${VERSION}' ],
    [ 'OSF1:.*?:.*?:.*alpha.*',
      sub {
          my $OSFMAJOR = $RELEASE;
          $OSFMAJOR =~ 's/^V([0-9]*)\..*$/\1/';
          return "${MACHINE}-dec-tru64" if $OSFMAJOR =~ m@[45]@;
          return "${MACHINE}-dec-osf";
      }
    ],
    [ 'Paragon.*?:.*',              'i860-intel-osf1' ],
    [ 'Rhapsody:.*',                'ppc-apple-rhapsody' ],
    [ 'Darwin:.*?:.*?:Power.*',     'ppc-apple-darwin' ],
    [ 'Darwin:.*',                  '${MACHINE}-apple-darwin' ],
    [ 'SunOS:5\..*',                '${MACHINE}-whatever-solaris2' ],
    [ 'SunOS:.*',                   '${MACHINE}-sun-sunos4' ],
    [ 'UNIX_System_V:4\..*?:.*',    '${MACHINE}-whatever-sysv4' ],
    [ 'VOS:.*?:.*?:i786',           'i386-stratus-vos' ],
    [ 'VOS:.*?:.*?:.*',             'hppa1.1-stratus-vos' ],
    [ '.*?:4.*?:R4.*?:m88k',        '${MACHINE}-whatever-sysv4' ],
    [ 'DYNIX\/ptx:4.*?:.*',         '${MACHINE}-whatever-sysv4' ],
    [ '.*?:4\.0:3\.0:3[34]..(,.*)?', 'i486-ncr-sysv4' ],
    [ 'ULTRIX:.*',                  '${MACHINE}-unknown-ultrix' ],
    [ 'POSIX-BC.*',                 'BS2000-siemens-sysv4' ],
    [ 'machten:.*',                 '${MACHINE}-tenon-${SYSTEM}' ],
    [ 'library:.*',                 '${MACHINE}-ncr-sysv4' ],
    [ 'ConvexOS:.*?:11\.0:.*',      '${MACHINE}-v11-${SYSTEM}' ],
    [ 'MINGW64.*?:.*?:.*?:x86_64',  '${MACHINE}-whatever-mingw64' ],
    [ 'MINGW.*',                    '${MACHINE}-whatever-mingw' ],
    [ 'CYGWIN.*',                   '${MACHINE}-pc-cygwin' ],
    [ 'vxworks.*',                  '${MACHINE}-whatever-vxworks' ],

    # The MACHINE part of the array POSIX::uname() returns on VMS isn't
    # worth the bits wasted on it.  It's better, then, to rely on perl's
    # %Config, which has a trustworthy item 'archname', especially since
    # VMS installation aren't multiarch (yet)
    [ 'OpenVMS:.*',                 "$Config{archname}-whatever-OpenVMS" ],

    # Note: there's also NEO and NSR, but they are old and unsupported
    [ 'NONSTOP_KERNEL:.*:NSE-.*?',  'nse-tandem-nsk${RELEASE}' ],
    [ 'NONSTOP_KERNEL:.*:NSV-.*?',  'nsv-tandem-nsk${RELEASE}' ],
    [ 'NONSTOP_KERNEL:.*:NSX-.*?',  'nsx-tandem-nsk${RELEASE}' ],

    [ sub { -d '/usr/apollo' },     'whatever-apollo-whatever' ],
];

# Run a command, return true if exit zero else false.
# Multiple args are glued together into a pipeline.
# Name comes from OpenSSL tests, often written as "ok(run(...."
sub okrun {
    my $command = join(' | ', @_);
    my $status = system($command) >> 8;
    return $status == 0;
}

# Give user a chance to abort/interrupt if interactive if interactive.
sub maybe_abort {
    if ( $WAIT && -t 1 ) {
        eval {
            local $SIG{ALRM} = sub { die "Timeout"; };
            local $| = 1;
            alarm(5);
            print "You have about five seconds to abort: ";
            my $ignored = <STDIN>;
            alarm(0);
        };
        print "\n" if $@ =~ /Timeout/;
    }
}

# Look for ISC/SCO with its unique uname program
sub is_sco_uname {
    return undef unless IPC::Cmd::can_run('uname');

    open UNAME, "uname -X 2>/dev/null|" or return '';
    my $line = "";
    my $os = "";
    while ( <UNAME> ) {
        chop;
        $line = $_ if m@^Release@;
        $os = $_ if m@^System@;
    }
    close UNAME;

    return undef if $line eq '' or $os eq 'System = SunOS';

    my @fields = split(/\s+/, $line);
    return $fields[2];
}

sub get_sco_type {
    my $REL = shift;

    if ( -f "/etc/kconfig" ) {
        return "${MACHINE}-whatever-isc4" if $REL eq '4.0' || $REL eq '4.1';
    } else {
        return "whatever-whatever-sco3" if $REL eq '3.2v4.2';
        return "whatever-whatever-sco5" if $REL =~ m@3\.2v5\.0.*@;
        if ( $REL eq "4.2MP" ) {
            return "whatever-whatever-unixware20" if $VERSION =~ m@2\.0.*@;
            return "whatever-whatever-unixware21" if $VERSION =~ m@2\.1.*@;
            return "whatever-whatever-unixware2" if $VERSION =~ m@2.*@;
        }
        return "whatever-whatever-unixware1" if $REL eq "4.2";
        if ( $REL =~ m@5.*@ ) {
            # We hardcode i586 in place of ${MACHINE} for the following
            # reason: even though Pentium is minimum requirement for
            # platforms in question, ${MACHINE} gets always assigned to
            # i386. This means i386 gets passed to Configure, which will
            # cause bad assembler code to be generated.
            return "i586-sco-unixware7" if $VERSION =~ m@[678].*@;
        }
    }
}

# Return the cputype-vendor-osversion
sub guess_system {
    ($SYSTEM, undef, $RELEASE, $VERSION, $MACHINE) = POSIX::uname();
    my $sys = "${SYSTEM}:${RELEASE}:${VERSION}:${MACHINE}";
    
    # Special-cases for ISC, SCO, Unixware
    my $REL = is_sco_uname();
    if ( defined $REL ) {
        my $result = get_sco_type($REL);
        return eval "\"$result\"" if $result ne '';
    }

    # Now pattern-match

    # Simple cases
    foreach my $tuple ( @$guess_patterns ) {
        my $pat = @$tuple[0];
        my $check = ref $pat eq 'CODE' ? $pat->($sys) : $sys =~ /^(${pat})$/;
        next unless $check;

        my $result = @$tuple[1];
        $result = $result->() if ref $result eq 'CODE';
        return eval "\"$result\"";
    }

    # Oh well.
    return "${MACHINE}-whatever-${SYSTEM}";
}

# We would use List::Util::pair() for this...  unfortunately, that function
# only appeared in perl v5.19.3, and we claim to support perl v5.10 and on.
# Therefore, we implement a quick cheap variant of our own.
sub _pairs (@) {
    croak "Odd number of arguments" if @_ & 1;

    my @pairlist = ();

    while (@_) {
        my $x = [ shift, shift ];
        push @pairlist, $x;
    }
    return @pairlist;
}

# Figure out CC, GCCVAR, etc.
sub determine_compiler_settings {
    # Make a copy and don't touch it.  That helps determine if we're finding
    # the compiler here (false), or if it was set by the user (true.
    my $cc = $CC;

    # Set certain default
    $CCVER = 0;                 # Unknown
    $CCVENDOR = '';             # Dunno, don't care (unless found later)

    # Find a compiler if we don't already have one
    if ( ! $cc ) {
        foreach (@c_compilers) {
            next unless IPC::Cmd::can_run("$CROSS_COMPILE$_");
            $CC = $_;
            last;
        }
    }

    if ( $CC ) {
        # Find the compiler vendor and version number for certain compilers
        foreach my $pair (_pairs @cc_version) {
            # Try to get the version number.
            # Failure gets us undef or an empty string
            my ( $k, $v ) = @$pair;
            $v = $v->();

            # If we got a version number, process it
            if ($v) {
                $v =~ s/[^.]*.0*// if $SYSTEM eq 'HP-UX';
                $CCVENDOR = $k;

                # The returned version is expected to be one of
                #
                # MAJOR
                # MAJOR.MINOR
                # MAJOR.MINOR.{whatever}
                #
                # We don't care what comes after MAJOR.MINOR.  All we need is
                # to have them calculated into a single number, using this
                # formula:
                #
                # MAJOR * 100 + MINOR
                # Here are a few examples of what we should get:
                #
                # 2.95.1    => 295
                # 3.1       => 301
                # 9         => 900
                my @numbers = split /\./, $v;
                my @factors = (100, 1);
                while (@numbers && @factors) {
                    $CCVER += shift(@numbers) * shift(@factors)
                }
                last;
            }
        }
    }

    # Vendor specific overrides, only if we didn't determine the compiler here
    if ( ! $cc ) {
        if ( $SYSTEM eq 'OpenVMS' ) {
            my $v = `CC/VERSION NLA0:`;
            if ($? == 0) {
                # The normal releases have a version number prefixed with a V.
                # However, other letters have been seen as well (for example X),
                # and it's documented that HP (now VSI) reserve the letter W, X,
                # Y and Z for their own uses.
                my ($vendor, $arch, $version, $extra) =
                    ( $v =~ m/^
                              ([A-Z]+)                  # Usually VSI
                              \s+ C
                              (?:\s+(.*?))?             # Possible build arch
                              \s+ [VWXYZ]([0-9\.-]+)    # Version
                              (?:\s+\((.*?)\))?         # Possible extra data
                              \s+ on
                             /x );
                my ($major, $minor, $patch) =
                    ( $version =~ m/^([0-9]+)\.([0-9]+)-0*?(0|[1-9][0-9]*)$/ );
                $CC = 'CC';
                $CCVENDOR = $vendor;
                $CCVER = ( $major * 100 + $minor ) * 100 + $patch;
            }
        }

        if ( ${SYSTEM} eq 'AIX' ) {
            # favor vendor cc over gcc
            if (IPC::Cmd::can_run('cc')) {
                $CC = 'cc';
                $CCVENDOR = ''; # Determine later
                $CCVER = 0;
            }
        }

        if ( $SYSTEM eq "SunOS" ) {
            # check for Oracle Developer Studio, expected output is "cc: blah-blah C x.x blah-blah"
            my $v = `(cc -V 2>&1) 2>/dev/null | egrep -e '^cc: .* C [0-9]\.[0-9]'`;
            my @numbers = 
                    ( $v =~ m/^.* C ([0-9]+)\.([0-9]+) .*/ );
            my @factors = (100, 1);
            $v = 0;
            while (@numbers && @factors) {
                $v += shift(@numbers) * shift(@factors)
            }

            if ($v > 500) {
                $CC = 'cc';
                $CCVENDOR = 'sun';
                $CCVER = $v;
            }
        }

        # 'Windows NT' is the system name according to POSIX::uname()!
        if ( $SYSTEM eq "Windows NT" ) {
            # favor vendor cl over gcc
            if (IPC::Cmd::can_run('cl')) {
                $CC = 'cl';
                $CCVENDOR = ''; # Determine later
                $CCVER = 0;

                my $v = `cl 2>&1`;
                if ( $v =~ /Microsoft .* Version ([0-9\.]+) for (x86|x64|ARM|ia64)/ ) {
                    $CCVER = $1;
                    $CL_ARCH = $2;
                }
            }
        }
    }

    # If no C compiler has been determined at this point, we die.  Hard.
    die <<_____
ERROR!
No C compiler found, please specify one with the environment variable CC,
or configure with an explicit configuration target.
_____
        unless $CC;

    # On some systems, we assume a cc vendor if it's not already determined

    if ( ! $CCVENDOR ) {
        $CCVENDOR = 'aix' if $SYSTEM eq 'AIX';
        $CCVENDOR = 'sun' if $SYSTEM eq 'SunOS';
    }

    # Some systems need to know extra details

    if ( $SYSTEM eq "HP-UX" && $CCVENDOR eq 'gnu' ) {
        # By default gcc is a ILP32 compiler (with long long == 64).
        $GCC_BITS = "32";
        if ( $CCVER >= 300 ) {
            # PA64 support only came in with gcc 3.0.x.
            # We check if the preprocessor symbol __LP64__ is defined.
            if ( okrun('echo __LP64__',
                       "$CC -v -E -x c - 2>/dev/null",
                       'grep "^__LP64__" 2>&1 >/dev/null') ) {
                # __LP64__ has slipped through, it therefore is not defined
            } else {
                $GCC_BITS = '64';
            }
        }
    }

    if ( $SYSTEM eq "SunOS" && $CCVENDOR eq 'gnu' ) {
        if ( $CCVER >= 300 ) {
            # 64-bit ABI isn't officially supported in gcc 3.0, but seems
            # to be working; at the very least 'make test' passes.
            if ( okrun("$CC -v -E -x c /dev/null 2>&1",
                       'grep __arch64__ >/dev/null') ) {
                $GCC_ARCH = "-m64"
            } else {
                $GCC_ARCH = "-m32"
            }
        }
    }

    if ($VERBOSE) {
        my $vendor = $CCVENDOR ? $CCVENDOR : "(undetermined)";
        my $version = $CCVER ? $CCVER : "(undetermined)";
        print "C compiler: $CC\n";
        print "C compiler vendor: $vendor\n";
        print "C compiler version: $version\n";
    }
}

my $map_patterns =
    [ [ 'uClinux.*64.*',          { target => 'uClinux-dist64' } ],
      [ 'uClinux.*',              { target => 'uClinux-dist' } ],
      [ 'mips3-sgi-irix',         { target => 'irix-mips3' } ],
      [ 'mips4-sgi-irix64',
        sub {
            print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure irix64-mips4-$CC
EOF
            maybe_abort();
            return { target => "irix-mips3" };
        }
      ],
      [ 'ppc-apple-rhapsody',     { target => "rhapsody-ppc" } ],
      [ 'ppc-apple-darwin.*',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS} // '';
            my $ISA64 = `sysctl -n hw.optional.64bitops 2>/dev/null`;
            if ( $ISA64 == 1 && $KERNEL_BITS eq '' ) {
                print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure darwin64-ppc-cc
EOF
                maybe_abort();
            }
            return { target => "darwin64-ppc" }
                if $ISA64 == 1 && $KERNEL_BITS eq '64';
            return { target => "darwin-ppc" };
        }
      ],
      [ 'i.86-apple-darwin.*',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS} // '';
            my $ISA64 = `sysctl -n hw.optional.x86_64 2>/dev/null`;
            if ( $ISA64 == 1 && $KERNEL_BITS eq '' ) {
                print <<EOF;
WARNING! To build 64-bit package, do this:
         KERNEL_BITS=64 $WHERE/Configure [options...]
EOF
                maybe_abort();
            }
            return { target => "darwin64-x86_64" }
                if $ISA64 == 1 && $KERNEL_BITS eq '64';
            return { target => "darwin-i386" };
        }
      ],
      [ 'x86_64-apple-darwin.*',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS} // '';
            # macOS >= 10.15 is 64-bit only
            my $SW_VERS = `sw_vers -productVersion 2>/dev/null`;
            if ($SW_VERS =~ /^(\d+)\.(\d+)\.(\d+)$/) {
                if ($1 > 10 || ($1 == 10 && $2 >= 15)) {
                    die "32-bit applications not supported on macOS 10.15 or later\n" if $KERNEL_BITS eq '32';
                    return { target => "darwin64-x86_64" };
                }
            }
            return { target => "darwin-i386" } if $KERNEL_BITS eq '32';

            print <<EOF;
WARNING! To build 32-bit package, do this:
         KERNEL_BITS=32 $WHERE/Configure [options...]
EOF
            maybe_abort();
            return { target => "darwin64-x86_64" };
        }
      ],
      [ 'arm64-apple-darwin.*', { target => "darwin64-arm64" } ],
      [ 'armv6\+7-.*-iphoneos',
        { target => "iphoneos-cross",
          cflags => [ qw(-arch armv6 -arch armv7) ],
          cxxflags => [ qw(-arch armv6 -arch armv7) ] }
      ],
      [ 'arm64-.*-iphoneos|.*-.*-ios64',
        { target => "ios64-cross" }
      ],
      [ '.*-.*-iphoneos',
        sub { return { target => "iphoneos-cross",
                       cflags => [ "-arch ${MACHINE}" ],
                       cxxflags => [ "-arch ${MACHINE}" ] }; }
      ],
      [ 'alpha-.*-linux2.*',
        sub {
            my $ISA = `awk '/cpu model/{print \$4;exit(0);}' /proc/cpuinfo`;
            $ISA //= 'generic';
            my %config = ();
            if ( $CCVENDOR eq "gnu" ) {
                if ( $ISA =~ 'EV5|EV45' ) {
                    %config = ( cflags => [ '-mcpu=ev5' ],
                                cxxflags =>  [ '-mcpu=ev5' ] );
                } elsif ( $ISA =~ 'EV56|PCA56' ) {
                    %config = ( cflags => [ '-mcpu=ev56' ],
                                cxxflags =>  [ '-mcpu=ev56' ] );
                } else {
                    %config = ( cflags => [ '-mcpu=ev6' ],
                                cxxflags =>  [ '-mcpu=ev6' ] );
                }
            }
            return { target => "linux-alpha",
                     %config };
        }
      ],
      [ 'ppc64-.*-linux2',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS} // '';
            if ( $KERNEL_BITS eq '' ) {
                print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure linux-ppc64
EOF
                maybe_abort();
            }
            return { target => "linux-ppc64" } if $KERNEL_BITS eq '64';

            my %config = ();
            if (!okrun('echo __LP64__',
                       'gcc -E -x c - 2>/dev/null',
                       'grep "^__LP64__" 2>&1 >/dev/null') ) {
                %config = ( cflags => [ '-m32' ],
                            cxxflags =>  [ '-m32' ] );
            }
            return { target => "linux-ppc",
                     %config };
        }
      ],
      [ 'ppc64le-.*-linux2',      { target => "linux-ppc64le" } ],
      [ 'ppc-.*-linux2',          { target => "linux-ppc" } ],
      [ 'mips64.*-*-linux2',
        sub {
            print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure linux64-mips64
EOF
            maybe_abort();
            return { target => "linux-mips64" };
        }
      ],
      [ 'mips.*-.*-linux2',       { target => "linux-mips32" } ],
      [ 'ppc60x-.*-vxworks.*',    { target => "vxworks-ppc60x" } ],
      [ 'ppcgen-.*-vxworks.*',    { target => "vxworks-ppcgen" } ],
      [ 'pentium-.*-vxworks.*',   { target => "vxworks-pentium" } ],
      [ 'simlinux-.*-vxworks.*',  { target => "vxworks-simlinux" } ],
      [ 'mips-.*-vxworks.*',      { target => "vxworks-mips" } ],
      [ 'e2k-.*-linux.*',         { target => "linux-generic64",
                                    defines => [ 'L_ENDIAN' ] } ],
      [ 'ia64-.*-linux.',         { target => "linux-ia64" } ],
      [ 'sparc64-.*-linux2',
        sub {
            print <<EOF;
WARNING! If you *know* that your GNU C supports 64-bit/V9 ABI and you
         want to build 64-bit library, do this:
         $WHERE/Configure linux64-sparcv9
EOF
            maybe_abort();
            return { target => "linux-sparcv9" };
        }
      ],
      [ 'sparc-.*-linux2',
        sub {
            my $KARCH = `awk '/^type/{print \$3;exit(0);}' /proc/cpuinfo`;
            $KARCH //= "sun4";
            return { target => "linux-sparcv9" } if $KARCH =~ 'sun4u.*';
            return { target => "linux-sparcv8" } if $KARCH =~ 'sun4[md]';
            return { target => "linux-generic32",
                     defines => [ 'L_ENDIAN' ] };
        }
      ],
      [ 'parisc.*-.*-linux2',
        sub {
            # 64-bit builds under parisc64 linux are not supported and
            # compiler is expected to generate 32-bit objects...
            my $CPUARCH =
                `awk '/cpu family/{print substr(\$5,1,3); exit(0);}' /proc/cpuinfo`;
            my $CPUSCHEDULE =
                `awk '/^cpu.[ 	]*: PA/{print substr(\$3,3); exit(0);}' /proc/cpuinfo`;
            # TODO XXX  Model transformations
            # 0. CPU Architecture for the 1.1 processor has letter suffixes.
            #    We strip that off assuming no further arch. identification
            #    will ever be used by GCC.
            # 1. I'm most concerned about whether is a 7300LC is closer to a
            #    7100 versus a 7100LC.
            # 2. The variant 64-bit processors cause concern should GCC support
            #    explicit schedulers for these chips in the future.
            #         PA7300LC -> 7100LC (1.1)
            #         PA8200   -> 8000   (2.0)
            #         PA8500   -> 8000   (2.0)
            #         PA8600   -> 8000   (2.0)
            $CPUSCHEDULE =~ s/7300LC/7100LC/;
            $CPUSCHEDULE =~ s/8.00/8000/;
            return
                { target => "linux-generic32",
                  defines => [ 'B_ENDIAN' ],
                  cflags => [ "-mschedule=$CPUSCHEDULE", "-march=$CPUARCH" ],
                  cxxflags => [ "-mschedule=$CPUSCHEDULE", "-march=$CPUARCH" ]
                };
        }
      ],
      [ 'armv[1-3].*-.*-linux2',  { target => "linux-generic32" } ],
      [ 'armv[7-9].*-.*-linux2',  { target => "linux-armv4",
                                    cflags => [ '-march=armv7-a' ],
                                    cxxflags => [ '-march=armv7-a' ] } ],
      [ 'arm.*-.*-linux2',        { target => "linux-armv4" } ],
      [ 'aarch64-.*-linux2',      { target => "linux-aarch64" } ],
      [ 'sh.*b-.*-linux2',        { target => "linux-generic32",
                                    defines => [ 'B_ENDIAN' ] } ],
      [ 'sh.*-.*-linux2',         { target => "linux-generic32",
                                    defines => [ 'L_ENDIAN' ] } ],
      [ 'loongarch64-.*-linux2',
        sub {
            my $disable = [ 'asm' ];
            if ( okrun('echo xvadd.w \$xr0,\$xr0,\$xr0',
                       "$CC -c -x assembler - -o /dev/null 2>/dev/null") ) {
                $disable = [];
            }
            return { target => "linux64-loongarch64",
                     disable => $disable, };
        }
      ],
      [ 'm68k.*-.*-linux2',       { target => "linux-generic32",
                                    defines => [ 'B_ENDIAN' ] } ],
      [ 's390-.*-linux2',         { target => "linux-generic32",
                                    defines => [ 'B_ENDIAN' ] } ],
      [ 's390x-.*-linux2',
        sub {
            # Disabled until a glibc bug is fixed; see Configure.
            if (0
                || okrun('egrep -e \'^features.* highgprs\' /proc/cpuinfo >/dev/null') )
                {
                    print <<EOF;
WARNING! To build "highgprs" 32-bit package, do this:
         $WHERE/Configure linux32-s390x
EOF
                    maybe_abort();
                }
            return { target => "linux64-s390x" };
        }
      ],
      [ 'x86_64-.*-linux.',
        sub {
            return { target => "linux-x32" }
                if okrun("$CC -dM -E -x c /dev/null 2>&1",
                         'grep -q ILP32 >/dev/null');
            return { target => "linux-x86_64" };
        }
      ],
      [ '.*86-.*-linux2',
        sub {
            # On machines where the compiler understands -m32, prefer a
            # config target that uses it
            return { target => "linux-x86" }
                if okrun("$CC -m32 -E -x c /dev/null >/dev/null 2>&1");
            return { target => "linux-elf" };
        }
      ],
      [ '.*86-.*-linux1',         { target => "linux-aout" } ],
      [ 'riscv64-.*-linux.',      { target => "linux64-riscv64" } ],
      [ '.*-.*-linux.',           { target => "linux-generic32" } ],
      [ 'sun4[uv].*-.*-solaris2',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS};
            my $ISA64 = `isainfo 2>/dev/null | grep sparcv9`;
            my $KB = $KERNEL_BITS // '64';
            if ( $ISA64 ne "" && $KB eq '64' ) {
                if ( $CCVENDOR eq "sun" && $CCVER >= 500 ) {
                    print <<EOF;
WARNING! To build 32-bit package, do this:
         $WHERE/Configure solaris-sparcv9-cc
EOF
                    maybe_abort();
                } elsif ( $CCVENDOR eq "gnu" && $GCC_ARCH eq "-m64" ) {
                    # $GCC_ARCH denotes default ABI chosen by compiler driver
                    # (first one found on the $PATH). I assume that user
                    # expects certain consistency with the rest of his builds
                    # and therefore switch over to 64-bit. <appro>
                    print <<EOF;
WARNING! To build 32-bit package, do this:
         $WHERE/Configure solaris-sparcv9-gcc
EOF
                    maybe_abort();
                    return { target => "solaris64-sparcv9-gcc" };
                } elsif ( $GCC_ARCH eq "-m32" ) {
                    print <<EOF;
NOTICE! If you *know* that your GNU C supports 64-bit/V9 ABI and you wish
        to build 64-bit library, do this:
        $WHERE/Configure solaris64-sparcv9-gcc
EOF
                    maybe_abort();
                }
            }
            return { target => "solaris64-sparcv9-cc" }
                if $ISA64 ne "" && $KB eq '64';
            return { target => "solaris-sparcv9-cc" };
        }
      ],
      [ 'sun4m-.*-solaris2',      { target => "solaris-sparcv8" } ],
      [ 'sun4d-.*-solaris2',      { target => "solaris-sparcv8" } ],
      [ 'sun4.*-.*-solaris2',     { target => "solaris-sparcv7" } ],
      [ '.*86.*-.*-solaris2',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS};
            my $ISA64 = `isainfo 2>/dev/null | grep amd64`;
            my $KB = $KERNEL_BITS // '64';
            if ($ISA64 ne "" && $KB eq '64') {
                return { target => "solaris64-x86_64-gcc" } if $CCVENDOR eq "gnu";
                return { target => "solaris64-x86_64-cc" };
            }
            my $REL = uname('-r');
            $REL =~ s/5\.//;
            my @tmp_disable = ();
            push @tmp_disable, 'sse2' if int($REL) < 10;
            #There is no solaris-x86-cc target
            return { target => "solaris-x86-gcc",
                     disable => [ @tmp_disable ] };
        }
      ],
      # We don't have any sunos target in Configurations/*.conf, so why here?
      [ '.*-.*-sunos4',           { target => "sunos" } ],
      [ '.*86.*-.*-bsdi4',        { target => "BSD-x86-elf",
                                    lflags => [ '-ldl' ],
                                    disable => [ 'sse2' ] } ],
      [ 'alpha.*-.*-.*bsd.*',     { target => "BSD-generic64",
                                    defines => [ 'L_ENDIAN' ] } ],
      [ 'powerpc-.*-.*bsd.*',     { target => "BSD-ppc" } ],
      [ 'powerpc64-.*-.*bsd.*',   { target => "BSD-ppc64" } ],
      [ 'powerpc64le-.*-.*bsd.*', { target => "BSD-ppc64le" } ],
      [ 'riscv64-.*-.*bsd.*',     { target => "BSD-riscv64" } ],
      [ 'sparc64-.*-.*bsd.*',     { target => "BSD-sparc64" } ],
      [ 'ia64-.*-openbsd.*',      { target => "BSD-nodef-ia64" } ],
      [ 'ia64-.*-.*bsd.*',        { target => "BSD-ia64" } ],
      [ 'x86_64-.*-dragonfly.*',  { target => "BSD-x86_64" } ],
      [ 'amd64-.*-openbsd.*',     { target => "BSD-nodef-x86_64" } ],
      [ 'amd64-.*-.*bsd.*',       { target => "BSD-x86_64" } ],
      [ 'arm64-.*-.*bsd.*',       { target => "BSD-aarch64" } ],
      [ 'armv6-.*-.*bsd.*',       { target => "BSD-armv4" } ],
      [ 'armv7-.*-.*bsd.*',       { target => "BSD-armv4" } ],
      [ '.*86.*-.*-.*bsd.*',
        sub {
            # mimic ld behaviour when it's looking for libc...
            my $libc;
            if ( -l "/usr/lib/libc.so" ) {
                $libc = "/usr/lib/libc.so";
            } else {
                # ld searches for highest libc.so.* and so do we
                $libc =
                    `(ls /usr/lib/libc.so.* /lib/libc.so.* | tail -1) 2>/dev/null`;
            }
            my $what = `file -L $libc 2>/dev/null`;
            return { target => "BSD-x86-elf" } if $what =~ /ELF/;
            return { target => "BSD-x86",
                     disable => [ 'sse2' ] };
        }
      ],
      [ '.*-.*-openbsd.*',        { target => "BSD-nodef-generic32" } ],
      [ '.*-.*-.*bsd.*',          { target => "BSD-generic32" } ],
      [ 'x86_64-.*-haiku',        { target => "haiku-x86_64" } ],
      [ '.*-.*-haiku',            { target => "haiku-x86" } ],
      [ '.*-.*-osf',              { target => "osf1-alpha" } ],
      [ '.*-.*-tru64',            { target => "tru64-alpha" } ],
      [ '.*-.*-[Uu]nix[Ww]are7',
        sub {
            return { target => "unixware-7",
                     disable => [ 'sse2' ] } if $CCVENDOR eq "gnu";
            return { target => "unixware-7",
                     defines => [ '__i386__' ] };
        }
      ],
      [ '.*-.*-[Uu]nix[Ww]are20.*', { target => "unixware-2.0",
                                      disable => [ 'sse2', 'sha512' ] } ],
      [ '.*-.*-[Uu]nix[Ww]are21.*', { target => "unixware-2.1",
                                      disable => [ 'sse2', 'sha512' ] } ],
      [ '.*-.*-vos',              { target => "vos",
                                    disable => [ 'threads', 'shared', 'asm',
                                                 'dso' ] } ],
      [ 'BS2000-siemens-sysv4',   { target => "BS2000-OSD" } ],
      [ 'i[3456]86-.*-cygwin',    { target => "Cygwin-x86" } ],
      [ '.*-.*-cygwin',
        sub { return { target => "Cygwin-${MACHINE}" } } ],
      [ 'x86-.*-android|i.86-.*-android', { target => "android-x86" } ],
      [ 'armv[7-9].*-.*-android', { target => "android-armeabi",
                                    cflags => [ '-march=armv7-a' ],
                                    cxxflags => [ '-march=armv7-a' ] } ],
      [ 'arm.*-.*-android',       { target => "android-armeabi" } ],
      [ 'riscv64-.*-android',     { target => "android-riscv64" } ],
      [ '.*-hpux1.*',
        sub {
            my $KERNEL_BITS = $ENV{KERNEL_BITS};
            my %common_return = ( defines => [ '_REENTRANT' ] );
            $KERNEL_BITS ||= `getconf KERNEL_BITS 2>/dev/null` // '32';
            # See <sys/unistd.h> for further info on CPU_VERSION.
            my $CPU_VERSION = `getconf CPU_VERSION 2>/dev/null` // 0;
            if ( $CPU_VERSION >= 768 ) {
                # IA-64 CPU
                return { target => "hpux64-ia64",
                         %common_return }
                    if $KERNEL_BITS eq '64' && ! $CCVENDOR;
                return { target => "hpux-ia64",
                         %common_return };
            }
            if ( $CPU_VERSION >= 532 ) {
                # PA-RISC 2.x CPU
                # PA-RISC 2.0 is no longer supported as separate 32-bit
                # target. This is compensated for by run-time detection
                # in most critical assembly modules and taking advantage
                # of 2.0 architecture in PA-RISC 1.1 build.
                my $target = ($CCVENDOR eq "gnu" && $GCC_BITS eq '64')
                    ? "hpux64-parisc2"
                    : "hpux-parisc1_1";
                if ( $KERNEL_BITS eq '64' && ! $CCVENDOR ) {
                    print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure hpux64-parisc2-cc
EOF
                    maybe_abort();
                }
                return { target => $target,
                         %common_return };
            }
            # PA-RISC 1.1+ CPU?
            return { target => "hpux-parisc1_1",
                     %common_return } if $CPU_VERSION >= 528;
            # PA-RISC 1.0 CPU
            return { target => "hpux-parisc",
                     %common_return } if $CPU_VERSION >= 523;
            # Motorola(?) CPU
            return { target => "hpux",
                     %common_return };
        }
      ],
      [ '.*-hpux',                { target => "hpux-parisc" } ],
      [ '.*-aix',
        sub {
            my %config = ();
            my $KERNEL_BITS = $ENV{KERNEL_BITS};
            $KERNEL_BITS ||= `getconf KERNEL_BITMODE 2>/dev/null`;
            $KERNEL_BITS ||= '32';
            my $OBJECT_MODE = $ENV{OBJECT_MODE};
            $OBJECT_MODE ||= 32;
            $config{target} = "aix";
            if ( $OBJECT_MODE == 64 ) {
                print 'Your $OBJECT_MODE was found to be set to 64';
                $config{target} = "aix64";
            } else {
                if ( $CCVENDOR ne 'gnu' && $KERNEL_BITS eq '64' ) {
                    print <<EOF;
WARNING! To build 64-bit package, do this:
         $WHERE/Configure aix64-cc
EOF
                    maybe_abort();
                }
            }
            if ( okrun(
                       "(lsattr -E -O -l `lsdev -c processor|awk '{print \$1;exit}'`",
                       'grep -i powerpc) >/dev/null 2>&1') ) {
                # this applies even to Power3 and later, as they return
                # PowerPC_POWER[345]
            } else {
                $config{disable} = [ 'asm' ];
            }
            return { %config };
        }
      ],

      # Windows values found by looking at Perl 5's win32/win32.c
      [ '(amd64|ia64|x86|ARM)-.*?-Windows NT',
        sub {
            # If we determined the arch by asking cl, take that value,
            # otherwise the SYSTEM we got from from POSIX::uname().
            my $arch = $CL_ARCH // $1;
            my $config;

            if ($arch) {
                $config = { 'amd64' => { target => 'VC-WIN64A'    },
                            'ia64'  => { target => 'VC-WIN64I'    },
                            'x86'   => { target => 'VC-WIN32'     },
                            'x64'   => { target => 'VC-WIN64A'    },
                            'ARM'   => { target => 'VC-WIN64-ARM' },
                          } -> {$arch};
                die <<_____ unless defined $config;
ERROR
I do not know how to handle ${arch}.
_____
            }
            die <<_____ unless defined $config;
ERROR
Could not figure out the architecture.
_____

            return $config;
        }
      ],

      # VMS values found by observation on existing machinery.
      [ 'VMS_AXP-.*?-OpenVMS',    { target => 'vms-alpha'  } ],
      [ 'VMS_IA64-.*?-OpenVMS',   { target => 'vms-ia64'   } ],
      [ 'VMS_x86_64-.*?-OpenVMS', { target => 'vms-x86_64' } ],

      # TODO: There are a few more choices among OpenSSL config targets, but
      # reaching them involves a bit more than just a host tripet.  Select
      # environment variables could do the job to cover for more granular
      # build options such as data model (ILP32 or LP64), thread support
      # model (PUT, SPT or nothing), target execution environment (OSS or
      # GUARDIAN).  And still, there must be some kind of default when
      # nothing else is said.
      #
      # nsv is a virtual x86 environment, equivalent to nsx, so we enforce
      # the latter.
      [ 'nse-tandem-nsk.*',       { target => 'nonstop-nse' } ],
      [ 'nsv-tandem-nsk.*',       { target => 'nonstop-nsx' } ],
      [ 'nsx-tandem-nsk.*',       { target => 'nonstop-nsx' } ],

    ];

# Map GUESSOS into OpenSSL terminology.
# Returns a hash table with diverse entries, most importantly 'target',
# but also other entries that are fitting for Configure's %config
# and MACHINE.
# It would be nice to fix this so that this weren't necessary. :( XXX
sub map_guess {
    my $GUESSOS = shift;

    foreach my $tuple ( @$map_patterns ) {
        my $pat = @$tuple[0];
        next if $GUESSOS !~ /^${pat}$/;
        my $result = @$tuple[1];
        $result = $result->() if ref $result eq 'CODE';
        return %$result;
    }

    # Last case, return "z" from x-y-z
    my @fields = split(/-/, $GUESSOS);
    return ( target => $fields[2] );
}

# gcc < 2.8 does not support -march=ultrasparc
sub check_solaris_sparc8 {
    my $OUT = shift;
    if ( $CCVENDOR eq 'gnu' && $CCVER < 208 ) {
        if ( $OUT eq 'solaris-sparcv9-gcc' ) {
            print <<EOF;
WARNING! Downgrading to solaris-sparcv8-gcc
         Upgrade to gcc-2.8 or later.
EOF
            maybe_abort();
            return 'solaris-sparcv8-gcc';
        }
        if ( $OUT eq "linux-sparcv9" ) {
            print <<EOF;
WARNING! Downgrading to linux-sparcv8
         Upgrade to gcc-2.8 or later.
EOF
            maybe_abort();
            return 'linux-sparcv8';
        }
    }
    return $OUT;
}

###
###   MAIN PROCESSING
###

sub get_platform {
    my %options = @_;

    $VERBOSE = 1 if defined $options{verbose};
    $WAIT = 0 if defined $options{nowait};
    $CC = $options{CC};
    $CROSS_COMPILE = $options{CROSS_COMPILE} // '';

    my $GUESSOS = guess_system();
    determine_compiler_settings();

    my %ret = map_guess($GUESSOS);
    $ret{target} = check_solaris_sparc8($ret{target});
    return %ret;
}

1;
