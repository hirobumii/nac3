#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::enum_glob_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::similar_names,
    clippy::too_many_lines
)]

// users of nac3core need to use the same version of these dependencies, so expose them as nac3core::*
pub use inkwell;
pub use nac3parser;

pub mod codegen;
pub mod symbol_resolver;
pub mod toplevel;
pub mod typecheck;

extern crate self as nac3core;
