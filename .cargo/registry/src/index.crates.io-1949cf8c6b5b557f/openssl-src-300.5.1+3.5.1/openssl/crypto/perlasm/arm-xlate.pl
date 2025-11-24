#! /usr/bin/env perl
# Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

use strict;

my $flavour = shift;
my $output = shift;
open STDOUT,">$output" || die "can't open $output: $!";

$flavour = "linux32" if (!$flavour or $flavour eq "void");

my %GLOBALS;
my $dotinlocallabels=($flavour=~/linux/)?1:0;

################################################################
# directives which need special treatment on different platforms
################################################################
my $arch = sub {
    if ($flavour =~ /linux/)	{ ".arch\t".join(',',@_); }
    elsif ($flavour =~ /win64/) { ".arch\t".join(',',@_); }
    else			{ ""; }
};
my $fpu = sub {
    if ($flavour =~ /linux/)	{ ".fpu\t".join(',',@_); }
    else			{ ""; }
};
my $rodata = sub {
    SWITCH: for ($flavour) {
	/linux/		&& return ".section\t.rodata";
	/ios/		&& return ".section\t__TEXT,__const";
	/win64/		&& return ".section\t.rodata";
	last;
    }
};
my $previous = sub {
    SWITCH: for ($flavour) {
	/linux/		&& return ".previous";
	/ios/		&& return ".previous";
	/win64/		&& return ".text";
	last;
    }
};
my $hidden = sub {
    if ($flavour =~ /ios/)	{ ".private_extern\t".join(',',@_); }
    elsif ($flavour =~ /win64/) { ""; }
    else			{ ".hidden\t".join(',',@_); }
};
my $comm = sub {
    my @args = split(/,\s*/,shift);
    my $name = @args[0];
    my $global = \$GLOBALS{$name};
    my $ret;

    if ($flavour =~ /ios32/)	{
	$ret = ".comm\t_$name,@args[1]\n";
	$ret .= ".non_lazy_symbol_pointer\n";
	$ret .= "$name:\n";
	$ret .= ".indirect_symbol\t_$name\n";
	$ret .= ".long\t0";
	$name = "_$name";
    } else			{ $ret = ".comm\t".join(',',@args); }

    $$global = $name;
    $ret;
};
my $globl = sub {
    my $name = shift;
    my $global = \$GLOBALS{$name};
    my $ret;

    SWITCH: for ($flavour) {
	/ios/		&& do { $name = "_$name";
				last;
			      };
    }

    $ret = ".globl	$name" if (!$ret);
    $$global = $name;
    $ret;
};
my $global = $globl;
my $extern = sub {
    &$globl(@_);
    return;	# return nothing
};
my $type = sub {
    if ($flavour =~ /linux/)	{ ".type\t".join(',',@_); }
    elsif ($flavour =~ /ios32/)	{ if (join(',',@_) =~ /(\w+),%function/) {
					"#ifdef __thumb2__\n".
					".thumb_func	$1\n".
					"#endif";
				  }
			        }
    elsif ($flavour =~ /win64/) { if (join(',',@_) =~ /(\w+),%function/) {
                # See https://sourceware.org/binutils/docs/as/Pseudo-Ops.html
                # Per https://docs.microsoft.com/en-us/windows/win32/debug/pe-format#coff-symbol-table,
                # the type for functions is 0x20, or 32.
                ".def $1\n".
                "   .type 32\n".
                ".endef";
            }
        }
    else			{ ""; }
};
my $size = sub {
    if ($flavour =~ /linux/)	{ ".size\t".join(',',@_); }
    else			{ ""; }
};
my $inst = sub {
    if ($flavour =~ /linux/)    { ".inst\t".join(',',@_); }
    else                        { ".long\t".join(',',@_); }
};
my $asciz = sub {
    my $line = join(",",@_);
    if ($line =~ /^"(.*)"$/)
    {	".byte	" . join(",",unpack("C*",$1),0) . "\n.align	2";	}
    else
    {	"";	}
};

my $adrp = sub {
    my ($args,$comment) = split(m|\s*//|,shift);
    if ($flavour =~ /ios64/) {
        "\tadrp\t$args\@PAGE";
    } elsif ($flavour =~ /linux/) {
        #
        # there seem to be two forms of 'addrp' instruction
        # to calculate offset:
	#    addrp	x3,x3,:lo12:Lrcon
        # and alternate form:
	#    addrp	x3,x3,:#lo12:Lrcon
        # the '#' is mandatory for some compilers
        # so make sure our asm always uses '#' here.
        #
        $args =~ s/(\w+)#?:lo2:(\.?\w+)/$1#:lo2:$2/;
        if ($flavour =~ /linux32/) {
            "\tadr\t$args";
        } else {
            "\tadrp\t$args";
        }
    }
} if (($flavour =~ /ios64/) || ($flavour =~ /linux/));

sub range {
  my ($r,$sfx,$start,$end) = @_;

    join(",",map("$r$_$sfx",($start..$end)));
}

sub expand_line {
  my $line = shift;
  my @ret = ();

    pos($line)=0;

    while ($line =~ m/\G[^@\/\{\"]*/g) {
	if ($line =~ m/\G(@|\/\/|$)/gc) {
	    last;
	}
	elsif ($line =~ m/\G\{/gc) {
	    my $saved_pos = pos($line);
	    $line =~ s/\G([rdqv])([0-9]+)([^\-]*)\-\1([0-9]+)\3/range($1,$3,$2,$4)/e;
	    pos($line) = $saved_pos;
	    $line =~ m/\G[^\}]*\}/g;
	}
	elsif ($line =~ m/\G\"/gc) {
	    $line =~ m/\G[^\"]*\"/g;
	}
    }

    $line =~ s/\b(\w+)/$GLOBALS{$1} or $1/ge;

    if ($flavour =~ /ios64/) {
	$line =~ s/#?:lo12:(\w+)/$1\@PAGEOFF/;
    } elsif($flavour =~ /linux/) {
        #
        # make '#' mandatory for :lo12: (similar to adrp above)
        #
	$line =~ s/#?:lo12:(\.?\w+)/\#:lo12:$1/;
    }

    return $line;
}

while(my $line=<>) {

    if ($line =~ m/^\s*(#|@|\/\/)/)	{ print $line; next; }

    $line =~ s|/\*.*\*/||;	# get rid of C-style comments...
    $line =~ s|^\s+||;		# ... and skip whitespace in beginning...
    $line =~ s|\s+$||;		# ... and at the end

    {
	$line =~ s|[\b\.]L(\w{2,})|L$1|g;	# common denominator for Locallabel
	$line =~ s|\bL(\w{2,})|\.L$1|g	if ($dotinlocallabels);
    }

    {
	if ($line =~ s|(^[\.\w]+)\:\s*||) {
	    my $label = $1;
	    printf "%s:",($GLOBALS{$label} or $label);
	}
    }

    if ($line !~ m/^[#@]/) {
	$line =~ s|^\s*(\.?)(\S+)\s*||;
	my $c = $1; $c = "\t" if ($c eq "");
	my $mnemonic = $2;
	my $opcode;
	if ($mnemonic =~ m/([^\.]+)\.([^\.]+)/) {
	    $opcode = eval("\$$1_$2");
	} else {
	    $opcode = eval("\$$mnemonic");
	}

	my $arg=expand_line($line);

	if (ref($opcode) eq 'CODE') {
		$line = &$opcode($arg);
	} elsif ($mnemonic)         {
		$line = $c.$mnemonic;
		$line.= "\t$arg" if ($arg ne "");
	}
    }

    # ldr REG, #VALUE psuedo-instruction - avoid clang issue with Neon registers
    #
    if ($line =~ /^\s*ldr\s+([qd]\d\d?)\s*,\s*=(\w+)/i) {
        # Immediate load via literal pool into qN or DN - clang max is 2^32-1
        my ($reg, $value) = ($1, $2);
        # If $value is hex, 0x + 8 hex chars = 10 chars total will be okay
        # If $value is decimal, 2^32 - 1 = 4294967295 will be okay (also 10 chars)
        die("$line: immediate load via literal pool into $reg: value too large for clang - redo manually") if length($value) > 10;
    }

    print $line if ($line);
    print "\n";
}

close STDOUT or die "error closing STDOUT: $!";
