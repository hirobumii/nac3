#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

// users of nac3core need to use the same version of these dependencies, so expose them as nac3core::*
pub use inkwell;
pub use nac3parser;

pub mod codegen;
pub mod symbol_resolver;
pub mod toplevel;
pub mod typecheck;

extern crate self as nac3core;
