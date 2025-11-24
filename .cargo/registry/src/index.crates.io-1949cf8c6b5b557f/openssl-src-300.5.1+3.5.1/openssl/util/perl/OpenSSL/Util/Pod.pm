# Copyright 2016-2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::Util::Pod;

use strict;
use warnings;

use Exporter;
use vars qw($VERSION @ISA @EXPORT @EXPORT_OK %EXPORT_TAGS);
$VERSION = "0.1";
@ISA = qw(Exporter);
@EXPORT = qw(extract_pod_info);
@EXPORT_OK = qw();

=head1 NAME

OpenSSL::Util::Pod - utilities to manipulate .pod files

=head1 SYNOPSIS

  use OpenSSL::Util::Pod;

  my %podinfo = extract_pod_info("foo.pod");

  # or if the file is already opened...  Note that this consumes the
  # remainder of the file.

  my %podinfo = extract_pod_info(\*STDIN);

=head1 DESCRIPTION

=over

=item B<extract_pod_info "FILENAME", HASHREF>

=item B<extract_pod_info "FILENAME">

=item B<extract_pod_info GLOB, HASHREF>

=item B<extract_pod_info GLOB>

Extracts information from a .pod file, given a STRING (file name) or a
GLOB (a file handle).  The result is given back as a hash table.

The additional hash is for extra parameters:

=over

=item B<section =E<gt> N>

The value MUST be a number, and will be the man section number
to be used with the given .pod file.

=item B<debug =E<gt> 0|1>

If set to 1, extra debug text will be printed on STDERR

=back

=back

=head1 RETURN VALUES

=over

=item B<extract_pod_info> returns a hash table with the following
items:

=over

=item B<section =E<gt> N>

The man section number this .pod file belongs to.  Often the same as
was given as input.

=item B<names =E<gt> [ "name", ... ]>

All the names extracted from the NAME section.

=item B<contents =E<gt> "...">

The whole contents of the .pod file.

=back

=back

=cut

sub extract_pod_info {
    my $input = shift;
    my $defaults_ref = shift || {};
    my %defaults = ( debug => 0, section => 0, %$defaults_ref );
    my $fh = undef;
    my $filename = undef;
    my $contents;

    # If not a file handle, then it's assume to be a file path (a string)
    if (ref $input eq "") {
        $filename = $input;
        open $fh, $input or die "Trying to read $filename: $!\n";
        print STDERR "DEBUG: Reading $input\n" if $defaults{debug};
        $input = $fh;
    }
    if (ref $input eq "GLOB") {
        local $/ = undef;
        $contents = <$input>;
    } else {
        die "Unknown input type";
    }

    my @invisible_names = ();
    my %podinfo = ( section => $defaults{section});
    $podinfo{lastsecttext} = ""; # init needed in case input file is empty

    # Regexp to split a text into paragraphs found at
    # https://www.perlmonks.org/?node_id=584367
    # Most of all, \G (continue at last match end) and /g (anchor
    # this match for \G) are significant
    foreach (map { /\G((?:(?!\n\n).)*\n+|.+\z)/sg } $contents) {
        # Remove as many line endings as possible from the end of the paragraph
        while (s|\R$||) {}

        print STDERR "DEBUG: Paragraph:\n$_\n"
            if $defaults{debug};

        # Stop reading when we have reached past the NAME section.
        last if (m|^=head1|
                 && defined $podinfo{lastsect}
                 && $podinfo{lastsect} eq "NAME");

        # Collect the section name
        if (m|^=head1\s*(.*)|) {
            $podinfo{lastsect} = $1;
            $podinfo{lastsect} =~ s/\s+$//;
            print STDERR "DEBUG: Found new pod section $1\n"
                if $defaults{debug};
            print STDERR "DEBUG: Clearing pod section text\n"
                if $defaults{debug};
            $podinfo{lastsecttext} = "";
        }

        # Add invisible names
        if (m|^=for\s+openssl\s+names:\s*(.*)|s) {
            my $x = $1;
            my @tmp = map { map { s/\s+//g; $_ } split(/,/, $_) } $x;
            print STDERR
                "DEBUG: Found invisible names: ", join(', ', @tmp), "\n"
                if $defaults{debug};
            push @invisible_names, @tmp;
        }

        next if (m|^=| || m|^\s*$|);

        # Collect the section text
        print STDERR "DEBUG: accumulating pod section text \"$_\"\n"
            if $defaults{debug};
        $podinfo{lastsecttext} .= " " if $podinfo{lastsecttext};
        $podinfo{lastsecttext} .= $_;
    }


    if (defined $fh) {
        close $fh;
        print STDERR "DEBUG: Done reading $filename\n" if $defaults{debug};
    }

    $podinfo{lastsecttext} =~ s|\s+-\s+.*$||s;

    my @names =
        map { s/^\s+//g;        # Trim prefix blanks
              s/\s+$//g;        # Trim suffix blanks
              s|/|-|g;          # Treat slash as dash
              $_ }
        split(m|,|, $podinfo{lastsecttext});

    print STDERR
        "DEBUG: Collected names are: ",
        join(', ', @names, @invisible_names), "\n"
        if $defaults{debug};

    return ( section => $podinfo{section},
             names => [ @names, @invisible_names ],
             contents => $contents,
             filename => $filename );
}

1;
