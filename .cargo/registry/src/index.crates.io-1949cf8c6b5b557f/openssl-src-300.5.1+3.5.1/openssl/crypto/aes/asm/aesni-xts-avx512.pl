#! /usr/bin/env perl
# Copyright (C) 2023 Intel Corporation
#
# Licensed under the OpenSSL license (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html

# This implementation is based on the AES-XTS code (AVX512VAES + VPCLMULQDQ)
# from Intel(R) Intelligent Storage Acceleration Library Crypto Version
# (https://github.com/intel/isa-l_crypto).
#
######################################################################
# The main building block of the loop is code that encrypts/decrypts
# 8/16 blocks of data stitching with generation of tweak for the next
# 8/16 blocks, utilizing VAES and VPCLMULQDQ instructions with full width
# of ZMM registers. The main loop is selected based on the input length.
# main_loop_run_16 encrypts/decrypts 16 blocks in parallel and it's selected
# when input length >= 256 bytes (16 blocks)
# main_loop_run_8 encrypts/decrypts 8 blocks in parallel and it's selected
# when 128 bytes <= input length < 256 bytes (8-15 blocks)
# Input length < 128 bytes (8 blocks) is handled by do_n_blocks.
#
# This implementation mainly uses vpshrdq from AVX-512-VBMI2 family and vaesenc,
# vaesdec, vpclmulqdq from AVX-512F family.
$output = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.| ? shift : undef;

$win64=0; $win64=1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);
$avx512vaes=0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/; $dir=$1;
( $xlate="${dir}x86_64-xlate.pl" and -f $xlate ) or
( $xlate="${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate) or
die "can't locate x86_64-xlate.pl";

if (`$ENV{CC} -Wa,-v -c -o /dev/null -x assembler /dev/null 2>&1`
        =~ /GNU assembler version ([2-9]\.[0-9]+)/) {
    $avx512vaes = ($1>=2.30);
}

if (!$avx512vaes && $win64 && ($flavour =~ /nasm/ || $ENV{ASM} =~ /nasm/) &&
       `nasm -v 2>&1` =~ /NASM version ([2-9]\.[0-9]+)(?:\.([0-9]+))?/) {
    $avx512vaes = ($1==2.11 && $2>=8) + ($1>=2.12);
}

if (!$avx512vaes && `$ENV{CC} -v 2>&1`
    =~ /(Apple)?\s*((?:clang|LLVM) version|.*based on LLVM) ([0-9]+)\.([0-9]+)\.([0-9]+)?/) {
    my $ver = $3 + $4/100.0 + $5/10000.0; # 3.1.0->3.01, 3.10.1->3.1001
    if ($1) {
        # Apple conditions, they use a different version series, see
        # https://en.wikipedia.org/wiki/Xcode#Xcode_7.0_-_10.x_(since_Free_On-Device_Development)_2
        # clang 7.0.0 is Apple clang 10.0.1
        $avx512vaes = ($ver>=10.0001)
    } else {
        $avx512vaes = ($ver>=7.0);
    }
}

open OUT,"| \"$^X\" \"$xlate\" $flavour \"$output\""
    or die "can't call $xlate: $!";
*STDOUT=*OUT;

#======================================================================

if ($avx512vaes) {

  my $GP_STORAGE  = $win64 ? (16 * 18)  : (16 * 8);    # store rbx
  my $XMM_STORAGE = $win64 ? (16 * 8) : 0;     # store xmm6:xmm15
  my $VARIABLE_OFFSET = $win64 ? (16*8 + 16*10 + 8*3) :
                                 (16*8 + 8*1);

  # right now, >= 0x80 (128) is used for expanded keys. all usages of
  # rsp should be invoked via $TW, not shadowed by any other name or
  # used directly.
  my $TW = "%rsp";
  my $TEMPHIGH = "%rbx";
  my $TEMPLOW = "%rax";
  my $ZPOLY = "%zmm25";

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;;; Function arguments abstraction
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  my ($key2, $key1, $tweak, $length, $input, $output);

 
$input    = "%rdi";
$output   = "%rsi";
$length   = "%rdx";
$key1     = "%rcx";
$key2     = "%r8";
$tweak    = "%r9";

  # arguments for temp parameters
  my ($tmp1, $gf_poly_8b, $gf_poly_8b_temp);
    $tmp1                = "%r8";
    $gf_poly_8b       = "%r10";
    $gf_poly_8b_temp  = "%r11";

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;;; Helper functions
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

  # Generates "random" local labels
  sub random_string() {
    my @chars  = ('a' .. 'z', 'A' .. 'Z', '0' .. '9', '_');
    my $length = 15;
    my $str;
    map { $str .= $chars[rand(33)] } 1 .. $length;
    return $str;
  }

  # ; Seed the RNG so the labels are generated deterministically
  srand(12345);

  sub encrypt_tweak {
    my $state_tweak = $_[0];
    my $is_128 = $_[1];

    $code.=<<___;
    vpxor	($key2), $state_tweak, $state_tweak
    vaesenc	0x10($key2), $state_tweak, $state_tweak
    vaesenc	0x20($key2), $state_tweak, $state_tweak
    vaesenc	0x30($key2), $state_tweak, $state_tweak
    vaesenc	0x40($key2), $state_tweak, $state_tweak
    vaesenc	0x50($key2), $state_tweak, $state_tweak
    vaesenc	0x60($key2), $state_tweak, $state_tweak
    vaesenc	0x70($key2), $state_tweak, $state_tweak
    vaesenc	0x80($key2), $state_tweak, $state_tweak
    vaesenc	0x90($key2), $state_tweak, $state_tweak
___

    if ($is_128) {
      $code .= "vaesenclast	0xa0($key2), $state_tweak, $state_tweak\n";
    } else {
      $code .= "vaesenc	0xa0($key2), $state_tweak, $state_tweak\n";
      $code .= "vaesenc	0xb0($key2), $state_tweak, $state_tweak\n";
      $code .= "vaesenc	0xc0($key2), $state_tweak, $state_tweak\n";
      $code .= "vaesenc	0xd0($key2), $state_tweak, $state_tweak\n";
      $code .= "vaesenclast	0xe0($key2), $state_tweak, $state_tweak\n";
    }
    $code .= "vmovdqa	$state_tweak, ($TW)\n";
  }

  sub encrypt_final {
    my $st = $_[0];
    my $tw = $_[1];
    my $is_128 = $_[2];

    # xor Tweak value
	$code .= "vpxor	$tw, $st, $st\n";
    $code .= "vpxor	($key1), $st, $st\n";

    my $rounds = $is_128 ? 10 : 14;
    for (my $i = 1; $i < $rounds; $i++) {
      $code .= "vaesenc	16*$i($key1), $st, $st\n";
    }

    $code .=<<___;
    vaesenclast 16*$rounds($key1), $st, $st
    vpxor	$tw, $st, $st
___
  }

  # decrypt initial blocks of AES
  # 1, 2, 3, 4, 5, 6 or 7 blocks are encrypted
  # next 8 Tweak values are generated
  sub decrypt_initial {
    my @st;
    $st[0] = $_[0];
    $st[1] = $_[1];
    $st[2] = $_[2];
    $st[3] = $_[3];
    $st[4] = $_[4];
    $st[5] = $_[5];
    $st[6] = $_[6];
    $st[7] = $_[7];

    my @tw;
    $tw[0] = $_[8];
    $tw[1] = $_[9];
    $tw[2] = $_[10];
    $tw[3] = $_[11];
    $tw[4] = $_[12];
    $tw[5] = $_[13];
    $tw[6] = $_[14];
    my $t0 = $_[15];
    my $num_blocks = $_[16];
    my $lt128 = $_[17];
    my $is_128 = $_[18];

    # num_blocks blocks encrypted
    # num_blocks can be 1, 2, 3, 4, 5, 6, 7

    #  xor Tweak value
    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vpxor $tw[$i], $st[$i], $st[$i]\n";
    }

    $code .= "vmovdqu  ($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vpxor $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
___
    }
    # round 1
    $code .= "vmovdqu 0x10($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
    $code .= <<___;
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, ($TW)     # next Tweak1 generated
      mov     $TEMPLOW, 0x08($TW)
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
___
    }

    # round 2
    $code .= "vmovdqu 0x20($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x10($TW) # next Tweak2 generated
___
    }

    # round 3
    $code .= "vmovdqu 0x30($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      mov     $TEMPHIGH, 0x18($TW)
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
___
    }

    # round 4
    $code .= "vmovdqu 0x40($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
    $code .= <<___;
    xor     $gf_poly_8b_temp, $TEMPLOW
    mov     $TEMPLOW, 0x20($TW) # next Tweak3 generated
    mov     $TEMPHIGH, 0x28($TW)
    xor     $gf_poly_8b_temp, $gf_poly_8b_temp
    shl     \$1, $TEMPLOW
___
    }

    # round 5
    $code .= "vmovdqu 0x50($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
    $code .= <<___;
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x30($TW) # next Tweak4 generated
      mov     $TEMPHIGH, 0x38($TW)
___
    }

    # round 6
    $code .= "vmovdqu 0x60($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x40($TW) # next Tweak5 generated
      mov     $TEMPHIGH, 0x48($TW)
___
    }

    # round 7
    $code .= "vmovdqu 0x70($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x50($TW) # next Tweak6 generated
      mov     $TEMPHIGH, 0x58($TW)
___
    }

    # round 8
    $code .= "vmovdqu 0x80($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x60($TW) # next Tweak7 generated
      mov     $TEMPHIGH, 0x68($TW)
___
    }

    # round 9
    $code .= "vmovdqu 0x90($key1), $t0\n";

    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      $code .= <<___;
      xor     $gf_poly_8b_temp, $gf_poly_8b_temp
      shl     \$1, $TEMPLOW
      adc     $TEMPHIGH, $TEMPHIGH
      cmovc   $gf_poly_8b, $gf_poly_8b_temp
      xor     $gf_poly_8b_temp, $TEMPLOW
      mov     $TEMPLOW, 0x70($TW) # next Tweak8 generated
      mov     $TEMPHIGH, 0x78($TW)
___
    }

    if ($is_128) {
      # round 10
      $code .= "vmovdqu 0xa0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdeclast $t0, $st[$i], $st[$i]\n";
      }
    } else {
      # round 10
      $code .= "vmovdqu 0xa0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 11
      $code .= "vmovdqu 0xb0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 12
      $code .= "vmovdqu 0xc0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 13
      $code .= "vmovdqu 0xd0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 14
      $code .= "vmovdqu 0xe0($key1), $t0\n";
      for (my $i = 0; $i < $num_blocks; $i++) {
        $code .= "vaesdeclast $t0, $st[$i], $st[$i]\n";
      }
    }

    # xor Tweak values
    for (my $i = 0; $i < $num_blocks; $i++) {
      $code .= "vpxor $tw[$i], $st[$i], $st[$i]\n";
    }

    if (0 == $lt128) {
      # load next Tweak values
      $code .= <<___;
      vmovdqa  ($TW), $tw1
      vmovdqa  0x10($TW), $tw2
      vmovdqa  0x20($TW), $tw3
      vmovdqa  0x30($TW), $tw4
      vmovdqa  0x40($TW), $tw5
      vmovdqa  0x50($TW), $tw6
      vmovdqa  0x60($TW), $tw7
___
    }
  }

  sub initialize {
    my @st;
    $st[0] = $_[0];
    $st[1] = $_[1];
    $st[2] = $_[2];
    $st[3] = $_[3];
    $st[4] = $_[4];
    $st[5] = $_[5];
    $st[6] = $_[6];
    $st[7] = $_[7];

    my @tw;
    $tw[0] = $_[8];
    $tw[1] = $_[9];
    $tw[2] = $_[10];
    $tw[3] = $_[11];
    $tw[4] = $_[12];
    $tw[5] = $_[13];
    $tw[6] = $_[14];
    my $num_initial_blocks = $_[15];

    $code .= <<___;
    vmovdqa  0x0($TW), $tw[0]
    mov      0x0($TW), $TEMPLOW
    mov      0x08($TW), $TEMPHIGH
    vmovdqu  0x0($input), $st[0]
___

    if ($num_initial_blocks >= 2) {
      for (my $i = 1; $i < $num_initial_blocks; $i++) {
        $code .= "xor      $gf_poly_8b_temp, $gf_poly_8b_temp\n";
        $code .= "shl      \$1, $TEMPLOW\n";
        $code .= "adc      $TEMPHIGH, $TEMPHIGH\n";
        $code .= "cmovc    $gf_poly_8b, $gf_poly_8b_temp\n";
        $code .= "xor      $gf_poly_8b_temp, $TEMPLOW\n";
        my $offset = $i * 16;
        $code .= "mov      $TEMPLOW, $offset($TW)\n";
        $code .= "mov      $TEMPHIGH, $offset + 8($TW)\n";
        $code .= "vmovdqa  $offset($TW), $tw[$i]\n";
        $code .= "vmovdqu  $offset($input), $st[$i]\n";
      }
    }
  }

  # Encrypt 4 blocks in parallel
  sub encrypt_by_four {
    my $st1 = $_[0]; # state 1
    my $tw1 = $_[1]; # tweak 1
    my $tmp = $_[2];
    my $is_128 = $_[3];

    $code .= "vbroadcasti32x4 ($key1), $tmp\n";
    $code .= "vpternlogq      \$0x96, $tmp, $tw1, $st1\n";

    my $rounds = $is_128 ? 10 : 14;
    for (my $i = 1; $i < $rounds; $i++) {
      $code .= "vbroadcasti32x4 16*$i($key1), $tmp\n";
      $code .= "vaesenc  $tmp, $st1, $st1\n";
    }

    $code .= "vbroadcasti32x4 16*$rounds($key1), $tmp\n";
    $code .= "vaesenclast  $tmp, $st1, $st1\n";

    $code .= "vpxorq $tw1, $st1, $st1\n";
  }

  # Encrypt 8 blocks in parallel
  # generate next 8 tweak values
  sub encrypt_by_eight_zmm {
    my $st1 = $_[0];
    my $st2 = $_[1];
    my $tw1 = $_[2];
    my $tw2 = $_[3];
    my $t0 = $_[4];
    my $last_eight = $_[5];
    my $is_128 = $_[6];

    $code .= <<___;
	vbroadcasti32x4 ($key1), $t0
	vpternlogq    \$0x96, $t0, $tw1, $st1
	vpternlogq    \$0x96, $t0, $tw2, $st2
___

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw1, %zmm13
      vpclmulqdq	\$0x0, $ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw1, %zmm15
      vpxord		%zmm14, %zmm15, %zmm15
___
    }
    # round 1
    $code .= <<___;
    vbroadcasti32x4 0x10($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 2
    vbroadcasti32x4 0x20($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 3
    vbroadcasti32x4 0x30($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2
___

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw2, %zmm13
      vpclmulqdq	\$0x0, $ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw2, %zmm16
      vpxord		%zmm14, %zmm16, %zmm16
___
    }

    $code .= <<___;
    # round 4
    vbroadcasti32x4 0x40($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 5
    vbroadcasti32x4 0x50($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 6
    vbroadcasti32x4 0x60($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 7
    vbroadcasti32x4 0x70($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 8
    vbroadcasti32x4 0x80($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2

    # round 9
    vbroadcasti32x4 0x90($key1), $t0
    vaesenc  $t0, $st1, $st1
    vaesenc  $t0, $st2, $st2
___

    if ($is_128) {
      $code .= <<___;
      # round 10
      vbroadcasti32x4 0xa0($key1), $t0
      vaesenclast  $t0, $st1, $st1
      vaesenclast  $t0, $st2, $st2
___
    } else {
      $code .= <<___;
      # round 10
      vbroadcasti32x4 0xa0($key1), $t0
      vaesenc  $t0, $st1, $st1
      vaesenc  $t0, $st2, $st2

      # round 11
      vbroadcasti32x4 0xb0($key1), $t0
      vaesenc  $t0, $st1, $st1
      vaesenc  $t0, $st2, $st2

      # round 12
      vbroadcasti32x4 0xc0($key1), $t0
      vaesenc  $t0, $st1, $st1
      vaesenc  $t0, $st2, $st2

      # round 13
      vbroadcasti32x4 0xd0($key1), $t0
      vaesenc  $t0, $st1, $st1
      vaesenc  $t0, $st2, $st2

      # round 14
      vbroadcasti32x4 0xe0($key1), $t0
      vaesenclast  $t0, $st1, $st1
      vaesenclast  $t0, $st2, $st2
___
    }
	
    # xor Tweak values
    $code .= "vpxorq    $tw1, $st1, $st1\n";
    $code .= "vpxorq    $tw2, $st2, $st2\n";

    if (0 == $last_eight) {
      # load next Tweak values
      $code .= <<___;
      vmovdqa32  %zmm15, $tw1
      vmovdqa32  %zmm16, $tw2
___
    }
  }

  # Decrypt 8 blocks in parallel
  # generate next 8 tweak values
  sub decrypt_by_eight_zmm {
    my $st1 = $_[0];
    my $st2 = $_[1];
    my $tw1 = $_[2];
    my $tw2 = $_[3];
    my $t0 = $_[4];
    my $last_eight = $_[5];
    my $is_128 = $_[6];

    $code .= <<___;
    # xor Tweak values
    vpxorq    $tw1, $st1, $st1
    vpxorq    $tw2, $st2, $st2

    # ARK
    vbroadcasti32x4 ($key1), $t0
    vpxorq    $t0, $st1, $st1
    vpxorq    $t0, $st2, $st2
___

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw1, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw1, %zmm15
      vpxord		%zmm14, %zmm15, %zmm15
___
    }
    # round 1
    $code .= <<___;
    vbroadcasti32x4 0x10($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 2
    vbroadcasti32x4 0x20($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 3
    vbroadcasti32x4 0x30($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2
___

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw2, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw2, %zmm16
      vpxord		%zmm14, %zmm16, %zmm16
___
    }

    $code .= <<___;
    # round 4
    vbroadcasti32x4 0x40($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 5
    vbroadcasti32x4 0x50($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 6
    vbroadcasti32x4 0x60($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 7
    vbroadcasti32x4 0x70($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 8
    vbroadcasti32x4 0x80($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

    # round 9
    vbroadcasti32x4 0x90($key1), $t0
    vaesdec  $t0, $st1, $st1
    vaesdec  $t0, $st2, $st2

___
    if ($is_128) {
      $code .= <<___;
      # round 10
      vbroadcasti32x4 0xa0($key1), $t0
      vaesdeclast  $t0, $st1, $st1
      vaesdeclast  $t0, $st2, $st2
___
    } else {
      $code .= <<___;
      # round 10
      vbroadcasti32x4 0xa0($key1), $t0
      vaesdec  $t0, $st1, $st1
      vaesdec  $t0, $st2, $st2

      # round 11
      vbroadcasti32x4 0xb0($key1), $t0
      vaesdec  $t0, $st1, $st1
      vaesdec  $t0, $st2, $st2

      # round 12
      vbroadcasti32x4 0xc0($key1), $t0
      vaesdec  $t0, $st1, $st1
      vaesdec  $t0, $st2, $st2

      # round 13
      vbroadcasti32x4 0xd0($key1), $t0
      vaesdec  $t0, $st1, $st1
      vaesdec  $t0, $st2, $st2

      # round 14
      vbroadcasti32x4 0xe0($key1), $t0
      vaesdeclast  $t0, $st1, $st1
      vaesdeclast  $t0, $st2, $st2
___
    }

    $code .= <<___;
    # xor Tweak values
    vpxorq    $tw1, $st1, $st1
    vpxorq    $tw2, $st2, $st2

    # load next Tweak values
    vmovdqa32  %zmm15, $tw1
    vmovdqa32  %zmm16, $tw2
___
  }

  # Encrypt 16 blocks in parallel
  # generate next 16 tweak values
  sub encrypt_by_16_zmm {
    my @st;
    $st[0] = $_[0];
    $st[1] = $_[1];
    $st[2] = $_[2];
    $st[3] = $_[3];

    my @tw;
    $tw[0] = $_[4];
    $tw[1] = $_[5];
    $tw[2] = $_[6];
    $tw[3] = $_[7];

    my $t0 = $_[8];
    my $last_eight = $_[9];
    my $is_128 = $_[10];

    # xor Tweak values
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq    $tw[$i], $st[$i], $st[$i]\n";
    }

    # ARK
    $code .= "vbroadcasti32x4 ($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw[2], %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw[2], %zmm15
      vpxord		%zmm14, %zmm15, %zmm15
___
    }

    # round 1
    $code .= "vbroadcasti32x4 0x10($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 2
    $code .= "vbroadcasti32x4 0x20($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 3
    $code .= "vbroadcasti32x4 0x30($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw[3], %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw[3], %zmm16
      vpxord		%zmm14, %zmm16, %zmm16
___
    }
    # round 4
    $code .= "vbroadcasti32x4 0x40($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 5
    $code .= "vbroadcasti32x4 0x50($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 6
    $code .= "vbroadcasti32x4 0x60($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, %zmm15, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, %zmm15, %zmm17
      vpxord		%zmm14, %zmm17, %zmm17
___
    }
    # round 7
    $code .= "vbroadcasti32x4 0x70($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 8
    $code .= "vbroadcasti32x4 0x80($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    # round 9
    $code .= "vbroadcasti32x4 0x90($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, %zmm16, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, %zmm16, %zmm18
      vpxord		%zmm14, %zmm18, %zmm18
___
    }
    if ($is_128) {
      # round 10
      $code .= "vbroadcasti32x4 0xa0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenclast $t0, $st[$i], $st[$i]\n";
      }
    } else {
      # round 10
      $code .= "vbroadcasti32x4 0xa0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
      }
      # round 11
      $code .= "vbroadcasti32x4 0xb0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
      }
      # round 12
      $code .= "vbroadcasti32x4 0xc0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
      }
      # round 13
      $code .= "vbroadcasti32x4 0xd0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenc $t0, $st[$i], $st[$i]\n";
      }
      # round 14
      $code .= "vbroadcasti32x4 0xe0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesenclast $t0, $st[$i], $st[$i]\n";
      }
    }

    # xor Tweak values
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq    $tw[$i], $st[$i], $st[$i]\n";
    }

    $code .= <<___;
    # load next Tweak values
    vmovdqa32  %zmm15, $tw[0]
    vmovdqa32  %zmm16, $tw[1]
    vmovdqa32  %zmm17, $tw[2]
    vmovdqa32  %zmm18, $tw[3]
___
  }

  # Decrypt 16 blocks in parallel
  # generate next 8 tweak values
  sub decrypt_by_16_zmm {
    my @st;
    $st[0] = $_[0];
    $st[1] = $_[1];
    $st[2] = $_[2];
    $st[3] = $_[3];

    my @tw;
    $tw[0] = $_[4];
    $tw[1] = $_[5];
    $tw[2] = $_[6];
    $tw[3] = $_[7];

    my $t0 = $_[8];
    my $last_eight = $_[9];
    my $is_128 = $_[10];

    # xor Tweak values
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq    $tw[$i], $st[$i], $st[$i]\n";
    }

    # ARK
    $code .= "vbroadcasti32x4 ($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw[2], %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw[2], %zmm15
      vpxord		%zmm14, %zmm15, %zmm15
___
    }

    # round 1
    $code .= "vbroadcasti32x4 0x10($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 2
    $code .= "vbroadcasti32x4 0x20($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 3
    $code .= "vbroadcasti32x4 0x30($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, $tw[3], %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, $tw[3], %zmm16
      vpxord		%zmm14, %zmm16, %zmm16
___
    }
    # round 4
    $code .= "vbroadcasti32x4 0x40($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 5
    $code .= "vbroadcasti32x4 0x50($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 6
    $code .= "vbroadcasti32x4 0x60($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, %zmm15, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, %zmm15, %zmm17
      vpxord		%zmm14, %zmm17, %zmm17
___
    }
    # round 7
    $code .= "vbroadcasti32x4 0x70($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 8
    $code .= "vbroadcasti32x4 0x80($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    # round 9
    $code .= "vbroadcasti32x4 0x90($key1), $t0\n";
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
    }

    if (0 == $last_eight) {
      $code .= <<___;
      vpsrldq		\$0xf, %zmm16, %zmm13
      vpclmulqdq	\$0x0,$ZPOLY, %zmm13, %zmm14
      vpslldq		\$0x1, %zmm16, %zmm18
      vpxord		%zmm14, %zmm18, %zmm18
___
    }
    if ($is_128) {
      # round 10
      $code .= "vbroadcasti32x4 0xa0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdeclast $t0, $st[$i], $st[$i]\n";
      }
    } else {
      # round 10
      $code .= "vbroadcasti32x4 0xa0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 11
      $code .= "vbroadcasti32x4 0xb0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 12
      $code .= "vbroadcasti32x4 0xc0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 13
      $code .= "vbroadcasti32x4 0xd0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdec $t0, $st[$i], $st[$i]\n";
      }

      # round 14
      $code .= "vbroadcasti32x4 0xe0($key1), $t0\n";
      for (my $i = 0; $i < 4; $i++) {
        $code .= "vaesdeclast $t0, $st[$i], $st[$i]\n";
      }
    }

    # xor Tweak values
    for (my $i = 0; $i < 4; $i++) {
      $code .= "vpxorq    $tw[$i], $st[$i], $st[$i]\n";
    }

    $code .= <<___;
    # load next Tweak values
    vmovdqa32  %zmm15, $tw[0]
    vmovdqa32  %zmm16, $tw[1]
    vmovdqa32  %zmm17, $tw[2]
    vmovdqa32  %zmm18, $tw[3]
___
  }

  $code .= ".text\n";

  {
    $code.=<<"___";
    .extern	OPENSSL_ia32cap_P
    .globl	aesni_xts_avx512_eligible
    .type	aesni_xts_avx512_eligible,\@abi-omnipotent
    .align	32
    aesni_xts_avx512_eligible:
        mov	OPENSSL_ia32cap_P+8(%rip), %ecx
        xor	%eax,%eax
    	# 1<<31|1<<30|1<<17|1<<16 avx512vl + avx512bw + avx512dq + avx512f
        and	\$0xc0030000, %ecx
        cmp	\$0xc0030000, %ecx
        jne	.L_done
        mov	OPENSSL_ia32cap_P+12(%rip), %ecx
    	# 1<<10|1<<9|1<<6 vaes + vpclmulqdq + vbmi2
        and	\$0x640, %ecx
        cmp	\$0x640, %ecx
        cmove	%ecx,%eax
        .L_done:
        ret
    .size   aesni_xts_avx512_eligible, .-aesni_xts_avx512_eligible
___
  }


  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;void aesni_xts_[128|256]_encrypt_avx512(
  # ;               const uint8_t *in,        // input data
  # ;               uint8_t *out,             // output data
  # ;               size_t length,            // sector size, in bytes
  # ;               const AES_KEY *key1,      // key used for "ECB" encryption
  # ;               const AES_KEY *key2,      // key used for tweaking
  # ;               const uint8_t iv[16])     // initial tweak value, 16 bytes
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  sub enc {
    my $is_128 = $_[0];
    my $rndsuffix = &random_string();

    if ($is_128) {
      $code.=<<___;
      .globl	aesni_xts_128_encrypt_avx512
      .hidden	aesni_xts_128_encrypt_avx512
      .type	aesni_xts_128_encrypt_avx512,\@function,6
      .align	32
      aesni_xts_128_encrypt_avx512:
      .cfi_startproc
      endbranch
___
    } else {
      $code.=<<___;
      .globl	aesni_xts_256_encrypt_avx512
      .hidden	aesni_xts_256_encrypt_avx512
      .type	aesni_xts_256_encrypt_avx512,\@function,6
      .align	32
      aesni_xts_256_encrypt_avx512:
      .cfi_startproc
      endbranch
___
    }
    $code .= "push 	 %rbp\n";
    $code .= "mov 	 $TW,%rbp\n";
    $code .= "sub 	 \$$VARIABLE_OFFSET,$TW\n";
    $code .= "and 	 \$0xffffffffffffffc0,$TW\n";
    $code .= "mov 	 %rbx,$GP_STORAGE($TW)\n";

    if ($win64) {
      $code .= "mov 	 %rdi,$GP_STORAGE + 8*1($TW)\n";
      $code .= "mov 	 %rsi,$GP_STORAGE + 8*2($TW)\n";
      $code .= "vmovdqa      %xmm6, $XMM_STORAGE + 16*0($TW)\n";
      $code .= "vmovdqa      %xmm7, $XMM_STORAGE + 16*1($TW)\n";
      $code .= "vmovdqa      %xmm8, $XMM_STORAGE + 16*2($TW)\n";
      $code .= "vmovdqa      %xmm9, $XMM_STORAGE + 16*3($TW)\n";
      $code .= "vmovdqa      %xmm10, $XMM_STORAGE + 16*4($TW)\n";
      $code .= "vmovdqa      %xmm11, $XMM_STORAGE + 16*5($TW)\n";
      $code .= "vmovdqa      %xmm12, $XMM_STORAGE + 16*6($TW)\n";
      $code .= "vmovdqa      %xmm13, $XMM_STORAGE + 16*7($TW)\n";
      $code .= "vmovdqa      %xmm14, $XMM_STORAGE + 16*8($TW)\n";
      $code .= "vmovdqa      %xmm15, $XMM_STORAGE + 16*9($TW)\n";
    }

    $code .= "mov 	 \$0x87, $gf_poly_8b\n";
    $code .= "vmovdqu 	 ($tweak),%xmm1\n";      # read initial tweak values

    encrypt_tweak("%xmm1", $is_128);

    if ($win64) {
      $code .= "mov	 $input, 8 + 8*5(%rbp)\n";  # ciphertext pointer
      $code .= "mov        $output, 8 + 8*6(%rbp)\n"; # plaintext pointer
    }

    {
    $code.=<<___;

    cmp 	 \$0x80,$length
    jl 	 .L_less_than_128_bytes_${rndsuffix}
    vpbroadcastq 	 $gf_poly_8b,$ZPOLY
    cmp 	 \$0x100,$length
    jge 	 .L_start_by16_${rndsuffix}
    cmp 	 \$0x80,$length
    jge 	 .L_start_by8_${rndsuffix}

    .L_do_n_blocks_${rndsuffix}:
    cmp 	 \$0x0,$length
    je 	 .L_ret_${rndsuffix}
    cmp 	 \$0x70,$length
    jge 	 .L_remaining_num_blocks_is_7_${rndsuffix}
    cmp 	 \$0x60,$length
    jge 	 .L_remaining_num_blocks_is_6_${rndsuffix}
    cmp 	 \$0x50,$length
    jge 	 .L_remaining_num_blocks_is_5_${rndsuffix}
    cmp 	 \$0x40,$length
    jge 	 .L_remaining_num_blocks_is_4_${rndsuffix}
    cmp 	 \$0x30,$length
    jge 	 .L_remaining_num_blocks_is_3_${rndsuffix}
    cmp 	 \$0x20,$length
    jge 	 .L_remaining_num_blocks_is_2_${rndsuffix}
    cmp 	 \$0x10,$length
    jge 	 .L_remaining_num_blocks_is_1_${rndsuffix}
    vmovdqa 	 %xmm0,%xmm8
    vmovdqa 	 %xmm9,%xmm0
    jmp 	 .L_steal_cipher_${rndsuffix}

    .L_remaining_num_blocks_is_7_${rndsuffix}:
    mov 	 \$0x0000ffffffffffff,$tmp1
    kmovq 	 $tmp1,%k1
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2{%k1}
    add 	 \$0x70,$input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %zmm2,0x40($output){%k1}
    add 	 \$0x70,$output
    vextracti32x4 	 \$0x2,%zmm2,%xmm8
    vextracti32x4 	 \$0x3,%zmm10,%xmm0
    and 	 \$0xf,$length
    je 	 .L_ret_${rndsuffix}
    jmp 	 .L_steal_cipher_${rndsuffix}

    .L_remaining_num_blocks_is_6_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%ymm2
    add 	 \$0x60,$input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %ymm2,0x40($output)
    add 	 \$0x60,$output
    vextracti32x4 	 \$0x1,%zmm2,%xmm8
    vextracti32x4 	 \$0x2,%zmm10,%xmm0
    and 	 \$0xf,$length
    je 	 .L_ret_${rndsuffix}
    jmp 	 .L_steal_cipher_${rndsuffix}

    .L_remaining_num_blocks_is_5_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu 	 0x40($input),%xmm2
    add 	 \$0x50,$input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu 	 %xmm2,0x40($output)
    add 	 \$0x50,$output
    vmovdqa 	 %xmm2,%xmm8
    vextracti32x4 	 \$0x1,%zmm10,%xmm0
    and 	 \$0xf,$length
    je 	 .L_ret_${rndsuffix}
    jmp 	 .L_steal_cipher_${rndsuffix}

    .L_remaining_num_blocks_is_4_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    add 	 \$0x40,$input
___
    }

    encrypt_by_four("%zmm1", "%zmm9", "%zmm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1,($output)
    add	\$0x40,$output
    vextracti32x4	\$0x3,%zmm1,%xmm8
    vmovdqa64	%xmm10, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_remaining_num_blocks_is_3_${rndsuffix}:
    mov	\$-1, $tmp1
    shr	\$0x10, $tmp1
    kmovq	$tmp1, %k1
    vmovdqu8	($input), %zmm1{%k1}
    add	\$0x30, $input
___
    }

    encrypt_by_four("%zmm1", "%zmm9", "%zmm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, ($output){%k1}
    add	\$0x30, $output
    vextracti32x4	\$0x2, %zmm1, %xmm8
    vextracti32x4	\$0x3, %zmm9, %xmm0
    and	\$0xf, $length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_remaining_num_blocks_is_2_${rndsuffix}:
    vmovdqu8	($input), %ymm1
    add	\$0x20, $input
___
    }

    encrypt_by_four("%ymm1", "%ymm9", "%ymm0", $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %ymm1,($output)
    add 	 \$0x20,$output
    vextracti32x4	\$0x1, %zmm1, %xmm8
    vextracti32x4	\$0x2,%zmm9,%xmm0
    and 	 \$0xf,$length
    je 	 .L_ret_${rndsuffix}
    jmp 	 .L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_remaining_num_blocks_is_1_${rndsuffix}:
    vmovdqu 	 ($input),%xmm1
    add 	 \$0x10,$input
___
    }

    encrypt_final("%xmm1", "%xmm9", $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    add 	 \$0x10,$output
    vmovdqa 	 %xmm1,%xmm8
    vextracti32x4 	 \$0x1,%zmm9,%xmm0
    and 	 \$0xf,$length
    je 	 .L_ret_${rndsuffix}
    jmp 	 .L_steal_cipher_${rndsuffix}


    .L_start_by16_${rndsuffix}:
    vbroadcasti32x4 	 ($TW),%zmm0
    vbroadcasti32x4 shufb_15_7(%rip),%zmm8
    mov 	 \$0xaa,$tmp1
    kmovq 	 $tmp1,%k2
    vpshufb 	 %zmm8,%zmm0,%zmm1
    vpsllvq const_dq3210(%rip),%zmm0,%zmm4
    vpsrlvq const_dq5678(%rip),%zmm1,%zmm2
    vpclmulqdq 	 \$0x0,%zmm25,%zmm2,%zmm3
    vpxorq 	 %zmm2,%zmm4,%zmm4{%k2}
    vpxord 	 %zmm4,%zmm3,%zmm9
    vpsllvq const_dq7654(%rip),%zmm0,%zmm5
    vpsrlvq const_dq1234(%rip),%zmm1,%zmm6
    vpclmulqdq 	 \$0x0,%zmm25,%zmm6,%zmm7
    vpxorq 	 %zmm6,%zmm5,%zmm5{%k2}
    vpxord 	 %zmm5,%zmm7,%zmm10
    vpsrldq 	 \$0xf,%zmm9,%zmm13
    vpclmulqdq 	 \$0x0,%zmm25,%zmm13,%zmm14
    vpslldq 	 \$0x1,%zmm9,%zmm11
    vpxord 	 %zmm14,%zmm11,%zmm11
    vpsrldq 	 \$0xf,%zmm10,%zmm15
    vpclmulqdq 	 \$0x0,%zmm25,%zmm15,%zmm16
    vpslldq 	 \$0x1,%zmm10,%zmm12
    vpxord 	 %zmm16,%zmm12,%zmm12

    .L_main_loop_run_16_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2
    vmovdqu8 	 0x80($input),%zmm3
    vmovdqu8 	 0xc0($input),%zmm4
    add 	 \$0x100,$input
___
    }

    encrypt_by_16_zmm("%zmm1", "%zmm2", "%zmm3", "%zmm4", "%zmm9",
                      "%zmm10", "%zmm11", "%zmm12", "%zmm0", 0, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %zmm2,0x40($output)
    vmovdqu8 	 %zmm3,0x80($output)
    vmovdqu8 	 %zmm4,0xc0($output)
    add 	 \$0x100,$output
    sub 	 \$0x100,$length
    cmp 	 \$0x100,$length
    jae 	 .L_main_loop_run_16_${rndsuffix}
    cmp 	 \$0x80,$length
    jae 	 .L_main_loop_run_8_${rndsuffix}
    vextracti32x4 	 \$0x3,%zmm4,%xmm0
    jmp 	 .L_do_n_blocks_${rndsuffix}

    .L_start_by8_${rndsuffix}:
    vbroadcasti32x4 	 ($TW),%zmm0
    vbroadcasti32x4 shufb_15_7(%rip),%zmm8
    mov 	 \$0xaa,$tmp1
    kmovq 	 $tmp1,%k2
    vpshufb 	 %zmm8,%zmm0,%zmm1
    vpsllvq const_dq3210(%rip),%zmm0,%zmm4
    vpsrlvq const_dq5678(%rip),%zmm1,%zmm2
    vpclmulqdq 	 \$0x0,%zmm25,%zmm2,%zmm3
    vpxorq 	 %zmm2,%zmm4,%zmm4{%k2}
    vpxord 	 %zmm4,%zmm3,%zmm9
    vpsllvq const_dq7654(%rip),%zmm0,%zmm5
    vpsrlvq const_dq1234(%rip),%zmm1,%zmm6
    vpclmulqdq 	 \$0x0,%zmm25,%zmm6,%zmm7
    vpxorq 	 %zmm6,%zmm5,%zmm5{%k2}
    vpxord 	 %zmm5,%zmm7,%zmm10

    .L_main_loop_run_8_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2
    add 	 \$0x80,$input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 0, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %zmm2,0x40($output)
    add 	 \$0x80,$output
    sub 	 \$0x80,$length
    cmp 	 \$0x80,$length
    jae 	 .L_main_loop_run_8_${rndsuffix}
    vextracti32x4 	 \$0x3,%zmm2,%xmm0
    jmp 	 .L_do_n_blocks_${rndsuffix}

    .L_steal_cipher_${rndsuffix}:
    vmovdqa	%xmm8,%xmm2
    lea	vpshufb_shf_table(%rip),$TEMPLOW
    vmovdqu	($TEMPLOW,$length,1),%xmm10
    vpshufb	%xmm10,%xmm8,%xmm8
    vmovdqu	-0x10($input,$length,1),%xmm3
    vmovdqu	%xmm8,-0x10($output,$length,1)
    lea	vpshufb_shf_table(%rip),$TEMPLOW
    add	\$16, $TEMPLOW
    sub	$length,$TEMPLOW
    vmovdqu	($TEMPLOW),%xmm10
    vpxor	mask1(%rip),%xmm10,%xmm10
    vpshufb	%xmm10,%xmm3,%xmm3
    vpblendvb	%xmm10,%xmm2,%xmm3,%xmm3
    vpxor	%xmm0,%xmm3,%xmm8
    vpxor	($key1),%xmm8,%xmm8
    vaesenc	0x10($key1),%xmm8,%xmm8
    vaesenc	0x20($key1),%xmm8,%xmm8
    vaesenc	0x30($key1),%xmm8,%xmm8
    vaesenc	0x40($key1),%xmm8,%xmm8
    vaesenc	0x50($key1),%xmm8,%xmm8
    vaesenc	0x60($key1),%xmm8,%xmm8
    vaesenc	0x70($key1),%xmm8,%xmm8
    vaesenc	0x80($key1),%xmm8,%xmm8
    vaesenc	0x90($key1),%xmm8,%xmm8
___
    if ($is_128) {
      $code .= "vaesenclast	0xa0($key1),%xmm8,%xmm8\n";
    } else {
      $code .= <<___
      vaesenc	0xa0($key1),%xmm8,%xmm8
      vaesenc	0xb0($key1),%xmm8,%xmm8
      vaesenc	0xc0($key1),%xmm8,%xmm8
      vaesenc	0xd0($key1),%xmm8,%xmm8
      vaesenclast	0xe0($key1),%xmm8,%xmm8
___
    }
    $code .= "vpxor	%xmm0,%xmm8,%xmm8\n";
    $code .= "vmovdqu	%xmm8,-0x10($output)\n";
    }

    {
    $code .= <<___;
    .L_ret_${rndsuffix}:
    mov 	 $GP_STORAGE($TW),%rbx
    xor    $tmp1,$tmp1
    mov    $tmp1,$GP_STORAGE($TW)
    # Zero-out the whole of `%zmm0`.
    vpxorq %zmm0,%zmm0,%zmm0
___
    }

    if ($win64) {
      $code .= <<___;
      mov $GP_STORAGE + 8*1($TW),%rdi
      mov $tmp1,$GP_STORAGE + 8*1($TW)
      mov $GP_STORAGE + 8*2($TW),%rsi
      mov $tmp1,$GP_STORAGE + 8*2($TW)

      vmovdqa $XMM_STORAGE + 16 * 0($TW), %xmm6
      vmovdqa $XMM_STORAGE + 16 * 1($TW), %xmm7
      vmovdqa $XMM_STORAGE + 16 * 2($TW), %xmm8
      vmovdqa $XMM_STORAGE + 16 * 3($TW), %xmm9

      # Zero the 64 bytes we just restored to the xmm registers.
      vmovdqa64 %zmm0,$XMM_STORAGE($TW)

      vmovdqa $XMM_STORAGE + 16 * 4($TW), %xmm10
      vmovdqa $XMM_STORAGE + 16 * 5($TW), %xmm11
      vmovdqa $XMM_STORAGE + 16 * 6($TW), %xmm12
      vmovdqa $XMM_STORAGE + 16 * 7($TW), %xmm13

      # And again.
      vmovdqa64 %zmm0,$XMM_STORAGE + 16 * 4($TW)

      vmovdqa $XMM_STORAGE + 16 * 8($TW), %xmm14
      vmovdqa $XMM_STORAGE + 16 * 9($TW), %xmm15

      # Last round is only 32 bytes (256-bits), so we use `%ymm` as the
      # source operand.
      vmovdqa %ymm0,$XMM_STORAGE + 16 * 8($TW)
___
    }

    {
    $code .= <<___;
    mov %rbp,$TW
    pop %rbp
    vzeroupper
    ret

    .L_less_than_128_bytes_${rndsuffix}:
    vpbroadcastq	$gf_poly_8b, $ZPOLY
    cmp 	 \$0x10,$length
    jb 	 .L_ret_${rndsuffix}
    vbroadcasti32x4	($TW), %zmm0
    vbroadcasti32x4	shufb_15_7(%rip), %zmm8
    movl    \$0xaa, %r8d
    kmovq	%r8, %k2
    mov	$length,$tmp1
    and	\$0x70,$tmp1
    cmp	\$0x60,$tmp1
    je	.L_num_blocks_is_6_${rndsuffix}
    cmp	\$0x50,$tmp1
    je	.L_num_blocks_is_5_${rndsuffix}
    cmp	\$0x40,$tmp1
    je	.L_num_blocks_is_4_${rndsuffix}
    cmp	\$0x30,$tmp1
    je	.L_num_blocks_is_3_${rndsuffix}
    cmp	\$0x20,$tmp1
    je	.L_num_blocks_is_2_${rndsuffix}
    cmp	\$0x10,$tmp1
    je	.L_num_blocks_is_1_${rndsuffix}

    .L_num_blocks_is_7_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9
    vpsllvq	const_dq7654(%rip), %zmm0, %zmm5
    vpsrlvq	const_dq1234(%rip), %zmm1, %zmm6
    vpclmulqdq	\$0x00, $ZPOLY, %zmm6, %zmm7
    vpxorq	%zmm6, %zmm5, %zmm5{%k2}
    vpxord	%zmm5, %zmm7, %zmm10
    mov	\$0x0000ffffffffffff, $tmp1
    kmovq	$tmp1, %k1
    vmovdqu8	16*0($input), %zmm1
    vmovdqu8	16*4($input), %zmm2{%k1}

    add	\$0x70,$input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, 16*0($output)
    vmovdqu8	%zmm2, 16*4($output){%k1}
    add	\$0x70,$output
    vextracti32x4	\$0x2, %zmm2, %xmm8
    vextracti32x4	\$0x3, %zmm10, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_6_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9
    vpsllvq	const_dq7654(%rip), %zmm0, %zmm5
    vpsrlvq	const_dq1234(%rip), %zmm1, %zmm6
    vpclmulqdq	\$0x00, $ZPOLY, %zmm6, %zmm7
    vpxorq	%zmm6, %zmm5, %zmm5{%k2}
    vpxord	%zmm5, %zmm7, %zmm10
    vmovdqu8	16*0($input), %zmm1
    vmovdqu8	16*4($input), %ymm2
    add	\$96, $input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, 16*0($output)
    vmovdqu8	%ymm2, 16*4($output)
    add	\$96, $output

    vextracti32x4	\$0x1, %ymm2, %xmm8
    vextracti32x4	\$0x2, %zmm10, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_5_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9
    vpsllvq	const_dq7654(%rip), %zmm0, %zmm5
    vpsrlvq	const_dq1234(%rip), %zmm1, %zmm6
    vpclmulqdq	\$0x00, $ZPOLY, %zmm6, %zmm7
    vpxorq	%zmm6, %zmm5, %zmm5{%k2}
    vpxord	%zmm5, %zmm7, %zmm10
    vmovdqu8	16*0($input), %zmm1
    vmovdqu8	16*4($input), %xmm2
    add	\$80, $input
___
    }

    encrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, 16*0($output)
    vmovdqu8	%xmm2, 16*4($output)
    add	\$80, $output

    vmovdqa	%xmm2, %xmm8
    vextracti32x4	\$0x1, %zmm10, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_4_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9
    vpsllvq	const_dq7654(%rip), %zmm0, %zmm5
    vpsrlvq	const_dq1234(%rip), %zmm1, %zmm6
    vpclmulqdq	\$0x00, $ZPOLY, %zmm6, %zmm7
    vpxorq	%zmm6, %zmm5, %zmm5{%k2}
    vpxord	%zmm5, %zmm7, %zmm10
    vmovdqu8	16*0($input), %zmm1
    add	\$64, $input
___
    }

    encrypt_by_four("%zmm1", "%zmm9", "%zmm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, 16*0($output)
    add	\$64, $output
    vextracti32x4	\$0x3, %zmm1, %xmm8
    vmovdqa	%xmm10, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_3_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9
    mov	\$0x0000ffffffffffff, $tmp1
    kmovq	$tmp1, %k1
    vmovdqu8	16*0($input), %zmm1{%k1}
    add	\$48, $input
___
    }

    encrypt_by_four("%zmm1", "%zmm9", "%zmm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%zmm1, 16*0($output){%k1}
    add	\$48, $output
    vextracti32x4	\$2, %zmm1, %xmm8
    vextracti32x4	\$3, %zmm9, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_2_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9

    vmovdqu8	16*0($input), %ymm1
    add	\$32, $input
___
    }

    encrypt_by_four("%ymm1", "%ymm9", "%ymm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%ymm1, 16*0($output)
    add	\$32, $output

    vextracti32x4	\$1, %ymm1, %xmm8
    vextracti32x4	\$2, %zmm9, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
___
    }

    {
    $code .= <<___;
    .L_num_blocks_is_1_${rndsuffix}:
    vpshufb	%zmm8, %zmm0, %zmm1
    vpsllvq	const_dq3210(%rip), %zmm0, %zmm4
    vpsrlvq	const_dq5678(%rip), %zmm1, %zmm2
    vpclmulqdq	\$0x00, $ZPOLY, %zmm2, %zmm3
    vpxorq	%zmm2, %zmm4, %zmm4{%k2}
    vpxord	%zmm4, %zmm3, %zmm9

    vmovdqu8	16*0($input), %xmm1
    add	\$16, $input
___
    }

    encrypt_by_four("%ymm1", "%ymm9", "%ymm0", $is_128);

    {
    $code .= <<___;
    vmovdqu8	%xmm1, 16*0($output)
    add	\$16, $output

    vmovdqa	%xmm1, %xmm8
    vextracti32x4	\$1, %zmm9, %xmm0
    and	\$0xf,$length
    je	.L_ret_${rndsuffix}
    jmp	.L_steal_cipher_${rndsuffix}
    .cfi_endproc
___
    }
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;void aesni_xts_[128|256]_decrypt_avx512(
  # ;               const uint8_t *in,        // input data
  # ;               uint8_t *out,             // output data
  # ;               size_t length,            // sector size, in bytes
  # ;               const AES_KEY *key1,      // key used for "ECB" encryption, 16*2 bytes
  # ;               const AES_KEY *key2,      // key used for tweaking, 16*2 bytes
  # ;               const uint8_t iv[16])      // initial tweak value, 16 bytes
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  sub dec {
    my $is_128 = $_[0];
    my $rndsuffix = &random_string();

    if ($is_128) {
      $code.=<<___;
      .globl	aesni_xts_128_decrypt_avx512
      .hidden	aesni_xts_128_decrypt_avx512
      .type	aesni_xts_128_decrypt_avx512,\@function,6
      .align	32
      aesni_xts_128_decrypt_avx512:
      .cfi_startproc
      endbranch
___
    } else {
      $code.=<<___;
      .globl	aesni_xts_256_decrypt_avx512
      .hidden	aesni_xts_256_decrypt_avx512
      .type	aesni_xts_256_decrypt_avx512,\@function,6
      .align	32
      aesni_xts_256_decrypt_avx512:
      .cfi_startproc
      endbranch
___
    }
    $code .= "push 	 %rbp\n";
    $code .= "mov 	 $TW,%rbp\n";
    $code .= "sub 	 \$$VARIABLE_OFFSET,$TW\n";
    $code .= "and 	 \$0xffffffffffffffc0,$TW\n";
    $code .= "mov 	 %rbx,$GP_STORAGE($TW)\n";

    if ($win64) {
      $code .= "mov 	 %rdi,$GP_STORAGE + 8*1($TW)\n";
      $code .= "mov 	 %rsi,$GP_STORAGE + 8*2($TW)\n";
      $code .= "vmovdqa      %xmm6, $XMM_STORAGE + 16*0($TW)\n";
      $code .= "vmovdqa      %xmm7, $XMM_STORAGE + 16*1($TW)\n";
      $code .= "vmovdqa      %xmm8, $XMM_STORAGE + 16*2($TW)\n";
      $code .= "vmovdqa      %xmm9, $XMM_STORAGE + 16*3($TW)\n";
      $code .= "vmovdqa      %xmm10, $XMM_STORAGE + 16*4($TW)\n";
      $code .= "vmovdqa      %xmm11, $XMM_STORAGE + 16*5($TW)\n";
      $code .= "vmovdqa      %xmm12, $XMM_STORAGE + 16*6($TW)\n";
      $code .= "vmovdqa      %xmm13, $XMM_STORAGE + 16*7($TW)\n";
      $code .= "vmovdqa      %xmm14, $XMM_STORAGE + 16*8($TW)\n";
      $code .= "vmovdqa      %xmm15, $XMM_STORAGE + 16*9($TW)\n";
    }

    $code .= "mov 	 \$0x87, $gf_poly_8b\n";
    $code .= "vmovdqu 	 ($tweak),%xmm1\n";      # read initial tweak values

    encrypt_tweak("%xmm1", $is_128);

    if ($win64) {
      $code .= "mov	 $input, 8 + 8*5(%rbp)\n"; # ciphertext pointer
      $code .= "mov        $output, 8 + 8*6(%rbp)\n"; # plaintext pointer
    }

    {
    $code.=<<___;

    cmp 	 \$0x80,$length
    jb 	 .L_less_than_128_bytes_${rndsuffix}
    vpbroadcastq 	 $gf_poly_8b,$ZPOLY
    cmp 	 \$0x100,$length
    jge 	 .L_start_by16_${rndsuffix}
    jmp 	 .L_start_by8_${rndsuffix}

    .L_do_n_blocks_${rndsuffix}:
    cmp 	 \$0x0,$length
    je 	 .L_ret_${rndsuffix}
    cmp 	 \$0x70,$length
    jge 	 .L_remaining_num_blocks_is_7_${rndsuffix}
    cmp 	 \$0x60,$length
    jge 	 .L_remaining_num_blocks_is_6_${rndsuffix}
    cmp 	 \$0x50,$length
    jge 	 .L_remaining_num_blocks_is_5_${rndsuffix}
    cmp 	 \$0x40,$length
    jge 	 .L_remaining_num_blocks_is_4_${rndsuffix}
    cmp 	 \$0x30,$length
    jge 	 .L_remaining_num_blocks_is_3_${rndsuffix}
    cmp 	 \$0x20,$length
    jge 	 .L_remaining_num_blocks_is_2_${rndsuffix}
    cmp 	 \$0x10,$length
    jge 	 .L_remaining_num_blocks_is_1_${rndsuffix}

    # _remaining_num_blocks_is_0:
    vmovdqu		%xmm5, %xmm1
    # xmm5 contains last full block to decrypt with next teawk
___
    }
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 1, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu %xmm1, -0x10($output)
    vmovdqa %xmm1, %xmm8

    # Calc previous tweak
    mov		\$0x1,$tmp1
    kmovq		$tmp1, %k1
    vpsllq	\$0x3f,%xmm9,%xmm13
    vpsraq	\$0x3f,%xmm13,%xmm14
    vpandq	%xmm25,%xmm14,%xmm5
    vpxorq        %xmm5,%xmm9,%xmm9{%k1}
    vpsrldq       \$0x8,%xmm9,%xmm10
    .byte 98, 211, 181, 8, 115, 194, 1 #vpshrdq \$0x1,%xmm10,%xmm9,%xmm0
    vpslldq       \$0x8,%xmm13,%xmm13
    vpxorq        %xmm13,%xmm0,%xmm0
    jmp           .L_steal_cipher_${rndsuffix}

    .L_remaining_num_blocks_is_7_${rndsuffix}:
    mov 	 \$0xffffffffffffffff,$tmp1
    shr 	 \$0x10,$tmp1
    kmovq 	 $tmp1,%k1
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2{%k1}
    add 	         \$0x70,$input
    and            \$0xf,$length
    je             .L_done_7_remain_${rndsuffix}
    vextracti32x4   \$0x2,%zmm10,%xmm12
    vextracti32x4   \$0x3,%zmm10,%xmm13
    vinserti32x4    \$0x2,%xmm13,%zmm10,%zmm10
___
    }

    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1, ($output)
    vmovdqu8 	 %zmm2, 0x40($output){%k1}
    add 	         \$0x70, $output
    vextracti32x4  \$0x2,%zmm2,%xmm8
    vmovdqa        %xmm12,%xmm0
    jmp            .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_7_remain_${rndsuffix}:\n";
    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8        %zmm1, ($output)
    vmovdqu8        %zmm2, 0x40($output){%k1}
    jmp     .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_6_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%ymm2
    add 	         \$0x60,$input
    and            \$0xf, $length
    je             .L_done_6_remain_${rndsuffix}
    vextracti32x4   \$0x1,%zmm10,%xmm12
    vextracti32x4   \$0x2,%zmm10,%xmm13
    vinserti32x4    \$0x1,%xmm13,%zmm10,%zmm10
___
    }

    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1, ($output)
    vmovdqu8 	 %ymm2, 0x40($output)
    add 	         \$0x60,$output
    vextracti32x4  \$0x1,%zmm2,%xmm8
    vmovdqa        %xmm12,%xmm0
    jmp            .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_6_remain_${rndsuffix}:\n";
    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8        %zmm1, ($output)
    vmovdqu8        %ymm2,0x40($output)
    jmp             .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_5_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu 	 0x40($input),%xmm2
    add 	         \$0x50,$input
    and            \$0xf,$length
    je             .L_done_5_remain_${rndsuffix}
    vmovdqa        %xmm10,%xmm12
    vextracti32x4  \$0x1,%zmm10,%xmm10
___
    }

    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8         %zmm1, ($output)
    vmovdqu          %xmm2, 0x40($output)
    add              \$0x50, $output
    vmovdqa          %xmm2,%xmm8
    vmovdqa          %xmm12,%xmm0
    jmp              .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_5_remain_${rndsuffix}:\n";
    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8        %zmm1, ($output)
    vmovdqu8        %xmm2, 0x40($output)
    jmp             .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_4_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    add 	         \$0x40,$input
    and            \$0xf, $length
    je             .L_done_4_remain_${rndsuffix}
    vextracti32x4   \$0x3,%zmm9,%xmm12
    vinserti32x4    \$0x3,%xmm10,%zmm9,%zmm9
___
    }

    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8        %zmm1,($output)
    add             \$0x40,$output
    vextracti32x4   \$0x3,%zmm1,%xmm8
    vmovdqa         %xmm12,%xmm0
    jmp             .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_4_remain_${rndsuffix}:\n";
    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 1, $is_128);

    {
    $code .= <<___;
    vmovdqu8        %zmm1, ($output)
    jmp             .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_3_${rndsuffix}:
    vmovdqu         ($input),%xmm1
    vmovdqu         0x10($input),%xmm2
    vmovdqu         0x20($input),%xmm3
    add             \$0x30,$input
    and             \$0xf,$length
    je              .L_done_3_remain_${rndsuffix}
    vextracti32x4   \$0x2,%zmm9,%xmm13
    vextracti32x4   \$0x1,%zmm9,%xmm10
    vextracti32x4   \$0x3,%zmm9,%xmm11
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 3, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    add 	         \$0x30,$output
    vmovdqa 	 %xmm3,%xmm8
    vmovdqa        %xmm13,%xmm0
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }
    $code .= "\n.L_done_3_remain_${rndsuffix}:\n";
    $code .= "vextracti32x4   \$0x1,%zmm9,%xmm10\n";
    $code .= "vextracti32x4   \$0x2,%zmm9,%xmm11\n";

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 3, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu %xmm1,($output)
    vmovdqu %xmm2,0x10($output)
    vmovdqu %xmm3,0x20($output)
    jmp     .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_2_${rndsuffix}:
    vmovdqu         ($input),%xmm1
    vmovdqu         0x10($input),%xmm2
    add             \$0x20,$input
    and             \$0xf,$length
    je              .L_done_2_remain_${rndsuffix}
    vextracti32x4   \$0x2,%zmm9,%xmm10
    vextracti32x4   \$0x1,%zmm9,%xmm12
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 2, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    add 	         \$0x20,$output
    vmovdqa 	 %xmm2,%xmm8
    vmovdqa 	 %xmm12,%xmm0
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }
    $code .= "\n.L_done_2_remain_${rndsuffix}:\n";
    $code .= "vextracti32x4   \$0x1,%zmm9,%xmm10\n";

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 2, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu   %xmm1,($output)
    vmovdqu   %xmm2,0x10($output)
    jmp       .L_ret_${rndsuffix}

    .L_remaining_num_blocks_is_1_${rndsuffix}:
    vmovdqu 	 ($input),%xmm1
    add 	         \$0x10,$input
    and            \$0xf,$length
    je             .L_done_1_remain_${rndsuffix}
    vextracti32x4  \$0x1,%zmm9,%xmm11
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm11", "%xmm10", "%xmm9", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 1, 1, $is_128);
    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    add 	         \$0x10,$output
    vmovdqa 	 %xmm1,%xmm8
    vmovdqa 	 %xmm9,%xmm0
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_1_remain_${rndsuffix}:\n";

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 1, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu   %xmm1, ($output)
    jmp       .L_ret_${rndsuffix}

    .L_start_by16_${rndsuffix}:
    vbroadcasti32x4 	 ($TW),%zmm0
    vbroadcasti32x4 shufb_15_7(%rip),%zmm8
    mov 	 \$0xaa,$tmp1
    kmovq 	 $tmp1,%k2

    # Mult tweak by 2^{3, 2, 1, 0}
    vpshufb 	 %zmm8,%zmm0,%zmm1
    vpsllvq const_dq3210(%rip),%zmm0,%zmm4
    vpsrlvq const_dq5678(%rip),%zmm1,%zmm2
    vpclmulqdq 	 \$0x0,$ZPOLY,%zmm2,%zmm3
    vpxorq 	 %zmm2,%zmm4,%zmm4{%k2}
    vpxord 	 %zmm4,%zmm3,%zmm9

    # Mult tweak by 2^{7, 6, 5, 4}
    vpsllvq const_dq7654(%rip),%zmm0,%zmm5
    vpsrlvq const_dq1234(%rip),%zmm1,%zmm6
    vpclmulqdq 	 \$0x0,%zmm25,%zmm6,%zmm7
    vpxorq 	 %zmm6,%zmm5,%zmm5{%k2}
    vpxord 	 %zmm5,%zmm7,%zmm10

    # Make next 8 tweek values by all x 2^8
    vpsrldq 	 \$0xf,%zmm9,%zmm13
    vpclmulqdq 	 \$0x0,%zmm25,%zmm13,%zmm14
    vpslldq 	 \$0x1,%zmm9,%zmm11
    vpxord 	 %zmm14,%zmm11,%zmm11

    vpsrldq 	 \$0xf,%zmm10,%zmm15
    vpclmulqdq 	 \$0x0,%zmm25,%zmm15,%zmm16
    vpslldq 	 \$0x1,%zmm10,%zmm12
    vpxord 	 %zmm16,%zmm12,%zmm12

    .L_main_loop_run_16_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2
    vmovdqu8 	 0x80($input),%zmm3
    vmovdqu8 	 0xc0($input),%zmm4
    vmovdqu8 	 0xf0($input),%xmm5
    add 	 \$0x100,$input
___
    }

    decrypt_by_16_zmm("%zmm1", "%zmm2", "%zmm3", "%zmm4", "%zmm9",
                      "%zmm10", "%zmm11", "%zmm12", "%zmm0", 0, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %zmm2,0x40($output)
    vmovdqu8 	 %zmm3,0x80($output)
    vmovdqu8 	 %zmm4,0xc0($output)
    add 	 \$0x100,$output
    sub 	 \$0x100,$length
    cmp 	 \$0x100,$length
    jge 	 .L_main_loop_run_16_${rndsuffix}

    cmp 	 \$0x80,$length
    jge 	 .L_main_loop_run_8_${rndsuffix}
    jmp 	 .L_do_n_blocks_${rndsuffix}

    .L_start_by8_${rndsuffix}:
    # Make first 7 tweek values
    vbroadcasti32x4 	 ($TW),%zmm0
    vbroadcasti32x4 shufb_15_7(%rip),%zmm8
    mov 	 \$0xaa,$tmp1
    kmovq 	 $tmp1,%k2

    # Mult tweak by 2^{3, 2, 1, 0}
    vpshufb 	 %zmm8,%zmm0,%zmm1
    vpsllvq const_dq3210(%rip),%zmm0,%zmm4
    vpsrlvq const_dq5678(%rip),%zmm1,%zmm2
    vpclmulqdq 	 \$0x0,%zmm25,%zmm2,%zmm3
    vpxorq 	 %zmm2,%zmm4,%zmm4{%k2}
    vpxord 	 %zmm4,%zmm3,%zmm9

    # Mult tweak by 2^{7, 6, 5, 4}
    vpsllvq const_dq7654(%rip),%zmm0,%zmm5
    vpsrlvq const_dq1234(%rip),%zmm1,%zmm6
    vpclmulqdq 	 \$0x0,%zmm25,%zmm6,%zmm7
    vpxorq 	 %zmm6,%zmm5,%zmm5{%k2}
    vpxord 	 %zmm5,%zmm7,%zmm10

    .L_main_loop_run_8_${rndsuffix}:
    vmovdqu8 	 ($input),%zmm1
    vmovdqu8 	 0x40($input),%zmm2
    vmovdqu8 	 0x70($input),%xmm5
    add 	         \$0x80,$input
___
    }


    decrypt_by_eight_zmm("%zmm1", "%zmm2", "%zmm9", "%zmm10", "%zmm0", 0, $is_128);

    {
    $code .= <<___;
    vmovdqu8 	 %zmm1,($output)
    vmovdqu8 	 %zmm2,0x40($output)
    add 	 \$0x80,$output
    sub 	 \$0x80,$length
    cmp 	 \$0x80,$length
    jge 	 .L_main_loop_run_8_${rndsuffix}
    jmp 	 .L_do_n_blocks_${rndsuffix}

    .L_steal_cipher_${rndsuffix}:
    # start cipher stealing simplified: xmm8-last cipher block, xmm0-next tweak
    vmovdqa 	 %xmm8,%xmm2

    # shift xmm8 to the left by 16-N_val bytes
    lea vpshufb_shf_table(%rip),$TEMPLOW
    vmovdqu 	 ($TEMPLOW,$length,1),%xmm10
    vpshufb 	 %xmm10,%xmm8,%xmm8


    vmovdqu 	 -0x10($input,$length,1),%xmm3
    vmovdqu 	 %xmm8,-0x10($output,$length,1)

    # shift xmm3 to the right by 16-N_val bytes
    lea vpshufb_shf_table(%rip), $TEMPLOW
    add \$16, $TEMPLOW
    sub 	 $length,$TEMPLOW
    vmovdqu 	 ($TEMPLOW),%xmm10
    vpxor mask1(%rip),%xmm10,%xmm10
    vpshufb 	 %xmm10,%xmm3,%xmm3

    vpblendvb 	 %xmm10,%xmm2,%xmm3,%xmm3

    # xor Tweak value
    vpxor 	 %xmm0,%xmm3,%xmm8

    # decrypt last block with cipher stealing
    vpxor	($key1),%xmm8,%xmm8
    vaesdec	0x10($key1),%xmm8,%xmm8
    vaesdec	0x20($key1),%xmm8,%xmm8
    vaesdec	0x30($key1),%xmm8,%xmm8
    vaesdec	0x40($key1),%xmm8,%xmm8
    vaesdec	0x50($key1),%xmm8,%xmm8
    vaesdec	0x60($key1),%xmm8,%xmm8
    vaesdec	0x70($key1),%xmm8,%xmm8
    vaesdec	0x80($key1),%xmm8,%xmm8
    vaesdec	0x90($key1),%xmm8,%xmm8
___
    if ($is_128) {
      $code .= "vaesdeclast	0xa0($key1),%xmm8,%xmm8\n";
    } else {
      $code .= <<___;
      vaesdec	0xa0($key1),%xmm8,%xmm8
      vaesdec	0xb0($key1),%xmm8,%xmm8
      vaesdec	0xc0($key1),%xmm8,%xmm8
      vaesdec	0xd0($key1),%xmm8,%xmm8
      vaesdeclast	0xe0($key1),%xmm8,%xmm8
___
    }
    $code .= <<___
    # xor Tweak value
    vpxor 	 %xmm0,%xmm8,%xmm8

    .L_done_${rndsuffix}:
    # store last ciphertext value
    vmovdqu 	 %xmm8,-0x10($output)
___
    }

    {
    $code .= <<___;
    .L_ret_${rndsuffix}:
    mov 	 $GP_STORAGE($TW),%rbx
    xor    $tmp1,$tmp1
    mov    $tmp1,$GP_STORAGE($TW)
    # Zero-out the whole of `%zmm0`.
    vpxorq %zmm0,%zmm0,%zmm0
___
    }

    if ($win64) {
      $code .= <<___;
      mov $GP_STORAGE + 8*1($TW),%rdi
      mov $tmp1,$GP_STORAGE + 8*1($TW)
      mov $GP_STORAGE + 8*2($TW),%rsi
      mov $tmp1,$GP_STORAGE + 8*2($TW)

      vmovdqa $XMM_STORAGE + 16 * 0($TW), %xmm6
      vmovdqa $XMM_STORAGE + 16 * 1($TW), %xmm7
      vmovdqa $XMM_STORAGE + 16 * 2($TW), %xmm8
      vmovdqa $XMM_STORAGE + 16 * 3($TW), %xmm9

      # Zero the 64 bytes we just restored to the xmm registers.
      vmovdqa64 %zmm0,$XMM_STORAGE($TW)

      vmovdqa $XMM_STORAGE + 16 * 4($TW), %xmm10
      vmovdqa $XMM_STORAGE + 16 * 5($TW), %xmm11
      vmovdqa $XMM_STORAGE + 16 * 6($TW), %xmm12
      vmovdqa $XMM_STORAGE + 16 * 7($TW), %xmm13

      # And again.
      vmovdqa64 %zmm0,$XMM_STORAGE + 16 * 4($TW)

      vmovdqa $XMM_STORAGE + 16 * 8($TW), %xmm14
      vmovdqa $XMM_STORAGE + 16 * 9($TW), %xmm15

      # Last round is only 32 bytes (256-bits), so we use `%ymm` as the
      # source operand.
      vmovdqa %ymm0,$XMM_STORAGE + 16 * 8($TW)
___
    }

    {
    $code .= <<___;
    mov %rbp,$TW
    pop %rbp
    vzeroupper
    ret

    .L_less_than_128_bytes_${rndsuffix}:
    cmp 	 \$0x10,$length
    jb 	 .L_ret_${rndsuffix}

    mov 	 $length,$tmp1
    and 	 \$0x70,$tmp1
    cmp 	 \$0x60,$tmp1
    je 	 .L_num_blocks_is_6_${rndsuffix}
    cmp 	 \$0x50,$tmp1
    je 	 .L_num_blocks_is_5_${rndsuffix}
    cmp 	 \$0x40,$tmp1
    je 	 .L_num_blocks_is_4_${rndsuffix}
    cmp 	 \$0x30,$tmp1
    je 	 .L_num_blocks_is_3_${rndsuffix}
    cmp 	 \$0x20,$tmp1
    je 	 .L_num_blocks_is_2_${rndsuffix}
    cmp 	 \$0x10,$tmp1
    je 	 .L_num_blocks_is_1_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_7_${rndsuffix}:\n";
    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 7);

    {
    $code .= <<___;
    add    \$0x70,$input
    and    \$0xf,$length
    je      .L_done_7_${rndsuffix}

    .L_steal_cipher_7_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm15,%xmm16
     vmovdqa     0x10($TW),%xmm15
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 7, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    vmovdqu 	 %xmm5,0x40($output)
    vmovdqu 	 %xmm6,0x50($output)
    add 	         \$0x70,$output
    vmovdqa64 	 %xmm16,%xmm0
    vmovdqa 	 %xmm7,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_7_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 7, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    vmovdqu 	 %xmm5,0x40($output)
    vmovdqu 	 %xmm6,0x50($output)
    add 	         \$0x70,$output
    vmovdqa 	 %xmm7,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_6_${rndsuffix}:\n";
    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 6);

    {
    $code .= <<___;
    add    \$0x60,$input
    and    \$0xf,$length
    je      .L_done_6_${rndsuffix}

    .L_steal_cipher_6_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm14,%xmm15
     vmovdqa     0x10($TW),%xmm14
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 6, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    vmovdqu 	 %xmm5,0x40($output)
    add 	         \$0x60,$output
    vmovdqa 	 %xmm15,%xmm0
    vmovdqa 	 %xmm6,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }
    $code .= "\n.L_done_6_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 6, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    vmovdqu 	 %xmm5,0x40($output)
    add 	         \$0x60,$output
    vmovdqa 	 %xmm6,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_5_${rndsuffix}:\n";
    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 5);

    {
    $code .= <<___;
    add    \$0x50,$input
    and    \$0xf,$length
    je      .L_done_5_${rndsuffix}

    .L_steal_cipher_5_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm13,%xmm14
     vmovdqa     0x10($TW),%xmm13
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 5, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    add 	         \$0x50,$output
    vmovdqa 	 %xmm14,%xmm0
    vmovdqa 	 %xmm5,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_5_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 5, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    vmovdqu 	 %xmm4,0x30($output)
    add 	         \$0x50,$output
    vmovdqa 	 %xmm5,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_4_${rndsuffix}:\n";

    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 4);

    {
    $code .= <<___;
    add    \$0x40,$input
    and    \$0xf,$length
    je      .L_done_4_${rndsuffix}

    .L_steal_cipher_4_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm12,%xmm13
     vmovdqa     0x10($TW),%xmm12
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 4, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    add 	         \$0x40,$output
    vmovdqa 	 %xmm13,%xmm0
    vmovdqa 	 %xmm4,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_4_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 4, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    vmovdqu 	 %xmm3,0x20($output)
    add 	         \$0x40,$output
    vmovdqa 	 %xmm4,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_3_${rndsuffix}:\n";

    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 3);

    {
    $code .= <<___;
    add    \$0x30,$input
    and    \$0xf,$length
    je      .L_done_3_${rndsuffix}

    .L_steal_cipher_3_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm11,%xmm12
     vmovdqa     0x10($TW),%xmm11
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 3, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    add 	         \$0x30,$output
    vmovdqa 	 %xmm12,%xmm0
    vmovdqa 	 %xmm3,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }
    $code .= "\n.L_done_3_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 3, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    vmovdqu 	 %xmm2,0x10($output)
    add 	         \$0x30,$output
    vmovdqa 	 %xmm3,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_2_${rndsuffix}:\n";

    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 2);

    {
    $code .= <<___;
    add    \$0x20,$input
    and    \$0xf,$length
    je      .L_done_2_${rndsuffix}

    .L_steal_cipher_2_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm10,%xmm11
     vmovdqa     0x10($TW),%xmm10
___
    }

    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 2, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    add 	         \$0x20,$output
    vmovdqa 	 %xmm11,%xmm0
    vmovdqa 	 %xmm2,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }

    $code .= "\n.L_done_2_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 2, 1, $is_128);

    {
    $code .= <<___;
    vmovdqu 	 %xmm1,($output)
    add 	         \$0x20,$output
    vmovdqa 	 %xmm2,%xmm8
    jmp 	         .L_done_${rndsuffix}
___
    }

    $code .= "\n.L_num_blocks_is_1_${rndsuffix}:\n";

    initialize("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
               "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
               "%xmm13", "%xmm14", "%xmm15", 1);

    {
    $code .= <<___;
    add    \$0x10,$input
    and    \$0xf,$length
    je      .L_done_1_${rndsuffix}

    .L_steal_cipher_1_${rndsuffix}:
     xor         $gf_poly_8b_temp, $gf_poly_8b_temp
     shl         \$1, $TEMPLOW
     adc         $TEMPHIGH, $TEMPHIGH
     cmovc       $gf_poly_8b, $gf_poly_8b_temp
     xor         $gf_poly_8b_temp, $TEMPLOW
     mov         $TEMPLOW,0x10($TW)
     mov         $TEMPHIGH,0x18($TW)
     vmovdqa64   %xmm9,%xmm10
     vmovdqa     0x10($TW),%xmm9
___
    }
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 1, 1, $is_128);

    {
    $code .= <<___;
    add 	         \$0x10,$output
    vmovdqa 	 %xmm10,%xmm0
    vmovdqa 	 %xmm1,%xmm8
    jmp 	         .L_steal_cipher_${rndsuffix}
___
    }
    $code .= "\n.L_done_1_${rndsuffix}:\n";
    decrypt_initial("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5", "%xmm6",
                    "%xmm7", "%xmm8", "%xmm9", "%xmm10", "%xmm11", "%xmm12",
                    "%xmm13", "%xmm14", "%xmm15", "%xmm0", 1, 1, $is_128);

    {
    $code .= <<___;
    add 	         \$0x10,$output
    vmovdqa 	 %xmm1,%xmm8
    jmp 	         .L_done_${rndsuffix}
    .cfi_endproc
___
    }

  }

  # The only difference between AES-XTS-128 and -256 is the number of rounds,
  # so we generate from the same perlasm base, extending to 14 rounds when
  # `$is_128' is 0.

  enc(1);
  dec(1);

  enc(0);
  dec(0);

  $code .= <<___;
  .section .rodata
  .align 16

  vpshufb_shf_table:
    .quad 0x8786858483828100, 0x8f8e8d8c8b8a8988
    .quad 0x0706050403020100, 0x000e0d0c0b0a0908

  mask1:
    .quad 0x8080808080808080, 0x8080808080808080

  const_dq3210:
    .quad 0, 0, 1, 1, 2, 2, 3, 3
  const_dq5678:
    .quad 8, 8, 7, 7, 6, 6, 5, 5
  const_dq7654:
    .quad 4, 4, 5, 5, 6, 6, 7, 7
  const_dq1234:
    .quad 4, 4, 3, 3, 2, 2, 1, 1

  shufb_15_7:
    .byte  15, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 7, 0xff, 0xff
    .byte  0xff, 0xff, 0xff, 0xff, 0xff

.text
___

} else {
    $code .= <<___;
    .text
    .globl  aesni_xts_128_encrypt_avx512
    .globl  aesni_xts_128_decrypt_avx512

    aesni_xts_128_encrypt_avx512:
    aesni_xts_128_decrypt_avx512:
    .byte   0x0f,0x0b    # ud2
    ret

    .globl  aesni_xts_256_encrypt_avx512
    .globl  aesni_xts_256_decrypt_avx512

    aesni_xts_256_encrypt_avx512:
    aesni_xts_256_decrypt_avx512:
    .byte   0x0f,0x0b    # ud2
    ret

    .globl  aesni_xts_avx512_eligible
    .type   aesni_xts_avx512_eligible,\@abi-omnipotent
    aesni_xts_avx512_eligible:
    xor	%eax,%eax
    ret
    .size   aesni_xts_avx512_eligible, .-aesni_xts_avx512_eligible

___
}

print $code;

close STDOUT or die "error closing STDOUT: $!";
