//! This crate can be used to parse python sourcecode into a so
//! called AST (abstract syntax tree).
//!
//! The stages involved in this process are lexical analysis and
//! parsing. The lexical analysis splits the sourcecode into
//! tokens, and the parsing transforms those tokens into an AST.
//!
//! For example, one could do this:
//!
//! ```
//! use nac3parser::{parser, ast};
//!
//! let python_source = "print('Hello world')";
//! let python_ast = parser::parse_expression(python_source).unwrap();
//!
//! ```

#![deny(future_incompatible, let_underscore, nonstandard_style, clippy::all)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::enum_glob_use,
    clippy::fn_params_excessive_bools,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines
)]

#[macro_use]
extern crate log;
use lalrpop_util::lalrpop_mod;
pub use nac3ast as ast;

pub mod error;
mod fstring;
mod function;
pub mod lexer;
pub mod mode;
pub mod parser;
lalrpop_mod!(
    #[allow(unused, clippy::all, clippy::pedantic)]
    python
);
pub mod config_comment_helper;
pub mod token;
