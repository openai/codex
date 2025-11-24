#!/usr/bin/env perl

# Copyright (c) 2020, CloudFlare Ltd.
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
while (($output=shift) && ($output!~/\w[\w\-]*\.\w+$/)) {}

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}arm-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/arm-xlate.pl" and -f $xlate) or
die "can't locate arm-xlate.pl";

open OUT,"| \"$^X\" $xlate $flavour $output";
*STDOUT=*OUT;

my ($oup,$inp,$inl,$adp,$adl,$keyp,$itr1,$itr2) = ("x0","x1","x2","x3","x4","x5","x6","x7");
my ($acc0,$acc1,$acc2) = map("x$_",(8..10));
my ($t0,$t1,$t2,$t3) = map("x$_",(11..14));
my ($one, $r0, $r1) = ("x15","x16","x17");
my ($t0w) = $t0 =~ s/x/w/r;

my ($A0,$A1,$A2,$A3,$A4,$B0,$B1,$B2,$B3,$B4,$C0,$C1,$C2,$C3,$C4,$D0,$D1,$D2,$D3,$D4) = map("v$_",(0..19));
my ($T0,$T1,$T2,$T3) = map("v$_",(20..23));

my $CONSTS = "v24";
my $INC = "v25";
my $ROL8 = "v26";
my $CLAMP = "v27";

my ($B_STORE, $C_STORE, $D_STORE) = map("v$_",(28..30));

my $S_STORE = $CLAMP;
my $LEN_STORE = "v31";

sub chacha_qr {
my ($a,$b,$c,$d,$t,$dir)=@_;
my ($shift_b,$shift_d) = $dir =~ /left/ ? ("#4","#12") : ("#12","#4");
$code.=<<___;
    add   $a.4s, $a.4s, $b.4s
    eor   $d.16b, $d.16b, $a.16b
    rev32 $d.8h, $d.8h

    add   $c.4s, $c.4s, $d.4s
    eor   $b.16b, $b.16b, $c.16b
    ushr  $t.4s, $b.4s, #20
    sli   $t.4s, $b.4s, #12
___
    ($t,$b) = ($b,$t);
$code.=<<___;
    add   $a.4s, $a.4s, $b.4s
    eor   $d.16b, $d.16b, $a.16b
    tbl   $d.16b, {$d.16b}, $ROL8.16b

    add   $c.4s, $c.4s, $d.4s
    eor   $b.16b, $b.16b, $c.16b
    ushr  $t.4s, $b.4s, #25
    sli   $t.4s, $b.4s, #7
___
    ($t,$b) = ($b,$t);
$code.=<<___;
    ext $b.16b, $b.16b, $b.16b, $shift_b
    ext $c.16b, $c.16b, $c.16b, #8
    ext $d.16b, $d.16b, $d.16b, $shift_d
___
}

sub poly_add {
my ($src)=@_;
$code.="ldp  $t0, $t1, [$src], 16
        adds $acc0, $acc0, $t0
        adcs $acc1, $acc1, $t1
        adc  $acc2, $acc2, $one\n";
}

sub poly_add_vec {
my ($src)=@_;
$code.="mov  $t0, $src.d[0]
        mov  $t1, $src.d[1]
        adds $acc0, $acc0, $t0
        adcs $acc1, $acc1, $t1
        adc  $acc2, $acc2, $one\n";
}

sub poly_stage1 {
$code.="mul   $t0, $acc0, $r0     // [t2:t1:t0] = [acc2:acc1:acc0] * r0
        umulh $t1, $acc0, $r0
        mul   $t2, $acc1, $r0
        umulh $t3, $acc1, $r0
        adds  $t1, $t1, $t2
        mul   $t2, $acc2, $r0
        adc   $t2, $t2, $t3\n";
}

sub poly_stage2 {
$code.="mul   $t3, $acc0, $r1       // [t3:t2:t1:t0] = [acc2:acc1:acc0] * [r1:r0]
        umulh $acc0, $acc0, $r1
        adds  $t1, $t1, $t3
        mul   $t3, $acc1, $r1
        umulh $acc1, $acc1, $r1
        adcs  $t3, $t3, $acc0
        mul   $acc2, $acc2, $r1
        adc   $acc2, $acc2, $acc1
        adds  $t2, $t2, $t3
        adc   $t3, $acc2, xzr\n";
}

# At the beginning of the reduce stage t = [t3:t2:t1:t0] is a product of
# r = [r1:r0] and acc = [acc2:acc1:acc0]
# r is 124 bits at most (due to clamping) and acc is 131 bits at most
# (acc2 is at most 4 before the addition and can be at most 6 when we add in
# the next block) therefore t is at most 255 bits big, and t3 is 63 bits.
sub poly_reduce_stage {
$code.="and  $acc2, $t2, #3         // At this point acc2 is 2 bits at most (value of 3)
        and  $acc0, $t2, #-4
        extr $t2, $t3, $t2, #2
        adds $acc0, $acc0, $t0
        lsr  $t0, $t3, #2
        adc  $acc1, $t3, $t0        // No carry out since t0 is 61 bits and t3 is 63 bits
        adds $acc0, $acc0, $t2
        adcs $acc1, $acc1, $t1
        adc  $acc2, $acc2, xzr      // At this point acc2 has the value of 4 at most \n";
}

sub poly_mul {
    &poly_stage1();
    &poly_stage2();
    &poly_reduce_stage();
}

sub chacha_qr_x3 {
my ($dir)=@_;
my ($shift_b,$shift_d) = $dir =~ /left/ ? ("#4","#12") : ("#12","#4");
$code.=<<___;
    add   $A0.4s, $A0.4s, $B0.4s
    add   $A1.4s, $A1.4s, $B1.4s
    add   $A2.4s, $A2.4s, $B2.4s
    eor   $D0.16b, $D0.16b, $A0.16b
    eor   $D1.16b, $D1.16b, $A1.16b
    eor   $D2.16b, $D2.16b, $A2.16b
    rev32 $D0.8h, $D0.8h
    rev32 $D1.8h, $D1.8h
    rev32 $D2.8h, $D2.8h

    add   $C0.4s, $C0.4s, $D0.4s
    add   $C1.4s, $C1.4s, $D1.4s
    add   $C2.4s, $C2.4s, $D2.4s
    eor   $B0.16b, $B0.16b, $C0.16b
    eor   $B1.16b, $B1.16b, $C1.16b
    eor   $B2.16b, $B2.16b, $C2.16b
    ushr  $T0.4s, $B0.4s, #20
    sli   $T0.4s, $B0.4s, #12
    ushr  $B0.4s, $B1.4s, #20
    sli   $B0.4s, $B1.4s, #12
    ushr  $B1.4s, $B2.4s, #20
    sli   $B1.4s, $B2.4s, #12

    add   $A0.4s, $A0.4s, $T0.4s
    add   $A1.4s, $A1.4s, $B0.4s
    add   $A2.4s, $A2.4s, $B1.4s
    eor   $D0.16b, $D0.16b, $A0.16b
    eor   $D1.16b, $D1.16b, $A1.16b
    eor   $D2.16b, $D2.16b, $A2.16b
    tbl   $D0.16b, {$D0.16b}, $ROL8.16b
    tbl   $D1.16b, {$D1.16b}, $ROL8.16b
    tbl   $D2.16b, {$D2.16b}, $ROL8.16b

    add   $C0.4s, $C0.4s, $D0.4s
    add   $C1.4s, $C1.4s, $D1.4s
    add   $C2.4s, $C2.4s, $D2.4s
    eor   $T0.16b, $T0.16b, $C0.16b
    eor   $B0.16b, $B0.16b, $C1.16b
    eor   $B1.16b, $B1.16b, $C2.16b
    ushr  $B2.4s, $B1.4s, #25
    sli   $B2.4s, $B1.4s, #7
    ushr  $B1.4s, $B0.4s, #25
    sli   $B1.4s, $B0.4s, #7
    ushr  $B0.4s, $T0.4s, #25
    sli   $B0.4s, $T0.4s, #7

    ext $B0.16b, $B0.16b, $B0.16b, $shift_b
    ext $B1.16b, $B1.16b, $B1.16b, $shift_b
    ext $B2.16b, $B2.16b, $B2.16b, $shift_b

    ext $C0.16b, $C0.16b, $C0.16b, #8
    ext $C1.16b, $C1.16b, $C1.16b, #8
    ext $C2.16b, $C2.16b, $C2.16b, #8

    ext $D0.16b, $D0.16b, $D0.16b, $shift_d
    ext $D1.16b, $D1.16b, $D1.16b, $shift_d
    ext $D2.16b, $D2.16b, $D2.16b, $shift_d
___
}

# When preparing 5 ChaCha20 blocks in parallel, we operate on 4 blocks vertically as introduced by Andrew Moon
# the fifth block is done horizontally
sub chacha_qr_x5 {
my ($dir)=@_;
my ($a0,$a1,$a2,$a3) = $dir =~ /left/ ? ($A0,$A1,$A2,$A3) : ($A0,$A1,$A2,$A3);
my ($b0,$b1,$b2,$b3) = $dir =~ /left/ ? ($B0,$B1,$B2,$B3) : ($B1,$B2,$B3,$B0);
my ($c0,$c1,$c2,$c3) = $dir =~ /left/ ? ($C0,$C1,$C2,$C3) : ($C2,$C3,$C0,$C1);
my ($d0,$d1,$d2,$d3) = $dir =~ /left/ ? ($D0,$D1,$D2,$D3) : ($D3,$D0,$D1,$D2);
my ($shift_b,$shift_d) = $dir =~ /left/ ? ("#4","#12") : ("#12","#4");
$code.=<<___;
    add   $a0.4s, $a0.4s, $b0.4s
    add   $a1.4s, $a1.4s, $b1.4s
    add   $a2.4s, $a2.4s, $b2.4s
    add   $a3.4s, $a3.4s, $b3.4s
    add   $A4.4s, $A4.4s, $B4.4s

    eor   $d0.16b, $d0.16b, $a0.16b
    eor   $d1.16b, $d1.16b, $a1.16b
    eor   $d2.16b, $d2.16b, $a2.16b
    eor   $d3.16b, $d3.16b, $a3.16b
    eor   $D4.16b, $D4.16b, $A4.16b

    rev32 $d0.8h, $d0.8h
    rev32 $d1.8h, $d1.8h
    rev32 $d2.8h, $d2.8h
    rev32 $d3.8h, $d3.8h
    rev32 $D4.8h, $D4.8h

    add   $c0.4s, $c0.4s, $d0.4s
    add   $c1.4s, $c1.4s, $d1.4s
    add   $c2.4s, $c2.4s, $d2.4s
    add   $c3.4s, $c3.4s, $d3.4s
    add   $C4.4s, $C4.4s, $D4.4s

    eor   $b0.16b, $b0.16b, $c0.16b
    eor   $b1.16b, $b1.16b, $c1.16b
    eor   $b2.16b, $b2.16b, $c2.16b
    eor   $b3.16b, $b3.16b, $c3.16b
    eor   $B4.16b, $B4.16b, $C4.16b

    ushr  $T0.4s, $b0.4s, #20
    sli   $T0.4s, $b0.4s, #12
    ushr  $b0.4s, $b1.4s, #20
    sli   $b0.4s, $b1.4s, #12
    ushr  $b1.4s, $b2.4s, #20
    sli   $b1.4s, $b2.4s, #12
    ushr  $b2.4s, $b3.4s, #20
    sli   $b2.4s, $b3.4s, #12
    ushr  $b3.4s, $B4.4s, #20
    sli   $b3.4s, $B4.4s, #12

    add   $a0.4s, $a0.4s, $T0.4s
    add   $a1.4s, $a1.4s, $b0.4s
    add   $a2.4s, $a2.4s, $b1.4s
    add   $a3.4s, $a3.4s, $b2.4s
    add   $A4.4s, $A4.4s, $b3.4s

    eor   $d0.16b, $d0.16b, $a0.16b
    eor   $d1.16b, $d1.16b, $a1.16b
    eor   $d2.16b, $d2.16b, $a2.16b
    eor   $d3.16b, $d3.16b, $a3.16b
    eor   $D4.16b, $D4.16b, $A4.16b

    tbl   $d0.16b, {$d0.16b}, $ROL8.16b
    tbl   $d1.16b, {$d1.16b}, $ROL8.16b
    tbl   $d2.16b, {$d2.16b}, $ROL8.16b
    tbl   $d3.16b, {$d3.16b}, $ROL8.16b
    tbl   $D4.16b, {$D4.16b}, $ROL8.16b

    add   $c0.4s, $c0.4s, $d0.4s
    add   $c1.4s, $c1.4s, $d1.4s
    add   $c2.4s, $c2.4s, $d2.4s
    add   $c3.4s, $c3.4s, $d3.4s
    add   $C4.4s, $C4.4s, $D4.4s

    eor   $T0.16b, $T0.16b, $c0.16b
    eor   $b0.16b, $b0.16b, $c1.16b
    eor   $b1.16b, $b1.16b, $c2.16b
    eor   $b2.16b, $b2.16b, $c3.16b
    eor   $b3.16b, $b3.16b, $C4.16b

    ushr  $B4.4s, $b3.4s, #25
    sli   $B4.4s, $b3.4s, #7
    ushr  $b3.4s, $b2.4s, #25
    sli   $b3.4s, $b2.4s, #7
    ushr  $b2.4s, $b1.4s, #25
    sli   $b2.4s, $b1.4s, #7
    ushr  $b1.4s, $b0.4s, #25
    sli   $b1.4s, $b0.4s, #7
    ushr  $b0.4s, $T0.4s, #25
    sli   $b0.4s, $T0.4s, #7

    ext $B4.16b, $B4.16b, $B4.16b, $shift_b
    ext $C4.16b, $C4.16b, $C4.16b, #8
    ext $D4.16b, $D4.16b, $D4.16b, $shift_d
___
}

{
$code.=<<___;
.section .rodata

.align 7
.Lchacha20_consts:
.byte 'e','x','p','a','n','d',' ','3','2','-','b','y','t','e',' ','k'
.Linc:
.long 1,2,3,4
.Lrol8:
.byte 3,0,1,2, 7,4,5,6, 11,8,9,10, 15,12,13,14
.Lclamp:
.quad 0x0FFFFFFC0FFFFFFF, 0x0FFFFFFC0FFFFFFC

.text

.type   .Lpoly_hash_ad_internal,%function
.align  6
.Lpoly_hash_ad_internal:
    .cfi_startproc
    cbnz $adl, .Lpoly_hash_intro
    ret

.Lpoly_hash_intro:
    cmp $adl, #16
    b.lt .Lpoly_hash_ad_tail
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
        sub $adl, $adl, #16
        b .Lpoly_hash_ad_internal

.Lpoly_hash_ad_tail:
    cbz $adl, .Lpoly_hash_ad_ret

    eor $T0.16b, $T0.16b, $T0.16b // Use T0 to load the AAD
    sub $adl, $adl, #1

.Lpoly_hash_tail_16_compose:
        ext  $T0.16b, $T0.16b, $T0.16b, #15
        ldrb $t0w, [$adp, $adl]
        mov  $T0.b[0], $t0w
        subs $adl, $adl, #1
        b.ge .Lpoly_hash_tail_16_compose
___
    &poly_add_vec($T0);
    &poly_mul();
$code.=<<___;

.Lpoly_hash_ad_ret:
    ret
    .cfi_endproc
.size .Lpoly_hash_ad_internal, .-.Lpoly_hash_ad_internal

/////////////////////////////////
//
// void chacha20_poly1305_seal(uint8_t *pt, uint8_t *ct, size_t len_in, uint8_t *ad, size_t len_ad, union open_data *seal_data);
//
.globl  chacha20_poly1305_seal
.type   chacha20_poly1305_seal,%function
.align  6
chacha20_poly1305_seal:
    AARCH64_SIGN_LINK_REGISTER
.cfi_startproc
    stp x29, x30, [sp, #-80]!
.cfi_def_cfa_offset 80
.cfi_offset w30, -72
.cfi_offset w29, -80
    mov x29, sp
    // We probably could do .cfi_def_cfa w29, 80 at this point, but since
    // we don't actually use the frame pointer like that, it's probably not
    // worth bothering.
    stp d8, d9, [sp, #16]
    stp d10, d11, [sp, #32]
    stp d12, d13, [sp, #48]
    stp d14, d15, [sp, #64]
.cfi_offset b15, -8
.cfi_offset b14, -16
.cfi_offset b13, -24
.cfi_offset b12, -32
.cfi_offset b11, -40
.cfi_offset b10, -48
.cfi_offset b9, -56
.cfi_offset b8, -64

    adrp $t0, :pg_hi21:.Lchacha20_consts
    add  $t0, $t0, :lo12:.Lchacha20_consts

    ld1 {$CONSTS.16b - $CLAMP.16b}, [$t0] // Load the CONSTS, INC, ROL8 and CLAMP values
    ld1 {$B_STORE.16b - $D_STORE.16b}, [$keyp]

    mov $one, #1 // Prepare the Poly1305 state
    mov $acc0, #0
    mov $acc1, #0
    mov $acc2, #0

    ldr $t1, [$keyp, #56]   // The total cipher text length includes extra_in_len
    add $t1, $t1, $inl
    mov $LEN_STORE.d[0], $adl  // Store the input and aad lengths
    mov $LEN_STORE.d[1], $t1

    cmp $inl, #128
    b.le .Lseal_128 // Optimization for smaller buffers

    // Initially we prepare 5 ChaCha20 blocks. Four to encrypt up to 4 blocks (256 bytes) of plaintext,
    // and one for the Poly1305 R and S keys. The first four blocks (A0-A3..D0-D3) are computed vertically,
    // the fifth block (A4-D4) horizontally.
    ld4r {$A0.4s-$A3.4s}, [$t0]
    mov $A4.16b, $CONSTS.16b

    ld4r {$B0.4s-$B3.4s}, [$keyp], #16
    mov $B4.16b, $B_STORE.16b

    ld4r {$C0.4s-$C3.4s}, [$keyp], #16
    mov $C4.16b, $C_STORE.16b

    ld4r {$D0.4s-$D3.4s}, [$keyp]
    add $D0.4s, $D0.4s, $INC.4s
    mov $D4.16b, $D_STORE.16b

    sub $keyp, $keyp, #32

    mov  $itr1, #10

.align 5
.Lseal_init_rounds:
___
        &chacha_qr_x5("left");
        &chacha_qr_x5("right");
$code.=<<___;
        subs $itr1, $itr1, #1
    b.hi .Lseal_init_rounds

    add $D0.4s, $D0.4s, $INC.4s
    mov $t0, #4
    dup $T0.4s, $t0w
    add $INC.4s, $INC.4s, $T0.4s

    zip1 $T0.4s, $A0.4s, $A1.4s
    zip2 $T1.4s, $A0.4s, $A1.4s
    zip1 $T2.4s, $A2.4s, $A3.4s
    zip2 $T3.4s, $A2.4s, $A3.4s

    zip1 $A0.2d, $T0.2d, $T2.2d
    zip2 $A1.2d, $T0.2d, $T2.2d
    zip1 $A2.2d, $T1.2d, $T3.2d
    zip2 $A3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $B0.4s, $B1.4s
    zip2 $T1.4s, $B0.4s, $B1.4s
    zip1 $T2.4s, $B2.4s, $B3.4s
    zip2 $T3.4s, $B2.4s, $B3.4s

    zip1 $B0.2d, $T0.2d, $T2.2d
    zip2 $B1.2d, $T0.2d, $T2.2d
    zip1 $B2.2d, $T1.2d, $T3.2d
    zip2 $B3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $C0.4s, $C1.4s
    zip2 $T1.4s, $C0.4s, $C1.4s
    zip1 $T2.4s, $C2.4s, $C3.4s
    zip2 $T3.4s, $C2.4s, $C3.4s

    zip1 $C0.2d, $T0.2d, $T2.2d
    zip2 $C1.2d, $T0.2d, $T2.2d
    zip1 $C2.2d, $T1.2d, $T3.2d
    zip2 $C3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $D0.4s, $D1.4s
    zip2 $T1.4s, $D0.4s, $D1.4s
    zip1 $T2.4s, $D2.4s, $D3.4s
    zip2 $T3.4s, $D2.4s, $D3.4s

    zip1 $D0.2d, $T0.2d, $T2.2d
    zip2 $D1.2d, $T0.2d, $T2.2d
    zip1 $D2.2d, $T1.2d, $T3.2d
    zip2 $D3.2d, $T1.2d, $T3.2d

    add $A4.4s, $A4.4s, $CONSTS.4s
    add $B4.4s, $B4.4s, $B_STORE.4s
    and $A4.16b, $A4.16b, $CLAMP.16b

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s

    add $A1.4s, $A1.4s, $CONSTS.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s
    add $D1.4s, $D1.4s, $D_STORE.4s

    add $A2.4s, $A2.4s, $CONSTS.4s
    add $B2.4s, $B2.4s, $B_STORE.4s
    add $C2.4s, $C2.4s, $C_STORE.4s
    add $D2.4s, $D2.4s, $D_STORE.4s

    add $A3.4s, $A3.4s, $CONSTS.4s
    add $B3.4s, $B3.4s, $B_STORE.4s
    add $C3.4s, $C3.4s, $C_STORE.4s
    add $D3.4s, $D3.4s, $D_STORE.4s

    mov $r0, $A4.d[0] // Move the R key to GPRs
    mov $r1, $A4.d[1]
    mov $S_STORE.16b, $B4.16b // Store the S key

    bl  .Lpoly_hash_ad_internal

    mov $adp, $oup
    cmp $inl, #256
    b.le .Lseal_tail

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A0.16b
    eor $T1.16b, $T1.16b, $B0.16b
    eor $T2.16b, $T2.16b, $C0.16b
    eor $T3.16b, $T3.16b, $D0.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A1.16b
    eor $T1.16b, $T1.16b, $B1.16b
    eor $T2.16b, $T2.16b, $C1.16b
    eor $T3.16b, $T3.16b, $D1.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A2.16b
    eor $T1.16b, $T1.16b, $B2.16b
    eor $T2.16b, $T2.16b, $C2.16b
    eor $T3.16b, $T3.16b, $D2.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A3.16b
    eor $T1.16b, $T1.16b, $B3.16b
    eor $T2.16b, $T2.16b, $C3.16b
    eor $T3.16b, $T3.16b, $D3.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #256

    mov $itr1, #4 // In the first run of the loop we need to hash 256 bytes, therefore we hash one block for the first 4 rounds
    mov $itr2, #6 // and two blocks for the remaining 6, for a total of (1 * 4 + 2 * 6) * 16 = 256

.Lseal_main_loop:
    adrp $t0, :pg_hi21:.Lchacha20_consts
    add  $t0, $t0, :lo12:.Lchacha20_consts

    ld4r {$A0.4s-$A3.4s}, [$t0]
    mov $A4.16b, $CONSTS.16b

    ld4r {$B0.4s-$B3.4s}, [$keyp], #16
    mov $B4.16b, $B_STORE.16b

    ld4r {$C0.4s-$C3.4s}, [$keyp], #16
    mov $C4.16b, $C_STORE.16b

    ld4r {$D0.4s-$D3.4s}, [$keyp]
    add $D0.4s, $D0.4s, $INC.4s
    mov $D4.16b, $D_STORE.16b

    eor $T0.16b, $T0.16b, $T0.16b //zero
    not $T1.16b, $T0.16b // -1
    sub $T1.4s, $INC.4s, $T1.4s // Add +1
    ext $T0.16b, $T1.16b, $T0.16b, #12 // Get the last element (counter)
    add $D4.4s, $D4.4s, $T0.4s

    sub $keyp, $keyp, #32
.align 5
.Lseal_main_loop_rounds:
___
        &chacha_qr_x5("left");
        &poly_add($adp);
        &poly_mul();
        &chacha_qr_x5("right");
$code.=<<___;
        subs $itr1, $itr1, #1
        b.ge .Lseal_main_loop_rounds
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
        subs $itr2, $itr2, #1
        b.gt .Lseal_main_loop_rounds

    eor $T0.16b, $T0.16b, $T0.16b //zero
    not $T1.16b, $T0.16b // -1
    sub $T1.4s, $INC.4s, $T1.4s // Add +1
    ext $T0.16b, $T1.16b, $T0.16b, #12 // Get the last element (counter)
    add $D4.4s, $D4.4s, $T0.4s

    add $D0.4s, $D0.4s, $INC.4s
    mov $t0, #5
    dup $T0.4s, $t0w
    add $INC.4s, $INC.4s, $T0.4s

    zip1 $T0.4s, $A0.4s, $A1.4s
    zip2 $T1.4s, $A0.4s, $A1.4s
    zip1 $T2.4s, $A2.4s, $A3.4s
    zip2 $T3.4s, $A2.4s, $A3.4s

    zip1 $A0.2d, $T0.2d, $T2.2d
    zip2 $A1.2d, $T0.2d, $T2.2d
    zip1 $A2.2d, $T1.2d, $T3.2d
    zip2 $A3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $B0.4s, $B1.4s
    zip2 $T1.4s, $B0.4s, $B1.4s
    zip1 $T2.4s, $B2.4s, $B3.4s
    zip2 $T3.4s, $B2.4s, $B3.4s

    zip1 $B0.2d, $T0.2d, $T2.2d
    zip2 $B1.2d, $T0.2d, $T2.2d
    zip1 $B2.2d, $T1.2d, $T3.2d
    zip2 $B3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $C0.4s, $C1.4s
    zip2 $T1.4s, $C0.4s, $C1.4s
    zip1 $T2.4s, $C2.4s, $C3.4s
    zip2 $T3.4s, $C2.4s, $C3.4s

    zip1 $C0.2d, $T0.2d, $T2.2d
    zip2 $C1.2d, $T0.2d, $T2.2d
    zip1 $C2.2d, $T1.2d, $T3.2d
    zip2 $C3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $D0.4s, $D1.4s
    zip2 $T1.4s, $D0.4s, $D1.4s
    zip1 $T2.4s, $D2.4s, $D3.4s
    zip2 $T3.4s, $D2.4s, $D3.4s

    zip1 $D0.2d, $T0.2d, $T2.2d
    zip2 $D1.2d, $T0.2d, $T2.2d
    zip1 $D2.2d, $T1.2d, $T3.2d
    zip2 $D3.2d, $T1.2d, $T3.2d

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s

    add $A1.4s, $A1.4s, $CONSTS.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s
    add $D1.4s, $D1.4s, $D_STORE.4s

    add $A2.4s, $A2.4s, $CONSTS.4s
    add $B2.4s, $B2.4s, $B_STORE.4s
    add $C2.4s, $C2.4s, $C_STORE.4s
    add $D2.4s, $D2.4s, $D_STORE.4s

    add $A3.4s, $A3.4s, $CONSTS.4s
    add $B3.4s, $B3.4s, $B_STORE.4s
    add $C3.4s, $C3.4s, $C_STORE.4s
    add $D3.4s, $D3.4s, $D_STORE.4s

    add $A4.4s, $A4.4s, $CONSTS.4s
    add $B4.4s, $B4.4s, $B_STORE.4s
    add $C4.4s, $C4.4s, $C_STORE.4s
    add $D4.4s, $D4.4s, $D_STORE.4s

    cmp $inl, #320
    b.le .Lseal_tail

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A0.16b
    eor $T1.16b, $T1.16b, $B0.16b
    eor $T2.16b, $T2.16b, $C0.16b
    eor $T3.16b, $T3.16b, $D0.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A1.16b
    eor $T1.16b, $T1.16b, $B1.16b
    eor $T2.16b, $T2.16b, $C1.16b
    eor $T3.16b, $T3.16b, $D1.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A2.16b
    eor $T1.16b, $T1.16b, $B2.16b
    eor $T2.16b, $T2.16b, $C2.16b
    eor $T3.16b, $T3.16b, $D2.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A3.16b
    eor $T1.16b, $T1.16b, $B3.16b
    eor $T2.16b, $T2.16b, $C3.16b
    eor $T3.16b, $T3.16b, $D3.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A4.16b
    eor $T1.16b, $T1.16b, $B4.16b
    eor $T2.16b, $T2.16b, $C4.16b
    eor $T3.16b, $T3.16b, $D4.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #320

    mov $itr1, #0
    mov $itr2, #10 // For the remainder of the loop we always hash and encrypt 320 bytes per iteration

    b .Lseal_main_loop

.Lseal_tail:
    // This part of the function handles the storage and authentication of the last [0,320) bytes
    // We assume A0-A4 ... D0-D4 hold at least inl (320 max) bytes of the stream data.
    cmp $inl, #64
    b.lt .Lseal_tail_64

    // Store and authenticate 64B blocks per iteration
    ld1 {$T0.16b - $T3.16b}, [$inp], #64

    eor $T0.16b, $T0.16b, $A0.16b
    eor $T1.16b, $T1.16b, $B0.16b
    eor $T2.16b, $T2.16b, $C0.16b
    eor $T3.16b, $T3.16b, $D0.16b
___
    &poly_add_vec($T0);
    &poly_mul();
    &poly_add_vec($T1);
    &poly_mul();
    &poly_add_vec($T2);
    &poly_mul();
    &poly_add_vec($T3);
    &poly_mul();
$code.=<<___;
    st1 {$T0.16b - $T3.16b}, [$oup], #64
    sub $inl, $inl, #64

    // Shift the state left by 64 bytes for the next iteration of the loop
    mov $A0.16b, $A1.16b
    mov $B0.16b, $B1.16b
    mov $C0.16b, $C1.16b
    mov $D0.16b, $D1.16b

    mov $A1.16b, $A2.16b
    mov $B1.16b, $B2.16b
    mov $C1.16b, $C2.16b
    mov $D1.16b, $D2.16b

    mov $A2.16b, $A3.16b
    mov $B2.16b, $B3.16b
    mov $C2.16b, $C3.16b
    mov $D2.16b, $D3.16b

    mov $A3.16b, $A4.16b
    mov $B3.16b, $B4.16b
    mov $C3.16b, $C4.16b
    mov $D3.16b, $D4.16b

    b .Lseal_tail

.Lseal_tail_64:
    ldp $adp, $adl, [$keyp, #48] // extra_in_len and extra_in_ptr

    // Here we handle the last [0,64) bytes of plaintext
    cmp $inl, #16
    b.lt .Lseal_tail_16
    // Each iteration encrypt and authenticate a 16B block
    ld1 {$T0.16b}, [$inp], #16
    eor $T0.16b, $T0.16b, $A0.16b
___
    &poly_add_vec($T0);
    &poly_mul();
$code.=<<___;
    st1 {$T0.16b}, [$oup], #16

    sub $inl, $inl, #16

    // Shift the state left by 16 bytes for the next iteration of the loop
    mov $A0.16b, $B0.16b
    mov $B0.16b, $C0.16b
    mov $C0.16b, $D0.16b

    b .Lseal_tail_64

.Lseal_tail_16:
    // Here we handle the last [0,16) bytes of ciphertext that require a padded block
    cbz $inl, .Lseal_hash_extra

    eor $T0.16b, $T0.16b, $T0.16b // Use T0 to load the plaintext/extra in
    eor $T1.16b, $T1.16b, $T1.16b // Use T1 to generate an AND mask that will only mask the ciphertext bytes
    not $T2.16b, $T0.16b

    mov $itr1, $inl
    add $inp, $inp, $inl

    cbz $adl, .Lseal_tail_16_compose // No extra data to pad with, zero padding

    mov $itr2, #16          // We need to load some extra_in first for padding
    sub $itr2, $itr2, $inl
    cmp $adl, $itr2
    csel $itr2, $adl, $itr2, lt // Load the minimum of extra_in_len and the amount needed to fill the register
    mov $t1, $itr2
    add $adp, $adp, $itr2
    sub $adl, $adl, $itr2

.Lseal_tail16_compose_extra_in:
        ext  $T0.16b, $T0.16b, $T0.16b, #15
        ldrb $t0w, [$adp, #-1]!
        mov  $T0.b[0], $t0w
        subs $itr2, $itr2, #1
        b.gt .Lseal_tail16_compose_extra_in

    add $adp, $adp, $t1

.Lseal_tail_16_compose:
        ext  $T0.16b, $T0.16b, $T0.16b, #15
        ldrb $t0w, [$inp, #-1]!
        mov  $T0.b[0], $t0w
        ext  $T1.16b, $T2.16b, $T1.16b, #15
        subs $inl, $inl, #1
        b.gt .Lseal_tail_16_compose

    and $A0.16b, $A0.16b, $T1.16b
    eor $T0.16b, $T0.16b, $A0.16b
    mov $T1.16b, $T0.16b

.Lseal_tail_16_store:
        umov $t0w, $T0.b[0]
        strb $t0w, [$oup], #1
        ext  $T0.16b, $T0.16b, $T0.16b, #1
        subs $itr1, $itr1, #1
        b.gt .Lseal_tail_16_store

    // Hash in the final ct block concatenated with extra_in
___
    &poly_add_vec($T1);
    &poly_mul();
$code.=<<___;

.Lseal_hash_extra:
    cbz $adl, .Lseal_finalize

.Lseal_hash_extra_loop:
    cmp $adl, #16
    b.lt .Lseal_hash_extra_tail
    ld1 {$T0.16b}, [$adp], #16
___
    &poly_add_vec($T0);
    &poly_mul();
$code.=<<___;
    sub $adl, $adl, #16
    b .Lseal_hash_extra_loop

.Lseal_hash_extra_tail:
    cbz $adl, .Lseal_finalize
    eor $T0.16b, $T0.16b, $T0.16b // Use T0 to load the remaining extra ciphertext
    add $adp, $adp, $adl

.Lseal_hash_extra_load:
        ext  $T0.16b, $T0.16b, $T0.16b, #15
        ldrb $t0w, [$adp, #-1]!
        mov  $T0.b[0], $t0w
        subs $adl, $adl, #1
        b.gt .Lseal_hash_extra_load

    // Hash in the final padded extra_in blcok
___
    &poly_add_vec($T0);
    &poly_mul();
$code.=<<___;

.Lseal_finalize:
___
    &poly_add_vec($LEN_STORE);
    &poly_mul();
$code.=<<___;
    // Final reduction step
    sub  $t1, xzr, $one
    orr  $t2, xzr, #3
    subs $t0, $acc0, #-5
    sbcs $t1, $acc1, $t1
    sbcs $t2, $acc2, $t2
    csel $acc0, $t0, $acc0, cs
    csel $acc1, $t1, $acc1, cs
    csel $acc2, $t2, $acc2, cs
___
    &poly_add_vec($S_STORE);
$code.=<<___;

    stp  $acc0, $acc1, [$keyp]

    ldp d8, d9, [sp, #16]
    ldp d10, d11, [sp, #32]
    ldp d12, d13, [sp, #48]
    ldp d14, d15, [sp, #64]
.cfi_restore b15
.cfi_restore b14
.cfi_restore b13
.cfi_restore b12
.cfi_restore b11
.cfi_restore b10
.cfi_restore b9
.cfi_restore b8
    ldp x29, x30, [sp], 80
.cfi_restore w29
.cfi_restore w30
.cfi_def_cfa_offset 0
    AARCH64_VALIDATE_LINK_REGISTER
    ret

.Lseal_128:
    // On some architectures preparing 5 blocks for small buffers is wasteful
    eor $INC.16b, $INC.16b, $INC.16b
    mov $t0, #1
    mov $INC.s[0], $t0w
    mov $A0.16b, $CONSTS.16b
    mov $A1.16b, $CONSTS.16b
    mov $A2.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $B1.16b, $B_STORE.16b
    mov $B2.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $C1.16b, $C_STORE.16b
    mov $C2.16b, $C_STORE.16b
    mov $D2.16b, $D_STORE.16b
    add $D0.4s, $D2.4s, $INC.4s
    add $D1.4s, $D0.4s, $INC.4s

    mov  $itr1, #10

.Lseal_128_rounds:
___
        &chacha_qr_x3("left");
        &chacha_qr_x3("right");
$code.=<<___;
        subs $itr1, $itr1, #1
    b.hi .Lseal_128_rounds

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $A1.4s, $A1.4s, $CONSTS.4s
    add $A2.4s, $A2.4s, $CONSTS.4s

    add $B0.4s, $B0.4s, $B_STORE.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $B2.4s, $B2.4s, $B_STORE.4s

    // Only the first 32 bytes of the third block (counter = 0) are needed,
    // so skip updating $C2 and $D2.
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s

    add $D_STORE.4s, $D_STORE.4s, $INC.4s
    add $D0.4s, $D0.4s, $D_STORE.4s
    add $D_STORE.4s, $D_STORE.4s, $INC.4s
    add $D1.4s, $D1.4s, $D_STORE.4s

    and $A2.16b, $A2.16b, $CLAMP.16b
    mov $r0, $A2.d[0] // Move the R key to GPRs
    mov $r1, $A2.d[1]
    mov $S_STORE.16b, $B2.16b // Store the S key

    bl  .Lpoly_hash_ad_internal
    b   .Lseal_tail
.cfi_endproc
.size chacha20_poly1305_seal,.-chacha20_poly1305_seal

/////////////////////////////////
//
// void chacha20_poly1305_open(uint8_t *pt, uint8_t *ct, size_t len_in, uint8_t *ad, size_t len_ad, union open_data *aead_data);
//
.globl	chacha20_poly1305_open
.type	chacha20_poly1305_open,%function
.align	6
chacha20_poly1305_open:
    AARCH64_SIGN_LINK_REGISTER
.cfi_startproc
    stp x29, x30, [sp, #-80]!
.cfi_def_cfa_offset 80
.cfi_offset w30, -72
.cfi_offset w29, -80
    mov x29, sp
    // We probably could do .cfi_def_cfa w29, 80 at this point, but since
    // we don't actually use the frame pointer like that, it's probably not
    // worth bothering.
    stp	d8, d9, [sp, #16]
    stp	d10, d11, [sp, #32]
    stp	d12, d13, [sp, #48]
    stp	d14, d15, [sp, #64]
.cfi_offset b15, -8
.cfi_offset b14, -16
.cfi_offset b13, -24
.cfi_offset b12, -32
.cfi_offset b11, -40
.cfi_offset b10, -48
.cfi_offset b9, -56
.cfi_offset b8, -64

    adrp $t0, :pg_hi21:.Lchacha20_consts
    add  $t0, $t0, :lo12:.Lchacha20_consts

    ld1 {$CONSTS.16b - $CLAMP.16b}, [$t0] // Load the CONSTS, INC, ROL8 and CLAMP values
    ld1 {$B_STORE.16b - $D_STORE.16b}, [$keyp]

    mov $one, #1 // Prepare the Poly1305 state
    mov $acc0, #0
    mov $acc1, #0
    mov $acc2, #0

    mov $LEN_STORE.d[0], $adl  // Store the input and aad lengths
    mov $LEN_STORE.d[1], $inl

    cmp $inl, #128
    b.le .Lopen_128 // Optimization for smaller buffers

    // Initially we prepare a single ChaCha20 block for the Poly1305 R and S keys
    mov $A0.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $D0.16b, $D_STORE.16b

    mov  $itr1, #10

.align 5
.Lopen_init_rounds:
___
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "left");
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "right");
$code.=<<___;
        subs $itr1, $itr1, #1
    b.hi .Lopen_init_rounds

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s

    and $A0.16b, $A0.16b, $CLAMP.16b
    mov $r0, $A0.d[0] // Move the R key to GPRs
    mov $r1, $A0.d[1]
    mov $S_STORE.16b, $B0.16b // Store the S key

    bl  .Lpoly_hash_ad_internal

.Lopen_ad_done:
    mov $adp, $inp

// Each iteration of the loop hash 320 bytes, and prepare stream for 320 bytes
.Lopen_main_loop:

    cmp $inl, #192
    b.lt .Lopen_tail

    adrp $t0, :pg_hi21:.Lchacha20_consts
    add  $t0, $t0, :lo12:.Lchacha20_consts

    ld4r {$A0.4s-$A3.4s}, [$t0]
    mov $A4.16b, $CONSTS.16b

    ld4r {$B0.4s-$B3.4s}, [$keyp], #16
    mov $B4.16b, $B_STORE.16b

    ld4r {$C0.4s-$C3.4s}, [$keyp], #16
    mov $C4.16b, $C_STORE.16b

    ld4r {$D0.4s-$D3.4s}, [$keyp]
    sub $keyp, $keyp, #32
    add $D0.4s, $D0.4s, $INC.4s
    mov $D4.16b, $D_STORE.16b

    eor $T0.16b, $T0.16b, $T0.16b //zero
    not $T1.16b, $T0.16b // -1
    sub $T1.4s, $INC.4s, $T1.4s // Add +1
    ext $T0.16b, $T1.16b, $T0.16b, #12 // Get the last element (counter)
    add $D4.4s, $D4.4s, $T0.4s

    lsr $adl, $inl, #4 // How many whole blocks we have to hash, will always be at least 12
    sub $adl, $adl, #10

    mov $itr2, #10
    subs $itr1, $itr2, $adl
    subs $itr1, $itr2, $adl // itr1 can be negative if we have more than 320 bytes to hash
    csel $itr2, $itr2, $adl, le // if itr1 is zero or less, itr2 should be 10 to indicate all 10 rounds are full

    cbz $itr2, .Lopen_main_loop_rounds_short

.align 5
.Lopen_main_loop_rounds:
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
.Lopen_main_loop_rounds_short:
___
        &chacha_qr_x5("left");
        &poly_add($adp);
        &poly_mul();
        &chacha_qr_x5("right");
$code.=<<___;
        subs $itr2, $itr2, #1
        b.gt .Lopen_main_loop_rounds
        subs $itr1, $itr1, #1
        b.ge .Lopen_main_loop_rounds_short
___
$code.=<<___;

    eor $T0.16b, $T0.16b, $T0.16b //zero
    not $T1.16b, $T0.16b // -1
    sub $T1.4s, $INC.4s, $T1.4s // Add +1
    ext $T0.16b, $T1.16b, $T0.16b, #12 // Get the last element (counter)
    add $D4.4s, $D4.4s, $T0.4s

    add $D0.4s, $D0.4s, $INC.4s
    mov $t0, #5
    dup $T0.4s, $t0w
    add $INC.4s, $INC.4s, $T0.4s

    zip1 $T0.4s, $A0.4s, $A1.4s
    zip2 $T1.4s, $A0.4s, $A1.4s
    zip1 $T2.4s, $A2.4s, $A3.4s
    zip2 $T3.4s, $A2.4s, $A3.4s

    zip1 $A0.2d, $T0.2d, $T2.2d
    zip2 $A1.2d, $T0.2d, $T2.2d
    zip1 $A2.2d, $T1.2d, $T3.2d
    zip2 $A3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $B0.4s, $B1.4s
    zip2 $T1.4s, $B0.4s, $B1.4s
    zip1 $T2.4s, $B2.4s, $B3.4s
    zip2 $T3.4s, $B2.4s, $B3.4s

    zip1 $B0.2d, $T0.2d, $T2.2d
    zip2 $B1.2d, $T0.2d, $T2.2d
    zip1 $B2.2d, $T1.2d, $T3.2d
    zip2 $B3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $C0.4s, $C1.4s
    zip2 $T1.4s, $C0.4s, $C1.4s
    zip1 $T2.4s, $C2.4s, $C3.4s
    zip2 $T3.4s, $C2.4s, $C3.4s

    zip1 $C0.2d, $T0.2d, $T2.2d
    zip2 $C1.2d, $T0.2d, $T2.2d
    zip1 $C2.2d, $T1.2d, $T3.2d
    zip2 $C3.2d, $T1.2d, $T3.2d

    zip1 $T0.4s, $D0.4s, $D1.4s
    zip2 $T1.4s, $D0.4s, $D1.4s
    zip1 $T2.4s, $D2.4s, $D3.4s
    zip2 $T3.4s, $D2.4s, $D3.4s

    zip1 $D0.2d, $T0.2d, $T2.2d
    zip2 $D1.2d, $T0.2d, $T2.2d
    zip1 $D2.2d, $T1.2d, $T3.2d
    zip2 $D3.2d, $T1.2d, $T3.2d

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s

    add $A1.4s, $A1.4s, $CONSTS.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s
    add $D1.4s, $D1.4s, $D_STORE.4s

    add $A2.4s, $A2.4s, $CONSTS.4s
    add $B2.4s, $B2.4s, $B_STORE.4s
    add $C2.4s, $C2.4s, $C_STORE.4s
    add $D2.4s, $D2.4s, $D_STORE.4s

    add $A3.4s, $A3.4s, $CONSTS.4s
    add $B3.4s, $B3.4s, $B_STORE.4s
    add $C3.4s, $C3.4s, $C_STORE.4s
    add $D3.4s, $D3.4s, $D_STORE.4s

    add $A4.4s, $A4.4s, $CONSTS.4s
    add $B4.4s, $B4.4s, $B_STORE.4s
    add $C4.4s, $C4.4s, $C_STORE.4s
    add $D4.4s, $D4.4s, $D_STORE.4s

    // We can always safely store 192 bytes
    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A0.16b
    eor $T1.16b, $T1.16b, $B0.16b
    eor $T2.16b, $T2.16b, $C0.16b
    eor $T3.16b, $T3.16b, $D0.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A1.16b
    eor $T1.16b, $T1.16b, $B1.16b
    eor $T2.16b, $T2.16b, $C1.16b
    eor $T3.16b, $T3.16b, $D1.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A2.16b
    eor $T1.16b, $T1.16b, $B2.16b
    eor $T2.16b, $T2.16b, $C2.16b
    eor $T3.16b, $T3.16b, $D2.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #192

    mov $A0.16b, $A3.16b
    mov $B0.16b, $B3.16b
    mov $C0.16b, $C3.16b
    mov $D0.16b, $D3.16b

    cmp $inl, #64
    b.lt .Lopen_tail_64_store

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A3.16b
    eor $T1.16b, $T1.16b, $B3.16b
    eor $T2.16b, $T2.16b, $C3.16b
    eor $T3.16b, $T3.16b, $D3.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #64

    mov $A0.16b, $A4.16b
    mov $B0.16b, $B4.16b
    mov $C0.16b, $C4.16b
    mov $D0.16b, $D4.16b

    cmp $inl, #64
    b.lt .Lopen_tail_64_store

    ld1 {$T0.16b - $T3.16b}, [$inp], #64
    eor $T0.16b, $T0.16b, $A4.16b
    eor $T1.16b, $T1.16b, $B4.16b
    eor $T2.16b, $T2.16b, $C4.16b
    eor $T3.16b, $T3.16b, $D4.16b
    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #64
    b .Lopen_main_loop

.Lopen_tail:

    cbz $inl, .Lopen_finalize

    lsr $adl, $inl, #4 // How many whole blocks we have to hash

    cmp $inl, #64
    b.le .Lopen_tail_64
    cmp $inl, #128
    b.le .Lopen_tail_128

.Lopen_tail_192:
     // We need three more blocks
    mov $A0.16b, $CONSTS.16b
    mov $A1.16b, $CONSTS.16b
    mov $A2.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $B1.16b, $B_STORE.16b
    mov $B2.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $C1.16b, $C_STORE.16b
    mov $C2.16b, $C_STORE.16b
    mov $D0.16b, $D_STORE.16b
    mov $D1.16b, $D_STORE.16b
    mov $D2.16b, $D_STORE.16b
    eor $T3.16b, $T3.16b, $T3.16b
    eor $T1.16b, $T1.16b, $T1.16b
    ins $T3.s[0], $INC.s[0]
    ins $T1.d[0], $one

    add $T2.4s, $T3.4s, $T1.4s
    add $T1.4s, $T2.4s, $T1.4s

    add $D0.4s, $D0.4s, $T1.4s
    add $D1.4s, $D1.4s, $T3.4s
    add $D2.4s, $D2.4s, $T2.4s

    mov $itr2, #10
    subs $itr1, $itr2, $adl // itr1 can be negative if we have more than 160 bytes to hash
    csel $itr2, $itr2, $adl, le // if itr1 is zero or less, itr2 should be 10 to indicate all 10 rounds are hashing
    sub $adl, $adl, $itr2

    cbz $itr2, .Lopen_tail_192_rounds_no_hash

.Lopen_tail_192_rounds:
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
.Lopen_tail_192_rounds_no_hash:
___
        &chacha_qr_x3("left");
        &chacha_qr_x3("right");
$code.=<<___;
        subs $itr2, $itr2, #1
        b.gt .Lopen_tail_192_rounds
        subs $itr1, $itr1, #1
        b.ge .Lopen_tail_192_rounds_no_hash

    // We hashed 160 bytes at most, may still have 32 bytes left
.Lopen_tail_192_hash:
    cbz $adl, .Lopen_tail_192_hash_done
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
    sub $adl, $adl, #1
    b .Lopen_tail_192_hash

.Lopen_tail_192_hash_done:

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $A1.4s, $A1.4s, $CONSTS.4s
    add $A2.4s, $A2.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $B2.4s, $B2.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s
    add $C2.4s, $C2.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s
    add $D1.4s, $D1.4s, $D_STORE.4s
    add $D2.4s, $D2.4s, $D_STORE.4s

    add $D0.4s, $D0.4s, $T1.4s
    add $D1.4s, $D1.4s, $T3.4s
    add $D2.4s, $D2.4s, $T2.4s

    ld1 {$T0.16b - $T3.16b}, [$inp], #64

    eor $T0.16b, $T0.16b, $A1.16b
    eor $T1.16b, $T1.16b, $B1.16b
    eor $T2.16b, $T2.16b, $C1.16b
    eor $T3.16b, $T3.16b, $D1.16b

    st1 {$T0.16b - $T3.16b}, [$oup], #64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64

    eor $T0.16b, $T0.16b, $A2.16b
    eor $T1.16b, $T1.16b, $B2.16b
    eor $T2.16b, $T2.16b, $C2.16b
    eor $T3.16b, $T3.16b, $D2.16b

    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #128
    b .Lopen_tail_64_store

.Lopen_tail_128:
     // We need two more blocks
    mov $A0.16b, $CONSTS.16b
    mov $A1.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $B1.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $C1.16b, $C_STORE.16b
    mov $D0.16b, $D_STORE.16b
    mov $D1.16b, $D_STORE.16b
    eor $T3.16b, $T3.16b, $T3.16b
    eor $T2.16b, $T2.16b, $T2.16b
    ins $T3.s[0], $INC.s[0]
    ins $T2.d[0], $one
    add $T2.4s, $T2.4s, $T3.4s

    add $D0.4s, $D0.4s, $T2.4s
    add $D1.4s, $D1.4s, $T3.4s

    mov $itr1, #10
    sub $itr1, $itr1, $adl

.Lopen_tail_128_rounds:
___
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "left");
        &chacha_qr($A1, $B1, $C1, $D1, $T0, "left");
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "right");
        &chacha_qr($A1, $B1, $C1, $D1, $T0, "right");
$code.=<<___;
        subs $itr1, $itr1, #1
        b.gt .Lopen_tail_128_rounds
        cbz $adl, .Lopen_tail_128_rounds_done
        subs $adl, $adl, #1
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
    b .Lopen_tail_128_rounds

.Lopen_tail_128_rounds_done:
    add $A0.4s, $A0.4s, $CONSTS.4s
    add $A1.4s, $A1.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s
    add $D1.4s, $D1.4s, $D_STORE.4s
    add $D0.4s, $D0.4s, $T2.4s
    add $D1.4s, $D1.4s, $T3.4s

    ld1 {$T0.16b - $T3.16b}, [$inp], #64

    eor $T0.16b, $T0.16b, $A1.16b
    eor $T1.16b, $T1.16b, $B1.16b
    eor $T2.16b, $T2.16b, $C1.16b
    eor $T3.16b, $T3.16b, $D1.16b

    st1 {$T0.16b - $T3.16b}, [$oup], #64
    sub $inl, $inl, #64

    b .Lopen_tail_64_store

.Lopen_tail_64:
    // We just need a single block
    mov $A0.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $D0.16b, $D_STORE.16b
    eor $T3.16b, $T3.16b, $T3.16b
    ins $T3.s[0], $INC.s[0]
    add $D0.4s, $D0.4s, $T3.4s

    mov $itr1, #10
    sub $itr1, $itr1, $adl

.Lopen_tail_64_rounds:
___
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "left");
        &chacha_qr($A0, $B0, $C0, $D0, $T0, "right");
$code.=<<___;
        subs $itr1, $itr1, #1
        b.gt .Lopen_tail_64_rounds
        cbz $adl, .Lopen_tail_64_rounds_done
        subs $adl, $adl, #1
___
        &poly_add($adp);
        &poly_mul();
$code.=<<___;
    b .Lopen_tail_64_rounds

.Lopen_tail_64_rounds_done:
    add $A0.4s, $A0.4s, $CONSTS.4s
    add $B0.4s, $B0.4s, $B_STORE.4s
    add $C0.4s, $C0.4s, $C_STORE.4s
    add $D0.4s, $D0.4s, $D_STORE.4s
    add $D0.4s, $D0.4s, $T3.4s

.Lopen_tail_64_store:
    cmp $inl, #16
    b.lt .Lopen_tail_16

    ld1 {$T0.16b}, [$inp], #16
    eor $T0.16b, $T0.16b, $A0.16b
    st1 {$T0.16b}, [$oup], #16
    mov $A0.16b, $B0.16b
    mov $B0.16b, $C0.16b
    mov $C0.16b, $D0.16b
    sub $inl, $inl, #16
    b .Lopen_tail_64_store

.Lopen_tail_16:
    // Here we handle the last [0,16) bytes that require a padded block
    cbz $inl, .Lopen_finalize

    eor $T0.16b, $T0.16b, $T0.16b // Use T0 to load the ciphertext
    eor $T1.16b, $T1.16b, $T1.16b // Use T1 to generate an AND mask
    not $T2.16b, $T0.16b

    add $itr2, $inp, $inl
    mov $itr1, $inl

.Lopen_tail_16_compose:
    ext  $T0.16b, $T0.16b, $T0.16b, #15
    ldrb $t0w, [$itr2, #-1]!
    mov  $T0.b[0], $t0w
    ext  $T1.16b, $T2.16b, $T1.16b, #15
    subs $inl, $inl, #1
    b.gt .Lopen_tail_16_compose

    and $T0.16b, $T0.16b, $T1.16b
    // Hash in the final padded block
___
    &poly_add_vec($T0);
    &poly_mul();
$code.=<<___;
    eor $T0.16b, $T0.16b, $A0.16b

.Lopen_tail_16_store:
    umov $t0w, $T0.b[0]
    strb $t0w, [$oup], #1
    ext  $T0.16b, $T0.16b, $T0.16b, #1
    subs $itr1, $itr1, #1
    b.gt .Lopen_tail_16_store

.Lopen_finalize:
___
    &poly_add_vec($LEN_STORE);
    &poly_mul();
$code.=<<___;
    // Final reduction step
    sub  $t1, xzr, $one
    orr  $t2, xzr, #3
    subs $t0, $acc0, #-5
    sbcs $t1, $acc1, $t1
    sbcs $t2, $acc2, $t2
    csel $acc0, $t0, $acc0, cs
    csel $acc1, $t1, $acc1, cs
    csel $acc2, $t2, $acc2, cs
___
    &poly_add_vec($S_STORE);
$code.=<<___;

    stp  $acc0, $acc1, [$keyp]

    ldp	d8, d9, [sp, #16]
    ldp	d10, d11, [sp, #32]
    ldp	d12, d13, [sp, #48]
    ldp	d14, d15, [sp, #64]
.cfi_restore b15
.cfi_restore b14
.cfi_restore b13
.cfi_restore b12
.cfi_restore b11
.cfi_restore b10
.cfi_restore b9
.cfi_restore b8
    ldp x29, x30, [sp], 80
.cfi_restore w29
.cfi_restore w30
.cfi_def_cfa_offset 0
    AARCH64_VALIDATE_LINK_REGISTER
    ret

.Lopen_128:
    // On some architectures preparing 5 blocks for small buffers is wasteful
    eor $INC.16b, $INC.16b, $INC.16b
    mov $t0, #1
    mov $INC.s[0], $t0w
    mov $A0.16b, $CONSTS.16b
    mov $A1.16b, $CONSTS.16b
    mov $A2.16b, $CONSTS.16b
    mov $B0.16b, $B_STORE.16b
    mov $B1.16b, $B_STORE.16b
    mov $B2.16b, $B_STORE.16b
    mov $C0.16b, $C_STORE.16b
    mov $C1.16b, $C_STORE.16b
    mov $C2.16b, $C_STORE.16b
    mov $D2.16b, $D_STORE.16b
    add $D0.4s, $D2.4s, $INC.4s
    add $D1.4s, $D0.4s, $INC.4s

    mov  $itr1, #10

.Lopen_128_rounds:
___
        &chacha_qr_x3("left");
        &chacha_qr_x3("right");
$code.=<<___;
        subs $itr1, $itr1, #1
    b.hi .Lopen_128_rounds

    add $A0.4s, $A0.4s, $CONSTS.4s
    add $A1.4s, $A1.4s, $CONSTS.4s
    add $A2.4s, $A2.4s, $CONSTS.4s

    add $B0.4s, $B0.4s, $B_STORE.4s
    add $B1.4s, $B1.4s, $B_STORE.4s
    add $B2.4s, $B2.4s, $B_STORE.4s

    add $C0.4s, $C0.4s, $C_STORE.4s
    add $C1.4s, $C1.4s, $C_STORE.4s

    add $D_STORE.4s, $D_STORE.4s, $INC.4s
    add $D0.4s, $D0.4s, $D_STORE.4s
    add $D_STORE.4s, $D_STORE.4s, $INC.4s
    add $D1.4s, $D1.4s, $D_STORE.4s

    and $A2.16b, $A2.16b, $CLAMP.16b
    mov $r0, $A2.d[0] // Move the R key to GPRs
    mov $r1, $A2.d[1]
    mov $S_STORE.16b, $B2.16b // Store the S key

    bl  .Lpoly_hash_ad_internal

.Lopen_128_store:
    cmp $inl, #64
    b.lt .Lopen_128_store_64

    ld1 {$T0.16b - $T3.16b}, [$inp], #64

___
    &poly_add_vec($T0);
    &poly_mul();
    &poly_add_vec($T1);
    &poly_mul();
    &poly_add_vec($T2);
    &poly_mul();
    &poly_add_vec($T3);
    &poly_mul();
$code.=<<___;

    eor $T0.16b, $T0.16b, $A0.16b
    eor $T1.16b, $T1.16b, $B0.16b
    eor $T2.16b, $T2.16b, $C0.16b
    eor $T3.16b, $T3.16b, $D0.16b

    st1 {$T0.16b - $T3.16b}, [$oup], #64

    sub $inl, $inl, #64

    mov $A0.16b, $A1.16b
    mov $B0.16b, $B1.16b
    mov $C0.16b, $C1.16b
    mov $D0.16b, $D1.16b

.Lopen_128_store_64:

    lsr $adl, $inl, #4
    mov $adp, $inp

.Lopen_128_hash_64:
    cbz $adl, .Lopen_tail_64_store
___
    &poly_add($adp);
    &poly_mul();
$code.=<<___;
    sub $adl, $adl, #1
    b .Lopen_128_hash_64
.cfi_endproc
.size chacha20_poly1305_open,.-chacha20_poly1305_open
___
}

foreach (split("\n",$code)) {
	s/\`([^\`]*)\`/eval $1/ge;

	print $_,"\n";
}
close STDOUT or die "error closing STDOUT";
