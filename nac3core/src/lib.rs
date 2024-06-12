#![deny(
    future_incompatible,
    let_underscore,
    nonstandard_style,
    rust_2024_compatibility,
    clippy::all
)]
#![warn(clippy::pedantic)]
#![allow(
    dead_code,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::enum_glob_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

pub mod codegen;
pub mod symbol_resolver;
pub mod toplevel;
pub mod typecheck;
