use inkwell::{AddressSpace, context::Context};

use super::*;
impl RawPointer {
    pub fn ty<'ctx>(ctx: &'ctx Context) -> Type<'ctx, Self> {
        Type::new(ctx.ptr_type(AddressSpace::default()))
    }
}

pub enum Bool {}
type_tag!(Bool : Int, Basic);
impl Bool {
    pub fn ty<'ctx>(ctx: &'ctx Context) -> Type<'ctx, Self> {
        unsafe { Type::transmute_from(ctx.bool_type()) }
    }
}

pub enum I32 {}
type_tag!(I32 : Int, Basic);
impl I32 {
    pub fn ty<'ctx>(ctx: &'ctx Context) -> Type<'ctx, Self> {
        unsafe { Type::transmute_from(ctx.i32_type()) }
    }
}

pub enum I64 {}
type_tag!(I64 : Int, Basic);
impl I64 {
    pub fn ty<'ctx>(ctx: &'ctx Context) -> Type<'ctx, Self> {
        unsafe { Type::transmute_from(ctx.i64_type()) }
    }
}

pub enum Usize {}
type_tag!(Usize : Int, Basic);
impl Usize {
    pub fn new<'ctx>(ctx: &'ctx Context, bits: u32) -> Type<'ctx, Self> {
        let usize_t = match bits {
            32 => ctx.i32_type(),
            64 => ctx.i64_type(),
            _ => panic!("usize has {bits} bits?"),
        };
        unsafe { Type::from_raw_parts(usize_t, ()) }
    }
}
