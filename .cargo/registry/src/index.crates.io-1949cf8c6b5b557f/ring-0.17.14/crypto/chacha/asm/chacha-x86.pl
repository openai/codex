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
# January 2015
#
# ChaCha20 for x86.
#
# Performance in cycles per byte out of large buffer.
#
#		1xIALU/gcc	4xSSSE3
# Pentium	17.5/+80%
# PIII		14.2/+60%
# P4		18.6/+84%
# Core2		9.56/+89%	4.83
# Westmere	9.50/+45%	3.35
# Sandy Bridge	10.5/+47%	3.20
# Haswell	8.15/+50%	2.83
# Skylake	7.53/+22%	2.75
# Silvermont	17.4/+36%	8.35
# Goldmont	13.4/+40%	4.36
# Sledgehammer	10.2/+54%
# Bulldozer	13.4/+50%	4.38(*)
#
# (*)	Bulldozer actually executes 4xXOP code path that delivers 3.55;
#
# Modified from upstream OpenSSL to remove the XOP code.

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC,"${dir}","${dir}../../perlasm");
require "x86asm.pl";

$output=pop;
open STDOUT,">$output";

&asm_init($ARGV[0]);

$xmm=$ymm=1;
$gasver=999;  # enable everything

$a="eax";
($b,$b_)=("ebx","ebp");
($c,$c_)=("ecx","esi");
($d,$d_)=("edx","edi");

&static_label("ssse3_data");
&static_label("pic_point");

if ($xmm) {
my ($xa,$xa_,$xb,$xb_,$xc,$xc_,$xd,$xd_)=map("xmm$_",(0..7));
my ($out,$inp,$len)=("edi","esi","ecx");

sub QUARTERROUND_SSSE3 {
my ($ai,$bi,$ci,$di,$i)=@_;
my ($an,$bn,$cn,$dn)=map(($_&~3)+(($_+1)&3),($ai,$bi,$ci,$di));	# next
my ($ap,$bp,$cp,$dp)=map(($_&~3)+(($_-1)&3),($ai,$bi,$ci,$di));	# previous

	#       a   b   c   d
	#
	#       0   4   8  12 < even round
	#       1   5   9  13
	#       2   6  10  14
	#       3   7  11  15
	#       0   5  10  15 < odd round
	#       1   6  11  12
	#       2   7   8  13
	#       3   4   9  14

	if ($i==0) {
            my $j=4;
	    ($ap,$bp,$cp,$dp)=map(($_&~3)+(($_-$j--)&3),($ap,$bp,$cp,$dp));
	} elsif ($i==3) {
            my $j=0;
	    ($an,$bn,$cn,$dn)=map(($_&~3)+(($_+$j++)&3),($an,$bn,$cn,$dn));
	} elsif ($i==4) {
            my $j=4;
	    ($ap,$bp,$cp,$dp)=map(($_&~3)+(($_+$j--)&3),($ap,$bp,$cp,$dp));
	} elsif ($i==7) {
            my $j=0;
	    ($an,$bn,$cn,$dn)=map(($_&~3)+(($_-$j++)&3),($an,$bn,$cn,$dn));
	}

	#&paddd	($xa,$xb);			# see elsewhere
	#&pxor	($xd,$xa);			# see elsewhere
	 &movdqa(&QWP(16*$cp-128,"ebx"),$xc_)	if ($ai>0 && $ai<3);
	&pshufb	($xd,&QWP(0,"eax"));		# rot16
	 &movdqa(&QWP(16*$bp-128,"ebx"),$xb_)	if ($i!=0);
	&paddd	($xc,$xd);
	 &movdqa($xc_,&QWP(16*$cn-128,"ebx"))	if ($ai>0 && $ai<3);
	&pxor	($xb,$xc);
	 &movdqa($xb_,&QWP(16*$bn-128,"ebx"))	if ($i<7);
	&movdqa	($xa_,$xb);			# borrow as temporary
	&pslld	($xb,12);
	&psrld	($xa_,20);
	&por	($xb,$xa_);
	 &movdqa($xa_,&QWP(16*$an-128,"ebx"));
	&paddd	($xa,$xb);
	 &movdqa($xd_,&QWP(16*$dn-128,"ebx"))	if ($di!=$dn);
	&pxor	($xd,$xa);
	&movdqa	(&QWP(16*$ai-128,"ebx"),$xa);
	&pshufb	($xd,&QWP(16,"eax"));		# rot8
	&paddd	($xc,$xd);
	&movdqa	(&QWP(16*$di-128,"ebx"),$xd)	if ($di!=$dn);
	&movdqa	($xd_,$xd)			if ($di==$dn);
	&pxor	($xb,$xc);
	 &paddd	($xa_,$xb_)			if ($i<7);	# elsewhere
	&movdqa	($xa,$xb);			# borrow as temporary
	&pslld	($xb,7);
	&psrld	($xa,25);
	 &pxor	($xd_,$xa_)			if ($i<7);	# elsewhere
	&por	($xb,$xa);

	($xa,$xa_)=($xa_,$xa);
	($xb,$xb_)=($xb_,$xb);
	($xc,$xc_)=($xc_,$xc);
	($xd,$xd_)=($xd_,$xd);
}

&function_begin("ChaCha20_ctr32_ssse3");
	&call	(&label("pic_point"));
&set_label("pic_point");
	&blindpop("eax");

	&mov		($out,&wparam(0));
	&mov		($inp,&wparam(1));
	&mov		($len,&wparam(2));
	&mov		("edx",&wparam(3));		# key
	&mov		("ebx",&wparam(4));		# counter and nonce

	&mov		("ebp","esp");
	&stack_push	(131);
	&and		("esp",-64);
	&mov		(&DWP(512,"esp"),"ebp");

	&lea		("eax",&DWP(&label("ssse3_data")."-".
				    &label("pic_point"),"eax"));
	&movdqu		("xmm3",&QWP(0,"ebx"));		# counter and nonce

if (defined($gasver) && $gasver>=2.17) {		# even though we encode
							# pshufb manually, we
							# handle only register
							# operands, while this
							# segment uses memory
							# operand...
	&cmp		($len,64*4);
	&jb		(&label("1x"));

	&mov		(&DWP(512+4,"esp"),"edx");	# offload pointers
	&mov		(&DWP(512+8,"esp"),"ebx");
	&sub		($len,64*4);			# bias len
	&lea		("ebp",&DWP(256+128,"esp"));	# size optimization

	&movdqu		("xmm7",&QWP(0,"edx"));		# key
	&pshufd		("xmm0","xmm3",0x00);
	&pshufd		("xmm1","xmm3",0x55);
	&pshufd		("xmm2","xmm3",0xaa);
	&pshufd		("xmm3","xmm3",0xff);
	 &paddd		("xmm0",&QWP(16*3,"eax"));	# fix counters
	&pshufd		("xmm4","xmm7",0x00);
	&pshufd		("xmm5","xmm7",0x55);
	 &psubd		("xmm0",&QWP(16*4,"eax"));
	&pshufd		("xmm6","xmm7",0xaa);
	&pshufd		("xmm7","xmm7",0xff);
	&movdqa		(&QWP(16*12-128,"ebp"),"xmm0");
	&movdqa		(&QWP(16*13-128,"ebp"),"xmm1");
	&movdqa		(&QWP(16*14-128,"ebp"),"xmm2");
	&movdqa		(&QWP(16*15-128,"ebp"),"xmm3");
	 &movdqu	("xmm3",&QWP(16,"edx"));	# key
	&movdqa		(&QWP(16*4-128,"ebp"),"xmm4");
	&movdqa		(&QWP(16*5-128,"ebp"),"xmm5");
	&movdqa		(&QWP(16*6-128,"ebp"),"xmm6");
	&movdqa		(&QWP(16*7-128,"ebp"),"xmm7");
	 &movdqa	("xmm7",&QWP(16*2,"eax"));	# sigma
	 &lea		("ebx",&DWP(128,"esp"));	# size optimization

	&pshufd		("xmm0","xmm3",0x00);
	&pshufd		("xmm1","xmm3",0x55);
	&pshufd		("xmm2","xmm3",0xaa);
	&pshufd		("xmm3","xmm3",0xff);
	&pshufd		("xmm4","xmm7",0x00);
	&pshufd		("xmm5","xmm7",0x55);
	&pshufd		("xmm6","xmm7",0xaa);
	&pshufd		("xmm7","xmm7",0xff);
	&movdqa		(&QWP(16*8-128,"ebp"),"xmm0");
	&movdqa		(&QWP(16*9-128,"ebp"),"xmm1");
	&movdqa		(&QWP(16*10-128,"ebp"),"xmm2");
	&movdqa		(&QWP(16*11-128,"ebp"),"xmm3");
	&movdqa		(&QWP(16*0-128,"ebp"),"xmm4");
	&movdqa		(&QWP(16*1-128,"ebp"),"xmm5");
	&movdqa		(&QWP(16*2-128,"ebp"),"xmm6");
	&movdqa		(&QWP(16*3-128,"ebp"),"xmm7");

	&lea		($inp,&DWP(128,$inp));		# size optimization
	&lea		($out,&DWP(128,$out));		# size optimization
	&jmp		(&label("outer_loop"));

&set_label("outer_loop",16);
	#&movdqa	("xmm0",&QWP(16*0-128,"ebp"));	# copy key material
	&movdqa		("xmm1",&QWP(16*1-128,"ebp"));
	&movdqa		("xmm2",&QWP(16*2-128,"ebp"));
	&movdqa		("xmm3",&QWP(16*3-128,"ebp"));
	#&movdqa	("xmm4",&QWP(16*4-128,"ebp"));
	&movdqa		("xmm5",&QWP(16*5-128,"ebp"));
	&movdqa		("xmm6",&QWP(16*6-128,"ebp"));
	&movdqa		("xmm7",&QWP(16*7-128,"ebp"));
	#&movdqa	(&QWP(16*0-128,"ebx"),"xmm0");
	&movdqa		(&QWP(16*1-128,"ebx"),"xmm1");
	&movdqa		(&QWP(16*2-128,"ebx"),"xmm2");
	&movdqa		(&QWP(16*3-128,"ebx"),"xmm3");
	#&movdqa	(&QWP(16*4-128,"ebx"),"xmm4");
	&movdqa		(&QWP(16*5-128,"ebx"),"xmm5");
	&movdqa		(&QWP(16*6-128,"ebx"),"xmm6");
	&movdqa		(&QWP(16*7-128,"ebx"),"xmm7");
	#&movdqa	("xmm0",&QWP(16*8-128,"ebp"));
	#&movdqa	("xmm1",&QWP(16*9-128,"ebp"));
	&movdqa		("xmm2",&QWP(16*10-128,"ebp"));
	&movdqa		("xmm3",&QWP(16*11-128,"ebp"));
	&movdqa		("xmm4",&QWP(16*12-128,"ebp"));
	&movdqa		("xmm5",&QWP(16*13-128,"ebp"));
	&movdqa		("xmm6",&QWP(16*14-128,"ebp"));
	&movdqa		("xmm7",&QWP(16*15-128,"ebp"));
	&paddd		("xmm4",&QWP(16*4,"eax"));	# counter value
	#&movdqa	(&QWP(16*8-128,"ebx"),"xmm0");
	#&movdqa	(&QWP(16*9-128,"ebx"),"xmm1");
	&movdqa		(&QWP(16*10-128,"ebx"),"xmm2");
	&movdqa		(&QWP(16*11-128,"ebx"),"xmm3");
	&movdqa		(&QWP(16*12-128,"ebx"),"xmm4");
	&movdqa		(&QWP(16*13-128,"ebx"),"xmm5");
	&movdqa		(&QWP(16*14-128,"ebx"),"xmm6");
	&movdqa		(&QWP(16*15-128,"ebx"),"xmm7");
	&movdqa		(&QWP(16*12-128,"ebp"),"xmm4");	# save counter value

	&movdqa		($xa, &QWP(16*0-128,"ebp"));
	&movdqa		($xd, "xmm4");
	&movdqa		($xb_,&QWP(16*4-128,"ebp"));
	&movdqa		($xc, &QWP(16*8-128,"ebp"));
	&movdqa		($xc_,&QWP(16*9-128,"ebp"));

	&mov		("edx",10);			# loop counter
	&nop		();

&set_label("loop",16);
	&paddd		($xa,$xb_);			# elsewhere
	&movdqa		($xb,$xb_);
	&pxor		($xd,$xa);			# elsewhere
	&QUARTERROUND_SSSE3(0, 4, 8, 12, 0);
	&QUARTERROUND_SSSE3(1, 5, 9, 13, 1);
	&QUARTERROUND_SSSE3(2, 6,10, 14, 2);
	&QUARTERROUND_SSSE3(3, 7,11, 15, 3);
	&QUARTERROUND_SSSE3(0, 5,10, 15, 4);
	&QUARTERROUND_SSSE3(1, 6,11, 12, 5);
	&QUARTERROUND_SSSE3(2, 7, 8, 13, 6);
	&QUARTERROUND_SSSE3(3, 4, 9, 14, 7);
	&dec		("edx");
	&jnz		(&label("loop"));

	&movdqa		(&QWP(16*4-128,"ebx"),$xb_);
	&movdqa		(&QWP(16*8-128,"ebx"),$xc);
	&movdqa		(&QWP(16*9-128,"ebx"),$xc_);
	&movdqa		(&QWP(16*12-128,"ebx"),$xd);
	&movdqa		(&QWP(16*14-128,"ebx"),$xd_);

    my ($xa0,$xa1,$xa2,$xa3,$xt0,$xt1,$xt2,$xt3)=map("xmm$_",(0..7));

	#&movdqa	($xa0,&QWP(16*0-128,"ebx"));	# it's there
	&movdqa		($xa1,&QWP(16*1-128,"ebx"));
	&movdqa		($xa2,&QWP(16*2-128,"ebx"));
	&movdqa		($xa3,&QWP(16*3-128,"ebx"));

    for($i=0;$i<256;$i+=64) {
	&paddd		($xa0,&QWP($i+16*0-128,"ebp"));	# accumulate key material
	&paddd		($xa1,&QWP($i+16*1-128,"ebp"));
	&paddd		($xa2,&QWP($i+16*2-128,"ebp"));
	&paddd		($xa3,&QWP($i+16*3-128,"ebp"));

	&movdqa		($xt2,$xa0);		# "de-interlace" data
	&punpckldq	($xa0,$xa1);
	&movdqa		($xt3,$xa2);
	&punpckldq	($xa2,$xa3);
	&punpckhdq	($xt2,$xa1);
	&punpckhdq	($xt3,$xa3);
	&movdqa		($xa1,$xa0);
	&punpcklqdq	($xa0,$xa2);		# "a0"
	&movdqa		($xa3,$xt2);
	&punpcklqdq	($xt2,$xt3);		# "a2"
	&punpckhqdq	($xa1,$xa2);		# "a1"
	&punpckhqdq	($xa3,$xt3);		# "a3"

	#($xa2,$xt2)=($xt2,$xa2);

	&movdqu		($xt0,&QWP(64*0-128,$inp));	# load input
	&movdqu		($xt1,&QWP(64*1-128,$inp));
	&movdqu		($xa2,&QWP(64*2-128,$inp));
	&movdqu		($xt3,&QWP(64*3-128,$inp));
	&lea		($inp,&QWP($i<192?16:(64*4-16*3),$inp));
	&pxor		($xt0,$xa0);
	&movdqa		($xa0,&QWP($i+16*4-128,"ebx"))	if ($i<192);
	&pxor		($xt1,$xa1);
	&movdqa		($xa1,&QWP($i+16*5-128,"ebx"))	if ($i<192);
	&pxor		($xt2,$xa2);
	&movdqa		($xa2,&QWP($i+16*6-128,"ebx"))	if ($i<192);
	&pxor		($xt3,$xa3);
	&movdqa		($xa3,&QWP($i+16*7-128,"ebx"))	if ($i<192);
	&movdqu		(&QWP(64*0-128,$out),$xt0);	# store output
	&movdqu		(&QWP(64*1-128,$out),$xt1);
	&movdqu		(&QWP(64*2-128,$out),$xt2);
	&movdqu		(&QWP(64*3-128,$out),$xt3);
	&lea		($out,&QWP($i<192?16:(64*4-16*3),$out));
    }
	&sub		($len,64*4);
	&jnc		(&label("outer_loop"));

	&add		($len,64*4);
	&jz		(&label("done"));

	&mov		("ebx",&DWP(512+8,"esp"));	# restore pointers
	&lea		($inp,&DWP(-128,$inp));
	&mov		("edx",&DWP(512+4,"esp"));
	&lea		($out,&DWP(-128,$out));

	&movd		("xmm2",&DWP(16*12-128,"ebp"));	# counter value
	&movdqu		("xmm3",&QWP(0,"ebx"));
	&paddd		("xmm2",&QWP(16*6,"eax"));	# +four
	&pand		("xmm3",&QWP(16*7,"eax"));
	&por		("xmm3","xmm2");		# counter value
}
{
my ($a,$b,$c,$d,$t,$t1,$rot16,$rot24)=map("xmm$_",(0..7));

sub SSSE3ROUND {	# critical path is 20 "SIMD ticks" per round
	&paddd		($a,$b);
	&pxor		($d,$a);
	&pshufb		($d,$rot16);

	&paddd		($c,$d);
	&pxor		($b,$c);
	&movdqa		($t,$b);
	&psrld		($b,20);
	&pslld		($t,12);
	&por		($b,$t);

	&paddd		($a,$b);
	&pxor		($d,$a);
	&pshufb		($d,$rot24);

	&paddd		($c,$d);
	&pxor		($b,$c);
	&movdqa		($t,$b);
	&psrld		($b,25);
	&pslld		($t,7);
	&por		($b,$t);
}

&set_label("1x");
	&movdqa		($a,&QWP(16*2,"eax"));		# sigma
	&movdqu		($b,&QWP(0,"edx"));
	&movdqu		($c,&QWP(16,"edx"));
	#&movdqu	($d,&QWP(0,"ebx"));		# already loaded
	&movdqa		($rot16,&QWP(0,"eax"));
	&movdqa		($rot24,&QWP(16,"eax"));
	&mov		(&DWP(16*3,"esp"),"ebp");

	&movdqa		(&QWP(16*0,"esp"),$a);
	&movdqa		(&QWP(16*1,"esp"),$b);
	&movdqa		(&QWP(16*2,"esp"),$c);
	&movdqa		(&QWP(16*3,"esp"),$d);
	&mov		("edx",10);
	&jmp		(&label("loop1x"));

&set_label("outer1x",16);
	&movdqa		($d,&QWP(16*5,"eax"));		# one
	&movdqa		($a,&QWP(16*0,"esp"));
	&movdqa		($b,&QWP(16*1,"esp"));
	&movdqa		($c,&QWP(16*2,"esp"));
	&paddd		($d,&QWP(16*3,"esp"));
	&mov		("edx",10);
	&movdqa		(&QWP(16*3,"esp"),$d);
	&jmp		(&label("loop1x"));

&set_label("loop1x",16);
	&SSSE3ROUND();
	&pshufd	($c,$c,0b01001110);
	&pshufd	($b,$b,0b00111001);
	&pshufd	($d,$d,0b10010011);
	&nop	();

	&SSSE3ROUND();
	&pshufd	($c,$c,0b01001110);
	&pshufd	($b,$b,0b10010011);
	&pshufd	($d,$d,0b00111001);

	&dec		("edx");
	&jnz		(&label("loop1x"));

	&paddd		($a,&QWP(16*0,"esp"));
	&paddd		($b,&QWP(16*1,"esp"));
	&paddd		($c,&QWP(16*2,"esp"));
	&paddd		($d,&QWP(16*3,"esp"));

	&cmp		($len,64);
	&jb		(&label("tail"));

	&movdqu		($t,&QWP(16*0,$inp));
	&movdqu		($t1,&QWP(16*1,$inp));
	&pxor		($a,$t);		# xor with input
	&movdqu		($t,&QWP(16*2,$inp));
	&pxor		($b,$t1);
	&movdqu		($t1,&QWP(16*3,$inp));
	&pxor		($c,$t);
	&pxor		($d,$t1);
	&lea		($inp,&DWP(16*4,$inp));	# inp+=64

	&movdqu		(&QWP(16*0,$out),$a);	# write output
	&movdqu		(&QWP(16*1,$out),$b);
	&movdqu		(&QWP(16*2,$out),$c);
	&movdqu		(&QWP(16*3,$out),$d);
	&lea		($out,&DWP(16*4,$out));	# inp+=64

	&sub		($len,64);
	&jnz		(&label("outer1x"));

	&jmp		(&label("done"));

&set_label("tail");
	&movdqa		(&QWP(16*0,"esp"),$a);
	&movdqa		(&QWP(16*1,"esp"),$b);
	&movdqa		(&QWP(16*2,"esp"),$c);
	&movdqa		(&QWP(16*3,"esp"),$d);

	&xor		("eax","eax");
	&xor		("edx","edx");
	&xor		("ebp","ebp");

&set_label("tail_loop");
	&movb		("al",&BP(0,"esp","ebp"));
	&movb		("dl",&BP(0,$inp,"ebp"));
	&lea		("ebp",&DWP(1,"ebp"));
	&xor		("al","dl");
	&movb		(&BP(-1,$out,"ebp"),"al");
	&dec		($len);
	&jnz		(&label("tail_loop"));
}
&set_label("done");
	&mov		("esp",&DWP(512,"esp"));
&function_end("ChaCha20_ctr32_ssse3");

&align	(64);
&set_label("ssse3_data");
&data_byte(0x2,0x3,0x0,0x1, 0x6,0x7,0x4,0x5, 0xa,0xb,0x8,0x9, 0xe,0xf,0xc,0xd);
&data_byte(0x3,0x0,0x1,0x2, 0x7,0x4,0x5,0x6, 0xb,0x8,0x9,0xa, 0xf,0xc,0xd,0xe);
&data_word(0x61707865,0x3320646e,0x79622d32,0x6b206574);
&data_word(0,1,2,3);
&data_word(4,4,4,4);
&data_word(1,0,0,0);
&data_word(4,0,0,0);
&data_word(0,-1,-1,-1);
&align	(64);
}
&asciz	("ChaCha20 for x86, CRYPTOGAMS by <appro\@openssl.org>");

&asm_finish();

close STDOUT or die "error closing STDOUT: $!";
