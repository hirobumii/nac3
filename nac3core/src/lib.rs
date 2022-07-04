#![warn(clippy::all)]
#![allow(dead_code)]

#[macro_use]
extern crate lazy_static;

pub mod codegen;
pub mod symbol_resolver;
pub mod toplevel;
pub mod typecheck;
