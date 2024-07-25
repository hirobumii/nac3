#![allow(non_camel_case_types)]
#![allow(unused)]

// ARTIQ Exception struct declaration
use cslice::CSlice;

// Note: CSlice within an exception may not be actual cslice, they may be strings that exist only
// in the host. If the length == usize:MAX, the pointer is actually a string key in the host.
#[repr(C)]
#[derive(Clone)]
pub struct Exception<'a> {
    pub id: u32,
    pub file: CSlice<'a, u8>,
    pub line: u32,
    pub column: u32,
    pub function: CSlice<'a, u8>,
    pub message: CSlice<'a, u8>,
    pub param: [i64; 3],
}

fn str_err(_: core::str::Utf8Error) -> core::fmt::Error {
    core::fmt::Error
}

fn exception_str<'a>(s: &'a CSlice<'a, u8>) -> Result<&'a str, core::str::Utf8Error> {
    if s.len() == usize::MAX {
        Ok("<host string>")
    } else {
        core::str::from_utf8(s.as_ref())
    }
}

pub unsafe fn raise(exception: *const Exception) -> ! {
    let e = &*exception;
    let f1 = exception_str(&e.function).map_err(str_err).unwrap();
    let f2 = exception_str(&e.file).map_err(str_err).unwrap();
    let f3 = exception_str(&e.message).map_err(str_err).unwrap();

    panic!("Exception {} from {} in {}:{}:{}, message: {}", e.id, f1, f2, e.line, e.column, f3);
}

static EXCEPTION_ID_LOOKUP: [(&str, u32); 14] = [
    ("RuntimeError", 0),
    ("RTIOUnderflow", 1),
    ("RTIOOverflow", 2),
    ("RTIODestinationUnreachable", 3),
    ("DMAError", 4),
    ("I2CError", 5),
    ("CacheError", 6),
    ("SPIError", 7),
    ("ZeroDivisionError", 8),
    ("IndexError", 9),
    ("UnwrapNoneError", 10),
    ("Value", 11),
    ("ValueError", 12),
    ("LinAlgError", 13),
];

pub fn get_exception_id(name: &str) -> u32 {
    for (n, id) in EXCEPTION_ID_LOOKUP.iter() {
        if *n == name {
            return *id;
        }
    }
    unimplemented!("unallocated internal exception id")
}
