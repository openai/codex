#! /usr/bin/env perl
# Copyright 2011-2016 The OpenSSL Project Authors. All Rights Reserved.
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


######################################################################
## Constant-time SSSE3 AES core implementation.
## version 0.1
##
## By Mike Hamburg (Stanford University), 2009
## Public domain.
##
## For details see http://shiftleft.org/papers/vector_aes/ and
## http://crypto.stanford.edu/vpaes/.

######################################################################
# September 2011.
#
# Port vpaes-x86_64.pl as 32-bit "almost" drop-in replacement for
# aes-586.pl. "Almost" refers to the fact that AES_cbc_encrypt
# doesn't handle partial vectors (doesn't have to if called from
# EVP only). "Drop-in" implies that this module doesn't share key
# schedule structure with the original nor does it make assumption
# about its alignment...
#
# Performance summary. aes-586.pl column lists large-block CBC
# encrypt/decrypt/with-hyper-threading-off(*) results in cycles per
# byte processed with 128-bit key, and vpaes-x86.pl column - [also
# large-block CBC] encrypt/decrypt.
#
#		aes-586.pl		vpaes-x86.pl
#
# Core 2(**)	28.1/41.4/18.3		21.9/25.2(***)
# Nehalem	27.9/40.4/18.1		10.2/11.9
# Atom		70.7/92.1/60.1		61.1/75.4(***)
# Silvermont	45.4/62.9/24.1		49.2/61.1(***)
#
# (*)	"Hyper-threading" in the context refers rather to cache shared
#	among multiple cores, than to specifically Intel HTT. As vast
#	majority of contemporary cores share cache, slower code path
#	is common place. In other words "with-hyper-threading-off"
#	results are presented mostly for reference purposes.
#
# (**)	"Core 2" refers to initial 65nm design, a.k.a. Conroe.
#
# (***)	Less impressive improvement on Core 2 and Atom is due to slow
#	pshufb,	yet it's respectable +28%/64%  improvement on Core 2
#	and +15% on Atom (as implied, over "hyper-threading-safe"
#	code path).
#
#						<appro@openssl.org>

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
push(@INC,"${dir}","${dir}../../../perlasm");
require "x86asm.pl";

$output = pop;
open OUT,">$output";
*STDOUT=*OUT;

&asm_init($ARGV[0]);

$PREFIX="vpaes";

my  ($round, $base, $magic, $key, $const, $inp, $out)=
    ("eax",  "ebx", "ecx",  "edx","ebp",  "esi","edi");

&preprocessor_ifdef("BORINGSSL_DISPATCH_TEST")
&external_label("BORINGSSL_function_hit");
&preprocessor_endif();
&static_label("_vpaes_consts");
&static_label("_vpaes_schedule_low_round");

&set_label("_vpaes_consts",64);
$k_inv=-0x30;		# inv, inva
	&data_word(0x0D080180,0x0E05060F,0x0A0B0C02,0x04070309);
	&data_word(0x0F0B0780,0x01040A06,0x02050809,0x030D0E0C);

$k_s0F=-0x10;		# s0F
	&data_word(0x0F0F0F0F,0x0F0F0F0F,0x0F0F0F0F,0x0F0F0F0F);

$k_ipt=0x00;		# input transform (lo, hi)
	&data_word(0x5A2A7000,0xC2B2E898,0x52227808,0xCABAE090);
	&data_word(0x317C4D00,0x4C01307D,0xB0FDCC81,0xCD80B1FC);

$k_sb1=0x20;		# sb1u, sb1t
	&data_word(0xCB503E00,0xB19BE18F,0x142AF544,0xA5DF7A6E);
	&data_word(0xFAE22300,0x3618D415,0x0D2ED9EF,0x3BF7CCC1);
$k_sb2=0x40;		# sb2u, sb2t
	&data_word(0x0B712400,0xE27A93C6,0xBC982FCD,0x5EB7E955);
	&data_word(0x0AE12900,0x69EB8840,0xAB82234A,0xC2A163C8);
$k_sbo=0x60;		# sbou, sbot
	&data_word(0x6FBDC700,0xD0D26D17,0xC502A878,0x15AABF7A);
	&data_word(0x5FBB6A00,0xCFE474A5,0x412B35FA,0x8E1E90D1);

$k_mc_forward=0x80;	# mc_forward
	&data_word(0x00030201,0x04070605,0x080B0A09,0x0C0F0E0D);
	&data_word(0x04070605,0x080B0A09,0x0C0F0E0D,0x00030201);
	&data_word(0x080B0A09,0x0C0F0E0D,0x00030201,0x04070605);
	&data_word(0x0C0F0E0D,0x00030201,0x04070605,0x080B0A09);

$k_mc_backward=0xc0;	# mc_backward
	&data_word(0x02010003,0x06050407,0x0A09080B,0x0E0D0C0F);
	&data_word(0x0E0D0C0F,0x02010003,0x06050407,0x0A09080B);
	&data_word(0x0A09080B,0x0E0D0C0F,0x02010003,0x06050407);
	&data_word(0x06050407,0x0A09080B,0x0E0D0C0F,0x02010003);

$k_sr=0x100;		# sr
	&data_word(0x03020100,0x07060504,0x0B0A0908,0x0F0E0D0C);
	&data_word(0x0F0A0500,0x030E0904,0x07020D08,0x0B06010C);
	&data_word(0x0B020900,0x0F060D04,0x030A0108,0x070E050C);
	&data_word(0x070A0D00,0x0B0E0104,0x0F020508,0x0306090C);

$k_rcon=0x140;		# rcon
	&data_word(0xAF9DEEB6,0x1F8391B9,0x4D7C7D81,0x702A9808);

$k_s63=0x150;		# s63: all equal to 0x63 transformed
	&data_word(0x5B5B5B5B,0x5B5B5B5B,0x5B5B5B5B,0x5B5B5B5B);

$k_opt=0x160;		# output transform
	&data_word(0xD6B66000,0xFF9F4929,0xDEBE6808,0xF7974121);
	&data_word(0x50BCEC00,0x01EDBD51,0xB05C0CE0,0xE10D5DB1);

$k_deskew=0x180;	# deskew tables: inverts the sbox's "skew"
	&data_word(0x47A4E300,0x07E4A340,0x5DBEF91A,0x1DFEB95A);
	&data_word(0x83EA6900,0x5F36B5DC,0xF49D1E77,0x2841C2AB);

&asciz	("Vector Permutation AES for x86/SSSE3, Mike Hamburg (Stanford University)");
&align	(64);

&function_begin_B("_vpaes_preheat");
	&add	($const,&DWP(0,"esp"));
	&movdqa	("xmm7",&QWP($k_inv,$const));
	&movdqa	("xmm6",&QWP($k_s0F,$const));
	&ret	();
&function_end_B("_vpaes_preheat");

##
##  _aes_encrypt_core
##
##  AES-encrypt %xmm0.
##
##  Inputs:
##     %xmm0 = input
##     %xmm6-%xmm7 as in _vpaes_preheat
##    (%edx) = scheduled keys
##
##  Output in %xmm0
##  Clobbers  %xmm1-%xmm5, %eax, %ebx, %ecx, %edx
##
##
&function_begin_B("_vpaes_encrypt_core");
	&mov	($magic,16);
	&mov	($round,&DWP(240,$key));
	&movdqa	("xmm1","xmm6")
	&movdqa	("xmm2",&QWP($k_ipt,$const));
	&pandn	("xmm1","xmm0");
	&pand	("xmm0","xmm6");
	&movdqu	("xmm5",&QWP(0,$key));
	&pshufb	("xmm2","xmm0");
	&movdqa	("xmm0",&QWP($k_ipt+16,$const));
	&pxor	("xmm2","xmm5");
	&psrld	("xmm1",4);
	&add	($key,16);
	&pshufb	("xmm0","xmm1");
	&lea	($base,&DWP($k_mc_backward,$const));
	&pxor	("xmm0","xmm2");
	&jmp	(&label("enc_entry"));


&set_label("enc_loop",16);
	# middle of middle round
	&movdqa	("xmm4",&QWP($k_sb1,$const));	# 4 : sb1u
	&movdqa	("xmm0",&QWP($k_sb1+16,$const));# 0 : sb1t
	&pshufb	("xmm4","xmm2");		# 4 = sb1u
	&pshufb	("xmm0","xmm3");		# 0 = sb1t
	&pxor	("xmm4","xmm5");		# 4 = sb1u + k
	&movdqa	("xmm5",&QWP($k_sb2,$const));	# 4 : sb2u
	&pxor	("xmm0","xmm4");		# 0 = A
	&movdqa	("xmm1",&QWP(-0x40,$base,$magic));# .Lk_mc_forward[]
	&pshufb	("xmm5","xmm2");		# 4 = sb2u
	&movdqa	("xmm2",&QWP($k_sb2+16,$const));# 2 : sb2t
	&movdqa	("xmm4",&QWP(0,$base,$magic));	# .Lk_mc_backward[]
	&pshufb	("xmm2","xmm3");		# 2 = sb2t
	&movdqa	("xmm3","xmm0");		# 3 = A
	&pxor	("xmm2","xmm5");		# 2 = 2A
	&pshufb	("xmm0","xmm1");		# 0 = B
	&add	($key,16);			# next key
	&pxor	("xmm0","xmm2");		# 0 = 2A+B
	&pshufb	("xmm3","xmm4");		# 3 = D
	&add	($magic,16);			# next mc
	&pxor	("xmm3","xmm0");		# 3 = 2A+B+D
	&pshufb	("xmm0","xmm1");		# 0 = 2B+C
	&and	($magic,0x30);			# ... mod 4
	&sub	($round,1);			# nr--
	&pxor	("xmm0","xmm3");		# 0 = 2A+3B+C+D

&set_label("enc_entry");
	# top of round
	&movdqa	("xmm1","xmm6");		# 1 : i
	&movdqa	("xmm5",&QWP($k_inv+16,$const));# 2 : a/k
	&pandn	("xmm1","xmm0");		# 1 = i<<4
	&psrld	("xmm1",4);			# 1 = i
	&pand	("xmm0","xmm6");		# 0 = k
	&pshufb	("xmm5","xmm0");		# 2 = a/k
	&movdqa	("xmm3","xmm7");		# 3 : 1/i
	&pxor	("xmm0","xmm1");		# 0 = j
	&pshufb	("xmm3","xmm1");		# 3 = 1/i
	&movdqa	("xmm4","xmm7");		# 4 : 1/j
	&pxor	("xmm3","xmm5");		# 3 = iak = 1/i + a/k
	&pshufb	("xmm4","xmm0");		# 4 = 1/j
	&movdqa	("xmm2","xmm7");		# 2 : 1/iak
	&pxor	("xmm4","xmm5");		# 4 = jak = 1/j + a/k
	&pshufb	("xmm2","xmm3");		# 2 = 1/iak
	&movdqa	("xmm3","xmm7");		# 3 : 1/jak
	&pxor	("xmm2","xmm0");		# 2 = io
	&pshufb	("xmm3","xmm4");		# 3 = 1/jak
	&movdqu	("xmm5",&QWP(0,$key));
	&pxor	("xmm3","xmm1");		# 3 = jo
	&jnz	(&label("enc_loop"));

	# middle of last round
	&movdqa	("xmm4",&QWP($k_sbo,$const));	# 3 : sbou      .Lk_sbo
	&movdqa	("xmm0",&QWP($k_sbo+16,$const));# 3 : sbot      .Lk_sbo+16
	&pshufb	("xmm4","xmm2");		# 4 = sbou
	&pxor	("xmm4","xmm5");		# 4 = sb1u + k
	&pshufb	("xmm0","xmm3");		# 0 = sb1t
	&movdqa	("xmm1",&QWP(0x40,$base,$magic));# .Lk_sr[]
	&pxor	("xmm0","xmm4");		# 0 = A
	&pshufb	("xmm0","xmm1");
	&ret	();
&function_end_B("_vpaes_encrypt_core");

########################################################
##                                                    ##
##                  AES key schedule                  ##
##                                                    ##
########################################################
&function_begin_B("_vpaes_schedule_core");
	&add	($const,&DWP(0,"esp"));
	&movdqu	("xmm0",&QWP(0,$inp));		# load key (unaligned)
	&movdqa	("xmm2",&QWP($k_rcon,$const));	# load rcon

	# input transform
	&movdqa	("xmm3","xmm0");
	&lea	($base,&DWP($k_ipt,$const));
	&movdqa	(&QWP(4,"esp"),"xmm2");		# xmm8
	&call	("_vpaes_schedule_transform");
	&movdqa	("xmm7","xmm0");

	&test	($out,$out);
	&jnz	(&label("schedule_am_decrypting"));

	# encrypting, output zeroth round key after transform
	&movdqu	(&QWP(0,$key),"xmm0");
	&jmp	(&label("schedule_go"));

&set_label("schedule_am_decrypting");
	# decrypting, output zeroth round key after shiftrows
	&movdqa	("xmm1",&QWP($k_sr,$const,$magic));
	&pshufb	("xmm3","xmm1");
	&movdqu	(&QWP(0,$key),"xmm3");
	&xor	($magic,0x30);

&set_label("schedule_go");
	&cmp	($round,192);
	&ja	(&label("schedule_256"));
	# 192-bit key support was removed. 
	# 128: fall though

##
##  .schedule_128
##
##  128-bit specific part of key schedule.
##
##  This schedule is really simple, because all its parts
##  are accomplished by the subroutines.
##
&set_label("schedule_128");
	&mov	($round,10);

&set_label("loop_schedule_128");
	&call	("_vpaes_schedule_round");
	&dec	($round);
	&jz	(&label("schedule_mangle_last"));
	&call	("_vpaes_schedule_mangle");	# write output
	&jmp	(&label("loop_schedule_128"));

##
##  .aes_schedule_256
##
##  256-bit specific part of key schedule.
##
##  The structure here is very similar to the 128-bit
##  schedule, but with an additional "low side" in
##  %xmm6.  The low side's rounds are the same as the
##  high side's, except no rcon and no rotation.
##
&set_label("schedule_256",16);
	&movdqu	("xmm0",&QWP(16,$inp));		# load key part 2 (unaligned)
	&call	("_vpaes_schedule_transform");	# input transform
	&mov	($round,7);

&set_label("loop_schedule_256");
	&call	("_vpaes_schedule_mangle");	# output low result
	&movdqa	("xmm6","xmm0");		# save cur_lo in xmm6

	# high round
	&call	("_vpaes_schedule_round");
	&dec	($round);
	&jz	(&label("schedule_mangle_last"));
	&call	("_vpaes_schedule_mangle");

	# low round. swap xmm7 and xmm6
	&pshufd	("xmm0","xmm0",0xFF);
	&movdqa	(&QWP(20,"esp"),"xmm7");
	&movdqa	("xmm7","xmm6");
	&call	("_vpaes_schedule_low_round");
	&movdqa	("xmm7",&QWP(20,"esp"));

	&jmp	(&label("loop_schedule_256"));

##
##  .aes_schedule_mangle_last
##
##  Mangler for last round of key schedule
##  Mangles %xmm0
##    when encrypting, outputs out(%xmm0) ^ 63
##    when decrypting, outputs unskew(%xmm0)
##
##  Always called right before return... jumps to cleanup and exits
##
&set_label("schedule_mangle_last",16);
	# schedule last round key from xmm0
	&lea	($base,&DWP($k_deskew,$const));
	&test	($out,$out);
	&jnz	(&label("schedule_mangle_last_dec"));

	# encrypting
	&movdqa	("xmm1",&QWP($k_sr,$const,$magic));
	&pshufb	("xmm0","xmm1");		# output permute
	&lea	($base,&DWP($k_opt,$const));	# prepare to output transform
	&add	($key,32);

&set_label("schedule_mangle_last_dec");
	&add	($key,-16);
	&pxor	("xmm0",&QWP($k_s63,$const));
	&call	("_vpaes_schedule_transform");	# output transform
	&movdqu	(&QWP(0,$key),"xmm0");		# save last key

	# cleanup
	&pxor	("xmm0","xmm0");
	&pxor	("xmm1","xmm1");
	&pxor	("xmm2","xmm2");
	&pxor	("xmm3","xmm3");
	&pxor	("xmm4","xmm4");
	&pxor	("xmm5","xmm5");
	&pxor	("xmm6","xmm6");
	&pxor	("xmm7","xmm7");
	&ret	();
&function_end_B("_vpaes_schedule_core");

##
##  .aes_schedule_round
##
##  Runs one main round of the key schedule on %xmm0, %xmm7
##
##  Specifically, runs subbytes on the high dword of %xmm0
##  then rotates it by one byte and xors into the low dword of
##  %xmm7.
##
##  Adds rcon from low byte of %xmm8, then rotates %xmm8 for
##  next rcon.
##
##  Smears the dwords of %xmm7 by xoring the low into the
##  second low, result into third, result into highest.
##
##  Returns results in %xmm7 = %xmm0.
##  Clobbers %xmm1-%xmm5.
##
&function_begin_B("_vpaes_schedule_round");
	# extract rcon from xmm8
	&movdqa	("xmm2",&QWP(8,"esp"));		# xmm8
	&pxor	("xmm1","xmm1");
	&palignr("xmm1","xmm2",15);
	&palignr("xmm2","xmm2",15);
	&pxor	("xmm7","xmm1");

	# rotate
	&pshufd	("xmm0","xmm0",0xFF);
	&palignr("xmm0","xmm0",1);

	# fall through...
	&movdqa	(&QWP(8,"esp"),"xmm2");		# xmm8

	# low round: same as high round, but no rotation and no rcon.
&set_label("_vpaes_schedule_low_round");
	# smear xmm7
	&movdqa	("xmm1","xmm7");
	&pslldq	("xmm7",4);
	&pxor	("xmm7","xmm1");
	&movdqa	("xmm1","xmm7");
	&pslldq	("xmm7",8);
	&pxor	("xmm7","xmm1");
	&pxor	("xmm7",&QWP($k_s63,$const));

	# subbyte
	&movdqa	("xmm4",&QWP($k_s0F,$const));
	&movdqa	("xmm5",&QWP($k_inv,$const));	# 4 : 1/j
	&movdqa	("xmm1","xmm4");
	&pandn	("xmm1","xmm0");
	&psrld	("xmm1",4);			# 1 = i
	&pand	("xmm0","xmm4");		# 0 = k
	&movdqa	("xmm2",&QWP($k_inv+16,$const));# 2 : a/k
	&pshufb	("xmm2","xmm0");		# 2 = a/k
	&pxor	("xmm0","xmm1");		# 0 = j
	&movdqa	("xmm3","xmm5");		# 3 : 1/i
	&pshufb	("xmm3","xmm1");		# 3 = 1/i
	&pxor	("xmm3","xmm2");		# 3 = iak = 1/i + a/k
	&movdqa	("xmm4","xmm5");		# 4 : 1/j
	&pshufb	("xmm4","xmm0");		# 4 = 1/j
	&pxor	("xmm4","xmm2");		# 4 = jak = 1/j + a/k
	&movdqa	("xmm2","xmm5");		# 2 : 1/iak
	&pshufb	("xmm2","xmm3");		# 2 = 1/iak
	&pxor	("xmm2","xmm0");		# 2 = io
	&movdqa	("xmm3","xmm5");		# 3 : 1/jak
	&pshufb	("xmm3","xmm4");		# 3 = 1/jak
	&pxor	("xmm3","xmm1");		# 3 = jo
	&movdqa	("xmm4",&QWP($k_sb1,$const));	# 4 : sbou
	&pshufb	("xmm4","xmm2");		# 4 = sbou
	&movdqa	("xmm0",&QWP($k_sb1+16,$const));# 0 : sbot
	&pshufb	("xmm0","xmm3");		# 0 = sb1t
	&pxor	("xmm0","xmm4");		# 0 = sbox output

	# add in smeared stuff
	&pxor	("xmm0","xmm7");
	&movdqa	("xmm7","xmm0");
	&ret	();
&function_end_B("_vpaes_schedule_round");

##
##  .aes_schedule_transform
##
##  Linear-transform %xmm0 according to tables at (%ebx)
##
##  Output in %xmm0
##  Clobbers %xmm1, %xmm2
##
&function_begin_B("_vpaes_schedule_transform");
	&movdqa	("xmm2",&QWP($k_s0F,$const));
	&movdqa	("xmm1","xmm2");
	&pandn	("xmm1","xmm0");
	&psrld	("xmm1",4);
	&pand	("xmm0","xmm2");
	&movdqa	("xmm2",&QWP(0,$base));
	&pshufb	("xmm2","xmm0");
	&movdqa	("xmm0",&QWP(16,$base));
	&pshufb	("xmm0","xmm1");
	&pxor	("xmm0","xmm2");
	&ret	();
&function_end_B("_vpaes_schedule_transform");

##
##  .aes_schedule_mangle
##
##  Mangle xmm0 from (basis-transformed) standard version
##  to our version.
##
##  On encrypt,
##    xor with 0x63
##    multiply by circulant 0,1,1,1
##    apply shiftrows transform
##
##  On decrypt,
##    xor with 0x63
##    multiply by "inverse mixcolumns" circulant E,B,D,9
##    deskew
##    apply shiftrows transform
##
##
##  Writes out to (%edx), and increments or decrements it
##  Keeps track of round number mod 4 in %ecx
##  Preserves xmm0
##  Clobbers xmm1-xmm5
##
&function_begin_B("_vpaes_schedule_mangle");
	&movdqa	("xmm4","xmm0");	# save xmm0 for later
	&movdqa	("xmm5",&QWP($k_mc_forward,$const));
	&test	($out,$out);
	&jnz	(&label("schedule_mangle_dec"));

	# encrypting
	&add	($key,16);
	&pxor	("xmm4",&QWP($k_s63,$const));
	&pshufb	("xmm4","xmm5");
	&movdqa	("xmm3","xmm4");
	&pshufb	("xmm4","xmm5");
	&pxor	("xmm3","xmm4");
	&pshufb	("xmm4","xmm5");
	&pxor	("xmm3","xmm4");

	&jmp	(&label("schedule_mangle_both"));

&set_label("schedule_mangle_dec",16);
	# inverse mix columns
	&movdqa	("xmm2",&QWP($k_s0F,$const));
	&lea	($inp,&DWP($k_dksd,$const));
	&movdqa	("xmm1","xmm2");
	&pandn	("xmm1","xmm4");
	&psrld	("xmm1",4);			# 1 = hi
	&pand	("xmm4","xmm2");		# 4 = lo

	&movdqa	("xmm2",&QWP(0,$inp));
	&pshufb	("xmm2","xmm4");
	&movdqa	("xmm3",&QWP(0x10,$inp));
	&pshufb	("xmm3","xmm1");
	&pxor	("xmm3","xmm2");
	&pshufb	("xmm3","xmm5");

	&movdqa	("xmm2",&QWP(0x20,$inp));
	&pshufb	("xmm2","xmm4");
	&pxor	("xmm2","xmm3");
	&movdqa	("xmm3",&QWP(0x30,$inp));
	&pshufb	("xmm3","xmm1");
	&pxor	("xmm3","xmm2");
	&pshufb	("xmm3","xmm5");

	&movdqa	("xmm2",&QWP(0x40,$inp));
	&pshufb	("xmm2","xmm4");
	&pxor	("xmm2","xmm3");
	&movdqa	("xmm3",&QWP(0x50,$inp));
	&pshufb	("xmm3","xmm1");
	&pxor	("xmm3","xmm2");
	&pshufb	("xmm3","xmm5");

	&movdqa	("xmm2",&QWP(0x60,$inp));
	&pshufb	("xmm2","xmm4");
	&pxor	("xmm2","xmm3");
	&movdqa	("xmm3",&QWP(0x70,$inp));
	&pshufb	("xmm3","xmm1");
	&pxor	("xmm3","xmm2");

	&add	($key,-16);

&set_label("schedule_mangle_both");
	&movdqa	("xmm1",&QWP($k_sr,$const,$magic));
	&pshufb	("xmm3","xmm1");
	&add	($magic,-16);
	&and	($magic,0x30);
	&movdqu	(&QWP(0,$key),"xmm3");
	&ret	();
&function_end_B("_vpaes_schedule_mangle");

#
# Interface to OpenSSL
#
&function_begin("${PREFIX}_set_encrypt_key");
	record_function_hit(5);

	&mov	($inp,&wparam(0));		# inp
	&lea	($base,&DWP(-56,"esp"));
	&mov	($round,&wparam(1));		# bits
	&and	($base,-16);
	&mov	($key,&wparam(2));		# key
	&xchg	($base,"esp");			# alloca
	&mov	(&DWP(48,"esp"),$base);

	&mov	($base,$round);
	&shr	($base,5);
	&add	($base,5);
	&mov	(&DWP(240,$key),$base);		# AES_KEY->rounds = nbits/32+5;
	&mov	($magic,0x30);
	&mov	($out,0);

	&lea	($const,&DWP(&label("_vpaes_consts")."+0x30-".&label("pic_point")));
	&call	("_vpaes_schedule_core");
&set_label("pic_point");

	&mov	("esp",&DWP(48,"esp"));
	&xor	("eax","eax");
&function_end("${PREFIX}_set_encrypt_key");

&function_begin("${PREFIX}_encrypt");
	record_function_hit(4);

	&lea	($const,&DWP(&label("_vpaes_consts")."+0x30-".&label("pic_point")));
	&call	("_vpaes_preheat");
&set_label("pic_point");
	&mov	($inp,&wparam(0));		# inp
	&lea	($base,&DWP(-56,"esp"));
	&mov	($out,&wparam(1));		# out
	&and	($base,-16);
	&mov	($key,&wparam(2));		# key
	&xchg	($base,"esp");			# alloca
	&mov	(&DWP(48,"esp"),$base);

	&movdqu	("xmm0",&QWP(0,$inp));
	&call	("_vpaes_encrypt_core");
	&movdqu	(&QWP(0,$out),"xmm0");

	&mov	("esp",&DWP(48,"esp"));
&function_end("${PREFIX}_encrypt");

&asm_finish();

close STDOUT or die "error closing STDOUT: $!";
