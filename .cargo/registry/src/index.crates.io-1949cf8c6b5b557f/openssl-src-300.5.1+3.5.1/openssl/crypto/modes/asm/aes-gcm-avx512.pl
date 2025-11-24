# Copyright 2021-2024 The OpenSSL Project Authors. All Rights Reserved.
# Copyright (c) 2021, Intel Corporation. All Rights Reserved.
#
# Licensed under the Apache License 2.0 (the "License").  You may not use
# this file except in compliance with the License.  You can obtain a copy
# in the file LICENSE in the source distribution or at
# https://www.openssl.org/source/license.html
#
#
# This implementation is based on the AES-GCM code (AVX512VAES + VPCLMULQDQ)
# from Intel(R) Multi-Buffer Crypto for IPsec Library v1.1
# (https://github.com/intel/intel-ipsec-mb).
# Original author is Tomasz Kantecki <tomasz.kantecki@intel.com>.
#
# References:
#  [1] Vinodh Gopal et. al. Optimized Galois-Counter-Mode Implementation on
#      Intel Architecture Processors. August, 2010.
#  [2] Erdinc Ozturk et. al. Enabling High-Performance Galois-Counter-Mode on
#      Intel Architecture Processors. October, 2012.
#  [3] Shay Gueron et. al. Intel Carry-Less Multiplication Instruction and its
#      Usage for Computing the GCM Mode. May, 2010.
#
#
# December 2021
#
# Initial release.
#
# GCM128_CONTEXT structure has storage for 16 hkeys only, but this
# implementation can use up to 48.  To avoid extending the context size,
# precompute and store in the context first 16 hkeys only, and compute the rest
# on demand keeping them in the local frame.
#
#======================================================================
# $output is the last argument if it looks like a file (it has an extension)
# $flavour is the first argument if it doesn't look like a file
$output  = $#ARGV >= 0 && $ARGV[$#ARGV] =~ m|\.\w+$| ? pop   : undef;
$flavour = $#ARGV >= 0 && $ARGV[0] !~ m|\.|          ? shift : undef;

$win64 = 0;
$win64 = 1 if ($flavour =~ /[nm]asm|mingw64/ || $output =~ /\.asm$/);

$avx512vaes = 0;

$0 =~ m/(.*[\/\\])[^\/\\]+$/;
$dir = $1;
($xlate = "${dir}x86_64-xlate.pl" and -f $xlate)
  or ($xlate = "${dir}../../perlasm/x86_64-xlate.pl" and -f $xlate)
  or die "can't locate x86_64-xlate.pl";

if (`$ENV{CC} -Wa,-v -c -o /dev/null -x assembler /dev/null 2>&1` =~ /GNU assembler version ([2-9]\.[0-9]+)/) {
  $avx512vaes = ($1 >= 2.30);
}

if (!$avx512vaes
  && $win64
  && ($flavour =~ /nasm/ || $ENV{ASM} =~ /nasm/)
  && `nasm -v 2>&1` =~ /NASM version ([2-9]\.[0-9]+)(?:\.([0-9]+))?/)
{
  $avx512vaes = ($1 == 2.13 && $2 >= 3) + ($1 >= 2.14);
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

open OUT, "| \"$^X\" \"$xlate\" $flavour \"$output\""
  or die "can't call $xlate: $!";
*STDOUT = *OUT;

#======================================================================
if ($avx512vaes>0) { #<<<

$code .= <<___;
.extern OPENSSL_ia32cap_P
.globl  ossl_vaes_vpclmulqdq_capable
.type   ossl_vaes_vpclmulqdq_capable,\@abi-omnipotent
.align 32
ossl_vaes_vpclmulqdq_capable:
    mov OPENSSL_ia32cap_P+8(%rip), %rcx
    # avx512vpclmulqdq + avx512vaes + avx512vl + avx512bw + avx512dq + avx512f
    mov \$`1<<42|1<<41|1<<31|1<<30|1<<17|1<<16`,%rdx
    xor %eax,%eax
    and %rdx,%rcx
    cmp %rdx,%rcx
    cmove %rcx,%rax
    ret
.size   ossl_vaes_vpclmulqdq_capable, .-ossl_vaes_vpclmulqdq_capable
___

# ; Mapping key length -> AES rounds count
my %aes_rounds = (
  128 => 9,
  192 => 11,
  256 => 13);

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Code generation control switches
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

# ; ABI-aware zeroing of volatile registers in EPILOG().
# ; Disabled due to performance reasons.
my $CLEAR_SCRATCH_REGISTERS = 0;

# ; Zero HKeys storage from the stack if they are stored there
my $CLEAR_HKEYS_STORAGE_ON_EXIT = 1;

# ; Enable / disable check of function arguments for null pointer
# ; Currently disabled, as this check is handled outside.
my $CHECK_FUNCTION_ARGUMENTS = 0;

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Global constants
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

# AES block size in bytes
my $AES_BLOCK_SIZE = 16;

# Storage capacity in elements
my $HKEYS_STORAGE_CAPACITY = 48;
my $LOCAL_STORAGE_CAPACITY = 48;
my $HKEYS_CONTEXT_CAPACITY = 16;

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Stack frame definition
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

# (1) -> +64(Win)/+48(Lin)-byte space for pushed GPRs
# (2) -> +8-byte space for 16-byte alignment of XMM storage
# (3) -> Frame pointer (%RBP)
# (4) -> +160-byte XMM storage (Windows only, zero on Linux)
# (5) -> +48-byte space for 64-byte alignment of %RSP from p.8
# (6) -> +768-byte LOCAL storage (optional, can be omitted in some functions)
# (7) -> +768-byte HKEYS storage
# (8) -> Stack pointer (%RSP) aligned on 64-byte boundary

my $GP_STORAGE  = $win64 ? 8 * 8     : 8 * 6;    # ; space for saved non-volatile GP registers (pushed on stack)
my $XMM_STORAGE = $win64 ? (10 * 16) : 0;        # ; space for saved XMM registers
my $HKEYS_STORAGE = ($HKEYS_STORAGE_CAPACITY * $AES_BLOCK_SIZE);    # ; space for HKeys^i, i=1..48
my $LOCAL_STORAGE = ($LOCAL_STORAGE_CAPACITY * $AES_BLOCK_SIZE);    # ; space for up to 48 AES blocks

my $STACK_HKEYS_OFFSET = 0;
my $STACK_LOCAL_OFFSET = ($STACK_HKEYS_OFFSET + $HKEYS_STORAGE);

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Function arguments abstraction
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
my ($arg1, $arg2, $arg3, $arg4, $arg5, $arg6, $arg7, $arg8, $arg9, $arg10, $arg11);

# ; Counter used for assembly label generation
my $label_count = 0;

# ; This implementation follows the convention: for non-leaf functions (they
# ; must call PROLOG) %rbp is used as a frame pointer, and has fixed offset from
# ; the function entry: $GP_STORAGE + [8 bytes alignment (Windows only)].  This
# ; helps to facilitate SEH handlers writing.
#
# ; Leaf functions here do not use more than 4 input arguments.
if ($win64) {
  $arg1  = "%rcx";
  $arg2  = "%rdx";
  $arg3  = "%r8";
  $arg4  = "%r9";
  $arg5  = "`$GP_STORAGE + 8 + 8*5`(%rbp)";    # +8 - alignment bytes
  $arg6  = "`$GP_STORAGE + 8 + 8*6`(%rbp)";
  $arg7  = "`$GP_STORAGE + 8 + 8*7`(%rbp)";
  $arg8  = "`$GP_STORAGE + 8 + 8*8`(%rbp)";
  $arg9  = "`$GP_STORAGE + 8 + 8*9`(%rbp)";
  $arg10 = "`$GP_STORAGE + 8 + 8*10`(%rbp)";
  $arg11 = "`$GP_STORAGE + 8 + 8*11`(%rbp)";
} else {
  $arg1  = "%rdi";
  $arg2  = "%rsi";
  $arg3  = "%rdx";
  $arg4  = "%rcx";
  $arg5  = "%r8";
  $arg6  = "%r9";
  $arg7  = "`$GP_STORAGE + 8*1`(%rbp)";
  $arg8  = "`$GP_STORAGE + 8*2`(%rbp)";
  $arg9  = "`$GP_STORAGE + 8*3`(%rbp)";
  $arg10 = "`$GP_STORAGE + 8*4`(%rbp)";
  $arg11 = "`$GP_STORAGE + 8*5`(%rbp)";
}

# ; Offsets in gcm128_context structure (see include/crypto/modes.h)
my $CTX_OFFSET_CurCount  = (16 * 0);          #  ; (Yi) Current counter for generation of encryption key
my $CTX_OFFSET_PEncBlock = (16 * 1);          #  ; (repurposed EKi field) Partial block buffer
my $CTX_OFFSET_EK0       = (16 * 2);          #  ; (EK0) Encrypted Y0 counter (see gcm spec notation)
my $CTX_OFFSET_AadLen    = (16 * 3);          #  ; (len.u[0]) Length of Hash which has been input
my $CTX_OFFSET_InLen     = ((16 * 3) + 8);    #  ; (len.u[1]) Length of input data which will be encrypted or decrypted
my $CTX_OFFSET_AadHash   = (16 * 4);          #  ; (Xi) Current hash
my $CTX_OFFSET_HTable    = (16 * 6);          #  ; (Htable) Precomputed table (allows 16 values)

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Helper functions
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

sub BYTE {
  my ($reg) = @_;
  if ($reg =~ /%r[abcd]x/i) {
    $reg =~ s/%r([abcd])x/%${1}l/i;
  } elsif ($reg =~ /%r[sdb][ip]/i) {
    $reg =~ s/%r([sdb][ip])/%${1}l/i;
  } elsif ($reg =~ /%r[0-9]{1,2}/i) {
    $reg =~ s/%(r[0-9]{1,2})/%${1}b/i;
  } else {
    die "BYTE: unknown register: $reg\n";
  }
  return $reg;
}

sub WORD {
  my ($reg) = @_;
  if ($reg =~ /%r[abcdsdb][xip]/i) {
    $reg =~ s/%r([abcdsdb])([xip])/%${1}${2}/i;
  } elsif ($reg =~ /%r[0-9]{1,2}/) {
    $reg =~ s/%(r[0-9]{1,2})/%${1}w/i;
  } else {
    die "WORD: unknown register: $reg\n";
  }
  return $reg;
}

sub DWORD {
  my ($reg) = @_;
  if ($reg =~ /%r[abcdsdb][xip]/i) {
    $reg =~ s/%r([abcdsdb])([xip])/%e${1}${2}/i;
  } elsif ($reg =~ /%r[0-9]{1,2}/i) {
    $reg =~ s/%(r[0-9]{1,2})/%${1}d/i;
  } else {
    die "DWORD: unknown register: $reg\n";
  }
  return $reg;
}

sub XWORD {
  my ($reg) = @_;
  if ($reg =~ /%[xyz]mm/i) {
    $reg =~ s/%[xyz]mm/%xmm/i;
  } else {
    die "XWORD: unknown register: $reg\n";
  }
  return $reg;
}

sub YWORD {
  my ($reg) = @_;
  if ($reg =~ /%[xyz]mm/i) {
    $reg =~ s/%[xyz]mm/%ymm/i;
  } else {
    die "YWORD: unknown register: $reg\n";
  }
  return $reg;
}

sub ZWORD {
  my ($reg) = @_;
  if ($reg =~ /%[xyz]mm/i) {
    $reg =~ s/%[xyz]mm/%zmm/i;
  } else {
    die "ZWORD: unknown register: $reg\n";
  }
  return $reg;
}

# ; Helper function to construct effective address based on two kinds of
# ; offsets: numerical or located in the register
sub EffectiveAddress {
  my ($base, $offset, $displacement) = @_;
  $displacement = 0 if (!$displacement);

  if ($offset =~ /^\d+\z/) {    # numerical offset
    return "`$offset + $displacement`($base)";
  } else {                      # offset resides in register
    return "$displacement($base,$offset,1)";
  }
}

# ; Provides memory location of corresponding HashKey power
sub HashKeyByIdx {
  my ($idx, $base) = @_;
  my $base_str = ($base eq "%rsp") ? "frame" : "context";

  my $offset = &HashKeyOffsetByIdx($idx, $base_str);
  return "$offset($base)";
}

# ; Provides offset (in bytes) of corresponding HashKey power from the highest key in the storage
sub HashKeyOffsetByIdx {
  my ($idx, $base) = @_;
  die "HashKeyOffsetByIdx: base should be either 'frame' or 'context'; base = $base"
    if (($base ne "frame") && ($base ne "context"));

  my $offset_base;
  my $offset_idx;
  if ($base eq "frame") {    # frame storage
    die "HashKeyOffsetByIdx: idx out of bounds (1..48)! idx = $idx\n" if ($idx > $HKEYS_STORAGE_CAPACITY || $idx < 1);
    $offset_base = $STACK_HKEYS_OFFSET;
    $offset_idx  = ($AES_BLOCK_SIZE * ($HKEYS_STORAGE_CAPACITY - $idx));
  } else {                   # context storage
    die "HashKeyOffsetByIdx: idx out of bounds (1..16)! idx = $idx\n" if ($idx > $HKEYS_CONTEXT_CAPACITY || $idx < 1);
    $offset_base = $CTX_OFFSET_HTable;
    $offset_idx  = ($AES_BLOCK_SIZE * ($HKEYS_CONTEXT_CAPACITY - $idx));
  }
  return $offset_base + $offset_idx;
}

# ; Creates local frame and does back up of non-volatile registers.
# ; Holds stack unwinding directives.
sub PROLOG {
  my ($need_hkeys_stack_storage, $need_aes_stack_storage, $func_name) = @_;

  my $DYNAMIC_STACK_ALLOC_SIZE            = 0;
  my $DYNAMIC_STACK_ALLOC_ALIGNMENT_SPACE = $win64 ? 48 : 52;

  if ($need_hkeys_stack_storage) {
    $DYNAMIC_STACK_ALLOC_SIZE += $HKEYS_STORAGE;
  }

  if ($need_aes_stack_storage) {
    if (!$need_hkeys_stack_storage) {
      die "PROLOG: unsupported case - aes storage without hkeys one";
    }
    $DYNAMIC_STACK_ALLOC_SIZE += $LOCAL_STORAGE;
  }

  $code .= <<___;
    push    %rbx
.cfi_push   %rbx
.L${func_name}_seh_push_rbx:
    push    %rbp
.cfi_push   %rbp
.L${func_name}_seh_push_rbp:
    push    %r12
.cfi_push   %r12
.L${func_name}_seh_push_r12:
    push    %r13
.cfi_push   %r13
.L${func_name}_seh_push_r13:
    push    %r14
.cfi_push   %r14
.L${func_name}_seh_push_r14:
    push    %r15
.cfi_push   %r15
.L${func_name}_seh_push_r15:
___

  if ($win64) {
    $code .= <<___;
    push    %rdi
.L${func_name}_seh_push_rdi:
    push    %rsi
.L${func_name}_seh_push_rsi:

    sub     \$`$XMM_STORAGE+8`,%rsp   # +8 alignment
.L${func_name}_seh_allocstack_xmm:
___
  }
  $code .= <<___;
    # ; %rbp contains stack pointer right after GP regs pushed at stack + [8
    # ; bytes of alignment (Windows only)].  It serves as a frame pointer in SEH
    # ; handlers. The requirement for a frame pointer is that its offset from
    # ; RSP shall be multiple of 16, and not exceed 240 bytes. The frame pointer
    # ; itself seems to be reasonable to use here, because later we do 64-byte stack
    # ; alignment which gives us non-determinate offsets and complicates writing
    # ; SEH handlers.
    #
    # ; It also serves as an anchor for retrieving stack arguments on both Linux
    # ; and Windows.
    lea     `$XMM_STORAGE`(%rsp),%rbp
.cfi_def_cfa_register %rbp
.L${func_name}_seh_setfp:
___
  if ($win64) {

    # ; xmm6:xmm15 need to be preserved on Windows
    foreach my $reg_idx (6 .. 15) {
      my $xmm_reg_offset = ($reg_idx - 6) * 16;
      $code .= <<___;
        vmovdqu           %xmm${reg_idx},$xmm_reg_offset(%rsp)
.L${func_name}_seh_save_xmm${reg_idx}:
___
    }
  }

  $code .= <<___;
# Prolog ends here. Next stack allocation is treated as "dynamic".
.L${func_name}_seh_prolog_end:
___

  if ($DYNAMIC_STACK_ALLOC_SIZE) {
    $code .= <<___;
        sub               \$`$DYNAMIC_STACK_ALLOC_SIZE + $DYNAMIC_STACK_ALLOC_ALIGNMENT_SPACE`,%rsp
        and               \$(-64),%rsp
___
  }
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Restore register content for the caller.
# ;;; And cleanup stack.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub EPILOG {
  my ($hkeys_storage_on_stack, $payload_len) = @_;

  my $label_suffix = $label_count++;

  if ($hkeys_storage_on_stack && $CLEAR_HKEYS_STORAGE_ON_EXIT) {

    # ; There is no need in hkeys cleanup if payload len was small, i.e. no hkeys
    # ; were stored in the local frame storage
    $code .= <<___;
        cmpq              \$`16*16`,$payload_len
        jbe               .Lskip_hkeys_cleanup_${label_suffix}
        vpxor             %xmm0,%xmm0,%xmm0
___
    for (my $i = 0; $i < int($HKEYS_STORAGE / 64); $i++) {
      $code .= "vmovdqa64         %zmm0,`$STACK_HKEYS_OFFSET + 64*$i`(%rsp)\n";
    }
    $code .= ".Lskip_hkeys_cleanup_${label_suffix}:\n";
  }

  if ($CLEAR_SCRATCH_REGISTERS) {
    &clear_scratch_gps_asm();
    &clear_scratch_zmms_asm();
  } else {
    $code .= "vzeroupper\n";
  }

  if ($win64) {

    # ; restore xmm15:xmm6
    for (my $reg_idx = 15; $reg_idx >= 6; $reg_idx--) {
      my $xmm_reg_offset = -$XMM_STORAGE + ($reg_idx - 6) * 16;
      $code .= <<___;
        vmovdqu           $xmm_reg_offset(%rbp),%xmm${reg_idx},
___
    }
  }

  if ($win64) {

    # Forming valid epilog for SEH with use of frame pointer.
    # https://docs.microsoft.com/en-us/cpp/build/prolog-and-epilog?view=msvc-160#epilog-code
    $code .= "lea      8(%rbp),%rsp\n";
  } else {
    $code .= "lea      (%rbp),%rsp\n";
    $code .= ".cfi_def_cfa_register %rsp\n";
  }

  if ($win64) {
    $code .= <<___;
     pop     %rsi
.cfi_pop     %rsi
     pop     %rdi
.cfi_pop     %rdi
___
  }
  $code .= <<___;
     pop     %r15
.cfi_pop     %r15
     pop     %r14
.cfi_pop     %r14
     pop     %r13
.cfi_pop     %r13
     pop     %r12
.cfi_pop     %r12
     pop     %rbp
.cfi_pop     %rbp
     pop     %rbx
.cfi_pop     %rbx
___
}

# ; Clears all scratch ZMM registers
# ;
# ; It should be called before restoring the XMM registers
# ; for Windows (XMM6-XMM15).
# ;
sub clear_scratch_zmms_asm {

  # ; On Linux, all ZMM registers are scratch registers
  if (!$win64) {
    $code .= "vzeroall\n";
  } else {
    foreach my $i (0 .. 5) {
      $code .= "vpxorq  %xmm${i},%xmm${i},%xmm${i}\n";
    }
  }
  foreach my $i (16 .. 31) {
    $code .= "vpxorq  %xmm${i},%xmm${i},%xmm${i}\n";
  }
}

# Clears all scratch GP registers
sub clear_scratch_gps_asm {
  foreach my $reg ("%rax", "%rcx", "%rdx", "%r8", "%r9", "%r10", "%r11") {
    $code .= "xor $reg,$reg\n";
  }
  if (!$win64) {
    foreach my $reg ("%rsi", "%rdi") {
      $code .= "xor $reg,$reg\n";
    }
  }
}

sub precompute_hkeys_on_stack {
  my $GCM128_CTX  = $_[0];
  my $HKEYS_READY = $_[1];
  my $ZTMP0       = $_[2];
  my $ZTMP1       = $_[3];
  my $ZTMP2       = $_[4];
  my $ZTMP3       = $_[5];
  my $ZTMP4       = $_[6];
  my $ZTMP5       = $_[7];
  my $ZTMP6       = $_[8];
  my $HKEYS_RANGE = $_[9];    # ; "first16", "mid16", "all", "first32", "last32"

  die "precompute_hkeys_on_stack: Unexpected value of HKEYS_RANGE: $HKEYS_RANGE"
    if ($HKEYS_RANGE ne "first16"
    && $HKEYS_RANGE ne "mid16"
    && $HKEYS_RANGE ne "all"
    && $HKEYS_RANGE ne "first32"
    && $HKEYS_RANGE ne "last32");

  my $label_suffix = $label_count++;

  $code .= <<___;
        test              $HKEYS_READY,$HKEYS_READY
        jnz               .L_skip_hkeys_precomputation_${label_suffix}
___

  if ($HKEYS_RANGE eq "first16" || $HKEYS_RANGE eq "first32" || $HKEYS_RANGE eq "all") {

    # ; Fill the stack with the first 16 hkeys from the context
    $code .= <<___;
        # ; Move 16 hkeys from the context to stack
        vmovdqu64         @{[HashKeyByIdx(4,$GCM128_CTX)]},$ZTMP0
        vmovdqu64         $ZTMP0,@{[HashKeyByIdx(4,"%rsp")]}

        vmovdqu64         @{[HashKeyByIdx(8,$GCM128_CTX)]},$ZTMP1
        vmovdqu64         $ZTMP1,@{[HashKeyByIdx(8,"%rsp")]}

        # ; broadcast HashKey^8
        vshufi64x2        \$0x00,$ZTMP1,$ZTMP1,$ZTMP1

        vmovdqu64         @{[HashKeyByIdx(12,$GCM128_CTX)]},$ZTMP2
        vmovdqu64         $ZTMP2,@{[HashKeyByIdx(12,"%rsp")]}

        vmovdqu64         @{[HashKeyByIdx(16,$GCM128_CTX)]},$ZTMP3
        vmovdqu64         $ZTMP3,@{[HashKeyByIdx(16,"%rsp")]}
___
  }

  if ($HKEYS_RANGE eq "mid16" || $HKEYS_RANGE eq "last32") {
    $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx(8,"%rsp")]},$ZTMP1

        # ; broadcast HashKey^8
        vshufi64x2        \$0x00,$ZTMP1,$ZTMP1,$ZTMP1

        vmovdqu64         @{[HashKeyByIdx(12,"%rsp")]},$ZTMP2
        vmovdqu64         @{[HashKeyByIdx(16,"%rsp")]},$ZTMP3
___

  }

  if ($HKEYS_RANGE eq "mid16" || $HKEYS_RANGE eq "first32" || $HKEYS_RANGE eq "last32" || $HKEYS_RANGE eq "all") {

    # ; Precompute hkeys^i, i=17..32
    my $i = 20;
    foreach (1 .. int((32 - 16) / 8)) {

      # ;; compute HashKey^(4 + n), HashKey^(3 + n), ... HashKey^(1 + n)
      &GHASH_MUL($ZTMP2, $ZTMP1, $ZTMP4, $ZTMP5, $ZTMP6);
      $code .= "vmovdqu64         $ZTMP2,@{[HashKeyByIdx($i,\"%rsp\")]}\n";
      $i += 4;

      # ;; compute HashKey^(8 + n), HashKey^(7 + n), ... HashKey^(5 + n)
      &GHASH_MUL($ZTMP3, $ZTMP1, $ZTMP4, $ZTMP5, $ZTMP6);
      $code .= "vmovdqu64         $ZTMP3,@{[HashKeyByIdx($i,\"%rsp\")]}\n";
      $i += 4;
    }
  }

  if ($HKEYS_RANGE eq "last32" || $HKEYS_RANGE eq "all") {

    # ; Precompute hkeys^i, i=33..48 (HKEYS_STORAGE_CAPACITY = 48)
    my $i = 36;
    foreach (1 .. int((48 - 32) / 8)) {

      # ;; compute HashKey^(4 + n), HashKey^(3 + n), ... HashKey^(1 + n)
      &GHASH_MUL($ZTMP2, $ZTMP1, $ZTMP4, $ZTMP5, $ZTMP6);
      $code .= "vmovdqu64         $ZTMP2,@{[HashKeyByIdx($i,\"%rsp\")]}\n";
      $i += 4;

      # ;; compute HashKey^(8 + n), HashKey^(7 + n), ... HashKey^(5 + n)
      &GHASH_MUL($ZTMP3, $ZTMP1, $ZTMP4, $ZTMP5, $ZTMP6);
      $code .= "vmovdqu64         $ZTMP3,@{[HashKeyByIdx($i,\"%rsp\")]}\n";
      $i += 4;
    }
  }

  $code .= ".L_skip_hkeys_precomputation_${label_suffix}:\n";
}

# ;; =============================================================================
# ;; Generic macro to produce code that executes $OPCODE instruction
# ;; on selected number of AES blocks (16 bytes long ) between 0 and 16.
# ;; All three operands of the instruction come from registers.
# ;; Note: if 3 blocks are left at the end instruction is produced to operate all
# ;;       4 blocks (full width of ZMM)
sub ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16 {
  my $NUM_BLOCKS = $_[0];    # [in] numerical value, number of AES blocks (0 to 16)
  my $OPCODE     = $_[1];    # [in] instruction name
  my @DST;
  $DST[0] = $_[2];           # [out] destination ZMM register
  $DST[1] = $_[3];           # [out] destination ZMM register
  $DST[2] = $_[4];           # [out] destination ZMM register
  $DST[3] = $_[5];           # [out] destination ZMM register
  my @SRC1;
  $SRC1[0] = $_[6];          # [in] source 1 ZMM register
  $SRC1[1] = $_[7];          # [in] source 1 ZMM register
  $SRC1[2] = $_[8];          # [in] source 1 ZMM register
  $SRC1[3] = $_[9];          # [in] source 1 ZMM register
  my @SRC2;
  $SRC2[0] = $_[10];         # [in] source 2 ZMM register
  $SRC2[1] = $_[11];         # [in] source 2 ZMM register
  $SRC2[2] = $_[12];         # [in] source 2 ZMM register
  $SRC2[3] = $_[13];         # [in] source 2 ZMM register

  die "ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16: num_blocks is out of bounds = $NUM_BLOCKS\n"
    if ($NUM_BLOCKS > 16 || $NUM_BLOCKS < 0);

  my $reg_idx     = 0;
  my $blocks_left = $NUM_BLOCKS;

  foreach (1 .. ($NUM_BLOCKS / 4)) {
    $code .= "$OPCODE        $SRC2[$reg_idx],$SRC1[$reg_idx],$DST[$reg_idx]\n";
    $reg_idx++;
    $blocks_left -= 4;
  }

  my $DSTREG  = $DST[$reg_idx];
  my $SRC1REG = $SRC1[$reg_idx];
  my $SRC2REG = $SRC2[$reg_idx];

  if ($blocks_left == 1) {
    $code .= "$OPCODE         @{[XWORD($SRC2REG)]},@{[XWORD($SRC1REG)]},@{[XWORD($DSTREG)]}\n";
  } elsif ($blocks_left == 2) {
    $code .= "$OPCODE         @{[YWORD($SRC2REG)]},@{[YWORD($SRC1REG)]},@{[YWORD($DSTREG)]}\n";
  } elsif ($blocks_left == 3) {
    $code .= "$OPCODE         $SRC2REG,$SRC1REG,$DSTREG\n";
  }
}

# ;; =============================================================================
# ;; Loads specified number of AES blocks into ZMM registers using mask register
# ;; for the last loaded register (xmm, ymm or zmm).
# ;; Loads take place at 1 byte granularity.
sub ZMM_LOAD_MASKED_BLOCKS_0_16 {
  my $NUM_BLOCKS  = $_[0];    # [in] numerical value, number of AES blocks (0 to 16)
  my $INP         = $_[1];    # [in] input data pointer to read from
  my $DATA_OFFSET = $_[2];    # [in] offset to the output pointer (GP or numerical)
  my @DST;
  $DST[0] = $_[3];            # [out] ZMM register with loaded data
  $DST[1] = $_[4];            # [out] ZMM register with loaded data
  $DST[2] = $_[5];            # [out] ZMM register with loaded data
  $DST[3] = $_[6];            # [out] ZMM register with loaded data
  my $MASK = $_[7];           # [in] mask register

  die "ZMM_LOAD_MASKED_BLOCKS_0_16: num_blocks is out of bounds = $NUM_BLOCKS\n"
    if ($NUM_BLOCKS > 16 || $NUM_BLOCKS < 0);

  my $src_offset  = 0;
  my $dst_idx     = 0;
  my $blocks_left = $NUM_BLOCKS;

  if ($NUM_BLOCKS > 0) {
    foreach (1 .. (int(($NUM_BLOCKS + 3) / 4) - 1)) {
      $code .= "vmovdqu8          @{[EffectiveAddress($INP,$DATA_OFFSET,$src_offset)]},$DST[$dst_idx]\n";
      $src_offset += 64;
      $dst_idx++;
      $blocks_left -= 4;
    }
  }

  my $DSTREG = $DST[$dst_idx];

  if ($blocks_left == 1) {
    $code .= "vmovdqu8          @{[EffectiveAddress($INP,$DATA_OFFSET,$src_offset)]},@{[XWORD($DSTREG)]}\{$MASK\}{z}\n";
  } elsif ($blocks_left == 2) {
    $code .= "vmovdqu8          @{[EffectiveAddress($INP,$DATA_OFFSET,$src_offset)]},@{[YWORD($DSTREG)]}\{$MASK\}{z}\n";
  } elsif (($blocks_left == 3 || $blocks_left == 4)) {
    $code .= "vmovdqu8          @{[EffectiveAddress($INP,$DATA_OFFSET,$src_offset)]},$DSTREG\{$MASK\}{z}\n";
  }
}

# ;; =============================================================================
# ;; Stores specified number of AES blocks from ZMM registers with mask register
# ;; for the last loaded register (xmm, ymm or zmm).
# ;; Stores take place at 1 byte granularity.
sub ZMM_STORE_MASKED_BLOCKS_0_16 {
  my $NUM_BLOCKS  = $_[0];    # [in] numerical value, number of AES blocks (0 to 16)
  my $OUTP        = $_[1];    # [in] output data pointer to write to
  my $DATA_OFFSET = $_[2];    # [in] offset to the output pointer (GP or numerical)
  my @SRC;
  $SRC[0] = $_[3];            # [in] ZMM register with data to store
  $SRC[1] = $_[4];            # [in] ZMM register with data to store
  $SRC[2] = $_[5];            # [in] ZMM register with data to store
  $SRC[3] = $_[6];            # [in] ZMM register with data to store
  my $MASK = $_[7];           # [in] mask register

  die "ZMM_STORE_MASKED_BLOCKS_0_16: num_blocks is out of bounds = $NUM_BLOCKS\n"
    if ($NUM_BLOCKS > 16 || $NUM_BLOCKS < 0);

  my $dst_offset  = 0;
  my $src_idx     = 0;
  my $blocks_left = $NUM_BLOCKS;

  if ($NUM_BLOCKS > 0) {
    foreach (1 .. (int(($NUM_BLOCKS + 3) / 4) - 1)) {
      $code .= "vmovdqu8          $SRC[$src_idx],`$dst_offset`($OUTP,$DATA_OFFSET,1)\n";
      $dst_offset += 64;
      $src_idx++;
      $blocks_left -= 4;
    }
  }

  my $SRCREG = $SRC[$src_idx];

  if ($blocks_left == 1) {
    $code .= "vmovdqu8          @{[XWORD($SRCREG)]},`$dst_offset`($OUTP,$DATA_OFFSET,1){$MASK}\n";
  } elsif ($blocks_left == 2) {
    $code .= "vmovdqu8          @{[YWORD($SRCREG)]},`$dst_offset`($OUTP,$DATA_OFFSET,1){$MASK}\n";
  } elsif ($blocks_left == 3 || $blocks_left == 4) {
    $code .= "vmovdqu8          $SRCREG,`$dst_offset`($OUTP,$DATA_OFFSET,1){$MASK}\n";
  }
}

# ;;; ===========================================================================
# ;;; Handles AES encryption rounds
# ;;; It handles special cases: the last and first rounds
# ;;; Optionally, it performs XOR with data after the last AES round.
# ;;; Uses NROUNDS parameter to check what needs to be done for the current round.
# ;;; If 3 blocks are trailing then operation on whole ZMM is performed (4 blocks).
sub ZMM_AESENC_ROUND_BLOCKS_0_16 {
  my $L0B0_3   = $_[0];     # [in/out] zmm; blocks 0 to 3
  my $L0B4_7   = $_[1];     # [in/out] zmm; blocks 4 to 7
  my $L0B8_11  = $_[2];     # [in/out] zmm; blocks 8 to 11
  my $L0B12_15 = $_[3];     # [in/out] zmm; blocks 12 to 15
  my $KEY      = $_[4];     # [in] zmm containing round key
  my $ROUND    = $_[5];     # [in] round number
  my $D0_3     = $_[6];     # [in] zmm or no_data; plain/cipher text blocks 0-3
  my $D4_7     = $_[7];     # [in] zmm or no_data; plain/cipher text blocks 4-7
  my $D8_11    = $_[8];     # [in] zmm or no_data; plain/cipher text blocks 8-11
  my $D12_15   = $_[9];     # [in] zmm or no_data; plain/cipher text blocks 12-15
  my $NUMBL    = $_[10];    # [in] number of blocks; numerical value
  my $NROUNDS  = $_[11];    # [in] number of rounds; numerical value

  # ;;; === first AES round
  if ($ROUND < 1) {

    # ;;  round 0
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUMBL,  "vpxorq", $L0B0_3,   $L0B4_7, $L0B8_11, $L0B12_15, $L0B0_3,
      $L0B4_7, $L0B8_11, $L0B12_15, $KEY,    $KEY,     $KEY,      $KEY);
  }

  # ;;; === middle AES rounds
  if ($ROUND >= 1 && $ROUND <= $NROUNDS) {

    # ;; rounds 1 to 9/11/13
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUMBL,  "vaesenc", $L0B0_3,   $L0B4_7, $L0B8_11, $L0B12_15, $L0B0_3,
      $L0B4_7, $L0B8_11,  $L0B12_15, $KEY,    $KEY,     $KEY,      $KEY);
  }

  # ;;; === last AES round
  if ($ROUND > $NROUNDS) {

    # ;; the last round - mix enclast with text xor's
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUMBL,  "vaesenclast", $L0B0_3,   $L0B4_7, $L0B8_11, $L0B12_15, $L0B0_3,
      $L0B4_7, $L0B8_11,      $L0B12_15, $KEY,    $KEY,     $KEY,      $KEY);

    # ;;; === XOR with data
    if ( ($D0_3 ne "no_data")
      && ($D4_7 ne "no_data")
      && ($D8_11 ne "no_data")
      && ($D12_15 ne "no_data"))
    {
      &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
        $NUMBL,  "vpxorq", $L0B0_3,   $L0B4_7, $L0B8_11, $L0B12_15, $L0B0_3,
        $L0B4_7, $L0B8_11, $L0B12_15, $D0_3,   $D4_7,    $D8_11,    $D12_15);
    }
  }
}

# ;;; Horizontal XOR - 4 x 128bits xored together
sub VHPXORI4x128 {
  my $REG = $_[0];    # [in/out] ZMM with 4x128bits to xor; 128bit output
  my $TMP = $_[1];    # [clobbered] ZMM temporary register
  $code .= <<___;
        vextracti64x4     \$1,$REG,@{[YWORD($TMP)]}
        vpxorq            @{[YWORD($TMP)]},@{[YWORD($REG)]},@{[YWORD($REG)]}
        vextracti32x4     \$1,@{[YWORD($REG)]},@{[XWORD($TMP)]}
        vpxorq            @{[XWORD($TMP)]},@{[XWORD($REG)]},@{[XWORD($REG)]}
___
}

# ;;; AVX512 reduction macro
sub VCLMUL_REDUCE {
  my $OUT   = $_[0];    # [out] zmm/ymm/xmm: result (must not be $TMP1 or $HI128)
  my $POLY  = $_[1];    # [in] zmm/ymm/xmm: polynomial
  my $HI128 = $_[2];    # [in] zmm/ymm/xmm: high 128b of hash to reduce
  my $LO128 = $_[3];    # [in] zmm/ymm/xmm: low 128b of hash to reduce
  my $TMP0  = $_[4];    # [in] zmm/ymm/xmm: temporary register
  my $TMP1  = $_[5];    # [in] zmm/ymm/xmm: temporary register

  $code .= <<___;
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; first phase of the reduction
        vpclmulqdq        \$0x01,$LO128,$POLY,$TMP0
        vpslldq           \$8,$TMP0,$TMP0         # ; shift-L 2 DWs
        vpxorq            $TMP0,$LO128,$TMP0      # ; first phase of the reduction complete
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; second phase of the reduction
        vpclmulqdq        \$0x00,$TMP0,$POLY,$TMP1
        vpsrldq           \$4,$TMP1,$TMP1          # ; shift-R only 1-DW to obtain 2-DWs shift-R
        vpclmulqdq        \$0x10,$TMP0,$POLY,$OUT
        vpslldq           \$4,$OUT,$OUT            # ; shift-L 1-DW to obtain result with no shifts
        vpternlogq        \$0x96,$HI128,$TMP1,$OUT # ; OUT/GHASH = OUT xor TMP1 xor HI128
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
___
}

# ;; ===========================================================================
# ;; schoolbook multiply of 16 blocks (16 x 16 bytes)
# ;; - it is assumed that data read from $INPTR is already shuffled and
# ;;   $INPTR address is 64 byte aligned
# ;; - there is an option to pass ready blocks through ZMM registers too.
# ;;   4 extra parameters need to be passed in such case and 21st ($ZTMP9) argument can be empty
sub GHASH_16 {
  my $TYPE  = $_[0];     # [in] ghash type: start (xor hash), mid, end (same as mid; no reduction),
                         # end_reduce (end with reduction), start_reduce
  my $GH    = $_[1];     # [in/out] ZMM ghash sum: high 128-bits
  my $GM    = $_[2];     # [in/out] ZMM ghash sum: middle 128-bits
  my $GL    = $_[3];     # [in/out] ZMM ghash sum: low 128-bits
  my $INPTR = $_[4];     # [in] data input pointer
  my $INOFF = $_[5];     # [in] data input offset
  my $INDIS = $_[6];     # [in] data input displacement
  my $HKPTR = $_[7];     # [in] hash key pointer
  my $HKOFF = $_[8];     # [in] hash key offset (can be either numerical offset, or register containing offset)
  my $HKDIS = $_[9];     # [in] hash key displacement
  my $HASH  = $_[10];    # [in/out] ZMM hash value in/out
  my $ZTMP0 = $_[11];    # [clobbered] temporary ZMM
  my $ZTMP1 = $_[12];    # [clobbered] temporary ZMM
  my $ZTMP2 = $_[13];    # [clobbered] temporary ZMM
  my $ZTMP3 = $_[14];    # [clobbered] temporary ZMM
  my $ZTMP4 = $_[15];    # [clobbered] temporary ZMM
  my $ZTMP5 = $_[16];    # [clobbered] temporary ZMM
  my $ZTMP6 = $_[17];    # [clobbered] temporary ZMM
  my $ZTMP7 = $_[18];    # [clobbered] temporary ZMM
  my $ZTMP8 = $_[19];    # [clobbered] temporary ZMM
  my $ZTMP9 = $_[20];    # [clobbered] temporary ZMM, can be empty if 4 extra parameters below are provided
  my $DAT0  = $_[21];    # [in] ZMM with 4 blocks of input data (INPTR, INOFF, INDIS unused)
  my $DAT1  = $_[22];    # [in] ZMM with 4 blocks of input data (INPTR, INOFF, INDIS unused)
  my $DAT2  = $_[23];    # [in] ZMM with 4 blocks of input data (INPTR, INOFF, INDIS unused)
  my $DAT3  = $_[24];    # [in] ZMM with 4 blocks of input data (INPTR, INOFF, INDIS unused)

  my $start_ghash  = 0;
  my $do_reduction = 0;
  if ($TYPE eq "start") {
    $start_ghash = 1;
  }

  if ($TYPE eq "start_reduce") {
    $start_ghash  = 1;
    $do_reduction = 1;
  }

  if ($TYPE eq "end_reduce") {
    $do_reduction = 1;
  }

  # ;; ghash blocks 0-3
  if (scalar(@_) == 21) {
    $code .= "vmovdqa64         @{[EffectiveAddress($INPTR,$INOFF,($INDIS+0*64))]},$ZTMP9\n";
  } else {
    $ZTMP9 = $DAT0;
  }

  if ($start_ghash != 0) {
    $code .= "vpxorq            $HASH,$ZTMP9,$ZTMP9\n";
  }
  $code .= <<___;
        vmovdqu64         @{[EffectiveAddress($HKPTR,$HKOFF,($HKDIS+0*64))]},$ZTMP8
        vpclmulqdq        \$0x11,$ZTMP8,$ZTMP9,$ZTMP0      # ; T0H = a1*b1
        vpclmulqdq        \$0x00,$ZTMP8,$ZTMP9,$ZTMP1      # ; T0L = a0*b0
        vpclmulqdq        \$0x01,$ZTMP8,$ZTMP9,$ZTMP2      # ; T0M1 = a1*b0
        vpclmulqdq        \$0x10,$ZTMP8,$ZTMP9,$ZTMP3      # ; T0M2 = a0*b1
___

  # ;; ghash blocks 4-7
  if (scalar(@_) == 21) {
    $code .= "vmovdqa64         @{[EffectiveAddress($INPTR,$INOFF,($INDIS+1*64))]},$ZTMP9\n";
  } else {
    $ZTMP9 = $DAT1;
  }
  $code .= <<___;
        vmovdqu64         @{[EffectiveAddress($HKPTR,$HKOFF,($HKDIS+1*64))]},$ZTMP8
        vpclmulqdq        \$0x11,$ZTMP8,$ZTMP9,$ZTMP4      # ; T1H = a1*b1
        vpclmulqdq        \$0x00,$ZTMP8,$ZTMP9,$ZTMP5      # ; T1L = a0*b0
        vpclmulqdq        \$0x01,$ZTMP8,$ZTMP9,$ZTMP6      # ; T1M1 = a1*b0
        vpclmulqdq        \$0x10,$ZTMP8,$ZTMP9,$ZTMP7      # ; T1M2 = a0*b1
___

  # ;; update sums
  if ($start_ghash != 0) {
    $code .= <<___;
        vpxorq            $ZTMP6,$ZTMP2,$GM             # ; GM = T0M1 + T1M1
        vpxorq            $ZTMP4,$ZTMP0,$GH             # ; GH = T0H + T1H
        vpxorq            $ZTMP5,$ZTMP1,$GL             # ; GL = T0L + T1L
        vpternlogq        \$0x96,$ZTMP7,$ZTMP3,$GM      # ; GM = T0M2 + T1M1
___
  } else {    # ;; mid, end, end_reduce
    $code .= <<___;
        vpternlogq        \$0x96,$ZTMP6,$ZTMP2,$GM      # ; GM += T0M1 + T1M1
        vpternlogq        \$0x96,$ZTMP4,$ZTMP0,$GH      # ; GH += T0H + T1H
        vpternlogq        \$0x96,$ZTMP5,$ZTMP1,$GL      # ; GL += T0L + T1L
        vpternlogq        \$0x96,$ZTMP7,$ZTMP3,$GM      # ; GM += T0M2 + T1M1
___
  }

  # ;; ghash blocks 8-11
  if (scalar(@_) == 21) {
    $code .= "vmovdqa64         @{[EffectiveAddress($INPTR,$INOFF,($INDIS+2*64))]},$ZTMP9\n";
  } else {
    $ZTMP9 = $DAT2;
  }
  $code .= <<___;
        vmovdqu64         @{[EffectiveAddress($HKPTR,$HKOFF,($HKDIS+2*64))]},$ZTMP8
        vpclmulqdq        \$0x11,$ZTMP8,$ZTMP9,$ZTMP0      # ; T0H = a1*b1
        vpclmulqdq        \$0x00,$ZTMP8,$ZTMP9,$ZTMP1      # ; T0L = a0*b0
        vpclmulqdq        \$0x01,$ZTMP8,$ZTMP9,$ZTMP2      # ; T0M1 = a1*b0
        vpclmulqdq        \$0x10,$ZTMP8,$ZTMP9,$ZTMP3      # ; T0M2 = a0*b1
___

  # ;; ghash blocks 12-15
  if (scalar(@_) == 21) {
    $code .= "vmovdqa64         @{[EffectiveAddress($INPTR,$INOFF,($INDIS+3*64))]},$ZTMP9\n";
  } else {
    $ZTMP9 = $DAT3;
  }
  $code .= <<___;
        vmovdqu64         @{[EffectiveAddress($HKPTR,$HKOFF,($HKDIS+3*64))]},$ZTMP8
        vpclmulqdq        \$0x11,$ZTMP8,$ZTMP9,$ZTMP4      # ; T1H = a1*b1
        vpclmulqdq        \$0x00,$ZTMP8,$ZTMP9,$ZTMP5      # ; T1L = a0*b0
        vpclmulqdq        \$0x01,$ZTMP8,$ZTMP9,$ZTMP6      # ; T1M1 = a1*b0
        vpclmulqdq        \$0x10,$ZTMP8,$ZTMP9,$ZTMP7      # ; T1M2 = a0*b1
        # ;; update sums
        vpternlogq        \$0x96,$ZTMP6,$ZTMP2,$GM         # ; GM += T0M1 + T1M1
        vpternlogq        \$0x96,$ZTMP4,$ZTMP0,$GH         # ; GH += T0H + T1H
        vpternlogq        \$0x96,$ZTMP5,$ZTMP1,$GL         # ; GL += T0L + T1L
        vpternlogq        \$0x96,$ZTMP7,$ZTMP3,$GM         # ; GM += T0M2 + T1M1
___
  if ($do_reduction != 0) {
    $code .= <<___;
        # ;; integrate GM into GH and GL
        vpsrldq           \$8,$GM,$ZTMP0
        vpslldq           \$8,$GM,$ZTMP1
        vpxorq            $ZTMP0,$GH,$GH
        vpxorq            $ZTMP1,$GL,$GL
___

    # ;; add GH and GL 128-bit words horizontally
    &VHPXORI4x128($GH, $ZTMP0);
    &VHPXORI4x128($GL, $ZTMP1);

    # ;; reduction
    $code .= "vmovdqa64         POLY2(%rip),@{[XWORD($ZTMP2)]}\n";
    &VCLMUL_REDUCE(&XWORD($HASH), &XWORD($ZTMP2), &XWORD($GH), &XWORD($GL), &XWORD($ZTMP0), &XWORD($ZTMP1));
  }
}

# ;; ===========================================================================
# ;; GHASH 1 to 16 blocks of cipher text
# ;; - performs reduction at the end
# ;; - it doesn't load the data and it assumed it is already loaded and shuffled
sub GHASH_1_TO_16 {
  my $GCM128_CTX  = $_[0];     # [in] pointer to expanded keys
  my $GHASH       = $_[1];     # [out] ghash output
  my $T0H         = $_[2];     # [clobbered] temporary ZMM
  my $T0L         = $_[3];     # [clobbered] temporary ZMM
  my $T0M1        = $_[4];     # [clobbered] temporary ZMM
  my $T0M2        = $_[5];     # [clobbered] temporary ZMM
  my $T1H         = $_[6];     # [clobbered] temporary ZMM
  my $T1L         = $_[7];     # [clobbered] temporary ZMM
  my $T1M1        = $_[8];     # [clobbered] temporary ZMM
  my $T1M2        = $_[9];     # [clobbered] temporary ZMM
  my $HK          = $_[10];    # [clobbered] temporary ZMM
  my $AAD_HASH_IN = $_[11];    # [in] input hash value
  my @CIPHER_IN;
  $CIPHER_IN[0] = $_[12];      # [in] ZMM with cipher text blocks 0-3
  $CIPHER_IN[1] = $_[13];      # [in] ZMM with cipher text blocks 4-7
  $CIPHER_IN[2] = $_[14];      # [in] ZMM with cipher text blocks 8-11
  $CIPHER_IN[3] = $_[15];      # [in] ZMM with cipher text blocks 12-15
  my $NUM_BLOCKS = $_[16];     # [in] numerical value, number of blocks
  my $GH         = $_[17];     # [in] ZMM with hi product part
  my $GM         = $_[18];     # [in] ZMM with mid product part
  my $GL         = $_[19];     # [in] ZMM with lo product part

  die "GHASH_1_TO_16: num_blocks is out of bounds = $NUM_BLOCKS\n" if ($NUM_BLOCKS > 16 || $NUM_BLOCKS < 0);

  if (scalar(@_) == 17) {
    $code .= "vpxorq            $AAD_HASH_IN,$CIPHER_IN[0],$CIPHER_IN[0]\n";
  }

  if ($NUM_BLOCKS == 16) {
    $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[0],$T0H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[0],$T0L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[0],$T0M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[0],$T0M2       # ; M2 = a0*b1
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-1*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[1],$T1H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[1],$T1L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[1],$T1M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[1],$T1M2       # ; M2 = a0*b1
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-2*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[2],$CIPHER_IN[0] # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[2],$CIPHER_IN[1] # ; L = a0*b0
        vpternlogq        \$0x96,$T1H,$CIPHER_IN[0],$T0H
        vpternlogq        \$0x96,$T1L,$CIPHER_IN[1],$T0L
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[2],$CIPHER_IN[0] # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[2],$CIPHER_IN[1] # ; M2 = a0*b1
        vpternlogq        \$0x96,$T1M1,$CIPHER_IN[0],$T0M1
        vpternlogq        \$0x96,$T1M2,$CIPHER_IN[1],$T0M2
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-3*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[3],$T1H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[3],$T1L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[3],$T1M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[3],$T1M2       # ; M2 = a0*b1
        vpxorq            $T1H,$T0H,$T1H
        vpxorq            $T1L,$T0L,$T1L
        vpxorq            $T1M1,$T0M1,$T1M1
        vpxorq            $T1M2,$T0M2,$T1M2
___
  } elsif ($NUM_BLOCKS >= 12) {
    $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[0],$T0H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[0],$T0L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[0],$T0M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[0],$T0M2       # ; M2 = a0*b1
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-1*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[1],$T1H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[1],$T1L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[1],$T1M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[1],$T1M2       # ; M2 = a0*b1
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-2*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[2],$CIPHER_IN[0] # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[2],$CIPHER_IN[1] # ; L = a0*b0
        vpternlogq        \$0x96,$T0H,$CIPHER_IN[0],$T1H
        vpternlogq        \$0x96,$T0L,$CIPHER_IN[1],$T1L
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[2],$CIPHER_IN[0] # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[2],$CIPHER_IN[1] # ; M2 = a0*b1
        vpternlogq        \$0x96,$T0M1,$CIPHER_IN[0],$T1M1
        vpternlogq        \$0x96,$T0M2,$CIPHER_IN[1],$T1M2
___
  } elsif ($NUM_BLOCKS >= 8) {
    $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[0],$T0H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[0],$T0L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[0],$T0M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[0],$T0M2       # ; M2 = a0*b1
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS-1*4, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[1],$T1H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[1],$T1L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[1],$T1M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[1],$T1M2       # ; M2 = a0*b1
        vpxorq            $T1H,$T0H,$T1H
        vpxorq            $T1L,$T0L,$T1L
        vpxorq            $T1M1,$T0M1,$T1M1
        vpxorq            $T1M2,$T0M2,$T1M2
___
  } elsif ($NUM_BLOCKS >= 4) {
    $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($NUM_BLOCKS, $GCM128_CTX)]},$HK
        vpclmulqdq        \$0x11,$HK,$CIPHER_IN[0],$T1H        # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$CIPHER_IN[0],$T1L        # ; L = a0*b0
        vpclmulqdq        \$0x01,$HK,$CIPHER_IN[0],$T1M1       # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$CIPHER_IN[0],$T1M2       # ; M2 = a0*b1
___
  }

  # ;; T1H/L/M1/M2 - hold current product sums (provided $NUM_BLOCKS >= 4)
  my $blocks_left = ($NUM_BLOCKS % 4);
  if ($blocks_left > 0) {

    # ;; =====================================================
    # ;; There are 1, 2 or 3 blocks left to process.
    # ;; It may also be that they are the only blocks to process.

    # ;; Set hash key and register index position for the remaining 1 to 3 blocks
    my $reg_idx = ($NUM_BLOCKS / 4);
    my $REG_IN  = $CIPHER_IN[$reg_idx];

    if ($blocks_left == 1) {
      $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($blocks_left, $GCM128_CTX)]},@{[XWORD($HK)]}
        vpclmulqdq        \$0x01,@{[XWORD($HK)]},@{[XWORD($REG_IN)]},@{[XWORD($T0M1)]} # ; M1 = a1*b0
        vpclmulqdq        \$0x10,@{[XWORD($HK)]},@{[XWORD($REG_IN)]},@{[XWORD($T0M2)]} # ; M2 = a0*b1
        vpclmulqdq        \$0x11,@{[XWORD($HK)]},@{[XWORD($REG_IN)]},@{[XWORD($T0H)]}  # ; H = a1*b1
        vpclmulqdq        \$0x00,@{[XWORD($HK)]},@{[XWORD($REG_IN)]},@{[XWORD($T0L)]}  # ; L = a0*b0
___
    } elsif ($blocks_left == 2) {
      $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($blocks_left, $GCM128_CTX)]},@{[YWORD($HK)]}
        vpclmulqdq        \$0x01,@{[YWORD($HK)]},@{[YWORD($REG_IN)]},@{[YWORD($T0M1)]} # ; M1 = a1*b0
        vpclmulqdq        \$0x10,@{[YWORD($HK)]},@{[YWORD($REG_IN)]},@{[YWORD($T0M2)]} # ; M2 = a0*b1
        vpclmulqdq        \$0x11,@{[YWORD($HK)]},@{[YWORD($REG_IN)]},@{[YWORD($T0H)]}  # ; H = a1*b1
        vpclmulqdq        \$0x00,@{[YWORD($HK)]},@{[YWORD($REG_IN)]},@{[YWORD($T0L)]}  # ; L = a0*b0
___
    } else {    # ; blocks_left == 3
      $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx($blocks_left, $GCM128_CTX)]},@{[YWORD($HK)]}
        vinserti64x2      \$2,@{[HashKeyByIdx($blocks_left-2, $GCM128_CTX)]},$HK,$HK
        vpclmulqdq        \$0x01,$HK,$REG_IN,$T0M1                                     # ; M1 = a1*b0
        vpclmulqdq        \$0x10,$HK,$REG_IN,$T0M2                                     # ; M2 = a0*b1
        vpclmulqdq        \$0x11,$HK,$REG_IN,$T0H                                      # ; H = a1*b1
        vpclmulqdq        \$0x00,$HK,$REG_IN,$T0L                                      # ; L = a0*b0
___
    }

    if (scalar(@_) == 20) {

      # ;; *** GH/GM/GL passed as arguments
      if ($NUM_BLOCKS >= 4) {
        $code .= <<___;
        # ;; add ghash product sums from the first 4, 8 or 12 blocks
        vpxorq            $T1M1,$T0M1,$T0M1
        vpternlogq        \$0x96,$T1M2,$GM,$T0M2
        vpternlogq        \$0x96,$T1H,$GH,$T0H
        vpternlogq        \$0x96,$T1L,$GL,$T0L
___
      } else {
        $code .= <<___;
        vpxorq            $GM,$T0M1,$T0M1
        vpxorq            $GH,$T0H,$T0H
        vpxorq            $GL,$T0L,$T0L
___
      }
    } else {

      # ;; *** GH/GM/GL NOT passed as arguments
      if ($NUM_BLOCKS >= 4) {
        $code .= <<___;
        # ;; add ghash product sums from the first 4, 8 or 12 blocks
        vpxorq            $T1M1,$T0M1,$T0M1
        vpxorq            $T1M2,$T0M2,$T0M2
        vpxorq            $T1H,$T0H,$T0H
        vpxorq            $T1L,$T0L,$T0L
___
      }
    }
    $code .= <<___;
        # ;; integrate TM into TH and TL
        vpxorq            $T0M2,$T0M1,$T0M1
        vpsrldq           \$8,$T0M1,$T1M1
        vpslldq           \$8,$T0M1,$T1M2
        vpxorq            $T1M1,$T0H,$T0H
        vpxorq            $T1M2,$T0L,$T0L
___
  } else {

    # ;; =====================================================
    # ;; number of blocks is 4, 8, 12 or 16
    # ;; T1H/L/M1/M2 include product sums not T0H/L/M1/M2
    if (scalar(@_) == 20) {
      $code .= <<___;
        # ;; *** GH/GM/GL passed as arguments
        vpxorq            $GM,$T1M1,$T1M1
        vpxorq            $GH,$T1H,$T1H
        vpxorq            $GL,$T1L,$T1L
___
    }
    $code .= <<___;
        # ;; integrate TM into TH and TL
        vpxorq            $T1M2,$T1M1,$T1M1
        vpsrldq           \$8,$T1M1,$T0M1
        vpslldq           \$8,$T1M1,$T0M2
        vpxorq            $T0M1,$T1H,$T0H
        vpxorq            $T0M2,$T1L,$T0L
___
  }

  # ;; add TH and TL 128-bit words horizontally
  &VHPXORI4x128($T0H, $T1M1);
  &VHPXORI4x128($T0L, $T1M2);

  # ;; reduction
  $code .= "vmovdqa64         POLY2(%rip),@{[XWORD($HK)]}\n";
  &VCLMUL_REDUCE(
    @{[XWORD($GHASH)]},
    @{[XWORD($HK)]},
    @{[XWORD($T0H)]},
    @{[XWORD($T0L)]},
    @{[XWORD($T0M1)]},
    @{[XWORD($T0M2)]});
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; GHASH_MUL MACRO to implement: Data*HashKey mod (x^128 + x^127 + x^126 +x^121 + 1)
# ;; Input: A and B (128-bits each, bit-reflected)
# ;; Output: C = A*B*x mod poly, (i.e. >>1 )
# ;; To compute GH = GH*HashKey mod poly, give HK = HashKey<<1 mod poly as input
# ;; GH = GH * HK * x mod poly which is equivalent to GH*HashKey mod poly.
# ;;
# ;; Refer to [3] for more details.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub GHASH_MUL {
  my $GH = $_[0];    #; [in/out] xmm/ymm/zmm with multiply operand(s) (128-bits)
  my $HK = $_[1];    #; [in] xmm/ymm/zmm with hash key value(s) (128-bits)
  my $T1 = $_[2];    #; [clobbered] xmm/ymm/zmm
  my $T2 = $_[3];    #; [clobbered] xmm/ymm/zmm
  my $T3 = $_[4];    #; [clobbered] xmm/ymm/zmm

  $code .= <<___;
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        vpclmulqdq        \$0x11,$HK,$GH,$T1 # ; $T1 = a1*b1
        vpclmulqdq        \$0x00,$HK,$GH,$T2 # ; $T2 = a0*b0
        vpclmulqdq        \$0x01,$HK,$GH,$T3 # ; $T3 = a1*b0
        vpclmulqdq        \$0x10,$HK,$GH,$GH # ; $GH = a0*b1
        vpxorq            $T3,$GH,$GH

        vpsrldq           \$8,$GH,$T3        # ; shift-R $GH 2 DWs
        vpslldq           \$8,$GH,$GH        # ; shift-L $GH 2 DWs
        vpxorq            $T3,$T1,$T1
        vpxorq            $T2,$GH,$GH

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;first phase of the reduction
        vmovdqu64         POLY2(%rip),$T3

        vpclmulqdq        \$0x01,$GH,$T3,$T2
        vpslldq           \$8,$T2,$T2        # ; shift-L $T2 2 DWs
        vpxorq            $T2,$GH,$GH        # ; first phase of the reduction complete

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;second phase of the reduction
        vpclmulqdq        \$0x00,$GH,$T3,$T2
        vpsrldq           \$4,$T2,$T2        # ; shift-R only 1-DW to obtain 2-DWs shift-R
        vpclmulqdq        \$0x10,$GH,$T3,$GH
        vpslldq           \$4,$GH,$GH        # ; Shift-L 1-DW to obtain result with no shifts
                                             # ; second phase of the reduction complete, the result is in $GH
        vpternlogq        \$0x96,$T2,$T1,$GH # ; GH = GH xor T1 xor T2
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; PRECOMPUTE computes HashKey_i
sub PRECOMPUTE {
  my $GCM128_CTX = $_[0];    #; [in/out] context pointer, hkeys content updated
  my $HK         = $_[1];    #; [in] xmm, hash key
  my $T1         = $_[2];    #; [clobbered] xmm
  my $T2         = $_[3];    #; [clobbered] xmm
  my $T3         = $_[4];    #; [clobbered] xmm
  my $T4         = $_[5];    #; [clobbered] xmm
  my $T5         = $_[6];    #; [clobbered] xmm
  my $T6         = $_[7];    #; [clobbered] xmm

  my $ZT1 = &ZWORD($T1);
  my $ZT2 = &ZWORD($T2);
  my $ZT3 = &ZWORD($T3);
  my $ZT4 = &ZWORD($T4);
  my $ZT5 = &ZWORD($T5);
  my $ZT6 = &ZWORD($T6);

  my $YT1 = &YWORD($T1);
  my $YT2 = &YWORD($T2);
  my $YT3 = &YWORD($T3);
  my $YT4 = &YWORD($T4);
  my $YT5 = &YWORD($T5);
  my $YT6 = &YWORD($T6);

  $code .= <<___;
        vshufi32x4   \$0x00,@{[YWORD($HK)]},@{[YWORD($HK)]},$YT5
        vmovdqa      $YT5,$YT4
___

  # ;; calculate HashKey^2<<1 mod poly
  &GHASH_MUL($YT4, $YT5, $YT1, $YT2, $YT3);

  $code .= <<___;
        vmovdqu64         $T4,@{[HashKeyByIdx(2,$GCM128_CTX)]}
        vinserti64x2      \$1,$HK,$YT4,$YT5
        vmovdqa64         $YT5,$YT6                             # ;; YT6 = HashKey | HashKey^2
___

  # ;; use 2x128-bit computation
  # ;; calculate HashKey^4<<1 mod poly, HashKey^3<<1 mod poly
  &GHASH_MUL($YT5, $YT4, $YT1, $YT2, $YT3);    # ;; YT5 = HashKey^3 | HashKey^4

  $code .= <<___;
        vmovdqu64         $YT5,@{[HashKeyByIdx(4,$GCM128_CTX)]}

        vinserti64x4      \$1,$YT6,$ZT5,$ZT5                    # ;; ZT5 = YT6 | YT5

        # ;; switch to 4x128-bit computations now
        vshufi64x2        \$0x00,$ZT5,$ZT5,$ZT4                 # ;; broadcast HashKey^4 across all ZT4
        vmovdqa64         $ZT5,$ZT6                             # ;; save HashKey^4 to HashKey^1 in ZT6
___

  # ;; calculate HashKey^5<<1 mod poly, HashKey^6<<1 mod poly, ... HashKey^8<<1 mod poly
  &GHASH_MUL($ZT5, $ZT4, $ZT1, $ZT2, $ZT3);
  $code .= <<___;
        vmovdqu64         $ZT5,@{[HashKeyByIdx(8,$GCM128_CTX)]} # ;; HashKey^8 to HashKey^5 in ZT5 now
        vshufi64x2        \$0x00,$ZT5,$ZT5,$ZT4                 # ;; broadcast HashKey^8 across all ZT4
___

  # ;; calculate HashKey^9<<1 mod poly, HashKey^10<<1 mod poly, ... HashKey^16<<1 mod poly
  # ;; use HashKey^8 as multiplier against ZT6 and ZT5 - this allows deeper ooo execution

  # ;; compute HashKey^(12), HashKey^(11), ... HashKey^(9)
  &GHASH_MUL($ZT6, $ZT4, $ZT1, $ZT2, $ZT3);
  $code .= "vmovdqu64         $ZT6,@{[HashKeyByIdx(12,$GCM128_CTX)]}\n";

  # ;; compute HashKey^(16), HashKey^(15), ... HashKey^(13)
  &GHASH_MUL($ZT5, $ZT4, $ZT1, $ZT2, $ZT3);
  $code .= "vmovdqu64         $ZT5,@{[HashKeyByIdx(16,$GCM128_CTX)]}\n";

  # ; Hkeys 17..48 will be precomputed somewhere else as context can hold only 16 hkeys
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; READ_SMALL_DATA_INPUT
# ;; Packs xmm register with data when data input is less or equal to 16 bytes
# ;; Returns 0 if data has length 0
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub READ_SMALL_DATA_INPUT {
  my $OUTPUT = $_[0];    # [out] xmm register
  my $INPUT  = $_[1];    # [in] buffer pointer to read from
  my $LENGTH = $_[2];    # [in] number of bytes to read
  my $TMP1   = $_[3];    # [clobbered]
  my $TMP2   = $_[4];    # [clobbered]
  my $MASK   = $_[5];    # [out] k1 to k7 register to store the partial block mask

  $code .= <<___;
        mov               \$16,@{[DWORD($TMP2)]}
        lea               byte_len_to_mask_table(%rip),$TMP1
        cmp               $TMP2,$LENGTH
        cmovc             $LENGTH,$TMP2
___
  if ($win64) {
    $code .= <<___;
        add               $TMP2,$TMP1
        add               $TMP2,$TMP1
        kmovw             ($TMP1),$MASK
___
  } else {
    $code .= "kmovw           ($TMP1,$TMP2,2),$MASK\n";
  }
  $code .= "vmovdqu8          ($INPUT),${OUTPUT}{$MASK}{z}\n";
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
#  CALC_AAD_HASH: Calculates the hash of the data which will not be encrypted.
#  Input: The input data (A_IN), that data's length (A_LEN), and the hash key (HASH_KEY).
#  Output: The hash of the data (AAD_HASH).
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub CALC_AAD_HASH {
  my $A_IN       = $_[0];     # [in] AAD text pointer
  my $A_LEN      = $_[1];     # [in] AAD length
  my $AAD_HASH   = $_[2];     # [in/out] xmm ghash value
  my $GCM128_CTX = $_[3];     # [in] pointer to context
  my $ZT0        = $_[4];     # [clobbered] ZMM register
  my $ZT1        = $_[5];     # [clobbered] ZMM register
  my $ZT2        = $_[6];     # [clobbered] ZMM register
  my $ZT3        = $_[7];     # [clobbered] ZMM register
  my $ZT4        = $_[8];     # [clobbered] ZMM register
  my $ZT5        = $_[9];     # [clobbered] ZMM register
  my $ZT6        = $_[10];    # [clobbered] ZMM register
  my $ZT7        = $_[11];    # [clobbered] ZMM register
  my $ZT8        = $_[12];    # [clobbered] ZMM register
  my $ZT9        = $_[13];    # [clobbered] ZMM register
  my $ZT10       = $_[14];    # [clobbered] ZMM register
  my $ZT11       = $_[15];    # [clobbered] ZMM register
  my $ZT12       = $_[16];    # [clobbered] ZMM register
  my $ZT13       = $_[17];    # [clobbered] ZMM register
  my $ZT14       = $_[18];    # [clobbered] ZMM register
  my $ZT15       = $_[19];    # [clobbered] ZMM register
  my $ZT16       = $_[20];    # [clobbered] ZMM register
  my $T1         = $_[21];    # [clobbered] GP register
  my $T2         = $_[22];    # [clobbered] GP register
  my $T3         = $_[23];    # [clobbered] GP register
  my $MASKREG    = $_[24];    # [clobbered] mask register

  my $HKEYS_READY = "%rbx";

  my $SHFMSK = $ZT13;

  my $label_suffix = $label_count++;

  $code .= <<___;
        mov               $A_IN,$T1      # ; T1 = AAD
        mov               $A_LEN,$T2     # ; T2 = aadLen
        or                $T2,$T2
        jz                .L_CALC_AAD_done_${label_suffix}

        xor               $HKEYS_READY,$HKEYS_READY
        vmovdqa64         SHUF_MASK(%rip),$SHFMSK

.L_get_AAD_loop48x16_${label_suffix}:
        cmp               \$`(48*16)`,$T2
        jl                .L_exit_AAD_loop48x16_${label_suffix}
___

  $code .= <<___;
        vmovdqu64         `64*0`($T1),$ZT1      # ; Blocks 0-3
        vmovdqu64         `64*1`($T1),$ZT2      # ; Blocks 4-7
        vmovdqu64         `64*2`($T1),$ZT3      # ; Blocks 8-11
        vmovdqu64         `64*3`($T1),$ZT4      # ; Blocks 12-15
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  &precompute_hkeys_on_stack($GCM128_CTX, $HKEYS_READY, $ZT0, $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, $ZT14, "all");
  $code .= "mov     \$1,$HKEYS_READY\n";

  &GHASH_16(
    "start",        $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", "%rsp",
    &HashKeyOffsetByIdx(48, "frame"), 0, "@{[ZWORD($AAD_HASH)]}", $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        vmovdqu64         `16*16 + 64*0`($T1),$ZT1      # ; Blocks 16-19
        vmovdqu64         `16*16 + 64*1`($T1),$ZT2      # ; Blocks 20-23
        vmovdqu64         `16*16 + 64*2`($T1),$ZT3      # ; Blocks 24-27
        vmovdqu64         `16*16 + 64*3`($T1),$ZT4      # ; Blocks 28-31
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  &GHASH_16(
    "mid",          $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", "%rsp",
    &HashKeyOffsetByIdx(32, "frame"), 0, "NO_HASH_IN_OUT", $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        vmovdqu64         `32*16 + 64*0`($T1),$ZT1      # ; Blocks 32-35
        vmovdqu64         `32*16 + 64*1`($T1),$ZT2      # ; Blocks 36-39
        vmovdqu64         `32*16 + 64*2`($T1),$ZT3      # ; Blocks 40-43
        vmovdqu64         `32*16 + 64*3`($T1),$ZT4      # ; Blocks 44-47
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  &GHASH_16(
    "end_reduce",   $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", "%rsp",
    &HashKeyOffsetByIdx(16, "frame"), 0, &ZWORD($AAD_HASH), $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        sub               \$`(48*16)`,$T2
        je                .L_CALC_AAD_done_${label_suffix}

        add               \$`(48*16)`,$T1
        jmp               .L_get_AAD_loop48x16_${label_suffix}

.L_exit_AAD_loop48x16_${label_suffix}:
        # ; Less than 48x16 bytes remaining
        cmp               \$`(32*16)`,$T2
        jl                .L_less_than_32x16_${label_suffix}
___

  $code .= <<___;
        # ; Get next 16 blocks
        vmovdqu64         `64*0`($T1),$ZT1
        vmovdqu64         `64*1`($T1),$ZT2
        vmovdqu64         `64*2`($T1),$ZT3
        vmovdqu64         `64*3`($T1),$ZT4
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  &precompute_hkeys_on_stack($GCM128_CTX, $HKEYS_READY, $ZT0, $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, $ZT14, "first32");
  $code .= "mov     \$1,$HKEYS_READY\n";

  &GHASH_16(
    "start",        $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", "%rsp",
    &HashKeyOffsetByIdx(32, "frame"), 0, &ZWORD($AAD_HASH), $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        vmovdqu64         `16*16 + 64*0`($T1),$ZT1
        vmovdqu64         `16*16 + 64*1`($T1),$ZT2
        vmovdqu64         `16*16 + 64*2`($T1),$ZT3
        vmovdqu64         `16*16 + 64*3`($T1),$ZT4
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  &GHASH_16(
    "end_reduce",   $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", "%rsp",
    &HashKeyOffsetByIdx(16, "frame"), 0, &ZWORD($AAD_HASH), $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        sub               \$`(32*16)`,$T2
        je                .L_CALC_AAD_done_${label_suffix}

        add               \$`(32*16)`,$T1
        jmp               .L_less_than_16x16_${label_suffix}

.L_less_than_32x16_${label_suffix}:
        cmp               \$`(16*16)`,$T2
        jl                .L_less_than_16x16_${label_suffix}
        # ; Get next 16 blocks
        vmovdqu64         `64*0`($T1),$ZT1
        vmovdqu64         `64*1`($T1),$ZT2
        vmovdqu64         `64*2`($T1),$ZT3
        vmovdqu64         `64*3`($T1),$ZT4
        vpshufb           $SHFMSK,$ZT1,$ZT1
        vpshufb           $SHFMSK,$ZT2,$ZT2
        vpshufb           $SHFMSK,$ZT3,$ZT3
        vpshufb           $SHFMSK,$ZT4,$ZT4
___

  # ; This code path does not use more than 16 hkeys, so they can be taken from the context
  # ; (not from the stack storage)
  &GHASH_16(
    "start_reduce", $ZT5,           $ZT6,           $ZT7,
    "NO_INPUT_PTR", "NO_INPUT_PTR", "NO_INPUT_PTR", $GCM128_CTX,
    &HashKeyOffsetByIdx(16, "context"), 0, &ZWORD($AAD_HASH), $ZT0,
    $ZT8,     $ZT9,  $ZT10, $ZT11,
    $ZT12,    $ZT14, $ZT15, $ZT16,
    "NO_ZMM", $ZT1,  $ZT2,  $ZT3,
    $ZT4);

  $code .= <<___;
        sub               \$`(16*16)`,$T2
        je                .L_CALC_AAD_done_${label_suffix}

        add               \$`(16*16)`,$T1
        # ; Less than 16x16 bytes remaining
.L_less_than_16x16_${label_suffix}:
        # ;; prep mask source address
        lea               byte64_len_to_mask_table(%rip),$T3
        lea               ($T3,$T2,8),$T3

        # ;; calculate number of blocks to ghash (including partial bytes)
        add               \$15,@{[DWORD($T2)]}
        shr               \$4,@{[DWORD($T2)]}
        cmp               \$2,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_1_${label_suffix}
        je                .L_AAD_blocks_2_${label_suffix}
        cmp               \$4,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_3_${label_suffix}
        je                .L_AAD_blocks_4_${label_suffix}
        cmp               \$6,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_5_${label_suffix}
        je                .L_AAD_blocks_6_${label_suffix}
        cmp               \$8,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_7_${label_suffix}
        je                .L_AAD_blocks_8_${label_suffix}
        cmp               \$10,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_9_${label_suffix}
        je                .L_AAD_blocks_10_${label_suffix}
        cmp               \$12,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_11_${label_suffix}
        je                .L_AAD_blocks_12_${label_suffix}
        cmp               \$14,@{[DWORD($T2)]}
        jb                .L_AAD_blocks_13_${label_suffix}
        je                .L_AAD_blocks_14_${label_suffix}
        cmp               \$15,@{[DWORD($T2)]}
        je                .L_AAD_blocks_15_${label_suffix}
___

  # ;; fall through for 16 blocks

  # ;; The flow of each of these cases is identical:
  # ;; - load blocks plain text
  # ;; - shuffle loaded blocks
  # ;; - xor in current hash value into block 0
  # ;; - perform up multiplications with ghash keys
  # ;; - jump to reduction code

  for (my $aad_blocks = 16; $aad_blocks > 0; $aad_blocks--) {
    $code .= ".L_AAD_blocks_${aad_blocks}_${label_suffix}:\n";
    if ($aad_blocks > 12) {
      $code .= "sub               \$`12*16*8`, $T3\n";
    } elsif ($aad_blocks > 8) {
      $code .= "sub               \$`8*16*8`, $T3\n";
    } elsif ($aad_blocks > 4) {
      $code .= "sub               \$`4*16*8`, $T3\n";
    }
    $code .= "kmovq             ($T3),$MASKREG\n";

    &ZMM_LOAD_MASKED_BLOCKS_0_16($aad_blocks, $T1, 0, $ZT1, $ZT2, $ZT3, $ZT4, $MASKREG);

    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16($aad_blocks, "vpshufb", $ZT1, $ZT2, $ZT3, $ZT4,
      $ZT1, $ZT2, $ZT3, $ZT4, $SHFMSK, $SHFMSK, $SHFMSK, $SHFMSK);

    &GHASH_1_TO_16($GCM128_CTX, &ZWORD($AAD_HASH),
      $ZT0, $ZT5, $ZT6, $ZT7, $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, &ZWORD($AAD_HASH), $ZT1, $ZT2, $ZT3, $ZT4, $aad_blocks);

    if ($aad_blocks > 1) {

      # ;; fall through to CALC_AAD_done in 1 block case
      $code .= "jmp           .L_CALC_AAD_done_${label_suffix}\n";
    }

  }
  $code .= ".L_CALC_AAD_done_${label_suffix}:\n";

  # ;; result in AAD_HASH
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; PARTIAL_BLOCK
# ;; Handles encryption/decryption and the tag partial blocks between
# ;; update calls.
# ;; Requires the input data be at least 1 byte long.
# ;; Output:
# ;; A cipher/plain of the first partial block (CIPH_PLAIN_OUT),
# ;; AAD_HASH and updated GCM128_CTX
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub PARTIAL_BLOCK {
  my $GCM128_CTX     = $_[0];     # [in] key pointer
  my $PBLOCK_LEN     = $_[1];     # [in] partial block length
  my $CIPH_PLAIN_OUT = $_[2];     # [in] output buffer
  my $PLAIN_CIPH_IN  = $_[3];     # [in] input buffer
  my $PLAIN_CIPH_LEN = $_[4];     # [in] buffer length
  my $DATA_OFFSET    = $_[5];     # [out] data offset (gets set)
  my $AAD_HASH       = $_[6];     # [out] updated GHASH value
  my $ENC_DEC        = $_[7];     # [in] cipher direction
  my $GPTMP0         = $_[8];     # [clobbered] GP temporary register
  my $GPTMP1         = $_[9];     # [clobbered] GP temporary register
  my $GPTMP2         = $_[10];    # [clobbered] GP temporary register
  my $ZTMP0          = $_[11];    # [clobbered] ZMM temporary register
  my $ZTMP1          = $_[12];    # [clobbered] ZMM temporary register
  my $ZTMP2          = $_[13];    # [clobbered] ZMM temporary register
  my $ZTMP3          = $_[14];    # [clobbered] ZMM temporary register
  my $ZTMP4          = $_[15];    # [clobbered] ZMM temporary register
  my $ZTMP5          = $_[16];    # [clobbered] ZMM temporary register
  my $ZTMP6          = $_[17];    # [clobbered] ZMM temporary register
  my $ZTMP7          = $_[18];    # [clobbered] ZMM temporary register
  my $MASKREG        = $_[19];    # [clobbered] mask temporary register

  my $XTMP0 = &XWORD($ZTMP0);
  my $XTMP1 = &XWORD($ZTMP1);
  my $XTMP2 = &XWORD($ZTMP2);
  my $XTMP3 = &XWORD($ZTMP3);
  my $XTMP4 = &XWORD($ZTMP4);
  my $XTMP5 = &XWORD($ZTMP5);
  my $XTMP6 = &XWORD($ZTMP6);
  my $XTMP7 = &XWORD($ZTMP7);

  my $LENGTH = $DATA_OFFSET;
  my $IA0    = $GPTMP1;
  my $IA1    = $GPTMP2;
  my $IA2    = $GPTMP0;

  my $label_suffix = $label_count++;

  $code .= <<___;
        # ;; if no partial block present then LENGTH/DATA_OFFSET will be set to zero
        mov             ($PBLOCK_LEN),$LENGTH
        or              $LENGTH,$LENGTH
        je              .L_partial_block_done_${label_suffix}         #  ;Leave Macro if no partial blocks
___

  &READ_SMALL_DATA_INPUT($XTMP0, $PLAIN_CIPH_IN, $PLAIN_CIPH_LEN, $IA0, $IA2, $MASKREG);

  $code .= <<___;
        # ;; XTMP1 = my_ctx_data.partial_block_enc_key
        vmovdqu64         $CTX_OFFSET_PEncBlock($GCM128_CTX),$XTMP1
        vmovdqu64         @{[HashKeyByIdx(1,$GCM128_CTX)]},$XTMP2

        # ;; adjust the shuffle mask pointer to be able to shift right $LENGTH bytes
        # ;; (16 - $LENGTH) is the number of bytes in plaintext mod 16)
        lea               SHIFT_MASK(%rip),$IA0
        add               $LENGTH,$IA0
        vmovdqu64         ($IA0),$XTMP3         # ; shift right shuffle mask
        vpshufb           $XTMP3,$XTMP1,$XTMP1
___

  if ($ENC_DEC eq "DEC") {
    $code .= <<___;
        # ;;  keep copy of cipher text in $XTMP4
        vmovdqa64         $XTMP0,$XTMP4
___
  }
  $code .= <<___;
        vpxorq            $XTMP0,$XTMP1,$XTMP1  # ; Ciphertext XOR E(K, Yn)
        # ;; Set $IA1 to be the amount of data left in CIPH_PLAIN_IN after filling the block
        # ;; Determine if partial block is not being filled and shift mask accordingly
___
  if ($win64) {
    $code .= <<___;
        mov               $PLAIN_CIPH_LEN,$IA1
        add               $LENGTH,$IA1
___
  } else {
    $code .= "lea               ($PLAIN_CIPH_LEN, $LENGTH, 1),$IA1\n";
  }
  $code .= <<___;
        sub               \$16,$IA1
        jge               .L_no_extra_mask_${label_suffix}
        sub               $IA1,$IA0
.L_no_extra_mask_${label_suffix}:
        # ;; get the appropriate mask to mask out bottom $LENGTH bytes of $XTMP1
        # ;; - mask out bottom $LENGTH bytes of $XTMP1
        # ;; sizeof(SHIFT_MASK) == 16 bytes
        vmovdqu64         16($IA0),$XTMP0
        vpand             $XTMP0,$XTMP1,$XTMP1
___

  if ($ENC_DEC eq "DEC") {
    $code .= <<___;
        vpand             $XTMP0,$XTMP4,$XTMP4
        vpshufb           SHUF_MASK(%rip),$XTMP4,$XTMP4
        vpshufb           $XTMP3,$XTMP4,$XTMP4
        vpxorq            $XTMP4,$AAD_HASH,$AAD_HASH
___
  } else {
    $code .= <<___;
        vpshufb           SHUF_MASK(%rip),$XTMP1,$XTMP1
        vpshufb           $XTMP3,$XTMP1,$XTMP1
        vpxorq            $XTMP1,$AAD_HASH,$AAD_HASH
___
  }
  $code .= <<___;
        cmp               \$0,$IA1
        jl                .L_partial_incomplete_${label_suffix}
___

  # ;; GHASH computation for the last <16 Byte block
  &GHASH_MUL($AAD_HASH, $XTMP2, $XTMP5, $XTMP6, $XTMP7);

  $code .= <<___;
        movq              \$0, ($PBLOCK_LEN)
        # ;;  Set $LENGTH to be the number of bytes to write out
        mov               $LENGTH,$IA0
        mov               \$16,$LENGTH
        sub               $IA0,$LENGTH
        jmp               .L_enc_dec_done_${label_suffix}

.L_partial_incomplete_${label_suffix}:
___
  if ($win64) {
    $code .= <<___;
        mov               $PLAIN_CIPH_LEN,$IA0
        add               $IA0,($PBLOCK_LEN)
___
  } else {
    $code .= "add               $PLAIN_CIPH_LEN,($PBLOCK_LEN)\n";
  }
  $code .= <<___;
        mov               $PLAIN_CIPH_LEN,$LENGTH

.L_enc_dec_done_${label_suffix}:
        # ;; output encrypted Bytes

        lea               byte_len_to_mask_table(%rip),$IA0
        kmovw             ($IA0,$LENGTH,2),$MASKREG
        vmovdqu64         $AAD_HASH,$CTX_OFFSET_AadHash($GCM128_CTX)
___

  if ($ENC_DEC eq "ENC") {
    $code .= <<___;
        # ;; shuffle XTMP1 back to output as ciphertext
        vpshufb           SHUF_MASK(%rip),$XTMP1,$XTMP1
        vpshufb           $XTMP3,$XTMP1,$XTMP1
___
  }
  $code .= <<___;
        mov               $CIPH_PLAIN_OUT,$IA0
        vmovdqu8          $XTMP1,($IA0){$MASKREG}
.L_partial_block_done_${label_suffix}:
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; Ciphers 1 to 16 blocks and prepares them for later GHASH compute operation
sub INITIAL_BLOCKS_PARTIAL_CIPHER {
  my $AES_KEYS        = $_[0];     # [in] key pointer
  my $GCM128_CTX      = $_[1];     # [in] context pointer
  my $CIPH_PLAIN_OUT  = $_[2];     # [in] text output pointer
  my $PLAIN_CIPH_IN   = $_[3];     # [in] text input pointer
  my $LENGTH          = $_[4];     # [in/clobbered] length in bytes
  my $DATA_OFFSET     = $_[5];     # [in/out] current data offset (updated)
  my $NUM_BLOCKS      = $_[6];     # [in] can only be 1, 2, 3, 4, 5, ..., 15 or 16 (not 0)
  my $CTR             = $_[7];     # [in/out] current counter value
  my $ENC_DEC         = $_[8];     # [in] cipher direction (ENC/DEC)
  my $DAT0            = $_[9];     # [out] ZMM with cipher text shuffled for GHASH
  my $DAT1            = $_[10];    # [out] ZMM with cipher text shuffled for GHASH
  my $DAT2            = $_[11];    # [out] ZMM with cipher text shuffled for GHASH
  my $DAT3            = $_[12];    # [out] ZMM with cipher text shuffled for GHASH
  my $LAST_CIPHER_BLK = $_[13];    # [out] XMM to put ciphered counter block partially xor'ed with text
  my $LAST_GHASH_BLK  = $_[14];    # [out] XMM to put last cipher text block shuffled for GHASH
  my $CTR0            = $_[15];    # [clobbered] ZMM temporary
  my $CTR1            = $_[16];    # [clobbered] ZMM temporary
  my $CTR2            = $_[17];    # [clobbered] ZMM temporary
  my $CTR3            = $_[18];    # [clobbered] ZMM temporary
  my $ZT1             = $_[19];    # [clobbered] ZMM temporary
  my $IA0             = $_[20];    # [clobbered] GP temporary
  my $IA1             = $_[21];    # [clobbered] GP temporary
  my $MASKREG         = $_[22];    # [clobbered] mask register
  my $SHUFMASK        = $_[23];    # [out] ZMM loaded with BE/LE shuffle mask

  if ($NUM_BLOCKS == 1) {
    $code .= "vmovdqa64         SHUF_MASK(%rip),@{[XWORD($SHUFMASK)]}\n";
  } elsif ($NUM_BLOCKS == 2) {
    $code .= "vmovdqa64         SHUF_MASK(%rip),@{[YWORD($SHUFMASK)]}\n";
  } else {
    $code .= "vmovdqa64         SHUF_MASK(%rip),$SHUFMASK\n";
  }

  # ;; prepare AES counter blocks
  if ($NUM_BLOCKS == 1) {
    $code .= "vpaddd            ONE(%rip),$CTR,@{[XWORD($CTR0)]}\n";
  } elsif ($NUM_BLOCKS == 2) {
    $code .= <<___;
        vshufi64x2        \$0,@{[YWORD($CTR)]},@{[YWORD($CTR)]},@{[YWORD($CTR0)]}
        vpaddd            ddq_add_1234(%rip),@{[YWORD($CTR0)]},@{[YWORD($CTR0)]}
___
  } else {
    $code .= <<___;
        vshufi64x2        \$0,@{[ZWORD($CTR)]},@{[ZWORD($CTR)]},@{[ZWORD($CTR)]}
        vpaddd            ddq_add_1234(%rip),@{[ZWORD($CTR)]},$CTR0
___
    if ($NUM_BLOCKS > 4) {
      $code .= "vpaddd            ddq_add_5678(%rip),@{[ZWORD($CTR)]},$CTR1\n";
    }
    if ($NUM_BLOCKS > 8) {
      $code .= "vpaddd            ddq_add_8888(%rip),$CTR0,$CTR2\n";
    }
    if ($NUM_BLOCKS > 12) {
      $code .= "vpaddd            ddq_add_8888(%rip),$CTR1,$CTR3\n";
    }
  }

  # ;; get load/store mask
  $code .= <<___;
        lea               byte64_len_to_mask_table(%rip),$IA0
        mov               $LENGTH,$IA1
___
  if ($NUM_BLOCKS > 12) {
    $code .= "sub               \$`3*64`,$IA1\n";
  } elsif ($NUM_BLOCKS > 8) {
    $code .= "sub               \$`2*64`,$IA1\n";
  } elsif ($NUM_BLOCKS > 4) {
    $code .= "sub               \$`1*64`,$IA1\n";
  }
  $code .= "kmovq             ($IA0,$IA1,8),$MASKREG\n";

  # ;; extract new counter value
  # ;; shuffle the counters for AES rounds
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 1)`,$CTR0,$CTR\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 5)`,$CTR1,$CTR\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 9)`,$CTR2,$CTR\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 13)`,$CTR3,$CTR\n";
  }
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vpshufb", $CTR0, $CTR1,     $CTR2,     $CTR3,     $CTR0,
    $CTR1,       $CTR2,     $CTR3, $SHUFMASK, $SHUFMASK, $SHUFMASK, $SHUFMASK);

  # ;; load plain/cipher text
  &ZMM_LOAD_MASKED_BLOCKS_0_16($NUM_BLOCKS, $PLAIN_CIPH_IN, $DATA_OFFSET, $DAT0, $DAT1, $DAT2, $DAT3, $MASKREG);

  # ;; AES rounds and XOR with plain/cipher text
  foreach my $j (0 .. ($NROUNDS + 1)) {
    $code .= "vbroadcastf64x2    `($j * 16)`($AES_KEYS),$ZT1\n";
    &ZMM_AESENC_ROUND_BLOCKS_0_16($CTR0, $CTR1, $CTR2, $CTR3, $ZT1, $j,
      $DAT0, $DAT1, $DAT2, $DAT3, $NUM_BLOCKS, $NROUNDS);
  }

  # ;; retrieve the last cipher counter block (partially XOR'ed with text)
  # ;; - this is needed for partial block cases
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 1)`,$CTR0,$LAST_CIPHER_BLK\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 5)`,$CTR1,$LAST_CIPHER_BLK\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 9)`,$CTR2,$LAST_CIPHER_BLK\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 13)`,$CTR3,$LAST_CIPHER_BLK\n";
  }

  # ;; write cipher/plain text back to output and
  $code .= "mov       $CIPH_PLAIN_OUT,$IA0\n";
  &ZMM_STORE_MASKED_BLOCKS_0_16($NUM_BLOCKS, $IA0, $DATA_OFFSET, $CTR0, $CTR1, $CTR2, $CTR3, $MASKREG);

  # ;; zero bytes outside the mask before hashing
  if ($NUM_BLOCKS <= 4) {
    $code .= "vmovdqu8          $CTR0,${CTR0}{$MASKREG}{z}\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vmovdqu8          $CTR1,${CTR1}{$MASKREG}{z}\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vmovdqu8          $CTR2,${CTR2}{$MASKREG}{z}\n";
  } else {
    $code .= "vmovdqu8          $CTR3,${CTR3}{$MASKREG}{z}\n";
  }

  # ;; Shuffle the cipher text blocks for hashing part
  # ;; ZT5 and ZT6 are expected outputs with blocks for hashing
  if ($ENC_DEC eq "DEC") {

    # ;; Decrypt case
    # ;; - cipher blocks are in ZT5 & ZT6
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vpshufb", $DAT0, $DAT1,     $DAT2,     $DAT3,     $DAT0,
      $DAT1,       $DAT2,     $DAT3, $SHUFMASK, $SHUFMASK, $SHUFMASK, $SHUFMASK);
  } else {

    # ;; Encrypt case
    # ;; - cipher blocks are in CTR0-CTR3
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vpshufb", $DAT0, $DAT1,     $DAT2,     $DAT3,     $CTR0,
      $CTR1,       $CTR2,     $CTR3, $SHUFMASK, $SHUFMASK, $SHUFMASK, $SHUFMASK);
  }

  # ;; Extract the last block for partials and multi_call cases
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-1)`,$DAT0,$LAST_GHASH_BLK\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-5)`,$DAT1,$LAST_GHASH_BLK\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-9)`,$DAT2,$LAST_GHASH_BLK\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-13)`,$DAT3,$LAST_GHASH_BLK\n";
  }

}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; Computes GHASH on 1 to 16 blocks
sub INITIAL_BLOCKS_PARTIAL_GHASH {
  my $AES_KEYS        = $_[0];     # [in] key pointer
  my $GCM128_CTX      = $_[1];     # [in] context pointer
  my $LENGTH          = $_[2];     # [in/clobbered] length in bytes
  my $NUM_BLOCKS      = $_[3];     # [in] can only be 1, 2, 3, 4, 5, ..., 15 or 16 (not 0)
  my $HASH_IN_OUT     = $_[4];     # [in/out] XMM ghash in/out value
  my $ENC_DEC         = $_[5];     # [in] cipher direction (ENC/DEC)
  my $DAT0            = $_[6];     # [in] ZMM with cipher text shuffled for GHASH
  my $DAT1            = $_[7];     # [in] ZMM with cipher text shuffled for GHASH
  my $DAT2            = $_[8];     # [in] ZMM with cipher text shuffled for GHASH
  my $DAT3            = $_[9];     # [in] ZMM with cipher text shuffled for GHASH
  my $LAST_CIPHER_BLK = $_[10];    # [in] XMM with ciphered counter block partially xor'ed with text
  my $LAST_GHASH_BLK  = $_[11];    # [in] XMM with last cipher text block shuffled for GHASH
  my $ZT0             = $_[12];    # [clobbered] ZMM temporary
  my $ZT1             = $_[13];    # [clobbered] ZMM temporary
  my $ZT2             = $_[14];    # [clobbered] ZMM temporary
  my $ZT3             = $_[15];    # [clobbered] ZMM temporary
  my $ZT4             = $_[16];    # [clobbered] ZMM temporary
  my $ZT5             = $_[17];    # [clobbered] ZMM temporary
  my $ZT6             = $_[18];    # [clobbered] ZMM temporary
  my $ZT7             = $_[19];    # [clobbered] ZMM temporary
  my $ZT8             = $_[20];    # [clobbered] ZMM temporary
  my $PBLOCK_LEN      = $_[21];    # [in] partial block length
  my $GH              = $_[22];    # [in] ZMM with hi product part
  my $GM              = $_[23];    # [in] ZMM with mid product part
  my $GL              = $_[24];    # [in] ZMM with lo product part

  my $label_suffix = $label_count++;

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;;; - Hash all but the last partial block of data
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

  # ;; update data offset
  if ($NUM_BLOCKS > 1) {

    # ;; The final block of data may be <16B
    $code .= "sub               \$16 * ($NUM_BLOCKS - 1),$LENGTH\n";
  }

  if ($NUM_BLOCKS < 16) {
    $code .= <<___;
        # ;; NOTE: the 'jl' is always taken for num_initial_blocks = 16.
        # ;;      This is run in the context of GCM_ENC_DEC_SMALL for length < 256.
        cmp               \$16,$LENGTH
        jl                .L_small_initial_partial_block_${label_suffix}

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;;; Handle a full length final block - encrypt and hash all blocks
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

        sub               \$16,$LENGTH
        movq              \$0,($PBLOCK_LEN)
___

    # ;; Hash all of the data
    if (scalar(@_) == 22) {

      # ;; start GHASH compute
      &GHASH_1_TO_16($GCM128_CTX, $HASH_IN_OUT, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4,
        $ZT5, $ZT6, $ZT7, $ZT8, &ZWORD($HASH_IN_OUT), $DAT0, $DAT1, $DAT2, $DAT3, $NUM_BLOCKS);
    } elsif (scalar(@_) == 25) {

      # ;; continue GHASH compute
      &GHASH_1_TO_16($GCM128_CTX, $HASH_IN_OUT, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4,
        $ZT5, $ZT6, $ZT7, $ZT8, &ZWORD($HASH_IN_OUT), $DAT0, $DAT1, $DAT2, $DAT3, $NUM_BLOCKS, $GH, $GM, $GL);
    }
    $code .= "jmp           .L_small_initial_compute_done_${label_suffix}\n";
  }

  $code .= <<___;
.L_small_initial_partial_block_${label_suffix}:

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;;; Handle ghash for a <16B final block
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

        # ;; As it's an init / update / finalize series we need to leave the
        # ;; last block if it's less than a full block of data.

        mov               $LENGTH,($PBLOCK_LEN)
        vmovdqu64         $LAST_CIPHER_BLK,$CTX_OFFSET_PEncBlock($GCM128_CTX)
___

  my $k                  = ($NUM_BLOCKS - 1);
  my $last_block_to_hash = 1;
  if (($NUM_BLOCKS > $last_block_to_hash)) {

    # ;; ZT12-ZT20 - temporary registers
    if (scalar(@_) == 22) {

      # ;; start GHASH compute
      &GHASH_1_TO_16($GCM128_CTX, $HASH_IN_OUT, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4,
        $ZT5, $ZT6, $ZT7, $ZT8, &ZWORD($HASH_IN_OUT), $DAT0, $DAT1, $DAT2, $DAT3, $k);
    } elsif (scalar(@_) == 25) {

      # ;; continue GHASH compute
      &GHASH_1_TO_16($GCM128_CTX, $HASH_IN_OUT, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4,
        $ZT5, $ZT6, $ZT7, $ZT8, &ZWORD($HASH_IN_OUT), $DAT0, $DAT1, $DAT2, $DAT3, $k, $GH, $GM, $GL);
    }

    # ;; just fall through no jmp needed
  } else {

    if (scalar(@_) == 25) {
      $code .= <<___;
        # ;; Reduction is required in this case.
        # ;; Integrate GM into GH and GL.
        vpsrldq           \$8,$GM,$ZT0
        vpslldq           \$8,$GM,$ZT1
        vpxorq            $ZT0,$GH,$GH
        vpxorq            $ZT1,$GL,$GL
___

      # ;; Add GH and GL 128-bit words horizontally
      &VHPXORI4x128($GH, $ZT0);
      &VHPXORI4x128($GL, $ZT1);

      # ;; 256-bit to 128-bit reduction
      $code .= "vmovdqa64         POLY2(%rip),@{[XWORD($ZT0)]}\n";
      &VCLMUL_REDUCE(&XWORD($HASH_IN_OUT), &XWORD($ZT0), &XWORD($GH), &XWORD($GL), &XWORD($ZT1), &XWORD($ZT2));
    }
    $code .= <<___;
        # ;; Record that a reduction is not needed -
        # ;; In this case no hashes are computed because there
        # ;; is only one initial block and it is < 16B in length.
        # ;; We only need to check if a reduction is needed if
        # ;; initial_blocks == 1 and init/update/final is being used.
        # ;; In this case we may just have a partial block, and that
        # ;; gets hashed in finalize.

        # ;; The hash should end up in HASH_IN_OUT.
        # ;; The only way we should get here is if there is
        # ;; a partial block of data, so xor that into the hash.
        vpxorq            $LAST_GHASH_BLK,$HASH_IN_OUT,$HASH_IN_OUT
        # ;; The result is in $HASH_IN_OUT
        jmp               .L_after_reduction_${label_suffix}
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;;; After GHASH reduction
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

  $code .= ".L_small_initial_compute_done_${label_suffix}:\n";

  # ;; If using init/update/finalize, we need to xor any partial block data
  # ;; into the hash.
  if ($NUM_BLOCKS > 1) {

    # ;; NOTE: for $NUM_BLOCKS = 0 the xor never takes place
    if ($NUM_BLOCKS != 16) {
      $code .= <<___;
        # ;; NOTE: for $NUM_BLOCKS = 16, $LENGTH, stored in [PBlockLen] is never zero
        or                $LENGTH,$LENGTH
        je                .L_after_reduction_${label_suffix}
___
    }
    $code .= "vpxorq            $LAST_GHASH_BLK,$HASH_IN_OUT,$HASH_IN_OUT\n";
  }

  $code .= ".L_after_reduction_${label_suffix}:\n";

  # ;; Final hash is now in HASH_IN_OUT
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; INITIAL_BLOCKS_PARTIAL macro with support for a partial final block.
# ;; It may look similar to INITIAL_BLOCKS but its usage is different:
# ;; - first encrypts/decrypts required number of blocks and then
# ;;   ghashes these blocks
# ;; - Small packets or left over data chunks (<256 bytes)
# ;; - Remaining data chunks below 256 bytes (multi buffer code)
# ;;
# ;; num_initial_blocks is expected to include the partial final block
# ;; in the count.
sub INITIAL_BLOCKS_PARTIAL {
  my $AES_KEYS        = $_[0];     # [in] key pointer
  my $GCM128_CTX      = $_[1];     # [in] context pointer
  my $CIPH_PLAIN_OUT  = $_[2];     # [in] text output pointer
  my $PLAIN_CIPH_IN   = $_[3];     # [in] text input pointer
  my $LENGTH          = $_[4];     # [in/clobbered] length in bytes
  my $DATA_OFFSET     = $_[5];     # [in/out] current data offset (updated)
  my $NUM_BLOCKS      = $_[6];     # [in] can only be 1, 2, 3, 4, 5, ..., 15 or 16 (not 0)
  my $CTR             = $_[7];     # [in/out] current counter value
  my $HASH_IN_OUT     = $_[8];     # [in/out] XMM ghash in/out value
  my $ENC_DEC         = $_[9];     # [in] cipher direction (ENC/DEC)
  my $CTR0            = $_[10];    # [clobbered] ZMM temporary
  my $CTR1            = $_[11];    # [clobbered] ZMM temporary
  my $CTR2            = $_[12];    # [clobbered] ZMM temporary
  my $CTR3            = $_[13];    # [clobbered] ZMM temporary
  my $DAT0            = $_[14];    # [clobbered] ZMM temporary
  my $DAT1            = $_[15];    # [clobbered] ZMM temporary
  my $DAT2            = $_[16];    # [clobbered] ZMM temporary
  my $DAT3            = $_[17];    # [clobbered] ZMM temporary
  my $LAST_CIPHER_BLK = $_[18];    # [clobbered] ZMM temporary
  my $LAST_GHASH_BLK  = $_[19];    # [clobbered] ZMM temporary
  my $ZT0             = $_[20];    # [clobbered] ZMM temporary
  my $ZT1             = $_[21];    # [clobbered] ZMM temporary
  my $ZT2             = $_[22];    # [clobbered] ZMM temporary
  my $ZT3             = $_[23];    # [clobbered] ZMM temporary
  my $ZT4             = $_[24];    # [clobbered] ZMM temporary
  my $IA0             = $_[25];    # [clobbered] GP temporary
  my $IA1             = $_[26];    # [clobbered] GP temporary
  my $MASKREG         = $_[27];    # [clobbered] mask register
  my $SHUFMASK        = $_[28];    # [clobbered] ZMM for BE/LE shuffle mask
  my $PBLOCK_LEN      = $_[29];    # [in] partial block length

  &INITIAL_BLOCKS_PARTIAL_CIPHER(
    $AES_KEYS, $GCM128_CTX,              $CIPH_PLAIN_OUT,         $PLAIN_CIPH_IN,
    $LENGTH,   $DATA_OFFSET,             $NUM_BLOCKS,             $CTR,
    $ENC_DEC,  $DAT0,                    $DAT1,                   $DAT2,
    $DAT3,     &XWORD($LAST_CIPHER_BLK), &XWORD($LAST_GHASH_BLK), $CTR0,
    $CTR1,     $CTR2,                    $CTR3,                   $ZT0,
    $IA0,      $IA1,                     $MASKREG,                $SHUFMASK);

  &INITIAL_BLOCKS_PARTIAL_GHASH($AES_KEYS, $GCM128_CTX, $LENGTH, $NUM_BLOCKS, $HASH_IN_OUT, $ENC_DEC, $DAT0,
    $DAT1, $DAT2, $DAT3, &XWORD($LAST_CIPHER_BLK),
    &XWORD($LAST_GHASH_BLK), $CTR0, $CTR1, $CTR2, $CTR3, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4, $PBLOCK_LEN);
}

# ;; ===========================================================================
# ;; Stitched GHASH of 16 blocks (with reduction) with encryption of N blocks
# ;; followed with GHASH of the N blocks.
sub GHASH_16_ENCRYPT_N_GHASH_N {
  my $AES_KEYS           = $_[0];     # [in] key pointer
  my $GCM128_CTX         = $_[1];     # [in] context pointer
  my $CIPH_PLAIN_OUT     = $_[2];     # [in] pointer to output buffer
  my $PLAIN_CIPH_IN      = $_[3];     # [in] pointer to input buffer
  my $DATA_OFFSET        = $_[4];     # [in] data offset
  my $LENGTH             = $_[5];     # [in] data length
  my $CTR_BE             = $_[6];     # [in/out] ZMM counter blocks (last 4) in big-endian
  my $CTR_CHECK          = $_[7];     # [in/out] GP with 8-bit counter for overflow check
  my $HASHKEY_OFFSET     = $_[8];     # [in] numerical offset for the highest hash key
                                      # (can be in form of register or numerical value)
  my $GHASHIN_BLK_OFFSET = $_[9];     # [in] numerical offset for GHASH blocks in
  my $SHFMSK             = $_[10];    # [in] ZMM with byte swap mask for pshufb
  my $B00_03             = $_[11];    # [clobbered] temporary ZMM
  my $B04_07             = $_[12];    # [clobbered] temporary ZMM
  my $B08_11             = $_[13];    # [clobbered] temporary ZMM
  my $B12_15             = $_[14];    # [clobbered] temporary ZMM
  my $GH1H_UNUSED        = $_[15];    # [clobbered] temporary ZMM
  my $GH1L               = $_[16];    # [clobbered] temporary ZMM
  my $GH1M               = $_[17];    # [clobbered] temporary ZMM
  my $GH1T               = $_[18];    # [clobbered] temporary ZMM
  my $GH2H               = $_[19];    # [clobbered] temporary ZMM
  my $GH2L               = $_[20];    # [clobbered] temporary ZMM
  my $GH2M               = $_[21];    # [clobbered] temporary ZMM
  my $GH2T               = $_[22];    # [clobbered] temporary ZMM
  my $GH3H               = $_[23];    # [clobbered] temporary ZMM
  my $GH3L               = $_[24];    # [clobbered] temporary ZMM
  my $GH3M               = $_[25];    # [clobbered] temporary ZMM
  my $GH3T               = $_[26];    # [clobbered] temporary ZMM
  my $AESKEY1            = $_[27];    # [clobbered] temporary ZMM
  my $AESKEY2            = $_[28];    # [clobbered] temporary ZMM
  my $GHKEY1             = $_[29];    # [clobbered] temporary ZMM
  my $GHKEY2             = $_[30];    # [clobbered] temporary ZMM
  my $GHDAT1             = $_[31];    # [clobbered] temporary ZMM
  my $GHDAT2             = $_[32];    # [clobbered] temporary ZMM
  my $ZT01               = $_[33];    # [clobbered] temporary ZMM
  my $ADDBE_4x4          = $_[34];    # [in] ZMM with 4x128bits 4 in big-endian
  my $ADDBE_1234         = $_[35];    # [in] ZMM with 4x128bits 1, 2, 3 and 4 in big-endian
  my $GHASH_TYPE         = $_[36];    # [in] "start", "start_reduce", "mid", "end_reduce"
  my $TO_REDUCE_L        = $_[37];    # [in] ZMM for low 4x128-bit GHASH sum
  my $TO_REDUCE_H        = $_[38];    # [in] ZMM for hi 4x128-bit GHASH sum
  my $TO_REDUCE_M        = $_[39];    # [in] ZMM for medium 4x128-bit GHASH sum
  my $ENC_DEC            = $_[40];    # [in] cipher direction
  my $HASH_IN_OUT        = $_[41];    # [in/out] XMM ghash in/out value
  my $IA0                = $_[42];    # [clobbered] GP temporary
  my $IA1                = $_[43];    # [clobbered] GP temporary
  my $MASKREG            = $_[44];    # [clobbered] mask register
  my $NUM_BLOCKS         = $_[45];    # [in] numerical value with number of blocks to be encrypted/ghashed (1 to 16)
  my $PBLOCK_LEN         = $_[46];    # [in] partial block length

  die "GHASH_16_ENCRYPT_N_GHASH_N: num_blocks is out of bounds = $NUM_BLOCKS\n"
    if ($NUM_BLOCKS > 16 || $NUM_BLOCKS < 0);

  my $label_suffix = $label_count++;

  my $GH1H = $HASH_IN_OUT;

  # ; this is to avoid additional move in do_reduction case

  my $LAST_GHASH_BLK  = $GH1L;
  my $LAST_CIPHER_BLK = $GH1T;

  my $RED_POLY = $GH2T;
  my $RED_P1   = $GH2L;
  my $RED_T1   = $GH2H;
  my $RED_T2   = $GH2M;

  my $DATA1 = $GH3H;
  my $DATA2 = $GH3L;
  my $DATA3 = $GH3M;
  my $DATA4 = $GH3T;

  # ;; do reduction after the 16 blocks ?
  my $do_reduction = 0;

  # ;; is 16 block chunk a start?
  my $is_start = 0;

  if ($GHASH_TYPE eq "start_reduce") {
    $is_start     = 1;
    $do_reduction = 1;
  }

  if ($GHASH_TYPE eq "start") {
    $is_start = 1;
  }

  if ($GHASH_TYPE eq "end_reduce") {
    $do_reduction = 1;
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; - get load/store mask
  # ;; - load plain/cipher text
  # ;; get load/store mask
  $code .= <<___;
        lea               byte64_len_to_mask_table(%rip),$IA0
        mov               $LENGTH,$IA1
___
  if ($NUM_BLOCKS > 12) {
    $code .= "sub               \$`3*64`,$IA1\n";
  } elsif ($NUM_BLOCKS > 8) {
    $code .= "sub               \$`2*64`,$IA1\n";
  } elsif ($NUM_BLOCKS > 4) {
    $code .= "sub               \$`1*64`,$IA1\n";
  }
  $code .= "kmovq             ($IA0,$IA1,8),$MASKREG\n";

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; prepare counter blocks

  $code .= <<___;
        cmp               \$`(256 - $NUM_BLOCKS)`,@{[DWORD($CTR_CHECK)]}
        jae               .L_16_blocks_overflow_${label_suffix}
___

  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vpaddd", $B00_03, $B04_07,     $B08_11,    $B12_15,    $CTR_BE,
    $B00_03,     $B04_07,  $B08_11, $ADDBE_1234, $ADDBE_4x4, $ADDBE_4x4, $ADDBE_4x4);
  $code .= <<___;
        jmp               .L_16_blocks_ok_${label_suffix}

.L_16_blocks_overflow_${label_suffix}:
        vpshufb           $SHFMSK,$CTR_BE,$CTR_BE
        vpaddd            ddq_add_1234(%rip),$CTR_BE,$B00_03
___
  if ($NUM_BLOCKS > 4) {
    $code .= <<___;
        vmovdqa64         ddq_add_4444(%rip),$B12_15
        vpaddd            $B12_15,$B00_03,$B04_07
___
  }
  if ($NUM_BLOCKS > 8) {
    $code .= "vpaddd            $B12_15,$B04_07,$B08_11\n";
  }
  if ($NUM_BLOCKS > 12) {
    $code .= "vpaddd            $B12_15,$B08_11,$B12_15\n";
  }
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vpshufb", $B00_03, $B04_07, $B08_11, $B12_15, $B00_03,
    $B04_07,     $B08_11,   $B12_15, $SHFMSK, $SHFMSK, $SHFMSK, $SHFMSK);
  $code .= <<___;
.L_16_blocks_ok_${label_suffix}:

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; - pre-load constants
        # ;; - add current hash into the 1st block
        vbroadcastf64x2    `(16 * 0)`($AES_KEYS),$AESKEY1
___
  if ($is_start != 0) {
    $code .= "vpxorq            `$GHASHIN_BLK_OFFSET + (0*64)`(%rsp),$HASH_IN_OUT,$GHDAT1\n";
  } else {
    $code .= "vmovdqa64         `$GHASHIN_BLK_OFFSET + (0*64)`(%rsp),$GHDAT1\n";
  }

  $code .= "vmovdqu64         @{[EffectiveAddress(\"%rsp\",$HASHKEY_OFFSET,0*64)]},$GHKEY1\n";

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; save counter for the next round
  # ;; increment counter overflow check register
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 1)`,$B00_03,@{[XWORD($CTR_BE)]}\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 5)`,$B04_07,@{[XWORD($CTR_BE)]}\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 9)`,$B08_11,@{[XWORD($CTR_BE)]}\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 13)`,$B12_15,@{[XWORD($CTR_BE)]}\n";
  }
  $code .= "vshufi64x2        \$0b00000000,$CTR_BE,$CTR_BE,$CTR_BE\n";

  $code .= <<___;
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; pre-load constants
        vbroadcastf64x2    `(16 * 1)`($AES_KEYS),$AESKEY2
        vmovdqu64         @{[EffectiveAddress("%rsp",$HASHKEY_OFFSET,1*64)]},$GHKEY2
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (1*64)`(%rsp),$GHDAT2
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; stitch AES rounds with GHASH

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 0 - ARK

  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vpxorq", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,  $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
  $code .= "vbroadcastf64x2    `(16 * 2)`($AES_KEYS),$AESKEY1\n";

  $code .= <<___;
        # ;;==================================================
        # ;; GHASH 4 blocks (15 to 12)
        vpclmulqdq        \$0x11,$GHKEY1,$GHDAT1,$GH1H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY1,$GHDAT1,$GH1L      # ; a0*b0
        vpclmulqdq        \$0x01,$GHKEY1,$GHDAT1,$GH1M      # ; a1*b0
        vpclmulqdq        \$0x10,$GHKEY1,$GHDAT1,$GH1T      # ; a0*b1
        vmovdqu64         @{[EffectiveAddress("%rsp",$HASHKEY_OFFSET,2*64)]},$GHKEY1
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (2*64)`(%rsp),$GHDAT1
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 1
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
  $code .= "vbroadcastf64x2    `(16 * 3)`($AES_KEYS),$AESKEY2\n";

  $code .= <<___;
        # ;; =================================================
        # ;; GHASH 4 blocks (11 to 8)
        vpclmulqdq        \$0x10,$GHKEY2,$GHDAT2,$GH2M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY2,$GHDAT2,$GH2T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY2,$GHDAT2,$GH2H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY2,$GHDAT2,$GH2L      # ; a0*b0
        vmovdqu64         @{[EffectiveAddress("%rsp",$HASHKEY_OFFSET,3*64)]},$GHKEY2
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (3*64)`(%rsp),$GHDAT2
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 2
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
  $code .= "vbroadcastf64x2    `(16 * 4)`($AES_KEYS),$AESKEY1\n";

  $code .= <<___;
        # ;; =================================================
        # ;; GHASH 4 blocks (7 to 4)
        vpclmulqdq        \$0x10,$GHKEY1,$GHDAT1,$GH3M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY1,$GHDAT1,$GH3T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY1,$GHDAT1,$GH3H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY1,$GHDAT1,$GH3L      # ; a0*b0
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES rounds 3
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
  $code .= "vbroadcastf64x2    `(16 * 5)`($AES_KEYS),$AESKEY2\n";

  $code .= <<___;
        # ;; =================================================
        # ;; Gather (XOR) GHASH for 12 blocks
        vpternlogq        \$0x96,$GH3H,$GH2H,$GH1H
        vpternlogq        \$0x96,$GH3L,$GH2L,$GH1L
        vpternlogq        \$0x96,$GH3T,$GH2T,$GH1T
        vpternlogq        \$0x96,$GH3M,$GH2M,$GH1M
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES rounds 4
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
  $code .= "vbroadcastf64x2    `(16 * 6)`($AES_KEYS),$AESKEY1\n";

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; load plain/cipher text
  &ZMM_LOAD_MASKED_BLOCKS_0_16($NUM_BLOCKS, $PLAIN_CIPH_IN, $DATA_OFFSET, $DATA1, $DATA2, $DATA3, $DATA4, $MASKREG);

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES rounds 5
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
  $code .= "vbroadcastf64x2    `(16 * 7)`($AES_KEYS),$AESKEY2\n";

  $code .= <<___;
        # ;; =================================================
        # ;; GHASH 4 blocks (3 to 0)
        vpclmulqdq        \$0x10,$GHKEY2,$GHDAT2,$GH2M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY2,$GHDAT2,$GH2T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY2,$GHDAT2,$GH2H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY2,$GHDAT2,$GH2L      # ; a0*b0
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 6
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
  $code .= "vbroadcastf64x2    `(16 * 8)`($AES_KEYS),$AESKEY1\n";

  # ;; =================================================
  # ;; gather GHASH in GH1L (low), GH1H (high), GH1M (mid)
  # ;; - add GH2[MTLH] to GH1[MTLH]
  $code .= "vpternlogq        \$0x96,$GH2T,$GH1T,$GH1M\n";
  if ($do_reduction != 0) {

    if ($is_start != 0) {
      $code .= "vpxorq            $GH2M,$GH1M,$GH1M\n";
    } else {
      $code .= <<___;
        vpternlogq        \$0x96,$GH2H,$TO_REDUCE_H,$GH1H
        vpternlogq        \$0x96,$GH2L,$TO_REDUCE_L,$GH1L
        vpternlogq        \$0x96,$GH2M,$TO_REDUCE_M,$GH1M
___
    }

  } else {

    # ;; Update H/M/L hash sums if not carrying reduction
    if ($is_start != 0) {
      $code .= <<___;
        vpxorq            $GH2H,$GH1H,$TO_REDUCE_H
        vpxorq            $GH2L,$GH1L,$TO_REDUCE_L
        vpxorq            $GH2M,$GH1M,$TO_REDUCE_M
___
    } else {
      $code .= <<___;
        vpternlogq        \$0x96,$GH2H,$GH1H,$TO_REDUCE_H
        vpternlogq        \$0x96,$GH2L,$GH1L,$TO_REDUCE_L
        vpternlogq        \$0x96,$GH2M,$GH1M,$TO_REDUCE_M
___
    }

  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 7
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
  $code .= "vbroadcastf64x2    `(16 * 9)`($AES_KEYS),$AESKEY2\n";

  # ;; =================================================
  # ;; prepare mid sum for adding to high & low
  # ;; load polynomial constant for reduction
  if ($do_reduction != 0) {
    $code .= <<___;
        vpsrldq           \$8,$GH1M,$GH2M
        vpslldq           \$8,$GH1M,$GH1M

        vmovdqa64         POLY2(%rip),@{[XWORD($RED_POLY)]}
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 8
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
  $code .= "vbroadcastf64x2    `(16 * 10)`($AES_KEYS),$AESKEY1\n";

  # ;; =================================================
  # ;; Add mid product to high and low
  if ($do_reduction != 0) {
    if ($is_start != 0) {
      $code .= <<___;
        vpternlogq        \$0x96,$GH2M,$GH2H,$GH1H      # ; TH = TH1 + TH2 + TM>>64
        vpternlogq        \$0x96,$GH1M,$GH2L,$GH1L      # ; TL = TL1 + TL2 + TM<<64
___
    } else {
      $code .= <<___;
        vpxorq            $GH2M,$GH1H,$GH1H      # ; TH = TH1 + TM>>64
        vpxorq            $GH1M,$GH1L,$GH1L      # ; TL = TL1 + TM<<64
___
    }
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 9
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);

  # ;; =================================================
  # ;; horizontal xor of low and high 4x128
  if ($do_reduction != 0) {
    &VHPXORI4x128($GH1H, $GH2H);
    &VHPXORI4x128($GH1L, $GH2L);
  }

  if (($NROUNDS >= 11)) {
    $code .= "vbroadcastf64x2    `(16 * 11)`($AES_KEYS),$AESKEY2\n";
  }

  # ;; =================================================
  # ;; first phase of reduction
  if ($do_reduction != 0) {
    $code .= <<___;
        vpclmulqdq        \$0x01,@{[XWORD($GH1L)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_P1)]}
        vpslldq           \$8,@{[XWORD($RED_P1)]},@{[XWORD($RED_P1)]}                    # ; shift-L 2 DWs
        vpxorq            @{[XWORD($RED_P1)]},@{[XWORD($GH1L)]},@{[XWORD($RED_P1)]}      # ; first phase of the reduct
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES rounds up to 11 (AES192) or 13 (AES256)
  # ;; AES128 is done
  if (($NROUNDS >= 11)) {
    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
      $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
    $code .= "vbroadcastf64x2    `(16 * 12)`($AES_KEYS),$AESKEY1\n";

    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
      $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
    if (($NROUNDS == 13)) {
      $code .= "vbroadcastf64x2    `(16 * 13)`($AES_KEYS),$AESKEY2\n";

      &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
        $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
        $B04_07,     $B08_11,   $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);
      $code .= "vbroadcastf64x2    `(16 * 14)`($AES_KEYS),$AESKEY1\n";

      &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
        $NUM_BLOCKS, "vaesenc", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
        $B04_07,     $B08_11,   $B12_15, $AESKEY2, $AESKEY2, $AESKEY2, $AESKEY2);
    }
  }

  # ;; =================================================
  # ;; second phase of the reduction
  if ($do_reduction != 0) {
    $code .= <<___;
        vpclmulqdq        \$0x00,@{[XWORD($RED_P1)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_T1)]}
        vpsrldq           \$4,@{[XWORD($RED_T1)]},@{[XWORD($RED_T1)]}      # ; shift-R 1-DW to obtain 2-DWs shift-R
        vpclmulqdq        \$0x10,@{[XWORD($RED_P1)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_T2)]}
        vpslldq           \$4,@{[XWORD($RED_T2)]},@{[XWORD($RED_T2)]}      # ; shift-L 1-DW for result without shifts
        # ;; GH1H = GH1H + RED_T1 + RED_T2
        vpternlogq        \$0x96,@{[XWORD($RED_T1)]},@{[XWORD($RED_T2)]},@{[XWORD($GH1H)]}
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; the last AES round
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vaesenclast", $B00_03, $B04_07,  $B08_11,  $B12_15,  $B00_03,
    $B04_07,     $B08_11,       $B12_15, $AESKEY1, $AESKEY1, $AESKEY1, $AESKEY1);

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; XOR against plain/cipher text
  &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
    $NUM_BLOCKS, "vpxorq", $B00_03, $B04_07, $B08_11, $B12_15, $B00_03,
    $B04_07,     $B08_11,  $B12_15, $DATA1,  $DATA2,  $DATA3,  $DATA4);

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; retrieve the last cipher counter block (partially XOR'ed with text)
  # ;; - this is needed for partial block cases
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 1)`,$B00_03,@{[XWORD($LAST_CIPHER_BLK)]}\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 5)`,$B04_07,@{[XWORD($LAST_CIPHER_BLK)]}\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 9)`,$B08_11,@{[XWORD($LAST_CIPHER_BLK)]}\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS - 13)`,$B12_15,@{[XWORD($LAST_CIPHER_BLK)]}\n";
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; store cipher/plain text
  $code .= "mov       $CIPH_PLAIN_OUT,$IA0\n";
  &ZMM_STORE_MASKED_BLOCKS_0_16($NUM_BLOCKS, $IA0, $DATA_OFFSET, $B00_03, $B04_07, $B08_11, $B12_15, $MASKREG);

  # ;; =================================================
  # ;; shuffle cipher text blocks for GHASH computation
  if ($ENC_DEC eq "ENC") {

    # ;; zero bytes outside the mask before hashing
    if ($NUM_BLOCKS <= 4) {
      $code .= "vmovdqu8           $B00_03,${B00_03}{$MASKREG}{z}\n";
    } elsif ($NUM_BLOCKS <= 8) {
      $code .= "vmovdqu8          $B04_07,${B04_07}{$MASKREG}{z}\n";
    } elsif ($NUM_BLOCKS <= 12) {
      $code .= "vmovdqu8          $B08_11,${B08_11}{$MASKREG}{z}\n";
    } else {
      $code .= "vmovdqu8          $B12_15,${B12_15}{$MASKREG}{z}\n";
    }

    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vpshufb", $DATA1,  $DATA2,  $DATA3,  $DATA4,  $B00_03,
      $B04_07,     $B08_11,   $B12_15, $SHFMSK, $SHFMSK, $SHFMSK, $SHFMSK);
  } else {

    # ;; zero bytes outside the mask before hashing
    if ($NUM_BLOCKS <= 4) {
      $code .= "vmovdqu8          $DATA1,${DATA1}{$MASKREG}{z}\n";
    } elsif ($NUM_BLOCKS <= 8) {
      $code .= "vmovdqu8          $DATA2,${DATA2}{$MASKREG}{z}\n";
    } elsif ($NUM_BLOCKS <= 12) {
      $code .= "vmovdqu8          $DATA3,${DATA3}{$MASKREG}{z}\n";
    } else {
      $code .= "vmovdqu8          $DATA4,${DATA4}{$MASKREG}{z}\n";
    }

    &ZMM_OPCODE3_DSTR_SRC1R_SRC2R_BLOCKS_0_16(
      $NUM_BLOCKS, "vpshufb", $DATA1, $DATA2,  $DATA3,  $DATA4,  $DATA1,
      $DATA2,      $DATA3,    $DATA4, $SHFMSK, $SHFMSK, $SHFMSK, $SHFMSK);
  }

  # ;; =================================================
  # ;; Extract the last block for partial / multi_call cases
  if ($NUM_BLOCKS <= 4) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-1)`,$DATA1,@{[XWORD($LAST_GHASH_BLK)]}\n";
  } elsif ($NUM_BLOCKS <= 8) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-5)`,$DATA2,@{[XWORD($LAST_GHASH_BLK)]}\n";
  } elsif ($NUM_BLOCKS <= 12) {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-9)`,$DATA3,@{[XWORD($LAST_GHASH_BLK)]}\n";
  } else {
    $code .= "vextracti32x4     \$`($NUM_BLOCKS-13)`,$DATA4,@{[XWORD($LAST_GHASH_BLK)]}\n";
  }

  if ($do_reduction != 0) {

    # ;; GH1H holds reduced hash value
    # ;; - normally do "vmovdqa64 &XWORD($GH1H), &XWORD($HASH_IN_OUT)"
    # ;; - register rename trick obsoletes the above move
  }

  # ;; =================================================
  # ;; GHASH last N blocks
  # ;; - current hash value in HASH_IN_OUT or
  # ;;   product parts in TO_REDUCE_H/M/L
  # ;; - DATA1-DATA4 include blocks for GHASH

  if ($do_reduction == 0) {
    &INITIAL_BLOCKS_PARTIAL_GHASH(
      $AES_KEYS,            $GCM128_CTX, $LENGTH,                  $NUM_BLOCKS,
      &XWORD($HASH_IN_OUT), $ENC_DEC,    $DATA1,                   $DATA2,
      $DATA3,               $DATA4,      &XWORD($LAST_CIPHER_BLK), &XWORD($LAST_GHASH_BLK),
      $B00_03,              $B04_07,     $B08_11,                  $B12_15,
      $GHDAT1,              $GHDAT2,     $AESKEY1,                 $AESKEY2,
      $GHKEY1,              $PBLOCK_LEN, $TO_REDUCE_H,             $TO_REDUCE_M,
      $TO_REDUCE_L);
  } else {
    &INITIAL_BLOCKS_PARTIAL_GHASH(
      $AES_KEYS,            $GCM128_CTX, $LENGTH,                  $NUM_BLOCKS,
      &XWORD($HASH_IN_OUT), $ENC_DEC,    $DATA1,                   $DATA2,
      $DATA3,               $DATA4,      &XWORD($LAST_CIPHER_BLK), &XWORD($LAST_GHASH_BLK),
      $B00_03,              $B04_07,     $B08_11,                  $B12_15,
      $GHDAT1,              $GHDAT2,     $AESKEY1,                 $AESKEY2,
      $GHKEY1,              $PBLOCK_LEN);
  }
}

# ;; ===========================================================================
# ;; ===========================================================================
# ;; Stitched GHASH of 16 blocks (with reduction) with encryption of N blocks
# ;; followed with GHASH of the N blocks.
sub GCM_ENC_DEC_LAST {
  my $AES_KEYS           = $_[0];     # [in] key pointer
  my $GCM128_CTX         = $_[1];     # [in] context pointer
  my $CIPH_PLAIN_OUT     = $_[2];     # [in] pointer to output buffer
  my $PLAIN_CIPH_IN      = $_[3];     # [in] pointer to input buffer
  my $DATA_OFFSET        = $_[4];     # [in] data offset
  my $LENGTH             = $_[5];     # [in/clobbered] data length
  my $CTR_BE             = $_[6];     # [in/out] ZMM counter blocks (last 4) in big-endian
  my $CTR_CHECK          = $_[7];     # [in/out] GP with 8-bit counter for overflow check
  my $HASHKEY_OFFSET     = $_[8];     # [in] numerical offset for the highest hash key
                                      # (can be register or numerical offset)
  my $GHASHIN_BLK_OFFSET = $_[9];     # [in] numerical offset for GHASH blocks in
  my $SHFMSK             = $_[10];    # [in] ZMM with byte swap mask for pshufb
  my $ZT00               = $_[11];    # [clobbered] temporary ZMM
  my $ZT01               = $_[12];    # [clobbered] temporary ZMM
  my $ZT02               = $_[13];    # [clobbered] temporary ZMM
  my $ZT03               = $_[14];    # [clobbered] temporary ZMM
  my $ZT04               = $_[15];    # [clobbered] temporary ZMM
  my $ZT05               = $_[16];    # [clobbered] temporary ZMM
  my $ZT06               = $_[17];    # [clobbered] temporary ZMM
  my $ZT07               = $_[18];    # [clobbered] temporary ZMM
  my $ZT08               = $_[19];    # [clobbered] temporary ZMM
  my $ZT09               = $_[20];    # [clobbered] temporary ZMM
  my $ZT10               = $_[21];    # [clobbered] temporary ZMM
  my $ZT11               = $_[22];    # [clobbered] temporary ZMM
  my $ZT12               = $_[23];    # [clobbered] temporary ZMM
  my $ZT13               = $_[24];    # [clobbered] temporary ZMM
  my $ZT14               = $_[25];    # [clobbered] temporary ZMM
  my $ZT15               = $_[26];    # [clobbered] temporary ZMM
  my $ZT16               = $_[27];    # [clobbered] temporary ZMM
  my $ZT17               = $_[28];    # [clobbered] temporary ZMM
  my $ZT18               = $_[29];    # [clobbered] temporary ZMM
  my $ZT19               = $_[30];    # [clobbered] temporary ZMM
  my $ZT20               = $_[31];    # [clobbered] temporary ZMM
  my $ZT21               = $_[32];    # [clobbered] temporary ZMM
  my $ZT22               = $_[33];    # [clobbered] temporary ZMM
  my $ADDBE_4x4          = $_[34];    # [in] ZMM with 4x128bits 4 in big-endian
  my $ADDBE_1234         = $_[35];    # [in] ZMM with 4x128bits 1, 2, 3 and 4 in big-endian
  my $GHASH_TYPE         = $_[36];    # [in] "start", "start_reduce", "mid", "end_reduce"
  my $TO_REDUCE_L        = $_[37];    # [in] ZMM for low 4x128-bit GHASH sum
  my $TO_REDUCE_H        = $_[38];    # [in] ZMM for hi 4x128-bit GHASH sum
  my $TO_REDUCE_M        = $_[39];    # [in] ZMM for medium 4x128-bit GHASH sum
  my $ENC_DEC            = $_[40];    # [in] cipher direction
  my $HASH_IN_OUT        = $_[41];    # [in/out] XMM ghash in/out value
  my $IA0                = $_[42];    # [clobbered] GP temporary
  my $IA1                = $_[43];    # [clobbered] GP temporary
  my $MASKREG            = $_[44];    # [clobbered] mask register
  my $PBLOCK_LEN         = $_[45];    # [in] partial block length

  my $label_suffix = $label_count++;

  $code .= <<___;
        mov               @{[DWORD($LENGTH)]},@{[DWORD($IA0)]}
        add               \$15,@{[DWORD($IA0)]}
        shr               \$4,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_0_${label_suffix}

        cmp               \$8,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_8_${label_suffix}
        jb                .L_last_num_blocks_is_7_1_${label_suffix}


        cmp               \$12,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_12_${label_suffix}
        jb                .L_last_num_blocks_is_11_9_${label_suffix}

        # ;; 16, 15, 14 or 13
        cmp               \$15,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_15_${label_suffix}
        ja                .L_last_num_blocks_is_16_${label_suffix}
        cmp               \$14,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_14_${label_suffix}
        jmp               .L_last_num_blocks_is_13_${label_suffix}

.L_last_num_blocks_is_11_9_${label_suffix}:
        # ;; 11, 10 or 9
        cmp               \$10,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_10_${label_suffix}
        ja                .L_last_num_blocks_is_11_${label_suffix}
        jmp               .L_last_num_blocks_is_9_${label_suffix}

.L_last_num_blocks_is_7_1_${label_suffix}:
        cmp               \$4,@{[DWORD($IA0)]}
        je                .L_last_num_blocks_is_4_${label_suffix}
        jb                .L_last_num_blocks_is_3_1_${label_suffix}
        # ;; 7, 6 or 5
        cmp               \$6,@{[DWORD($IA0)]}
        ja                .L_last_num_blocks_is_7_${label_suffix}
        je                .L_last_num_blocks_is_6_${label_suffix}
        jmp               .L_last_num_blocks_is_5_${label_suffix}

.L_last_num_blocks_is_3_1_${label_suffix}:
        # ;; 3, 2 or 1
        cmp               \$2,@{[DWORD($IA0)]}
        ja                .L_last_num_blocks_is_3_${label_suffix}
        je                .L_last_num_blocks_is_2_${label_suffix}
___

  # ;; fall through for `jmp .L_last_num_blocks_is_1`

  # ;; Use rep to generate different block size variants
  # ;; - one block size has to be the first one
  for my $num_blocks (1 .. 16) {
    $code .= ".L_last_num_blocks_is_${num_blocks}_${label_suffix}:\n";
    &GHASH_16_ENCRYPT_N_GHASH_N(
      $AES_KEYS,   $GCM128_CTX,  $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET,
      $LENGTH,     $CTR_BE,      $CTR_CHECK,      $HASHKEY_OFFSET, $GHASHIN_BLK_OFFSET,
      $SHFMSK,     $ZT00,        $ZT01,           $ZT02,           $ZT03,
      $ZT04,       $ZT05,        $ZT06,           $ZT07,           $ZT08,
      $ZT09,       $ZT10,        $ZT11,           $ZT12,           $ZT13,
      $ZT14,       $ZT15,        $ZT16,           $ZT17,           $ZT18,
      $ZT19,       $ZT20,        $ZT21,           $ZT22,           $ADDBE_4x4,
      $ADDBE_1234, $GHASH_TYPE,  $TO_REDUCE_L,    $TO_REDUCE_H,    $TO_REDUCE_M,
      $ENC_DEC,    $HASH_IN_OUT, $IA0,            $IA1,            $MASKREG,
      $num_blocks, $PBLOCK_LEN);

    $code .= "jmp           .L_last_blocks_done_${label_suffix}\n";
  }

  $code .= ".L_last_num_blocks_is_0_${label_suffix}:\n";

  # ;; if there is 0 blocks to cipher then there are only 16 blocks for ghash and reduction
  # ;; - convert mid into end_reduce
  # ;; - convert start into start_reduce
  if ($GHASH_TYPE eq "mid") {
    $GHASH_TYPE = "end_reduce";
  }
  if ($GHASH_TYPE eq "start") {
    $GHASH_TYPE = "start_reduce";
  }

  &GHASH_16($GHASH_TYPE, $TO_REDUCE_H, $TO_REDUCE_M, $TO_REDUCE_L, "%rsp",
    $GHASHIN_BLK_OFFSET, 0, "%rsp", $HASHKEY_OFFSET, 0, $HASH_IN_OUT, $ZT00, $ZT01,
    $ZT02, $ZT03, $ZT04, $ZT05, $ZT06, $ZT07, $ZT08, $ZT09);

  $code .= ".L_last_blocks_done_${label_suffix}:\n";
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;; Main GCM macro stitching cipher with GHASH
# ;; - operates on single stream
# ;; - encrypts 16 blocks at a time
# ;; - ghash the 16 previously encrypted ciphertext blocks
# ;; - no partial block or multi_call handling here
sub GHASH_16_ENCRYPT_16_PARALLEL {
  my $AES_KEYS           = $_[0];     # [in] key pointer
  my $CIPH_PLAIN_OUT     = $_[1];     # [in] pointer to output buffer
  my $PLAIN_CIPH_IN      = $_[2];     # [in] pointer to input buffer
  my $DATA_OFFSET        = $_[3];     # [in] data offset
  my $CTR_BE             = $_[4];     # [in/out] ZMM counter blocks (last 4) in big-endian
  my $CTR_CHECK          = $_[5];     # [in/out] GP with 8-bit counter for overflow check
  my $HASHKEY_OFFSET     = $_[6];     # [in] numerical offset for the highest hash key (hash key index value)
  my $AESOUT_BLK_OFFSET  = $_[7];     # [in] numerical offset for AES-CTR out
  my $GHASHIN_BLK_OFFSET = $_[8];     # [in] numerical offset for GHASH blocks in
  my $SHFMSK             = $_[9];     # [in] ZMM with byte swap mask for pshufb
  my $ZT1                = $_[10];    # [clobbered] temporary ZMM (cipher)
  my $ZT2                = $_[11];    # [clobbered] temporary ZMM (cipher)
  my $ZT3                = $_[12];    # [clobbered] temporary ZMM (cipher)
  my $ZT4                = $_[13];    # [clobbered] temporary ZMM (cipher)
  my $ZT5                = $_[14];    # [clobbered/out] temporary ZMM or GHASH OUT (final_reduction)
  my $ZT6                = $_[15];    # [clobbered] temporary ZMM (cipher)
  my $ZT7                = $_[16];    # [clobbered] temporary ZMM (cipher)
  my $ZT8                = $_[17];    # [clobbered] temporary ZMM (cipher)
  my $ZT9                = $_[18];    # [clobbered] temporary ZMM (cipher)
  my $ZT10               = $_[19];    # [clobbered] temporary ZMM (ghash)
  my $ZT11               = $_[20];    # [clobbered] temporary ZMM (ghash)
  my $ZT12               = $_[21];    # [clobbered] temporary ZMM (ghash)
  my $ZT13               = $_[22];    # [clobbered] temporary ZMM (ghash)
  my $ZT14               = $_[23];    # [clobbered] temporary ZMM (ghash)
  my $ZT15               = $_[24];    # [clobbered] temporary ZMM (ghash)
  my $ZT16               = $_[25];    # [clobbered] temporary ZMM (ghash)
  my $ZT17               = $_[26];    # [clobbered] temporary ZMM (ghash)
  my $ZT18               = $_[27];    # [clobbered] temporary ZMM (ghash)
  my $ZT19               = $_[28];    # [clobbered] temporary ZMM
  my $ZT20               = $_[29];    # [clobbered] temporary ZMM
  my $ZT21               = $_[30];    # [clobbered] temporary ZMM
  my $ZT22               = $_[31];    # [clobbered] temporary ZMM
  my $ZT23               = $_[32];    # [clobbered] temporary ZMM
  my $ADDBE_4x4          = $_[33];    # [in] ZMM with 4x128bits 4 in big-endian
  my $ADDBE_1234         = $_[34];    # [in] ZMM with 4x128bits 1, 2, 3 and 4 in big-endian
  my $TO_REDUCE_L        = $_[35];    # [in/out] ZMM for low 4x128-bit GHASH sum
  my $TO_REDUCE_H        = $_[36];    # [in/out] ZMM for hi 4x128-bit GHASH sum
  my $TO_REDUCE_M        = $_[37];    # [in/out] ZMM for medium 4x128-bit GHASH sum
  my $DO_REDUCTION       = $_[38];    # [in] "no_reduction", "final_reduction", "first_time"
  my $ENC_DEC            = $_[39];    # [in] cipher direction
  my $DATA_DISPL         = $_[40];    # [in] fixed numerical data displacement/offset
  my $GHASH_IN           = $_[41];    # [in] current GHASH value or "no_ghash_in"
  my $IA0                = $_[42];    # [clobbered] temporary GPR

  my $B00_03 = $ZT1;
  my $B04_07 = $ZT2;
  my $B08_11 = $ZT3;
  my $B12_15 = $ZT4;

  my $GH1H = $ZT5;

  # ; @note: do not change this mapping
  my $GH1L = $ZT6;
  my $GH1M = $ZT7;
  my $GH1T = $ZT8;

  my $GH2H = $ZT9;
  my $GH2L = $ZT10;
  my $GH2M = $ZT11;
  my $GH2T = $ZT12;

  my $RED_POLY = $GH2T;
  my $RED_P1   = $GH2L;
  my $RED_T1   = $GH2H;
  my $RED_T2   = $GH2M;

  my $GH3H = $ZT13;
  my $GH3L = $ZT14;
  my $GH3M = $ZT15;
  my $GH3T = $ZT16;

  my $DATA1 = $ZT13;
  my $DATA2 = $ZT14;
  my $DATA3 = $ZT15;
  my $DATA4 = $ZT16;

  my $AESKEY1 = $ZT17;
  my $AESKEY2 = $ZT18;

  my $GHKEY1 = $ZT19;
  my $GHKEY2 = $ZT20;
  my $GHDAT1 = $ZT21;
  my $GHDAT2 = $ZT22;

  my $label_suffix = $label_count++;

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; prepare counter blocks

  $code .= <<___;
        cmpb              \$`(256 - 16)`,@{[BYTE($CTR_CHECK)]}
        jae               .L_16_blocks_overflow_${label_suffix}
        vpaddd            $ADDBE_1234,$CTR_BE,$B00_03
        vpaddd            $ADDBE_4x4,$B00_03,$B04_07
        vpaddd            $ADDBE_4x4,$B04_07,$B08_11
        vpaddd            $ADDBE_4x4,$B08_11,$B12_15
        jmp               .L_16_blocks_ok_${label_suffix}
.L_16_blocks_overflow_${label_suffix}:
        vpshufb           $SHFMSK,$CTR_BE,$CTR_BE
        vmovdqa64         ddq_add_4444(%rip),$B12_15
        vpaddd            ddq_add_1234(%rip),$CTR_BE,$B00_03
        vpaddd            $B12_15,$B00_03,$B04_07
        vpaddd            $B12_15,$B04_07,$B08_11
        vpaddd            $B12_15,$B08_11,$B12_15
        vpshufb           $SHFMSK,$B00_03,$B00_03
        vpshufb           $SHFMSK,$B04_07,$B04_07
        vpshufb           $SHFMSK,$B08_11,$B08_11
        vpshufb           $SHFMSK,$B12_15,$B12_15
.L_16_blocks_ok_${label_suffix}:
___

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; pre-load constants
  $code .= "vbroadcastf64x2    `(16 * 0)`($AES_KEYS),$AESKEY1\n";
  if ($GHASH_IN ne "no_ghash_in") {
    $code .= "vpxorq            `$GHASHIN_BLK_OFFSET + (0*64)`(%rsp),$GHASH_IN,$GHDAT1\n";
  } else {
    $code .= "vmovdqa64         `$GHASHIN_BLK_OFFSET + (0*64)`(%rsp),$GHDAT1\n";
  }

  $code .= <<___;
        vmovdqu64         @{[HashKeyByIdx(($HASHKEY_OFFSET - (0*4)),"%rsp")]},$GHKEY1

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; save counter for the next round
        # ;; increment counter overflow check register
        vshufi64x2        \$0b11111111,$B12_15,$B12_15,$CTR_BE
        addb              \$16,@{[BYTE($CTR_CHECK)]}
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; pre-load constants
        vbroadcastf64x2    `(16 * 1)`($AES_KEYS),$AESKEY2
        vmovdqu64         @{[HashKeyByIdx(($HASHKEY_OFFSET - (1*4)),"%rsp")]},$GHKEY2
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (1*64)`(%rsp),$GHDAT2

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; stitch AES rounds with GHASH

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES round 0 - ARK

        vpxorq            $AESKEY1,$B00_03,$B00_03
        vpxorq            $AESKEY1,$B04_07,$B04_07
        vpxorq            $AESKEY1,$B08_11,$B08_11
        vpxorq            $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 2)`($AES_KEYS),$AESKEY1

        # ;;==================================================
        # ;; GHASH 4 blocks (15 to 12)
        vpclmulqdq        \$0x11,$GHKEY1,$GHDAT1,$GH1H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY1,$GHDAT1,$GH1L      # ; a0*b0
        vpclmulqdq        \$0x01,$GHKEY1,$GHDAT1,$GH1M      # ; a1*b0
        vpclmulqdq        \$0x10,$GHKEY1,$GHDAT1,$GH1T      # ; a0*b1
        vmovdqu64         @{[HashKeyByIdx(($HASHKEY_OFFSET - (2*4)),"%rsp")]},$GHKEY1
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (2*64)`(%rsp),$GHDAT1

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES round 1
        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 3)`($AES_KEYS),$AESKEY2

        # ;; =================================================
        # ;; GHASH 4 blocks (11 to 8)
        vpclmulqdq        \$0x10,$GHKEY2,$GHDAT2,$GH2M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY2,$GHDAT2,$GH2T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY2,$GHDAT2,$GH2H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY2,$GHDAT2,$GH2L      # ; a0*b0
        vmovdqu64         @{[HashKeyByIdx(($HASHKEY_OFFSET - (3*4)),"%rsp")]},$GHKEY2
        vmovdqa64         `$GHASHIN_BLK_OFFSET + (3*64)`(%rsp),$GHDAT2

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES round 2
        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 4)`($AES_KEYS),$AESKEY1

        # ;; =================================================
        # ;; GHASH 4 blocks (7 to 4)
        vpclmulqdq        \$0x10,$GHKEY1,$GHDAT1,$GH3M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY1,$GHDAT1,$GH3T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY1,$GHDAT1,$GH3H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY1,$GHDAT1,$GH3L      # ; a0*b0
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES rounds 3
        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 5)`($AES_KEYS),$AESKEY2

        # ;; =================================================
        # ;; Gather (XOR) GHASH for 12 blocks
        vpternlogq        \$0x96,$GH3H,$GH2H,$GH1H
        vpternlogq        \$0x96,$GH3L,$GH2L,$GH1L
        vpternlogq        \$0x96,$GH3T,$GH2T,$GH1T
        vpternlogq        \$0x96,$GH3M,$GH2M,$GH1M

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES rounds 4
        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 6)`($AES_KEYS),$AESKEY1

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; load plain/cipher text (recycle GH3xx registers)
        vmovdqu8          `$DATA_DISPL + (0 * 64)`($PLAIN_CIPH_IN,$DATA_OFFSET),$DATA1
        vmovdqu8          `$DATA_DISPL + (1 * 64)`($PLAIN_CIPH_IN,$DATA_OFFSET),$DATA2
        vmovdqu8          `$DATA_DISPL + (2 * 64)`($PLAIN_CIPH_IN,$DATA_OFFSET),$DATA3
        vmovdqu8          `$DATA_DISPL + (3 * 64)`($PLAIN_CIPH_IN,$DATA_OFFSET),$DATA4

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES rounds 5
        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 7)`($AES_KEYS),$AESKEY2

        # ;; =================================================
        # ;; GHASH 4 blocks (3 to 0)
        vpclmulqdq        \$0x10,$GHKEY2,$GHDAT2,$GH2M      # ; a0*b1
        vpclmulqdq        \$0x01,$GHKEY2,$GHDAT2,$GH2T      # ; a1*b0
        vpclmulqdq        \$0x11,$GHKEY2,$GHDAT2,$GH2H      # ; a1*b1
        vpclmulqdq        \$0x00,$GHKEY2,$GHDAT2,$GH2L      # ; a0*b0
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; AES round 6
        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 8)`($AES_KEYS),$AESKEY1
___

  # ;; =================================================
  # ;; gather GHASH in GH1L (low) and GH1H (high)
  if ($DO_REDUCTION eq "first_time") {
    $code .= <<___;
        vpternlogq        \$0x96,$GH2T,$GH1T,$GH1M      # ; TM
        vpxorq            $GH2M,$GH1M,$TO_REDUCE_M      # ; TM
        vpxorq            $GH2H,$GH1H,$TO_REDUCE_H      # ; TH
        vpxorq            $GH2L,$GH1L,$TO_REDUCE_L      # ; TL
___
  }
  if ($DO_REDUCTION eq "no_reduction") {
    $code .= <<___;
        vpternlogq        \$0x96,$GH2T,$GH1T,$GH1M             # ; TM
        vpternlogq        \$0x96,$GH2M,$GH1M,$TO_REDUCE_M      # ; TM
        vpternlogq        \$0x96,$GH2H,$GH1H,$TO_REDUCE_H      # ; TH
        vpternlogq        \$0x96,$GH2L,$GH1L,$TO_REDUCE_L      # ; TL
___
  }
  if ($DO_REDUCTION eq "final_reduction") {
    $code .= <<___;
        # ;; phase 1: add mid products together
        # ;; also load polynomial constant for reduction
        vpternlogq        \$0x96,$GH2T,$GH1T,$GH1M      # ; TM
        vpternlogq        \$0x96,$GH2M,$TO_REDUCE_M,$GH1M

        vpsrldq           \$8,$GH1M,$GH2M
        vpslldq           \$8,$GH1M,$GH1M

        vmovdqa64         POLY2(%rip),@{[XWORD($RED_POLY)]}
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 7
  $code .= <<___;
        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 9)`($AES_KEYS),$AESKEY2
___

  # ;; =================================================
  # ;; Add mid product to high and low
  if ($DO_REDUCTION eq "final_reduction") {
    $code .= <<___;
        vpternlogq        \$0x96,$GH2M,$GH2H,$GH1H      # ; TH = TH1 + TH2 + TM>>64
        vpxorq            $TO_REDUCE_H,$GH1H,$GH1H
        vpternlogq        \$0x96,$GH1M,$GH2L,$GH1L      # ; TL = TL1 + TL2 + TM<<64
        vpxorq            $TO_REDUCE_L,$GH1L,$GH1L
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 8
  $code .= <<___;
        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 10)`($AES_KEYS),$AESKEY1
___

  # ;; =================================================
  # ;; horizontal xor of low and high 4x128
  if ($DO_REDUCTION eq "final_reduction") {
    &VHPXORI4x128($GH1H, $GH2H);
    &VHPXORI4x128($GH1L, $GH2L);
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES round 9
  $code .= <<___;
        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
___
  if (($NROUNDS >= 11)) {
    $code .= "vbroadcastf64x2    `(16 * 11)`($AES_KEYS),$AESKEY2\n";
  }

  # ;; =================================================
  # ;; first phase of reduction
  if ($DO_REDUCTION eq "final_reduction") {
    $code .= <<___;
        vpclmulqdq        \$0x01,@{[XWORD($GH1L)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_P1)]}
        vpslldq           \$8,@{[XWORD($RED_P1)]},@{[XWORD($RED_P1)]}                    # ; shift-L 2 DWs
        vpxorq            @{[XWORD($RED_P1)]},@{[XWORD($GH1L)]},@{[XWORD($RED_P1)]}      # ; first phase of the reduct
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; AES rounds up to 11 (AES192) or 13 (AES256)
  # ;; AES128 is done
  if (($NROUNDS >= 11)) {
    $code .= <<___;
        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 12)`($AES_KEYS),$AESKEY1

        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
___
    if (($NROUNDS == 13)) {
      $code .= <<___;
        vbroadcastf64x2    `(16 * 13)`($AES_KEYS),$AESKEY2

        vaesenc           $AESKEY1,$B00_03,$B00_03
        vaesenc           $AESKEY1,$B04_07,$B04_07
        vaesenc           $AESKEY1,$B08_11,$B08_11
        vaesenc           $AESKEY1,$B12_15,$B12_15
        vbroadcastf64x2    `(16 * 14)`($AES_KEYS),$AESKEY1

        vaesenc           $AESKEY2,$B00_03,$B00_03
        vaesenc           $AESKEY2,$B04_07,$B04_07
        vaesenc           $AESKEY2,$B08_11,$B08_11
        vaesenc           $AESKEY2,$B12_15,$B12_15
___
    }
  }

  # ;; =================================================
  # ;; second phase of the reduction
  if ($DO_REDUCTION eq "final_reduction") {
    $code .= <<___;
        vpclmulqdq        \$0x00,@{[XWORD($RED_P1)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_T1)]}
        vpsrldq           \$4,@{[XWORD($RED_T1)]},@{[XWORD($RED_T1)]}      # ; shift-R 1-DW to obtain 2-DWs shift-R
        vpclmulqdq        \$0x10,@{[XWORD($RED_P1)]},@{[XWORD($RED_POLY)]},@{[XWORD($RED_T2)]}
        vpslldq           \$4,@{[XWORD($RED_T2)]},@{[XWORD($RED_T2)]}      # ; shift-L 1-DW for result without shifts
        # ;; GH1H = GH1H x RED_T1 x RED_T2
        vpternlogq        \$0x96,@{[XWORD($RED_T1)]},@{[XWORD($RED_T2)]},@{[XWORD($GH1H)]}
___
  }

  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;; the last AES round
  $code .= <<___;
        vaesenclast       $AESKEY1,$B00_03,$B00_03
        vaesenclast       $AESKEY1,$B04_07,$B04_07
        vaesenclast       $AESKEY1,$B08_11,$B08_11
        vaesenclast       $AESKEY1,$B12_15,$B12_15

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; XOR against plain/cipher text
        vpxorq            $DATA1,$B00_03,$B00_03
        vpxorq            $DATA2,$B04_07,$B04_07
        vpxorq            $DATA3,$B08_11,$B08_11
        vpxorq            $DATA4,$B12_15,$B12_15

        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; store cipher/plain text
        mov               $CIPH_PLAIN_OUT,$IA0
        vmovdqu8          $B00_03,`$DATA_DISPL + (0 * 64)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B04_07,`$DATA_DISPL + (1 * 64)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B08_11,`$DATA_DISPL + (2 * 64)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B12_15,`$DATA_DISPL + (3 * 64)`($IA0,$DATA_OFFSET,1)
___

  # ;; =================================================
  # ;; shuffle cipher text blocks for GHASH computation
  if ($ENC_DEC eq "ENC") {
    $code .= <<___;
        vpshufb           $SHFMSK,$B00_03,$B00_03
        vpshufb           $SHFMSK,$B04_07,$B04_07
        vpshufb           $SHFMSK,$B08_11,$B08_11
        vpshufb           $SHFMSK,$B12_15,$B12_15
___
  } else {
    $code .= <<___;
        vpshufb           $SHFMSK,$DATA1,$B00_03
        vpshufb           $SHFMSK,$DATA2,$B04_07
        vpshufb           $SHFMSK,$DATA3,$B08_11
        vpshufb           $SHFMSK,$DATA4,$B12_15
___
  }

  # ;; =================================================
  # ;; store shuffled cipher text for ghashing
  $code .= <<___;
        vmovdqa64         $B00_03,`$AESOUT_BLK_OFFSET + (0*64)`(%rsp)
        vmovdqa64         $B04_07,`$AESOUT_BLK_OFFSET + (1*64)`(%rsp)
        vmovdqa64         $B08_11,`$AESOUT_BLK_OFFSET + (2*64)`(%rsp)
        vmovdqa64         $B12_15,`$AESOUT_BLK_OFFSET + (3*64)`(%rsp)
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Encryption of a single block
sub ENCRYPT_SINGLE_BLOCK {
  my $AES_KEY = $_[0];    # ; [in]
  my $XMM0    = $_[1];    # ; [in/out]
  my $GPR1    = $_[2];    # ; [clobbered]

  my $label_suffix = $label_count++;

  $code .= <<___;
        # ; load number of rounds from AES_KEY structure (offset in bytes is
        # ; size of the |rd_key| buffer)
        mov             `4*15*4`($AES_KEY),@{[DWORD($GPR1)]}
        cmp             \$9,@{[DWORD($GPR1)]}
        je              .Laes_128_${label_suffix}
        cmp             \$11,@{[DWORD($GPR1)]}
        je              .Laes_192_${label_suffix}
        cmp             \$13,@{[DWORD($GPR1)]}
        je              .Laes_256_${label_suffix}
        jmp             .Lexit_aes_${label_suffix}
___
  for my $keylen (sort keys %aes_rounds) {
    my $nr = $aes_rounds{$keylen};
    $code .= <<___;
.align 32
.Laes_${keylen}_${label_suffix}:
___
    $code .= "vpxorq          `16*0`($AES_KEY),$XMM0, $XMM0\n\n";
    for (my $i = 1; $i <= $nr; $i++) {
      $code .= "vaesenc         `16*$i`($AES_KEY),$XMM0,$XMM0\n\n";
    }
    $code .= <<___;
        vaesenclast     `16*($nr+1)`($AES_KEY),$XMM0,$XMM0
        jmp .Lexit_aes_${label_suffix}
___
  }
  $code .= ".Lexit_aes_${label_suffix}:\n\n";
}

sub CALC_J0 {
  my $GCM128_CTX = $_[0];     #; [in] Pointer to GCM context
  my $IV         = $_[1];     #; [in] Pointer to IV
  my $IV_LEN     = $_[2];     #; [in] IV length
  my $J0         = $_[3];     #; [out] XMM reg to contain J0
  my $ZT0        = $_[4];     #; [clobbered] ZMM register
  my $ZT1        = $_[5];     #; [clobbered] ZMM register
  my $ZT2        = $_[6];     #; [clobbered] ZMM register
  my $ZT3        = $_[7];     #; [clobbered] ZMM register
  my $ZT4        = $_[8];     #; [clobbered] ZMM register
  my $ZT5        = $_[9];     #; [clobbered] ZMM register
  my $ZT6        = $_[10];    #; [clobbered] ZMM register
  my $ZT7        = $_[11];    #; [clobbered] ZMM register
  my $ZT8        = $_[12];    #; [clobbered] ZMM register
  my $ZT9        = $_[13];    #; [clobbered] ZMM register
  my $ZT10       = $_[14];    #; [clobbered] ZMM register
  my $ZT11       = $_[15];    #; [clobbered] ZMM register
  my $ZT12       = $_[16];    #; [clobbered] ZMM register
  my $ZT13       = $_[17];    #; [clobbered] ZMM register
  my $ZT14       = $_[18];    #; [clobbered] ZMM register
  my $ZT15       = $_[19];    #; [clobbered] ZMM register
  my $ZT16       = $_[20];    #; [clobbered] ZMM register
  my $T1         = $_[21];    #; [clobbered] GP register
  my $T2         = $_[22];    #; [clobbered] GP register
  my $T3         = $_[23];    #; [clobbered] GP register
  my $MASKREG    = $_[24];    #; [clobbered] mask register

  # ;; J0 = GHASH(IV || 0s+64 || len(IV)64)
  # ;; s = 16 * RoundUp(len(IV)/16) -  len(IV) */

  # ;; Calculate GHASH of (IV || 0s)
  $code .= "vpxor             $J0,$J0,$J0\n";
  &CALC_AAD_HASH($IV, $IV_LEN, $J0, $GCM128_CTX, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4,
    $ZT5, $ZT6, $ZT7, $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, $ZT13, $ZT14, $ZT15, $ZT16, $T1, $T2, $T3, $MASKREG);

  # ;; Calculate GHASH of last 16-byte block (0 || len(IV)64)
  $code .= <<___;
        mov               $IV_LEN,$T1
        shl               \$3,$T1      # ; IV length in bits
        vmovq             $T1,@{[XWORD($ZT2)]}

        # ;; Might need shuffle of ZT2
        vpxorq            $J0,@{[XWORD($ZT2)]},$J0

        vmovdqu64         @{[HashKeyByIdx(1,$GCM128_CTX)]},@{[XWORD($ZT0)]}
___
  &GHASH_MUL($J0, @{[XWORD($ZT0)]}, @{[XWORD($ZT1)]}, @{[XWORD($ZT2)]}, @{[XWORD($ZT3)]});

  $code .= "vpshufb           SHUF_MASK(%rip),$J0,$J0      # ; perform a 16Byte swap\n";
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; GCM_INIT_IV performs an initialization of gcm128_ctx struct to prepare for
# ;;; encoding/decoding.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub GCM_INIT_IV {
  my $AES_KEYS   = $_[0];     # [in] AES key schedule
  my $GCM128_CTX = $_[1];     # [in/out] GCM context
  my $IV         = $_[2];     # [in] IV pointer
  my $IV_LEN     = $_[3];     # [in] IV length
  my $GPR1       = $_[4];     # [clobbered] GP register
  my $GPR2       = $_[5];     # [clobbered] GP register
  my $GPR3       = $_[6];     # [clobbered] GP register
  my $MASKREG    = $_[7];     # [clobbered] mask register
  my $CUR_COUNT  = $_[8];     # [out] XMM with current counter
  my $ZT0        = $_[9];     # [clobbered] ZMM register
  my $ZT1        = $_[10];    # [clobbered] ZMM register
  my $ZT2        = $_[11];    # [clobbered] ZMM register
  my $ZT3        = $_[12];    # [clobbered] ZMM register
  my $ZT4        = $_[13];    # [clobbered] ZMM register
  my $ZT5        = $_[14];    # [clobbered] ZMM register
  my $ZT6        = $_[15];    # [clobbered] ZMM register
  my $ZT7        = $_[16];    # [clobbered] ZMM register
  my $ZT8        = $_[17];    # [clobbered] ZMM register
  my $ZT9        = $_[18];    # [clobbered] ZMM register
  my $ZT10       = $_[19];    # [clobbered] ZMM register
  my $ZT11       = $_[20];    # [clobbered] ZMM register
  my $ZT12       = $_[21];    # [clobbered] ZMM register
  my $ZT13       = $_[22];    # [clobbered] ZMM register
  my $ZT14       = $_[23];    # [clobbered] ZMM register
  my $ZT15       = $_[24];    # [clobbered] ZMM register
  my $ZT16       = $_[25];    # [clobbered] ZMM register

  my $ZT0x = $ZT0;
  $ZT0x =~ s/zmm/xmm/;

  $code .= <<___;
        cmp     \$12,$IV_LEN
        je      iv_len_12_init_IV
___

  # ;; IV is different than 12 bytes
  &CALC_J0($GCM128_CTX, $IV, $IV_LEN, $CUR_COUNT, $ZT0, $ZT1, $ZT2, $ZT3, $ZT4, $ZT5, $ZT6, $ZT7,
    $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, $ZT13, $ZT14, $ZT15, $ZT16, $GPR1, $GPR2, $GPR3, $MASKREG);
  $code .= <<___;
       jmp      skip_iv_len_12_init_IV
iv_len_12_init_IV:   # ;; IV is 12 bytes
        # ;; read 12 IV bytes and pad with 0x00000001
        vmovdqu8          ONEf(%rip),$CUR_COUNT
        mov               $IV,$GPR2
        mov               \$0x0000000000000fff,@{[DWORD($GPR1)]}
        kmovq             $GPR1,$MASKREG
        vmovdqu8          ($GPR2),${CUR_COUNT}{$MASKREG}         # ; ctr = IV | 0x1
skip_iv_len_12_init_IV:
        vmovdqu           $CUR_COUNT,$ZT0x
___
  &ENCRYPT_SINGLE_BLOCK($AES_KEYS, "$ZT0x", "$GPR1");    # ; E(K, Y0)
  $code .= <<___;
        vmovdqu           $ZT0x,`$CTX_OFFSET_EK0`($GCM128_CTX)   # ; save EK0 for finalization stage

        # ;; store IV as counter in LE format
        vpshufb           SHUF_MASK(%rip),$CUR_COUNT,$CUR_COUNT
        vmovdqu           $CUR_COUNT,`$CTX_OFFSET_CurCount`($GCM128_CTX)   # ; save current counter Yi
___
}

sub GCM_UPDATE_AAD {
  my $GCM128_CTX = $_[0];  # [in] GCM context pointer
  my $A_IN       = $_[1];  # [in] AAD pointer
  my $A_LEN      = $_[2];  # [in] AAD length in bytes
  my $GPR1       = $_[3];  # [clobbered] GP register
  my $GPR2       = $_[4];  # [clobbered] GP register
  my $GPR3       = $_[5];  # [clobbered] GP register
  my $MASKREG    = $_[6];  # [clobbered] mask register
  my $AAD_HASH   = $_[7];  # [out] XMM for AAD_HASH value
  my $ZT0        = $_[8];  # [clobbered] ZMM register
  my $ZT1        = $_[9];  # [clobbered] ZMM register
  my $ZT2        = $_[10]; # [clobbered] ZMM register
  my $ZT3        = $_[11]; # [clobbered] ZMM register
  my $ZT4        = $_[12]; # [clobbered] ZMM register
  my $ZT5        = $_[13]; # [clobbered] ZMM register
  my $ZT6        = $_[14]; # [clobbered] ZMM register
  my $ZT7        = $_[15]; # [clobbered] ZMM register
  my $ZT8        = $_[16]; # [clobbered] ZMM register
  my $ZT9        = $_[17]; # [clobbered] ZMM register
  my $ZT10       = $_[18]; # [clobbered] ZMM register
  my $ZT11       = $_[19]; # [clobbered] ZMM register
  my $ZT12       = $_[20]; # [clobbered] ZMM register
  my $ZT13       = $_[21]; # [clobbered] ZMM register
  my $ZT14       = $_[22]; # [clobbered] ZMM register
  my $ZT15       = $_[23]; # [clobbered] ZMM register
  my $ZT16       = $_[24]; # [clobbered] ZMM register

  # ; load current hash
  $code .= "vmovdqu64         $CTX_OFFSET_AadHash($GCM128_CTX),$AAD_HASH\n";

  &CALC_AAD_HASH($A_IN, $A_LEN, $AAD_HASH, $GCM128_CTX, $ZT0, $ZT1, $ZT2,
    $ZT3, $ZT4, $ZT5, $ZT6, $ZT7, $ZT8, $ZT9, $ZT10, $ZT11, $ZT12, $ZT13,
    $ZT14, $ZT15, $ZT16, $GPR1, $GPR2, $GPR3, $MASKREG);

  # ; load current hash
  $code .= "vmovdqu64         $AAD_HASH,$CTX_OFFSET_AadHash($GCM128_CTX)\n";
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Cipher and ghash of payloads shorter than 256 bytes
# ;;; - number of blocks in the message comes as argument
# ;;; - depending on the number of blocks an optimized variant of
# ;;;   INITIAL_BLOCKS_PARTIAL is invoked
sub GCM_ENC_DEC_SMALL {
  my $AES_KEYS       = $_[0];     # [in] key pointer
  my $GCM128_CTX     = $_[1];     # [in] context pointer
  my $CIPH_PLAIN_OUT = $_[2];     # [in] output buffer
  my $PLAIN_CIPH_IN  = $_[3];     # [in] input buffer
  my $PLAIN_CIPH_LEN = $_[4];     # [in] buffer length
  my $ENC_DEC        = $_[5];     # [in] cipher direction
  my $DATA_OFFSET    = $_[6];     # [in] data offset
  my $LENGTH         = $_[7];     # [in] data length
  my $NUM_BLOCKS     = $_[8];     # [in] number of blocks to process 1 to 16
  my $CTR            = $_[9];     # [in/out] XMM counter block
  my $HASH_IN_OUT    = $_[10];    # [in/out] XMM GHASH value
  my $ZTMP0          = $_[11];    # [clobbered] ZMM register
  my $ZTMP1          = $_[12];    # [clobbered] ZMM register
  my $ZTMP2          = $_[13];    # [clobbered] ZMM register
  my $ZTMP3          = $_[14];    # [clobbered] ZMM register
  my $ZTMP4          = $_[15];    # [clobbered] ZMM register
  my $ZTMP5          = $_[16];    # [clobbered] ZMM register
  my $ZTMP6          = $_[17];    # [clobbered] ZMM register
  my $ZTMP7          = $_[18];    # [clobbered] ZMM register
  my $ZTMP8          = $_[19];    # [clobbered] ZMM register
  my $ZTMP9          = $_[20];    # [clobbered] ZMM register
  my $ZTMP10         = $_[21];    # [clobbered] ZMM register
  my $ZTMP11         = $_[22];    # [clobbered] ZMM register
  my $ZTMP12         = $_[23];    # [clobbered] ZMM register
  my $ZTMP13         = $_[24];    # [clobbered] ZMM register
  my $ZTMP14         = $_[25];    # [clobbered] ZMM register
  my $IA0            = $_[26];    # [clobbered] GP register
  my $IA1            = $_[27];    # [clobbered] GP register
  my $MASKREG        = $_[28];    # [clobbered] mask register
  my $SHUFMASK       = $_[29];    # [in] ZMM with BE/LE shuffle mask
  my $PBLOCK_LEN     = $_[30];    # [in] partial block length

  my $label_suffix = $label_count++;

  $code .= <<___;
        cmp               \$8,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_8_${label_suffix}
        jl                .L_small_initial_num_blocks_is_7_1_${label_suffix}


        cmp               \$12,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_12_${label_suffix}
        jl                .L_small_initial_num_blocks_is_11_9_${label_suffix}

        # ;; 16, 15, 14 or 13
        cmp               \$16,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_16_${label_suffix}
        cmp               \$15,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_15_${label_suffix}
        cmp               \$14,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_14_${label_suffix}
        jmp               .L_small_initial_num_blocks_is_13_${label_suffix}

.L_small_initial_num_blocks_is_11_9_${label_suffix}:
        # ;; 11, 10 or 9
        cmp               \$11,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_11_${label_suffix}
        cmp               \$10,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_10_${label_suffix}
        jmp               .L_small_initial_num_blocks_is_9_${label_suffix}

.L_small_initial_num_blocks_is_7_1_${label_suffix}:
        cmp               \$4,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_4_${label_suffix}
        jl                .L_small_initial_num_blocks_is_3_1_${label_suffix}
        # ;; 7, 6 or 5
        cmp               \$7,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_7_${label_suffix}
        cmp               \$6,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_6_${label_suffix}
        jmp               .L_small_initial_num_blocks_is_5_${label_suffix}

.L_small_initial_num_blocks_is_3_1_${label_suffix}:
        # ;; 3, 2 or 1
        cmp               \$3,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_3_${label_suffix}
        cmp               \$2,$NUM_BLOCKS
        je                .L_small_initial_num_blocks_is_2_${label_suffix}

        # ;; for $NUM_BLOCKS == 1, just fall through and no 'jmp' needed

        # ;; Generation of different block size variants
        # ;; - one block size has to be the first one
___

  for (my $num_blocks = 1; $num_blocks <= 16; $num_blocks++) {
    $code .= ".L_small_initial_num_blocks_is_${num_blocks}_${label_suffix}:\n";
    &INITIAL_BLOCKS_PARTIAL(
      $AES_KEYS,   $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN, $LENGTH,   $DATA_OFFSET,
      $num_blocks, $CTR,        $HASH_IN_OUT,    $ENC_DEC,       $ZTMP0,    $ZTMP1,
      $ZTMP2,      $ZTMP3,      $ZTMP4,          $ZTMP5,         $ZTMP6,    $ZTMP7,
      $ZTMP8,      $ZTMP9,      $ZTMP10,         $ZTMP11,        $ZTMP12,   $ZTMP13,
      $ZTMP14,     $IA0,        $IA1,            $MASKREG,       $SHUFMASK, $PBLOCK_LEN);

    if ($num_blocks != 16) {
      $code .= "jmp           .L_small_initial_blocks_encrypted_${label_suffix}\n";
    }
  }

  $code .= ".L_small_initial_blocks_encrypted_${label_suffix}:\n";
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ; GCM_ENC_DEC Encrypts/Decrypts given data. Assumes that the passed gcm128_context
# ; struct has been initialized by GCM_INIT_IV
# ; Requires the input data be at least 1 byte long because of READ_SMALL_INPUT_DATA.
# ; Clobbers rax, r10-r15, and zmm0-zmm31, k1
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub GCM_ENC_DEC {
  my $AES_KEYS       = $_[0];    # [in] AES Key schedule
  my $GCM128_CTX     = $_[1];    # [in] context pointer
  my $PBLOCK_LEN     = $_[2];    # [in] length of partial block at the moment of previous update
  my $PLAIN_CIPH_IN  = $_[3];    # [in] input buffer pointer
  my $PLAIN_CIPH_LEN = $_[4];    # [in] buffer length
  my $CIPH_PLAIN_OUT = $_[5];    # [in] output buffer pointer
  my $ENC_DEC        = $_[6];    # [in] cipher direction

  my $IA0 = "%r10";
  my $IA1 = "%r12";
  my $IA2 = "%r13";
  my $IA3 = "%r15";
  my $IA4 = "%r11";
  my $IA5 = "%rax";
  my $IA6 = "%rbx";
  my $IA7 = "%r14";

  my $LENGTH = $win64 ? $IA2 : $PLAIN_CIPH_LEN;

  my $CTR_CHECK   = $IA3;
  my $DATA_OFFSET = $IA4;
  my $HASHK_PTR   = $IA6;

  my $HKEYS_READY = $IA7;

  my $CTR_BLOCKz = "%zmm2";
  my $CTR_BLOCKx = "%xmm2";

  # ; hardcoded in GCM_INIT

  my $AAD_HASHz = "%zmm14";
  my $AAD_HASHx = "%xmm14";

  # ; hardcoded in GCM_COMPLETE

  my $ZTMP0  = "%zmm0";
  my $ZTMP1  = "%zmm3";
  my $ZTMP2  = "%zmm4";
  my $ZTMP3  = "%zmm5";
  my $ZTMP4  = "%zmm6";
  my $ZTMP5  = "%zmm7";
  my $ZTMP6  = "%zmm10";
  my $ZTMP7  = "%zmm11";
  my $ZTMP8  = "%zmm12";
  my $ZTMP9  = "%zmm13";
  my $ZTMP10 = "%zmm15";
  my $ZTMP11 = "%zmm16";
  my $ZTMP12 = "%zmm17";

  my $ZTMP13 = "%zmm19";
  my $ZTMP14 = "%zmm20";
  my $ZTMP15 = "%zmm21";
  my $ZTMP16 = "%zmm30";
  my $ZTMP17 = "%zmm31";
  my $ZTMP18 = "%zmm1";
  my $ZTMP19 = "%zmm18";
  my $ZTMP20 = "%zmm8";
  my $ZTMP21 = "%zmm22";
  my $ZTMP22 = "%zmm23";

  my $GH        = "%zmm24";
  my $GL        = "%zmm25";
  my $GM        = "%zmm26";
  my $SHUF_MASK = "%zmm29";

  # ; Unused in the small packet path
  my $ADDBE_4x4  = "%zmm27";
  my $ADDBE_1234 = "%zmm28";

  my $MASKREG = "%k1";

  my $label_suffix = $label_count++;

  # ;; reduction every 48 blocks, depth 32 blocks
  # ;; @note 48 blocks is the maximum capacity of the stack frame
  my $big_loop_nblocks = 48;
  my $big_loop_depth   = 32;

  # ;;; Macro flow depending on packet size
  # ;;; - LENGTH <= 16 blocks
  # ;;;   - cipher followed by hashing (reduction)
  # ;;; - 16 blocks < LENGTH < 32 blocks
  # ;;;   - cipher 16 blocks
  # ;;;   - cipher N blocks & hash 16 blocks, hash N blocks (reduction)
  # ;;; - 32 blocks < LENGTH < 48 blocks
  # ;;;   - cipher 2 x 16 blocks
  # ;;;   - hash 16 blocks
  # ;;;   - cipher N blocks & hash 16 blocks, hash N blocks (reduction)
  # ;;; - LENGTH >= 48 blocks
  # ;;;   - cipher 2 x 16 blocks
  # ;;;   - while (data_to_cipher >= 48 blocks):
  # ;;;     - cipher 16 blocks & hash 16 blocks
  # ;;;     - cipher 16 blocks & hash 16 blocks
  # ;;;     - cipher 16 blocks & hash 16 blocks (reduction)
  # ;;;   - if (data_to_cipher >= 32 blocks):
  # ;;;     - cipher 16 blocks & hash 16 blocks
  # ;;;     - cipher 16 blocks & hash 16 blocks
  # ;;;     - hash 16 blocks (reduction)
  # ;;;     - cipher N blocks & hash 16 blocks, hash N blocks (reduction)
  # ;;;   - elif (data_to_cipher >= 16 blocks):
  # ;;;     - cipher 16 blocks & hash 16 blocks
  # ;;;     - hash 16 blocks
  # ;;;     - cipher N blocks & hash 16 blocks, hash N blocks (reduction)
  # ;;;   - else:
  # ;;;     - hash 16 blocks
  # ;;;     - cipher N blocks & hash 16 blocks, hash N blocks (reduction)

  if ($win64) {
    $code .= "cmpq              \$0,$PLAIN_CIPH_LEN\n";
  } else {
    $code .= "or                $PLAIN_CIPH_LEN,$PLAIN_CIPH_LEN\n";
  }
  $code .= "je            .L_enc_dec_done_${label_suffix}\n";

  # Length value from context $CTX_OFFSET_InLen`($GCM128_CTX) is updated in
  # 'providers/implementations/ciphers/cipher_aes_gcm_hw_vaes_avx512.inc'

  $code .= "xor                $HKEYS_READY, $HKEYS_READY\n";
  $code .= "vmovdqu64         `$CTX_OFFSET_AadHash`($GCM128_CTX),$AAD_HASHx\n";

  # ;; Used for the update flow - if there was a previous partial
  # ;; block fill the remaining bytes here.
  &PARTIAL_BLOCK(
    $GCM128_CTX,  $PBLOCK_LEN, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN, $PLAIN_CIPH_LEN,
    $DATA_OFFSET, $AAD_HASHx,  $ENC_DEC,        $IA0,           $IA1,
    $IA2,         $ZTMP0,      $ZTMP1,          $ZTMP2,         $ZTMP3,
    $ZTMP4,       $ZTMP5,      $ZTMP6,          $ZTMP7,         $MASKREG);

  $code .= "vmovdqu64         `$CTX_OFFSET_CurCount`($GCM128_CTX),$CTR_BLOCKx\n";

  # ;; Save the amount of data left to process in $LENGTH
  # ;; NOTE: PLAIN_CIPH_LEN is a register on linux;
  if ($win64) {
    $code .= "mov               $PLAIN_CIPH_LEN,$LENGTH\n";
  }

  # ;; There may be no more data if it was consumed in the partial block.
  $code .= <<___;
        sub               $DATA_OFFSET,$LENGTH
        je                .L_enc_dec_done_${label_suffix}
___

  $code .= <<___;
        cmp               \$`(16 * 16)`,$LENGTH
        jbe              .L_message_below_equal_16_blocks_${label_suffix}

        vmovdqa64         SHUF_MASK(%rip),$SHUF_MASK
        vmovdqa64         ddq_addbe_4444(%rip),$ADDBE_4x4
        vmovdqa64         ddq_addbe_1234(%rip),$ADDBE_1234

        # ;; start the pipeline
        # ;; - 32 blocks aes-ctr
        # ;; - 16 blocks ghash + aes-ctr

        # ;; set up CTR_CHECK
        vmovd             $CTR_BLOCKx,@{[DWORD($CTR_CHECK)]}
        and               \$255,@{[DWORD($CTR_CHECK)]}
        # ;; in LE format after init, convert to BE
        vshufi64x2        \$0,$CTR_BLOCKz,$CTR_BLOCKz,$CTR_BLOCKz
        vpshufb           $SHUF_MASK,$CTR_BLOCKz,$CTR_BLOCKz
___

  # ;; ==== AES-CTR - first 16 blocks
  my $aesout_offset      = ($STACK_LOCAL_OFFSET + (0 * 16));
  my $data_in_out_offset = 0;
  &INITIAL_BLOCKS_16(
    $PLAIN_CIPH_IN, $CIPH_PLAIN_OUT, $AES_KEYS,      $DATA_OFFSET,        "no_ghash", $CTR_BLOCKz,
    $CTR_CHECK,     $ADDBE_4x4,      $ADDBE_1234,    $ZTMP0,              $ZTMP1,     $ZTMP2,
    $ZTMP3,         $ZTMP4,          $ZTMP5,         $ZTMP6,              $ZTMP7,     $ZTMP8,
    $SHUF_MASK,     $ENC_DEC,        $aesout_offset, $data_in_out_offset, $IA0);

  &precompute_hkeys_on_stack($GCM128_CTX, $HKEYS_READY, $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6,
    "first16");

  $code .= <<___;
        cmp               \$`(32 * 16)`,$LENGTH
        jb                .L_message_below_32_blocks_${label_suffix}
___

  # ;; ==== AES-CTR - next 16 blocks
  $aesout_offset      = ($STACK_LOCAL_OFFSET + (16 * 16));
  $data_in_out_offset = (16 * 16);
  &INITIAL_BLOCKS_16(
    $PLAIN_CIPH_IN, $CIPH_PLAIN_OUT, $AES_KEYS,      $DATA_OFFSET,        "no_ghash", $CTR_BLOCKz,
    $CTR_CHECK,     $ADDBE_4x4,      $ADDBE_1234,    $ZTMP0,              $ZTMP1,     $ZTMP2,
    $ZTMP3,         $ZTMP4,          $ZTMP5,         $ZTMP6,              $ZTMP7,     $ZTMP8,
    $SHUF_MASK,     $ENC_DEC,        $aesout_offset, $data_in_out_offset, $IA0);

  &precompute_hkeys_on_stack($GCM128_CTX, $HKEYS_READY, $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6,
    "last32");
  $code .= "mov     \$1,$HKEYS_READY\n";

  $code .= <<___;
        add               \$`(32 * 16)`,$DATA_OFFSET
        sub               \$`(32 * 16)`,$LENGTH

        cmp               \$`($big_loop_nblocks * 16)`,$LENGTH
        jb                .L_no_more_big_nblocks_${label_suffix}
___

  # ;; ====
  # ;; ==== AES-CTR + GHASH - 48 blocks loop
  # ;; ====
  $code .= ".L_encrypt_big_nblocks_${label_suffix}:\n";

  # ;; ==== AES-CTR + GHASH - 16 blocks, start
  $aesout_offset      = ($STACK_LOCAL_OFFSET + (32 * 16));
  $data_in_out_offset = (0 * 16);
  my $ghashin_offset = ($STACK_LOCAL_OFFSET + (0 * 16));
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    48,        $aesout_offset,  $ghashin_offset, $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "first_time",    $ENC_DEC,     $data_in_out_offset, $AAD_HASHz,
    $IA0);

  # ;; ==== AES-CTR + GHASH - 16 blocks, no reduction
  $aesout_offset      = ($STACK_LOCAL_OFFSET + (0 * 16));
  $data_in_out_offset = (16 * 16);
  $ghashin_offset     = ($STACK_LOCAL_OFFSET + (16 * 16));
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    32,        $aesout_offset,  $ghashin_offset, $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "no_reduction",  $ENC_DEC,     $data_in_out_offset, "no_ghash_in",
    $IA0);

  # ;; ==== AES-CTR + GHASH - 16 blocks, reduction
  $aesout_offset      = ($STACK_LOCAL_OFFSET + (16 * 16));
  $data_in_out_offset = (32 * 16);
  $ghashin_offset     = ($STACK_LOCAL_OFFSET + (32 * 16));
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,    $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    16,        $aesout_offset,  $ghashin_offset,   $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,            $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,           $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,           $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,           $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "final_reduction", $ENC_DEC,     $data_in_out_offset, "no_ghash_in",
    $IA0);

  # ;; === xor cipher block 0 with GHASH (ZT4)
  $code .= <<___;
        vmovdqa64         $ZTMP4,$AAD_HASHz

        add               \$`($big_loop_nblocks * 16)`,$DATA_OFFSET
        sub               \$`($big_loop_nblocks * 16)`,$LENGTH
        cmp               \$`($big_loop_nblocks * 16)`,$LENGTH
        jae               .L_encrypt_big_nblocks_${label_suffix}

.L_no_more_big_nblocks_${label_suffix}:

        cmp               \$`(32 * 16)`,$LENGTH
        jae               .L_encrypt_32_blocks_${label_suffix}

        cmp               \$`(16 * 16)`,$LENGTH
        jae               .L_encrypt_16_blocks_${label_suffix}
___

  # ;; =====================================================
  # ;; =====================================================
  # ;; ==== GHASH 1 x 16 blocks
  # ;; ==== GHASH 1 x 16 blocks (reduction) & encrypt N blocks
  # ;; ====      then GHASH N blocks
  $code .= ".L_encrypt_0_blocks_ghash_32_${label_suffix}:\n";

  # ;; calculate offset to the right hash key
  $code .= <<___;
mov               @{[DWORD($LENGTH)]},@{[DWORD($IA0)]}
and               \$~15,@{[DWORD($IA0)]}
mov               \$`@{[HashKeyOffsetByIdx(32,"frame")]}`,@{[DWORD($HASHK_PTR)]}
sub               @{[DWORD($IA0)]},@{[DWORD($HASHK_PTR)]}
___

  # ;; ==== GHASH 32 blocks and follow with reduction
  &GHASH_16("start", $GH, $GM, $GL, "%rsp", $STACK_LOCAL_OFFSET, (0 * 16),
    "%rsp", $HASHK_PTR, 0, $AAD_HASHz, $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6, $ZTMP7, $ZTMP8, $ZTMP9);

  # ;; ==== GHASH 1 x 16 blocks with reduction + cipher and ghash on the reminder
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (16 * 16));
  $code .= "add               \$`(16 * 16)`,@{[DWORD($HASHK_PTR)]}\n";
  &GCM_ENC_DEC_LAST(
    $AES_KEYS,   $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $LENGTH,
    $CTR_BLOCKz, $CTR_CHECK,  $HASHK_PTR,      $ghashin_offset, $SHUF_MASK,   $ZTMP0,
    $ZTMP1,      $ZTMP2,      $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,
    $ZTMP7,      $ZTMP8,      $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,
    $ZTMP13,     $ZTMP14,     $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,
    $ZTMP19,     $ZTMP20,     $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,
    "mid",       $GL,         $GH,             $GM,             $ENC_DEC,     $AAD_HASHz,
    $IA0,        $IA5,        $MASKREG,        $PBLOCK_LEN);

  $code .= "vpshufb           @{[XWORD($SHUF_MASK)]},$CTR_BLOCKx,$CTR_BLOCKx\n";
  $code .= "jmp           .L_ghash_done_${label_suffix}\n";

  # ;; =====================================================
  # ;; =====================================================
  # ;; ==== GHASH & encrypt 1 x 16 blocks
  # ;; ==== GHASH & encrypt 1 x 16 blocks
  # ;; ==== GHASH 1 x 16 blocks (reduction)
  # ;; ==== GHASH 1 x 16 blocks (reduction) & encrypt N blocks
  # ;; ====      then GHASH N blocks
  $code .= ".L_encrypt_32_blocks_${label_suffix}:\n";

  # ;; ==== AES-CTR + GHASH - 16 blocks, start
  $aesout_offset  = ($STACK_LOCAL_OFFSET + (32 * 16));
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (0 * 16));
  $data_in_out_offset = (0 * 16);
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    48,        $aesout_offset,  $ghashin_offset, $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "first_time",    $ENC_DEC,     $data_in_out_offset, $AAD_HASHz,
    $IA0);

  # ;; ==== AES-CTR + GHASH - 16 blocks, no reduction
  $aesout_offset  = ($STACK_LOCAL_OFFSET + (0 * 16));
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (16 * 16));
  $data_in_out_offset = (16 * 16);
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    32,        $aesout_offset,  $ghashin_offset, $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "no_reduction",  $ENC_DEC,     $data_in_out_offset, "no_ghash_in",
    $IA0);

  # ;; ==== GHASH 16 blocks with reduction
  &GHASH_16(
    "end_reduce", $GH, $GM, $GL, "%rsp", $STACK_LOCAL_OFFSET, (32 * 16),
    "%rsp", &HashKeyOffsetByIdx(16, "frame"),
    0, $AAD_HASHz, $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6, $ZTMP7, $ZTMP8, $ZTMP9);

  # ;; ==== GHASH 1 x 16 blocks with reduction + cipher and ghash on the reminder
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (0 * 16));
  $code .= <<___;
        sub               \$`(32 * 16)`,$LENGTH
        add               \$`(32 * 16)`,$DATA_OFFSET
___

  # ;; calculate offset to the right hash key
  $code .= "mov               @{[DWORD($LENGTH)]},@{[DWORD($IA0)]}\n";
  $code .= <<___;
        and               \$~15,@{[DWORD($IA0)]}
        mov               \$`@{[HashKeyOffsetByIdx(16,"frame")]}`,@{[DWORD($HASHK_PTR)]}
        sub               @{[DWORD($IA0)]},@{[DWORD($HASHK_PTR)]}
___
  &GCM_ENC_DEC_LAST(
    $AES_KEYS,   $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $LENGTH,
    $CTR_BLOCKz, $CTR_CHECK,  $HASHK_PTR,      $ghashin_offset, $SHUF_MASK,   $ZTMP0,
    $ZTMP1,      $ZTMP2,      $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,
    $ZTMP7,      $ZTMP8,      $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,
    $ZTMP13,     $ZTMP14,     $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,
    $ZTMP19,     $ZTMP20,     $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,
    "start",     $GL,         $GH,             $GM,             $ENC_DEC,     $AAD_HASHz,
    $IA0,        $IA5,        $MASKREG,        $PBLOCK_LEN);

  $code .= "vpshufb           @{[XWORD($SHUF_MASK)]},$CTR_BLOCKx,$CTR_BLOCKx\n";
  $code .= "jmp           .L_ghash_done_${label_suffix}\n";

  # ;; =====================================================
  # ;; =====================================================
  # ;; ==== GHASH & encrypt 16 blocks (done before)
  # ;; ==== GHASH 1 x 16 blocks
  # ;; ==== GHASH 1 x 16 blocks (reduction) & encrypt N blocks
  # ;; ====      then GHASH N blocks
  $code .= ".L_encrypt_16_blocks_${label_suffix}:\n";

  # ;; ==== AES-CTR + GHASH - 16 blocks, start
  $aesout_offset  = ($STACK_LOCAL_OFFSET + (32 * 16));
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (0 * 16));
  $data_in_out_offset = (0 * 16);
  &GHASH_16_ENCRYPT_16_PARALLEL(
    $AES_KEYS, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $CTR_BLOCKz,         $CTR_CHECK,
    48,        $aesout_offset,  $ghashin_offset, $SHUF_MASK,   $ZTMP0,              $ZTMP1,
    $ZTMP2,    $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,              $ZTMP7,
    $ZTMP8,    $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,             $ZTMP13,
    $ZTMP14,   $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,             $ZTMP19,
    $ZTMP20,   $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,         $GL,
    $GH,       $GM,             "first_time",    $ENC_DEC,     $data_in_out_offset, $AAD_HASHz,
    $IA0);

  # ;; ==== GHASH 1 x 16 blocks
  &GHASH_16(
    "mid", $GH, $GM, $GL, "%rsp", $STACK_LOCAL_OFFSET, (16 * 16),
    "%rsp", &HashKeyOffsetByIdx(32, "frame"),
    0, "no_hash_input", $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6, $ZTMP7, $ZTMP8, $ZTMP9);

  # ;; ==== GHASH 1 x 16 blocks with reduction + cipher and ghash on the reminder
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (32 * 16));
  $code .= <<___;
        sub               \$`(16 * 16)`,$LENGTH
        add               \$`(16 * 16)`,$DATA_OFFSET
___
  &GCM_ENC_DEC_LAST(
    $AES_KEYS,    $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,
    $DATA_OFFSET, $LENGTH,     $CTR_BLOCKz,     $CTR_CHECK,
    &HashKeyOffsetByIdx(16, "frame"), $ghashin_offset, $SHUF_MASK, $ZTMP0,
    $ZTMP1,       $ZTMP2,     $ZTMP3,     $ZTMP4,
    $ZTMP5,       $ZTMP6,     $ZTMP7,     $ZTMP8,
    $ZTMP9,       $ZTMP10,    $ZTMP11,    $ZTMP12,
    $ZTMP13,      $ZTMP14,    $ZTMP15,    $ZTMP16,
    $ZTMP17,      $ZTMP18,    $ZTMP19,    $ZTMP20,
    $ZTMP21,      $ZTMP22,    $ADDBE_4x4, $ADDBE_1234,
    "end_reduce", $GL,        $GH,        $GM,
    $ENC_DEC,     $AAD_HASHz, $IA0,       $IA5,
    $MASKREG,     $PBLOCK_LEN);

  $code .= "vpshufb           @{[XWORD($SHUF_MASK)]},$CTR_BLOCKx,$CTR_BLOCKx\n";
  $code .= <<___;
        jmp               .L_ghash_done_${label_suffix}

.L_message_below_32_blocks_${label_suffix}:
        # ;; 32 > number of blocks > 16

        sub               \$`(16 * 16)`,$LENGTH
        add               \$`(16 * 16)`,$DATA_OFFSET
___
  $ghashin_offset = ($STACK_LOCAL_OFFSET + (0 * 16));

  # ;; calculate offset to the right hash key
  $code .= "mov               @{[DWORD($LENGTH)]},@{[DWORD($IA0)]}\n";

  &precompute_hkeys_on_stack($GCM128_CTX, $HKEYS_READY, $ZTMP0, $ZTMP1, $ZTMP2, $ZTMP3, $ZTMP4, $ZTMP5, $ZTMP6,
    "mid16");
  $code .= "mov     \$1,$HKEYS_READY\n";

  $code .= <<___;
and               \$~15,@{[DWORD($IA0)]}
mov               \$`@{[HashKeyOffsetByIdx(16,"frame")]}`,@{[DWORD($HASHK_PTR)]}
sub               @{[DWORD($IA0)]},@{[DWORD($HASHK_PTR)]}
___

  &GCM_ENC_DEC_LAST(
    $AES_KEYS,   $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN,  $DATA_OFFSET, $LENGTH,
    $CTR_BLOCKz, $CTR_CHECK,  $HASHK_PTR,      $ghashin_offset, $SHUF_MASK,   $ZTMP0,
    $ZTMP1,      $ZTMP2,      $ZTMP3,          $ZTMP4,          $ZTMP5,       $ZTMP6,
    $ZTMP7,      $ZTMP8,      $ZTMP9,          $ZTMP10,         $ZTMP11,      $ZTMP12,
    $ZTMP13,     $ZTMP14,     $ZTMP15,         $ZTMP16,         $ZTMP17,      $ZTMP18,
    $ZTMP19,     $ZTMP20,     $ZTMP21,         $ZTMP22,         $ADDBE_4x4,   $ADDBE_1234,
    "start",     $GL,         $GH,             $GM,             $ENC_DEC,     $AAD_HASHz,
    $IA0,        $IA5,        $MASKREG,        $PBLOCK_LEN);

  $code .= "vpshufb           @{[XWORD($SHUF_MASK)]},$CTR_BLOCKx,$CTR_BLOCKx\n";
  $code .= <<___;
        jmp           .L_ghash_done_${label_suffix}

.L_message_below_equal_16_blocks_${label_suffix}:
        # ;; Determine how many blocks to process
        # ;; - process one additional block if there is a partial block
        mov               @{[DWORD($LENGTH)]},@{[DWORD($IA1)]}
        add               \$15,@{[DWORD($IA1)]}
        shr               \$4, @{[DWORD($IA1)]}     # ; $IA1 can be in the range from 0 to 16
___
  &GCM_ENC_DEC_SMALL(
    $AES_KEYS,    $GCM128_CTX, $CIPH_PLAIN_OUT, $PLAIN_CIPH_IN, $PLAIN_CIPH_LEN, $ENC_DEC,
    $DATA_OFFSET, $LENGTH,     $IA1,            $CTR_BLOCKx,    $AAD_HASHx,      $ZTMP0,
    $ZTMP1,       $ZTMP2,      $ZTMP3,          $ZTMP4,         $ZTMP5,          $ZTMP6,
    $ZTMP7,       $ZTMP8,      $ZTMP9,          $ZTMP10,        $ZTMP11,         $ZTMP12,
    $ZTMP13,      $ZTMP14,     $IA0,            $IA3,           $MASKREG,        $SHUF_MASK,
    $PBLOCK_LEN);

  # ;; fall through to exit

  $code .= ".L_ghash_done_${label_suffix}:\n";

  # ;; save the last counter block
  $code .= "vmovdqu64         $CTR_BLOCKx,`$CTX_OFFSET_CurCount`($GCM128_CTX)\n";
  $code .= <<___;
        vmovdqu64         $AAD_HASHx,`$CTX_OFFSET_AadHash`($GCM128_CTX)
.L_enc_dec_done_${label_suffix}:
___
}

# ;;; ===========================================================================
# ;;; Encrypt/decrypt the initial 16 blocks
sub INITIAL_BLOCKS_16 {
  my $IN          = $_[0];     # [in] input buffer
  my $OUT         = $_[1];     # [in] output buffer
  my $AES_KEYS    = $_[2];     # [in] pointer to expanded keys
  my $DATA_OFFSET = $_[3];     # [in] data offset
  my $GHASH       = $_[4];     # [in] ZMM with AAD (low 128 bits)
  my $CTR         = $_[5];     # [in] ZMM with CTR BE blocks 4x128 bits
  my $CTR_CHECK   = $_[6];     # [in/out] GPR with counter overflow check
  my $ADDBE_4x4   = $_[7];     # [in] ZMM 4x128bits with value 4 (big endian)
  my $ADDBE_1234  = $_[8];     # [in] ZMM 4x128bits with values 1, 2, 3 & 4 (big endian)
  my $T0          = $_[9];     # [clobered] temporary ZMM register
  my $T1          = $_[10];    # [clobered] temporary ZMM register
  my $T2          = $_[11];    # [clobered] temporary ZMM register
  my $T3          = $_[12];    # [clobered] temporary ZMM register
  my $T4          = $_[13];    # [clobered] temporary ZMM register
  my $T5          = $_[14];    # [clobered] temporary ZMM register
  my $T6          = $_[15];    # [clobered] temporary ZMM register
  my $T7          = $_[16];    # [clobered] temporary ZMM register
  my $T8          = $_[17];    # [clobered] temporary ZMM register
  my $SHUF_MASK   = $_[18];    # [in] ZMM with BE/LE shuffle mask
  my $ENC_DEC     = $_[19];    # [in] ENC (encrypt) or DEC (decrypt) selector
  my $BLK_OFFSET  = $_[20];    # [in] stack frame offset to ciphered blocks
  my $DATA_DISPL  = $_[21];    # [in] fixed numerical data displacement/offset
  my $IA0         = $_[22];    # [clobered] temporary GP register

  my $B00_03 = $T5;
  my $B04_07 = $T6;
  my $B08_11 = $T7;
  my $B12_15 = $T8;

  my $label_suffix = $label_count++;

  my $stack_offset = $BLK_OFFSET;
  $code .= <<___;
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        # ;; prepare counter blocks

        cmpb              \$`(256 - 16)`,@{[BYTE($CTR_CHECK)]}
        jae               .L_next_16_overflow_${label_suffix}
        vpaddd            $ADDBE_1234,$CTR,$B00_03
        vpaddd            $ADDBE_4x4,$B00_03,$B04_07
        vpaddd            $ADDBE_4x4,$B04_07,$B08_11
        vpaddd            $ADDBE_4x4,$B08_11,$B12_15
        jmp               .L_next_16_ok_${label_suffix}
.L_next_16_overflow_${label_suffix}:
        vpshufb           $SHUF_MASK,$CTR,$CTR
        vmovdqa64         ddq_add_4444(%rip),$B12_15
        vpaddd            ddq_add_1234(%rip),$CTR,$B00_03
        vpaddd            $B12_15,$B00_03,$B04_07
        vpaddd            $B12_15,$B04_07,$B08_11
        vpaddd            $B12_15,$B08_11,$B12_15
        vpshufb           $SHUF_MASK,$B00_03,$B00_03
        vpshufb           $SHUF_MASK,$B04_07,$B04_07
        vpshufb           $SHUF_MASK,$B08_11,$B08_11
        vpshufb           $SHUF_MASK,$B12_15,$B12_15
.L_next_16_ok_${label_suffix}:
        vshufi64x2        \$0b11111111,$B12_15,$B12_15,$CTR
        addb               \$16,@{[BYTE($CTR_CHECK)]}
        # ;; === load 16 blocks of data
        vmovdqu8          `$DATA_DISPL + (64*0)`($IN,$DATA_OFFSET,1),$T0
        vmovdqu8          `$DATA_DISPL + (64*1)`($IN,$DATA_OFFSET,1),$T1
        vmovdqu8          `$DATA_DISPL + (64*2)`($IN,$DATA_OFFSET,1),$T2
        vmovdqu8          `$DATA_DISPL + (64*3)`($IN,$DATA_OFFSET,1),$T3

        # ;; move to AES encryption rounds
        vbroadcastf64x2    `(16*0)`($AES_KEYS),$T4
        vpxorq            $T4,$B00_03,$B00_03
        vpxorq            $T4,$B04_07,$B04_07
        vpxorq            $T4,$B08_11,$B08_11
        vpxorq            $T4,$B12_15,$B12_15
___
  foreach (1 .. ($NROUNDS)) {
    $code .= <<___;
        vbroadcastf64x2    `(16*$_)`($AES_KEYS),$T4
        vaesenc            $T4,$B00_03,$B00_03
        vaesenc            $T4,$B04_07,$B04_07
        vaesenc            $T4,$B08_11,$B08_11
        vaesenc            $T4,$B12_15,$B12_15
___
  }
  $code .= <<___;
        vbroadcastf64x2    `(16*($NROUNDS+1))`($AES_KEYS),$T4
        vaesenclast         $T4,$B00_03,$B00_03
        vaesenclast         $T4,$B04_07,$B04_07
        vaesenclast         $T4,$B08_11,$B08_11
        vaesenclast         $T4,$B12_15,$B12_15

        # ;;  xor against text
        vpxorq            $T0,$B00_03,$B00_03
        vpxorq            $T1,$B04_07,$B04_07
        vpxorq            $T2,$B08_11,$B08_11
        vpxorq            $T3,$B12_15,$B12_15

        # ;; store
        mov               $OUT, $IA0
        vmovdqu8          $B00_03,`$DATA_DISPL + (64*0)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B04_07,`$DATA_DISPL + (64*1)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B08_11,`$DATA_DISPL + (64*2)`($IA0,$DATA_OFFSET,1)
        vmovdqu8          $B12_15,`$DATA_DISPL + (64*3)`($IA0,$DATA_OFFSET,1)
___
  if ($ENC_DEC eq "DEC") {
    $code .= <<___;
        # ;; decryption - cipher text needs to go to GHASH phase
        vpshufb           $SHUF_MASK,$T0,$B00_03
        vpshufb           $SHUF_MASK,$T1,$B04_07
        vpshufb           $SHUF_MASK,$T2,$B08_11
        vpshufb           $SHUF_MASK,$T3,$B12_15
___
  } else {
    $code .= <<___;
        # ;; encryption
        vpshufb           $SHUF_MASK,$B00_03,$B00_03
        vpshufb           $SHUF_MASK,$B04_07,$B04_07
        vpshufb           $SHUF_MASK,$B08_11,$B08_11
        vpshufb           $SHUF_MASK,$B12_15,$B12_15
___
  }

  if ($GHASH ne "no_ghash") {
    $code .= <<___;
        # ;; === xor cipher block 0 with GHASH for the next GHASH round
        vpxorq            $GHASH,$B00_03,$B00_03
___
  }
  $code .= <<___;
        vmovdqa64         $B00_03,`$stack_offset + (0 * 64)`(%rsp)
        vmovdqa64         $B04_07,`$stack_offset + (1 * 64)`(%rsp)
        vmovdqa64         $B08_11,`$stack_offset + (2 * 64)`(%rsp)
        vmovdqa64         $B12_15,`$stack_offset + (3 * 64)`(%rsp)
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ; GCM_COMPLETE Finishes ghash calculation
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
sub GCM_COMPLETE {
  my $GCM128_CTX = $_[0];
  my $PBLOCK_LEN = $_[1];

  my $label_suffix = $label_count++;

  $code .= <<___;
        vmovdqu           @{[HashKeyByIdx(1,$GCM128_CTX)]},%xmm2
        vmovdqu           $CTX_OFFSET_EK0($GCM128_CTX),%xmm3      # ; xmm3 = E(K,Y0)
___

  $code .= <<___;
        vmovdqu           `$CTX_OFFSET_AadHash`($GCM128_CTX),%xmm4

        # ;; Process the final partial block.
        cmp               \$0,$PBLOCK_LEN
        je                .L_partial_done_${label_suffix}
___

  #  ;GHASH computation for the last <16 Byte block
  &GHASH_MUL("%xmm4", "%xmm2", "%xmm0", "%xmm16", "%xmm17");

  $code .= <<___;
.L_partial_done_${label_suffix}:
        vmovq           `$CTX_OFFSET_InLen`($GCM128_CTX), %xmm5
        vpinsrq         \$1, `$CTX_OFFSET_AadLen`($GCM128_CTX), %xmm5, %xmm5    #  ; xmm5 = len(A)||len(C)
        vpsllq          \$3, %xmm5, %xmm5                                       #  ; convert bytes into bits

        vpxor           %xmm5,%xmm4,%xmm4
___

  &GHASH_MUL("%xmm4", "%xmm2", "%xmm0", "%xmm16", "%xmm17");

  $code .= <<___;
        vpshufb         SHUF_MASK(%rip),%xmm4,%xmm4      # ; perform a 16Byte swap
        vpxor           %xmm4,%xmm3,%xmm3

.L_return_T_${label_suffix}:
        vmovdqu           %xmm3,`$CTX_OFFSET_AadHash`($GCM128_CTX)
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;;; Functions definitions
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

$code .= ".text\n";
{
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  # ;void   ossl_aes_gcm_init_avx512 /
  # ;       (const void *aes_keys,
  # ;        void *gcm128ctx)
  # ;
  # ; Precomputes hashkey table for GHASH optimization.
  # ; Leaf function (does not allocate stack space, does not use non-volatile registers).
  # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
  $code .= <<___;
.globl ossl_aes_gcm_init_avx512
.type ossl_aes_gcm_init_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_init_avx512:
.cfi_startproc
        endbranch
___
  if ($CHECK_FUNCTION_ARGUMENTS) {
    $code .= <<___;
        # ;; Check aes_keys != NULL
        test               $arg1,$arg1
        jz                .Labort_init

        # ;; Check gcm128ctx != NULL
        test               $arg2,$arg2
        jz                .Labort_init
___
  }
  $code .= "vpxorq            %xmm16,%xmm16,%xmm16\n";
  &ENCRYPT_SINGLE_BLOCK("$arg1", "%xmm16", "%rax");    # ; xmm16 = HashKey
  $code .= <<___;
        vpshufb           SHUF_MASK(%rip),%xmm16,%xmm16
        # ;;;  PRECOMPUTATION of HashKey<<1 mod poly from the HashKey ;;;
        vmovdqa64         %xmm16,%xmm2
        vpsllq            \$1,%xmm16,%xmm16
        vpsrlq            \$63,%xmm2,%xmm2
        vmovdqa           %xmm2,%xmm1
        vpslldq           \$8,%xmm2,%xmm2
        vpsrldq           \$8,%xmm1,%xmm1
        vporq             %xmm2,%xmm16,%xmm16
        # ;reduction
        vpshufd           \$0b00100100,%xmm1,%xmm2
        vpcmpeqd          TWOONE(%rip),%xmm2,%xmm2
        vpand             POLY(%rip),%xmm2,%xmm2
        vpxorq            %xmm2,%xmm16,%xmm16                  # ; xmm16 holds the HashKey<<1 mod poly
        # ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
        vmovdqu64         %xmm16,@{[HashKeyByIdx(1,$arg2)]} # ; store HashKey<<1 mod poly
___
  &PRECOMPUTE("$arg2", "%xmm16", "%xmm0", "%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5");
  if ($CLEAR_SCRATCH_REGISTERS) {
    &clear_scratch_gps_asm();
    &clear_scratch_zmms_asm();
  } else {
    $code .= "vzeroupper\n";
  }
  $code .= <<___;
.Labort_init:
ret
.cfi_endproc
.size ossl_aes_gcm_init_avx512, .-ossl_aes_gcm_init_avx512
___
}

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void   ossl_aes_gcm_setiv_avx512
# ;       (const void *aes_keys,
# ;        void *gcm128ctx,
# ;        const unsigned char *iv,
# ;        size_t ivlen)
# ;
# ; Computes E(K,Y0) for finalization, updates current counter Yi in gcm128_context structure.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_aes_gcm_setiv_avx512
.type ossl_aes_gcm_setiv_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_setiv_avx512:
.cfi_startproc
.Lsetiv_seh_begin:
        endbranch
___
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check aes_keys != NULL
        test               $arg1,$arg1
        jz                 .Labort_setiv

        # ;; Check gcm128ctx != NULL
        test               $arg2,$arg2
        jz                 .Labort_setiv

        # ;; Check iv != NULL
        test               $arg3,$arg3
        jz                 .Labort_setiv

        # ;; Check ivlen != 0
        test               $arg4,$arg4
        jz                 .Labort_setiv
___
}

# ; NOTE: code before PROLOG() must not modify any registers
&PROLOG(
  1,    # allocate stack space for hkeys
  0,    # do not allocate stack space for AES blocks
  "setiv");
&GCM_INIT_IV(
  "$arg1",  "$arg2",  "$arg3",  "$arg4",  "%r10",   "%r11",  "%r12",  "%k1",   "%xmm2",  "%zmm1",
  "%zmm11", "%zmm3",  "%zmm4",  "%zmm5",  "%zmm6",  "%zmm7", "%zmm8", "%zmm9", "%zmm10", "%zmm12",
  "%zmm13", "%zmm15", "%zmm16", "%zmm17", "%zmm18", "%zmm19");
&EPILOG(
  1,    # hkeys were allocated
  $arg4);
$code .= <<___;
.Labort_setiv:
ret
.Lsetiv_seh_end:
.cfi_endproc
.size ossl_aes_gcm_setiv_avx512, .-ossl_aes_gcm_setiv_avx512
___

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void ossl_aes_gcm_update_aad_avx512
# ;     (unsigned char *gcm128ctx,
# ;      const unsigned char *aad,
# ;      size_t aadlen)
# ;
# ; Updates AAD hash in gcm128_context structure.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_aes_gcm_update_aad_avx512
.type ossl_aes_gcm_update_aad_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_update_aad_avx512:
.cfi_startproc
.Lghash_seh_begin:
        endbranch
___
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check gcm128ctx != NULL
        test               $arg1,$arg1
        jz                 .Lexit_update_aad

        # ;; Check aad != NULL
        test               $arg2,$arg2
        jz                 .Lexit_update_aad

        # ;; Check aadlen != 0
        test               $arg3,$arg3
        jz                 .Lexit_update_aad
___
}

# ; NOTE: code before PROLOG() must not modify any registers
&PROLOG(
  1,    # allocate stack space for hkeys,
  0,    # do not allocate stack space for AES blocks
  "ghash");
&GCM_UPDATE_AAD(
  "$arg1",  "$arg2",  "$arg3",  "%r10",   "%r11",  "%r12",  "%k1",   "%xmm14", "%zmm1",  "%zmm11",
  "%zmm3",  "%zmm4",  "%zmm5",  "%zmm6",  "%zmm7", "%zmm8", "%zmm9", "%zmm10", "%zmm12", "%zmm13",
  "%zmm15", "%zmm16", "%zmm17", "%zmm18", "%zmm19");
&EPILOG(
  1,    # hkeys were allocated
  $arg3);
$code .= <<___;
.Lexit_update_aad:
ret
.Lghash_seh_end:
.cfi_endproc
.size ossl_aes_gcm_update_aad_avx512, .-ossl_aes_gcm_update_aad_avx512
___

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void   ossl_aes_gcm_encrypt_avx512
# ;       (const void* aes_keys,
# ;        void *gcm128ctx,
# ;        unsigned int *pblocklen,
# ;        const unsigned char *in,
# ;        size_t len,
# ;        unsigned char *out);
# ;
# ; Performs encryption of data |in| of len |len|, and stores the output in |out|.
# ; Stores encrypted partial block (if any) in gcm128ctx and its length in |pblocklen|.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_aes_gcm_encrypt_avx512
.type ossl_aes_gcm_encrypt_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_encrypt_avx512:
.cfi_startproc
.Lencrypt_seh_begin:
        endbranch
___

# ; NOTE: code before PROLOG() must not modify any registers
&PROLOG(
  1,    # allocate stack space for hkeys
  1,    # allocate stack space for AES blocks
  "encrypt");
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check aes_keys != NULL
        test               $arg1,$arg1
        jz                 .Lexit_gcm_encrypt

        # ;; Check gcm128ctx != NULL
        test               $arg2,$arg2
        jz                 .Lexit_gcm_encrypt

        # ;; Check pblocklen != NULL
        test               $arg3,$arg3
        jz                 .Lexit_gcm_encrypt

        # ;; Check in != NULL
        test               $arg4,$arg4
        jz                 .Lexit_gcm_encrypt

        # ;; Check if len != 0
        cmp                \$0,$arg5
        jz                 .Lexit_gcm_encrypt

        # ;; Check out != NULL
        cmp                \$0,$arg6
        jz                 .Lexit_gcm_encrypt
___
}
$code .= <<___;
        # ; load number of rounds from AES_KEY structure (offset in bytes is
        # ; size of the |rd_key| buffer)
        mov             `4*15*4`($arg1),%eax
        cmp             \$9,%eax
        je              .Laes_gcm_encrypt_128_avx512
        cmp             \$11,%eax
        je              .Laes_gcm_encrypt_192_avx512
        cmp             \$13,%eax
        je              .Laes_gcm_encrypt_256_avx512
        xor             %eax,%eax
        jmp             .Lexit_gcm_encrypt
___
for my $keylen (sort keys %aes_rounds) {
  $NROUNDS = $aes_rounds{$keylen};
  $code .= <<___;
.align 32
.Laes_gcm_encrypt_${keylen}_avx512:
___
  &GCM_ENC_DEC("$arg1", "$arg2", "$arg3", "$arg4", "$arg5", "$arg6", "ENC");
  $code .= "jmp .Lexit_gcm_encrypt\n";
}
$code .= ".Lexit_gcm_encrypt:\n";
&EPILOG(1, $arg5);
$code .= <<___;
ret
.Lencrypt_seh_end:
.cfi_endproc
.size ossl_aes_gcm_encrypt_avx512, .-ossl_aes_gcm_encrypt_avx512
___

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void   ossl_aes_gcm_decrypt_avx512
# ;       (const void* keys,
# ;        void *gcm128ctx,
# ;        unsigned int *pblocklen,
# ;        const unsigned char *in,
# ;        size_t len,
# ;        unsigned char *out);
# ;
# ; Performs decryption of data |in| of len |len|, and stores the output in |out|.
# ; Stores decrypted partial block (if any) in gcm128ctx and its length in |pblocklen|.
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_aes_gcm_decrypt_avx512
.type ossl_aes_gcm_decrypt_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_decrypt_avx512:
.cfi_startproc
.Ldecrypt_seh_begin:
        endbranch
___

# ; NOTE: code before PROLOG() must not modify any registers
&PROLOG(
  1,    # allocate stack space for hkeys
  1,    # allocate stack space for AES blocks
  "decrypt");
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check keys != NULL
        test               $arg1,$arg1
        jz                 .Lexit_gcm_decrypt

        # ;; Check gcm128ctx != NULL
        test               $arg2,$arg2
        jz                 .Lexit_gcm_decrypt

        # ;; Check pblocklen != NULL
        test               $arg3,$arg3
        jz                 .Lexit_gcm_decrypt

        # ;; Check in != NULL
        test               $arg4,$arg4
        jz                 .Lexit_gcm_decrypt

        # ;; Check if len != 0
        cmp                \$0,$arg5
        jz                 .Lexit_gcm_decrypt

        # ;; Check out != NULL
        cmp                \$0,$arg6
        jz                 .Lexit_gcm_decrypt
___
}
$code .= <<___;
        # ; load number of rounds from AES_KEY structure (offset in bytes is
        # ; size of the |rd_key| buffer)
        mov             `4*15*4`($arg1),%eax
        cmp             \$9,%eax
        je              .Laes_gcm_decrypt_128_avx512
        cmp             \$11,%eax
        je              .Laes_gcm_decrypt_192_avx512
        cmp             \$13,%eax
        je              .Laes_gcm_decrypt_256_avx512
        xor             %eax,%eax
        jmp             .Lexit_gcm_decrypt
___
for my $keylen (sort keys %aes_rounds) {
  $NROUNDS = $aes_rounds{$keylen};
  $code .= <<___;
.align 32
.Laes_gcm_decrypt_${keylen}_avx512:
___
  &GCM_ENC_DEC("$arg1", "$arg2", "$arg3", "$arg4", "$arg5", "$arg6", "DEC");
  $code .= "jmp .Lexit_gcm_decrypt\n";
}
$code .= ".Lexit_gcm_decrypt:\n";
&EPILOG(1, $arg5);
$code .= <<___;
ret
.Ldecrypt_seh_end:
.cfi_endproc
.size ossl_aes_gcm_decrypt_avx512, .-ossl_aes_gcm_decrypt_avx512
___

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void   ossl_aes_gcm_finalize_vaes_avx512
# ;       (void *gcm128ctx,
# ;       unsigned int pblocklen);
# ;
# ; Finalizes encryption / decryption
# ; Leaf function (does not allocate stack space, does not use non-volatile registers).
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_aes_gcm_finalize_avx512
.type ossl_aes_gcm_finalize_avx512,\@abi-omnipotent
.align 32
ossl_aes_gcm_finalize_avx512:
.cfi_startproc
        endbranch
___
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check gcm128ctx != NULL
        test               $arg1,$arg1
        jz                 .Labort_finalize
___
}

&GCM_COMPLETE("$arg1", "$arg2");

$code .= <<___;
.Labort_finalize:
ret
.cfi_endproc
.size ossl_aes_gcm_finalize_avx512, .-ossl_aes_gcm_finalize_avx512
___

# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
# ;void ossl_gcm_gmult_avx512(u64 Xi[2],
# ;                           const void* gcm128ctx)
# ;
# ; Leaf function (does not allocate stack space, does not use non-volatile registers).
# ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;
$code .= <<___;
.globl ossl_gcm_gmult_avx512
.hidden ossl_gcm_gmult_avx512
.type ossl_gcm_gmult_avx512,\@abi-omnipotent
.align 32
ossl_gcm_gmult_avx512:
.cfi_startproc
        endbranch
___
if ($CHECK_FUNCTION_ARGUMENTS) {
  $code .= <<___;
        # ;; Check Xi != NULL
        test               $arg1,$arg1
        jz                 .Labort_gmult

        # ;; Check gcm128ctx != NULL
        test               $arg2,$arg2
        jz                 .Labort_gmult
___
}
$code .= "vmovdqu64         ($arg1),%xmm1\n";
$code .= "vmovdqu64         @{[HashKeyByIdx(1,$arg2)]},%xmm2\n";

&GHASH_MUL("%xmm1", "%xmm2", "%xmm3", "%xmm4", "%xmm5");

$code .= "vmovdqu64         %xmm1,($arg1)\n";
if ($CLEAR_SCRATCH_REGISTERS) {
  &clear_scratch_gps_asm();
  &clear_scratch_zmms_asm();
} else {
  $code .= "vzeroupper\n";
}
$code .= <<___;
.Labort_gmult:
ret
.cfi_endproc
.size ossl_gcm_gmult_avx512, .-ossl_gcm_gmult_avx512
___

if ($win64) {

  # Add unwind metadata for SEH.

  # See https://docs.microsoft.com/en-us/cpp/build/exception-handling-x64?view=msvc-160
  my $UWOP_PUSH_NONVOL = 0;
  my $UWOP_ALLOC_LARGE = 1;
  my $UWOP_SET_FPREG   = 3;
  my $UWOP_SAVE_XMM128 = 8;
  my %UWOP_REG_NUMBER  = (
    rax => 0,
    rcx => 1,
    rdx => 2,
    rbx => 3,
    rsp => 4,
    rbp => 5,
    rsi => 6,
    rdi => 7,
    map(("r$_" => $_), (8 .. 15)));

  $code .= <<___;
.section    .pdata
.align  4
    .rva    .Lsetiv_seh_begin
    .rva    .Lsetiv_seh_end
    .rva    .Lsetiv_seh_info

    .rva    .Lghash_seh_begin
    .rva    .Lghash_seh_end
    .rva    .Lghash_seh_info

    .rva    .Lencrypt_seh_begin
    .rva    .Lencrypt_seh_end
    .rva    .Lencrypt_seh_info

    .rva    .Ldecrypt_seh_begin
    .rva    .Ldecrypt_seh_end
    .rva    .Ldecrypt_seh_info

.section    .xdata
___

  foreach my $func_name ("setiv", "ghash", "encrypt", "decrypt") {
    $code .= <<___;
.align  8
.L${func_name}_seh_info:
    .byte   1   # version 1, no flags
    .byte   .L${func_name}_seh_prolog_end-.L${func_name}_seh_begin
    .byte   31 # num_slots = 1*8 + 2 + 1 + 2*10
    # FR = rbp; Offset from RSP = $XMM_STORAGE scaled on 16
    .byte   @{[$UWOP_REG_NUMBER{rbp} | (($XMM_STORAGE / 16 ) << 4)]}
___

    # Metadata for %xmm15-%xmm6
    # Occupy 2 slots each
    for (my $reg_idx = 15; $reg_idx >= 6; $reg_idx--) {

      # Scaled-by-16 stack offset
      my $xmm_reg_offset = ($reg_idx - 6);
      $code .= <<___;
    .byte   .L${func_name}_seh_save_xmm${reg_idx}-.L${func_name}_seh_begin
    .byte   @{[$UWOP_SAVE_XMM128 | (${reg_idx} << 4)]}
    .value  $xmm_reg_offset
___
    }

    $code .= <<___;
    # Frame pointer (occupy 1 slot)
    .byte   .L${func_name}_seh_setfp-.L${func_name}_seh_begin
    .byte   $UWOP_SET_FPREG

    # Occupy 2 slots, as stack allocation < 512K, but > 128 bytes
    .byte   .L${func_name}_seh_allocstack_xmm-.L${func_name}_seh_begin
    .byte   $UWOP_ALLOC_LARGE
    .value  `($XMM_STORAGE + 8) / 8`
___

    # Metadata for GPR regs
    # Occupy 1 slot each
    foreach my $reg ("rsi", "rdi", "r15", "r14", "r13", "r12", "rbp", "rbx") {
      $code .= <<___;
    .byte   .L${func_name}_seh_push_${reg}-.L${func_name}_seh_begin
    .byte   @{[$UWOP_PUSH_NONVOL | ($UWOP_REG_NUMBER{$reg} << 4)]}
___
    }
  }
}

$code .= <<___;
.section .rodata align=16
.align 16
POLY:   .quad     0x0000000000000001, 0xC200000000000000

.align 64
POLY2:
        .quad     0x00000001C2000000, 0xC200000000000000
        .quad     0x00000001C2000000, 0xC200000000000000
        .quad     0x00000001C2000000, 0xC200000000000000
        .quad     0x00000001C2000000, 0xC200000000000000

.align 16
TWOONE: .quad     0x0000000000000001, 0x0000000100000000

# ;;; Order of these constants should not change.
# ;;; More specifically, ALL_F should follow SHIFT_MASK, and ZERO should follow ALL_F
.align 64
SHUF_MASK:
        .quad     0x08090A0B0C0D0E0F, 0x0001020304050607
        .quad     0x08090A0B0C0D0E0F, 0x0001020304050607
        .quad     0x08090A0B0C0D0E0F, 0x0001020304050607
        .quad     0x08090A0B0C0D0E0F, 0x0001020304050607

.align 16
SHIFT_MASK:
        .quad     0x0706050403020100, 0x0f0e0d0c0b0a0908

ALL_F:
        .quad     0xffffffffffffffff, 0xffffffffffffffff

ZERO:
        .quad     0x0000000000000000, 0x0000000000000000

.align 16
ONE:
        .quad     0x0000000000000001, 0x0000000000000000

.align 16
ONEf:
        .quad     0x0000000000000000, 0x0100000000000000

.align 64
ddq_add_1234:
        .quad  0x0000000000000001, 0x0000000000000000
        .quad  0x0000000000000002, 0x0000000000000000
        .quad  0x0000000000000003, 0x0000000000000000
        .quad  0x0000000000000004, 0x0000000000000000

.align 64
ddq_add_5678:
        .quad  0x0000000000000005, 0x0000000000000000
        .quad  0x0000000000000006, 0x0000000000000000
        .quad  0x0000000000000007, 0x0000000000000000
        .quad  0x0000000000000008, 0x0000000000000000

.align 64
ddq_add_4444:
        .quad  0x0000000000000004, 0x0000000000000000
        .quad  0x0000000000000004, 0x0000000000000000
        .quad  0x0000000000000004, 0x0000000000000000
        .quad  0x0000000000000004, 0x0000000000000000

.align 64
ddq_add_8888:
        .quad  0x0000000000000008, 0x0000000000000000
        .quad  0x0000000000000008, 0x0000000000000000
        .quad  0x0000000000000008, 0x0000000000000000
        .quad  0x0000000000000008, 0x0000000000000000

.align 64
ddq_addbe_1234:
        .quad  0x0000000000000000, 0x0100000000000000
        .quad  0x0000000000000000, 0x0200000000000000
        .quad  0x0000000000000000, 0x0300000000000000
        .quad  0x0000000000000000, 0x0400000000000000

.align 64
ddq_addbe_4444:
        .quad  0x0000000000000000, 0x0400000000000000
        .quad  0x0000000000000000, 0x0400000000000000
        .quad  0x0000000000000000, 0x0400000000000000
        .quad  0x0000000000000000, 0x0400000000000000

.align 64
byte_len_to_mask_table:
        .value      0x0000, 0x0001, 0x0003, 0x0007
        .value      0x000f, 0x001f, 0x003f, 0x007f
        .value      0x00ff, 0x01ff, 0x03ff, 0x07ff
        .value      0x0fff, 0x1fff, 0x3fff, 0x7fff
        .value      0xffff

.align 64
byte64_len_to_mask_table:
        .quad      0x0000000000000000, 0x0000000000000001
        .quad      0x0000000000000003, 0x0000000000000007
        .quad      0x000000000000000f, 0x000000000000001f
        .quad      0x000000000000003f, 0x000000000000007f
        .quad      0x00000000000000ff, 0x00000000000001ff
        .quad      0x00000000000003ff, 0x00000000000007ff
        .quad      0x0000000000000fff, 0x0000000000001fff
        .quad      0x0000000000003fff, 0x0000000000007fff
        .quad      0x000000000000ffff, 0x000000000001ffff
        .quad      0x000000000003ffff, 0x000000000007ffff
        .quad      0x00000000000fffff, 0x00000000001fffff
        .quad      0x00000000003fffff, 0x00000000007fffff
        .quad      0x0000000000ffffff, 0x0000000001ffffff
        .quad      0x0000000003ffffff, 0x0000000007ffffff
        .quad      0x000000000fffffff, 0x000000001fffffff
        .quad      0x000000003fffffff, 0x000000007fffffff
        .quad      0x00000000ffffffff, 0x00000001ffffffff
        .quad      0x00000003ffffffff, 0x00000007ffffffff
        .quad      0x0000000fffffffff, 0x0000001fffffffff
        .quad      0x0000003fffffffff, 0x0000007fffffffff
        .quad      0x000000ffffffffff, 0x000001ffffffffff
        .quad      0x000003ffffffffff, 0x000007ffffffffff
        .quad      0x00000fffffffffff, 0x00001fffffffffff
        .quad      0x00003fffffffffff, 0x00007fffffffffff
        .quad      0x0000ffffffffffff, 0x0001ffffffffffff
        .quad      0x0003ffffffffffff, 0x0007ffffffffffff
        .quad      0x000fffffffffffff, 0x001fffffffffffff
        .quad      0x003fffffffffffff, 0x007fffffffffffff
        .quad      0x00ffffffffffffff, 0x01ffffffffffffff
        .quad      0x03ffffffffffffff, 0x07ffffffffffffff
        .quad      0x0fffffffffffffff, 0x1fffffffffffffff
        .quad      0x3fffffffffffffff, 0x7fffffffffffffff
        .quad      0xffffffffffffffff
___

} else {
# Fallback for old assembler
$code .= <<___;
.text
.globl  ossl_vaes_vpclmulqdq_capable
.type   ossl_vaes_vpclmulqdq_capable,\@abi-omnipotent
ossl_vaes_vpclmulqdq_capable:
    xor     %eax,%eax
    ret
.size   ossl_vaes_vpclmulqdq_capable, .-ossl_vaes_vpclmulqdq_capable

.globl ossl_aes_gcm_init_avx512
.globl ossl_aes_gcm_setiv_avx512
.globl ossl_aes_gcm_update_aad_avx512
.globl ossl_aes_gcm_encrypt_avx512
.globl ossl_aes_gcm_decrypt_avx512
.globl ossl_aes_gcm_finalize_avx512
.globl ossl_gcm_gmult_avx512

.type ossl_aes_gcm_init_avx512,\@abi-omnipotent
ossl_aes_gcm_init_avx512:
ossl_aes_gcm_setiv_avx512:
ossl_aes_gcm_update_aad_avx512:
ossl_aes_gcm_encrypt_avx512:
ossl_aes_gcm_decrypt_avx512:
ossl_aes_gcm_finalize_avx512:
ossl_gcm_gmult_avx512:
    .byte   0x0f,0x0b    # ud2
    ret
.size   ossl_aes_gcm_init_avx512, .-ossl_aes_gcm_init_avx512
___
}

$code =~ s/\`([^\`]*)\`/eval $1/gem;
print $code;
close STDOUT or die "error closing STDOUT: $!";
