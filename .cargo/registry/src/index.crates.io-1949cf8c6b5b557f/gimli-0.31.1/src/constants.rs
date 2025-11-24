// This file originally from https://github.com/philipc/rust-dwarf/ and
// distributed under either MIT or Apache 2.0 licenses.
//
// Copyright 2016 The rust-dwarf Developers
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

//! Constant definitions.
//!
//! The DWARF spec's `DW_AT_*` type is represented as `struct DwAt(u16)`,
//! `DW_FORM_*` as `DwForm(u16)`, etc.
//!
//! There are also exported const definitions for each constant.

#![allow(non_upper_case_globals)]
#![allow(missing_docs)]

use core::{fmt, ops};

// The `dw!` macro turns this:
//
//     dw!(DwFoo(u32) {
//         DW_FOO_bar = 0,
//         DW_FOO_baz = 1,
//         DW_FOO_bang = 2,
//     });
//
// into this:
//
//     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
//     pub struct DwFoo(pub u32);
//
//     pub const DW_FOO_bar: DwFoo = DwFoo(0);
//     pub const DW_FOO_baz: DwFoo = DwFoo(1);
//     pub const DW_FOO_bang: DwFoo = DwFoo(2);
//
//     impl DwFoo {
//         pub fn static_string(&self) -> Option<&'static str> {
//             ...
//         }
//     }
//
//     impl fmt::Display for DwFoo {
//         fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
//             ...
//         }
//     }
macro_rules! dw {
    ($(#[$meta:meta])* $struct_name:ident($struct_type:ty)
        { $($name:ident = $val:expr),+ $(,)? }
        $(, aliases { $($alias_name:ident = $alias_val:expr),+ $(,)? })?
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $struct_name(pub $struct_type);

        $(
            pub const $name: $struct_name = $struct_name($val);
        )+
        $($(
            pub const $alias_name: $struct_name = $struct_name($alias_val);
        )+)*

        impl $struct_name {
            pub fn static_string(&self) -> Option<&'static str> {
                Some(match *self {
                    $(
                        $name => stringify!($name),
                    )+
                    _ => return None,
                })
            }
        }

        impl fmt::Display for $struct_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
                if let Some(s) = self.static_string() {
                    f.pad(s)
                } else {
                    #[cfg(feature = "read")]
                    {
                        f.pad(&format!("Unknown {}: {}", stringify!($struct_name), self.0))
                    }
                    #[cfg(not(feature = "read"))]
                    {
                        write!(f, "Unknown {}: {}", stringify!($struct_name), self.0)
                    }
                }
            }
        }
    };
}

dw!(
/// The section type field in a `.dwp` unit index.
///
/// This is used for version 5 and later.
///
/// See Section 7.3.5.
DwSect(u32) {
    DW_SECT_INFO = 1,
    DW_SECT_ABBREV = 3,
    DW_SECT_LINE = 4,
    DW_SECT_LOCLISTS = 5,
    DW_SECT_STR_OFFSETS = 6,
    DW_SECT_MACRO = 7,
    DW_SECT_RNGLISTS = 8,
});

dw!(
/// The section type field in a `.dwp` unit index with version 2.
DwSectV2(u32) {
    DW_SECT_V2_INFO = 1,
    DW_SECT_V2_TYPES = 2,
    DW_SECT_V2_ABBREV = 3,
    DW_SECT_V2_LINE = 4,
    DW_SECT_V2_LOC = 5,
    DW_SECT_V2_STR_OFFSETS = 6,
    DW_SECT_V2_MACINFO = 7,
    DW_SECT_V2_MACRO = 8,
});

dw!(
/// The unit type field in a unit header.
///
/// See Section 7.5.1, Table 7.2.
DwUt(u8) {
    DW_UT_compile = 0x01,
    DW_UT_type = 0x02,
    DW_UT_partial = 0x03,
    DW_UT_skeleton = 0x04,
    DW_UT_split_compile = 0x05,
    DW_UT_split_type = 0x06,
    DW_UT_lo_user = 0x80,
    DW_UT_hi_user = 0xff,
});

dw!(
/// The opcode for a call frame instruction.
///
/// Section 7.24:
/// > Call frame instructions are encoded in one or more bytes. The primary
/// > opcode is encoded in the high order two bits of the first byte (that is,
/// > opcode = byte >> 6). An operand or extended opcode may be encoded in the
/// > low order 6 bits. Additional operands are encoded in subsequent bytes.
DwCfa(u8) {
    DW_CFA_advance_loc = 0x01 << 6,
    DW_CFA_offset = 0x02 << 6,
    DW_CFA_restore = 0x03 << 6,
    DW_CFA_nop = 0,
    DW_CFA_set_loc = 0x01,
    DW_CFA_advance_loc1 = 0x02,
    DW_CFA_advance_loc2 = 0x03,
    DW_CFA_advance_loc4 = 0x04,
    DW_CFA_offset_extended = 0x05,
    DW_CFA_restore_extended = 0x06,
    DW_CFA_undefined = 0x07,
    DW_CFA_same_value = 0x08,
    DW_CFA_register = 0x09,
    DW_CFA_remember_state = 0x0a,
    DW_CFA_restore_state = 0x0b,
    DW_CFA_def_cfa = 0x0c,
    DW_CFA_def_cfa_register = 0x0d,
    DW_CFA_def_cfa_offset = 0x0e,
    DW_CFA_def_cfa_expression = 0x0f,
    DW_CFA_expression = 0x10,
    DW_CFA_offset_extended_sf = 0x11,
    DW_CFA_def_cfa_sf = 0x12,
    DW_CFA_def_cfa_offset_sf = 0x13,
    DW_CFA_val_offset = 0x14,
    DW_CFA_val_offset_sf = 0x15,
    DW_CFA_val_expression = 0x16,

    DW_CFA_lo_user = 0x1c,
    DW_CFA_hi_user = 0x3f,

    DW_CFA_MIPS_advance_loc8 = 0x1d,
    DW_CFA_GNU_window_save = 0x2d,
    DW_CFA_GNU_args_size = 0x2e,
    DW_CFA_GNU_negative_offset_extended = 0x2f,
},
aliases {
    DW_CFA_AARCH64_negate_ra_state = 0x2d,
});

dw!(
/// The child determination encodings for DIE attributes.
///
/// See Section 7.5.3, Table 7.4.
DwChildren(u8) {
    DW_CHILDREN_no = 0,
    DW_CHILDREN_yes = 1,
});

dw!(
/// The tag encodings for DIE attributes.
///
/// See Section 7.5.3, Table 7.3.
DwTag(u16) {
    DW_TAG_null = 0x00,

    DW_TAG_array_type = 0x01,
    DW_TAG_class_type = 0x02,
    DW_TAG_entry_point = 0x03,
    DW_TAG_enumeration_type = 0x04,
    DW_TAG_formal_parameter = 0x05,
    DW_TAG_imported_declaration = 0x08,
    DW_TAG_label = 0x0a,
    DW_TAG_lexical_block = 0x0b,
    DW_TAG_member = 0x0d,
    DW_TAG_pointer_type = 0x0f,
    DW_TAG_reference_type = 0x10,
    DW_TAG_compile_unit = 0x11,
    DW_TAG_string_type = 0x12,
    DW_TAG_structure_type = 0x13,
    DW_TAG_subroutine_type = 0x15,
    DW_TAG_typedef = 0x16,
    DW_TAG_union_type = 0x17,
    DW_TAG_unspecified_parameters = 0x18,
    DW_TAG_variant = 0x19,
    DW_TAG_common_block = 0x1a,
    DW_TAG_common_inclusion = 0x1b,
    DW_TAG_inheritance = 0x1c,
    DW_TAG_inlined_subroutine = 0x1d,
    DW_TAG_module = 0x1e,
    DW_TAG_ptr_to_member_type = 0x1f,
    DW_TAG_set_type = 0x20,
    DW_TAG_subrange_type = 0x21,
    DW_TAG_with_stmt = 0x22,
    DW_TAG_access_declaration = 0x23,
    DW_TAG_base_type = 0x24,
    DW_TAG_catch_block = 0x25,
    DW_TAG_const_type = 0x26,
    DW_TAG_constant = 0x27,
    DW_TAG_enumerator = 0x28,
    DW_TAG_file_type = 0x29,
    DW_TAG_friend = 0x2a,
    DW_TAG_namelist = 0x2b,
    DW_TAG_namelist_item = 0x2c,
    DW_TAG_packed_type = 0x2d,
    DW_TAG_subprogram = 0x2e,
    DW_TAG_template_type_parameter = 0x2f,
    DW_TAG_template_value_parameter = 0x30,
    DW_TAG_thrown_type = 0x31,
    DW_TAG_try_block = 0x32,
    DW_TAG_variant_part = 0x33,
    DW_TAG_variable = 0x34,
    DW_TAG_volatile_type = 0x35,

// DWARF 3.
    DW_TAG_dwarf_procedure = 0x36,
    DW_TAG_restrict_type = 0x37,
    DW_TAG_interface_type = 0x38,
    DW_TAG_namespace = 0x39,
    DW_TAG_imported_module = 0x3a,
    DW_TAG_unspecified_type = 0x3b,
    DW_TAG_partial_unit = 0x3c,
    DW_TAG_imported_unit = 0x3d,
    DW_TAG_condition = 0x3f,
    DW_TAG_shared_type = 0x40,

// DWARF 4.
    DW_TAG_type_unit = 0x41,
    DW_TAG_rvalue_reference_type = 0x42,
    DW_TAG_template_alias = 0x43,

// DWARF 5.
    DW_TAG_coarray_type = 0x44,
    DW_TAG_generic_subrange = 0x45,
    DW_TAG_dynamic_type = 0x46,
    DW_TAG_atomic_type = 0x47,
    DW_TAG_call_site = 0x48,
    DW_TAG_call_site_parameter = 0x49,
    DW_TAG_skeleton_unit = 0x4a,
    DW_TAG_immutable_type = 0x4b,

    DW_TAG_lo_user = 0x4080,
    DW_TAG_hi_user = 0xffff,

// SGI/MIPS extensions.
    DW_TAG_MIPS_loop = 0x4081,

// HP extensions.
    DW_TAG_HP_array_descriptor = 0x4090,
    DW_TAG_HP_Bliss_field = 0x4091,
    DW_TAG_HP_Bliss_field_set = 0x4092,

// GNU extensions.
    DW_TAG_format_label = 0x4101,
    DW_TAG_function_template = 0x4102,
    DW_TAG_class_template = 0x4103,
    DW_TAG_GNU_BINCL = 0x4104,
    DW_TAG_GNU_EINCL = 0x4105,
    DW_TAG_GNU_template_template_param = 0x4106,
    DW_TAG_GNU_template_parameter_pack = 0x4107,
    DW_TAG_GNU_formal_parameter_pack = 0x4108,
    DW_TAG_GNU_call_site = 0x4109,
    DW_TAG_GNU_call_site_parameter = 0x410a,

    DW_TAG_APPLE_property = 0x4200,

// SUN extensions.
    DW_TAG_SUN_function_template = 0x4201,
    DW_TAG_SUN_class_template = 0x4202,
    DW_TAG_SUN_struct_template = 0x4203,
    DW_TAG_SUN_union_template = 0x4204,
    DW_TAG_SUN_indirect_inheritance = 0x4205,
    DW_TAG_SUN_codeflags = 0x4206,
    DW_TAG_SUN_memop_info = 0x4207,
    DW_TAG_SUN_omp_child_func = 0x4208,
    DW_TAG_SUN_rtti_descriptor = 0x4209,
    DW_TAG_SUN_dtor_info = 0x420a,
    DW_TAG_SUN_dtor = 0x420b,
    DW_TAG_SUN_f90_interface = 0x420c,
    DW_TAG_SUN_fortran_vax_structure = 0x420d,

// ALTIUM extensions.
    DW_TAG_ALTIUM_circ_type = 0x5101,
    DW_TAG_ALTIUM_mwa_circ_type = 0x5102,
    DW_TAG_ALTIUM_rev_carry_type = 0x5103,
    DW_TAG_ALTIUM_rom = 0x5111,

// Extensions for UPC.
    DW_TAG_upc_shared_type = 0x8765,
    DW_TAG_upc_strict_type = 0x8766,
    DW_TAG_upc_relaxed_type = 0x8767,

// PGI (STMicroelectronics) extensions.
    DW_TAG_PGI_kanji_type = 0xa000,
    DW_TAG_PGI_interface_block = 0xa020,

// Borland extensions.
    DW_TAG_BORLAND_property = 0xb000,
    DW_TAG_BORLAND_Delphi_string = 0xb001,
    DW_TAG_BORLAND_Delphi_dynamic_array = 0xb002,
    DW_TAG_BORLAND_Delphi_set = 0xb003,
    DW_TAG_BORLAND_Delphi_variant = 0xb004,
});

dw!(
/// The attribute encodings for DIE attributes.
///
/// See Section 7.5.4, Table 7.5.
DwAt(u16) {
    DW_AT_null = 0x00,

    DW_AT_sibling = 0x01,
    DW_AT_location = 0x02,
    DW_AT_name = 0x03,
    DW_AT_ordering = 0x09,
    DW_AT_byte_size = 0x0b,
    DW_AT_bit_offset = 0x0c,
    DW_AT_bit_size = 0x0d,
    DW_AT_stmt_list = 0x10,
    DW_AT_low_pc = 0x11,
    DW_AT_high_pc = 0x12,
    DW_AT_language = 0x13,
    DW_AT_discr = 0x15,
    DW_AT_discr_value = 0x16,
    DW_AT_visibility = 0x17,
    DW_AT_import = 0x18,
    DW_AT_string_length = 0x19,
    DW_AT_common_reference = 0x1a,
    DW_AT_comp_dir = 0x1b,
    DW_AT_const_value = 0x1c,
    DW_AT_containing_type = 0x1d,
    DW_AT_default_value = 0x1e,
    DW_AT_inline = 0x20,
    DW_AT_is_optional = 0x21,
    DW_AT_lower_bound = 0x22,
    DW_AT_producer = 0x25,
    DW_AT_prototyped = 0x27,
    DW_AT_return_addr = 0x2a,
    DW_AT_start_scope = 0x2c,
    DW_AT_bit_stride = 0x2e,
    DW_AT_upper_bound = 0x2f,
    DW_AT_abstract_origin = 0x31,
    DW_AT_accessibility = 0x32,
    DW_AT_address_class = 0x33,
    DW_AT_artificial = 0x34,
    DW_AT_base_types = 0x35,
    DW_AT_calling_convention = 0x36,
    DW_AT_count = 0x37,
    DW_AT_data_member_location = 0x38,
    DW_AT_decl_column = 0x39,
    DW_AT_decl_file = 0x3a,
    DW_AT_decl_line = 0x3b,
    DW_AT_declaration = 0x3c,
    DW_AT_discr_list = 0x3d,
    DW_AT_encoding = 0x3e,
    DW_AT_external = 0x3f,
    DW_AT_frame_base = 0x40,
    DW_AT_friend = 0x41,
    DW_AT_identifier_case = 0x42,
    DW_AT_macro_info = 0x43,
    DW_AT_namelist_item = 0x44,
    DW_AT_priority = 0x45,
    DW_AT_segment = 0x46,
    DW_AT_specification = 0x47,
    DW_AT_static_link = 0x48,
    DW_AT_type = 0x49,
    DW_AT_use_location = 0x4a,
    DW_AT_variable_parameter = 0x4b,
    DW_AT_virtuality = 0x4c,
    DW_AT_vtable_elem_location = 0x4d,

// DWARF 3.
    DW_AT_allocated = 0x4e,
    DW_AT_associated = 0x4f,
    DW_AT_data_location = 0x50,
    DW_AT_byte_stride = 0x51,
    DW_AT_entry_pc = 0x52,
    DW_AT_use_UTF8 = 0x53,
    DW_AT_extension = 0x54,
    DW_AT_ranges = 0x55,
    DW_AT_trampoline = 0x56,
    DW_AT_call_column = 0x57,
    DW_AT_call_file = 0x58,
    DW_AT_call_line = 0x59,
    DW_AT_description = 0x5a,
    DW_AT_binary_scale = 0x5b,
    DW_AT_decimal_scale = 0x5c,
    DW_AT_small = 0x5d,
    DW_AT_decimal_sign = 0x5e,
    DW_AT_digit_count = 0x5f,
    DW_AT_picture_string = 0x60,
    DW_AT_mutable = 0x61,
    DW_AT_threads_scaled = 0x62,
    DW_AT_explicit = 0x63,
    DW_AT_object_pointer = 0x64,
    DW_AT_endianity = 0x65,
    DW_AT_elemental = 0x66,
    DW_AT_pure = 0x67,
    DW_AT_recursive = 0x68,

// DWARF 4.
    DW_AT_signature = 0x69,
    DW_AT_main_subprogram = 0x6a,
    DW_AT_data_bit_offset = 0x6b,
    DW_AT_const_expr = 0x6c,
    DW_AT_enum_class = 0x6d,
    DW_AT_linkage_name = 0x6e,

// DWARF 5.
    DW_AT_string_length_bit_size = 0x6f,
    DW_AT_string_length_byte_size = 0x70,
    DW_AT_rank = 0x71,
    DW_AT_str_offsets_base = 0x72,
    DW_AT_addr_base = 0x73,
    DW_AT_rnglists_base = 0x74,
    DW_AT_dwo_name = 0x76,
    DW_AT_reference = 0x77,
    DW_AT_rvalue_reference = 0x78,
    DW_AT_macros = 0x79,
    DW_AT_call_all_calls = 0x7a,
    DW_AT_call_all_source_calls = 0x7b,
    DW_AT_call_all_tail_calls = 0x7c,
    DW_AT_call_return_pc = 0x7d,
    DW_AT_call_value = 0x7e,
    DW_AT_call_origin = 0x7f,
    DW_AT_call_parameter = 0x80,
    DW_AT_call_pc = 0x81,
    DW_AT_call_tail_call = 0x82,
    DW_AT_call_target = 0x83,
    DW_AT_call_target_clobbered = 0x84,
    DW_AT_call_data_location = 0x85,
    DW_AT_call_data_value = 0x86,
    DW_AT_noreturn = 0x87,
    DW_AT_alignment = 0x88,
    DW_AT_export_symbols = 0x89,
    DW_AT_deleted = 0x8a,
    DW_AT_defaulted = 0x8b,
    DW_AT_loclists_base = 0x8c,

    DW_AT_lo_user = 0x2000,
    DW_AT_hi_user = 0x3fff,

// SGI/MIPS extensions.
    DW_AT_MIPS_fde = 0x2001,
    DW_AT_MIPS_loop_begin = 0x2002,
    DW_AT_MIPS_tail_loop_begin = 0x2003,
    DW_AT_MIPS_epilog_begin = 0x2004,
    DW_AT_MIPS_loop_unroll_factor = 0x2005,
    DW_AT_MIPS_software_pipeline_depth = 0x2006,
    DW_AT_MIPS_linkage_name = 0x2007,
    DW_AT_MIPS_stride = 0x2008,
    DW_AT_MIPS_abstract_name = 0x2009,
    DW_AT_MIPS_clone_origin = 0x200a,
    DW_AT_MIPS_has_inlines = 0x200b,
    DW_AT_MIPS_stride_byte = 0x200c,
    DW_AT_MIPS_stride_elem = 0x200d,
    DW_AT_MIPS_ptr_dopetype = 0x200e,
    DW_AT_MIPS_allocatable_dopetype = 0x200f,
    DW_AT_MIPS_assumed_shape_dopetype = 0x2010,

// This one appears to have only been implemented by Open64 for
// fortran and may conflict with other extensions.
    DW_AT_MIPS_assumed_size = 0x2011,

// TODO: HP/CPQ extensions.
// These conflict with the MIPS extensions.

    DW_AT_INTEL_other_endian = 0x2026,

// GNU extensions
    DW_AT_sf_names = 0x2101,
    DW_AT_src_info = 0x2102,
    DW_AT_mac_info = 0x2103,
    DW_AT_src_coords = 0x2104,
    DW_AT_body_begin = 0x2105,
    DW_AT_body_end = 0x2106,
    DW_AT_GNU_vector = 0x2107,
    DW_AT_GNU_guarded_by = 0x2108,
    DW_AT_GNU_pt_guarded_by = 0x2109,
    DW_AT_GNU_guarded = 0x210a,
    DW_AT_GNU_pt_guarded = 0x210b,
    DW_AT_GNU_locks_excluded = 0x210c,
    DW_AT_GNU_exclusive_locks_required = 0x210d,
    DW_AT_GNU_shared_locks_required = 0x210e,
    DW_AT_GNU_odr_signature = 0x210f,
    DW_AT_GNU_template_name = 0x2110,
    DW_AT_GNU_call_site_value = 0x2111,
    DW_AT_GNU_call_site_data_value = 0x2112,
    DW_AT_GNU_call_site_target = 0x2113,
    DW_AT_GNU_call_site_target_clobbered = 0x2114,
    DW_AT_GNU_tail_call = 0x2115,
    DW_AT_GNU_all_tail_call_sites = 0x2116,
    DW_AT_GNU_all_call_sites = 0x2117,
    DW_AT_GNU_all_source_call_sites = 0x2118,
    DW_AT_GNU_macros = 0x2119,
    DW_AT_GNU_deleted = 0x211a,

// Extensions for Fission proposal.
    DW_AT_GNU_dwo_name = 0x2130,
    DW_AT_GNU_dwo_id = 0x2131,
    DW_AT_GNU_ranges_base = 0x2132,
    DW_AT_GNU_addr_base = 0x2133,
    DW_AT_GNU_pubnames = 0x2134,
    DW_AT_GNU_pubtypes = 0x2135,
    DW_AT_GNU_discriminator = 0x2136,
    DW_AT_GNU_locviews = 0x2137,
    DW_AT_GNU_entry_view = 0x2138,

// Conflict with Sun.
// DW_AT_VMS_rtnbeg_pd_address = 0x2201,

// Sun extensions.
    DW_AT_SUN_template = 0x2201,
    DW_AT_SUN_alignment = 0x2202,
    DW_AT_SUN_vtable = 0x2203,
    DW_AT_SUN_count_guarantee = 0x2204,
    DW_AT_SUN_command_line = 0x2205,
    DW_AT_SUN_vbase = 0x2206,
    DW_AT_SUN_compile_options = 0x2207,
    DW_AT_SUN_language = 0x2208,
    DW_AT_SUN_browser_file = 0x2209,
    DW_AT_SUN_vtable_abi = 0x2210,
    DW_AT_SUN_func_offsets = 0x2211,
    DW_AT_SUN_cf_kind = 0x2212,
    DW_AT_SUN_vtable_index = 0x2213,
    DW_AT_SUN_omp_tpriv_addr = 0x2214,
    DW_AT_SUN_omp_child_func = 0x2215,
    DW_AT_SUN_func_offset = 0x2216,
    DW_AT_SUN_memop_type_ref = 0x2217,
    DW_AT_SUN_profile_id = 0x2218,
    DW_AT_SUN_memop_signature = 0x2219,
    DW_AT_SUN_obj_dir = 0x2220,
    DW_AT_SUN_obj_file = 0x2221,
    DW_AT_SUN_original_name = 0x2222,
    DW_AT_SUN_hwcprof_signature = 0x2223,
    DW_AT_SUN_amd64_parmdump = 0x2224,
    DW_AT_SUN_part_link_name = 0x2225,
    DW_AT_SUN_link_name = 0x2226,
    DW_AT_SUN_pass_with_const = 0x2227,
    DW_AT_SUN_return_with_const = 0x2228,
    DW_AT_SUN_import_by_name = 0x2229,
    DW_AT_SUN_f90_pointer = 0x222a,
    DW_AT_SUN_pass_by_ref = 0x222b,
    DW_AT_SUN_f90_allocatable = 0x222c,
    DW_AT_SUN_f90_assumed_shape_array = 0x222d,
    DW_AT_SUN_c_vla = 0x222e,
    DW_AT_SUN_return_value_ptr = 0x2230,
    DW_AT_SUN_dtor_start = 0x2231,
    DW_AT_SUN_dtor_length = 0x2232,
    DW_AT_SUN_dtor_state_initial = 0x2233,
    DW_AT_SUN_dtor_state_final = 0x2234,
    DW_AT_SUN_dtor_state_deltas = 0x2235,
    DW_AT_SUN_import_by_lname = 0x2236,
    DW_AT_SUN_f90_use_only = 0x2237,
    DW_AT_SUN_namelist_spec = 0x2238,
    DW_AT_SUN_is_omp_child_func = 0x2239,
    DW_AT_SUN_fortran_main_alias = 0x223a,
    DW_AT_SUN_fortran_based = 0x223b,

    DW_AT_ALTIUM_loclist = 0x2300,

    DW_AT_use_GNAT_descriptive_type = 0x2301,
    DW_AT_GNAT_descriptive_type = 0x2302,
    DW_AT_GNU_numerator = 0x2303,
    DW_AT_GNU_denominator = 0x2304,
    DW_AT_GNU_bias = 0x2305,

    DW_AT_upc_threads_scaled = 0x3210,

// PGI (STMicroelectronics) extensions.
    DW_AT_PGI_lbase = 0x3a00,
    DW_AT_PGI_soffset = 0x3a01,
    DW_AT_PGI_lstride = 0x3a02,

// Borland extensions.
    DW_AT_BORLAND_property_read = 0x3b11,
    DW_AT_BORLAND_property_write = 0x3b12,
    DW_AT_BORLAND_property_implements = 0x3b13,
    DW_AT_BORLAND_property_index = 0x3b14,
    DW_AT_BORLAND_property_default = 0x3b15,
    DW_AT_BORLAND_Delphi_unit = 0x3b20,
    DW_AT_BORLAND_Delphi_class = 0x3b21,
    DW_AT_BORLAND_Delphi_record = 0x3b22,
    DW_AT_BORLAND_Delphi_metaclass = 0x3b23,
    DW_AT_BORLAND_Delphi_constructor = 0x3b24,
    DW_AT_BORLAND_Delphi_destructor = 0x3b25,
    DW_AT_BORLAND_Delphi_anonymous_method = 0x3b26,
    DW_AT_BORLAND_Delphi_interface = 0x3b27,
    DW_AT_BORLAND_Delphi_ABI = 0x3b28,
    DW_AT_BORLAND_Delphi_return = 0x3b29,
    DW_AT_BORLAND_Delphi_frameptr = 0x3b30,
    DW_AT_BORLAND_closure = 0x3b31,

// LLVM project extensions.
    DW_AT_LLVM_include_path = 0x3e00,
    DW_AT_LLVM_config_macros = 0x3e01,
    DW_AT_LLVM_isysroot = 0x3e02,

// Apple extensions.
    DW_AT_APPLE_optimized = 0x3fe1,
    DW_AT_APPLE_flags = 0x3fe2,
    DW_AT_APPLE_isa = 0x3fe3,
    DW_AT_APPLE_block = 0x3fe4,
    DW_AT_APPLE_major_runtime_vers = 0x3fe5,
    DW_AT_APPLE_runtime_class = 0x3fe6,
    DW_AT_APPLE_omit_frame_ptr = 0x3fe7,
    DW_AT_APPLE_property_name = 0x3fe8,
    DW_AT_APPLE_property_getter = 0x3fe9,
    DW_AT_APPLE_property_setter = 0x3fea,
    DW_AT_APPLE_property_attribute = 0x3feb,
    DW_AT_APPLE_objc_complete_type = 0x3fec,
    DW_AT_APPLE_property = 0x3fed
});

dw!(
/// The attribute form encodings for DIE attributes.
///
/// See Section 7.5.6, Table 7.6.
DwForm(u16) {
    DW_FORM_null = 0x00,

    DW_FORM_addr = 0x01,
    DW_FORM_block2 = 0x03,
    DW_FORM_block4 = 0x04,
    DW_FORM_data2 = 0x05,
    DW_FORM_data4 = 0x06,
    DW_FORM_data8 = 0x07,
    DW_FORM_string = 0x08,
    DW_FORM_block = 0x09,
    DW_FORM_block1 = 0x0a,
    DW_FORM_data1 = 0x0b,
    DW_FORM_flag = 0x0c,
    DW_FORM_sdata = 0x0d,
    DW_FORM_strp = 0x0e,
    DW_FORM_udata = 0x0f,
    DW_FORM_ref_addr = 0x10,
    DW_FORM_ref1 = 0x11,
    DW_FORM_ref2 = 0x12,
    DW_FORM_ref4 = 0x13,
    DW_FORM_ref8 = 0x14,
    DW_FORM_ref_udata = 0x15,
    DW_FORM_indirect = 0x16,

// DWARF 4.
    DW_FORM_sec_offset = 0x17,
    DW_FORM_exprloc = 0x18,
    DW_FORM_flag_present = 0x19,
    DW_FORM_ref_sig8 = 0x20,

// DWARF 5.
    DW_FORM_strx = 0x1a,
    DW_FORM_addrx = 0x1b,
    DW_FORM_ref_sup4 = 0x1c,
    DW_FORM_strp_sup = 0x1d,
    DW_FORM_data16 = 0x1e,
    DW_FORM_line_strp = 0x1f,
    DW_FORM_implicit_const = 0x21,
    DW_FORM_loclistx = 0x22,
    DW_FORM_rnglistx = 0x23,
    DW_FORM_ref_sup8 = 0x24,
    DW_FORM_strx1 = 0x25,
    DW_FORM_strx2 = 0x26,
    DW_FORM_strx3 = 0x27,
    DW_FORM_strx4 = 0x28,
    DW_FORM_addrx1 = 0x29,
    DW_FORM_addrx2 = 0x2a,
    DW_FORM_addrx3 = 0x2b,
    DW_FORM_addrx4 = 0x2c,

// Extensions for Fission proposal
    DW_FORM_GNU_addr_index = 0x1f01,
    DW_FORM_GNU_str_index = 0x1f02,

// Alternate debug sections proposal (output of "dwz" tool).
    DW_FORM_GNU_ref_alt = 0x1f20,
    DW_FORM_GNU_strp_alt = 0x1f21
});

dw!(
/// The encodings of the constants used in the `DW_AT_encoding` attribute.
///
/// See Section 7.8, Table 7.11.
DwAte(u8) {
    DW_ATE_address = 0x01,
    DW_ATE_boolean = 0x02,
    DW_ATE_complex_float = 0x03,
    DW_ATE_float = 0x04,
    DW_ATE_signed = 0x05,
    DW_ATE_signed_char = 0x06,
    DW_ATE_unsigned = 0x07,
    DW_ATE_unsigned_char = 0x08,

// DWARF 3.
    DW_ATE_imaginary_float = 0x09,
    DW_ATE_packed_decimal = 0x0a,
    DW_ATE_numeric_string = 0x0b,
    DW_ATE_edited = 0x0c,
    DW_ATE_signed_fixed = 0x0d,
    DW_ATE_unsigned_fixed = 0x0e,
    DW_ATE_decimal_float = 0x0f ,

// DWARF 4.
    DW_ATE_UTF = 0x10,
    DW_ATE_UCS = 0x11,
    DW_ATE_ASCII = 0x12,

    DW_ATE_lo_user = 0x80,
    DW_ATE_hi_user = 0xff,
});

dw!(
/// The encodings of the constants used in location list entries.
///
/// See Section 7.7.3, Table 7.10.
DwLle(u8) {
    DW_LLE_end_of_list = 0x00,
    DW_LLE_base_addressx = 0x01,
    DW_LLE_startx_endx = 0x02,
    DW_LLE_startx_length = 0x03,
    DW_LLE_offset_pair = 0x04,
    DW_LLE_default_location = 0x05,
    DW_LLE_base_address = 0x06,
    DW_LLE_start_end = 0x07,
    DW_LLE_start_length = 0x08,
    DW_LLE_GNU_view_pair = 0x09,
});

dw!(
/// The encodings of the constants used in the `DW_AT_decimal_sign` attribute.
///
/// See Section 7.8, Table 7.12.
DwDs(u8) {
    DW_DS_unsigned = 0x01,
    DW_DS_leading_overpunch = 0x02,
    DW_DS_trailing_overpunch = 0x03,
    DW_DS_leading_separate = 0x04,
    DW_DS_trailing_separate = 0x05,
});

dw!(
/// The encodings of the constants used in the `DW_AT_endianity` attribute.
///
/// See Section 7.8, Table 7.13.
DwEnd(u8) {
    DW_END_default = 0x00,
    DW_END_big = 0x01,
    DW_END_little = 0x02,
    DW_END_lo_user = 0x40,
    DW_END_hi_user = 0xff,
});

dw!(
/// The encodings of the constants used in the `DW_AT_accessibility` attribute.
///
/// See Section 7.9, Table 7.14.
DwAccess(u8) {
    DW_ACCESS_public = 0x01,
    DW_ACCESS_protected = 0x02,
    DW_ACCESS_private = 0x03,
});

dw!(
/// The encodings of the constants used in the `DW_AT_visibility` attribute.
///
/// See Section 7.10, Table 7.15.
DwVis(u8) {
    DW_VIS_local = 0x01,
    DW_VIS_exported = 0x02,
    DW_VIS_qualified = 0x03,
});

dw!(
/// The encodings of the constants used in the `DW_AT_virtuality` attribute.
///
/// See Section 7.11, Table 7.16.
DwVirtuality(u8) {
    DW_VIRTUALITY_none = 0x00,
    DW_VIRTUALITY_virtual = 0x01,
    DW_VIRTUALITY_pure_virtual = 0x02,
});

dw!(
/// The encodings of the constants used in the `DW_AT_language` attribute.
///
/// See Section 7.12, Table 7.17.
DwLang(u16) {
    DW_LANG_C89 = 0x0001,
    DW_LANG_C = 0x0002,
    DW_LANG_Ada83 = 0x0003,
    DW_LANG_C_plus_plus = 0x0004,
    DW_LANG_Cobol74 = 0x0005,
    DW_LANG_Cobol85 = 0x0006,
    DW_LANG_Fortran77 = 0x0007,
    DW_LANG_Fortran90 = 0x0008,
    DW_LANG_Pascal83 = 0x0009,
    DW_LANG_Modula2 = 0x000a,
    DW_LANG_Java = 0x000b,
    DW_LANG_C99 = 0x000c,
    DW_LANG_Ada95 = 0x000d,
    DW_LANG_Fortran95 = 0x000e,
    DW_LANG_PLI = 0x000f,
    DW_LANG_ObjC = 0x0010,
    DW_LANG_ObjC_plus_plus = 0x0011,
    DW_LANG_UPC = 0x0012,
    DW_LANG_D = 0x0013,
    DW_LANG_Python = 0x0014,
    DW_LANG_OpenCL = 0x0015,
    DW_LANG_Go = 0x0016,
    DW_LANG_Modula3 = 0x0017,
    DW_LANG_Haskell = 0x0018,
    DW_LANG_C_plus_plus_03 = 0x0019,
    DW_LANG_C_plus_plus_11 = 0x001a,
    DW_LANG_OCaml = 0x001b,
    DW_LANG_Rust = 0x001c,
    DW_LANG_C11 = 0x001d,
    DW_LANG_Swift = 0x001e,
    DW_LANG_Julia = 0x001f,
    DW_LANG_Dylan = 0x0020,
    DW_LANG_C_plus_plus_14 = 0x0021,
    DW_LANG_Fortran03 = 0x0022,
    DW_LANG_Fortran08 = 0x0023,
    DW_LANG_RenderScript = 0x0024,
    DW_LANG_BLISS = 0x0025,
    DW_LANG_Kotlin = 0x0026,
    DW_LANG_Zig = 0x0027,
    DW_LANG_Crystal = 0x0028,
    DW_LANG_C_plus_plus_17 = 0x002a,
    DW_LANG_C_plus_plus_20 = 0x002b,
    DW_LANG_C17 = 0x002c,
    DW_LANG_Fortran18 = 0x002d,
    DW_LANG_Ada2005 = 0x002e,
    DW_LANG_Ada2012 = 0x002f,

    DW_LANG_lo_user = 0x8000,
    DW_LANG_hi_user = 0xffff,

    DW_LANG_Mips_Assembler = 0x8001,
    DW_LANG_GOOGLE_RenderScript = 0x8e57,
    DW_LANG_SUN_Assembler = 0x9001,
    DW_LANG_ALTIUM_Assembler = 0x9101,
    DW_LANG_BORLAND_Delphi = 0xb000,
});

impl DwLang {
    /// Get the default DW_AT_lower_bound for this language.
    pub fn default_lower_bound(self) -> Option<usize> {
        match self {
            DW_LANG_C89
            | DW_LANG_C
            | DW_LANG_C_plus_plus
            | DW_LANG_Java
            | DW_LANG_C99
            | DW_LANG_ObjC
            | DW_LANG_ObjC_plus_plus
            | DW_LANG_UPC
            | DW_LANG_D
            | DW_LANG_Python
            | DW_LANG_OpenCL
            | DW_LANG_Go
            | DW_LANG_Haskell
            | DW_LANG_C_plus_plus_03
            | DW_LANG_C_plus_plus_11
            | DW_LANG_OCaml
            | DW_LANG_Rust
            | DW_LANG_C11
            | DW_LANG_Swift
            | DW_LANG_Dylan
            | DW_LANG_C_plus_plus_14
            | DW_LANG_RenderScript
            | DW_LANG_BLISS => Some(0),
            DW_LANG_Ada83 | DW_LANG_Cobol74 | DW_LANG_Cobol85 | DW_LANG_Fortran77
            | DW_LANG_Fortran90 | DW_LANG_Pascal83 | DW_LANG_Modula2 | DW_LANG_Ada95
            | DW_LANG_Fortran95 | DW_LANG_PLI | DW_LANG_Modula3 | DW_LANG_Julia
            | DW_LANG_Fortran03 | DW_LANG_Fortran08 => Some(1),
            _ => None,
        }
    }
}

dw!(
/// The encodings of the constants used in the `DW_AT_address_class` attribute.
///
/// There is only one value that is common to all target architectures.
/// See Section 7.13.
DwAddr(u64) {
    DW_ADDR_none = 0x00,
});

dw!(
/// The encodings of the constants used in the `DW_AT_identifier_case` attribute.
///
/// See Section 7.14, Table 7.18.
DwId(u8) {
    DW_ID_case_sensitive = 0x00,
    DW_ID_up_case = 0x01,
    DW_ID_down_case = 0x02,
    DW_ID_case_insensitive = 0x03,
});

dw!(
/// The encodings of the constants used in the `DW_AT_calling_convention` attribute.
///
/// See Section 7.15, Table 7.19.
DwCc(u8) {
    DW_CC_normal = 0x01,
    DW_CC_program = 0x02,
    DW_CC_nocall = 0x03,
    DW_CC_pass_by_reference = 0x04,
    DW_CC_pass_by_value = 0x05,
    DW_CC_lo_user = 0x40,
    DW_CC_hi_user = 0xff,
});

dw!(
/// The encodings of the constants used in the `DW_AT_inline` attribute.
///
/// See Section 7.16, Table 7.20.
DwInl(u8) {
    DW_INL_not_inlined = 0x00,
    DW_INL_inlined = 0x01,
    DW_INL_declared_not_inlined = 0x02,
    DW_INL_declared_inlined = 0x03,
});

dw!(
/// The encodings of the constants used in the `DW_AT_ordering` attribute.
///
/// See Section 7.17, Table 7.17.
DwOrd(u8) {
    DW_ORD_row_major = 0x00,
    DW_ORD_col_major = 0x01,
});

dw!(
/// The encodings of the constants used in the `DW_AT_discr_list` attribute.
///
/// See Section 7.18, Table 7.22.
DwDsc(u8) {
    DW_DSC_label = 0x00,
    DW_DSC_range = 0x01,
});

dw!(
/// Name index attribute encodings.
///
/// See Section 7.19, Table 7.23.
DwIdx(u16) {
    DW_IDX_compile_unit = 1,
    DW_IDX_type_unit = 2,
    DW_IDX_die_offset = 3,
    DW_IDX_parent = 4,
    DW_IDX_type_hash = 5,
    DW_IDX_lo_user = 0x2000,
    DW_IDX_hi_user = 0x3fff,
});

dw!(
/// The encodings of the constants used in the `DW_AT_defaulted` attribute.
///
/// See Section 7.20, Table 7.24.
DwDefaulted(u8) {
    DW_DEFAULTED_no = 0x00,
    DW_DEFAULTED_in_class = 0x01,
    DW_DEFAULTED_out_of_class = 0x02,
});

dw!(
/// The encodings for the standard opcodes for line number information.
///
/// See Section 7.22, Table 7.25.
DwLns(u8) {
    DW_LNS_copy = 0x01,
    DW_LNS_advance_pc = 0x02,
    DW_LNS_advance_line = 0x03,
    DW_LNS_set_file = 0x04,
    DW_LNS_set_column = 0x05,
    DW_LNS_negate_stmt = 0x06,
    DW_LNS_set_basic_block = 0x07,
    DW_LNS_const_add_pc = 0x08,
    DW_LNS_fixed_advance_pc = 0x09,
    DW_LNS_set_prologue_end = 0x0a,
    DW_LNS_set_epilogue_begin = 0x0b,
    DW_LNS_set_isa = 0x0c,
});

dw!(
/// The encodings for the extended opcodes for line number information.
///
/// See Section 7.22, Table 7.26.
DwLne(u8) {
    DW_LNE_end_sequence = 0x01,
    DW_LNE_set_address = 0x02,
    DW_LNE_define_file = 0x03,
    DW_LNE_set_discriminator = 0x04,

    DW_LNE_lo_user = 0x80,
    DW_LNE_hi_user = 0xff,
});

dw!(
/// The encodings for the line number header entry formats.
///
/// See Section 7.22, Table 7.27.
DwLnct(u16) {
    DW_LNCT_path = 0x1,
    DW_LNCT_directory_index = 0x2,
    DW_LNCT_timestamp = 0x3,
    DW_LNCT_size = 0x4,
    DW_LNCT_MD5 = 0x5,
    // DW_LNCT_source = 0x6,
    DW_LNCT_lo_user = 0x2000,
    // We currently only implement the LLVM embedded source code extension for DWARF v5.
    DW_LNCT_LLVM_source = 0x2001,
    DW_LNCT_hi_user = 0x3fff,
});

dw!(
/// The encodings for macro information entry types.
///
/// See Section 7.23, Table 7.28.
DwMacro(u8) {
    DW_MACRO_define = 0x01,
    DW_MACRO_undef = 0x02,
    DW_MACRO_start_file = 0x03,
    DW_MACRO_end_file = 0x04,
    DW_MACRO_define_strp = 0x05,
    DW_MACRO_undef_strp = 0x06,
    DW_MACRO_import = 0x07,
    DW_MACRO_define_sup = 0x08,
    DW_MACRO_undef_sup = 0x09,
    DW_MACRO_import_sup = 0x0a,
    DW_MACRO_define_strx = 0x0b,
    DW_MACRO_undef_strx = 0x0c,
    DW_MACRO_lo_user = 0xe0,
    DW_MACRO_hi_user = 0xff,
});

dw!(
/// Range list entry encoding values.
///
/// See Section 7.25, Table 7.30.
DwRle(u8) {
    DW_RLE_end_of_list = 0x00,
    DW_RLE_base_addressx = 0x01,
    DW_RLE_startx_endx = 0x02,
    DW_RLE_startx_length = 0x03,
    DW_RLE_offset_pair = 0x04,
    DW_RLE_base_address = 0x05,
    DW_RLE_start_end = 0x06,
    DW_RLE_start_length = 0x07,
});

dw!(
/// The encodings for DWARF expression operations.
///
/// See Section 7.7.1, Table 7.9.
DwOp(u8) {
    DW_OP_addr = 0x03,
    DW_OP_deref = 0x06,
    DW_OP_const1u = 0x08,
    DW_OP_const1s = 0x09,
    DW_OP_const2u = 0x0a,
    DW_OP_const2s = 0x0b,
    DW_OP_const4u = 0x0c,
    DW_OP_const4s = 0x0d,
    DW_OP_const8u = 0x0e,
    DW_OP_const8s = 0x0f,
    DW_OP_constu = 0x10,
    DW_OP_consts = 0x11,
    DW_OP_dup = 0x12,
    DW_OP_drop = 0x13,
    DW_OP_over = 0x14,
    DW_OP_pick = 0x15,
    DW_OP_swap = 0x16,
    DW_OP_rot = 0x17,
    DW_OP_xderef = 0x18,
    DW_OP_abs = 0x19,
    DW_OP_and = 0x1a,
    DW_OP_div = 0x1b,
    DW_OP_minus = 0x1c,
    DW_OP_mod = 0x1d,
    DW_OP_mul = 0x1e,
    DW_OP_neg = 0x1f,
    DW_OP_not = 0x20,
    DW_OP_or = 0x21,
    DW_OP_plus = 0x22,
    DW_OP_plus_uconst = 0x23,
    DW_OP_shl = 0x24,
    DW_OP_shr = 0x25,
    DW_OP_shra = 0x26,
    DW_OP_xor = 0x27,
    DW_OP_bra = 0x28,
    DW_OP_eq = 0x29,
    DW_OP_ge = 0x2a,
    DW_OP_gt = 0x2b,
    DW_OP_le = 0x2c,
    DW_OP_lt = 0x2d,
    DW_OP_ne = 0x2e,
    DW_OP_skip = 0x2f,
    DW_OP_lit0 = 0x30,
    DW_OP_lit1 = 0x31,
    DW_OP_lit2 = 0x32,
    DW_OP_lit3 = 0x33,
    DW_OP_lit4 = 0x34,
    DW_OP_lit5 = 0x35,
    DW_OP_lit6 = 0x36,
    DW_OP_lit7 = 0x37,
    DW_OP_lit8 = 0x38,
    DW_OP_lit9 = 0x39,
    DW_OP_lit10 = 0x3a,
    DW_OP_lit11 = 0x3b,
    DW_OP_lit12 = 0x3c,
    DW_OP_lit13 = 0x3d,
    DW_OP_lit14 = 0x3e,
    DW_OP_lit15 = 0x3f,
    DW_OP_lit16 = 0x40,
    DW_OP_lit17 = 0x41,
    DW_OP_lit18 = 0x42,
    DW_OP_lit19 = 0x43,
    DW_OP_lit20 = 0x44,
    DW_OP_lit21 = 0x45,
    DW_OP_lit22 = 0x46,
    DW_OP_lit23 = 0x47,
    DW_OP_lit24 = 0x48,
    DW_OP_lit25 = 0x49,
    DW_OP_lit26 = 0x4a,
    DW_OP_lit27 = 0x4b,
    DW_OP_lit28 = 0x4c,
    DW_OP_lit29 = 0x4d,
    DW_OP_lit30 = 0x4e,
    DW_OP_lit31 = 0x4f,
    DW_OP_reg0 = 0x50,
    DW_OP_reg1 = 0x51,
    DW_OP_reg2 = 0x52,
    DW_OP_reg3 = 0x53,
    DW_OP_reg4 = 0x54,
    DW_OP_reg5 = 0x55,
    DW_OP_reg6 = 0x56,
    DW_OP_reg7 = 0x57,
    DW_OP_reg8 = 0x58,
    DW_OP_reg9 = 0x59,
    DW_OP_reg10 = 0x5a,
    DW_OP_reg11 = 0x5b,
    DW_OP_reg12 = 0x5c,
    DW_OP_reg13 = 0x5d,
    DW_OP_reg14 = 0x5e,
    DW_OP_reg15 = 0x5f,
    DW_OP_reg16 = 0x60,
    DW_OP_reg17 = 0x61,
    DW_OP_reg18 = 0x62,
    DW_OP_reg19 = 0x63,
    DW_OP_reg20 = 0x64,
    DW_OP_reg21 = 0x65,
    DW_OP_reg22 = 0x66,
    DW_OP_reg23 = 0x67,
    DW_OP_reg24 = 0x68,
    DW_OP_reg25 = 0x69,
    DW_OP_reg26 = 0x6a,
    DW_OP_reg27 = 0x6b,
    DW_OP_reg28 = 0x6c,
    DW_OP_reg29 = 0x6d,
    DW_OP_reg30 = 0x6e,
    DW_OP_reg31 = 0x6f,
    DW_OP_breg0 = 0x70,
    DW_OP_breg1 = 0x71,
    DW_OP_breg2 = 0x72,
    DW_OP_breg3 = 0x73,
    DW_OP_breg4 = 0x74,
    DW_OP_breg5 = 0x75,
    DW_OP_breg6 = 0x76,
    DW_OP_breg7 = 0x77,
    DW_OP_breg8 = 0x78,
    DW_OP_breg9 = 0x79,
    DW_OP_breg10 = 0x7a,
    DW_OP_breg11 = 0x7b,
    DW_OP_breg12 = 0x7c,
    DW_OP_breg13 = 0x7d,
    DW_OP_breg14 = 0x7e,
    DW_OP_breg15 = 0x7f,
    DW_OP_breg16 = 0x80,
    DW_OP_breg17 = 0x81,
    DW_OP_breg18 = 0x82,
    DW_OP_breg19 = 0x83,
    DW_OP_breg20 = 0x84,
    DW_OP_breg21 = 0x85,
    DW_OP_breg22 = 0x86,
    DW_OP_breg23 = 0x87,
    DW_OP_breg24 = 0x88,
    DW_OP_breg25 = 0x89,
    DW_OP_breg26 = 0x8a,
    DW_OP_breg27 = 0x8b,
    DW_OP_breg28 = 0x8c,
    DW_OP_breg29 = 0x8d,
    DW_OP_breg30 = 0x8e,
    DW_OP_breg31 = 0x8f,
    DW_OP_regx = 0x90,
    DW_OP_fbreg = 0x91,
    DW_OP_bregx = 0x92,
    DW_OP_piece = 0x93,
    DW_OP_deref_size = 0x94,
    DW_OP_xderef_size = 0x95,
    DW_OP_nop = 0x96,
    DW_OP_push_object_address = 0x97,
    DW_OP_call2 = 0x98,
    DW_OP_call4 = 0x99,
    DW_OP_call_ref = 0x9a,
    DW_OP_form_tls_address = 0x9b,
    DW_OP_call_frame_cfa = 0x9c,
    DW_OP_bit_piece = 0x9d,
    DW_OP_implicit_value = 0x9e,
    DW_OP_stack_value = 0x9f,
    DW_OP_implicit_pointer = 0xa0,
    DW_OP_addrx = 0xa1,
    DW_OP_constx = 0xa2,
    DW_OP_entry_value = 0xa3,
    DW_OP_const_type = 0xa4,
    DW_OP_regval_type = 0xa5,
    DW_OP_deref_type = 0xa6,
    DW_OP_xderef_type = 0xa7,
    DW_OP_convert = 0xa8,
    DW_OP_reinterpret = 0xa9,

    // GNU extensions
    DW_OP_GNU_push_tls_address = 0xe0,
    DW_OP_GNU_implicit_pointer = 0xf2,
    DW_OP_GNU_entry_value = 0xf3,
    DW_OP_GNU_const_type = 0xf4,
    DW_OP_GNU_regval_type = 0xf5,
    DW_OP_GNU_deref_type = 0xf6,
    DW_OP_GNU_convert = 0xf7,
    DW_OP_GNU_reinterpret = 0xf9,
    DW_OP_GNU_parameter_ref = 0xfa,
    DW_OP_GNU_addr_index = 0xfb,
    DW_OP_GNU_const_index = 0xfc,

    // Wasm extensions
    DW_OP_WASM_location = 0xed,
});

dw!(
/// Pointer encoding used by `.eh_frame`.
///
/// The four lower bits describe the
/// format of the pointer, the upper four bits describe how the encoding should
/// be applied.
///
/// Defined in `<https://refspecs.linuxfoundation.org/LSB_4.0.0/LSB-Core-generic/LSB-Core-generic/dwarfext.html>`
DwEhPe(u8) {
// Format of pointer encoding.

// "Unsigned value is encoded using the Little Endian Base 128"
    DW_EH_PE_uleb128 = 0x1,
// "A 2 bytes unsigned value."
    DW_EH_PE_udata2 = 0x2,
// "A 4 bytes unsigned value."
    DW_EH_PE_udata4 = 0x3,
// "An 8 bytes unsigned value."
    DW_EH_PE_udata8 = 0x4,
// "Signed value is encoded using the Little Endian Base 128"
    DW_EH_PE_sleb128 = 0x9,
// "A 2 bytes signed value."
    DW_EH_PE_sdata2 = 0x0a,
// "A 4 bytes signed value."
    DW_EH_PE_sdata4 = 0x0b,
// "An 8 bytes signed value."
    DW_EH_PE_sdata8 = 0x0c,

// How the pointer encoding should be applied.

// `DW_EH_PE_pcrel` pointers are relative to their own location.
    DW_EH_PE_pcrel = 0x10,
// "Value is relative to the beginning of the .text section."
    DW_EH_PE_textrel = 0x20,
// "Value is relative to the beginning of the .got or .eh_frame_hdr section."
    DW_EH_PE_datarel = 0x30,
// "Value is relative to the beginning of the function."
    DW_EH_PE_funcrel = 0x40,
// "Value is aligned to an address unit sized boundary."
    DW_EH_PE_aligned = 0x50,

// This bit can be set for any of the above encoding applications. When set,
// the encoded value is the address of the real pointer result, not the
// pointer result itself.
//
// This isn't defined in the DWARF or the `.eh_frame` standards, but is
// generated by both GNU/Linux and macOS tooling.
    DW_EH_PE_indirect = 0x80,

// These constants apply to both the lower and upper bits.

// "The Value is a literal pointer whose size is determined by the
// architecture."
    DW_EH_PE_absptr = 0x0,
// The absence of a pointer and encoding.
    DW_EH_PE_omit = 0xff,
});

const DW_EH_PE_FORMAT_MASK: u8 = 0b0000_1111;

// Ignores indirection bit.
const DW_EH_PE_APPLICATION_MASK: u8 = 0b0111_0000;

impl ops::BitOr for DwEhPe {
    type Output = DwEhPe;

    fn bitor(self, rhs: DwEhPe) -> DwEhPe {
        DwEhPe(self.0 | rhs.0)
    }
}

impl DwEhPe {
    /// Get the pointer encoding's format.
    #[inline]
    pub fn format(self) -> DwEhPe {
        DwEhPe(self.0 & DW_EH_PE_FORMAT_MASK)
    }

    /// Get the pointer encoding's application.
    #[inline]
    pub fn application(self) -> DwEhPe {
        DwEhPe(self.0 & DW_EH_PE_APPLICATION_MASK)
    }

    /// Is this encoding the absent pointer encoding?
    #[inline]
    pub fn is_absent(self) -> bool {
        self == DW_EH_PE_omit
    }

    /// Is this coding indirect? If so, its encoded value is the address of the
    /// real pointer result, not the pointer result itself.
    #[inline]
    pub fn is_indirect(self) -> bool {
        self.0 & DW_EH_PE_indirect.0 != 0
    }

    /// Is this a known, valid pointer encoding?
    pub fn is_valid_encoding(self) -> bool {
        if self.is_absent() {
            return true;
        }

        match self.format() {
            DW_EH_PE_absptr | DW_EH_PE_uleb128 | DW_EH_PE_udata2 | DW_EH_PE_udata4
            | DW_EH_PE_udata8 | DW_EH_PE_sleb128 | DW_EH_PE_sdata2 | DW_EH_PE_sdata4
            | DW_EH_PE_sdata8 => {}
            _ => return false,
        }

        match self.application() {
            DW_EH_PE_absptr | DW_EH_PE_pcrel | DW_EH_PE_textrel | DW_EH_PE_datarel
            | DW_EH_PE_funcrel | DW_EH_PE_aligned => {}
            _ => return false,
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dw_eh_pe_format() {
        let encoding = DW_EH_PE_pcrel | DW_EH_PE_uleb128;
        assert_eq!(encoding.format(), DW_EH_PE_uleb128);
    }

    #[test]
    fn test_dw_eh_pe_application() {
        let encoding = DW_EH_PE_pcrel | DW_EH_PE_uleb128;
        assert_eq!(encoding.application(), DW_EH_PE_pcrel);
    }

    #[test]
    fn test_dw_eh_pe_is_absent() {
        assert!(!DW_EH_PE_absptr.is_absent());
        assert!(DW_EH_PE_omit.is_absent());
    }

    #[test]
    fn test_dw_eh_pe_is_valid_encoding_ok() {
        let encoding = DW_EH_PE_uleb128 | DW_EH_PE_pcrel;
        assert!(encoding.is_valid_encoding());
        assert!(DW_EH_PE_absptr.is_valid_encoding());
        assert!(DW_EH_PE_omit.is_valid_encoding());
    }

    #[test]
    fn test_dw_eh_pe_is_valid_encoding_bad_format() {
        let encoding = DwEhPe((DW_EH_PE_sdata8.0 + 1) | DW_EH_PE_pcrel.0);
        assert!(!encoding.is_valid_encoding());
    }

    #[test]
    fn test_dw_eh_pe_is_valid_encoding_bad_application() {
        let encoding = DwEhPe(DW_EH_PE_sdata8.0 | (DW_EH_PE_aligned.0 + 1));
        assert!(!encoding.is_valid_encoding());
    }
}
