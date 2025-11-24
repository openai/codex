//! Helper for writing PE files.
use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

use crate::endian::{LittleEndian as LE, *};
use crate::pe;
use crate::write::util;
use crate::write::{Error, Result, WritableBuffer};

/// A helper for writing PE files.
///
/// Writing uses a two phase approach. The first phase reserves file ranges and virtual
/// address ranges for everything in the order that they will be written.
///
/// The second phase writes everything out in order. Thus the caller must ensure writing
/// is in the same order that file ranges were reserved.
#[allow(missing_debug_implementations)]
pub struct Writer<'a> {
    is_64: bool,
    section_alignment: u32,
    file_alignment: u32,

    buffer: &'a mut dyn WritableBuffer,
    len: u32,
    virtual_len: u32,
    headers_len: u32,

    code_address: u32,
    data_address: u32,
    code_len: u32,
    data_len: u32,
    bss_len: u32,

    nt_headers_offset: u32,
    data_directories: Vec<DataDirectory>,
    section_header_num: u16,
    sections: Vec<Section>,

    symbol_offset: u32,
    symbol_num: u32,

    reloc_blocks: Vec<RelocBlock>,
    relocs: Vec<U16<LE>>,
    reloc_offset: u32,
}

impl<'a> Writer<'a> {
    /// Create a new `Writer`.
    ///
    /// The alignment values must be powers of two.
    pub fn new(
        is_64: bool,
        section_alignment: u32,
        file_alignment: u32,
        buffer: &'a mut dyn WritableBuffer,
    ) -> Self {
        Writer {
            is_64,
            section_alignment,
            file_alignment,

            buffer,
            len: 0,
            virtual_len: 0,
            headers_len: 0,

            code_address: 0,
            data_address: 0,
            code_len: 0,
            data_len: 0,
            bss_len: 0,

            nt_headers_offset: 0,
            data_directories: Vec::new(),
            section_header_num: 0,
            sections: Vec::new(),

            symbol_offset: 0,
            symbol_num: 0,

            reloc_blocks: Vec::new(),
            relocs: Vec::new(),
            reloc_offset: 0,
        }
    }

    /// Return the current virtual address size that has been reserved.
    ///
    /// This is only valid after section headers have been reserved.
    pub fn virtual_len(&self) -> u32 {
        self.virtual_len
    }

    /// Reserve a virtual address range with the given size.
    ///
    /// The reserved length will be increased to match the section alignment.
    ///
    /// Returns the aligned offset of the start of the range.
    pub fn reserve_virtual(&mut self, len: u32) -> u32 {
        let offset = self.virtual_len;
        self.virtual_len += len;
        self.virtual_len = util::align_u32(self.virtual_len, self.section_alignment);
        offset
    }

    /// Reserve up to the given virtual address.
    ///
    /// The reserved length will be increased to match the section alignment.
    pub fn reserve_virtual_until(&mut self, address: u32) {
        debug_assert!(self.virtual_len <= address);
        self.virtual_len = util::align_u32(address, self.section_alignment);
    }

    /// Return the current file length that has been reserved.
    pub fn reserved_len(&self) -> u32 {
        self.len
    }

    /// Return the current file length that has been written.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Reserve a file range with the given size and starting alignment.
    ///
    /// Returns the aligned offset of the start of the range.
    pub fn reserve(&mut self, len: u32, align_start: u32) -> u32 {
        if len == 0 {
            return self.len;
        }
        self.reserve_align(align_start);
        let offset = self.len;
        self.len += len;
        offset
    }

    /// Reserve a file range with the given size and using the file alignment.
    ///
    /// Returns the aligned offset of the start of the range.
    pub fn reserve_file(&mut self, len: u32) -> u32 {
        self.reserve(len, self.file_alignment)
    }

    /// Write data.
    pub fn write(&mut self, data: &[u8]) {
        self.buffer.write_bytes(data);
    }

    /// Reserve alignment padding bytes.
    pub fn reserve_align(&mut self, align_start: u32) {
        self.len = util::align_u32(self.len, align_start);
    }

    /// Write alignment padding bytes.
    pub fn write_align(&mut self, align_start: u32) {
        util::write_align(self.buffer, align_start as usize);
    }

    /// Write padding up to the next multiple of file alignment.
    pub fn write_file_align(&mut self) {
        self.write_align(self.file_alignment);
    }

    /// Reserve the file range up to the given file offset.
    pub fn reserve_until(&mut self, offset: u32) {
        debug_assert!(self.len <= offset);
        self.len = offset;
    }

    /// Write padding up to the given file offset.
    pub fn pad_until(&mut self, offset: u32) {
        debug_assert!(self.buffer.len() <= offset as usize);
        self.buffer.resize(offset as usize);
    }

    /// Reserve the range for the DOS header.
    ///
    /// This must be at the start of the file.
    ///
    /// When writing, you may use `write_custom_dos_header` or `write_empty_dos_header`.
    pub fn reserve_dos_header(&mut self) {
        debug_assert_eq!(self.len, 0);
        self.reserve(mem::size_of::<pe::ImageDosHeader>() as u32, 1);
    }

    /// Write a custom DOS header.
    ///
    /// This must be at the start of the file.
    pub fn write_custom_dos_header(&mut self, dos_header: &pe::ImageDosHeader) -> Result<()> {
        debug_assert_eq!(self.buffer.len(), 0);

        // Start writing.
        self.buffer
            .reserve(self.len as usize)
            .map_err(|_| Error(String::from("Cannot allocate buffer")))?;

        self.buffer.write(dos_header);
        Ok(())
    }

    /// Write the DOS header for a file without a stub.
    ///
    /// This must be at the start of the file.
    ///
    /// Uses default values for all fields.
    pub fn write_empty_dos_header(&mut self) -> Result<()> {
        self.write_custom_dos_header(&pe::ImageDosHeader {
            e_magic: U16::new(LE, pe::IMAGE_DOS_SIGNATURE),
            e_cblp: U16::new(LE, 0),
            e_cp: U16::new(LE, 0),
            e_crlc: U16::new(LE, 0),
            e_cparhdr: U16::new(LE, 0),
            e_minalloc: U16::new(LE, 0),
            e_maxalloc: U16::new(LE, 0),
            e_ss: U16::new(LE, 0),
            e_sp: U16::new(LE, 0),
            e_csum: U16::new(LE, 0),
            e_ip: U16::new(LE, 0),
            e_cs: U16::new(LE, 0),
            e_lfarlc: U16::new(LE, 0),
            e_ovno: U16::new(LE, 0),
            e_res: [U16::new(LE, 0); 4],
            e_oemid: U16::new(LE, 0),
            e_oeminfo: U16::new(LE, 0),
            e_res2: [U16::new(LE, 0); 10],
            e_lfanew: U32::new(LE, self.nt_headers_offset),
        })
    }

    /// Reserve a fixed DOS header and stub.
    ///
    /// Use `reserve_dos_header` and `reserve` if you need a custom stub.
    pub fn reserve_dos_header_and_stub(&mut self) {
        self.reserve_dos_header();
        self.reserve(64, 1);
    }

    /// Write a fixed DOS header and stub.
    ///
    /// Use `write_custom_dos_header` and `write` if you need a custom stub.
    pub fn write_dos_header_and_stub(&mut self) -> Result<()> {
        self.write_custom_dos_header(&pe::ImageDosHeader {
            e_magic: U16::new(LE, pe::IMAGE_DOS_SIGNATURE),
            e_cblp: U16::new(LE, 0x90),
            e_cp: U16::new(LE, 3),
            e_crlc: U16::new(LE, 0),
            e_cparhdr: U16::new(LE, 4),
            e_minalloc: U16::new(LE, 0),
            e_maxalloc: U16::new(LE, 0xffff),
            e_ss: U16::new(LE, 0),
            e_sp: U16::new(LE, 0xb8),
            e_csum: U16::new(LE, 0),
            e_ip: U16::new(LE, 0),
            e_cs: U16::new(LE, 0),
            e_lfarlc: U16::new(LE, 0x40),
            e_ovno: U16::new(LE, 0),
            e_res: [U16::new(LE, 0); 4],
            e_oemid: U16::new(LE, 0),
            e_oeminfo: U16::new(LE, 0),
            e_res2: [U16::new(LE, 0); 10],
            e_lfanew: U32::new(LE, self.nt_headers_offset),
        })?;

        #[rustfmt::skip]
        self.buffer.write_bytes(&[
            0x0e, 0x1f, 0xba, 0x0e, 0x00, 0xb4, 0x09, 0xcd,
            0x21, 0xb8, 0x01, 0x4c, 0xcd, 0x21, 0x54, 0x68,
            0x69, 0x73, 0x20, 0x70, 0x72, 0x6f, 0x67, 0x72,
            0x61, 0x6d, 0x20, 0x63, 0x61, 0x6e, 0x6e, 0x6f,
            0x74, 0x20, 0x62, 0x65, 0x20, 0x72, 0x75, 0x6e,
            0x20, 0x69, 0x6e, 0x20, 0x44, 0x4f, 0x53, 0x20,
            0x6d, 0x6f, 0x64, 0x65, 0x2e, 0x0d, 0x0d, 0x0a,
            0x24, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);

        Ok(())
    }

    fn nt_headers_size(&self) -> u32 {
        if self.is_64 {
            mem::size_of::<pe::ImageNtHeaders64>() as u32
        } else {
            mem::size_of::<pe::ImageNtHeaders32>() as u32
        }
    }

    fn optional_header_size(&self) -> u32 {
        let size = if self.is_64 {
            mem::size_of::<pe::ImageOptionalHeader64>() as u32
        } else {
            mem::size_of::<pe::ImageOptionalHeader32>() as u32
        };
        size + self.data_directories.len() as u32 * mem::size_of::<pe::ImageDataDirectory>() as u32
    }

    /// Return the offset of the NT headers, if reserved.
    pub fn nt_headers_offset(&self) -> u32 {
        self.nt_headers_offset
    }

    /// Reserve the range for the NT headers.
    pub fn reserve_nt_headers(&mut self, data_directory_num: usize) {
        debug_assert_eq!(self.nt_headers_offset, 0);
        self.nt_headers_offset = self.reserve(self.nt_headers_size(), 8);
        self.data_directories = vec![DataDirectory::default(); data_directory_num];
        self.reserve(
            data_directory_num as u32 * mem::size_of::<pe::ImageDataDirectory>() as u32,
            1,
        );
    }

    /// Set the virtual address and size of a data directory.
    pub fn set_data_directory(&mut self, index: usize, virtual_address: u32, size: u32) {
        self.data_directories[index] = DataDirectory {
            virtual_address,
            size,
        }
    }

    /// Write the NT headers.
    pub fn write_nt_headers(&mut self, nt_headers: NtHeaders) {
        self.pad_until(self.nt_headers_offset);
        self.buffer.write(&U32::new(LE, pe::IMAGE_NT_SIGNATURE));
        let file_header = pe::ImageFileHeader {
            machine: U16::new(LE, nt_headers.machine),
            number_of_sections: U16::new(LE, self.section_header_num),
            time_date_stamp: U32::new(LE, nt_headers.time_date_stamp),
            pointer_to_symbol_table: U32::new(LE, self.symbol_offset),
            number_of_symbols: U32::new(LE, self.symbol_num),
            size_of_optional_header: U16::new(LE, self.optional_header_size() as u16),
            characteristics: U16::new(LE, nt_headers.characteristics),
        };
        self.buffer.write(&file_header);
        if self.is_64 {
            let optional_header = pe::ImageOptionalHeader64 {
                magic: U16::new(LE, pe::IMAGE_NT_OPTIONAL_HDR64_MAGIC),
                major_linker_version: nt_headers.major_linker_version,
                minor_linker_version: nt_headers.minor_linker_version,
                size_of_code: U32::new(LE, self.code_len),
                size_of_initialized_data: U32::new(LE, self.data_len),
                size_of_uninitialized_data: U32::new(LE, self.bss_len),
                address_of_entry_point: U32::new(LE, nt_headers.address_of_entry_point),
                base_of_code: U32::new(LE, self.code_address),
                image_base: U64::new(LE, nt_headers.image_base),
                section_alignment: U32::new(LE, self.section_alignment),
                file_alignment: U32::new(LE, self.file_alignment),
                major_operating_system_version: U16::new(
                    LE,
                    nt_headers.major_operating_system_version,
                ),
                minor_operating_system_version: U16::new(
                    LE,
                    nt_headers.minor_operating_system_version,
                ),
                major_image_version: U16::new(LE, nt_headers.major_image_version),
                minor_image_version: U16::new(LE, nt_headers.minor_image_version),
                major_subsystem_version: U16::new(LE, nt_headers.major_subsystem_version),
                minor_subsystem_version: U16::new(LE, nt_headers.minor_subsystem_version),
                win32_version_value: U32::new(LE, 0),
                size_of_image: U32::new(LE, self.virtual_len),
                size_of_headers: U32::new(LE, self.headers_len),
                check_sum: U32::new(LE, 0),
                subsystem: U16::new(LE, nt_headers.subsystem),
                dll_characteristics: U16::new(LE, nt_headers.dll_characteristics),
                size_of_stack_reserve: U64::new(LE, nt_headers.size_of_stack_reserve),
                size_of_stack_commit: U64::new(LE, nt_headers.size_of_stack_commit),
                size_of_heap_reserve: U64::new(LE, nt_headers.size_of_heap_reserve),
                size_of_heap_commit: U64::new(LE, nt_headers.size_of_heap_commit),
                loader_flags: U32::new(LE, 0),
                number_of_rva_and_sizes: U32::new(LE, self.data_directories.len() as u32),
            };
            self.buffer.write(&optional_header);
        } else {
            let optional_header = pe::ImageOptionalHeader32 {
                magic: U16::new(LE, pe::IMAGE_NT_OPTIONAL_HDR32_MAGIC),
                major_linker_version: nt_headers.major_linker_version,
                minor_linker_version: nt_headers.minor_linker_version,
                size_of_code: U32::new(LE, self.code_len),
                size_of_initialized_data: U32::new(LE, self.data_len),
                size_of_uninitialized_data: U32::new(LE, self.bss_len),
                address_of_entry_point: U32::new(LE, nt_headers.address_of_entry_point),
                base_of_code: U32::new(LE, self.code_address),
                base_of_data: U32::new(LE, self.data_address),
                image_base: U32::new(LE, nt_headers.image_base as u32),
                section_alignment: U32::new(LE, self.section_alignment),
                file_alignment: U32::new(LE, self.file_alignment),
                major_operating_system_version: U16::new(
                    LE,
                    nt_headers.major_operating_system_version,
                ),
                minor_operating_system_version: U16::new(
                    LE,
                    nt_headers.minor_operating_system_version,
                ),
                major_image_version: U16::new(LE, nt_headers.major_image_version),
                minor_image_version: U16::new(LE, nt_headers.minor_image_version),
                major_subsystem_version: U16::new(LE, nt_headers.major_subsystem_version),
                minor_subsystem_version: U16::new(LE, nt_headers.minor_subsystem_version),
                win32_version_value: U32::new(LE, 0),
                size_of_image: U32::new(LE, self.virtual_len),
                size_of_headers: U32::new(LE, self.headers_len),
                check_sum: U32::new(LE, 0),
                subsystem: U16::new(LE, nt_headers.subsystem),
                dll_characteristics: U16::new(LE, nt_headers.dll_characteristics),
                size_of_stack_reserve: U32::new(LE, nt_headers.size_of_stack_reserve as u32),
                size_of_stack_commit: U32::new(LE, nt_headers.size_of_stack_commit as u32),
                size_of_heap_reserve: U32::new(LE, nt_headers.size_of_heap_reserve as u32),
                size_of_heap_commit: U32::new(LE, nt_headers.size_of_heap_commit as u32),
                loader_flags: U32::new(LE, 0),
                number_of_rva_and_sizes: U32::new(LE, self.data_directories.len() as u32),
            };
            self.buffer.write(&optional_header);
        }

        for dir in &self.data_directories {
            self.buffer.write(&pe::ImageDataDirectory {
                virtual_address: U32::new(LE, dir.virtual_address),
                size: U32::new(LE, dir.size),
            })
        }
    }

    /// Reserve the section headers.
    ///
    /// The number of reserved section headers must be the same as the number of sections that
    /// are later reserved.
    // TODO: change this to a maximum number of sections?
    pub fn reserve_section_headers(&mut self, section_header_num: u16) {
        debug_assert_eq!(self.section_header_num, 0);
        self.section_header_num = section_header_num;
        self.reserve(
            u32::from(section_header_num) * mem::size_of::<pe::ImageSectionHeader>() as u32,
            1,
        );
        // Padding before sections must be included in headers_len.
        self.reserve_align(self.file_alignment);
        self.headers_len = self.len;
        self.reserve_virtual(self.len);
    }

    /// Write the section headers.
    ///
    /// This uses information that was recorded when the sections were reserved.
    pub fn write_section_headers(&mut self) {
        debug_assert_eq!(self.section_header_num as usize, self.sections.len());
        for section in &self.sections {
            let section_header = pe::ImageSectionHeader {
                name: section.name,
                virtual_size: U32::new(LE, section.range.virtual_size),
                virtual_address: U32::new(LE, section.range.virtual_address),
                size_of_raw_data: U32::new(LE, section.range.file_size),
                pointer_to_raw_data: U32::new(LE, section.range.file_offset),
                pointer_to_relocations: U32::new(LE, 0),
                pointer_to_linenumbers: U32::new(LE, 0),
                number_of_relocations: U16::new(LE, 0),
                number_of_linenumbers: U16::new(LE, 0),
                characteristics: U32::new(LE, section.characteristics),
            };
            self.buffer.write(&section_header);
        }
    }

    /// Reserve a section.
    ///
    /// Returns the file range and virtual address range that are reserved
    /// for the section.
    pub fn reserve_section(
        &mut self,
        name: [u8; 8],
        characteristics: u32,
        virtual_size: u32,
        data_size: u32,
    ) -> SectionRange {
        let virtual_address = self.reserve_virtual(virtual_size);

        // Padding after section must be included in section file size.
        let file_size = util::align_u32(data_size, self.file_alignment);
        let file_offset = if file_size != 0 {
            self.reserve(file_size, self.file_alignment)
        } else {
            0
        };

        // Sizes in optional header use the virtual size with the file alignment.
        let aligned_virtual_size = util::align_u32(virtual_size, self.file_alignment);
        if characteristics & pe::IMAGE_SCN_CNT_CODE != 0 {
            if self.code_address == 0 {
                self.code_address = virtual_address;
            }
            self.code_len += aligned_virtual_size;
        } else if characteristics & pe::IMAGE_SCN_CNT_INITIALIZED_DATA != 0 {
            if self.data_address == 0 {
                self.data_address = virtual_address;
            }
            self.data_len += aligned_virtual_size;
        } else if characteristics & pe::IMAGE_SCN_CNT_UNINITIALIZED_DATA != 0 {
            if self.data_address == 0 {
                self.data_address = virtual_address;
            }
            self.bss_len += aligned_virtual_size;
        }

        let range = SectionRange {
            virtual_address,
            virtual_size,
            file_offset,
            file_size,
        };
        self.sections.push(Section {
            name,
            characteristics,
            range,
        });
        range
    }

    /// Write the data for a section.
    pub fn write_section(&mut self, offset: u32, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        self.pad_until(offset);
        self.write(data);
        self.write_align(self.file_alignment);
    }

    /// Reserve a `.text` section.
    ///
    /// Contains executable code.
    pub fn reserve_text_section(&mut self, size: u32) -> SectionRange {
        self.reserve_section(
            *b".text\0\0\0",
            pe::IMAGE_SCN_CNT_CODE | pe::IMAGE_SCN_MEM_EXECUTE | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        )
    }

    /// Reserve a `.data` section.
    ///
    /// Contains initialized data.
    ///
    /// May also contain uninitialized data if `virtual_size` is greater than `data_size`.
    pub fn reserve_data_section(&mut self, virtual_size: u32, data_size: u32) -> SectionRange {
        self.reserve_section(
            *b".data\0\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ | pe::IMAGE_SCN_MEM_WRITE,
            virtual_size,
            data_size,
        )
    }

    /// Reserve a `.rdata` section.
    ///
    /// Contains read-only initialized data.
    pub fn reserve_rdata_section(&mut self, size: u32) -> SectionRange {
        self.reserve_section(
            *b".rdata\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        )
    }

    /// Reserve a `.bss` section.
    ///
    /// Contains uninitialized data.
    pub fn reserve_bss_section(&mut self, size: u32) -> SectionRange {
        self.reserve_section(
            *b".bss\0\0\0\0",
            pe::IMAGE_SCN_CNT_UNINITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ | pe::IMAGE_SCN_MEM_WRITE,
            size,
            0,
        )
    }

    /// Reserve an `.idata` section.
    ///
    /// Contains import tables. Note that it is permissible to store import tables in a different
    /// section.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_IMPORT` data directory.
    pub fn reserve_idata_section(&mut self, size: u32) -> SectionRange {
        let range = self.reserve_section(
            *b".idata\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ | pe::IMAGE_SCN_MEM_WRITE,
            size,
            size,
        );
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_IMPORT];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: range.virtual_address,
            size,
        };
        range
    }

    /// Reserve an `.edata` section.
    ///
    /// Contains export tables.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_EXPORT` data directory.
    pub fn reserve_edata_section(&mut self, size: u32) -> SectionRange {
        let range = self.reserve_section(
            *b".edata\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        );
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_EXPORT];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: range.virtual_address,
            size,
        };
        range
    }

    /// Reserve a `.pdata` section.
    ///
    /// Contains exception information.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_EXCEPTION` data directory.
    pub fn reserve_pdata_section(&mut self, size: u32) -> SectionRange {
        let range = self.reserve_section(
            *b".pdata\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        );
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_EXCEPTION];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: range.virtual_address,
            size,
        };
        range
    }

    /// Reserve a `.xdata` section.
    ///
    /// Contains exception information.
    pub fn reserve_xdata_section(&mut self, size: u32) -> SectionRange {
        self.reserve_section(
            *b".xdata\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        )
    }

    /// Reserve a `.rsrc` section.
    ///
    /// Contains the resource directory.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_RESOURCE` data directory.
    pub fn reserve_rsrc_section(&mut self, size: u32) -> SectionRange {
        let range = self.reserve_section(
            *b".rsrc\0\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA | pe::IMAGE_SCN_MEM_READ,
            size,
            size,
        );
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_RESOURCE];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: range.virtual_address,
            size,
        };
        range
    }

    /// Add a base relocation.
    ///
    /// `typ` must be one of the `IMAGE_REL_BASED_*` constants.
    pub fn add_reloc(&mut self, mut virtual_address: u32, typ: u16) {
        let reloc = U16::new(LE, typ << 12 | (virtual_address & 0xfff) as u16);
        virtual_address &= !0xfff;
        if let Some(block) = self.reloc_blocks.last_mut() {
            if block.virtual_address == virtual_address {
                self.relocs.push(reloc);
                block.count += 1;
                return;
            }
            // Blocks must have an even number of relocations.
            if block.count & 1 != 0 {
                self.relocs.push(U16::new(LE, 0));
                block.count += 1;
            }
            debug_assert!(block.virtual_address < virtual_address);
        }
        self.relocs.push(reloc);
        self.reloc_blocks.push(RelocBlock {
            virtual_address,
            count: 1,
        });
    }

    /// Return true if a base relocation has been added.
    pub fn has_relocs(&mut self) -> bool {
        !self.relocs.is_empty()
    }

    /// Reserve a `.reloc` section.
    ///
    /// This contains the base relocations that were added with `add_reloc`.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_BASERELOC` data directory.
    pub fn reserve_reloc_section(&mut self) -> SectionRange {
        if let Some(block) = self.reloc_blocks.last_mut() {
            // Blocks must have an even number of relocations.
            if block.count & 1 != 0 {
                self.relocs.push(U16::new(LE, 0));
                block.count += 1;
            }
        }
        let size = self.reloc_blocks.iter().map(RelocBlock::size).sum();
        let range = self.reserve_section(
            *b".reloc\0\0",
            pe::IMAGE_SCN_CNT_INITIALIZED_DATA
                | pe::IMAGE_SCN_MEM_READ
                | pe::IMAGE_SCN_MEM_DISCARDABLE,
            size,
            size,
        );
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_BASERELOC];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: range.virtual_address,
            size,
        };
        self.reloc_offset = range.file_offset;
        range
    }

    /// Write a `.reloc` section.
    ///
    /// This contains the base relocations that were added with `add_reloc`.
    pub fn write_reloc_section(&mut self) {
        if self.reloc_offset == 0 {
            return;
        }
        self.pad_until(self.reloc_offset);

        let mut total = 0;
        for block in &self.reloc_blocks {
            self.buffer.write(&pe::ImageBaseRelocation {
                virtual_address: U32::new(LE, block.virtual_address),
                size_of_block: U32::new(LE, block.size()),
            });
            self.buffer
                .write_slice(&self.relocs[total..][..block.count as usize]);
            total += block.count as usize;
        }
        debug_assert_eq!(total, self.relocs.len());

        self.write_align(self.file_alignment);
    }

    /// Reserve the certificate table.
    ///
    /// This also sets the `pe::IMAGE_DIRECTORY_ENTRY_SECURITY` data directory.
    // TODO: reserve individual certificates
    pub fn reserve_certificate_table(&mut self, size: u32) {
        let size = util::align_u32(size, 8);
        let offset = self.reserve(size, 8);
        let dir = &mut self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_SECURITY];
        debug_assert_eq!(dir.virtual_address, 0);
        *dir = DataDirectory {
            virtual_address: offset,
            size,
        };
    }

    /// Write the certificate table.
    // TODO: write individual certificates
    pub fn write_certificate_table(&mut self, data: &[u8]) {
        let dir = self.data_directories[pe::IMAGE_DIRECTORY_ENTRY_SECURITY];
        self.pad_until(dir.virtual_address);
        self.write(data);
        self.pad_until(dir.virtual_address + dir.size);
    }
}

/// Information required for writing [`pe::ImageNtHeaders32`] or [`pe::ImageNtHeaders64`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct NtHeaders {
    // ImageFileHeader
    pub machine: u16,
    pub time_date_stamp: u32,
    pub characteristics: u16,
    // ImageOptionalHeader
    pub major_linker_version: u8,
    pub minor_linker_version: u8,
    pub address_of_entry_point: u32,
    pub image_base: u64,
    pub major_operating_system_version: u16,
    pub minor_operating_system_version: u16,
    pub major_image_version: u16,
    pub minor_image_version: u16,
    pub major_subsystem_version: u16,
    pub minor_subsystem_version: u16,
    pub subsystem: u16,
    pub dll_characteristics: u16,
    pub size_of_stack_reserve: u64,
    pub size_of_stack_commit: u64,
    pub size_of_heap_reserve: u64,
    pub size_of_heap_commit: u64,
}

#[derive(Default, Clone, Copy)]
struct DataDirectory {
    virtual_address: u32,
    size: u32,
}

/// Information required for writing [`pe::ImageSectionHeader`].
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct Section {
    pub name: [u8; pe::IMAGE_SIZEOF_SHORT_NAME],
    pub characteristics: u32,
    pub range: SectionRange,
}

/// The file range and virtual address range for a section.
#[allow(missing_docs)]
#[derive(Debug, Default, Clone, Copy)]
pub struct SectionRange {
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub file_offset: u32,
    pub file_size: u32,
}

struct RelocBlock {
    virtual_address: u32,
    count: u32,
}

impl RelocBlock {
    fn size(&self) -> u32 {
        mem::size_of::<pe::ImageBaseRelocation>() as u32 + self.count * mem::size_of::<u16>() as u32
    }
}
