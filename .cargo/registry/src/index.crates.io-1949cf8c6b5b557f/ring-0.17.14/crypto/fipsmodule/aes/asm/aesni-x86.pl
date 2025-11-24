#! /usr/bin/env perl
# Copyright 2009-2016 The OpenSSL Project Authors. All Rights Reserved.
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


# ====================================================================
# Written by Andy Polyakov <appro@openssl.org> for the OpenSSL
# project.
# ====================================================================
#
# This module implements support for Intel AES-NI extension. In
# OpenSSL context it's used with Intel engine, but can also be used as
# drop-in replacement for crypto/aes/asm/aes-586.pl [see below for
# details].
#
# Performance.
#
# To start with see corresponding paragraph in aesni-x86_64.pl...
# Instead of filling table similar to one found there I've chosen to
# summarize *comparison* results for raw ECB, CTR and CBC benchmarks.
# The simplified table below represents 32-bit performance relative
# to 64-bit one in every given point. Ratios vary for different
# encryption modes, therefore interval values.
#
#	16-byte     64-byte     256-byte    1-KB        8-KB
#	53-67%      67-84%      91-94%      95-98%      97-99.5%
#
# Lower ratios for smaller block sizes are perfectly understandable,
# because function call overhead is higher in 32-bit mode. Largest
# 8-KB block performance is virtually same: 32-bit code is less than
# 1% slower for ECB, CBC and CCM, and ~3% slower otherwise.

# January 2011
#
# See aesni-x86_64.pl for details. Unlike x86_64 version this module
# interleaves at most 6 aes[enc|dec] instructions, because there are
# not enough registers for 8x interleave [which should be optimal for
# Sandy Bridge]. Actually, performance results for 6x interleave
# factor presented in aesni-x86_64.pl (except for CTR) are for this
# module.

# April 2011
#
# Add aesni_xts_[en|de]crypt. Westmere spends 1.50 cycles processing
# one byte out of 8KB with 128-bit key, Sandy Bridge - 1.09.

# November 2015
#
# Add aesni_ocb_[en|de]crypt. [Removed in BoringSSL]

######################################################################
# Current large-block performance in cycles per byte processed with
# 128-bit key (less is better).
#
#		CBC en-/decrypt	CTR	XTS	ECB	OCB
# Westmere	3.77/1.37	1.37	1.52	1.27
# * Bridge	5.07/0.98	0.99	1.09	0.91	1.10
# Haswell	4.44/0.80	0.97	1.03	0.72	0.76
# Skylake	2.68/0.65	0.65	0.66	0.64	0.66
# Silvermont	5.77/3.56	3.67	4.03	3.46	4.03
# Goldmont	3.84/1.39	1.39	1.63	1.31	1.70
# Bulldozer	5.80/0.98	1.05	1.24	0.93	1.23

$PREFIX="aes_hw";	# if $PREFIX is set to "AES", the script
			# generates drop-in replacement for
			# crypto/aes/asm/aes-586.pl:-)
$AESNI_PREFIX="aes_hw";
$inline=1;		# inline _aesni_[en|de]crypt

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC,"${dir}","${dir}../../../perlasm");
require "x86asm.pl";

$output = pop;
open OUT,">$output";
*STDOUT=*OUT;

&asm_init($ARGV[0]);

&preprocessor_ifdef("BORINGSSL_DISPATCH_TEST")
&external_label("BORINGSSL_function_hit");
&preprocessor_endif();
&static_label("key_const");

if ($PREFIX eq $AESNI_PREFIX)	{ $movekey=\&movups; }
else			{ $movekey=\&movups; }

$len="eax";
$rounds="ecx";
$key="edx";
$inp="esi";
$out="edi";
$rounds_="ebx";	# backup copy for $rounds
$key_="ebp";	# backup copy for $key

$rndkey0="xmm0";
$rndkey1="xmm1";
$inout0="xmm2";
$inout1="xmm3";
$inout2="xmm4";
$inout3="xmm5";	$in1="xmm5";
$inout4="xmm6";	$in0="xmm6";
$inout5="xmm7";	$ivec="xmm7";

# AESNI extension
sub aeskeygenassist
{ my($dst,$src,$imm)=@_;
    if ("$dst:$src" =~ /xmm([0-7]):xmm([0-7])/)
    {	&data_byte(0x66,0x0f,0x3a,0xdf,0xc0|($1<<3)|$2,$imm);	}
}
sub aescommon
{ my($opcodelet,$dst,$src)=@_;
    if ("$dst:$src" =~ /xmm([0-7]):xmm([0-7])/)
    {	&data_byte(0x66,0x0f,0x38,$opcodelet,0xc0|($1<<3)|$2);}
}
sub aesimc	{ aescommon(0xdb,@_); }
sub aesenc	{ aescommon(0xdc,@_); }
sub aesenclast	{ aescommon(0xdd,@_); }

# Inline version of internal aesni_[en|de]crypt1
{ my $sn;
sub aesni_inline_generate1
{ my ($p,$inout,$ivec)=@_; $inout=$inout0 if (!defined($inout));
  $sn++;

    &$movekey		($rndkey0,&QWP(0,$key));
    &$movekey		($rndkey1,&QWP(16,$key));
    &xorps		($ivec,$rndkey0)	if (defined($ivec));
    &lea		($key,&DWP(32,$key));
    &xorps		($inout,$ivec)		if (defined($ivec));
    &xorps		($inout,$rndkey0)	if (!defined($ivec));
    &set_label("${p}1_loop_$sn");
	eval"&aes${p}	($inout,$rndkey1)";
	&dec		($rounds);
	&$movekey	($rndkey1,&QWP(0,$key));
	&lea		($key,&DWP(16,$key));
    &jnz		(&label("${p}1_loop_$sn"));
    eval"&aes${p}last	($inout,$rndkey1)";
}}

sub aesni_generate1	# fully unrolled loop
{ my ($p,$inout)=@_; $inout=$inout0 if (!defined($inout));

    &function_begin_B("_aesni_${p}rypt1");
	&movups		($rndkey0,&QWP(0,$key));
	&$movekey	($rndkey1,&QWP(0x10,$key));
	&xorps		($inout,$rndkey0);
	&$movekey	($rndkey0,&QWP(0x20,$key));
	&lea		($key,&DWP(0x30,$key));
	&cmp		($rounds,11);
	&jb		(&label("${p}128"));
	&lea		($key,&DWP(0x40,$key));
	# 192-bit key support was removed.
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(-0x40,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-0x30,$key));
	# 192-bit key support was removed.
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(-0x20,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-0x10,$key));
    &set_label("${p}128");
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(0x10,$key));
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0x20,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(0x30,$key));
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0x40,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(0x50,$key));
	eval"&aes${p}	($inout,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0x60,$key));
	eval"&aes${p}	($inout,$rndkey0)";
	&$movekey	($rndkey0,&QWP(0x70,$key));
	eval"&aes${p}	($inout,$rndkey1)";
    eval"&aes${p}last	($inout,$rndkey0)";
    &ret();
    &function_end_B("_aesni_${p}rypt1");
}


# _aesni_[en|de]cryptN are private interfaces, N denotes interleave
# factor. Why 3x subroutine were originally used in loops? Even though
# aes[enc|dec] latency was originally 6, it could be scheduled only
# every *2nd* cycle. Thus 3x interleave was the one providing optimal
# utilization, i.e. when subroutine's throughput is virtually same as
# of non-interleaved subroutine [for number of input blocks up to 3].
# This is why it originally made no sense to implement 2x subroutine.
# But times change and it became appropriate to spend extra 192 bytes
# on 2x subroutine on Atom Silvermont account. For processors that
# can schedule aes[enc|dec] every cycle optimal interleave factor
# equals to corresponding instructions latency. 8x is optimal for
# * Bridge, but it's unfeasible to accommodate such implementation
# in XMM registers addressable in 32-bit mode and therefore maximum
# of 6x is used instead...

sub aesni_generate2
{ my $p=shift;

    &function_begin_B("_aesni_${p}rypt2");
	&$movekey	($rndkey0,&QWP(0,$key));
	&shl		($rounds,4);
	&$movekey	($rndkey1,&QWP(16,$key));
	&xorps		($inout0,$rndkey0);
	&pxor		($inout1,$rndkey0);
	&$movekey	($rndkey0,&QWP(32,$key));
	&lea		($key,&DWP(32,$key,$rounds));
	&neg		($rounds);
	&add		($rounds,16);

    &set_label("${p}2_loop");
	eval"&aes${p}	($inout0,$rndkey1)";
	eval"&aes${p}	($inout1,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0,$key,$rounds));
	&add		($rounds,32);
	eval"&aes${p}	($inout0,$rndkey0)";
	eval"&aes${p}	($inout1,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-16,$key,$rounds));
	&jnz		(&label("${p}2_loop"));
    eval"&aes${p}	($inout0,$rndkey1)";
    eval"&aes${p}	($inout1,$rndkey1)";
    eval"&aes${p}last	($inout0,$rndkey0)";
    eval"&aes${p}last	($inout1,$rndkey0)";
    &ret();
    &function_end_B("_aesni_${p}rypt2");
}

sub aesni_generate3
{ my $p=shift;

    &function_begin_B("_aesni_${p}rypt3");
	&$movekey	($rndkey0,&QWP(0,$key));
	&shl		($rounds,4);
	&$movekey	($rndkey1,&QWP(16,$key));
	&xorps		($inout0,$rndkey0);
	&pxor		($inout1,$rndkey0);
	&pxor		($inout2,$rndkey0);
	&$movekey	($rndkey0,&QWP(32,$key));
	&lea		($key,&DWP(32,$key,$rounds));
	&neg		($rounds);
	&add		($rounds,16);

    &set_label("${p}3_loop");
	eval"&aes${p}	($inout0,$rndkey1)";
	eval"&aes${p}	($inout1,$rndkey1)";
	eval"&aes${p}	($inout2,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0,$key,$rounds));
	&add		($rounds,32);
	eval"&aes${p}	($inout0,$rndkey0)";
	eval"&aes${p}	($inout1,$rndkey0)";
	eval"&aes${p}	($inout2,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-16,$key,$rounds));
	&jnz		(&label("${p}3_loop"));
    eval"&aes${p}	($inout0,$rndkey1)";
    eval"&aes${p}	($inout1,$rndkey1)";
    eval"&aes${p}	($inout2,$rndkey1)";
    eval"&aes${p}last	($inout0,$rndkey0)";
    eval"&aes${p}last	($inout1,$rndkey0)";
    eval"&aes${p}last	($inout2,$rndkey0)";
    &ret();
    &function_end_B("_aesni_${p}rypt3");
}

# 4x interleave is implemented to improve small block performance,
# most notably [and naturally] 4 block by ~30%. One can argue that one
# should have implemented 5x as well, but improvement  would be <20%,
# so it's not worth it...
sub aesni_generate4
{ my $p=shift;

    &function_begin_B("_aesni_${p}rypt4");
	&$movekey	($rndkey0,&QWP(0,$key));
	&$movekey	($rndkey1,&QWP(16,$key));
	&shl		($rounds,4);
	&xorps		($inout0,$rndkey0);
	&pxor		($inout1,$rndkey0);
	&pxor		($inout2,$rndkey0);
	&pxor		($inout3,$rndkey0);
	&$movekey	($rndkey0,&QWP(32,$key));
	&lea		($key,&DWP(32,$key,$rounds));
	&neg		($rounds);
	&data_byte	(0x0f,0x1f,0x40,0x00);
	&add		($rounds,16);

    &set_label("${p}4_loop");
	eval"&aes${p}	($inout0,$rndkey1)";
	eval"&aes${p}	($inout1,$rndkey1)";
	eval"&aes${p}	($inout2,$rndkey1)";
	eval"&aes${p}	($inout3,$rndkey1)";
	&$movekey	($rndkey1,&QWP(0,$key,$rounds));
	&add		($rounds,32);
	eval"&aes${p}	($inout0,$rndkey0)";
	eval"&aes${p}	($inout1,$rndkey0)";
	eval"&aes${p}	($inout2,$rndkey0)";
	eval"&aes${p}	($inout3,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-16,$key,$rounds));
    &jnz		(&label("${p}4_loop"));

    eval"&aes${p}	($inout0,$rndkey1)";
    eval"&aes${p}	($inout1,$rndkey1)";
    eval"&aes${p}	($inout2,$rndkey1)";
    eval"&aes${p}	($inout3,$rndkey1)";
    eval"&aes${p}last	($inout0,$rndkey0)";
    eval"&aes${p}last	($inout1,$rndkey0)";
    eval"&aes${p}last	($inout2,$rndkey0)";
    eval"&aes${p}last	($inout3,$rndkey0)";
    &ret();
    &function_end_B("_aesni_${p}rypt4");
}

sub aesni_generate6
{ my $p=shift;

    &function_begin_B("_aesni_${p}rypt6");
    &static_label("_aesni_${p}rypt6_enter");
	&$movekey	($rndkey0,&QWP(0,$key));
	&shl		($rounds,4);
	&$movekey	($rndkey1,&QWP(16,$key));
	&xorps		($inout0,$rndkey0);
	&pxor		($inout1,$rndkey0);	# pxor does better here
	&pxor		($inout2,$rndkey0);
	eval"&aes${p}	($inout0,$rndkey1)";
	&pxor		($inout3,$rndkey0);
	&pxor		($inout4,$rndkey0);
	eval"&aes${p}	($inout1,$rndkey1)";
	&lea		($key,&DWP(32,$key,$rounds));
	&neg		($rounds);
	eval"&aes${p}	($inout2,$rndkey1)";
	&pxor		($inout5,$rndkey0);
	&$movekey	($rndkey0,&QWP(0,$key,$rounds));
	&add		($rounds,16);
	&jmp		(&label("_aesni_${p}rypt6_inner"));

    &set_label("${p}6_loop",16);
	eval"&aes${p}	($inout0,$rndkey1)";
	eval"&aes${p}	($inout1,$rndkey1)";
	eval"&aes${p}	($inout2,$rndkey1)";
    &set_label("_aesni_${p}rypt6_inner");
	eval"&aes${p}	($inout3,$rndkey1)";
	eval"&aes${p}	($inout4,$rndkey1)";
	eval"&aes${p}	($inout5,$rndkey1)";
    &set_label("_aesni_${p}rypt6_enter");
	&$movekey	($rndkey1,&QWP(0,$key,$rounds));
	&add		($rounds,32);
	eval"&aes${p}	($inout0,$rndkey0)";
	eval"&aes${p}	($inout1,$rndkey0)";
	eval"&aes${p}	($inout2,$rndkey0)";
	eval"&aes${p}	($inout3,$rndkey0)";
	eval"&aes${p}	($inout4,$rndkey0)";
	eval"&aes${p}	($inout5,$rndkey0)";
	&$movekey	($rndkey0,&QWP(-16,$key,$rounds));
    &jnz		(&label("${p}6_loop"));

    eval"&aes${p}	($inout0,$rndkey1)";
    eval"&aes${p}	($inout1,$rndkey1)";
    eval"&aes${p}	($inout2,$rndkey1)";
    eval"&aes${p}	($inout3,$rndkey1)";
    eval"&aes${p}	($inout4,$rndkey1)";
    eval"&aes${p}	($inout5,$rndkey1)";
    eval"&aes${p}last	($inout0,$rndkey0)";
    eval"&aes${p}last	($inout1,$rndkey0)";
    eval"&aes${p}last	($inout2,$rndkey0)";
    eval"&aes${p}last	($inout3,$rndkey0)";
    eval"&aes${p}last	($inout4,$rndkey0)";
    eval"&aes${p}last	($inout5,$rndkey0)";
    &ret();
    &function_end_B("_aesni_${p}rypt6");
}
&aesni_generate2("enc") if ($PREFIX eq $AESNI_PREFIX);
&aesni_generate3("enc") if ($PREFIX eq $AESNI_PREFIX);
&aesni_generate4("enc") if ($PREFIX eq $AESNI_PREFIX);
&aesni_generate6("enc") if ($PREFIX eq $AESNI_PREFIX);

if ($PREFIX eq $AESNI_PREFIX) {

######################################################################
# void aes_hw_ctr32_encrypt_blocks (const void *in, void *out,
#                         size_t blocks, const AES_KEY *key,
#                         const char *ivec);
#
# Handles only complete blocks, operates on 32-bit counter and
# does not update *ivec! (see crypto/modes/ctr128.c for details)
#
# stack layout:
#	0	pshufb mask
#	16	vector addend: 0,6,6,6
# 	32	counter-less ivec
#	48	1st triplet of counter vector
#	64	2nd triplet of counter vector
#	80	saved %esp

&function_begin("${PREFIX}_ctr32_encrypt_blocks");
	&record_function_hit(0);

	&mov	($inp,&wparam(0));
	&mov	($out,&wparam(1));
	&mov	($len,&wparam(2));
	&mov	($key,&wparam(3));
	&mov	($rounds_,&wparam(4));
	&mov	($key_,"esp");
	&sub	("esp",88);
	&and	("esp",-16);			# align stack
	&mov	(&DWP(80,"esp"),$key_);

	&cmp	($len,1);
	&je	(&label("ctr32_one_shortcut"));

	&movdqu	($inout5,&QWP(0,$rounds_));	# load ivec

	# compose byte-swap control mask for pshufb on stack
	&mov	(&DWP(0,"esp"),0x0c0d0e0f);
	&mov	(&DWP(4,"esp"),0x08090a0b);
	&mov	(&DWP(8,"esp"),0x04050607);
	&mov	(&DWP(12,"esp"),0x00010203);

	# compose counter increment vector on stack
	&mov	($rounds,6);
	&xor	($key_,$key_);
	&mov	(&DWP(16,"esp"),$rounds);
	&mov	(&DWP(20,"esp"),$rounds);
	&mov	(&DWP(24,"esp"),$rounds);
	&mov	(&DWP(28,"esp"),$key_);

	&pextrd	($rounds_,$inout5,3);		# pull 32-bit counter
	&pinsrd	($inout5,$key_,3);		# wipe 32-bit counter

	&mov	($rounds,&DWP(240,$key));	# key->rounds

	# compose 2 vectors of 3x32-bit counters
	&bswap	($rounds_);
	&pxor	($rndkey0,$rndkey0);
	&pxor	($rndkey1,$rndkey1);
	&movdqa	($inout0,&QWP(0,"esp"));	# load byte-swap mask
	&pinsrd	($rndkey0,$rounds_,0);
	&lea	($key_,&DWP(3,$rounds_));
	&pinsrd	($rndkey1,$key_,0);
	&inc	($rounds_);
	&pinsrd	($rndkey0,$rounds_,1);
	&inc	($key_);
	&pinsrd	($rndkey1,$key_,1);
	&inc	($rounds_);
	&pinsrd	($rndkey0,$rounds_,2);
	&inc	($key_);
	&pinsrd	($rndkey1,$key_,2);
	&movdqa	(&QWP(48,"esp"),$rndkey0);	# save 1st triplet
	&pshufb	($rndkey0,$inout0);		# byte swap
	&movdqu	($inout4,&QWP(0,$key));		# key[0]
	&movdqa	(&QWP(64,"esp"),$rndkey1);	# save 2nd triplet
	&pshufb	($rndkey1,$inout0);		# byte swap

	&pshufd	($inout0,$rndkey0,3<<6);	# place counter to upper dword
	&pshufd	($inout1,$rndkey0,2<<6);
	&cmp	($len,6);
	&jb	(&label("ctr32_tail"));
	&pxor	($inout5,$inout4);		# counter-less ivec^key[0]
	&shl	($rounds,4);
	&mov	($rounds_,16);
	&movdqa	(&QWP(32,"esp"),$inout5);	# save counter-less ivec^key[0]
	&mov	($key_,$key);			# backup $key
	&sub	($rounds_,$rounds);		# backup twisted $rounds
	&lea	($key,&DWP(32,$key,$rounds));
	&sub	($len,6);
	&jmp	(&label("ctr32_loop6"));

&set_label("ctr32_loop6",16);
	# inlining _aesni_encrypt6's prologue gives ~6% improvement...
	&pshufd	($inout2,$rndkey0,1<<6);
	&movdqa	($rndkey0,&QWP(32,"esp"));	# pull counter-less ivec
	&pshufd	($inout3,$rndkey1,3<<6);
	&pxor		($inout0,$rndkey0);	# merge counter-less ivec
	&pshufd	($inout4,$rndkey1,2<<6);
	&pxor		($inout1,$rndkey0);
	&pshufd	($inout5,$rndkey1,1<<6);
	&$movekey	($rndkey1,&QWP(16,$key_));
	&pxor		($inout2,$rndkey0);
	&pxor		($inout3,$rndkey0);
	&aesenc		($inout0,$rndkey1);
	&pxor		($inout4,$rndkey0);
	&pxor		($inout5,$rndkey0);
	&aesenc		($inout1,$rndkey1);
	&$movekey	($rndkey0,&QWP(32,$key_));
	&mov		($rounds,$rounds_);
	&aesenc		($inout2,$rndkey1);
	&aesenc		($inout3,$rndkey1);
	&aesenc		($inout4,$rndkey1);
	&aesenc		($inout5,$rndkey1);

	&call		(&label("_aesni_encrypt6_enter"));

	&movups	($rndkey1,&QWP(0,$inp));
	&movups	($rndkey0,&QWP(0x10,$inp));
	&xorps	($inout0,$rndkey1);
	&movups	($rndkey1,&QWP(0x20,$inp));
	&xorps	($inout1,$rndkey0);
	&movups	(&QWP(0,$out),$inout0);
	&movdqa	($rndkey0,&QWP(16,"esp"));	# load increment
	&xorps	($inout2,$rndkey1);
	&movdqa	($rndkey1,&QWP(64,"esp"));	# load 2nd triplet
	&movups	(&QWP(0x10,$out),$inout1);
	&movups	(&QWP(0x20,$out),$inout2);

	&paddd	($rndkey1,$rndkey0);		# 2nd triplet increment
	&paddd	($rndkey0,&QWP(48,"esp"));	# 1st triplet increment
	&movdqa	($inout0,&QWP(0,"esp"));	# load byte swap mask

	&movups	($inout1,&QWP(0x30,$inp));
	&movups	($inout2,&QWP(0x40,$inp));
	&xorps	($inout3,$inout1);
	&movups	($inout1,&QWP(0x50,$inp));
	&lea	($inp,&DWP(0x60,$inp));
	&movdqa	(&QWP(48,"esp"),$rndkey0);	# save 1st triplet
	&pshufb	($rndkey0,$inout0);		# byte swap
	&xorps	($inout4,$inout2);
	&movups	(&QWP(0x30,$out),$inout3);
	&xorps	($inout5,$inout1);
	&movdqa	(&QWP(64,"esp"),$rndkey1);	# save 2nd triplet
	&pshufb	($rndkey1,$inout0);		# byte swap
	&movups	(&QWP(0x40,$out),$inout4);
	&pshufd	($inout0,$rndkey0,3<<6);
	&movups	(&QWP(0x50,$out),$inout5);
	&lea	($out,&DWP(0x60,$out));

	&pshufd	($inout1,$rndkey0,2<<6);
	&sub	($len,6);
	&jnc	(&label("ctr32_loop6"));

	&add	($len,6);
	&jz	(&label("ctr32_ret"));
	&movdqu	($inout5,&QWP(0,$key_));
	&mov	($key,$key_);
	&pxor	($inout5,&QWP(32,"esp"));	# restore count-less ivec
	&mov	($rounds,&DWP(240,$key_));	# restore $rounds

&set_label("ctr32_tail");
	&por	($inout0,$inout5);
	&cmp	($len,2);
	&jb	(&label("ctr32_one"));

	&pshufd	($inout2,$rndkey0,1<<6);
	&por	($inout1,$inout5);
	&je	(&label("ctr32_two"));

	&pshufd	($inout3,$rndkey1,3<<6);
	&por	($inout2,$inout5);
	&cmp	($len,4);
	&jb	(&label("ctr32_three"));

	&pshufd	($inout4,$rndkey1,2<<6);
	&por	($inout3,$inout5);
	&je	(&label("ctr32_four"));

	&por	($inout4,$inout5);
	&call	("_aesni_encrypt6");
	&movups	($rndkey1,&QWP(0,$inp));
	&movups	($rndkey0,&QWP(0x10,$inp));
	&xorps	($inout0,$rndkey1);
	&movups	($rndkey1,&QWP(0x20,$inp));
	&xorps	($inout1,$rndkey0);
	&movups	($rndkey0,&QWP(0x30,$inp));
	&xorps	($inout2,$rndkey1);
	&movups	($rndkey1,&QWP(0x40,$inp));
	&xorps	($inout3,$rndkey0);
	&movups	(&QWP(0,$out),$inout0);
	&xorps	($inout4,$rndkey1);
	&movups	(&QWP(0x10,$out),$inout1);
	&movups	(&QWP(0x20,$out),$inout2);
	&movups	(&QWP(0x30,$out),$inout3);
	&movups	(&QWP(0x40,$out),$inout4);
	&jmp	(&label("ctr32_ret"));

&set_label("ctr32_one_shortcut",16);
	&movups	($inout0,&QWP(0,$rounds_));	# load ivec
	&mov	($rounds,&DWP(240,$key));

&set_label("ctr32_one");
	if ($inline)
	{   &aesni_inline_generate1("enc");	}
	else
	{   &call	("_aesni_encrypt1");	}
	&movups	($in0,&QWP(0,$inp));
	&xorps	($in0,$inout0);
	&movups	(&QWP(0,$out),$in0);
	&jmp	(&label("ctr32_ret"));

&set_label("ctr32_two",16);
	&call	("_aesni_encrypt2");
	&movups	($inout3,&QWP(0,$inp));
	&movups	($inout4,&QWP(0x10,$inp));
	&xorps	($inout0,$inout3);
	&xorps	($inout1,$inout4);
	&movups	(&QWP(0,$out),$inout0);
	&movups	(&QWP(0x10,$out),$inout1);
	&jmp	(&label("ctr32_ret"));

&set_label("ctr32_three",16);
	&call	("_aesni_encrypt3");
	&movups	($inout3,&QWP(0,$inp));
	&movups	($inout4,&QWP(0x10,$inp));
	&xorps	($inout0,$inout3);
	&movups	($inout5,&QWP(0x20,$inp));
	&xorps	($inout1,$inout4);
	&movups	(&QWP(0,$out),$inout0);
	&xorps	($inout2,$inout5);
	&movups	(&QWP(0x10,$out),$inout1);
	&movups	(&QWP(0x20,$out),$inout2);
	&jmp	(&label("ctr32_ret"));

&set_label("ctr32_four",16);
	&call	("_aesni_encrypt4");
	&movups	($inout4,&QWP(0,$inp));
	&movups	($inout5,&QWP(0x10,$inp));
	&movups	($rndkey1,&QWP(0x20,$inp));
	&xorps	($inout0,$inout4);
	&movups	($rndkey0,&QWP(0x30,$inp));
	&xorps	($inout1,$inout5);
	&movups	(&QWP(0,$out),$inout0);
	&xorps	($inout2,$rndkey1);
	&movups	(&QWP(0x10,$out),$inout1);
	&xorps	($inout3,$rndkey0);
	&movups	(&QWP(0x20,$out),$inout2);
	&movups	(&QWP(0x30,$out),$inout3);

&set_label("ctr32_ret");
	&pxor	("xmm0","xmm0");		# clear register bank
	&pxor	("xmm1","xmm1");
	&pxor	("xmm2","xmm2");
	&pxor	("xmm3","xmm3");
	&pxor	("xmm4","xmm4");
	&movdqa	(&QWP(32,"esp"),"xmm0");	# clear stack
	&pxor	("xmm5","xmm5");
	&movdqa	(&QWP(48,"esp"),"xmm0");
	&pxor	("xmm6","xmm6");
	&movdqa	(&QWP(64,"esp"),"xmm0");
	&pxor	("xmm7","xmm7");
	&mov	("esp",&DWP(80,"esp"));
&function_end("${PREFIX}_ctr32_encrypt_blocks");
}

######################################################################
# Mechanical port from aesni-x86_64.pl.

# int $PREFIX_set_encrypt_key_base (const unsigned char *userKey, int bits,
#                                   AES_KEY *key)
&function_begin_B("${PREFIX}_set_encrypt_key_base");
	&record_function_hit(3);

	&mov	("eax",&wparam(0));
	&mov	($rounds,&wparam(1));
	&mov	($key,&wparam(2));
	&push	("ebx");

	&call	(&label("pic"));
&set_label("pic");
	&blindpop("ebx");
	&lea	("ebx",&DWP(&label("key_const")."-".&label("pic"),"ebx"));

	&movups	("xmm0",&QWP(0,"eax"));	# pull first 128 bits of *userKey
	&xorps	("xmm4","xmm4");	# low dword of xmm4 is assumed 0
	&lea	($key,&DWP(16,$key));
	&cmp	($rounds,256);
	&je	(&label("14rounds"));
	# 192-bit key support was removed.
	&cmp	($rounds,128);
	&jne	(&label("bad_keybits"));

&set_label("10rounds",16);
	&mov		($rounds,9);
	&$movekey	(&QWP(-16,$key),"xmm0");	# round 0
	&aeskeygenassist("xmm1","xmm0",0x01);		# round 1
	&call		(&label("key_128_cold"));
	&aeskeygenassist("xmm1","xmm0",0x2);		# round 2
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x04);		# round 3
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x08);		# round 4
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x10);		# round 5
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x20);		# round 6
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x40);		# round 7
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x80);		# round 8
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x1b);		# round 9
	&call		(&label("key_128"));
	&aeskeygenassist("xmm1","xmm0",0x36);		# round 10
	&call		(&label("key_128"));
	&$movekey	(&QWP(0,$key),"xmm0");
	&mov		(&DWP(80,$key),$rounds);

	&jmp	(&label("good_key"));

&set_label("key_128",16);
	&$movekey	(&QWP(0,$key),"xmm0");
	&lea		($key,&DWP(16,$key));
&set_label("key_128_cold");
	&shufps		("xmm4","xmm0",0b00010000);
	&xorps		("xmm0","xmm4");
	&shufps		("xmm4","xmm0",0b10001100);
	&xorps		("xmm0","xmm4");
	&shufps		("xmm1","xmm1",0b11111111);	# critical path
	&xorps		("xmm0","xmm1");
	&ret();

&set_label("14rounds",16);
	&movups		("xmm2",&QWP(16,"eax"));	# remaining half of *userKey
	&lea		($key,&DWP(16,$key));

	&mov		($rounds,13);
	&$movekey	(&QWP(-32,$key),"xmm0");	# round 0
	&$movekey	(&QWP(-16,$key),"xmm2");	# round 1
	&aeskeygenassist("xmm1","xmm2",0x01);		# round 2
	&call		(&label("key_256a_cold"));
	&aeskeygenassist("xmm1","xmm0",0x01);		# round 3
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x02);		# round 4
	&call		(&label("key_256a"));
	&aeskeygenassist("xmm1","xmm0",0x02);		# round 5
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x04);		# round 6
	&call		(&label("key_256a"));
	&aeskeygenassist("xmm1","xmm0",0x04);		# round 7
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x08);		# round 8
	&call		(&label("key_256a"));
	&aeskeygenassist("xmm1","xmm0",0x08);		# round 9
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x10);		# round 10
	&call		(&label("key_256a"));
	&aeskeygenassist("xmm1","xmm0",0x10);		# round 11
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x20);		# round 12
	&call		(&label("key_256a"));
	&aeskeygenassist("xmm1","xmm0",0x20);		# round 13
	&call		(&label("key_256b"));
	&aeskeygenassist("xmm1","xmm2",0x40);		# round 14
	&call		(&label("key_256a"));
	&$movekey	(&QWP(0,$key),"xmm0");
	&mov		(&DWP(16,$key),$rounds);
	&xor		("eax","eax");

	&jmp	(&label("good_key"));

&set_label("key_256a",16);
	&$movekey	(&QWP(0,$key),"xmm2");
	&lea		($key,&DWP(16,$key));
&set_label("key_256a_cold");
	&shufps		("xmm4","xmm0",0b00010000);
	&xorps		("xmm0","xmm4");
	&shufps		("xmm4","xmm0",0b10001100);
	&xorps		("xmm0","xmm4");
	&shufps		("xmm1","xmm1",0b11111111);	# critical path
	&xorps		("xmm0","xmm1");
	&ret();

&set_label("key_256b",16);
	&$movekey	(&QWP(0,$key),"xmm0");
	&lea		($key,&DWP(16,$key));

	&shufps		("xmm4","xmm2",0b00010000);
	&xorps		("xmm2","xmm4");
	&shufps		("xmm4","xmm2",0b10001100);
	&xorps		("xmm2","xmm4");
	&shufps		("xmm1","xmm1",0b10101010);	# critical path
	&xorps		("xmm2","xmm1");
	&ret();

&set_label("good_key");
	&pxor	("xmm0","xmm0");
	&pxor	("xmm1","xmm1");
	&pxor	("xmm2","xmm2");
	&pxor	("xmm3","xmm3");
	&pxor	("xmm4","xmm4");
	&pxor	("xmm5","xmm5");
	&xor	("eax","eax");
	&pop	("ebx");
	&ret	();

&set_label("bad_keybits",4);
	&pxor	("xmm0","xmm0");
	&mov	("eax",-2);
	&pop	("ebx");
	&ret	();
&function_end_B("${PREFIX}_set_encrypt_key_base");

# int $PREFIX_set_encrypt_key_alt (const unsigned char *userKey, int bits,
#                                  AES_KEY *key)
&function_begin_B("${PREFIX}_set_encrypt_key_alt");
	&record_function_hit(3);

	&mov	("eax",&wparam(0));
	&mov	($rounds,&wparam(1));
	&mov	($key,&wparam(2));
	&push	("ebx");

	&call	(&label("pic"));
&set_label("pic");
	&blindpop("ebx");
	&lea	("ebx",&DWP(&label("key_const")."-".&label("pic"),"ebx"));

	&movups	("xmm0",&QWP(0,"eax"));	# pull first 128 bits of *userKey
	&xorps	("xmm4","xmm4");	# low dword of xmm4 is assumed 0
	&lea	($key,&DWP(16,$key));
	&cmp	($rounds,256);
	&je	(&label("14rounds_alt"));
	# 192-bit key support was removed.
	&cmp	($rounds,128);
	&jne	(&label("bad_keybits"));

&set_label("10rounds_alt",16);
	&movdqa		("xmm5",&QWP(0x00,"ebx"));
	&mov		($rounds,8);
	&movdqa		("xmm4",&QWP(0x20,"ebx"));
	&movdqa		("xmm2","xmm0");
	&movdqu		(&QWP(-16,$key),"xmm0");

&set_label("loop_key128");
	&pshufb		("xmm0","xmm5");
	&aesenclast	("xmm0","xmm4");
	&pslld		("xmm4",1);
	&lea		($key,&DWP(16,$key));

	&movdqa		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm2","xmm3");

	&pxor		("xmm0","xmm2");
	&movdqu		(&QWP(-16,$key),"xmm0");
	&movdqa		("xmm2","xmm0");

	&dec		($rounds);
	&jnz		(&label("loop_key128"));

	&movdqa		("xmm4",&QWP(0x30,"ebx"));

	&pshufb		("xmm0","xmm5");
	&aesenclast	("xmm0","xmm4");
	&pslld		("xmm4",1);

	&movdqa		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm2","xmm3");

	&pxor		("xmm0","xmm2");
	&movdqu		(&QWP(0,$key),"xmm0");

	&movdqa		("xmm2","xmm0");
	&pshufb		("xmm0","xmm5");
	&aesenclast	("xmm0","xmm4");

	&movdqa		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm3","xmm2");
	&pslldq		("xmm2",4);
	&pxor		("xmm2","xmm3");

	&pxor		("xmm0","xmm2");
	&movdqu		(&QWP(16,$key),"xmm0");

	&mov		($rounds,9);
	&mov		(&DWP(96,$key),$rounds);

	&jmp	(&label("good_key"));

	# 192-bit key support was removed.

&set_label("14rounds_alt",16);
	&movups		("xmm2",&QWP(16,"eax"));	# remaining half of *userKey
	&lea		($key,&DWP(16,$key));
	&movdqa		("xmm5",&QWP(0x00,"ebx"));
	&movdqa		("xmm4",&QWP(0x20,"ebx"));
	&mov		($rounds,7);
	&movdqu		(&QWP(-32,$key),"xmm0");
	&movdqa		("xmm1","xmm2");
	&movdqu		(&QWP(-16,$key),"xmm2");

&set_label("loop_key256");
	&pshufb		("xmm2","xmm5");
	&aesenclast	("xmm2","xmm4");

	&movdqa		("xmm3","xmm0");
	&pslldq		("xmm0",4);
	&pxor		("xmm3","xmm0");
	&pslldq		("xmm0",4);
	&pxor		("xmm3","xmm0");
	&pslldq		("xmm0",4);
	&pxor		("xmm0","xmm3");
	&pslld		("xmm4",1);

	&pxor		("xmm0","xmm2");
	&movdqu		(&QWP(0,$key),"xmm0");

	&dec		($rounds);
	&jz		(&label("done_key256"));

	&pshufd		("xmm2","xmm0",0xff);
	&pxor		("xmm3","xmm3");
	&aesenclast	("xmm2","xmm3");

	&movdqa		("xmm3","xmm1");
	&pslldq		("xmm1",4);
	&pxor		("xmm3","xmm1");
	&pslldq		("xmm1",4);
	&pxor		("xmm3","xmm1");
	&pslldq		("xmm1",4);
	&pxor		("xmm1","xmm3");

	&pxor		("xmm2","xmm1");
	&movdqu		(&QWP(16,$key),"xmm2");
	&lea		($key,&DWP(32,$key));
	&movdqa		("xmm1","xmm2");
	&jmp		(&label("loop_key256"));

&set_label("done_key256");
	&mov		($rounds,13);
	&mov		(&DWP(16,$key),$rounds);

&set_label("good_key");
	&pxor	("xmm0","xmm0");
	&pxor	("xmm1","xmm1");
	&pxor	("xmm2","xmm2");
	&pxor	("xmm3","xmm3");
	&pxor	("xmm4","xmm4");
	&pxor	("xmm5","xmm5");
	&xor	("eax","eax");
	&pop	("ebx");
	&ret	();

&set_label("bad_keybits",4);
	&pxor	("xmm0","xmm0");
	&mov	("eax",-2);
	&pop	("ebx");
	&ret	();
&function_end_B("${PREFIX}_set_encrypt_key_alt");


&set_label("key_const",64);
&data_word(0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d,0x0c0f0e0d);
&data_word(0x04070605,0x04070605,0x04070605,0x04070605);
&data_word(1,1,1,1);
&data_word(0x1b,0x1b,0x1b,0x1b);
&asciz("AES for Intel AES-NI, CRYPTOGAMS by <appro\@openssl.org>");

&asm_finish();

close STDOUT or die "error closing STDOUT: $!";
