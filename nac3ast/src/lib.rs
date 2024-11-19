#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::too_many_lines,
    clippy::wildcard_imports
)]

mod ast_gen;
mod constant;
#[cfg(feature = "fold")]
mod fold_helpers;
mod impls;
mod location;

pub use ast_gen::*;
pub use location::{FileName, Location};

pub type Suite<U = ()> = Vec<Stmt<U>>;
