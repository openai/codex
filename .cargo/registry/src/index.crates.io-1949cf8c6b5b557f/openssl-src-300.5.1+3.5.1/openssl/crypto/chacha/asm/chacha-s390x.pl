#! /usr/bin/env perl
# Copyright 2016-2019 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

#
# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project. The module is, however, dual licensed under OpenSSL and
# CRYPTOGAMS licenses depending on where you obtain it. For further
# details see http://www.openssl.org/~appro/cryptogams/.
# ====================================================================
#
# December 2015
#
# ChaCha20 for s390x.
#
# 3 times faster than compiler-generated code.

#
# August 2018
#
# Add vx code path: 4x"vertical".
#
# Copyright IBM Corp. 2018
# Author: Patrick Steuer <patrick.steuer@de.ibm.com>

#
# February 2019
#
# Add 6x"horizontal" VX implementation. It's ~25% faster than IBM's
# 4x"vertical" submission [on z13] and >3 faster than scalar code.
# But to harness overheads revert to transliteration of VSX code path
# from chacha-ppc module, which is also 4x"vertical", to handle inputs
# not longer than 256 bytes.

use strict;
use FindBin qw($Bin);
use lib "$Bin/../..";
use perlasm::s390x qw(:DEFAULT :VX :EI AUTOLOAD LABEL INCLUDE);

# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
my $output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
my $flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

my ($z,$SIZE_T);
if ($flavour =~ /3[12]/) {
	$z=0;	# S/390 ABI
	$SIZE_T=4;
} else {
	$z=1;	# zSeries ABI
	$SIZE_T=8;
}

my $sp="%r15";
my $stdframe=16*$SIZE_T+4*8;

sub ROUND {
my @x=map("%r$_",(0..7,"x","x","x","x",(10..13)));
my @t=map("%r$_",(8,9));
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my ($xc,$xc_)=map("$_",@t);

	# Consider order in which variables are addressed by their
	# index:
	#
	#	a   b   c   d
	#
	#	0   4   8  12 < even round
	#	1   5   9  13
	#	2   6  10  14
	#	3   7  11  15
	#	0   5  10  15 < odd round
	#	1   6  11  12
	#	2   7   8  13
	#	3   4   9  14
	#
	# 'a', 'b' and 'd's are permanently allocated in registers,
	# @x[0..7,12..15], while 'c's are maintained in memory. If
	# you observe 'c' column, you'll notice that pair of 'c's is
	# invariant between rounds. This means that we have to reload
	# them once per round, in the middle. This is why you'll see
	# 'c' stores and loads in the middle, but none in the beginning
	# or end.

	alr	(@x[$a0],@x[$b0]);	# Q1
	 alr	(@x[$a1],@x[$b1]);	# Q2
	xr	(@x[$d0],@x[$a0]);
	 xr	(@x[$d1],@x[$a1]);
	rll	(@x[$d0],@x[$d0],16);
	 rll	(@x[$d1],@x[$d1],16);

	alr	($xc,@x[$d0]);
	 alr	($xc_,@x[$d1]);
	xr	(@x[$b0],$xc);
	 xr	(@x[$b1],$xc_);
	rll	(@x[$b0],@x[$b0],12);
	 rll	(@x[$b1],@x[$b1],12);

	alr	(@x[$a0],@x[$b0]);
	 alr	(@x[$a1],@x[$b1]);
	xr	(@x[$d0],@x[$a0]);
	 xr	(@x[$d1],@x[$a1]);
	rll	(@x[$d0],@x[$d0],8);
	 rll	(@x[$d1],@x[$d1],8);

	alr	($xc,@x[$d0]);
	 alr	($xc_,@x[$d1]);
	xr	(@x[$b0],$xc);
	 xr	(@x[$b1],$xc_);
	rll	(@x[$b0],@x[$b0],7);
	 rll	(@x[$b1],@x[$b1],7);

	stm	($xc,$xc_,"$stdframe+4*8+4*$c0($sp)");	# reload pair of 'c's
	lm	($xc,$xc_,"$stdframe+4*8+4*$c2($sp)");

	alr	(@x[$a2],@x[$b2]);	# Q3
	 alr	(@x[$a3],@x[$b3]);	# Q4
	xr	(@x[$d2],@x[$a2]);
	 xr	(@x[$d3],@x[$a3]);
	rll	(@x[$d2],@x[$d2],16);
	 rll	(@x[$d3],@x[$d3],16);

	alr	($xc,@x[$d2]);
	 alr	($xc_,@x[$d3]);
	xr	(@x[$b2],$xc);
	 xr	(@x[$b3],$xc_);
	rll	(@x[$b2],@x[$b2],12);
	 rll	(@x[$b3],@x[$b3],12);

	alr	(@x[$a2],@x[$b2]);
	 alr	(@x[$a3],@x[$b3]);
	xr	(@x[$d2],@x[$a2]);
	 xr	(@x[$d3],@x[$a3]);
	rll	(@x[$d2],@x[$d2],8);
	 rll	(@x[$d3],@x[$d3],8);

	alr	($xc,@x[$d2]);
	 alr	($xc_,@x[$d3]);
	xr	(@x[$b2],$xc);
	 xr	(@x[$b3],$xc_);
	rll	(@x[$b2],@x[$b2],7);
	 rll	(@x[$b3],@x[$b3],7);
}

sub VX_lane_ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my @x=map("%v$_",(0..15));

	vaf	(@x[$a0],@x[$a0],@x[$b0]);	# Q1
	vx	(@x[$d0],@x[$d0],@x[$a0]);
	verllf	(@x[$d0],@x[$d0],16);
	vaf	(@x[$a1],@x[$a1],@x[$b1]);	# Q2
	vx	(@x[$d1],@x[$d1],@x[$a1]);
	verllf	(@x[$d1],@x[$d1],16);
	vaf	(@x[$a2],@x[$a2],@x[$b2]);	# Q3
	vx	(@x[$d2],@x[$d2],@x[$a2]);
	verllf	(@x[$d2],@x[$d2],16);
	vaf	(@x[$a3],@x[$a3],@x[$b3]);	# Q4
	vx	(@x[$d3],@x[$d3],@x[$a3]);
	verllf	(@x[$d3],@x[$d3],16);

	vaf	(@x[$c0],@x[$c0],@x[$d0]);
	vx	(@x[$b0],@x[$b0],@x[$c0]);
	verllf	(@x[$b0],@x[$b0],12);
	vaf	(@x[$c1],@x[$c1],@x[$d1]);
	vx	(@x[$b1],@x[$b1],@x[$c1]);
	verllf	(@x[$b1],@x[$b1],12);
	vaf	(@x[$c2],@x[$c2],@x[$d2]);
	vx	(@x[$b2],@x[$b2],@x[$c2]);
	verllf	(@x[$b2],@x[$b2],12);
	vaf	(@x[$c3],@x[$c3],@x[$d3]);
	vx	(@x[$b3],@x[$b3],@x[$c3]);
	verllf	(@x[$b3],@x[$b3],12);

	vaf	(@x[$a0],@x[$a0],@x[$b0]);
	vx	(@x[$d0],@x[$d0],@x[$a0]);
	verllf	(@x[$d0],@x[$d0],8);
	vaf	(@x[$a1],@x[$a1],@x[$b1]);
	vx	(@x[$d1],@x[$d1],@x[$a1]);
	verllf	(@x[$d1],@x[$d1],8);
	vaf	(@x[$a2],@x[$a2],@x[$b2]);
	vx	(@x[$d2],@x[$d2],@x[$a2]);
	verllf	(@x[$d2],@x[$d2],8);
	vaf	(@x[$a3],@x[$a3],@x[$b3]);
	vx	(@x[$d3],@x[$d3],@x[$a3]);
	verllf	(@x[$d3],@x[$d3],8);

	vaf	(@x[$c0],@x[$c0],@x[$d0]);
	vx	(@x[$b0],@x[$b0],@x[$c0]);
	verllf	(@x[$b0],@x[$b0],7);
	vaf	(@x[$c1],@x[$c1],@x[$d1]);
	vx	(@x[$b1],@x[$b1],@x[$c1]);
	verllf	(@x[$b1],@x[$b1],7);
	vaf	(@x[$c2],@x[$c2],@x[$d2]);
	vx	(@x[$b2],@x[$b2],@x[$c2]);
	verllf	(@x[$b2],@x[$b2],7);
	vaf	(@x[$c3],@x[$c3],@x[$d3]);
	vx	(@x[$b3],@x[$b3],@x[$c3]);
	verllf	(@x[$b3],@x[$b3],7);
}

sub VX_ROUND {
my @a=@_[0..5];
my @b=@_[6..11];
my @c=@_[12..17];
my @d=@_[18..23];
my $odd=@_[24];

	vaf		(@a[$_],@a[$_],@b[$_]) for (0..5);
	vx		(@d[$_],@d[$_],@a[$_]) for (0..5);
	verllf		(@d[$_],@d[$_],16) for (0..5);

	vaf		(@c[$_],@c[$_],@d[$_]) for (0..5);
	vx		(@b[$_],@b[$_],@c[$_]) for (0..5);
	verllf		(@b[$_],@b[$_],12) for (0..5);

	vaf		(@a[$_],@a[$_],@b[$_]) for (0..5);
	vx		(@d[$_],@d[$_],@a[$_]) for (0..5);
	verllf		(@d[$_],@d[$_],8) for (0..5);

	vaf		(@c[$_],@c[$_],@d[$_]) for (0..5);
	vx		(@b[$_],@b[$_],@c[$_]) for (0..5);
	verllf		(@b[$_],@b[$_],7) for (0..5);

	vsldb		(@c[$_],@c[$_],@c[$_],8) for (0..5);
	vsldb		(@b[$_],@b[$_],@b[$_],$odd?12:4) for (0..5);
	vsldb		(@d[$_],@d[$_],@d[$_],$odd?4:12) for (0..5);
}

PERLASM_BEGIN($output);

INCLUDE	("s390x_arch.h");
TEXT	();

################
# void ChaCha20_ctr32(unsigned char *out, const unsigned char *inp, size_t len,
#                     const unsigned int key[8], const unsigned int counter[4])
my ($out,$inp,$len,$key,$counter)=map("%r$_",(2..6));
{
my $frame=$stdframe+4*20;
my @x=map("%r$_",(0..7,"x","x","x","x",(10..13)));
my @t=map("%r$_",(8,9));

GLOBL	("ChaCha20_ctr32");
TYPE	("ChaCha20_ctr32","\@function");
ALIGN	(32);
LABEL	("ChaCha20_ctr32");
	larl	("%r1","OPENSSL_s390xcap_P");

	lghi	("%r0",64);
&{$z?	\&ltgr:\&ltr}	($len,$len);		# len==0?
	bzr	("%r14");
	lg	("%r1","S390X_STFLE+16(%r1)");
&{$z?	\&clgr:\&clr}	($len,"%r0");
	jle	(".Lshort");

	tmhh	("%r1",0x4000);			# check for vx bit
	jnz	(".LChaCha20_ctr32_vx");

LABEL	(".Lshort");
&{$z?	\&aghi:\&ahi}	($len,-64);
&{$z?	\&lghi:\&lhi}	("%r1",-$frame);
&{$z?	\&stmg:\&stm}	("%r6","%r15","6*$SIZE_T($sp)");
&{$z?	\&slgr:\&slr}	($out,$inp);	# difference
	la	($len,"0($inp,$len)");	# end of input minus 64
	larl	("%r7",".Lsigma");
	lgr	("%r0",$sp);
	la	($sp,"0(%r1,$sp)");
&{$z?	\&stg:\&st}	("%r0","0($sp)");

	lmg	("%r8","%r11","0($key)");	# load key
	lmg	("%r12","%r13","0($counter)");	# load counter
	lmg	("%r6","%r7","0(%r7)");	# load sigma constant

	la	("%r14","0($inp)");
&{$z?	\&stg:\&st}	($out,"$frame+3*$SIZE_T($sp)");
&{$z?	\&stg:\&st}	($len,"$frame+4*$SIZE_T($sp)");
	stmg	("%r6","%r13","$stdframe($sp)");# copy key schedule to stack
	srlg	(@x[12],"%r12",32);	# 32-bit counter value
	j	(".Loop_outer");

ALIGN	(16);
LABEL	(".Loop_outer");
	lm	(@x[0],@x[7],"$stdframe+4*0($sp)");	# load x[0]-x[7]
	lm	(@t[0],@t[1],"$stdframe+4*10($sp)");	# load x[10]-x[11]
	lm	(@x[13],@x[15],"$stdframe+4*13($sp)");	# load x[13]-x[15]
	stm	(@t[0],@t[1],"$stdframe+4*8+4*10($sp)");# offload x[10]-x[11]
	lm	(@t[0],@t[1],"$stdframe+4*8($sp)");	# load x[8]-x[9]
	st	(@x[12],"$stdframe+4*12($sp)");	# save counter
&{$z?	\&stg:\&st}	("%r14","$frame+2*$SIZE_T($sp)");# save input pointer
	lhi	("%r14",10);
	j	(".Loop");

ALIGN	(4);
LABEL	(".Loop");
	ROUND	(0, 4, 8,12);
	ROUND	(0, 5,10,15);
	brct	("%r14",".Loop");

&{$z?	\&lg:\&l}	("%r14","$frame+2*$SIZE_T($sp)");# pull input pointer
	stm	(@t[0],@t[1],"$stdframe+4*8+4*8($sp)");	# offload x[8]-x[9]
&{$z?	\&lmg:\&lm}	(@t[0],@t[1],"$frame+3*$SIZE_T($sp)");

	al	(@x[0],"$stdframe+4*0($sp)");	# accumulate key schedule
	al	(@x[1],"$stdframe+4*1($sp)");
	al	(@x[2],"$stdframe+4*2($sp)");
	al	(@x[3],"$stdframe+4*3($sp)");
	al	(@x[4],"$stdframe+4*4($sp)");
	al	(@x[5],"$stdframe+4*5($sp)");
	al	(@x[6],"$stdframe+4*6($sp)");
	al	(@x[7],"$stdframe+4*7($sp)");
	lrvr	(@x[0],@x[0]);
	lrvr	(@x[1],@x[1]);
	lrvr	(@x[2],@x[2]);
	lrvr	(@x[3],@x[3]);
	lrvr	(@x[4],@x[4]);
	lrvr	(@x[5],@x[5]);
	lrvr	(@x[6],@x[6]);
	lrvr	(@x[7],@x[7]);
	al	(@x[12],"$stdframe+4*12($sp)");
	al	(@x[13],"$stdframe+4*13($sp)");
	al	(@x[14],"$stdframe+4*14($sp)");
	al	(@x[15],"$stdframe+4*15($sp)");
	lrvr	(@x[12],@x[12]);
	lrvr	(@x[13],@x[13]);
	lrvr	(@x[14],@x[14]);
	lrvr	(@x[15],@x[15]);

	la	(@t[0],"0(@t[0],%r14)");	# reconstruct output pointer
&{$z?	\&clgr:\&clr}	("%r14",@t[1]);
	jh	(".Ltail");

	x	(@x[0],"4*0(%r14)");	# xor with input
	x	(@x[1],"4*1(%r14)");
	st	(@x[0],"4*0(@t[0])");	# store output
	x	(@x[2],"4*2(%r14)");
	st	(@x[1],"4*1(@t[0])");
	x	(@x[3],"4*3(%r14)");
	st	(@x[2],"4*2(@t[0])");
	x	(@x[4],"4*4(%r14)");
	st	(@x[3],"4*3(@t[0])");
	 lm	(@x[0],@x[3],"$stdframe+4*8+4*8($sp)");	# load x[8]-x[11]
	x	(@x[5],"4*5(%r14)");
	st	(@x[4],"4*4(@t[0])");
	x	(@x[6],"4*6(%r14)");
	 al	(@x[0],"$stdframe+4*8($sp)");
	st	(@x[5],"4*5(@t[0])");
	x	(@x[7],"4*7(%r14)");
	 al	(@x[1],"$stdframe+4*9($sp)");
	st	(@x[6],"4*6(@t[0])");
	x	(@x[12],"4*12(%r14)");
	 al	(@x[2],"$stdframe+4*10($sp)");
	st	(@x[7],"4*7(@t[0])");
	x	(@x[13],"4*13(%r14)");
	 al	(@x[3],"$stdframe+4*11($sp)");
	st	(@x[12],"4*12(@t[0])");
	x	(@x[14],"4*14(%r14)");
	st	(@x[13],"4*13(@t[0])");
	x	(@x[15],"4*15(%r14)");
	st	(@x[14],"4*14(@t[0])");
	 lrvr	(@x[0],@x[0]);
	st	(@x[15],"4*15(@t[0])");
	 lrvr	(@x[1],@x[1]);
	 lrvr	(@x[2],@x[2]);
	 lrvr	(@x[3],@x[3]);
	lhi	(@x[12],1);
	 x	(@x[0],"4*8(%r14)");
	al	(@x[12],"$stdframe+4*12($sp)");	# increment counter
	 x	(@x[1],"4*9(%r14)");
	 st	(@x[0],"4*8(@t[0])");
	 x	(@x[2],"4*10(%r14)");
	 st	(@x[1],"4*9(@t[0])");
	 x	(@x[3],"4*11(%r14)");
	 st	(@x[2],"4*10(@t[0])");
	 st	(@x[3],"4*11(@t[0])");

&{$z?	\&clgr:\&clr}	("%r14",@t[1]);	# done yet?
	la	("%r14","64(%r14)");
	jl	(".Loop_outer");

LABEL	(".Ldone");
	xgr	("%r0","%r0");
	xgr	("%r1","%r1");
	xgr	("%r2","%r2");
	xgr	("%r3","%r3");
	stmg	("%r0","%r3","$stdframe+4*4($sp)");	# wipe key copy
	stmg	("%r0","%r3","$stdframe+4*12($sp)");

&{$z?	\&lmg:\&lm}	("%r6","%r15","$frame+6*$SIZE_T($sp)");
	br	("%r14");

ALIGN	(16);
LABEL	(".Ltail");
	la	(@t[1],"64($t[1])");
	stm	(@x[0],@x[7],"$stdframe+4*0($sp)");
&{$z?	\&slgr:\&slr}	(@t[1],"%r14");
	lm	(@x[0],@x[3],"$stdframe+4*8+4*8($sp)");
&{$z?	\&lghi:\&lhi}	(@x[6],0);
	stm	(@x[12],@x[15],"$stdframe+4*12($sp)");
	al	(@x[0],"$stdframe+4*8($sp)");
	al	(@x[1],"$stdframe+4*9($sp)");
	al	(@x[2],"$stdframe+4*10($sp)");
	al	(@x[3],"$stdframe+4*11($sp)");
	lrvr	(@x[0],@x[0]);
	lrvr	(@x[1],@x[1]);
	lrvr	(@x[2],@x[2]);
	lrvr	(@x[3],@x[3]);
	stm	(@x[0],@x[3],"$stdframe+4*8($sp)");

LABEL	(".Loop_tail");
	llgc	(@x[4],"0(@x[6],%r14)");
	llgc	(@x[5],"$stdframe(@x[6],$sp)");
	xr	(@x[5],@x[4]);
	stc	(@x[5],"0(@x[6],@t[0])");
	la	(@x[6],"1(@x[6])");
	brct	(@t[1],".Loop_tail");

	j	(".Ldone");
SIZE	("ChaCha20_ctr32",".-ChaCha20_ctr32");
}

########################################################################
# 4x"vertical" layout minimizes amount of instructions, but pipeline
# runs underutilized [because of vector instructions' high latency].
# On the other hand minimum amount of data it takes to fully utilize
# the pipeline is higher, so that effectively, short inputs would be
# processed slower. Hence this code path targeting <=256 bytes lengths.
#
{
my ($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
    $xc0,$xc1,$xc2,$xc3, $xd0,$xd1,$xd2,$xd3)=map("%v$_",(0..15));
my @K=map("%v$_",(16..19));
my $CTR="%v26";
my ($xt0,$xt1,$xt2,$xt3)=map("%v$_",(27..30));
my $beperm="%v31";
my ($x00,$x10,$x20,$x30)=(0,map("r$_",(8..10)));
my $FRAME=$stdframe+4*16;

ALIGN	(32);
LABEL	("ChaCha20_ctr32_4x");
LABEL	(".LChaCha20_ctr32_4x");
&{$z?	\&stmg:\&stm}	("%r6","%r7","6*$SIZE_T($sp)");
if (!$z) {
	std	("%f4","16*$SIZE_T+2*8($sp)");
	std	("%f6","16*$SIZE_T+3*8($sp)");
}
&{$z?	\&lghi:\&lhi}	("%r1",-$FRAME);
	lgr	("%r0",$sp);
	la	($sp,"0(%r1,$sp)");
&{$z?	\&stg:\&st}	("%r0","0($sp)");	# back-chain
if ($z) {
	std	("%f8","$stdframe+8*0($sp)");
	std	("%f9","$stdframe+8*1($sp)");
	std	("%f10","$stdframe+8*2($sp)");
	std	("%f11","$stdframe+8*3($sp)");
	std	("%f12","$stdframe+8*4($sp)");
	std	("%f13","$stdframe+8*5($sp)");
	std	("%f14","$stdframe+8*6($sp)");
	std	("%f15","$stdframe+8*7($sp)");
}
	larl	("%r7",".Lsigma");
	lhi	("%r0",10);
	lhi	("%r1",0);

	vl	(@K[0],"0(%r7)");		# load sigma
	vl	(@K[1],"0($key)");		# load key
	vl	(@K[2],"16($key)");
	vl	(@K[3],"0($counter)");		# load counter

	vl	($beperm,"0x40(%r7)");
	vl	($xt1,"0x50(%r7)");
	vrepf	($CTR,@K[3],0);
	vlvgf	(@K[3],"%r1",0);		# clear @K[3].word[0]
	vaf	($CTR,$CTR,$xt1);

#LABEL	(".Loop_outer_4x");
	vlm	($xa0,$xa3,"0x60(%r7)");	# load [smashed] sigma

	vrepf	($xb0,@K[1],0);			# smash the key
	vrepf	($xb1,@K[1],1);
	vrepf	($xb2,@K[1],2);
	vrepf	($xb3,@K[1],3);

	vrepf	($xc0,@K[2],0);
	vrepf	($xc1,@K[2],1);
	vrepf	($xc2,@K[2],2);
	vrepf	($xc3,@K[2],3);

	vlr	($xd0,$CTR);
	vrepf	($xd1,@K[3],1);
	vrepf	($xd2,@K[3],2);
	vrepf	($xd3,@K[3],3);

LABEL	(".Loop_4x");
	VX_lane_ROUND(0, 4, 8,12);
	VX_lane_ROUND(0, 5,10,15);
	brct	("%r0",".Loop_4x");

	vaf	($xd0,$xd0,$CTR);

	vmrhf	($xt0,$xa0,$xa1);		# transpose data
	vmrhf	($xt1,$xa2,$xa3);
	vmrlf	($xt2,$xa0,$xa1);
	vmrlf	($xt3,$xa2,$xa3);
	vpdi	($xa0,$xt0,$xt1,0b0000);
	vpdi	($xa1,$xt0,$xt1,0b0101);
	vpdi	($xa2,$xt2,$xt3,0b0000);
	vpdi	($xa3,$xt2,$xt3,0b0101);

	vmrhf	($xt0,$xb0,$xb1);
	vmrhf	($xt1,$xb2,$xb3);
	vmrlf	($xt2,$xb0,$xb1);
	vmrlf	($xt3,$xb2,$xb3);
	vpdi	($xb0,$xt0,$xt1,0b0000);
	vpdi	($xb1,$xt0,$xt1,0b0101);
	vpdi	($xb2,$xt2,$xt3,0b0000);
	vpdi	($xb3,$xt2,$xt3,0b0101);

	vmrhf	($xt0,$xc0,$xc1);
	vmrhf	($xt1,$xc2,$xc3);
	vmrlf	($xt2,$xc0,$xc1);
	vmrlf	($xt3,$xc2,$xc3);
	vpdi	($xc0,$xt0,$xt1,0b0000);
	vpdi	($xc1,$xt0,$xt1,0b0101);
	vpdi	($xc2,$xt2,$xt3,0b0000);
	vpdi	($xc3,$xt2,$xt3,0b0101);

	vmrhf	($xt0,$xd0,$xd1);
	vmrhf	($xt1,$xd2,$xd3);
	vmrlf	($xt2,$xd0,$xd1);
	vmrlf	($xt3,$xd2,$xd3);
	vpdi	($xd0,$xt0,$xt1,0b0000);
	vpdi	($xd1,$xt0,$xt1,0b0101);
	vpdi	($xd2,$xt2,$xt3,0b0000);
	vpdi	($xd3,$xt2,$xt3,0b0101);

	#vrepif	($xt0,4);
	#vaf	($CTR,$CTR,$xt0);		# next counter value

	vaf	($xa0,$xa0,@K[0]);
	vaf	($xb0,$xb0,@K[1]);
	vaf	($xc0,$xc0,@K[2]);
	vaf	($xd0,$xd0,@K[3]);

	vperm	($xa0,$xa0,$xa0,$beperm);
	vperm	($xb0,$xb0,$xb0,$beperm);
	vperm	($xc0,$xc0,$xc0,$beperm);
	vperm	($xd0,$xd0,$xd0,$beperm);

	#&{$z?	\&clgfi:\&clfi} ($len,0x40);
	#jl	(".Ltail_4x");

	vlm	($xt0,$xt3,"0($inp)");

	vx	($xt0,$xt0,$xa0);
	vx	($xt1,$xt1,$xb0);
	vx	($xt2,$xt2,$xc0);
	vx	($xt3,$xt3,$xd0);

	vstm	($xt0,$xt3,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	#je	(".Ldone_4x");

	vaf	($xa0,$xa1,@K[0]);
	vaf	($xb0,$xb1,@K[1]);
	vaf	($xc0,$xc1,@K[2]);
	vaf	($xd0,$xd1,@K[3]);

	vperm	($xa0,$xa0,$xa0,$beperm);
	vperm	($xb0,$xb0,$xb0,$beperm);
	vperm	($xc0,$xc0,$xc0,$beperm);
	vperm	($xd0,$xd0,$xd0,$beperm);

&{$z?	\&clgfi:\&clfi} ($len,0x40);
	jl	(".Ltail_4x");

	vlm	($xt0,$xt3,"0($inp)");

	vx	($xt0,$xt0,$xa0);
	vx	($xt1,$xt1,$xb0);
	vx	($xt2,$xt2,$xc0);
	vx	($xt3,$xt3,$xd0);

	vstm	($xt0,$xt3,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_4x");

	vaf	($xa0,$xa2,@K[0]);
	vaf	($xb0,$xb2,@K[1]);
	vaf	($xc0,$xc2,@K[2]);
	vaf	($xd0,$xd2,@K[3]);

	vperm	($xa0,$xa0,$xa0,$beperm);
	vperm	($xb0,$xb0,$xb0,$beperm);
	vperm	($xc0,$xc0,$xc0,$beperm);
	vperm	($xd0,$xd0,$xd0,$beperm);

&{$z?	\&clgfi:\&clfi} ($len,0x40);
	jl	(".Ltail_4x");

	vlm	($xt0,$xt3,"0($inp)");

	vx	($xt0,$xt0,$xa0);
	vx	($xt1,$xt1,$xb0);
	vx	($xt2,$xt2,$xc0);
	vx	($xt3,$xt3,$xd0);

	vstm	($xt0,$xt3,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_4x");

	vaf	($xa0,$xa3,@K[0]);
	vaf	($xb0,$xb3,@K[1]);
	vaf	($xc0,$xc3,@K[2]);
	vaf	($xd0,$xd3,@K[3]);

	vperm	($xa0,$xa0,$xa0,$beperm);
	vperm	($xb0,$xb0,$xb0,$beperm);
	vperm	($xc0,$xc0,$xc0,$beperm);
	vperm	($xd0,$xd0,$xd0,$beperm);

&{$z?	\&clgfi:\&clfi} ($len,0x40);
	jl	(".Ltail_4x");

	vlm	($xt0,$xt3,"0($inp)");

	vx	($xt0,$xt0,$xa0);
	vx	($xt1,$xt1,$xb0);
	vx	($xt2,$xt2,$xc0);
	vx	($xt3,$xt3,$xd0);

	vstm	($xt0,$xt3,"0($out)");

	#la	$inp,0x40($inp));
	#la	$out,0x40($out));
	#lhi	%r0,10);
	#&{$z?	\&aghi:\&ahi}	$len,-0x40);
	#jne	.Loop_outer_4x);

LABEL	(".Ldone_4x");
if (!$z) {
	ld	("%f4","$FRAME+16*$SIZE_T+2*8($sp)");
	ld	("%f6","$FRAME+16*$SIZE_T+3*8($sp)");
} else {
	ld	("%f8","$stdframe+8*0($sp)");
	ld	("%f9","$stdframe+8*1($sp)");
	ld	("%f10","$stdframe+8*2($sp)");
	ld	("%f11","$stdframe+8*3($sp)");
	ld	("%f12","$stdframe+8*4($sp)");
	ld	("%f13","$stdframe+8*5($sp)");
	ld	("%f14","$stdframe+8*6($sp)");
	ld	("%f15","$stdframe+8*7($sp)");
}
&{$z?	\&lmg:\&lm}	("%r6","%r7","$FRAME+6*$SIZE_T($sp)");
	la	($sp,"$FRAME($sp)");
	br	("%r14");

ALIGN	(16);
LABEL	(".Ltail_4x");
if (!$z) {
	vlr	($xt0,$xb0);
	ld	("%f4","$FRAME+16*$SIZE_T+2*8($sp)");
	ld	("%f6","$FRAME+16*$SIZE_T+3*8($sp)");

	vst	($xa0,"$stdframe+0x00($sp)");
	vst	($xt0,"$stdframe+0x10($sp)");
	vst	($xc0,"$stdframe+0x20($sp)");
	vst	($xd0,"$stdframe+0x30($sp)");
} else {
	vlr	($xt0,$xc0);
	ld	("%f8","$stdframe+8*0($sp)");
	ld	("%f9","$stdframe+8*1($sp)");
	ld	("%f10","$stdframe+8*2($sp)");
	ld	("%f11","$stdframe+8*3($sp)");
	vlr	($xt1,$xd0);
	ld	("%f12","$stdframe+8*4($sp)");
	ld	("%f13","$stdframe+8*5($sp)");
	ld	("%f14","$stdframe+8*6($sp)");
	ld	("%f15","$stdframe+8*7($sp)");

	vst	($xa0,"$stdframe+0x00($sp)");
	vst	($xb0,"$stdframe+0x10($sp)");
	vst	($xt0,"$stdframe+0x20($sp)");
	vst	($xt1,"$stdframe+0x30($sp)");
}
	lghi	("%r1",0);

LABEL	(".Loop_tail_4x");
	llgc	("%r5","0(%r1,$inp)");
	llgc	("%r6","$stdframe(%r1,$sp)");
	xr	("%r6","%r5");
	stc	("%r6","0(%r1,$out)");
	la	("%r1","1(%r1)");
	brct	($len,".Loop_tail_4x");

&{$z?	\&lmg:\&lm}	("%r6","%r7","$FRAME+6*$SIZE_T($sp)");
	la	($sp,"$FRAME($sp)");
	br	("%r14");
SIZE	("ChaCha20_ctr32_4x",".-ChaCha20_ctr32_4x");
}

########################################################################
# 6x"horizontal" layout is optimal fit for the platform in its current
# shape, more specifically for given vector instructions' latency. Well,
# computational part of 8x"vertical" would be faster, but it consumes
# all registers and dealing with that will diminish the return...
#
{
my ($a0,$b0,$c0,$d0, $a1,$b1,$c1,$d1,
    $a2,$b2,$c2,$d2, $a3,$b3,$c3,$d3,
    $a4,$b4,$c4,$d4, $a5,$b5,$c5,$d5)=map("%v$_",(0..23));
my @K=map("%v$_",(27,24..26));
my ($t0,$t1,$t2,$t3)=map("%v$_",27..30);
my $beperm="%v31";
my $FRAME=$stdframe + 4*16;

GLOBL	("ChaCha20_ctr32_vx");
ALIGN	(32);
LABEL	("ChaCha20_ctr32_vx");
LABEL	(".LChaCha20_ctr32_vx");
&{$z?	\&clgfi:\&clfi}	($len,256);
	jle	(".LChaCha20_ctr32_4x");
&{$z?	\&stmg:\&stm}	("%r6","%r7","6*$SIZE_T($sp)");
if (!$z) {
	std	("%f4","16*$SIZE_T+2*8($sp)");
	std	("%f6","16*$SIZE_T+3*8($sp)");
}
&{$z?	\&lghi:\&lhi}	("%r1",-$FRAME);
	lgr	("%r0",$sp);
	la	($sp,"0(%r1,$sp)");
&{$z?	\&stg:\&st}	("%r0","0($sp)");	# back-chain
if ($z) {
	std	("%f8","$FRAME-8*8($sp)");
	std	("%f9","$FRAME-8*7($sp)");
	std	("%f10","$FRAME-8*6($sp)");
	std	("%f11","$FRAME-8*5($sp)");
	std	("%f12","$FRAME-8*4($sp)");
	std	("%f13","$FRAME-8*3($sp)");
	std	("%f14","$FRAME-8*2($sp)");
	std	("%f15","$FRAME-8*1($sp)");
}
	larl	("%r7",".Lsigma");
	lhi	("%r0",10);

	vlm	(@K[1],@K[2],"0($key)");	# load key
	vl	(@K[3],"0($counter)");		# load counter

	vlm	(@K[0],"$beperm","0(%r7)");	# load sigma, increments, ...

LABEL	(".Loop_outer_vx");
	vlr	($a0,@K[0]);
	vlr	($b0,@K[1]);
	vlr	($a1,@K[0]);
	vlr	($b1,@K[1]);
	vlr	($a2,@K[0]);
	vlr	($b2,@K[1]);
	vlr	($a3,@K[0]);
	vlr	($b3,@K[1]);
	vlr	($a4,@K[0]);
	vlr	($b4,@K[1]);
	vlr	($a5,@K[0]);
	vlr	($b5,@K[1]);

	vlr	($d0,@K[3]);
	vaf	($d1,@K[3],$t1);		# K[3]+1
	vaf	($d2,@K[3],$t2);		# K[3]+2
	vaf	($d3,@K[3],$t3);		# K[3]+3
	vaf	($d4,$d2,$t2);			# K[3]+4
	vaf	($d5,$d2,$t3);			# K[3]+5

	vlr	($c0,@K[2]);
	vlr	($c1,@K[2]);
	vlr	($c2,@K[2]);
	vlr	($c3,@K[2]);
	vlr	($c4,@K[2]);
	vlr	($c5,@K[2]);

	vlr	($t1,$d1);
	vlr	($t2,$d2);
	vlr	($t3,$d3);

ALIGN	(4);
LABEL	(".Loop_vx");

	VX_ROUND($a0,$a1,$a2,$a3,$a4,$a5,
		 $b0,$b1,$b2,$b3,$b4,$b5,
		 $c0,$c1,$c2,$c3,$c4,$c5,
		 $d0,$d1,$d2,$d3,$d4,$d5,
		 0);

	VX_ROUND($a0,$a1,$a2,$a3,$a4,$a5,
		 $b0,$b1,$b2,$b3,$b4,$b5,
		 $c0,$c1,$c2,$c3,$c4,$c5,
		 $d0,$d1,$d2,$d3,$d4,$d5,
		 1);

	brct	("%r0",".Loop_vx");

	vaf	($a0,$a0,@K[0]);
	vaf	($b0,$b0,@K[1]);
	vaf	($c0,$c0,@K[2]);
	vaf	($d0,$d0,@K[3]);
	vaf	($a1,$a1,@K[0]);
	vaf	($d1,$d1,$t1);			# +K[3]+1

	vperm	($a0,$a0,$a0,$beperm);
	vperm	($b0,$b0,$b0,$beperm);
	vperm	($c0,$c0,$c0,$beperm);
	vperm	($d0,$d0,$d0,$beperm);

&{$z?	\&clgfi:\&clfi}	($len,0x40);
	jl	(".Ltail_vx");

	vaf	($d2,$d2,$t2);			# +K[3]+2
	vaf	($d3,$d3,$t3);			# +K[3]+3
	vlm	($t0,$t3,"0($inp)");

	vx	($a0,$a0,$t0);
	vx	($b0,$b0,$t1);
	vx	($c0,$c0,$t2);
	vx	($d0,$d0,$t3);

	vlm	(@K[0],$t3,"0(%r7)");		# re-load sigma and increments

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_vx");

	vaf	($b1,$b1,@K[1]);
	vaf	($c1,$c1,@K[2]);

	vperm	($a0,$a1,$a1,$beperm);
	vperm	($b0,$b1,$b1,$beperm);
	vperm	($c0,$c1,$c1,$beperm);
	vperm	($d0,$d1,$d1,$beperm);

&{$z?	\&clgfi:\&clfi} ($len,0x40);
	jl	(".Ltail_vx");

	vlm	($a1,$d1,"0($inp)");

	vx	($a0,$a0,$a1);
	vx	($b0,$b0,$b1);
	vx	($c0,$c0,$c1);
	vx	($d0,$d0,$d1);

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_vx");

	vaf	($a2,$a2,@K[0]);
	vaf	($b2,$b2,@K[1]);
	vaf	($c2,$c2,@K[2]);

	vperm	($a0,$a2,$a2,$beperm);
	vperm	($b0,$b2,$b2,$beperm);
	vperm	($c0,$c2,$c2,$beperm);
	vperm	($d0,$d2,$d2,$beperm);

&{$z?	\&clgfi:\&clfi}	($len,0x40);
	jl	(".Ltail_vx");

	vlm	($a1,$d1,"0($inp)");

	vx	($a0,$a0,$a1);
	vx	($b0,$b0,$b1);
	vx	($c0,$c0,$c1);
	vx	($d0,$d0,$d1);

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_vx");

	vaf	($a3,$a3,@K[0]);
	vaf	($b3,$b3,@K[1]);
	vaf	($c3,$c3,@K[2]);
	vaf	($d2,@K[3],$t3);		# K[3]+3

	vperm	($a0,$a3,$a3,$beperm);
	vperm	($b0,$b3,$b3,$beperm);
	vperm	($c0,$c3,$c3,$beperm);
	vperm	($d0,$d3,$d3,$beperm);

&{$z?	\&clgfi:\&clfi}	($len,0x40);
	jl	(".Ltail_vx");

	vaf	($d3,$d2,$t1);			# K[3]+4
	vlm	($a1,$d1,"0($inp)");

	vx	($a0,$a0,$a1);
	vx	($b0,$b0,$b1);
	vx	($c0,$c0,$c1);
	vx	($d0,$d0,$d1);

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_vx");

	vaf	($a4,$a4,@K[0]);
	vaf	($b4,$b4,@K[1]);
	vaf	($c4,$c4,@K[2]);
	vaf	($d4,$d4,$d3);			# +K[3]+4
	vaf	($d3,$d3,$t1);			# K[3]+5
	vaf	(@K[3],$d2,$t3);		# K[3]+=6

	vperm	($a0,$a4,$a4,$beperm);
	vperm	($b0,$b4,$b4,$beperm);
	vperm	($c0,$c4,$c4,$beperm);
	vperm	($d0,$d4,$d4,$beperm);

&{$z?	\&clgfi:\&clfi}	($len,0x40);
	jl	(".Ltail_vx");

	vlm	($a1,$d1,"0($inp)");

	vx	($a0,$a0,$a1);
	vx	($b0,$b0,$b1);
	vx	($c0,$c0,$c1);
	vx	($d0,$d0,$d1);

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	je	(".Ldone_vx");

	vaf	($a5,$a5,@K[0]);
	vaf	($b5,$b5,@K[1]);
	vaf	($c5,$c5,@K[2]);
	vaf	($d5,$d5,$d3);			# +K[3]+5

	vperm	($a0,$a5,$a5,$beperm);
	vperm	($b0,$b5,$b5,$beperm);
	vperm	($c0,$c5,$c5,$beperm);
	vperm	($d0,$d5,$d5,$beperm);

&{$z?	\&clgfi:\&clfi} ($len,0x40);
	jl	(".Ltail_vx");

	vlm	($a1,$d1,"0($inp)");

	vx	($a0,$a0,$a1);
	vx	($b0,$b0,$b1);
	vx	($c0,$c0,$c1);
	vx	($d0,$d0,$d1);

	vstm	($a0,$d0,"0($out)");

	la	($inp,"0x40($inp)");
	la	($out,"0x40($out)");
	lhi	("%r0",10);
&{$z?	\&aghi:\&ahi}	($len,-0x40);
	jne	(".Loop_outer_vx");

LABEL	(".Ldone_vx");
if (!$z) {
	ld	("%f4","$FRAME+16*$SIZE_T+2*8($sp)");
	ld	("%f6","$FRAME+16*$SIZE_T+3*8($sp)");
} else {
	ld	("%f8","$FRAME-8*8($sp)");
	ld	("%f9","$FRAME-8*7($sp)");
	ld	("%f10","$FRAME-8*6($sp)");
	ld	("%f11","$FRAME-8*5($sp)");
	ld	("%f12","$FRAME-8*4($sp)");
	ld	("%f13","$FRAME-8*3($sp)");
	ld	("%f14","$FRAME-8*2($sp)");
	ld	("%f15","$FRAME-8*1($sp)");
}
&{$z?	\&lmg:\&lm}	("%r6","%r7","$FRAME+6*$SIZE_T($sp)");
	la	($sp,"$FRAME($sp)");
	br	("%r14");

ALIGN	(16);
LABEL	(".Ltail_vx");
if (!$z) {
	ld	("%f4","$FRAME+16*$SIZE_T+2*8($sp)");
	ld	("%f6","$FRAME+16*$SIZE_T+3*8($sp)");
} else {
	ld	("%f8","$FRAME-8*8($sp)");
	ld	("%f9","$FRAME-8*7($sp)");
	ld	("%f10","$FRAME-8*6($sp)");
	ld	("%f11","$FRAME-8*5($sp)");
	ld	("%f12","$FRAME-8*4($sp)");
	ld	("%f13","$FRAME-8*3($sp)");
	ld	("%f14","$FRAME-8*2($sp)");
	ld	("%f15","$FRAME-8*1($sp)");
}
	vstm	($a0,$d0,"$stdframe($sp)");
	lghi	("%r1",0);

LABEL	(".Loop_tail_vx");
	llgc	("%r5","0(%r1,$inp)");
	llgc	("%r6","$stdframe(%r1,$sp)");
	xr	("%r6","%r5");
	stc	("%r6","0(%r1,$out)");
	la	("%r1","1(%r1)");
	brct	($len,".Loop_tail_vx");

&{$z?	\&lmg:\&lm}	("%r6","%r7","$FRAME+6*$SIZE_T($sp)");
	la	($sp,"$FRAME($sp)");
	br	("%r14");
SIZE	("ChaCha20_ctr32_vx",".-ChaCha20_ctr32_vx");
}
################

ALIGN	(32);
LABEL	(".Lsigma");
LONG	(0x61707865,0x3320646e,0x79622d32,0x6b206574);	# endian-neutral sigma
LONG	(1,0,0,0);
LONG	(2,0,0,0);
LONG	(3,0,0,0);
LONG	(0x03020100,0x07060504,0x0b0a0908,0x0f0e0d0c);	# byte swap

LONG	(0,1,2,3);
LONG	(0x61707865,0x61707865,0x61707865,0x61707865);	# smashed sigma
LONG	(0x3320646e,0x3320646e,0x3320646e,0x3320646e);
LONG	(0x79622d32,0x79622d32,0x79622d32,0x79622d32);
LONG	(0x6b206574,0x6b206574,0x6b206574,0x6b206574);

ASCIZ	("\"ChaCha20 for s390x, CRYPTOGAMS by <appro\@openssl.org>\"");
ALIGN	(4);

PERLASM_END();
