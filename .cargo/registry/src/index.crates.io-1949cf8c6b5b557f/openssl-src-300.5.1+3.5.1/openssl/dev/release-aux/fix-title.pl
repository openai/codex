#! /usr/bin/env perl

BEGIN { my $prev }
($_ = $prev) =~ s|^( *)(.*)$|"$1" . '=' x length($2)|e
    if m|==========|;
$prev = $_;
