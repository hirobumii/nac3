#![allow(nonstandard_style, non_upper_case_globals)]

use crate::include::dwarf::*;
use std::mem;

use byteorder::{ByteOrder, LittleEndian};

#[derive(Clone)]
pub struct DwarfReader<'a> {
    pub slice: &'a [u8],
    pub virt_addr: u32,
}

impl DwarfReader<'_> {
    pub const fn new(slice: &[u8], virt_addr: u32) -> DwarfReader<'_> {
        DwarfReader { slice, virt_addr }
    }

    pub fn offset(&mut self, offset: u32) {
        self.slice = &self.slice[offset as usize..];
        self.virt_addr = self.virt_addr.wrapping_add(offset);
    }

    /// ULEB128 and SLEB128 encodings are defined in Section 7.6 - "Variable Length Data" of the
    /// [DWARF-4 Manual](https://dwarfstd.org/doc/DWARF4.pdf).
    pub fn read_uleb128(&mut self) -> u64 {
        let mut shift: usize = 0;
        let mut result: u64 = 0;
        let mut byte: u8;
        loop {
            byte = self.read_u8();
            result |= u64::from(byte & 0x7F) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        result
    }

    pub fn read_sleb128(&mut self) -> i64 {
        let mut shift: u32 = 0;
        let mut result: u64 = 0;
        let mut byte: u8;
        loop {
            byte = self.read_u8();
            result |= u64::from(byte & 0x7F) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        // sign-extend
        if shift < u64::BITS && (byte & 0x40) != 0 {
            result |= (!0u64) << shift;
        }
        result as i64
    }

    pub fn read_u8(&mut self) -> u8 {
        let val = self.slice[0];
        self.slice = &self.slice[1..];
        self.virt_addr += 1;
        val
    }

    pub fn read_i8(&mut self) -> i8 {
        let val = self.slice[0] as i8;
        self.slice = &self.slice[1..];
        self.virt_addr += 1;
        val
    }

    pub fn read_slice(&mut self, len: usize) -> &[u8] {
        let (slice, remaining) = self.slice.split_at(len);
        self.slice = remaining;
        self.virt_addr += len as u32;
        slice
    }

    pub fn read_str(&mut self) -> &[u8] {
        let str_len = self
            .slice
            .iter()
            .position(|byte| *byte == 0)
            .expect("string should be null-terminated");
        &self.read_slice(str_len + 1)[..str_len] // null-terminator
    }
}

macro_rules! impl_read_fn {
    ( $($type: ty, $byteorder_fn: ident);* ) => {
        impl<'a> DwarfReader<'a> {
            $(
                pub fn $byteorder_fn(&mut self) -> $type {
                    let val = LittleEndian::$byteorder_fn(self.slice);
                    self.slice = &self.slice[mem::size_of::<$type>()..];
                    self.virt_addr += mem::size_of::<$type>() as u32;
                    val
                }
            )*
        }
    }
}

impl_read_fn!(
    u16,    read_u16;
    u32,    read_u32;
    u64,    read_u64;
    i16,    read_i16;
    i32,    read_i32;
    i64,    read_i64
);

macro_rules! read_block_fn {
    ( $($read_block_fn: ident, $read_length_fn: ident);* ) => {
        impl<'a> DwarfReader<'a> {
            $(
                pub fn $read_block_fn(&mut self) -> &[u8] {
                    let len = self.$read_length_fn() as usize;
                    self.read_slice(len)
                }
            )*
        }
    }
}

read_block_fn!(
    read_form_block,    read_uleb128;
    read_form_block1,   read_u8;
    read_form_block2,   read_u16;
    read_form_block4,   read_u32
);

macro_rules! alias_read_fn {
    ( $($type: ty, $read_form_fn: ident, $aliased_fn: ident);* ) => {
        impl<'a> DwarfReader<'a> {
            $(
                pub fn $read_form_fn(&mut self) -> $type {
                    self.$aliased_fn()
                }
            )*
        }
    }
}

alias_read_fn!(
    u32,    read_form_addr,         read_u32;
    u8,     read_form_data1,        read_u8;
    u16,    read_form_data2,        read_u16;
    u32,    read_form_data4,        read_u32;
    u64,    read_form_data8,        read_u64;
    i64,    read_form_sdata,        read_i64;
    u64,    read_form_udata,        read_u64;
    &[u8],  read_form_exprloc,      read_form_block;
    u8,     read_form_flag,         read_u8;
    u32,    read_form_sec_offset,   read_u32
);

impl DwarfReader<'_> {
    // Helper to perform constant read
    // Everything is casted as u64 (as the broadest type).
    // Use at your own risk.
    pub fn read_form_constant(&mut self, attr_form: u64) -> u64 {
        match attr_form {
            DW_FORM_data1 => self.read_form_data1() as u64,
            DW_FORM_data2 => self.read_form_data2() as u64,
            DW_FORM_data4 => self.read_form_data4() as u64,
            DW_FORM_data8 => self.read_form_data8() as u64,
            DW_FORM_sdata => self.read_form_sdata() as u64,
            DW_FORM_udata => self.read_form_udata() as u64,
            _ => unreachable!("form should be a constant"),
        }
    }

    pub fn skip_form(&mut self, attr_form: u64) {
        match attr_form {
            DW_FORM_addr | DW_FORM_ref_addr | DW_FORM_strp => {
                self.read_form_addr();
            }
            DW_FORM_data1 | DW_FORM_ref1 | DW_FORM_flag => {
                self.read_form_data1();
            }
            DW_FORM_data2 | DW_FORM_ref2 => {
                self.read_form_data2();
            }
            DW_FORM_data4 | DW_FORM_ref4 => {
                self.read_form_data4();
            }
            DW_FORM_data8 | DW_FORM_ref8 => {
                self.read_form_data8();
            }
            DW_FORM_sdata => {
                self.read_form_sdata();
            }
            DW_FORM_udata | DW_FORM_ref_udata => {
                self.read_form_udata();
            }
            DW_FORM_block | DW_FORM_exprloc => {
                self.read_form_block();
            }
            DW_FORM_block1 => {
                self.read_form_block1();
            }
            DW_FORM_block2 => {
                self.read_form_block2();
            }
            DW_FORM_block4 => {
                self.read_form_block4();
            }
            DW_FORM_string => {
                self.read_str();
            }
            DW_FORM_ref_sig8 => {
                self.read_u64();
            }
            DW_FORM_flag_present => (),

            DW_FORM_sec_offset => {
                self.read_form_sec_offset();
            }

            DW_FORM_indirect => {
                // Section 7.5.3
                // ... the attribute value itself ... begins with an unsigned
                // LEB128 number that represents its form.
                let inner_form = self.read_uleb128();
                self.skip_form(inner_form);
            }

            _ => unreachable!("unrecognized attribute form"),
        }
    }
}

pub struct DwarfWriter<'a> {
    pub slice: &'a mut [u8],
    pub offset: usize,
}

impl DwarfWriter<'_> {
    pub const fn new(slice: &mut [u8]) -> DwarfWriter<'_> {
        DwarfWriter { slice, offset: 0 }
    }

    pub fn write_u8(&mut self, data: u8) {
        self.slice[self.offset] = data;
        self.offset += 1;
    }

    pub fn write_u32(&mut self, data: u32) {
        LittleEndian::write_u32(&mut self.slice[self.offset..], data);
        self.offset += 4;
    }
}

fn read_encoded_pointer(reader: &mut DwarfReader, encoding: u8) -> Result<usize, ()> {
    if encoding == DW_EH_PE_omit {
        return Err(());
    }

    // DW_EH_PE_aligned implies it's an absolute pointer value
    // However, we are linking library for 32-bits architecture
    // The size of variable should be 4 bytes instead
    if encoding == DW_EH_PE_aligned {
        let shifted_virt_addr = round_up(reader.virt_addr as usize, mem::size_of::<u32>())?;
        let addr_inc = shifted_virt_addr - reader.virt_addr as usize;

        reader.slice = &reader.slice[addr_inc..];
        reader.virt_addr = shifted_virt_addr as u32;
        return Ok(reader.read_u32() as usize);
    }

    match encoding & 0x0F {
        DW_EH_PE_absptr | DW_EH_PE_udata4 => Ok(reader.read_u32() as usize),
        DW_EH_PE_uleb128 => Ok(reader.read_uleb128() as usize),
        DW_EH_PE_udata2 => Ok(reader.read_u16() as usize),
        DW_EH_PE_udata8 => Ok(reader.read_u64() as usize),
        DW_EH_PE_sleb128 => Ok(reader.read_sleb128() as usize),
        DW_EH_PE_sdata2 => Ok(reader.read_i16() as usize),
        DW_EH_PE_sdata4 => Ok(reader.read_i32() as usize),
        DW_EH_PE_sdata8 => Ok(reader.read_i64() as usize),
        _ => Err(()),
    }
}

fn read_encoded_pointer_with_pc(reader: &mut DwarfReader, encoding: u8) -> Result<usize, ()> {
    let entry_virt_addr = reader.virt_addr;
    let mut result = read_encoded_pointer(reader, encoding)?;

    // DW_EH_PE_aligned implies it's an absolute pointer value
    if encoding == DW_EH_PE_aligned {
        return Ok(result);
    }

    result = match encoding & 0x70 {
        DW_EH_PE_pcrel => result.wrapping_add(entry_virt_addr as usize),

        // .eh_frame normally would not have these kinds of relocations
        // These would not be supported by a dedicated linker relocation schemes for RISC-V
        DW_EH_PE_textrel | DW_EH_PE_datarel | DW_EH_PE_funcrel | DW_EH_PE_aligned => {
            unimplemented!()
        }

        // Other values should be impossible
        _ => unreachable!(),
    };

    if encoding & DW_EH_PE_indirect != 0 {
        // There should not be a need for indirect addressing, as assembly code from
        // the dynamic library should not be freely moved relative to the EH frame.
        unreachable!()
    }

    Ok(result)
}

#[inline]
const fn round_up(unrounded: usize, align: usize) -> Result<usize, ()> {
    if align.is_power_of_two() { Ok((unrounded + align - 1) & !(align - 1)) } else { Err(()) }
}

/// Minimalistic structure to store everything needed for parsing FDEs to synthesize `.eh_frame_hdr`
/// section.
///
/// Refer to [The Linux Standard Base Core Specification, Generic Part](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/ehframechpt.html)
/// for more information.
pub struct EH_Frame<'a> {
    reader: DwarfReader<'a>,
}

impl<'a> EH_Frame<'a> {
    /// Creates an [`EH_Frame`] using the bytes in the `.eh_frame` section and its address in the
    /// ELF file.
    pub const fn new(eh_frame_slice: &[u8], eh_frame_addr: u32) -> EH_Frame<'_> {
        EH_Frame { reader: DwarfReader::new(eh_frame_slice, eh_frame_addr) }
    }

    /// Returns an [Iterator] over all Call Frame Information (CFI) records.
    pub fn cfi_records(&self) -> CFI_Records<'a> {
        let reader = self.reader.clone();
        let len = reader.slice.len();

        CFI_Records { reader, available: len }
    }
}

/// A single Call Frame Information (CFI) record.
///
/// From the [specification](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/ehframechpt.html):
///
/// > Each CFI record contains a Common Information Entry (CIE) record followed by 1 or more Frame
/// > Description Entry (FDE) records.
pub struct CFI_Record<'a> {
    // It refers to the augmentation data that corresponds to 'R' in the augmentation string
    fde_pointer_encoding: u8,
    fde_reader: DwarfReader<'a>,
}

impl<'a> CFI_Record<'a> {
    pub fn from_reader(cie_reader: &mut DwarfReader<'a>) -> Result<Self, ()> {
        let length = cie_reader.read_u32();
        let fde_reader = match length {
            // eh_frame with 0 lengths means the CIE is terminated
            0 => panic!("Cannot create an EH_Frame from a termination CIE"),

            // length == u32::MAX means that the length is only representable with 64 bits,
            // which does not make sense in a system with 32-bit address.
            0xFFFF_FFFF => unimplemented!(),

            _ => {
                let mut fde_reader = cie_reader.clone();
                fde_reader.offset(length);
                fde_reader
            }
        };

        // Routine check on the .eh_frame well-formness, in terms of CIE ID & Version args.
        let cie_ptr = cie_reader.read_u32();
        assert_eq!(cie_ptr, 0);
        assert_eq!(cie_reader.read_u8(), 1);

        // Parse augmentation string
        // The first character must be 'z', there is no way to proceed otherwise
        assert_eq!(cie_reader.read_u8(), b'z');

        // Establish a pointer that skips ahead of the string
        // Skip code/data alignment factors & return address register along the way as well
        // We only tackle the case where 'z' and 'R' are part of the augmentation string, otherwise
        // we cannot get the addresses to make .eh_frame_hdr
        let mut aug_data_reader = cie_reader.clone();
        let mut aug_str_len = 0;
        loop {
            if aug_data_reader.read_u8() == b'\0' {
                break;
            }
            aug_str_len += 1;
        }
        if aug_str_len == 0 {
            unimplemented!();
        }
        aug_data_reader.read_uleb128(); // Code alignment factor
        aug_data_reader.read_sleb128(); // Data alignment factor
        aug_data_reader.read_uleb128(); // Return address register
        aug_data_reader.read_uleb128(); // Augmentation data length
        let mut fde_pointer_encoding = DW_EH_PE_omit;
        for _ in 0..aug_str_len {
            match cie_reader.read_u8() {
                b'L' => {
                    aug_data_reader.read_u8();
                }

                b'P' => {
                    let encoding = aug_data_reader.read_u8();
                    read_encoded_pointer(&mut aug_data_reader, encoding)?;
                }

                b'R' => {
                    fde_pointer_encoding = aug_data_reader.read_u8();
                }

                // Other characters are not supported
                _ => unimplemented!(),
            }
        }
        assert_ne!(fde_pointer_encoding, DW_EH_PE_omit);

        Ok(CFI_Record { fde_pointer_encoding, fde_reader })
    }

    /// Returns a [`DwarfReader`] initialized to the first Frame Description Entry (FDE) of this CFI
    /// record.
    pub fn get_fde_reader(&self) -> DwarfReader<'a> {
        self.fde_reader.clone()
    }

    /// Returns an [Iterator] over all Frame Description Entries (FDEs).
    pub fn fde_records(&self) -> FDE_Records<'a> {
        let reader = self.get_fde_reader();
        let len = reader.slice.len();

        FDE_Records { pointer_encoding: self.fde_pointer_encoding, reader, available: len }
    }
}

/// [Iterator] over Call Frame Information (CFI) records in an
/// [Exception Handling (EH) frame][EH_Frame].
pub struct CFI_Records<'a> {
    reader: DwarfReader<'a>,
    available: usize,
}

impl<'a> Iterator for CFI_Records<'a> {
    type Item = CFI_Record<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.available == 0 {
                return None;
            }

            let mut this_reader = self.reader.clone();

            // Remove the length of the header and the content from the counter
            let length = self.reader.read_u32();
            let length = match length {
                // eh_frame with 0-length means the CIE is terminated
                0 => return None,
                0xFFFF_FFFF => unimplemented!("CIE entries larger than 4 bytes not supported"),
                other => other,
            } as usize;

            // Remove the length of the header and the content from the counter
            self.available -= length + mem::size_of::<u32>();
            let mut next_reader = self.reader.clone();
            next_reader.offset(length as u32);

            let cie_ptr = self.reader.read_u32();

            self.reader = next_reader;

            // Skip this record if it is a FDE
            if cie_ptr == 0 {
                // Rewind back to the start of the CFI Record
                return Some(CFI_Record::from_reader(&mut this_reader).ok().unwrap());
            }
        }
    }
}

/// [Iterator] over Frame Description Entries (FDEs) in an
/// [Exception Handling (EH) frame][EH_Frame].
pub struct FDE_Records<'a> {
    pointer_encoding: u8,
    reader: DwarfReader<'a>,
    available: usize,
}

impl Iterator for FDE_Records<'_> {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        // Parse each FDE to obtain the starting address that the FDE applies to
        // Send the FDE offset and the mentioned address to a callback that write up the
        // .eh_frame_hdr section

        if self.available == 0 {
            return None;
        }

        let fde_addr = self.reader.virt_addr;

        // Remove the length of the header and the content from the counter
        let length = match self.reader.read_u32() {
            // eh_frame with 0-length means the CIE is terminated
            0 => return None,
            0xFFFF_FFFF => unimplemented!("CIE entries larger than 4 bytes not supported"),
            other => other,
        } as usize;

        // Remove the length of the header and the content from the counter
        self.available -= length + mem::size_of::<u32>();
        let mut next_fde_reader = self.reader.clone();
        next_fde_reader.offset(length as u32);

        let cie_ptr = self.reader.read_u32();
        let next_val = if cie_ptr != 0 {
            let pc_begin = read_encoded_pointer_with_pc(&mut self.reader, self.pointer_encoding)
                .expect("Failed to read PC Begin");
            Some((pc_begin as u32, fde_addr))
        } else {
            None
        };

        self.reader = next_fde_reader;

        next_val
    }
}

pub struct EH_Frame_Hdr<'a> {
    fde_writer: DwarfWriter<'a>,
    eh_frame_hdr_addr: u32,
    fdes: Vec<(u32, u32)>,
}

impl EH_Frame_Hdr<'_> {
    /// Create a [`EH_Frame_Hdr`] object, and write out the fixed fields of `.eh_frame_hdr` to
    /// memory.
    ///
    /// Load address is not known at this point.
    pub fn new(
        eh_frame_hdr_slice: &mut [u8],
        eh_frame_hdr_addr: u32,
        eh_frame_addr: u32,
    ) -> EH_Frame_Hdr<'_> {
        let mut writer = DwarfWriter::new(eh_frame_hdr_slice);

        writer.write_u8(1); // version
        writer.write_u8(0x1B); // eh_frame_ptr_enc - PC-relative 4-byte signed value
        writer.write_u8(0x03); // fde_count_enc - 4-byte unsigned value
        writer.write_u8(0x3B); // table_enc - .eh_frame_hdr section-relative 4-byte signed value

        let eh_frame_offset = eh_frame_addr.wrapping_sub(eh_frame_hdr_addr + writer.offset as u32);
        writer.write_u32(eh_frame_offset); // eh_frame_ptr
        writer.write_u32(0); // `fde_count`, will be written in finalize_fde

        EH_Frame_Hdr { fde_writer: writer, eh_frame_hdr_addr, fdes: Vec::new() }
    }

    /// The offset of the `fde_count` value relative to the start of the `.eh_frame_hdr` section in
    /// bytes.
    const fn fde_count_offset() -> usize {
        8
    }

    pub fn add_fde(&mut self, init_loc: u32, addr: u32) {
        self.fdes.push((
            init_loc.wrapping_sub(self.eh_frame_hdr_addr),
            addr.wrapping_sub(self.eh_frame_hdr_addr),
        ));
    }

    pub fn finalize_fde(mut self) {
        self.fdes
            .sort_by(|(left_init_loc, _), (right_init_loc, _)| left_init_loc.cmp(right_init_loc));
        for (init_loc, addr) in &self.fdes {
            self.fde_writer.write_u32(*init_loc);
            self.fde_writer.write_u32(*addr);
        }
        LittleEndian::write_u32(
            &mut self.fde_writer.slice[Self::fde_count_offset()..],
            self.fdes.len() as u32,
        );
    }

    pub fn size_from_eh_frame(eh_frame: &[u8]) -> usize {
        // The virtual address of the EH frame does not matter in this case
        // Calculation of size does not involve modifying any headers
        let mut reader = DwarfReader::new(eh_frame, 0);
        let mut fde_count = 0;
        while !reader.slice.is_empty() {
            // The original length field should be able to hold the entire value.
            // The device memory space is limited to 32-bits addresses anyway.
            let entry_length = reader.read_u32();
            if entry_length == 0 || entry_length == 0xFFFF_FFFF {
                unimplemented!()
            }

            // This slot stores the CIE ID (for CIE)/CIE Pointer (for FDE).
            // This value must be non-zero for FDEs.
            let cie_ptr = reader.read_u32();
            if cie_ptr != 0 {
                fde_count += 1;
            }

            reader.offset(entry_length - mem::size_of::<u32>() as u32);
        }

        12 + fde_count * 8
    }
}
