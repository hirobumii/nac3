#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic, clippy::nursery)]

#[allow(
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::wildcard_imports,
    clippy::needless_pass_by_value
)]
mod ast_gen;

mod constant;
#[cfg(feature = "fold")]
mod fold_helpers;
mod impls;
mod location;
mod str_ref;

pub use ast_gen::*;
pub use location::{FileName, Location};
pub use str_ref::*;

pub type Suite<U = ()> = Vec<Stmt<U>>;
