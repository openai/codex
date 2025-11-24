#!/usr/bin/env perl

# Copyright (c) 2015, CloudFlare Ltd.
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

##############################################################################
#                                                                            #
# Author:  Vlad Krasnov                                                      #
#                                                                            #
##############################################################################

$flavour = shift;
$output  = shift;
if ($flavour =~ /\./) { $output = $flavour; undef $flavour; }

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\"";
*STDOUT=*OUT;

$avx = 2;

$code.=<<___;
.section .rodata
.align 64
chacha20_poly1305_constants:
.Lchacha20_consts:
.byte 'e','x','p','a','n','d',' ','3','2','-','b','y','t','e',' ','k'
.byte 'e','x','p','a','n','d',' ','3','2','-','b','y','t','e',' ','k'
.Lrol8:
.byte 3,0,1,2, 7,4,5,6, 11,8,9,10, 15,12,13,14
.byte 3,0,1,2, 7,4,5,6, 11,8,9,10, 15,12,13,14
.Lrol16:
.byte 2,3,0,1, 6,7,4,5, 10,11,8,9, 14,15,12,13
.byte 2,3,0,1, 6,7,4,5, 10,11,8,9, 14,15,12,13
.Lavx2_init:
.long 0,0,0,0
.Lsse_inc:
.long 1,0,0,0
.Lavx2_inc:
.long 2,0,0,0,2,0,0,0
.Lclamp:
.quad 0x0FFFFFFC0FFFFFFF, 0x0FFFFFFC0FFFFFFC
.quad 0xFFFFFFFFFFFFFFFF, 0xFFFFFFFFFFFFFFFF
.align 16
.Land_masks:
.byte 0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x00
.byte 0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff
.text
___

my ($oup,$inp,$inl,$adp,$keyp,$itr1,$itr2,$adl)=("%rdi","%rsi","%rbx","%rcx","%r9","%rcx","%r8","%r8");
my ($acc0,$acc1,$acc2)=map("%r$_",(10..12));
my ($t0,$t1,$t2,$t3)=("%r13","%r14","%r15","%r9");
my ($A0,$A1,$A2,$A3,$B0,$B1,$B2,$B3,$C0,$C1,$C2,$C3,$D0,$D1,$D2,$D3)=map("%xmm$_",(0..15));
my ($T0,$T1,$T2,$T3)=($A3,$B3,$C3,$D3);
my $xmm_storage = 0;
if ($win64) {
    $xmm_storage = 10*16;
}
my $xmm_store="0*16(%rbp)";
my $r_store="$xmm_storage+0*16(%rbp)";
my $s_store="$xmm_storage+1*16(%rbp)";
my $len_store="$xmm_storage+2*16(%rbp)";
my $state1_store="$xmm_storage+3*16(%rbp)";
my $state2_store="$xmm_storage+4*16(%rbp)";
my $tmp_store="$xmm_storage+5*16(%rbp)";
my $ctr0_store="$xmm_storage+6*16(%rbp)";
my $ctr1_store="$xmm_storage+7*16(%rbp)";
my $ctr2_store="$xmm_storage+8*16(%rbp)";
my $ctr3_store="$xmm_storage+9*16(%rbp)";

sub chacha_qr {
my ($a,$b,$c,$d,$t,$dir)=@_;
$code.="movdqa $t, $tmp_store\n" if ($dir =~ /store/);
$code.="paddd $b, $a
        pxor $a, $d
        pshufb .Lrol16(%rip), $d
        paddd $d, $c
        pxor $c, $b
        movdqa $b, $t
        pslld \$12, $t
        psrld \$20, $b
        pxor $t, $b
        paddd $b, $a
        pxor $a, $d
        pshufb .Lrol8(%rip), $d
        paddd $d, $c
        pxor $c, $b
        movdqa $b, $t
        pslld \$7, $t
        psrld \$25, $b
        pxor $t, $b\n";
$code.="palignr \$4, $b, $b
        palignr \$8, $c, $c
        palignr \$12, $d, $d\n" if ($dir =~ /left/);
$code.="palignr \$12, $b, $b
        palignr \$8, $c, $c
        palignr \$4, $d, $d\n" if ($dir =~ /right/);
$code.="movdqa $tmp_store, $t\n" if ($dir =~ /load/);
}

sub poly_add {
my ($src)=@_;
$code.="add 0+$src, $acc0
        adc 8+$src, $acc1
        adc \$1, $acc2\n";
}

sub poly_stage1 {
$code.="mov 0+$r_store, %rax
        mov %rax, $t2
        mul $acc0
        mov %rax, $t0
        mov %rdx, $t1
        mov 0+$r_store, %rax
        mul $acc1
        imulq $acc2, $t2
        add %rax, $t1
        adc %rdx, $t2\n";
}

sub poly_stage2 {
$code.="mov 8+$r_store, %rax
        mov %rax, $t3
        mul $acc0
        add %rax, $t1
        adc \$0, %rdx
        mov %rdx, $acc0
        mov 8+$r_store, %rax
        mul $acc1
        add %rax, $t2
        adc \$0, %rdx\n";
}

sub poly_stage3 {
$code.="imulq $acc2, $t3
        add $acc0, $t2
        adc %rdx, $t3\n";
}

# At the beginning of the reduce stage t = [t3:t2:t1:t0] is a product of
# r = [r1:r0] and acc = [acc2:acc1:acc0]
# r is 124 bits at most (due to clamping) and acc is 131 bits at most
# (acc2 is at most 4 before the addition and can be at most 6 when we add in
# the next block) therefore t is at most 255 bits big, and t3 is 63 bits.
sub poly_reduce_stage {
$code.="mov $t0, $acc0
        mov $t1, $acc1
        mov $t2, $acc2
        and \$3, $acc2 # At this point acc2 is 2 bits at most (value of 3)
        mov $t2, $t0
        and \$-4, $t0
        mov $t3, $t1
        shrd \$2, $t3, $t2
        shr \$2, $t3
        add $t0, $t2
        adc $t1, $t3 # No carry out since t3 is 61 bits and t1 is 63 bits
        add $t2, $acc0
        adc $t3, $acc1
        adc \$0, $acc2\n"; # At this point acc2 has the value of 4 at most
}

sub poly_mul {
    &poly_stage1();
    &poly_stage2();
    &poly_stage3();
    &poly_reduce_stage();
}

sub prep_state {
my ($n)=@_;
$code.="movdqa .Lchacha20_consts(%rip), $A0
        movdqa $state1_store, $B0
        movdqa $state2_store, $C0\n";
$code.="movdqa $A0, $A1
        movdqa $B0, $B1
        movdqa $C0, $C1\n" if ($n ge 2);
$code.="movdqa $A0, $A2
        movdqa $B0, $B2
        movdqa $C0, $C2\n" if ($n ge 3);
$code.="movdqa $A0, $A3
        movdqa $B0, $B3
        movdqa $C0, $C3\n" if ($n ge 4);
$code.="movdqa $ctr0_store, $D0
        paddd .Lsse_inc(%rip), $D0
        movdqa $D0, $ctr0_store\n" if ($n eq 1);
$code.="movdqa $ctr0_store, $D1
        paddd .Lsse_inc(%rip), $D1
        movdqa $D1, $D0
        paddd .Lsse_inc(%rip), $D0
        movdqa $D0, $ctr0_store
        movdqa $D1, $ctr1_store\n" if ($n eq 2);
$code.="movdqa $ctr0_store, $D2
        paddd .Lsse_inc(%rip), $D2
        movdqa $D2, $D1
        paddd .Lsse_inc(%rip), $D1
        movdqa $D1, $D0
        paddd .Lsse_inc(%rip), $D0
        movdqa $D0, $ctr0_store
        movdqa $D1, $ctr1_store
        movdqa $D2, $ctr2_store\n" if ($n eq 3);
$code.="movdqa $ctr0_store, $D3
        paddd .Lsse_inc(%rip), $D3
        movdqa $D3, $D2
        paddd .Lsse_inc(%rip), $D2
        movdqa $D2, $D1
        paddd .Lsse_inc(%rip), $D1
        movdqa $D1, $D0
        paddd .Lsse_inc(%rip), $D0
        movdqa $D0, $ctr0_store
        movdqa $D1, $ctr1_store
        movdqa $D2, $ctr2_store
        movdqa $D3, $ctr3_store\n" if ($n eq 4);
}

sub finalize_state {
my ($n)=@_;
$code.="paddd .Lchacha20_consts(%rip), $A3
        paddd $state1_store, $B3
        paddd $state2_store, $C3
        paddd $ctr3_store, $D3\n" if ($n eq 4);
$code.="paddd .Lchacha20_consts(%rip), $A2
        paddd $state1_store, $B2
        paddd $state2_store, $C2
        paddd $ctr2_store, $D2\n" if ($n ge 3);
$code.="paddd .Lchacha20_consts(%rip), $A1
        paddd $state1_store, $B1
        paddd $state2_store, $C1
        paddd $ctr1_store, $D1\n" if ($n ge 2);
$code.="paddd .Lchacha20_consts(%rip), $A0
        paddd $state1_store, $B0
        paddd $state2_store, $C0
        paddd $ctr0_store, $D0\n";
}

sub xor_stream {
my ($A, $B, $C, $D, $offset)=@_;
$code.="movdqu 0*16 + $offset($inp), $A3
        movdqu 1*16 + $offset($inp), $B3
        movdqu 2*16 + $offset($inp), $C3
        movdqu 3*16 + $offset($inp), $D3
        pxor $A3, $A
        pxor $B3, $B
        pxor $C3, $C
        pxor $D, $D3
        movdqu $A, 0*16 + $offset($oup)
        movdqu $B, 1*16 + $offset($oup)
        movdqu $C, 2*16 + $offset($oup)
        movdqu $D3, 3*16 + $offset($oup)\n";
}

sub xor_stream_using_temp {
my ($A, $B, $C, $D, $offset, $temp)=@_;
$code.="movdqa $temp, $tmp_store
        movdqu 0*16 + $offset($inp), $temp
        pxor $A, $temp
        movdqu $temp, 0*16 + $offset($oup)
        movdqu 1*16 + $offset($inp), $temp
        pxor $B, $temp
        movdqu $temp, 1*16 + $offset($oup)
        movdqu 2*16 + $offset($inp), $temp
        pxor $C, $temp
        movdqu $temp, 2*16 + $offset($oup)
        movdqu 3*16 + $offset($inp), $temp
        pxor $D, $temp
        movdqu $temp, 3*16 + $offset($oup)\n";
}

sub gen_chacha_round {
my ($rot1, $rot2, $shift)=@_;
my $round="";
$round.="movdqa $C0, $tmp_store\n" if ($rot1 eq 20);
$round.="movdqa $rot2, $C0
         paddd $B3, $A3
         paddd $B2, $A2
         paddd $B1, $A1
         paddd $B0, $A0
         pxor $A3, $D3
         pxor $A2, $D2
         pxor $A1, $D1
         pxor $A0, $D0
         pshufb $C0, $D3
         pshufb $C0, $D2
         pshufb $C0, $D1
         pshufb $C0, $D0
         movdqa $tmp_store, $C0
         paddd $D3, $C3
         paddd $D2, $C2
         paddd $D1, $C1
         paddd $D0, $C0
         pxor $C3, $B3
         pxor $C2, $B2
         pxor $C1, $B1
         pxor $C0, $B0
         movdqa $C0, $tmp_store
         movdqa $B3, $C0
         psrld \$$rot1, $C0
         pslld \$32-$rot1, $B3
         pxor $C0, $B3
         movdqa $B2, $C0
         psrld \$$rot1, $C0
         pslld \$32-$rot1, $B2
         pxor $C0, $B2
         movdqa $B1, $C0
         psrld \$$rot1, $C0
         pslld \$32-$rot1, $B1
         pxor $C0, $B1
         movdqa $B0, $C0
         psrld \$$rot1, $C0
         pslld \$32-$rot1, $B0
         pxor $C0, $B0\n";
($s1,$s2,$s3)=(4,8,12) if ($shift =~ /left/);
($s1,$s2,$s3)=(12,8,4) if ($shift =~ /right/);
$round.="movdqa $tmp_store, $C0
         palignr \$$s1, $B3, $B3
         palignr \$$s2, $C3, $C3
         palignr \$$s3, $D3, $D3
         palignr \$$s1, $B2, $B2
         palignr \$$s2, $C2, $C2
         palignr \$$s3, $D2, $D2
         palignr \$$s1, $B1, $B1
         palignr \$$s2, $C1, $C1
         palignr \$$s3, $D1, $D1
         palignr \$$s1, $B0, $B0
         palignr \$$s2, $C0, $C0
         palignr \$$s3, $D0, $D0\n"
if (($shift =~ /left/) || ($shift =~ /right/));
return $round;
};

$chacha_body = &gen_chacha_round(20, ".Lrol16(%rip)") .
               &gen_chacha_round(25, ".Lrol8(%rip)", "left") .
               &gen_chacha_round(20, ".Lrol16(%rip)") .
               &gen_chacha_round(25, ".Lrol8(%rip)", "right");

my @loop_body = split /\n/, $chacha_body;

sub emit_body {
my ($n)=@_;
    for (my $i=0; $i < $n; $i++) {
        $code=$code.shift(@loop_body)."\n";
    };
}

{
################################################################################
# void poly_hash_ad_internal();
$code.="
.type poly_hash_ad_internal,\@abi-omnipotent
.align 64
poly_hash_ad_internal:
.cfi_startproc
.cfi_def_cfa rsp, 8
    xor $acc0, $acc0
    xor $acc1, $acc1
    xor $acc2, $acc2
    cmp \$13,  $itr2
    jne .Lhash_ad_loop
.Lpoly_fast_tls_ad:
    # Special treatment for the TLS case of 13 bytes
    mov ($adp), $acc0
    mov 5($adp), $acc1
    shr \$24, $acc1
    mov \$1, $acc2\n";
    &poly_mul(); $code.="
    ret
.Lhash_ad_loop:
        # Hash in 16 byte chunk
        cmp \$16, $itr2
        jb .Lhash_ad_tail\n";
        &poly_add("0($adp)");
        &poly_mul(); $code.="
        lea 1*16($adp), $adp
        sub \$16, $itr2
    jmp .Lhash_ad_loop
.Lhash_ad_tail:
    cmp \$0, $itr2
    je .Lhash_ad_done
    # Hash last < 16 byte tail
    xor $t0, $t0
    xor $t1, $t1
    xor $t2, $t2
    add $itr2, $adp
.Lhash_ad_tail_loop:
        shld \$8, $t0, $t1
        shl \$8, $t0
        movzxb -1($adp), $t2
        xor $t2, $t0
        dec $adp
        dec $itr2
    jne .Lhash_ad_tail_loop

    add $t0, $acc0
    adc $t1, $acc1
    adc \$1, $acc2\n";
    &poly_mul(); $code.="
    # Finished AD
.Lhash_ad_done:
    ret
.cfi_endproc
.size poly_hash_ad_internal, .-poly_hash_ad_internal\n";
}

{
################################################################################
# void chacha20_poly1305_open(uint8_t *out_plaintext, const uint8_t *ciphertext,
#                             size_t plaintext_len, const uint8_t *ad,
#                             size_t ad_len,
#                             union chacha20_poly1305_open_data *aead_data)
#
$code.="
.globl chacha20_poly1305_open_sse41
.type chacha20_poly1305_open_sse41,\@function,6
.align 64
chacha20_poly1305_open_sse41:
.cfi_startproc
    _CET_ENDBR
    push %rbp
.cfi_push %rbp
    push %rbx
.cfi_push %rbx
    push %r12
.cfi_push %r12
    push %r13
.cfi_push %r13
    push %r14
.cfi_push %r14
    push %r15
.cfi_push %r15
    # We write the calculated authenticator back to keyp at the end, so save
    # the pointer on the stack too.
    push $keyp
.cfi_push $keyp
    sub \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset 288 + 32

    lea 32(%rsp), %rbp
    and \$-32, %rbp\n";
$code.="
    movaps %xmm6,16*0+$xmm_store
    movaps %xmm7,16*1+$xmm_store
    movaps %xmm8,16*2+$xmm_store
    movaps %xmm9,16*3+$xmm_store
    movaps %xmm10,16*4+$xmm_store
    movaps %xmm11,16*5+$xmm_store
    movaps %xmm12,16*6+$xmm_store
    movaps %xmm13,16*7+$xmm_store
    movaps %xmm14,16*8+$xmm_store
    movaps %xmm15,16*9+$xmm_store\n" if ($win64);
$code.="
    mov %rdx, $inl
    mov $adl, 0+$len_store
    mov $inl, 8+$len_store

    cmp \$128, $inl
    jbe .Lopen_sse_128
    # For long buffers, prepare the poly key first
    movdqa .Lchacha20_consts(%rip), $A0
    movdqu 0*16($keyp), $B0
    movdqu 1*16($keyp), $C0
    movdqu 2*16($keyp), $D0

    movdqa $D0, $T1
    # Store on stack, to free keyp
    movdqa $B0, $state1_store
    movdqa $C0, $state2_store
    movdqa $D0, $ctr0_store
    mov \$10, $acc0
.Lopen_sse_init_rounds:\n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right"); $code.="
        dec $acc0
    jne .Lopen_sse_init_rounds
    # A0|B0 hold the Poly1305 32-byte key, C0,D0 can be discarded
    paddd .Lchacha20_consts(%rip), $A0
    paddd $state1_store, $B0
    # Clamp and store the key
    pand .Lclamp(%rip), $A0
    movdqa $A0, $r_store
    movdqa $B0, $s_store
    # Hash
    mov $adl, $itr2
    call poly_hash_ad_internal
.Lopen_sse_main_loop:
        cmp \$16*16, $inl
        jb .Lopen_sse_tail
        # Load state, increment counter blocks\n";
        &prep_state(4); $code.="
        # There are 10 ChaCha20 iterations of 2QR each, so for 6 iterations we
        # hash 2 blocks, and for the remaining 4 only 1 block - for a total of 16
        mov \$4, $itr1
        mov $inp, $itr2
.Lopen_sse_main_loop_rounds:\n";
            &emit_body(20);
            &poly_add("0($itr2)"); $code.="
            lea 2*8($itr2), $itr2\n";
            &emit_body(20);
            &poly_stage1();
            &emit_body(20);
            &poly_stage2();
            &emit_body(20);
            &poly_stage3();
            &emit_body(20);
            &poly_reduce_stage();
            foreach $l (@loop_body) {$code.=$l."\n";}
            @loop_body = split /\n/, $chacha_body; $code.="
            dec $itr1
        jge .Lopen_sse_main_loop_rounds\n";
            &poly_add("0($itr2)");
            &poly_mul(); $code.="
            lea 2*8($itr2), $itr2
            cmp \$-6, $itr1
        jg .Lopen_sse_main_loop_rounds\n";
        &finalize_state(4);
        &xor_stream_using_temp($A3, $B3, $C3, $D3, "0*16", $D0);
        &xor_stream($A2, $B2, $C2, $D2, "4*16");
        &xor_stream($A1, $B1, $C1, $D1, "8*16");
        &xor_stream($A0, $B0, $C0, $tmp_store, "12*16"); $code.="
        lea 16*16($inp), $inp
        lea 16*16($oup), $oup
        sub \$16*16, $inl
    jmp .Lopen_sse_main_loop
.Lopen_sse_tail:
    # Handle the various tail sizes efficiently
    test $inl, $inl
    jz .Lopen_sse_finalize
    cmp \$12*16, $inl
    ja .Lopen_sse_tail_256
    cmp \$8*16, $inl
    ja .Lopen_sse_tail_192
    cmp \$4*16, $inl
    ja .Lopen_sse_tail_128\n";
###############################################################################
    # At most 64 bytes are left
    &prep_state(1); $code.="
    xor $itr2, $itr2
    mov $inl, $itr1
    cmp \$16, $itr1
    jb .Lopen_sse_tail_64_rounds
.Lopen_sse_tail_64_rounds_and_x1hash: \n";
        &poly_add("0($inp,$itr2)");
        &poly_mul(); $code.="
        sub \$16, $itr1
.Lopen_sse_tail_64_rounds:
        add \$16, $itr2\n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right"); $code.="
        cmp \$16, $itr1
    jae .Lopen_sse_tail_64_rounds_and_x1hash
        cmp \$10*16, $itr2
    jne .Lopen_sse_tail_64_rounds\n";
    &finalize_state(1); $code.="
    jmp .Lopen_sse_tail_64_dec_loop
###############################################################################
.Lopen_sse_tail_128:\n";
    # 65 - 128 bytes are left
    &prep_state(2); $code.="
    mov $inl, $itr1
    and \$-16, $itr1
    xor $itr2, $itr2
.Lopen_sse_tail_128_rounds_and_x1hash: \n";
        &poly_add("0($inp,$itr2)");
        &poly_mul(); $code.="
.Lopen_sse_tail_128_rounds:
        add \$16, $itr2\n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right");$code.="
        cmp $itr1, $itr2
    jb .Lopen_sse_tail_128_rounds_and_x1hash
        cmp \$10*16, $itr2
    jne .Lopen_sse_tail_128_rounds\n";
    &finalize_state(2);
    &xor_stream($A1, $B1, $C1, $D1, "0*16"); $code.="
    sub \$4*16, $inl
    lea 4*16($inp), $inp
    lea 4*16($oup), $oup
    jmp .Lopen_sse_tail_64_dec_loop
###############################################################################
.Lopen_sse_tail_192:\n";
    # 129 - 192 bytes are left
    &prep_state(3); $code.="
    mov $inl, $itr1
    mov \$10*16, $itr2
    cmp \$10*16, $itr1
    cmovg $itr2, $itr1
    and \$-16, $itr1
    xor $itr2, $itr2
.Lopen_sse_tail_192_rounds_and_x1hash: \n";
        &poly_add("0($inp,$itr2)");
        &poly_mul(); $code.="
.Lopen_sse_tail_192_rounds:
        add \$16, $itr2\n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        cmp $itr1, $itr2
    jb .Lopen_sse_tail_192_rounds_and_x1hash
        cmp \$10*16, $itr2
    jne .Lopen_sse_tail_192_rounds
    cmp \$11*16, $inl
    jb .Lopen_sse_tail_192_finish\n";
    &poly_add("10*16($inp)");
    &poly_mul(); $code.="
    cmp \$12*16, $inl
    jb .Lopen_sse_tail_192_finish\n";
    &poly_add("11*16($inp)");
    &poly_mul(); $code.="
.Lopen_sse_tail_192_finish: \n";
    &finalize_state(3);
    &xor_stream($A2, $B2, $C2, $D2, "0*16");
    &xor_stream($A1, $B1, $C1, $D1, "4*16"); $code.="
    sub \$8*16, $inl
    lea 8*16($inp), $inp
    lea 8*16($oup), $oup
    jmp .Lopen_sse_tail_64_dec_loop
###############################################################################
.Lopen_sse_tail_256:\n";
    # 193 - 255 bytes are left
    &prep_state(4); $code.="
    xor $itr2, $itr2
.Lopen_sse_tail_256_rounds_and_x1hash: \n";
        &poly_add("0($inp,$itr2)");
        &chacha_qr($A0,$B0,$C0,$D0,$C3,"store_left");
        &chacha_qr($A1,$B1,$C1,$D1,$C3,"left");
        &chacha_qr($A2,$B2,$C2,$D2,$C3,"left_load");
        &poly_stage1();
        &chacha_qr($A3,$B3,$C3,$D3,$C1,"store_left_load");
        &poly_stage2();
        &chacha_qr($A0,$B0,$C0,$D0,$C3,"store_right");
        &chacha_qr($A1,$B1,$C1,$D1,$C3,"right");
        &poly_stage3();
        &chacha_qr($A2,$B2,$C2,$D2,$C3,"right_load");
        &poly_reduce_stage();
        &chacha_qr($A3,$B3,$C3,$D3,$C1,"store_right_load"); $code.="
        add \$16, $itr2
        cmp \$10*16, $itr2
    jb .Lopen_sse_tail_256_rounds_and_x1hash

    mov $inl, $itr1
    and \$-16, $itr1
.Lopen_sse_tail_256_hash: \n";
        &poly_add("0($inp,$itr2)");
        &poly_mul(); $code.="
        add \$16, $itr2
        cmp $itr1, $itr2
    jb .Lopen_sse_tail_256_hash\n";
    &finalize_state(4);
    &xor_stream_using_temp($A3, $B3, $C3, $D3, "0*16", $D0);
    &xor_stream($A2, $B2, $C2, $D2, "4*16");
    &xor_stream($A1, $B1, $C1, $D1, "8*16"); $code.="
    movdqa $tmp_store, $D0
    sub \$12*16, $inl
    lea 12*16($inp), $inp
    lea 12*16($oup), $oup
###############################################################################
    # Decrypt the remaining data, 16B at a time, using existing stream
.Lopen_sse_tail_64_dec_loop:
    cmp \$16, $inl
    jb .Lopen_sse_tail_16_init
        sub \$16, $inl
        movdqu ($inp), $T0
        pxor $T0, $A0
        movdqu $A0, ($oup)
        lea 16($inp), $inp
        lea 16($oup), $oup
        movdqa $B0, $A0
        movdqa $C0, $B0
        movdqa $D0, $C0
    jmp .Lopen_sse_tail_64_dec_loop
.Lopen_sse_tail_16_init:
    movdqa $A0, $A1

    # Decrypt up to 16 bytes at the end.
.Lopen_sse_tail_16:
    test $inl, $inl
    jz .Lopen_sse_finalize

    # Read the final bytes into $T0. They need to be read in reverse order so
    # that they end up in the correct order in $T0.
    pxor $T0, $T0
    lea -1($inp,$inl), $inp
    movq $inl, $itr2
.Lopen_sse_tail_16_compose:
        pslldq \$1, $T0
        pinsrb \$0, ($inp), $T0
        sub \$1, $inp
        sub \$1, $itr2
        jnz .Lopen_sse_tail_16_compose

    movq $T0, $t0
    pextrq \$1, $T0, $t1
    # The final bytes of keystream are in $A1.
    pxor $A1, $T0

    # Copy the plaintext bytes out.
.Lopen_sse_tail_16_extract:
        pextrb \$0, $T0, ($oup)
        psrldq \$1, $T0
        add \$1, $oup
        sub \$1, $inl
    jne .Lopen_sse_tail_16_extract

    add $t0, $acc0
    adc $t1, $acc1
    adc \$1, $acc2\n";
    &poly_mul(); $code.="

.Lopen_sse_finalize:\n";
    &poly_add($len_store);
    &poly_mul(); $code.="
    # Final reduce
    mov $acc0, $t0
    mov $acc1, $t1
    mov $acc2, $t2
    sub \$-5, $acc0
    sbb \$-1, $acc1
    sbb \$3, $acc2
    cmovc $t0, $acc0
    cmovc $t1, $acc1
    cmovc $t2, $acc2
    # Add in s part of the key
    add 0+$s_store, $acc0
    adc 8+$s_store, $acc1\n";

$code.="
    movaps 16*0+$xmm_store, %xmm6
    movaps 16*1+$xmm_store, %xmm7
    movaps 16*2+$xmm_store, %xmm8
    movaps 16*3+$xmm_store, %xmm9
    movaps 16*4+$xmm_store, %xmm10
    movaps 16*5+$xmm_store, %xmm11
    movaps 16*6+$xmm_store, %xmm12
    movaps 16*7+$xmm_store, %xmm13
    movaps 16*8+$xmm_store, %xmm14
    movaps 16*9+$xmm_store, %xmm15\n" if ($win64);
$code.="
.cfi_remember_state
    add \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset -(288 + 32)
    # The tag replaces the key on return
    pop $keyp
.cfi_pop $keyp
    mov $acc0, ($keyp)
    mov $acc1, 8($keyp)
    pop %r15
.cfi_pop %r15
    pop %r14
.cfi_pop %r14
    pop %r13
.cfi_pop %r13
    pop %r12
.cfi_pop %r12
    pop %rbx
.cfi_pop %rbx
    pop %rbp
.cfi_pop %rbp
    ret
###############################################################################
.Lopen_sse_128:
.cfi_restore_state
    movdqu .Lchacha20_consts(%rip), $A0\nmovdqa $A0, $A1\nmovdqa $A0, $A2
    movdqu 0*16($keyp), $B0\nmovdqa $B0, $B1\nmovdqa $B0, $B2
    movdqu 1*16($keyp), $C0\nmovdqa $C0, $C1\nmovdqa $C0, $C2
    movdqu 2*16($keyp), $D0
    movdqa $D0, $D1\npaddd .Lsse_inc(%rip), $D1
    movdqa $D1, $D2\npaddd .Lsse_inc(%rip), $D2
    movdqa $B0, $T1\nmovdqa $C0, $T2\nmovdqa $D1, $T3
    mov \$10, $acc0

.Lopen_sse_128_rounds:  \n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"right"); $code.="
    dec $acc0
    jnz .Lopen_sse_128_rounds
    paddd .Lchacha20_consts(%rip), $A0
    paddd .Lchacha20_consts(%rip), $A1
    paddd .Lchacha20_consts(%rip), $A2
    paddd $T1, $B0\npaddd $T1, $B1\npaddd $T1, $B2
    paddd $T2, $C1\npaddd $T2, $C2
    paddd $T3, $D1
    paddd .Lsse_inc(%rip), $T3
    paddd $T3, $D2
    # Clamp and store the key
    pand .Lclamp(%rip), $A0
    movdqa $A0, $r_store
    movdqa $B0, $s_store
    # Hash
    mov $adl, $itr2
    call poly_hash_ad_internal
.Lopen_sse_128_xor_hash:
        cmp \$16, $inl
        jb .Lopen_sse_tail_16
        sub \$16, $inl\n";
        # Load for hashing
        &poly_add("0*8($inp)"); $code.="
        # Load for decryption
        movdqu 0*16($inp), $T0
        pxor $T0, $A1
        movdqu $A1, 0*16($oup)
        lea 1*16($inp), $inp
        lea 1*16($oup), $oup\n";
        &poly_mul(); $code.="
        # Shift the stream left
        movdqa $B1, $A1
        movdqa $C1, $B1
        movdqa $D1, $C1
        movdqa $A2, $D1
        movdqa $B2, $A2
        movdqa $C2, $B2
        movdqa $D2, $C2
    jmp .Lopen_sse_128_xor_hash
.size chacha20_poly1305_open_sse41, .-chacha20_poly1305_open_sse41
.cfi_endproc

################################################################################
################################################################################
# void chacha20_poly1305_seal(uint8_t *out_ciphertext, const uint8_t *plaintext,
#                             size_t plaintext_len, const uint8_t *ad,
#                             size_t ad_len,
#                             union chacha20_poly1305_seal_data *data);
.globl  chacha20_poly1305_seal_sse41
.type chacha20_poly1305_seal_sse41,\@function,6
.align 64
chacha20_poly1305_seal_sse41:
.cfi_startproc
    _CET_ENDBR
    push %rbp
.cfi_push %rbp
    push %rbx
.cfi_push %rbx
    push %r12
.cfi_push %r12
    push %r13
.cfi_push %r13
    push %r14
.cfi_push %r14
    push %r15
.cfi_push %r15   
# We write the calculated authenticator back to keyp at the end, so save
# the pointer on the stack too.
    push $keyp
.cfi_push $keyp
    sub \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset 288 + 32
    lea 32(%rsp), %rbp
    and \$-32, %rbp\n";
$code.="
    movaps %xmm6,16*0+$xmm_store
    movaps %xmm7,16*1+$xmm_store
    movaps %xmm8,16*2+$xmm_store
    movaps %xmm9,16*3+$xmm_store
    movaps %xmm10,16*4+$xmm_store
    movaps %xmm11,16*5+$xmm_store
    movaps %xmm12,16*6+$xmm_store
    movaps %xmm13,16*7+$xmm_store
    movaps %xmm14,16*8+$xmm_store
    movaps %xmm15,16*9+$xmm_store\n" if ($win64);
$code.="
    mov 56($keyp), $inl  # extra_in_len
    addq %rdx, $inl
    mov $adl, 0+$len_store
    mov $inl, 8+$len_store
    mov %rdx, $inl

    cmp \$128, $inl
    jbe .Lseal_sse_128
    # For longer buffers, prepare the poly key + some stream
    movdqa .Lchacha20_consts(%rip), $A0
    movdqu 0*16($keyp), $B0
    movdqu 1*16($keyp), $C0
    movdqu 2*16($keyp), $D0

    movdqa $A0, $A1
    movdqa $A0, $A2
    movdqa $A0, $A3
    movdqa $B0, $B1
    movdqa $B0, $B2
    movdqa $B0, $B3
    movdqa $C0, $C1
    movdqa $C0, $C2
    movdqa $C0, $C3
    movdqa $D0, $D3
    paddd .Lsse_inc(%rip), $D0
    movdqa $D0, $D2
    paddd .Lsse_inc(%rip), $D0
    movdqa $D0, $D1
    paddd .Lsse_inc(%rip), $D0
    # Store on stack
    movdqa $B0, $state1_store
    movdqa $C0, $state2_store
    movdqa $D0, $ctr0_store
    movdqa $D1, $ctr1_store
    movdqa $D2, $ctr2_store
    movdqa $D3, $ctr3_store
    mov \$10, $acc0
.Lseal_sse_init_rounds:  \n";
        foreach $l (@loop_body) {$code.=$l."\n";}
        @loop_body = split /\n/, $chacha_body; $code.="
        dec $acc0
    jnz .Lseal_sse_init_rounds\n";
    &finalize_state(4); $code.="
    # Clamp and store the key
    pand .Lclamp(%rip), $A3
    movdqa $A3, $r_store
    movdqa $B3, $s_store
    # Hash
    mov $adl, $itr2
    call poly_hash_ad_internal\n";
    &xor_stream($A2,$B2,$C2,$D2,"0*16");
    &xor_stream($A1,$B1,$C1,$D1,"4*16"); $code.="
    cmp \$12*16, $inl
    ja .Lseal_sse_main_init
    mov \$8*16, $itr1
    sub \$8*16, $inl
    lea 8*16($inp), $inp
    jmp .Lseal_sse_128_tail_hash
.Lseal_sse_main_init:\n";
    &xor_stream($A0, $B0, $C0, $D0, "8*16"); $code.="
    mov \$12*16, $itr1
    sub \$12*16, $inl
    lea 12*16($inp), $inp
    mov \$2, $itr1
    mov \$8, $itr2
    cmp \$4*16, $inl
    jbe .Lseal_sse_tail_64
    cmp \$8*16, $inl
    jbe .Lseal_sse_tail_128
    cmp \$12*16, $inl
    jbe .Lseal_sse_tail_192

.Lseal_sse_main_loop: \n";
    # The main loop
        &prep_state(4); $code.="
.align 32
.Lseal_sse_main_rounds: \n";
            &emit_body(20);
            &poly_add("0($oup)");
            &emit_body(20);
            &poly_stage1();
            &emit_body(20);
            &poly_stage2();
            &emit_body(20);
            &poly_stage3();
            &emit_body(20);
            &poly_reduce_stage();
            foreach $l (@loop_body) {$code.=$l."\n";}
            @loop_body = split /\n/, $chacha_body; $code.="
            lea 16($oup), $oup
            dec $itr2
        jge .Lseal_sse_main_rounds\n";
            &poly_add("0*8($oup)");
            &poly_mul(); $code.="
            lea 16($oup), $oup
            dec $itr1
        jg .Lseal_sse_main_rounds\n";

        &finalize_state(4);$code.="
        movdqa $D2, $tmp_store\n";
        &xor_stream_using_temp($A3,$B3,$C3,$D3,0*16,$D2); $code.="
        movdqa $tmp_store, $D2\n";
        &xor_stream($A2,$B2,$C2,$D2, 4*16);
        &xor_stream($A1,$B1,$C1,$D1, 8*16); $code.="
        cmp \$16*16, $inl
        ja .Lseal_sse_main_loop_xor

        mov \$12*16, $itr1
        sub \$12*16, $inl
        lea 12*16($inp), $inp
        jmp .Lseal_sse_128_tail_hash
.Lseal_sse_main_loop_xor: \n";
        &xor_stream($A0,$B0,$C0,$D0,"12*16"); $code.="
        lea 16*16($inp), $inp
        sub \$16*16, $inl
        mov \$6, $itr1
        mov \$4, $itr2
        cmp \$12*16, $inl
    jg .Lseal_sse_main_loop
    mov $inl, $itr1
    test $inl, $inl
    je .Lseal_sse_128_tail_hash
    mov \$6, $itr1
    cmp \$8*16, $inl
    ja .Lseal_sse_tail_192
    cmp \$4*16, $inl
    ja .Lseal_sse_tail_128
###############################################################################
.Lseal_sse_tail_64: \n";
    &prep_state(1); $code.="
.Lseal_sse_tail_64_rounds_and_x2hash: \n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 16($oup), $oup
.Lseal_sse_tail_64_rounds_and_x1hash: \n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 16($oup), $oup
    dec $itr1
    jg .Lseal_sse_tail_64_rounds_and_x2hash
    dec $itr2
    jge .Lseal_sse_tail_64_rounds_and_x1hash\n";
    &finalize_state(1); $code.="
    jmp .Lseal_sse_128_tail_xor
###############################################################################
.Lseal_sse_tail_128:\n";
    &prep_state(2); $code.="
.Lseal_sse_tail_128_rounds_and_x2hash: \n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 16($oup), $oup
.Lseal_sse_tail_128_rounds_and_x1hash: \n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &poly_add("0($oup)");
        &poly_mul();
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right"); $code.="
        lea 16($oup), $oup
    dec $itr1
    jg .Lseal_sse_tail_128_rounds_and_x2hash
    dec $itr2
    jge .Lseal_sse_tail_128_rounds_and_x1hash\n";
    &finalize_state(2);
    &xor_stream($A1,$B1,$C1,$D1,0*16); $code.="
    mov \$4*16, $itr1
    sub \$4*16, $inl
    lea 4*16($inp), $inp
    jmp .Lseal_sse_128_tail_hash
###############################################################################
.Lseal_sse_tail_192:\n";
    &prep_state(3); $code.="
.Lseal_sse_tail_192_rounds_and_x2hash: \n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 16($oup), $oup
.Lseal_sse_tail_192_rounds_and_x1hash: \n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"left");
        &poly_add("0($oup)");
        &poly_mul();
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        lea 16($oup), $oup
    dec $itr1
    jg .Lseal_sse_tail_192_rounds_and_x2hash
    dec $itr2
    jge .Lseal_sse_tail_192_rounds_and_x1hash\n";
    &finalize_state(3);
    &xor_stream($A2,$B2,$C2,$D2,0*16);
    &xor_stream($A1,$B1,$C1,$D1,4*16); $code.="
    mov \$8*16, $itr1
    sub \$8*16, $inl
    lea 8*16($inp), $inp
###############################################################################
.Lseal_sse_128_tail_hash:
        cmp \$16, $itr1
        jb .Lseal_sse_128_tail_xor\n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        sub \$16, $itr1
        lea 16($oup), $oup
    jmp .Lseal_sse_128_tail_hash

.Lseal_sse_128_tail_xor:
        cmp \$16, $inl
        jb .Lseal_sse_tail_16
        sub \$16, $inl
        # Load for decryption
        movdqu 0*16($inp), $T0
        pxor $T0, $A0
        movdqu $A0, 0*16($oup)
        # Then hash
        add 0*8($oup), $acc0
        adc 1*8($oup), $acc1
        adc \$1, $acc2
        lea 1*16($inp), $inp
        lea 1*16($oup), $oup\n";
        &poly_mul(); $code.="
        # Shift the stream left
        movdqa $B0, $A0
        movdqa $C0, $B0
        movdqa $D0, $C0
        movdqa $A1, $D0
        movdqa $B1, $A1
        movdqa $C1, $B1
        movdqa $D1, $C1
    jmp .Lseal_sse_128_tail_xor

.Lseal_sse_tail_16:
    test $inl, $inl
    jz .Lprocess_blocks_of_extra_in
    # We can only load the PT one byte at a time to avoid buffer overread
    mov $inl, $itr2
    mov $inl, $itr1
    lea -1($inp,$inl), $inp
    pxor $T3, $T3
.Lseal_sse_tail_16_compose:
        pslldq \$1, $T3
        pinsrb \$0, ($inp), $T3
        lea -1($inp), $inp
        dec $itr1
        jne .Lseal_sse_tail_16_compose

    # XOR the keystream with the plaintext.
    pxor $A0, $T3

    # Write ciphertext out, byte-by-byte.
    movq $inl, $itr1
    movdqu $T3, $A0
.Lseal_sse_tail_16_extract:
        pextrb \$0, $A0, ($oup)
        psrldq \$1, $A0
        add \$1, $oup
        sub \$1, $itr1
        jnz .Lseal_sse_tail_16_extract

    # $T3 contains the final (partial, non-empty) block of ciphertext which
    # needs to be fed into the Poly1305 state. The right-most $inl bytes of it
    # are valid. We need to fill it with extra_in bytes until full, or until we
    # run out of bytes.
    #
    # $keyp points to the tag output, which is actually a struct with the
    # extra_in pointer and length at offset 48.
    movq 288 + $xmm_storage + 32(%rsp), $keyp
    movq 56($keyp), $t1  # extra_in_len
    movq 48($keyp), $t0  # extra_in
    test $t1, $t1
    jz .Lprocess_partial_block  # Common case: no bytes of extra_in

    movq \$16, $t2
    subq $inl, $t2  # 16-$inl is the number of bytes that fit into $T3.
    cmpq $t2, $t1   # if extra_in_len < 16-$inl, only copy extra_in_len
                    # (note that AT&T syntax reverses the arguments)
    jge .Lload_extra_in
    movq $t1, $t2

.Lload_extra_in:
    # $t2 contains the number of bytes of extra_in (pointed to by $t0) to load
    # into $T3. They are loaded in reverse order.
    leaq -1($t0,$t2), $inp
    # Update extra_in and extra_in_len to reflect the bytes that are about to
    # be read.
    addq $t2, $t0
    subq $t2, $t1
    movq $t0, 48($keyp)
    movq $t1, 56($keyp)

    # Update $itr2, which is used to select the mask later on, to reflect the
    # extra bytes about to be added.
    addq $t2, $itr2

    # Load $t2 bytes of extra_in into $T2.
    pxor $T2, $T2
.Lload_extra_load_loop:
        pslldq \$1, $T2
        pinsrb \$0, ($inp), $T2
        lea -1($inp), $inp
        sub \$1, $t2
        jnz .Lload_extra_load_loop

    # Shift $T2 up the length of the remainder from the main encryption. Sadly,
    # the shift for an XMM register has to be a constant, thus we loop to do
    # this.
    movq $inl, $t2

.Lload_extra_shift_loop:
        pslldq \$1, $T2
        sub \$1, $t2
        jnz .Lload_extra_shift_loop

    # Mask $T3 (the remainder from the main encryption) so that superfluous
    # bytes are zero. This means that the non-zero bytes in $T2 and $T3 are
    # disjoint and so we can merge them with an OR.
    lea .Land_masks(%rip), $t2
    shl \$4, $inl
    pand -16($t2,$inl), $T3

    # Merge $T2 into $T3, forming the remainder block.
    por $T2, $T3

    # The block of ciphertext + extra_in is ready to be included in the
    # Poly1305 state.
    movq $T3, $t0
    pextrq \$1, $T3, $t1
    add $t0, $acc0
    adc $t1, $acc1
    adc \$1, $acc2\n";
    &poly_mul(); $code.="

.Lprocess_blocks_of_extra_in:
    # There may be additional bytes of extra_in to process.
    movq 288+32+$xmm_storage (%rsp), $keyp
    movq 48($keyp), $inp   # extra_in
    movq 56($keyp), $itr2  # extra_in_len
    movq $itr2, $itr1
    shr \$4, $itr2         # number of blocks

.Lprocess_extra_hash_loop:
        jz process_extra_in_trailer\n";
        &poly_add("0($inp)");
        &poly_mul(); $code.="
        leaq 16($inp), $inp
        subq \$1, $itr2
        jmp .Lprocess_extra_hash_loop
process_extra_in_trailer:
    andq \$15, $itr1       # remaining num bytes (<16) of extra_in
    movq $itr1, $inl
    jz .Ldo_length_block
    leaq -1($inp,$itr1), $inp

.Lprocess_extra_in_trailer_load:
        pslldq \$1, $T3
        pinsrb \$0, ($inp), $T3
        lea -1($inp), $inp
        sub \$1, $itr1
        jnz .Lprocess_extra_in_trailer_load

.Lprocess_partial_block:
    # $T3 contains $inl bytes of data to be fed into Poly1305. $inl != 0
    lea .Land_masks(%rip), $t2
    shl \$4, $inl
    pand -16($t2,$inl), $T3
    movq $T3, $t0
    pextrq \$1, $T3, $t1
    add $t0, $acc0
    adc $t1, $acc1
    adc \$1, $acc2\n";
    &poly_mul(); $code.="

.Ldo_length_block:\n";
    &poly_add($len_store);
    &poly_mul(); $code.="
    # Final reduce
    mov $acc0, $t0
    mov $acc1, $t1
    mov $acc2, $t2
    sub \$-5, $acc0
    sbb \$-1, $acc1
    sbb \$3, $acc2
    cmovc $t0, $acc0
    cmovc $t1, $acc1
    cmovc $t2, $acc2
    # Add in s part of the key
    add 0+$s_store, $acc0
    adc 8+$s_store, $acc1\n";

$code.="
    movaps 16*0+$xmm_store, %xmm6
    movaps 16*1+$xmm_store, %xmm7
    movaps 16*2+$xmm_store, %xmm8
    movaps 16*3+$xmm_store, %xmm9
    movaps 16*4+$xmm_store, %xmm10
    movaps 16*5+$xmm_store, %xmm11
    movaps 16*6+$xmm_store, %xmm12
    movaps 16*7+$xmm_store, %xmm13
    movaps 16*8+$xmm_store, %xmm14
    movaps 16*9+$xmm_store, %xmm15\n" if ($win64);
$code.="
.cfi_remember_state
    add \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset -(288 + 32)
    # The tag replaces the key on return
    pop $keyp
.cfi_pop $keyp
    mov $acc0, ($keyp)
    mov $acc1, 8($keyp)
    pop %r15
.cfi_pop %r15
    pop %r14
.cfi_pop %r14
    pop %r13
.cfi_pop %r13
    pop %r12
.cfi_pop %r12
    pop %rbx
.cfi_pop %rbx
    pop %rbp
.cfi_pop %rbp
    ret
################################################################################
.Lseal_sse_128:
.cfi_restore_state
    movdqu .Lchacha20_consts(%rip), $A0\nmovdqa $A0, $A1\nmovdqa $A0, $A2
    movdqu 0*16($keyp), $B0\nmovdqa $B0, $B1\nmovdqa $B0, $B2
    movdqu 1*16($keyp), $C0\nmovdqa $C0, $C1\nmovdqa $C0, $C2
    movdqu 2*16($keyp), $D2
    movdqa $D2, $D0\npaddd .Lsse_inc(%rip), $D0
    movdqa $D0, $D1\npaddd .Lsse_inc(%rip), $D1
    movdqa $B0, $T1\nmovdqa $C0, $T2\nmovdqa $D0, $T3
    mov \$10, $acc0

.Lseal_sse_128_rounds:\n";
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        dec $acc0
    jnz .Lseal_sse_128_rounds
    paddd .Lchacha20_consts(%rip), $A0
    paddd .Lchacha20_consts(%rip), $A1
    paddd .Lchacha20_consts(%rip), $A2
    paddd $T1, $B0\npaddd $T1, $B1\npaddd $T1, $B2
    paddd $T2, $C0\npaddd $T2, $C1
    paddd $T3, $D0
    paddd .Lsse_inc(%rip), $T3
    paddd $T3, $D1
    # Clamp and store the key
    pand .Lclamp(%rip), $A2
    movdqa $A2, $r_store
    movdqa $B2, $s_store
    # Hash
    mov %r8, $itr2
    call poly_hash_ad_internal
    jmp .Lseal_sse_128_tail_xor
.size chacha20_poly1305_seal_sse41, .-chacha20_poly1305_seal_sse41
.cfi_endproc\n";
}

if ($avx>1) {

($A0,$A1,$A2,$A3,$B0,$B1,$B2,$B3,$C0,$C1,$C2,$C3,$D0,$D1,$D2,$D3)=map("%ymm$_",(0..15));
my ($A0x,$A1x,$A2x,$A3x,$B0x,$B1x,$B2x,$B3x,$C0x,$C1x,$C2x,$C3x,$D0x,$D1x,$D2x,$D3x)=map("%xmm$_",(0..15));
($T0,$T1,$T2,$T3)=($A3,$B3,$C3,$D3);
$state1_store="$xmm_storage+2*32(%rbp)";
$state2_store="$xmm_storage+3*32(%rbp)";
$tmp_store="$xmm_storage+4*32(%rbp)";
$ctr0_store="$xmm_storage+5*32(%rbp)";
$ctr1_store="$xmm_storage+6*32(%rbp)";
$ctr2_store="$xmm_storage+7*32(%rbp)";
$ctr3_store="$xmm_storage+8*32(%rbp)";

sub chacha_qr_avx2 {
my ($a,$b,$c,$d,$t,$dir)=@_;
$code.=<<___ if ($dir =~ /store/);
    vmovdqa $t, $tmp_store
___
$code.=<<___;
    vpaddd $b, $a, $a
    vpxor $a, $d, $d
    vpshufb .Lrol16(%rip), $d, $d
    vpaddd $d, $c, $c
    vpxor $c, $b, $b
    vpsrld \$20, $b, $t
    vpslld \$12, $b, $b
    vpxor $t, $b, $b
    vpaddd $b, $a, $a
    vpxor $a, $d, $d
    vpshufb .Lrol8(%rip), $d, $d
    vpaddd $d, $c, $c
    vpxor $c, $b, $b
    vpslld \$7, $b, $t
    vpsrld \$25, $b, $b
    vpxor $t, $b, $b
___
$code.=<<___ if ($dir =~ /left/);
    vpalignr \$12, $d, $d, $d
    vpalignr \$8, $c, $c, $c
    vpalignr \$4, $b, $b, $b
___
$code.=<<___ if ($dir =~ /right/);
    vpalignr \$4, $d, $d, $d
    vpalignr \$8, $c, $c, $c
    vpalignr \$12, $b, $b, $b
___
$code.=<<___ if ($dir =~ /load/);
    vmovdqa $tmp_store, $t
___
}

sub prep_state_avx2 {
my ($n)=@_;
$code.=<<___;
    vmovdqa .Lchacha20_consts(%rip), $A0
    vmovdqa $state1_store, $B0
    vmovdqa $state2_store, $C0
___
$code.=<<___ if ($n ge 2);
    vmovdqa $A0, $A1
    vmovdqa $B0, $B1
    vmovdqa $C0, $C1
___
$code.=<<___ if ($n ge 3);
    vmovdqa $A0, $A2
    vmovdqa $B0, $B2
    vmovdqa $C0, $C2
___
$code.=<<___ if ($n ge 4);
    vmovdqa $A0, $A3
    vmovdqa $B0, $B3
    vmovdqa $C0, $C3
___
$code.=<<___ if ($n eq 1);
    vmovdqa .Lavx2_inc(%rip), $D0
    vpaddd $ctr0_store, $D0, $D0
    vmovdqa $D0, $ctr0_store
___
$code.=<<___ if ($n eq 2);
    vmovdqa .Lavx2_inc(%rip), $D0
    vpaddd $ctr0_store, $D0, $D1
    vpaddd $D1, $D0, $D0
    vmovdqa $D0, $ctr0_store
    vmovdqa $D1, $ctr1_store
___
$code.=<<___ if ($n eq 3);
    vmovdqa .Lavx2_inc(%rip), $D0
    vpaddd $ctr0_store, $D0, $D2
    vpaddd $D2, $D0, $D1
    vpaddd $D1, $D0, $D0
    vmovdqa $D0, $ctr0_store
    vmovdqa $D1, $ctr1_store
    vmovdqa $D2, $ctr2_store
___
$code.=<<___ if ($n eq 4);
    vmovdqa .Lavx2_inc(%rip), $D0
    vpaddd $ctr0_store, $D0, $D3
    vpaddd $D3, $D0, $D2
    vpaddd $D2, $D0, $D1
    vpaddd $D1, $D0, $D0
    vmovdqa $D3, $ctr3_store
    vmovdqa $D2, $ctr2_store
    vmovdqa $D1, $ctr1_store
    vmovdqa $D0, $ctr0_store
___
}

sub finalize_state_avx2 {
my ($n)=@_;
$code.=<<___ if ($n eq 4);
    vpaddd .Lchacha20_consts(%rip), $A3, $A3
    vpaddd $state1_store, $B3, $B3
    vpaddd $state2_store, $C3, $C3
    vpaddd $ctr3_store, $D3, $D3
___
$code.=<<___ if ($n ge 3);
    vpaddd .Lchacha20_consts(%rip), $A2, $A2
    vpaddd $state1_store, $B2, $B2
    vpaddd $state2_store, $C2, $C2
    vpaddd $ctr2_store, $D2, $D2
___
$code.=<<___ if ($n ge 2);
    vpaddd .Lchacha20_consts(%rip), $A1, $A1
    vpaddd $state1_store, $B1, $B1
    vpaddd $state2_store, $C1, $C1
    vpaddd $ctr1_store, $D1, $D1
___
$code.=<<___;
    vpaddd .Lchacha20_consts(%rip), $A0, $A0
    vpaddd $state1_store, $B0, $B0
    vpaddd $state2_store, $C0, $C0
    vpaddd $ctr0_store, $D0, $D0
___
}

sub xor_stream_avx2 {
my ($A, $B, $C, $D, $offset, $hlp)=@_;
$code.=<<___;
    vperm2i128 \$0x02, $A, $B, $hlp
    vperm2i128 \$0x13, $A, $B, $B
    vperm2i128 \$0x02, $C, $D, $A
    vperm2i128 \$0x13, $C, $D, $C
    vpxor 0*32+$offset($inp), $hlp, $hlp
    vpxor 1*32+$offset($inp), $A, $A
    vpxor 2*32+$offset($inp), $B, $B
    vpxor 3*32+$offset($inp), $C, $C
    vmovdqu $hlp, 0*32+$offset($oup)
    vmovdqu $A, 1*32+$offset($oup)
    vmovdqu $B, 2*32+$offset($oup)
    vmovdqu $C, 3*32+$offset($oup)
___
}

sub finish_stream_avx2 {
my ($A, $B, $C, $D, $hlp)=@_;
$code.=<<___;
    vperm2i128 \$0x13, $A, $B, $hlp
    vperm2i128 \$0x02, $A, $B, $A
    vperm2i128 \$0x02, $C, $D, $B
    vperm2i128 \$0x13, $C, $D, $D
    vmovdqa $hlp, $C
___
}

sub poly_stage1_mulx {
$code.=<<___;
    mov 0+$r_store, %rdx
    mov %rdx, $t2
    mulx $acc0, $t0, $t1
    mulx $acc1, %rax, %rdx
    imulq $acc2, $t2
    add %rax, $t1
    adc %rdx, $t2
___
}

sub poly_stage2_mulx {
$code.=<<___;
    mov 8+$r_store, %rdx
    mulx $acc0, $acc0, %rax
    add $acc0, $t1
    mulx $acc1, $acc1, $t3
    adc $acc1, $t2
    adc \$0, $t3
    imulq $acc2, %rdx
___
}

sub poly_stage3_mulx {
$code.=<<___;
    add %rax, $t2
    adc %rdx, $t3
___
}

sub poly_mul_mulx {
    &poly_stage1_mulx();
    &poly_stage2_mulx();
    &poly_stage3_mulx();
    &poly_reduce_stage();
}

sub gen_chacha_round_avx2 {
my ($rot1, $rot2, $shift)=@_;
my $round="";
$round=$round ."vmovdqa $C0, $tmp_store\n" if ($rot1 eq 20);
$round=$round ."vmovdqa $rot2, $C0
                vpaddd $B3, $A3, $A3
                vpaddd $B2, $A2, $A2
                vpaddd $B1, $A1, $A1
                vpaddd $B0, $A0, $A0
                vpxor $A3, $D3, $D3
                vpxor $A2, $D2, $D2
                vpxor $A1, $D1, $D1
                vpxor $A0, $D0, $D0
                vpshufb $C0, $D3, $D3
                vpshufb $C0, $D2, $D2
                vpshufb $C0, $D1, $D1
                vpshufb $C0, $D0, $D0
                vpaddd $D3, $C3, $C3
                vpaddd $D2, $C2, $C2
                vpaddd $D1, $C1, $C1
                vpaddd $tmp_store, $D0, $C0
                vpxor $C3, $B3, $B3
                vpxor $C2, $B2, $B2
                vpxor $C1, $B1, $B1
                vpxor $C0, $B0, $B0
                vmovdqa $C0, $tmp_store
                vpsrld \$$rot1, $B3, $C0
                vpslld \$32-$rot1, $B3, $B3
                vpxor $C0, $B3, $B3
                vpsrld \$$rot1, $B2, $C0
                vpslld \$32-$rot1, $B2, $B2
                vpxor $C0, $B2, $B2
                vpsrld \$$rot1, $B1, $C0
                vpslld \$32-$rot1, $B1, $B1
                vpxor $C0, $B1, $B1
                vpsrld \$$rot1, $B0, $C0
                vpslld \$32-$rot1, $B0, $B0
                vpxor $C0, $B0, $B0\n";
($s1,$s2,$s3)=(4,8,12) if ($shift =~ /left/);
($s1,$s2,$s3)=(12,8,4) if ($shift =~ /right/);
$round=$round ."vmovdqa $tmp_store, $C0
                vpalignr \$$s1, $B3, $B3, $B3
                vpalignr \$$s2, $C3, $C3, $C3
                vpalignr \$$s3, $D3, $D3, $D3
                vpalignr \$$s1, $B2, $B2, $B2
                vpalignr \$$s2, $C2, $C2, $C2
                vpalignr \$$s3, $D2, $D2, $D2
                vpalignr \$$s1, $B1, $B1, $B1
                vpalignr \$$s2, $C1, $C1, $C1
                vpalignr \$$s3, $D1, $D1, $D1
                vpalignr \$$s1, $B0, $B0, $B0
                vpalignr \$$s2, $C0, $C0, $C0
                vpalignr \$$s3, $D0, $D0, $D0\n"
if (($shift =~ /left/) || ($shift =~ /right/));
return $round;
};

$chacha_body = &gen_chacha_round_avx2(20, ".Lrol16(%rip)") .
               &gen_chacha_round_avx2(25, ".Lrol8(%rip)", "left") .
               &gen_chacha_round_avx2(20, ".Lrol16(%rip)") .
               &gen_chacha_round_avx2(25, ".Lrol8(%rip)", "right");

@loop_body = split /\n/, $chacha_body;

$code.="
###############################################################################
.globl chacha20_poly1305_open_avx2
.type chacha20_poly1305_open_avx2,\@function,6
.align 64
chacha20_poly1305_open_avx2:
.cfi_startproc
    _CET_ENDBR
    push %rbp
.cfi_push %rbp
    push %rbx
.cfi_push %rbx
    push %r12
.cfi_push %r12
    push %r13
.cfi_push %r13
    push %r14
.cfi_push %r14
    push %r15
.cfi_push %r15
    # We write the calculated authenticator back to keyp at the end, so save
    # the pointer on the stack too.
    push $keyp
.cfi_push $keyp
    sub \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset 288 + 32

    lea 32(%rsp), %rbp
    and \$-32, %rbp\n";
$code.="
    movaps %xmm6,16*0+$xmm_store
    movaps %xmm7,16*1+$xmm_store
    movaps %xmm8,16*2+$xmm_store
    movaps %xmm9,16*3+$xmm_store
    movaps %xmm10,16*4+$xmm_store
    movaps %xmm11,16*5+$xmm_store
    movaps %xmm12,16*6+$xmm_store
    movaps %xmm13,16*7+$xmm_store
    movaps %xmm14,16*8+$xmm_store
    movaps %xmm15,16*9+$xmm_store\n" if ($win64);
$code.="
    mov %rdx, $inl
    mov $adl, 0+$len_store
    mov $inl, 8+$len_store

    vzeroupper
    vmovdqa .Lchacha20_consts(%rip), $A0
    vbroadcasti128 0*16($keyp), $B0
    vbroadcasti128 1*16($keyp), $C0
    vbroadcasti128 2*16($keyp), $D0
    vpaddd .Lavx2_init(%rip), $D0, $D0
    cmp \$6*32, $inl
    jbe .Lopen_avx2_192
    cmp \$10*32, $inl
    jbe .Lopen_avx2_320

    vmovdqa $B0, $state1_store
    vmovdqa $C0, $state2_store
    vmovdqa $D0, $ctr0_store
    mov \$10, $acc0
.Lopen_avx2_init_rounds:  \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right"); $code.="
        dec $acc0
    jne .Lopen_avx2_init_rounds
    vpaddd .Lchacha20_consts(%rip), $A0, $A0
    vpaddd $state1_store, $B0, $B0
    vpaddd $state2_store, $C0, $C0
    vpaddd $ctr0_store, $D0, $D0

    vperm2i128 \$0x02, $A0, $B0, $T0
    # Clamp and store key
    vpand .Lclamp(%rip), $T0, $T0
    vmovdqa $T0, $r_store
    # Stream for the first 64 bytes
    vperm2i128 \$0x13, $A0, $B0, $A0
    vperm2i128 \$0x13, $C0, $D0, $B0
    # Hash AD + first 64 bytes
    mov $adl, $itr2
    call poly_hash_ad_internal
    # Hash first 64 bytes
    xor $itr1, $itr1
.Lopen_avx2_init_hash: \n";
       &poly_add("0($inp,$itr1)");
       &poly_mul(); $code.="
       add \$16, $itr1
       cmp \$2*32, $itr1
    jne .Lopen_avx2_init_hash
    # Decrypt first 64 bytes
    vpxor 0*32($inp), $A0, $A0
    vpxor 1*32($inp), $B0, $B0
    # Store first 64 bytes of decrypted data
    vmovdqu $A0, 0*32($oup)
    vmovdqu $B0, 1*32($oup)
    lea 2*32($inp), $inp
    lea 2*32($oup), $oup
    sub \$2*32, $inl
.Lopen_avx2_main_loop:
        # Hash and decrypt 512 bytes each iteration
        cmp \$16*32, $inl
        jb .Lopen_avx2_main_loop_done\n";
        &prep_state_avx2(4); $code.=" 
        xor $itr1, $itr1
.Lopen_avx2_main_loop_rounds: \n";
            &poly_add("0*8($inp,$itr1)");
            &emit_body(10);
            &poly_stage1_mulx();
            &emit_body(9);
            &poly_stage2_mulx();
            &emit_body(12);
            &poly_stage3_mulx();
            &emit_body(10);
            &poly_reduce_stage();
            &emit_body(9);
            &poly_add("2*8($inp,$itr1)");
            &emit_body(8);
            &poly_stage1_mulx();
            &emit_body(18);
            &poly_stage2_mulx();
            &emit_body(18);
            &poly_stage3_mulx();
            &emit_body(9);
            &poly_reduce_stage();
            &emit_body(8);
            &poly_add("4*8($inp,$itr1)"); $code.="
            lea 6*8($itr1), $itr1\n";
            &emit_body(18);
            &poly_stage1_mulx();
            &emit_body(8);
            &poly_stage2_mulx();
            &emit_body(8);
            &poly_stage3_mulx();
            &emit_body(18);
            &poly_reduce_stage();
            foreach $l (@loop_body) {$code.=$l."\n";}
            @loop_body = split /\n/, $chacha_body; $code.="
            cmp \$10*6*8, $itr1
        jne .Lopen_avx2_main_loop_rounds\n";
        &finalize_state_avx2(4); $code.="
        vmovdqa $A0, $tmp_store\n";
        &poly_add("10*6*8($inp)");
        &xor_stream_avx2($A3, $B3, $C3, $D3, 0*32, $A0); $code.="
        vmovdqa $tmp_store, $A0\n";
        &poly_mul();
        &xor_stream_avx2($A2, $B2, $C2, $D2, 4*32, $A3);
        &poly_add("10*6*8+2*8($inp)");
        &xor_stream_avx2($A1, $B1, $C1, $D1, 8*32, $A3);
        &poly_mul();
        &xor_stream_avx2($A0, $B0, $C0, $D0, 12*32, $A3); $code.="
        lea 16*32($inp), $inp
        lea 16*32($oup), $oup
        sub \$16*32, $inl
    jmp .Lopen_avx2_main_loop
.Lopen_avx2_main_loop_done:
    test $inl, $inl
    vzeroupper
    je .Lopen_sse_finalize

    cmp \$12*32, $inl
    ja .Lopen_avx2_tail_512
    cmp \$8*32, $inl
    ja .Lopen_avx2_tail_384
    cmp \$4*32, $inl
    ja .Lopen_avx2_tail_256\n";
###############################################################################
    # 1-128 bytes left
    &prep_state_avx2(1); $code.="
    xor $itr2, $itr2
    mov $inl, $itr1
    and \$-16, $itr1
    test $itr1, $itr1
    je .Lopen_avx2_tail_128_rounds # Have nothing to hash
.Lopen_avx2_tail_128_rounds_and_x1hash: \n";
        &poly_add("0*8($inp,$itr2)");
        &poly_mul(); $code.="
.Lopen_avx2_tail_128_rounds:
        add \$16, $itr2\n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right"); $code.="
        cmp $itr1, $itr2
    jb .Lopen_avx2_tail_128_rounds_and_x1hash
        cmp \$160, $itr2
    jne .Lopen_avx2_tail_128_rounds\n";
    &finalize_state_avx2(1);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T0); $code.="
    jmp .Lopen_avx2_tail_128_xor
###############################################################################
.Lopen_avx2_tail_256: \n";
    # 129-256 bytes left
    &prep_state_avx2(2); $code.="
    mov $inl, $tmp_store
    mov $inl, $itr1
    sub \$4*32, $itr1
    shr \$4, $itr1
    mov \$10, $itr2
    cmp \$10, $itr1
    cmovg $itr2, $itr1
    mov $inp, $inl
    xor $itr2, $itr2
.Lopen_avx2_tail_256_rounds_and_x1hash: \n";
        &poly_add("0*8($inl)");
        &poly_mul_mulx(); $code.="
        lea 16($inl), $inl
.Lopen_avx2_tail_256_rounds: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left"); $code.="
        inc $itr2\n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        cmp $itr1, $itr2
    jb .Lopen_avx2_tail_256_rounds_and_x1hash
        cmp \$10, $itr2
    jne .Lopen_avx2_tail_256_rounds
    mov $inl, $itr2
    sub $inp, $inl
    mov $inl, $itr1
    mov $tmp_store, $inl
.Lopen_avx2_tail_256_hash:
        add \$16, $itr1
        cmp $inl, $itr1
        jg .Lopen_avx2_tail_256_done\n";
        &poly_add("0*8($itr2)");
        &poly_mul_mulx(); $code.="
        lea 16($itr2), $itr2
    jmp .Lopen_avx2_tail_256_hash
.Lopen_avx2_tail_256_done: \n";
    &finalize_state_avx2(2);
    &xor_stream_avx2($A1, $B1, $C1, $D1, 0*32, $T0);
    &finish_stream_avx2($A0, $B0, $C0, $D0, $T0); $code.="
    lea 4*32($inp), $inp
    lea 4*32($oup), $oup
    sub \$4*32, $inl
    jmp .Lopen_avx2_tail_128_xor
###############################################################################
.Lopen_avx2_tail_384: \n";
    # 257-383 bytes left
    &prep_state_avx2(3); $code.="
    mov $inl, $tmp_store
    mov $inl, $itr1
    sub \$8*32, $itr1
    shr \$4, $itr1
    add \$6, $itr1
    mov \$10, $itr2
    cmp \$10, $itr1
    cmovg $itr2, $itr1
    mov $inp, $inl
    xor $itr2, $itr2
.Lopen_avx2_tail_384_rounds_and_x2hash: \n";
        &poly_add("0*8($inl)");
        &poly_mul_mulx(); $code.="
        lea 16($inl), $inl
.Lopen_avx2_tail_384_rounds_and_x1hash: \n";
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &poly_add("0*8($inl)");
        &poly_mul(); $code.="
        lea 16($inl), $inl
        inc $itr2\n";
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right"); $code.="
        cmp $itr1, $itr2
    jb  .Lopen_avx2_tail_384_rounds_and_x2hash
        cmp \$10, $itr2
    jne .Lopen_avx2_tail_384_rounds_and_x1hash
    mov $inl, $itr2
    sub $inp, $inl
    mov $inl, $itr1
    mov $tmp_store, $inl
.Lopen_avx2_384_tail_hash:
        add \$16, $itr1
        cmp $inl, $itr1
        jg .Lopen_avx2_384_tail_done\n";
        &poly_add("0*8($itr2)");
        &poly_mul_mulx(); $code.="
        lea 16($itr2), $itr2
    jmp .Lopen_avx2_384_tail_hash
.Lopen_avx2_384_tail_done: \n";
    &finalize_state_avx2(3);
    &xor_stream_avx2($A2, $B2, $C2, $D2, 0*32, $T0);
    &xor_stream_avx2($A1, $B1, $C1, $D1, 4*32, $T0);
    &finish_stream_avx2($A0, $B0, $C0, $D0, $T0); $code.="
    lea 8*32($inp), $inp
    lea 8*32($oup), $oup
    sub \$8*32, $inl
    jmp .Lopen_avx2_tail_128_xor
###############################################################################
.Lopen_avx2_tail_512: \n";
    # 384-512 bytes left
    &prep_state_avx2(4); $code.="
    xor $itr1, $itr1
    mov $inp, $itr2
.Lopen_avx2_tail_512_rounds_and_x2hash: \n";
        &poly_add("0*8($itr2)");
        &poly_mul(); $code.="
        lea 2*8($itr2), $itr2
.Lopen_avx2_tail_512_rounds_and_x1hash: \n";
        &emit_body(37);
        &poly_add("0*8($itr2)");
        &poly_mul_mulx();
        &emit_body(48);
        &poly_add("2*8($itr2)");
        &poly_mul_mulx(); $code.="
        lea 4*8($itr2), $itr2\n";
        foreach $l (@loop_body) {$code.=$l."\n";}
        @loop_body = split /\n/, $chacha_body; $code.="
        inc $itr1
        cmp \$4, $itr1
    jl  .Lopen_avx2_tail_512_rounds_and_x2hash
        cmp \$10, $itr1
    jne .Lopen_avx2_tail_512_rounds_and_x1hash
    mov $inl, $itr1
    sub \$12*32, $itr1
    and \$-16, $itr1
.Lopen_avx2_tail_512_hash:
        test $itr1, $itr1
        je .Lopen_avx2_tail_512_done\n";
        &poly_add("0*8($itr2)");
        &poly_mul_mulx(); $code.="
        lea 2*8($itr2), $itr2
        sub \$2*8, $itr1
    jmp .Lopen_avx2_tail_512_hash
.Lopen_avx2_tail_512_done: \n";
    &finalize_state_avx2(4); $code.="
    vmovdqa $A0, $tmp_store\n";
    &xor_stream_avx2($A3, $B3, $C3, $D3, 0*32, $A0); $code.="
    vmovdqa $tmp_store, $A0\n";
    &xor_stream_avx2($A2, $B2, $C2, $D2, 4*32, $A3);
    &xor_stream_avx2($A1, $B1, $C1, $D1, 8*32, $A3);
    &finish_stream_avx2($A0, $B0, $C0, $D0, $A3); $code.="
    lea 12*32($inp), $inp
    lea 12*32($oup), $oup
    sub \$12*32, $inl
.Lopen_avx2_tail_128_xor:
    cmp \$32, $inl
    jb .Lopen_avx2_tail_32_xor
        sub \$32, $inl
        vpxor ($inp), $A0, $A0
        vmovdqu $A0, ($oup)
        lea 1*32($inp), $inp
        lea 1*32($oup), $oup
        vmovdqa $B0, $A0
        vmovdqa $C0, $B0
        vmovdqa $D0, $C0
    jmp .Lopen_avx2_tail_128_xor
.Lopen_avx2_tail_32_xor:
    cmp \$16, $inl
    vmovdqa $A0x, $A1x
    jb .Lopen_avx2_exit
    sub \$16, $inl
    #load for decryption
    vpxor ($inp), $A0x, $A1x
    vmovdqu $A1x, ($oup)
    lea 1*16($inp), $inp
    lea 1*16($oup), $oup
    vperm2i128 \$0x11, $A0, $A0, $A0
    vmovdqa $A0x, $A1x
.Lopen_avx2_exit:
    vzeroupper
    jmp .Lopen_sse_tail_16
###############################################################################
.Lopen_avx2_192:
    vmovdqa $A0, $A1
    vmovdqa $A0, $A2
    vmovdqa $B0, $B1
    vmovdqa $B0, $B2
    vmovdqa $C0, $C1
    vmovdqa $C0, $C2
    vpaddd .Lavx2_inc(%rip), $D0, $D1
    vmovdqa $D0, $T2
    vmovdqa $D1, $T3
    mov \$10, $acc0
.Lopen_avx2_192_rounds: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right"); $code.="
        dec $acc0
    jne .Lopen_avx2_192_rounds
    vpaddd $A2, $A0, $A0
    vpaddd $A2, $A1, $A1
    vpaddd $B2, $B0, $B0
    vpaddd $B2, $B1, $B1
    vpaddd $C2, $C0, $C0
    vpaddd $C2, $C1, $C1
    vpaddd $T2, $D0, $D0
    vpaddd $T3, $D1, $D1
    vperm2i128 \$0x02, $A0, $B0, $T0
    # Clamp and store the key
    vpand .Lclamp(%rip), $T0, $T0
    vmovdqa $T0, $r_store
    # Stream for up to 192 bytes
    vperm2i128 \$0x13, $A0, $B0, $A0
    vperm2i128 \$0x13, $C0, $D0, $B0
    vperm2i128 \$0x02, $A1, $B1, $C0
    vperm2i128 \$0x02, $C1, $D1, $D0
    vperm2i128 \$0x13, $A1, $B1, $A1
    vperm2i128 \$0x13, $C1, $D1, $B1
.Lopen_avx2_short:
    mov $adl, $itr2
    call poly_hash_ad_internal
.Lopen_avx2_short_hash_and_xor_loop:
        cmp \$32, $inl
        jb .Lopen_avx2_short_tail_32
        sub \$32, $inl\n";
        # Load + hash
        &poly_add("0*8($inp)");
        &poly_mul();
        &poly_add("2*8($inp)");
        &poly_mul(); $code.="
        # Load + decrypt
        vpxor ($inp), $A0, $A0
        vmovdqu $A0, ($oup)
        lea 1*32($inp), $inp
        lea 1*32($oup), $oup
        # Shift stream
        vmovdqa $B0, $A0
        vmovdqa $C0, $B0
        vmovdqa $D0, $C0
        vmovdqa $A1, $D0
        vmovdqa $B1, $A1
        vmovdqa $C1, $B1
        vmovdqa $D1, $C1
        vmovdqa $A2, $D1
        vmovdqa $B2, $A2
    jmp .Lopen_avx2_short_hash_and_xor_loop
.Lopen_avx2_short_tail_32:
    cmp \$16, $inl
    vmovdqa $A0x, $A1x
    jb .Lopen_avx2_short_tail_32_exit
    sub \$16, $inl\n";
    &poly_add("0*8($inp)");
    &poly_mul(); $code.="
    vpxor ($inp), $A0x, $A3x
    vmovdqu $A3x, ($oup)
    lea 1*16($inp), $inp
    lea 1*16($oup), $oup
    vextracti128 \$1, $A0, $A1x
.Lopen_avx2_short_tail_32_exit:
    vzeroupper
    jmp .Lopen_sse_tail_16
###############################################################################
.Lopen_avx2_320:
    vmovdqa $A0, $A1
    vmovdqa $A0, $A2
    vmovdqa $B0, $B1
    vmovdqa $B0, $B2
    vmovdqa $C0, $C1
    vmovdqa $C0, $C2
    vpaddd .Lavx2_inc(%rip), $D0, $D1
    vpaddd .Lavx2_inc(%rip), $D1, $D2
    vmovdqa $B0, $T1
    vmovdqa $C0, $T2
    vmovdqa $D0, $ctr0_store
    vmovdqa $D1, $ctr1_store
    vmovdqa $D2, $ctr2_store
    mov \$10, $acc0
.Lopen_avx2_320_rounds:  \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        dec $acc0
    jne .Lopen_avx2_320_rounds
    vpaddd .Lchacha20_consts(%rip), $A0, $A0
    vpaddd .Lchacha20_consts(%rip), $A1, $A1
    vpaddd .Lchacha20_consts(%rip), $A2, $A2
    vpaddd $T1, $B0, $B0
    vpaddd $T1, $B1, $B1
    vpaddd $T1, $B2, $B2
    vpaddd $T2, $C0, $C0
    vpaddd $T2, $C1, $C1
    vpaddd $T2, $C2, $C2
    vpaddd $ctr0_store, $D0, $D0
    vpaddd $ctr1_store, $D1, $D1
    vpaddd $ctr2_store, $D2, $D2
    vperm2i128 \$0x02, $A0, $B0, $T0
    # Clamp and store the key
    vpand .Lclamp(%rip), $T0, $T0
    vmovdqa $T0, $r_store
    # Stream for up to 320 bytes
    vperm2i128 \$0x13, $A0, $B0, $A0
    vperm2i128 \$0x13, $C0, $D0, $B0
    vperm2i128 \$0x02, $A1, $B1, $C0
    vperm2i128 \$0x02, $C1, $D1, $D0
    vperm2i128 \$0x13, $A1, $B1, $A1
    vperm2i128 \$0x13, $C1, $D1, $B1
    vperm2i128 \$0x02, $A2, $B2, $C1
    vperm2i128 \$0x02, $C2, $D2, $D1
    vperm2i128 \$0x13, $A2, $B2, $A2
    vperm2i128 \$0x13, $C2, $D2, $B2
    jmp .Lopen_avx2_short
.size chacha20_poly1305_open_avx2, .-chacha20_poly1305_open_avx2
.cfi_endproc
###############################################################################
###############################################################################
.globl chacha20_poly1305_seal_avx2
.type chacha20_poly1305_seal_avx2,\@function,6
.align 64
chacha20_poly1305_seal_avx2:
.cfi_startproc
    _CET_ENDBR
    push %rbp
.cfi_push %rbp
    push %rbx
.cfi_push %rbx
    push %r12
.cfi_push %r12
    push %r13
.cfi_push %r13
    push %r14
.cfi_push %r14
    push %r15
.cfi_push %r15   
# We write the calculated authenticator back to keyp at the end, so save
# the pointer on the stack too.
    push $keyp
.cfi_push $keyp
    sub \$288 + $xmm_storage + 32, %rsp
.cfi_adjust_cfa_offset 288 + 32
    lea 32(%rsp), %rbp
    and \$-32, %rbp\n";
$code.="
    movaps %xmm6,16*0+$xmm_store
    movaps %xmm7,16*1+$xmm_store
    movaps %xmm8,16*2+$xmm_store
    movaps %xmm9,16*3+$xmm_store
    movaps %xmm10,16*4+$xmm_store
    movaps %xmm11,16*5+$xmm_store
    movaps %xmm12,16*6+$xmm_store
    movaps %xmm13,16*7+$xmm_store
    movaps %xmm14,16*8+$xmm_store
    movaps %xmm15,16*9+$xmm_store\n" if ($win64);
$code.="
    mov 56($keyp), $inl  # extra_in_len
    addq %rdx, $inl
    mov $adl, 0+$len_store
    mov $inl, 8+$len_store
    mov %rdx, $inl

    vzeroupper
    vmovdqa .Lchacha20_consts(%rip), $A0
    vbroadcasti128 0*16($keyp), $B0
    vbroadcasti128 1*16($keyp), $C0
    vbroadcasti128 2*16($keyp), $D0
    vpaddd .Lavx2_init(%rip), $D0, $D0
    cmp \$6*32, $inl
    jbe .Lseal_avx2_192
    cmp \$10*32, $inl
    jbe .Lseal_avx2_320
    vmovdqa $A0, $A1
    vmovdqa $A0, $A2
    vmovdqa $A0, $A3
    vmovdqa $B0, $B1
    vmovdqa $B0, $B2
    vmovdqa $B0, $B3
    vmovdqa $B0, $state1_store
    vmovdqa $C0, $C1
    vmovdqa $C0, $C2
    vmovdqa $C0, $C3
    vmovdqa $C0, $state2_store
    vmovdqa $D0, $D3
    vpaddd .Lavx2_inc(%rip), $D3, $D2
    vpaddd .Lavx2_inc(%rip), $D2, $D1
    vpaddd .Lavx2_inc(%rip), $D1, $D0
    vmovdqa $D0, $ctr0_store
    vmovdqa $D1, $ctr1_store
    vmovdqa $D2, $ctr2_store
    vmovdqa $D3, $ctr3_store
    mov \$10, $acc0
.Lseal_avx2_init_rounds: \n";
        foreach $l (@loop_body) {$code.=$l."\n";}
        @loop_body = split /\n/, $chacha_body; $code.="
        dec $acc0
        jnz .Lseal_avx2_init_rounds\n";
    &finalize_state_avx2(4); $code.="
    vperm2i128 \$0x13, $C3, $D3, $C3
    vperm2i128 \$0x02, $A3, $B3, $D3
    vperm2i128 \$0x13, $A3, $B3, $A3
    vpand .Lclamp(%rip), $D3, $D3
    vmovdqa $D3, $r_store
    mov $adl, $itr2
    call poly_hash_ad_internal
    # Safely store 320 bytes (otherwise would handle with optimized call)
    vpxor 0*32($inp), $A3, $A3
    vpxor 1*32($inp), $C3, $C3
    vmovdqu $A3, 0*32($oup)
    vmovdqu $C3, 1*32($oup)\n";
    &xor_stream_avx2($A2,$B2,$C2,$D2,2*32,$T3);
    &xor_stream_avx2($A1,$B1,$C1,$D1,6*32,$T3);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T3); $code.="
    lea 10*32($inp), $inp
    sub \$10*32, $inl
    mov \$10*32, $itr1
    cmp \$4*32, $inl
    jbe .Lseal_avx2_short_hash_remainder
    vpxor 0*32($inp), $A0, $A0
    vpxor 1*32($inp), $B0, $B0
    vpxor 2*32($inp), $C0, $C0
    vpxor 3*32($inp), $D0, $D0
    vmovdqu $A0, 10*32($oup)
    vmovdqu $B0, 11*32($oup)
    vmovdqu $C0, 12*32($oup)
    vmovdqu $D0, 13*32($oup)
    lea 4*32($inp), $inp
    sub \$4*32, $inl
    mov \$8, $itr1
    mov \$2, $itr2
    cmp \$4*32, $inl
    jbe .Lseal_avx2_tail_128
    cmp \$8*32, $inl
    jbe .Lseal_avx2_tail_256
    cmp \$12*32, $inl
    jbe .Lseal_avx2_tail_384
    cmp \$16*32, $inl
    jbe .Lseal_avx2_tail_512\n";
    # We have 448 bytes to hash, but main loop hashes 512 bytes at a time - perform some rounds, before the main loop
    &prep_state_avx2(4);
    foreach $l (@loop_body) {$code.=$l."\n";}
    @loop_body = split /\n/, $chacha_body;
    &emit_body(41);
    @loop_body = split /\n/, $chacha_body; $code.="
    sub \$16, $oup
    mov \$9, $itr1
    jmp .Lseal_avx2_main_loop_rounds_entry
.align 32
.Lseal_avx2_main_loop: \n";
        &prep_state_avx2(4); $code.="
        mov \$10, $itr1
.align 32
.Lseal_avx2_main_loop_rounds: \n";
            &poly_add("0*8($oup)");
            &emit_body(10);
            &poly_stage1_mulx();
            &emit_body(9);
            &poly_stage2_mulx();
            &emit_body(12);
            &poly_stage3_mulx();
            &emit_body(10);
            &poly_reduce_stage(); $code.="
.Lseal_avx2_main_loop_rounds_entry: \n";
            &emit_body(9);
            &poly_add("2*8($oup)");
            &emit_body(8);
            &poly_stage1_mulx();
            &emit_body(18);
            &poly_stage2_mulx();
            &emit_body(18);
            &poly_stage3_mulx();
            &emit_body(9);
            &poly_reduce_stage();
            &emit_body(8);
            &poly_add("4*8($oup)"); $code.="
            lea 6*8($oup), $oup\n";
            &emit_body(18);
            &poly_stage1_mulx();
            &emit_body(8);
            &poly_stage2_mulx();
            &emit_body(8);
            &poly_stage3_mulx();
            &emit_body(18);
            &poly_reduce_stage();
            foreach $l (@loop_body) {$code.=$l."\n";}
            @loop_body = split /\n/, $chacha_body; $code.="
            dec $itr1
        jne .Lseal_avx2_main_loop_rounds\n";
        &finalize_state_avx2(4); $code.="
        vmovdqa $A0, $tmp_store\n";
        &poly_add("0*8($oup)");
        &poly_mul_mulx();
        &poly_add("2*8($oup)");
        &poly_mul_mulx(); $code.="
        lea 4*8($oup), $oup\n";
        &xor_stream_avx2($A3, $B3, $C3, $D3, 0*32, $A0); $code.="
        vmovdqa $tmp_store, $A0\n";
        &xor_stream_avx2($A2, $B2, $C2, $D2, 4*32, $A3);
        &xor_stream_avx2($A1, $B1, $C1, $D1, 8*32, $A3);
        &xor_stream_avx2($A0, $B0, $C0, $D0, 12*32, $A3); $code.="
        lea 16*32($inp), $inp
        sub \$16*32, $inl
        cmp \$16*32, $inl
    jg .Lseal_avx2_main_loop
\n";
    &poly_add("0*8($oup)");
    &poly_mul_mulx();
    &poly_add("2*8($oup)");
    &poly_mul_mulx(); $code.="
    lea 4*8($oup), $oup
    mov \$10, $itr1
    xor $itr2, $itr2

    cmp \$12*32, $inl
    ja  .Lseal_avx2_tail_512
    cmp \$8*32, $inl
    ja  .Lseal_avx2_tail_384
    cmp \$4*32, $inl
    ja  .Lseal_avx2_tail_256
###############################################################################
.Lseal_avx2_tail_128:\n";
    &prep_state_avx2(1); $code.="
.Lseal_avx2_tail_128_rounds_and_3xhash: \n";
        &poly_add("0($oup)");
        &poly_mul_mulx(); $code.="
        lea 2*8($oup), $oup
.Lseal_avx2_tail_128_rounds_and_2xhash: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &poly_add("0*8($oup)");
        &poly_mul_mulx();
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &poly_add("2*8($oup)");
        &poly_mul_mulx(); $code.="
        lea 4*8($oup), $oup
        dec $itr1
    jg  .Lseal_avx2_tail_128_rounds_and_3xhash
        dec $itr2
    jge .Lseal_avx2_tail_128_rounds_and_2xhash\n";
    &finalize_state_avx2(1);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T0); $code.="
    jmp .Lseal_avx2_short_loop
###############################################################################
.Lseal_avx2_tail_256:\n";
    &prep_state_avx2(2); $code.="
.Lseal_avx2_tail_256_rounds_and_3xhash: \n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 2*8($oup), $oup
.Lseal_avx2_tail_256_rounds_and_2xhash: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &poly_add("0*8($oup)");
        &poly_mul();
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &poly_add("2*8($oup)");
        &poly_mul(); $code.="
        lea 4*8($oup), $oup
        dec $itr1
    jg  .Lseal_avx2_tail_256_rounds_and_3xhash
        dec $itr2
    jge .Lseal_avx2_tail_256_rounds_and_2xhash\n";
    &finalize_state_avx2(2);
    &xor_stream_avx2($A1,$B1,$C1,$D1,0*32,$T0);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T0); $code.="
    mov \$4*32, $itr1
    lea 4*32($inp), $inp
    sub \$4*32, $inl
    jmp .Lseal_avx2_short_hash_remainder
###############################################################################
.Lseal_avx2_tail_384:\n";
    &prep_state_avx2(3); $code.="
.Lseal_avx2_tail_384_rounds_and_3xhash: \n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        lea 2*8($oup), $oup
.Lseal_avx2_tail_384_rounds_and_2xhash: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &poly_add("0*8($oup)");
        &poly_mul();
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &poly_add("2*8($oup)");
        &poly_mul();
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        lea 4*8($oup), $oup
        dec $itr1
    jg  .Lseal_avx2_tail_384_rounds_and_3xhash
        dec $itr2
    jge .Lseal_avx2_tail_384_rounds_and_2xhash\n";
    &finalize_state_avx2(3);
    &xor_stream_avx2($A2,$B2,$C2,$D2,0*32,$T0);
    &xor_stream_avx2($A1,$B1,$C1,$D1,4*32,$T0);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T0); $code.="
    mov \$8*32, $itr1
    lea 8*32($inp), $inp
    sub \$8*32, $inl
    jmp .Lseal_avx2_short_hash_remainder
###############################################################################
.Lseal_avx2_tail_512:\n";
    &prep_state_avx2(4); $code.="
.Lseal_avx2_tail_512_rounds_and_3xhash: \n";
        &poly_add("0($oup)");
        &poly_mul_mulx(); $code.="
        lea 2*8($oup), $oup
.Lseal_avx2_tail_512_rounds_and_2xhash: \n";
        &emit_body(20);
        &poly_add("0*8($oup)");
        &emit_body(20);
        &poly_stage1_mulx();
        &emit_body(20);
        &poly_stage2_mulx();
        &emit_body(20);
        &poly_stage3_mulx();
        &emit_body(20);
        &poly_reduce_stage();
        &emit_body(20);
        &poly_add("2*8($oup)");
        &emit_body(20);
        &poly_stage1_mulx();
        &emit_body(20);
        &poly_stage2_mulx();
        &emit_body(20);
        &poly_stage3_mulx();
        &emit_body(20);
        &poly_reduce_stage();
        foreach $l (@loop_body) {$code.=$l."\n";}
        @loop_body = split /\n/, $chacha_body; $code.="
        lea 4*8($oup), $oup
        dec $itr1
    jg .Lseal_avx2_tail_512_rounds_and_3xhash
        dec $itr2
    jge .Lseal_avx2_tail_512_rounds_and_2xhash\n";
    &finalize_state_avx2(4); $code.="
    vmovdqa $A0, $tmp_store\n";
    &xor_stream_avx2($A3, $B3, $C3, $D3, 0*32, $A0); $code.="
    vmovdqa $tmp_store, $A0\n";
    &xor_stream_avx2($A2, $B2, $C2, $D2, 4*32, $A3);
    &xor_stream_avx2($A1, $B1, $C1, $D1, 8*32, $A3);
    &finish_stream_avx2($A0,$B0,$C0,$D0,$T0); $code.="
    mov \$12*32, $itr1
    lea 12*32($inp), $inp
    sub \$12*32, $inl
    jmp .Lseal_avx2_short_hash_remainder
################################################################################
.Lseal_avx2_320:
    vmovdqa $A0, $A1
    vmovdqa $A0, $A2
    vmovdqa $B0, $B1
    vmovdqa $B0, $B2
    vmovdqa $C0, $C1
    vmovdqa $C0, $C2
    vpaddd .Lavx2_inc(%rip), $D0, $D1
    vpaddd .Lavx2_inc(%rip), $D1, $D2
    vmovdqa $B0, $T1
    vmovdqa $C0, $T2
    vmovdqa $D0, $ctr0_store
    vmovdqa $D1, $ctr1_store
    vmovdqa $D2, $ctr2_store
    mov \$10, $acc0
.Lseal_avx2_320_rounds: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right");
        &chacha_qr_avx2($A2,$B2,$C2,$D2,$T0,"right"); $code.="
        dec $acc0
    jne .Lseal_avx2_320_rounds
    vpaddd .Lchacha20_consts(%rip), $A0, $A0
    vpaddd .Lchacha20_consts(%rip), $A1, $A1
    vpaddd .Lchacha20_consts(%rip), $A2, $A2
    vpaddd $T1, $B0, $B0
    vpaddd $T1, $B1, $B1
    vpaddd $T1, $B2, $B2
    vpaddd $T2, $C0, $C0
    vpaddd $T2, $C1, $C1
    vpaddd $T2, $C2, $C2
    vpaddd $ctr0_store, $D0, $D0
    vpaddd $ctr1_store, $D1, $D1
    vpaddd $ctr2_store, $D2, $D2
    vperm2i128 \$0x02, $A0, $B0, $T0
    # Clamp and store the key
    vpand .Lclamp(%rip), $T0, $T0
    vmovdqa $T0, $r_store
    # Stream for up to 320 bytes
    vperm2i128 \$0x13, $A0, $B0, $A0
    vperm2i128 \$0x13, $C0, $D0, $B0
    vperm2i128 \$0x02, $A1, $B1, $C0
    vperm2i128 \$0x02, $C1, $D1, $D0
    vperm2i128 \$0x13, $A1, $B1, $A1
    vperm2i128 \$0x13, $C1, $D1, $B1
    vperm2i128 \$0x02, $A2, $B2, $C1
    vperm2i128 \$0x02, $C2, $D2, $D1
    vperm2i128 \$0x13, $A2, $B2, $A2
    vperm2i128 \$0x13, $C2, $D2, $B2
    jmp .Lseal_avx2_short
################################################################################
.Lseal_avx2_192:
    vmovdqa $A0, $A1
    vmovdqa $A0, $A2
    vmovdqa $B0, $B1
    vmovdqa $B0, $B2
    vmovdqa $C0, $C1
    vmovdqa $C0, $C2
    vpaddd .Lavx2_inc(%rip), $D0, $D1
    vmovdqa $D0, $T2
    vmovdqa $D1, $T3
    mov \$10, $acc0
.Lseal_avx2_192_rounds: \n";
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"left");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"left");
        &chacha_qr_avx2($A0,$B0,$C0,$D0,$T0,"right");
        &chacha_qr_avx2($A1,$B1,$C1,$D1,$T0,"right"); $code.="
        dec $acc0
    jne .Lseal_avx2_192_rounds
    vpaddd $A2, $A0, $A0
    vpaddd $A2, $A1, $A1
    vpaddd $B2, $B0, $B0
    vpaddd $B2, $B1, $B1
    vpaddd $C2, $C0, $C0
    vpaddd $C2, $C1, $C1
    vpaddd $T2, $D0, $D0
    vpaddd $T3, $D1, $D1
    vperm2i128 \$0x02, $A0, $B0, $T0
    # Clamp and store the key
    vpand .Lclamp(%rip), $T0, $T0
    vmovdqa $T0, $r_store
    # Stream for up to 192 bytes
    vperm2i128 \$0x13, $A0, $B0, $A0
    vperm2i128 \$0x13, $C0, $D0, $B0
    vperm2i128 \$0x02, $A1, $B1, $C0
    vperm2i128 \$0x02, $C1, $D1, $D0
    vperm2i128 \$0x13, $A1, $B1, $A1
    vperm2i128 \$0x13, $C1, $D1, $B1
.Lseal_avx2_short:
    mov $adl, $itr2
    call poly_hash_ad_internal
    xor $itr1, $itr1
.Lseal_avx2_short_hash_remainder:
        cmp \$16, $itr1
        jb .Lseal_avx2_short_loop\n";
        &poly_add("0($oup)");
        &poly_mul(); $code.="
        sub \$16, $itr1
        add \$16, $oup
    jmp .Lseal_avx2_short_hash_remainder
.Lseal_avx2_short_loop:
        cmp \$32, $inl
        jb .Lseal_avx2_short_tail
        sub \$32, $inl
        # Encrypt
        vpxor ($inp), $A0, $A0
        vmovdqu $A0, ($oup)
        lea 1*32($inp), $inp
        # Load + hash\n";
        &poly_add("0*8($oup)");
        &poly_mul();
        &poly_add("2*8($oup)");
        &poly_mul(); $code.="
        lea 1*32($oup), $oup
        # Shift stream
        vmovdqa $B0, $A0
        vmovdqa $C0, $B0
        vmovdqa $D0, $C0
        vmovdqa $A1, $D0
        vmovdqa $B1, $A1
        vmovdqa $C1, $B1
        vmovdqa $D1, $C1
        vmovdqa $A2, $D1
        vmovdqa $B2, $A2
    jmp .Lseal_avx2_short_loop
.Lseal_avx2_short_tail:
    cmp \$16, $inl
    jb .Lseal_avx2_exit
    sub \$16, $inl
    vpxor ($inp), $A0x, $A3x
    vmovdqu $A3x, ($oup)
    lea 1*16($inp), $inp\n";
    &poly_add("0*8($oup)");
    &poly_mul(); $code.="
    lea 1*16($oup), $oup
    vextracti128 \$1, $A0, $A0x
.Lseal_avx2_exit:
    vzeroupper
    jmp .Lseal_sse_tail_16
.cfi_endproc
.size chacha20_poly1305_seal_avx2, .-chacha20_poly1305_seal_avx2
";
}

$code =~ s/\`([^\`]*)\`/eval $1/gem;

print $code;

close STDOUT or die "error closing STDOUT: $!";
