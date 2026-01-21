#![allow(nonstandard_style)]

use std::{cmp, collections::HashMap, fmt::Error, mem, ptr, slice};

use crate::dwarf::DwarfReader;
use crate::include::dwarf::*;
use crate::include::elf::*;

#[derive(Debug, PartialEq, Clone)]
enum NameRef {
    Concrete(&'static str),
    Abstract(usize),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CallRecord {
    name: NameRef,
    pub address: Option<u32>,
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
    pub dir: Option<&'static str>,
}

impl CallRecord {
    pub fn get_name(&self) -> &'static str {
        if let NameRef::Concrete(name) = self.name {
            name
        } else {
            panic!("name reference should always be finalzied when read")
        }
    }
}

#[derive(Clone, Debug)]
struct AbbreviationEntry {
    tag: DW_TAG,
    has_child: DW_CHILDREN,
    attribute_specs: Vec<(DW_AT, DW_FORM)>,
}

impl AbbreviationEntry {
    // Returns (abbreviation code, entry)
    pub fn new(reader: &mut DwarfReader) -> Option<(u64, Self)> {
        let abbrev_code = reader.read_uleb128();
        if abbrev_code != 0 {
            let mut entry = Self { tag: 0, has_child: 0, attribute_specs: Vec::new() };
            entry.tag = reader.read_uleb128();
            entry.has_child = reader.read_u8();
            let mut attr = (reader.read_uleb128(), reader.read_uleb128());
            while attr != (0, 0) {
                entry.attribute_specs.push(attr);
                attr = (reader.read_uleb128(), reader.read_uleb128());
            }
            Some((abbrev_code, entry))
        } else {
            None
        }
    }
}

// Special section names
#[allow(clippy::struct_field_names)]
struct DebugInfoReader {
    debug_info: &'static [u8],
    debug_abbrev: &'static [u8],
    debug_line: &'static [u8],
    debug_ranges: &'static [u8],
    debug_str: &'static [u8],
}

impl DebugInfoReader {
    // -debug_infos are centralized into a section
    // We should not have partial compile unit
    pub fn search(&self, pc: u32) -> Vec<CallRecord> {
        self.search_compilation_units(DwarfReader::new(self.debug_info, 0), pc)
    }

    fn parse_die_attributes(
        &self,
        reader: &mut DwarfReader,
        attr_specs: &Vec<(DW_AT, DW_FORM)>,
        file_ptrs: &[(&'static str, Option<&'static str>)],
        pc: u32,
        start_addr: u32,
    ) -> (bool, u32, NameRef, Option<u32>, CallRecord) {
        // Compute PC range
        let mut in_range = false;
        let mut low_pc: Option<u32> = None;
        let mut high_pc_relative = false;
        let mut high_pc: Option<u32> = None;
        let mut stmt_list_offset: u32 = 0;
        let mut name_ref: NameRef = NameRef::Unknown;
        let mut call_record = CallRecord {
            // This is different from the `name_ref` variable
            // The call site's name is only resolvable by parent DIE.
            name: NameRef::Unknown,
            address: None,
            line: 0,
            column: 0,
            file: "",
            dir: None,
        };

        for (attr_name, attr_form) in attr_specs {
            match *attr_name {
                DW_AT_low_pc => {
                    assert_eq!(
                        *attr_form, DW_FORM_addr,
                        "DW_AT_low_pc should be specified by an address"
                    );
                    low_pc = Some(reader.read_form_addr());
                }
                DW_AT_high_pc => match *attr_form {
                    DW_FORM_addr => {
                        high_pc = Some(reader.read_form_addr());
                    }
                    DW_FORM_data1 | DW_FORM_data2 | DW_FORM_data4 | DW_FORM_data8
                    | DW_FORM_sdata | DW_FORM_udata => {
                        high_pc_relative = true;
                        high_pc = Some(reader.read_form_constant(*attr_form) as u32);
                    }
                    _ => panic!("DW_AT_high_pc value should either be an address or a constant"),
                },
                DW_AT_ranges => {
                    assert_eq!(
                        *attr_form, DW_FORM_sec_offset,
                        "DW_AT_ranges should be specified by an address"
                    );
                    let debug_ranges_offset = reader.read_form_addr() as usize;
                    let mut range_reader =
                        DwarfReader::new(&self.debug_ranges[debug_ranges_offset..], 0);
                    let mut begin_offset = range_reader.read_u32();
                    let mut end_offset = range_reader.read_u32();
                    while (begin_offset, end_offset) != (0, 0) {
                        if (begin_offset..end_offset).contains(&(pc - start_addr)) {
                            in_range = true;
                            break;
                        }
                        begin_offset = range_reader.read_u32();
                        end_offset = range_reader.read_u32();
                    }
                }
                DW_AT_stmt_list => {
                    assert_eq!(
                        *attr_form, DW_FORM_sec_offset,
                        "DW_AT_ranges should be specified by an address"
                    );
                    stmt_list_offset = reader.read_form_addr();
                }
                DW_AT_name => match *attr_form {
                    DW_FORM_string => {
                        name_ref = unsafe {
                            NameRef::Concrete(mem::transmute::<&'_ str, &'static str>(
                                reader.read_str(),
                            ))
                        };
                    }
                    DW_FORM_strp => {
                        let debug_str_offset = reader.read_form_addr() as usize;
                        let str_head = &self.debug_str[debug_str_offset..];
                        let str_len = str_head
                            .iter()
                            .position(|byte| *byte == 0)
                            .expect("string should be null terminated");
                        name_ref = unsafe {
                            NameRef::Concrete(str::from_utf8_unchecked(&str_head[..str_len]))
                        };
                    }
                    _ => panic!("name should be a string"),
                },
                // Inlined procedure may only invlude an abstract reference to another DIE
                // This replaces DW_AT_name, so we fetch DW_AT_name from that referred entry
                DW_AT_abstract_origin => {
                    assert_eq!(
                        *attr_form, DW_FORM_ref_addr,
                        "DW_AT_abstract_origin should be a pointer to a DIE, only .debug_info DIEs are supported"
                    );
                    let referred_die_addr = reader.read_form_addr() as usize;
                    name_ref = NameRef::Abstract(referred_die_addr);
                }
                DW_AT_call_file => {
                    // call_record.file_idx = reader.read_form_constant(*attr_form) as u32;
                    let file_idx = reader.read_form_constant(*attr_form) as usize;
                    (call_record.file, call_record.dir) = file_ptrs[file_idx - 1];
                }
                DW_AT_call_line => {
                    call_record.line = reader.read_form_constant(*attr_form) as u32;
                }
                DW_AT_call_column => {
                    call_record.column = reader.read_form_constant(*attr_form) as u32;
                }
                _ => {
                    // Unrecognized attributes
                    // They are valid, but we cannot process them
                    reader.skip_form(*attr_form);
                }
            }
        }

        // Determine if pc is within range
        let die_relevant = in_range || {
            low_pc.is_some_and(|low_pc| {
                let high_pc = high_pc.map_or_else(
                    || low_pc + 4,
                    |high_pc| if high_pc_relative { low_pc + high_pc } else { high_pc },
                );
                (low_pc..high_pc).contains(&pc)
            })
        };

        (die_relevant, stmt_list_offset, name_ref, low_pc, call_record)
    }

    fn search_compilation_units(&self, mut reader: DwarfReader, pc: u32) -> Vec<CallRecord> {
        while !reader.slice.is_empty() {
            // 7.5.1.1 Compilation Unit Header
            let unit_length = reader.read_u32();
            let mut next_reader = reader.clone();
            next_reader.offset(unit_length);
            assert_eq!(reader.read_u16(), 4, "expected DWARF version 4");
            let debug_abbrev_offset = reader.read_u32() as usize;
            assert_eq!(reader.read_u8(), 4, "only 32-bit system is supported");

            let abbrev_table = self.parse_abbrev(debug_abbrev_offset);

            // Parse the actual DW_TAG_compile_unit, skip the partially parsed unit if irrelevant
            let compile_unit_abbrev_code = reader.read_uleb128();
            let abbrev_entry = abbrev_table.get(&compile_unit_abbrev_code).expect(
                "all non-zero abbreviation code should be resolvable by the abbreviation table",
            );
            assert_eq!(
                abbrev_entry.tag, DW_TAG_compile_unit,
                "a normal compile unit should start with DW_TAG_compile_unit"
            );

            // FIXME: The start address is called base address in the docs
            //
            // The base address of a compilation unit is defined as the value of the DW_AT_low_pc attribute,
            // if present; otherwise, it is undefined
            let (cu_die_relevant, cu_stmt_list_offset, _cu_name_ref, start_addr, _cu_call_record) =
                self.parse_die_attributes(&mut reader, &abbrev_entry.attribute_specs, &[], pc, 0);

            let (immediate_call_record, file_ptrs) =
                self.parse_line_info(cu_stmt_list_offset as usize, pc);

            let mut call_sites = vec![immediate_call_record];

            if cu_die_relevant {
                self.search_dies(
                    &mut reader,
                    &abbrev_table,
                    &file_ptrs,
                    pc,
                    start_addr.unwrap(),
                    &mut call_sites,
                );

                // Resolve name references
                for rec in &mut call_sites {
                    while let NameRef::Abstract(die_offset) = rec.name {
                        let referred_die = &self.debug_info[die_offset..];
                        let mut referred_reader = DwarfReader::new(referred_die, 0);
                        let abbrev_code = referred_reader.read_uleb128();
                        let abbrev_entry = abbrev_table.get(&abbrev_code).expect("all non-zero abbreviation code should be resolvable by the abbreviation table");
                        let (_die_relevant, _stmt_list_offset, name_ref, _low_pc, _call_record) =
                            self.parse_die_attributes(
                                &mut referred_reader,
                                &abbrev_entry.attribute_specs,
                                &file_ptrs,
                                pc,
                                start_addr.unwrap(),
                            );
                        rec.name = name_ref;
                    }
                    assert!(
                        !(rec.name == NameRef::Unknown),
                        "found a call site with an unknown subroutine name"
                    );
                }

                return call_sites;
            }

            reader = next_reader;
        }

        unreachable!("no relevant debugging info to pc: {}", pc);
    }

    fn search_dies(
        &self,
        reader: &mut DwarfReader,
        abbrev_table: &HashMap<u64, AbbreviationEntry>,
        file_ptrs: &Vec<(&'static str, Option<&'static str>)>,
        pc: u32,
        start_addr: u32,
        call_sites: &mut Vec<CallRecord>,
    ) {
        let mut abbrev_code = reader.read_uleb128();
        while abbrev_code != 0 {
            let abbrev_entry = abbrev_table.get(&abbrev_code).expect(
                "all non-zero abbreviation code should be resolvable by the abbreviation table",
            );
            let (die_relevant, _stmt_list_offset, name_ref, _start_addr, call_record) = self
                .parse_die_attributes(
                    reader,
                    &abbrev_entry.attribute_specs,
                    file_ptrs,
                    pc,
                    start_addr,
                );

            if abbrev_entry.has_child != 0 {
                // Moving directly to its sibling is impossible if there are children
                // The entries are arranged with prefix ordering
                self.search_dies(reader, abbrev_table, file_ptrs, pc, start_addr, call_sites);
            }
            if die_relevant {
                let last_name_ref = call_sites.last_mut().unwrap();
                if last_name_ref.name == NameRef::Unknown {
                    last_name_ref.name = name_ref;
                }
                // Only inlined subprogram has a call record
                if abbrev_entry.tag == DW_TAG_inlined_subroutine {
                    call_sites.push(call_record);
                }
                return;
            }

            abbrev_code = reader.read_uleb128();
        }

        unreachable!("exhausted all debugging informatio entries (DIE)")
    }

    fn parse_abbrev(&self, abbrev_offset: usize) -> HashMap<u64, AbbreviationEntry> {
        let mut reader = DwarfReader::new(&self.debug_abbrev[abbrev_offset..], 0);
        let mut table: HashMap<u64, AbbreviationEntry> = HashMap::new();

        while let Some((code, entry)) = AbbreviationEntry::new(&mut reader) {
            table.insert(code, entry);
        }

        table
    }

    fn parse_line_info(
        &self,
        stmt_list_offset: usize,
        pc: u32,
    ) -> (CallRecord, Vec<(&'static str, Option<&'static str>)>) {
        let mut header_reader = DwarfReader::new(&self.debug_line[stmt_list_offset..], 0);

        // header begins
        let _unit_length = header_reader.read_u32(); // eventually consume all
        assert_eq!(header_reader.read_u16(), 4, "expected DWARF version 4 for .debug_line");
        let header_length = header_reader.read_u32();
        // Create a line program reader
        // So we can find the encoded call site first, then decode it directly
        let mut program_reader = header_reader.clone();
        program_reader.offset(header_length);

        let minimum_instruction_length = u32::from(header_reader.read_u8());
        let maximum_operations_per_instruction = u32::from(header_reader.read_u8());
        let default_is_stmt = header_reader.read_u8();
        let line_base = header_reader.read_i8();
        let line_range = header_reader.read_u8();
        let opcode_base = header_reader.read_u8();
        // var-len array
        // We simply take the reference to the array and index it as necessary
        // However, the array starts with index = 1.
        // There are only opcode_base - 1 elements since opcode 0 is reserved
        // for the preamble of extended opcode.
        let standard_opcode_lengths = unsafe {
            mem::transmute::<&'_ [u8], &'static [u8]>(
                header_reader.read_slice(opcode_base as usize - 1),
            )
        };

        {
            // Standard opcodes, if defined, must match the standard arities
            const MAX_STANDARD_OPCODES: usize = 12;
            const EXPECTED_ARITIES: [u8; MAX_STANDARD_OPCODES] =
                [0, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1];
            let standard_opcode_num = cmp::min(MAX_STANDARD_OPCODES, opcode_base as usize - 1);
            assert_eq!(
                standard_opcode_lengths[standard_opcode_num..],
                EXPECTED_ARITIES[standard_opcode_num..]
            );
        }

        let mut include_directories: Vec<&'static str> = vec![];
        let mut dir_str = header_reader.read_str();
        while !dir_str.is_empty() {
            include_directories.push(unsafe { mem::transmute::<&'_ str, &'static str>(dir_str) });
            dir_str = header_reader.read_str();
        }

        let mut file_ptrs: Vec<(&'static str, Option<&'static str>)> = vec![];
        loop {
            let path_name =
                unsafe { mem::transmute::<&'_ str, &'static str>(header_reader.read_str()) };
            if path_name.is_empty() {
                break;
            }
            let dir_index = header_reader.read_uleb128();
            let dir_name = if dir_index == 0 {
                None
            } else {
                Some(include_directories[dir_index as usize - 1])
            };
            let _last_modified = header_reader.read_uleb128();
            let _file_len = header_reader.read_uleb128();

            file_ptrs.push((path_name, dir_name));
        }

        // We execute the line program first
        // TODO: Register state can be trimmed for non-VLIW targets

        // To clippy: This struct only makes sense here.
        // Why would I care about line program register when not parsing it?
        // Starting a new scope does not sound clearer in my opinion.
        #[allow(clippy::items_after_statements, clippy::struct_excessive_bools)]
        #[derive(Copy, Clone)]
        struct Register {
            address: u32,
            op_index: u32,
            file: u32,
            line: u32,
            column: u32,
            is_stmt: bool,
            basic_block: bool,
            end_sequence: bool,
            prologue_end: bool,
            epilogue_begin: bool,
            isa: u32,
            discriminator: u32,
        }

        // The provisionally accepted entry
        // last_entry.address < (pc - start_addr)
        let mut last_entry = Register {
            address: 0,
            op_index: 0,
            file: 1,
            line: 1,
            column: 0,
            is_stmt: default_is_stmt != 0,
            basic_block: false,
            end_sequence: false,
            prologue_end: false,
            epilogue_begin: false,
            isa: 0,
            discriminator: 0,
        };

        // From spec: the address of each entries in the matrix is strictly increasing
        // It means that we don't need to maintain a var-len matrix, keeping
        // adjacent entries is sufficient.
        let mut curr_entry = last_entry;

        // Macro is avoidable by passing in the predetermined constants
        // But I would not like to pollute the arg list
        macro_rules! advance_address {
            ($operation_advance: ident) => {
                let unadjusted_op_index = curr_entry.op_index + $operation_advance;

                curr_entry.address += minimum_instruction_length
                    * (unadjusted_op_index / maximum_operations_per_instruction);
                curr_entry.op_index += unadjusted_op_index % maximum_operations_per_instruction;
            };
        }

        macro_rules! handle_special_opcode {
            ($opcode: expr, $update_line: literal) => {
                let adjusted_opcode = $opcode - opcode_base;
                let operation_advance = u32::from(adjusted_opcode / line_range);

                advance_address!(operation_advance);

                if $update_line {
                    curr_entry.line = curr_entry
                        .line
                        .wrapping_add_signed(i32::from(line_base))
                        .wrapping_add(u32::from(adjusted_opcode % line_range));
                }
            };
        }

        loop {
            // Decode opcode
            let opcode = program_reader.read_u8();
            match opcode {
                // Extended opcode
                0 => {
                    let insn_len = program_reader.read_uleb128();
                    let mut extended_opcode_reader = program_reader.clone();
                    extended_opcode_reader.slice =
                        &extended_opcode_reader.slice[..insn_len as usize];
                    program_reader.offset(insn_len as u32);

                    let extended_opcode = extended_opcode_reader.read_u8();
                    match extended_opcode {
                        DW_LNE_end_sequence => {
                            // No operands
                            curr_entry.end_sequence = true;

                            // curr_entry is pushed to the matrix
                            // We need to determine if we use curr_entry or last_entry
                            if !(last_entry.address..curr_entry.address).contains(&pc) {
                                last_entry = curr_entry;
                                break;
                            }
                            break;
                        }
                        DW_LNE_set_address => {
                            let address = extended_opcode_reader.read_u32();
                            curr_entry.address = address;
                            curr_entry.op_index = 0;
                        }
                        DW_LNE_define_file => {
                            let path_name = unsafe {
                                mem::transmute::<&'_ str, &'static str>(
                                    extended_opcode_reader.read_str(),
                                )
                            };
                            let dir_index = extended_opcode_reader.read_uleb128();
                            let dir_name = if dir_index == 0 {
                                None
                            } else {
                                Some(include_directories[dir_index as usize - 1])
                            };
                            let _last_modified = extended_opcode_reader.read_uleb128();
                            let _file_len = extended_opcode_reader.read_uleb128();

                            file_ptrs.push((path_name, dir_name));
                        }
                        DW_LNE_set_discriminator => {
                            let discriminator = extended_opcode_reader.read_uleb128() as u32;
                            curr_entry.discriminator = discriminator;
                        }
                        // It is posslbe that other user defined instruction appears
                        // But, we do not support them, nor do we know what to do about them
                        // Hence we simply skip these instructions
                        _ => (),
                    }
                }
                // Standard opcode
                DW_LNS_copy if DW_LNS_copy < opcode_base => {
                    // No operands

                    // This is the moment that we know if we have found the entry
                    // Indicated by the address overtaking pc
                    if (last_entry.address..curr_entry.address).contains(&pc) {
                        break;
                    }

                    // Update last entry otherwise
                    last_entry = curr_entry;
                }
                DW_LNS_advance_pc if DW_LNS_advance_pc < opcode_base => {
                    let operation_advance = program_reader.read_uleb128() as u32;
                    advance_address!(operation_advance);
                }
                DW_LNS_advance_line if DW_LNS_advance_line < opcode_base => {
                    let line_advance = program_reader.read_sleb128();
                    curr_entry.line = curr_entry.line.wrapping_add_signed(line_advance as i32);
                }
                DW_LNS_set_file if DW_LNS_set_file < opcode_base => {
                    let file = program_reader.read_uleb128();
                    curr_entry.file = file as u32;
                }
                DW_LNS_set_column if DW_LNS_set_column < opcode_base => {
                    let column = program_reader.read_uleb128();
                    curr_entry.column = column as u32;
                }
                DW_LNS_negate_stmt if DW_LNS_negate_stmt < opcode_base => {
                    // No operands
                    curr_entry.is_stmt = !curr_entry.is_stmt;
                }
                DW_LNS_set_basic_block if DW_LNS_set_basic_block < opcode_base => {
                    // No operands
                    curr_entry.basic_block = true;
                }
                DW_LNS_const_add_pc if DW_LNS_const_add_pc < opcode_base => {
                    // No operands
                    handle_special_opcode!(0xff, false);
                }
                DW_LNS_fixed_advance_pc if DW_LNS_fixed_advance_pc < opcode_base => {
                    let address_advance = u32::from(program_reader.read_u16());
                    curr_entry.address += address_advance;
                    curr_entry.op_index = 0;
                }
                DW_LNS_set_prologue_end if DW_LNS_set_prologue_end < opcode_base => {
                    // No operands
                    curr_entry.prologue_end = true;
                }
                DW_LNS_set_epilogue_begin if DW_LNS_set_epilogue_begin < opcode_base => {
                    // No operands
                    curr_entry.epilogue_begin = true;
                }
                DW_LNS_set_isa if DW_LNS_set_isa < opcode_base => {
                    let isa = program_reader.read_uleb128() as u32;
                    curr_entry.isa = isa;
                }
                // vendor specific extensions
                _ if (0..opcode_base).contains(&opcode) => {
                    // Skip an appropriate amount of operands
                    for _ in 0..standard_opcode_lengths[opcode as usize] {
                        program_reader.read_uleb128();
                    }
                }
                // Special opcode
                _ if (opcode_base..=0xff).contains(&opcode) => {
                    handle_special_opcode!(opcode, true);
                }

                _ => unreachable!(),
            }
        }

        (
            CallRecord {
                name: NameRef::Unknown,
                address: Some(pc),
                line: last_entry.line,
                file: file_ptrs[last_entry.file as usize - 1].0,
                dir: file_ptrs[last_entry.file as usize - 1].1,
                column: last_entry.column,
            },
            file_ptrs,
        )
    }
}

#[must_use]
pub fn symbolize(elf_byte: &[u8], pc_list: Vec<u32>) -> Vec<CallRecord> {
    let elf_ptr = elf_byte.as_ptr();
    let ehdr = unsafe { ptr::read::<Elf32_Ehdr>(elf_ptr.cast()) };
    let shdrs = unsafe {
        slice::from_raw_parts::<Elf32_Shdr>(
            elf_ptr.add(ehdr.e_shoff as usize).cast(),
            ehdr.e_shnum as usize,
        )
    };

    // Read .strtab
    let strtab = {
        let strtab_shdr = shdrs[ehdr.e_shstrndx as usize];
        unsafe {
            slice::from_raw_parts::<u8>(
                elf_ptr.add(strtab_shdr.sh_offset as usize).cast(),
                strtab_shdr.sh_size as usize,
            )
        }
    };

    let get_str = |str_offset: usize| -> Result<&[u8], Error> {
        let strtab_trimmed = &strtab[str_offset..];
        let str_len = strtab_trimmed
            .iter()
            .position(|&x| x == 0)
            .expect("string in string table should be null-terminated");
        Ok(&strtab_trimmed[..str_len])
    };

    // Retrieve these debug sections from the .elf file
    // .debug_info
    // .debug_line
    // .debug_abbrev
    // .debug_ranges (may)
    // .debug_str

    let debug_info = {
        let debug_info_shdr = shdrs
            .iter()
            .find(|shdr| get_str(shdr.sh_name as usize).unwrap() == b".debug_info")
            .expect("missing .debug_info section");
        unsafe {
            slice::from_raw_parts::<u8>(
                elf_ptr.add(debug_info_shdr.sh_offset as usize),
                debug_info_shdr.sh_size as usize,
            )
        }
    };

    let debug_line = {
        let debug_line_shdr = shdrs
            .iter()
            .find(|shdr| get_str(shdr.sh_name as usize).unwrap() == b".debug_line")
            .expect("missing .debug_line section");
        unsafe {
            slice::from_raw_parts::<u8>(
                elf_ptr.add(debug_line_shdr.sh_offset as usize),
                debug_line_shdr.sh_size as usize,
            )
        }
    };

    let debug_abbrev = {
        let debug_abbrev_shdr = shdrs
            .iter()
            .find(|shdr| get_str(shdr.sh_name as usize).unwrap() == b".debug_abbrev")
            .expect("missing .debug_abbrev section");
        unsafe {
            slice::from_raw_parts::<u8>(
                elf_ptr.add(debug_abbrev_shdr.sh_offset as usize),
                debug_abbrev_shdr.sh_size as usize,
            )
        }
    };

    let debug_ranges = {
        // It is tempting to just cast as slice of u32, but
        // .debug_* sections do not have the concept of alignment
        //
        // TODO: Coerce nac3ld to align the debug sections.
        shdrs
            .iter()
            .find(|shdr| get_str(shdr.sh_name as usize).unwrap() == b".debug_ranges")
            .map_or_else(
                || unsafe { slice::from_raw_parts::<u8>(elf_ptr.cast(), 0) },
                |debug_ranges_shdr| unsafe {
                    slice::from_raw_parts::<u8>(
                        elf_ptr.add(debug_ranges_shdr.sh_offset as usize).cast(),
                        debug_ranges_shdr.sh_size as usize,
                    )
                },
            )
    };

    let debug_str = {
        let debug_str_shdr = shdrs
            .iter()
            .find(|shdr| get_str(shdr.sh_name as usize).unwrap() == b".debug_str")
            .expect("missing .debug_str section");
        unsafe {
            slice::from_raw_parts::<u8>(
                elf_ptr.add(debug_str_shdr.sh_offset as usize).cast(),
                debug_str_shdr.sh_size as usize,
            )
        }
    };

    let info_reader =
        DebugInfoReader { debug_info, debug_abbrev, debug_line, debug_ranges, debug_str };

    let mut call_sites: Vec<CallRecord> = vec![];
    for pc in pc_list {
        call_sites.append(&mut info_reader.search(pc));
    }
    call_sites
}
