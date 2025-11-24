#! /usr/bin/env perl
# Copyright 2016 The OpenSSL Project Authors. All Rights Reserved.
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
# This module implements Poly1305 hash for s390x.
#
# June 2015
#
# ~6.6/2.3 cpb on z10/z196+, >2x improvement over compiler-generated
# code. For older compiler improvement coefficient is >3x, because
# then base 2^64 and base 2^32 implementations are compared.
#
# On side note, z13 enables vector base 2^26 implementation...

#
# January 2019
#
# Add vx code path (base 2^26).
#
# Copyright IBM Corp. 2019
# Author: Patrick Steuer <patrick.steuer@de.ibm.com>

#
# January 2019
#
# Add vector base 2^26 implementation. It's problematic to accurately
# measure performance, because reference system is hardly idle. But
# it's sub-cycle, i.e. less than 1 cycle per processed byte, and it's
# >=20% faster than IBM's submission on long inputs, and much faster on
# short ones, because calculation of key powers is postponed till we
# know that input is long enough to justify the additional overhead.

use strict;
use FindBin qw($Bin);
use lib "$Bin/../..";
use perlasm::s390x qw(:DEFAULT :GE :EI :MI1 :VX AUTOLOAD LABEL INCLUDE);

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

my $stdframe=16*$SIZE_T+4*8;
my $sp="%r15";

my ($ctx,$inp,$len,$padbit) = map("%r$_",(2..5));

PERLASM_BEGIN($output);

INCLUDE	("s390x_arch.h");
TEXT	();

################
# static void poly1305_init(void *ctx, const unsigned char key[16])
{
GLOBL	("poly1305_init");
TYPE	("poly1305_init","\@function");
ALIGN	(16);
LABEL	("poly1305_init");
	lghi	("%r0",0);
	lghi	("%r1",-1);
	stg	("%r0","0($ctx)");		# zero hash value
	stg	("%r0","8($ctx)");
	stg	("%r0","16($ctx)");
	st	("%r0","24($ctx)");		# clear is_base2_26
	lgr	("%r5",$ctx);			# reassign $ctx
	lghi	("%r2",0);

&{$z?	\&clgr:\&clr}	($inp,"%r0");
	je	(".Lno_key");

	lrvg	("%r2","0($inp)");		# load little-endian key
	lrvg	("%r3","8($inp)");

	nihl	("%r1",0xffc0);			# 0xffffffc0ffffffff
	srlg	("%r0","%r1",4);		# 0x0ffffffc0fffffff
	srlg	("%r1","%r1",4);
	nill	("%r1",0xfffc);			# 0x0ffffffc0ffffffc

	ngr	("%r2","%r0");
	ngr	("%r3","%r1");

	stmg	("%r2","%r3","32(%r5)");

	larl	("%r1","OPENSSL_s390xcap_P");
	lg	("%r0","16(%r1)");
	srlg	("%r0","%r0",62);
	nill	("%r0",1);			# extract vx bit
	lcgr	("%r0","%r0");
	larl	("%r1",".Lpoly1305_blocks");
	larl	("%r2",".Lpoly1305_blocks_vx");
	larl	("%r3",".Lpoly1305_emit");
&{$z?	\&xgr:\&xr}	("%r2","%r1");		# select between scalar and vector
&{$z?	\&ngr:\&nr}	("%r2","%r0");
&{$z?	\&xgr:\&xr}	("%r2","%r1");
&{$z?	\&stmg:\&stm}	("%r2","%r3","0(%r4)");
	lghi	("%r2",1);
LABEL	(".Lno_key");
	br	("%r14");
SIZE	("poly1305_init",".-poly1305_init");
}

################
# static void poly1305_blocks(void *ctx, const unsigned char *inp,
#                             size_t len, u32 padbit)
{
my ($d0hi,$d0lo,$d1hi,$d1lo,$t0,$h0,$t1,$h1,$h2) = map("%r$_",(6..14));
my ($r0,$r1,$s1) = map("%r$_",(0..2));

GLOBL	("poly1305_blocks");
TYPE	("poly1305_blocks","\@function");
ALIGN	(16);
LABEL	("poly1305_blocks");
LABEL	(".Lpoly1305_blocks");
&{$z?	\&ltgr:\&ltr}	("%r0",$len);
	jz	(".Lno_data");

&{$z?	\&stmg:\&stm}	("%r6","%r14","6*$SIZE_T($sp)");

	lg	($h0,"0($ctx)");		# load hash value
	lg	($h1,"8($ctx)");
	lg	($h2,"16($ctx)");

LABEL	(".Lpoly1305_blocks_entry");
if ($z) {
	srlg	($len,$len,4);
} else {
	srl	($len,4);
}
	llgfr   ($padbit,$padbit);		# clear upper half, much needed with
						# non-64-bit ABI
	lg	($r0,"32($ctx)");		# load key
	lg	($r1,"40($ctx)");

&{$z?	\&stg:\&st}	($ctx,"2*$SIZE_T($sp)");	# off-load $ctx
	srlg	($s1,$r1,2);
	algr	($s1,$r1);			# s1 = r1 + r1>>2
	j	(".Loop");

ALIGN	(16);
LABEL	(".Loop");
	lrvg	($d0lo,"0($inp)");		# load little-endian input
	lrvg	($d1lo,"8($inp)");
	la	($inp,"16($inp)");

	algr	($d0lo,$h0);			# accumulate input
	alcgr	($d1lo,$h1);
	alcgr	($h2,$padbit);

	lgr	($h0,$d0lo);
	mlgr	($d0hi,$r0);			# h0*r0	  -> $d0hi:$d0lo
	lgr	($h1,$d1lo);
	mlgr	($d1hi,$s1);			# h1*5*r1 -> $d1hi:$d1lo

	mlgr	($t0,$r1);			# h0*r1   -> $t0:$h0
	mlgr	($t1,$r0);			# h1*r0   -> $t1:$h1

	algr	($d0lo,$d1lo);
	lgr	($d1lo,$h2);
	alcgr	($d0hi,$d1hi);
	lghi	($d1hi,0);

	algr	($h1,$h0);
	alcgr	($t1,$t0);

	msgr	($d1lo,$s1);			# h2*s1
	msgr	($h2,$r0);			# h2*r0

	algr	($h1,$d1lo);
	alcgr	($t1,$d1hi);			# $d1hi is zero

	algr	($h1,$d0hi);
	alcgr	($h2,$t1);

	lghi	($h0,-4);			# final reduction step
	ngr	($h0,$h2);
	srlg	($t0,$h2,2);
	algr	($h0,$t0);
	lghi	($t1,3);
	ngr	($h2,$t1);

	algr	($h0,$d0lo);
	alcgr	($h1,$d1hi);			# $d1hi is still zero
	alcgr	($h2,$d1hi);			# $d1hi is still zero

&{$z?	\&brctg:\&brct}	($len,".Loop");

&{$z?	\&lg:\&l}	($ctx,"2*$SIZE_T($sp)");# restore $ctx

	stg	($h0,"0($ctx)");		# store hash value
	stg	($h1,"8($ctx)");
	stg	($h2,"16($ctx)");

&{$z?	\&lmg:\&lm}	("%r6","%r14","6*$SIZE_T($sp)");
LABEL	(".Lno_data");
	br	("%r14");
SIZE	("poly1305_blocks",".-poly1305_blocks");
}

################
# static void poly1305_blocks_vx(void *ctx, const unsigned char *inp,
#                                size_t len, u32 padbit)
{
my ($H0, $H1, $H2, $H3, $H4) = map("%v$_",(0..4));
my ($I0, $I1, $I2, $I3, $I4) = map("%v$_",(5..9));
my ($R0, $R1, $S1, $R2, $S2) = map("%v$_",(10..14));
my      ($R3, $S3, $R4, $S4) = map("%v$_",(15..18));
my ($ACC0, $ACC1, $ACC2, $ACC3, $ACC4) = map("%v$_",(19..23));
my      ($T1, $T2, $T3, $T4) = map("%v$_",(24..27));
my ($mask26,$bswaplo,$bswaphi,$bswapmi) = map("%v$_",(28..31));

my ($d2,$d0,$h0,$d1,$h1,$h2)=map("%r$_",(9..14));

TYPE	("poly1305_blocks_vx","\@function");
ALIGN	(16);
LABEL	("poly1305_blocks_vx");
LABEL	(".Lpoly1305_blocks_vx");
&{$z?	\&clgfi:\&clfi} ($len,128);
	jhe	("__poly1305_blocks_vx");

&{$z?	\&stmg:\&stm}	("%r6","%r14","6*$SIZE_T($sp)");

	lg	($d0,"0($ctx)");
	lg	($d1,"8($ctx)");
	lg	($d2,"16($ctx)");

	llgfr	("%r0",$d0);			# base 2^26 -> base 2^64
	srlg	($h0,$d0,32);
	llgfr	("%r1",$d1);
	srlg	($h1,$d1,32);
	srlg	($h2,$d2,32);

	sllg	("%r0","%r0",26);
	algr	($h0,"%r0");
	sllg	("%r0",$h1,52);
	srlg	($h1,$h1,12);
	sllg	("%r1","%r1",14);
	algr	($h0,"%r0");
	alcgr	($h1,"%r1");
	sllg	("%r0",$h2,40);
	srlg	($h2,$h2,24);
	lghi	("%r1",0);
	algr	($h1,"%r0");
	alcgr	($h2,"%r1");

	llgf	("%r0","24($ctx)");		# is_base2_26
	lcgr	("%r0","%r0");

	xgr	($h0,$d0);			# choose between radixes
	xgr	($h1,$d1);
	xgr	($h2,$d2);
	ngr	($h0,"%r0");
	ngr	($h1,"%r0");
	ngr	($h2,"%r0");
	xgr	($h0,$d0);
	xgr	($h1,$d1);
	xgr	($h2,$d2);

	lhi	("%r0",0);
	st	("%r0","24($ctx)");		# clear is_base2_26

	j	(".Lpoly1305_blocks_entry");
SIZE	("poly1305_blocks_vx",".-poly1305_blocks_vx");

TYPE	("__poly1305_mul","\@function");
ALIGN	(16);
LABEL	("__poly1305_mul");
	vmlof		($ACC0,$H0,$R0);
	vmlof		($ACC1,$H0,$R1);
	vmlof		($ACC2,$H0,$R2);
	vmlof		($ACC3,$H0,$R3);
	vmlof		($ACC4,$H0,$R4);

	vmalof		($ACC0,$H1,$S4,$ACC0);
	vmalof		($ACC1,$H1,$R0,$ACC1);
	vmalof		($ACC2,$H1,$R1,$ACC2);
	vmalof		($ACC3,$H1,$R2,$ACC3);
	vmalof		($ACC4,$H1,$R3,$ACC4);

	vmalof		($ACC0,$H2,$S3,$ACC0);
	vmalof		($ACC1,$H2,$S4,$ACC1);
	vmalof		($ACC2,$H2,$R0,$ACC2);
	vmalof		($ACC3,$H2,$R1,$ACC3);
	vmalof		($ACC4,$H2,$R2,$ACC4);

	vmalof		($ACC0,$H3,$S2,$ACC0);
	vmalof		($ACC1,$H3,$S3,$ACC1);
	vmalof		($ACC2,$H3,$S4,$ACC2);
	vmalof		($ACC3,$H3,$R0,$ACC3);
	vmalof		($ACC4,$H3,$R1,$ACC4);

	vmalof		($ACC0,$H4,$S1,$ACC0);
	vmalof		($ACC1,$H4,$S2,$ACC1);
	vmalof		($ACC2,$H4,$S3,$ACC2);
	vmalof		($ACC3,$H4,$S4,$ACC3);
	vmalof		($ACC4,$H4,$R0,$ACC4);

	################################################################
	# lazy reduction

	vesrlg		($H4,$ACC3,26);
	vesrlg		($H1,$ACC0,26);
	vn		($H3,$ACC3,$mask26);
	vn		($H0,$ACC0,$mask26);
	vag		($H4,$H4,$ACC4);	# h3 -> h4
	vag		($H1,$H1,$ACC1);	# h0 -> h1

	vesrlg		($ACC4,$H4,26);
	vesrlg		($ACC1,$H1,26);
	vn		($H4,$H4,$mask26);
	vn		($H1,$H1,$mask26);
	vag		($H0,$H0,$ACC4);
	vag		($H2,$ACC2,$ACC1);	# h1 -> h2

	veslg		($ACC4,$ACC4,2);	# <<2
	vesrlg		($ACC2,$H2,26);
	vn		($H2,$H2,$mask26);
	vag		($H0,$H0,$ACC4);	# h4 -> h0
	vag		($H3,$H3,$ACC2);	# h2 -> h3

	vesrlg		($ACC0,$H0,26);
	vesrlg		($ACC3,$H3,26);
	vn		($H0,$H0,$mask26);
	vn		($H3,$H3,$mask26);
	vag		($H1,$H1,$ACC0);	# h0 -> h1
	vag		($H4,$H4,$ACC3);	# h3 -> h4
	br		("%r14");
SIZE	("__poly1305_mul",".-__poly1305_mul");

TYPE	("__poly1305_blocks_vx","\@function");
ALIGN	(16);
LABEL	("__poly1305_blocks_vx");
&{$z?	\&lgr:\&lr}	("%r0",$sp);
&{$z?	\&stmg:\&stm}	("%r10","%r15","10*$SIZE_T($sp)");
if (!$z) {
	std	("%f4","16*$SIZE_T+2*8($sp)");
	std	("%f6","16*$SIZE_T+3*8($sp)");
	ahi	($sp,-$stdframe);
	st	("%r0","0($sp)");		# back-chain

	llgfr	($len,$len);			# so that srlg works on $len
} else {
	aghi	($sp,"-($stdframe+8*8)");
	stg	("%r0","0($sp)");		# back-chain

	std	("%f8","$stdframe+0*8($sp)");
	std	("%f9","$stdframe+1*8($sp)");
	std	("%f10","$stdframe+2*8($sp)");
	std	("%f11","$stdframe+3*8($sp)");
	std	("%f12","$stdframe+4*8($sp)");
	std	("%f13","$stdframe+5*8($sp)");
	std	("%f14","$stdframe+6*8($sp)");
	std	("%f15","$stdframe+7*8($sp)");
}
	larl	("%r1",".Lconst");
	vgmg	($mask26,38,63);
	vlm	($bswaplo,$bswapmi,"16(%r1)");

	&lt	("%r0","24($ctx)");		# is_base2_26?
	jnz	(".Lskip_init");

	lg	($h0,"32($ctx)");		# load key base 2^64
	lg	($h1,"40($ctx)");

	risbg	($d0,$h0,38,0x80+63,38);	# base 2^64 -> 2^26
	srlg	($d1,$h0,52);
	risbg	($h0,$h0,38,0x80+63,0);
	vlvgg	($R0,$h0,0);
	risbg	($d1,$h1,38,51,12);
	vlvgg	($R1,$d0,0);
	risbg	($d0,$h1,38,63,50);
	vlvgg	($R2,$d1,0);
	srlg	($d1,$h1,40);
	vlvgg	($R3,$d0,0);
	vlvgg	($R4,$d1,0);

	veslg	($S1,$R1,2);
	veslg	($S2,$R2,2);
	veslg	($S3,$R3,2);
	veslg	($S4,$R4,2);
	vlr	($H0,$R0);
	vlr	($H1,$R1);
	vlr	($H2,$R2);
	vlr	($H3,$R3);
	vlr	($H4,$R4);
	vag	($S1,$S1,$R1);			# * 5
	vag	($S2,$S2,$R2);
	vag	($S3,$S3,$R3);
	vag	($S4,$S4,$R4);

	brasl	("%r14","__poly1305_mul");	# r^1:- * r^1:-

	vpdi	($R0,$H0,$R0,0);		# r^2:r^1
	vpdi	($R1,$H1,$R1,0);
	vpdi	($R2,$H2,$R2,0);
	vpdi	($R3,$H3,$R3,0);
	vpdi	($R4,$H4,$R4,0);
	vpdi	($H0,$H0,$H0,0);		# r^2:r^2
	vpdi	($H1,$H1,$H1,0);
	vpdi	($H2,$H2,$H2,0);
	vpdi	($H3,$H3,$H3,0);
	vpdi	($H4,$H4,$H4,0);
	veslg	($S1,$R1,2);
	veslg	($S2,$R2,2);
	veslg	($S3,$R3,2);
	veslg	($S4,$R4,2);
	vag	($S1,$S1,$R1);			# * 5
	vag	($S2,$S2,$R2);
	vag	($S3,$S3,$R3);
	vag	($S4,$S4,$R4);

	brasl	("%r14,__poly1305_mul");	# r^2:r^2 * r^2:r^1

	vl	($I0,"0(%r1)");			# borrow $I0
	vperm	($R0,$R0,$H0,$I0);		# r^2:r^4:r^1:r^3
	vperm	($R1,$R1,$H1,$I0);
	vperm	($R2,$R2,$H2,$I0);
	vperm	($R3,$R3,$H3,$I0);
	vperm	($R4,$R4,$H4,$I0);
	veslf	($S1,$R1,2);
	veslf	($S2,$R2,2);
	veslf	($S3,$R3,2);
	veslf	($S4,$R4,2);
	vaf	($S1,$S1,$R1);			# * 5
	vaf	($S2,$S2,$R2);
	vaf	($S3,$S3,$R3);
	vaf	($S4,$S4,$R4);

	lg	($h0,"0($ctx)");		# load hash base 2^64
	lg	($h1,"8($ctx)");
	lg	($h2,"16($ctx)");

	vzero	($H0);
	vzero	($H1);
	vzero	($H2);
	vzero	($H3);
	vzero	($H4);

	risbg	($d0,$h0,38,0x80+63,38);	# base 2^64 -> 2^26
	srlg	($d1,$h0,52);
	risbg	($h0,$h0,38,0x80+63,0);
	vlvgg	($H0,$h0,0);
	risbg	($d1,$h1,38,51,12);
	vlvgg	($H1,$d0,0);
	risbg	($d0,$h1,38,63,50);
	vlvgg	($H2,$d1,0);
	srlg	($d1,$h1,40);
	vlvgg	($H3,$d0,0);
	risbg	($d1,$h2,37,39,24);
	vlvgg	($H4,$d1,0);

	lhi	("%r0",1);
	st	("%r0","24($ctx)");		# set is_base2_26

	vstm	($R0,$S4,"48($ctx)");		# save key schedule base 2^26

	vpdi	($R0,$R0,$R0,0);		# broadcast r^2:r^4
	vpdi	($R1,$R1,$R1,0);
	vpdi	($S1,$S1,$S1,0);
	vpdi	($R2,$R2,$R2,0);
	vpdi	($S2,$S2,$S2,0);
	vpdi	($R3,$R3,$R3,0);
	vpdi	($S3,$S3,$S3,0);
	vpdi	($R4,$R4,$R4,0);
	vpdi	($S4,$S4,$S4,0);

	j	(".Loaded_hash");

ALIGN	(16);
LABEL	(".Lskip_init");
	vllezf	($H0,"0($ctx)");		# load hash base 2^26
	vllezf	($H1,"4($ctx)");
	vllezf	($H2,"8($ctx)");
	vllezf	($H3,"12($ctx)");
	vllezf	($H4,"16($ctx)");

	vlrepg	($R0,"0x30($ctx)");		# broadcast r^2:r^4
	vlrepg	($R1,"0x40($ctx)");
	vlrepg	($S1,"0x50($ctx)");
	vlrepg	($R2,"0x60($ctx)");
	vlrepg	($S2,"0x70($ctx)");
	vlrepg	($R3,"0x80($ctx)");
	vlrepg	($S3,"0x90($ctx)");
	vlrepg	($R4,"0xa0($ctx)");
	vlrepg	($S4,"0xb0($ctx)");

LABEL	(".Loaded_hash");
	vzero	($I1);
	vzero	($I3);

	vlm	($T1,$T4,"0x00($inp)");		# load first input block
	la	($inp,"0x40($inp)");
	vgmg	($mask26,6,31);
	vgmf	($I4,5,5);			# padbit<<2

	vperm	($I0,$T3,$T4,$bswaplo);
	vperm	($I2,$T3,$T4,$bswapmi);
	vperm	($T3,$T3,$T4,$bswaphi);

	verimg	($I1,$I0,$mask26,6);		# >>26
	veslg	($I0,$I0,32);
	veslg	($I2,$I2,28);			# >>4
	verimg	($I3,$T3,$mask26,18);		# >>14
	verimg	($I4,$T3,$mask26,58);		# >>38
	vn	($I0,$I0,$mask26);
	vn	($I2,$I2,$mask26);
	vesrlf	($I4,$I4,2);			# >>2

	vgmg	($mask26,38,63);
	vperm	($T3,$T1,$T2,$bswaplo);
	vperm	($T4,$T1,$T2,$bswaphi);
	vperm	($T2,$T1,$T2,$bswapmi);

	verimg	($I0,$T3,$mask26,0);
	verimg	($I1,$T3,$mask26,38);		# >>26
	verimg	($I2,$T2,$mask26,60);		# >>4
	verimg	($I3,$T4,$mask26,50);		# >>14
	vesrlg	($T4,$T4,40);
	vo	($I4,$I4,$T4);

	srlg	("%r0",$len,6);
&{$z?	\&aghi:\&ahi}	("%r0",-1);

ALIGN	(16);
LABEL	(".Loop_vx");
	vmlef		($ACC0,$I0,$R0);
	vmlef		($ACC1,$I0,$R1);
	vmlef		($ACC2,$I0,$R2);
	vmlef		($ACC3,$I0,$R3);
	vmlef		($ACC4,$I0,$R4);

	vmalef		($ACC0,$I1,$S4,$ACC0);
	vmalef		($ACC1,$I1,$R0,$ACC1);
	vmalef		($ACC2,$I1,$R1,$ACC2);
	vmalef		($ACC3,$I1,$R2,$ACC3);
	vmalef		($ACC4,$I1,$R3,$ACC4);

	 vaf		($H2,$H2,$I2);
	 vaf		($H0,$H0,$I0);
	 vaf		($H3,$H3,$I3);
	 vaf		($H1,$H1,$I1);
	 vaf		($H4,$H4,$I4);

	vmalef		($ACC0,$I2,$S3,$ACC0);
	vmalef		($ACC1,$I2,$S4,$ACC1);
	vmalef		($ACC2,$I2,$R0,$ACC2);
	vmalef		($ACC3,$I2,$R1,$ACC3);
	vmalef		($ACC4,$I2,$R2,$ACC4);

	 vlm		($T1,$T4,"0x00($inp)");	# load next input block
	 la		($inp,"0x40($inp)");
	 vgmg		($mask26,6,31);

	vmalef		($ACC0,$I3,$S2,$ACC0);
	vmalef		($ACC1,$I3,$S3,$ACC1);
	vmalef		($ACC2,$I3,$S4,$ACC2);
	vmalef		($ACC3,$I3,$R0,$ACC3);
	vmalef		($ACC4,$I3,$R1,$ACC4);

	 vperm		($I0,$T3,$T4,$bswaplo);
	 vperm		($I2,$T3,$T4,$bswapmi);
	 vperm		($T3,$T3,$T4,$bswaphi);

	vmalef		($ACC0,$I4,$S1,$ACC0);
	vmalef		($ACC1,$I4,$S2,$ACC1);
	vmalef		($ACC2,$I4,$S3,$ACC2);
	vmalef		($ACC3,$I4,$S4,$ACC3);
	vmalef		($ACC4,$I4,$R0,$ACC4);

	 verimg		($I1,$I0,$mask26,6);	# >>26
	 veslg		($I0,$I0,32);
	 veslg		($I2,$I2,28);		# >>4
	 verimg		($I3,$T3,$mask26,18);	# >>14

	vmalof		($ACC0,$H0,$R0,$ACC0);
	vmalof		($ACC1,$H0,$R1,$ACC1);
	vmalof		($ACC2,$H0,$R2,$ACC2);
	vmalof		($ACC3,$H0,$R3,$ACC3);
	vmalof		($ACC4,$H0,$R4,$ACC4);

	 vgmf		($I4,5,5);		# padbit<<2
	 verimg		($I4,$T3,$mask26,58);	# >>38
	 vn		($I0,$I0,$mask26);
	 vn		($I2,$I2,$mask26);
	 vesrlf		($I4,$I4,2);		# >>2

	vmalof		($ACC0,$H1,$S4,$ACC0);
	vmalof		($ACC1,$H1,$R0,$ACC1);
	vmalof		($ACC2,$H1,$R1,$ACC2);
	vmalof		($ACC3,$H1,$R2,$ACC3);
	vmalof		($ACC4,$H1,$R3,$ACC4);

	 vgmg		($mask26,38,63);
	 vperm		($T3,$T1,$T2,$bswaplo);
	 vperm		($T4,$T1,$T2,$bswaphi);
	 vperm		($T2,$T1,$T2,$bswapmi);

	vmalof		($ACC0,$H2,$S3,$ACC0);
	vmalof		($ACC1,$H2,$S4,$ACC1);
	vmalof		($ACC2,$H2,$R0,$ACC2);
	vmalof		($ACC3,$H2,$R1,$ACC3);
	vmalof		($ACC4,$H2,$R2,$ACC4);

	 verimg		($I0,$T3,$mask26,0);
	 verimg		($I1,$T3,$mask26,38);	# >>26
	 verimg		($I2,$T2,$mask26,60);	# >>4

	vmalof		($ACC0,$H3,$S2,$ACC0);
	vmalof		($ACC1,$H3,$S3,$ACC1);
	vmalof		($ACC2,$H3,$S4,$ACC2);
	vmalof		($ACC3,$H3,$R0,$ACC3);
	vmalof		($ACC4,$H3,$R1,$ACC4);

	 verimg		($I3,$T4,$mask26,50);	# >>14
	 vesrlg		($T4,$T4,40);
	 vo		($I4,$I4,$T4);

	vmalof		($ACC0,$H4,$S1,$ACC0);
	vmalof		($ACC1,$H4,$S2,$ACC1);
	vmalof		($ACC2,$H4,$S3,$ACC2);
	vmalof		($ACC3,$H4,$S4,$ACC3);
	vmalof		($ACC4,$H4,$R0,$ACC4);

	################################################################
	# lazy reduction as discussed in "NEON crypto" by D.J. Bernstein
	# and P. Schwabe

	vesrlg		($H4,$ACC3,26);
	vesrlg		($H1,$ACC0,26);
	vn		($H3,$ACC3,$mask26);
	vn		($H0,$ACC0,$mask26);
	vag		($H4,$H4,$ACC4);	# h3 -> h4
	vag		($H1,$H1,$ACC1);	# h0 -> h1

	vesrlg		($ACC4,$H4,26);
	vesrlg		($ACC1,$H1,26);
	vn		($H4,$H4,$mask26);
	vn		($H1,$H1,$mask26);
	vag		($H0,$H0,$ACC4);
	vag		($H2,$ACC2,$ACC1);	# h1 -> h2

	veslg		($ACC4,$ACC4,2);	# <<2
	vesrlg		($ACC2,$H2,26);
	vn		($H2,$H2,$mask26);
	vag		($H0,$H0,$ACC4);	# h4 -> h0
	vag		($H3,$H3,$ACC2);	# h2 -> h3

	vesrlg		($ACC0,$H0,26);
	vesrlg		($ACC3,$H3,26);
	vn		($H0,$H0,$mask26);
	vn		($H3,$H3,$mask26);
	vag		($H1,$H1,$ACC0);	# h0 -> h1
	vag		($H4,$H4,$ACC3);	# h3 -> h4

&{$z?	\&brctg:\&brct}	("%r0",".Loop_vx");

	vlm	($R0,$S4,"48($ctx)");		# load all powers

	lghi	("%r0",0x30);
&{$z?	\&lcgr:\&lcr}	($len,$len);
&{$z?	\&ngr:\&nr}	($len,"%r0");
&{$z?	\&slgr:\&slr}	($inp,$len);

LABEL	(".Last");
	vmlef	($ACC0,$I0,$R0);
	vmlef	($ACC1,$I0,$R1);
	vmlef	($ACC2,$I0,$R2);
	vmlef	($ACC3,$I0,$R3);
	vmlef	($ACC4,$I0,$R4);

	vmalef	($ACC0,$I1,$S4,$ACC0);
	vmalef	($ACC1,$I1,$R0,$ACC1);
	vmalef	($ACC2,$I1,$R1,$ACC2);
	vmalef	($ACC3,$I1,$R2,$ACC3);
	vmalef	($ACC4,$I1,$R3,$ACC4);

	 vaf	($H0,$H0,$I0);
	 vaf	($H1,$H1,$I1);
	 vaf	($H2,$H2,$I2);
	 vaf	($H3,$H3,$I3);
	 vaf	($H4,$H4,$I4);

	vmalef	($ACC0,$I2,$S3,$ACC0);
	vmalef	($ACC1,$I2,$S4,$ACC1);
	vmalef	($ACC2,$I2,$R0,$ACC2);
	vmalef	($ACC3,$I2,$R1,$ACC3);
	vmalef	($ACC4,$I2,$R2,$ACC4);

	vmalef	($ACC0,$I3,$S2,$ACC0);
	vmalef	($ACC1,$I3,$S3,$ACC1);
	vmalef	($ACC2,$I3,$S4,$ACC2);
	vmalef	($ACC3,$I3,$R0,$ACC3);
	vmalef	($ACC4,$I3,$R1,$ACC4);

	vmalef	($ACC0,$I4,$S1,$ACC0);
	vmalef	($ACC1,$I4,$S2,$ACC1);
	vmalef	($ACC2,$I4,$S3,$ACC2);
	vmalef	($ACC3,$I4,$S4,$ACC3);
	vmalef	($ACC4,$I4,$R0,$ACC4);

	vmalof	($ACC0,$H0,$R0,$ACC0);
	vmalof	($ACC1,$H0,$R1,$ACC1);
	vmalof	($ACC2,$H0,$R2,$ACC2);
	vmalof	($ACC3,$H0,$R3,$ACC3);
	vmalof	($ACC4,$H0,$R4,$ACC4);

	vmalof	($ACC0,$H1,$S4,$ACC0);
	vmalof	($ACC1,$H1,$R0,$ACC1);
	vmalof	($ACC2,$H1,$R1,$ACC2);
	vmalof	($ACC3,$H1,$R2,$ACC3);
	vmalof	($ACC4,$H1,$R3,$ACC4);

	vmalof	($ACC0,$H2,$S3,$ACC0);
	vmalof	($ACC1,$H2,$S4,$ACC1);
	vmalof	($ACC2,$H2,$R0,$ACC2);
	vmalof	($ACC3,$H2,$R1,$ACC3);
	vmalof	($ACC4,$H2,$R2,$ACC4);

	vmalof	($ACC0,$H3,$S2,$ACC0);
	vmalof	($ACC1,$H3,$S3,$ACC1);
	vmalof	($ACC2,$H3,$S4,$ACC2);
	vmalof	($ACC3,$H3,$R0,$ACC3);
	vmalof	($ACC4,$H3,$R1,$ACC4);

	vmalof	($ACC0,$H4,$S1,$ACC0);
	vmalof	($ACC1,$H4,$S2,$ACC1);
	vmalof	($ACC2,$H4,$S3,$ACC2);
	vmalof	($ACC3,$H4,$S4,$ACC3);
	vmalof	($ACC4,$H4,$R0,$ACC4);

	################################################################
	# horizontal addition

	vzero	($H0);
	vsumqg	($ACC0,$ACC0,$H0);
	vsumqg	($ACC1,$ACC1,$H0);
	vsumqg	($ACC2,$ACC2,$H0);
	vsumqg	($ACC3,$ACC3,$H0);
	vsumqg	($ACC4,$ACC4,$H0);

	################################################################
	# lazy reduction

	vesrlg	($H4,$ACC3,26);
	vesrlg	($H1,$ACC0,26);
	vn	($H3,$ACC3,$mask26);
	vn	($H0,$ACC0,$mask26);
	vag	($H4,$H4,$ACC4);		# h3 -> h4
	vag	($H1,$H1,$ACC1);		# h0 -> h1

	vesrlg	($ACC4,$H4,26);
	vesrlg	($ACC1,$H1,26);
	vn	($H4,$H4,$mask26);
	vn	($H1,$H1,$mask26);
	vag	($H0,$H0,$ACC4);
	vag	($H2,$ACC2,$ACC1);		# h1 -> h2

	veslg	($ACC4,$ACC4,2);		# <<2
	vesrlg	($ACC2,$H2,26);
	vn	($H2,$H2,$mask26);
	vag	($H0,$H0,$ACC4);		# h4 -> h0
	vag	($H3,$H3,$ACC2);		# h2 -> h3

	vesrlg	($ACC0,$H0,26);
	vesrlg	($ACC3,$H3,26);
	vn	($H0,$H0,$mask26);
	vn	($H3,$H3,$mask26);
	vag	($H1,$H1,$ACC0);		# h0 -> h1
	vag	($H4,$H4,$ACC3);		# h3 -> h4

&{$z?	\&clgfi:\&clfi} ($len,0);
	je	(".Ldone");

	vlm	($T1,$T4,"0x00($inp)");		# load last partial block
	vgmg	($mask26,6,31);
	vgmf	($I4,5,5);			# padbit<<2

	vperm	($I0,$T3,$T4,$bswaplo);
	vperm	($I2,$T3,$T4,$bswapmi);
	vperm	($T3,$T3,$T4,$bswaphi);

	vl	($ACC0,"0x30($len,%r1)");	# borrow $ACC0,1
	vl	($ACC1,"0x60($len,%r1)");

	verimg	($I1,$I0,$mask26,6);		# >>26
	veslg	($I0,$I0,32);
	veslg	($I2,$I2,28);			# >>4
	verimg	($I3,$T3,$mask26,18);		# >>14
	verimg	($I4,$T3,$mask26,58);		# >>38
	vn	($I0,$I0,$mask26);
	vn	($I2,$I2,$mask26);
	vesrlf	($I4,$I4,2);			# >>2

	vgmg	($mask26,38,63);
	vperm	($T3,$T1,$T2,$bswaplo);
	vperm	($T4,$T1,$T2,$bswaphi);
	vperm	($T2,$T1,$T2,$bswapmi);

	verimg	($I0,$T3,$mask26,0);
	verimg	($I1,$T3,$mask26,38);		# >>26
	verimg	($I2,$T2,$mask26,60);		# >>4
	verimg	($I3,$T4,$mask26,50);		# >>14
	vesrlg	($T4,$T4,40);
	vo	($I4,$I4,$T4);

	vperm	($H0,$H0,$H0,$ACC0);		# move hash to right lane
	vn	($I0,$I0,$ACC1);		# mask redundant lane[s]
	vperm	($H1,$H1,$H1,$ACC0);
	vn	($I1,$I1,$ACC1);
	vperm	($H2,$H2,$H2,$ACC0);
	vn	($I2,$I2,$ACC1);
	vperm	($H3,$H3,$H3,$ACC0);
	vn	($I3,$I3,$ACC1);
	vperm	($H4,$H4,$H4,$ACC0);
	vn	($I4,$I4,$ACC1);

	vaf	($I0,$I0,$H0);			# accumulate hash
	vzero	($H0);				# wipe hash value
	vaf	($I1,$I1,$H1);
	vzero	($H1);
	vaf	($I2,$I2,$H2);
	vzero	($H2);
	vaf	($I3,$I3,$H3);
	vzero	($H3);
	vaf	($I4,$I4,$H4);
	vzero	($H4);

&{$z?	\&lghi:\&lhi}	($len,0);
	j	(".Last");
	# I don't bother to tell apart cases when only one multiplication
	# pass is sufficient, because I argue that mispredicted branch
	# penalties are comparable to overhead of sometimes redundant
	# multiplication pass...

LABEL	(".Ldone");
	vstef	($H0,"0($ctx)",3);		# store hash base 2^26
	vstef	($H1,"4($ctx)",3);
	vstef	($H2,"8($ctx)",3);
	vstef	($H3,"12($ctx)",3);
	vstef	($H4,"16($ctx)",3);

if ($z) {
	ld	("%f8","$stdframe+0*8($sp)");
	ld	("%f9","$stdframe+1*8($sp)");
	ld	("%f10","$stdframe+2*8($sp)");
	ld	("%f11","$stdframe+3*8($sp)");
	ld	("%f12","$stdframe+4*8($sp)");
	ld	("%f13","$stdframe+5*8($sp)");
	ld	("%f14","$stdframe+6*8($sp)");
	ld	("%f15","$stdframe+7*8($sp)");
&{$z?	\&lmg:\&lm}	("%r10","%r15","$stdframe+8*8+10*$SIZE_T($sp)");
} else {
	ld	("%f4","$stdframe+16*$SIZE_T+2*8($sp)");
	ld	("%f6","$stdframe+16*$SIZE_T+3*8($sp)");
&{$z?	\&lmg:\&lm}	("%r10","%r15","$stdframe+10*$SIZE_T($sp)");
}
	br	("%r14");
SIZE	("__poly1305_blocks_vx",".-__poly1305_blocks_vx");
}

################
# static void poly1305_emit(void *ctx, unsigned char mac[16],
#                           const u32 nonce[4])
{
my ($mac,$nonce)=($inp,$len);
my ($h0,$h1,$h2,$d0,$d1,$d2)=map("%r$_",(5..10));

GLOBL	("poly1305_emit");
TYPE	("poly1305_emit","\@function");
ALIGN	(16);
LABEL	("poly1305_emit");
LABEL	(".Lpoly1305_emit");
&{$z?	\&stmg:\&stm}	("%r6","%r10","6*$SIZE_T($sp)");

	lg	($d0,"0($ctx)");
	lg	($d1,"8($ctx)");
	lg	($d2,"16($ctx)");

	llgfr	("%r0",$d0);			# base 2^26 -> base 2^64
	srlg	($h0,$d0,32);
	llgfr	("%r1",$d1);
	srlg	($h1,$d1,32);
	srlg	($h2,$d2,32);

	sllg	("%r0","%r0",26);
	algr	($h0,"%r0");
	sllg	("%r0",$h1,52);
	srlg	($h1,$h1,12);
	sllg	("%r1","%r1",14);
	algr	($h0,"%r0");
	alcgr	($h1,"%r1");
	sllg	("%r0",$h2,40);
	srlg	($h2,$h2,24);
	lghi	("%r1",0);
	algr	($h1,"%r0");
	alcgr	($h2,"%r1");

	llgf	("%r0","24($ctx)");		# is_base2_26
	lcgr	("%r0","%r0");

	xgr	($h0,$d0);			# choose between radixes
	xgr	($h1,$d1);
	xgr	($h2,$d2);
	ngr	($h0,"%r0");
	ngr	($h1,"%r0");
	ngr	($h2,"%r0");
	xgr	($h0,$d0);
	xgr	($h1,$d1);
	xgr	($h2,$d2);

	lghi	("%r0",5);
	lgr	($d0,$h0);
	lgr	($d1,$h1);

	algr	($h0,"%r0");			# compare to modulus
	alcgr	($h1,"%r1");
	alcgr	($h2,"%r1");

	srlg	($h2,$h2,2);			# did it borrow/carry?
	slgr	("%r1",$h2);				# 0-$h2>>2
	lg	($d2,"0($nonce)");		# load nonce
	lg	($ctx,"8($nonce)");

	xgr	($h0,$d0);
	xgr	($h1,$d1);
	ngr	($h0,"%r1");
	ngr	($h1,"%r1");
	xgr	($h0,$d0);
	rllg	($d0,$d2,32);			# flip nonce words
	xgr	($h1,$d1);
	rllg	($d1,$ctx,32);

	algr	($h0,$d0);			# accumulate nonce
	alcgr	($h1,$d1);

	strvg	($h0,"0($mac)");		# write little-endian result
	strvg	($h1,"8($mac)");

&{$z?	\&lmg:\&lm}	("%r6","%r10","6*$SIZE_T($sp)");
	br	("%r14");
SIZE	("poly1305_emit",".-poly1305_emit");
}

################

ALIGN	(16);
LABEL	(".Lconst");
LONG	(0x04050607,0x14151617,0x0c0d0e0f,0x1c1d1e1f);	# merge odd
LONG	(0x07060504,0x03020100,0x17161514,0x13121110);	# byte swap masks
LONG	(0x0f0e0d0c,0x0b0a0908,0x1f1e1d1c,0x1b1a1918);
LONG	(0x00000000,0x09080706,0x00000000,0x19181716);

LONG	(0x00000000,0x00000000,0x00000000,0x0c0d0e0f);	# magic tail masks
LONG	(0x0c0d0e0f,0x00000000,0x00000000,0x00000000);
LONG	(0x00000000,0x00000000,0x0c0d0e0f,0x00000000);

LONG	(0xffffffff,0x00000000,0xffffffff,0xffffffff);
LONG	(0xffffffff,0x00000000,0xffffffff,0x00000000);
LONG	(0x00000000,0x00000000,0xffffffff,0x00000000);

STRING	("\"Poly1305 for s390x, CRYPTOGAMS by <appro\@openssl.org>\"");

PERLASM_END();
