#![allow(non_camel_case_types, non_upper_case_globals)]

use std::mem;

use byteorder::{ByteOrder, LittleEndian};

pub const DW_EH_PE_omit: u8 = 0xFF;
pub const DW_EH_PE_absptr: u8 = 0x00;

pub const DW_EH_PE_uleb128: u8 = 0x01;
pub const DW_EH_PE_udata2: u8 = 0x02;
pub const DW_EH_PE_udata4: u8 = 0x03;
pub const DW_EH_PE_udata8: u8 = 0x04;
pub const DW_EH_PE_sleb128: u8 = 0x09;
pub const DW_EH_PE_sdata2: u8 = 0x0A;
pub const DW_EH_PE_sdata4: u8 = 0x0B;
pub const DW_EH_PE_sdata8: u8 = 0x0C;

pub const DW_EH_PE_pcrel: u8 = 0x10;
pub const DW_EH_PE_textrel: u8 = 0x20;
pub const DW_EH_PE_datarel: u8 = 0x30;
pub const DW_EH_PE_funcrel: u8 = 0x40;
pub const DW_EH_PE_aligned: u8 = 0x50;

pub const DW_EH_PE_indirect: u8 = 0x80;

pub struct DwarfReader<'a> {
    pub slice: &'a [u8],
    pub virt_addr: u32,
}

impl<'a> DwarfReader<'a> {
    pub fn new(slice: &[u8], virt_addr: u32) -> DwarfReader {
        DwarfReader { slice, virt_addr }
    }

    pub fn offset(&mut self, offset: i32) {
        self.slice = &self.slice[offset as usize..];
        self.virt_addr = self.virt_addr.wrapping_add(offset as u32);
    }

    // ULEB128 and SLEB128 encodings are defined in Section 7.6 - "Variable
    // Length Data".
    pub fn read_uleb128(&mut self) -> u64 {
        let mut shift: usize = 0;
        let mut result: u64 = 0;
        let mut byte: u8;
        loop {
            byte = self.read_u8();
            result |= ((byte & 0x7F) as u64) << shift;
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
            result |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        // sign-extend
        if shift < u64::BITS && (byte & 0x40) != 0 {
            result |= (!0 as u64) << shift;
        }
        result as i64
    }

    pub fn read_u8(&mut self) -> u8 {
        let val = self.slice[0];
        self.slice = &self.slice[1..];
        val
    }
}

macro_rules! impl_read_fn {
    ( $($type: ty, $byteorder_fn: ident);* ) => {
        impl<'a> DwarfReader<'a> {
            $(
                pub fn $byteorder_fn(&mut self) -> $type {
                    let val = LittleEndian::$byteorder_fn(self.slice);
                    self.slice = &self.slice[mem::size_of::<$type>()..];
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

pub struct DwarfWriter<'a> {
    pub slice: &'a mut [u8],
    pub offset: usize,
}

impl<'a> DwarfWriter<'a> {
    pub fn new(slice: &mut [u8]) -> DwarfWriter {
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
        DW_EH_PE_absptr => Ok(reader.read_u32() as usize),
        DW_EH_PE_uleb128 => Ok(reader.read_uleb128() as usize),
        DW_EH_PE_udata2 => Ok(reader.read_u16() as usize),
        DW_EH_PE_udata4 => Ok(reader.read_u32() as usize),
        DW_EH_PE_udata8 => Ok(reader.read_u64() as usize),
        DW_EH_PE_sleb128 => Ok(reader.read_sleb128() as usize),
        DW_EH_PE_sdata2 => Ok(reader.read_i16() as usize),
        DW_EH_PE_sdata4 => Ok(reader.read_i32() as usize),
        DW_EH_PE_sdata8 => Ok(reader.read_i64() as usize),
        _ => Err(()),
    }
}

fn read_encoded_pointer_with_pc(
    reader: &mut DwarfReader,
    encoding: u8,
) -> Result<usize, ()> {
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
fn round_up(unrounded: usize, align: usize) -> Result<usize, ()> {
    if align.is_power_of_two() {
        Ok((unrounded + align - 1) & !(align - 1))
    } else {
        Err(())
    }
}

// Minimalistic structure to store everything needed for parsing FDEs to synthesize
// .eh_frame_hdr section. Since we are only linking 1 object file, there should only be 1 call
// frame information (CFI) record, so there should be only 1 common information entry (CIE).
// So the class parses the only CIE on init, cache the encoding info, then parse the FDE on
// iterations based on the cached encoding format.
pub struct EH_Frame<'a> {
    // It refers to the augmentation data that corresponds to 'R' in the augmentation string
    pub fde_pointer_encoding: u8,
    pub fde_reader: DwarfReader<'a>,
    pub fde_sz: usize,
}

impl<'a> EH_Frame<'a> {
    pub fn new(eh_frame_slice: &[u8], eh_frame_addr: u32) -> Result<EH_Frame, ()> {
        let mut cie_reader = DwarfReader::new(eh_frame_slice, eh_frame_addr);
        let eh_frame_size = eh_frame_slice.len();
        let length = cie_reader.read_u32();
        let fde_reader = match length {
            // eh_frame with 0 lengths means the CIE is terminated
            // while length == u32::MAX means that the length is only representable with 64 bits,
            // which does not make sense in a system with 32-bit address.
            0 | 0xFFFFFFFF => unimplemented!(),
            _ => {
                let mut fde_reader = DwarfReader::new(cie_reader.slice, cie_reader.virt_addr);
                fde_reader.offset(length as i32);
                fde_reader
            }
        };
        let fde_sz = eh_frame_size - mem::size_of::<u32>() - length as usize;

        // Routine check on the .eh_frame well-formness, in terms of CIE ID & Version args.
        assert_eq!(cie_reader.read_u32(), 0);
        assert_eq!(cie_reader.read_u8(), 1);

        // Parse augmentation string
        // The first character must be 'z', there is no way to proceed otherwise
        assert_eq!(cie_reader.read_u8(), b'z');

        // Establish a pointer that skips ahead of the string
        // Skip code/data alignment factors & return address register along the way as well
        // We only tackle the case where 'z' and 'R' are part of the augmentation string, otherwise
        // we cannot get the addresses to make .eh_frame_hdr
        let mut aug_data_reader = DwarfReader::new(cie_reader.slice, cie_reader.virt_addr);
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

        Ok(EH_Frame { fde_pointer_encoding, fde_reader, fde_sz })
    }

    pub fn iterate_fde(&self, callback: &mut dyn FnMut(u32, u32)) -> Result<(), ()> {
        // Parse each FDE to obtain the starting address that the FDE applies to
        // Send the FDE offset and the mentioned address to a callback that write up the
        // .eh_frame_hdr section
        let mut remaining_len = self.fde_sz;
        let mut reader = DwarfReader::new(self.fde_reader.slice, self.fde_reader.virt_addr);
        loop {
            if remaining_len == 0 {
                break;
            }

            let fde_virt_addr = reader.virt_addr;
            let length = match reader.read_u32() {
                0 | 0xFFFFFFFF => unimplemented!(),
                other => other,
            };

            // Remove the length of the header and the content from the counter
            remaining_len -= length as usize + mem::size_of::<u32>();
            let mut next_fde_reader = DwarfReader::new(reader.slice, reader.virt_addr);
            next_fde_reader.offset(length as i32);

            // Only parse FDEs, indicated by its CIE pointer being non-zero
            let cie_ptr = reader.read_u32();
            if cie_ptr != 0 {
                // Parse PC Begin using the encoding scheme mentioned in the CIE
                let pc_begin = read_encoded_pointer_with_pc(&mut reader, self.fde_pointer_encoding)?;

                callback(pc_begin as u32, fde_virt_addr);
            }

            reader = next_fde_reader;
        }

        Ok(())
    }
}

pub struct EH_Frame_Hdr<'a> {
    fde_writer: DwarfWriter<'a>,
    eh_frame_hdr_addr: u32,
    fdes: Vec<(u32, u32)>,
}

impl<'a> EH_Frame_Hdr<'a> {
    // Create a EH_Frame_Hdr object, and write out the fixed fields of .eh_frame_hdr to memory
    // eh_frame_ptr_enc will be 0x1B (PC-relative, 4 bytes)
    // table_enc will be 0x3B (Relative to the start of .eh_frame_hdr, 4 bytes)
    // Load address is not known at this point.
    pub fn new(
        eh_frame_hdr_slice: &mut [u8],
        eh_frame_hdr_addr: u32,
        eh_frame_addr: u32,
    ) -> EH_Frame_Hdr {
        let mut writer = DwarfWriter::new(eh_frame_hdr_slice);
        writer.write_u8(1);
        writer.write_u8(0x1B);
        writer.write_u8(0x03);
        writer.write_u8(0x3B);

        let eh_frame_offset =
            (eh_frame_addr).wrapping_sub(eh_frame_hdr_addr + ((mem::size_of::<u8>() as u32) * 4));
        writer.write_u32(eh_frame_offset);
        writer.write_u32(0);

        EH_Frame_Hdr { fde_writer: writer, eh_frame_hdr_addr, fdes: Vec::new() }
    }

    fn fde_count_offset() -> usize {
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
        LittleEndian::write_u32(&mut self.fde_writer.slice[Self::fde_count_offset()..], self.fdes.len() as u32);
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
            if entry_length == 0 || entry_length == 0xFFFFFFFF {
                unimplemented!()
            }

            // This slot stores the CIE ID (for CIE)/CIE Pointer (for FDE).
            // This value must be non-zero for FDEs.
            let cie_ptr = reader.read_u32();
            if cie_ptr != 0 {
                fde_count += 1;
            }

            reader.offset(entry_length as i32 - mem::size_of::<u32>() as i32)
        }

        12 + fde_count * 8
    }
}
