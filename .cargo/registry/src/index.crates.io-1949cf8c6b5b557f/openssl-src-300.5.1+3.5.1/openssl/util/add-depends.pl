#! /usr/bin/env perl
# Copyright 2018-2022 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;
use warnings;

use lib '.';
use configdata;

use File::Spec::Functions qw(:DEFAULT rel2abs);
use File::Compare qw(compare_text);
use feature 'state';

# When using stat() on Windows, we can get it to perform better by avoid some
# data.  This doesn't affect the mtime field, so we're not losing anything...
${^WIN32_SLOPPY_STAT} = 1;

my $debug = $ENV{ADD_DEPENDS_DEBUG};
my $buildfile = $config{build_file};
my $build_mtime = (stat($buildfile))[9];
my $configdata_mtime = (stat('configdata.pm'))[9];
my $rebuild = 0;
my $depext = $target{dep_extension} || ".d";
my @depfiles =
    sort
    grep {
        # This grep has side effects.  Not only does if check the existence
        # of the dependency file given in $_, but it also checks if it's
        # newer than the build file or older than configdata.pm, and if it
        # is, sets $rebuild.
        my @st = stat($_);
        $rebuild = 1
            if @st && ($st[9] > $build_mtime || $st[9] < $configdata_mtime);
        scalar @st > 0;         # Determines the grep result
    }
    map { (my $x = $_) =~ s|\.o$|$depext|; $x; }
    ( ( grep { $unified_info{sources}->{$_}->[0] =~ /\.cc?$/ }
            keys %{$unified_info{sources}} ),
      ( grep { $unified_info{shared_sources}->{$_}->[0] =~ /\.cc?$/ }
            keys %{$unified_info{shared_sources}} ) );

exit 0 unless $rebuild;

# Ok, primary checks are done, time to do some real work

my $producer = shift @ARGV;
die "Producer not given\n" unless $producer;

my $srcdir = $config{sourcedir};
my $blddir = $config{builddir};
my $abs_srcdir = rel2abs($srcdir);
my $abs_blddir = rel2abs($blddir);

# Convenient cache of absolute to relative map.  We start with filling it
# with mappings for the known generated header files.  They are relative to
# the current working directory, so that's an easy task.
# NOTE: there's more than C header files that are generated.  They will also
# generate entries in this map.  We could of course deal with C header files
# only, but in case we decide to handle more than just C files in the future,
# we already have the mechanism in place here.
# NOTE2: we lower case the index to make it searchable without regard for
# character case.  That could seem dangerous, but as long as we don't have
# files we depend on in the same directory that only differ by character case,
# we're fine.
my %depconv_cache =
    map { catfile($abs_blddir, $_) => $_ }
    keys %{$unified_info{generate}};

my %procedures = (
    'gcc' =>
        sub {
            (my $objfile = shift) =~ s|\.d$|.o|i;
            my $line = shift;

            # Remove the original object file
            $line =~ s|^.*\.o: | |;
            # All we got now is a dependency, shave off surrounding spaces
            $line =~ s/^\s+//;
            $line =~ s/\s+$//;
            # Also, shave off any continuation
            $line =~ s/\s*\\$//;

            # Split the line into individual header files, and keep those
            # that exist in some form
            my @headers;
            for (split(/\s+/, $line)) {
                my $x = rel2abs($_);

                if (!$depconv_cache{$x}) {
                    if (-f $x) {
                        $depconv_cache{$x} = $_;
                    }
                }

                if ($depconv_cache{$x}) {
                    push @headers, $_;
                } else {
                    print STDERR "DEBUG[$producer]: ignoring $objfile <- $line\n"
                        if $debug;
                }
            }
            return ($objfile, join(' ', @headers)) if @headers;
            return undef;
    },
    'makedepend' =>
        sub {
            # makedepend, in its infinite wisdom, wants to have the object file
            # in the same directory as the source file.  This doesn't work too
            # well with out-of-source-tree builds, so we must resort to tricks
            # to get things right.  Fortunately, the .d files are always placed
            # parallel with the object files, so all we need to do is construct
            # the object file name from the dep file name.
            (my $objfile = shift) =~ s|\.d$|.o|i;
            my $line = shift;

            # Discard comments
            return undef if $line =~ /^(#.*|\s*)$/;

            # Remove the original object file
            $line =~ s|^.*\.o: | |;
            # Also, remove any dependency that starts with a /, because those
            # are typically system headers
            $line =~ s/\s+\/(\\.|\S)*//g;
            # Finally, discard all empty lines
            return undef if $line =~ /^\s*$/;

            # All we got now is a dependency, just shave off surrounding spaces
            $line =~ s/^\s+//;
            $line =~ s/\s+$//;
            return ($objfile, $line);
        },
    'VMS C' =>
        sub {
            state $abs_srcdir_shaved = undef;
            state $srcdir_shaved = undef;

            unless (defined $abs_srcdir_shaved) {
                ($abs_srcdir_shaved = $abs_srcdir) =~ s|[>\]]$||;
                ($srcdir_shaved = $srcdir) =~ s|[>\]]$||;
            }

            # current versions of DEC / Compaq / HP / VSI C strips away all
            # directory information from the object file, so we must insert it
            # back.  To make life simpler, we simply replace it with the
            # corresponding .D file that's had its extension changed.  Since
            # .D files are always written parallel to the object files, we
            # thereby get the directory information for free.
            (my $objfile = shift) =~ s|\.D$|.OBJ|i;
            my $line = shift;

            # Shave off the target.
            #
            # The pattern for target and dependencies will always take this
            # form:
            #
            #   target SPACE : SPACE deps
            #
            # This is so a volume delimiter (a : without any spaces around it)
            # won't get mixed up with the target / deps delimiter.  We use this
            # to easily identify what needs to be removed.
            m|\s:\s|; $line = $';

            # We know that VMS has system header files in text libraries,
            # extension .TLB.  We also know that our header files aren't stored
            # in text libraries.  Finally, we know that VMS C produces exactly
            # one dependency per line, so we simply discard any line ending with
            # .TLB.
            return undef if /\.TLB\s*$/;

            # All we got now is a dependency, just shave off surrounding spaces
            $line =~ s/^\s+//;
            $line =~ s/\s+$//;

            # VMS C gives us absolute paths, always.  Let's see if we can
            # make them relative instead.
            $line = canonpath($line);

            unless (defined $depconv_cache{$line}) {
                my $dep = $line;
                # Since we have already pre-populated the cache with
                # mappings for generated headers, we only need to deal
                # with the source tree.
                if ($dep =~ s|^\Q$abs_srcdir_shaved\E([\.>\]])?|$srcdir_shaved$1|i) {
                    # Also check that the header actually exists
                    if (-f $line) {
                        $depconv_cache{$line} = $dep;
                    }
                }
            }
            return ($objfile, $depconv_cache{$line})
                if defined $depconv_cache{$line};
            print STDERR "DEBUG[$producer]: ignoring $objfile <- $line\n"
                if $debug;

            return undef;
        },
    'VC' =>
        sub {
            # With Microsoft Visual C the flags /Zs /showIncludes give us the
            # necessary output to be able to create dependencies that nmake
            # (or any 'make' implementation) should be able to read, with a
            # bit of help.  The output we're interested in looks something
            # like this (it always starts the same)
            #
            #   Note: including file: {whatever header file}
            #
            # This output is localized, so for example, the German pack gives
            # us this:
            #
            #   Hinweis: Einlesen der Datei:   {whatever header file}
            #
            # To accommodate, we need to use a very general regular expression
            # to parse those lines.
            #
            # Since there's no object file name at all in that information,
            # we must construct it ourselves.

            (my $objfile = shift) =~ s|\.d$|.obj|i;
            my $line = shift;

            # There are also other lines mixed in, for example compiler
            # warnings, so we simply discard anything that doesn't start with
            # the Note:

            if (/^[^:]*: [^:]*: */) {
                (my $tail = $') =~ s/\s*\R$//;

                # VC gives us absolute paths for all include files, so to
                # remove system header dependencies, we need to check that
                # they don't match $abs_srcdir or $abs_blddir.
                $tail = canonpath($tail);

                unless (defined $depconv_cache{$tail}) {
                    my $dep = $tail;
                    # Since we have already pre-populated the cache with
                    # mappings for generated headers, we only need to deal
                    # with the source tree.
                    if ($dep =~ s|^\Q$abs_srcdir\E\\|\$(SRCDIR)\\|i) {
                        # Also check that the header actually exists
                        if (-f $line) {
                            $depconv_cache{$tail} = $dep;
                        }
                    }
                }
                return ($objfile, '"'.$depconv_cache{$tail}.'"')
                    if defined $depconv_cache{$tail};
                print STDERR "DEBUG[$producer]: ignoring $objfile <- $tail\n"
                    if $debug;
            }

            return undef;
        },
    'embarcadero' =>
        sub {
            # With Embarcadero C++Builder's preprocessor (cpp32.exe) the -Sx -Hp
            # flags give us the list of #include files read, like the following:
            #
            #   Including ->->{whatever header file}
            #
            # where each "->" indicates the nesting level of the #include.  The
            # logic here is otherwise the same as the 'VC' scheme.
            #
            # Since there's no object file name at all in that information,
            # we must construct it ourselves.

            (my $objfile = shift) =~ s|\.d$|.obj|i;
            my $line = shift;

            # There are also other lines mixed in, for example compiler
            # warnings, so we simply discard anything that doesn't start with
            # the Note:

            if (/^Including (->)*/) {
                (my $tail = $') =~ s/\s*\R$//;

                # C++Builder gives us relative paths when possible, so to
                # remove system header dependencies, we convert them to
                # absolute paths and check that they don't match $abs_srcdir
                # or $abs_blddir, just as the 'VC' scheme.
                $tail = rel2abs($tail);

                unless (defined $depconv_cache{$tail}) {
                    my $dep = $tail;
                    # Since we have already pre-populated the cache with
                    # mappings for generated headers, we only need to deal
                    # with the source tree.
                    if ($dep =~ s|^\Q$abs_srcdir\E\\|\$(SRCDIR)\\|i) {
                        # Also check that the header actually exists
                        if (-f $line) {
                            $depconv_cache{$tail} = $dep;
                        }
                    }
                }
                return ($objfile, '"'.$depconv_cache{$tail}.'"')
                    if defined $depconv_cache{$tail};
                print STDERR "DEBUG[$producer]: ignoring $objfile <- $tail\n"
                    if $debug;
            }

            return undef;
        },
);
my %continuations = (
    'gcc' => "\\",
    'makedepend' => "\\",
    'VMS C' => "-",
    'VC' => "\\",
    'embarcadero' => "\\",
);

die "Producer unrecognised: $producer\n"
    unless exists $procedures{$producer} && exists $continuations{$producer};

my $procedure = $procedures{$producer};
my $continuation = $continuations{$producer};

my $buildfile_new = "$buildfile-$$";

my %collect = ();
foreach my $depfile (@depfiles) {
    open IDEP,$depfile or die "Trying to read $depfile: $!\n";
    while (<IDEP>) {
        s|\R$||;                # The better chomp
        my ($target, $deps) = $procedure->($depfile, $_);
        $collect{$target}->{$deps} = 1 if defined $target;
    }
    close IDEP;
}

open IBF, $buildfile or die "Trying to read $buildfile: $!\n";
open OBF, '>', $buildfile_new or die "Trying to write $buildfile_new: $!\n";
while (<IBF>) {
    last if /^# DO NOT DELETE THIS LINE/;
    print OBF or die "$!\n";
}
close IBF;

print OBF "# DO NOT DELETE THIS LINE -- make depend depends on it.\n";

foreach my $target (sort keys %collect) {
    my $prefix = $target . ' :';
    my @deps = sort keys %{$collect{$target}};

    while (@deps) {
        my $buf = $prefix;
        $prefix = '';

        while (@deps && ($buf eq ''
                         || length($buf) + length($deps[0]) <= 77)) {
            $buf .= ' ' . shift @deps;
        }
        $buf .= ' '.$continuation if @deps;

        print OBF $buf,"\n" or die "Trying to print: $!\n"
    }
}

close OBF;

if (compare_text($buildfile_new, $buildfile) != 0) {
    rename $buildfile_new, $buildfile
        or die "Trying to rename $buildfile_new -> $buildfile: $!\n";
}

END {
    # On VMS, we want to remove all generations of this file, in case there
    # are more than one, so we loop.
    if (defined $buildfile_new) {
        while (unlink $buildfile_new) {}
    }
}
