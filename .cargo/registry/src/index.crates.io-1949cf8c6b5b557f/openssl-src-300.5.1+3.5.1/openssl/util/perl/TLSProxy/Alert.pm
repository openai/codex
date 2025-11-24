# Copyright 2018 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

package TLSProxy::Alert;

sub new
{
    my $class = shift;
    my ($server,
        $encrypted,
        $level,
        $description) = @_;

    my $self = {
        server => $server,
        encrypted => $encrypted,
        level => $level,
        description => $description
    };

    return bless $self, $class;
}

#Read only accessors
sub server
{
    my $self = shift;
    return $self->{server};
}
sub encrypted
{
    my $self = shift;
    return $self->{encrypted};
}
sub level
{
    my $self = shift;
    return $self->{level};
}
sub description
{
    my $self = shift;
    return $self->{description};
}
1;
