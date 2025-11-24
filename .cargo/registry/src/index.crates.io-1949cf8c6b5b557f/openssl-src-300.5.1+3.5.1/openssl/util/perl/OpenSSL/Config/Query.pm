# Copyright 2021 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

package OpenSSL::Config::Query;

use 5.10.0;
use strict;
use warnings;
use Carp;

=head1 NAME

OpenSSL::Config::Query - Query OpenSSL configuration info

=head1 SYNOPSIS

    use OpenSSL::Config::Info;

    my $query = OpenSSL::Config::Query->new(info => \%unified_info);

    # Query for something that's expected to give a scalar back
    my $variable = $query->method(... args ...);

    # Query for something that's expected to give a list back
    my @variable = $query->method(... args ...);

=head1 DESCRIPTION

The unified info structure, commonly known as the %unified_info table, has
become quite complex, and a bit overwhelming to look through directly.  This
module makes querying this structure simpler, through diverse methods.

=head2 Constructor

=over 4

=item B<new> I<%options>

Creates an instance of the B<OpenSSL::Config::Query> class.  It takes options
in keyed pair form, i.e. a series of C<< key => value >> pairs.  Available
options are:

=over 4

=item B<info> =E<gt> I<HASHREF>

A reference to a unified information hash table, most commonly known as
%unified_info.

=item B<config> =E<gt> I<HASHREF>

A reference to a config information hash table, most commonly known as
%config.

=back

Example:

    my $info = OpenSSL::Config::Info->new(info => \%unified_info);

=back

=cut

sub new {
    my $class = shift;
    my %opts = @_;

    my @messages = _check_accepted_options(\%opts,
                                           info => 'HASH',
                                           config => 'HASH');
    croak $messages[0] if @messages;

    # We make a shallow copy of the input structure.  We might make
    # a different choice in the future...
    my $instance = { info => $opts{info} // {},
                     config => $opts{config} // {} };
    bless $instance, $class;

    return $instance;
}

=head2 Query methods

=over 4

=item B<get_sources> I<LIST>

LIST is expected to be the collection of names of end products, such as
programs, modules, libraries.

The returned result is a hash table reference, with each key being one of
these end product names, and its value being a reference to an array of
source file names that constitutes everything that will or may become part
of that end product.

=cut

sub get_sources {
    my $self = shift;

    my $result = {};
    foreach (@_) {
        my @sources = @{$self->{info}->{sources}->{$_} // []};
        my @staticlibs =
            grep { $_ =~ m|\.a$| } @{$self->{info}->{depends}->{$_} // []};

        my %parts = ( %{$self->get_sources(@sources)},
                      %{$self->get_sources(@staticlibs)} );
        my @parts = map { @{$_} } values %parts;

        my @generator =
            ( ( $self->{info}->{generate}->{$_} // [] ) -> [0] // () );
        my %generator_parts = %{$self->get_sources(@generator)};
        # if there are any generator parts, we ignore it, because that means
        # it's a compiled program and thus NOT part of the source that's
        # queried.
        @generator = () if %generator_parts;

        my @partial_result =
            ( ( map { @{$_} } values %parts ),
              ( grep { !defined($parts{$_}) } @sources, @generator ) );

        # Push conditionally, to avoid creating $result->{$_} with an empty
        # value
        push @{$result->{$_}}, @partial_result if @partial_result;
    }

    return $result;
}

=item B<get_config> I<LIST>

LIST is expected to be the collection of names of configuration data, such
as build_infos, sourcedir, ...

The returned result is a hash table reference, with each key being one of
these configuration data names, and its value being a reference to the value
corresponding to that name.

=cut

sub get_config {
    my $self = shift;

    return { map { $_ => $self->{config}->{$_} } @_ };
}

########
#
#  Helper functions
#

sub _check_accepted_options {
    my $opts = shift;           # HASH reference (hopefully)
    my %conds = @_;             # key => type

    my @messages;
    my %optnames = map { $_ => 1 } keys %$opts;
    foreach (keys %conds) {
        delete $optnames{$_};
    }
    push @messages, "Unknown options: " . join(', ', sort keys %optnames)
        if keys %optnames;
    foreach (sort keys %conds) {
        push @messages, "'$_' value not a $conds{$_} reference"
            if (defined $conds{$_} && defined $opts->{$_}
                && ref $opts->{$_} ne $conds{$_});
    }
    return @messages;
}

1;
