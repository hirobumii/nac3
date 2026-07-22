#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::wildcard_imports
)]
#![expect(nonstandard_style)]

mod dwarf;
mod include;
mod linker;
pub mod symbolizer;

pub use linker::Linker;
