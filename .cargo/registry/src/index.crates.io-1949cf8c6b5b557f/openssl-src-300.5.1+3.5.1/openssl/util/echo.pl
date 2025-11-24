#! /usr/bin/env perl

use strict;
use warnings;
use Getopt::Std;

our $opt_n = 0;

getopts('n') or die "Invalid option: $!\n";

print join(' ', @ARGV);
print "\n" unless $opt_n;
