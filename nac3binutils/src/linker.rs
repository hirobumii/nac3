use std::{collections::HashMap, mem, ptr, slice, str};

use byteorder::{ByteOrder, LittleEndian};

use crate::{
    dwarf::{EH_Frame, EH_Frame_Hdr},
    include::elf::*,
};

#[derive(PartialEq, Eq, Clone, Copy)]
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
    fn from(desc: &'static str) -> Self {
        Self::Parsing(desc)
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

// NONE type relocations. It is 0 across all targets.
//
// LLVM generates such dummy relocations with the symbol
// `__aeabi_unwind_cpp_pr0` in exception-throwing code.
const R_TYPE_NONE: u8 = 0;

// Number of program header
const ELF_PHNUM: usize = 5;
const DEBUG_PHNUM: usize = 0; // Debug image is not loadable

struct SectionRecord<'a> {
    shdr: Elf32_Shdr,
    name: &'a str,
    data: Vec<u8>,
}

const fn read_unaligned<T: Copy>(data: &[u8], offset: usize) -> Option<T> {
    if data.len() < offset + mem::size_of::<T>() {
        None
    } else {
        let ptr = data.as_ptr().wrapping_add(offset).cast();
        Some(unsafe { ptr::read_unaligned(ptr) })
    }
}

#[must_use]
pub const fn get_ref_slice<T: Copy>(data: &[u8], offset: usize, len: usize) -> Option<&[T]> {
    if data.len() < offset + mem::size_of::<T>() * len {
        None
    } else {
        let ptr = data.as_ptr().wrapping_add(offset).cast();
        Some(unsafe { slice::from_raw_parts(ptr, len) })
    }
}

fn from_struct_slice<T>(struct_vec: &[T]) -> Vec<u8> {
    let ptr = struct_vec.as_ptr();
    unsafe { slice::from_raw_parts(ptr.cast(), mem::size_of_val(struct_vec)) }.to_vec()
}

const fn to_struct_slice<T>(bytes: &[u8]) -> &[T] {
    unsafe { slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len() / mem::size_of::<T>()) }
}

const fn to_struct_mut_slice<T>(bytes: &mut [u8]) -> &mut [T] {
    unsafe {
        slice::from_raw_parts_mut(bytes.as_mut_ptr().cast(), bytes.len() / mem::size_of::<T>())
    }
}

fn elf_hash(name: &[u8]) -> u32 {
    let mut h: u32 = 0;
    for c in name {
        h = (h << 4) + u32::from(*c);
        let g = h & 0xf000_0000;
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

impl SymbolTableReader<'_> {
    pub fn find_index_by_name(&self, sym_name: &[u8]) -> Option<usize> {
        self.symtab.iter().position(|sym| {
            name_starting_at_slice(self.strtab, sym.st_name as usize)
                .is_ok_and(|dynsym_name| sym_name == dynsym_name)
        })
    }
}

struct Image {
    data: mem::MaybeUninit<Vec<u8>>,
    load_offset: u32,
    shdr_offset: u32,
}

impl Image {
    pub const fn register<'a>(
        &mut self,
        shdr: &Elf32_Shdr,
        sh_name_str: &'a str,
        data: Vec<u8>,
    ) -> SectionRecord<'a> {
        let mut elf_shdr = *shdr;

        // Maintain alignment requirement specified in sh_addralign
        let align = shdr.sh_addralign;
        let load_padding = (align - (self.load_offset % align)) % align;
        let image_padding = (align - (self.shdr_offset % align)) % align;

        let section_load_offset = if (shdr.sh_flags as usize & SHF_ALLOC) == SHF_ALLOC {
            self.load_offset + load_padding
        } else {
            0
        };
        let section_image_offset = self.shdr_offset + image_padding;

        elf_shdr.sh_addr = section_load_offset;
        elf_shdr.sh_offset = section_image_offset;

        if (shdr.sh_flags as usize & SHF_ALLOC) == SHF_ALLOC {
            self.load_offset = section_load_offset + shdr.sh_size;
        }
        if shdr.sh_type as usize != SHT_NOBITS {
            self.shdr_offset = section_image_offset + shdr.sh_size;
        }

        SectionRecord { shdr: elf_shdr, name: sh_name_str, data }
    }

    pub fn finalize(&mut self, shdr_recs: &[SectionRecord]) {
        let header_alignment = 4;

        let mut final_len = self.shdr_offset;
        final_len += (header_alignment - (final_len % header_alignment)) % header_alignment;
        self.shdr_offset = final_len;
        let final_len = final_len as usize + shdr_recs.len() * mem::size_of::<Elf32_Shdr>();

        self.data.write({
            // This leaks the memory, but ...
            let mut temp_vec: mem::ManuallyDrop<Vec<u32>> =
                mem::ManuallyDrop::new(Vec::with_capacity(final_len / header_alignment as usize));
            // This new vector picks up the memory.
            // The new vector is responsible to free the memory instead.
            unsafe { Vec::from_raw_parts(temp_vec.as_mut_ptr().cast(), final_len, final_len) }
        });
        let owned_buffer = unsafe { self.data.assume_init_mut() };

        let mut shdr_ptr =
            unsafe { owned_buffer.as_mut_ptr().add(self.shdr_offset as usize).cast() };

        for shdr_rec in shdr_recs {
            if shdr_rec.shdr.sh_type as usize != SHT_NOBITS {
                owned_buffer[shdr_rec.shdr.sh_offset as usize..][..shdr_rec.data.len()]
                    .clone_from_slice(&shdr_rec.data);
            }

            unsafe {
                *shdr_ptr = shdr_rec.shdr;
                shdr_ptr = shdr_ptr.add(1);
            }
        }
    }

    pub fn get_mut_ref<T>(&mut self, offset: usize, len: usize) -> &mut [T] {
        unsafe {
            let borrowed_buf = self.data.assume_init_mut();
            assert!(borrowed_buf.len() >= offset + len, "out of bound access to image buffer");
            slice::from_raw_parts_mut(borrowed_buf.as_mut_ptr().add(offset).cast(), len)
        }
    }

    pub const fn take(self) -> Vec<u8> {
        unsafe { self.data.assume_init() }
    }
}

pub struct Linker<'a> {
    isa: Isa,
    symtab: &'a [Elf32_Sym],
    strtab: &'a [u8],
    elf_shdrs: Vec<SectionRecord<'a>>,
    debug_shdrs: Vec<SectionRecord<'a>>,
    section_map: HashMap<usize, usize>,
    debug_section_map: HashMap<usize, usize>,
    dyn_lib_image: Image,
    debug_image: Image,
    rela_dyn_relas: Vec<Elf32_Rela>,
}

impl<'a> Linker<'a> {
    fn get_dynamic_symbol_table(&self) -> Result<SymbolTableReader<'_>, Error> {
        let dynsym_rec = get_section_by_name!(self, ".dynsym")
            .ok_or("cannot make SymbolTableReader using .dynsym")?;
        Ok(SymbolTableReader {
            symtab: to_struct_slice::<Elf32_Sym>(dynsym_rec.data.as_slice()),
            strtab: self.elf_shdrs[dynsym_rec.shdr.sh_link as usize].data.as_slice(),
        })
    }

    fn load_section(&mut self, shdr: &Elf32_Shdr, sh_name_str: &'a str, data: Vec<u8>) -> usize {
        self.elf_shdrs.push(self.dyn_lib_image.register(shdr, sh_name_str, data));
        self.elf_shdrs.len() - 1
    }

    fn load_debug_section(
        &mut self,
        shdr: &Elf32_Shdr,
        sh_name_str: &'a str,
        data: Vec<u8>,
    ) -> usize {
        self.debug_shdrs.push(self.debug_image.register(shdr, sh_name_str, data));
        self.debug_shdrs.len() - 1
    }

    // Perform relocation according to the relocation entries
    // Only symbols that support relative addressing would be resolved
    // This is because the loading address is not known yet
    fn resolve_relocatables<R: Relocatable + std::fmt::Debug>(
        &mut self,
        relocs: &[R],
        target_section: Elf32_Word,
    ) -> Result<(), Error> {
        type RelocateFn = fn(&mut [u8], Elf32_Word);

        struct RelocInfo<'a, R> {
            pub defined_val: bool,
            pub indirect_reloc: Option<&'a R>,
            pub pc_relative: bool,
            pub relocate: Option<Box<RelocateFn>>,
        }

        macro_rules! get_referred_section {
            ($target_section: ident, $return_cb: expr) => {
                if let Some(shdr_index) = self.section_map.get(&($target_section as usize)) {
                    Ok($return_cb(true, &self.elf_shdrs[*shdr_index], *shdr_index))
                } else if let Some(debug_index) =
                    self.debug_section_map.get(&($target_section as usize))
                {
                    Ok($return_cb(false, &self.debug_shdrs[*debug_index], *debug_index))
                } else {
                    Err(Error::Parsing("Cannot find section with matching sh_index"))
                }
            };
        }

        let (loaded, target_index) =
            get_referred_section!(target_section, |loaded, _, idx| (loaded, idx)).unwrap();

        macro_rules! get_shdr_attr {
            ($index: ident, $attr: ident) => {
                if loaded { &self.elf_shdrs[$index].$attr } else { &self.debug_shdrs[$index].$attr }
            };
        }

        macro_rules! get_mut_shdr_attr {
            ($index: ident, $attr: ident) => {
                if loaded {
                    &mut self.elf_shdrs[$index].$attr
                } else {
                    &mut self.debug_shdrs[$index].$attr
                }
            };
        }

        let target_section_alloc =
            get_shdr_attr!(target_index, shdr).sh_flags as usize & SHF_ALLOC == SHF_ALLOC;

        for reloc in relocs {
            if reloc.type_info() == R_TYPE_NONE {
                continue;
            }

            let sym = match reloc.sym_info() as usize {
                STN_UNDEF => None,
                sym_index => {
                    Some(self.symtab.get(sym_index).ok_or("symbol out of bounds of symbol table")?)
                }
            };

            let resolve_symbol_addr =
                |sym_option: Option<&Elf32_Sym>| -> Result<Elf32_Word, Error> {
                    let Some(sym) = sym_option else { return Ok(0) };

                    match sym.st_shndx {
                        SHN_UNDEF => Err(Error::Lookup("undefined symbol")),
                        SHN_ABS => Ok(sym.st_value),
                        // Section index may refer to either a debug section or a loaded section
                        // A debug relocation can still refer to a loaded section for symbol resolution
                        sec_ind => get_referred_section!(sec_ind, |_, rec: &SectionRecord, _| rec
                            .shdr
                            .sh_addr
                            as Elf32_Word
                            + sym.st_value),
                    }
                };

            let classify = |reloc: &R, sym_option: Option<&Elf32_Sym>| -> Option<RelocInfo<R>> {
                let defined_val = sym_option.is_none_or(|sym| {
                    sym.st_shndx != SHN_UNDEF || ELF32_ST_BIND(sym.st_info) == STB_LOCAL
                });
                match self.isa {
                    Isa::CortexA9 => match reloc.type_info() {
                        R_ARM_REL32 | R_ARM_TARGET2 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(target_word, value);
                            })),
                        }),

                        R_ARM_PREL31 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(
                                    target_word,
                                    (LittleEndian::read_u32(target_word) & 0x8000_0000)
                                        | value & 0x7FFF_FFFF,
                                );
                            })),
                        }),

                        R_ARM_ABS32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: if target_section_alloc {
                                None
                            } else {
                                Some(Box::new(|target_word, value| {
                                    LittleEndian::write_u32(target_word, value);
                                }))
                            },
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
                                        (auipc_raw & 0xFFF) | ((value + 0x800) & 0xFFFF_F000);
                                    LittleEndian::write_u32(target_word, auipc_insn);
                                })),
                            })
                        }

                        R_RISCV_32_PCREL => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: true,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(target_word, value);
                            })),
                        }),

                        R_RISCV_PCREL_LO12_I => {
                            let expected_offset = sym_option.map_or(0, |sym| sym.st_value);
                            let indirect_reloc =
                                relocs.iter().find(|reloc| reloc.offset() == expected_offset)?;
                            let indir_type_info = indirect_reloc.type_info();
                            let indirect_addressing = (indir_type_info == R_RISCV_CALL_PLT)
                                || (indir_type_info == R_RISCV_GOT_HI20);
                            let relocate = Some(Box::new(if indirect_addressing {
                                |target_word: &mut [u8], value: u32| {
                                    // Here, we convert to direct addressing
                                    // GOT reloc (indirect) -> lw + addi
                                    // PCREL reloc (direct) -> addi
                                    let (lo_opcode, lo_funct3) = (0b001_0011, 0b000);
                                    let addi_lw_raw = LittleEndian::read_u32(target_word);
                                    let addi_insn = lo_opcode
                                        | (addi_lw_raw & 0xF8F80)
                                        | (lo_funct3 << 12)
                                        | ((value & 0xFFF) << 20);

                                    LittleEndian::write_u32(target_word, addi_insn);
                                }
                            } else {
                                |target_word: &mut [u8], value: u32| {
                                    let i_raw = LittleEndian::read_u32(target_word);
                                    let i_insn = (i_raw & 0xFFFFF) | ((value & 0xFFF) << 20);
                                    LittleEndian::write_u32(target_word, i_insn);
                                }
                            }));
                            Some(RelocInfo {
                                defined_val: {
                                    let indirect_sym =
                                        self.symtab[indirect_reloc.sym_info() as usize];
                                    indirect_sym.st_shndx != SHN_UNDEF
                                        || ELF32_ST_BIND(indirect_sym.st_info) == STB_LOCAL
                                },
                                indirect_reloc: Some(indirect_reloc),
                                pc_relative: true,
                                relocate,
                            })
                        }

                        R_RISCV_PCREL_LO12_S => {
                            let expected_offset = sym_option.map_or(0, |sym| sym.st_value);
                            let indirect_reloc =
                                relocs.iter().find(|reloc| reloc.offset() == expected_offset)?;
                            Some(RelocInfo {
                                defined_val: {
                                    let indirect_sym =
                                        self.symtab[indirect_reloc.sym_info() as usize];
                                    indirect_sym.st_shndx != SHN_UNDEF
                                        || ELF32_ST_BIND(indirect_sym.st_info) == STB_LOCAL
                                },
                                indirect_reloc: Some(indirect_reloc),
                                pc_relative: true,
                                relocate: Some(Box::new(|target_word: &mut [u8], value: u32| {
                                    let store_raw = LittleEndian::read_u32(target_word);
                                    let store_insn = ((value & 0x1F) << 7)
                                        | ((value & 0xFE0) << 20)
                                        | (store_raw & 0x01FF_F07F);
                                    LittleEndian::write_u32(target_word, store_insn);
                                })),
                            })
                        }

                        R_RISCV_32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: if target_section_alloc {
                                None
                            } else {
                                Some(Box::new(|target_word, value| {
                                    LittleEndian::write_u32(target_word, value);
                                }))
                            },
                        }),

                        R_RISCV_SET32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u32(target_word, value);
                            })),
                        }),

                        R_RISCV_ADD32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u32(target_word);
                                LittleEndian::write_u32(target_word, old_value.wrapping_add(value));
                            })),
                        }),

                        R_RISCV_SUB32 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                let old_value = LittleEndian::read_u32(target_word);
                                LittleEndian::write_u32(target_word, old_value.wrapping_sub(value));
                            })),
                        }),

                        R_RISCV_SET16 => Some(RelocInfo {
                            defined_val,
                            indirect_reloc: None,
                            pc_relative: false,
                            relocate: Some(Box::new(|target_word, value| {
                                LittleEndian::write_u16(target_word, value as u16);
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
                                );
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
                                );
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
            let target_sec_off = get_shdr_attr!(target_index, shdr).sh_offset;

            if reloc_info.defined_val {
                let (refed_sym, refed_reloc) =
                    if let Some(indirect_reloc) = reloc_info.indirect_reloc {
                        (Some(&self.symtab[indirect_reloc.sym_info() as usize]), indirect_reloc)
                    } else {
                        (sym, reloc)
                    };
                let sym_addr = resolve_symbol_addr(refed_sym)?;
                let rela_off = target_sec_off + refed_reloc.offset();

                let target_sec_image = get_mut_shdr_attr!(target_index, data);
                let mut value =
                    sym_addr.wrapping_add(refed_reloc.addend(target_sec_image) as Elf32_Word);
                if reloc_info.pc_relative {
                    value = value.wrapping_sub(rela_off);
                }

                if let Some(relocate) = reloc_info.relocate {
                    let target_word = &mut target_sec_image[reloc.offset() as usize..];
                    relocate(target_word, value);
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
                let target_sec_image = &get_shdr_attr!(target_index, data);

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
        let eh_frame = EH_Frame::new(eh_frame_slice, eh_frame_rec.shdr.sh_offset);
        let mut eh_frame_hdr = EH_Frame_Hdr::new(
            eh_frame_hdr_vec.as_mut_slice(),
            eh_frame_hdr_rec.shdr.sh_offset,
            eh_frame_rec.shdr.sh_offset,
        );
        eh_frame.cfi_records().flat_map(|cfi| cfi.fde_records()).for_each(&mut |(
            init_pos,
            virt_addr,
        )| {
            eh_frame_hdr.add_fde(init_pos, virt_addr);
        });

        // Sort FDE entries in .eh_frame_hdr
        eh_frame_hdr.finalize_fde();

        // Replace the data buffer in the record
        get_mut_section_by_name!(self, ".eh_frame_hdr")
            .ok_or("cannot find .eh_frame_hdr from .elf")?
            .data = eh_frame_hdr_vec;

        Ok(())
    }

    pub fn ld(data: &'a [u8]) -> Result<(Vec<u8>, Vec<u8>), Error> {
        fn allocate_rela_dyn<R: Relocatable>(
            linker: &Linker,
            relocs: &[R],
        ) -> Result<(usize, Vec<u32>), Error> {
            let mut alloc_size = 0;
            let mut rela_dyn_sym_indices = Vec::new();
            for reloc in relocs {
                if reloc.type_info() == R_TYPE_NONE {
                    continue;
                }
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
                    (Isa::CortexA9, R_ARM_REL32 | R_ARM_PREL31 | R_ARM_TARGET2)
                    | (
                        Isa::RiscV32,
                        R_RISCV_CALL_PLT | R_RISCV_PCREL_HI20 | R_RISCV_GOT_HI20 | R_RISCV_32_PCREL
                        | R_RISCV_SET32 | R_RISCV_ADD32 | R_RISCV_SUB32 | R_RISCV_SET16
                        | R_RISCV_ADD16 | R_RISCV_SUB16 | R_RISCV_SET8 | R_RISCV_ADD8
                        | R_RISCV_SUB8 | R_RISCV_SET6 | R_RISCV_SUB6,
                    ) => {
                        if ELF32_ST_BIND(sym.st_info) == STB_GLOBAL && sym.st_shndx == SHN_UNDEF {
                            alloc_size += mem::size_of::<Elf32_Rela>(); // FIXME: RELA vs REL
                            rela_dyn_sym_indices.push(reloc.sym_info());
                        }
                    }

                    // RISC-V: Lower 12-bits relocations
                    // If the upper 20-bits relocation cannot be resolved,
                    // this relocation will be relayed to the runtime linker.
                    (Isa::RiscV32, R_RISCV_PCREL_LO12_I | R_RISCV_PCREL_LO12_S) => {
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

        let Some(ehdr) = read_unaligned::<Elf32_Ehdr>(data, 0) else {
            Err("cannot read ELF header")?
        };
        let isa = match ehdr.e_machine {
            EM_ARM => Isa::CortexA9,
            EM_RISCV => Isa::RiscV32,
            _ => return Err(Error::Parsing("unsupported architecture")),
        };

        let Some(shdrs) =
            get_ref_slice::<Elf32_Shdr>(data, ehdr.e_shoff as usize, ehdr.e_shnum as usize)
        else {
            Err("cannot read section header table")?
        };

        // Read .strtab
        let strtab_shdr = shdrs[ehdr.e_shstrndx as usize];
        let Some(strtab) =
            get_ref_slice::<u8>(data, strtab_shdr.sh_offset as usize, strtab_shdr.sh_size as usize)
        else {
            Err("cannot read the string table from data")?
        };

        // Read .symtab
        let symtab_shdr = shdrs
            .iter()
            .find(|shdr| shdr.sh_type as usize == SHT_SYMTAB)
            .ok_or(Error::Parsing("cannot find the symbol table"))?;
        let Some(symtab) = get_ref_slice::<Elf32_Sym>(
            data,
            symtab_shdr.sh_offset as usize,
            symtab_shdr.sh_size as usize / mem::size_of::<Elf32_Sym>(),
        ) else {
            Err("cannot read the symbol table from data")?
        };

        // Section table for the .elf paired with the section name
        // To be formalized incrementally
        // Very hashmap-like structure, but the order matters, so it is a vector
        let elf_shdrs = vec![SectionRecord {
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
        }];
        // Debug object also needs a starting NULL record
        let debug_shdrs = vec![SectionRecord {
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
        }];

        let elf_sh_data_off =
            mem::size_of::<Elf32_Ehdr>() + mem::size_of::<Elf32_Phdr>() * ELF_PHNUM;
        let debug_sh_data_off =
            mem::size_of::<Elf32_Ehdr>() + mem::size_of::<Elf32_Phdr>() * DEBUG_PHNUM;

        // Image of the linked dynamic library, to be formalized incrementally
        // just as the section table eventually does
        let dyn_lib_image = Image {
            data: mem::MaybeUninit::uninit(),
            load_offset: elf_sh_data_off as u32,
            shdr_offset: elf_sh_data_off as u32,
        };

        // Debug image
        // Only to the symbolizer for traceback generation
        let debug_image = Image {
            data: mem::MaybeUninit::uninit(),
            load_offset: debug_sh_data_off as u32,
            shdr_offset: debug_sh_data_off as u32,
        };

        // Section relocation table
        // A map of the original index of copied sections to the new sections
        let section_map = HashMap::new();

        // Section relocation table, but for debug sections
        let debug_section_map = HashMap::new();

        // Vector of relocation entries in .rela.dyn
        let rela_dyn_relas = Vec::new();

        let mut linker = Linker {
            isa,
            symtab,
            strtab,
            elf_shdrs,
            debug_shdrs,
            section_map,
            debug_section_map,
            dyn_lib_image,
            debug_image,
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
            data[text_shdr.sh_offset as usize
                ..text_shdr.sh_offset as usize + text_shdr.sh_size as usize]
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
                data[arm_exidx_shdr.sh_offset as usize
                    ..arm_exidx_shdr.sh_offset as usize + arm_exidx_shdr.sh_size as usize]
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
                data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize].to_vec(),
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
                        let Some(relocs) = get_ref_slice::<Elf32_Rela>(
                            data,
                            $shdr.sh_offset as usize,
                            $shdr.sh_size as usize / mem::size_of::<Elf32_Rela>(),
                        ) else {
                            Err("cannot parse relocations")?
                        };

                        #[allow(clippy::redundant_closure_call)]
                        $stmt(relocs)
                    }
                    SHT_REL => {
                        let Some(relocs) = get_ref_slice::<Elf32_Rel>(
                            data,
                            $shdr.sh_offset as usize,
                            $shdr.sh_size as usize / mem::size_of::<Elf32_Rel>(),
                        ) else {
                            Err("cannot parse relocations")?
                        };

                        #[allow(clippy::redundant_closure_call)]
                        $stmt(relocs)
                    }
                    _ => unreachable!(),
                }
            };
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
        rela_dyn_sym_indices.sort_unstable();
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
            let mut sym = linker.symtab[rela_dyn_sym_index as usize];
            let sym_name = name_starting_at_slice(strtab, sym.st_name as usize)
                .map_err(|_| "cannot read symbol name from the original .strtab")?;
            let dynstr_start_index = dynstr.len();

            sym.st_name = dynstr_start_index as Elf32_Word;
            if sym.st_shndx != SHN_UNDEF {
                let elf_shdr_index = linker
                    .section_map
                    .get(&(sym.st_shndx as usize))
                    .copied()
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
            .copied()
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

        for (sym_index, (str_start, str_end)) in
            dynsym_names.iter().enumerate().take(dynsym.len()).skip(1)
        {
            let hash = elf_hash(&dynstr[*str_start..*str_end]);
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
            from_struct_slice(&dynsym),
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
            from_struct_slice(&hash),
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
                    data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize]
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
            linker.load_section(&dynamic_shdr, ".dynamic", from_struct_slice(&dyn_entries));

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
                if shdr.sh_type as usize == SHT_NOBITS {
                    bss_index_vec.push((i, section_name));
                } else {
                    let elf_shdrs_index = linker.load_section(
                        shdr,
                        section_name,
                        data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize]
                            .to_vec(),
                    );
                    linker.section_map.insert(i, elf_shdrs_index);
                }
            }
        }

        let last_w_sec_elf_index = linker.elf_shdrs.len() - 1;

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
            for (bss_iter_index, &(bss_section_index, section_name)) in
                bss_index_vec.iter().enumerate()
            {
                let shdr = &shdrs[bss_section_index];
                let bss_elf_index = linker.load_section(
                    shdr,
                    section_name,
                    vec![0; 0], // NOBITS section has no data
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

        // Load unallocated PROGBITS sections
        // Mainly for debugging symbols
        for (i, shdr) in shdrs.iter().enumerate() {
            if (shdr.sh_type as usize != SHT_PROGBITS)
                || (shdr.sh_flags as usize & SHF_ALLOC == SHF_ALLOC)
            {
                continue;
            }
            let section_name = name_starting_at_slice(strtab, shdr.sh_name as usize)
                .map_err(|_| "cannot read section name")?;
            let elf_shdrs_index = linker.load_debug_section(
                shdr,
                str::from_utf8(section_name).unwrap(),
                data[shdr.sh_offset as usize..(shdr.sh_offset + shdr.sh_size) as usize].to_vec(),
            );
            linker.debug_section_map.insert(i, elf_shdrs_index);
        }

        for shdr in shdrs
            .iter()
            .filter(|shdr| shdr.sh_type as usize == SHT_RELA || shdr.sh_type as usize == SHT_REL)
        {
            reloc_invariant!(shdr, |relocs| linker.resolve_relocatables(relocs, shdr.sh_info))?;
        }

        // Load .rela.dyn symbols generated during relocation
        if rela_dyn_size != 0 {
            let rela_dyn_rec = get_mut_section_by_name!(linker, ".rela.dyn")
                .ok_or(".rela.dyn not initialized in the ELF file")?;
            let rela_dyn_slice =
                to_struct_mut_slice::<Elf32_Rela>(rela_dyn_rec.data.as_mut_slice());

            assert_eq!(linker.rela_dyn_relas.iter().len(), rela_dyn_slice.len());
            for (i, &rela) in linker.rela_dyn_relas.iter().enumerate() {
                rela_dyn_slice[i] = rela;
            }
        }

        // Prepare a STRTAB to hold the names of section headers
        // Fix the sh_name field of the section headers
        let mut shstrtab = Vec::new();
        for shdr_rec in &mut linker.elf_shdrs {
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

        // Same for the debug sections
        let mut debug_shstrtab = Vec::new();
        for shdr_rec in &mut linker.debug_shdrs {
            let shstrtab_index = debug_shstrtab.len();
            debug_shstrtab.extend(shdr_rec.name.as_bytes());
            debug_shstrtab.push(0);
            shdr_rec.shdr.sh_name = shstrtab_index as Elf32_Word;
        }
        // Add en entry for .shstrtab
        let debug_shstrtab_shdr_sh_name = debug_shstrtab.len();
        debug_shstrtab.extend(b".shstrtab");
        debug_shstrtab.push(0);

        let debug_shstrtab_shdr = Elf32_Shdr {
            sh_name: debug_shstrtab_shdr_sh_name as Elf32_Word,
            sh_type: SHT_STRTAB as Elf32_Word,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: 0,
            sh_size: debug_shstrtab.len() as Elf32_Word,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 1,
            sh_entsize: 0,
        };

        let shstrtab_elf_index = linker.load_section(&shstrtab_shdr, ".shstrtab", shstrtab);
        let debug_shstrtab_elf_index =
            linker.load_debug_section(&debug_shstrtab_shdr, ".shstrtab", debug_shstrtab);

        // Edit .eh_frame_hdr content
        if linker.isa == Isa::RiscV32 {
            linker.implement_eh_frame_hdr()?;
        }

        linker.dyn_lib_image.finalize(&linker.elf_shdrs);
        linker.debug_image.finalize(&linker.debug_shdrs);

        // Update the PHDRs
        let phdr_offset = mem::size_of::<Elf32_Ehdr>();
        let phdr_slice: &mut [Elf32_Phdr] =
            linker.dyn_lib_image.get_mut_ref(phdr_offset, ELF_PHNUM);
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
        // According to the specification, regarding PT_LOAD program header when filesz < memsz:
        // The ``extra`` bytes are defined to hold the value 0 and to follow the segment's initialized area.
        //
        // We use this specified behavior to handle NOBITS.
        let w_fsize = last_w_shdr.sh_offset + last_w_shdr.sh_size - first_w_addr;
        let w_msize = end_load_addr - first_w_addr;
        phdr_slice[2] = Elf32_Phdr {
            p_type: PT_LOAD,
            p_offset: first_w_addr as Elf32_Off,
            p_vaddr: first_w_addr as Elf32_Addr,
            p_paddr: first_w_addr as Elf32_Addr,
            p_filesz: w_fsize,
            p_memsz: w_msize,
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

        // Update the EHDR
        let dyn_lib_e_shoff = linker.dyn_lib_image.shdr_offset;
        let ehdr_ptr: &mut [Elf32_Ehdr] = linker.dyn_lib_image.get_mut_ref(0, 1);
        ehdr_ptr[0] = Elf32_Ehdr {
            e_ident: ehdr.e_ident,
            e_type: ET_DYN,
            e_machine: ehdr.e_machine,
            e_version: ehdr.e_version,
            e_entry: elf_sh_data_off as Elf32_Addr,
            e_phoff: phdr_offset as Elf32_Off,
            e_shoff: dyn_lib_e_shoff,
            e_flags: match linker.isa {
                Isa::RiscV32 => ehdr.e_flags,
                Isa::CortexA9 => ehdr.e_flags | EF_ARM_ABI_FLOAT_HARD as Elf32_Word,
            },
            e_ehsize: mem::size_of::<Elf32_Ehdr>() as Elf32_Half,
            e_phentsize: mem::size_of::<Elf32_Phdr>() as Elf32_Half,
            e_phnum: ELF_PHNUM as Elf32_Half,
            e_shentsize: mem::size_of::<Elf32_Shdr>() as Elf32_Half,
            e_shnum: linker.elf_shdrs.len() as Elf32_Half,
            e_shstrndx: shstrtab_elf_index as Elf32_Half,
        };

        let debug_e_shoff = linker.debug_image.shdr_offset;
        let ehdr_ptr: &mut [Elf32_Ehdr] = linker.debug_image.get_mut_ref(0, 1);
        ehdr_ptr[0] = Elf32_Ehdr {
            e_ident: ehdr.e_ident,
            e_type: ET_DYN,
            e_machine: ehdr.e_machine,
            e_version: ehdr.e_version,
            e_entry: debug_sh_data_off as Elf32_Addr,
            e_phoff: phdr_offset as Elf32_Off,
            e_shoff: debug_e_shoff,
            e_flags: match linker.isa {
                Isa::RiscV32 => ehdr.e_flags,
                Isa::CortexA9 => ehdr.e_flags | EF_ARM_ABI_FLOAT_HARD as Elf32_Word,
            },
            e_ehsize: mem::size_of::<Elf32_Ehdr>() as Elf32_Half,
            e_phentsize: mem::size_of::<Elf32_Phdr>() as Elf32_Half,
            e_phnum: DEBUG_PHNUM as Elf32_Half,
            e_shentsize: mem::size_of::<Elf32_Shdr>() as Elf32_Half,
            e_shnum: linker.debug_shdrs.len() as Elf32_Half,
            e_shstrndx: debug_shstrtab_elf_index as Elf32_Half,
        };

        Ok((linker.dyn_lib_image.take(), linker.debug_image.take()))
    }
}
