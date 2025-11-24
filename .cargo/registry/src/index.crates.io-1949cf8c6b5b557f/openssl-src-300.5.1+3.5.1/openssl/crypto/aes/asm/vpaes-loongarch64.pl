#! /usr/bin/env perl
# Copyright 2015-2025 The OpenSSL Project Authors. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

######################################################################
## Constant-time SSSE3 AES core implementation.
## version 0.1
##
## By Mike Hamburg (Stanford University), 2009
## Public domain.
##
## For details see http://shiftleft.org/papers/vector_aes/ and
## http://crypto.stanford.edu/vpaes/.
##
######################################################################

# Loongarch64 LSX adaptation by <zhuchen@loongson.cn>,
# <lujingfeng@loongson.cn> and <shichenlong@loongson.cn>
#

($zero,$ra,$tp,$sp)=map("\$r$_",(0..3));
($a0,$a1,$a2,$a3,$a4,$a5,$a6,$a7)=map("\$r$_",(4..11));
($t0,$t1,$t2,$t3,$t4,$t5,$t6,$t7,$t8,$t9)=map("\$r$_",(12..21));
($s0,$s1,$s2,$s3,$s4,$s5,$s6,$s7)=map("\$r$_",(23..30));
($vr0,$vr1,$vr2,$vr3,$vr4,$vr5,$vr6,$vr7,$vr8,$vr9,$vr10,$vr11,$vr12,$vr13,$vr14,$vr15,$vr16,$vr17,$vr18,$vr19)=map("\$vr$_",(0..19));
($fp)=map("\$r$_",(22));

# $output is the last argument if it looks like a file (it has an extension)
my $output;
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
open STDOUT,">$output";

$PREFIX="vpaes";

$code.=<<___;

##
##  _aes_encrypt_core
##
##  AES-encrypt %vr0.
##
##  Inputs:
##     %vr0 = input
##     %vr9-%vr15 as in _vpaes_preheat
##    (%a2) = scheduled keys
##
##  Output in %vr0
##  Clobbers  %vr1-%vr5, %r9, %r10, %r11, %t5
##  Preserves %vr6 - %vr8 so you get some local vectors
##
##
##.type	_vpaes_encrypt_core
.align 4
_vpaes_encrypt_core:
.cfi_startproc
    move      $a5,$a2
    li.d      $a7,0x10
    ld.w      $t5,$a2,240
    vori.b    $vr1,$vr9,0
    la.local  $t0,Lk_ipt
    vld       $vr2,$t0,0    # iptlo
    vandn.v   $vr1,$vr1,$vr0
    vld	      $vr5,$a5,0    # round0 key
    vsrli.w   $vr1,$vr1,4
    vand.v    $vr0,$vr0,$vr9
    vshuf.b   $vr2,$vr18,$vr2,$vr0
    vld       $vr0,$t0,16   # ipthi
    vshuf.b   $vr0,$vr18,$vr0,$vr1
    vxor.v    $vr2,$vr2,$vr5
    addi.d    $a5,$a5,16
    vxor.v    $vr0,$vr0,$vr2
    la.local  $a6,Lk_mc_backward
    b         .Lenc_entry

.align 4
.Lenc_loop:
    # middle of middle round
    vori.b    $vr4,$vr13,0           # 4 : sb1u
    vori.b    $vr0,$vr12,0           # 0 : sb1t
    vshuf.b   $vr4,$vr18,$vr4,$vr2    # 4 = sb1u
    vshuf.b   $vr0,$vr18,$vr0,$vr3    # 0 = sb1t
    vxor.v    $vr4,$vr4,$vr5         # 4 = sb1u + k
    vori.b    $vr5,$vr15,0           # 4 : sb2u
    vxor.v    $vr0,$vr0,$vr4         # 0 = A
    add.d     $t0,$a7,$a6            # Lk_mc_forward[]
    vld       $vr1,$t0,-0x40
    vshuf.b   $vr5,$vr18,$vr5,$vr2    # 4 = sb2u
    vld       $vr4,$t0,0             # Lk_mc_backward[]
    vori.b    $vr2,$vr14,0           # 2 : sb2t
    vshuf.b   $vr2,$vr18,$vr2,$vr3    # 2 = sb2t
    vori.b    $vr3,$vr0,0            # 3 = A
    vxor.v    $vr2,$vr5,$vr2         # 2 = 2A
    vshuf.b   $vr0,$vr18,$vr0,$vr1    # 0 = B
    addi.d    $a5,$a5,16             # next key
    vxor.v    $vr0,$vr0,$vr2         # 0 = 2A+B
    vshuf.b   $vr3,$vr18,$vr3,$vr4    # 3 = D
    addi.d    $a7,$a7,16             # next mc
    vxor.v    $vr3,$vr3,$vr0         # 3 = 2A+B+D
    vshuf.b   $vr0,$vr18,$vr0,$vr1    # 0 = 2B+C
    andi      $a7,$a7,0x30           # ... mod 4
    addi.d    $t5,$t5,-1             # nr--
    vxor.v    $vr0,$vr0,$vr3         # 0 = 2A+3B+C+D

.Lenc_entry:
    # top of round
    vori.b    $vr1,$vr9,0           # 1 : i
    vori.b    $vr5,$vr11,0          # 2 : a/k
    vandn.v   $vr1,$vr1,$vr0        # 1 = i<<4
    vsrli.w   $vr1,$vr1,4           # 1 = i
    vand.v    $vr0,$vr0,$vr9        # 0 = k
    vshuf.b   $vr5,$vr18,$vr5,$vr0   # 2 = a/k
    vori.b    $vr3,$vr10,0          # 3 : 1/i
    vxor.v    $vr0,$vr0,$vr1        # 0 = j
    vshuf.b   $vr3,$vr18,$vr3,$vr1   # 3 = 1/i
    vori.b    $vr4,$vr10,0          # 4 : 1/j
    vxor.v    $vr3,$vr3,$vr5        # 3 = iak = 1/i + a/k
    vshuf.b   $vr4,$vr18,$vr4,$vr0   # 4 = 1/j
    vori.b    $vr2,$vr10,0          # 2 : 1/iak
    vxor.v    $vr4,$vr4,$vr5        # 4 = jak = 1/j + a/k
    vshuf.b   $vr2,$vr18,$vr2,$vr3   # 2 = 1/iak
    vori.b    $vr3,$vr10,0          # 3 : 1/jak
    vxor.v    $vr2,$vr2,$vr0        # 2 = io
    vshuf.b   $vr3,$vr18,$vr3,$vr4   # 3 = 1/jak
    vld       $vr5,$a5,0
    vxor.v    $vr3,$vr3,$vr1        # 3 = jo
    bnez      $t5,.Lenc_loop

    # middle of last round
    vld       $vr4,$a6,	-0x60		# 3 : sbou	Lk_sbo
    vld       $vr0,$a6,	-0x50		# 0 : sbot	Lk_sbo+16
    vshuf.b   $vr4,$vr18,$vr4,$vr2	# 4 = sbou
    vxor.v    $vr4,$vr4,$vr5		# 4 = sb1u + k
    vshuf.b   $vr0,$vr18,$vr0,$vr3	# 0 = sb1t
    add.d     $t0,$a7,$a6		# Lk_sr[]
    vld       $vr1,$t0,0x40
    vxor.v    $vr0,$vr0,$vr4		# 0 = A
    vshuf.b   $vr0,$vr18,$vr0,$vr1
    jr        $ra
.cfi_endproc
.size	_vpaes_encrypt_core,.-_vpaes_encrypt_core

##
##  Decryption core
##
##  Same API as encryption core.
##
#.type	_vpaes_decrypt_core,\@abi-omnipotent
.align	4
_vpaes_decrypt_core:
.cfi_startproc
    move      $a5,$a2                  # load key
    ld.w      $t5,$a2,240
    vori.b    $vr1,$vr9,0
    la.local  $t0,Lk_dipt
    vld       $vr2,$t0,0               # iptlo
    vandn.v   $vr1,$vr1,$vr0
    move      $a7,$t5
    vsrli.w   $vr1,$vr1,4
    vld       $vr5,$a5,0               # round0 key
    slli.d    $a7,$a7,4
    vand.v    $vr0,$vr9,$vr0
    vshuf.b   $vr2,$vr18,$vr2,$vr0
    vld       $vr0,$t0,16              # ipthi
    xori      $a7,$a7,0x30
    la.local  $a6,Lk_dsbd
    vshuf.b   $vr0,$vr18,$vr0,$vr1
    andi      $a7,$a7,0x30
    vxor.v    $vr2,$vr2,$vr5
    la.local  $t0,Lk_mc_forward
    vld       $vr5,$t0,48
    vxor.v    $vr0,$vr0,$vr2
    addi.d    $a5,$a5,16
    add.d     $a7,$a7,$a6
    b         .Ldec_entry

.align 4
.Ldec_loop:
##
##  Inverse mix columns
##
    vld        $vr4,$a6,-0x20		# 4 : sb9u
    vld        $vr1,$a6,-0x10		# 0 : sb9t
    vshuf.b    $vr4,$vr18,$vr4,$vr2	# 4 = sb9u
    vshuf.b    $vr1,$vr18,$vr1,$vr3	# 0 = sb9t
    vxor.v     $vr0,$vr0,$vr4
    vld        $vr4,$a6,0x0		# 4 : sbdu
    vxor.v     $vr0,$vr0,$vr1		# 0 = ch
    vld        $vr1,$a6,0x10		# 0 : sbdt
    vshuf.b    $vr4,$vr18,$vr4,$vr2	# 4 = sbdu
    vshuf.b    $vr0,$vr18,$vr0,$vr5	# MC ch
    vshuf.b    $vr1,$vr18,$vr1,$vr3	# 0 = sbdt
    vxor.v     $vr0,$vr0,$vr4		# 4 = ch
    vld        $vr4,$a6,0x20		# 4 : sbbu
    vxor.v     $vr0,$vr0,$vr1		# 0 = ch
    vld        $vr1,$a6,0x30		# 0 : sbbt
    vshuf.b    $vr4,$vr18,$vr4,$vr2	# 4 = sbbu
    vshuf.b    $vr0,$vr18,$vr0,$vr5	# MC ch
    vshuf.b    $vr1,$vr18,$vr1,$vr3	# 0 = sbbt
    vxor.v     $vr0,$vr0,$vr4		# 4 = ch
    vld        $vr4,$a6,0x40		# 4 : sbeu
    vxor.v     $vr0,$vr0,$vr1		# 0 = ch
    vld        $vr1,$a6,0x50		# 0 : sbet
    vshuf.b    $vr4,$vr18,$vr4,$vr2	# 4 = sbeu
    vshuf.b    $vr0,$vr18,$vr0,$vr5	# MC ch
    vshuf.b    $vr1,$vr18,$vr1,$vr3	# 0 = sbet
    vxor.v     $vr0,$vr0,$vr4		# 4 = ch
    addi.d     $a5,$a5,	16		# next round key
    vbsrl.v    $vr16,$vr5,0xc
    vbsll.v    $vr5,$vr5,0x4
    vor.v      $vr5,$vr5,$vr16
    vxor.v     $vr0,$vr0,$vr1		# 0 = ch
    addi.d     $t5,$t5,-1		# nr--

.Ldec_entry:
    # top of round
    vori.b     $vr1,$vr9,0		# 1 : i
    vandn.v    $vr1,$vr1,$vr0		# 1 = i<<4
    vori.b     $vr2,$vr11,0		# 2 : a/k
    vsrli.w    $vr1,$vr1,4		# 1 = i
    vand.v     $vr0,$vr0,$vr9		# 0 = k
    vshuf.b    $vr2,$vr18,$vr2,$vr0	# 2 = a/k
    vori.b     $vr3,$vr10,0		# 3 : 1/i
    vxor.v     $vr0,$vr0,$vr1		# 0 = j
    vshuf.b    $vr3,$vr18,$vr3,$vr1	# 3 = 1/i
    vori.b     $vr4,$vr10,0		# 4 : 1/j
    vxor.v     $vr3,$vr3,$vr2		# 3 = iak = 1/i + a/k
    vshuf.b    $vr4,$vr18,$vr4,$vr0	# 4 = 1/j
    vxor.v     $vr4,$vr4,$vr2		# 4 = jak = 1/j + a/k
    vori.b     $vr2,$vr10,0		# 2 : 1/iak
    vshuf.b    $vr2,$vr18,$vr2,$vr3	# 2 = 1/iak
    vori.b     $vr3,$vr10,0		# 3 : 1/jak
    vxor.v     $vr2,$vr2,$vr0		# 2 = io
    vshuf.b    $vr3,$vr18,$vr3,$vr4	# 3 = 1/jak
    vld        $vr0,$a5,0
    vxor.v     $vr3,$vr3,$vr1		# 3 = jo
    bnez       $t5,.Ldec_loop

    # middle of last round
    vld        $vr4,$a6,0x60		# 3 : sbou
    vshuf.b    $vr4,$vr18,$vr4,$vr2	# 4 = sbou
    vxor.v     $vr4,$vr4,$vr0		# 4 = sb1u + k
    vld        $vr0,$a6,0x70		# 0 : sbot
    vld        $vr2,$a7,-0x160		# Lk_sr-.Lk_dsbd=-0x160
    vshuf.b    $vr0,$vr18,$vr0,$vr3	# 0 = sb1t
    vxor.v     $vr0,$vr0,$vr4		# 0 = A
    vshuf.b    $vr0,$vr18,$vr0,$vr2
    jr         $ra
.cfi_endproc
.size	_vpaes_decrypt_core,.-_vpaes_decrypt_core

########################################################
##                                                    ##
##                  AES key schedule                  ##
##                                                    ##
########################################################
#.type	_vpaes_schedule_core,\@abi-omnipotent
.align	4
_vpaes_schedule_core:
.cfi_startproc
    # a0 = key
    # a1 = size in bits
    # a2 = buffer
    # a3 = direction.  0=encrypt, 1=decrypt

    addi.d    $sp,$sp,-48
    st.d      $ra,$sp,40
    st.d      $fp,$sp,32

    bl	_vpaes_preheat			# load the tables
    la.local  $t0,Lk_rcon
    vld       $vr8,$t0,0		# load rcon
    vld       $vr0,$a0,0		# load key (unaligned)

    # input transform
    vori.b    $vr3,$vr0,0
    la.local  $a7,Lk_ipt
    bl	      _vpaes_schedule_transform
    vori.b    $vr7,$vr0,0

    la.local  $a6,Lk_sr
    bnez      $a3,.Lschedule_am_decrypting

    # encrypting, output zeroth round key after transform
    vst       $vr0,$a2,0
    b         .Lschedule_go

.Lschedule_am_decrypting:
    # decrypting, output zeroth round key after shiftrows
    add.d     $t2,$a4,$a6
    vld       $vr1,$t2,0
    vshuf.b   $vr3,$vr18,$vr3,$vr1
    vst       $vr3,$a2,0
    xori      $a4,$a4,0x30

.Lschedule_go:
    li.d      $t6,192
    bltu      $t6,$a1,.Lschedule_256
    beq       $t6,$a1,.Lschedule_192
    # 128: fall though

##
##  .schedule_128
##
##  128-bit specific part of key schedule.
##
##  This schedule is really simple, because all its parts
##  are accomplished by the subroutines.
##
.Lschedule_128:
     li.w    $a1,10

.Loop_schedule_128:
     bl      _vpaes_schedule_round
     addi.w  $a1,$a1,-1
     beqz    $a1,.Lschedule_mangle_last
     bl      _vpaes_schedule_mangle
     b       .Loop_schedule_128

##
##  .aes_schedule_192
##
##  192-bit specific part of key schedule.
##
##  The main body of this schedule is the same as the 128-bit
##  schedule, but with more smearing.  The long, high side is
##  stored in %vr7 as before, and the short, low side is in
##  the high bits of %vr6.
##
##  This schedule is somewhat nastier, however, because each
##  round produces 192 bits of key material, or 1.5 round keys.
##  Therefore, on each cycle we do 2 rounds and produce 3 round
##  keys.
##
.align	4
.Lschedule_192:
     vld        $vr0,$a0,8			#load key part 2
     bl        	_vpaes_schedule_transform	#input transform
     vaddi.du   $vr6,$vr0,0x0    		#save short part
     vxor.v     $vr4,$vr4,$vr4			#clear 4
     vpackod.d  $vr6,$vr6,$vr4			#clobber low side with zeros
     li.w       $a1,4

.Loop_schedule_192:
     bl         _vpaes_schedule_round
     vbsrl.v    $vr16,$vr6,0x8
     vbsll.v    $vr0,$vr0,0x8
     vor.v      $vr0,$vr0,$vr16

     bl         _vpaes_schedule_mangle  	# save key n
     bl         _vpaes_schedule_192_smear
     bl         _vpaes_schedule_mangle		# save key n+1
     bl         _vpaes_schedule_round
     addi.w     $a1,$a1,-1
     beqz       $a1,.Lschedule_mangle_last
     bl         _vpaes_schedule_mangle		# save key n+2
     bl         _vpaes_schedule_192_smear
     b          .Loop_schedule_192

##
##  .aes_schedule_256
##
##  256-bit specific part of key schedule.
##
##  The structure here is very similar to the 128-bit
##  schedule, but with an additional "low side" in
##  %vr6.  The low side's rounds are the same as the
##  high side's, except no rcon and no rotation.
##
.align	4
.Lschedule_256:
     vld        $vr0,$a0,16		        # load key part 2 (unaligned)
     bl         _vpaes_schedule_transform	# input transform
     addi.w     $a1,$zero,7

.Loop_schedule_256:
     bl         _vpaes_schedule_mangle          # output low result
     vori.b     $vr6,$vr0,0		        # save cur_lo in vr6

     # high round
     bl         _vpaes_schedule_round
     addi.d     $a1,$a1,-1
     beqz       $a1,.Lschedule_mangle_last
     bl         _vpaes_schedule_mangle

     # low round. swap vr7 and vr6
     vshuf4i.w  $vr0,$vr0,0xFF
     vori.b     $vr5,$vr7,0
     vori.b     $vr7,$vr6,0
     bl         _vpaes_schedule_low_round
     vori.b     $vr7,$vr5,0

     b          .Loop_schedule_256


##
##  .aes_schedule_mangle_last
##
##  Mangler for last round of key schedule
##  Mangles %vr0
##    when encrypting, outputs out(%vr0) ^ 63
##    when decrypting, outputs unskew(%vr0)
##
##  Always called right before return... jumps to cleanup and exits
##
.align	4
.Lschedule_mangle_last:
     # schedule last round key from vr0
     la.local   $a7,Lk_deskew			# prepare to deskew
     bnez       $a3,.Lschedule_mangle_last_dec

     # encrypting
     add.d      $t0,$a4,$a6
     vld        $vr1,$t0,0
     vshuf.b    $vr0,$vr18,$vr0,$vr1             # output permute
     la.local   $a7,Lk_opt                      # prepare to output transform
     addi.d     $a2,$a2,32

.Lschedule_mangle_last_dec:
     addi.d     $a2,$a2,-16
     la.local   $t0,Lk_s63
     vld        $vr16,$t0,0
     vxor.v     $vr0,$vr0,$vr16
     bl         _vpaes_schedule_transform	# output transform
     vst        $vr0,$a2,0			# save last key

     # cleanup
     vxor.v     $vr0,$vr0,$vr0
     vxor.v     $vr1,$vr1,$vr1
     vxor.v     $vr2,$vr2,$vr2
     vxor.v     $vr3,$vr3,$vr3
     vxor.v     $vr4,$vr4,$vr4
     vxor.v     $vr5,$vr5,$vr5
     vxor.v     $vr6,$vr6,$vr6
     vxor.v     $vr7,$vr7,$vr7
     ld.d       $ra,$sp,40
     ld.d       $fp,$sp,32
     addi.d     $sp,$sp,48
     jr         $ra
.cfi_endproc
.size	_vpaes_schedule_core,.-_vpaes_schedule_core

##
##  .aes_schedule_192_smear
##
##  Smear the short, low side in the 192-bit key schedule.
##
##  Inputs:
##    %vr7: high side, b  a  x  y
##    %vr6:  low side, d  c  0  0
##    %vr13: 0
##
##  Outputs:
##    %vr6: b+c+d  b+c  0  0
##    %vr0: b+c+d  b+c  b  a
##
#.type	_vpaes_schedule_192_smear,\@abi-omnipotent
.align	4
_vpaes_schedule_192_smear:
.cfi_startproc
    vshuf4i.w   $vr1,$vr6,0x80	# d c 0 0 -> c 0 0 0
    vshuf4i.w   $vr0,$vr7,0xFE	# b a _ _ -> b b b a
    vxor.v      $vr6,$vr6,$vr1	# -> c+d c 0 0
    vxor.v      $vr1,$vr1,$vr1
    vxor.v      $vr6,$vr6,$vr0	# -> b+c+d b+c b a
    vori.b      $vr0,$vr6,0
    vilvh.d     $vr6,$vr6,$vr1	# clobber low side with zeros
    jr		$ra
.cfi_endproc
.size	_vpaes_schedule_192_smear,.-_vpaes_schedule_192_smear

##
##  .aes_schedule_round
##
##  Runs one main round of the key schedule on %vr0, %vr7
##
##  Specifically, runs subbytes on the high dword of %vr0
##  then rotates it by one byte and xors into the low dword of
##  %vr7.
##
##  Adds rcon from low byte of %vr8, then rotates %vr8 for
##  next rcon.
##
##  Smears the dwords of %vr7 by xoring the low into the
##  second low, result into third, result into highest.
##
##  Returns results in %vr7 = %vr0.
##  Clobbers %vr1-%vr4, %a7.
##
#.type	_vpaes_schedule_round,\@abi-omnipotent
.align	4
_vpaes_schedule_round:
.cfi_startproc
    # extract rcon from vr8
    vxor.v      $vr1,$vr1,$vr1
    vbsrl.v     $vr16,$vr8,0xf
    vbsll.v     $vr1,$vr1,0x1
    vor.v       $vr1,$vr1,$vr16
    vbsrl.v     $vr16,$vr8,0xf
    vbsll.v     $vr8,$vr8,0x1
    vor.v       $vr8,$vr8,$vr16

    vxor.v      $vr7,$vr7,$vr1

    # rotate
    vshuf4i.w   $vr0,$vr0,0xff  		#put $vr0 lowest 32 bit to each words
    vbsrl.v     $vr16,$vr0,0x1
    vbsll.v     $vr0,$vr0,0xf
    vor.v       $vr0,$vr0,$vr16

    # fall through...

    # low round: same as high round, but no rotation and no rcon.
_vpaes_schedule_low_round:
    # smear vr7
    vaddi.du    $vr1,$vr7,0x0
    vbsll.v     $vr7,$vr7,0x4
    vxor.v      $vr7,$vr7,$vr1
    vaddi.du    $vr1,$vr7,0x0
    vbsll.v     $vr7,$vr7,0x8
    vxor.v      $vr7,$vr7,$vr1
    vxori.b     $vr7,$vr7,0x5B

    # subbytes
    vaddi.du    $vr1,$vr9,0x0
    vandn.v     $vr1,$vr1,$vr0
    vsrli.w     $vr1,$vr1,0x4			# 1 = i
    vand.v      $vr0,$vr0,$vr9			# 0 = k
    vaddi.du    $vr2,$vr11,0x0			# 2 : a/k
    vshuf.b     $vr2,$vr18,$vr2,$vr0		# 2 = a/k
    vxor.v      $vr0,$vr0,$vr1			# 0 = j
    vaddi.du    $vr3,$vr10,0x0			# 3 : 1/i
    vshuf.b     $vr3,$vr18,$vr3,$vr1		# 3 = 1/i
    vxor.v      $vr3,$vr3,$vr2			# 3 = iak = 1/i + a/k
    vaddi.du    $vr4,$vr10,0x0			# 4 : 1/j
    vshuf.b     $vr4,$vr18,$vr4,$vr0		# 4 = 1/j
    vxor.v      $vr4,$vr4,$vr2			# 4 = jak = 1/j + a/k
    vaddi.du    $vr2,$vr10,0x0			# 2 : 1/iak
    vshuf.b     $vr2,$vr18,$vr2,$vr3		# 2 = 1/iak
    vxor.v      $vr2,$vr2,$vr0			# 2 = io
    vaddi.du    $vr3,$vr10,0x0			# 3 : 1/jak
    vshuf.b     $vr3,$vr18,$vr3,$vr4		# 3 = 1/jak
    vxor.v      $vr3,$vr3,$vr1			# 3 = jo
    vaddi.du    $vr4,$vr13,0x0			# 4 : sbou
    vshuf.b     $vr4,$vr18,$vr4,$vr2		# 4 = sbou
    vaddi.du    $vr0,$vr12,0x0			# 0 : sbot
    vshuf.b     $vr0,$vr18,$vr0,$vr3		# 0 = sb1t
    vxor.v      $vr0,$vr0,$vr4			# 0 = sbox output

    # add in smeared stuff
    vxor.v      $vr0,$vr0,$vr7
    vaddi.du    $vr7,$vr0,0x0
    jr          $ra
.cfi_endproc
.size	_vpaes_schedule_round,.-_vpaes_schedule_round

##
##  .aes_schedule_transform
##
##  Linear-transform %vr0 according to tables at (%r11)
##
##  Requires that %vr9 = 0x0F0F... as in preheat
##  Output in %vr0
##  Clobbers %vr1, %vr2
##
#.type	_vpaes_schedule_transform,\@abi-omnipotent
.align	4
_vpaes_schedule_transform:
.cfi_startproc
    vori.b     $vr1,$vr9,0
    vandn.v    $vr1,$vr1,$vr0
    vsrli.w    $vr1,$vr1,4
    vand.v     $vr0,$vr0,$vr9
    vld        $vr2,$a7,0		# lo
    vshuf.b    $vr2,$vr18,$vr2,$vr0
    vld        $vr0,$a7,16		# hi
    vshuf.b    $vr0,$vr18,$vr0,$vr1
    vxor.v     $vr0,$vr0,$vr2
    jr         $ra
.cfi_endproc
.size	_vpaes_schedule_transform,.-_vpaes_schedule_transform

##
##  .aes_schedule_mangle
##
##  Mangle vr0 from (basis-transformed) standard version
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
##  Writes out to (%a2), and increments or decrements it
##  Keeps track of round number mod 4 in %a4
##  Preserves vr0
##  Clobbers vr1-vr5
##
#.type	_vpaes_schedule_mangle,\@abi-omnipotent
.align	4
_vpaes_schedule_mangle:
.cfi_startproc
    vori.b     $vr4,$vr0,0		# save vr0 for later
    la.local   $t0,Lk_mc_forward
    vld        $vr5,$t0,0
    bnez       $a3,.Lschedule_mangle_dec

    # encrypting
    addi.d     $a2,$a2,16
    la.local   $t0,Lk_s63
    vld        $vr16,$t0,0
    vxor.v     $vr4,$vr4,$vr16
    vshuf.b    $vr4,$vr18,$vr4,$vr5
    vori.b     $vr3,$vr4,0
    vshuf.b    $vr4,$vr18,$vr4,$vr5
    vxor.v     $vr3,$vr3,$vr4
    vshuf.b    $vr4,$vr18,$vr4,$vr5
    vxor.v     $vr3,$vr3,$vr4

    b          .Lschedule_mangle_both
.align	4
.Lschedule_mangle_dec:
    # inverse mix columns
    la.local $a7,Lk_dksd
    vori.b     $vr1,$vr9,0
    vandn.v    $vr1,$vr1,$vr4
    vsrli.w    $vr1,$vr1,4		# 1 = hi
    vand.v     $vr4,$vr4,$vr9		# 4 = lo

    vld        $vr2,$a7,0
    vshuf.b    $vr2,$vr18,$vr2,$vr4
    vld        $vr3,$a7,0x10
    vshuf.b    $vr3,$vr18,$vr3,$vr1
    vxor.v     $vr3,$vr3,$vr2
    vshuf.b    $vr3,$vr18,$vr3,$vr5

    vld        $vr2,$a7,0x20
    vshuf.b    $vr2,$vr18,$vr2,$vr4
    vxor.v     $vr2,$vr2,$vr3
    vld        $vr3,$a7,0x30
    vshuf.b    $vr3,$vr18,$vr3,$vr1
    vxor.v     $vr3,$vr3,$vr2
    vshuf.b    $vr3,$vr18,$vr3,$vr5

    vld        $vr2,$a7,0x40
    vshuf.b    $vr2,$vr18,$vr2,$vr4
    vxor.v     $vr2,$vr2,$vr3
    vld        $vr3,$a7,0x50
    vshuf.b    $vr3,$vr18,$vr3,$vr1
    vxor.v     $vr3,$vr3,$vr2
    vshuf.b    $vr3,$vr18,$vr3,$vr5

    vld        $vr2,$a7,0x60
    vshuf.b    $vr2,$vr18,$vr2,$vr4
    vxor.v     $vr2,$vr2,$vr3
    vld        $vr3,$a7,0x70
    vshuf.b    $vr3,$vr18,$vr3,$vr1
    vxor.v     $vr3,$vr3,$vr2

    addi.d     $a2,$a2,-16

.Lschedule_mangle_both:
    add.d      $t2,$a4,$a6
    vld        $vr1,$t2,0
    vshuf.b    $vr3,$vr18,$vr3,$vr1
    addi.d     $a4,$a4,-16
    andi       $a4,$a4,0x30
    vst        $vr3,$a2,0
    jirl       $zero,$ra,0
.cfi_endproc
.size	_vpaes_schedule_mangle,.-_vpaes_schedule_mangle

#
# Interface to OpenSSL
#
.globl	${PREFIX}_set_encrypt_key
#.type	${PREFIX}_set_encrypt_key,\@function,3
.align	4
${PREFIX}_set_encrypt_key:
.cfi_startproc
___
$code.=<<___;
    addi.d   $sp,$sp,-48
    st.d     $ra,$sp,40
    st.d     $fp,$sp,32
    move     $t5,$a1
    srli.w   $t5,$t5,0x5
    addi.w   $t5,$t5,0x5
    st.w     $t5,$a2,240	# AES_KEY->rounds = nbits/32+5;

    move     $a3,$zero
    li.d     $a4,0x30
    bl       _vpaes_schedule_core
___
$code.=<<___;
    xor      $a0,$a0,$a0
    ld.d     $ra,$sp,40
    ld.d     $fp,$sp,32
    addi.d   $sp,$sp,48
    jirl     $zero,$ra,0
.cfi_endproc
.size	${PREFIX}_set_encrypt_key,.-${PREFIX}_set_encrypt_key

.globl	${PREFIX}_set_decrypt_key
#.type	${PREFIX}_set_decrypt_key,\@function,3
.align	4
${PREFIX}_set_decrypt_key:
.cfi_startproc

.Ldec_key_body:
___
$code.=<<___;
    addi.d   $sp,$sp,-48
    st.d     $ra,$sp,40
    st.d     $fp,$sp,32

    move     $t5,$a1
    srli.w   $t5,$t5,5
    addi.w   $t5,$t5,5
    st.w     $t5,$a2,240	# AES_KEY->rounds = nbits/32+5;
    slli.w   $t5,$t5,4
    add.d    $t0,$a2,$t5
    addi.d   $a2,$t0,16

    li.d     $a3,0x1
    move     $a4,$a1
    srli.w   $a4,$a4,1
    andi     $a4,$a4,32
    xori     $a4,$a4,32		# nbits==192?0:32
    bl       _vpaes_schedule_core

.Ldec_key_epilogue:
___
$code.=<<___;
    xor      $a0,$a0,$a0
    ld.d     $ra,$sp,40
    ld.d     $fp,$sp,32
    addi.d   $sp,$sp,48
    jirl     $zero,$ra,0
.cfi_endproc
.size	${PREFIX}_set_decrypt_key,.-${PREFIX}_set_decrypt_key

.globl	${PREFIX}_encrypt
#.type	${PREFIX}_encrypt,\@function,3
.align	4
${PREFIX}_encrypt:
.cfi_startproc
.Lenc_body:
___
$code.=<<___;
    addi.d  $sp,$sp,-48
    st.d    $ra,$sp,40
    st.d    $fp,$sp,32
    vld     $vr0,$a0,0x0
    bl      _vpaes_preheat
    bl      _vpaes_encrypt_core
    vst     $vr0,$a1,0x0
.Lenc_epilogue:
___
$code.=<<___;
    ld.d     $ra,$sp,40
    ld.d     $fp,$sp,32
    addi.d   $sp,$sp,48
    jirl     $zero,$ra,0
.cfi_endproc
.size	${PREFIX}_encrypt,.-${PREFIX}_encrypt

.globl	${PREFIX}_decrypt
#.type	${PREFIX}_decrypt,\@function,3
.align	4
${PREFIX}_decrypt:
.cfi_startproc
___
$code.=<<___;
    addi.d  $sp,$sp,-48
    st.d    $ra,$sp,40
    st.d    $fp,$sp,32
    vld     $vr0,$a0,0x0
    bl      _vpaes_preheat
    bl      _vpaes_decrypt_core
    vst     $vr0,$a1,0x0
___
$code.=<<___;
    ld.d    $ra,$sp,40
    ld.d    $fp,$sp,32
    addi.d  $sp,$sp,48
    jirl    $zero,$ra,0
.cfi_endproc
.size	${PREFIX}_decrypt,.-${PREFIX}_decrypt
___
{
my ($inp,$out,$len,$key,$ivp,$enc)=("$a0","$a1","$a2","$a3","$a4","$a5");
# void AES_cbc_encrypt (const void char *inp, unsigned char *out,
#                       size_t length, const AES_KEY *key,
#                       unsigned char *ivp,const int enc);
$code.=<<___;
.globl	${PREFIX}_cbc_encrypt
#.type	${PREFIX}_cbc_encrypt,\@function,6
.align	4
${PREFIX}_cbc_encrypt:
.cfi_startproc
    addi.d  $sp,$sp,-48
    st.d    $ra,$sp,40
    st.d    $fp,$sp,32

    ori     $t0,$len,0
    ori     $len,$key,0
    ori     $key,$t0,0
___
($len,$key)=($key,$len);
$code.=<<___;
    addi.d  $len,$len,-16
    blt     $len,$zero,.Lcbc_abort
___
$code.=<<___;
    vld     $vr6,$ivp,0		# load IV
    sub.d   $out,$out,$inp
    bl      _vpaes_preheat
    beqz    $a5,.Lcbc_dec_loop
    b       .Lcbc_enc_loop
.align	4
.Lcbc_enc_loop:
    vld     $vr0,$inp,0
    vxor.v  $vr0,$vr0,$vr6
    bl      _vpaes_encrypt_core
    vori.b  $vr6,$vr0,0
    add.d   $t0,$out,$inp
    vst     $vr0,$t0,0
    addi.d  $inp,$inp,16
    addi.d  $len,$len,-16
    bge     $len,$zero,.Lcbc_enc_loop
    b       .Lcbc_done
.align	4
.Lcbc_dec_loop:
    vld     $vr0,$inp,0
    vori.b  $vr7,$vr0,0
    bl      _vpaes_decrypt_core
    vxor.v  $vr0,$vr0,$vr6
    vori.b  $vr6,$vr7,0
    add.d   $t0,$out,$inp
    vst     $vr0,$t0,0
    addi.d  $inp,$inp,16
    addi.d  $len,$len,-16
    bge     $len,$zero,.Lcbc_dec_loop
.Lcbc_done:
    vst     $vr6,$ivp,0		# save IV
___
$code.=<<___;
.Lcbc_abort:
    ld.d    $ra,$sp,40
    ld.d    $fp,$sp,32
    addi.d  $sp,$sp,48
    jirl    $zero,$ra,0
.cfi_endproc
.size	${PREFIX}_cbc_encrypt,.-${PREFIX}_cbc_encrypt
___
}
{
$code.=<<___;
##
##  _aes_preheat
##
##  Fills register %a6 -> .aes_consts (so you can -fPIC)
##  and %vr9-%vr15 as specified below.
##
#.type	_vpaes_preheat,\@abi-omnipotent
.align	4
_vpaes_preheat:
.cfi_startproc
    la.local  $a6,Lk_s0F
    vld       $vr10,$a6,-0x20  		# Lk_inv
    vld       $vr11,$a6,-0x10 		# Lk_inv+16
    vld       $vr9,$a6,0		# Lk_s0F
    vld       $vr13,$a6,0x30		# Lk_sb1
    vld       $vr12,$a6,0x40		# Lk_sb1+16
    vld       $vr15,$a6,0x50		# Lk_sb2
    vld       $vr14,$a6,0x60		# Lk_sb2+16
    vldi      $vr18,0                   # $vr18 in this program is equal to 0
    jirl      $zero,$ra,0
.cfi_endproc
.size	_vpaes_preheat,.-_vpaes_preheat
___
}
########################################################
##                                                    ##
##                     Constants                      ##
##                                                    ##
########################################################
$code.=<<___;
.section .rodata
.align	6
Lk_inv:	# inv, inva
    .quad	0x0E05060F0D080110, 0x040703090A0B0C02
    .quad	0x01040A060F0B0710, 0x030D0E0C02050809

Lk_s0F:	# s0F
    .quad	0x0F0F0F0F0F0F0F0F, 0x0F0F0F0F0F0F0F0F

Lk_ipt:	# input transform (lo, hi)
    .quad	0xC2B2E8985A2A7000, 0xCABAE09052227808
    .quad	0x4C01307D317C4D00, 0xCD80B1FCB0FDCC81

Lk_sb1:	# sb1u, sb1t
    .quad	0xB19BE18FCB503E00, 0xA5DF7A6E142AF544
    .quad	0x3618D415FAE22300, 0x3BF7CCC10D2ED9EF
Lk_sb2:	# sb2u, sb2t
    .quad	0xE27A93C60B712400, 0x5EB7E955BC982FCD
    .quad	0x69EB88400AE12900, 0xC2A163C8AB82234A
Lk_sbo:	# sbou, sbot
    .quad	0xD0D26D176FBDC700, 0x15AABF7AC502A878
    .quad	0xCFE474A55FBB6A00, 0x8E1E90D1412B35FA

Lk_mc_forward:	# mc_forward
    .quad	0x0407060500030201, 0x0C0F0E0D080B0A09
    .quad	0x080B0A0904070605, 0x000302010C0F0E0D
    .quad	0x0C0F0E0D080B0A09, 0x0407060500030201
    .quad	0x000302010C0F0E0D, 0x080B0A0904070605

Lk_mc_backward:# mc_backward
    .quad	0x0605040702010003, 0x0E0D0C0F0A09080B
    .quad	0x020100030E0D0C0F, 0x0A09080B06050407
    .quad	0x0E0D0C0F0A09080B, 0x0605040702010003
    .quad	0x0A09080B06050407, 0x020100030E0D0C0F

Lk_sr:		# sr
    .quad	0x0706050403020100, 0x0F0E0D0C0B0A0908
    .quad	0x030E09040F0A0500, 0x0B06010C07020D08
    .quad	0x0F060D040B020900, 0x070E050C030A0108
    .quad	0x0B0E0104070A0D00, 0x0306090C0F020508

Lk_rcon:	# rcon
    .quad	0x1F8391B9AF9DEEB6, 0x702A98084D7C7D81

Lk_s63:	# s63: all equal to 0x63 transformed
    .quad	0x5B5B5B5B5B5B5B5B, 0x5B5B5B5B5B5B5B5B

Lk_opt:	# output transform
    .quad	0xFF9F4929D6B66000, 0xF7974121DEBE6808
    .quad	0x01EDBD5150BCEC00, 0xE10D5DB1B05C0CE0

Lk_deskew:	# deskew tables: inverts the sbox's "skew"
    .quad	0x07E4A34047A4E300, 0x1DFEB95A5DBEF91A
    .quad	0x5F36B5DC83EA6900, 0x2841C2ABF49D1E77

##
##  Decryption stuff
##  Key schedule constants
##
Lk_dksd:	# decryption key schedule: invskew x*D
    .quad	0xFEB91A5DA3E44700, 0x0740E3A45A1DBEF9
    .quad	0x41C277F4B5368300, 0x5FDC69EAAB289D1E
Lk_dksb:	# decryption key schedule: invskew x*B
    .quad	0x9A4FCA1F8550D500, 0x03D653861CC94C99
    .quad	0x115BEDA7B6FC4A00, 0xD993256F7E3482C8
Lk_dkse:	# decryption key schedule: invskew x*E + 0x63
    .quad	0xD5031CCA1FC9D600, 0x53859A4C994F5086
    .quad	0xA23196054FDC7BE8, 0xCD5EF96A20B31487
Lk_dks9:	# decryption key schedule: invskew x*9
    .quad	0xB6116FC87ED9A700, 0x4AED933482255BFC
    .quad	0x4576516227143300, 0x8BB89FACE9DAFDCE

##
##  Decryption stuff
##  Round function constants
##
Lk_dipt:	# decryption input transform
    .quad	0x0F505B040B545F00, 0x154A411E114E451A
    .quad	0x86E383E660056500, 0x12771772F491F194

Lk_dsb9:	# decryption sbox output *9*u, *9*t
    .quad	0x851C03539A86D600, 0xCAD51F504F994CC9
    .quad	0xC03B1789ECD74900, 0x725E2C9EB2FBA565
Lk_dsbd:	# decryption sbox output *D*u, *D*t
    .quad	0x7D57CCDFE6B1A200, 0xF56E9B13882A4439
    .quad	0x3CE2FAF724C6CB00, 0x2931180D15DEEFD3
Lk_dsbb:	# decryption sbox output *B*u, *B*t
    .quad	0xD022649296B44200, 0x602646F6B0F2D404
    .quad	0xC19498A6CD596700, 0xF3FF0C3E3255AA6B
Lk_dsbe:	# decryption sbox output *E*u, *E*t
    .quad	0x46F2929626D4D000, 0x2242600464B4F6B0
    .quad	0x0C55A6CDFFAAC100, 0x9467F36B98593E32
Lk_dsbo:	# decryption sbox final output
    .quad	0x1387EA537EF94000, 0xC7AA6DB9D4943E2D
    .quad	0x12D7560F93441D00, 0xCA4B8159D8C58E9C
.asciz	"Vector Permutation AES for loongarch64/lsx, Mike Hamburg (Stanford University)"
.align	6
___


$code =~ s/\`([^\`]*)\`/eval($1)/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
