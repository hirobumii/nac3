use dwarf::*;
use elf::*;
use std::collections::HashMap;
use std::{mem, ptr, slice, str};

extern crate byteorder;
use byteorder::{ByteOrder, LittleEndian};

mod dwarf;
mod elf;

#[derive(PartialEq, Clone, Copy)]
pub enum Isa {
    CortexA9,
    RiscV32,
}

#[derive(Debug)]
pub enum Error {
    Parsing(&'static str),
    Lookup(&'static str),
}

impl From<&'static str> for Error {
    fn from(desc: &'static str) -> Error {
        Error::Parsing(desc)
    }
}

pub trait Relocatable {
    fn offset(&self) -> Elf32_Addr;
    fn type_info(&self) -> u8;
    fn sym_info(&self) -> Elf32_Word;
    fn addend(&self, sec_image: &[u8]) -> Elf32_Sword;
}

impl Relocatable for Elf32_Rel {
    fn offset(&self) -> Elf32_Addr {
        self.r_offset
    }
    fn type_info(&self) -> u8 {
        ELF32_R_TYPE(self.r_info)
    }
    fn sym_info(&self) -> Elf32_Word {
        ELF32_R_SYM(self.r_info)
    }
    fn addend(&self, sec_image: &[u8]) -> Elf32_Sword {
        LittleEndian::read_i32(&sec_image[self.offset() as usize..])
    }
}

impl Relocatable for Elf32_Rela {
    fn offset(&self) -> Elf32_Addr {
        self.r_offset
    }
    fn type_info(&self) -> u8 {
        ELF32_R_TYPE(self.r_info)
    }
    fn sym_info(&self) -> Elf32_Word {
        ELF32_R_SYM(self.r_info)
    }
    fn addend(&self, _: &[u8]) -> Elf32_Sword {
        self.r_addend
    }
}

struct SectionRecord<'a> {
    shdr: Elf32_Shdr,
    name: &'a str,
    data: Vec<u8>,
}

fn read_unaligned<T: Copy>(data: &[u8], offset: usize) -> Result<T, ()> {
    if data.len() < offset + mem::size_of::<T>() {
        Err(())
    } else {
        let ptr = data.as_ptr().wrapping_offset(offset as isize) as *const T;
        Ok(unsafe { ptr::read_unaligned(ptr) })
    }
}

pub fn get_ref_slice<T: Copy>(data: &[u8], offset: usize, len: usize) -> Result<&[T], ()> {
    if data.len() < offset + mem::size_of::<T>() * len {
        Err(())
    } else {
        let ptr = data.as_ptr().wrapping_offset(offset as isize) as *const T;
        Ok(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

fn from_struct_vec<T>(struct_vec: Vec<T>) -> Vec<u8> {
    let ptr = struct_vec.as_ptr();
    unsafe { slice::from_raw_parts(ptr as *const u8, struct_vec.len() * mem::size_of::<T>()) }
        .to_vec()
}

fn to_struct_slice<T>(bytes: &[u8]) -> &[T] {
    unsafe { slice::from_raw_parts(bytes.as_ptr() as *const T, bytes.len() / mem::size_of::<T>()) }
}

fn to_struct_mut_slice<T>(bytes: &mut [u8]) -> &mut [T] {
    unsafe {
        slice::from_raw_parts_mut(bytes.as_mut_ptr() as *mut T, bytes.len() / mem::size_of::<T>())
    }
}

fn elf_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for c in name {
        h = (h << 4) + *c as u32;
        let g = h & 0xf0000000;
        if g != 0 {
            h ^= g >> 24;
            h &= !g;
        }
    }
    h
}

fn name_starting_at_slice(slice: &[u8], offset: usize) -> Result<&[u8], Error> {
    let size = slice
        .iter()
        .skip(offset)
        .position(|&x| x == 0)
        .ok_or("symbol in symbol table not null-terminated")?;
    Ok(slice.get(offset..offset + size).ok_or("cannot read symbol name")?)
}

macro_rules! get_section_by_name {
    ($linker: ident, $sec_name: expr) => {
        $linker.elf_shdrs.iter().find(|rec| rec.name == $sec_name)
    };
}

macro_rules! get_mut_section_by_name {
    ($linker: ident, $sec_name: expr) => {
        $linker.elf_shdrs.iter_mut().find(|rec| rec.name == $sec_name)
    };
}

struct SymbolTableReader<'a> {
    symtab: &'a [Elf32_Sym],
    strtab: &'a [u8],
}

impl<'a> SymbolTableReader<'a> {
    pub fn find_index_by_name(&self, sym_name: &[u8]) -> Option<usize> {
        self.symtab.iter().position(|sym| {
            if let Ok(dynsym_name) = name_starting_at_slice(self.strtab, sym.st_name as usize) {
                sym_name == dynsym_name
            } else {
                false
            }
        })
    }
}

pub struct Linker<'a> {
    isa: Isa,
    symtab: &'a [Elf32_Sym],
    strtab: &'a [u8],
    elf_shdrs: Vec<SectionRecord<'a>>,
    section_map: HashMap<usize, usize>,
    image: Vec<u8>,
    load_offset: u32,
    rela_dyn_relas: Vec<Elf32_Rela>,
}

impl<'a> Linker<'a> {
    fn get_dynamic_symbol_table(&self) -> Result<SymbolTableReader, Error> {
        let dynsym_rec = get_section_by_name!(self, ".dynsym")
            .ok_or("cannot make SymbolTableReader using .dynsym")?;
        Ok(SymbolTableReader {
            symtab: to_struct_slice::<Elf32_Sym>(dynsym_rec.data.as_slice()),
            strtab: self.elf_shdrs[dynsym_rec.shdr.sh_link as usize].data.as_slice(),
        })
    }

    fn load_section(&mut self, shdr: &Elf32_Shdr, sh_name_str: &'a str, data: Vec<u8>) -> usize {
        let mut elf_shdr = shdr.clone();

        // Maintain alignment requirement specified in sh_addralign
        let align = shdr.sh_addralign;
        let padding = (align - (self.load_offset % align)) % align;
        self.load_offset += padding;

        elf_shdr.sh_addr =
            if (shdr.sh_flags as usize & SHF_ALLOC) == SHF_ALLOC { self.load_offset } else { 0 };
        elf_shdr.sh_offset = self.load_offset;
        self.elf_shdrs.push(SectionRecord { shdr: elf_shdr, name: sh_name_str, data });

        self.load_offset += shdr.sh_size;

        self.elf_shdrs.len() - 1
    }

    // Perform relocation according to the relocation entries
    // Only symbols that support relative addressing would be resolved
    // This is because the loading address is not known yet
    fn resolve_relocatables<R: Relocatable>(
        &mut self,
        relocs: &[R],
        target_section: Elf32_Word,
    ) -> Result<(), Error> {
        for reloc in relocs {
            let sym = match reloc.sym_info() as usize {
                STN_UNDEF => None,
                sym_index => Some(
                    self.symtab
                        .get(sym_index as usize)
                        .ok_or("symbol out of bounds of symbol table")?,
                ),
            };

            let resolve_symbol_addr =
                |sym_option: Option<&Elf32_Sym>| -> Result<Elf32_Word, Error> {
                    let sym = match sym_option {
                        Some(sym) => sym,
                        None => return Ok(0),
                    };

                    match sym.st_shndx {
                        SHN_UNDEF => Err(Error::Lookup("undefined symbol")),
                        SHN_ABS => Ok(sym.st_value),
                        sec_ind => self
                            .section_map
                            .get(&(sec_ind as usize))
                            .map(|&elf_sec_ind: &usize| {
                                // Unlike the code in artiq libdyld, the image offset value is
                                // irrelevant in this case.
                                // The .elf dynamic library can be linked to an arbitrary address
                                // within the kernel address space
                                self.elf_shdrs[elf_sec_ind].shdr.sh_offset as Elf32_Word
                                    + sym.st_value
                            })
                            .ok_or(Error::Parsing("section not mapped to the ELF file")),
                    }
                };

            let get_target_section_index = || -> Result<usize, Error> {
                self.section_map
                    .get(&(target_section as usize))
                    .map(|&index| index)
                    .ok_or(Error::Parsing("Cannot find section with matching sh_index"))
            };

            struct RelocInfo<'a, R> {
                pub defined_val: bool,
                pub indirect_reloc: Option<&'a R>,
                pub pc_relative: bool,
                pub relocate: Option<Box<dyn Fn(&mut [u8], Elf32_Word)>>,
            }

            let classify = |reloc: &R, sym_option: Option<&Elf32_Sym>| -> Option<RelocInfo<R>> {
                let defined_val = sym_option.map_or(true, |sym| {
                    sym.st_shndx != SHN_UNDEF || ELF32_ST_BIND(sym.st_info) == STB_LOCAL
                });
                match self.isa {
                    Isa::CortexA9 => match reloc.type_info() {
                        R_ARM_REL32 | R_ARM_TARGET2 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(target_word, value)
                            })),
                        }),

                        R_ARM_PREL31 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(
                                    target_word,
                                    (LittleEndian::read_u32(target_word) & 0x80000000)
                                        | value & 0x7FFFFFFF,
                                )
                            })),
                        }),

                        R_ARM_ABS32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: None,
                        }),
                        _ => None,
                    },

                    Isa::RiscV32 => match reloc.type_info() {
                        R_RISCV_CALL_PLT | R_RISCV_GOT_HI20 | R_RISCV_PCREL_HI20 => {
                            Some(RelocInfo {
                                defined_val,
                                indirect_reloc: None,
                                pc_relative: true,
                                relocate: Some(Box::new(|target_word, value| {
                                    let auipc_raw = LittleEndian::read_u32(target_word);
                                    let auipc_insn =
                                        (auipc_raw & 0xFFF) | ((value + 0x800) & 0xFFFFF000);
                                    LittleEndian::write_u32(target_word, auipc_insn)
                                })),
                            })
                        }

                        R_RISCV_32_PCREL => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(target_word, value)
                            })),
                        }),

                        R_RISCV_PCREL_LO12_I => {
                            let expected_offset = sym_option.map_or(0, |sym| sym.st_value);
                            let indirect_reloc = if let Some(reloc) =
                                relocs.iter().find(|reloc| reloc.offset() == expected_offset)
                            {
                                reloc
                            } else {
                                return None;
                            };
                            Some(RelocInfo {
                                defined_val: {
                                    let indirect_sym =
                                        self.symtab[indirect_reloc.sym_info() as usize];
                                    indirect_sym.st_shndx != SHN_UNDEF
                                        || ELF32_ST_BIND(indirect_sym.st_info) == STB_LOCAL
                                },
                                indirect_reloc: Some(indirect_reloc),
                                pc_relative: true,
                                relocate: Some(Box::new(|target_word, value| {
                                    // Here, we convert to direct addressing
                                    // GOT reloc (indirect) -> lw + addi
                                    // PCREL reloc (direct) -> addi
                                    let (lo_opcode, lo_funct3) = (0b0010011, 0b000);
                                    let addi_lw_raw = LittleEndian::read_u32(target_word);
                                    let addi_insn = lo_opcode
                                        | (addi_lw_raw & 0xF8F80)
                                        | (lo_funct3 << 12)
                                        | ((value & 0xFFF) << 20);

                                    LittleEndian::write_u32(target_word, addi_insn)
                                })),
                            })
                        }

                        R_RISCV_32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: None,
                        }),

                        R_RISCV_SET32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(
                                    target_word,
                                    value,
                                )
                            })),
                        }),

                        R_RISCV_ADD32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u32(target_word);
                                LittleEndian::write_u32(target_word, old_value.wrapping_add(value))
                            })),
                        }),

                        R_RISCV_SUB32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u32(target_word);
                                LittleEndian::write_u32(target_word, old_value.wrapping_sub(value))
                            })),
                        }),

                        R_RISCV_SET16 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u16(
                                    target_word,
                                    value as u16,
                                )
                            })),
                        }),

                        R_RISCV_ADD16 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u16(target_word);
                                LittleEndian::write_u16(
                                    target_word,
                                    old_value.wrapping_add(value as u16),
                                )
                            })),
                        }),

                        R_RISCV_SUB16 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u16(target_word);
                                LittleEndian::write_u16(
                                    target_word,
                                    old_value.wrapping_sub(value as u16),
                                )
                            })),
                        }),

                        R_RISCV_SET8 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                target_word[0] = value as u8;
                            })),
                        }),

                        R_RISCV_ADD8 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                target_word[0] = target_word[0].wrapping_add(value as u8);
                            })),
                        }),

                        R_RISCV_SUB8 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                target_word[0] = target_word[0].wrapping_sub(value as u8);
                            })),
                        }),

                        R_RISCV_SET6 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                target_word[0] = (target_word[0] & 0xC0) | ((value & 0x3F) as u8);
                            })),
                        }),

                        R_RISCV_SUB6 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let new_value = (target_word[0].wrapping_sub(value as u8)) & 0x3F;
                                target_word[0] = (target_word[0] & 0xC0) | new_value;
                            })),
                        }),

                        _ => None,
                    },
                }
            };

            let reloc_info =
                classify(reloc, sym).ok_or(Error::Parsing("unsupported relocation"))?;
            let target_index = get_target_section_index()?;
            let target_sec_off = self.elf_shdrs[target_index].shdr.sh_offset;

            if reloc_info.defined_val {
                let (sym_addr, rela_off) = {
                    let (refed_sym, refed_reloc) =
                        if let Some(indirect_reloc) = reloc_info.indirect_reloc {
                            (Some(&self.symtab[indirect_reloc.sym_info() as usize]), indirect_reloc)
                        } else {
                            (sym, reloc)
                        };
                    (resolve_symbol_addr(refed_sym)?, target_sec_off + refed_reloc.offset())
                };

                let target_sec_image = &mut self.elf_shdrs[target_index].data;
                let value = if reloc_info.pc_relative {
                    sym_addr
                        .wrapping_sub(rela_off)
                        .wrapping_add(reloc.addend(target_sec_image) as Elf32_Word)
                } else {
                    sym_addr.wrapping_add(reloc.addend(target_sec_image) as Elf32_Word)
                };

                if let Some(relocate) = reloc_info.relocate {
                    let target_word = &mut target_sec_image[reloc.offset() as usize..];
                    relocate(target_word, value)
                } else {
                    self.rela_dyn_relas.push(Elf32_Rela {
                        r_offset: rela_off,
                        r_info: ELF32_R_INFO(
                            0, // R_ARM_RELATIVE does not have associated symbol
                            match self.isa {
                                Isa::CortexA9 => R_ARM_RELATIVE,
                                Isa::RiscV32 => R_RISCV_RELATIVE,
                            },
                        ),
                        r_addend: value as Elf32_Sword,
                    });
                }
            } else {
                let target_sec_image = &self.elf_shdrs[target_index].data;

                let sym_name = name_starting_at_slice(self.strtab, sym.unwrap().st_name as usize)
                    .map_err(|_| "cannot read symbol name from original .strtab")?;
                let dynsymtab_index = self
                    .get_dynamic_symbol_table()?
                    .find_index_by_name(sym_name)
                    .ok_or("UNDEF relative symbol: cannot find symbol in .dynsym")?;

                self.rela_dyn_relas.push(Elf32_Rela {
                    r_offset: target_sec_off as Elf32_Addr + reloc.offset(),
                    r_info: ELF32_R_INFO(dynsymtab_index as Elf32_Word, reloc.type_info()),
                    r_addend: reloc.addend(target_sec_image),
                });
            }
        }
        Ok(())
    }

    // Fill in the .eh_frame_hdr section
    // Technically it can be done before relocation, but the FDE entries in the
    // eh_frame_hdr section should be sorted. There are no guarantees that those in
    // .eh_frame would be sorted.
    fn implement_eh_frame_hdr(&mut self) -> Result<(), Error> {
        // Fetch .eh_frame & .eh_frame_hdr from the custom section table
        let eh_frame_rec =
            get_section_by_name!(self, ".eh_frame").ok_or("cannot find .eh_frame from .elf")?;
        let eh_frame_hdr_rec = get_section_by_name!(self, ".eh_frame_hdr")
            .ok_or("cannot find .eh_frame_hdr from .elf")?;

        let eh_frame_slice = eh_frame_rec.data.as_slice();
        // Prepare a new buffer to dodge borrow check
        let mut eh_frame_hdr_vec: Vec<u8> = vec![0; eh_frame_hdr_rec.shdr.sh_size as usize];
        let eh_frame = EH_Frame::new(eh_frame_slice, eh_frame_rec.shdr.sh_offset)
            .map_err(|()| "cannot read EH frame")?;
        let mut eh_frame_hdr = EH_Frame_Hdr::new(
            eh_frame_hdr_vec.as_mut_slice(),
            eh_frame_hdr_rec.shdr.sh_offset,
            eh_frame_rec.shdr.sh_offset,
        );
        eh_frame.cfi_records()
            .flat_map(|cfi| cfi.fde_records())
            .for_each(&mut |(init_pos, virt_addr)| eh_frame_hdr.add_fde(init_pos, virt_addr));

        // Sort FDE entries in .eh_frame_hdr
        eh_frame_hdr.finalize_fde();

        // Replace the data buffer in the record
        get_mut_section_by_name!(self, ".eh_frame_hdr")
            .ok_or("cannot find .eh_frame_hdr from .elf")?
            .data = eh_frame_hdr_vec;

        Ok(())
    }

    pub fn ld(data: &'a [u8]) -> Result<Vec<u8>, Error> {
        let ehdr = read_unaligned::<Elf32_Ehdr>(data, 0).map_err(|()| "cannot read ELF header")?;
        let isa = match ehdr.e_machine {
            EM_ARM => Isa::CortexA9,
            EM_RISCV => Isa::RiscV32,
            _ => return Err(Error::Parsing("unsupported architecture")),
        };

        let shdrs = get_ref_slice::<Elf32_Shdr>(data, ehdr.e_shoff as usize, ehdr.e_shnum as usize)
            .map_err(|()| "cannot read section header table")?;

        // Read .strtab
        let strtab_shdr = shdrs[ehdr.e_shstrndx as usize];
        let strtab =
            get_ref_slice::<u8>(data, strtab_shdr.sh_offset as usize, strtab_shdr.sh_size as usize)
                .map_err(|()| "cannot read the string table from data")?;

        // Read .symtab
        let symtab_shdr = shdrs
            .iter()
            .find(|shdr| shdr.sh_type as usize == SHT_SYMTAB)
            .ok_or(Error::Parsing("cannot find the symbol table"))?;
        let symtab = get_ref_slice::<Elf32_Sym>(
            data,
            symtab_shdr.sh_offset as usize,
            symtab_shdr.sh_size as usize / mem::size_of::<Elf32_Sym>(),
        )
        .map_err(|()| "cannot read the symbol table from data")?;

        // Section table for the .elf paired with the section name
        // To be formalized incrementally
        // Very hashmap-like structure, but the order matters, so it is a vector
        let mut elf_shdrs = Vec::new();
        elf_shdrs.push(SectionRecord {
            shdr: Elf32_Shdr {
                sh_name: 0,
                sh_type: 0,
                sh_flags: 0,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: 0,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 0,
                sh_entsize: 0,
            },
            name: "",
            data: vec![0; 0],
        });
        let elf_sh_data_off = mem::size_of::<Elf32_Ehdr>() + mem::size_of::<Elf32_Phdr>() * 5;

        // Image of the linked dynamic library, to be formalized incrementally
        // just as the section table eventually does
        let image: Vec<u8> = vec![0; elf_sh_data_off];

        // Section relocation table
        // A map of the original index of copied sections to the new sections
        let section_map = HashMap::new();

        // Vector of relocation entries in .rela.dyn
        let rela_dyn_relas = Vec::new();

        let mut linker = Linker {
            isa,
            symtab,
            strtab,
            elf_shdrs,
            section_map,
            image,
            load_offset: elf_sh_data_off as u32,
            rela_dyn_relas,
        };

        // Generate .text, keep the section index to find .rela.text
        let is_text_shdr = |shdr: &Elf32_Shdr| {
            shdr.sh_flags as usize & (SHF_ALLOC | SHF_EXECINSTR) == (SHF_ALLOC | SHF_EXECINSTR)
        };
        let is_progbits = |shdr: &Elf32_Shdr| shdr.sh_type as usize == SHT_PROGBITS;

        let text_shdr_index = shdrs
            .iter()
            .position(|shdr| is_text_shdr(shdr) && is_progbits(shdr))
            .ok_or(Error::Parsing("cannot find the .text section"))?;
        let text_shdr = shdrs[text_shdr_index];

        linker.load_section(
            &text_shdr,
            ".text",
            (&data[text_shdr.sh_offset as usize
                ..text_shdr.sh_offset as usize + text_shdr.sh_size as usize])
                .to_vec(),
        );
        linker.section_map.insert(text_shdr_index, 1);

        // ARM: Prioritize the transfer of EXIDX before EXTAB
        // It is to ensure that EXIDX is within a LOAD program header
        // Otherwise, the runtime linker will not copy the index table
        if linker.isa == Isa::CortexA9 {
            let arm_exidx_shdr_index = shdrs
                .iter()
                .position(|shdr| shdr.sh_type as usize == SHT_ARM_EXIDX)
                .ok_or(Error::Parsing("cannot find the .ARM.exidx section"))?;
            let arm_exidx_shdr = shdrs[arm_exidx_shdr_index];

            let loaded_index = linker.load_section(
                &arm_exidx_shdr,
                ".ARM.exidx",
                (&data[arm_exidx_shdr.sh_offset as usize
                    ..arm_exidx_shdr.sh_offset as usize + arm_exidx_shdr.sh_size as usize])
                    .to_vec(),
            );
            linker.section_map.insert(arm_exidx_shdr_index, loaded_index);
        }

        // Prepare all read-only progbits except .eh_frame
        // The executable section is already loaded as .text
        for (i, shdr) in shdrs.iter().enumerate() {
            if shdr.sh_type as usize != SHT_PROGBITS
                || shdr.sh_flags as usize & (SHF_WRITE | SHF_ALLOC | SHF_EXECINSTR) != SHF_ALLOC
            {
                continue;
            }
            let section_name = name_starting_at_slice(strtab, shdr.sh_name as usize)
                .map_err(|_| "cannot read section name")?;
            let elf_shdrs_index = linker.load_section(
                shdr,
                str::from_utf8(section_name).unwrap(),
                (&data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize]).to_vec(),
            );
            linker.section_map.insert(i, elf_shdrs_index);
        }

        // Non-ARM targets use .eh_frame with an additional .eh_frame_hdr to perform
        // exception handling. ARM targets use .ARM.exidx, indicated by the ARM_EXIDX type
        // But the exception handling section would have been loaded beforehand.
        // Therefore, there is nothing to do for CortexA9 target.
        if linker.isa == Isa::RiscV32 {
            // Prepare .eh_frame and give a dummy .eh_frame_hdr
            // The header will be implemented later
            let eh_frame_shdr = shdrs
                .iter()
                .find(|shdr| {
                    name_starting_at_slice(strtab, shdr.sh_name as usize).unwrap() == b".eh_frame"
                })
                .ok_or("cannot find .eh_frame from object")?;

            // For some reason ld.lld would add an zero-entry of CIE at the end of the .eh_frame,
            // which obviously has no FDEs associated to it. That entry should be skippable.
            let eh_frame = &data[eh_frame_shdr.sh_offset as usize
                ..(eh_frame_shdr.sh_offset + eh_frame_shdr.sh_size) as usize];

            // Allocate memory for .eh_frame_hdr
            // Calculate the size by parsing .eh_frame at coarse as possible
            let eh_frame_hdr_size = EH_Frame_Hdr::size_from_eh_frame(eh_frame);

            // Describe the .eh_frame_hdr with a dummy shdr.
            let eh_frame_hdr_shdr = Elf32_Shdr {
                sh_name: 0,
                sh_type: SHT_PROGBITS as Elf32_Word,
                sh_flags: SHF_ALLOC as Elf32_Word,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: eh_frame_hdr_size as Elf32_Word,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 4,
                sh_entsize: 0,
            };
            linker.load_section(&eh_frame_hdr_shdr, ".eh_frame_hdr", vec![0; eh_frame_hdr_size]);
        }

        // Allocate memory for both .rela.dyn
        // The number of entries in .rela.dyn is found by counting relocations that either
        // - use global undefined symbols; or
        // - need the loading address
        let mut rela_dyn_size = 0;
        let mut rela_dyn_sym_indices = Vec::<u32>::new();

        // There are 2 types of relocation entries, RELA & REL.
        // There are essentially no difference in processing their fields.
        macro_rules! reloc_invariant {
            ($shdr: expr, $stmt: expr) => {
                match $shdr.sh_type as usize {
                    SHT_RELA => {
                        let relocs = get_ref_slice::<Elf32_Rela>(
                            data,
                            $shdr.sh_offset as usize,
                            $shdr.sh_size as usize / mem::size_of::<Elf32_Rela>(),
                        )
                        .map_err(|()| "cannot parse relocations")?;
                        $stmt(relocs)
                    }
                    SHT_REL => {
                        let relocs = get_ref_slice::<Elf32_Rel>(
                            data,
                            $shdr.sh_offset as usize,
                            $shdr.sh_size as usize / mem::size_of::<Elf32_Rel>(),
                        )
                        .map_err(|()| "cannot parse relocations")?;
                        $stmt(relocs)
                    }
                    _ => unreachable!(),
                }
            };
        }

        fn allocate_rela_dyn<R: Relocatable>(
            linker: &Linker,
            relocs: &[R],
        ) -> Result<(usize, Vec<u32>), Error> {
            let mut alloc_size = 0;
            let mut rela_dyn_sym_indices = Vec::new();
            for reloc in relocs {
                if reloc.sym_info() as usize == STN_UNDEF {
                    continue;
                }
                let sym: &Elf32_Sym = linker
                    .symtab
                    .get(reloc.sym_info() as usize)
                    .ok_or("symbol out of bounds of symbol table")?;

                match (linker.isa, reloc.type_info()) {
                    // Absolute address relocations
                    // A runtime relocation is needed to find the loading address
                    (Isa::CortexA9, R_ARM_ABS32) | (Isa::RiscV32, R_RISCV_32) => {
                        alloc_size += mem::size_of::<Elf32_Rela>(); // FIXME: RELA vs REL
                        if ELF32_ST_BIND(sym.st_info) == STB_GLOBAL && sym.st_shndx == SHN_UNDEF {
                            rela_dyn_sym_indices.push(reloc.sym_info());
                        }
                    }

                    // Relative address relocations
                    // Relay the relocation to the runtime linker only if the symbol is not defined
                    (Isa::CortexA9, R_ARM_REL32)
                    | (Isa::CortexA9, R_ARM_PREL31)
                    | (Isa::CortexA9, R_ARM_TARGET2)
                    | (Isa::RiscV32, R_RISCV_CALL_PLT)
                    | (Isa::RiscV32, R_RISCV_PCREL_HI20)
                    | (Isa::RiscV32, R_RISCV_GOT_HI20)
                    | (Isa::RiscV32, R_RISCV_32_PCREL)
                    | (Isa::RiscV32, R_RISCV_SET32)
                    | (Isa::RiscV32, R_RISCV_ADD32)
                    | (Isa::RiscV32, R_RISCV_SUB32)
                    | (Isa::RiscV32, R_RISCV_SET16)
                    | (Isa::RiscV32, R_RISCV_ADD16)
                    | (Isa::RiscV32, R_RISCV_SUB16)
                    | (Isa::RiscV32, R_RISCV_SET8)
                    | (Isa::RiscV32, R_RISCV_ADD8)
                    | (Isa::RiscV32, R_RISCV_SUB8)
                    | (Isa::RiscV32, R_RISCV_SET6)
                    | (Isa::RiscV32, R_RISCV_SUB6) => {
                        if ELF32_ST_BIND(sym.st_info) == STB_GLOBAL && sym.st_shndx == SHN_UNDEF {
                            alloc_size += mem::size_of::<Elf32_Rela>(); // FIXME: RELA vs REL
                            rela_dyn_sym_indices.push(reloc.sym_info());
                        }
                    }

                    // RISC-V: Lower 12-bits relocations
                    // If the upper 20-bits relocation cannot be resolved,
                    // this relocation will be relayed to the runtime linker.
                    (Isa::RiscV32, R_RISCV_PCREL_LO12_I) => {
                        // Find the HI20 relocation
                        let indirect_reloc = relocs
                            .iter()
                            .find(|reloc| reloc.offset() == sym.st_value)
                            .ok_or("malformatted LO12 relocation")?;
                        let indirect_sym = linker.symtab[indirect_reloc.sym_info() as usize];
                        if ELF32_ST_BIND(indirect_sym.st_info) == STB_GLOBAL
                            && indirect_sym.st_shndx == SHN_UNDEF
                        {
                            alloc_size += mem::size_of::<Elf32_Rela>(); // FIXME: RELA vs REL
                            rela_dyn_sym_indices.push(reloc.sym_info());
                        }
                    }

                    _ => {
                        println!("Relocation type 0x{:X?} is not supported", reloc.type_info());
                        unimplemented!()
                    }
                }
            }
            Ok((alloc_size, rela_dyn_sym_indices))
        }

        for shdr in shdrs
            .iter()
            .filter(|shdr| shdr.sh_type as usize == SHT_REL || shdr.sh_type as usize == SHT_RELA)
        {
            // If the reloction refers to a section that will not be loaded,
            // do not allocate space for the resulting relocations, it will not be processed
            let referred_shdr = shdrs
                .get(shdr.sh_info as usize)
                .ok_or("relocation is not specified to a valid section number")?;
            if (referred_shdr.sh_flags as usize & SHF_ALLOC) != SHF_ALLOC {
                continue;
            }

            reloc_invariant!(shdr, |relocs| {
                match allocate_rela_dyn(&linker, relocs) {
                    Ok((alloc_size, additional_indices)) => {
                        rela_dyn_size += alloc_size;
                        rela_dyn_sym_indices.extend(additional_indices);
                        Ok(())
                    }

                    Err(e) => Err(e),
                }
            })?;
        }

        // Avoid symbol duplication
        rela_dyn_sym_indices.sort();
        rela_dyn_sym_indices.dedup();

        if rela_dyn_size != 0 {
            let rela_dyn_shdr = Elf32_Shdr {
                sh_name: 0,
                sh_type: SHT_RELA as Elf32_Word,
                sh_flags: SHF_ALLOC as Elf32_Word,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: rela_dyn_size as Elf32_Word,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 4,
                sh_entsize: mem::size_of::<Elf32_Rela>() as Elf32_Word,
            };
            linker.load_section(&rela_dyn_shdr, ".rela.dyn", vec![0; rela_dyn_size]);
        }

        // Construct the .dynsym & .dynstr sections
        // .dynsym section should only contain the symbols needed for .rela.dyn
        let mut dynsym = Vec::new();
        let mut dynstr = Vec::new();
        let mut dynsym_names = Vec::new();
        dynsym.push(Elf32_Sym {
            st_name: 0,
            st_value: 0,
            st_size: 0,
            st_info: 0,
            st_other: 0,
            st_shndx: 0,
        });
        dynstr.push(0);
        dynsym_names.push((0, 0));

        for rela_dyn_sym_index in rela_dyn_sym_indices {
            let mut sym = linker.symtab[rela_dyn_sym_index as usize].clone();
            let sym_name = name_starting_at_slice(strtab, sym.st_name as usize)
                .map_err(|_| "cannot read symbol name from the original .strtab")?;
            let dynstr_start_index = dynstr.len();

            sym.st_name = dynstr_start_index as Elf32_Word;
            if sym.st_shndx != SHN_UNDEF {
                let elf_shdr_index = linker
                    .section_map
                    .get(&(sym.st_shndx as usize))
                    .map(|&index| index)
                    .ok_or(Error::Parsing("Cannot find section with matching sh_index"))?;
                let elf_shdr_offset = linker.elf_shdrs[elf_shdr_index].shdr.sh_offset;
                sym.st_value += elf_shdr_offset;
                // Convert scope of symbols to global
                // All relocation symbols must be visible to the dynamic linker
                sym.st_info = ELF32_ST_INFO(STB_GLOBAL, ELF32_ST_TYPE(sym.st_info));
                sym.st_shndx = elf_shdr_index as Elf32_Section;
            }
            dynsym.push(sym);
            dynstr.extend(sym_name);
            dynstr.push(0);
            dynsym_names.push((dynstr_start_index, dynstr_start_index + sym_name.len()));
        }

        // Copy __modinit__ symbol from object file
        let modinit_sym = symtab
            .iter()
            .find(|sym| {
                let sym_name = name_starting_at_slice(strtab, sym.st_name as usize).unwrap();
                sym_name == b"__modinit__"
            })
            .ok_or("__modinit__ symbol cannot be found")?;

        let modinit_shdr_index = linker
            .section_map
            .get(&(modinit_sym.st_shndx as usize))
            .map(|&index| index)
            .ok_or(Error::Parsing("Cannot find section with matching sh_index"))?;
        let modinit_shdr = linker.elf_shdrs[modinit_shdr_index].shdr;

        let dynstr_start_index = dynstr.len();
        dynsym.push(Elf32_Sym {
            st_name: dynstr_start_index as Elf32_Word,
            st_value: modinit_shdr.sh_offset + modinit_sym.st_value,
            st_size: modinit_sym.st_value,
            st_info: modinit_sym.st_info,
            st_other: modinit_sym.st_other,
            st_shndx: modinit_shdr_index as Elf32_Section,
        });
        let sym_slice = b"__modinit__";
        dynsym_names.push((dynstr.len(), dynstr.len() + sym_slice.len()));
        dynstr.extend(sym_slice);
        dynstr.push(0);

        // Additional symbols
        // st_name will be defined when synthesizing .dynstr
        // st_value & st_shndx will be finalized when .bss sections are processed
        let mut extra_sym_vec = vec![
            Elf32_Sym {
                st_name: 0,
                st_value: 0,
                st_size: 0,
                st_info: ELF32_ST_INFO(STB_GLOBAL, STT_NOTYPE),
                st_other: STV_DEFAULT as u8,
                st_shndx: 0,
            };
            3
        ];

        let sym_slice = b"__bss_start";
        dynsym_names.push((dynstr.len(), dynstr.len() + sym_slice.len()));
        extra_sym_vec[0].st_name = dynstr.len() as Elf32_Word;
        dynstr.extend(b"__bss_start");
        dynstr.push(0);

        let sym_slice = b"_end";
        dynsym_names.push((dynstr.len(), dynstr.len() + sym_slice.len()));
        extra_sym_vec[1].st_name = dynstr.len() as Elf32_Word;
        dynstr.extend(b"_end");
        dynstr.push(0);

        let sym_slice = b"_sstack_guard";
        dynsym_names.push((dynstr.len(), dynstr.len() + sym_slice.len()));
        extra_sym_vec[2].st_name = dynstr.len() as Elf32_Word;
        dynstr.extend(b"_sstack_guard");
        dynstr.push(0);

        dynsym.extend(extra_sym_vec);

        // There should be dynsym.len() buckets & chains
        // No entries could be skipped, even symbols like __modinit__ will be looked up
        let mut hash_bucket: Vec<u32> = vec![0; dynsym.len()];
        let mut hash_chain: Vec<u32> = vec![0; dynsym.len()];

        for sym_index in 1..dynsym.len() {
            let (str_start, str_end) = dynsym_names[sym_index];
            let hash = elf_hash(&dynstr[str_start..str_end]);
            let mut hash_index = hash as usize % hash_bucket.len();

            if hash_bucket[hash_index] == 0 {
                hash_bucket[hash_index] = sym_index as u32;
            } else {
                hash_index = hash_bucket[hash_index] as usize;
                while hash_chain[hash_index] != 0 {
                    hash_index = hash_chain[hash_index] as usize;
                }
                hash_chain[hash_index] = sym_index as u32;
            }
        }

        let mut hash: Vec<u32> = Vec::new();
        hash.push(hash_bucket.len() as u32);
        hash.push(hash_chain.len() as u32);
        hash.extend(hash_bucket);
        hash.extend(hash_chain);

        // Add .dynsym, .dynstr, .hash to the linker
        let dynstr_elf_index = linker.load_section(
            &Elf32_Shdr {
                sh_name: 0,
                sh_type: SHT_STRTAB as Elf32_Word,
                sh_flags: SHF_ALLOC as Elf32_Word,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: dynstr.len() as Elf32_Word,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: 1,
                sh_entsize: 0,
            },
            ".dynstr",
            dynstr,
        );
        let dynsym_elf_index = linker.load_section(
            &Elf32_Shdr {
                sh_name: 0,
                sh_type: SHT_DYNSYM as Elf32_Word,
                sh_flags: SHF_ALLOC as Elf32_Word,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: (dynsym.len() * mem::size_of::<Elf32_Sym>()) as Elf32_Word,
                sh_link: dynstr_elf_index as Elf32_Word, // Index of the .dynstr section, to be inserted
                sh_info: 1,                              // Last local symbol is at index 0 (NOTYPE)
                sh_addralign: mem::size_of::<Elf32_Sym>() as Elf32_Word,
                sh_entsize: mem::size_of::<Elf32_Sym>() as Elf32_Word,
            },
            ".dynsym",
            from_struct_vec(dynsym),
        );
        let hash_elf_index = linker.load_section(
            &Elf32_Shdr {
                sh_name: 0,
                sh_type: SHT_HASH as Elf32_Word,
                sh_flags: SHF_ALLOC as Elf32_Word,
                sh_addr: 0,
                sh_offset: 0,
                sh_size: (hash.len() * 4) as Elf32_Word,
                sh_link: dynsym_elf_index as Elf32_Word, // Index of the .dynsym section
                sh_info: 0,
                sh_addralign: 4,
                sh_entsize: 4,
            },
            ".hash",
            from_struct_vec(hash),
        );

        // Link .rela.dyn header to the .dynsym header
        get_mut_section_by_name!(linker, ".rela.dyn")
            .ok_or(".dynsym not initialized before .dynstr")?
            .shdr
            .sh_link = dynsym_elf_index as Elf32_Word;

        let first_writable_sec_elf_index = linker.elf_shdrs.len();

        // Load writable PROGBITS sections
        for (i, shdr) in shdrs.iter().enumerate() {
            if shdr.sh_type as usize == SHT_PROGBITS
                && shdr.sh_flags as usize & (SHF_WRITE | SHF_ALLOC | SHF_EXECINSTR)
                    == (SHF_WRITE | SHF_ALLOC)
            {
                let section_name = name_starting_at_slice(strtab, shdr.sh_name as usize)
                    .map_err(|_| "failed to load section name")?;
                let elf_shdrs_index = linker.load_section(
                    shdr,
                    str::from_utf8(section_name).unwrap(),
                    (&data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize])
                        .to_vec(),
                );
                linker.section_map.insert(i, elf_shdrs_index);
            }
        }

        // Load the .dynamic section
        // Initialize with mandatory dyn entries
        let mut dyn_entries = vec![
            Elf32_Dyn {
                d_tag: DT_HASH,
                d_un: Elf32_Dyn__bindgen_ty_1 {
                    d_ptr: linker.elf_shdrs[hash_elf_index].shdr.sh_offset,
                },
            },
            Elf32_Dyn {
                d_tag: DT_STRTAB,
                d_un: Elf32_Dyn__bindgen_ty_1 {
                    d_ptr: linker.elf_shdrs[dynstr_elf_index].shdr.sh_offset,
                },
            },
            Elf32_Dyn {
                d_tag: DT_SYMTAB,
                d_un: Elf32_Dyn__bindgen_ty_1 {
                    d_ptr: linker.elf_shdrs[dynsym_elf_index].shdr.sh_offset,
                },
            },
            Elf32_Dyn {
                d_tag: DT_STRSZ,
                d_un: Elf32_Dyn__bindgen_ty_1 {
                    d_val: linker.elf_shdrs[dynstr_elf_index].shdr.sh_size,
                },
            },
            Elf32_Dyn {
                d_tag: DT_SYMENT,
                d_un: Elf32_Dyn__bindgen_ty_1 {
                    d_val: linker.elf_shdrs[dynsym_elf_index].shdr.sh_entsize,
                },
            },
        ];

        if rela_dyn_size != 0 {
            let rela_dyn_shdr = get_section_by_name!(linker, ".rela.dyn")
                .ok_or(".rela.dyn header not properly initialised")?
                .shdr;
            dyn_entries.push(Elf32_Dyn {
                d_tag: DT_RELA,
                d_un: Elf32_Dyn__bindgen_ty_1 { d_ptr: rela_dyn_shdr.sh_offset },
            });
            dyn_entries.push(Elf32_Dyn {
                d_tag: DT_RELASZ,
                d_un: Elf32_Dyn__bindgen_ty_1 { d_ptr: rela_dyn_shdr.sh_size },
            });
            dyn_entries.push(Elf32_Dyn {
                d_tag: DT_RELAENT,
                d_un: Elf32_Dyn__bindgen_ty_1 { d_ptr: rela_dyn_shdr.sh_entsize },
            });
        }

        // Termination entry in .dynamic
        dyn_entries.push(Elf32_Dyn { d_tag: DT_NULL, d_un: Elf32_Dyn__bindgen_ty_1 { d_val: 0 } });

        let dynamic_shdr = Elf32_Shdr {
            sh_name: 0,
            sh_type: SHT_DYNAMIC as Elf32_Word,
            sh_flags: (SHF_WRITE | SHF_ALLOC) as Elf32_Word,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: (dyn_entries.len() * mem::size_of::<Elf32_Dyn>()) as Elf32_Word,
            sh_link: dynstr_elf_index as Elf32_Word,
            sh_info: 0,
            sh_addralign: 4,
            sh_entsize: mem::size_of::<Elf32_Dyn>() as Elf32_Word,
        };

        let dynamic_elf_index =
            linker.load_section(&dynamic_shdr, ".dynamic", from_struct_vec(dyn_entries));

        let last_w_sec_elf_index = linker.elf_shdrs.len() - 1;

        // Load all other A-flag non-PROGBITS sections (ARM: non-ARM_EXIDX as well)
        // .bss sections (i.e. .sbss, .sbss.*, .bss & .bss.*) will be loaded later
        let mut bss_index_vec = Vec::new();

        for (i, shdr) in shdrs.iter().enumerate() {
            if (shdr.sh_type as usize != SHT_PROGBITS)
                && (shdr.sh_type as usize != SHT_ARM_EXIDX)
                && ((shdr.sh_flags as usize & SHF_ALLOC) == SHF_ALLOC)
            {
                let section_name_slice = name_starting_at_slice(strtab, shdr.sh_name as usize)
                    .map_err(|_| "failed to load section name")?;
                let section_name =
                    str::from_utf8(section_name_slice).map_err(|_| "cannot parse section name")?;
                if section_name == ".bss"
                    || section_name == ".sbss"
                    || section_name.starts_with(".bss.")
                    || section_name.starts_with(".sbss.")
                {
                    bss_index_vec.push((i, section_name));
                } else {
                    let elf_shdrs_index = linker.load_section(
                        shdr,
                        section_name,
                        (&data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize])
                            .to_vec(),
                    );
                    linker.section_map.insert(i, elf_shdrs_index);
                }
            }
        }

        macro_rules! update_dynsym_record {
            ($sym_name: expr, $st_value: expr, $st_shndx: expr) => {
                let symbol_table = linker.get_dynamic_symbol_table()?;
                let bss_start_sym_index = symbol_table
                    .find_index_by_name($sym_name)
                    .ok_or(stringify!($sym_name symbol not initialized))?;
                let dynsyms = to_struct_mut_slice::<Elf32_Sym>(
                    get_mut_section_by_name!(linker, ".dynsym")
                        .ok_or("cannot make retrieve .dynsym")?
                        .data
                        .as_mut_slice(),
                );
                dynsyms[bss_start_sym_index].st_value = $st_value;
                dynsyms[bss_start_sym_index].st_shndx = $st_shndx;
            }
        }

        // Load the .bss sections, finalize the .bss symbols
        if bss_index_vec.is_empty() {
            // Insert a zero-size .bss section if there aren't any
            let bss_elf_index = linker.load_section(
                &Elf32_Shdr {
                    sh_name: 0,
                    sh_type: SHT_NOBITS as Elf32_Word,
                    sh_flags: (SHF_ALLOC | SHF_WRITE) as Elf32_Word,
                    sh_addr: 0,
                    sh_offset: 0,
                    sh_size: 0,
                    sh_link: 0,
                    sh_info: 0,
                    sh_addralign: 4,
                    sh_entsize: 0,
                },
                ".bss",
                vec![0; 0],
            );
            let bss_offset = linker.elf_shdrs[bss_elf_index].shdr.sh_offset;

            update_dynsym_record!(b"__bss_start", bss_offset, bss_elf_index as Elf32_Section);
            update_dynsym_record!(b"_end", bss_offset, bss_elf_index as Elf32_Section);
        } else {
            for (bss_iter_index, &(bss_section_index, section_name)) in bss_index_vec.iter().enumerate() {
                let shdr = &shdrs[bss_section_index];
                let bss_elf_index = linker.load_section(
                    shdr,
                    section_name,
                    (&data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize])
                        .to_vec(),
                );
                linker.section_map.insert(bss_section_index, bss_elf_index);

                let loaded_shdr = linker.elf_shdrs[bss_elf_index].shdr;

                if bss_iter_index == 0 {
                    update_dynsym_record!(
                        b"__bss_start",
                        loaded_shdr.sh_offset,
                        bss_elf_index as Elf32_Section
                    );
                }

                if bss_iter_index == bss_index_vec.len() - 1 {
                    update_dynsym_record!(
                        b"_end",
                        loaded_shdr.sh_offset + loaded_shdr.sh_size,
                        bss_elf_index as Elf32_Section
                    );
                }
            }
        }

        // All sections that should be allocated memory are loaded
        // The stack guard address can be determined
        let last_elf_shdr_index = linker.elf_shdrs.len() - 1;
        let last_load_shdr = linker.elf_shdrs[last_elf_shdr_index].shdr;
        let end_load_addr = last_load_shdr.sh_offset + last_load_shdr.sh_size;
        let stack_guard_addr = end_load_addr + ((0x1000 - (end_load_addr % 0x1000)) % 0x1000);
        update_dynsym_record!(
            b"_sstack_guard",
            stack_guard_addr,
            last_elf_shdr_index as Elf32_Section
        );

        for shdr in shdrs
            .iter()
            .filter(|shdr| shdr.sh_type as usize == SHT_RELA || shdr.sh_type as usize == SHT_REL)
        {
            // If the reloction refers to a section that will not be loaded,
            // do not process the relocations. The section will not be loaded
            let referred_shdr = shdrs
                .get(shdr.sh_info as usize)
                .ok_or("relocation is not specified to a valid section number")?;
            if (referred_shdr.sh_flags as usize & SHF_ALLOC) != SHF_ALLOC {
                continue;
            }

            reloc_invariant!(shdr, |relocs| linker.resolve_relocatables(relocs, shdr.sh_info))?;
        }

        // Load .rela.dyn symbols generated during relocation
        if rela_dyn_size != 0 {
            let rela_dyn_rec = get_mut_section_by_name!(linker, ".rela.dyn")
                .ok_or(".rela.dyn not initialized in the ELF file")?;
            let rela_dyn_slice =
                to_struct_mut_slice::<Elf32_Rela>(rela_dyn_rec.data.as_mut_slice());

            for (i, &rela) in linker.rela_dyn_relas.iter().enumerate() {
                rela_dyn_slice[i] = rela;
            }
        }

        // Prepare a STRTAB to hold the names of section headers
        // Fix the sh_name field of the section headers
        let mut shstrtab = Vec::new();
        for shdr_rec in linker.elf_shdrs.iter_mut() {
            let shstrtab_index = shstrtab.len();
            shstrtab.extend(shdr_rec.name.as_bytes());
            shstrtab.push(0);
            shdr_rec.shdr.sh_name = shstrtab_index as Elf32_Word;
        }
        // Add en entry for .shstrtab
        let shstrtab_shdr_sh_name = shstrtab.len();
        shstrtab.extend(b".shstrtab");
        shstrtab.push(0);

        let shstrtab_shdr = Elf32_Shdr {
            sh_name: shstrtab_shdr_sh_name as Elf32_Word,
            sh_type: SHT_STRTAB as Elf32_Word,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: shstrtab.len() as Elf32_Word,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        };

        let shstrtab_elf_index = linker.load_section(&shstrtab_shdr, ".shstrtab", shstrtab);

        // Edit .eh_frame_hdr content
        if linker.isa == Isa::RiscV32 {
            linker.implement_eh_frame_hdr()?;
        }

        // Load all section data into the image
        for rec in &linker.elf_shdrs[1..] {
            linker.image.extend(vec![0; (rec.shdr.sh_offset as usize) - linker.image.len()]);
            linker.image.extend(&rec.data);
        }

        // Load all section headers to the image
        let alignment = (4 - (linker.image.len() % 4)) % 4;
        let sec_headers_offset = linker.image.len() + alignment;
        linker.image.extend(vec![0; alignment]);
        for rec in linker.elf_shdrs.iter() {
            let shdr = rec.shdr;
            linker.image.extend(unsafe {
                slice::from_raw_parts(
                    &shdr as *const Elf32_Shdr as *const u8,
                    mem::size_of::<Elf32_Shdr>(),
                )
            });
        }

        // Update the PHDRs
        let phdr_offset = mem::size_of::<Elf32_Ehdr>();
        unsafe {
            let phdr_ptr = linker.image.as_mut_ptr().add(phdr_offset) as *mut Elf32_Phdr;
            let phdr_slice = slice::from_raw_parts_mut(phdr_ptr, 5);
            // List of program headers:
            // 1. ELF headers & program headers
            // 2. Read-only sections
            // 3. All other A-flag sections
            // 4. Dynamic
            // 5. EH frame & its header
            let header_size = mem::size_of::<Elf32_Ehdr>() + mem::size_of::<Elf32_Phdr>() * 5;
            phdr_slice[0] = Elf32_Phdr {
                p_type: PT_LOAD,
                p_offset: 0,
                p_vaddr: 0,
                p_paddr: 0,
                p_filesz: header_size as Elf32_Word,
                p_memsz: header_size as Elf32_Word,
                p_flags: PF_R as Elf32_Word,
                p_align: 0x1000,
            };
            let last_ro_shdr = linker.elf_shdrs[first_writable_sec_elf_index - 1].shdr;
            let last_ro_addr = last_ro_shdr.sh_offset + last_ro_shdr.sh_size;
            let ro_load_size = last_ro_addr - header_size as Elf32_Word;
            phdr_slice[1] = Elf32_Phdr {
                p_type: PT_LOAD,
                p_offset: header_size as Elf32_Off,
                p_vaddr: header_size as Elf32_Addr,
                p_paddr: header_size as Elf32_Addr,
                p_filesz: ro_load_size,
                p_memsz: ro_load_size,
                p_flags: (PF_R | PF_X) as Elf32_Word,
                p_align: 0x1000,
            };
            let first_w_shdr = linker.elf_shdrs[first_writable_sec_elf_index].shdr;
            let first_w_addr = first_w_shdr.sh_offset;
            let last_w_shdr = linker.elf_shdrs[last_w_sec_elf_index].shdr;
            let w_size = last_w_shdr.sh_offset + last_w_shdr.sh_size - first_w_addr;
            phdr_slice[2] = Elf32_Phdr {
                p_type: PT_LOAD,
                p_offset: first_w_addr as Elf32_Off,
                p_vaddr: first_w_addr as Elf32_Addr,
                p_paddr: first_w_addr as Elf32_Addr,
                p_filesz: w_size,
                p_memsz: w_size,
                p_flags: (PF_R | PF_W) as Elf32_Word,
                p_align: 0x1000,
            };
            let dynamic_shdr = linker.elf_shdrs[dynamic_elf_index].shdr;
            phdr_slice[3] = Elf32_Phdr {
                p_type: PT_DYNAMIC,
                p_offset: dynamic_shdr.sh_offset,
                p_vaddr: dynamic_shdr.sh_offset,
                p_paddr: dynamic_shdr.sh_offset,
                p_filesz: dynamic_shdr.sh_size,
                p_memsz: dynamic_shdr.sh_size,
                p_flags: (PF_R | PF_W) as Elf32_Word,
                p_align: 4,
            };
            let (eh_type, eh_shdr_name) = match linker.isa {
                Isa::CortexA9 => (PT_ARM_EXIDX, ".ARM.exidx"),
                Isa::RiscV32 => (PT_GNU_EH_FRAME, ".eh_frame_hdr"),
            };
            let eh_shdr = get_section_by_name!(linker, eh_shdr_name)
                .ok_or("cannot read error handling section when finalizing phdrs")?
                .shdr;
            phdr_slice[4] = Elf32_Phdr {
                p_type: eh_type,
                p_offset: eh_shdr.sh_offset,
                p_vaddr: eh_shdr.sh_offset,
                p_paddr: eh_shdr.sh_offset,
                p_filesz: eh_shdr.sh_size,
                p_memsz: eh_shdr.sh_size,
                p_flags: PF_R as Elf32_Word,
                p_align: 4,
            };
        }

        // Update the EHDR
        let ehdr_ptr = linker.image.as_mut_ptr() as *mut Elf32_Ehdr;
        unsafe {
            (*ehdr_ptr) = Elf32_Ehdr {
                e_ident: ehdr.e_ident,
                e_type: ET_DYN,
                e_machine: ehdr.e_machine,
                e_version: ehdr.e_version,
                e_entry: elf_sh_data_off as Elf32_Addr,
                e_phoff: phdr_offset as Elf32_Off,
                e_shoff: sec_headers_offset as Elf32_Off,
                e_flags: match linker.isa {
                    Isa::RiscV32 => ehdr.e_flags,
                    Isa::CortexA9 => ehdr.e_flags | EF_ARM_ABI_FLOAT_HARD as Elf32_Word,
                },
                e_ehsize: mem::size_of::<Elf32_Ehdr>() as Elf32_Half,
                e_phentsize: mem::size_of::<Elf32_Phdr>() as Elf32_Half,
                e_phnum: 5,
                e_shentsize: mem::size_of::<Elf32_Shdr>() as Elf32_Half,
                e_shnum: linker.elf_shdrs.len() as Elf32_Half,
                e_shstrndx: shstrtab_elf_index as Elf32_Half,
            }
        }

        Ok(linker.image)
    }
}
