// Copyright 1995-2016 The OpenSSL Project Authors. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#include <ring-core/base.h>


#if !defined(OPENSSL_NO_ASM) && (defined(OPENSSL_X86) || defined(OPENSSL_X86_64))

#if defined(_MSC_VER) && !defined(__clang__)
#pragma warning(push, 3)
#include <immintrin.h>
#include <intrin.h>
#pragma warning(pop)
#endif

#include "internal.h"


// OPENSSL_cpuid runs the cpuid instruction. |leaf| is passed in as EAX and ECX
// is set to zero. It writes EAX, EBX, ECX, and EDX to |*out_eax| through
// |*out_edx|.
static void OPENSSL_cpuid(uint32_t *out_eax, uint32_t *out_ebx,
                          uint32_t *out_ecx, uint32_t *out_edx, uint32_t leaf) {
#if defined(_MSC_VER) && !defined(__clang__)
  int tmp[4];
  __cpuid(tmp, (int)leaf);
  *out_eax = (uint32_t)tmp[0];
  *out_ebx = (uint32_t)tmp[1];
  *out_ecx = (uint32_t)tmp[2];
  *out_edx = (uint32_t)tmp[3];
#elif defined(__pic__) && defined(OPENSSL_32_BIT)
  // Inline assembly may not clobber the PIC register. For 32-bit, this is EBX.
  // See https://gcc.gnu.org/bugzilla/show_bug.cgi?id=47602.
  __asm__ volatile (
    "xor %%ecx, %%ecx\n"
    "mov %%ebx, %%edi\n"
    "cpuid\n"
    "xchg %%edi, %%ebx\n"
    : "=a"(*out_eax), "=D"(*out_ebx), "=c"(*out_ecx), "=d"(*out_edx)
    : "a"(leaf)
  );
#else
  __asm__ volatile (
    "xor %%ecx, %%ecx\n"
    "cpuid\n"
    : "=a"(*out_eax), "=b"(*out_ebx), "=c"(*out_ecx), "=d"(*out_edx)
    : "a"(leaf)
  );
#endif
}

// OPENSSL_xgetbv returns the value of an Intel Extended Control Register (XCR).
// Currently only XCR0 is defined by Intel so |xcr| should always be zero.
//
// See https://software.intel.com/en-us/articles/how-to-detect-new-instruction-support-in-the-4th-generation-intel-core-processor-family
static uint64_t OPENSSL_xgetbv(uint32_t xcr) {
#if defined(_MSC_VER) && !defined(__clang__)
  return (uint64_t)_xgetbv(xcr);
#else
  uint32_t eax, edx;
  __asm__ volatile ("xgetbv" : "=a"(eax), "=d"(edx) : "c"(xcr));
  return (((uint64_t)edx) << 32) | eax;
#endif
}

void OPENSSL_cpuid_setup(uint32_t OPENSSL_ia32cap_P[4]) {
  // Determine the vendor and maximum input value.
  uint32_t eax, ebx, ecx, edx;
  OPENSSL_cpuid(&eax, &ebx, &ecx, &edx, 0);

  uint32_t num_ids = eax;

  int is_intel = ebx == 0x756e6547 /* Genu */ &&
                 edx == 0x49656e69 /* ineI */ &&
                 ecx == 0x6c65746e /* ntel */;

  uint32_t extended_features[2] = {0};
  if (num_ids >= 7) {
    OPENSSL_cpuid(&eax, &ebx, &ecx, &edx, 7);
    extended_features[0] = ebx;
    extended_features[1] = ecx;
  }

  OPENSSL_cpuid(&eax, &ebx, &ecx, &edx, 1);

  const uint32_t base_family = (eax >> 8) & 15;
  const uint32_t base_model = (eax >> 4) & 15;

  uint32_t family = base_family;
  uint32_t model = base_model;
  if (base_family == 15) {
    const uint32_t ext_family = (eax >> 20) & 255;
    family += ext_family;
  }
  if (base_family == 6 || base_family == 15) {
    const uint32_t ext_model = (eax >> 16) & 15;
    model |= ext_model << 4;
  }

  // Reserved bit #30 is repurposed to signal an Intel CPU.
  if (is_intel) {
    edx |= (1u << 30);
  } else {
    edx &= ~(1u << 30);
  }

  uint64_t xcr0 = 0;
  if (ecx & (1u << 27)) {
    // XCR0 may only be queried if the OSXSAVE bit is set.
    xcr0 = OPENSSL_xgetbv(0);
  }
  // See Intel manual, volume 1, section 14.3.
  if ((xcr0 & 6) != 6) {
    // YMM registers cannot be used.
    ecx &= ~(1u << 28);  // AVX
    ecx &= ~(1u << 12);  // FMA
    ecx &= ~(1u << 11);  // AMD XOP
    extended_features[0] &= ~(1u << 5);   // AVX2
    extended_features[1] &= ~(1u << 9);   // VAES
    extended_features[1] &= ~(1u << 10);  // VPCLMULQDQ
  }
  // See Intel manual, volume 1, sections 15.2 ("Detection of AVX-512 Foundation
  // Instructions") through 15.4 ("Detection of Intel AVX-512 Instruction Groups
  // Operating at 256 and 128-bit Vector Lengths").
  if ((xcr0 & 0xe6) != 0xe6) {
    // Without XCR0.111xx11x, no AVX512 feature can be used. This includes ZMM
    // registers, masking, SIMD registers 16-31 (even if accessed as YMM or
    // XMM), and EVEX-coded instructions (even on YMM or XMM). Even if only
    // XCR0.ZMM_Hi256 is missing, it isn't valid to use AVX512 features on
    // shorter vectors, since AVX512 ties everything to the availability of
    // 512-bit vectors. See the above-mentioned sections of the Intel manual,
    // which say that *all* these XCR0 bits must be checked even when just using
    // 128-bit or 256-bit vectors, and also volume 2a section 2.7.11 ("#UD
    // Equations for EVEX") which says that all EVEX-coded instructions raise an
    // undefined-instruction exception if any of these XCR0 bits is zero.
    //
    // AVX10 fixes this by reorganizing the features that used to be part of
    // "AVX512" and allowing them to be used independently of 512-bit support.
    // TODO: add AVX10 detection.
    extended_features[0] &= ~(1u << 16);  // AVX512F
    extended_features[0] &= ~(1u << 17);  // AVX512DQ
    extended_features[0] &= ~(1u << 21);  // AVX512IFMA
    extended_features[0] &= ~(1u << 26);  // AVX512PF
    extended_features[0] &= ~(1u << 27);  // AVX512ER
    extended_features[0] &= ~(1u << 28);  // AVX512CD
    extended_features[0] &= ~(1u << 30);  // AVX512BW
    extended_features[0] &= ~(1u << 31);  // AVX512VL
    extended_features[1] &= ~(1u << 1);   // AVX512VBMI
    extended_features[1] &= ~(1u << 6);   // AVX512VBMI2
    extended_features[1] &= ~(1u << 11);  // AVX512VNNI
    extended_features[1] &= ~(1u << 12);  // AVX512BITALG
    extended_features[1] &= ~(1u << 14);  // AVX512VPOPCNTDQ
  }

  // Repurpose the bit for the removed MPX feature to indicate when using zmm
  // registers should be avoided even when they are supported. (When set, AVX512
  // features can still be used, but only using ymm or xmm registers.) Skylake
  // suffered from severe downclocking when zmm registers were used, which
  // affected unrelated code running on the system, making zmm registers not too
  // useful outside of benchmarks. The situation improved significantly by Ice
  // Lake, but a small amount of downclocking remained. (See
  // https://lore.kernel.org/linux-crypto/e8ce1146-3952-6977-1d0e-a22758e58914@intel.com/)
  // We take a conservative approach of not allowing zmm registers until after
  // Ice Lake and Tiger Lake, i.e. until Sapphire Rapids on the server side.
  //
  // AMD CPUs, which support AVX512 starting with Zen 4, have not been reported
  // to have any downclocking problem when zmm registers are used.
  if (is_intel && family == 6 &&
      (model == 85 ||    // Skylake, Cascade Lake, Cooper Lake (server)
       model == 106 ||   // Ice Lake (server)
       model == 108 ||   // Ice Lake (micro server)
       model == 125 ||   // Ice Lake (client)
       model == 126 ||   // Ice Lake (mobile)
       model == 140 ||   // Tiger Lake (mobile)
       model == 141)) {  // Tiger Lake (client)
    extended_features[0] |= 1u << 14;
  } else {
    extended_features[0] &= ~(1u << 14);
  }

  OPENSSL_ia32cap_P[0] = edx;
  OPENSSL_ia32cap_P[1] = ecx;
  OPENSSL_ia32cap_P[2] = extended_features[0];
  OPENSSL_ia32cap_P[3] = extended_features[1];
}

#endif  // !OPENSSL_NO_ASM && (OPENSSL_X86 || OPENSSL_X86_64)
