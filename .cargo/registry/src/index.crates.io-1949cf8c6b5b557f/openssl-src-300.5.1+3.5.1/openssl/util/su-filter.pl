#! /usr/bin/env perl
# Copyright 2015-2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

my $in_su = 0;
my $indent = 0;
my $out;
my $braces = 0;
my $arrcnt;
my $data;
my $tststr;
my $incomm = 0;

while(<>) {
    $tststr = $_;
    $incomm++ while $tststr =~ /\/\*/g;
    $incomm-- while $tststr =~ /\*\//g;

    if($in_su == 1) {
        if(/}(.*);/) {
            $out .= $_;
            do_output($out);
            $in_su = 0;
        } elsif(/^ *\} [^\s]+(\[\d*\])* = \{/) {
           $tststr = $1;
           $arrcnt = 0;
           $arrcnt++ while $tststr =~ /\[/g;
           $in_su++;
           $braces = 1;
           /^(.* = \{)(.*)$/;
           $data = $2;
           $out .= $1."\n";
        } else {
            $out .= $_;
        }
    } elsif($in_su == 2) {
        $data .= $_;
        if(/};$/) {
            #$data = "\n$data";
            $data =~ s/\n */\n/g;
            $data =~ s/};\n?//s;
            my @strucdata = structureData($data);
            $out .= displayData($indent, 0, \@strucdata);
            $out .= "\n$indent};\n";
            do_output($out);
            $in_su = 0;
        }
    } elsif($incomm <= 0 && /( *)(static )?(const )?(union|struct) ([a-zA-Z_\$][\$0-9a-zA-Z_]+ )?\{/) {
        $in_su = 1;
        $indent = $1;
        $out = $_;
        next;
    } else {
        do_output($_);
    }
}


sub structureData {
    my $data = $_[0];
    my @datalist = split(/(\{|\}|,|"|#|\n|\/\*|\*\/|\(|\))/, $data);
    my $item;
    my $dataitem = "";
    my @struclist = ();
    my $substruc;
    my $inquote = 0;
    my $inbrace = 0;
    my $preproc = 0;
    my $comment = 0;
    my $inparen = 0;


    foreach $item (@datalist) {
        if($comment) {
            if($item eq "*/") {
                $comment = 0;
                $dataitem .= "*/";
                push @struclist, $dataitem;
                $dataitem = "";
                next;
            }
            $dataitem .= $item;
            next;
        }
        if($inquote) {
            $dataitem .= $item;
            if($item eq "\"") {
                $inquote--;
            }
            next;
        }
        if($preproc) {
            if($item eq "\n") {
                $preproc = 0;
                push @struclist, $dataitem;
                $dataitem = "";
                next;
            }
            $dataitem .= $item;
            next;
        }
        if($inbrace) {
            if($item eq "}") {
                $inbrace --;

                if(!$inbrace) {
                    $substruc = structureData($dataitem);
                    $dataitem = $substruc;
                    next;
                }
            } elsif($item eq "{") {
                $inbrace++;
            } elsif ($item eq "\"") {
                $inquote++;
            }
            $dataitem .= $item;
            next;
        }
        if($inparen) {
            if($item eq ")") {
                $inparen--;
            }
            $dataitem .= $item;
            next;
        }
        if($item eq "\n") {
            next;
        }
        if($item eq "#") {
            $preproc = 1;
            push @struclist, $dataitem;
            $dataitem = "#";
            next;
        }
        if($item eq "/*") {
            $comment = 1;
            push @struclist, $dataitem;
            $dataitem= "/*";
            next;
        }
        if($item eq "\"") {
            $dataitem .= $item;
            $inquote++;
            next;
        }
        if($item eq "{") {
            $inbrace++;
            next;
        }
        if($item eq ",") {
            push @struclist, $dataitem;
            $dataitem = "";
            next;
        }
        if($item eq "(") {
            $dataitem .= $item;
            $inparen++;
            next;
        }
        if($item =~ /^\s*$/) {
            next;
        }
        if(ref $dataitem eq 'ARRAY') {
            push @struclist, $dataitem;
            $dataitem = "";
        }
        $dataitem .= $item;
    }
    push @struclist, $dataitem;
    return \@struclist;
}

sub displayData {
    my $indent = shift;
    my $depth = shift;
    my $data = shift;
    my $item;
    my $out = "";
    my $currline = "";
    my $first = 1;
    my $prevpreproc = 0;
    my $prevcomment = 0;

    foreach $item (@{$data}) {
        if($item =~ /^\/\*/) {
            #Comment
            $item =~ s/\n/\n$indent/g;
            if($out =~ /\n\s*$/s) {
                $out .= $item."\n".$indent;
            } else {
                $out .= "\n".$indent.$item."\n".$indent;
            }
            $currline = $indent;
            $prevcomment = 1;
            next;
        }
        $item =~ s/^\s+//;
        if($item =~ /^#/) {
            #Pre-processor directive
            if($out =~ /\n\s*$/s) {
                $out =~ s/\n\s*$/\n/;
                $out .= $item."\n".$indent;
            } else {
                $out .= "\n".$item."\n".$indent;
            }
            $currline = $indent;
            $prevpreproc = 1;
            next;
        }
        if($first) {
            $first = 0;
            if($depth != 0) {
                $out .= $indent;
                $currline = $indent;
            }
        } else {
            if(!$prevpreproc && !$prevcomment) {
                $out .= ", ";
                $currline .= ", ";
                if($depth == 1) {
                    $out .= "\n";
                    $currline = "";
                }
                if($depth == 1) {
                    $out .= $indent;
                    $currline .= $indent;
                }
            }

        }
        $prevpreproc = 0;
        $prevcomment = 0;

        if (ref $item eq 'ARRAY') {
            if($depth == 0) {
                $out .= displayData("$indent    ", $depth+1, $item);
            } else {
                $out .= "{\n".displayData("$indent    ", $depth+1, $item)."\n".$indent."}";
                $currline = $indent."}";
            }
        } else {
            if(length $currline.$item > 79) {
                $currline = $indent;
                $out .= "\n$indent";
            }
            $out .= $item;
            $currline .= $item;
        }
    }
    return $out;
}

sub do_output {
    my $out = shift;
    # Strip any trailing whitespace
    $out =~ s/\s+\n/\n/g;
    print $out;
}
