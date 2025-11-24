#! /usr/bin/env perl
# Copyright 2022-2025  The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
#
# ChaCha20 for ARMv8 via SVE
#
# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

sub AUTOLOAD()		# thunk [simplified] x86-style perlasm
{ my $opcode = $AUTOLOAD; $opcode =~ s/.*:://; $opcode =~ s/_/\./;
  my $arg = pop;
    $arg = "#$arg" if ($arg*1 eq $arg);
    $code .= "\t$opcode\t".join(',',@_,$arg)."\n";
}

$prefix="chacha_sve";
my ($outp,$inp,$len,$key,$ctr) = map("x$_",(0..4));
my ($veclen) = ("x5");
my ($counter) = ("x6");
my ($counter_w) = ("w6");
my @xx=(7..22);
my @sxx=map("x$_",@xx);
my @sx=map("w$_",@xx);
my @K=map("x$_",(23..30));
my @elem=(0,4,8,12,1,5,9,13,2,6,10,14,3,7,11,15);
my @KL=map("w$_",(23..30));
my @mx=map("z$_",@elem);
my @vx=map("v$_",@elem);
my ($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
    $xc0,$xc1,$xc2,$xc3, $xd0,$xd1,$xd2,$xd3) = @mx;
my ($zctr) = ("z16");
my @tt=(17..24);
my @xt=map("z$_",@tt);
my @vt=map("v$_",@tt);
my @perm=map("z$_",(25..30));
my ($rot8) = ("z31");
my @bak=(@perm[0],@perm[1],@perm[2],@perm[3],@perm[4],@perm[5],@xt[4],@xt[5],@xt[6],@xt[7],@xt[0],@xt[1],$zctr,@xt[2],@xt[3],$rot8);
my $debug_encoder=0;

sub SVE_ADD() {
	my $x = shift;
	my $y = shift;

$code.=<<___;
	add	@mx[$x].s,@mx[$x].s,@mx[$y].s
	.if mixin == 1
		add	@sx[$x],@sx[$x],@sx[$y]
	.endif
___
	if (@_) {
		&SVE_ADD(@_);
	}
}

sub SVE_EOR() {
	my $x = shift;
	my $y = shift;

$code.=<<___;
	eor	@mx[$x].d,@mx[$x].d,@mx[$y].d
	.if mixin == 1
		eor	@sx[$x],@sx[$x],@sx[$y]
	.endif
___
	if (@_) {
		&SVE_EOR(@_);
	}
}

sub SVE_LSL() {
	my $bits = shift;
	my $x = shift;
	my $y = shift;
	my $next = $x + 1;

$code.=<<___;
	lsl	@xt[$x].s,@mx[$y].s,$bits
___
	if (@_) {
		&SVE_LSL($bits,$next,@_);
	}
}

sub SVE_LSR() {
	my $bits = shift;
	my $x = shift;

$code.=<<___;
	lsr	@mx[$x].s,@mx[$x].s,$bits
	.if mixin == 1
		ror	@sx[$x],@sx[$x],$bits
	.endif
___
	if (@_) {
		&SVE_LSR($bits,@_);
	}
}

sub SVE_ORR() {
	my $x = shift;
	my $y = shift;
	my $next = $x + 1;

$code.=<<___;
	orr	@mx[$y].d,@mx[$y].d,@xt[$x].d
___
	if (@_) {
		&SVE_ORR($next,@_);
	}
}

sub SVE_REV16() {
	my $x = shift;

$code.=<<___;
	revh	@mx[$x].s,p0/m,@mx[$x].s
	.if mixin == 1
		ror	@sx[$x],@sx[$x],#16
	.endif
___
	if (@_) {
		&SVE_REV16(@_);
	}
}

sub SVE_ROT8() {
	my $x = shift;

$code.=<<___;
	tbl	@mx[$x].b,{@mx[$x].b},$rot8.b
	.if mixin == 1
		ror	@sx[$x],@sx[$x],#24
	.endif
___
	if (@_) {
		&SVE_ROT8(@_);
	}
}

sub SVE2_XAR() {
	my $bits = shift;
	my $x = shift;
	my $y = shift;
	my $rbits = 32-$bits;

$code.=<<___;
	.if mixin == 1
		eor	@sx[$x],@sx[$x],@sx[$y]
	.endif
	xar	@mx[$x].s,@mx[$x].s,@mx[$y].s,$rbits
	.if mixin == 1
		ror	@sx[$x],@sx[$x],$rbits
	.endif
___
	if (@_) {
		&SVE2_XAR($bits,@_);
	}
}

sub SVE2_QR_GROUP() {
	my ($a0,$b0,$c0,$d0,$a1,$b1,$c1,$d1,$a2,$b2,$c2,$d2,$a3,$b3,$c3,$d3) = @_;

	&SVE_ADD($a0,$b0,$a1,$b1,$a2,$b2,$a3,$b3);
	&SVE2_XAR(16,$d0,$a0,$d1,$a1,$d2,$a2,$d3,$a3);

	&SVE_ADD($c0,$d0,$c1,$d1,$c2,$d2,$c3,$d3);
	&SVE2_XAR(12,$b0,$c0,$b1,$c1,$b2,$c2,$b3,$c3);

	&SVE_ADD($a0,$b0,$a1,$b1,$a2,$b2,$a3,$b3);
	&SVE2_XAR(8,$d0,$a0,$d1,$a1,$d2,$a2,$d3,$a3);

	&SVE_ADD($c0,$d0,$c1,$d1,$c2,$d2,$c3,$d3);
	&SVE2_XAR(7,$b0,$c0,$b1,$c1,$b2,$c2,$b3,$c3);
}

sub SVE_QR_GROUP() {
	my ($a0,$b0,$c0,$d0,$a1,$b1,$c1,$d1,$a2,$b2,$c2,$d2,$a3,$b3,$c3,$d3) = @_;

	&SVE_ADD($a0,$b0,$a1,$b1,$a2,$b2,$a3,$b3);
	&SVE_EOR($d0,$a0,$d1,$a1,$d2,$a2,$d3,$a3);
	&SVE_REV16($d0,$d1,$d2,$d3);

	&SVE_ADD($c0,$d0,$c1,$d1,$c2,$d2,$c3,$d3);
	&SVE_EOR($b0,$c0,$b1,$c1,$b2,$c2,$b3,$c3);
	&SVE_LSL(12,0,$b0,$b1,$b2,$b3);
	&SVE_LSR(20,$b0,$b1,$b2,$b3);
	&SVE_ORR(0,$b0,$b1,$b2,$b3);

	&SVE_ADD($a0,$b0,$a1,$b1,$a2,$b2,$a3,$b3);
	&SVE_EOR($d0,$a0,$d1,$a1,$d2,$a2,$d3,$a3);
	&SVE_ROT8($d0,$d1,$d2,$d3);

	&SVE_ADD($c0,$d0,$c1,$d1,$c2,$d2,$c3,$d3);
	&SVE_EOR($b0,$c0,$b1,$c1,$b2,$c2,$b3,$c3);
	&SVE_LSL(7,0,$b0,$b1,$b2,$b3);
	&SVE_LSR(25,$b0,$b1,$b2,$b3);
	&SVE_ORR(0,$b0,$b1,$b2,$b3);
}

sub SVE_INNER_BLOCK() {
$code.=<<___;
	mov	$counter,#10
10:
.align	5
___
	&SVE_QR_GROUP(0,4,8,12,1,5,9,13,2,6,10,14,3,7,11,15);
	&SVE_QR_GROUP(0,5,10,15,1,6,11,12,2,7,8,13,3,4,9,14);
$code.=<<___;
	sub	$counter,$counter,1
	cbnz	$counter,10b
___
}

sub SVE2_INNER_BLOCK() {
$code.=<<___;
	mov	$counter,#10
10:
.align	5
___
	&SVE2_QR_GROUP(0,4,8,12,1,5,9,13,2,6,10,14,3,7,11,15);
	&SVE2_QR_GROUP(0,5,10,15,1,6,11,12,2,7,8,13,3,4,9,14);
$code.=<<___;
	sub	$counter,$counter,1
	cbnz	$counter,10b
___
}

sub load_regs() {
	my $offset = shift;
	my $reg = shift;
	my $next_offset = $offset + 1;
$code.=<<___;
	ld1w	{$reg.s},p0/z,[$inp,#$offset,MUL VL]
___
	if (@_) {
		&load_regs($next_offset, @_);
	} else {
$code.=<<___;
	addvl	$inp,$inp,$next_offset
___
	}
}

sub load() {
	if (@_) {
		&load_regs(0, @_);
	}
}

sub store_regs() {
	my $offset = shift;
	my $reg = shift;
	my $next_offset = $offset + 1;
$code.=<<___;
	st1w	{$reg.s},p0,[$outp,#$offset,MUL VL]
___
	if (@_) {
		&store_regs($next_offset, @_);
	} else {
$code.=<<___;
	addvl	$outp,$outp,$next_offset
___
	}
}

sub store() {
	if (@_) {
		&store_regs(0, @_);
	}
}

sub transpose() {
	my $xa = shift;
	my $xb = shift;
	my $xc = shift;
	my $xd = shift;
	my $xa1 = shift;
	my $xb1 = shift;
	my $xc1 = shift;
	my $xd1 = shift;
$code.=<<___;
	zip1	@xt[0].s,$xa.s,$xb.s
	zip2	@xt[1].s,$xa.s,$xb.s
	zip1	@xt[2].s,$xc.s,$xd.s
	zip2	@xt[3].s,$xc.s,$xd.s

	zip1	@xt[4].s,$xa1.s,$xb1.s
	zip2	@xt[5].s,$xa1.s,$xb1.s
	zip1	@xt[6].s,$xc1.s,$xd1.s
	zip2	@xt[7].s,$xc1.s,$xd1.s

	zip1	$xa.d,@xt[0].d,@xt[2].d
	zip2	$xb.d,@xt[0].d,@xt[2].d
	zip1	$xc.d,@xt[1].d,@xt[3].d
	zip2	$xd.d,@xt[1].d,@xt[3].d

	zip1	$xa1.d,@xt[4].d,@xt[6].d
	zip2	$xb1.d,@xt[4].d,@xt[6].d
	zip1	$xc1.d,@xt[5].d,@xt[7].d
	zip2	$xd1.d,@xt[5].d,@xt[7].d
___
}

sub ACCUM() {
	my $idx0 = shift;
	my $idx1 = $idx0 + 1;
	my $x0 = @sx[$idx0];
	my $xx0 = @sxx[$idx0];
	my $x1 = @sx[$idx1];
	my $xx1 = @sxx[$idx1];
	my $d = $idx0/2;
	my ($tmp,$tmpw) = ($counter,$counter_w);
	my $bk0 = @_ ? shift : @bak[$idx0];
	my $bk1 = @_ ? shift : @bak[$idx1];

$code.=<<___;
	.if mixin == 1
		add	@sx[$idx0],@sx[$idx0],@KL[$d]
	.endif
	add	@mx[$idx0].s,@mx[$idx0].s,$bk0.s
	.if mixin == 1
		add	@sxx[$idx1],@sxx[$idx1],@K[$d],lsr #32
	.endif
	add	@mx[$idx1].s,@mx[$idx1].s,$bk1.s
	.if mixin == 1
		add	@sxx[$idx0],@sxx[$idx0],$sxx[$idx1],lsl #32  // pack
	.endif
___
}

sub SCA_INP() {
	my $idx0 = shift;
	my $idx1 = $idx0 + 2;
$code.=<<___;
	.if mixin == 1
		ldp	@sxx[$idx0],@sxx[$idx1],[$inp],#16
	.endif
___
}

sub SVE_ACCUM_STATES() {
	my ($tmp,$tmpw) = ($counter,$counter_w);

$code.=<<___;
	lsr	$tmp,@K[5],#32
	dup	@bak[10].s,@KL[5]
	dup	@bak[11].s,$tmpw
	lsr	$tmp,@K[6],#32
	dup	@bak[13].s,$tmpw
	lsr	$tmp,@K[7],#32
___
	&ACCUM(0);
	&ACCUM(2);
	&SCA_INP(1);
	&ACCUM(4);
	&ACCUM(6);
	&SCA_INP(5);
	&ACCUM(8);
	&ACCUM(10);
	&SCA_INP(9);
$code.=<<___;
	dup	@bak[14].s,@KL[7]
	dup	@bak[0].s,$tmpw	// bak[15] not available for SVE
___
	&ACCUM(12);
	&ACCUM(14, @bak[14],@bak[0]);
	&SCA_INP(13);
}

sub SVE2_ACCUM_STATES() {
	&ACCUM(0);
	&ACCUM(2);
	&SCA_INP(1);
	&ACCUM(4);
	&ACCUM(6);
	&SCA_INP(5);
	&ACCUM(8);
	&ACCUM(10);
	&SCA_INP(9);
	&ACCUM(12);
	&ACCUM(14);
	&SCA_INP(13);
}

sub SCA_EOR() {
	my $idx0 = shift;
	my $idx1 = $idx0 + 1;
$code.=<<___;
	.if mixin == 1
		eor	@sxx[$idx0],@sxx[$idx0],@sxx[$idx1]
	.endif
___
}

sub SCA_SAVE() {
	my $idx0 = shift;
	my $idx1 = shift;
$code.=<<___;
	.if mixin == 1
		stp	@sxx[$idx0],@sxx[$idx1],[$outp],#16
	.endif
___
}

sub SVE_VL128_TRANSFORMS() {
	&SCA_EOR(0);
	&SCA_EOR(2);
	&SCA_EOR(4);
	&transpose($xa0,$xa1,$xa2,$xa3,$xb0,$xb1,$xb2,$xb3);
	&SCA_EOR(6);
	&SCA_EOR(8);
	&SCA_EOR(10);
	&transpose($xc0,$xc1,$xc2,$xc3,$xd0,$xd1,$xd2,$xd3);
	&SCA_EOR(12);
	&SCA_EOR(14);
$code.=<<___;
	ld1	{@vt[0].4s-@vt[3].4s},[$inp],#64
	ld1	{@vt[4].4s-@vt[7].4s},[$inp],#64
	eor	$xa0.d,$xa0.d,@xt[0].d
	eor	$xb0.d,$xb0.d,@xt[1].d
	eor	$xc0.d,$xc0.d,@xt[2].d
	eor	$xd0.d,$xd0.d,@xt[3].d
	eor	$xa1.d,$xa1.d,@xt[4].d
	eor	$xb1.d,$xb1.d,@xt[5].d
	eor	$xc1.d,$xc1.d,@xt[6].d
	eor	$xd1.d,$xd1.d,@xt[7].d
	ld1	{@vt[0].4s-@vt[3].4s},[$inp],#64
	ld1	{@vt[4].4s-@vt[7].4s},[$inp],#64
___
	&SCA_SAVE(0,2);
$code.=<<___;
	eor	$xa2.d,$xa2.d,@xt[0].d
	eor	$xb2.d,$xb2.d,@xt[1].d
___
	&SCA_SAVE(4,6);
$code.=<<___;
	eor	$xc2.d,$xc2.d,@xt[2].d
	eor	$xd2.d,$xd2.d,@xt[3].d
___
	&SCA_SAVE(8,10);
$code.=<<___;
	eor	$xa3.d,$xa3.d,@xt[4].d
	eor	$xb3.d,$xb3.d,@xt[5].d
___
	&SCA_SAVE(12,14);
$code.=<<___;
	eor	$xc3.d,$xc3.d,@xt[6].d
	eor	$xd3.d,$xd3.d,@xt[7].d
	st1	{@vx[0].4s-@vx[12].4s},[$outp],#64
	st1	{@vx[1].4s-@vx[13].4s},[$outp],#64
	st1	{@vx[2].4s-@vx[14].4s},[$outp],#64
	st1	{@vx[3].4s-@vx[15].4s},[$outp],#64
___
}

sub SVE_TRANSFORMS() {
$code.=<<___;
#ifdef	__AARCH64EB__
	rev	@sxx[0],@sxx[0]
	revb	@mx[0].s,p0/m,@mx[0].s
	revb	@mx[1].s,p0/m,@mx[1].s
	rev	@sxx[2],@sxx[2]
	revb	@mx[2].s,p0/m,@mx[2].s
	revb	@mx[3].s,p0/m,@mx[3].s
	rev	@sxx[4],@sxx[4]
	revb	@mx[4].s,p0/m,@mx[4].s
	revb	@mx[5].s,p0/m,@mx[5].s
	rev	@sxx[6],@sxx[6]
	revb	@mx[6].s,p0/m,@mx[6].s
	revb	@mx[7].s,p0/m,@mx[7].s
	rev	@sxx[8],@sxx[8]
	revb	@mx[8].s,p0/m,@mx[8].s
	revb	@mx[9].s,p0/m,@mx[9].s
	rev	@sxx[10],@sxx[10]
	revb	@mx[10].s,p0/m,@mx[10].s
	revb	@mx[11].s,p0/m,@mx[11].s
	rev	@sxx[12],@sxx[12]
	revb	@mx[12].s,p0/m,@mx[12].s
	revb	@mx[13].s,p0/m,@mx[13].s
	rev	@sxx[14],@sxx[14]
	revb	@mx[14].s,p0/m,@mx[14].s
	revb	@mx[15].s,p0/m,@mx[15].s
#endif
	.if mixin == 1
		add	@K[6],@K[6],#1
	.endif
	cmp	$veclen,4
	b.ne	200f
___
	&SVE_VL128_TRANSFORMS();
$code.=<<___;
	b	210f
200:
___
	&transpose($xa0,$xb0,$xc0,$xd0,$xa1,$xb1,$xc1,$xd1);
	&SCA_EOR(0);
	&SCA_EOR(2);
	&transpose($xa2,$xb2,$xc2,$xd2,$xa3,$xb3,$xc3,$xd3);
	&SCA_EOR(4);
	&SCA_EOR(6);
	&transpose($xa0,$xa1,$xa2,$xa3,$xb0,$xb1,$xb2,$xb3);
	&SCA_EOR(8);
	&SCA_EOR(10);
	&transpose($xc0,$xc1,$xc2,$xc3,$xd0,$xd1,$xd2,$xd3);
	&SCA_EOR(12);
	&SCA_EOR(14);
	&load(@xt[0],@xt[1],@xt[2],@xt[3],@xt[4],@xt[5],@xt[6],@xt[7]);
$code.=<<___;
	eor	$xa0.d,$xa0.d,@xt[0].d
	eor	$xa1.d,$xa1.d,@xt[1].d
	eor	$xa2.d,$xa2.d,@xt[2].d
	eor	$xa3.d,$xa3.d,@xt[3].d
	eor	$xb0.d,$xb0.d,@xt[4].d
	eor	$xb1.d,$xb1.d,@xt[5].d
	eor	$xb2.d,$xb2.d,@xt[6].d
	eor	$xb3.d,$xb3.d,@xt[7].d
___
	&load(@xt[0],@xt[1],@xt[2],@xt[3],@xt[4],@xt[5],@xt[6],@xt[7]);
	&SCA_SAVE(0,2);
$code.=<<___;
	eor	$xc0.d,$xc0.d,@xt[0].d
	eor	$xc1.d,$xc1.d,@xt[1].d
___
	&SCA_SAVE(4,6);
$code.=<<___;
	eor	$xc2.d,$xc2.d,@xt[2].d
	eor	$xc3.d,$xc3.d,@xt[3].d
___
	&SCA_SAVE(8,10);
$code.=<<___;
	eor	$xd0.d,$xd0.d,@xt[4].d
	eor	$xd1.d,$xd1.d,@xt[5].d
___
	&SCA_SAVE(12,14);
$code.=<<___;
	eor	$xd2.d,$xd2.d,@xt[6].d
	eor	$xd3.d,$xd3.d,@xt[7].d
___
	&store($xa0,$xa1,$xa2,$xa3,$xb0,$xb1,$xb2,$xb3);
	&store($xc0,$xc1,$xc2,$xc3,$xd0,$xd1,$xd2,$xd3);
$code.=<<___;
210:
	incw	@K[6], ALL, MUL #1
___
}

sub SET_STATE_BAK() {
	my $idx0 = shift;
	my $idx1 = $idx0 + 1;
	my $x0 = @sx[$idx0];
	my $xx0 = @sxx[$idx0];
	my $x1 = @sx[$idx1];
	my $xx1 = @sxx[$idx1];
	my $d = $idx0/2;

$code.=<<___;
	lsr	$xx1,@K[$d],#32
	dup	@mx[$idx0].s,@KL[$d]
	dup	@bak[$idx0].s,@KL[$d]
	.if mixin == 1
		mov	$x0,@KL[$d]
	.endif
	dup	@mx[$idx1].s,$x1
	dup	@bak[$idx1].s,$x1
___
}

sub SET_STATE() {
	my $idx0 = shift;
	my $idx1 = $idx0 + 1;
	my $x0 = @sx[$idx0];
	my $xx0 = @sxx[$idx0];
	my $x1 = @sx[$idx1];
	my $xx1 = @sxx[$idx1];
	my $d = $idx0/2;

$code.=<<___;
	lsr	$xx1,@K[$d],#32
	dup	@mx[$idx0].s,@KL[$d]
	.if mixin == 1
		mov	$x0,@KL[$d]
	.endif
	dup	@mx[$idx1].s,$x1
___
}

sub SVE_LOAD_STATES() {
	&SET_STATE_BAK(0);
	&SET_STATE_BAK(2);
	&SET_STATE_BAK(4);
	&SET_STATE_BAK(6);
	&SET_STATE_BAK(8);
	&SET_STATE(10);
	&SET_STATE(14);
$code.=<<___;
	.if mixin == 1
		add	@sx[13],@KL[6],#1
		mov	@sx[12],@KL[6]
		index	$zctr.s,@sx[13],1
		index	@mx[12].s,@sx[13],1
	.else
		index	$zctr.s,@KL[6],1
		index	@mx[12].s,@KL[6],1
	.endif
	lsr	@sxx[13],@K[6],#32
	dup	@mx[13].s,@sx[13]
___
}

sub SVE2_LOAD_STATES() {
	&SET_STATE_BAK(0);
	&SET_STATE_BAK(2);
	&SET_STATE_BAK(4);
	&SET_STATE_BAK(6);
	&SET_STATE_BAK(8);
	&SET_STATE_BAK(10);
	&SET_STATE_BAK(14);

$code.=<<___;
	.if mixin == 1
		add	@sx[13],@KL[6],#1
		mov	@sx[12],@KL[6]
		index	$zctr.s,@sx[13],1
		index	@mx[12].s,@sx[13],1
	.else
		index	$zctr.s,@KL[6],1
		index	@mx[12].s,@KL[6],1
	.endif
	lsr	@sxx[13],@K[6],#32
	dup	@mx[13].s,@sx[13]
	dup	@bak[13].s,@sx[13]
___
}

sub chacha20_sve() {
	my ($tmp) = (@sxx[0]);

$code.=<<___;
.align	5
100:
	subs	$tmp,$len,$veclen,lsl #6
	b.lt	110f
	mov	$len,$tmp
	b.eq	101f
	cmp	$len,64
	b.lt	101f
	mixin=1
___
	&SVE_LOAD_STATES();
	&SVE_INNER_BLOCK();
	&SVE_ACCUM_STATES();
	&SVE_TRANSFORMS();
$code.=<<___;
	subs	$len,$len,64
	b.gt	100b
	b	110f
101:
	mixin=0
___
	&SVE_LOAD_STATES();
	&SVE_INNER_BLOCK();
	&SVE_ACCUM_STATES();
	&SVE_TRANSFORMS();
$code.=<<___;
110:
___
}

sub chacha20_sve2() {
	my ($tmp) = (@sxx[0]);

$code.=<<___;
.align	5
100:
	subs	$tmp,$len,$veclen,lsl #6
	b.lt	110f
	mov	$len,$tmp
	b.eq	101f
	cmp	$len,64
	b.lt	101f
	mixin=1
___
	&SVE2_LOAD_STATES();
	&SVE2_INNER_BLOCK();
	&SVE2_ACCUM_STATES();
	&SVE_TRANSFORMS();
$code.=<<___;
	subs	$len,$len,64
	b.gt	100b
	b	110f
101:
	mixin=0
___
	&SVE2_LOAD_STATES();
	&SVE2_INNER_BLOCK();
	&SVE2_ACCUM_STATES();
	&SVE_TRANSFORMS();
$code.=<<___;
110:
___
}


{{{
	my ($tmp,$tmpw) = ("x6", "w6");
	my ($tmpw0,$tmp0,$tmpw1,$tmp1) = ("w9","x9", "w10","x10");
	my ($sve2flag) = ("x7");

$code.=<<___;
#include "arm_arch.h"

.arch   armv8-a

.extern	OPENSSL_armcap_P
.hidden	OPENSSL_armcap_P

.text

.rodata
.align	5
.type _${prefix}_consts,%object
_${prefix}_consts:
.Lchacha20_consts:
.quad	0x3320646e61707865,0x6b20657479622d32		// endian-neutral
.Lrot8:
	.word 0x02010003,0x04040404,0x02010003,0x04040404
.size _${prefix}_consts,.-_${prefix}_consts

.previous

.globl	ChaCha20_ctr32_sve
.type	ChaCha20_ctr32_sve,%function
.align	5
ChaCha20_ctr32_sve:
	AARCH64_VALID_CALL_TARGET
	cntw	$veclen, ALL, MUL #1
	cmp	$len,$veclen,lsl #6
	b.lt	.Lreturn
	mov	$sve2flag,0
	adrp	$tmp,OPENSSL_armcap_P
	ldr	$tmpw,[$tmp,#:lo12:OPENSSL_armcap_P]
	tst	$tmpw,#ARMV8_SVE2
	b.eq	1f
	mov	$sve2flag,1
	b	2f
1:
	cmp	$veclen,4
	b.le	.Lreturn
	adrp	$tmp,.Lrot8
	add	$tmp,$tmp,#:lo12:.Lrot8
	ldp	$tmpw0,$tmpw1,[$tmp]
	index	$rot8.s,$tmpw0,$tmpw1
2:
	AARCH64_SIGN_LINK_REGISTER
	stp	d8,d9,[sp,-192]!
	stp	d10,d11,[sp,16]
	stp	d12,d13,[sp,32]
	stp	d14,d15,[sp,48]
	stp	x16,x17,[sp,64]
	stp	x18,x19,[sp,80]
	stp	x20,x21,[sp,96]
	stp	x22,x23,[sp,112]
	stp	x24,x25,[sp,128]
	stp	x26,x27,[sp,144]
	stp	x28,x29,[sp,160]
	str	x30,[sp,176]

	adrp	$tmp,.Lchacha20_consts
	add	$tmp,$tmp,#:lo12:.Lchacha20_consts
	ldp	@K[0],@K[1],[$tmp]
	ldp	@K[2],@K[3],[$key]
	ldp	@K[4],@K[5],[$key, 16]
	ldp	@K[6],@K[7],[$ctr]
	ptrues	p0.s,ALL
#ifdef	__AARCH64EB__
	ror	@K[2],@K[2],#32
	ror	@K[3],@K[3],#32
	ror	@K[4],@K[4],#32
	ror	@K[5],@K[5],#32
	ror	@K[6],@K[6],#32
	ror	@K[7],@K[7],#32
#endif
	cbz	$sve2flag, 1f
___
	&chacha20_sve2();
$code.=<<___;
	b	2f
1:
___
	&chacha20_sve();
$code.=<<___;
2:
	str	@KL[6],[$ctr]
	ldp	d10,d11,[sp,16]
	ldp	d12,d13,[sp,32]
	ldp	d14,d15,[sp,48]
	ldp	x16,x17,[sp,64]
	ldp	x18,x19,[sp,80]
	ldp	x20,x21,[sp,96]
	ldp	x22,x23,[sp,112]
	ldp	x24,x25,[sp,128]
	ldp	x26,x27,[sp,144]
	ldp	x28,x29,[sp,160]
	ldr	x30,[sp,176]
	ldp	d8,d9,[sp],192
	AARCH64_VALIDATE_LINK_REGISTER
.Lreturn:
	ret
.size	ChaCha20_ctr32_sve,.-ChaCha20_ctr32_sve
___

}}}

########################################
{
my  %opcode_unpred = (
	"movprfx"      => 0x0420BC00,
	"eor"          => 0x04a03000,
	"add"          => 0x04200000,
	"orr"          => 0x04603000,
	"lsl"          => 0x04209C00,
	"lsr"          => 0x04209400,
	"incw"         => 0x04B00000,
	"xar"          => 0x04203400,
	"zip1"         => 0x05206000,
	"zip2"         => 0x05206400,
	"uzp1"         => 0x05206800,
	"uzp2"         => 0x05206C00,
	"index"        => 0x04204C00,
	"mov"          => 0x05203800,
	"dup"          => 0x05203800,
	"cntw"         => 0x04A0E000,
	"tbl"          => 0x05203000);

my  %opcode_imm_unpred = (
	"dup"          => 0x2538C000,
	"index"        => 0x04204400);

my %opcode_scalar_pred = (
	"mov"          => 0x0528A000,
	"cpy"          => 0x0528A000,
	"st4w"         => 0xE5606000,
	"st1w"         => 0xE5004000,
	"ld1w"         => 0xA5404000);

my %opcode_gather_pred = (
	"ld1w"         => 0x85204000);

my  %opcode_pred = (
	"eor"          => 0x04190000,
	"add"          => 0x04000000,
	"orr"          => 0x04180000,
	"whilelo"      => 0x25200C00,
	"whilelt"      => 0x25200400,
	"cntp"         => 0x25208000,
	"addvl"        => 0x04205000,
	"lsl"          => 0x04038000,
	"lsr"          => 0x04018000,
	"sel"          => 0x0520C000,
	"mov"          => 0x0520C000,
	"ptrue"        => 0x2518E000,
	"pfalse"       => 0x2518E400,
	"ptrues"       => 0x2519E000,
	"pnext"        => 0x2519C400,
	"ld4w"         => 0xA560E000,
	"st4w"         => 0xE570E000,
	"st1w"         => 0xE500E000,
	"ld1w"         => 0xA540A000,
	"ld1rw"        => 0x8540C000,
	"lasta"        => 0x0520A000,
	"revh"         => 0x05258000,
	"revb"         => 0x05248000);

my  %tsize = (
	'b'          => 0,
	'h'          => 1,
	's'          => 2,
	'd'          => 3);

my %sf = (
	"w"          => 0,
	"x"          => 1);

my %pattern = (
	"POW2"       => 0,
	"VL1"        => 1,
	"VL2"        => 2,
	"VL3"        => 3,
	"VL4"        => 4,
	"VL5"        => 5,
	"VL6"        => 6,
	"VL7"        => 7,
	"VL8"        => 8,
	"VL16"       => 9,
	"VL32"       => 10,
	"VL64"       => 11,
	"VL128"      => 12,
	"VL256"      => 13,
	"MUL4"       => 29,
	"MUL3"       => 30,
	"ALL"        => 31);

sub create_verifier {
	my $filename="./compile_sve.sh";

$scripts = <<___;
#! /bin/bash
set -e
CROSS_COMPILE=\${CROSS_COMPILE:-'aarch64-none-linux-gnu-'}

[ -z "\$1" ] && exit 1
ARCH=`uname -p | xargs echo -n`

# need gcc-10 and above to compile SVE code
# change this according to your system during debugging
if [ \$ARCH == 'aarch64' ]; then
	CC=gcc-11
	OBJDUMP=objdump
else
	CC=\${CROSS_COMPILE}gcc
	OBJDUMP=\${CROSS_COMPILE}objdump
fi
TMPFILE=/tmp/\$\$
cat > \$TMPFILE.c << EOF
extern __attribute__((noinline, section("disasm_output"))) void dummy_func()
{
	asm("\$@\\t\\n");
}
int main(int argc, char *argv[])
{
}
EOF
\$CC -march=armv8.2-a+sve+sve2 -o \$TMPFILE.out \$TMPFILE.c
\$OBJDUMP -d \$TMPFILE.out | awk -F"\\n" -v RS="\\n\\n" '\$1 ~ /dummy_func/' | awk 'FNR == 2 {printf "%s",\$2}'
rm \$TMPFILE.c \$TMPFILE.out
___
	open(FH, '>', $filename) or die $!;
	print FH $scripts;
	close(FH);
	system("chmod a+x ./compile_sve.sh");
}

sub compile_sve {
	return `./compile_sve.sh '@_'`
}

sub verify_inst {
	my ($code,$inst)=@_;
	my $hexcode = (sprintf "%08x", $code);

	if ($debug_encoder == 1) {
		my $expect=&compile_sve($inst);
		if ($expect ne $hexcode) {
			return (sprintf "%s // Encode Error! expect [%s] actual [%s]", $inst, $expect, $hexcode);
		}
	}
	return (sprintf ".inst\t0x%s\t//%s", $hexcode, $inst);
}

sub reg_code {
	my $code = shift;

	if ($code == "zr") {
		return "31";
	}
	return $code;
}

sub encode_size_imm() {
	my ($mnemonic, $isize, $const)=@_;
	my $esize = (8<<$tsize{$isize});
	my $tsize_imm = $esize + $const;

	if ($mnemonic eq "lsr" || $mnemonic eq "xar") {
		$tsize_imm = 2*$esize - $const;
	}
	return (($tsize_imm>>5)<<22)|(($tsize_imm&0x1f)<<16);
}

sub encode_shift_pred() {
	my ($mnemonic, $isize, $const)=@_;
	my $esize = (8<<$tsize{$isize});
	my $tsize_imm = $esize + $const;

	if ($mnemonic eq "lsr") {
		$tsize_imm = 2*$esize - $const;
	}
	return (($tsize_imm>>5)<<22)|(($tsize_imm&0x1f)<<5);
}

sub sve_unpred {
	my ($mnemonic,$arg)=@_;
	my $inst = (sprintf "%s %s", $mnemonic,$arg);

	if ($arg =~ m/z([0-9]+)\.([bhsd]),\s*\{\s*z([0-9]+)\.[bhsd].*\},\s*z([0-9]+)\.[bhsd].*/o) {
		return &verify_inst($opcode_unpred{$mnemonic}|$1|($3<<5)|($tsize{$2}<<22)|($4<<16),
					$inst)
	} elsif ($arg =~ m/z([0-9]+)\.([bhsd]),\s*([zwx][0-9]+.*)/o) {
       		my $regd = $1;
		my $isize = $2;
		my $regs=$3;

		if (($mnemonic eq "lsl") || ($mnemonic eq "lsr")) {
			if ($regs =~ m/z([0-9]+)[^,]*(?:,\s*#?([0-9]+))?/o
				&& ((8<<$tsize{$isize}) > $2)) {
				return &verify_inst($opcode_unpred{$mnemonic}|$regd|($1<<5)|&encode_size_imm($mnemonic,$isize,$2),
					$inst);
			}
		} elsif($regs =~ m/[wx]([0-9]+),\s*[wx]([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$regd|($tsize{$isize}<<22)|($1<<5)|($2<<16), $inst);
		} elsif ($regs =~ m/[wx]([0-9]+),\s*#?([0-9]+)/o) {
			return &verify_inst($opcode_imm_unpred{$mnemonic}|$regd|($tsize{$isize}<<22)|($1<<5)|($2<<16), $inst);
		} elsif ($regs =~ m/[wx]([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$regd|($tsize{$isize}<<22)|($1<<5), $inst);
		} else {
			my $encoded_size = 0;
			if (($mnemonic eq "add") || ($mnemonic =~ /zip./) || ($mnemonic =~ /uzp./) ) {
				$encoded_size = ($tsize{$isize}<<22);
			}
			if ($regs =~ m/z([0-9]+)\.[bhsd],\s*z([0-9]+)\.[bhsd],\s*([0-9]+)/o &&
				$1 == $regd) {
				return &verify_inst($opcode_unpred{$mnemonic}|$regd|($2<<5)|&encode_size_imm($mnemonic,$isize,$3), $inst);
			} elsif ($regs =~ m/z([0-9]+)\.[bhsd],\s*z([0-9]+)\.[bhsd]/o) {
				return &verify_inst($opcode_unpred{$mnemonic}|$regd|$encoded_size|($1<<5)|($2<<16), $inst);
			}
		}
	} elsif ($arg =~ m/z([0-9]+)\.([bhsd]),\s*#?([0-9]+)/o) {
		return &verify_inst($opcode_imm_unpred{$mnemonic}|$1|($3<<5)|($tsize{$2}<<22),
					$inst)
	}
	sprintf "%s // fail to parse", $inst;
}

sub sve_pred {
	my ($mnemonic,,$arg)=@_;
	my $inst = (sprintf "%s %s", $mnemonic,$arg);

	if ($arg =~ m/\{\s*z([0-9]+)\.([bhsd]).*\},\s*p([0-9])+(\/z)?,\s*\[(\s*[xs].*)\]/o) {
		my $zt = $1;
		my $size = $tsize{$2};
		my $pg = $3;
		my $addr = $5;
		my $xn = 31;

		if ($addr =~ m/x([0-9]+)\s*/o) {
			$xn = $1;
		}

		if ($mnemonic =~m/ld1r[bhwd]/o) {
			$size = 0;
		}
		if ($addr =~ m/\w+\s*,\s*x([0-9]+),.*/o) {
			return &verify_inst($opcode_scalar_pred{$mnemonic}|($size<<21)|$zt|($pg<<10)|($1<<16)|($xn<<5),$inst);
		} elsif ($addr =~ m/\w+\s*,\s*z([0-9]+)\.s,\s*([US]\w+)/o) {
			my $xs = ($2 eq "SXTW") ? 1 : 0;
			return &verify_inst($opcode_gather_pred{$mnemonic}|($xs<<22)|$zt|($pg<<10)|($1<<16)|($xn<<5),$inst);
		} elsif($addr =~ m/\w+\s*,\s*#?([0-9]+)/o) {
			return &verify_inst($opcode_pred{$mnemonic}|($size<<21)|$zt|($pg<<10)|($1<<16)|($xn<<5),$inst);
		} else {
			return &verify_inst($opcode_pred{$mnemonic}|($size<<21)|$zt|($pg<<10)|($xn<<5),$inst);
		}
	} elsif ($arg =~ m/z([0-9]+)\.([bhsd]),\s*p([0-9]+)\/([mz]),\s*([zwx][0-9]+.*)/o) {
		my $regd = $1;
		my $isize = $2;
		my $pg = $3;
		my $mod = $4;
		my $regs = $5;

		if (($mnemonic eq "lsl") || ($mnemonic eq "lsr")) {
			if ($regs =~ m/z([0-9]+)[^,]*(?:,\s*#?([0-9]+))?/o
				&& $regd == $1
				&& $mode == 'm'
				&& ((8<<$tsize{$isize}) > $2)) {
				return &verify_inst($opcode_pred{$mnemonic}|$regd|($pg<<10)|&encode_shift_pred($mnemonic,$isize,$2), $inst);
			}
		} elsif($regs =~ m/[wx]([0-9]+)/o) {
			return &verify_inst($opcode_scalar_pred{$mnemonic}|$regd|($tsize{$isize}<<22)|($pg<<10)|($1<<5), $inst);
		} elsif ($regs =~ m/z([0-9]+)[^,]*(?:,\s*z([0-9]+))?/o) {
			if ($mnemonic eq "sel") {
				return &verify_inst($opcode_pred{$mnemonic}|$regd|($tsize{$isize}<<22)|($pg<<10)|($1<<5)|($2<<16), $inst);
			} elsif ($mnemonic eq "mov") {
				return &verify_inst($opcode_pred{$mnemonic}|$regd|($tsize{$isize}<<22)|($pg<<10)|($1<<5)|($regd<<16), $inst);
			} elsif (length $2 > 0) {
				return &verify_inst($opcode_pred{$mnemonic}|$regd|($tsize{$isize}<<22)|($pg<<10)|($2<<5), $inst);
			} else {
				return &verify_inst($opcode_pred{$mnemonic}|$regd|($tsize{$isize}<<22)|($pg<<10)|($1<<5), $inst);
			}
		}
	} elsif ($arg =~ m/p([0-9]+)\.([bhsd]),\s*(\w+.*)/o) {
		my $pg = $1;
		my $isize = $2;
		my $regs = $3;

		if ($regs =~ m/([wx])(zr|[0-9]+),\s*[wx](zr|[0-9]+)/o) {
			return &verify_inst($opcode_pred{$mnemonic}|($tsize{$isize}<<22)|$pg|($sf{$1}<<12)|(&reg_code($2)<<5)|(&reg_code($3)<<16), $inst);
		} elsif ($regs =~ m/p([0-9]+),\s*p([0-9]+)\.[bhsd]/o) {
			return &verify_inst($opcode_pred{$mnemonic}|($tsize{$isize}<<22)|$pg|($1<<5), $inst);
		} else {
			return &verify_inst($opcode_pred{$mnemonic}|($tsize{$isize}<<22)|$pg|($pattern{$regs}<<5), $inst);
		}
	} elsif ($arg =~ m/p([0-9]+)\.([bhsd])/o) {
		return &verify_inst($opcode_pred{$mnemonic}|$1, $inst);
	}

	sprintf "%s // fail to parse", $inst;
}

sub sve_other {
	my ($mnemonic,$arg)=@_;
	my $inst = (sprintf "%s %s", $mnemonic,$arg);

	if ($arg =~ m/x([0-9]+)[^,]*,\s*p([0-9]+)[^,]*,\s*p([0-9]+)\.([bhsd])/o) {
		return &verify_inst($opcode_pred{$mnemonic}|($tsize{$4}<<22)|$1|($2<<10)|($3<<5), $inst);
	} elsif ($arg =~ m/(x|w)([0-9]+)[^,]*,\s*p([0-9]+)[^,]*,\s*z([0-9]+)\.([bhsd])/o) {
		return &verify_inst($opcode_pred{$mnemonic}|($tsize{$5}<<22)|$1|($3<<10)|($4<<5)|$2, $inst);
	}elsif ($mnemonic =~ /inc[bhdw]/) {
		if ($arg =~ m/x([0-9]+)[^,]*,\s*(\w+)[^,]*,\s*MUL\s*#?([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$1|($pattern{$2}<<5)|(2<<12)|(($3 - 1)<<16)|0xE000, $inst);
		} elsif ($arg =~ m/z([0-9]+)[^,]*,\s*(\w+)[^,]*,\s*MUL\s*#?([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$1|($pattern{$2}<<5)|(($3 - 1)<<16)|0xC000, $inst);
		} elsif ($arg =~ m/x([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$1|(31<<5)|(0<<16)|0xE000, $inst);
		}
	} elsif ($mnemonic =~ /cnt[bhdw]/) {
		if ($arg =~ m/x([0-9]+)[^,]*,\s*(\w+)[^,]*,\s*MUL\s*#?([0-9]+)/o) {
			return &verify_inst($opcode_unpred{$mnemonic}|$1|($pattern{$2}<<5)|(($3 - 1)<<16), $inst);
		}
	} elsif ($arg =~ m/x([0-9]+)[^,]*,\s*x([0-9]+)[^,]*,\s*#?([0-9]+)/o) {
		return &verify_inst($opcode_pred{$mnemonic}|$1|($2<<16)|($3<<5), $inst);
	} elsif ($arg =~ m/z([0-9]+)[^,]*,\s*z([0-9]+)/o) {
		return &verify_inst($opcode_unpred{$mnemonic}|$1|($2<<5), $inst);
	}
	sprintf "%s // fail to parse", $inst;
}
}

open SELF,$0;
while(<SELF>) {
	next if (/^#!/);
	last if (!s/^#/\/\// and !/^$/);
	print;
}
close SELF;

if ($debug_encoder == 1) {
	&create_verifier();
}

foreach(split("\n",$code)) {
	s/\`([^\`]*)\`/eval($1)/ge;
	s/\b(\w+)\s+(z[0-9]+\.[bhsd],\s*[#zwx]?[0-9]+.*)/sve_unpred($1,$2)/ge;
	s/\b(\w+)\s+(z[0-9]+\.[bhsd],\s*\{.*\},\s*z[0-9]+.*)/sve_unpred($1,$2)/ge;
	s/\b(\w+)\s+(z[0-9]+\.[bhsd],\s*p[0-9].*)/sve_pred($1,$2)/ge;
	s/\b(\w+[1-4]r[bhwd])\s+(\{\s*z[0-9]+.*\},\s*p[0-9]+.*)/sve_pred($1,$2)/ge;
	s/\b(\w+[1-4][bhwd])\s+(\{\s*z[0-9]+.*\},\s*p[0-9]+.*)/sve_pred($1,$2)/ge;
	s/\b(\w+)\s+(p[0-9]+\.[bhsd].*)/sve_pred($1,$2)/ge;
	s/\b(movprfx|lasta|cntp|cnt[bhdw]|addvl|inc[bhdw])\s+((x|z|w).*)/sve_other($1,$2)/ge;
	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
