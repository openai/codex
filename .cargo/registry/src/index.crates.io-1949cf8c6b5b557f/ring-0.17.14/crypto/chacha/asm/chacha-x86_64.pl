#! /usr/bin/env perl
# Copyright 2016 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

#
# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project.
# ====================================================================
#
# November 2014
#
# ChaCha20 for x86_64.
#
# December 2016
#
# Add AVX512F code path.
#
# Performance in cycles per byte out of large buffer.
#
#		IALU/gcc 4.8(i)	1xSSSE3/SSE2	4xSSSE3	    NxAVX(v)
#
# P4		9.48/+99%	-/22.7(ii)	-
# Core2		7.83/+55%	7.90/8.08	4.35
# Westmere	7.19/+50%	5.60/6.70	3.00
# Sandy Bridge	8.31/+42%	5.45/6.76	2.72
# Ivy Bridge	6.71/+46%	5.40/6.49	2.41
# Haswell	5.92/+43%	5.20/6.45	2.42	    1.23
# Skylake[-X]	5.87/+39%	4.70/-		2.31	    1.19[0.57]
# Silvermont	12.0/+33%	7.75/7.40	7.03(iii)
# Knights L	11.7/-		-		9.60(iii)   0.80
# Goldmont	10.6/+17%	5.10/-		3.28
# Sledgehammer	7.28/+52%	-/14.2(ii)	-
# Bulldozer	9.66/+28%	9.85/11.1	3.06(iv)
# Ryzen		5.96/+50%	5.19/-		2.40        2.09
# VIA Nano	10.5/+46%	6.72/8.60	6.05
#
# (i)	compared to older gcc 3.x one can observe >2x improvement on
#	most platforms;
# (ii)	as it can be seen, SSE2 performance is too low on legacy
#	processors; NxSSE2 results are naturally better, but not
#	impressively better than IALU ones, which is why you won't
#	find SSE2 code below;
# (iii)	this is not optimal result for Atom because of MSROM
#	limitations, SSE2 can do better, but gain is considered too
#	low to justify the [maintenance] effort;
# (iv)	Bulldozer actually executes 4xXOP code path that delivers 2.20;
#
# Modified from upstream OpenSSL to remove the XOP code.

$flavour = shift;
$output  = shift;
if ($flavour =~ /\./) { $output = $flavour; undef $flavour; }

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

$avx = 2;

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

# input parameter block
($out,$inp,$len,$key,$counter)=("%rdi","%rsi","%rdx","%rcx","%r8");

$code.=<<___;
.text

.section .rodata
.align	64
.Lzero:
.long	0,0,0,0
.Lone:
.long	1,0,0,0
.Linc:
.long	0,1,2,3
.Lfour:
.long	4,4,4,4
.Lincy:
.long	0,2,4,6,1,3,5,7
.Leight:
.long	8,8,8,8,8,8,8,8
.Lrot16:
.byte	0x2,0x3,0x0,0x1, 0x6,0x7,0x4,0x5, 0xa,0xb,0x8,0x9, 0xe,0xf,0xc,0xd
.Lrot24:
.byte	0x3,0x0,0x1,0x2, 0x7,0x4,0x5,0x6, 0xb,0x8,0x9,0xa, 0xf,0xc,0xd,0xe
.Lsigma:
.asciz	"expand 32-byte k"
.align	64
.Lzeroz:
.long	0,0,0,0, 1,0,0,0, 2,0,0,0, 3,0,0,0
.Lfourz:
.long	4,0,0,0, 4,0,0,0, 4,0,0,0, 4,0,0,0
.Lincz:
.long	0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
.Lsixteen:
.long	16,16,16,16,16,16,16,16,16,16,16,16,16,16,16,16
.asciz	"ChaCha20 for x86_64, CRYPTOGAMS by <appro\@openssl.org>"
.text
___

sub AUTOLOAD()          # thunk [simplified] 32-bit style perlasm
{ my $opcode = $AUTOLOAD; $opcode =~ s/.*:://;
  my $arg = pop;
    $arg = "\$$arg" if ($arg*1 eq $arg);
    $code .= "\t$opcode\t".join(',',$arg,reverse @_)."\n";
}

@x=("%eax","%ebx","%ecx","%edx",map("%r${_}d",(8..11)),
    "%nox","%nox","%nox","%nox",map("%r${_}d",(12..15)));
@t=("%esi","%edi");

sub ROUND {			# critical path is 24 cycles per round
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my ($xc,$xc_)=map("\"$_\"",@t);
my @x=map("\"$_\"",@x);

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
	# bunch of 'c' stores and loads in the middle, but none in
	# the beginning or end.

	# Normally instructions would be interleaved to favour in-order
	# execution. Generally out-of-order cores manage it gracefully,
	# but not this time for some reason. As in-order execution
	# cores are dying breed, old Atom is the only one around,
	# instructions are left uninterleaved. Besides, Atom is better
	# off executing 1xSSSE3 code anyway...

	(
	"&add	(@x[$a0],@x[$b0])",	# Q1
	"&xor	(@x[$d0],@x[$a0])",
	"&rol	(@x[$d0],16)",
	 "&add	(@x[$a1],@x[$b1])",	# Q2
	 "&xor	(@x[$d1],@x[$a1])",
	 "&rol	(@x[$d1],16)",

	"&add	($xc,@x[$d0])",
	"&xor	(@x[$b0],$xc)",
	"&rol	(@x[$b0],12)",
	 "&add	($xc_,@x[$d1])",
	 "&xor	(@x[$b1],$xc_)",
	 "&rol	(@x[$b1],12)",

	"&add	(@x[$a0],@x[$b0])",
	"&xor	(@x[$d0],@x[$a0])",
	"&rol	(@x[$d0],8)",
	 "&add	(@x[$a1],@x[$b1])",
	 "&xor	(@x[$d1],@x[$a1])",
	 "&rol	(@x[$d1],8)",

	"&add	($xc,@x[$d0])",
	"&xor	(@x[$b0],$xc)",
	"&rol	(@x[$b0],7)",
	 "&add	($xc_,@x[$d1])",
	 "&xor	(@x[$b1],$xc_)",
	 "&rol	(@x[$b1],7)",

	"&mov	(\"4*$c0(%rsp)\",$xc)",	# reload pair of 'c's
	 "&mov	(\"4*$c1(%rsp)\",$xc_)",
	"&mov	($xc,\"4*$c2(%rsp)\")",
	 "&mov	($xc_,\"4*$c3(%rsp)\")",

	"&add	(@x[$a2],@x[$b2])",	# Q3
	"&xor	(@x[$d2],@x[$a2])",
	"&rol	(@x[$d2],16)",
	 "&add	(@x[$a3],@x[$b3])",	# Q4
	 "&xor	(@x[$d3],@x[$a3])",
	 "&rol	(@x[$d3],16)",

	"&add	($xc,@x[$d2])",
	"&xor	(@x[$b2],$xc)",
	"&rol	(@x[$b2],12)",
	 "&add	($xc_,@x[$d3])",
	 "&xor	(@x[$b3],$xc_)",
	 "&rol	(@x[$b3],12)",

	"&add	(@x[$a2],@x[$b2])",
	"&xor	(@x[$d2],@x[$a2])",
	"&rol	(@x[$d2],8)",
	 "&add	(@x[$a3],@x[$b3])",
	 "&xor	(@x[$d3],@x[$a3])",
	 "&rol	(@x[$d3],8)",

	"&add	($xc,@x[$d2])",
	"&xor	(@x[$b2],$xc)",
	"&rol	(@x[$b2],7)",
	 "&add	($xc_,@x[$d3])",
	 "&xor	(@x[$b3],$xc_)",
	 "&rol	(@x[$b3],7)"
	);
}

########################################################################
# Generic code path that handles all lengths on pre-SSSE3 processors.
$code.=<<___;
.globl	ChaCha20_ctr32_nohw
.type	ChaCha20_ctr32_nohw,\@function,5
.align	64
ChaCha20_ctr32_nohw:
.cfi_startproc
	_CET_ENDBR
	push	%rbx
.cfi_push	rbx
	push	%rbp
.cfi_push	rbp
	push	%r12
.cfi_push	r12
	push	%r13
.cfi_push	r13
	push	%r14
.cfi_push	r14
	push	%r15
.cfi_push	r15
	sub	\$64+24,%rsp
.cfi_adjust_cfa_offset	`64+24`
.Lctr32_body:

	#movdqa	.Lsigma(%rip),%xmm0
	movdqu	($key),%xmm1
	movdqu	16($key),%xmm2
	movdqu	($counter),%xmm3
	movdqa	.Lone(%rip),%xmm4

	#movdqa	%xmm0,4*0(%rsp)		# key[0]
	movdqa	%xmm1,4*4(%rsp)		# key[1]
	movdqa	%xmm2,4*8(%rsp)		# key[2]
	movdqa	%xmm3,4*12(%rsp)	# key[3]
	mov	$len,%rbp		# reassign $len
	jmp	.Loop_outer

.align	32
.Loop_outer:
	mov	\$0x61707865,@x[0]      # 'expa'
	mov	\$0x3320646e,@x[1]      # 'nd 3'
	mov	\$0x79622d32,@x[2]      # '2-by'
	mov	\$0x6b206574,@x[3]      # 'te k'
	mov	4*4(%rsp),@x[4]
	mov	4*5(%rsp),@x[5]
	mov	4*6(%rsp),@x[6]
	mov	4*7(%rsp),@x[7]
	movd	%xmm3,@x[12]
	mov	4*13(%rsp),@x[13]
	mov	4*14(%rsp),@x[14]
	mov	4*15(%rsp),@x[15]

	mov	%rbp,64+0(%rsp)		# save len
	mov	\$10,%ebp
	mov	$inp,64+8(%rsp)		# save inp
	movq	%xmm2,%rsi		# "@x[8]"
	mov	$out,64+16(%rsp)	# save out
	mov	%rsi,%rdi
	shr	\$32,%rdi		# "@x[9]"
	jmp	.Loop

.align	32
.Loop:
___
	foreach (&ROUND (0, 4, 8,12)) { eval; }
	foreach (&ROUND	(0, 5,10,15)) { eval; }
	&dec	("%ebp");
	&jnz	(".Loop");

$code.=<<___;
	mov	@t[1],4*9(%rsp)		# modulo-scheduled
	mov	@t[0],4*8(%rsp)
	mov	64(%rsp),%rbp		# load len
	movdqa	%xmm2,%xmm1
	mov	64+8(%rsp),$inp		# load inp
	paddd	%xmm4,%xmm3		# increment counter
	mov	64+16(%rsp),$out	# load out

	add	\$0x61707865,@x[0]      # 'expa'
	add	\$0x3320646e,@x[1]      # 'nd 3'
	add	\$0x79622d32,@x[2]      # '2-by'
	add	\$0x6b206574,@x[3]      # 'te k'
	add	4*4(%rsp),@x[4]
	add	4*5(%rsp),@x[5]
	add	4*6(%rsp),@x[6]
	add	4*7(%rsp),@x[7]
	add	4*12(%rsp),@x[12]
	add	4*13(%rsp),@x[13]
	add	4*14(%rsp),@x[14]
	add	4*15(%rsp),@x[15]
	paddd	4*8(%rsp),%xmm1

	cmp	\$64,%rbp
	jb	.Ltail

	xor	4*0($inp),@x[0]		# xor with input
	xor	4*1($inp),@x[1]
	xor	4*2($inp),@x[2]
	xor	4*3($inp),@x[3]
	xor	4*4($inp),@x[4]
	xor	4*5($inp),@x[5]
	xor	4*6($inp),@x[6]
	xor	4*7($inp),@x[7]
	movdqu	4*8($inp),%xmm0
	xor	4*12($inp),@x[12]
	xor	4*13($inp),@x[13]
	xor	4*14($inp),@x[14]
	xor	4*15($inp),@x[15]
	lea	4*16($inp),$inp		# inp+=64
	pxor	%xmm1,%xmm0

	movdqa	%xmm2,4*8(%rsp)
	movd	%xmm3,4*12(%rsp)

	mov	@x[0],4*0($out)		# write output
	mov	@x[1],4*1($out)
	mov	@x[2],4*2($out)
	mov	@x[3],4*3($out)
	mov	@x[4],4*4($out)
	mov	@x[5],4*5($out)
	mov	@x[6],4*6($out)
	mov	@x[7],4*7($out)
	movdqu	%xmm0,4*8($out)
	mov	@x[12],4*12($out)
	mov	@x[13],4*13($out)
	mov	@x[14],4*14($out)
	mov	@x[15],4*15($out)
	lea	4*16($out),$out		# out+=64

	sub	\$64,%rbp
	jnz	.Loop_outer

	jmp	.Ldone

.align	16
.Ltail:
	mov	@x[0],4*0(%rsp)
	mov	@x[1],4*1(%rsp)
	xor	%rbx,%rbx
	mov	@x[2],4*2(%rsp)
	mov	@x[3],4*3(%rsp)
	mov	@x[4],4*4(%rsp)
	mov	@x[5],4*5(%rsp)
	mov	@x[6],4*6(%rsp)
	mov	@x[7],4*7(%rsp)
	movdqa	%xmm1,4*8(%rsp)
	mov	@x[12],4*12(%rsp)
	mov	@x[13],4*13(%rsp)
	mov	@x[14],4*14(%rsp)
	mov	@x[15],4*15(%rsp)

.Loop_tail:
	movzb	($inp,%rbx),%eax
	movzb	(%rsp,%rbx),%edx
	lea	1(%rbx),%rbx
	xor	%edx,%eax
	mov	%al,-1($out,%rbx)
	dec	%rbp
	jnz	.Loop_tail

.Ldone:
	lea	64+24+48(%rsp),%rsi
	mov	-48(%rsi),%r15
.cfi_restore	r15
	mov	-40(%rsi),%r14
.cfi_restore	r14
	mov	-32(%rsi),%r13
.cfi_restore	r13
	mov	-24(%rsi),%r12
.cfi_restore	r12
	mov	-16(%rsi),%rbp
.cfi_restore	rbp
	mov	-8(%rsi),%rbx
.cfi_restore	rbx
	lea	(%rsi),%rsp
.cfi_adjust_cfa_offset	`-64-24-48`
.Lno_data:
	ret
.cfi_endproc
.size	ChaCha20_ctr32_nohw,.-ChaCha20_ctr32_nohw
___

########################################################################
# SSSE3 code path that handles longer messages.
{
# assign variables to favor Atom front-end
my ($xd0,$xd1,$xd2,$xd3, $xt0,$xt1,$xt2,$xt3,
    $xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3)=map("%xmm$_",(0..15));
my  @xx=($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
	"%nox","%nox","%nox","%nox", $xd0,$xd1,$xd2,$xd3);

sub SSSE3_lane_ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my ($xc,$xc_,$t0,$t1)=map("\"$_\"",$xt0,$xt1,$xt2,$xt3);
my @x=map("\"$_\"",@xx);

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
	# bunch of 'c' stores and loads in the middle, but none in
	# the beginning or end.

	(
	"&paddd		(@x[$a0],@x[$b0])",	# Q1
	 "&paddd	(@x[$a1],@x[$b1])",	# Q2
	"&pxor		(@x[$d0],@x[$a0])",
	 "&pxor		(@x[$d1],@x[$a1])",
	"&pshufb	(@x[$d0],$t1)",
	 "&pshufb	(@x[$d1],$t1)",

	"&paddd		($xc,@x[$d0])",
	 "&paddd	($xc_,@x[$d1])",
	"&pxor		(@x[$b0],$xc)",
	 "&pxor		(@x[$b1],$xc_)",
	"&movdqa	($t0,@x[$b0])",
	"&pslld		(@x[$b0],12)",
	"&psrld		($t0,20)",
	 "&movdqa	($t1,@x[$b1])",
	 "&pslld	(@x[$b1],12)",
	"&por		(@x[$b0],$t0)",
	 "&psrld	($t1,20)",
	"&movdqa	($t0,'(%r11)')",	# .Lrot24(%rip)
	 "&por		(@x[$b1],$t1)",

	"&paddd		(@x[$a0],@x[$b0])",
	 "&paddd	(@x[$a1],@x[$b1])",
	"&pxor		(@x[$d0],@x[$a0])",
	 "&pxor		(@x[$d1],@x[$a1])",
	"&pshufb	(@x[$d0],$t0)",
	 "&pshufb	(@x[$d1],$t0)",

	"&paddd		($xc,@x[$d0])",
	 "&paddd	($xc_,@x[$d1])",
	"&pxor		(@x[$b0],$xc)",
	 "&pxor		(@x[$b1],$xc_)",
	"&movdqa	($t1,@x[$b0])",
	"&pslld		(@x[$b0],7)",
	"&psrld		($t1,25)",
	 "&movdqa	($t0,@x[$b1])",
	 "&pslld	(@x[$b1],7)",
	"&por		(@x[$b0],$t1)",
	 "&psrld	($t0,25)",
	"&movdqa	($t1,'(%r10)')",	# .Lrot16(%rip)
	 "&por		(@x[$b1],$t0)",

	"&movdqa	(\"`16*($c0-8)`(%rsp)\",$xc)",	# reload pair of 'c's
	 "&movdqa	(\"`16*($c1-8)`(%rsp)\",$xc_)",
	"&movdqa	($xc,\"`16*($c2-8)`(%rsp)\")",
	 "&movdqa	($xc_,\"`16*($c3-8)`(%rsp)\")",

	"&paddd		(@x[$a2],@x[$b2])",	# Q3
	 "&paddd	(@x[$a3],@x[$b3])",	# Q4
	"&pxor		(@x[$d2],@x[$a2])",
	 "&pxor		(@x[$d3],@x[$a3])",
	"&pshufb	(@x[$d2],$t1)",
	 "&pshufb	(@x[$d3],$t1)",

	"&paddd		($xc,@x[$d2])",
	 "&paddd	($xc_,@x[$d3])",
	"&pxor		(@x[$b2],$xc)",
	 "&pxor		(@x[$b3],$xc_)",
	"&movdqa	($t0,@x[$b2])",
	"&pslld		(@x[$b2],12)",
	"&psrld		($t0,20)",
	 "&movdqa	($t1,@x[$b3])",
	 "&pslld	(@x[$b3],12)",
	"&por		(@x[$b2],$t0)",
	 "&psrld	($t1,20)",
	"&movdqa	($t0,'(%r11)')",	# .Lrot24(%rip)
	 "&por		(@x[$b3],$t1)",

	"&paddd		(@x[$a2],@x[$b2])",
	 "&paddd	(@x[$a3],@x[$b3])",
	"&pxor		(@x[$d2],@x[$a2])",
	 "&pxor		(@x[$d3],@x[$a3])",
	"&pshufb	(@x[$d2],$t0)",
	 "&pshufb	(@x[$d3],$t0)",

	"&paddd		($xc,@x[$d2])",
	 "&paddd	($xc_,@x[$d3])",
	"&pxor		(@x[$b2],$xc)",
	 "&pxor		(@x[$b3],$xc_)",
	"&movdqa	($t1,@x[$b2])",
	"&pslld		(@x[$b2],7)",
	"&psrld		($t1,25)",
	 "&movdqa	($t0,@x[$b3])",
	 "&pslld	(@x[$b3],7)",
	"&por		(@x[$b2],$t1)",
	 "&psrld	($t0,25)",
	"&movdqa	($t1,'(%r10)')",	# .Lrot16(%rip)
	 "&por		(@x[$b3],$t0)"
	);
}

my $xframe = $win64 ? 0xa8 : 8;

$code.=<<___;
.globl	ChaCha20_ctr32_ssse3_4x
.type	ChaCha20_ctr32_ssse3_4x,\@function,5
.align	32
ChaCha20_ctr32_ssse3_4x:
.cfi_startproc
	_CET_ENDBR
	mov		%rsp,%r9		# frame pointer
.cfi_def_cfa_register	r9
___
$code.=<<___;
	sub		\$0x140+$xframe,%rsp
___
	################ stack layout
	# +0x00		SIMD equivalent of @x[8-12]
	# ...
	# +0x40		constant copy of key[0-2] smashed by lanes
	# ...
	# +0x100	SIMD counters (with nonce smashed by lanes)
	# ...
	# +0x140
$code.=<<___	if ($win64);
	movaps		%xmm6,-0xa8(%r9)
	movaps		%xmm7,-0x98(%r9)
	movaps		%xmm8,-0x88(%r9)
	movaps		%xmm9,-0x78(%r9)
	movaps		%xmm10,-0x68(%r9)
	movaps		%xmm11,-0x58(%r9)
	movaps		%xmm12,-0x48(%r9)
	movaps		%xmm13,-0x38(%r9)
	movaps		%xmm14,-0x28(%r9)
	movaps		%xmm15,-0x18(%r9)
.L4x_body:
___
$code.=<<___;
	movdqa		.Lsigma(%rip),$xa3	# key[0]
	movdqu		($key),$xb3		# key[1]
	movdqu		16($key),$xt3		# key[2]
	movdqu		($counter),$xd3		# key[3]
	lea		0x100(%rsp),%rcx	# size optimization
	lea		.Lrot16(%rip),%r10
	lea		.Lrot24(%rip),%r11

	pshufd		\$0x00,$xa3,$xa0	# smash key by lanes...
	pshufd		\$0x55,$xa3,$xa1
	movdqa		$xa0,0x40(%rsp)		# ... and offload
	pshufd		\$0xaa,$xa3,$xa2
	movdqa		$xa1,0x50(%rsp)
	pshufd		\$0xff,$xa3,$xa3
	movdqa		$xa2,0x60(%rsp)
	movdqa		$xa3,0x70(%rsp)

	pshufd		\$0x00,$xb3,$xb0
	pshufd		\$0x55,$xb3,$xb1
	movdqa		$xb0,0x80-0x100(%rcx)
	pshufd		\$0xaa,$xb3,$xb2
	movdqa		$xb1,0x90-0x100(%rcx)
	pshufd		\$0xff,$xb3,$xb3
	movdqa		$xb2,0xa0-0x100(%rcx)
	movdqa		$xb3,0xb0-0x100(%rcx)

	pshufd		\$0x00,$xt3,$xt0	# "$xc0"
	pshufd		\$0x55,$xt3,$xt1	# "$xc1"
	movdqa		$xt0,0xc0-0x100(%rcx)
	pshufd		\$0xaa,$xt3,$xt2	# "$xc2"
	movdqa		$xt1,0xd0-0x100(%rcx)
	pshufd		\$0xff,$xt3,$xt3	# "$xc3"
	movdqa		$xt2,0xe0-0x100(%rcx)
	movdqa		$xt3,0xf0-0x100(%rcx)

	pshufd		\$0x00,$xd3,$xd0
	pshufd		\$0x55,$xd3,$xd1
	paddd		.Linc(%rip),$xd0	# don't save counters yet
	pshufd		\$0xaa,$xd3,$xd2
	movdqa		$xd1,0x110-0x100(%rcx)
	pshufd		\$0xff,$xd3,$xd3
	movdqa		$xd2,0x120-0x100(%rcx)
	movdqa		$xd3,0x130-0x100(%rcx)

	jmp		.Loop_enter4x

.align	32
.Loop_outer4x:
	movdqa		0x40(%rsp),$xa0		# re-load smashed key
	movdqa		0x50(%rsp),$xa1
	movdqa		0x60(%rsp),$xa2
	movdqa		0x70(%rsp),$xa3
	movdqa		0x80-0x100(%rcx),$xb0
	movdqa		0x90-0x100(%rcx),$xb1
	movdqa		0xa0-0x100(%rcx),$xb2
	movdqa		0xb0-0x100(%rcx),$xb3
	movdqa		0xc0-0x100(%rcx),$xt0	# "$xc0"
	movdqa		0xd0-0x100(%rcx),$xt1	# "$xc1"
	movdqa		0xe0-0x100(%rcx),$xt2	# "$xc2"
	movdqa		0xf0-0x100(%rcx),$xt3	# "$xc3"
	movdqa		0x100-0x100(%rcx),$xd0
	movdqa		0x110-0x100(%rcx),$xd1
	movdqa		0x120-0x100(%rcx),$xd2
	movdqa		0x130-0x100(%rcx),$xd3
	paddd		.Lfour(%rip),$xd0	# next SIMD counters

.Loop_enter4x:
	movdqa		$xt2,0x20(%rsp)		# SIMD equivalent of "@x[10]"
	movdqa		$xt3,0x30(%rsp)		# SIMD equivalent of "@x[11]"
	movdqa		(%r10),$xt3		# .Lrot16(%rip)
	mov		\$10,%eax
	movdqa		$xd0,0x100-0x100(%rcx)	# save SIMD counters
	jmp		.Loop4x

.align	32
.Loop4x:
___
	foreach (&SSSE3_lane_ROUND(0, 4, 8,12)) { eval; }
	foreach (&SSSE3_lane_ROUND(0, 5,10,15)) { eval; }
$code.=<<___;
	dec		%eax
	jnz		.Loop4x

	paddd		0x40(%rsp),$xa0		# accumulate key material
	paddd		0x50(%rsp),$xa1
	paddd		0x60(%rsp),$xa2
	paddd		0x70(%rsp),$xa3

	movdqa		$xa0,$xt2		# "de-interlace" data
	punpckldq	$xa1,$xa0
	movdqa		$xa2,$xt3
	punpckldq	$xa3,$xa2
	punpckhdq	$xa1,$xt2
	punpckhdq	$xa3,$xt3
	movdqa		$xa0,$xa1
	punpcklqdq	$xa2,$xa0		# "a0"
	movdqa		$xt2,$xa3
	punpcklqdq	$xt3,$xt2		# "a2"
	punpckhqdq	$xa2,$xa1		# "a1"
	punpckhqdq	$xt3,$xa3		# "a3"
___
	($xa2,$xt2)=($xt2,$xa2);
$code.=<<___;
	paddd		0x80-0x100(%rcx),$xb0
	paddd		0x90-0x100(%rcx),$xb1
	paddd		0xa0-0x100(%rcx),$xb2
	paddd		0xb0-0x100(%rcx),$xb3

	movdqa		$xa0,0x00(%rsp)		# offload $xaN
	movdqa		$xa1,0x10(%rsp)
	movdqa		0x20(%rsp),$xa0		# "xc2"
	movdqa		0x30(%rsp),$xa1		# "xc3"

	movdqa		$xb0,$xt2
	punpckldq	$xb1,$xb0
	movdqa		$xb2,$xt3
	punpckldq	$xb3,$xb2
	punpckhdq	$xb1,$xt2
	punpckhdq	$xb3,$xt3
	movdqa		$xb0,$xb1
	punpcklqdq	$xb2,$xb0		# "b0"
	movdqa		$xt2,$xb3
	punpcklqdq	$xt3,$xt2		# "b2"
	punpckhqdq	$xb2,$xb1		# "b1"
	punpckhqdq	$xt3,$xb3		# "b3"
___
	($xb2,$xt2)=($xt2,$xb2);
	my ($xc0,$xc1,$xc2,$xc3)=($xt0,$xt1,$xa0,$xa1);
$code.=<<___;
	paddd		0xc0-0x100(%rcx),$xc0
	paddd		0xd0-0x100(%rcx),$xc1
	paddd		0xe0-0x100(%rcx),$xc2
	paddd		0xf0-0x100(%rcx),$xc3

	movdqa		$xa2,0x20(%rsp)		# keep offloading $xaN
	movdqa		$xa3,0x30(%rsp)

	movdqa		$xc0,$xt2
	punpckldq	$xc1,$xc0
	movdqa		$xc2,$xt3
	punpckldq	$xc3,$xc2
	punpckhdq	$xc1,$xt2
	punpckhdq	$xc3,$xt3
	movdqa		$xc0,$xc1
	punpcklqdq	$xc2,$xc0		# "c0"
	movdqa		$xt2,$xc3
	punpcklqdq	$xt3,$xt2		# "c2"
	punpckhqdq	$xc2,$xc1		# "c1"
	punpckhqdq	$xt3,$xc3		# "c3"
___
	($xc2,$xt2)=($xt2,$xc2);
	($xt0,$xt1)=($xa2,$xa3);		# use $xaN as temporary
$code.=<<___;
	paddd		0x100-0x100(%rcx),$xd0
	paddd		0x110-0x100(%rcx),$xd1
	paddd		0x120-0x100(%rcx),$xd2
	paddd		0x130-0x100(%rcx),$xd3

	movdqa		$xd0,$xt2
	punpckldq	$xd1,$xd0
	movdqa		$xd2,$xt3
	punpckldq	$xd3,$xd2
	punpckhdq	$xd1,$xt2
	punpckhdq	$xd3,$xt3
	movdqa		$xd0,$xd1
	punpcklqdq	$xd2,$xd0		# "d0"
	movdqa		$xt2,$xd3
	punpcklqdq	$xt3,$xt2		# "d2"
	punpckhqdq	$xd2,$xd1		# "d1"
	punpckhqdq	$xt3,$xd3		# "d3"
___
	($xd2,$xt2)=($xt2,$xd2);
$code.=<<___;
	cmp		\$64*4,$len
	jb		.Ltail4x

	movdqu		0x00($inp),$xt0		# xor with input
	movdqu		0x10($inp),$xt1
	movdqu		0x20($inp),$xt2
	movdqu		0x30($inp),$xt3
	pxor		0x00(%rsp),$xt0		# $xaN is offloaded, remember?
	pxor		$xb0,$xt1
	pxor		$xc0,$xt2
	pxor		$xd0,$xt3

	 movdqu		$xt0,0x00($out)
	movdqu		0x40($inp),$xt0
	 movdqu		$xt1,0x10($out)
	movdqu		0x50($inp),$xt1
	 movdqu		$xt2,0x20($out)
	movdqu		0x60($inp),$xt2
	 movdqu		$xt3,0x30($out)
	movdqu		0x70($inp),$xt3
	lea		0x80($inp),$inp		# size optimization
	pxor		0x10(%rsp),$xt0
	pxor		$xb1,$xt1
	pxor		$xc1,$xt2
	pxor		$xd1,$xt3

	 movdqu		$xt0,0x40($out)
	movdqu		0x00($inp),$xt0
	 movdqu		$xt1,0x50($out)
	movdqu		0x10($inp),$xt1
	 movdqu		$xt2,0x60($out)
	movdqu		0x20($inp),$xt2
	 movdqu		$xt3,0x70($out)
	 lea		0x80($out),$out		# size optimization
	movdqu		0x30($inp),$xt3
	pxor		0x20(%rsp),$xt0
	pxor		$xb2,$xt1
	pxor		$xc2,$xt2
	pxor		$xd2,$xt3

	 movdqu		$xt0,0x00($out)
	movdqu		0x40($inp),$xt0
	 movdqu		$xt1,0x10($out)
	movdqu		0x50($inp),$xt1
	 movdqu		$xt2,0x20($out)
	movdqu		0x60($inp),$xt2
	 movdqu		$xt3,0x30($out)
	movdqu		0x70($inp),$xt3
	lea		0x80($inp),$inp		# inp+=64*4
	pxor		0x30(%rsp),$xt0
	pxor		$xb3,$xt1
	pxor		$xc3,$xt2
	pxor		$xd3,$xt3
	movdqu		$xt0,0x40($out)
	movdqu		$xt1,0x50($out)
	movdqu		$xt2,0x60($out)
	movdqu		$xt3,0x70($out)
	lea		0x80($out),$out		# out+=64*4

	sub		\$64*4,$len
	jnz		.Loop_outer4x

	jmp		.Ldone4x

.Ltail4x:
	cmp		\$192,$len
	jae		.L192_or_more4x
	cmp		\$128,$len
	jae		.L128_or_more4x
	cmp		\$64,$len
	jae		.L64_or_more4x

	#movdqa		0x00(%rsp),$xt0		# $xaN is offloaded, remember?
	xor		%r10,%r10
	#movdqa		$xt0,0x00(%rsp)
	movdqa		$xb0,0x10(%rsp)
	movdqa		$xc0,0x20(%rsp)
	movdqa		$xd0,0x30(%rsp)
	jmp		.Loop_tail4x

.align	32
.L64_or_more4x:
	movdqu		0x00($inp),$xt0		# xor with input
	movdqu		0x10($inp),$xt1
	movdqu		0x20($inp),$xt2
	movdqu		0x30($inp),$xt3
	pxor		0x00(%rsp),$xt0		# $xaxN is offloaded, remember?
	pxor		$xb0,$xt1
	pxor		$xc0,$xt2
	pxor		$xd0,$xt3
	movdqu		$xt0,0x00($out)
	movdqu		$xt1,0x10($out)
	movdqu		$xt2,0x20($out)
	movdqu		$xt3,0x30($out)
	je		.Ldone4x

	movdqa		0x10(%rsp),$xt0		# $xaN is offloaded, remember?
	lea		0x40($inp),$inp		# inp+=64*1
	xor		%r10,%r10
	movdqa		$xt0,0x00(%rsp)
	movdqa		$xb1,0x10(%rsp)
	lea		0x40($out),$out		# out+=64*1
	movdqa		$xc1,0x20(%rsp)
	sub		\$64,$len		# len-=64*1
	movdqa		$xd1,0x30(%rsp)
	jmp		.Loop_tail4x

.align	32
.L128_or_more4x:
	movdqu		0x00($inp),$xt0		# xor with input
	movdqu		0x10($inp),$xt1
	movdqu		0x20($inp),$xt2
	movdqu		0x30($inp),$xt3
	pxor		0x00(%rsp),$xt0		# $xaN is offloaded, remember?
	pxor		$xb0,$xt1
	pxor		$xc0,$xt2
	pxor		$xd0,$xt3

	 movdqu		$xt0,0x00($out)
	movdqu		0x40($inp),$xt0
	 movdqu		$xt1,0x10($out)
	movdqu		0x50($inp),$xt1
	 movdqu		$xt2,0x20($out)
	movdqu		0x60($inp),$xt2
	 movdqu		$xt3,0x30($out)
	movdqu		0x70($inp),$xt3
	pxor		0x10(%rsp),$xt0
	pxor		$xb1,$xt1
	pxor		$xc1,$xt2
	pxor		$xd1,$xt3
	movdqu		$xt0,0x40($out)
	movdqu		$xt1,0x50($out)
	movdqu		$xt2,0x60($out)
	movdqu		$xt3,0x70($out)
	je		.Ldone4x

	movdqa		0x20(%rsp),$xt0		# $xaN is offloaded, remember?
	lea		0x80($inp),$inp		# inp+=64*2
	xor		%r10,%r10
	movdqa		$xt0,0x00(%rsp)
	movdqa		$xb2,0x10(%rsp)
	lea		0x80($out),$out		# out+=64*2
	movdqa		$xc2,0x20(%rsp)
	sub		\$128,$len		# len-=64*2
	movdqa		$xd2,0x30(%rsp)
	jmp		.Loop_tail4x

.align	32
.L192_or_more4x:
	movdqu		0x00($inp),$xt0		# xor with input
	movdqu		0x10($inp),$xt1
	movdqu		0x20($inp),$xt2
	movdqu		0x30($inp),$xt3
	pxor		0x00(%rsp),$xt0		# $xaN is offloaded, remember?
	pxor		$xb0,$xt1
	pxor		$xc0,$xt2
	pxor		$xd0,$xt3

	 movdqu		$xt0,0x00($out)
	movdqu		0x40($inp),$xt0
	 movdqu		$xt1,0x10($out)
	movdqu		0x50($inp),$xt1
	 movdqu		$xt2,0x20($out)
	movdqu		0x60($inp),$xt2
	 movdqu		$xt3,0x30($out)
	movdqu		0x70($inp),$xt3
	lea		0x80($inp),$inp		# size optimization
	pxor		0x10(%rsp),$xt0
	pxor		$xb1,$xt1
	pxor		$xc1,$xt2
	pxor		$xd1,$xt3

	 movdqu		$xt0,0x40($out)
	movdqu		0x00($inp),$xt0
	 movdqu		$xt1,0x50($out)
	movdqu		0x10($inp),$xt1
	 movdqu		$xt2,0x60($out)
	movdqu		0x20($inp),$xt2
	 movdqu		$xt3,0x70($out)
	 lea		0x80($out),$out		# size optimization
	movdqu		0x30($inp),$xt3
	pxor		0x20(%rsp),$xt0
	pxor		$xb2,$xt1
	pxor		$xc2,$xt2
	pxor		$xd2,$xt3
	movdqu		$xt0,0x00($out)
	movdqu		$xt1,0x10($out)
	movdqu		$xt2,0x20($out)
	movdqu		$xt3,0x30($out)
	je		.Ldone4x

	movdqa		0x30(%rsp),$xt0		# $xaN is offloaded, remember?
	lea		0x40($inp),$inp		# inp+=64*3
	xor		%r10,%r10
	movdqa		$xt0,0x00(%rsp)
	movdqa		$xb3,0x10(%rsp)
	lea		0x40($out),$out		# out+=64*3
	movdqa		$xc3,0x20(%rsp)
	sub		\$192,$len		# len-=64*3
	movdqa		$xd3,0x30(%rsp)

.Loop_tail4x:
	movzb		($inp,%r10),%eax
	movzb		(%rsp,%r10),%ecx
	lea		1(%r10),%r10
	xor		%ecx,%eax
	mov		%al,-1($out,%r10)
	dec		$len
	jnz		.Loop_tail4x

.Ldone4x:
___
$code.=<<___	if ($win64);
	movaps		-0xa8(%r9),%xmm6
	movaps		-0x98(%r9),%xmm7
	movaps		-0x88(%r9),%xmm8
	movaps		-0x78(%r9),%xmm9
	movaps		-0x68(%r9),%xmm10
	movaps		-0x58(%r9),%xmm11
	movaps		-0x48(%r9),%xmm12
	movaps		-0x38(%r9),%xmm13
	movaps		-0x28(%r9),%xmm14
	movaps		-0x18(%r9),%xmm15
___
$code.=<<___;
	lea		(%r9),%rsp
.cfi_def_cfa_register	rsp
.L4x_epilogue:
	ret
.cfi_endproc
.size	ChaCha20_ctr32_ssse3_4x,.-ChaCha20_ctr32_ssse3_4x
___
}

########################################################################
# AVX2 code path
if ($avx>1) {
my ($xb0,$xb1,$xb2,$xb3, $xd0,$xd1,$xd2,$xd3,
    $xa0,$xa1,$xa2,$xa3, $xt0,$xt1,$xt2,$xt3)=map("%ymm$_",(0..15));
my @xx=($xa0,$xa1,$xa2,$xa3, $xb0,$xb1,$xb2,$xb3,
	"%nox","%nox","%nox","%nox", $xd0,$xd1,$xd2,$xd3);

sub AVX2_lane_ROUND {
my ($a0,$b0,$c0,$d0)=@_;
my ($a1,$b1,$c1,$d1)=map(($_&~3)+(($_+1)&3),($a0,$b0,$c0,$d0));
my ($a2,$b2,$c2,$d2)=map(($_&~3)+(($_+1)&3),($a1,$b1,$c1,$d1));
my ($a3,$b3,$c3,$d3)=map(($_&~3)+(($_+1)&3),($a2,$b2,$c2,$d2));
my ($xc,$xc_,$t0,$t1)=map("\"$_\"",$xt0,$xt1,$xt2,$xt3);
my @x=map("\"$_\"",@xx);

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
	# bunch of 'c' stores and loads in the middle, but none in
	# the beginning or end.

	(
	"&vpaddd	(@x[$a0],@x[$a0],@x[$b0])",	# Q1
	"&vpxor		(@x[$d0],@x[$a0],@x[$d0])",
	"&vpshufb	(@x[$d0],@x[$d0],$t1)",
	 "&vpaddd	(@x[$a1],@x[$a1],@x[$b1])",	# Q2
	 "&vpxor	(@x[$d1],@x[$a1],@x[$d1])",
	 "&vpshufb	(@x[$d1],@x[$d1],$t1)",

	"&vpaddd	($xc,$xc,@x[$d0])",
	"&vpxor		(@x[$b0],$xc,@x[$b0])",
	"&vpslld	($t0,@x[$b0],12)",
	"&vpsrld	(@x[$b0],@x[$b0],20)",
	"&vpor		(@x[$b0],$t0,@x[$b0])",
	"&vbroadcasti128($t0,'(%r11)')",		# .Lrot24(%rip)
	 "&vpaddd	($xc_,$xc_,@x[$d1])",
	 "&vpxor	(@x[$b1],$xc_,@x[$b1])",
	 "&vpslld	($t1,@x[$b1],12)",
	 "&vpsrld	(@x[$b1],@x[$b1],20)",
	 "&vpor		(@x[$b1],$t1,@x[$b1])",

	"&vpaddd	(@x[$a0],@x[$a0],@x[$b0])",
	"&vpxor		(@x[$d0],@x[$a0],@x[$d0])",
	"&vpshufb	(@x[$d0],@x[$d0],$t0)",
	 "&vpaddd	(@x[$a1],@x[$a1],@x[$b1])",
	 "&vpxor	(@x[$d1],@x[$a1],@x[$d1])",
	 "&vpshufb	(@x[$d1],@x[$d1],$t0)",

	"&vpaddd	($xc,$xc,@x[$d0])",
	"&vpxor		(@x[$b0],$xc,@x[$b0])",
	"&vpslld	($t1,@x[$b0],7)",
	"&vpsrld	(@x[$b0],@x[$b0],25)",
	"&vpor		(@x[$b0],$t1,@x[$b0])",
	"&vbroadcasti128($t1,'(%r10)')",		# .Lrot16(%rip)
	 "&vpaddd	($xc_,$xc_,@x[$d1])",
	 "&vpxor	(@x[$b1],$xc_,@x[$b1])",
	 "&vpslld	($t0,@x[$b1],7)",
	 "&vpsrld	(@x[$b1],@x[$b1],25)",
	 "&vpor		(@x[$b1],$t0,@x[$b1])",

	"&vmovdqa	(\"`32*($c0-8)`(%rsp)\",$xc)",	# reload pair of 'c's
	 "&vmovdqa	(\"`32*($c1-8)`(%rsp)\",$xc_)",
	"&vmovdqa	($xc,\"`32*($c2-8)`(%rsp)\")",
	 "&vmovdqa	($xc_,\"`32*($c3-8)`(%rsp)\")",

	"&vpaddd	(@x[$a2],@x[$a2],@x[$b2])",	# Q3
	"&vpxor		(@x[$d2],@x[$a2],@x[$d2])",
	"&vpshufb	(@x[$d2],@x[$d2],$t1)",
	 "&vpaddd	(@x[$a3],@x[$a3],@x[$b3])",	# Q4
	 "&vpxor	(@x[$d3],@x[$a3],@x[$d3])",
	 "&vpshufb	(@x[$d3],@x[$d3],$t1)",

	"&vpaddd	($xc,$xc,@x[$d2])",
	"&vpxor		(@x[$b2],$xc,@x[$b2])",
	"&vpslld	($t0,@x[$b2],12)",
	"&vpsrld	(@x[$b2],@x[$b2],20)",
	"&vpor		(@x[$b2],$t0,@x[$b2])",
	"&vbroadcasti128($t0,'(%r11)')",		# .Lrot24(%rip)
	 "&vpaddd	($xc_,$xc_,@x[$d3])",
	 "&vpxor	(@x[$b3],$xc_,@x[$b3])",
	 "&vpslld	($t1,@x[$b3],12)",
	 "&vpsrld	(@x[$b3],@x[$b3],20)",
	 "&vpor		(@x[$b3],$t1,@x[$b3])",

	"&vpaddd	(@x[$a2],@x[$a2],@x[$b2])",
	"&vpxor		(@x[$d2],@x[$a2],@x[$d2])",
	"&vpshufb	(@x[$d2],@x[$d2],$t0)",
	 "&vpaddd	(@x[$a3],@x[$a3],@x[$b3])",
	 "&vpxor	(@x[$d3],@x[$a3],@x[$d3])",
	 "&vpshufb	(@x[$d3],@x[$d3],$t0)",

	"&vpaddd	($xc,$xc,@x[$d2])",
	"&vpxor		(@x[$b2],$xc,@x[$b2])",
	"&vpslld	($t1,@x[$b2],7)",
	"&vpsrld	(@x[$b2],@x[$b2],25)",
	"&vpor		(@x[$b2],$t1,@x[$b2])",
	"&vbroadcasti128($t1,'(%r10)')",		# .Lrot16(%rip)
	 "&vpaddd	($xc_,$xc_,@x[$d3])",
	 "&vpxor	(@x[$b3],$xc_,@x[$b3])",
	 "&vpslld	($t0,@x[$b3],7)",
	 "&vpsrld	(@x[$b3],@x[$b3],25)",
	 "&vpor		(@x[$b3],$t0,@x[$b3])"
	);
}

my $xframe = $win64 ? 0xa8 : 8;

$code.=<<___;
.globl	ChaCha20_ctr32_avx2
.type	ChaCha20_ctr32_avx2,\@function,5
.align	32
ChaCha20_ctr32_avx2:
.cfi_startproc
	_CET_ENDBR
	mov		%rsp,%r9		# frame register
.cfi_def_cfa_register	r9
	sub		\$0x280+$xframe,%rsp
	and		\$-32,%rsp
___
$code.=<<___	if ($win64);
	movaps		%xmm6,-0xa8(%r9)
	movaps		%xmm7,-0x98(%r9)
	movaps		%xmm8,-0x88(%r9)
	movaps		%xmm9,-0x78(%r9)
	movaps		%xmm10,-0x68(%r9)
	movaps		%xmm11,-0x58(%r9)
	movaps		%xmm12,-0x48(%r9)
	movaps		%xmm13,-0x38(%r9)
	movaps		%xmm14,-0x28(%r9)
	movaps		%xmm15,-0x18(%r9)
.L8x_body:
___
$code.=<<___;
	vzeroupper

	################ stack layout
	# +0x00		SIMD equivalent of @x[8-12]
	# ...
	# +0x80		constant copy of key[0-2] smashed by lanes
	# ...
	# +0x200	SIMD counters (with nonce smashed by lanes)
	# ...
	# +0x280

	vbroadcasti128	.Lsigma(%rip),$xa3	# key[0]
	vbroadcasti128	($key),$xb3		# key[1]
	vbroadcasti128	16($key),$xt3		# key[2]
	vbroadcasti128	($counter),$xd3		# key[3]
	lea		0x100(%rsp),%rcx	# size optimization
	lea		0x200(%rsp),%rax	# size optimization
	lea		.Lrot16(%rip),%r10
	lea		.Lrot24(%rip),%r11

	vpshufd		\$0x00,$xa3,$xa0	# smash key by lanes...
	vpshufd		\$0x55,$xa3,$xa1
	vmovdqa		$xa0,0x80-0x100(%rcx)	# ... and offload
	vpshufd		\$0xaa,$xa3,$xa2
	vmovdqa		$xa1,0xa0-0x100(%rcx)
	vpshufd		\$0xff,$xa3,$xa3
	vmovdqa		$xa2,0xc0-0x100(%rcx)
	vmovdqa		$xa3,0xe0-0x100(%rcx)

	vpshufd		\$0x00,$xb3,$xb0
	vpshufd		\$0x55,$xb3,$xb1
	vmovdqa		$xb0,0x100-0x100(%rcx)
	vpshufd		\$0xaa,$xb3,$xb2
	vmovdqa		$xb1,0x120-0x100(%rcx)
	vpshufd		\$0xff,$xb3,$xb3
	vmovdqa		$xb2,0x140-0x100(%rcx)
	vmovdqa		$xb3,0x160-0x100(%rcx)

	vpshufd		\$0x00,$xt3,$xt0	# "xc0"
	vpshufd		\$0x55,$xt3,$xt1	# "xc1"
	vmovdqa		$xt0,0x180-0x200(%rax)
	vpshufd		\$0xaa,$xt3,$xt2	# "xc2"
	vmovdqa		$xt1,0x1a0-0x200(%rax)
	vpshufd		\$0xff,$xt3,$xt3	# "xc3"
	vmovdqa		$xt2,0x1c0-0x200(%rax)
	vmovdqa		$xt3,0x1e0-0x200(%rax)

	vpshufd		\$0x00,$xd3,$xd0
	vpshufd		\$0x55,$xd3,$xd1
	vpaddd		.Lincy(%rip),$xd0,$xd0	# don't save counters yet
	vpshufd		\$0xaa,$xd3,$xd2
	vmovdqa		$xd1,0x220-0x200(%rax)
	vpshufd		\$0xff,$xd3,$xd3
	vmovdqa		$xd2,0x240-0x200(%rax)
	vmovdqa		$xd3,0x260-0x200(%rax)

	jmp		.Loop_enter8x

.align	32
.Loop_outer8x:
	vmovdqa		0x80-0x100(%rcx),$xa0	# re-load smashed key
	vmovdqa		0xa0-0x100(%rcx),$xa1
	vmovdqa		0xc0-0x100(%rcx),$xa2
	vmovdqa		0xe0-0x100(%rcx),$xa3
	vmovdqa		0x100-0x100(%rcx),$xb0
	vmovdqa		0x120-0x100(%rcx),$xb1
	vmovdqa		0x140-0x100(%rcx),$xb2
	vmovdqa		0x160-0x100(%rcx),$xb3
	vmovdqa		0x180-0x200(%rax),$xt0	# "xc0"
	vmovdqa		0x1a0-0x200(%rax),$xt1	# "xc1"
	vmovdqa		0x1c0-0x200(%rax),$xt2	# "xc2"
	vmovdqa		0x1e0-0x200(%rax),$xt3	# "xc3"
	vmovdqa		0x200-0x200(%rax),$xd0
	vmovdqa		0x220-0x200(%rax),$xd1
	vmovdqa		0x240-0x200(%rax),$xd2
	vmovdqa		0x260-0x200(%rax),$xd3
	vpaddd		.Leight(%rip),$xd0,$xd0	# next SIMD counters

.Loop_enter8x:
	vmovdqa		$xt2,0x40(%rsp)		# SIMD equivalent of "@x[10]"
	vmovdqa		$xt3,0x60(%rsp)		# SIMD equivalent of "@x[11]"
	vbroadcasti128	(%r10),$xt3
	vmovdqa		$xd0,0x200-0x200(%rax)	# save SIMD counters
	mov		\$10,%eax
	jmp		.Loop8x

.align	32
.Loop8x:
___
	foreach (&AVX2_lane_ROUND(0, 4, 8,12)) { eval; }
	foreach (&AVX2_lane_ROUND(0, 5,10,15)) { eval; }
$code.=<<___;
	dec		%eax
	jnz		.Loop8x

	lea		0x200(%rsp),%rax	# size optimization
	vpaddd		0x80-0x100(%rcx),$xa0,$xa0	# accumulate key
	vpaddd		0xa0-0x100(%rcx),$xa1,$xa1
	vpaddd		0xc0-0x100(%rcx),$xa2,$xa2
	vpaddd		0xe0-0x100(%rcx),$xa3,$xa3

	vpunpckldq	$xa1,$xa0,$xt2		# "de-interlace" data
	vpunpckldq	$xa3,$xa2,$xt3
	vpunpckhdq	$xa1,$xa0,$xa0
	vpunpckhdq	$xa3,$xa2,$xa2
	vpunpcklqdq	$xt3,$xt2,$xa1		# "a0"
	vpunpckhqdq	$xt3,$xt2,$xt2		# "a1"
	vpunpcklqdq	$xa2,$xa0,$xa3		# "a2"
	vpunpckhqdq	$xa2,$xa0,$xa0		# "a3"
___
	($xa0,$xa1,$xa2,$xa3,$xt2)=($xa1,$xt2,$xa3,$xa0,$xa2);
$code.=<<___;
	vpaddd		0x100-0x100(%rcx),$xb0,$xb0
	vpaddd		0x120-0x100(%rcx),$xb1,$xb1
	vpaddd		0x140-0x100(%rcx),$xb2,$xb2
	vpaddd		0x160-0x100(%rcx),$xb3,$xb3

	vpunpckldq	$xb1,$xb0,$xt2
	vpunpckldq	$xb3,$xb2,$xt3
	vpunpckhdq	$xb1,$xb0,$xb0
	vpunpckhdq	$xb3,$xb2,$xb2
	vpunpcklqdq	$xt3,$xt2,$xb1		# "b0"
	vpunpckhqdq	$xt3,$xt2,$xt2		# "b1"
	vpunpcklqdq	$xb2,$xb0,$xb3		# "b2"
	vpunpckhqdq	$xb2,$xb0,$xb0		# "b3"
___
	($xb0,$xb1,$xb2,$xb3,$xt2)=($xb1,$xt2,$xb3,$xb0,$xb2);
$code.=<<___;
	vperm2i128	\$0x20,$xb0,$xa0,$xt3	# "de-interlace" further
	vperm2i128	\$0x31,$xb0,$xa0,$xb0
	vperm2i128	\$0x20,$xb1,$xa1,$xa0
	vperm2i128	\$0x31,$xb1,$xa1,$xb1
	vperm2i128	\$0x20,$xb2,$xa2,$xa1
	vperm2i128	\$0x31,$xb2,$xa2,$xb2
	vperm2i128	\$0x20,$xb3,$xa3,$xa2
	vperm2i128	\$0x31,$xb3,$xa3,$xb3
___
	($xa0,$xa1,$xa2,$xa3,$xt3)=($xt3,$xa0,$xa1,$xa2,$xa3);
	my ($xc0,$xc1,$xc2,$xc3)=($xt0,$xt1,$xa0,$xa1);
$code.=<<___;
	vmovdqa		$xa0,0x00(%rsp)		# offload $xaN
	vmovdqa		$xa1,0x20(%rsp)
	vmovdqa		0x40(%rsp),$xc2		# $xa0
	vmovdqa		0x60(%rsp),$xc3		# $xa1

	vpaddd		0x180-0x200(%rax),$xc0,$xc0
	vpaddd		0x1a0-0x200(%rax),$xc1,$xc1
	vpaddd		0x1c0-0x200(%rax),$xc2,$xc2
	vpaddd		0x1e0-0x200(%rax),$xc3,$xc3

	vpunpckldq	$xc1,$xc0,$xt2
	vpunpckldq	$xc3,$xc2,$xt3
	vpunpckhdq	$xc1,$xc0,$xc0
	vpunpckhdq	$xc3,$xc2,$xc2
	vpunpcklqdq	$xt3,$xt2,$xc1		# "c0"
	vpunpckhqdq	$xt3,$xt2,$xt2		# "c1"
	vpunpcklqdq	$xc2,$xc0,$xc3		# "c2"
	vpunpckhqdq	$xc2,$xc0,$xc0		# "c3"
___
	($xc0,$xc1,$xc2,$xc3,$xt2)=($xc1,$xt2,$xc3,$xc0,$xc2);
$code.=<<___;
	vpaddd		0x200-0x200(%rax),$xd0,$xd0
	vpaddd		0x220-0x200(%rax),$xd1,$xd1
	vpaddd		0x240-0x200(%rax),$xd2,$xd2
	vpaddd		0x260-0x200(%rax),$xd3,$xd3

	vpunpckldq	$xd1,$xd0,$xt2
	vpunpckldq	$xd3,$xd2,$xt3
	vpunpckhdq	$xd1,$xd0,$xd0
	vpunpckhdq	$xd3,$xd2,$xd2
	vpunpcklqdq	$xt3,$xt2,$xd1		# "d0"
	vpunpckhqdq	$xt3,$xt2,$xt2		# "d1"
	vpunpcklqdq	$xd2,$xd0,$xd3		# "d2"
	vpunpckhqdq	$xd2,$xd0,$xd0		# "d3"
___
	($xd0,$xd1,$xd2,$xd3,$xt2)=($xd1,$xt2,$xd3,$xd0,$xd2);
$code.=<<___;
	vperm2i128	\$0x20,$xd0,$xc0,$xt3	# "de-interlace" further
	vperm2i128	\$0x31,$xd0,$xc0,$xd0
	vperm2i128	\$0x20,$xd1,$xc1,$xc0
	vperm2i128	\$0x31,$xd1,$xc1,$xd1
	vperm2i128	\$0x20,$xd2,$xc2,$xc1
	vperm2i128	\$0x31,$xd2,$xc2,$xd2
	vperm2i128	\$0x20,$xd3,$xc3,$xc2
	vperm2i128	\$0x31,$xd3,$xc3,$xd3
___
	($xc0,$xc1,$xc2,$xc3,$xt3)=($xt3,$xc0,$xc1,$xc2,$xc3);
	($xb0,$xb1,$xb2,$xb3,$xc0,$xc1,$xc2,$xc3)=
	($xc0,$xc1,$xc2,$xc3,$xb0,$xb1,$xb2,$xb3);
	($xa0,$xa1)=($xt2,$xt3);
$code.=<<___;
	vmovdqa		0x00(%rsp),$xa0		# $xaN was offloaded, remember?
	vmovdqa		0x20(%rsp),$xa1

	cmp		\$64*8,$len
	jb		.Ltail8x

	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	lea		0x80($inp),$inp		# size optimization
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	lea		0x80($out),$out		# size optimization

	vpxor		0x00($inp),$xa1,$xa1
	vpxor		0x20($inp),$xb1,$xb1
	vpxor		0x40($inp),$xc1,$xc1
	vpxor		0x60($inp),$xd1,$xd1
	lea		0x80($inp),$inp		# size optimization
	vmovdqu		$xa1,0x00($out)
	vmovdqu		$xb1,0x20($out)
	vmovdqu		$xc1,0x40($out)
	vmovdqu		$xd1,0x60($out)
	lea		0x80($out),$out		# size optimization

	vpxor		0x00($inp),$xa2,$xa2
	vpxor		0x20($inp),$xb2,$xb2
	vpxor		0x40($inp),$xc2,$xc2
	vpxor		0x60($inp),$xd2,$xd2
	lea		0x80($inp),$inp		# size optimization
	vmovdqu		$xa2,0x00($out)
	vmovdqu		$xb2,0x20($out)
	vmovdqu		$xc2,0x40($out)
	vmovdqu		$xd2,0x60($out)
	lea		0x80($out),$out		# size optimization

	vpxor		0x00($inp),$xa3,$xa3
	vpxor		0x20($inp),$xb3,$xb3
	vpxor		0x40($inp),$xc3,$xc3
	vpxor		0x60($inp),$xd3,$xd3
	lea		0x80($inp),$inp		# size optimization
	vmovdqu		$xa3,0x00($out)
	vmovdqu		$xb3,0x20($out)
	vmovdqu		$xc3,0x40($out)
	vmovdqu		$xd3,0x60($out)
	lea		0x80($out),$out		# size optimization

	sub		\$64*8,$len
	jnz		.Loop_outer8x

	jmp		.Ldone8x

.Ltail8x:
	cmp		\$448,$len
	jae		.L448_or_more8x
	cmp		\$384,$len
	jae		.L384_or_more8x
	cmp		\$320,$len
	jae		.L320_or_more8x
	cmp		\$256,$len
	jae		.L256_or_more8x
	cmp		\$192,$len
	jae		.L192_or_more8x
	cmp		\$128,$len
	jae		.L128_or_more8x
	cmp		\$64,$len
	jae		.L64_or_more8x

	xor		%r10,%r10
	vmovdqa		$xa0,0x00(%rsp)
	vmovdqa		$xb0,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L64_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	je		.Ldone8x

	lea		0x40($inp),$inp		# inp+=64*1
	xor		%r10,%r10
	vmovdqa		$xc0,0x00(%rsp)
	lea		0x40($out),$out		# out+=64*1
	sub		\$64,$len		# len-=64*1
	vmovdqa		$xd0,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L128_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	je		.Ldone8x

	lea		0x80($inp),$inp		# inp+=64*2
	xor		%r10,%r10
	vmovdqa		$xa1,0x00(%rsp)
	lea		0x80($out),$out		# out+=64*2
	sub		\$128,$len		# len-=64*2
	vmovdqa		$xb1,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L192_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vpxor		0x80($inp),$xa1,$xa1
	vpxor		0xa0($inp),$xb1,$xb1
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	vmovdqu		$xa1,0x80($out)
	vmovdqu		$xb1,0xa0($out)
	je		.Ldone8x

	lea		0xc0($inp),$inp		# inp+=64*3
	xor		%r10,%r10
	vmovdqa		$xc1,0x00(%rsp)
	lea		0xc0($out),$out		# out+=64*3
	sub		\$192,$len		# len-=64*3
	vmovdqa		$xd1,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L256_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vpxor		0x80($inp),$xa1,$xa1
	vpxor		0xa0($inp),$xb1,$xb1
	vpxor		0xc0($inp),$xc1,$xc1
	vpxor		0xe0($inp),$xd1,$xd1
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	vmovdqu		$xa1,0x80($out)
	vmovdqu		$xb1,0xa0($out)
	vmovdqu		$xc1,0xc0($out)
	vmovdqu		$xd1,0xe0($out)
	je		.Ldone8x

	lea		0x100($inp),$inp	# inp+=64*4
	xor		%r10,%r10
	vmovdqa		$xa2,0x00(%rsp)
	lea		0x100($out),$out	# out+=64*4
	sub		\$256,$len		# len-=64*4
	vmovdqa		$xb2,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L320_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vpxor		0x80($inp),$xa1,$xa1
	vpxor		0xa0($inp),$xb1,$xb1
	vpxor		0xc0($inp),$xc1,$xc1
	vpxor		0xe0($inp),$xd1,$xd1
	vpxor		0x100($inp),$xa2,$xa2
	vpxor		0x120($inp),$xb2,$xb2
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	vmovdqu		$xa1,0x80($out)
	vmovdqu		$xb1,0xa0($out)
	vmovdqu		$xc1,0xc0($out)
	vmovdqu		$xd1,0xe0($out)
	vmovdqu		$xa2,0x100($out)
	vmovdqu		$xb2,0x120($out)
	je		.Ldone8x

	lea		0x140($inp),$inp	# inp+=64*5
	xor		%r10,%r10
	vmovdqa		$xc2,0x00(%rsp)
	lea		0x140($out),$out	# out+=64*5
	sub		\$320,$len		# len-=64*5
	vmovdqa		$xd2,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L384_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vpxor		0x80($inp),$xa1,$xa1
	vpxor		0xa0($inp),$xb1,$xb1
	vpxor		0xc0($inp),$xc1,$xc1
	vpxor		0xe0($inp),$xd1,$xd1
	vpxor		0x100($inp),$xa2,$xa2
	vpxor		0x120($inp),$xb2,$xb2
	vpxor		0x140($inp),$xc2,$xc2
	vpxor		0x160($inp),$xd2,$xd2
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	vmovdqu		$xa1,0x80($out)
	vmovdqu		$xb1,0xa0($out)
	vmovdqu		$xc1,0xc0($out)
	vmovdqu		$xd1,0xe0($out)
	vmovdqu		$xa2,0x100($out)
	vmovdqu		$xb2,0x120($out)
	vmovdqu		$xc2,0x140($out)
	vmovdqu		$xd2,0x160($out)
	je		.Ldone8x

	lea		0x180($inp),$inp	# inp+=64*6
	xor		%r10,%r10
	vmovdqa		$xa3,0x00(%rsp)
	lea		0x180($out),$out	# out+=64*6
	sub		\$384,$len		# len-=64*6
	vmovdqa		$xb3,0x20(%rsp)
	jmp		.Loop_tail8x

.align	32
.L448_or_more8x:
	vpxor		0x00($inp),$xa0,$xa0	# xor with input
	vpxor		0x20($inp),$xb0,$xb0
	vpxor		0x40($inp),$xc0,$xc0
	vpxor		0x60($inp),$xd0,$xd0
	vpxor		0x80($inp),$xa1,$xa1
	vpxor		0xa0($inp),$xb1,$xb1
	vpxor		0xc0($inp),$xc1,$xc1
	vpxor		0xe0($inp),$xd1,$xd1
	vpxor		0x100($inp),$xa2,$xa2
	vpxor		0x120($inp),$xb2,$xb2
	vpxor		0x140($inp),$xc2,$xc2
	vpxor		0x160($inp),$xd2,$xd2
	vpxor		0x180($inp),$xa3,$xa3
	vpxor		0x1a0($inp),$xb3,$xb3
	vmovdqu		$xa0,0x00($out)
	vmovdqu		$xb0,0x20($out)
	vmovdqu		$xc0,0x40($out)
	vmovdqu		$xd0,0x60($out)
	vmovdqu		$xa1,0x80($out)
	vmovdqu		$xb1,0xa0($out)
	vmovdqu		$xc1,0xc0($out)
	vmovdqu		$xd1,0xe0($out)
	vmovdqu		$xa2,0x100($out)
	vmovdqu		$xb2,0x120($out)
	vmovdqu		$xc2,0x140($out)
	vmovdqu		$xd2,0x160($out)
	vmovdqu		$xa3,0x180($out)
	vmovdqu		$xb3,0x1a0($out)
	je		.Ldone8x

	lea		0x1c0($inp),$inp	# inp+=64*7
	xor		%r10,%r10
	vmovdqa		$xc3,0x00(%rsp)
	lea		0x1c0($out),$out	# out+=64*7
	sub		\$448,$len		# len-=64*7
	vmovdqa		$xd3,0x20(%rsp)

.Loop_tail8x:
	movzb		($inp,%r10),%eax
	movzb		(%rsp,%r10),%ecx
	lea		1(%r10),%r10
	xor		%ecx,%eax
	mov		%al,-1($out,%r10)
	dec		$len
	jnz		.Loop_tail8x

.Ldone8x:
	vzeroall
___
$code.=<<___	if ($win64);
	movaps		-0xa8(%r9),%xmm6
	movaps		-0x98(%r9),%xmm7
	movaps		-0x88(%r9),%xmm8
	movaps		-0x78(%r9),%xmm9
	movaps		-0x68(%r9),%xmm10
	movaps		-0x58(%r9),%xmm11
	movaps		-0x48(%r9),%xmm12
	movaps		-0x38(%r9),%xmm13
	movaps		-0x28(%r9),%xmm14
	movaps		-0x18(%r9),%xmm15
___
$code.=<<___;
	lea		(%r9),%rsp
.cfi_def_cfa_register	rsp
.L8x_epilogue:
	ret
.cfi_endproc
.size	ChaCha20_ctr32_avx2,.-ChaCha20_ctr32_avx2
___
}

########################################################################
# AVX512 code paths were removed

# EXCEPTION_DISPOSITION handler (EXCEPTION_RECORD *rec,ULONG64 frame,
#		CONTEXT *context,DISPATCHER_CONTEXT *disp)
if ($win64) {
$rec="%rcx";
$frame="%rdx";
$context="%r8";
$disp="%r9";

$code.=<<___;
.extern	__imp_RtlVirtualUnwind
.type	se_handler,\@abi-omnipotent
.align	16
se_handler:
	push	%rsi
	push	%rdi
	push	%rbx
	push	%rbp
	push	%r12
	push	%r13
	push	%r14
	push	%r15
	pushfq
	sub	\$64,%rsp

	mov	120($context),%rax	# pull context->Rax
	mov	248($context),%rbx	# pull context->Rip

	mov	8($disp),%rsi		# disp->ImageBase
	mov	56($disp),%r11		# disp->HandlerData

	lea	.Lctr32_body(%rip),%r10
	cmp	%r10,%rbx		# context->Rip<.Lprologue
	jb	.Lcommon_seh_tail

	mov	152($context),%rax	# pull context->Rsp

	lea	.Lno_data(%rip),%r10	# epilogue label
	cmp	%r10,%rbx		# context->Rip>=.Lepilogue
	jae	.Lcommon_seh_tail

	lea	64+24+48(%rax),%rax

	mov	-8(%rax),%rbx
	mov	-16(%rax),%rbp
	mov	-24(%rax),%r12
	mov	-32(%rax),%r13
	mov	-40(%rax),%r14
	mov	-48(%rax),%r15
	mov	%rbx,144($context)	# restore context->Rbx
	mov	%rbp,160($context)	# restore context->Rbp
	mov	%r12,216($context)	# restore context->R12
	mov	%r13,224($context)	# restore context->R13
	mov	%r14,232($context)	# restore context->R14
	mov	%r15,240($context)	# restore context->R14

.Lcommon_seh_tail:
	mov	8(%rax),%rdi
	mov	16(%rax),%rsi
	mov	%rax,152($context)	# restore context->Rsp
	mov	%rsi,168($context)	# restore context->Rsi
	mov	%rdi,176($context)	# restore context->Rdi

	mov	40($disp),%rdi		# disp->ContextRecord
	mov	$context,%rsi		# context
	mov	\$154,%ecx		# sizeof(CONTEXT)
	.long	0xa548f3fc		# cld; rep movsq

	mov	$disp,%rsi
	xor	%rcx,%rcx		# arg1, UNW_FLAG_NHANDLER
	mov	8(%rsi),%rdx		# arg2, disp->ImageBase
	mov	0(%rsi),%r8		# arg3, disp->ControlPc
	mov	16(%rsi),%r9		# arg4, disp->FunctionEntry
	mov	40(%rsi),%r10		# disp->ContextRecord
	lea	56(%rsi),%r11		# &disp->HandlerData
	lea	24(%rsi),%r12		# &disp->EstablisherFrame
	mov	%r10,32(%rsp)		# arg5
	mov	%r11,40(%rsp)		# arg6
	mov	%r12,48(%rsp)		# arg7
	mov	%rcx,56(%rsp)		# arg8, (NULL)
	call	*__imp_RtlVirtualUnwind(%rip)

	mov	\$1,%eax		# ExceptionContinueSearch
	add	\$64,%rsp
	popfq
	pop	%r15
	pop	%r14
	pop	%r13
	pop	%r12
	pop	%rbp
	pop	%rbx
	pop	%rdi
	pop	%rsi
	ret
.size	se_handler,.-se_handler

.type	ssse3_handler,\@abi-omnipotent
.align	16
ssse3_handler:
	push	%rsi
	push	%rdi
	push	%rbx
	push	%rbp
	push	%r12
	push	%r13
	push	%r14
	push	%r15
	pushfq
	sub	\$64,%rsp

	mov	120($context),%rax	# pull context->Rax
	mov	248($context),%rbx	# pull context->Rip

	mov	8($disp),%rsi		# disp->ImageBase
	mov	56($disp),%r11		# disp->HandlerData

	mov	0(%r11),%r10d		# HandlerData[0]
	lea	(%rsi,%r10),%r10	# prologue label
	cmp	%r10,%rbx		# context->Rip<prologue label
	jb	.Lcommon_seh_tail

	mov	192($context),%rax	# pull context->R9

	mov	4(%r11),%r10d		# HandlerData[1]
	lea	(%rsi,%r10),%r10	# epilogue label
	cmp	%r10,%rbx		# context->Rip>=epilogue label
	jae	.Lcommon_seh_tail

	lea	-0x28(%rax),%rsi
	lea	512($context),%rdi	# &context.Xmm6
	mov	\$4,%ecx
	.long	0xa548f3fc		# cld; rep movsq

	jmp	.Lcommon_seh_tail
.size	ssse3_handler,.-ssse3_handler

.type	full_handler,\@abi-omnipotent
.align	16
full_handler:
	push	%rsi
	push	%rdi
	push	%rbx
	push	%rbp
	push	%r12
	push	%r13
	push	%r14
	push	%r15
	pushfq
	sub	\$64,%rsp

	mov	120($context),%rax	# pull context->Rax
	mov	248($context),%rbx	# pull context->Rip

	mov	8($disp),%rsi		# disp->ImageBase
	mov	56($disp),%r11		# disp->HandlerData

	mov	0(%r11),%r10d		# HandlerData[0]
	lea	(%rsi,%r10),%r10	# prologue label
	cmp	%r10,%rbx		# context->Rip<prologue label
	jb	.Lcommon_seh_tail

	mov	192($context),%rax	# pull context->R9

	mov	4(%r11),%r10d		# HandlerData[1]
	lea	(%rsi,%r10),%r10	# epilogue label
	cmp	%r10,%rbx		# context->Rip>=epilogue label
	jae	.Lcommon_seh_tail

	lea	-0xa8(%rax),%rsi
	lea	512($context),%rdi	# &context.Xmm6
	mov	\$20,%ecx
	.long	0xa548f3fc		# cld; rep movsq

	jmp	.Lcommon_seh_tail
.size	full_handler,.-full_handler

.section	.pdata
.align	4
	.rva	.LSEH_begin_ChaCha20_ctr32_nohw
	.rva	.LSEH_end_ChaCha20_ctr32_nohw
	.rva	.LSEH_info_ChaCha20_ctr32_nohw

	.rva	.LSEH_begin_ChaCha20_ctr32_ssse3_4x
	.rva	.LSEH_end_ChaCha20_ctr32_ssse3_4x
	.rva	.LSEH_info_ChaCha20_ctr32_ssse3_4x
___
$code.=<<___ if ($avx>1);
	.rva	.LSEH_begin_ChaCha20_ctr32_avx2
	.rva	.LSEH_end_ChaCha20_ctr32_avx2
	.rva	.LSEH_info_ChaCha20_ctr32_avx2
___
$code.=<<___;
.section	.xdata
.align	8
.LSEH_info_ChaCha20_ctr32_nohw:
	.byte	9,0,0,0
	.rva	se_handler

.LSEH_info_ChaCha20_ctr32_ssse3_4x:
	.byte	9,0,0,0
	.rva	full_handler
	.rva	.L4x_body,.L4x_epilogue
___
$code.=<<___ if ($avx>1);
.LSEH_info_ChaCha20_ctr32_avx2:
	.byte	9,0,0,0
	.rva	full_handler
	.rva	.L8x_body,.L8x_epilogue			# HandlerData[]
___
}

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/ge;

	s/%x#%[yz]/%x/g;	# "down-shift"

	print $_,"\n";
}

close STDOUT or die "error closing STDOUT: $!";
