# Copyright 2019-2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

=head1 NAME

OpenSSL::fallback - push directories to the end of @INC at compile time

=cut

package OpenSSL::fallback;

use strict;
use warnings;
use Carp;

our $VERSION = '0.01';

=head1 SYNOPSIS

    use OpenSSL::fallback LIST;

=head1 DESCRIPTION

This small simple module simplifies the addition of fallback directories
in @INC at compile time.

It is used to add extra directories at the end of perl's search path so
that later "use" or "require" statements will find modules which are not
located on perl's default search path.

This is similar to L<lib>, except the paths are I<appended> to @INC rather
than prepended, thus allowing the use of a newer module on perl's default
search path if there is one.

=head1 CAVEAT

Just like with B<lib>, this only works with Unix filepaths.
Just like with L<lib>, this doesn't mean that it only works on Unix, but that
non-Unix users must first translate their file paths to Unix conventions.

    # VMS users wanting to put [.my.stuff] into their @INC should write:
    use fallback 'my/stuff';

=head1 NOTES

If you try to add a file to @INC as follows, you will be warned, and the file
will be ignored:

    use fallback 'file.txt';

The sole exception is the file F<MODULES.txt>, which must contain a list of
sub-directories relative to the location of that F<MODULES.txt> file.
All these sub-directories will be appended to @INC.

=cut

# Forward declare
sub glob;

use constant DEBUG => 0;

sub import {
    shift;                      # Skip module name

    foreach (@_) {
        my $path = $_;

        if ($path eq '') {
            carp "Empty compile time value given to use fallback";
            next;
        }

        print STDERR "DEBUG: $path\n" if DEBUG;

        unless (-e $path
                && ($path =~ m/(?:^|\/)MODULES.txt/ || -d $path)) {
            croak "Parameter to use fallback must be a directory, not a file";
            next;
        }

        my @dirs = ();
        if (-f $path) {         # It's a MODULES.txt file
            (my $dir = $path) =~ s|/[^/]*$||; # quick dirname
            open my $fh, $path or die "Could not open $path: $!\n";
            while (my $l = <$fh>) {
                $l =~ s|\R$||;        # Better chomp
                my $d = "$dir/$l";
                my $checked = $d;

                if ($^O eq 'VMS') {
                    # Some VMS unpackers replace periods with underscores
                    # We must be real careful not to convert the directories
                    # '.' and '..', though.
                    $checked =
                        join('/',
                             map { my $x = $_;
                                   $x =~ s|\.|_|g
                                       if ($x ne '..' && $x ne '.');
                                   $x }
                             split(m|/|, $checked))
                        unless -e $checked && -d $checked;
                }
                croak "All lines in $path must be a directory, not a file: $l"
                    unless -e $checked && -d $checked;
                push @INC, $checked;
            }
        } else {                # It's a directory
            push @INC, $path;
        }
    }
}

=head1 SEE ALSO

L<FindBin> - optional module which deals with paths relative to the source
file.

=head1 AUTHOR

Richard Levitte, 2019

=cut

