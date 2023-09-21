use std::io;
use std::io::Write;
use std::process::exit;

mod cslice {
    // copied from https://github.com/dherman/cslice
    use std::marker::PhantomData;
    use std::slice;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CSlice<'a, T> {
        base: *const T,
        len: usize,
        marker: PhantomData<&'a ()>,
    }

    impl<'a, T> AsRef<[T]> for CSlice<'a, T> {
        fn as_ref(&self) -> &[T] {
            unsafe { slice::from_raw_parts(self.base, self.len) }
        }
    }
}

#[no_mangle]
pub extern "C" fn output_int32(x: i32) {
    println!("{}", x);
}

#[no_mangle]
pub extern "C" fn output_int64(x: i64) {
    println!("{}", x);
}

#[no_mangle]
pub extern "C" fn output_uint32(x: u32) {
    println!("{}", x);
}

#[no_mangle]
pub extern "C" fn output_uint64(x: u64) {
    println!("{}", x);
}

#[no_mangle]
pub extern "C" fn output_float64(x: f64) {
    // debug output to preserve the digits after the decimal points
    // to match python `print` function
    println!("{:?}", x);
}

#[no_mangle]
pub extern "C" fn output_asciiart(x: i32) {
    let chars = " .,-:;i+hHM$*#@  ";
    if x < 0 {
        println!("");
    } else {
        print!("{}", chars.chars().nth(x as usize).unwrap());
    }
}

#[no_mangle]
pub extern "C" fn output_str(x: &cslice::CSlice<u8>) {
    for e in x.as_ref().iter() {
        print!("{}", char::from(*e));
    }
    println!();
}

#[no_mangle]
pub extern "C" fn output_int32_list(x: &cslice::CSlice<i32>) {
    print!("[");
    let mut it = x.as_ref().iter().peekable();
    while let Some(e) = it.next() {
        if it.peek().is_none() {
            print!("{}", e);
        } else {
            print!("{}, ", e);
        }
    }
    println!("]");
}

#[no_mangle]
pub extern "C" fn __nac3_personality(_state: u32, _exception_object: u32, _context: u32) -> u32 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn __nac3_raise(state: u32, exception_object: u32, context: u32) -> u32 {
    writeln!(io::stderr(),
             "__nac3_raise(state: {:#010x}, exception_object: {:#010x}, context: {:#010x})",
             state,
             exception_object,
             context
    ).unwrap();
    exit(101);
}

extern "C" {
    fn run() -> i32;
}

fn main() {
    unsafe {
        run();
    }
}
