#! /usr/bin/env perl

use Config;

# Check that the perl implementation file modules generate paths that
# we expect for the platform
use File::Spec::Functions qw(:DEFAULT rel2abs);

if (!$ENV{CONFIGURE_INSIST} && rel2abs('.') !~ m|\\|) {
    die <<EOF;

******************************************************************************
This perl implementation doesn't produce Windows like paths (with backward
slash directory separators).  Please use an implementation that matches your
building platform.

This Perl version: $Config{version} for $Config{archname}
******************************************************************************
EOF
}

1;
