#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::default_trait_access,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::wildcard_imports
)]

#[macro_use]
extern crate lazy_static;

mod ast_gen;
mod constant;
#[cfg(feature = "fold")]
mod fold_helpers;
mod impls;
mod location;

pub use ast_gen::*;
pub use location::{FileName, Location};

pub type Suite<U = ()> = Vec<Stmt<U>>;
