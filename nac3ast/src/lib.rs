#[allow(clippy::wildcard_imports, clippy::needless_pass_by_value, clippy::nursery)]
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
