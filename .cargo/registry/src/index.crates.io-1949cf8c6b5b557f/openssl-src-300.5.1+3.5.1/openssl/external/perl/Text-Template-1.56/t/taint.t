#!perl -T
# Tests for taint-mode features

use strict;
use warnings;
use lib 'blib/lib';
use Test::More tests => 21;
use File::Temp;

use_ok 'Text::Template' or exit 1;

if ($^O eq 'MSWin32') {
    # File::Temp (for all versions up to at least 0.2308) is currently bugged under MSWin32/taint mode [as of 2018-09]
    # ... fails unless "/tmp" on the current windows drive is a writable directory OR either $ENV{TMP} or $ENV{TEMP} are untainted and point to a writable directory
    # ref: [File-Temp: Fails under -T, Windows 7, Strawberry Perl 5.12.1](https://rt.cpan.org/Public/Bug/Display.html?id=60340)
    ($ENV{TEMP}) = $ENV{TEMP} =~ m/^.*$/gmsx; # untaint $ENV{TEMP}
    ($ENV{TMP})  = $ENV{TMP}  =~ m/^.*$/gmsx; # untaint $ENV{TMP}
}

my $tmpfile = File::Temp->new;
my $file    = $tmpfile->filename;

# makes its arguments tainted
sub taint {
    for (@_) {
        $_ .= substr($0, 0, 0);    # LOD
    }
}

my $template = 'The value of $n is {$n}.';

open my $fh, '>', $file or die "Couldn't write temporary file $file: $!";
print $fh $template, "\n";
close $fh or die "Couldn't finish temporary file $file: $!";

sub should_fail {
    my $obj = Text::Template->new(@_);
    eval { $obj->fill_in() };
    if ($@) {
        pass $@;
    }
    else {
        fail q[didn't fail];
    }
}

sub should_work {
    my $obj = Text::Template->new(@_);
    eval { $obj->fill_in() };
    if ($@) {
        fail $@;
    }
    else {
        pass;
    }
}

sub should_be_tainted {
    ok !Text::Template::_is_clean($_[0]);
}

sub should_be_clean {
    ok Text::Template::_is_clean($_[0]);
}

# Tainted filename should die with and without UNTAINT option
# untainted filename should die without UNTAINT option
# filehandle should die without UNTAINT option
# string and array with tainted data should die either way

# (2)-(7)
my $tfile = $file;
taint($tfile);
should_be_tainted($tfile);
should_be_clean($file);
should_fail TYPE => 'file', SOURCE => $tfile;
should_fail TYPE => 'file', SOURCE => $tfile, UNTAINT => 1;
should_fail TYPE => 'file', SOURCE => $file;
should_work TYPE => 'file', SOURCE => $file, UNTAINT => 1;

# (8-9)
open $fh, '<', $file or die "Couldn't open $file for reading: $!; aborting";
should_fail TYPE => 'filehandle', SOURCE => $fh;
close $fh;

open $fh, '<', $file or die "Couldn't open $file for reading: $!; aborting";
should_work TYPE => 'filehandle', SOURCE => $fh, UNTAINT => 1;
close $fh;

# (10-15)
my $ttemplate = $template;
taint($ttemplate);
should_be_tainted($ttemplate);
should_be_clean($template);
should_fail TYPE => 'string', SOURCE => $ttemplate;
should_fail TYPE => 'string', SOURCE => $ttemplate, UNTAINT => 1;
should_work TYPE => 'string', SOURCE => $template;
should_work TYPE => 'string', SOURCE => $template, UNTAINT => 1;

# (16-19)
my $array  = [$template];
my $tarray = [$ttemplate];
should_fail TYPE => 'array', SOURCE => $tarray;
should_fail TYPE => 'array', SOURCE => $tarray, UNTAINT => 1;
should_work TYPE => 'array', SOURCE => $array;
should_work TYPE => 'array', SOURCE => $array, UNTAINT => 1;

# (20-21) Test _unconditionally_untaint utility function
Text::Template::_unconditionally_untaint($ttemplate);
should_be_clean($ttemplate);
Text::Template::_unconditionally_untaint($tfile);
should_be_clean($tfile);
